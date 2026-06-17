use std::fs;
use std::path::Path;
use std::process::ExitCode;

use compiler_core::source::{SourceFile, SourceFileId};

use crate::echo;

/// Developer-facing compiler inspection command.
///
/// The debug command is intentionally read-only.
///
/// It loads one Gleam file and prints one or more intermediate compiler views.
pub struct Debugger<'a> {
    pub input: &'a Path,
    pub tree_sitter: bool,
}

impl<'a> Debugger<'a> {
    pub fn new(input: &'a Path, ts: bool) -> Self {
        Self { input, tree_sitter: ts }
    }
}

impl Debugger<'_> {
    pub fn run(&self) -> ExitCode {
        if !self.tree_sitter {
            echo::error("select at least one debug view, for example `--ts`");
            return ExitCode::FAILURE;
        }

        let text = match fs::read_to_string(self.input) {
            Ok(text) => text,
            Err(error) => return echo::fail("read", self.input.display(), error),
        };
        let source = SourceFile::with_path(SourceFileId(0), self.input, text);
        let cst = match compiler_core::parse::parse(source) {
            Ok(cst) => cst,
            Err(diagnostics) => return echo::fail_with_diagnostics("parse", self.input.display(), &diagnostics),
        };

        if self.tree_sitter {
            println!("{}", cst.tree.root_node().to_sexp());
        }

        // TODO: Add `--ast` output once the debug command has a stable shape
        // for printing Regulus AST after target selection.
        // TODO: Add `--spans` to include byte spans and field names for parser
        // and diagnostic source-mapping work.
        // TODO: Add `--json` for machine-readable debug output suitable for
        // editor tooling and snapshot fixtures.

        ExitCode::SUCCESS
    }
}
