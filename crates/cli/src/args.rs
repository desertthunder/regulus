use std::path::PathBuf;

use clap::{Parser, Subcommand, ValueEnum};

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
        /// Path for the generated .wasm file.
        #[arg(short, long)]
        output: Option<PathBuf>,
        /// Also write the generated WebAssembly text format.
        #[arg(long)]
        wat: Option<Option<PathBuf>>,
        /// Write compiler debug dumps to this directory.
        #[arg(long)]
        dump_dir: Option<PathBuf>,
        /// Select the intended runtime target.
        #[arg(long, value_enum, default_value_t = Target::Wasmtime)]
        target: Target,
    },
    /// Load a Gleam project and print discovered modules.
    Project {
        /// Project directory containing gleam.toml.
        input: PathBuf,
    },
}

#[derive(Debug, Clone, Copy, ValueEnum)]
pub enum Target {
    Wasmtime,
    Browser,
    Wasi,
}
