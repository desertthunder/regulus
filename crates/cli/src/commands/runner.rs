use std::fs;
use std::path::Path;
use std::process::ExitCode;

use compiler_core::source::{SourceFile, SourceFileId};

use crate::args::Target;
use crate::echo;

pub struct Runner<'a> {
    pub(super) input: &'a Path,
    pub(super) function: &'a str,
    pub(super) args: &'a [String],
    pub(super) target: Target,
    pub(super) verbose: bool,
    pub(super) json: bool,
}

impl Runner<'_> {
    pub fn run(&self) -> ExitCode {
        if self.json {
            return echo::fail("run", "--json", "machine-readable output is not implemented yet");
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

        match super::run_wasm_export(&compiled.wasm.bytes, self.function, self.args) {
            Ok(()) => ExitCode::SUCCESS,
            Err(message) => echo::fail("run", self.function, message),
        }
    }
}
