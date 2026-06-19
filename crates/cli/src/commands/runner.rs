use std::fs;
use std::path::Path;
use std::process::ExitCode;

use compiler_core::source::{SourceFile, SourceFileId};

use crate::args::Target;
use crate::echo;

pub struct Runner<'a> {
    pub input: &'a Path,
    pub function: &'a str,
    pub args: &'a [String],
    pub target: Target,
    pub verbose: bool,
    pub json: bool,
}

impl<'a> Runner<'a> {
    pub fn new(
        input: &'a Path, function: &'a str, args: &'a [String], target: Target, verbose: bool, json: bool,
    ) -> Self {
        Self { input, function, args, target, verbose, json }
    }
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

        let return_type = compiled
            .ir
            .exports
            .iter()
            .find(|export| export.name == self.function)
            .and_then(|export| {
                compiled
                    .ir
                    .functions
                    .iter()
                    .find(|function| function.name == export.backend_name())
            })
            .map(|function| &function.return_type);

        match super::run_wasm_export(&compiled.wasm.bytes, self.function, self.args, return_type) {
            Ok(()) => ExitCode::SUCCESS,
            Err(message) => echo::fail("run", self.function, message),
        }
    }
}
