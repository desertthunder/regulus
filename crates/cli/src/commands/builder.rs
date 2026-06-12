use std::path::{Path, PathBuf};
use std::process::ExitCode;

use compiler_core::project::{ProjectLoadOptions, ProjectLoadProgress};

use crate::args::{Emit, Target};
use crate::echo;

pub struct Builder<'a> {
    pub(super) input: Option<&'a Path>,
    pub(super) output: Option<PathBuf>,
    pub(super) out_dir: Option<PathBuf>,
    pub(super) target: Option<Target>,
    pub(super) emit: Vec<Emit>,
    pub(super) dump_dir: Option<PathBuf>,
    pub(super) verbose: bool,
    pub(super) json: bool,
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

fn package_label(name: &str, version: Option<&str>) -> String {
    match version {
        Some(version) => format!("{name} {version}"),
        None => name.to_string(),
    }
}

impl Builder<'_> {
    pub fn build(&mut self) -> ExitCode {
        if self.json {
            return echo::fail("build", "--json", "machine-readable output is not implemented yet");
        }
        let input = self.input.unwrap_or_else(|| Path::new("."));
        let verbose = self.verbose;
        let mut progress = move |event| print_project_load_progress(event, verbose);
        let project = match compiler_core::project::load_project_with_options(
            input,
            ProjectLoadOptions { progress: Some(&mut progress) },
        ) {
            Ok(project) => project,
            Err(diagnostics) => return echo::fail_with_diagnostics("load project", input.display(), &diagnostics),
        };
        if self.verbose {
            for module in &project.graph.modules {
                echo::status("compile", format!("{} -> {}", module.name, module.path.display()));
            }
        }

        let target = self
            .target
            .map(Into::into)
            .unwrap_or_else(|| compiler_core::target::project_compile_target(project.config.target.as_ref()));
        let typed = match compiler_core::types::check_project(&project) {
            Ok(typed) => typed,
            Err(diagnostics) => {
                return echo::fail_with_diagnostics("compile project", project.root.display(), &diagnostics);
            }
        };
        let ir = match compiler_core::ir::lower_project(typed) {
            Ok(ir) => ir,
            Err(diagnostics) => {
                return echo::fail_with_diagnostics("compile project", project.root.display(), &diagnostics);
            }
        };
        let wasm = match ir.emit_wasm_with_options(target.into()) {
            Ok(wasm) => wasm,
            Err(diagnostics) => return echo::fail_with_diagnostics("emit wasm", project.root.display(), &diagnostics),
        };

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

        if self.emit.contains(&Emit::Wasm) {
            if let Err(error) = super::write_file(&output, &wasm.bytes) {
                return echo::fail("write", output.display(), error);
            }
            echo::status("wasm", format!("{} ({} bytes)", output.display(), wasm.bytes.len()));
        }
        if self.emit.contains(&Emit::Wat) {
            let wat_path = super::artifact_path(self.out_dir.as_deref(), &output, &artifact_base, "wat");
            if let Err(error) = super::write_file(&wat_path, wasm.wat.as_bytes()) {
                return echo::fail("write", wat_path.display(), error);
            }
            echo::status("wat", wat_path.display().to_string());
        }
        if let Some(dump_dir) = self.dump_dir.clone()
            && let Err(error) = super::write_project_debug_dumps(&dump_dir, &ir, &wasm)
        {
            return echo::fail("write", "debug dumps", error);
        }

        ExitCode::SUCCESS
    }
}
