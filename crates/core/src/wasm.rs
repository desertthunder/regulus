use std::fmt::Write;

use crate::{
    diagnostic::{Diagnostic, DiagnosticCode, Diagnostics, Label},
    ir::{self, ExpressionKind, Instruction, LiteralKind},
    types::Type,
};

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
    let mut emitter = Emitter { wat: String::from("(module\n"), diagnostics: Vec::new() };

    for function in &module.functions {
        emitter.function(function);
    }

    emitter.wat.push_str(")\n");
    if emitter.diagnostics.is_empty() { Ok(emitter.wat) } else { Err(emitter.diagnostics) }
}

struct Emitter {
    wat: String,
    diagnostics: Diagnostics,
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

        write!(self.wat, "  (func ${}", function.name).expect("write WAT");
        if function.public {
            write!(self.wat, " (export \"{}\")", function.name).expect("write WAT");
        }

        for param in &function.params {
            match wasm_type(&param.type_) {
                Some(type_) => write!(self.wat, " (param ${} {type_})", local_name(param)).expect("write WAT"),
                None => self.unsupported_type(&param.type_, param.span),
            }
        }

        if !return_type.is_empty() {
            write!(self.wat, " (result {return_type})").expect("write WAT");
        }
        self.wat.push('\n');

        for local in function.locals.iter().skip(function.params.len()) {
            match wasm_type(&local.type_) {
                Some(type_) => writeln!(self.wat, "    (local ${} {type_})", local_name(local)).expect("write WAT"),
                None => self.unsupported_type(&local.type_, local.span),
            }
        }

        self.block(&function.body);
        self.wat.push_str("  )\n");
    }

    fn block(&mut self, block: &ir::Block) {
        for instruction in &block.instructions {
            match instruction {
                Instruction::LocalSet { local, value, .. } => {
                    self.expression(value);
                    writeln!(self.wat, "    local.set ${}", local.0).expect("write WAT");
                }
            }
        }
        self.expression(&block.result);
    }

    fn expression(&mut self, expression: &ir::Expression) {
        match &expression.kind {
            ExpressionKind::Literal(literal) => match literal.kind {
                LiteralKind::Int => writeln!(self.wat, "    i64.const {}", literal.source).expect("write WAT"),
                LiteralKind::Float => writeln!(self.wat, "    f64.const {}", literal.source).expect("write WAT"),
                LiteralKind::Bool => writeln!(
                    self.wat,
                    "    i32.const {}",
                    if literal.source == "True" { 1 } else { 0 }
                )
                .expect("write WAT"),
                LiteralKind::Nil => {}
                LiteralKind::String => self.diagnostics.push(
                    Diagnostic::new(DiagnosticCode::WasmError, "strings need a runtime representation")
                        .with_label(Label::primary(expression.span, "string value here")),
                ),
            },
            ExpressionKind::LocalGet(local) => writeln!(self.wat, "    local.get ${}", local.0).expect("write WAT"),
            ExpressionKind::Call { function, arguments } => {
                for argument in arguments {
                    self.expression(argument);
                }
                writeln!(self.wat, "    call ${function}").expect("write WAT");
            }
            ExpressionKind::Branch(_) => self.diagnostics.push(
                Diagnostic::new(DiagnosticCode::WasmError, "branch code generation is not supported")
                    .with_label(Label::primary(expression.span, "branch expression here")),
            ),
        }
    }

    fn unsupported_type(&mut self, type_: &Type, span: crate::source::Span) {
        let message = match type_ {
            Type::String => "strings need a runtime representation".to_string(),
            _ => format!("type `{type_:?}` cannot be represented in WebAssembly"),
        };
        self.diagnostics.push(
            Diagnostic::new(DiagnosticCode::WasmError, message)
                .with_label(Label::primary(span, "unsupported type here")),
        );
    }
}

fn wasm_type(type_: &Type) -> Option<&'static str> {
    match type_ {
        Type::Int => Some("i64"),
        Type::Float => Some("f64"),
        Type::Bool => Some("i32"),
        Type::Nil | Type::String | Type::Function { .. } => None,
    }
}

fn local_name(local: &ir::Local) -> String {
    local.id.0.to_string()
}

#[cfg(test)]
mod tests {
    use wasmtime::{Engine, Instance, Module, Store};

    use crate::{
        ast, ir, parse, resolve,
        source::{SourceFile, SourceFileId},
        types,
    };

    use super::*;

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
    fn rejects_runtime_managed_strings() {
        let source = SourceFile::new(SourceFileId(0), "pub fn main() { \"hello\" }");
        let cst = parse::parse(source).expect("parse source");
        let ast = ast::build(cst).expect("build ast");
        let resolved = resolve::resolve(ast).expect("resolve names");
        let typed = types::check(resolved).expect("type check source");
        let ir = ir::lower(typed).expect("lower source");
        let diagnostics = emit(ir).expect_err("strings should fail");

        assert!(
            diagnostics
                .iter()
                .any(|diagnostic| diagnostic.message.contains("strings need a runtime representation"))
        );
    }
}
