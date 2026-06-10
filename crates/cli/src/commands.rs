use std::{
    fs,
    path::{Path, PathBuf},
    process::ExitCode,
};

use compiler_core::{self, source::SourceFile, source::SourceFileId};

use crate::{
    args::{Command, Target},
    echo,
};

pub fn run(command: Command) -> ExitCode {
    match command {
        Command::Compile { input, output, wat, dump_dir, target } => compile(&input, output, wat, dump_dir, target),
        Command::Project { input } => project(&input),
    }
}

fn project(input: &Path) -> ExitCode {
    match compiler_core::project::load_project(input) {
        Ok(project) => {
            echo::status(
                "project",
                format!(
                    "{} {} ({} modules)",
                    project.config.name,
                    project.config.version,
                    project.graph.modules.len()
                ),
            );
            for module in project.graph.modules {
                echo::status("module", format!("{} -> {}", module.name, module.path.display()));
            }
            ExitCode::SUCCESS
        }
        Err(diagnostics) => echo::fail_with_diagnostics("load project", input.display(), &diagnostics),
    }
}

fn compile(
    input: &Path, output: Option<PathBuf>, wat: Option<Option<PathBuf>>, dump_dir: Option<PathBuf>, target: Target,
) -> ExitCode {
    let source = match fs::read_to_string(input) {
        Ok(source) => SourceFile::with_path(SourceFileId(0), input, source),
        Err(error) => return echo::fail("read", input.display(), error),
    };

    let compiled = match compile_with_dumps(source, target.into()) {
        Ok(compiled) => compiled,
        Err(diagnostics) => return echo::fail_with_diagnostics("compile", input.display(), &diagnostics),
    };

    if let Some(dump_dir) = dump_dir
        && let Err(error) = write_debug_dumps(&dump_dir, &compiled)
    {
        return echo::fail("write", "debug dumps", error);
    }

    let output = output.unwrap_or_else(|| input.with_extension("wasm"));
    if let Err(error) = write_file(&output, &compiled.wasm.bytes) {
        return echo::fail("write", output.display(), error);
    }
    echo::status(
        "wasm",
        format!("{} ({} bytes)", output.display(), compiled.wasm.bytes.len()),
    );

    if let Some(wat_path) = wat {
        let wat_path = wat_path.unwrap_or_else(|| output.with_extension("wat"));
        if let Err(error) = write_file(&wat_path, compiled.wasm.wat.as_bytes()) {
            return echo::fail("write", wat_path.display(), error);
        }
        echo::status("wat", wat_path.display().to_string());
    }

    ExitCode::SUCCESS
}

struct CompiledModule {
    ast: compiler_core::ast::Module,
    resolved: compiler_core::resolve::ResolvedModule,
    typed: compiler_core::types::TypedModule,
    ir: compiler_core::ir::Module,
    wasm: compiler_core::wasm::WasmModule,
}

fn compile_with_dumps(
    source: SourceFile, target: compiler_core::target::CompileTarget,
) -> Result<CompiledModule, compiler_core::diagnostic::Diagnostics> {
    let cst = compiler_core::parse::parse(source)?;
    let ast = compiler_core::ast::build(&cst)?;
    let ast = compiler_core::target::select_module(ast, target)?;
    let resolved = compiler_core::resolve::resolve(ast.clone())?;
    let typed = compiler_core::types::check(resolved.clone())?;
    let ir = compiler_core::ir::lower(typed.clone())?;
    let wasm = ir.emit_wasm_with_options(target.into())?;
    Ok(CompiledModule { ast, resolved, typed, ir, wasm })
}

impl From<Target> for compiler_core::target::CompileTarget {
    fn from(target: Target) -> Self {
        match target {
            Target::Wasmtime => Self::Wasmtime,
            Target::Browser => Self::Browser,
            Target::Wasi => Self::Wasi,
        }
    }
}

fn write_debug_dumps(dump_dir: &Path, compiled: &CompiledModule) -> std::io::Result<()> {
    fs::create_dir_all(dump_dir)?;
    fs::write(dump_dir.join("ast.txt"), format!("{:#?}\n", compiled.ast))?;
    fs::write(dump_dir.join("resolved.txt"), format!("{:#?}\n", compiled.resolved))?;
    fs::write(dump_dir.join("typed.txt"), format!("{:#?}\n", compiled.typed))?;
    fs::write(dump_dir.join("ir.txt"), format!("{:#?}\n", compiled.ir))?;
    fs::write(dump_dir.join("wat.wat"), &compiled.wasm.wat)?;
    Ok(())
}

fn write_file(path: &Path, contents: &[u8]) -> std::io::Result<()> {
    if let Some(parent) = path.parent()
        && !parent.as_os_str().is_empty()
    {
        fs::create_dir_all(parent)?;
    }
    fs::write(path, contents)
}
