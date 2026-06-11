use std::{
    fs,
    path::{Path, PathBuf},
    process::ExitCode,
};

use compiler_core::{self, source::SourceFile, source::SourceFileId};

use crate::{
    args::{Command, Emit, Target},
    echo,
};

pub fn run(command: Command) -> ExitCode {
    match command {
        Command::Build { project, output, out_dir, target, emit, dump_dir, verbose, json } => build(
            project.as_deref(),
            output,
            out_dir,
            target,
            emit,
            dump_dir,
            verbose,
            json,
        ),
        Command::Compile { input, output, out_dir, target, emit, wat, dump_dir, verbose, json } => {
            compile(&input, output, out_dir, wat, dump_dir, target, emit, verbose, json)
        }
        Command::List { project } => list(project.as_deref().unwrap_or_else(|| Path::new("."))),
    }
}

fn list(input: &Path) -> ExitCode {
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

fn build(
    input: Option<&Path>, output: Option<PathBuf>, out_dir: Option<PathBuf>, target: Option<Target>, emit: Vec<Emit>,
    dump_dir: Option<PathBuf>, verbose: bool, json: bool,
) -> ExitCode {
    if json {
        return echo::fail("build", "--json", "machine-readable output is not implemented yet");
    }
    let input = input.unwrap_or_else(|| Path::new("."));
    let project = match compiler_core::project::load_project(input) {
        Ok(project) => project,
        Err(diagnostics) => return echo::fail_with_diagnostics("load project", input.display(), &diagnostics),
    };
    if verbose {
        for module in &project.graph.modules {
            echo::status("compile", format!("{} -> {}", module.name, module.path.display()));
        }
    }

    let target = target
        .map(Into::into)
        .unwrap_or_else(|| compiler_core::target::project_compile_target(project.config.target.as_ref()));
    let typed = match compiler_core::types::check_project(&project) {
        Ok(typed) => typed,
        Err(diagnostics) => {
            return echo::fail_with_diagnostics("compile project", project.root.display(), &diagnostics);
        }
    };
    let ir = match compiler_core::ir::lower_project(typed) {
        Ok(ir) => ir,
        Err(diagnostics) => {
            return echo::fail_with_diagnostics("compile project", project.root.display(), &diagnostics);
        }
    };
    let wasm = match ir.emit_wasm_with_options(target.into()) {
        Ok(wasm) => wasm,
        Err(diagnostics) => return echo::fail_with_diagnostics("emit wasm", project.root.display(), &diagnostics),
    };

    let artifact_base = project.config.name.replace('-', "_");
    let output = match final_wasm_path(output, out_dir.as_deref(), &project.root, &artifact_base) {
        Ok(path) => path,
        Err(message) => return echo::fail("build", "output", message),
    };

    if emit.contains(&Emit::Wasm) {
        if let Err(error) = write_file(&output, &wasm.bytes) {
            return echo::fail("write", output.display(), error);
        }
        echo::status("wasm", format!("{} ({} bytes)", output.display(), wasm.bytes.len()));
    }
    if emit.contains(&Emit::Wat) {
        let wat_path = artifact_path(out_dir.as_deref(), &output, &artifact_base, "wat");
        if let Err(error) = write_file(&wat_path, wasm.wat.as_bytes()) {
            return echo::fail("write", wat_path.display(), error);
        }
        echo::status("wat", wat_path.display().to_string());
    }
    if let Some(dump_dir) = dump_dir
        && let Err(error) = write_project_debug_dumps(&dump_dir, &ir, &wasm)
    {
        return echo::fail("write", "debug dumps", error);
    }

    ExitCode::SUCCESS
}

fn compile(
    input: &Path, output: Option<PathBuf>, out_dir: Option<PathBuf>, wat: Option<Option<PathBuf>>,
    dump_dir: Option<PathBuf>, target: Target, mut emit: Vec<Emit>, verbose: bool, json: bool,
) -> ExitCode {
    if json {
        return echo::fail("compile", "--json", "machine-readable output is not implemented yet");
    }
    if wat.is_some() && !emit.contains(&Emit::Wat) {
        emit.push(Emit::Wat);
    }
    if verbose {
        echo::status("compile", input.display().to_string());
    }
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

    let artifact_base = input.file_stem().and_then(|stem| stem.to_str()).unwrap_or("module");
    let output = match (output, out_dir.as_deref()) {
        (Some(_), Some(_)) => return echo::fail("compile", "output", "--output and --out-dir cannot be used together"),
        (Some(path), None) => path,
        (None, Some(dir)) => dir.join(format!("{artifact_base}.wasm")),
        (None, None) => input.with_extension("wasm"),
    };

    if emit.contains(&Emit::Wasm) {
        if let Err(error) = write_file(&output, &compiled.wasm.bytes) {
            return echo::fail("write", output.display(), error);
        }
        echo::status(
            "wasm",
            format!("{} ({} bytes)", output.display(), compiled.wasm.bytes.len()),
        );
    }

    if emit.contains(&Emit::Wat) {
        let wat_path = wat
            .flatten()
            .unwrap_or_else(|| artifact_path(out_dir.as_deref(), &output, artifact_base, "wat"));
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

fn final_wasm_path(
    output: Option<PathBuf>, out_dir: Option<&Path>, root: &Path, artifact_base: &str,
) -> Result<PathBuf, &'static str> {
    match (output, out_dir) {
        (Some(_), Some(_)) => Err("--output and --out-dir cannot be used together"),
        (Some(path), None) => Ok(path),
        (None, Some(dir)) => Ok(dir.join(format!("{artifact_base}.wasm"))),
        (None, None) => Ok(root.join("build").join(format!("{artifact_base}.wasm"))),
    }
}

fn artifact_path(out_dir: Option<&Path>, wasm_path: &Path, artifact_base: &str, extension: &str) -> PathBuf {
    out_dir
        .map(|dir| dir.join(format!("{artifact_base}.{extension}")))
        .unwrap_or_else(|| wasm_path.with_extension(extension))
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

fn write_project_debug_dumps(
    dump_dir: &Path, ir: &compiler_core::ir::Module, wasm: &compiler_core::wasm::WasmModule,
) -> std::io::Result<()> {
    fs::create_dir_all(dump_dir)?;
    fs::write(dump_dir.join("ir.txt"), format!("{ir:#?}\n"))?;
    fs::write(dump_dir.join("wat.wat"), &wasm.wat)?;
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
