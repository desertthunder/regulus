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
    match dependency.path() {
        Some(path) => {
            let path = PathBuf::from(path);
            return Some(if path.is_absolute() { path } else { root.join(path) });
        }
        None => {
            let hex_root = root.join("build").join("packages").join(name);
            hex_root.is_dir().then_some(hex_root)
        }
    }
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
