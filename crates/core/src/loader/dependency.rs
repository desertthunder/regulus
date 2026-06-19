use std::path::{Path, PathBuf};
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
}

#[derive(Debug, Deserialize)]
struct PackagesToml {
    #[serde(default)]
    packages: HashMap<String, String>,
}

pub fn load_dependency_interfaces(
    root: &Path, dependencies: &[(String, DependencyToml, bool)], compile_target: target::CompileTarget,
) -> Result<DependencyInterfaces, Diagnostics> {
    let mut progress = None;
    load_dependency_interfaces_with_progress(root, dependencies, compile_target, &mut progress)
}

pub fn load_dependency_interfaces_with_progress(
    root: &Path, dependencies: &[(String, DependencyToml, bool)], compile_target: target::CompileTarget,
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

    if !dependencies.is_empty()
        && let Some(progress) = progress.as_deref_mut()
    {
        progress(ProjectLoadProgress::ResolvingDependencies);
    }

    for (name, dep, _dev) in dependencies {
        if is_registry_backed_dependency(name) {
            continue;
        }

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
        match load_package_sources(package, package_index) {
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

fn is_registry_backed_dependency(name: &str) -> bool {
    name == "gleam_stdlib"
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
        Ok(DependencySourcePackage { package: package.clone(), modules, sources })
    } else {
        Err(diagnostics)
    }
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

    use crate::project::{GleamToml, PackageGraph, PackageNode, Project};
    use crate::source::SourceFile;
    use crate::{ir, types};

    use super::*;

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
    fn registry_backed_stdlib_dependency_does_not_parse_cached_source() {
        let temp = tempfile::tempdir().expect("temp dir");
        let root = temp.path().join("app");
        write(
            &root.join("build/packages/gleam_stdlib/src/gleam/io.gleam"),
            "@external(javascript, \"../gleam_stdlib.mjs\", \"print\")\npub fn print(string: String) -> Nil\n",
        );

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

        assert!(interfaces.packages.is_empty());
        assert!(interfaces.modules.is_empty());
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
    }

    #[test]
    fn snapshots_first_compile_blocker_for_each_upstream_stdlib_module() {
        let report = upstream_stdlib_blocker_report();

        insta::assert_snapshot!(report, @r#"
## dependency metadata
- `gleam/bytes_tree`: ResolveError: module `gleam/string_tree` has no member `StringTree`
    --> file 2000002 bytes 1222..1232
        unknown module member
- `gleam/dict`: ResolveError: duplicate name `Option`
    --> file 2000003 bytes 26..32
        defined again here
    --> file 4294967295 bytes 0..0
        previously defined here
- `gleam/dynamic/decode`: ResolveError: duplicate name `Option`
    --> file 2000004 bytes 9268..9274
        defined again here
    --> file 4294967295 bytes 0..0
        previously defined here
- `gleam/float`: ResolveError: duplicate name `Order`
    --> file 2000006 bytes 1386..1391
        defined again here
    --> file 4294967295 bytes 0..0
        previously defined here
- `gleam/int`: ResolveError: duplicate name `Order`
    --> file 2000008 bytes 522..527
        defined again here
    --> file 4294967295 bytes 0..0
        previously defined here
- `gleam/list`: ResolveError: duplicate name `Order`
    --> file 2000010 bytes 812..817
        defined again here
    --> file 4294967295 bytes 0..0
        previously defined here
- `gleam/option`: ResolveError: duplicate name `Option`
    --> file 2000011 bytes 893..899
        defined again here
    --> file 4294967295 bytes 0..0
        previously defined here
- `gleam/order`: ResolveError: duplicate name `Order`
    --> file 2000012 bytes 115..120
        defined again here
    --> file 4294967295 bytes 0..0
        previously defined here
- `gleam/result`: ResolveError: unknown constructor `Error`
    --> file 2000014 bytes 405..410
        constructor not found
- `gleam/string`: ResolveError: duplicate name `Option`
    --> file 2000016 bytes 166..172
        defined again here
    --> file 4294967295 bytes 0..0
        previously defined here
- `gleam/uri`: ResolveError: duplicate name `Option`
    --> file 2000018 bytes 485..491
        defined again here
    --> file 4294967295 bytes 0..0
        previously defined here

## none
- `gleam/io`: compiles through lowering

## package asset
- `gleam/dynamic`: ResolveError: unknown name `cast`
    --> file 2000005 bytes 2861..2865
        not found in scope
- `gleam/string_tree`: ResolveError: unknown name `from_strings`
    --> file 2000017 bytes 1100..1112
        not found in scope

## source language feature
- `gleam/bool`: LoweringError: function `guard` has generic type `Function { params: [Bool, Generic("a"), Function { params: [], return_type: Generic("a") }], return_type: Generic("a") }` that cannot be lowered without monomorphization
    --> file 2000001 bytes 4663..4668
        generic function type here
- `gleam/function`: LoweringError: function `identity` has generic type `Function { params: [Generic("a")], return_type: Generic("a") }` that cannot be lowered without monomorphization
    --> file 2000007 bytes 75..83
        generic function type here
- `gleam/pair`: LoweringError: function `first` has generic type `Function { params: [Tuple([Generic("a"), Generic("b")])], return_type: Generic("a") }` that cannot be lowered without monomorphization
    --> file 2000013 bytes 128..133
        generic function type here

## target filtering
- `gleam/bit_array`: ResolveError: duplicate name `is_utf8_loop`
    --> file 2000000 bytes 2125..2137
        defined again here
    --> file 2000000 bytes 1961..1973
        previously defined here
- `gleam/set`: ResolveError: duplicate name `Token`
    --> file 2000015 bytes 284..289
        defined again here
    --> file 2000015 bytes 204..209
        previously defined here
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

    fn upstream_stdlib_blocker_report() -> String {
        let package = DependencyPackage {
            name: "gleam_stdlib".to_string(),
            version: Some("1.0.3".to_string()),
            root: published_stdlib_fixture_root(),
            source: DependencySource::Hex,
        };
        let package_sources = load_package_sources(&package, 0).expect("load stdlib source package");

        let mut grouped: BTreeMap<&'static str, Vec<String>> = BTreeMap::new();
        for (module, source) in package_sources
            .modules
            .iter()
            .cloned()
            .zip(package_sources.sources.into_iter())
        {
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
