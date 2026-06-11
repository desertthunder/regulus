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
    /// Compile a Gleam project rooted at a directory or gleam.toml.
    Build {
        /// Project directory or gleam.toml path. Defaults to the current directory.
        project: Option<PathBuf>,
        /// Path for the generated .wasm file.
        #[arg(short, long)]
        output: Option<PathBuf>,
        /// Directory for compiler-named artifacts.
        #[arg(long)]
        out_dir: Option<PathBuf>,
        /// Select the intended runtime target.
        #[arg(long, value_enum)]
        target: Option<Target>,
        /// Select emitted artifacts.
        #[arg(long, value_enum, value_delimiter = ',', default_value = "wasm")]
        emit: Vec<Emit>,
        /// Write compiler debug dumps to this directory.
        #[arg(long)]
        dump_dir: Option<PathBuf>,
        /// Print modules as they are compiled.
        #[arg(short, long)]
        verbose: bool,
        /// Reserved for future machine-readable output.
        #[arg(long)]
        json: bool,
    },
    /// Run the compiler pipeline for a source file.
    Compile {
        /// Gleam source file to compile.
        input: PathBuf,
        /// Path for the generated .wasm file.
        #[arg(short, long)]
        output: Option<PathBuf>,
        /// Directory for compiler-named artifacts.
        #[arg(long)]
        out_dir: Option<PathBuf>,
        /// Select the intended runtime target.
        #[arg(long, value_enum, default_value_t = Target::Wasmtime)]
        target: Target,
        /// Select emitted artifacts.
        #[arg(long, value_enum, value_delimiter = ',', default_value = "wasm")]
        emit: Vec<Emit>,
        /// Also write the generated WebAssembly text format.
        #[arg(long)]
        wat: Option<Option<PathBuf>>,
        /// Write compiler debug dumps to this directory.
        #[arg(long)]
        dump_dir: Option<PathBuf>,
        /// Print the input as it is compiled.
        #[arg(short, long)]
        verbose: bool,
        /// Reserved for future machine-readable output.
        #[arg(long)]
        json: bool,
    },
    /// Load a Gleam project and print discovered modules.
    List {
        /// Project directory or gleam.toml path. Defaults to the current directory.
        project: Option<PathBuf>,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum Emit {
    Wasm,
    Wat,
    Ast,
    Resolved,
    Typed,
    Ir,
}

#[derive(Debug, Clone, Copy, ValueEnum)]
pub enum Target {
    Wasmtime,
    Browser,
    Wasi,
}
