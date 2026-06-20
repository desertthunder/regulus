use std::path::PathBuf;

use clap::{Args as ClapArgs, Parser, Subcommand, ValueEnum};
use compiler_core::target::CompileTarget;

#[derive(Debug, Parser)]
#[command(about = "Compile Gleam source to WebAssembly")]
pub struct Args {
    /// Disable ANSI colors in human-readable output.
    ///
    /// This is also enabled when the NO_COLOR environment variable is set.
    #[arg(long, global = true)]
    pub no_color: bool,
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
        /// Also write the generated WebAssembly text format.
        #[arg(long)]
        wat: Option<Option<PathBuf>>,
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
    /// Inspect compiler-internal source or project views.
    #[command(visible_alias = "dbg")]
    Debug {
        #[command(subcommand)]
        view: Option<DebugCommand>,
        /// Gleam source file to inspect.
        ///
        /// Required when no debug subcommand is used.
        input: Option<PathBuf>,
        /// Print the raw tree-sitter concrete syntax tree as an S-expression.
        ///
        /// Use this to look at exact node kinds, field names, or the way Gleam
        /// syntax is split across adjacent tree-sitter nodes.
        #[arg(long = "ts", alias = "tree-sitter")]
        tree_sitter: bool,
        /// Print the Regulus AST built from the tree-sitter tree.
        #[arg(long)]
        ast: bool,
        /// Include tree-sitter byte spans, positions, and field names.
        #[arg(long)]
        spans: bool,
        /// Print selected debug views as JSON.
        #[arg(long)]
        json: bool,
        /// Disable ANSI colors in human-readable debug output.
        #[arg(long)]
        no_color: bool,
    },
    /// Load a Gleam project and print discovered modules.
    List {
        /// Project directory or gleam.toml path. Defaults to the current directory.
        project: Option<PathBuf>,
    },
}

#[derive(Debug, Subcommand)]
pub enum DebugCommand {
    /// Print the raw tree-sitter concrete syntax tree.
    #[command(visible_alias = "tree-sitter")]
    Ts(DebugTreeArgs),
    /// Print tree-sitter nodes with byte spans, positions, and field names.
    Spans(DebugTreeArgs),
    /// Print the Regulus AST built from the tree-sitter tree.
    Ast(DebugAstArgs),
    /// Print selected debug views as JSON.
    Json(DebugJsonArgs),
    /// Load a project and print linked IR.
    Ir(DebugIrArgs),
}

#[derive(Debug, ClapArgs)]
pub struct DebugTreeArgs {
    /// Gleam source file to inspect.
    pub input: PathBuf,
    /// Print this view as JSON.
    #[arg(long)]
    pub json: bool,
    /// Disable ANSI colors in human-readable output.
    #[arg(long)]
    pub no_color: bool,
}

#[derive(Debug, ClapArgs)]
pub struct DebugAstArgs {
    /// Gleam source file to inspect.
    pub input: PathBuf,
    /// Print this view as JSON.
    #[arg(long)]
    pub json: bool,
    /// Disable ANSI colors in human-readable output.
    #[arg(long)]
    pub no_color: bool,
}

#[derive(Debug, ClapArgs)]
pub struct DebugJsonArgs {
    /// Gleam source file to inspect.
    pub input: PathBuf,
    /// Include the tree-sitter S-expression.
    #[arg(long = "ts", alias = "tree-sitter")]
    pub tree_sitter: bool,
    /// Include the Regulus AST.
    #[arg(long)]
    pub ast: bool,
    /// Include tree-sitter span details instead of only the S-expression.
    #[arg(long)]
    pub spans: bool,
}

#[derive(Debug, ClapArgs)]
pub struct DebugIrArgs {
    /// Project directory or gleam.toml path to inspect.
    pub input: PathBuf,
    /// Disable ANSI colors in human-readable output.
    #[arg(long)]
    pub no_color: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum Emit {
    Wasm,
    Wat,
    Ast,
    Resolved,
    Typed,
    Ir,
    Runtime,
    Abi,
}

impl Emit {
    pub fn is_debug(self) -> bool {
        matches!(
            self,
            Self::Ast | Self::Resolved | Self::Typed | Self::Ir | Self::Runtime | Self::Abi
        )
    }

    pub fn is_pre_lower_debug(self) -> bool {
        matches!(self, Self::Ast | Self::Resolved | Self::Typed)
    }
}

#[derive(Clone, Copy)]
pub struct DebugOptions {
    pub ts: bool,
    pub ast: bool,
    pub spans: bool,
    pub json: bool,
    pub no_color: bool,
}

impl DebugOptions {
    pub fn new(ts: bool, ast: bool, spans: bool, json: bool, no_color: bool) -> Self {
        Self { ts, ast, spans, json, no_color }
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

impl Target {
    pub fn name(self) -> &'static str {
        match self {
            Self::Wasmtime => "wasmtime",
            Self::Browser => "browser",
            Self::Bundler => "bundler",
            Self::Nodejs => "nodejs",
            Self::Wasi => "wasi",
        }
    }
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
