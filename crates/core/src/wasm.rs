use std::{collections::HashMap, fmt::Write};

use crate::diagnostic::{Diagnostic, DiagnosticCode, Diagnostics, Label};
use crate::ir::{self, ExpressionKind, Instruction};
use crate::{ast::LiteralKind, runtime, types::Type};

/// WebAssembly output from the backend.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WasmModule {
    pub wat: String,
    pub bytes: Vec<u8>,
}

pub fn emit(module: &ir::Module) -> Result<WasmModule, Diagnostics> {
    let wat = emit_wat(module)?;
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
        imports: String::new(),
        functions: String::new(),
        diagnostics: Vec::new(),
        data: Vec::new(),
        config: runtime::RuntimeConfig::DEFAULT,
        next_static_offset: runtime::RuntimeConfig::DEFAULT.static_data_start,
        uses_runtime: false,
        function_ids: module
            .functions
            .iter()
            .enumerate()
            .map(|(id, function)| (function.name.clone(), id as u32))
            .collect(),
        function_order: module.functions.iter().map(|function| function.name.clone()).collect(),
        function_signatures: module
            .functions
            .iter()
            .map(|function| {
                (
                    function.name.clone(),
                    (
                        function.params.iter().map(|param| param.type_.clone()).collect(),
                        function.return_type.clone(),
                    ),
                )
            })
            .collect(),
        current_scratch: None,
    };

    for constant in &module.constants {
        emitter.constant(constant);
    }

    for function in &module.functions {
        match &function.abi.boundary {
            ir::CallBoundary::HostImport { .. } | ir::CallBoundary::ModuleImport { .. } => {
                emitter.import_function(function);
            }
            ir::CallBoundary::Internal | ir::CallBoundary::ModuleExport => emitter.function(function),
        }
    }

    if !emitter.diagnostics.is_empty() {
        return Err(emitter.diagnostics);
    }

    let mut wat = String::from("(module\n");
    if emitter.uses_runtime {
        wat.push_str(&runtime_prelude(emitter.config));
    }
    wat.push_str(&emitter.imports);
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
    imports: String,
    functions: String,
    diagnostics: Diagnostics,
    data: Vec<runtime::StaticObject>,
    config: runtime::RuntimeConfig,
    next_static_offset: u32,
    uses_runtime: bool,
    function_ids: HashMap<String, u32>,
    function_order: Vec<String>,
    function_signatures: HashMap<String, (Vec<Type>, Type)>,
    current_scratch: Option<String>,
}

impl Emitter {
    fn constant(&mut self, constant: &ir::Constant) {
        if let ir::ConstantValue::Literal(ir::Literal { kind: LiteralKind::String, source }) = &constant.value {
            self.static_string(source);
        }
    }

    fn import_function(&mut self, function: &ir::Function) {
        let import = match &function.abi.boundary {
            ir::CallBoundary::HostImport { module, name } => (module.as_str(), name.as_str()),
            ir::CallBoundary::ModuleImport { module } => (module.as_str(), function.name.as_str()),
            ir::CallBoundary::Internal | ir::CallBoundary::ModuleExport => return,
        };
        if !self.validate_host_abi(function) {
            return;
        }

        write!(
            self.imports,
            "  (import \"{}\" \"{}\" (func ${}",
            import.0, import.1, function.name
        )
        .expect("write WAT");
        for param in &function.params {
            if let Some(type_) = wasm_type(&param.type_) {
                write!(self.imports, " (param {type_})").expect("write WAT");
            }
        }
        if let Some(return_type) = wasm_type(&function.return_type) {
            write!(self.imports, " (result {return_type})").expect("write WAT");
        }
        self.imports.push_str("))\n");
    }

    fn function(&mut self, function: &ir::Function) {
        if !self.validate_host_abi(function) {
            return;
        }

        let return_type = match wasm_type(&function.return_type) {
            Some(return_type) => return_type,
            None if function.return_type == Type::Nil => "",
            None => {
                self.unsupported_type(&function.return_type, function.span);
                return;
            }
        };

        write!(self.functions, "  (func ${}", function.name).expect("write WAT");
        if matches!(function.abi.boundary, ir::CallBoundary::ModuleExport) {
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

        let previous_scratch = self.current_scratch.clone();
        if block_contains_indirect_call(&function.body) {
            writeln!(self.functions, "    (local $__callee i32)").expect("write WAT");
            self.current_scratch = Some("__callee".into());
        }

        self.block(&function.body);
        self.current_scratch = previous_scratch;
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
                    writeln!(self.functions, "    call $__panic").expect("write WAT");
                    writeln!(self.functions, "    end").expect("write WAT");
                    self.uses_runtime = true;
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
            ExpressionKind::BitArray(bit_array) => {
                let pointer = self.static_bit_array(bit_array);
                writeln!(self.functions, "    i32.const {pointer}").expect("write WAT");
            }
            ExpressionKind::Tuple(items) => {
                let pointer = self.static_tuple(items);
                writeln!(self.functions, "    i32.const {pointer}").expect("write WAT");
            }
            ExpressionKind::List(items) => {
                let pointer = self.static_list(items);
                writeln!(self.functions, "    i32.const {pointer}").expect("write WAT");
            }
            ExpressionKind::Record(record) => {
                let pointer = self.static_record(record);
                writeln!(self.functions, "    i32.const {pointer}").expect("write WAT");
            }
            ExpressionKind::Constructor(constructor) => {
                let pointer = self.static_custom(constructor);
                writeln!(self.functions, "    i32.const {pointer}").expect("write WAT");
            }
            ExpressionKind::FunctionValue(function) => {
                let pointer = self.static_closure(self.function_id(&function.name), &[]);
                writeln!(self.functions, "    i32.const {pointer}").expect("write WAT");
            }
            ExpressionKind::AnonymousFunction(function) => {
                let captures = function
                    .captures
                    .iter()
                    .map(|capture| capture.source.0)
                    .collect::<Vec<_>>();
                let pointer = self.static_closure(0, &captures);
                writeln!(self.functions, "    i32.const {pointer}").expect("write WAT");
            }
            ExpressionKind::ListCons { head, tail } => {
                self.expression(head);
                self.expression(tail);
                writeln!(self.functions, "    call $__list_cons").expect("write WAT");
                self.uses_runtime = true;
            }
            ExpressionKind::BitArrayConcat { left, right } => {
                self.expression(left);
                self.expression(right);
                writeln!(self.functions, "    call $__bit_array_append").expect("write WAT");
                self.uses_runtime = true;
            }
            ExpressionKind::RuntimeEquality { left, right } => {
                self.expression(left);
                self.expression(right);
                writeln!(self.functions, "    call $__equal_ptr").expect("write WAT");
                self.uses_runtime = true;
            }
            ExpressionKind::FieldAccess { record, .. } => self.managed_field_load(record, 0, &expression.type_),
            ExpressionKind::TupleElement { tuple, index } => self.managed_field_load(tuple, *index, &expression.type_),
            ExpressionKind::Failure(_) => {
                writeln!(self.functions, "    call $__panic").expect("write WAT");
                self.uses_runtime = true;
            }
            ExpressionKind::Memory(operation) => self.memory_operation(operation),
            ExpressionKind::IndirectCall(call) => self.indirect_call(call, expression.span),

            ExpressionKind::Pipeline(_)
            | ExpressionKind::Use(_)
            | ExpressionKind::BitStringDeconstruct { .. }
            | ExpressionKind::RecordUpdate { .. }
            | ExpressionKind::ListDeconstruct { .. }
            | ExpressionKind::Compare { .. } => self.unsupported_expression(expression),
        }
    }

    fn indirect_call(&mut self, call: &ir::IndirectCall, span: crate::source::Span) {
        let Some(scratch) = self.current_scratch.clone() else {
            self.diagnostics.push(
                Diagnostic::new(DiagnosticCode::WasmError, "indirect calls need a scratch local")
                    .with_label(Label::primary(span, "indirect call here")),
            );
            return;
        };
        self.expression(&call.callee);
        writeln!(self.functions, "    local.set ${scratch}").expect("write WAT");

        let result_type = call
            .abi
            .return_
            .as_ref()
            .and_then(|value| wasm_type(&value.type_))
            .unwrap_or("");
        self.indirect_call_branch(call, 0, result_type, &scratch);
    }

    fn indirect_call_branch(&mut self, call: &ir::IndirectCall, index: usize, result_type: &str, scratch: &str) {
        let Some(name) = self.function_order.get(index).cloned() else {
            if result_type.is_empty() {
                writeln!(self.functions, "    call $__panic").expect("write WAT");
                self.uses_runtime = true;
            } else {
                writeln!(self.functions, "    unreachable").expect("write WAT");
            }
            return;
        };
        if !self.function_matches_indirect_call(&name, call) {
            self.indirect_call_branch(call, index + 1, result_type, scratch);
            return;
        }
        let id = self.function_id(&name);

        writeln!(self.functions, "    local.get ${scratch}").expect("write WAT");
        writeln!(self.functions, "    i32.const 8").expect("write WAT");
        writeln!(self.functions, "    i32.add").expect("write WAT");
        writeln!(self.functions, "    i32.load").expect("write WAT");
        writeln!(self.functions, "    i32.const {id}").expect("write WAT");
        writeln!(self.functions, "    i32.eq").expect("write WAT");
        if result_type.is_empty() {
            writeln!(self.functions, "    if").expect("write WAT");
        } else {
            writeln!(self.functions, "    if (result {result_type})").expect("write WAT");
        }
        for argument in &call.arguments {
            self.expression(&argument.value);
        }
        writeln!(self.functions, "    call ${name}").expect("write WAT");
        writeln!(self.functions, "    else").expect("write WAT");
        self.indirect_call_branch(call, index + 1, result_type, scratch);
        writeln!(self.functions, "    end").expect("write WAT");
    }

    fn function_id(&self, name: &str) -> u32 {
        self.function_ids.get(name).copied().unwrap_or_default()
    }

    fn function_matches_indirect_call(&self, name: &str, call: &ir::IndirectCall) -> bool {
        let Some((params, return_type)) = self.function_signatures.get(name) else {
            return false;
        };
        params.len() == call.arguments.len()
            && params
                .iter()
                .zip(call.arguments.iter())
                .all(|(param, argument)| param == &argument.value.type_)
            && call.abi.return_.as_ref().map(|value| &value.type_) == Some(return_type)
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
            ir::IrPattern::Tuple(elements) => {
                for (index, element) in elements.iter().enumerate() {
                    self.bind_managed_pattern_field(subject, element, 8 + index * 8);
                }
            }
            ir::IrPattern::List { elements, tail } => {
                if let Some(head) = elements.first() {
                    self.bind_managed_pattern_field(subject, head, 8);
                }
                if let Some(local) = tail {
                    self.expression(subject);
                    writeln!(self.functions, "    i32.const 16").expect("write WAT");
                    writeln!(self.functions, "    i32.add").expect("write WAT");
                    writeln!(self.functions, "    i32.load").expect("write WAT");
                    writeln!(self.functions, "    local.set ${}", local.0).expect("write WAT");
                }
            }
            ir::IrPattern::Constructor { arguments, .. } => {
                for (index, argument) in arguments.iter().enumerate() {
                    self.bind_managed_pattern_field(subject, &argument.pattern, 12 + index * 8);
                }
            }
            ir::IrPattern::Discard | ir::IrPattern::Literal(_) | ir::IrPattern::BitString(_) => {}
        }
    }

    fn bind_managed_pattern_field(&mut self, subject: &ir::Expression, pattern: &ir::IrPattern, offset: usize) {
        match pattern {
            ir::IrPattern::Binding(local) => {
                self.expression(subject);
                writeln!(self.functions, "    i32.const {offset}").expect("write WAT");
                writeln!(self.functions, "    i32.add").expect("write WAT");
                writeln!(self.functions, "    i64.load").expect("write WAT");
                writeln!(self.functions, "    local.set ${}", local.0).expect("write WAT");
            }
            ir::IrPattern::Alias { pattern, local } => {
                self.bind_managed_pattern_field(subject, pattern, offset);
                self.expression(subject);
                writeln!(self.functions, "    local.set ${}", local.0).expect("write WAT");
            }
            _ => {}
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
            ir::IrPattern::Tuple(elements) => {
                self.managed_tag_test(subject, runtime::ObjectTag::Tuple, Some(elements.len() as u32))
            }
            ir::IrPattern::List { elements, .. } if elements.is_empty() => {
                self.expression(subject);
                writeln!(self.functions, "    i32.eqz").expect("write WAT");
            }
            ir::IrPattern::List { .. } => self.managed_tag_test(subject, runtime::ObjectTag::ListCons, None),
            ir::IrPattern::Constructor { name, .. } => {
                self.managed_tag_test(subject, runtime::ObjectTag::Custom, None);
                self.expression(subject);
                writeln!(self.functions, "    i32.const 8").expect("write WAT");
                writeln!(self.functions, "    i32.add").expect("write WAT");
                writeln!(self.functions, "    i32.load").expect("write WAT");
                writeln!(self.functions, "    i32.const {}", constructor_tag(name)).expect("write WAT");
                writeln!(self.functions, "    i32.eq").expect("write WAT");
                writeln!(self.functions, "    i32.and").expect("write WAT");
            }
            ir::IrPattern::BitString(_) => self.managed_tag_test(subject, runtime::ObjectTag::BitArray, None),
        }
    }

    fn managed_tag_test(&mut self, subject: &ir::Expression, tag: runtime::ObjectTag, size: Option<u32>) {
        self.expression(subject);
        writeln!(self.functions, "    i32.load").expect("write WAT");
        writeln!(self.functions, "    i32.const {}", u32::from(tag)).expect("write WAT");
        writeln!(self.functions, "    i32.eq").expect("write WAT");
        if let Some(size) = size {
            self.expression(subject);
            writeln!(self.functions, "    i32.const 4").expect("write WAT");
            writeln!(self.functions, "    i32.add").expect("write WAT");
            writeln!(self.functions, "    i32.load").expect("write WAT");
            writeln!(self.functions, "    i32.const {size}").expect("write WAT");
            writeln!(self.functions, "    i32.eq").expect("write WAT");
            writeln!(self.functions, "    i32.and").expect("write WAT");
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

    fn managed_field_load(&mut self, object: &ir::Expression, index: usize, type_: &Type) {
        self.expression(object);
        writeln!(self.functions, "    i32.const {}", 8 + index * 8).expect("write WAT");
        writeln!(self.functions, "    i32.add").expect("write WAT");
        match type_ {
            Type::Int => writeln!(self.functions, "    i64.load").expect("write WAT"),
            Type::Float => writeln!(self.functions, "    f64.load").expect("write WAT"),
            _ => writeln!(self.functions, "    i32.load").expect("write WAT"),
        }
    }

    fn memory_operation(&mut self, operation: &ir::MemoryOperation) {
        match operation {
            ir::MemoryOperation::Allocate { bytes } => {
                self.expression(bytes);
                writeln!(self.functions, "    call $__alloc").expect("write WAT");
                self.uses_runtime = true;
            }
            ir::MemoryOperation::Load { address, type_ } => {
                self.expression(address);
                match type_ {
                    ir::RepresentationType::Scalar(ir::ScalarRepresentation::I64) => {
                        writeln!(self.functions, "    i64.load").expect("write WAT")
                    }
                    ir::RepresentationType::Scalar(ir::ScalarRepresentation::F64) => {
                        writeln!(self.functions, "    f64.load").expect("write WAT")
                    }
                    _ => writeln!(self.functions, "    i32.load").expect("write WAT"),
                }
            }
            ir::MemoryOperation::Store { address, value } => {
                self.expression(address);
                self.expression(value);
                match value.type_ {
                    Type::Int => writeln!(self.functions, "    i64.store").expect("write WAT"),
                    Type::Float => writeln!(self.functions, "    f64.store").expect("write WAT"),
                    _ => writeln!(self.functions, "    i32.store").expect("write WAT"),
                }
            }
        }
    }

    fn static_value(&mut self, expression: &ir::Expression) -> Option<u64> {
        match &expression.kind {
            ExpressionKind::Literal(literal) => match literal.kind {
                LiteralKind::Int => literal.source.parse::<u64>().ok(),
                LiteralKind::Bool => Some(if literal.source == "True" { 1 } else { 0 }),
                LiteralKind::Nil => Some(0),
                LiteralKind::String => Some(self.static_string(&literal.source) as u64),
                LiteralKind::Float => None,
            },
            ExpressionKind::BitArray(bit_array) => Some(self.static_bit_array(bit_array) as u64),
            ExpressionKind::Tuple(items) => Some(self.static_tuple(items) as u64),
            ExpressionKind::List(items) => Some(self.static_list(items) as u64),
            ExpressionKind::Record(record) => Some(self.static_record(record) as u64),
            ExpressionKind::Constructor(constructor) => Some(self.static_custom(constructor) as u64),
            ExpressionKind::FunctionValue(_) => Some(self.static_closure(0, &[]) as u64),
            _ => None,
        }
    }

    fn static_tuple(&mut self, items: &[ir::Expression]) -> u32 {
        let fields = items
            .iter()
            .filter_map(|item| self.static_value(item))
            .collect::<Vec<_>>();
        self.push_static(runtime::tuple_object(self.config, self.next_static_offset, &fields))
    }

    fn static_list(&mut self, items: &[ir::Expression]) -> u32 {
        let mut tail = 0;
        for item in items.iter().rev() {
            let head = self.static_value(item).unwrap_or(0);
            tail = self.push_static(runtime::list_cons_object(
                self.config,
                self.next_static_offset,
                head,
                tail,
            ));
        }
        tail
    }

    fn static_record(&mut self, record: &ir::RecordValue) -> u32 {
        let fields = record
            .fields
            .iter()
            .filter_map(|field| self.static_value(&field.value))
            .collect::<Vec<_>>();
        self.push_static(runtime::record_object(self.config, self.next_static_offset, &fields))
    }

    fn static_custom(&mut self, constructor: &ir::ConstructorValue) -> u32 {
        let fields = constructor
            .arguments
            .iter()
            .filter_map(|field| self.static_value(field))
            .collect::<Vec<_>>();
        self.push_static(runtime::custom_object(
            self.config,
            self.next_static_offset,
            constructor_tag(&constructor.name),
            &fields,
        ))
    }

    fn static_closure(&mut self, function_id: u32, captures: &[u32]) -> u32 {
        self.push_static(runtime::closure_object(
            self.config,
            self.next_static_offset,
            function_id,
            captures,
        ))
    }

    fn push_static(&mut self, object: runtime::StaticObject) -> u32 {
        self.uses_runtime = true;
        let pointer = object.offset;
        self.next_static_offset = self.config.layout.align_to(object.offset + object.bytes.len() as u32);
        self.data.push(object);
        pointer
    }

    fn static_string(&mut self, source: &str) -> u32 {
        let string = source.trim_matches('"');
        self.push_static(runtime::string_object(self.config, self.next_static_offset, string))
    }

    fn static_bit_array(&mut self, bit_array: &ir::BitArrayLiteral) -> u32 {
        let bytes = bit_array_bytes(bit_array);
        self.push_static(runtime::bit_array_object(
            self.config,
            self.next_static_offset,
            &bytes,
            bit_array.bit_len,
        ))
    }

    fn validate_host_abi(&mut self, function: &ir::Function) -> bool {
        match function.abi.boundary {
            ir::CallBoundary::Internal => return true,
            ir::CallBoundary::ModuleExport
            | ir::CallBoundary::ModuleImport { .. }
            | ir::CallBoundary::HostImport { .. } => {}
        }

        let mut supported = true;
        for param in &function.params {
            if !is_supported_host_abi_type(&param.type_) {
                self.unsupported_abi_type(&param.type_, param.span, &function.name);
                supported = false;
            }
        }
        if !matches!(function.return_type, Type::Nil) && !is_supported_host_abi_type(&function.return_type) {
            self.unsupported_abi_type(&function.return_type, function.span, &function.name);
            supported = false;
        }
        supported
    }

    fn unsupported_abi_type(&mut self, type_: &Type, span: crate::source::Span, function: &str) {
        self.diagnostics.push(
            Diagnostic::new(
                DiagnosticCode::WasmError,
                format!("function `{function}` has unsupported host ABI type `{type_:?}`"),
            )
            .with_label(Label::primary(span, "unsupported ABI type here")),
        );
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
        prelude.helpers();
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

    fn helpers(&mut self) {
        self.line("  (func $__panic");
        self.line("    unreachable");
        self.line("  )");
        self.line("  (func $__equal_ptr (param $left i32) (param $right i32) (result i32)");
        self.line("    local.get $left");
        self.line("    local.get $right");
        self.line("    i32.eq");
        self.line("  )");
        self.line("  (func $__list_cons (param $head i64) (param $tail i32) (result i32)");
        self.line("    (local $ptr i32)");
        self.line("    i32.const 24");
        self.line("    call $__alloc");
        self.line("    local.set $ptr");
        self.line("    local.get $ptr");
        self.line("    i32.const 2");
        self.line("    i32.store");
        self.line("    local.get $ptr");
        self.line("    i32.const 4");
        self.line("    i32.add");
        self.line("    i32.const 2");
        self.line("    i32.store");
        self.line("    local.get $ptr");
        self.line("    i32.const 8");
        self.line("    i32.add");
        self.line("    local.get $head");
        self.line("    i64.store");
        self.line("    local.get $ptr");
        self.line("    i32.const 16");
        self.line("    i32.add");
        self.line("    local.get $tail");
        self.line("    i32.store");
        self.line("    local.get $ptr");
        self.line("  )");
        self.line("  (func $__bit_array_append (param $left i32) (param $right i32) (result i32)");
        self.line("    local.get $left");
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

fn block_contains_indirect_call(block: &ir::Block) -> bool {
    block.instructions.iter().any(|instruction| match instruction {
        Instruction::Evaluate { expression, .. }
        | Instruction::LocalSet { value: expression, .. }
        | Instruction::AssertMatch { value: expression, .. } => expression_contains_indirect_call(expression),
    }) || expression_contains_indirect_call(&block.result)
}

fn expression_contains_indirect_call(expression: &ir::Expression) -> bool {
    match &expression.kind {
        ExpressionKind::IndirectCall(_) => true,
        ExpressionKind::DirectCall(call) => call
            .arguments
            .iter()
            .any(|argument| expression_contains_indirect_call(&argument.value)),
        ExpressionKind::Branch(branch) => {
            branch.subjects.iter().any(expression_contains_indirect_call)
                || branch.clauses.iter().any(|clause| {
                    clause.guard.as_ref().is_some_and(expression_contains_indirect_call)
                        || expression_contains_indirect_call(&clause.body)
                })
        }
        ExpressionKind::Tuple(items) | ExpressionKind::List(items) => {
            items.iter().any(expression_contains_indirect_call)
        }
        ExpressionKind::BitArrayConcat { left, right }
        | ExpressionKind::Compare { left, right, .. }
        | ExpressionKind::RuntimeEquality { left, right } => {
            expression_contains_indirect_call(left) || expression_contains_indirect_call(right)
        }
        ExpressionKind::BitStringDeconstruct { bit_array, .. }
        | ExpressionKind::FieldAccess { record: bit_array, .. }
        | ExpressionKind::TupleElement { tuple: bit_array, .. }
        | ExpressionKind::ListDeconstruct { list: bit_array, .. } => expression_contains_indirect_call(bit_array),
        ExpressionKind::Record(record) => record
            .fields
            .iter()
            .any(|field| expression_contains_indirect_call(&field.value)),
        ExpressionKind::Constructor(constructor) => constructor.arguments.iter().any(expression_contains_indirect_call),
        ExpressionKind::RecordUpdate { record, updates } => {
            expression_contains_indirect_call(record)
                || updates
                    .iter()
                    .any(|field| expression_contains_indirect_call(&field.value))
        }
        ExpressionKind::ListCons { head, tail } => {
            expression_contains_indirect_call(head) || expression_contains_indirect_call(tail)
        }
        ExpressionKind::Memory(operation) => match operation {
            ir::MemoryOperation::Allocate { bytes } => expression_contains_indirect_call(bytes),
            ir::MemoryOperation::Load { address, .. } => expression_contains_indirect_call(address),
            ir::MemoryOperation::Store { address, value } => {
                expression_contains_indirect_call(address) || expression_contains_indirect_call(value)
            }
        },
        ExpressionKind::Literal(_)
        | ExpressionKind::LocalGet(_)
        | ExpressionKind::FunctionValue(_)
        | ExpressionKind::AnonymousFunction(_)
        | ExpressionKind::Pipeline(_)
        | ExpressionKind::Use(_)
        | ExpressionKind::BitArray(_)
        | ExpressionKind::Failure(_) => false,
    }
}

fn constructor_tag(name: &str) -> u32 {
    name.bytes().fold(0x811c_9dc5, |hash, byte| {
        hash.wrapping_mul(0x0100_0193) ^ u32::from(byte)
    })
}

fn bit_array_bytes(bit_array: &ir::BitArrayLiteral) -> Vec<u8> {
    let mut bytes = vec![0; runtime::bit_array_payload_len(bit_array.bit_len) as usize];
    let mut offset = 0;
    for segment in &bit_array.segments {
        for bit_index in 0..segment.bit_size {
            let source_shift = segment.bit_size - bit_index - 1;
            let bit = if source_shift < u64::BITS { (segment.value >> source_shift) & 1 } else { 0 };
            if bit == 1 {
                let byte = &mut bytes[(offset / 8) as usize];
                let target_shift = 7 - offset % 8;
                *byte |= 1 << target_shift;
            }
            offset += 1;
        }
    }
    bytes
}

fn is_supported_host_abi_type(type_: &Type) -> bool {
    matches!(
        type_,
        Type::Int
            | Type::Float
            | Type::Bool
            | Type::String
            | Type::BitArray
            | Type::Tuple(_)
            | Type::List(_)
            | Type::Record { .. }
            | Type::Custom { .. }
            | Type::Function { .. }
    )
}

fn wasm_type(type_: &Type) -> Option<&'static str> {
    match type_ {
        Type::Int => Some("i64"),
        Type::Float => Some("f64"),
        Type::Bool | Type::String | Type::BitArray => Some("i32"),
        Type::Tuple(_)
        | Type::List(_)
        | Type::Record { .. }
        | Type::Custom { .. }
        | Type::Opaque { .. }
        | Type::Function { .. } => Some("i32"),
        Type::Nil | Type::Generic(_) => None,
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
        source::{SourceFile, SourceFileId, Span},
        types,
    };
    use wasmtime::{Engine, Instance, Linker, Module, Store};

    fn compile_wasm(source: &str) -> WasmModule {
        let source = SourceFile::new(SourceFileId(0), source);
        let cst = parse::parse(source).expect("parse source");
        let ast = ast::build(&cst).expect("build ast");
        let resolved = resolve::resolve(ast).expect("resolve names");
        let typed = types::check(resolved).expect("type check source");
        let ir = ir::lower(typed).expect("lower source");
        emit(&ir).expect("emit wasm")
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
            imports: Vec::new(),
            declarations: Vec::new(),
            constants: Vec::new(),
            init: ir::ModuleInit::default(),
            references: Vec::new(),
            exports: Vec::new(),
            functions,
        }
    }

    fn host_import_module(span: Span) -> ir::Module {
        let import_abi = ir::CallAbi {
            params: vec![ir::AbiValue::from(&Type::Int)],
            return_: Some(ir::AbiValue::from(&Type::Int)),
            boundary: ir::CallBoundary::HostImport { module: "env".into(), name: "inc".into() },
        };
        let imported = ir::Function {
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
    fn emits_host_import_before_exported_function() {
        let module = host_import_module(Span::new(SourceFileId(0), 0, 0));

        insta::assert_snapshot!(emit_wat(&module).expect("emit wat"), @r#"
(module
  (import "env" "inc" (func $host_inc (param i64) (result i64)))
  (func $main (export "main") (result i64)
    i64.const 41
    call $host_inc
  )
)
"#);
    }

    #[test]
    fn runs_host_import_in_wasmtime() {
        let wasm = emit(&host_import_module(Span::new(SourceFileId(0), 0, 0))).expect("emit wasm");
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
    fn rejects_unsupported_export_abi_before_wat_assembly() {
        let span = Span::new(SourceFileId(0), 0, 0);
        let generic = Type::Generic("value".into());
        let function = ir::Function {
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
                result: Box::new(ir::Expression {
                    type_: generic,
                    span,
                    kind: ExpressionKind::LocalGet(ir::LocalId(0)),
                }),
                span,
            },
            span,
        };
        let diagnostics = emit_wat(&ir_module(vec![function], span)).expect_err("unsupported ABI");

        assert!(
            diagnostics
                .iter()
                .any(|diagnostic| diagnostic.message.contains("unsupported host ABI"))
        );
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
