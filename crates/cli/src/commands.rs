use std::{fs, process::ExitCode};

use compiler_core::{self, source::SourceFile, source::SourceFileId};

use crate::{args::Command, echo};

pub fn run(command: Command) -> ExitCode {
    match command {
        Command::Compile { input } => compile(input),
    }
}

fn compile(input: std::path::PathBuf) -> ExitCode {
    let source = match fs::read_to_string(&input) {
        Ok(source) => source,
        Err(error) => {
            echo::error(format!("could not read {}: {error}", input.display()));
            return ExitCode::FAILURE;
        }
    };

    let source = SourceFile::with_path(SourceFileId(0), input.clone(), source);
    match compiler_core::compile_source(source) {
        Ok(output) => {
            echo::status(
                "compiled",
                format!("{} ({} bytes)", input.display(), output.wasm.bytes.len()),
            );
            ExitCode::SUCCESS
        }
        Err(diagnostics) => {
            echo::error(format!("could not compile {}", input.display()));
            for diagnostic in diagnostics {
                echo::diagnostic(diagnostic.message);
            }
            ExitCode::FAILURE
        }
    }
}
