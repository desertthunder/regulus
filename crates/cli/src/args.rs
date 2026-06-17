use std::path::PathBuf;

use clap::{Parser, Subcommand, ValueEnum};
use compiler_core::target::CompileTarget;

#[derive(Debug, Parser)]
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
    /// Compile a source file and execute one exported function with Wasmtime.
    #[command(visible_alias = "exec")]
    Run {
        /// Gleam source file to compile and run.
        input: PathBuf,
        /// Exported function to invoke.
        #[arg(short, long, default_value = "main")]
        function: String,
        /// Positional arguments passed to the exported function.
        args: Vec<String>,
        /// Select the intended runtime target.
        #[arg(long, value_enum, default_value_t = Target::Wasmtime)]
        target: Target,
        /// Print the input as it is compiled.
        #[arg(short, long)]
        verbose: bool,
        /// Reserved for future machine-readable output.
        #[arg(long)]
        json: bool,
    },
    /// Inspect compiler-internal views of one Gleam source file.
    #[command(visible_alias = "dbg")]
    Debug {
        /// Gleam source file to inspect.
        input: PathBuf,
        /// Print the raw tree-sitter concrete syntax tree as an S-expression.
        ///
        /// Use this to look at exact node kinds, field names, or the way Gleam
        /// syntax is split across adjacent tree-sitter nodes.
        #[arg(long = "ts", alias = "tree-sitter")]
        tree_sitter: bool,
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

impl Emit {
    pub fn is_debug(self) -> bool {
        matches!(self, Self::Ast | Self::Resolved | Self::Typed | Self::Ir)
    }

    pub fn is_pre_lower_debug(self) -> bool {
        matches!(self, Self::Ast | Self::Resolved | Self::Typed)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum Target {
    Wasmtime,
    Browser,
    Bundler,
    Nodejs,
    Wasi,
}

impl From<Target> for CompileTarget {
    fn from(target: Target) -> Self {
        match target {
            Target::Wasmtime => Self::Wasmtime,
            Target::Browser => Self::Browser,
            Target::Bundler => Self::Bundler,
            Target::Nodejs => Self::Nodejs,
            Target::Wasi => Self::Wasi,
        }
    }
}
