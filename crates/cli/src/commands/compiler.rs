use std::fs;

use std::path::{Path, PathBuf};
use std::process::ExitCode;

use compiler_core::source::{SourceFile, SourceFileId};

use crate::args::{Emit, Target};
use crate::echo;

pub struct Compiler<'a> {
    pub(super) input: &'a Path,
    pub(super) output: Option<PathBuf>,
    pub(super) out_dir: Option<PathBuf>,
    pub(super) wat: Option<Option<PathBuf>>,
    pub(super) dump_dir: Option<PathBuf>,
    pub(super) target: Target,
    pub(super) emit: Vec<Emit>,
    pub(super) verbose: bool,
    pub(super) json: bool,
}

impl Compiler<'_> {
    pub fn compile(&mut self) -> ExitCode {
        if self.json {
            return echo::fail("compile", "--json", "machine-readable output is not implemented yet");
        }
        if self.wat.is_some() && !self.emit.contains(&Emit::Wat) {
            self.emit.push(Emit::Wat);
        }
        if self.verbose {
            echo::status("compile", self.input.display().to_string());
        }
        let source = match fs::read_to_string(self.input) {
            Ok(source) => SourceFile::with_path(SourceFileId(0), self.input, source),
            Err(error) => return echo::fail("read", self.input.display(), error),
        };

        let compiled = match super::CompiledModule::with_dumps(source, self.target.into()) {
            Ok(compiled) => compiled,
            Err(diagnostics) => return echo::fail_with_diagnostics("compile", self.input.display(), &diagnostics),
        };

        if let Some(dump_dir) = self.dump_dir.clone()
            && let Err(error) = write_debug_dumps(&dump_dir, &compiled)
        {
            return echo::fail("write", "debug dumps", error);
        }

        let artifact_base = self
            .input
            .file_stem()
            .and_then(|stem| stem.to_str())
            .unwrap_or("module");
        let output = match (self.output.clone(), self.out_dir.as_deref()) {
            (Some(_), Some(_)) => {
                return echo::fail("compile", "output", "--output and --out-dir cannot be used together");
            }
            (Some(path), None) => path,
            (None, Some(dir)) => dir.join(format!("{artifact_base}.wasm")),
            (None, None) => self.input.with_extension("wasm"),
        };

        if self.emit.contains(&Emit::Wasm) {
            if let Err(error) = super::write_file(&output, &compiled.wasm.bytes) {
                return echo::fail("write", output.display(), error);
            }
            echo::status(
                "wasm",
                format!("{} ({} bytes)", output.display(), compiled.wasm.bytes.len()),
            );
            if matches!(self.target, Target::Browser | Target::Bundler | Target::Nodejs) {
                let adapter_path = super::artifact_path(self.out_dir.as_deref(), &output, artifact_base, "mjs");
                let adapter = compiler_core::adapter::js_adapter_for_module(
                    output
                        .file_name()
                        .and_then(|name| name.to_str())
                        .unwrap_or("module.wasm"),
                    &compiled.ir,
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
                .unwrap_or_else(|| super::artifact_path(self.out_dir.as_deref(), &output, artifact_base, "wat"));
            if let Err(error) = super::write_file(&wat_path, compiled.wasm.wat.as_bytes()) {
                return echo::fail("write", wat_path.display(), error);
            }
            echo::status("wat", wat_path.display().to_string());
        }

        ExitCode::SUCCESS
    }
}

fn write_debug_dumps(dump_dir: &Path, compiled: &super::CompiledModule) -> std::io::Result<()> {
    fs::create_dir_all(dump_dir)?;
    fs::write(dump_dir.join("ast.txt"), format!("{:#?}\n", compiled.ast))?;
    fs::write(dump_dir.join("resolved.txt"), format!("{:#?}\n", compiled.resolved))?;
    fs::write(dump_dir.join("typed.txt"), format!("{:#?}\n", compiled.typed))?;
    fs::write(dump_dir.join("ir.txt"), format!("{:#?}\n", compiled.ir))?;
    fs::write(dump_dir.join("wat.wat"), &compiled.wasm.wat)?;
    Ok(())
}
