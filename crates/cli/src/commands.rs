use std::fs;
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use compiler_core::source::{SourceFile, SourceFileId};
use compiler_core::{diagnostic::Diagnostics, target::CompileTarget};

use crate::args::{Command, Emit, Target};
use crate::echo;

pub fn run(command: Command) -> ExitCode {
    match command {
        Command::Build { project, output, out_dir, target, emit, dump_dir, verbose, json } => {
            let mut builder =
                Builder { input: project.as_deref(), output, out_dir, target, emit, dump_dir, verbose, json };

            builder.build()
        }
        Command::Compile { input, output, out_dir, target, emit, wat, dump_dir, verbose, json } => {
            let mut compiler = Compiler { input: &input, output, out_dir, wat, dump_dir, target, emit, verbose, json };
            compiler.compile()
        }
        Command::Run { input, function, args, target, verbose, json } => {
            let runner = Runner { input: &input, function: &function, args: &args, target, verbose, json };
            runner.run()
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

struct Builder<'a> {
    input: Option<&'a Path>,
    output: Option<PathBuf>,
    out_dir: Option<PathBuf>,
    target: Option<Target>,
    emit: Vec<Emit>,
    dump_dir: Option<PathBuf>,
    verbose: bool,
    json: bool,
}

impl Builder<'_> {
    pub fn build(&mut self) -> ExitCode {
        if self.json {
            return echo::fail("build", "--json", "machine-readable output is not implemented yet");
        }
        let input = self.input.unwrap_or_else(|| Path::new("."));
        let project = match compiler_core::project::load_project(input) {
            Ok(project) => project,
            Err(diagnostics) => return echo::fail_with_diagnostics("load project", input.display(), &diagnostics),
        };
        if self.verbose {
            for module in &project.graph.modules {
                echo::status("compile", format!("{} -> {}", module.name, module.path.display()));
            }
        }

        let target = self
            .target
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
        let output = match final_wasm_path(
            self.output.clone(),
            self.out_dir.as_deref(),
            &project.root,
            &artifact_base,
        ) {
            Ok(path) => path,
            Err(message) => return echo::fail("build", "output", message),
        };

        if self.emit.contains(&Emit::Wasm) {
            if let Err(error) = write_file(&output, &wasm.bytes) {
                return echo::fail("write", output.display(), error);
            }
            echo::status("wasm", format!("{} ({} bytes)", output.display(), wasm.bytes.len()));
        }
        if self.emit.contains(&Emit::Wat) {
            let wat_path = artifact_path(self.out_dir.as_deref(), &output, &artifact_base, "wat");
            if let Err(error) = write_file(&wat_path, wasm.wat.as_bytes()) {
                return echo::fail("write", wat_path.display(), error);
            }
            echo::status("wat", wat_path.display().to_string());
        }
        if let Some(dump_dir) = self.dump_dir.clone()
            && let Err(error) = write_project_debug_dumps(&dump_dir, &ir, &wasm)
        {
            return echo::fail("write", "debug dumps", error);
        }

        ExitCode::SUCCESS
    }
}

struct Compiler<'a> {
    input: &'a Path,
    output: Option<PathBuf>,
    out_dir: Option<PathBuf>,
    wat: Option<Option<PathBuf>>,
    dump_dir: Option<PathBuf>,
    target: Target,
    emit: Vec<Emit>,
    verbose: bool,
    json: bool,
}

impl Compiler<'_> {
    pub fn compile(&mut self) -> ExitCode {
        if self.json {
            return echo::fail("compile", "--json", "machine-readable output is not implemented yet");
        }
        if self.wat.is_some() && !self.emit.contains(&Emit::Wat) {
            self.emit.push(Emit::Wat);
        }
        if self.verbose {
            echo::status("compile", self.input.display().to_string());
        }
        let source = match fs::read_to_string(self.input) {
            Ok(source) => SourceFile::with_path(SourceFileId(0), self.input, source),
            Err(error) => return echo::fail("read", self.input.display(), error),
        };

        let compiled = match compile_with_dumps(source, self.target.into()) {
            Ok(compiled) => compiled,
            Err(diagnostics) => return echo::fail_with_diagnostics("compile", self.input.display(), &diagnostics),
        };

        if let Some(dump_dir) = self.dump_dir.clone()
            && let Err(error) = write_debug_dumps(&dump_dir, &compiled)
        {
            return echo::fail("write", "debug dumps", error);
        }

        let artifact_base = self
            .input
            .file_stem()
            .and_then(|stem| stem.to_str())
            .unwrap_or("module");
        let output = match (self.output.clone(), self.out_dir.as_deref()) {
            (Some(_), Some(_)) => {
                return echo::fail("compile", "output", "--output and --out-dir cannot be used together");
            }
            (Some(path), None) => path,
            (None, Some(dir)) => dir.join(format!("{artifact_base}.wasm")),
            (None, None) => self.input.with_extension("wasm"),
        };

        if self.emit.contains(&Emit::Wasm) {
            if let Err(error) = write_file(&output, &compiled.wasm.bytes) {
                return echo::fail("write", output.display(), error);
            }
            echo::status(
                "wasm",
                format!("{} ({} bytes)", output.display(), compiled.wasm.bytes.len()),
            );
        }

        if self.emit.contains(&Emit::Wat) {
            let wat_path = self
                .wat
                .clone()
                .flatten()
                .unwrap_or_else(|| artifact_path(self.out_dir.as_deref(), &output, artifact_base, "wat"));
            if let Err(error) = write_file(&wat_path, compiled.wasm.wat.as_bytes()) {
                return echo::fail("write", wat_path.display(), error);
            }
            echo::status("wat", wat_path.display().to_string());
        }

        ExitCode::SUCCESS
    }
}

struct Runner<'a> {
    input: &'a Path,
    function: &'a str,
    args: &'a [String],
    target: Target,
    verbose: bool,
    json: bool,
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
        let compiled = match compile_with_dumps(source, self.target.into()) {
            Ok(compiled) => compiled,
            Err(diagnostics) => return echo::fail_with_diagnostics("compile", self.input.display(), &diagnostics),
        };

        match run_wasm_export(&compiled.wasm.bytes, self.function, self.args) {
            Ok(()) => ExitCode::SUCCESS,
            Err(message) => echo::fail("run", self.function, message),
        }
    }
}

fn run_wasm_export(bytes: &[u8], function: &str, args: &[String]) -> Result<(), String> {
    use wasmtime::{Caller, Engine, Linker, Module, Store};

    let engine = Engine::default();
    let module = Module::new(&engine, bytes).map_err(|error| error.to_string())?;
    let mut linker = Linker::new(&engine);
    linker
        .func_wrap("env", "print", |mut caller: Caller<'_, ()>, ptr: i32| {
            print!("{}", read_host_string(&mut caller, ptr));
            let _ = io::stdout().flush();
        })
        .map_err(|error| error.to_string())?;
    linker
        .func_wrap("env", "println", |mut caller: Caller<'_, ()>, ptr: i32| {
            println!("{}", read_host_string(&mut caller, ptr));
        })
        .map_err(|error| error.to_string())?;
    linker
        .func_wrap("env", "debug_i64", |value: i64| println!("{value}"))
        .map_err(|error| error.to_string())?;
    linker
        .func_wrap("env", "debug_value", |mut caller: Caller<'_, ()>, ptr: i32| {
            println!("{}", read_host_string(&mut caller, ptr));
        })
        .map_err(|error| error.to_string())?;

    let mut store = Store::new(&engine, ());
    let instance = linker
        .instantiate(&mut store, &module)
        .map_err(|error| error.to_string())?;
    let func = instance
        .get_func(&mut store, function)
        .ok_or_else(|| format!("export `{function}` was not found"))?;
    let ty = func.ty(&store);
    let params = ty.params().collect::<Vec<_>>();
    let results = ty.results().collect::<Vec<_>>();
    if params.len() != args.len() {
        return Err(format!(
            "export `{function}` expects {} argument(s), got {}",
            params.len(),
            args.len()
        ));
    }
    let values = args
        .iter()
        .zip(params.iter())
        .map(|(arg, type_)| parse_wasm_arg(arg, type_))
        .collect::<Result<Vec<_>, _>>()?;
    let mut result_values = results.iter().map(default_wasm_value).collect::<Result<Vec<_>, _>>()?;
    func.call(&mut store, &values, &mut result_values)
        .map_err(|error| error.to_string())?;

    for result in result_values {
        println!("{}", format_wasm_value(&result));
    }
    Ok(())
}

fn parse_wasm_arg(arg: &str, type_: &wasmtime::ValType) -> Result<wasmtime::Val, String> {
    use wasmtime::{Val, ValType};

    match type_ {
        ValType::I32 => Ok(Val::I32(arg.parse::<i32>().map_err(|error| error.to_string())?)),
        ValType::I64 => Ok(Val::I64(arg.parse::<i64>().map_err(|error| error.to_string())?)),
        ValType::F32 => Ok(Val::F32(
            arg.parse::<f32>().map_err(|error| error.to_string())?.to_bits(),
        )),
        ValType::F64 => Ok(Val::F64(
            arg.parse::<f64>().map_err(|error| error.to_string())?.to_bits(),
        )),
        other => Err(format!("cannot pass CLI argument for Wasm type `{other}`")),
    }
}

fn default_wasm_value(type_: &wasmtime::ValType) -> Result<wasmtime::Val, String> {
    use wasmtime::{Val, ValType};

    match type_ {
        ValType::I32 => Ok(Val::I32(0)),
        ValType::I64 => Ok(Val::I64(0)),
        ValType::F32 => Ok(Val::F32(0)),
        ValType::F64 => Ok(Val::F64(0)),
        other => Err(format!("cannot print Wasm result type `{other}`")),
    }
}

fn format_wasm_value(value: &wasmtime::Val) -> String {
    match value {
        wasmtime::Val::I32(value) => value.to_string(),
        wasmtime::Val::I64(value) => value.to_string(),
        wasmtime::Val::F32(value) => f32::from_bits(*value).to_string(),
        wasmtime::Val::F64(value) => f64::from_bits(*value).to_string(),
        other => format!("{other:?}"),
    }
}

fn read_host_string(caller: &mut wasmtime::Caller<'_, ()>, ptr: i32) -> String {
    let Some(memory) = caller.get_export("memory").and_then(|export| export.into_memory()) else {
        return "<missing memory export>".into();
    };
    let ptr = ptr as usize;
    let mut header = [0; 8];
    if memory.read(&mut *caller, ptr, &mut header).is_err() {
        return "<invalid string header>".into();
    }
    let len = u32::from_le_bytes(header[4..8].try_into().expect("string length header")) as usize;
    let mut bytes = vec![0; len];
    if memory.read(&mut *caller, ptr + 8, &mut bytes).is_err() {
        return "<invalid string data>".into();
    }
    String::from_utf8(bytes).unwrap_or_else(|_| "<invalid utf-8 string>".into())
}

struct CompiledModule {
    ast: compiler_core::ast::Module,
    resolved: compiler_core::resolve::ResolvedModule,
    typed: compiler_core::types::TypedModule,
    ir: compiler_core::ir::Module,
    wasm: compiler_core::wasm::WasmModule,
}

fn compile_with_dumps(source: SourceFile, target: CompileTarget) -> Result<CompiledModule, Diagnostics> {
    let cst = compiler_core::parse::parse(source)?;
    let ast = compiler_core::ast::build(&cst)?;
    let ast = compiler_core::target::select_module(ast, target)?;
    let resolved = compiler_core::resolve::resolve(ast.clone())?;
    let typed = compiler_core::types::check(resolved.clone())?;
    let ir = compiler_core::ir::lower(typed.clone())?;
    let wasm = ir.emit_wasm_with_options(target.into())?;
    Ok(CompiledModule { ast, resolved, typed, ir, wasm })
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
