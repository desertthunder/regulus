use std::path::PathBuf;

use clap::{Parser, Subcommand};

#[derive(Debug, Parser)]
#[command(name = "gleam-wasm")]
#[command(about = "Compile Gleam source to WebAssembly")]
pub struct Args {
    #[command(subcommand)]
    pub command: Command,
}

#[derive(Debug, Subcommand)]
pub enum Command {
    /// Run the compiler pipeline for a source file.
    Compile {
        /// Gleam source file to compile.
        input: PathBuf,
    },
    /// Load a Gleam project and print discovered modules.
    Project {
        /// Project directory containing gleam.toml.
        input: PathBuf,
    },
}
