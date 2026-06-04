use std::fmt::Write;

use crate::diagnostic::{Diagnostic, DiagnosticCode, Diagnostics, Label};
use crate::ir::{self, ExpressionKind, Instruction};
use crate::{ast::LiteralKind, runtime, types::Type};

/// WebAssembly output from the backend.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WasmModule {
    pub wat: String,
    pub bytes: Vec<u8>,
}

pub fn emit(module: ir::Module) -> Result<WasmModule, Diagnostics> {
    let wat = emit_wat(&module)?;
    let bytes = wat::parse_str(&wat).map_err(|error| {
        vec![Diagnostic::new(
            DiagnosticCode::WasmError,
            format!("could not assemble WAT: {error}"),
        )]
    })?;

    Ok(WasmModule { wat, bytes })
}

pub fn emit_wat(module: &ir::Module) -> Result<String, Diagnostics> {
    let mut emitter = Emitter {
        functions: String::new(),
        diagnostics: Vec::new(),
        data: Vec::new(),
        config: runtime::RuntimeConfig::DEFAULT,
        next_static_offset: runtime::RuntimeConfig::DEFAULT.static_data_start,
        uses_runtime: false,
    };

    for function in &module.functions {
        emitter.function(function);
    }

    if !emitter.diagnostics.is_empty() {
        return Err(emitter.diagnostics);
    }

    let mut wat = String::from("(module\n");
    if emitter.uses_runtime {
        wat.push_str(&runtime_prelude(emitter.config));
    }
    wat.push_str(&emitter.functions);
    for object in emitter.data {
        writeln!(
            wat,
            "  (data (i32.const {}) \"{}\")",
            object.offset,
            wat_bytes(&object.bytes)
        )
        .expect("write WAT");
    }
    wat.push_str(")\n");
    Ok(wat)
}

struct Emitter {
    functions: String,
    diagnostics: Diagnostics,
    data: Vec<runtime::StaticObject>,
    config: runtime::RuntimeConfig,
    next_static_offset: u32,
    uses_runtime: bool,
}

impl Emitter {
    fn function(&mut self, function: &ir::Function) {
        let return_type = match wasm_type(&function.return_type) {
            Some(return_type) => return_type,
            None if function.return_type == Type::Nil => "",
            None => {
                self.unsupported_type(&function.return_type, function.span);
                return;
            }
        };

        write!(self.functions, "  (func ${}", function.name).expect("write WAT");
        if function.public {
            write!(self.functions, " (export \"{}\")", function.name).expect("write WAT");
        }

        for param in &function.params {
            match wasm_type(&param.type_) {
                Some(type_) => write!(self.functions, " (param ${} {type_})", local_name(param)).expect("write WAT"),
                None => self.unsupported_type(&param.type_, param.span),
            }
        }

        if !return_type.is_empty() {
            write!(self.functions, " (result {return_type})").expect("write WAT");
        }
        self.functions.push('\n');

        for local in function.locals.iter().skip(function.params.len()) {
            match wasm_type(&local.type_) {
                Some(type_) => {
                    writeln!(self.functions, "    (local ${} {type_})", local_name(local)).expect("write WAT")
                }
                None => self.unsupported_type(&local.type_, local.span),
            }
        }

        self.block(&function.body);
        self.functions.push_str("  )\n");
    }

    fn block(&mut self, block: &ir::Block) {
        for instruction in &block.instructions {
            match instruction {
                Instruction::Evaluate { expression, .. } => {
                    self.expression(expression);
                    if wasm_type(&expression.type_).is_some() {
                        writeln!(self.functions, "    drop").expect("write WAT");
                    }
                }
                Instruction::LocalSet { local, value, .. } => {
                    self.expression(value);
                    writeln!(self.functions, "    local.set ${}", local.0).expect("write WAT");
                }
                Instruction::AssertMatch { value, pattern, span, .. } => {
                    self.pattern_test(value, pattern, *span);
                    writeln!(self.functions, "    if").expect("write WAT");
                    writeln!(self.functions, "    else").expect("write WAT");
                    writeln!(self.functions, "    unreachable").expect("write WAT");
                    writeln!(self.functions, "    end").expect("write WAT");
                }
            }
        }
        self.expression(&block.result);
    }

    fn expression(&mut self, expression: &ir::Expression) {
        match &expression.kind {
            ExpressionKind::Literal(literal) => match literal.kind {
                LiteralKind::Int => writeln!(self.functions, "    i64.const {}", literal.source).expect("write WAT"),
                LiteralKind::Float => writeln!(self.functions, "    f64.const {}", literal.source).expect("write WAT"),
                LiteralKind::Bool => writeln!(
                    self.functions,
                    "    i32.const {}",
                    if literal.source == "True" { 1 } else { 0 }
                )
                .expect("write WAT"),
                LiteralKind::Nil => {}
                LiteralKind::String => {
                    let pointer = self.static_string(&literal.source);
                    writeln!(self.functions, "    i32.const {pointer}").expect("write WAT");
                }
            },
            ExpressionKind::LocalGet(local) => {
                writeln!(self.functions, "    local.get ${}", local.0).expect("write WAT")
            }
            ExpressionKind::DirectCall(call) => {
                for argument in &call.arguments {
                    self.expression(&argument.value);
                }
                writeln!(self.functions, "    call ${}", call.function).expect("write WAT");
            }
            ExpressionKind::Branch(branch) => self.branch(branch, &expression.type_, expression.span),
            ExpressionKind::IndirectCall(_)
            | ExpressionKind::FunctionValue(_)
            | ExpressionKind::AnonymousFunction(_)
            | ExpressionKind::Pipeline(_)
            | ExpressionKind::Use(_)
            | ExpressionKind::Tuple(_)
            | ExpressionKind::List(_)
            | ExpressionKind::Record(_)
            | ExpressionKind::Constructor(_)
            | ExpressionKind::FieldAccess { .. }
            | ExpressionKind::RecordUpdate { .. }
            | ExpressionKind::ListCons { .. }
            | ExpressionKind::ListDeconstruct { .. }
            | ExpressionKind::TupleElement { .. }
            | ExpressionKind::Compare { .. }
            | ExpressionKind::RuntimeEquality { .. }
            | ExpressionKind::Memory(_)
            | ExpressionKind::Failure(_) => self.unsupported_expression(expression),
        }
    }

    fn branch(&mut self, branch: &ir::Branch, type_: &Type, span: crate::source::Span) {
        if branch
            .subjects
            .iter()
            .any(|subject| wasm_type(&subject.type_).is_none())
        {
            self.diagnostics.push(
                Diagnostic::new(DiagnosticCode::WasmError, "branch subject type is not supported")
                    .with_label(Label::primary(span, "unsupported branch here")),
            );
            return;
        }

        self.branch_clause(branch, 0, type_, span);
    }

    fn branch_clause(&mut self, branch: &ir::Branch, index: usize, type_: &Type, span: crate::source::Span) {
        let result_type = wasm_type(type_).unwrap_or("");
        let Some(clause) = branch.clauses.get(index) else {
            writeln!(self.functions, "    unreachable").expect("write WAT");
            return;
        };

        self.branch_test(&branch.subjects, &clause.patterns, clause.guard.as_ref(), clause.span);
        if result_type.is_empty() {
            writeln!(self.functions, "    if").expect("write WAT");
        } else {
            writeln!(self.functions, "    if (result {result_type})").expect("write WAT");
        }
        for (subject, pattern) in branch.subjects.iter().zip(&clause.patterns) {
            self.bind_pattern_values(subject, pattern);
        }
        self.expression(&clause.body);
        writeln!(self.functions, "    else").expect("write WAT");

        self.branch_clause(branch, index + 1, type_, span);
        writeln!(self.functions, "    end").expect("write WAT");
    }

    fn branch_test(
        &mut self, subjects: &[ir::Expression], patterns: &[ir::IrPattern], guard: Option<&ir::Expression>,
        span: crate::source::Span,
    ) {
        if subjects.len() != patterns.len() {
            self.diagnostics.push(
                Diagnostic::new(DiagnosticCode::WasmError, "branch subject and pattern counts differ")
                    .with_label(Label::primary(span, "invalid branch here")),
            );
            writeln!(self.functions, "    i32.const 0").expect("write WAT");
            return;
        }

        let mut emitted_any = false;
        for (subject, pattern) in subjects.iter().zip(patterns) {
            self.pattern_test(subject, pattern, span);
            if emitted_any {
                writeln!(self.functions, "    i32.and").expect("write WAT");
            }
            emitted_any = true;
        }

        if !emitted_any {
            writeln!(self.functions, "    i32.const 1").expect("write WAT");
        }

        if let Some(guard) = guard {
            self.expression(guard);
            writeln!(self.functions, "    i32.and").expect("write WAT");
        }
    }

    fn bind_pattern_values(&mut self, subject: &ir::Expression, pattern: &ir::IrPattern) {
        match pattern {
            ir::IrPattern::Binding(local) => {
                self.expression(subject);
                writeln!(self.functions, "    local.set ${}", local.0).expect("write WAT");
            }
            ir::IrPattern::Alias { pattern, local } => {
                self.bind_pattern_values(subject, pattern);
                self.expression(subject);
                writeln!(self.functions, "    local.set ${}", local.0).expect("write WAT");
            }
            ir::IrPattern::Discard
            | ir::IrPattern::Literal(_)
            | ir::IrPattern::Tuple(_)
            | ir::IrPattern::List { .. }
            | ir::IrPattern::Constructor { .. } => {}
        }
    }

    fn pattern_test(&mut self, subject: &ir::Expression, pattern: &ir::IrPattern, span: crate::source::Span) {
        match pattern {
            ir::IrPattern::Discard | ir::IrPattern::Binding(_) => {
                writeln!(self.functions, "    i32.const 1").expect("write WAT");
            }
            ir::IrPattern::Alias { pattern, .. } => self.pattern_test(subject, pattern, span),
            ir::IrPattern::Literal(literal) => {
                self.expression(subject);
                self.literal(literal);
                match subject.type_ {
                    Type::Int => writeln!(self.functions, "    i64.eq").expect("write WAT"),
                    Type::Float => writeln!(self.functions, "    f64.eq").expect("write WAT"),
                    Type::Bool | Type::String => writeln!(self.functions, "    i32.eq").expect("write WAT"),
                    Type::Nil => writeln!(self.functions, "    i32.const 1").expect("write WAT"),
                    _ => self.diagnostics.push(
                        Diagnostic::new(DiagnosticCode::WasmError, "pattern type is not supported")
                            .with_label(Label::primary(span, "unsupported pattern here")),
                    ),
                }
            }
            ir::IrPattern::Tuple(_) | ir::IrPattern::List { .. } | ir::IrPattern::Constructor { .. } => {
                self.diagnostics.push(
                    Diagnostic::new(
                        DiagnosticCode::WasmError,
                        "managed-value patterns are not supported by the WASM backend yet",
                    )
                    .with_label(Label::primary(span, "unsupported pattern here")),
                );
                writeln!(self.functions, "    i32.const 0").expect("write WAT");
            }
        }
    }

    fn literal(&mut self, literal: &ir::Literal) {
        match literal.kind {
            LiteralKind::Int => writeln!(self.functions, "    i64.const {}", literal.source).expect("write WAT"),
            LiteralKind::Float => writeln!(self.functions, "    f64.const {}", literal.source).expect("write WAT"),
            LiteralKind::Bool => writeln!(
                self.functions,
                "    i32.const {}",
                if literal.source == "True" { 1 } else { 0 }
            )
            .expect("write WAT"),
            LiteralKind::Nil => {}
            LiteralKind::String => {
                let pointer = self.static_string(&literal.source);
                writeln!(self.functions, "    i32.const {pointer}").expect("write WAT");
            }
        }
    }

    fn static_string(&mut self, source: &str) -> u32 {
        self.uses_runtime = true;
        let string = source.trim_matches('"');
        let object = runtime::string_object(self.config, self.next_static_offset, string);
        let pointer = object.offset;
        self.next_static_offset = self.config.layout.align_to(object.offset + object.bytes.len() as u32);
        self.data.push(object);
        pointer
    }

    fn unsupported_expression(&mut self, expression: &ir::Expression) {
        self.diagnostics.push(
            Diagnostic::new(
                DiagnosticCode::WasmError,
                format!("IR expression `{:?}` cannot be emitted yet", expression.kind),
            )
            .with_label(Label::primary(expression.span, "unsupported IR expression here")),
        );
    }

    fn unsupported_type(&mut self, type_: &Type, span: crate::source::Span) {
        self.diagnostics.push(
            Diagnostic::new(
                DiagnosticCode::WasmError,
                format!("type `{type_:?}` cannot be represented in WebAssembly"),
            )
            .with_label(Label::primary(span, "unsupported type here")),
        );
    }
}

fn runtime_prelude(config: runtime::RuntimeConfig) -> String {
    RuntimePrelude::new(config).into()
}

struct RuntimePrelude {
    wat: String,
}

impl RuntimePrelude {
    fn new(config: runtime::RuntimeConfig) -> Self {
        let mut prelude = Self { wat: String::new() };
        prelude.memory(config);
        prelude.alloc(config);
        prelude
    }

    fn memory(&mut self, config: runtime::RuntimeConfig) {
        writeln!(self.wat, "  (memory (export \"memory\") 1)").expect("write WAT");
        writeln!(
            self.wat,
            "  (global $__heap (mut i32) (i32.const {}))",
            config.heap_start
        )
        .expect("write WAT");
    }

    fn alloc(&mut self, config: runtime::RuntimeConfig) {
        let alignment_mask = config.layout.alignment - 1;

        self.line("  (func $__alloc (param $size i32) (result i32)");
        self.line("    (local $ptr i32)");
        self.line("    global.get $__heap");
        self.line("    local.set $ptr");
        self.line("    global.get $__heap");
        self.line("    local.get $size");
        self.line("    i32.add");
        self.line(format!("    i32.const {alignment_mask}"));
        self.line("    i32.add");
        self.line(format!("    i32.const -{}", config.layout.alignment));
        self.line("    i32.and");
        self.line("    global.set $__heap");
        self.line("    local.get $ptr");
        self.line("  )");
    }

    fn line(&mut self, line: impl AsRef<str>) {
        writeln!(self.wat, "{}", line.as_ref()).expect("write WAT");
    }
}

impl From<RuntimePrelude> for String {
    fn from(prelude: RuntimePrelude) -> Self {
        prelude.wat
    }
}

fn wasm_type(type_: &Type) -> Option<&'static str> {
    match type_ {
        Type::Int => Some("i64"),
        Type::Float => Some("f64"),
        Type::Bool | Type::String => Some("i32"),
        Type::Nil
        | Type::Tuple(_)
        | Type::List(_)
        | Type::Record { .. }
        | Type::Custom { .. }
        | Type::Generic(_)
        | Type::Opaque { .. }
        | Type::Function { .. } => None,
    }
}

fn wat_bytes(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("\\{:02x}", byte)).collect()
}

fn local_name(local: &ir::Local) -> String {
    local.id.0.to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        ast, ir, parse, resolve,
        runtime::ObjectTag,
        source::{SourceFile, SourceFileId},
        types,
    };
    use wasmtime::{Engine, Instance, Module, Store};

    fn compile_wasm(source: &str) -> WasmModule {
        let source = SourceFile::new(SourceFileId(0), source);
        let cst = parse::parse(source).expect("parse source");
        let ast = ast::build(cst).expect("build ast");
        let resolved = resolve::resolve(ast).expect("resolve names");
        let typed = types::check(resolved).expect("type check source");
        let ir = ir::lower(typed).expect("lower source");
        emit(ir).expect("emit wasm")
    }

    #[test]
    fn emits_wat_for_public_scalar_function() {
        let wasm = compile_wasm("pub fn id(x: Int) -> Int { x }");

        insta::assert_snapshot!(wasm.wat, @r#"
(module
  (func $id (export "id") (param $0 i64) (result i64)
    local.get $0
  )
)
"#);
        assert!(!wasm.bytes.is_empty());
    }

    #[test]
    fn emits_wat_with_runtime_for_string_function() {
        let wasm = compile_wasm("pub fn greeting() { \"hello\" }");

        assert!(wasm.wat.contains("(memory (export \"memory\") 1)"));
        assert!(wasm.wat.contains("(func $__alloc"));
        assert!(wasm.wat.contains("(func $greeting (export \"greeting\") (result i32)"));
        assert!(wasm.wat.contains(&format!(
            "i32.const {}",
            runtime::RuntimeConfig::DEFAULT.static_data_start
        )));
    }

    #[test]
    fn runs_exported_function_in_wasmtime() {
        let wasm = compile_wasm(include_str!("../../../fixtures/e2e/public_id.gleam"));
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
}
