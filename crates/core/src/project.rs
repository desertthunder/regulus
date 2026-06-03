use std::{
    collections::{BTreeMap, HashMap},
    fs,
    path::{Path, PathBuf},
};

use serde::Deserialize;

use crate::{
    diagnostic::{Diagnostic, DiagnosticCode, Diagnostics},
    source::{SourceFile, SourceFileId},
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Project {
    pub root: PathBuf,
    pub config: GleamToml,
    pub graph: PackageGraph,
    pub sources: Vec<SourceFile>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PackageGraph {
    pub root_package: PackageNode,
    pub dependencies: Vec<Dependency>,
    pub modules: Vec<ModuleInfo>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PackageNode {
    pub name: String,
    pub version: String,
    pub root: PathBuf,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Dependency {
    pub name: String,
    pub requirement: String,
    pub dev: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModuleInfo {
    pub name: String,
    pub path: PathBuf,
    pub source_id: SourceFileId,
    pub source_root: SourceRoot,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SourceRoot {
    Src,
    Test,
}

#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
pub struct GleamToml {
    pub name: String,
    pub version: String,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub licences: Vec<String>,
    #[serde(default)]
    pub repository: Option<String>,
    #[serde(default)]
    pub links: Vec<Link>,
    #[serde(default)]
    pub gleam: Option<String>,
    #[serde(default)]
    pub target: Option<Target>,
    #[serde(default)]
    pub dependencies: BTreeMap<String, DependencyToml>,
    #[serde(rename = "dev-dependencies", default)]
    pub dev_dependencies: BTreeMap<String, DependencyToml>,
}

#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
pub struct Link {
    pub title: String,
    pub href: String,
}

#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum Target {
    Erlang,
    Javascript,
}

#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
#[serde(untagged)]
pub enum DependencyToml {
    Version(String),
    Options {
        version: Option<String>,
        path: Option<String>,
        git: Option<String>,
    },
}

impl DependencyToml {
    fn requirement(&self) -> String {
        match self {
            DependencyToml::Version(version) => version.clone(),
            DependencyToml::Options { version, path, git } => version
                .clone()
                .or_else(|| path.clone().map(|path| format!("path:{path}")))
                .or_else(|| git.clone().map(|git| format!("git:{git}")))
                .unwrap_or_else(|| "*".into()),
        }
    }
}

pub fn load_project(path: impl AsRef<Path>) -> Result<Project, Diagnostics> {
    let root = project_root(path.as_ref());
    let config = read_config(&root)?;
    let (sources, modules) = discover_modules(&root)?;
    let dependencies = dependencies(&config);

    Ok(Project {
        graph: PackageGraph {
            root_package: PackageNode {
                name: config.name.clone(),
                version: config.version.clone(),
                root: root.clone(),
            },
            dependencies,
            modules,
        },
        root,
        config,
        sources,
    })
}

pub fn source_file(path: impl AsRef<Path>) -> Result<SourceFile, Diagnostics> {
    let path = path.as_ref();
    let text = fs::read_to_string(path).map_err(|error| {
        vec![Diagnostic::new(
            DiagnosticCode::ProjectError,
            format!("could not read {}: {error}", path.display()),
        )]
    })?;
    Ok(SourceFile::with_path(SourceFileId(0), path, text))
}

impl Project {
    pub fn module(&self, name: &str) -> Result<&ModuleInfo, Diagnostics> {
        self.graph
            .modules
            .iter()
            .find(|module| module.name == name)
            .ok_or_else(|| {
                vec![Diagnostic::new(
                    DiagnosticCode::ProjectError,
                    format!("missing module `{name}`"),
                )]
            })
    }
}

fn project_root(path: &Path) -> PathBuf {
    if path.is_file() { path.parent().unwrap_or(path).to_path_buf() } else { path.to_path_buf() }
}

fn read_config(root: &Path) -> Result<GleamToml, Diagnostics> {
    let path = root.join("gleam.toml");
    let text = fs::read_to_string(&path).map_err(|error| {
        vec![Diagnostic::new(
            DiagnosticCode::ProjectError,
            format!("could not read {}: {error}", path.display()),
        )]
    })?;
    toml::from_str(&text).map_err(|error| {
        vec![Diagnostic::new(
            DiagnosticCode::ProjectError,
            format!("could not parse {}: {error}", path.display()),
        )]
    })
}

fn dependencies(config: &GleamToml) -> Vec<Dependency> {
    let normal = config.dependencies.iter().map(|(name, dependency)| Dependency {
        name: name.clone(),
        requirement: dependency.requirement(),
        dev: false,
    });
    let dev = config.dev_dependencies.iter().map(|(name, dependency)| Dependency {
        name: name.clone(),
        requirement: dependency.requirement(),
        dev: true,
    });
    normal.chain(dev).collect()
}

fn discover_modules(root: &Path) -> Result<(Vec<SourceFile>, Vec<ModuleInfo>), Diagnostics> {
    let mut entries = Vec::new();
    collect_gleam_files(root, SourceRoot::Src, &mut entries)?;
    collect_gleam_files(root, SourceRoot::Test, &mut entries)?;
    entries.sort_by(|a, b| a.1.cmp(&b.1));

    let mut seen = HashMap::new();
    let mut sources = Vec::new();
    let mut modules = Vec::new();
    let mut diagnostics = Vec::new();

    for (source_root, path) in entries {
        let source_id = SourceFileId(sources.len() as u32);
        let module_name = module_name(root, source_root, &path);
        if let Some(previous) = seen.insert(module_name.clone(), path.clone()) {
            diagnostics.push(Diagnostic::new(
                DiagnosticCode::ProjectError,
                format!(
                    "duplicate module `{}` in {} and {}",
                    module_name,
                    previous.display(),
                    path.display()
                ),
            ));
            continue;
        }

        let text = fs::read_to_string(&path).map_err(|error| {
            vec![Diagnostic::new(
                DiagnosticCode::ProjectError,
                format!("could not read {}: {error}", path.display()),
            )]
        })?;
        sources.push(SourceFile::with_path(source_id, path.clone(), text));
        modules.push(ModuleInfo { name: module_name, path, source_id, source_root });
    }

    if diagnostics.is_empty() { Ok((sources, modules)) } else { Err(diagnostics) }
}

fn collect_gleam_files(
    root: &Path, source_root: SourceRoot, entries: &mut Vec<(SourceRoot, PathBuf)>,
) -> Result<(), Diagnostics> {
    let dir = match source_root {
        SourceRoot::Src => root.join("src"),
        SourceRoot::Test => root.join("test"),
    };
    if !dir.exists() {
        return Ok(());
    }
    collect_gleam_files_in_dir(source_root, &dir, entries)
}

fn collect_gleam_files_in_dir(
    source_root: SourceRoot, dir: &Path, entries: &mut Vec<(SourceRoot, PathBuf)>,
) -> Result<(), Diagnostics> {
    let read_dir = fs::read_dir(dir).map_err(|error| {
        vec![Diagnostic::new(
            DiagnosticCode::ProjectError,
            format!("could not read directory {}: {error}", dir.display()),
        )]
    })?;

    for entry in read_dir {
        let entry = entry.map_err(|error| {
            vec![Diagnostic::new(
                DiagnosticCode::ProjectError,
                format!("could not read directory entry: {error}"),
            )]
        })?;
        let path = entry.path();
        if path.is_dir() {
            collect_gleam_files_in_dir(source_root, &path, entries)?;
        } else if path.extension().is_some_and(|extension| extension == "gleam") {
            entries.push((source_root, path));
        }
    }

    Ok(())
}

fn module_name(root: &Path, source_root: SourceRoot, path: &Path) -> String {
    let dir = match source_root {
        SourceRoot::Src => root.join("src"),
        SourceRoot::Test => root.join("test"),
    };
    let relative = path.strip_prefix(dir).unwrap_or(path);
    let without_extension = relative.with_extension("");
    without_extension
        .components()
        .map(|component| component.as_os_str().to_string_lossy())
        .collect::<Vec<_>>()
        .join("/")
}

#[cfg(test)]
mod tests {
    use std::fs;

    use tempfile::tempdir;

    use super::*;

    fn write(path: &Path, text: &str) {
        fs::create_dir_all(path.parent().expect("fixture parent")).expect("create fixture dir");
        fs::write(path, text).expect("write fixture");
    }

    #[test]
    fn loads_gleam_toml_and_discovers_modules() {
        let dir = tempdir().expect("tempdir");
        write(
            &dir.path().join("gleam.toml"),
            r#"name = "sample"
version = "1.0.0"
description = "sample project"
licences = ["Apache-2.0"]
target = "javascript"

[dependencies]
gleam_stdlib = ">= 0.44.0 and < 2.0.0"

[dev-dependencies]
gleeunit = ">= 1.0.0 and < 2.0.0"
"#,
        );
        write(&dir.path().join("src/app.gleam"), "pub fn main() { Nil }");
        write(&dir.path().join("src/app/view.gleam"), "pub fn view() { Nil }");

        let project = load_project(dir.path()).expect("load project");

        assert_eq!(project.config.name, "sample");
        assert_eq!(project.graph.modules.len(), 2);
        assert_eq!(project.graph.modules[0].source_id, SourceFileId(0));
        assert!(project.graph.modules.iter().any(|module| module.name == "app/view"));
        assert_eq!(project.graph.dependencies.len(), 2);
    }

    #[test]
    fn reports_duplicate_modules_across_source_roots() {
        let dir = tempdir().expect("tempdir");
        write(
            &dir.path().join("gleam.toml"),
            "name = \"sample\"\nversion = \"1.0.0\"\n",
        );
        write(&dir.path().join("src/app.gleam"), "pub fn main() { Nil }");
        write(&dir.path().join("test/app.gleam"), "pub fn test() { Nil }");

        let diagnostics = load_project(dir.path()).expect_err("duplicate should fail");

        assert_eq!(diagnostics[0].code, DiagnosticCode::ProjectError);
        assert!(diagnostics[0].message.contains("duplicate module `app`"));
    }

    #[test]
    fn reports_missing_modules() {
        let dir = tempdir().expect("tempdir");
        write(
            &dir.path().join("gleam.toml"),
            "name = \"sample\"\nversion = \"1.0.0\"\n",
        );
        write(&dir.path().join("src/app.gleam"), "pub fn main() { Nil }");
        let project = load_project(dir.path()).expect("load project");

        let diagnostics = project.module("missing").expect_err("missing module should fail");

        assert_eq!(diagnostics[0].code, DiagnosticCode::ProjectError);
        assert!(diagnostics[0].message.contains("missing module `missing`"));
    }

    #[test]
    fn keeps_single_file_loading_available() {
        let dir = tempdir().expect("tempdir");
        let path = dir.path().join("main.gleam");
        write(&path, "pub fn main() { Nil }");

        let source = source_file(&path).expect("load source file");

        assert_eq!(source.id, SourceFileId(0));
        assert_eq!(source.path.as_deref(), Some(path.as_path()));
    }
}
