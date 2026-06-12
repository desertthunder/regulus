use super::*;
use crate::ast::LiteralKind;
use crate::ir::ExpressionKind;
use crate::runtime::ObjectTag;
use crate::source::{SourceFile, SourceFileId, Span};
use crate::types::Type;
use crate::{ast, ir, parse, resolve, types};
use wasmtime::{Caller, Engine, Instance, Linker, Memory as WasmtimeMemory, Module, Store};

fn lower_ir(source: &str) -> ir::Module {
    let source = SourceFile::new(SourceFileId(0), source);
    let cst = parse::parse(source).expect("parse source");
    let ast = ast::build(&cst).expect("build ast");
    let resolved = resolve::resolve(ast).expect("resolve names");
    let typed = types::check(resolved).expect("type check source");
    ir::lower(typed).expect("lower source")
}

fn compile_wasm(source: &str) -> WasmModule {
    lower_ir(source).emit_wasm().expect("emit wasm")
}

fn compile_wasm_target(source: &str, target: CompileTarget) -> Result<WasmModule, Diagnostics> {
    crate::compile_source_with_options(
        SourceFile::new(SourceFileId(0), source),
        crate::CompileOptions { target },
    )
    .map(|output| output.wasm)
}

fn int_expr(source: &str, span: Span) -> ir::Expression {
    ir::Expression {
        type_: Type::Int,
        span,
        kind: ExpressionKind::Literal(ir::Literal { kind: LiteralKind::Int, source: source.into() }),
    }
}

fn int_body(source: &str, span: Span) -> ir::Block {
    ir::Block { instructions: Vec::new(), result: Box::new(int_expr(source, span)), span }
}

fn ir_module(functions: Vec<ir::Function>, span: Span) -> ir::Module {
    ir::Module {
        span,
        identity: None,
        imports: Vec::new(),
        declarations: Vec::new(),
        constants: Vec::new(),
        init: ir::ModuleInit::default(),
        references: Vec::new(),
        exports: Vec::new(),
        functions,
        linked_names: Vec::new(),
    }
}

fn host_import_module(span: Span) -> ir::Module {
    let import_abi = ir::CallAbi {
        params: vec![ir::AbiValue::from(&Type::Int)],
        return_: Some(ir::AbiValue::from(&Type::Int)),
        boundary: ir::CallBoundary::HostImport { module: "env".into(), name: "inc".into() },
    };
    let imported = ir::Function {
        closure_captures: Vec::new(),
        name: "host_inc".into(),
        public: false,
        params: vec![ir::Local { id: ir::LocalId(0), name: "x".into(), type_: Type::Int, span }],
        locals: vec![ir::Local { id: ir::LocalId(0), name: "x".into(), type_: Type::Int, span }],
        return_type: Type::Int,
        abi: import_abi.clone(),
        body: int_body("0", span),
        span,
    };
    let exported = ir::Function {
        closure_captures: Vec::new(),
        name: "main".into(),
        public: true,
        params: Vec::new(),
        locals: Vec::new(),
        return_type: Type::Int,
        abi: ir::CallAbi {
            params: Vec::new(),
            return_: Some(ir::AbiValue::from(&Type::Int)),
            boundary: ir::CallBoundary::ModuleExport,
        },
        body: ir::Block {
            instructions: Vec::new(),
            result: Box::new(ir::Expression {
                type_: Type::Int,
                span,
                kind: ExpressionKind::DirectCall(ir::DirectCall {
                    function: "host_inc".into(),
                    arguments: vec![ir::CallArgument { label: None, value: int_expr("41", span), span }],
                    abi: import_abi,
                }),
            }),
            span,
        },
        span,
    };
    ir_module(vec![imported, exported], span)
}

fn bit_array_expr(values: &[u64], span: Span) -> ir::Expression {
    let segments = values
        .iter()
        .map(|value| ir::BitArraySegment {
            value: *value,
            bit_size: 8,
            type_: ir::BitSegmentType::Integer,
            options: Vec::new(),
            span,
        })
        .collect::<Vec<_>>();
    ir::Expression {
        type_: Type::BitArray,
        span,
        kind: ExpressionKind::BitArray(ir::BitArrayLiteral { bit_len: values.len() as u32 * 8, segments }),
    }
}

#[test]
fn emits_wat_for_public_scalar_function() {
    let wasm = compile_wasm("pub fn id(x: Int) -> Int { x }");

    insta::assert_snapshot!(wasm.wat, @r#"
(module
  (type (func (param i64) (result i64)))
  (func $id (type 0) (param i64) (result i64)
    local.get 0
  )
  (export "id" (func 0))
)
"#);
    assert!(!wasm.bytes.is_empty());
}

#[test]
fn emits_wat_with_runtime_for_string_function() {
    let wasm = compile_wasm("pub fn greeting() { \"hello\" }");

    assert!(wasm.wat.contains("(memory 1)"));
    assert!(!wasm.wat.contains("(func $__alloc"));
    assert!(!wasm.wat.contains("(export \"__regulus_string_len\")"));
    assert!(!wasm.wat.contains("(export \"__regulus_value_tag\")"));
    assert!(wasm.wat.contains("(func $greeting (type 0) (result i32)"));
    assert!(wasm.wat.contains("(export \"greeting\" (func 0))"));
    assert!(wasm.wat.contains(&format!(
        "i32.const {}",
        runtime::RuntimeConfig::DEFAULT.static_data_start
    )));
}

#[test]
fn omits_unreachable_runtime_fragment_domains() {
    let wasm = compile_wasm("pub fn join() { \"a\" <> \"b\" }");

    assert!(!wasm.wat.contains("(func $__string_concat"), "{}", wasm.wat);
    assert!(!wasm.wat.contains("(func $__alloc"), "{}", wasm.wat);
    assert!(!wasm.wat.contains("(func $__dict_new"), "{}", wasm.wat);
    assert!(!wasm.wat.contains("(func $__list_cons"), "{}", wasm.wat);
    assert!(!wasm.wat.contains("(func $__bit_array_new"), "{}", wasm.wat);
}

#[test]
fn includes_transitive_runtime_fragment_dependencies() {
    let wasm = compile_wasm("pub fn join() { \"a\" <> \"b\" }");

    assert!(!wasm.wat.contains("$__string_concat"), "{}", wasm.wat);
    assert!(!wasm.wat.contains("$__string_len"), "{}", wasm.wat);
    assert!(!wasm.wat.contains("$__string_data"), "{}", wasm.wat);
    assert!(!wasm.wat.contains("$__string_new"), "{}", wasm.wat);
    assert!(!wasm.wat.contains("$__copy_bytes"), "{}", wasm.wat);
}

#[test]
fn renders_deterministic_structured_wat_for_managed_values() {
    let wasm = compile_wasm("pub fn pair() { #(1, 2) }");

    insta::assert_snapshot!(wasm.wat, @r#"
(module
  (type (func (result i32)))
  (memory 1)
  (func $pair (type 0) (result i32)
    i32.const 1024
  )
  (export "pair" (func 0))
  (export "memory" (memory 0))
  (data (memory 0) (offset i32.const 1024) "\03\00\00\00\02\00\00\00\01\00\00\00\00\00\00\00\02\00\00\00\00\00\00\00")
)
"#);
}

#[test]
fn structured_codegen_runs_dynamic_tuple_and_list_literals() {
    let wasm = compile_wasm(
        r#"import gleam/list

pub fn first(x: Int) -> Int {
case #(x, 2) {
 #(left, _) -> left
}
}
pub fn count(x: Int) -> Int { list.length([x, 2, 3]) }
"#,
    );

    assert!(wasm.wat.contains("(global $__heap (mut i32)"), "{}", wasm.wat);
    assert!(!wasm.wat.contains("structured Wasm emitter does not support"));

    let engine = Engine::default();
    let module = Module::new(&engine, &wasm.bytes).expect("compile wasm module");
    let mut store = Store::new(&engine, ());
    let instance = Instance::new(&mut store, &module, &[]).expect("instantiate module");
    let first = instance
        .get_typed_func::<i64, i64>(&mut store, "first")
        .expect("get first export");
    assert_eq!(first.call(&mut store, 41).expect("call first"), 41);
    let count = instance
        .get_typed_func::<i64, i64>(&mut store, "count")
        .expect("get count export");
    assert_eq!(count.call(&mut store, 41).expect("call count"), 3);
}

#[test]
fn structured_codegen_runs_dynamic_record_literals() {
    let span = Span::new(SourceFileId(0), 0, 1);
    let record_type = Type::Record {
        name: "Point".into(),
        fields: vec![
            types::FieldInfo { name: "x".into(), type_: Type::Int },
            types::FieldInfo { name: "y".into(), type_: Type::Int },
        ],
    };
    let function = ir::Function {
        closure_captures: Vec::new(),
        name: "point".into(),
        public: true,
        params: vec![ir::Local { id: ir::LocalId(0), name: "x".into(), type_: Type::Int, span }],
        locals: vec![ir::Local { id: ir::LocalId(0), name: "x".into(), type_: Type::Int, span }],
        return_type: record_type.clone(),
        abi: ir::CallAbi {
            params: vec![ir::AbiValue::from(&Type::Int)],
            return_: Some(ir::AbiValue::from(&record_type)),
            boundary: ir::CallBoundary::ModuleExport,
        },
        body: ir::Block {
            instructions: Vec::new(),
            result: Box::new(ir::Expression {
                type_: record_type,
                span,
                kind: ExpressionKind::Record(ir::RecordValue {
                    name: "Point".into(),
                    fields: vec![
                        ir::RecordFieldValue {
                            name: "x".into(),
                            value: ir::Expression {
                                type_: Type::Int,
                                span,
                                kind: ExpressionKind::LocalGet(ir::LocalId(0)),
                            },
                        },
                        ir::RecordFieldValue { name: "y".into(), value: int_expr("2", span) },
                    ],
                }),
            }),
            span,
        },
        span,
    };
    let wasm = ir_module(vec![function], span).emit_wasm().expect("emit wasm");
    let engine = Engine::default();
    let module = Module::new(&engine, &wasm.bytes).expect("compile wasm module");
    let mut store = Store::new(&engine, ());
    let instance = Instance::new(&mut store, &module, &[]).expect("instantiate module");
    let point = instance
        .get_typed_func::<i64, i32>(&mut store, "point")
        .expect("get point export");
    let pointer = point.call(&mut store, 41).expect("call point") as usize;
    let memory = instance.get_memory(&mut store, "memory").expect("memory export");
    let mut bytes = [0; 24];
    memory.read(&store, pointer, &mut bytes).expect("read record");
    assert_eq!(
        u32::from_le_bytes(bytes[0..4].try_into().unwrap()),
        u32::from(ObjectTag::Record)
    );
    assert_eq!(u64::from_le_bytes(bytes[8..16].try_into().unwrap()), 41);
    assert_eq!(u64::from_le_bytes(bytes[16..24].try_into().unwrap()), 2);
}

#[test]
fn structured_codegen_runs_string_length_intrinsics() {
    let wasm = compile_wasm(
        r#"import gleam/string

pub fn text_len() -> Int { string.length("abc") }
pub fn text_empty() -> Bool { string.is_empty("") }
"#,
    );

    assert!(!wasm.wat.contains("$__string_len"), "{}", wasm.wat);

    let engine = Engine::default();
    let module = Module::new(&engine, &wasm.bytes).expect("compile wasm module");
    let mut store = Store::new(&engine, ());
    let instance = Instance::new(&mut store, &module, &[]).expect("instantiate module");
    let text_len = instance
        .get_typed_func::<(), i64>(&mut store, "text_len")
        .expect("get text_len");
    assert_eq!(text_len.call(&mut store, ()).expect("call text_len"), 3);
    let text_empty = instance
        .get_typed_func::<(), i32>(&mut store, "text_empty")
        .expect("get text_empty");
    assert_eq!(text_empty.call(&mut store, ()).expect("call text_empty"), 1);
}

#[test]
fn structured_codegen_ports_helper_backed_stdlib_intrinsics() {
    let cases = [
        ("int", "import gleam/int\npub fn number() { int.to_string(-42) }"),
        (
            "float",
            "import gleam/float\npub fn float_text() { float.to_string(1.5) }",
        ),
        (
            "string_concat",
            "import gleam/string\npub fn text_len() -> Int { string.length(string.concat([\"a\", \"bc\"])) }",
        ),
        (
            "list_length",
            "import gleam/list\npub fn item_count() -> Int { list.length([1, 2, 3]) }",
        ),
        (
            "list_reverse",
            "import gleam/list\npub fn reversed() { list.reverse([1, 2, 3]) }",
        ),
        (
            "bit_array_starts_with",
            "import gleam/bit_array\npub fn bits_start() -> Bool { bit_array.starts_with(<<1, 2, 3>>, <<1, 2>>) }",
        ),
        (
            "bit_array_concat",
            "import gleam/bit_array\npub fn bits_joined() -> BitArray { bit_array.concat([<<1>>, <<2>>, <<3>>]) }",
        ),
        (
            "bit_array_append",
            "import gleam/bit_array\npub fn bits_append_size() -> Int { bit_array.bit_size(bit_array.append(<<1>>, <<2>>)) }",
        ),
        (
            "dict",
            r#"import gleam/dict
pub fn dict_value() {
let values = dict.insert(dict.new(), "a", 42)
dict.get(values, "a")
}
pub fn dict_missing() -> Bool {
let values = dict.insert(dict.new(), "a", 42)
dict.has_key(dict.delete(values, "a"), "a")
}
pub fn dict_persistent_size() -> Int {
let original = dict.new()
let updated = dict.insert(original, "a", 42)
dict.size(original) + dict.size(updated)
}
"#,
        ),
    ];
    let mut helper_wat = String::new();
    for (name, source) in cases {
        let ir = lower_ir(source);
        let module =
            codegen::emit(&ir, EmitOptions::default()).unwrap_or_else(|_| panic!("{name} failed structured codegen"));
        helper_wat.push_str(&module.raw_wat_items.join("\n"));
    }
    for helper in [
        "$__int_to_string",
        "$__float_to_string",
        "$__string_concat_list",
        "$__list_length",
        "$__list_reverse",
        "$__bit_array_append",
        "$__bit_array_concat_list",
        "$__bit_array_match",
        "$__dict_new",
        "$__dict_insert",
        "$__dict_get",
        "$__dict_has_key",
        "$__dict_delete",
        "$__dict_size",
    ] {
        assert!(helper_wat.contains(helper), "missing {helper}\n{helper_wat}");
    }
}

#[test]
fn structured_codegen_runs_higher_order_allocation() {
    let wasm = compile_wasm(
        r#"import gleam/list

pub fn mapped_head() -> Int {
case list.map([1], fn(x) { x + 1 }) {
 [x] -> x
 _ -> 0
}
}
"#,
    );

    assert!(wasm.wat.contains("(global (mut i32)"), "{}", wasm.wat);
    assert!(!wasm.wat.contains("$__list_cons"), "{}", wasm.wat);
    assert!(!wasm.wat.contains("$__closure_new"), "{}", wasm.wat);

    let engine = Engine::default();
    let module = Module::new(&engine, &wasm.bytes).expect("compile wasm module");
    let mut store = Store::new(&engine, ());
    let instance = Instance::new(&mut store, &module, &[]).expect("instantiate module");
    let function = instance
        .get_typed_func::<(), i64>(&mut store, "mapped_head")
        .expect("get export");
    assert_eq!(function.call(&mut store, ()).expect("call export"), 2);
}

#[test]
fn structured_codegen_runs_nested_managed_patterns() {
    let wasm = compile_wasm(
        r#"import gleam/option.{Some}

pub fn nested_list() -> Int {
case [[1]] {
 [[x]] -> x
 _ -> 0
}
}

pub fn nested_option() -> Int {
case Some(Some(2)) {
 Some(Some(x)) -> x
 _ -> 0
}
}
"#,
    );

    assert!(!wasm.wat.contains("$__list_cons"), "{}", wasm.wat);

    let engine = Engine::default();
    let module = Module::new(&engine, &wasm.bytes).expect("compile wasm module");
    let mut store = Store::new(&engine, ());
    let instance = Instance::new(&mut store, &module, &[]).expect("instantiate module");
    for (name, expected) in [("nested_list", 1), ("nested_option", 2)] {
        let function = instance
            .get_typed_func::<(), i64>(&mut store, name)
            .expect("get export");
        assert_eq!(function.call(&mut store, ()).expect("call export"), expected, "{name}");
    }
}

fn exported_function_with_body(name: &str, return_type: &Type, result: ir::Expression, span: Span) -> ir::Function {
    ir::Function {
        closure_captures: Vec::new(),
        name: name.into(),
        public: true,
        params: Vec::new(),
        locals: Vec::new(),
        return_type: return_type.clone(),
        abi: ir::CallAbi {
            params: Vec::new(),
            return_: Some(ir::AbiValue::from(return_type)),
            boundary: ir::CallBoundary::ModuleExport,
        },
        body: ir::Block { instructions: Vec::new(), result: Box::new(result), span },
        span,
    }
}

fn assert_emit_wasm_error(module: &ir::Module, expected: &str, span: Span) {
    let errors = module.emit_wat().expect_err("invalid module should fail Wasm emission");
    assert!(
        errors.iter().any(|diagnostic| diagnostic.message.contains(expected)),
        "{errors:?}"
    );
    assert!(
        errors
            .iter()
            .any(|diagnostic| diagnostic.labels.iter().any(|label| label.span == span)),
        "{errors:?}"
    );
}

#[test]
fn literal_parse_failures_report_source_spanned_diagnostics() {
    let span = Span::new(SourceFileId(0), 3, 9);
    let result = ir::Expression {
        type_: Type::Int,
        span,
        kind: ExpressionKind::Literal(ir::Literal { kind: LiteralKind::Int, source: "nope".into() }),
    };
    let function = exported_function_with_body("bad", &Type::Int, result, span);
    assert_emit_wasm_error(&ir_module(vec![function], span), "invalid int literal", span);
}

#[test]
fn static_value_parse_failures_report_source_spanned_diagnostics() {
    let span = Span::new(SourceFileId(0), 10, 16);
    let bad_field = ir::Expression::new(
        Type::Int,
        span,
        ExpressionKind::Literal(ir::Literal { kind: LiteralKind::Int, source: "nope".into() }),
    );
    let result = ir::Expression::new(
        Type::Tuple(vec![Type::Int]),
        span,
        ExpressionKind::Tuple(vec![bad_field]),
    );
    let function = exported_function_with_body("bad_tuple", &Type::Tuple(vec![Type::Int]), result, span);
    assert_emit_wasm_error(&ir_module(vec![function], span), "invalid int literal", span);
}

#[test]
fn structured_codegen_ports_residual_ir_forms() {
    let span = Span::new(SourceFileId(0), 30, 40);
    let bit_concat = exported_function_with_body(
        "bits",
        &Type::BitArray,
        ir::Expression {
            type_: Type::BitArray,
            span,
            kind: ExpressionKind::BitArrayConcat {
                left: Box::new(bit_array_expr(&[1], span)),
                right: Box::new(bit_array_expr(&[2], span)),
            },
        },
        span,
    );
    let bit_test = exported_function_with_body(
        "is_bits",
        &Type::Bool,
        ir::Expression {
            type_: Type::Bool,
            span,
            kind: ExpressionKind::BitStringDeconstruct {
                bit_array: Box::new(bit_array_expr(&[1], span)),
                segments: Vec::new(),
            },
        },
        span,
    );
    let allocate = exported_function_with_body(
        "allocate",
        &Type::String,
        ir::Expression {
            type_: Type::String,
            span,
            kind: ExpressionKind::Memory(ir::MemoryOperation::Allocate { bytes: Box::new(int_expr("16", span)) }),
        },
        span,
    );
    let wasm = ir_module(vec![bit_concat, bit_test, allocate], span)
        .emit_wasm()
        .expect("emit residual IR wasm");
    let engine = Engine::default();
    let module = Module::new(&engine, &wasm.bytes).expect("compile wasm module");
    let mut store = Store::new(&engine, ());
    let instance = Instance::new(&mut store, &module, &[]).expect("instantiate module");
    let memory = instance.get_memory(&mut store, "memory").expect("memory export");
    let bits = instance
        .get_typed_func::<(), i32>(&mut store, "bits")
        .expect("get bits export");
    let pointer = bits.call(&mut store, ()).expect("call bits") as usize;
    let mut bytes = [0; 16];
    memory.read(&store, pointer, &mut bytes).expect("read bit array");
    assert_eq!(u32::from_le_bytes(bytes[4..8].try_into().unwrap()), 16);
    let is_bits = instance
        .get_typed_func::<(), i32>(&mut store, "is_bits")
        .expect("get is_bits export");
    assert_eq!(is_bits.call(&mut store, ()).expect("call is_bits"), 1);
    let allocate = instance
        .get_typed_func::<(), i32>(&mut store, "allocate")
        .expect("get allocate export");
    assert!(allocate.call(&mut store, ()).expect("call allocate") >= 4096);
}

#[test]
fn structured_codegen_ports_list_deconstruct_ir() {
    let span = Span::new(SourceFileId(0), 41, 50);
    let list = ir::Expression {
        type_: Type::List(Box::new(Type::Int)),
        span,
        kind: ExpressionKind::List(vec![int_expr("41", span)]),
    };
    let function = ir::Function {
        closure_captures: Vec::new(),
        name: "head".into(),
        public: true,
        params: Vec::new(),
        locals: vec![
            ir::Local { id: ir::LocalId(0), name: "head".into(), type_: Type::Int, span },
            ir::Local { id: ir::LocalId(1), name: "tail".into(), type_: Type::List(Box::new(Type::Int)), span },
        ],
        return_type: Type::Int,
        abi: ir::CallAbi {
            params: Vec::new(),
            return_: Some(ir::AbiValue::from(&Type::Int)),
            boundary: ir::CallBoundary::ModuleExport,
        },
        body: ir::Block {
            instructions: vec![ir::Instruction::Evaluate {
                expression: ir::Expression {
                    type_: Type::Nil,
                    span,
                    kind: ExpressionKind::ListDeconstruct {
                        list: Box::new(list),
                        head: ir::LocalId(0),
                        tail: ir::LocalId(1),
                    },
                },
                span,
            }],
            result: Box::new(ir::Expression { type_: Type::Int, span, kind: ExpressionKind::LocalGet(ir::LocalId(0)) }),
            span,
        },
        span,
    };
    let wasm = ir_module(vec![function], span)
        .emit_wasm()
        .expect("emit list deconstruct wasm");
    let engine = Engine::default();
    let module = Module::new(&engine, &wasm.bytes).expect("compile wasm module");
    let mut store = Store::new(&engine, ());
    let instance = Instance::new(&mut store, &module, &[]).expect("instantiate module");
    let head = instance
        .get_typed_func::<(), i64>(&mut store, "head")
        .expect("get head export");
    assert_eq!(head.call(&mut store, ()).expect("call head"), 41);
}

#[test]
fn structured_codegen_ports_failure_ir() {
    let wasm = compile_wasm("pub fn fails() -> Int { panic }");
    let engine = Engine::default();
    let module = Module::new(&engine, &wasm.bytes).expect("compile wasm module");
    let mut store = Store::new(&engine, ());
    let instance = Instance::new(&mut store, &module, &[]).expect("instantiate module");
    let fails = instance
        .get_typed_func::<(), i64>(&mut store, "fails")
        .expect("get fails export");
    assert!(fails.call(&mut store, ()).is_err());
}

#[test]
fn backend_validation_reports_source_spanned_target_adapter_diagnostics() {
    let module = host_import_module(Span::new(SourceFileId(0), 21, 30));
    let errors = module
        .emit_wat_with_options(EmitOptions::new(WasmTarget::Browser))
        .expect_err("unsupported target import");

    assert!(
        errors
            .iter()
            .any(|diagnostic| diagnostic.message.contains("target Browser expects `browser`"))
    );
    assert!(errors.iter().any(|diagnostic| {
        diagnostic
            .labels
            .iter()
            .any(|label| label.span == Span::new(SourceFileId(0), 21, 30))
    }));
}

#[test]
fn emits_host_import_before_exported_function() {
    let module = host_import_module(Span::new(SourceFileId(0), 0, 0));

    insta::assert_snapshot!(module.emit_wat().expect("emit wat"), @r#"
(module
  (type (func (param i64) (result i64)))
  (type (func (result i64)))
  (import "env" "inc" (func (type 0) (param i64) (result i64)))
  (func $main (type 1) (result i64)
    i64.const 41
    call 0
  )
  (export "main" (func 1))
)
"#);
}

#[test]
fn runs_host_import_in_wasmtime() {
    let wasm = host_import_module(Span::new(SourceFileId(0), 0, 0))
        .emit_wasm()
        .expect("emit wasm");
    let engine = Engine::default();
    let module = Module::new(&engine, &wasm.bytes).expect("compile wasm module");
    let mut store = Store::new(&engine, ());
    let mut linker = Linker::new(&engine);
    linker
        .func_wrap("env", "inc", |value: i64| value + 1)
        .expect("define import");
    let instance = linker.instantiate(&mut store, &module).expect("instantiate module");
    let main = instance
        .get_typed_func::<(), i64>(&mut store, "main")
        .expect("get main export");

    assert_eq!(main.call(&mut store, ()).expect("call main"), 42);
}

#[test]
fn lowers_external_functions_to_target_host_imports() {
    let wasm = compile_wasm(
        r#"external fn inc(value: Int) -> Int = "env" "host_inc"
pub fn main() -> Int { inc(41) }"#,
    );

    assert!(
        wasm.wat
            .contains("(import \"env\" \"host_inc\" (func (type 0) (param i64) (result i64)))")
    );
    assert!(wasm.wat.contains("call 0"));
}

#[test]
fn target_groups_allow_external_imports_to_reuse_local_names() {
    let source = r#"if javascript {
external fn load(key: String) -> String = "browser" "localStorage.getItem"
}

if erlang {
external fn load(key: String) -> String = "env" "load"
}

pub fn main() -> String { load("weather") }"#;

    let browser = compile_wasm_target(source, CompileTarget::Browser).expect("compile browser target");
    let wasmtime = compile_wasm_target(source, CompileTarget::Wasmtime).expect("compile wasmtime target");

    assert!(browser.wat.contains("(import \"browser\" \"localStorage.getItem\""));
    assert!(!browser.wat.contains("(import \"env\" \"load\""));
    assert!(wasmtime.wat.contains("(import \"env\" \"load\""));
    assert!(!wasmtime.wat.contains("(import \"browser\" \"localStorage.getItem\""));
}

#[test]
fn preserves_external_import_module_and_function_names() {
    let wasm = compile_wasm_target(
        r#"external fn load(key: String) -> String = "browser" "localStorage.getItem"
pub fn main() -> String { load("weather") }"#,
        CompileTarget::Browser,
    )
    .expect("compile browser external import");

    assert!(
        wasm.wat
            .contains("(import \"browser\" \"localStorage.getItem\" (func (type 0) (param i32) (result i32)))")
    );
}

#[test]
fn bundler_target_accepts_shared_js_imports_and_exports_string_helpers() {
    let wasm = compile_wasm_target(
        r#"external fn request_text(input: String) -> String = "regulus/js" "request_text"
pub fn main(input: String) -> String { request_text(input) }"#,
        CompileTarget::Bundler,
    )
    .expect("compile bundler external import");

    assert!(
        wasm.wat
            .contains("(import \"regulus/js\" \"request_text\" (func (type 0) (param i32) (result i32)))"),
        "{}",
        wasm.wat
    );
    for name in [
        "__regulus_alloc",
        "__regulus_string_new",
        "__regulus_string_len",
        "__regulus_string_data",
        "__regulus_value_tag",
        "__regulus_value_arity",
        "__regulus_value_constructor",
        "__regulus_value_field",
    ] {
        assert!(
            wasm.wat.contains(&format!("(export \"{name}\"")),
            "missing {name} in {}",
            wasm.wat
        );
    }
}

#[test]
fn rejects_unsupported_external_parameter_abi_shapes_before_byte_emission() {
    let diagnostics = compile_wasm_target(
        r#"external fn bad(value: Nil) -> Int = "env" "bad"
pub fn main() -> Int { 1 }"#,
        CompileTarget::Wasmtime,
    )
    .expect_err("Nil parameter should be rejected");

    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("external function `bad` parameter 1 uses unsupported ABI shape `Nil`")
    }));
    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic
            .labels
            .iter()
            .any(|label| label.message.as_deref() == Some("unsupported external ABI shape here"))
    }));
}

#[test]
fn rejects_unsupported_external_return_abi_shapes_before_byte_emission() {
    let diagnostics = compile_wasm_target(
        r#"external fn bad(value: Int) -> value = "env" "bad"
pub fn main() -> Int { 1 }"#,
        CompileTarget::Wasmtime,
    )
    .expect_err("generic return should be rejected");

    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("external function `bad` return uses unsupported ABI shape `Generic(\"value\")`")
    }));
}

#[test]
fn validates_external_import_modules_against_target() {
    let diagnostics = compile_wasm_target(
        r#"external fn load(key: String) -> String = "browser" "localStorage.getItem"
pub fn main() -> String { load("weather") }"#,
        CompileTarget::Wasmtime,
    )
    .expect_err("browser import should not compile for wasmtime");

    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("imports host module `browser`, but target Wasmtime expects `env`")
    }));
}

#[test]
fn table_tests_selected_external_target_groups() {
    let source = r#"if javascript {
external fn selected(key: String) -> String = "browser" "selected"
}

if erlang {
external fn selected(key: String) -> String = "env" "selected"
}

pub fn main() -> String { selected("key") }"#;
    let cases = [
        (
            CompileTarget::Browser,
            "(import \"browser\" \"selected\"",
            "(import \"env\" \"selected\"",
        ),
        (
            CompileTarget::Wasmtime,
            "(import \"env\" \"selected\"",
            "(import \"browser\" \"selected\"",
        ),
    ];

    for (target, expected, absent) in cases {
        let wasm = compile_wasm_target(source, target).expect("compile selected target group");
        assert!(wasm.wat.contains(expected), "missing {expected} for {target:?}");
        assert!(!wasm.wat.contains(absent), "unexpected {absent} for {target:?}");
    }
}

#[test]
fn table_tests_unsupported_external_abi_shapes() {
    let cases = [
        (
            "Nil parameter",
            r#"external fn bad(value: Nil) -> Int = "env" "bad"
pub fn main() -> Int { 1 }"#,
            "external function `bad` parameter 1 uses unsupported ABI shape `Nil`",
        ),
        (
            "generic parameter",
            r#"external fn bad(value: value) -> Int = "env" "bad"
pub fn main() -> Int { 1 }"#,
            "external function `bad` parameter 1 uses unsupported ABI shape `Generic(\"value\")`",
        ),
        (
            "generic list return",
            r#"external fn bad() -> List(value) = "env" "bad"
pub fn main() -> Int { 1 }"#,
            "external function `bad` return uses unsupported ABI shape `List(Generic(\"value\"))`",
        ),
    ];

    for (name, source, expected) in cases {
        let diagnostics = match compile_wasm_target(source, CompileTarget::Wasmtime) {
            Ok(_) => panic!("{name} should fail"),
            Err(diagnostics) => diagnostics,
        };
        assert!(
            diagnostics
                .iter()
                .any(|diagnostic| diagnostic.message.contains(expected)),
            "{name} did not report {expected}; got {diagnostics:?}"
        );
    }
}

#[test]
fn table_tests_supported_managed_external_abi_shapes() {
    let cases = [
        (
            "string",
            r#"external fn host(value: String) -> String = "env" "string"
pub fn main() -> Int { 1 }"#,
            "(import \"env\" \"string\" (func (type 0) (param i32) (result i32)))",
        ),
        (
            "list",
            r#"external fn host(value: List(String)) -> List(String) = "env" "list"
pub fn main() -> Int { 1 }"#,
            "(import \"env\" \"list\" (func (type 0) (param i32) (result i32)))",
        ),
        (
            "tuple",
            r#"external fn host(value: #(Int, String)) -> #(Int, String) = "env" "tuple"
pub fn main() -> Int { 1 }"#,
            "(import \"env\" \"tuple\" (func (type 0) (param i32) (result i32)))",
        ),
        (
            "custom record",
            r#"pub type Response { Response(status: Int, body: String) }
external fn host(value: Response) -> Nil = "env" "record"
pub fn main() -> Int { 1 }"#,
            "(import \"env\" \"record\" (func (type 0) (param i32)))",
        ),
        (
            "opaque handle",
            r#"pub external type Handle
external fn host(value: Handle) -> Handle = "env" "handle"
pub fn main() -> Int { 1 }"#,
            "(import \"env\" \"handle\" (func (type 0) (param i32) (result i32)))",
        ),
    ];

    for (name, source, expected_import) in cases {
        let wasm = compile_wasm_target(source, CompileTarget::Wasmtime)
            .unwrap_or_else(|errors| panic!("{name} should compile: {errors:?}"));
        assert!(
            wasm.wat.contains(expected_import),
            "{name} missing import {expected_import}; wat:\n{}",
            wasm.wat
        );
    }
}

#[test]
fn table_tests_browser_and_worker_style_external_imports() {
    let cases = [
        (
            "fetch",
            r#"external fn host(url: String) -> String = "browser" "fetch"
pub fn main() -> Int { 1 }"#,
            "(import \"browser\" \"fetch\" (func (type 0) (param i32) (result i32)))",
        ),
        (
            "local storage",
            r#"external fn host(key: String) -> String = "browser" "localStorage.getItem"
pub fn main() -> Int { 1 }"#,
            "(import \"browser\" \"localStorage.getItem\" (func (type 0) (param i32) (result i32)))",
        ),
    ];

    for (name, source, expected_import) in cases {
        let wasm = compile_wasm_target(source, CompileTarget::Browser)
            .unwrap_or_else(|errors| panic!("{name} should compile: {errors:?}"));
        assert!(
            wasm.wat.contains(expected_import),
            "{name} missing import {expected_import}; wat:\n{}",
            wasm.wat
        );
    }
}

#[test]
fn rejects_unsupported_js_host_import_shapes_before_byte_emission() {
    let diagnostics = compile_wasm_target(
        r#"pub type Response { Response(status: Int, body: String) }
external fn host(response: Response) -> Nil = "browser" "worker.respond"
pub fn main() -> Int { 1 }"#,
        CompileTarget::Browser,
    )
    .expect_err("record JS import should be rejected until managed readers are stable");

    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("JS host import `host` parameter 1 uses unsupported ABI shape")
    }));
    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic
            .labels
            .iter()
            .any(|label| label.message.as_deref() == Some("unsupported JS host ABI shape here"))
    }));
}

#[test]
fn rejects_unsupported_js_host_export_shapes_before_byte_emission() {
    let diagnostics = compile_wasm_target(
        r#"pub type Response { Response(status: Int, body: String) }
pub fn main() -> Response { Response(200, "ok") }"#,
        CompileTarget::Bundler,
    )
    .expect_err("record JS export should be rejected until managed readers are stable");

    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("JS host export `main` return uses unsupported ABI shape")
    }));
}

#[test]
fn accepts_first_stable_js_host_import_and_export_shapes() {
    let source = r#"external fn host(i: Int, f: Float, b: Bool, s: String) -> String = "regulus/js" "host"
pub fn main(i: Int, f: Float, b: Bool, s: String) -> String { host(i, f, b, s) }"#;

    compile_wasm_target(source, CompileTarget::Bundler).expect("stable JS ABI shapes should compile");
}

#[test]
fn emits_string_export_adapters_for_host_boundaries() {
    let wasm = compile_wasm("pub fn greeting() { \"hello\" }");
    assert!(wasm.wat.contains("(func $greeting__data"));
    assert!(wasm.wat.contains("(export \"greeting__data\""));
    assert!(wasm.wat.contains("(func $greeting__len"));
    assert!(wasm.wat.contains("(export \"greeting__len\""));
}

#[test]
fn keeps_generated_wat_and_wasm_deterministic() {
    let source = "pub fn add(x: Int) -> Int { x + 1 }";
    let first = compile_wasm(source);
    let second = compile_wasm(source);
    assert_eq!(first.wat, second.wat);
    assert_eq!(first.bytes, second.bytes);
}

#[test]
fn rejects_host_imports_for_the_wrong_target_before_assembly() {
    let span = Span::new(SourceFileId(0), 0, 0);
    let module = host_import_module(span);
    let diagnostics = module
        .emit_wat_with_options(EmitOptions::new(WasmTarget::Browser))
        .expect_err("unsupported target import");

    assert!(
        diagnostics
            .iter()
            .any(|diagnostic| { diagnostic.message.contains("target Browser expects `browser`") })
    );
}

#[test]
fn rejects_unsupported_export_abi_before_wat_assembly() {
    let span = Span::new(SourceFileId(0), 0, 0);
    let generic = Type::Generic("value".into());
    let function = ir::Function {
        closure_captures: Vec::new(),
        name: "id".into(),
        public: true,
        params: vec![ir::Local { id: ir::LocalId(0), name: "x".into(), type_: generic.clone(), span }],
        locals: vec![ir::Local { id: ir::LocalId(0), name: "x".into(), type_: generic.clone(), span }],
        return_type: generic.clone(),
        abi: ir::CallAbi {
            params: vec![ir::AbiValue::from(&generic)],
            return_: Some(ir::AbiValue::from(&generic)),
            boundary: ir::CallBoundary::ModuleExport,
        },
        body: ir::Block {
            instructions: Vec::new(),
            result: Box::new(ir::Expression { type_: generic, span, kind: ExpressionKind::LocalGet(ir::LocalId(0)) }),
            span,
        },
        span,
    };
    let diagnostics = ir_module(vec![function], span).emit_wat().expect_err("unsupported ABI");

    assert!(
        diagnostics
            .iter()
            .any(|diagnostic| diagnostic.message.contains("unsupported host ABI"))
    );
}

#[test]
fn rejects_residual_use_ir_before_wat_assembly() {
    let span = Span::new(SourceFileId(0), 0, 0);
    let function = ir::Function {
        closure_captures: Vec::new(),
        name: "main".into(),
        public: true,
        params: Vec::new(),
        locals: Vec::new(),
        return_type: Type::Int,
        abi: ir::CallAbi {
            params: Vec::new(),
            return_: Some(ir::AbiValue::from(&Type::Int)),
            boundary: ir::CallBoundary::ModuleExport,
        },
        body: ir::Block {
            instructions: Vec::new(),
            result: Box::new(ir::Expression {
                type_: Type::Int,
                span,
                kind: ExpressionKind::Use(ir::UseLowering {
                    callback: Box::new(int_expr("1", span)),
                    call: Box::new(int_expr("2", span)),
                }),
            }),
            span,
        },
        span,
    };
    assert_emit_wasm_error(
        &ir_module(vec![function], span),
        "raw `use` IR reached the Wasm backend",
        span,
    );
}

fn generic_expr(span: Span) -> ir::Expression {
    ir::Expression {
        type_: Type::Generic("a".into()),
        span,
        kind: ExpressionKind::Literal(ir::Literal { kind: LiteralKind::Int, source: "1".into() }),
    }
}

#[test]
fn residual_generic_debug_reports_source_spanned_diagnostic() {
    let span = Span::new(SourceFileId(0), 3, 8);
    let debug = ir::Expression {
        type_: Type::Generic("a".into()),
        span,
        kind: ExpressionKind::DirectCall(ir::DirectCall {
            function: "__stdlib_gleam_io_debug".into(),
            arguments: vec![ir::CallArgument { label: None, value: generic_expr(span), span }],
            abi: ir::CallAbi {
                params: vec![ir::AbiValue::from(&Type::Generic("a".into()))],
                return_: Some(ir::AbiValue::from(&Type::Generic("a".into()))),
                boundary: ir::CallBoundary::Internal,
            },
        }),
    };
    let function = exported_function_with_body(
        "main",
        &Type::Int,
        ir::Expression {
            type_: Type::Int,
            span,
            kind: ExpressionKind::Literal(ir::Literal { kind: LiteralKind::Int, source: "0".into() }),
        },
        span,
    );
    let mut function = function;
    function
        .body
        .instructions
        .push(ir::Instruction::Evaluate { expression: debug, span });

    assert_emit_wasm_error(
        &ir_module(vec![function], span),
        "debug intrinsic does not support generic values",
        span,
    );
}

#[test]
fn residual_generic_comparison_reports_source_spanned_diagnostic() {
    let span = Span::new(SourceFileId(0), 10, 15);
    let result = ir::Expression {
        type_: Type::Bool,
        span,
        kind: ExpressionKind::Compare {
            op: ir::ComparisonOp::Less,
            left: Box::new(generic_expr(span)),
            right: Box::new(generic_expr(span)),
        },
    };
    let function = exported_function_with_body("main", &Type::Bool, result, span);

    assert_emit_wasm_error(
        &ir_module(vec![function], span),
        "comparison type is not supported",
        span,
    );
}

#[test]
fn residual_generic_equality_reports_source_spanned_diagnostic() {
    let span = Span::new(SourceFileId(0), 20, 25);
    let result = ir::Expression {
        type_: Type::Bool,
        span,
        kind: ExpressionKind::RuntimeEquality {
            left: Box::new(generic_expr(span)),
            right: Box::new(generic_expr(span)),
        },
    };
    let function = exported_function_with_body("main", &Type::Bool, result, span);

    assert_emit_wasm_error(
        &ir_module(vec![function], span),
        "runtime equality does not support generic values",
        span,
    );
}

#[test]
fn runs_record_updates_with_scalar_and_managed_fields() {
    let wasm = compile_wasm(
        r#"
pub type User { User(name: String, age: Int) }

pub fn updated_age(age: Int) -> Int {
let user = User(age: age, name: "Ada")
let older = User(..user, age: age + 1)
case older {
 User(name: _, age: value) -> value
}
}

pub fn preserves_age_when_updating_name(age: Int) -> Int {
let user = User(name: "Ada", age: age)
let renamed = User(..user, name: "Grace")
case renamed {
 User(name: _, age: value) -> value
}
}
"#,
    );
    let engine = Engine::default();
    let module = Module::new(&engine, &wasm.bytes).expect("compile wasm module");
    let mut store = Store::new(&engine, ());
    let instance = Instance::new(&mut store, &module, &[]).expect("instantiate module");
    let updated_age = instance
        .get_typed_func::<i64, i64>(&mut store, "updated_age")
        .expect("get updated_age export");
    let preserves_age = instance
        .get_typed_func::<i64, i64>(&mut store, "preserves_age_when_updating_name")
        .expect("get preserves_age_when_updating_name export");

    assert_eq!(updated_age.call(&mut store, 41).expect("call updated_age"), 42);
    assert_eq!(preserves_age.call(&mut store, 36).expect("call preserves_age"), 36);
}

#[test]
fn runs_use_callback_that_updates_record() {
    let wasm = compile_wasm(
        r#"
pub type User { User(name: String, age: Int) }

fn with_user(user: User, callback: fn(User) -> Int) -> Int {
callback(user)
}

pub fn use_updated_age(age: Int) -> Int {
let user = User(name: "Ada", age: age)
use current <- with_user(user)
let older = User(..current, age: age + 2)
case older {
 User(name: _, age: value) -> value
}
}
"#,
    );
    let engine = Engine::default();
    let module = Module::new(&engine, &wasm.bytes).expect("compile wasm module");
    let mut store = Store::new(&engine, ());
    let instance = Instance::new(&mut store, &module, &[]).expect("instantiate module");
    let use_updated_age = instance
        .get_typed_func::<i64, i64>(&mut store, "use_updated_age")
        .expect("get use_updated_age export");

    assert_eq!(use_updated_age.call(&mut store, 40).expect("call use_updated_age"), 42);
}

#[test]
fn runs_exported_function_in_wasmtime() {
    let wasm = compile_wasm(include_str!("../../../../fixtures/e2e/public_id.gleam"));
    let engine = Engine::default();
    let module = Module::new(&engine, &wasm.bytes).expect("compile wasm module");
    let mut store = Store::new(&engine, ());
    let instance = Instance::new(&mut store, &module, &[]).expect("instantiate module");
    let id = instance
        .get_typed_func::<i64, i64>(&mut store, "id")
        .expect("get id export");

    assert_eq!(id.call(&mut store, 42).expect("call id"), 42);
}

#[test]
fn runs_scalar_case_expression() {
    let wasm = compile_wasm("pub fn choose(x: Int) -> Int { case x { 0 -> 1 _ -> 2 } }");
    let engine = Engine::default();
    let module = Module::new(&engine, &wasm.bytes).expect("compile wasm module");
    let mut store = Store::new(&engine, ());
    let instance = Instance::new(&mut store, &module, &[]).expect("instantiate module");
    let choose = instance
        .get_typed_func::<i64, i64>(&mut store, "choose")
        .expect("get choose export");

    assert_eq!(choose.call(&mut store, 0).expect("call choose"), 1);
    assert_eq!(choose.call(&mut store, 42).expect("call choose"), 2);
}

#[test]
fn runs_bool_case_expression() {
    let wasm = compile_wasm("pub fn bit(x: Bool) -> Int { case x { True -> 1 False -> 0 } }");
    let engine = Engine::default();
    let module = Module::new(&engine, &wasm.bytes).expect("compile wasm module");
    let mut store = Store::new(&engine, ());
    let instance = Instance::new(&mut store, &module, &[]).expect("instantiate module");
    let bit = instance
        .get_typed_func::<i32, i64>(&mut store, "bit")
        .expect("get bit export");

    assert_eq!(bit.call(&mut store, 1).expect("call bit"), 1);
    assert_eq!(bit.call(&mut store, 0).expect("call bit"), 0);
}

#[test]
fn binds_case_pattern_values() {
    let wasm = compile_wasm("pub fn keep(x: Int) -> Int { case x { y -> y } }");
    let engine = Engine::default();
    let module = Module::new(&engine, &wasm.bytes).expect("compile wasm module");
    let mut store = Store::new(&engine, ());
    let instance = Instance::new(&mut store, &module, &[]).expect("instantiate module");
    let keep = instance
        .get_typed_func::<i64, i64>(&mut store, "keep")
        .expect("get keep export");

    assert_eq!(keep.call(&mut store, 42).expect("call keep"), 42);
}

#[test]
fn runs_guarded_case_expression() {
    let wasm =
        compile_wasm("pub fn choose(flag: Bool, other: Bool) -> Int { case flag { True if other -> 1 _ -> 0 } }");
    let engine = Engine::default();
    let module = Module::new(&engine, &wasm.bytes).expect("compile wasm module");
    let mut store = Store::new(&engine, ());
    let instance = Instance::new(&mut store, &module, &[]).expect("instantiate module");
    let choose = instance
        .get_typed_func::<(i32, i32), i64>(&mut store, "choose")
        .expect("get choose export");

    assert_eq!(choose.call(&mut store, (1, 1)).expect("call choose"), 1);
    assert_eq!(choose.call(&mut store, (1, 0)).expect("call choose"), 0);
    assert_eq!(choose.call(&mut store, (0, 1)).expect("call choose"), 0);
}

#[test]
fn traps_failed_let_assert() {
    let wasm = compile_wasm("pub fn require_true(flag: Bool) -> Int { let assert True = flag 1 }");
    let engine = Engine::default();
    let module = Module::new(&engine, &wasm.bytes).expect("compile wasm module");
    let mut store = Store::new(&engine, ());
    let instance = Instance::new(&mut store, &module, &[]).expect("instantiate module");
    let require_true = instance
        .get_typed_func::<i32, i64>(&mut store, "require_true")
        .expect("get require_true export");

    assert_eq!(require_true.call(&mut store, 1).expect("call require_true"), 1);
    assert!(require_true.call(&mut store, 0).is_err());
}

#[test]
fn binds_alias_patterns() {
    let wasm = compile_wasm("pub fn keep(x: Int) -> Int { case x { 1 as one -> one other -> other } }");
    let engine = Engine::default();
    let module = Module::new(&engine, &wasm.bytes).expect("compile wasm module");
    let mut store = Store::new(&engine, ());
    let instance = Instance::new(&mut store, &module, &[]).expect("instantiate module");
    let keep = instance
        .get_typed_func::<i64, i64>(&mut store, "keep")
        .expect("get keep export");

    assert_eq!(keep.call(&mut store, 1).expect("call keep"), 1);
    assert_eq!(keep.call(&mut store, 42).expect("call keep"), 42);
}

#[test]
fn runs_indirect_call_through_function_value() {
    let wasm = compile_wasm(
        r#"pub fn id(x: Int) -> Int { x }
fn apply(x: Int, f: fn(Int) -> Int) -> Int { f(x) }
pub fn main() { apply(41, id) }
"#,
    );
    let engine = Engine::default();
    let module = Module::new(&engine, &wasm.bytes).expect("compile wasm module");
    let mut store = Store::new(&engine, ());
    let instance = Instance::new(&mut store, &module, &[]).expect("instantiate module");
    let main = instance
        .get_typed_func::<(), i64>(&mut store, "main")
        .expect("get main export");

    assert_eq!(main.call(&mut store, ()).expect("call main"), 41);
}

#[test]
fn runs_closure_with_scalar_and_managed_captures() {
    let wasm = compile_wasm(
        r#"fn call(f: fn(Int) -> Int, x: Int) -> Int { f(x) }
pub fn scalar(x: Int) -> Int {
let add = fn(y) { x + y }
call(add, 2)
}
pub fn managed() -> Bool {
let prefix = "ok"
let same = fn(value) { value == prefix }
same("ok")
}
"#,
    );
    let engine = Engine::default();
    let module = Module::new(&engine, &wasm.bytes).expect("compile wasm module");
    let mut store = Store::new(&engine, ());
    let instance = Instance::new(&mut store, &module, &[]).expect("instantiate module");
    let scalar = instance
        .get_typed_func::<i64, i64>(&mut store, "scalar")
        .expect("get scalar export");
    let managed = instance
        .get_typed_func::<(), i32>(&mut store, "managed")
        .expect("get managed export");

    assert_eq!(scalar.call(&mut store, 40).expect("call scalar"), 42);
    assert_eq!(managed.call(&mut store, ()).expect("call managed"), 1);
}

#[test]
fn runs_use_lowered_callback_with_captures() {
    let wasm = compile_wasm(
        r#"fn with_value(x: Int, f: fn(Int) -> Int) -> Int { f(x) }
pub fn main() -> Int {
let offset = 1
use value <- with_value(41)
value + offset
}
"#,
    );
    let engine = Engine::default();
    let module = Module::new(&engine, &wasm.bytes).expect("compile wasm module");
    let mut store = Store::new(&engine, ());
    let instance = Instance::new(&mut store, &module, &[]).expect("instantiate module");
    let main = instance
        .get_typed_func::<(), i64>(&mut store, "main")
        .expect("get main export");

    assert_eq!(main.call(&mut store, ()).expect("call main"), 42);
}

#[test]
fn runs_use_callback_pattern_failure_path() {
    let wasm = compile_wasm(
        r#"fn with_values(f: fn(Int, Int) -> Int) -> Int { f(1, 2) }
pub fn main() -> Int {
use 1, value <- with_values()
value + 40
}
"#,
    );
    let engine = Engine::default();
    let module = Module::new(&engine, &wasm.bytes).expect("compile wasm module");
    let mut store = Store::new(&engine, ());
    let instance = Instance::new(&mut store, &module, &[]).expect("instantiate module");
    let main = instance
        .get_typed_func::<(), i64>(&mut store, "main")
        .expect("get main export");

    assert_eq!(main.call(&mut store, ()).expect("call main"), 42);
}

#[test]
fn runs_labelled_calls_in_parameter_order() {
    let wasm = compile_wasm(
        r#"fn subtract(left x: Int, right y: Int) -> Int { x - y }
pub fn main() -> Int { subtract(right: 2, left: 44) }
"#,
    );
    let engine = Engine::default();
    let module = Module::new(&engine, &wasm.bytes).expect("compile wasm module");
    let mut store = Store::new(&engine, ());
    let instance = Instance::new(&mut store, &module, &[]).expect("instantiate module");
    let main = instance
        .get_typed_func::<(), i64>(&mut store, "main")
        .expect("get main export");

    assert_eq!(main.call(&mut store, ()).expect("call main"), 42);
}

#[test]
fn runs_use_with_labelled_callback_insertion() {
    let wasm = compile_wasm(
        r#"fn labelled(callback f: fn(Int) -> Int, value x: Int) -> Int { f(x) }
pub fn main() -> Int {
use value <- labelled(value: 41)
value + 1
}
"#,
    );
    let engine = Engine::default();
    let module = Module::new(&engine, &wasm.bytes).expect("compile wasm module");
    let mut store = Store::new(&engine, ());
    let instance = Instance::new(&mut store, &module, &[]).expect("instantiate module");
    let main = instance
        .get_typed_func::<(), i64>(&mut store, "main")
        .expect("get main export");

    assert_eq!(main.call(&mut store, ()).expect("call main"), 42);
}

#[test]
fn runs_nested_use_callbacks() {
    let wasm = compile_wasm(
        r#"fn with_value(x: Int, f: fn(Int) -> Int) -> Int { f(x) }
pub fn main() -> Int {
use x <- with_value(40)
use y <- with_value(2)
x + y
}
"#,
    );
    let engine = Engine::default();
    let module = Module::new(&engine, &wasm.bytes).expect("compile wasm module");
    let mut store = Store::new(&engine, ());
    let instance = Instance::new(&mut store, &module, &[]).expect("instantiate module");
    let main = instance
        .get_typed_func::<(), i64>(&mut store, "main")
        .expect("get main export");

    assert_eq!(main.call(&mut store, ()).expect("call main"), 42);
}

#[test]
fn runs_partial_application_closure() {
    let wasm = compile_wasm(
        r#"fn call(f: fn(Int) -> Int, x: Int) -> Int { f(x) }
fn add(a: Int, b: Int) -> Int { a + b }
pub fn partial(x: Int) -> Int {
let addx = add(x, _)
call(addx, 2)
}
"#,
    );
    let engine = Engine::default();
    let module = Module::new(&engine, &wasm.bytes).expect("compile wasm module");
    let mut store = Store::new(&engine, ());
    let instance = Instance::new(&mut store, &module, &[]).expect("instantiate module");
    let partial = instance
        .get_typed_func::<i64, i64>(&mut store, "partial")
        .expect("get partial export");

    assert_eq!(partial.call(&mut store, 40).expect("call partial"), 42);
}

#[test]
fn runs_nested_closures_capturing_multiple_scopes() {
    let wasm = compile_wasm(
        r#"pub fn nested(x: Int) -> Int {
let outer = fn(y) {
 let inner = fn(z) { x + y + z }
 inner(3)
}
outer(2)
}
"#,
    );
    let engine = Engine::default();
    let module = Module::new(&engine, &wasm.bytes).expect("compile wasm module");
    let mut store = Store::new(&engine, ());
    let instance = Instance::new(&mut store, &module, &[]).expect("instantiate module");
    let nested = instance
        .get_typed_func::<i64, i64>(&mut store, "nested")
        .expect("get nested export");

    assert_eq!(nested.call(&mut store, 37).expect("call nested"), 42);
}

#[test]
fn emits_managed_pattern_control_flow() {
    let wasm = compile_wasm("pub fn first(pair: #(Int, Int)) -> Int { case pair { #(left, _) -> left } }");

    assert!(wasm.wat.contains("i32.load"));
    assert!(wasm.wat.contains("i64.load"));
}

#[test]
fn returns_tuple_list_and_custom_pointers_with_inspectable_memory_layouts() {
    let wasm = compile_wasm(
        r#"type Box { Box(Int) }
pub fn pair() { #(1, 2) }
pub fn items() { [1, 2] }
pub fn boxed() { Box(42) }
"#,
    );
    let engine = Engine::default();
    let module = Module::new(&engine, &wasm.bytes).expect("compile wasm module");
    let mut store = Store::new(&engine, ());
    let instance = Instance::new(&mut store, &module, &[]).expect("instantiate module");
    let memory = instance.get_memory(&mut store, "memory").expect("memory export");

    let pair = instance
        .get_typed_func::<(), i32>(&mut store, "pair")
        .expect("get pair export");
    let pair_pointer = pair.call(&mut store, ()).expect("call pair") as usize;
    let mut pair_bytes = [0; 24];
    memory
        .read(&store, pair_pointer, &mut pair_bytes)
        .expect("read tuple object");
    assert_eq!(
        ObjectTag::try_from(u32::from_le_bytes(pair_bytes[0..4].try_into().unwrap())),
        Ok(ObjectTag::Tuple)
    );
    assert_eq!(u32::from_le_bytes(pair_bytes[4..8].try_into().unwrap()), 2);

    let items = instance
        .get_typed_func::<(), i32>(&mut store, "items")
        .expect("get items export");
    let items_pointer = items.call(&mut store, ()).expect("call items") as usize;
    let mut list_bytes = [0; 24];
    memory
        .read(&store, items_pointer, &mut list_bytes)
        .expect("read list object");
    assert_eq!(
        ObjectTag::try_from(u32::from_le_bytes(list_bytes[0..4].try_into().unwrap())),
        Ok(ObjectTag::ListCons)
    );
    assert_eq!(u64::from_le_bytes(list_bytes[8..16].try_into().unwrap()), 1);

    let boxed = instance
        .get_typed_func::<(), i32>(&mut store, "boxed")
        .expect("get boxed export");
    let boxed_pointer = boxed.call(&mut store, ()).expect("call boxed") as usize;
    let mut boxed_bytes = [0; 24];
    memory
        .read(&store, boxed_pointer, &mut boxed_bytes)
        .expect("read custom object");
    assert_eq!(
        ObjectTag::try_from(u32::from_le_bytes(boxed_bytes[0..4].try_into().unwrap())),
        Ok(ObjectTag::Custom)
    );
    assert_eq!(u64::from_le_bytes(boxed_bytes[12..20].try_into().unwrap()), 42);
}

#[test]
fn returns_bit_array_pointer_with_inspectable_memory_layout() {
    let wasm = compile_wasm("pub fn bits() { <<1, 2, 3>> }");
    let engine = Engine::default();
    let module = Module::new(&engine, &wasm.bytes).expect("compile wasm module");
    let mut store = Store::new(&engine, ());
    let instance = Instance::new(&mut store, &module, &[]).expect("instantiate module");
    let bits = instance
        .get_typed_func::<(), i32>(&mut store, "bits")
        .expect("get bits export");
    let pointer = bits.call(&mut store, ()).expect("call bits") as usize;
    let memory = instance.get_memory(&mut store, "memory").expect("memory export");
    let mut bytes = [0; 16];
    memory.read(&store, pointer, &mut bytes).expect("read bit array object");

    assert_eq!(
        ObjectTag::try_from(u32::from_le_bytes(bytes[0..4].try_into().unwrap())),
        Ok(ObjectTag::BitArray)
    );
    assert_eq!(u32::from_le_bytes(bytes[4..8].try_into().unwrap()), 24);
    assert_eq!(&bytes[8..11], &[1, 2, 3]);
}

#[test]
fn runs_bit_string_pattern_matching() {
    let wasm = compile_wasm(
        r#"pub fn matches() { case <<1, 2>> { <<1, 2>> -> True _ -> False } }
pub fn fails() { case <<1, 3>> { <<1, 2>> -> True _ -> False } }
pub fn binds() { case <<42>> { <<x>> -> x } }
pub fn rest() { case <<1, 2, 3>> { <<1, rest:bits>> -> rest } }
"#,
    );
    assert!(!wasm.wat.contains("$__bit_array_get_bit"), "{}", wasm.wat);
    assert!(!wasm.wat.contains("$__bit_array_slice"), "{}", wasm.wat);
    let engine = Engine::default();
    let module = Module::new(&engine, &wasm.bytes).expect("compile wasm module");
    let mut store = Store::new(&engine, ());
    let instance = Instance::new(&mut store, &module, &[]).expect("instantiate module");
    let matches = instance
        .get_typed_func::<(), i32>(&mut store, "matches")
        .expect("get matches export");
    assert_eq!(matches.call(&mut store, ()).expect("call matches"), 1);
    let fails = instance
        .get_typed_func::<(), i32>(&mut store, "fails")
        .expect("get fails export");
    assert_eq!(fails.call(&mut store, ()).expect("call fails"), 0);
    let binds = instance
        .get_typed_func::<(), i64>(&mut store, "binds")
        .expect("get binds export");
    assert_eq!(binds.call(&mut store, ()).expect("call binds"), 42);
    let rest = instance
        .get_typed_func::<(), i32>(&mut store, "rest")
        .expect("get rest export");
    let pointer = rest.call(&mut store, ()).expect("call rest") as usize;
    let memory = instance.get_memory(&mut store, "memory").expect("memory export");
    let mut bytes = [0; 24];
    memory.read(&store, pointer, &mut bytes).expect("read rest bit array");
    assert_eq!(u32::from_le_bytes(bytes[4..8].try_into().unwrap()), 16);
    assert_eq!(&bytes[8..10], &[2, 3]);
}

#[test]
fn returns_string_pointer_with_inspectable_memory_layout() {
    let wasm = compile_wasm("pub fn greeting() { \"hello\" }");
    let engine = Engine::default();
    let module = Module::new(&engine, &wasm.bytes).expect("compile wasm module");
    let mut store = Store::new(&engine, ());
    let instance = Instance::new(&mut store, &module, &[]).expect("instantiate module");
    let greeting = instance
        .get_typed_func::<(), i32>(&mut store, "greeting")
        .expect("get greeting export");
    let pointer = greeting.call(&mut store, ()).expect("call greeting") as usize;
    let memory = instance.get_memory(&mut store, "memory").expect("memory export");
    let mut bytes = [0; 16];
    memory.read(&store, pointer, &mut bytes).expect("read string object");

    assert_eq!(
        ObjectTag::try_from(u32::from_le_bytes(bytes[0..4].try_into().unwrap())),
        Ok(ObjectTag::String)
    );
    assert_eq!(u32::from_le_bytes(bytes[4..8].try_into().unwrap()), 5);
    assert_eq!(&bytes[8..13], b"hello");
}

#[test]
fn runs_arithmetic_comparison_and_short_circuit_operators() {
    let wasm = compile_wasm(
        r#"pub fn arithmetic(x: Int) -> Int { x + 2 * 3 - 4 }
pub fn compare(x: Int) -> Bool { x >= 4 }
pub fn choose(left: Bool, right: Bool) -> Bool { left || right }
"#,
    );
    let engine = Engine::default();
    let module = Module::new(&engine, &wasm.bytes).expect("compile wasm module");
    let mut store = Store::new(&engine, ());
    let instance = Instance::new(&mut store, &module, &[]).expect("instantiate module");
    let arithmetic = instance
        .get_typed_func::<i64, i64>(&mut store, "arithmetic")
        .expect("get arithmetic export");
    assert_eq!(arithmetic.call(&mut store, 10).expect("call arithmetic"), 12);
    let compare = instance
        .get_typed_func::<i64, i32>(&mut store, "compare")
        .expect("get compare export");
    assert_eq!(compare.call(&mut store, 4).expect("call compare"), 1);
    assert_eq!(compare.call(&mut store, 3).expect("call compare"), 0);
    let choose = instance
        .get_typed_func::<(i32, i32), i32>(&mut store, "choose")
        .expect("get choose export");
    assert_eq!(choose.call(&mut store, (0, 1)).expect("call choose"), 1);
}

#[test]
fn imports_initial_stdlib_io_host_calls() {
    let wasm = compile_wasm(
        r#"import gleam/io

pub fn main() {
io.print("hi")
io.println("!")
}
"#,
    );
    let engine = Engine::default();
    let module = Module::new(&engine, &wasm.bytes).expect("compile wasm module");
    let mut linker = Linker::new(&engine);
    linker
        .func_wrap("env", "print", |mut caller: Caller<'_, String>, ptr: i32| {
            let text = read_host_string(&mut caller, ptr);
            caller.data_mut().push_str(&text);
        })
        .expect("define print");
    linker
        .func_wrap("env", "println", |mut caller: Caller<'_, String>, ptr: i32| {
            let text = read_host_string(&mut caller, ptr);
            caller.data_mut().push_str(&text);
            caller.data_mut().push('\n');
        })
        .expect("define println");
    let mut store = Store::new(&engine, String::new());
    let instance = linker.instantiate(&mut store, &module).expect("instantiate module");
    let main = instance
        .get_typed_func::<(), ()>(&mut store, "main")
        .expect("get main export");
    main.call(&mut store, ()).expect("call main");
    assert_eq!(store.data(), "hi!\n");
}

fn read_host_string(caller: &mut Caller<'_, String>, ptr: i32) -> String {
    let memory = caller
        .get_export("memory")
        .and_then(|export| export.into_memory())
        .expect("memory export");
    let ptr = ptr as usize;
    let mut header = [0; 8];
    memory.read(&mut *caller, ptr, &mut header).expect("read string header");
    assert_eq!(u32::from_le_bytes(header[0..4].try_into().unwrap()), 1);
    let len = u32::from_le_bytes(header[4..8].try_into().unwrap()) as usize;
    let mut bytes = vec![0; len];
    memory
        .read(&mut *caller, ptr + 8, &mut bytes)
        .expect("read string data");
    String::from_utf8(bytes).expect("utf-8 string")
}

fn instantiate_debug_module(bytes: &[u8]) -> (Store<String>, Instance, WasmtimeMemory) {
    let engine = Engine::default();
    let module = Module::new(&engine, bytes).expect("compile wasm module");
    let mut linker = Linker::new(&engine);
    linker
        .func_wrap("env", "debug_i64", |mut caller: Caller<'_, String>, value: i64| {
            caller.data_mut().push_str(&value.to_string());
            caller.data_mut().push('\n');
        })
        .expect("define debug_i64");
    linker
        .func_wrap("env", "debug_value", |mut caller: Caller<'_, String>, ptr: i32| {
            let text = read_host_string(&mut caller, ptr);
            caller.data_mut().push_str(&text);
            caller.data_mut().push('\n');
        })
        .expect("define debug_value");
    let mut store = Store::new(&engine, String::new());
    let instance = linker.instantiate(&mut store, &module).expect("instantiate module");
    let memory = instance.get_memory(&mut store, "memory").expect("memory export");
    (store, instance, memory)
}

fn call_i64_export(instance: &Instance, store: &mut Store<String>, name: &str) -> i64 {
    instance
        .get_typed_func::<(), i64>(&mut *store, name)
        .unwrap_or_else(|_| panic!("get {name} export"))
        .call(store, ())
        .unwrap_or_else(|_| panic!("call {name}"))
}

fn call_i32_export(instance: &Instance, store: &mut Store<String>, name: &str) -> i32 {
    instance
        .get_typed_func::<(), i32>(&mut *store, name)
        .unwrap_or_else(|_| panic!("get {name} export"))
        .call(store, ())
        .unwrap_or_else(|_| panic!("call {name}"))
}

fn call_f64_export(instance: &Instance, store: &mut Store<String>, name: &str) -> f64 {
    instance
        .get_typed_func::<(), f64>(&mut *store, name)
        .unwrap_or_else(|_| panic!("get {name} export"))
        .call(store, ())
        .unwrap_or_else(|_| panic!("call {name}"))
}

fn read_exported_bytes(
    instance: &Instance, store: &mut Store<String>, memory: &WasmtimeMemory, name: &str, len: usize,
) -> Vec<u8> {
    let pointer = call_i32_export(instance, store, name) as usize;
    let mut bytes = vec![0; len];
    memory
        .read(&*store, pointer, &mut bytes)
        .unwrap_or_else(|_| panic!("read {name}"));
    bytes
}

fn assert_string_and_debug_intrinsics(instance: &Instance, store: &mut Store<String>, memory: &WasmtimeMemory) {
    let bytes = read_exported_bytes(instance, store, memory, "number", 16);
    assert_eq!(u32::from_le_bytes(bytes[4..8].try_into().unwrap()), 3);
    assert_eq!(&bytes[8..11], b"-42");

    let bytes = read_exported_bytes(instance, store, memory, "text", 16);
    assert_eq!(&bytes[8..10], b"ab");

    assert_eq!(call_i64_export(instance, store, "text_len"), 3);
    assert_eq!(call_i32_export(instance, store, "empty"), 1);
    assert_eq!(call_i64_export(instance, store, "debugged"), 42);

    let bytes = read_exported_bytes(instance, store, memory, "debugged_text", 16);
    assert_eq!(&bytes[8..10], b"ok");
    assert_eq!(store.data(), "42\nok\n");
}

fn assert_collection_and_number_intrinsics(instance: &Instance, store: &mut Store<String>, memory: &WasmtimeMemory) {
    assert_eq!(call_i64_export(instance, store, "item_count"), 3);
    assert_eq!(call_i64_export(instance, store, "reversed_head"), 3);

    let bytes = read_exported_bytes(instance, store, memory, "bool_text", 16);
    assert_eq!(&bytes[8..12], b"True");
    assert_eq!(call_i64_export(instance, store, "bool_rank"), -1);
    assert_eq!(call_i64_export(instance, store, "dict_value"), 42);
    assert_eq!(call_i32_export(instance, store, "dict_missing"), 0);
    assert_eq!(call_i64_export(instance, store, "dict_persistent_size"), 1);
    assert_eq!(call_i64_export(instance, store, "float_rank"), -1);
    assert_eq!(call_f64_export(instance, store, "float_larger"), 2.5);

    let bytes = read_exported_bytes(instance, store, memory, "float_text", 16);
    assert_eq!(&bytes[8..16], b"1.500000");

    assert_eq!(call_i64_export(instance, store, "same_value"), 9);
    assert_eq!(call_i64_export(instance, store, "constant_value"), 7);
}

fn assert_bit_array_intrinsics(instance: &Instance, store: &mut Store<String>, memory: &WasmtimeMemory) {
    assert_eq!(call_i64_export(instance, store, "bits_size"), 24);
    assert_eq!(call_i64_export(instance, store, "bytes_size"), 2);
    assert_eq!(call_i32_export(instance, store, "bits_empty"), 1);
    assert_eq!(call_i32_export(instance, store, "bits_start"), 1);
    assert_eq!(call_i64_export(instance, store, "bits_append_size"), 16);

    let bytes = read_exported_bytes(instance, store, memory, "bits_joined", 16);
    assert_eq!(u32::from_le_bytes(bytes[4..8].try_into().unwrap()), 24);
    assert_eq!(&bytes[8..11], &[1, 2, 3]);
}

#[test]
fn selects_browser_stdlib_io_host_imports() {
    let wasm = compile_wasm_target(
        r#"import gleam/io

pub fn main() { io.println("hi") }
"#,
        CompileTarget::Browser,
    )
    .expect("compile browser io");

    assert!(
        wasm.wat
            .contains("(import \"browser\" \"println\" (func (type 0) (param i32)))"),
        "{}",
        wasm.wat
    );
    assert!(!wasm.wat.contains("__stdlib_gleam_io_print "), "{}", wasm.wat);
}

#[test]
fn reports_unsupported_wasi_stdlib_io_host_imports() {
    let errors = compile_wasm_target(
        r#"import gleam/io

pub fn main() { io.println("hi") }
"#,
        CompileTarget::Wasi,
    )
    .expect_err("wasi io should be unsupported");

    assert!(errors.iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("stdlib host call `gleam/io.println` is not supported for target `wasi`")
    }));
}

#[test]
fn runs_initial_stdlib_intrinsics() {
    let wasm = compile_wasm(
        r#"import gleam/bit_array
import gleam/bool
import gleam/dict
import gleam/float
import gleam/function
import gleam/int
import gleam/io
import gleam/option
import gleam/order
import gleam/string
import gleam/list

pub fn number() { int.to_string(-42) }
pub fn text() { string.append("a", "b") }
pub fn text_len() -> Int { string.length(string.concat(["a", "bc"])) }
pub fn empty() -> Bool { string.is_empty("") }
pub fn item_count() -> Int { list.length([1, 2, 3]) }
pub fn reversed_head() -> Int {
case list.reverse([1, 2, 3]) {
 [head, ..] -> head
 _ -> 0
}
}
pub fn debugged() -> Int { io.debug(42) }
pub fn debugged_text() -> String { io.debug("ok") }
pub fn bool_text() -> String { bool.to_string(True) }
pub fn bool_rank() -> Int {
case bool.compare(False, True) {
 order.Lt -> -1
 order.Eq -> 0
 order.Gt -> 1
}
}
pub fn dict_value() -> Int {
let values = dict.insert(dict.new(), "a", 42)
case dict.get(values, "a") {
 option.Some(value) -> value
 option.None -> 0
}
}
pub fn dict_missing() -> Bool {
let values = dict.insert(dict.new(), "a", 42)
dict.has_key(dict.delete(values, "a"), "a")
}
pub fn dict_persistent_size() -> Int {
let original = dict.new()
let updated = dict.insert(original, "a", 42)
dict.size(original) + dict.size(updated)
}
pub fn float_rank() -> Int {
case float.compare(1.0, 2.0) {
 order.Lt -> -1
 order.Eq -> 0
 order.Gt -> 1
}
}
pub fn float_larger() -> Float { float.max(1.5, float.negate(-2.5)) }
pub fn float_text() -> String { float.to_string(1.5) }
pub fn same_value() -> Int { function.identity(9) }
pub fn constant_value() -> Int { function.constant(7, "ignored") }
pub fn bits_size() -> Int { bit_array.bit_size(<<1, 2, 3>>) }
pub fn bytes_size() -> Int { bit_array.byte_size(<<1:4, 2:4, 3:4>>) }
pub fn bits_empty() -> Bool { bit_array.is_empty(<<>>) }
pub fn bits_start() -> Bool { bit_array.starts_with(<<1, 2, 3>>, <<1, 2>>) }
pub fn bits_joined() -> BitArray { bit_array.concat([<<1>>, <<2>>, <<3>>]) }
pub fn bits_append_size() -> Int { bit_array.bit_size(bit_array.append(<<1>>, <<2>>)) }
"#,
    );
    let (mut store, instance, memory) = instantiate_debug_module(&wasm.bytes);

    assert_string_and_debug_intrinsics(&instance, &mut store, &memory);
    assert_collection_and_number_intrinsics(&instance, &mut store, &memory);
    assert_bit_array_intrinsics(&instance, &mut store, &memory);
}

#[test]
fn compiles_common_stdlib_fixture() {
    let wasm = compile_wasm(include_str!("../../../../fixtures/wasm/common_stdlib.gleam"));
    assert!(!wasm.wat.contains("(import \"env\" \"print\""), "{}", wasm.wat);
}

#[test]
fn runs_primitive_dynamic_decode_intrinsics() {
    let wasm = compile_wasm(
        r#"import gleam/dynamic
import gleam/dynamic/decode
import gleam/result.{Ok, Error}

pub fn decoded_int() -> Int {
case decode.run(dynamic.int(42), decode.int) {
 Ok(value) -> value
 Error(_) -> 0
}
}
"#,
    );

    let engine = Engine::default();
    let module = Module::new(&engine, &wasm.bytes).expect("compile wasm module");
    let mut store = Store::new(&engine, ());
    let instance = Instance::new(&mut store, &module, &[]).expect("instantiate module");
    let decoded_int = instance
        .get_typed_func::<(), i64>(&mut store, "decoded_int")
        .expect("get decoded_int export");

    assert_eq!(decoded_int.call(&mut store, ()).expect("call decoded_int"), 42);
}

#[test]
fn runs_higher_order_stdlib_intrinsics() {
    let wasm = compile_wasm(
        r#"import gleam/function
import gleam/list
import gleam/option.{Some}
import gleam/result.{Ok, Error}
import gleam/string

pub fn mapped_head() -> Int {
case list.map([1], fn(x) { x + 1 }) {
 [x] -> x
 _ -> 0
}
}

pub fn folded() -> Int {
list.fold([1, 2, 3], 0, fn(acc, x) { acc + x })
}

pub fn option_mapped() -> Int {
case option.map(Some(4), fn(x) { x + 3 }) {
 Some(x) -> x
 _ -> 0
}
}

pub fn result_mapped() -> Int {
case result.map(Ok(4), fn(x) { x + 5 }) {
 Ok(x) -> x
 Error(e) -> e
}
}

pub fn composed() -> Int {
let add1 = fn(x) { x + 1 }
let double = fn(x) { x * 2 }
let f = function.compose(add1, double)
f(4)
}

pub fn flipped() -> Int {
let sub = fn(a, b) { a - b }
let f = function.flip(sub)
f(3, 10)
}

pub fn string_mapped_length() -> Int {
case list.map(["a"], fn(x) { x <> "bc" }) {
 [x] -> string.length(x)
 _ -> 0
}
}

pub fn option_string_length() -> Int {
case option.map(Some("a"), fn(x) { x <> "bc" }) {
 Some(x) -> string.length(x)
 _ -> 0
}
}

pub fn result_string_length() -> Int {
case result.map(Ok("a"), fn(x) { x <> "bc" }) {
 Ok(x) -> string.length(x)
 Error(e) -> string.length(e)
}
}

pub fn nested_list_string_length() -> Int {
case list.map([["a"]], fn(xs) { xs }) {
 [[x]] -> string.length(x)
 _ -> 0
}
}

pub fn nested_option_string_length() -> Int {
case option.map(Some("a"), fn(x) { Some(x) }) {
 Some(Some(x)) -> string.length(x)
 _ -> 0
}
}

pub fn float_mapped_value() -> Float {
case list.map([1.0], fn(x) { x +. 1.5 }) {
 [x] -> x
 _ -> 0.0
}
}
"#,
    );

    let engine = Engine::default();
    let module = Module::new(&engine, &wasm.bytes).expect("compile wasm module");
    let mut store = Store::new(&engine, ());
    let instance = Instance::new(&mut store, &module, &[]).expect("instantiate module");
    for (name, expected) in [
        ("mapped_head", 2),
        ("folded", 6),
        ("option_mapped", 7),
        ("result_mapped", 9),
        ("composed", 9),
        ("flipped", 7),
        ("string_mapped_length", 3),
        ("option_string_length", 3),
        ("result_string_length", 3),
        ("nested_list_string_length", 1),
        ("nested_option_string_length", 1),
    ] {
        let function = instance
            .get_typed_func::<(), i64>(&mut store, name)
            .expect("get export");
        assert_eq!(function.call(&mut store, ()).expect("call export"), expected, "{name}");
    }

    let float_mapped = instance
        .get_typed_func::<(), f64>(&mut store, "float_mapped_value")
        .expect("get float_mapped_value export");
    assert_eq!(float_mapped.call(&mut store, ()).expect("call export"), 2.5);
}

#[test]
fn runs_closure_values_passed_to_stdlib_intrinsics() {
    let wasm = compile_wasm(
        r#"import gleam/function
import gleam/list
import gleam/option.{Some}

pub fn mapped_with_value_closure() -> Int {
let inc = fn(x) { x + 1 }
case list.map([1], inc) {
 [x] -> x
 _ -> 0
}
}

pub fn composed_with_value_closure() -> Int {
let inc = fn(x) { x + 1 }
let double = fn(x) { x * 2 }
let composed = function.compose(inc, double)
composed(4)
}

pub fn option_with_value_closure() -> Int {
let inc = fn(x) { x + 1 }
case option.map(Some(4), inc) {
 Some(x) -> x
 _ -> 0
}
}
"#,
    );

    let engine = Engine::default();
    let module = Module::new(&engine, &wasm.bytes).expect("compile wasm module");
    let mut store = Store::new(&engine, ());
    let instance = Instance::new(&mut store, &module, &[]).expect("instantiate module");
    for (name, expected) in [
        ("mapped_with_value_closure", 2),
        ("composed_with_value_closure", 9),
        ("option_with_value_closure", 5),
    ] {
        let function = instance
            .get_typed_func::<(), i64>(&mut store, name)
            .expect("get export");
        assert_eq!(function.call(&mut store, ()).expect("call export"), expected, "{name}");
    }
}

#[test]
fn runs_captured_closures_passed_to_stdlib_intrinsics() {
    let wasm = compile_wasm(
        r#"import gleam/list
import gleam/option.{Some}

pub fn mapped_with_capture(seed: Int) -> Int {
case list.map([1, 2], fn(x) { x + seed }) {
 [a, b] -> a + b
 _ -> 0
}
}

pub fn folded_with_capture(seed: Int) -> Int {
list.fold([1, 2, 3], seed, fn(acc, x) { acc + x })
}

pub fn option_with_capture(seed: Int) -> Int {
case option.map(Some(4), fn(x) { x + seed }) {
 Some(x) -> x
 _ -> 0
}
}
"#,
    );

    let engine = Engine::default();
    let module = Module::new(&engine, &wasm.bytes).expect("compile wasm module");
    let mut store = Store::new(&engine, ());
    let instance = Instance::new(&mut store, &module, &[]).expect("instantiate module");
    let mapped = instance
        .get_typed_func::<i64, i64>(&mut store, "mapped_with_capture")
        .expect("get mapped_with_capture export");
    assert_eq!(mapped.call(&mut store, 10).expect("call export"), 23);
    let folded = instance
        .get_typed_func::<i64, i64>(&mut store, "folded_with_capture")
        .expect("get folded_with_capture export");
    assert_eq!(folded.call(&mut store, 10).expect("call export"), 16);
    let option = instance
        .get_typed_func::<i64, i64>(&mut store, "option_with_capture")
        .expect("get option_with_capture export");
    assert_eq!(option.call(&mut store, 10).expect("call export"), 14);
}

#[test]
fn runs_nested_stdlib_intrinsic_callbacks() {
    let wasm = compile_wasm(
        r#"import gleam/list
import gleam/option.{Some}

pub fn nested_option_in_list_map() -> Int {
case list.map([1, 2], fn(x) {
 case option.map(Some(x), fn(y) { y + 1 }) {
   Some(y) -> y
   _ -> 0
 }
}) {
 [a, b] -> a + b
 _ -> 0
}
}

pub fn nested_list_in_option_map() -> Int {
case option.map(Some([1, 2]), fn(xs) {
 case list.map(xs, fn(x) { x * 2 }) {
   [a, b] -> a + b
   _ -> 0
 }
}) {
 Some(x) -> x
 _ -> 0
}
}
"#,
    );

    let engine = Engine::default();
    let module = Module::new(&engine, &wasm.bytes).expect("compile wasm module");
    let mut store = Store::new(&engine, ());
    let instance = Instance::new(&mut store, &module, &[]).expect("instantiate module");
    for (name, expected) in [("nested_option_in_list_map", 5), ("nested_list_in_option_map", 6)] {
        let function = instance
            .get_typed_func::<(), i64>(&mut store, name)
            .expect("get export");
        assert_eq!(function.call(&mut store, ()).expect("call export"), expected, "{name}");
    }
}

#[test]
fn reports_generic_callback_diagnostics_before_wasm_emission() {
    let diagnostics = compile_wasm_target(
        r#"import gleam/list

fn id(x) { x }

pub fn main() -> Int {
case list.map([1], id) {
 [x] -> x
 _ -> 0
}
}
"#,
        CompileTarget::Wasmtime,
    )
    .expect_err("generic callback should fail before Wasm emission");

    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("cannot be lowered without monomorphization")
    }));
}

#[test]
fn callback_failures_in_stdlib_intrinsics_trap() {
    let wasm = compile_wasm(
        r#"import gleam/list
import gleam/option.{Some}

pub fn list_map_callback_failure() -> Int {
case list.map([1], fn(x) {
 case x {
   0 -> 0
   _ -> panic
 }
}) {
 [x] -> x
 _ -> 0
}
}

pub fn option_map_callback_failure() -> Int {
case option.map(Some(1), fn(x) {
 case x {
   0 -> 0
   _ -> panic
 }
}) {
 Some(x) -> x
 _ -> 0
}
}
"#,
    );

    let engine = Engine::default();
    let module = Module::new(&engine, &wasm.bytes).expect("compile wasm module");
    let mut store = Store::new(&engine, ());
    let instance = Instance::new(&mut store, &module, &[]).expect("instantiate module");
    for name in ["list_map_callback_failure", "option_map_callback_failure"] {
        let function = instance
            .get_typed_func::<(), i64>(&mut store, name)
            .expect("get export");
        assert!(function.call(&mut store, ()).is_err(), "{name} should trap");
    }
}

#[test]
fn runs_string_concat_and_value_equality_codegen() {
    let wasm = compile_wasm(
        r#"pub fn join() { "ab" <> "cd" }
pub fn same() { "hi" == "hi" }
"#,
    );
    let engine = Engine::default();
    let module = Module::new(&engine, &wasm.bytes).expect("compile wasm module");
    let mut store = Store::new(&engine, ());
    let instance = Instance::new(&mut store, &module, &[]).expect("instantiate module");
    let join = instance
        .get_typed_func::<(), i32>(&mut store, "join")
        .expect("get join export");
    let pointer = join.call(&mut store, ()).expect("call join") as usize;
    let memory = instance.get_memory(&mut store, "memory").expect("memory export");
    let mut bytes = [0; 16];
    memory.read(&store, pointer, &mut bytes).expect("read string");
    assert_eq!(u32::from_le_bytes(bytes[4..8].try_into().unwrap()), 4);
    assert_eq!(&bytes[8..12], b"abcd");

    let same = instance
        .get_typed_func::<(), i32>(&mut store, "same")
        .expect("get same export");
    assert_eq!(same.call(&mut store, ()).expect("call same"), 1);
}

#[test]
fn runtime_helpers_allocate_strings_and_compare_concatenation() {
    let instance = runtime_helper_instance(
        r#"
(data (i32.const 2048) "ab")
(data (i32.const 2050) "cd")
(func $concat (export "concat") (result i32)
 i32.const 2048
 i32.const 2
 call $__string_new
 i32.const 2050
 i32.const 2
 call $__string_new
 call $__string_concat)
(func $compare (export "compare") (result i32)
 i32.const 2048
 i32.const 2
 call $__string_new
 i32.const 2050
 i32.const 2
 call $__string_new
 call $__string_compare)
"#,
    );
    let (engine, mut store, instance) = instance;
    let _engine = engine;
    let concat = instance
        .get_typed_func::<(), i32>(&mut store, "concat")
        .expect("get concat export");
    let pointer = concat.call(&mut store, ()).expect("call concat") as usize;
    let memory = instance.get_memory(&mut store, "memory").expect("memory export");
    let mut bytes = [0; 16];
    memory.read(&store, pointer, &mut bytes).expect("read concat string");
    assert_eq!(u32::from_le_bytes(bytes[4..8].try_into().unwrap()), 4);
    assert_eq!(&bytes[8..12], b"abcd");

    let compare = instance
        .get_typed_func::<(), i32>(&mut store, "compare")
        .expect("get compare export");
    assert_eq!(compare.call(&mut store, ()).expect("call compare"), -1);
}

#[test]
fn runtime_allocation_grows_memory_without_moving_existing_objects() {
    let instance = runtime_helper_instance(
        r#"
(data (i32.const 2048) "ab")
(func $stable (export "stable") (result i32)
 (local $ptr i32)
 i32.const 2048
 i32.const 2
 call $__string_new
 local.set $ptr
 i32.const 2048
 i32.const 70000
 call $__string_new
 drop
 local.get $ptr)
(func $pages (export "pages") (result i32)
 memory.size)
"#,
    );
    let (engine, mut store, instance) = instance;
    let _engine = engine;
    let pages = instance
        .get_typed_func::<(), i32>(&mut store, "pages")
        .expect("get pages export");
    assert_eq!(pages.call(&mut store, ()).expect("call pages"), 1);
    let stable = instance
        .get_typed_func::<(), i32>(&mut store, "stable")
        .expect("get stable export");
    let pointer = stable.call(&mut store, ()).expect("call stable") as usize;
    assert_eq!(pointer, runtime::RuntimeConfig::DEFAULT.heap_start as usize);
    assert!(pages.call(&mut store, ()).expect("call pages after growth") > 1);

    let memory = instance.get_memory(&mut store, "memory").expect("memory export");
    let mut bytes = [0; 16];
    memory.read(&store, pointer, &mut bytes).expect("read stable object");
    assert_eq!(u32::from_le_bytes(bytes[0..4].try_into().unwrap()), 1);
    assert_eq!(u32::from_le_bytes(bytes[4..8].try_into().unwrap()), 2);
    assert_eq!(&bytes[8..10], b"ab");
}

#[test]
fn runtime_allocation_failure_writes_structured_panic_payload() {
    let instance = runtime_helper_instance_with_memory(
        r#"
(data (i32.const 2048) "ab")
(func $too_large (export "too_large") (result i32)
 i32.const 2048
 i32.const 70000
 call $__string_new)
"#,
        "  (memory (export \"memory\") 1 1)",
    );
    let (engine, mut store, instance) = instance;
    let _engine = engine;
    let too_large = instance
        .get_typed_func::<(), i32>(&mut store, "too_large")
        .expect("get too_large export");
    assert!(too_large.call(&mut store, ()).is_err());

    let last_panic = instance
        .get_typed_func::<(), i32>(&mut store, "__last_panic")
        .expect("get __last_panic export");
    let pointer = last_panic.call(&mut store, ()).expect("call __last_panic") as usize;
    assert_eq!(pointer, 64);

    let memory = instance.get_memory(&mut store, "memory").expect("memory export");
    let mut bytes = [0; 28];
    memory.read(&store, pointer, &mut bytes).expect("read allocation panic");
    assert_eq!(u32::from_le_bytes(bytes[0..4].try_into().unwrap()), 10);
    assert_eq!(u32::from_le_bytes(bytes[4..8].try_into().unwrap()), 2);
    assert_eq!(u32::from_le_bytes(bytes[8..12].try_into().unwrap()), 1);
    assert_eq!(u64::from_le_bytes(bytes[12..20].try_into().unwrap()), 70008);
    assert_eq!(u64::from_le_bytes(bytes[20..28].try_into().unwrap()), 4096);
}

#[test]
fn runtime_host_borrowed_pointer_stays_stable_across_growth() {
    let instance = runtime_helper_instance(
        r#"
(data (i32.const 2048) "ab")
(func $borrow (export "borrow") (result i32)
 i32.const 2048
 i32.const 2
 call $__string_new)
(func $grow (export "grow")
 i32.const 2048
 i32.const 70000
 call $__string_new
 drop)
"#,
    );
    let (engine, mut store, instance) = instance;
    let _engine = engine;
    let borrow = instance
        .get_typed_func::<(), i32>(&mut store, "borrow")
        .expect("get borrow export");
    let pointer = borrow.call(&mut store, ()).expect("call borrow") as usize;
    assert_eq!(pointer, runtime::RuntimeConfig::DEFAULT.heap_start as usize);

    let grow = instance
        .get_typed_func::<(), ()>(&mut store, "grow")
        .expect("get grow export");
    grow.call(&mut store, ()).expect("call grow");

    let memory = instance.get_memory(&mut store, "memory").expect("memory export");
    let mut bytes = [0; 16];
    memory.read(&store, pointer, &mut bytes).expect("read borrowed pointer");
    assert_eq!(u32::from_le_bytes(bytes[0..4].try_into().unwrap()), 1);
    assert_eq!(u32::from_le_bytes(bytes[4..8].try_into().unwrap()), 2);
    assert_eq!(&bytes[8..10], b"ab");
}

#[test]
fn runtime_helpers_compare_nested_managed_values_structurally() {
    let instance = runtime_helper_instance(
        r#"
(data (i32.const 2048) "aa")
(data (i32.const 2050) "bb")
(func $nested_equal (export "nested_equal") (result i32)
 (local $left_inner i32) (local $right_inner i32)
 i32.const 3000
 i32.const 2048
 i32.const 2
 call $__string_new
 i64.extend_i32_u
 i64.store
 i32.const 3008
 i64.const 7
 i64.store
 i32.const 2
 i32.const 3000
 call $__tuple_new
 local.set $left_inner
 i32.const 3000
 i32.const 2048
 i32.const 2
 call $__string_new
 i64.extend_i32_u
 i64.store
 i32.const 3008
 i64.const 7
 i64.store
 i32.const 2
 i32.const 3000
 call $__tuple_new
 local.set $right_inner
 local.get $left_inner
 i64.extend_i32_u
 i32.const 0
 call $__list_cons
 local.get $right_inner
 i64.extend_i32_u
 i32.const 0
 call $__list_cons
 call $__equal_value)
(func $nested_order (export "nested_order") (result i32)
 i32.const 3000
 i32.const 2048
 i32.const 2
 call $__string_new
 i64.extend_i32_u
 i64.store
 i32.const 1
 i32.const 3000
 call $__tuple_new
 i32.const 3000
 i32.const 2050
 i32.const 2
 call $__string_new
 i64.extend_i32_u
 i64.store
 i32.const 1
 i32.const 3000
 call $__tuple_new
 call $__compare_value)
"#,
    );
    let (engine, mut store, instance) = instance;
    let _engine = engine;
    let nested_equal = instance
        .get_typed_func::<(), i32>(&mut store, "nested_equal")
        .expect("get nested_equal export");
    assert_eq!(nested_equal.call(&mut store, ()).expect("call nested_equal"), 1);
    let nested_order = instance
        .get_typed_func::<(), i32>(&mut store, "nested_order")
        .expect("get nested_order export");
    assert_eq!(nested_order.call(&mut store, ()).expect("call nested_order"), -1);
}

#[test]
fn runtime_helpers_allocate_and_append_bit_arrays() {
    let instance = runtime_helper_instance(
        r#"
(data (i32.const 2048) "\a0\c0")
(data (i32.const 2052) "\ac")
(func $append (export "append") (result i32)
 i32.const 2048
 i32.const 4
 call $__bit_array_new
 i32.const 2049
 i32.const 4
 call $__bit_array_new
 call $__bit_array_append)
(func $matches (export "matches") (result i32)
 call $append
 i32.const 0
 i32.const 2052
 i32.const 8
 call $__bit_array_new
 call $__bit_array_match)
"#,
    );
    let (engine, mut store, instance) = instance;
    let _engine = engine;
    let append = instance
        .get_typed_func::<(), i32>(&mut store, "append")
        .expect("get append export");
    let pointer = append.call(&mut store, ()).expect("call append") as usize;
    let memory = instance.get_memory(&mut store, "memory").expect("memory export");
    let mut bytes = [0; 16];
    memory.read(&store, pointer, &mut bytes).expect("read bit array");
    assert_eq!(u32::from_le_bytes(bytes[4..8].try_into().unwrap()), 8);
    assert_eq!(bytes[8], 0b1010_1100);

    let matches = instance
        .get_typed_func::<(), i32>(&mut store, "matches")
        .expect("get matches export");
    assert_eq!(matches.call(&mut store, ()).expect("call matches"), 1);
}

#[test]
fn runtime_helpers_allocate_managed_values_and_expose_debug_data() {
    let instance = runtime_helper_instance(
        r#"
(data (i32.const 2048) "\2a\00\00\00\00\00\00\00\2b\00\00\00\00\00\00\00")
(data (i32.const 2080) "\20\00\00\00")
(func $tuple (export "tuple") (result i32)
 i32.const 2
 i32.const 2048
 call $__tuple_new)
(func $custom (export "custom") (result i32)
 i32.const 99
 i32.const 2
 i32.const 2048
 call $__custom_new)
(func $closure (export "closure") (result i32)
 i32.const 7
 i32.const 1
 i32.const 2080
 call $__closure_new)
(func $panic_value (export "panic_value") (result i32)
 i32.const 3
 i32.const 1
 i32.const 2048
 call $__panic_value_new)
(func $panic_reason (export "panic_reason") (result i32)
 call $panic_value
 call $__debug_reason)
(func $panic_payload_0 (export "panic_payload_0") (result i64)
 call $panic_value
 i32.const 0
 call $__debug_payload_i64)
(func $tuple_tag (export "tuple_tag") (result i32)
 call $tuple
 call $__debug_tag)
(func $tuple_first (export "tuple_first") (result i64)
 call $tuple
 i32.const 0
 call $__field_load_i64)
(func $assert_ok (export "assert_ok")
 i32.const 1
 call $__assert)
(func $assert_fail (export "assert_fail")
 i32.const 0
 call $__assert)
"#,
    );
    let (engine, mut store, instance) = instance;
    let _engine = engine;
    let tuple_tag = instance
        .get_typed_func::<(), i32>(&mut store, "tuple_tag")
        .expect("get tuple_tag export");
    assert_eq!(tuple_tag.call(&mut store, ()).expect("call tuple_tag"), 3);

    let tuple_first = instance
        .get_typed_func::<(), i64>(&mut store, "tuple_first")
        .expect("get tuple_first export");
    assert_eq!(tuple_first.call(&mut store, ()).expect("call tuple_first"), 42);

    let custom = instance
        .get_typed_func::<(), i32>(&mut store, "custom")
        .expect("get custom export");
    let custom_pointer = custom.call(&mut store, ()).expect("call custom") as usize;
    let memory = instance.get_memory(&mut store, "memory").expect("memory export");
    let mut custom_bytes = [0; 32];
    memory
        .read(&store, custom_pointer, &mut custom_bytes)
        .expect("read custom object");
    assert_eq!(u32::from_le_bytes(custom_bytes[0..4].try_into().unwrap()), 5);
    assert_eq!(u32::from_le_bytes(custom_bytes[8..12].try_into().unwrap()), 99);

    let closure = instance
        .get_typed_func::<(), i32>(&mut store, "closure")
        .expect("get closure export");
    let closure_pointer = closure.call(&mut store, ()).expect("call closure") as usize;
    let mut closure_bytes = [0; 16];
    memory
        .read(&store, closure_pointer, &mut closure_bytes)
        .expect("read closure object");
    assert_eq!(u32::from_le_bytes(closure_bytes[0..4].try_into().unwrap()), 6);
    assert_eq!(u32::from_le_bytes(closure_bytes[8..12].try_into().unwrap()), 7);
    assert_eq!(u32::from_le_bytes(closure_bytes[12..16].try_into().unwrap()), 32);

    let panic_value = instance
        .get_typed_func::<(), i32>(&mut store, "panic_value")
        .expect("get panic_value export");
    let panic_pointer = panic_value.call(&mut store, ()).expect("call panic value") as usize;
    let mut panic_bytes = [0; 24];
    memory
        .read(&store, panic_pointer, &mut panic_bytes)
        .expect("read panic object");
    assert_eq!(u32::from_le_bytes(panic_bytes[0..4].try_into().unwrap()), 10);
    assert_eq!(u32::from_le_bytes(panic_bytes[8..12].try_into().unwrap()), 3);

    let panic_reason = instance
        .get_typed_func::<(), i32>(&mut store, "panic_reason")
        .expect("get panic_reason export");
    assert_eq!(panic_reason.call(&mut store, ()).expect("call panic reason"), 3);
    let panic_payload_0 = instance
        .get_typed_func::<(), i64>(&mut store, "panic_payload_0")
        .expect("get panic_payload_0 export");
    assert_eq!(panic_payload_0.call(&mut store, ()).expect("call panic payload"), 42);

    let assert_ok = instance
        .get_typed_func::<(), ()>(&mut store, "assert_ok")
        .expect("get assert_ok export");
    assert_ok.call(&mut store, ()).expect("assert ok");
    let assert_fail = instance
        .get_typed_func::<(), ()>(&mut store, "assert_fail")
        .expect("get assert_fail export");
    assert!(assert_fail.call(&mut store, ()).is_err());
}

fn runtime_helper_instance(extra_wat: &str) -> (Engine, Store<()>, Instance) {
    runtime_helper_instance_with_memory(extra_wat, "  (memory (export \"memory\") 1)")
}

fn runtime_helper_instance_with_memory(extra_wat: &str, memory_wat: &str) -> (Engine, Store<()>, Instance) {
    let roots = runtime_helper_roots(extra_wat);
    let helpers = runtime_helper_wat(runtime::RuntimeConfig::DEFAULT, &roots);
    let wat = format!(
        "(module\n{memory_wat}\n  (global $__heap (mut i32) (i32.const {}))\n  (global $__last_panic_payload (mut i32) (i32.const 0))\n{helpers}{extra_wat})\n",
        runtime::RuntimeConfig::DEFAULT.heap_start
    );
    let bytes = wat::parse_str(&wat).expect("parse runtime helper wat");
    let engine = Engine::default();
    let module = Module::new(&engine, bytes).expect("compile helper module");
    let mut store = Store::new(&engine, ());
    let instance = Instance::new(&mut store, &module, &[]).expect("instantiate helper module");
    (engine, store, instance)
}
