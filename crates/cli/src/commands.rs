mod builder;
mod compiler;
mod debug;
mod runner;

use std::fmt::Write as _;
use std::fs;
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use wasmtime::{Caller, Engine, Linker, Module, Store, Val, ValType};

use compiler_core::source::SourceFile;
use compiler_core::types::Type;
use compiler_core::{diagnostic::Diagnostics, target::CompileTarget};

use crate::args::DebugOptions;

use super::args::{Command, DebugCommand};
use super::echo;

use builder::Builder;
use compiler::Compiler;
use debug::Debugger;
use runner::Runner;

pub struct CompiledModule {
    ast: compiler_core::ast::Module,
    resolved: compiler_core::resolve::ResolvedModule,
    typed: compiler_core::types::TypedModule,
    ir: compiler_core::ir::Module,
    wasm: compiler_core::wasm::WasmModule,
}

impl CompiledModule {
    fn with_dumps(source: SourceFile, target: CompileTarget) -> Result<Self, Diagnostics> {
        let cst = compiler_core::parse::parse(source)?;
        let ast = compiler_core::ast::build(&cst)?;
        let ast = compiler_core::target::select_module(ast, target)?;
        let resolved = compiler_core::resolve::resolve(ast.clone())?;
        let typed = compiler_core::types::check(resolved.clone())?;
        let ir = compiler_core::ir::lower(typed.clone())?;
        let wasm = ir.emit_wasm_with_options(target.into())?;
        Ok(Self { ast, resolved, typed, ir, wasm })
    }
}

pub fn run(command: Command, no_color: bool) -> ExitCode {
    match command {
        Command::Build { project, output, out_dir, target, emit, wat, dump_dir, verbose, json } => {
            let mut builder =
                Builder { input: project.as_deref(), output, out_dir, target, emit, wat, dump_dir, verbose, json };

            builder.build()
        }
        Command::Compile { input, output, out_dir, target, emit, wat, dump_dir, verbose, json } => {
            let mut compiler = Compiler { input: &input, output, out_dir, wat, dump_dir, target, emit, verbose, json };
            compiler.compile()
        }
        Command::Run { input, function, args, target, verbose, json } => {
            Runner::new(&input, &function, &args, target, verbose, json).run()
        }
        Command::Debug { view, input, tree_sitter, ast, spans, json, no_color: debug_no_color } => run_debug(
            view,
            input.as_deref(),
            DebugOptions::new(tree_sitter, ast, spans, json, no_color || debug_no_color),
            no_color,
        ),
        Command::List { project } => list(project.as_deref().unwrap_or_else(|| Path::new("."))),
    }
}

fn run_debug(view: Option<DebugCommand>, input: Option<&Path>, opts: DebugOptions, no_color: bool) -> ExitCode {
    match view {
        Some(DebugCommand::Ts(args)) => Debugger::new(
            &args.input,
            DebugOptions::new(true, false, false, args.json, no_color || args.no_color),
        )
        .run(),
        Some(DebugCommand::Spans(args)) => Debugger::new(
            &args.input,
            DebugOptions::new(true, false, true, args.json, no_color || args.no_color),
        )
        .run(),
        Some(DebugCommand::Ast(args)) => Debugger::new(
            &args.input,
            DebugOptions::new(false, true, false, args.json, no_color || args.no_color),
        )
        .run(),
        Some(DebugCommand::Json(args)) => Debugger::new(
            &args.input,
            DebugOptions::new(
                args.tree_sitter || args.spans || !args.ast,
                args.ast,
                args.spans,
                true,
                false,
            ),
        )
        .run(),
        Some(DebugCommand::Ir(args)) => debug::ProjectIrDebugger::new(&args.input, no_color || args.no_color).run(),
        None => match input {
            Some(input) => Debugger::new(input, opts).run(),
            None => {
                echo::error("debug requires a subcommand or a source file with at least one view flag");
                ExitCode::FAILURE
            }
        },
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

fn run_wasm_export(bytes: &[u8], function: &str, args: &[String], return_type: Option<&Type>) -> Result<(), String> {
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
    let arena_mark = instance.get_typed_func::<(), i32>(&mut store, "__arena_mark").ok();
    let arena_reset = instance.get_typed_func::<i32, ()>(&mut store, "__arena_reset").ok();
    let mark = match &arena_mark {
        Some(mark) => Some(mark.call(&mut store, ()).map_err(|error| error.to_string())?),
        None => None,
    };
    let call_result = func
        .call(&mut store, &values, &mut result_values)
        .map_err(|error| error.to_string())
        .and_then(|_| format_wasm_results(&instance, &mut store, &result_values, return_type));
    let reset_result = reset_arena(&mut store, arena_reset.as_ref(), mark);
    let formatted = match (call_result, reset_result) {
        (Ok(formatted), Ok(())) => formatted,
        (Err(error), Ok(())) => return Err(error),
        (Ok(_), Err(error)) | (Err(_), Err(error)) => return Err(error),
    };

    for result in formatted {
        println!("{result}");
    }
    Ok(())
}

fn reset_arena(
    store: &mut wasmtime::Store<()>, arena_reset: Option<&wasmtime::TypedFunc<i32, ()>>, mark: Option<i32>,
) -> Result<(), String> {
    let (Some(reset), Some(mark)) = (arena_reset, mark) else {
        return Ok(());
    };
    reset.call(store, mark).map_err(|error| error.to_string())
}

fn parse_wasm_arg(arg: &str, type_: &ValType) -> Result<Val, String> {
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

fn default_wasm_value(type_: &ValType) -> Result<Val, String> {
    match type_ {
        ValType::I32 => Ok(Val::I32(0)),
        ValType::I64 => Ok(Val::I64(0)),
        ValType::F32 => Ok(Val::F32(0)),
        ValType::F64 => Ok(Val::F64(0)),
        other => Err(format!("cannot print Wasm result type `{other}`")),
    }
}

fn format_wasm_value(value: &Val) -> String {
    match value {
        Val::I32(value) => value.to_string(),
        Val::I64(value) => value.to_string(),
        Val::F32(value) => f32::from_bits(*value).to_string(),
        Val::F64(value) => f64::from_bits(*value).to_string(),
        other => format!("{other:?}"),
    }
}

fn format_wasm_results(
    instance: &wasmtime::Instance, store: &mut Store<()>, values: &[Val], return_type: Option<&Type>,
) -> Result<Vec<String>, String> {
    if values.len() == 1
        && let Some(type_) = return_type
        && let Some(value) = values.first()
        && let Some(formatted) = format_typed_wasm_value(instance, store, value, type_)?
    {
        return Ok(vec![formatted]);
    }
    Ok(values.iter().map(format_wasm_value).collect())
}

fn format_typed_wasm_value(
    instance: &wasmtime::Instance, store: &mut Store<()>, value: &Val, type_: &Type,
) -> Result<Option<String>, String> {
    match (type_, value) {
        (Type::String, Val::I32(ptr)) => read_memory_string(instance, store, *ptr).map(Some),
        (
            Type::BitArray
            | Type::Tuple(_)
            | Type::List(_)
            | Type::Record { .. }
            | Type::Custom { .. }
            | Type::Opaque { .. }
            | Type::Function { .. }
            | Type::Generic(_),
            Val::I32(ptr),
        ) => read_memory_debug(instance, store, *ptr).map(Some),
        _ => Ok(None),
    }
}

fn read_memory_string(instance: &wasmtime::Instance, store: &mut Store<()>, ptr: i32) -> Result<String, String> {
    let memory = instance
        .get_memory(&mut *store, "memory")
        .ok_or_else(|| "missing memory export".to_string())?;
    let ptr = ptr as usize;
    let mut header = [0; 8];

    memory
        .read(&mut *store, ptr, &mut header)
        .map_err(|error| error.to_string())?;

    if u32::from_le_bytes(header[0..4].try_into().expect("string tag header")) != 1 {
        return Err("managed return value is not a string".into());
    }
    let len = u32::from_le_bytes(header[4..8].try_into().expect("string length header")) as usize;
    let mut bytes = vec![0; len];

    memory
        .read(&mut *store, ptr + 8, &mut bytes)
        .map_err(|error| error.to_string())?;
    String::from_utf8(bytes).map_err(|error| error.to_string())
}

fn read_memory_debug(instance: &wasmtime::Instance, store: &mut Store<()>, ptr: i32) -> Result<String, String> {
    let memory = instance
        .get_memory(&mut *store, "memory")
        .ok_or_else(|| "missing memory export".to_string())?;
    let data = memory.data(&mut *store);
    compiler_core::runtime::debug_render(data, ptr as u32).ok_or_else(|| "could not decode managed return value".into())
}

fn read_host_string(caller: &mut Caller<'_, ()>, ptr: i32) -> String {
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

fn artifact_path(out_dir: Option<&Path>, wasm_path: &Path, artifact_base: &str, ext: &str) -> PathBuf {
    out_dir
        .map(|dir| dir.join(format!("{artifact_base}.{ext}")))
        .unwrap_or_else(|| wasm_path.with_extension(ext))
}

fn runtime_debug_dump() -> String {
    use compiler_core::runtime::{ObjectTag, RuntimeConfig, WASM_PAGE_SIZE};

    let config = RuntimeConfig::DEFAULT;
    let layout = config.layout;
    let mut out = String::new();
    writeln!(&mut out, "runtime layout:").expect("write runtime debug dump");
    writeln!(&mut out, "  word_size: {}", layout.word_size).expect("write runtime debug dump");
    writeln!(&mut out, "  alignment: {}", layout.alignment).expect("write runtime debug dump");
    writeln!(&mut out, "  header_size: {}", layout.header_size).expect("write runtime debug dump");
    writeln!(&mut out).expect("write runtime debug dump");
    writeln!(&mut out, "memory:").expect("write runtime debug dump");
    writeln!(&mut out, "  wasm_page_size: {WASM_PAGE_SIZE}").expect("write runtime debug dump");
    writeln!(&mut out, "  static_data_start: {}", config.static_data_start).expect("write runtime debug dump");
    writeln!(&mut out, "  heap_start: {}", config.heap_start).expect("write runtime debug dump");
    writeln!(&mut out, "  memory_max_pages: {}", config.memory_max_pages).expect("write runtime debug dump");
    writeln!(&mut out, "  memory_limit_bytes: {}", config.memory_limit_bytes()).expect("write runtime debug dump");
    writeln!(&mut out).expect("write runtime debug dump");
    writeln!(&mut out, "object tags:").expect("write runtime debug dump");
    for tag in [
        ObjectTag::String,
        ObjectTag::ListCons,
        ObjectTag::Tuple,
        ObjectTag::Record,
        ObjectTag::Custom,
        ObjectTag::Closure,
        ObjectTag::BitArray,
        ObjectTag::Opaque,
        ObjectTag::Error,
        ObjectTag::Panic,
    ] {
        writeln!(&mut out, "  {:?}: {}", tag, u32::from(tag)).expect("write runtime debug dump");
    }
    writeln!(&mut out).expect("write runtime debug dump");
    writeln!(&mut out, "sample object sizes:").expect("write runtime debug dump");
    writeln!(&mut out, "  string(5): {}", layout.string_size(5)).expect("write runtime debug dump");
    writeln!(&mut out, "  bit_array(13): {}", layout.bit_array_size(13)).expect("write runtime debug dump");
    writeln!(&mut out, "  list_cons: {}", layout.list_cons_size(8)).expect("write runtime debug dump");
    writeln!(&mut out, "  tuple(2): {}", layout.tuple_size(2, 8)).expect("write runtime debug dump");
    writeln!(&mut out, "  record(2): {}", layout.record_size(2, 8)).expect("write runtime debug dump");
    writeln!(&mut out, "  custom(2): {}", layout.custom_size(2, 8)).expect("write runtime debug dump");
    writeln!(&mut out, "  closure(2): {}", layout.closure_size(2)).expect("write runtime debug dump");
    writeln!(&mut out, "  opaque: {}", layout.opaque_size()).expect("write runtime debug dump");
    out
}

fn abi_debug_dump(module: &compiler_core::ir::Module, target: CompileTarget) -> String {
    use compiler_core::ir::{CallBoundary, ExportKind};

    let mut out = String::new();
    writeln!(&mut out, "target: {target:?}").expect("write ABI debug dump");
    writeln!(&mut out).expect("write ABI debug dump");

    let mut imports = module
        .functions
        .iter()
        .filter(|function| {
            matches!(
                function.abi.boundary,
                CallBoundary::HostImport { .. } | CallBoundary::ModuleImport { .. }
            )
        })
        .collect::<Vec<_>>();
    imports.sort_by_key(|function| function.name.as_str());
    writeln!(&mut out, "imports:").expect("write ABI debug dump");
    if imports.is_empty() {
        writeln!(&mut out, "  none").expect("write ABI debug dump");
    }
    for function in imports {
        writeln!(
            &mut out,
            "  {} {}",
            function.name,
            function_abi_signature(&function.params, &function.return_type)
        )
        .expect("write ABI debug dump");
        writeln!(&mut out, "    boundary: {}", call_boundary(&function.abi.boundary)).expect("write ABI debug dump");
        write_call_abi(&mut out, &function.abi);
    }

    let mut exports = module
        .exports
        .iter()
        .filter(|export| export.kind == ExportKind::Function)
        .collect::<Vec<_>>();
    exports.sort_by_key(|export| export.name.as_str());
    writeln!(&mut out).expect("write ABI debug dump");
    writeln!(&mut out, "exports:").expect("write ABI debug dump");
    if exports.is_empty() {
        writeln!(&mut out, "  none").expect("write ABI debug dump");
    }
    for export in exports {
        let Some(function) = module
            .functions
            .iter()
            .find(|function| function.name == export.backend_name())
        else {
            writeln!(
                &mut out,
                "  {} -> {} (missing function)",
                export.name,
                export.backend_name()
            )
            .expect("write ABI debug dump");
            continue;
        };
        writeln!(
            &mut out,
            "  {} -> {} {}",
            export.name,
            export.backend_name(),
            function_abi_signature(&function.params, &function.return_type)
        )
        .expect("write ABI debug dump");
        write_call_abi(&mut out, &function.abi);
    }

    out
}

fn function_abi_signature(params: &[compiler_core::ir::Local], return_type: &Type) -> String {
    let params = params
        .iter()
        .map(|param| format!("{}: {:?}", param.name, param.type_))
        .collect::<Vec<_>>()
        .join(", ");
    format!("({params}) -> {return_type:?}")
}

fn write_call_abi(out: &mut String, abi: &compiler_core::ir::CallAbi) {
    writeln!(out, "    params:").expect("write ABI debug dump");
    if abi.params.is_empty() {
        writeln!(out, "      none").expect("write ABI debug dump");
    }
    for (index, param) in abi.params.iter().enumerate() {
        writeln!(out, "      {index}: {:?} as {:?}", param.type_, param.representation).expect("write ABI debug dump");
    }
    match &abi.return_ {
        Some(return_) => writeln!(out, "    result: {:?} as {:?}", return_.type_, return_.representation)
            .expect("write ABI debug dump"),
        None => writeln!(out, "    result: none").expect("write ABI debug dump"),
    }
}

fn call_boundary(boundary: &compiler_core::ir::CallBoundary) -> String {
    match boundary {
        compiler_core::ir::CallBoundary::Internal => "internal".into(),
        compiler_core::ir::CallBoundary::ModuleExport => "module export".into(),
        compiler_core::ir::CallBoundary::ModuleImport { module, name } => format!("module import {module}.{name}"),
        compiler_core::ir::CallBoundary::HostImport { module, name } => format!("host import {module}.{name}"),
    }
}

fn write_file(path: &Path, contents: &[u8]) -> std::io::Result<()> {
    if let Some(parent) = path.parent()
        && !parent.as_os_str().is_empty()
    {
        fs::create_dir_all(parent)?;
    }
    fs::write(path, contents)
}
