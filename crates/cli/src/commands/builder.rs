use std::fs;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use compiler_core::project::{ProjectLoadOptions, ProjectLoadProgress};
use compiler_core::types::TypedProject;
use compiler_core::{ir, wasm};

use crate::args::{self, Emit, Target};
use crate::echo;

pub struct Builder<'a> {
    pub input: Option<&'a Path>,
    pub output: Option<PathBuf>,
    pub out_dir: Option<PathBuf>,
    pub target: Option<Target>,
    pub emit: Vec<Emit>,
    pub wat: Option<Option<PathBuf>>,
    pub dump_dir: Option<PathBuf>,
    pub verbose: bool,
    pub json: bool,
}

impl Builder<'_> {
    pub fn build(&mut self) -> ExitCode {
        if self.json {
            return echo::fail("build", "--json", "machine-readable output is not implemented yet");
        }
        if self.wat.is_some() && !self.emit.contains(&Emit::Wat) {
            self.emit.push(Emit::Wat);
        }
        let input = self.input.unwrap_or_else(|| Path::new("."));
        let verbose = self.verbose;
        let mut progress = move |event| print_project_load_progress(event, verbose);
        let target_override = self.target.map(Into::into);
        let project = match compiler_core::project::load_project_with_options(
            input,
            ProjectLoadOptions { progress: Some(&mut progress), compile_target: target_override },
        ) {
            Ok(project) => project,
            Err(diagnostics) => return echo::fail_with_diagnostics("load project", input.display(), &diagnostics),
        };
        if self.verbose {
            for module in &project.graph.modules {
                echo::status("compile", format!("{} -> {}", module.name, module.path.display()));
            }
        }

        let target = project.compile_target;
        let artifact_base = project.config.name.replace('-', "_");
        let output = match super::final_wasm_path(
            self.output.clone(),
            self.out_dir.as_deref(),
            &project.root,
            &artifact_base,
        ) {
            Ok(path) => path,
            Err(message) => return echo::fail("build", "output", message),
        };
        let debug_dir = self.dump_dir.clone().or_else(|| {
            self.emit.iter().any(|emit| emit.is_debug()).then(|| {
                output
                    .parent()
                    .filter(|parent| !parent.as_os_str().is_empty())
                    .map(Path::to_path_buf)
                    .unwrap_or_else(|| PathBuf::from("."))
            })
        });
        let dump_all = self.dump_dir.is_some();

        let typed = match compiler_core::types::check_project(&project) {
            Ok(typed) => typed,
            Err(diagnostics) => {
                return echo::fail_with_source_diagnostics(
                    "compile project",
                    project.root.display(),
                    &diagnostics,
                    &project.sources,
                );
            }
        };
        if let Some(debug_dir) = debug_dir.as_deref()
            && (dump_all || self.emit.iter().any(|emit| emit.is_pre_lower_debug()))
            && let Err(error) = ProjectDebugArtifacts::with_typed(&typed).write_project_debug_dumps(
                debug_dir,
                &artifact_base,
                &self.emit,
                dump_all,
                target,
            )
        {
            return echo::fail("write", "debug dumps", error);
        }

        let ir = match compiler_core::ir::lower_project(typed.clone()) {
            Ok(ir) => ir,
            Err(diagnostics) => {
                return echo::fail_with_source_diagnostics(
                    "compile project",
                    project.root.display(),
                    &diagnostics,
                    &project.sources,
                );
            }
        };
        if let Some(debug_dir) = debug_dir.as_deref()
            && (dump_all || self.emit.contains(&Emit::Ir))
            && let Err(error) = ProjectDebugArtifacts::with_ir(&ir).write_project_debug_dumps(
                debug_dir,
                &artifact_base,
                &self.emit,
                dump_all,
                target,
            )
        {
            return echo::fail("write", "debug dumps", error);
        }

        let wasm = match ir.emit_wasm_with_options(target.into()) {
            Ok(wasm) => wasm,
            Err(diagnostics) => {
                return echo::fail_with_source_diagnostics(
                    "emit wasm",
                    project.root.display(),
                    &diagnostics,
                    &project.sources,
                );
            }
        };
        if let Some(debug_dir) = debug_dir.as_deref()
            && (dump_all || self.emit.contains(&Emit::Runtime) || self.emit.contains(&Emit::Abi))
            && let Err(error) = ProjectDebugArtifacts::with(&typed, &ir, &wasm).write_project_debug_dumps(
                debug_dir,
                &artifact_base,
                &self.emit,
                dump_all,
                target,
            )
        {
            return echo::fail("write", "debug dumps", error);
        }

        if self.emit.contains(&Emit::Wasm) {
            if let Err(error) = super::write_file(&output, &wasm.bytes) {
                return echo::fail("write", output.display(), error);
            }
            echo::status("wasm", format!("{} ({} bytes)", output.display(), wasm.bytes.len()));
            if matches!(
                target,
                compiler_core::target::CompileTarget::Browser
                    | compiler_core::target::CompileTarget::Bundler
                    | compiler_core::target::CompileTarget::Nodejs
            ) {
                let adapter_path = super::artifact_path(self.out_dir.as_deref(), &output, &artifact_base, "mjs");
                let adapter = compiler_core::adapter::js_adapter_for_module(
                    output
                        .file_name()
                        .and_then(|name| name.to_str())
                        .unwrap_or("module.wasm"),
                    &ir,
                );
                if let Err(error) = super::write_file(&adapter_path, adapter.as_bytes()) {
                    return echo::fail("write", adapter_path.display(), error);
                }
                echo::status("js", adapter_path.display().to_string());
            }
        }
        if self.emit.contains(&Emit::Wat) {
            let wat_path = self
                .wat
                .clone()
                .flatten()
                .unwrap_or_else(|| super::artifact_path(self.out_dir.as_deref(), &output, &artifact_base, "wat"));
            if let Err(error) = super::write_file(&wat_path, wasm.wat.as_bytes()) {
                return echo::fail("write", wat_path.display(), error);
            }
            echo::status("wat", wat_path.display().to_string());
        }
        if let Some(dump_dir) = self.dump_dir.clone()
            && let Err(error) = ProjectDebugArtifacts::with(&typed, &ir, &wasm).write_project_debug_dumps(
                &dump_dir,
                &artifact_base,
                &self.emit,
                true,
                target,
            )
        {
            return echo::fail("write", "debug dumps", error);
        }

        ExitCode::SUCCESS
    }
}

#[derive(Clone, Copy)]
pub struct ProjectDebugArtifacts<'a> {
    pub typed: Option<&'a TypedProject>,
    pub ir: Option<&'a ir::Module>,
    pub wasm: Option<&'a wasm::WasmModule>,
}

impl<'a> ProjectDebugArtifacts<'a> {
    fn new(typed: Option<&'a TypedProject>, ir: Option<&'a ir::Module>, wasm: Option<&'a wasm::WasmModule>) -> Self {
        Self { typed, ir, wasm }
    }

    pub fn with_typed(typed: &'a TypedProject) -> Self {
        Self::new(Some(typed), None, None)
    }

    pub fn with_ir(ir: &'a ir::Module) -> Self {
        Self::new(None, Some(ir), None)
    }

    pub fn with(typed: &'a TypedProject, ir: &'a ir::Module, wasm: &'a wasm::WasmModule) -> Self {
        Self::new(Some(typed), Some(ir), Some(wasm))
    }
}

impl ProjectDebugArtifacts<'_> {
    fn write_project_debug_dumps(
        self, dump_dir: &Path, artifact_base: &str, emit: &[args::Emit], dump_all: bool,
        target: compiler_core::target::CompileTarget,
    ) -> std::io::Result<()> {
        fs::create_dir_all(dump_dir)?;

        if let Some(typed) = self.typed {
            for module in &typed.modules {
                let package = module.package_name.as_deref().unwrap_or(&typed.package_name);
                let name = module.module_name.as_deref().unwrap_or("module");
                if dump_all || emit.contains(&args::Emit::Ast) {
                    let path = project_module_artifact_name(artifact_base, package, name, "ast.txt");
                    fs::write(dump_dir.join(path), format!("{:#?}\n", module.resolved.ast))?;
                }
                if dump_all || emit.contains(&args::Emit::Resolved) {
                    let path = project_module_artifact_name(artifact_base, package, name, "resolved.txt");
                    fs::write(dump_dir.join(path), format!("{:#?}\n", module.resolved))?;
                }
                if dump_all || emit.contains(&args::Emit::Typed) {
                    let path = project_module_artifact_name(artifact_base, package, name, "typed.txt");
                    fs::write(dump_dir.join(path), format!("{:#?}\n", module))?;
                }
            }
        }

        if let Some(ir) = self.ir
            && (dump_all || emit.contains(&args::Emit::Ir))
        {
            fs::write(dump_dir.join(format!("{artifact_base}.ir.txt")), ir.linked_debug_dump())?;
        }
        if let Some(wasm) = self.wasm
            && dump_all
        {
            fs::write(dump_dir.join(format!("{artifact_base}.wat")), &wasm.wat)?;
        }
        if dump_all || emit.contains(&args::Emit::Runtime) {
            fs::write(
                dump_dir.join(format!("{artifact_base}.runtime.txt")),
                super::runtime_debug_dump(),
            )?;
        }
        if let Some(ir) = self.ir
            && (dump_all || emit.contains(&args::Emit::Abi))
        {
            fs::write(
                dump_dir.join(format!("{artifact_base}.abi.txt")),
                super::abi_debug_dump(ir, target),
            )?;
        }
        Ok(())
    }
}

fn project_module_artifact_name(base: &str, package: &str, module: &str, suffix: &str) -> String {
    format!(
        "{}.{}.{}.{}",
        base,
        artifact_component_escape(package),
        artifact_component_escape(module),
        suffix
    )
}

fn artifact_component_escape(component: &str) -> String {
    let mut escaped = String::new();
    for byte in component.bytes() {
        match byte {
            b'a'..=b'z' | b'A'..=b'Z' | b'0'..=b'9' => escaped.push(byte as char),
            b'_' => escaped.push_str("__"),
            other => escaped.push_str(&format!("_{other:02x}")),
        }
    }
    escaped
}

fn package_label(name: &str, version: Option<&str>) -> String {
    match version {
        Some(version) => format!("{name} {version}"),
        None => name.to_string(),
    }
}

fn print_project_load_progress(event: ProjectLoadProgress, verbose: bool) {
    match event {
        ProjectLoadProgress::ResolvingDependencies => echo::progress("Resolving dependencies"),
        ProjectLoadProgress::UsingCachedPackage { name, version, path } => {
            let package = package_label(&name, version.as_deref());
            if verbose {
                echo::progress(format!("Using cached {package} ({})", path.display()));
            } else {
                echo::progress(format!("Using cached {package}"));
            }
        }
        ProjectLoadProgress::UsingPathPackage { name, version, path } => {
            let package = package_label(&name, version.as_deref());
            if verbose {
                echo::progress(format!("Using path {package} ({})", path.display()));
            } else {
                echo::progress(format!("Using path {package}"));
            }
        }
    }
}
