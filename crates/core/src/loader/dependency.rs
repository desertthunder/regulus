use std::path::{Path, PathBuf};
use std::{collections::HashMap, fs};

use serde::Deserialize;

use crate::diagnostic::{Diagnostic, DiagnosticCode, Diagnostics};
use crate::project::{Dependency, DependencySource, DependencyToml};
use crate::source::{SourceFile, SourceFileId};
use crate::types::ModuleInterface;
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
    pub modules: HashMap<String, ModuleInterface>,
}

#[derive(Debug, Deserialize)]
struct PackagesToml {
    #[serde(default)]
    packages: HashMap<String, String>,
}

pub fn load_dependency_interfaces(
    root: &Path, dependencies: &[(String, DependencyToml, bool)], compile_target: target::CompileTarget,
) -> Result<DependencyInterfaces, Diagnostics> {
    let package_versions = match fs::read_to_string(root.join("build").join("packages").join("packages.toml")) {
        Ok(text) => toml::from_str::<PackagesToml>(&text)
            .map(|packages| packages.packages)
            .unwrap_or_default(),
        Err(_) => HashMap::new(),
    };
    let mut output = DependencyInterfaces::default();
    let mut diagnostics = Vec::new();

    for (name, dep, _dev) in dependencies {
        match dependency_root(root, name, dep) {
            Some(pkg_root) => {
                let version = dep.get_dep_ver(name, &package_versions, &pkg_root);
                let source = DependencySource::from_toml(dep);
                let pkg = DependencyPackage { name: name.clone(), version, root: pkg_root.clone(), source };
                output.packages.push(pkg);
                match load_pkg_interfaces(&pkg_root, compile_target) {
                    Ok(interfaces) => output.modules.extend(interfaces),
                    Err(mut errs) => diagnostics.append(&mut errs),
                }
            }
            None => continue,
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
    pkg_root: &Path, compile_target: target::CompileTarget,
) -> Result<HashMap<String, ModuleInterface>, Diagnostics> {
    let mut paths = Vec::new();
    collect_gleam_files(&pkg_root.join("src"), &mut paths)?;
    paths.sort();

    let mut interfaces = HashMap::new();
    let mut diagnostics = Vec::new();
    for (index, path) in paths.into_iter().enumerate() {
        let source_id = SourceFileId(1_000_000 + index as u32);
        let module_name = path
            .strip_prefix(pkg_root.join("src"))
            .unwrap_or(&path)
            .with_extension("")
            .components()
            .map(|component| component.as_os_str().to_string_lossy())
            .collect::<Vec<_>>()
            .join("/");
        match parse_module(&path, source_id, compile_target) {
            Ok(module) => {
                interfaces.insert(module_name, ModuleInterface::from(&module));
            }
            Err(mut errors) => diagnostics.append(&mut errors),
        }
    }

    if diagnostics.is_empty() { Ok(interfaces) } else { Err(diagnostics) }
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
    use std::fs;
    use std::path::Path;

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
        assert!(interface.functions.contains_key("from_path"));
        assert!(!interface.functions.contains_key("from_hex_cache"));
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
                .functions
                .contains_key("from_absolute_path")
        );
    }

    fn write(path: &Path, contents: &str) {
        fs::create_dir_all(path.parent().expect("parent directory")).expect("create parent directory");
        fs::write(path, contents).expect("write file");
    }
}
