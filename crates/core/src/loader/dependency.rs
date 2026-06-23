use std::path::{Component, Path, PathBuf};
use std::{collections::HashMap, fs};

use serde::Deserialize;

use crate::diagnostic::{Diagnostic, DiagnosticCode, Diagnostics};
use crate::project::{Dependency, DependencySource, DependencyToml, ModuleInfo, ProjectLoadProgress, SourceRoot};
use crate::source::{SourceFile, SourceFileId};
use crate::types::{InterfaceEntry, ModuleInterface};
use crate::{ast, parse, target};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DependencyPackage {
    pub name: String,
    pub version: Option<String>,
    pub root: PathBuf,
    pub source: DependencySource,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct DependencyInterfaces {
    pub packages: Vec<DependencyPackage>,
    pub modules: HashMap<String, InterfaceEntry>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DependencySourcePackage {
    pub package: DependencyPackage,
    pub modules: Vec<ModuleInfo>,
    pub sources: Vec<SourceFile>,
    pub assets: Vec<DependencyPackageAsset>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DependencyPackageAsset {
    pub path: PathBuf,
    pub relative_path: PathBuf,
}

#[derive(Debug, Deserialize)]
struct PackagesToml {
    #[serde(default)]
    packages: HashMap<String, String>,
}

pub fn load_dependency_interfaces(
    root: &Path, deps: &[(String, DependencyToml, bool)], compile_target: target::CompileTarget,
) -> Result<DependencyInterfaces, Diagnostics> {
    let mut progress = None;
    load_dependency_interfaces_with_progress(root, deps, compile_target, &mut progress)
}

pub fn load_dependency_interfaces_with_progress(
    root: &Path, deps: &[(String, DependencyToml, bool)], compile_target: target::CompileTarget,
    progress: &mut Option<&mut dyn FnMut(ProjectLoadProgress)>,
) -> Result<DependencyInterfaces, Diagnostics> {
    let package_versions = match fs::read_to_string(root.join("build").join("packages").join("packages.toml")) {
        Ok(text) => toml::from_str::<PackagesToml>(&text)
            .map(|packages| packages.packages)
            .unwrap_or_default(),
        Err(_) => HashMap::new(),
    };
    let mut output = DependencyInterfaces::default();
    let mut diagnostics = Vec::new();

    if !deps.is_empty()
        && let Some(progress) = progress.as_deref_mut()
    {
        progress(ProjectLoadProgress::ResolvingDependencies);
    }

    for (name, dep, _dev) in deps {
        match dependency_root(root, name, dep) {
            Some(pkg_root) => {
                let version = dep.get_dep_ver(name, &package_versions, &pkg_root);
                let source = DependencySource::from_toml(dep);
                if let Some(progress) = progress.as_deref_mut() {
                    let event = match source {
                        DependencySource::Path | DependencySource::Git => ProjectLoadProgress::UsingPathPackage {
                            name: name.clone(),
                            version: version.clone(),
                            path: pkg_root.clone(),
                        },
                        DependencySource::Hex => ProjectLoadProgress::UsingCachedPackage {
                            name: name.clone(),
                            version: version.clone(),
                            path: pkg_root.clone(),
                        },
                    };
                    progress(event);
                }
                let pkg = DependencyPackage { name: name.clone(), version, root: pkg_root.clone(), source };
                output.packages.push(pkg);
                if is_registry_interface_backed_dependency(name) {
                    continue;
                }
                match load_pkg_interfaces(name, &pkg_root, compile_target) {
                    Ok(interfaces) => output.modules.extend(interfaces),
                    Err(mut errs) => diagnostics.append(&mut errs),
                }
            }
            None => continue,
        }
    }

    if diagnostics.is_empty() { Ok(output) } else { Err(diagnostics) }
}

pub fn load_dependency_sources(packages: &[DependencyPackage]) -> Result<Vec<DependencySourcePackage>, Diagnostics> {
    let mut output = Vec::new();
    let mut diagnostics = Vec::new();

    for (package_index, package) in packages.iter().enumerate() {
        let loaded = if package.name == "gleam_stdlib" {
            load_supported_stdlib_sources(package, package_index)
        } else {
            load_package_sources(package, package_index)
        };
        match loaded {
            Ok(sources) => output.push(sources),
            Err(mut errors) => diagnostics.append(&mut errors),
        }
    }

    if diagnostics.is_empty() { Ok(output) } else { Err(diagnostics) }
}

pub fn dependency_nodes(packages: &[DependencyPackage], configured: Vec<Dependency>) -> Vec<Dependency> {
    configured
        .into_iter()
        .map(|mut dep| {
            if let Some(package) = packages.iter().find(|package| package.name == dep.name) {
                dep.version = package.version.clone();
                dep.root = Some(package.root.clone());
                dep.source = package.source;
            }
            dep
        })
        .collect()
}

fn is_registry_interface_backed_dependency(name: &str) -> bool {
    name == "gleam_stdlib"
}

const SUPPORTED_STDLIB_SOURCE_MODULES: &[&str] = &[
    "gleam/order",
    "gleam/result",
    "gleam/option",
    "gleam/list",
    "gleam/int",
    "gleam/float",
    "gleam/bool",
    "gleam/function",
];

fn load_supported_stdlib_sources(
    package: &DependencyPackage, package_index: usize,
) -> Result<DependencySourcePackage, Diagnostics> {
    let source_id_base = 2_000_000 + (package_index as u32 * 100_000);
    let mut modules = Vec::new();
    let mut sources = Vec::new();
    let mut diagnostics = Vec::new();

    for (module_index, module) in SUPPORTED_STDLIB_SOURCE_MODULES.iter().enumerate() {
        let source_id = SourceFileId(source_id_base + module_index as u32);
        let path = package.root.join("src").join(format!("{module}.gleam"));
        match fs::read_to_string(&path) {
            Ok(text) => {
                modules.push(ModuleInfo {
                    name: (*module).to_string(),
                    path: path.clone(),
                    source_id,
                    source_root: SourceRoot::Src,
                });
                sources.push(SourceFile::with_path(
                    source_id,
                    path,
                    supported_stdlib_module_source(module, &text),
                ));
            }
            Err(error) => diagnostics.push(Diagnostic::new(
                DiagnosticCode::ProjectError,
                format!("could not read dependency source {}: {error}", path.display()),
            )),
        }
    }

    if diagnostics.is_empty() {
        Ok(DependencySourcePackage {
            package: package.clone(),
            modules,
            sources,
            assets: collect_stdlib_package_assets(package)?,
        })
    } else {
        Err(diagnostics)
    }
}

fn supported_stdlib_module_source(module: &str, source: &str) -> String {
    match module {
        "gleam/order" => [slice_between(
            source,
            "/// Represents the result",
            "/// Compares two `Order`",
        )]
        .join("\n"),
        "gleam/bool" => with_bool_compare_source(source),
        "gleam/function" => with_function_helper_source(source),
        "gleam/result" => [
            slice_between(source, "/// Checks whether the result", "/// Merges a nested `Result`"),
            slice_between(source, "/// Extracts the `Ok` value", "/// Combines a list of results"),
            slice_between(
                source,
                "/// Replace the value within a result",
                "/// Given a list of results, returns only",
            ),
            slice_from(source, "pub fn try_recover"),
        ]
        .join("\n"),
        "gleam/option" => {
            let source = strip_external_attributes(source);
            [
                slice_between(&source, "/// `Option` represents", "/// Combines a list of `Option`s"),
                slice_between(
                    &source,
                    "/// Checks whether the `Option`",
                    "/// Merges a nested `Option`",
                ),
                slice_between(&source, "/// Returns the first value", "/// Given a list of `Option`s"),
            ]
            .join("\n")
        }
        "gleam/list" => {
            let source = strip_external_attributes(&remove_imports(source));
            [
                slice_between(
                    &source,
                    "/// Counts the number",
                    "/// Determines whether or not a given element",
                ),
                slice_between(&source, "/// Gets the first element", "/// Groups the elements"),
                slice_between(
                    &source,
                    "/// Returns the given item wrapped",
                    "/// Joins one list onto the end",
                ),
                slice_between(&source, "/// Returns a new list containing", "/// Combines two lists"),
                slice_between(&source, "/// Prefixes an item", "/// Joins a list of lists"),
                slice_between(
                    &source,
                    "/// Reduces a list of elements into a single value by calling a given function\n/// on each element, going from left to right",
                    "/// Reduces a list of elements into a single value by calling a given function\n/// on each element, going from right to left",
                ),
            ]
            .join("\n")
        }
        "gleam/int" => {
            let source = remove_imports(source);
            [
                int_to_string_native_source(),
                slice_between(
                    &source,
                    "/// Returns the absolute value",
                    "/// Returns the result of the base",
                ),
                slice_between(
                    &source,
                    "/// Compares two ints, returning the smaller",
                    "/// Generates a random int",
                ),
                slice_from(&source, "/// Run a function for each int"),
            ]
            .join("\n")
        }
        "gleam/float" => [
            "import gleam/order".to_string(),
            float_to_string_native_source(),
            slice_between(
                source,
                "/// Compares two `Float`s, returning an `Order`",
                "/// Compares two `Float`s within a tolerance",
            )
            .replace(") -> Order", ") -> order.Order"),
            slice_between(
                source,
                "/// Compares two `Float`s, returning the smaller",
                "/// Rounds the value to the next highest",
            ),
            slice_between(source, "/// Returns the negative", "/// Sums a list"),
            slice_between(
                &remove_imports(source),
                "/// Adds two floats together",
                "/// Returns the natural logarithm",
            ),
        ]
        .join("\n"),
        _ => source.to_string(),
    }
}

fn with_bool_compare_source(source: &str) -> String {
    [
        "import gleam/order".to_string(),
        source.to_string(),
        r#"pub fn compare(a: Bool, with b: Bool) -> order.Order {
  case a == b {
    True -> order.Eq
    False ->
      case a {
        False -> order.Lt
        True -> order.Gt
      }
  }
}"#
        .to_string(),
    ]
    .join("\n")
}

fn with_function_helper_source(source: &str) -> String {
    [
        source.to_string(),
        r#"pub fn constant(value: a, _argument: b) -> a {
  value
}

pub fn compose(outer: fn(b) -> c, inner: fn(a) -> b) -> fn(a) -> c {
  fn(value) { outer(inner(value)) }
}

pub fn flip(function: fn(a, b) -> c) -> fn(b, a) -> c {
  fn(second, first) { function(first, second) }
}"#
        .to_string(),
    ]
    .join("\n")
}

fn int_to_string_native_source() -> String {
    r#"external fn __regulus_int_to_string(x: Int) -> String =
  "__regulus_native" "int_to_string"

pub fn to_string(x: Int) -> String {
  __regulus_int_to_string(x)
}"#
    .to_string()
}

fn float_to_string_native_source() -> String {
    r#"external fn __regulus_float_to_string(x: Float) -> String =
  "__regulus_native" "float_to_string"

pub fn to_string(x: Float) -> String {
  __regulus_float_to_string(x)
}"#
    .to_string()
}

fn slice_between(source: &str, start: &str, end: &str) -> String {
    let start_index = source.find(start).expect("stdlib source slice start");
    let end_index = source[start_index..]
        .find(end)
        .map(|index| start_index + index)
        .expect("stdlib source slice end");
    source[start_index..end_index].trim().to_string()
}

fn slice_from(source: &str, start: &str) -> String {
    let start_index = source.find(start).expect("stdlib source slice start");
    source[start_index..].trim().to_string()
}

fn remove_imports(source: &str) -> String {
    source
        .lines()
        .filter(|line| !line.trim_start().starts_with("import "))
        .collect::<Vec<_>>()
        .join("\n")
}

fn strip_external_attributes(source: &str) -> String {
    source
        .lines()
        .filter(|line| !line.trim_start().starts_with("@external("))
        .collect::<Vec<_>>()
        .join("\n")
}

fn load_package_sources(
    package: &DependencyPackage, package_index: usize,
) -> Result<DependencySourcePackage, Diagnostics> {
    let src_root = package.root.join("src");
    let mut paths = Vec::new();
    collect_gleam_files(&src_root, &mut paths)?;
    paths.sort();

    let source_id_base = 2_000_000 + (package_index as u32 * 100_000);
    let mut modules = Vec::new();
    let mut sources = Vec::new();
    let mut diagnostics = Vec::new();
    for (module_index, path) in paths.into_iter().enumerate() {
        let source_id = SourceFileId(source_id_base + module_index as u32);
        let name = module_name_from_path(&src_root, &path);
        match fs::read_to_string(&path) {
            Ok(text) => {
                if package.name == "gleam_stdlib" {
                    validate_stdlib_js_assets(package, &path, source_id, &text, &mut diagnostics);
                }
                modules.push(ModuleInfo { name, path: path.clone(), source_id, source_root: SourceRoot::Src });
                sources.push(SourceFile::with_path(source_id, &path, text));
            }
            Err(error) => diagnostics.push(Diagnostic::new(
                DiagnosticCode::ProjectError,
                format!("could not read dependency source {}: {error}", path.display()),
            )),
        }
    }

    if diagnostics.is_empty() {
        Ok(DependencySourcePackage {
            package: package.clone(),
            modules,
            sources,
            assets: collect_stdlib_package_assets(package)?,
        })
    } else {
        Err(diagnostics)
    }
}

fn collect_stdlib_package_assets(package: &DependencyPackage) -> Result<Vec<DependencyPackageAsset>, Diagnostics> {
    if package.name != "gleam_stdlib" {
        return Ok(Vec::new());
    }
    let src_root = package.root.join("src");
    let mut paths = Vec::new();
    collect_package_asset_files(&src_root, &mut paths)?;
    paths.sort();
    Ok(paths
        .into_iter()
        .map(|path| DependencyPackageAsset {
            relative_path: path.strip_prefix(&package.root).unwrap_or(&path).to_path_buf(),
            path,
        })
        .collect())
}

fn collect_package_asset_files(dir: &Path, paths: &mut Vec<PathBuf>) -> Result<(), Diagnostics> {
    if !dir.exists() {
        return Ok(());
    }
    let read_dir = fs::read_dir(dir).map_err(|error| {
        vec![Diagnostic::new(
            DiagnosticCode::ProjectError,
            format!("could not read dependency asset directory {}: {error}", dir.display()),
        )]
    })?;
    for entry in read_dir {
        let entry = entry.map_err(|error| {
            vec![Diagnostic::new(
                DiagnosticCode::ProjectError,
                format!("could not read dependency asset directory entry: {error}"),
            )]
        })?;
        let path = entry.path();
        if path.is_dir() {
            collect_package_asset_files(&path, paths)?;
        } else if path.extension().is_some_and(|extension| extension == "mjs") {
            paths.push(path);
        }
    }
    Ok(())
}

fn validate_stdlib_js_assets(
    package: &DependencyPackage, source_path: &Path, source_id: SourceFileId, text: &str, diagnostics: &mut Diagnostics,
) {
    let source = SourceFile::with_path(source_id, source_path, text.to_string());
    let Ok(cst) = parse::parse(source) else {
        return;
    };
    let Ok(module) = ast::build(&cst) else {
        return;
    };

    for declaration in &module.declarations {
        validate_stdlib_js_assets_in_declaration(package, source_path, declaration, diagnostics);
    }
}

fn validate_stdlib_js_assets_in_declaration(
    package: &DependencyPackage, source_path: &Path, declaration: &ast::Declaration, diagnostics: &mut Diagnostics,
) {
    match declaration {
        ast::Declaration::ExternalFunction(function) => {
            let is_javascript = function
                .body
                .target
                .as_ref()
                .is_some_and(|target| target.text == "javascript");
            if !is_javascript {
                return;
            }
            let module = crate::shared::unquote(&function.body.module.source);
            if !module.starts_with("../") {
                return;
            }
            if !module.ends_with(".mjs") {
                diagnostics.push(
                    Diagnostic::spanned(
                        DiagnosticCode::ProjectError,
                        format!(
                            "stdlib JS external module `{module}` is not a `.mjs` package asset"
                        ),
                        function.body.module.span,
                        "invalid stdlib JS asset here",
                    )
                    .with_note("package-relative JavaScript externals are currently allowed only for `gleam_stdlib` `.mjs` assets"),
                );
                return;
            }

            let Some(parent) = source_path.parent() else {
                return;
            };
            let asset_path = normalize_path(parent.join(&module));
            let package_root = normalize_path(&package.root);
            if !asset_path.starts_with(&package_root) {
                diagnostics.push(
                    Diagnostic::spanned(
                        DiagnosticCode::ProjectError,
                        format!("stdlib JS external module `{module}` escapes the package root"),
                        function.body.module.span,
                        "invalid stdlib JS asset here",
                    )
                    .with_note("stdlib package assets must resolve inside the loaded `gleam_stdlib` package root"),
                );
                return;
            }
            if !asset_path.is_file() {
                diagnostics.push(
                    Diagnostic::spanned(
                        DiagnosticCode::ProjectError,
                        format!("stdlib JS external module `{module}` does not resolve to a package asset"),
                        function.body.module.span,
                        "missing stdlib JS asset here",
                    )
                    .with_note(format!("expected package asset at {}", asset_path.display())),
                );
            }
        }
        ast::Declaration::TargetGroup(group) => {
            for declaration in &group.declarations {
                validate_stdlib_js_assets_in_declaration(package, source_path, declaration, diagnostics);
            }
        }
        _ => {}
    }
}

fn normalize_path(path: impl AsRef<Path>) -> PathBuf {
    let mut normalized = PathBuf::new();
    for component in path.as_ref().components() {
        match component {
            Component::CurDir => {}
            Component::ParentDir => {
                normalized.pop();
            }
            _ => normalized.push(component.as_os_str()),
        }
    }
    normalized
}

fn dependency_root(root: &Path, name: &str, dependency: &DependencyToml) -> Option<PathBuf> {
    path_dependency_root(root, dependency).or_else(|| {
        let hex_root = root.join("build").join("packages").join(name);
        hex_root.is_dir().then_some(hex_root)
    })
}

fn path_dependency_root(root: &Path, dependency: &DependencyToml) -> Option<PathBuf> {
    dependency.path().map(|path| {
        let path = PathBuf::from(path);
        let path = if path.is_absolute() { path } else { root.join(path) };
        path.canonicalize().unwrap_or(path)
    })
}

fn load_pkg_interfaces(
    package: &str, pkg_root: &Path, compile_target: target::CompileTarget,
) -> Result<HashMap<String, InterfaceEntry>, Diagnostics> {
    let mut paths = Vec::new();
    collect_gleam_files(&pkg_root.join("src"), &mut paths)?;
    paths.sort();

    let mut interfaces = HashMap::new();
    let mut diagnostics = Vec::new();
    for (index, path) in paths.into_iter().enumerate() {
        let source_id = SourceFileId(1_000_000 + index as u32);
        let module_name = module_name_from_path(&pkg_root.join("src"), &path);
        match parse_module(&path, source_id, compile_target) {
            Ok(module) => {
                let interface = ModuleInterface::from(&module);
                interfaces.insert(
                    module_name.clone(),
                    InterfaceEntry::new(package, module_name, interface),
                );
            }
            Err(mut errors) => diagnostics.append(&mut errors),
        }
    }

    if diagnostics.is_empty() { Ok(interfaces) } else { Err(diagnostics) }
}

fn module_name_from_path(src_root: &Path, path: &Path) -> String {
    path.strip_prefix(src_root)
        .unwrap_or(path)
        .with_extension("")
        .components()
        .map(|component| component.as_os_str().to_string_lossy())
        .collect::<Vec<_>>()
        .join("/")
}

fn parse_module(
    path: &Path, source_id: SourceFileId, compile_target: target::CompileTarget,
) -> Result<ast::Module, Diagnostics> {
    let text = fs::read_to_string(path).map_err(|error| {
        vec![Diagnostic::new(
            DiagnosticCode::ProjectError,
            format!("could not read dependency source {}: {error}", path.display()),
        )]
    })?;
    parse::parse(SourceFile::with_path(source_id, path, text))
        .and_then(|cst| ast::build(&cst))
        .and_then(|module| target::select_module(module, compile_target))
}

fn collect_gleam_files(dir: &Path, paths: &mut Vec<PathBuf>) -> Result<(), Diagnostics> {
    if !dir.exists() {
        return Ok(());
    }
    let read_dir = fs::read_dir(dir).map_err(|error| {
        vec![Diagnostic::new(
            DiagnosticCode::ProjectError,
            format!("could not read dependency directory {}: {error}", dir.display()),
        )]
    })?;
    for entry in read_dir {
        let entry = entry.map_err(|error| {
            vec![Diagnostic::new(
                DiagnosticCode::ProjectError,
                format!("could not read dependency directory entry: {error}"),
            )]
        })?;
        let path = entry.path();
        if path.is_dir() {
            collect_gleam_files(&path, paths)?;
        } else if path.extension().is_some_and(|extension| extension == "gleam") {
            paths.push(path);
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::collections::{BTreeMap, HashMap};
    use std::fs;
    use std::path::{Path, PathBuf};

    use super::*;
    use crate::project::{GleamToml, PackageGraph, PackageNode, Project};
    use crate::source::SourceFile;
    use crate::{
        ir,
        stdlib::StdlibRegistry,
        types::{self, Type},
    };
    use wasmtime::{Engine, Instance, Memory as WasmtimeMemory, Module, Store};

    #[test]
    fn path_dependency_source_is_loaded_from_declared_path() {
        let temp = tempfile::tempdir().expect("temp dir");
        let root = temp.path().join("app");
        let path_dep = temp.path().join("path_dep");
        write(
            &path_dep.join("gleam.toml"),
            "name = \"path_dep\"\nversion = \"2.0.0\"\n",
        );
        write(
            &path_dep.join("src/path_dep.gleam"),
            "pub fn from_path() -> Int { 1 }\n",
        );
        write(
            &root.join("build/packages/path_dep/src/path_dep.gleam"),
            "pub fn from_hex_cache() -> Int { 1 }\n",
        );

        let interfaces = load_dependency_interfaces(
            &root,
            &[(
                "path_dep".to_string(),
                DependencyToml::Options { version: None, path: Some("../path_dep".to_string()), git: None },
                false,
            )],
            target::CompileTarget::Wasmtime,
        )
        .expect("dependency interfaces");

        assert_eq!(interfaces.packages.len(), 1);
        assert_eq!(interfaces.packages[0].source, DependencySource::Path);
        assert_eq!(
            interfaces.packages[0].root,
            path_dep.canonicalize().expect("canonical path dep")
        );
        assert_eq!(interfaces.packages[0].version.as_deref(), Some("2.0.0"));
        let interface = interfaces.modules.get("path_dep").expect("path_dep interface");
        assert_eq!(interface.package, "path_dep");
        assert_eq!(interface.module, "path_dep");
        assert!(interface.interface.functions.contains_key("from_path"));
        assert!(!interface.interface.functions.contains_key("from_hex_cache"));
    }

    #[test]
    fn registry_backed_stdlib_dependency_loads_supported_source_by_default() {
        let root = published_stdlib_fixture_root()
            .parent()
            .and_then(Path::parent)
            .and_then(Path::parent)
            .expect("scalar app root")
            .to_path_buf();

        let interfaces = load_dependency_interfaces(
            &root,
            &[(
                "gleam_stdlib".to_string(),
                DependencyToml::Version(">= 0.44.0 and < 2.0.0".to_string()),
                false,
            )],
            target::CompileTarget::Wasmtime,
        )
        .expect("dependency interfaces");

        assert_eq!(interfaces.packages.len(), 1);
        assert_eq!(interfaces.packages[0].name, "gleam_stdlib");
        assert!(interfaces.modules.is_empty());

        let sources = load_dependency_sources(&interfaces.packages).expect("dependency source modules");
        assert_eq!(sources.len(), 1);
        assert_eq!(
            sources[0]
                .modules
                .iter()
                .map(|module| module.name.as_str())
                .collect::<Vec<_>>(),
            SUPPORTED_STDLIB_SOURCE_MODULES,
        );
        assert_eq!(
            sources[0]
                .assets
                .iter()
                .map(|asset| asset.relative_path.to_string_lossy().to_string())
                .collect::<Vec<_>>(),
            vec!["src/dict.mjs", "src/gleam_stdlib.mjs"],
        );
    }

    #[test]
    fn dependency_source_loader_discovers_selected_package_modules() {
        let temp = tempfile::tempdir().expect("temp dir");
        let root = temp.path().join("app");
        fs::create_dir_all(&root).expect("root dir");
        let path_dep = temp.path().join("source_dep");
        write(&path_dep.join("src/dep/foo.gleam"), "pub fn answer() -> Int { 1 }\n");
        write(&path_dep.join("src/dep/bar.gleam"), "pub fn value() -> Int { 2 }\n");

        let interfaces = load_dependency_interfaces(
            &root,
            &[(
                "source_dep".to_string(),
                DependencyToml::Options {
                    version: Some("1.0.0".to_string()),
                    path: Some("../source_dep".to_string()),
                    git: None,
                },
                false,
            )],
            target::CompileTarget::Wasmtime,
        )
        .expect("dependency interfaces");
        let sources = load_dependency_sources(&interfaces.packages).expect("dependency source modules");

        assert_eq!(sources.len(), 1);
        assert_eq!(sources[0].package.name, "source_dep");
        assert_eq!(
            sources[0]
                .modules
                .iter()
                .map(|module| module.name.as_str())
                .collect::<Vec<_>>(),
            vec!["dep/bar", "dep/foo"],
        );
        assert_eq!(sources[0].sources.len(), 2);
    }

    #[test]
    fn published_stdlib_source_fixture_loads_as_dependency_package() {
        let root = published_stdlib_fixture_root();
        let package = DependencyPackage {
            name: "gleam_stdlib".to_string(),
            version: Some("1.0.3".to_string()),
            root: root.clone(),
            source: DependencySource::Hex,
        };

        let package_sources = load_package_sources(&package, 0).expect("load stdlib source package");

        assert_eq!(package_sources.package.name, "gleam_stdlib");
        assert_eq!(package_sources.package.root, root);
        assert_eq!(
            package_sources
                .modules
                .iter()
                .map(|module| module.name.as_str())
                .collect::<Vec<_>>(),
            vec![
                "gleam/bit_array",
                "gleam/bool",
                "gleam/bytes_tree",
                "gleam/dict",
                "gleam/dynamic/decode",
                "gleam/dynamic",
                "gleam/float",
                "gleam/function",
                "gleam/int",
                "gleam/io",
                "gleam/list",
                "gleam/option",
                "gleam/order",
                "gleam/pair",
                "gleam/result",
                "gleam/set",
                "gleam/string",
                "gleam/string_tree",
                "gleam/uri",
            ],
        );
        assert_eq!(package_sources.sources.len(), 19);
        assert_eq!(
            package_sources
                .assets
                .iter()
                .map(|asset| asset.relative_path.to_string_lossy().to_string())
                .collect::<Vec<_>>(),
            vec!["src/dict.mjs", "src/gleam_stdlib.mjs"],
        );
    }

    #[test]
    fn validates_upstream_stdlib_relative_js_assets() {
        let root = published_stdlib_fixture_root();
        let package = DependencyPackage {
            name: "gleam_stdlib".to_string(),
            version: Some("1.0.3".to_string()),
            root,
            source: DependencySource::Hex,
        };

        let package_sources = load_package_sources(&package, 0).expect("load stdlib source package");

        assert!(package_sources.sources.iter().any(|source| {
            source
                .text
                .contains(r#"@external(javascript, "../gleam_stdlib.mjs", "to_string")"#)
        }));
        assert!(
            package_sources
                .sources
                .iter()
                .any(|source| { source.text.contains(r#"@external(javascript, "../dict.mjs", "make")"#) })
        );
        assert!(package_sources.assets.iter().any(|asset| {
            asset.relative_path == *"src/gleam_stdlib.mjs"
                && asset.path == package_sources.package.root.join("src/gleam_stdlib.mjs")
        }));
        assert!(package_sources.assets.iter().any(|asset| {
            asset.relative_path == *"src/dict.mjs" && asset.path == package_sources.package.root.join("src/dict.mjs")
        }));
    }

    #[test]
    fn reports_missing_upstream_stdlib_relative_js_asset() {
        let temp = tempfile::tempdir().expect("temp dir");
        let root = temp.path().join("gleam_stdlib");
        write(
            &root.join("src/gleam/bad.gleam"),
            r#"@external(javascript, "../missing.mjs", "run")
pub fn run() -> Int
"#,
        );
        let package = DependencyPackage {
            name: "gleam_stdlib".to_string(),
            version: Some("1.0.3".to_string()),
            root,
            source: DependencySource::Path,
        };

        let diagnostics = load_package_sources(&package, 0).expect_err("missing asset should fail");

        assert_eq!(
            diagnostics[0].message,
            "stdlib JS external module `../missing.mjs` does not resolve to a package asset"
        );
        assert!(diagnostics[0].labels.iter().any(|label| {
            label.message.as_deref() == Some("missing stdlib JS asset here")
                && label.span.start == 22
                && label.span.end == 38
        }));
        assert!(
            diagnostics[0]
                .notes
                .iter()
                .any(|note| note.ends_with("gleam_stdlib/src/missing.mjs"))
        );
    }

    #[test]
    fn snapshots_first_compile_blocker_for_each_upstream_stdlib_module() {
        let report = upstream_stdlib_blocker_report();

        insta::assert_snapshot!(report, @r#"
## dependency metadata
- `gleam/bytes_tree`: ResolveError: unknown module `gleam/bit_array`
    --> file 2000002 bytes 1157..1172
        module not found
- `gleam/dict`: ResolveError: unknown module `gleam/option`
    --> file 2000003 bytes 7..19
        module not found
- `gleam/dynamic/decode`: ResolveError: unknown module `gleam/bit_array`
    --> file 2000004 bytes 9121..9136
        module not found
- `gleam/float`: ResolveError: unknown module `gleam/order`
    --> file 2000006 bytes 1368..1379
        module not found
- `gleam/int`: ResolveError: unknown module `gleam/float`
    --> file 2000008 bytes 485..496
        module not found
- `gleam/list`: ResolveError: unknown module `gleam/dict`
    --> file 2000010 bytes 728..738
        module not found
- `gleam/result`: ResolveError: unknown module `gleam/list`
    --> file 2000014 bytes 152..162
        module not found
- `gleam/string`: ResolveError: unknown module `gleam/list`
    --> file 2000016 bytes 129..139
        module not found
- `gleam/uri`: ResolveError: unknown module `gleam/int`
    --> file 2000018 bytes 431..440
        module not found

## none
- `gleam/bool`: compiles through lowering
- `gleam/function`: compiles through lowering
- `gleam/io`: compiles through lowering
- `gleam/pair`: compiles through lowering

## package asset
- `gleam/dynamic`: ResolveError: unknown module `gleam/dict`
    --> file 2000005 bytes 7..17
        module not found
- `gleam/option`: LoweringError: external function `reverse` parameter 1 uses unsupported ABI shape `List(Generic("a"))`
    --> file 2000011 bytes 1741..1748
        unsupported external ABI shape here
    note: host import `lists.reverse` must use concrete scalar values, managed values, or Nil returns
- `gleam/string_tree`: ResolveError: unknown module `gleam/list`
    --> file 2000017 bytes 7..17
        module not found

## source language feature
- `gleam/order`: TypeError: case branch is unreachable
    --> file 2000012 bytes 1178..1188
        unreachable branch

## target filtering
- `gleam/bit_array`: ResolveError: unknown module `gleam/int`
    --> file 2000000 bytes 68..77
        module not found
- `gleam/set`: ResolveError: unknown module `gleam/dict`
    --> file 2000015 bytes 7..17
        module not found
"#);
    }

    #[test]
    fn dependency_package_module_compiles_through_project_pipeline() {
        let temp = tempfile::tempdir().expect("temp dir");
        let root = temp.path().join("app");
        fs::create_dir_all(&root).expect("root dir");
        let path_dep = temp.path().join("compile_dep");
        write(&path_dep.join("src/dep/foo.gleam"), "pub fn answer() -> Int { 1 }\n");

        let interfaces = load_dependency_interfaces(
            &root,
            &[(
                "compile_dep".to_string(),
                DependencyToml::Options {
                    version: Some("1.0.0".to_string()),
                    path: Some("../compile_dep".to_string()),
                    git: None,
                },
                false,
            )],
            target::CompileTarget::Wasmtime,
        )
        .expect("dependency interfaces");
        let mut sources = load_dependency_sources(&interfaces.packages).expect("dependency source modules");
        let package_sources = sources.pop().expect("one dependency source package");

        let project = Project {
            root: package_sources.package.root.clone(),
            config: GleamToml {
                name: package_sources.package.name.clone(),
                version: package_sources
                    .package
                    .version
                    .clone()
                    .unwrap_or_else(|| "1.0.0".to_string()),
                description: None,
                licences: Vec::new(),
                repository: None,
                links: Vec::new(),
                gleam: None,
                target: None,
                dependencies: BTreeMap::new(),
                dev_dependencies: BTreeMap::new(),
            },
            compile_target: target::CompileTarget::Wasmtime,
            graph: PackageGraph {
                root_package: PackageNode {
                    name: package_sources.package.name.clone(),
                    version: package_sources
                        .package
                        .version
                        .clone()
                        .unwrap_or_else(|| "1.0.0".to_string()),
                    root: package_sources.package.root.clone(),
                },
                dependencies: Vec::new(),
                dependency_interfaces: HashMap::new(),
                dependency_sources: Vec::new(),
                modules: package_sources.modules,
            },
            sources: package_sources.sources,
        };

        let typed = types::check_project(&project).expect("type check dependency package");
        let lowered = ir::lower_project(typed).expect("lower dependency package");

        assert_eq!(lowered.functions.len(), 1);
        assert!(lowered.linked_debug_dump().contains("dep/foo.answer"));
    }

    #[test]
    fn compiles_upstream_gleam_pair_source_without_registry_entry() {
        let package_sources = upstream_pair_source_package();
        let project = project_from_dependency_source_package(package_sources);

        let typed = types::check_project(&project).expect("type check upstream pair");
        let lowered = ir::lower_project(typed).expect("lower upstream pair");

        assert_eq!(lowered.functions.len(), 6);
        assert!(lowered.linked_debug_dump().contains("gleam_stdlib:gleam/pair.first"));
        assert!(StdlibRegistry::new().module("gleam/pair").is_none());
    }

    #[test]
    fn root_project_imports_gleam_pair_from_compiled_dependency_source() {
        let package_sources = upstream_pair_source_package();
        let pair_interface = interface_from_source(package_sources.sources[0].clone());
        let mut dependency_interfaces = HashMap::new();
        dependency_interfaces.insert(
            "gleam/pair".to_string(),
            InterfaceEntry::new("gleam_stdlib", "gleam/pair", pair_interface),
        );
        let root = package_sources.package.root.join("__regulus_pair_proof");
        let source = SourceFile::with_path(
            SourceFileId(0),
            root.join("src/app.gleam"),
            "import gleam/pair\n\npub fn main() -> Int {\n  pair.first(#(1, \"one\"))\n}\n",
        );
        let project = Project {
            root: root.clone(),
            config: GleamToml {
                name: "pair_proof".to_string(),
                version: "1.0.0".to_string(),
                description: None,
                licences: Vec::new(),
                repository: None,
                links: Vec::new(),
                gleam: None,
                target: None,
                dependencies: BTreeMap::new(),
                dev_dependencies: BTreeMap::new(),
            },
            compile_target: target::CompileTarget::Wasmtime,
            graph: PackageGraph {
                root_package: PackageNode { name: "pair_proof".to_string(), version: "1.0.0".to_string(), root },
                dependencies: Vec::new(),
                dependency_interfaces,
                dependency_sources: vec![package_sources],
                modules: vec![ModuleInfo {
                    name: "app".to_string(),
                    path: PathBuf::from("src/app.gleam"),
                    source_id: SourceFileId(0),
                    source_root: SourceRoot::Src,
                }],
            },
            sources: vec![source],
        };

        let typed = types::check_project(&project).expect("type check project using upstream pair");
        let lowered = ir::lower_project(typed).expect("lower project using upstream pair");
        let dump = lowered.linked_debug_dump();

        assert!(dump.contains("gleam_stdlib:gleam/pair.first"));
        assert!(dump.contains("pair_proof:app.main"));
        assert!(!dump.contains("__stdlib_gleam_pair_first"));
    }

    #[test]
    fn compiles_pure_portions_of_upstream_stdlib_modules() {
        let package_sources = pure_stdlib_source_package(&[
            "gleam/order",
            "gleam/result",
            "gleam/option",
            "gleam/list",
            "gleam/int",
            "gleam/float",
            "gleam/bool",
            "gleam/function",
        ]);
        let project = project_from_dependency_source_package(package_sources);

        let typed = types::check_project(&project).expect("type check pure stdlib source portions");
        let lowered = ir::lower_project(typed).expect("lower pure stdlib source portions");
        let dump = lowered.linked_debug_dump();

        for module in [
            "gleam/order",
            "gleam/result",
            "gleam/option",
            "gleam/list",
            "gleam/int",
            "gleam/float",
            "gleam/bool",
            "gleam/function",
        ] {
            assert!(dump.contains(&format!("gleam_stdlib:{module}.")), "{dump}");
        }
        for function in [
            "gleam_stdlib:gleam/bool.compare",
            "gleam_stdlib:gleam/bool.negate",
            "gleam_stdlib:gleam/bool.to_string",
            "gleam_stdlib:gleam/float.compare",
            "gleam_stdlib:gleam/float.max",
            "gleam_stdlib:gleam/float.min",
            "gleam_stdlib:gleam/float.negate",
            "gleam_stdlib:gleam/float.to_string",
            "gleam_stdlib:gleam/function.compose",
            "gleam_stdlib:gleam/function.constant",
            "gleam_stdlib:gleam/function.flip",
            "gleam_stdlib:gleam/function.identity",
            "gleam_stdlib:gleam/int.to_string",
            "gleam_stdlib:gleam/list.fold",
            "gleam_stdlib:gleam/list.length",
            "gleam_stdlib:gleam/list.map",
            "gleam_stdlib:gleam/list.reverse",
            "gleam_stdlib:gleam/option.map",
            "gleam_stdlib:gleam/result.map",
        ] {
            assert!(dump.contains(function), "{dump}");
        }
        assert!(!dump.contains("__stdlib_gleam_order"));
        assert!(!dump.contains("__stdlib_gleam_result"));
        assert!(!dump.contains("__stdlib_gleam_option"));
        assert!(!dump.contains("__stdlib_gleam_list"));
        assert!(!dump.contains("__stdlib_gleam_int"));
        assert!(!dump.contains("__stdlib_gleam_float"));
        assert!(!dump.contains("__stdlib_gleam_bool"));
        assert!(!dump.contains("__stdlib_gleam_function"));
        assert!(
            lowered
                .js_externals
                .iter()
                .any(|external| { external.module == "../gleam_stdlib.mjs" && external.name == "to_string" })
        );
        assert!(
            lowered
                .js_externals
                .iter()
                .any(|external| { external.module == "../gleam_stdlib.mjs" && external.name == "float_to_string" })
        );
    }

    #[test]
    fn upstream_anything_native_externals_keep_anything_annotations() {
        let dynamic_cast = upstream_external_function_type("gleam/dynamic", "cast", target::CompileTarget::Browser);
        assert_eq!(
            dynamic_cast,
            Type::Function {
                params: vec![Type::Anything],
                return_type: Box::new(Type::Custom { name: "Dynamic".into(), args: Vec::new() }),
            }
        );

        let bare_index =
            upstream_external_function_type("gleam/dynamic/decode", "bare_index", target::CompileTarget::Browser);
        assert_eq!(
            bare_index,
            Type::Function {
                params: vec![
                    Type::Custom { name: "Dynamic".into(), args: Vec::new() },
                    Type::Anything
                ],
                return_type: Box::new(Type::Custom {
                    name: "Result".into(),
                    args: vec![
                        Type::Custom {
                            name: "Option".into(),
                            args: vec![Type::Custom { name: "Dynamic".into(), args: Vec::new() }],
                        },
                        Type::String,
                    ],
                }),
            }
        );

        let inspect = upstream_external_function_type("gleam/string", "do_inspect", target::CompileTarget::Browser);
        assert_eq!(
            inspect,
            Type::Function {
                params: vec![Type::Anything],
                return_type: Box::new(Type::Custom { name: "StringTree".into(), args: Vec::new() }),
            }
        );
    }

    #[test]
    fn links_source_backed_stdlib_calls_without_runtime_dispatch() {
        let package_sources = pure_stdlib_source_package(&[
            "gleam/order",
            "gleam/result",
            "gleam/option",
            "gleam/list",
            "gleam/int",
            "gleam/float",
            "gleam/bool",
            "gleam/function",
        ]);
        let project = project_using_stdlib_source_package(
            package_sources,
            r#"import gleam/bool
import gleam/float
import gleam/function
import gleam/int
import gleam/list
import gleam/option.{Some}
import gleam/order
import gleam/result.{Ok, Error}

pub fn bool_negated() -> Bool { bool.negate(False) }
pub fn bool_text_matches() -> Bool { bool.to_string(True) == "True" }
pub fn bool_rank() -> Int {
  case bool.compare(False, True) {
    order.Lt -> -1
    order.Eq -> 0
    order.Gt -> 1
  }
}

pub fn float_larger() -> Float { float.max(1.5, float.negate(-2.5)) }
pub fn float_smaller() -> Float { float.min(1.5, 2.5) }
pub fn float_text_matches() -> Bool { float.to_string(1.5) == "1.5" }

pub fn float_rank() -> Int {
  case float.compare(1.0, 2.0) {
    order.Lt -> -1
    order.Eq -> 0
    order.Gt -> 1
  }
}

pub fn same_value() -> Int { function.identity(9) }
pub fn constant_value() -> Int { function.constant(7, "ignored") }
pub fn int_text_matches() -> Bool { int.to_string(-42) == "-42" }
pub fn item_count() -> Int { list.length([1, 2, 3]) }

pub fn composed() -> Int {
  let add1 = fn(x) { x + 1 }
  let double = fn(x) { x * 2 }
  let f = function.compose(add1, double)
  f(4)
}

pub fn flipped() -> Int {
  let sub = fn(a, b) { a - b }
  let f = function.flip(sub)
  f(3, 10)
}

pub fn reversed_head() -> Int {
  case list.reverse([1, 2, 3]) {
    [head, ..] -> head
    _ -> 0
  }
}

pub fn mapped_head() -> Int {
  list.fold(list.map([1], fn(x) { x + 1 }), 0, fn(acc, x) { acc + x })
}

pub fn folded() -> Int {
  list.fold([1, 2, 3], 0, fn(acc, x) { acc + x })
}

pub fn option_mapped() -> Int {
  case option.map(Some(4), fn(x) { x + 3 }) {
    Some(x) -> x
    _ -> 0
  }
}

pub fn result_mapped() -> Int {
  case result.map(Ok(4), fn(x) { x + 5 }) {
    Ok(x) -> x
    Error(e) -> e
  }
}
"#,
        );

        let typed = types::check_project(&project).expect("type check source-backed stdlib calls");
        let lowered = ir::lower_project(typed).expect("lower source-backed stdlib calls");
        let dump = lowered.linked_debug_dump();
        for function in [
            "gleam_stdlib:gleam/bool.compare",
            "gleam_stdlib:gleam/bool.negate",
            "gleam_stdlib:gleam/bool.to_string",
            "gleam_stdlib:gleam/float.compare",
            "gleam_stdlib:gleam/float.max",
            "gleam_stdlib:gleam/float.min",
            "gleam_stdlib:gleam/float.negate",
            "gleam_stdlib:gleam/float.to_string",
            "gleam_stdlib:gleam/function.compose",
            "gleam_stdlib:gleam/function.constant",
            "gleam_stdlib:gleam/function.flip",
            "gleam_stdlib:gleam/function.identity",
            "gleam_stdlib:gleam/int.to_string",
            "gleam_stdlib:gleam/list.fold",
            "gleam_stdlib:gleam/list.length",
            "gleam_stdlib:gleam/list.map",
            "gleam_stdlib:gleam/list.reverse",
            "gleam_stdlib:gleam/option.map",
            "gleam_stdlib:gleam/result.map",
        ] {
            assert!(dump.contains(function), "{dump}");
        }
        assert!(!dump.contains("__stdlib_gleam_"), "{dump}");
    }

    #[test]
    fn runs_monomorphized_source_backed_stdlib_helpers_in_wasmtime() {
        let package_sources =
            behavior_stdlib_source_package(&["gleam/order", "gleam/function", "gleam/int", "gleam/float"]);
        let project = project_using_stdlib_source_package(
            package_sources,
            // TODO: this should be an embedded file with include_str!
            r#"import gleam/float
import gleam/function
import gleam/int

pub fn identity_int() -> Int {
  function.identity(4)
}

pub fn identity_text() -> String {
  function.identity("ok")
}

pub fn int_text() -> String {
  int.to_string(-42)
}

pub fn float_text() -> String {
  float.to_string(1.5)
}
"#,
        );

        let typed = types::check_project(&project).expect("type check monomorphized stdlib helpers");
        let lowered = ir::lower_project(typed).expect("lower monomorphized stdlib helpers");
        let wasm = lowered.emit_wasm().expect("emit Wasm for monomorphized stdlib helpers");
        assert!(wasm.wat.contains("(module"), "{}", wasm.wat);
        assert!(!wasm.wat.contains("__stdlib_gleam_list_map"), "{}", wasm.wat);
        assert!(!wasm.wat.contains("__stdlib_gleam_option_map"), "{}", wasm.wat);
        assert!(!wasm.wat.contains("__stdlib_gleam_result_map"), "{}", wasm.wat);

        let (mut store, instance, memory) = instantiate_wasm(&wasm.bytes);
        for (name, expected) in [("identity_int", 4)] {
            assert_eq!(call_i64_export(&instance, &mut store, name), expected, "{name}");
        }
        assert_eq!(
            call_string_export(&instance, &mut store, &memory, "identity_text"),
            "ok"
        );
        assert_eq!(call_string_export(&instance, &mut store, &memory, "int_text"), "-42");
        assert_eq!(
            call_string_export(&instance, &mut store, &memory, "float_text"),
            "1.500000"
        );
    }

    #[test]
    fn reports_unsupported_dependency_specialization_shape_before_wasm_emission() {
        let source = SourceFile::new(
            SourceFileId(1),
            r#"pub fn expose() -> anything {
  panic as "unsupported"
}
"#,
        );
        let package_sources = DependencySourcePackage {
            package: DependencyPackage {
                name: "gleam_stdlib".to_string(),
                version: Some("1.0.3".to_string()),
                root: PathBuf::new(),
                source: DependencySource::Path,
            },
            modules: vec![ModuleInfo {
                name: "gleam/unsupported".to_string(),
                path: PathBuf::from("src/gleam/unsupported.gleam"),
                source_id: source.id,
                source_root: SourceRoot::Src,
            }],
            sources: vec![source],
            assets: Vec::new(),
        };
        let project = project_using_stdlib_source_package(
            package_sources,
            r#"import gleam/unsupported

pub fn main() {
  unsupported.expose()
  Nil
}
"#,
        );
        let typed = types::check_project(&project).expect("type check project");
        let errors = ir::lower_project(typed).expect_err("unsupported specialization should fail lowering");

        insta::assert_snapshot!(errors[0].render_plain(), @r#"
LoweringError: dependency specialization `gleam_stdlib:gleam/unsupported.expose` uses unsupported type `fn() -> anything`
  --> file 1 bytes 7..13
      unsupported dependency specialization shape here
  note: dependency specializations must have concrete internal runtime shapes before Wasm emission
"#);
    }

    #[test]
    fn absolute_path_dependency_source_is_loaded_directly() {
        let temp = tempfile::tempdir().expect("temp dir");
        let root = temp.path().join("app");
        let path_dep = temp.path().join("absolute_dep");
        write(
            &path_dep.join("src/absolute_dep.gleam"),
            "pub fn from_absolute_path() -> Int { 1 }\n",
        );

        let interfaces = load_dependency_interfaces(
            &root,
            &[(
                "absolute_dep".to_string(),
                DependencyToml::Options {
                    version: Some("1.2.3".to_string()),
                    path: Some(path_dep.to_string_lossy().to_string()),
                    git: None,
                },
                false,
            )],
            target::CompileTarget::Wasmtime,
        )
        .expect("dependency interfaces");

        assert_eq!(
            interfaces.packages[0].root,
            path_dep.canonicalize().expect("canonical path dep")
        );
        assert_eq!(interfaces.packages[0].version.as_deref(), Some("1.2.3"));
        assert!(
            interfaces
                .modules
                .get("absolute_dep")
                .expect("absolute_dep interface")
                .interface
                .functions
                .contains_key("from_absolute_path")
        );
    }

    fn write(path: &Path, contents: &str) {
        fs::create_dir_all(path.parent().expect("parent directory")).expect("create parent directory");
        fs::write(path, contents).expect("write file");
    }

    fn published_stdlib_fixture_root() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/projects/scalar_app/build/packages/gleam_stdlib")
    }

    fn upstream_pair_source_package() -> DependencySourcePackage {
        let package = DependencyPackage {
            name: "gleam_stdlib".to_string(),
            version: Some("1.0.3".to_string()),
            root: published_stdlib_fixture_root(),
            source: DependencySource::Hex,
        };
        let package_sources = load_package_sources(&package, 0).expect("load stdlib source package");
        let Some((index, module)) = package_sources
            .modules
            .iter()
            .enumerate()
            .find(|(_, module)| module.name == "gleam/pair")
        else {
            panic!("published stdlib fixture should contain gleam/pair");
        };

        DependencySourcePackage {
            package,
            modules: vec![module.clone()],
            sources: vec![package_sources.sources[index].clone()],
            assets: package_sources.assets,
        }
    }

    fn pure_stdlib_source_package(modules: &[&str]) -> DependencySourcePackage {
        let package = DependencyPackage {
            name: "gleam_stdlib".to_string(),
            version: Some("1.0.3".to_string()),
            root: published_stdlib_fixture_root(),
            source: DependencySource::Hex,
        };
        let mut module_infos = Vec::new();
        let mut sources = Vec::new();
        for (index, module) in modules.iter().enumerate() {
            let source_id = SourceFileId(2_500_000 + index as u32);
            let path = package.root.join("src").join(format!("{module}.gleam"));
            module_infos.push(ModuleInfo {
                name: (*module).to_string(),
                path: path.clone(),
                source_id,
                source_root: SourceRoot::Src,
            });
            sources.push(SourceFile::with_path(
                source_id,
                path,
                pure_stdlib_module_source(module),
            ));
        }
        DependencySourcePackage { package, modules: module_infos, sources, assets: Vec::new() }
    }

    fn behavior_stdlib_source_package(modules: &[&str]) -> DependencySourcePackage {
        let package = DependencyPackage {
            name: "gleam_stdlib".to_string(),
            version: Some("1.0.3".to_string()),
            root: published_stdlib_fixture_root(),
            source: DependencySource::Hex,
        };
        let mut module_infos = Vec::new();
        let mut sources = Vec::new();
        for (index, module) in modules.iter().enumerate() {
            let source_id = SourceFileId(2_600_000 + index as u32);
            let path = package.root.join("src").join(format!("{module}.gleam"));
            let source = pure_stdlib_module_source(module);
            module_infos.push(ModuleInfo {
                name: (*module).to_string(),
                path: path.clone(),
                source_id,
                source_root: SourceRoot::Src,
            });
            sources.push(SourceFile::with_path(source_id, path, source));
        }
        DependencySourcePackage { package, modules: module_infos, sources, assets: Vec::new() }
    }

    fn pure_stdlib_module_source(module: &str) -> String {
        assert!(
            SUPPORTED_STDLIB_SOURCE_MODULES.contains(&module),
            "no pure stdlib source fixture for {module}"
        );
        let source = upstream_stdlib_module_source(module);
        supported_stdlib_module_source(module, &source)
    }

    fn upstream_stdlib_module_source(module: &str) -> String {
        fs::read_to_string(
            published_stdlib_fixture_root()
                .join("src")
                .join(format!("{module}.gleam")),
        )
        .expect("read upstream stdlib source")
    }

    fn upstream_external_function_type(
        module: &str, function_name: &str, compile_target: target::CompileTarget,
    ) -> Type {
        let source = SourceFile::with_path(
            SourceFileId(2_900_000),
            published_stdlib_fixture_root()
                .join("src")
                .join(format!("{module}.gleam")),
            upstream_stdlib_module_source(module),
        );
        let cst = parse::parse(source).expect("parse upstream stdlib source");
        let module = ast::build(&cst).expect("build upstream stdlib ast");
        let module = target::select_module(module, compile_target).expect("select upstream stdlib target");
        find_external_function_type(&module.declarations, function_name)
            .unwrap_or_else(|| panic!("upstream stdlib function `{function_name}` should exist"))
    }

    fn find_external_function_type(declarations: &[ast::Declaration], function_name: &str) -> Option<Type> {
        for declaration in declarations {
            match declaration {
                ast::Declaration::ExternalFunction(function) if function.name.text == function_name => {
                    let params = function
                        .parameters
                        .iter()
                        .map(|parameter| {
                            parameter
                                .type_annotation
                                .as_ref()
                                .and_then(|annotation| Type::from_source(&annotation.source))
                        })
                        .collect::<Option<Vec<_>>>()?;
                    let return_type = Type::from_source(&function.return_type.source)?;
                    return Some(Type::Function { params, return_type: Box::new(return_type) });
                }
                ast::Declaration::TargetGroup(group) => {
                    if let Some(type_) = find_external_function_type(&group.declarations, function_name) {
                        return Some(type_);
                    }
                }
                _ => {}
            }
        }
        None
    }

    fn project_from_dependency_source_package(package_sources: DependencySourcePackage) -> Project {
        Project {
            root: package_sources.package.root.clone(),
            config: GleamToml {
                name: package_sources.package.name.clone(),
                version: package_sources
                    .package
                    .version
                    .clone()
                    .unwrap_or_else(|| "1.0.3".to_string()),
                description: None,
                licences: Vec::new(),
                repository: None,
                links: Vec::new(),
                gleam: None,
                target: None,
                dependencies: BTreeMap::new(),
                dev_dependencies: BTreeMap::new(),
            },
            compile_target: target::CompileTarget::Wasmtime,
            graph: PackageGraph {
                root_package: PackageNode {
                    name: package_sources.package.name.clone(),
                    version: package_sources
                        .package
                        .version
                        .clone()
                        .unwrap_or_else(|| "1.0.3".to_string()),
                    root: package_sources.package.root.clone(),
                },
                dependencies: Vec::new(),
                dependency_interfaces: HashMap::new(),
                dependency_sources: Vec::new(),
                modules: package_sources.modules,
            },
            sources: package_sources.sources,
        }
    }

    fn project_using_stdlib_source_package(package_sources: DependencySourcePackage, source: &str) -> Project {
        let mut dependency_interfaces = HashMap::new();
        let registry = StdlibRegistry::new();
        for (module, source) in package_sources.modules.iter().zip(package_sources.sources.iter()) {
            let interface = registry
                .interface(&module.name)
                .cloned()
                .unwrap_or_else(|| interface_from_source(source.clone()));
            dependency_interfaces.insert(
                module.name.clone(),
                InterfaceEntry::new(package_sources.package.name.clone(), module.name.clone(), interface),
            );
        }

        let root = package_sources.package.root.join("__regulus_stdlib_source_proof");
        let source = SourceFile::with_path(SourceFileId(0), root.join("src/app.gleam"), source);
        Project {
            root: root.clone(),
            config: GleamToml {
                name: "stdlib_source_proof".to_string(),
                version: "1.0.0".to_string(),
                description: None,
                licences: Vec::new(),
                repository: None,
                links: Vec::new(),
                gleam: None,
                target: None,
                dependencies: BTreeMap::new(),
                dev_dependencies: BTreeMap::new(),
            },
            compile_target: target::CompileTarget::Wasmtime,
            graph: PackageGraph {
                root_package: PackageNode {
                    name: "stdlib_source_proof".to_string(),
                    version: "1.0.0".to_string(),
                    root,
                },
                dependencies: Vec::new(),
                dependency_interfaces,
                dependency_sources: vec![package_sources],
                modules: vec![ModuleInfo {
                    name: "app".to_string(),
                    path: PathBuf::from("src/app.gleam"),
                    source_id: SourceFileId(0),
                    source_root: SourceRoot::Src,
                }],
            },
            sources: vec![source],
        }
    }

    fn instantiate_wasm(bytes: &[u8]) -> (Store<()>, Instance, WasmtimeMemory) {
        let engine = Engine::default();
        let module = Module::new(&engine, bytes).expect("compile wasm module");
        let mut store = Store::new(&engine, ());
        let instance = Instance::new(&mut store, &module, &[]).expect("instantiate module");
        let memory = instance.get_memory(&mut store, "memory").expect("memory export");
        (store, instance, memory)
    }

    fn call_i64_export(instance: &Instance, store: &mut Store<()>, name: &str) -> i64 {
        instance
            .get_typed_func::<(), i64>(&mut *store, name)
            .unwrap_or_else(|_| panic!("get {name} export"))
            .call(store, ())
            .unwrap_or_else(|error| panic!("call {name}: {error}"))
    }

    fn call_i32_export(instance: &Instance, store: &mut Store<()>, name: &str) -> i32 {
        instance
            .get_typed_func::<(), i32>(&mut *store, name)
            .unwrap_or_else(|_| panic!("get {name} export"))
            .call(store, ())
            .unwrap_or_else(|error| panic!("call {name}: {error}"))
    }

    fn call_string_export(instance: &Instance, store: &mut Store<()>, memory: &WasmtimeMemory, name: &str) -> String {
        let ptr = call_i32_export(instance, store, name) as usize;
        let mut header = [0; 8];
        memory.read(&*store, ptr, &mut header).expect("read string header");
        assert_eq!(u32::from_le_bytes(header[0..4].try_into().unwrap()), 1);
        let len = u32::from_le_bytes(header[4..8].try_into().unwrap()) as usize;
        let mut bytes = vec![0; len];
        memory.read(&*store, ptr + 8, &mut bytes).expect("read string data");
        String::from_utf8(bytes).expect("utf-8 string")
    }

    fn interface_from_source(source: SourceFile) -> ModuleInterface {
        let cst = parse::parse(source).expect("parse dependency source interface");
        let module = ast::build(&cst).expect("build dependency source interface");
        ModuleInterface::from(&module)
    }

    fn upstream_stdlib_blocker_report() -> String {
        let package = DependencyPackage {
            name: "gleam_stdlib".to_string(),
            version: Some("1.0.3".to_string()),
            root: published_stdlib_fixture_root(),
            source: DependencySource::Hex,
        };
        let package_sources = load_package_sources(&package, 0).expect("load stdlib source package");

        let mut grouped: BTreeMap<&'static str, Vec<String>> = BTreeMap::new();
        for (module, source) in package_sources.modules.iter().cloned().zip(package_sources.sources) {
            let blocker = first_compile_blocker(&package_sources.package, module.clone(), source);
            grouped.entry(blocker.category).or_default().push(format!(
                "- `{}`: {}",
                module.name,
                blocker.message.replace('\n', "\n  ")
            ));
        }

        grouped
            .into_iter()
            .map(|(category, blockers)| format!("## {category}\n{}", blockers.join("\n")))
            .collect::<Vec<_>>()
            .join("\n\n")
    }

    struct CompileBlocker {
        category: &'static str,
        message: String,
    }

    fn first_compile_blocker(package: &DependencyPackage, module: ModuleInfo, source: SourceFile) -> CompileBlocker {
        let module_name = module.name.clone();
        let project = Project {
            root: package.root.clone(),
            config: GleamToml {
                name: package.name.clone(),
                version: package.version.clone().unwrap_or_else(|| "1.0.3".to_string()),
                description: None,
                licences: Vec::new(),
                repository: None,
                links: Vec::new(),
                gleam: None,
                target: None,
                dependencies: BTreeMap::new(),
                dev_dependencies: BTreeMap::new(),
            },
            compile_target: target::CompileTarget::Wasmtime,
            graph: PackageGraph {
                root_package: PackageNode {
                    name: package.name.clone(),
                    version: package.version.clone().unwrap_or_else(|| "1.0.3".to_string()),
                    root: package.root.clone(),
                },
                dependencies: Vec::new(),
                dependency_interfaces: HashMap::new(),
                dependency_sources: Vec::new(),
                modules: vec![module],
            },
            sources: vec![source],
        };

        match types::check_project(&project).and_then(ir::lower_project) {
            Ok(_) => CompileBlocker { category: "none", message: "compiles through lowering".to_string() },
            Err(diagnostics) => {
                let diagnostic = diagnostics
                    .first()
                    .expect("compile blocker diagnostics should not be empty");
                CompileBlocker {
                    category: blocker_category(&module_name, &diagnostic.render_plain()),
                    message: diagnostic.render_plain(),
                }
            }
        }
    }

    fn blocker_category(module: &str, diagnostic: &str) -> &'static str {
        if matches!(module, "gleam/bit_array" | "gleam/set")
            || diagnostic.contains("unsupported target")
            || diagnostic.contains("target group")
            || diagnostic.contains("target `")
        {
            "target filtering"
        } else if matches!(module, "gleam/dynamic" | "gleam/string_tree")
            || diagnostic.contains("external")
            || diagnostic.contains(".mjs")
        {
            "package asset"
        } else if diagnostic.contains("host call") || diagnostic.contains("ABI") {
            "host ABI"
        } else if diagnostic.contains("wasm") || diagnostic.contains("runtime") || diagnostic.contains("stdlib member")
        {
            "runtime primitive"
        } else if diagnostic.contains("unknown module")
            || diagnostic.contains("unknown constructor")
            || diagnostic.contains("duplicate name `Option`")
            || diagnostic.contains("duplicate name `Order`")
            || diagnostic.contains("file 4294967295")
            || diagnostic.contains("dependency")
            || diagnostic.contains("package")
        {
            "dependency metadata"
        } else {
            "source language feature"
        }
    }
}
