//! Incremental IR-to-structured-Wasm code generation.

use std::collections::HashMap;

use super::builder::{
    BlockType, DataSegment, Export, ExportDesc, Function, FunctionId, FunctionType, Import, ImportDesc, Instruction,
    Local, LocalId, Memory, MemoryArg, MemoryId, Module, TypeId, ValueType,
};
use super::{EmitOptions, WasmTarget};
use crate::ast::LiteralKind;
use crate::diagnostic::{Diagnostic, DiagnosticCode, Diagnostics, Label};
use crate::ir::{self, ExpressionKind};
use crate::{ClosureConstants, runtime, stdlib::STDLIB_IO_HOST_MODULE, types::Type};

pub(super) fn emit(module: &ir::Module, options: EmitOptions) -> Result<Option<Module>, Diagnostics> {
    let emitter = StructuredEmitter::new(module, options);
    match emitter.module(module) {
        Ok(module) => Ok(Some(module)),
        Err(StructuredError::Unsupported) => Ok(None),
        Err(StructuredError::Diagnostics(diagnostics)) => Err(diagnostics),
    }
}

struct StructuredEmitter<'a> {
    source: &'a ir::Module,
    module: Module,
    signatures: HashMap<String, FunctionSignature>,
    function_ids: HashMap<String, FunctionId>,
    local_indices: HashMap<ir::LocalId, LocalId>,
    debug_imports: HashMap<DebugImport, FunctionId>,
    debug_locals: HashMap<DebugImport, LocalId>,
    scratch_local: Option<LocalId>,
    options: EmitOptions,
    config: runtime::RuntimeConfig,
    next_static_offset: u32,
    memory: Option<MemoryId>,
    imported_functions: u32,
}

#[derive(Debug, Clone)]
struct FunctionSignature {
    type_id: TypeId,
    type_: FunctionType,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
enum DebugImport {
    Bool,
    Value,
    I64,
    F64,
}

impl DebugImport {
    fn name(self) -> &'static str {
        match self {
            Self::Bool => "debug_bool",
            Self::Value => "debug_value",
            Self::I64 => "debug_i64",
            Self::F64 => "debug_f64",
        }
    }

    fn value_type(self) -> ValueType {
        match self {
            Self::Bool | Self::Value => ValueType::I32,
            Self::I64 => ValueType::I64,
            Self::F64 => ValueType::F64,
        }
    }
}

#[derive(Debug)]
enum StructuredError {
    Unsupported,
    Diagnostics(Diagnostics),
}

type StructuredResult<T> = Result<T, StructuredError>;

impl<'a> StructuredEmitter<'a> {
    fn new(source: &'a ir::Module, options: EmitOptions) -> Self {
        Self {
            source,
            module: Module::new(),
            signatures: HashMap::new(),
            function_ids: HashMap::new(),
            local_indices: HashMap::new(),
            debug_imports: HashMap::new(),
            debug_locals: HashMap::new(),
            scratch_local: None,
            options,
            config: runtime::RuntimeConfig::DEFAULT,
            next_static_offset: runtime::RuntimeConfig::DEFAULT.static_data_start,
            memory: None,
            imported_functions: 0,
        }
    }

    fn module(mut self, source: &ir::Module) -> StructuredResult<Module> {
        self.module.source_span = source.functions.first().map(|function| function.span);
        for function in &source.functions {
            if matches!(function.abi.boundary, ir::CallBoundary::ModuleExport) && function.return_type == Type::String {
                return Err(StructuredError::Unsupported);
            }
            let signature = self.function_signature(function)?;
            self.signatures.insert(function.name.clone(), signature);
        }

        for function in &source.functions {
            if matches!(
                function.abi.boundary,
                ir::CallBoundary::HostImport { .. } | ir::CallBoundary::ModuleImport { .. }
            ) {
                self.import_function(function)?;
            }
        }

        for function in &source.functions {
            for import in needed_debug_imports(function) {
                if self.options.target == WasmTarget::Wasi {
                    return Err(StructuredError::Diagnostics(vec![
                        Diagnostic::new(
                            DiagnosticCode::WasmError,
                            format!(
                                "stdlib host call `gleam/io.debug` is not supported for target `{}`",
                                self.options.target.name()
                            ),
                        )
                        .with_label(Label::primary(function.span, "unsupported host call for this target"))
                        .with_note("supported targets for `gleam/io` host calls are `wasmtime` and `browser`"),
                    ]));
                }
                self.ensure_debug_import(import);
            }
        }

        for function in &source.functions {
            if matches!(
                function.abi.boundary,
                ir::CallBoundary::Internal | ir::CallBoundary::ModuleExport
            ) {
                let name = function.name.clone();
                let function = self.function(function)?;
                let id = self.module.push_function(function);
                self.function_ids.insert(name, id);
            }
        }

        for constant in &source.constants {
            self.constant(constant)?;
        }

        for function in &source.functions {
            if matches!(function.abi.boundary, ir::CallBoundary::ModuleExport) {
                let function_id = self.function_id_structured(&function.name);
                self.module
                    .exports
                    .push(Export { name: function.name.clone(), desc: ExportDesc::Function(function_id) });
            }
        }

        if let Some(memory) = self.memory {
            self.module
                .exports
                .push(Export { name: "memory".into(), desc: ExportDesc::Memory(memory) });
        }

        Ok(self.module)
    }

    fn concrete_import(&self, function: &ir::Function) -> StructuredResult<Option<(String, String)>> {
        match &function.abi.boundary {
            ir::CallBoundary::HostImport { module, name } if module == STDLIB_IO_HOST_MODULE => {
                Ok(Some(self.stdlib_io_import(name, function.span)?))
            }
            ir::CallBoundary::HostImport { module, name } if module == self.options.target.host_module() => {
                Ok(Some((module.clone(), name.clone())))
            }
            ir::CallBoundary::HostImport { module, .. } => Err(StructuredError::Diagnostics(vec![
                Diagnostic::new(
                    DiagnosticCode::WasmError,
                    format!(
                        "function `{}` imports host module `{module}`, but target {:?} expects `{}`",
                        function.name,
                        self.options.target,
                        self.options.target.host_module()
                    ),
                )
                .with_label(Label::primary(function.span, "unsupported target import here")),
            ])),
            ir::CallBoundary::ModuleImport { module } => Ok(Some((module.clone(), function.name.clone()))),
            ir::CallBoundary::Internal | ir::CallBoundary::ModuleExport => Ok(None),
        }
    }

    fn stdlib_io_import(&self, name: &str, span: crate::source::Span) -> StructuredResult<(String, String)> {
        match self.options.target {
            WasmTarget::Wasmtime => Ok(("env".into(), name.into())),
            WasmTarget::Browser => Ok(("browser".into(), name.into())),
            WasmTarget::Wasi => Err(StructuredError::Diagnostics(vec![
                Diagnostic::new(
                    DiagnosticCode::WasmError,
                    format!(
                        "stdlib host call `gleam/io.{name}` is not supported for target `{}`",
                        self.options.target.name()
                    ),
                )
                .with_label(Label::primary(span, "unsupported host call for this target"))
                .with_note("supported targets for `gleam/io` host calls are `wasmtime` and `browser`"),
            ])),
        }
    }

    fn import_function(&mut self, function: &ir::Function) -> StructuredResult<()> {
        let Some((module, name)) = self.concrete_import(function)? else {
            return Ok(());
        };
        let signature = self
            .signatures
            .get(&function.name)
            .expect("function signature should be registered")
            .clone();
        self.module
            .push_import(Import { module, name, desc: ImportDesc::Function(signature.type_id) });
        let id = FunctionId(self.imported_functions);
        self.imported_functions += 1;
        self.function_ids.insert(function.name.clone(), id);
        Ok(())
    }

    fn function_signature(&mut self, function: &ir::Function) -> StructuredResult<FunctionSignature> {
        let params = function
            .params
            .iter()
            .map(|param| value_type(&param.type_, param.span))
            .collect::<StructuredResult<Vec<_>>>()?;
        let results = result_types(&function.return_type, function.span)?;
        let type_ = FunctionType::new(params, results);
        let type_id = self.module.push_type(type_.clone());
        Ok(FunctionSignature { type_id, type_ })
    }

    fn function(&mut self, function: &ir::Function) -> StructuredResult<Function> {
        let signature = self
            .signatures
            .get(&function.name)
            .expect("function signature should be registered")
            .clone();
        self.local_indices.clear();
        self.debug_locals.clear();
        self.scratch_local = None;

        let mut structured = Function::new(signature.type_id);
        structured.name = Some(function.name.clone());

        for (index, param) in function.params.iter().enumerate() {
            let type_ = value_type(&param.type_, param.span)?;
            self.local_indices.insert(param.id, LocalId(index as u32));
            structured.params.push(Local { name: Some(param.name.clone()), type_ });
        }

        for local in function.locals.iter().skip(function.params.len()) {
            let type_ = value_type(&local.type_, local.span)?;
            let index = structured.params.len() + structured.locals.len();
            self.local_indices.insert(local.id, LocalId(index as u32));
            structured.locals.push(Local { name: Some(local.name.clone()), type_ });
        }

        if needs_scratch(function) {
            let id = LocalId((structured.params.len() + structured.locals.len()) as u32);
            structured
                .locals
                .push(Local { name: Some("__scratch".into()), type_: ValueType::I32 });
            self.scratch_local = Some(id);
        }
        for debug_import in needed_debug_imports(function) {
            let id = LocalId((structured.params.len() + structured.locals.len()) as u32);
            structured
                .locals
                .push(Local { name: Some(format!("__{}", debug_import.name())), type_: debug_import.value_type() });
            self.debug_locals.insert(debug_import, id);
        }
        structured.body = self.block(&function.body)?;
        Ok(structured)
    }

    fn block(&mut self, block: &ir::Block) -> StructuredResult<Vec<Instruction>> {
        let mut instructions = Vec::new();
        for instruction in &block.instructions {
            match instruction {
                ir::Instruction::Evaluate { expression, .. } => {
                    self.expression(expression, &mut instructions)?;
                    if let Some(type_) = maybe_value_type(&expression.type_) {
                        instructions.push(Instruction::Drop(type_));
                    }
                }
                ir::Instruction::LocalSet { local, value, .. } => {
                    self.expression(value, &mut instructions)?;
                    let local = self.local(*local, value.span)?;
                    let type_ = value_type(&value.type_, value.span)?;
                    instructions.push(Instruction::LocalSet { local, type_ });
                }
                ir::Instruction::AssertMatch { value, pattern, .. } => {
                    self.pattern_test(value, pattern, &mut instructions)?;
                    instructions.push(Instruction::If {
                        type_: BlockType::empty(),
                        then_body: Vec::new(),
                        else_body: vec![Instruction::Unreachable],
                    });
                }
            }
        }
        self.expression(&block.result, &mut instructions)?;
        Ok(instructions)
    }

    fn expression(&mut self, expression: &ir::Expression, out: &mut Vec<Instruction>) -> StructuredResult<()> {
        match &expression.kind {
            ExpressionKind::Literal(literal) => self.literal(literal, out),
            ExpressionKind::LocalGet(local) => {
                let type_ = value_type(&expression.type_, expression.span)?;
                out.push(Instruction::LocalGet { local: self.local(*local, expression.span)?, type_ });
                Ok(())
            }
            ExpressionKind::DirectCall(call) => self.direct_call(call, out),
            ExpressionKind::Compare { op, left, right } => self.compare(*op, left, right, out),
            ExpressionKind::RuntimeEquality { left, right } => self.runtime_equality(left, right, out),
            ExpressionKind::Branch(branch) => self.branch(branch, &expression.type_, expression.span, out),
            ExpressionKind::Pipeline(pipeline) => self.pipeline(pipeline, out),
            ExpressionKind::Tuple(items) => {
                let fields = self.static_values(items)?;
                let object = runtime::tuple_object(self.config, self.next_static_offset, &fields);
                self.static_pointer(object, out)
            }
            ExpressionKind::List(items) => {
                let pointer = self.static_list(items)?;
                out.push(Instruction::I32Const(pointer as i32));
                Ok(())
            }
            ExpressionKind::Record(record) => {
                let fields = self.static_values(record.fields.iter().map(|field| &field.value))?;
                let object = runtime::record_object(self.config, self.next_static_offset, &fields);
                self.static_pointer(object, out)
            }
            ExpressionKind::Constructor(constructor) => {
                let fields = self.static_values(&constructor.arguments)?;
                let object = runtime::custom_object(
                    self.config,
                    self.next_static_offset,
                    super::constructor_tag(&constructor.name),
                    &fields,
                );
                self.static_pointer(object, out)
            }
            ExpressionKind::FunctionValue(function) => self.static_pointer(
                runtime::closure_object(
                    self.config,
                    self.next_static_offset,
                    self.function_id(&function.name),
                    &[],
                ),
                out,
            ),
            ExpressionKind::FieldAccess { record, .. } => self.managed_field_load(record, 0, &expression.type_, out),
            ExpressionKind::TupleElement { tuple, index } => {
                self.managed_field_load(tuple, *index, &expression.type_, out)
            }
            ExpressionKind::Memory(operation) => self.memory_operation(operation, out),
            ExpressionKind::IndirectCall(call) => self.indirect_call(call, out),
            ExpressionKind::BitArray(bit_array) => {
                let bytes = bit_array.bytes();
                self.static_pointer(
                    runtime::bit_array_object(self.config, self.next_static_offset, &bytes, bit_array.bit_len),
                    out,
                )
            }
            _ => Err(StructuredError::Unsupported),
        }
    }

    fn literal(&mut self, literal: &ir::Literal, out: &mut Vec<Instruction>) -> StructuredResult<()> {
        match literal.kind {
            LiteralKind::Int => {
                let value = literal
                    .source
                    .parse::<i64>()
                    .map_err(|_| StructuredError::Unsupported)?;
                out.push(Instruction::I64Const(value));
            }
            LiteralKind::Float => {
                let value = literal
                    .source
                    .parse::<f64>()
                    .map_err(|_| StructuredError::Unsupported)?;
                out.push(Instruction::F64Const(value.to_bits()));
            }
            LiteralKind::Bool => out.push(Instruction::I32Const(if literal.source == "True" { 1 } else { 0 })),
            LiteralKind::Nil => {}
            LiteralKind::String => {
                let string = literal.source.trim_matches('"');
                return self.static_pointer(
                    runtime::string_object(self.config, self.next_static_offset, string),
                    out,
                );
            }
        }
        Ok(())
    }

    fn direct_call(&mut self, call: &ir::DirectCall, out: &mut Vec<Instruction>) -> StructuredResult<()> {
        match call.function.as_str() {
            "__op_add" | "__op_subtract" | "__op_multiply" | "__op_divide" | "__op_remainder" => {
                self.binary_arguments(call, out)?;
                out.push(match call.function.as_str() {
                    "__op_add" => Instruction::I64Add,
                    "__op_subtract" => Instruction::I64Sub,
                    "__op_multiply" => Instruction::I64Mul,
                    "__op_divide" => Instruction::I64DivS,
                    "__op_remainder" => Instruction::I64RemS,
                    _ => unreachable!(),
                });
            }
            "__op_float_add" | "__op_float_subtract" | "__op_float_multiply" | "__op_float_divide" => {
                self.binary_arguments(call, out)?;
                out.push(match call.function.as_str() {
                    "__op_float_add" => Instruction::F64Add,
                    "__op_float_subtract" => Instruction::F64Sub,
                    "__op_float_multiply" => Instruction::F64Mul,
                    "__op_float_divide" => Instruction::F64Div,
                    _ => unreachable!(),
                });
            }
            "__op_not" | "__stdlib_gleam_bool_negate" => {
                self.expression(&call.arguments[0].value, out)?;
                out.push(Instruction::I32Eqz);
            }
            "__op_negate" => {
                out.push(Instruction::I64Const(0));
                self.expression(&call.arguments[0].value, out)?;
                out.push(Instruction::I64Sub);
            }
            "__op_and" => self.short_circuit_bool(call, false, out)?,
            "__op_or" => self.short_circuit_bool(call, true, out)?,
            "__stdlib_gleam_float_negate" => {
                out.push(Instruction::F64Const((-0.0f64).to_bits()));
                self.expression(&call.arguments[0].value, out)?;
                out.push(Instruction::F64Sub);
            }
            "__stdlib_gleam_float_max" | "__stdlib_gleam_float_min" => {
                // Keep these in the legacy path until structured locals can
                // preserve each argument without evaluating it twice.
                return Err(StructuredError::Unsupported);
            }
            "__stdlib_gleam_function_identity" | "__stdlib_gleam_function_constant" => {
                self.expression(&call.arguments[0].value, out)?;
            }
            "__stdlib_gleam_io_debug" => self.stdlib_io_debug(call, out)?,
            _ => {
                let id = self
                    .function_ids
                    .get(&call.function)
                    .copied()
                    .ok_or(StructuredError::Unsupported)?;
                let signature = self
                    .signatures
                    .get(&call.function)
                    .ok_or(StructuredError::Unsupported)?
                    .clone();
                for argument in &call.arguments {
                    self.expression(&argument.value, out)?;
                }
                out.push(Instruction::Call { function: id, type_: signature.type_ });
            }
        }
        Ok(())
    }

    fn binary_arguments(&mut self, call: &ir::DirectCall, out: &mut Vec<Instruction>) -> StructuredResult<()> {
        self.expression(&call.arguments[0].value, out)?;
        self.expression(&call.arguments[1].value, out)
    }

    fn stdlib_io_debug(&mut self, call: &ir::DirectCall, out: &mut Vec<Instruction>) -> StructuredResult<()> {
        let value = &call.arguments[0].value;
        let import = match value.type_ {
            Type::Int => DebugImport::I64,
            Type::Float => DebugImport::F64,
            Type::Bool => DebugImport::Bool,
            Type::String
            | Type::BitArray
            | Type::Tuple(_)
            | Type::List(_)
            | Type::Record { .. }
            | Type::Custom { .. }
            | Type::Opaque { .. }
            | Type::Function { .. } => DebugImport::Value,
            Type::Nil => return Ok(()),
            Type::Generic(_) => return Err(StructuredError::Unsupported),
        };
        if self.options.target == WasmTarget::Wasi {
            return Err(StructuredError::Diagnostics(vec![
                Diagnostic::new(
                    DiagnosticCode::WasmError,
                    format!(
                        "stdlib host call `gleam/io.debug` is not supported for target `{}`",
                        self.options.target.name()
                    ),
                )
                .with_label(Label::primary(value.span, "unsupported host call for this target"))
                .with_note("supported targets for `gleam/io` host calls are `wasmtime` and `browser`"),
            ]));
        }
        let local = self
            .debug_locals
            .get(&import)
            .copied()
            .ok_or(StructuredError::Unsupported)?;
        let function = self.ensure_debug_import(import);
        self.expression(value, out)?;
        out.push(Instruction::LocalTee { local, type_: import.value_type() });
        out.push(Instruction::Call { function, type_: FunctionType::new([import.value_type()], []) });
        out.push(Instruction::LocalGet { local, type_: import.value_type() });
        Ok(())
    }

    fn ensure_debug_import(&mut self, import: DebugImport) -> FunctionId {
        if let Some(id) = self.debug_imports.get(&import).copied() {
            return id;
        }
        let type_ = FunctionType::new([import.value_type()], []);
        let type_id = self.module.push_type(type_);
        self.module.push_import(Import {
            module: self.options.target.host_module().into(),
            name: import.name().into(),
            desc: ImportDesc::Function(type_id),
        });
        let id = FunctionId(self.imported_functions);
        self.imported_functions += 1;
        self.debug_imports.insert(import, id);
        id
    }

    fn short_circuit_bool(
        &mut self, call: &ir::DirectCall, is_or: bool, out: &mut Vec<Instruction>,
    ) -> StructuredResult<()> {
        self.expression(&call.arguments[0].value, out)?;
        let mut then_body = Vec::new();
        let mut else_body = Vec::new();
        if is_or {
            then_body.push(Instruction::I32Const(1));
            self.expression(&call.arguments[1].value, &mut else_body)?;
        } else {
            self.expression(&call.arguments[1].value, &mut then_body)?;
            else_body.push(Instruction::I32Const(0));
        }
        out.push(Instruction::If { type_: BlockType::new([], [ValueType::I32]), then_body, else_body });
        Ok(())
    }

    fn compare(
        &mut self, op: ir::ComparisonOp, left: &ir::Expression, right: &ir::Expression, out: &mut Vec<Instruction>,
    ) -> StructuredResult<()> {
        match op {
            ir::ComparisonOp::Equal | ir::ComparisonOp::NotEqual => self.runtime_equality(left, right, out)?,
            ir::ComparisonOp::Less
            | ir::ComparisonOp::LessEqual
            | ir::ComparisonOp::Greater
            | ir::ComparisonOp::GreaterEqual => {
                self.expression(left, out)?;
                self.expression(right, out)?;
                out.push(match (&left.type_, op) {
                    (Type::Int, ir::ComparisonOp::Less) => Instruction::I64LtS,
                    (Type::Int, ir::ComparisonOp::LessEqual) => Instruction::I64LeS,
                    (Type::Int, ir::ComparisonOp::Greater) => Instruction::I64GtS,
                    (Type::Int, ir::ComparisonOp::GreaterEqual) => Instruction::I64GeS,
                    (Type::Float, ir::ComparisonOp::Less) => Instruction::F64Lt,
                    (Type::Float, ir::ComparisonOp::LessEqual) => Instruction::F64Le,
                    (Type::Float, ir::ComparisonOp::Greater) => Instruction::F64Gt,
                    (Type::Float, ir::ComparisonOp::GreaterEqual) => Instruction::F64Ge,
                    (Type::Bool, ir::ComparisonOp::Less) => Instruction::I32LtS,
                    (Type::Bool, ir::ComparisonOp::LessEqual) => Instruction::I32LeS,
                    (Type::Bool, ir::ComparisonOp::Greater) => Instruction::I32GtS,
                    (Type::Bool, ir::ComparisonOp::GreaterEqual) => Instruction::I32GeS,
                    _ => return Err(StructuredError::Unsupported),
                });
            }
        }
        if matches!(op, ir::ComparisonOp::NotEqual) {
            out.push(Instruction::I32Eqz);
        }
        Ok(())
    }

    fn runtime_equality(
        &mut self, left: &ir::Expression, right: &ir::Expression, out: &mut Vec<Instruction>,
    ) -> StructuredResult<()> {
        self.expression(left, out)?;
        self.expression(right, out)?;
        out.push(match left.type_ {
            Type::Int => Instruction::I64Eq,
            Type::Float => Instruction::F64Eq,
            Type::Bool => Instruction::I32Eq,
            Type::Nil => Instruction::I32Const(1),
            _ => return Err(StructuredError::Unsupported),
        });
        Ok(())
    }

    fn pipeline(&mut self, pipeline: &ir::PipelineLowering, out: &mut Vec<Instruction>) -> StructuredResult<()> {
        match &pipeline.call.kind {
            ExpressionKind::DirectCall(call) => {
                let mut call = call.clone();
                call.arguments.insert(
                    pipeline.inserted_argument,
                    ir::CallArgument { label: None, value: pipeline.input.as_ref().clone(), span: pipeline.input.span },
                );
                self.direct_call(&call, out)
            }
            _ => self.expression(&pipeline.call, out),
        }
    }

    fn branch(
        &mut self, branch: &ir::Branch, type_: &Type, span: crate::source::Span, out: &mut Vec<Instruction>,
    ) -> StructuredResult<()> {
        let results = result_types(type_, span)?;
        let body = self.branch_clause(branch, 0, type_, span, results)?;
        out.extend(body);
        Ok(())
    }

    fn branch_clause(
        &mut self, branch: &ir::Branch, index: usize, type_: &Type, span: crate::source::Span, results: Vec<ValueType>,
    ) -> StructuredResult<Vec<Instruction>> {
        let Some(clause) = branch.clauses.get(index) else {
            let mut failure = Vec::new();
            if !matches!(type_, Type::Nil) && results.is_empty() {
                return Err(StructuredError::Unsupported);
            }
            failure.push(Instruction::Unreachable);
            return Ok(failure);
        };

        let mut condition = Vec::new();
        self.branch_condition(
            &branch.subjects,
            &clause.patterns,
            clause.guard.as_ref(),
            &mut condition,
        )?;
        let mut then_body = Vec::new();
        self.bind_patterns(&branch.subjects, &clause.patterns, &mut then_body)?;
        self.expression(&clause.body, &mut then_body)?;
        let else_body = self.branch_clause(branch, index + 1, type_, span, results.clone())?;
        condition.push(Instruction::If { type_: BlockType::new([], results), then_body, else_body });
        Ok(condition)
    }

    fn branch_condition(
        &mut self, subjects: &[ir::Expression], patterns: &[ir::IrPattern], guard: Option<&ir::Expression>,
        out: &mut Vec<Instruction>,
    ) -> StructuredResult<()> {
        out.push(Instruction::I32Const(1));
        for (subject, pattern) in subjects.iter().zip(patterns) {
            self.pattern_test(subject, pattern, out)?;
            out.push(Instruction::I32And);
        }
        if let Some(guard) = guard {
            self.expression(guard, out)?;
            out.push(Instruction::I32And);
        }
        Ok(())
    }

    fn pattern_test(
        &mut self, subject: &ir::Expression, pattern: &ir::IrPattern, out: &mut Vec<Instruction>,
    ) -> StructuredResult<()> {
        match pattern {
            ir::IrPattern::Discard | ir::IrPattern::Binding(_) => out.push(Instruction::I32Const(1)),
            ir::IrPattern::Alias { pattern, .. } => self.pattern_test(subject, pattern, out)?,
            ir::IrPattern::Literal(literal) => {
                self.expression(subject, out)?;
                let literal_expression = ir::Expression {
                    type_: subject.type_.clone(),
                    span: subject.span,
                    kind: ExpressionKind::Literal(literal.clone()),
                };
                self.expression(&literal_expression, out)?;
                out.push(match subject.type_ {
                    Type::Int => Instruction::I64Eq,
                    Type::Float => Instruction::F64Eq,
                    Type::Bool => Instruction::I32Eq,
                    Type::String => Instruction::I32Eq,
                    Type::Nil => Instruction::I32Const(1),
                    _ => return Err(StructuredError::Unsupported),
                });
            }
            ir::IrPattern::Tuple(elements) => {
                self.managed_tag_test(subject, runtime::ObjectTag::Tuple, Some(elements.len() as u32), out)?;
            }
            ir::IrPattern::List { elements, .. } if elements.is_empty() => {
                self.expression(subject, out)?;
                out.push(Instruction::I32Eqz);
            }
            ir::IrPattern::List { .. } => {
                self.managed_tag_test(subject, runtime::ObjectTag::ListCons, None, out)?;
            }
            ir::IrPattern::Constructor { name, .. } => {
                self.managed_tag_test(subject, runtime::ObjectTag::Custom, None, out)?;
                self.expression(subject, out)?;
                out.push(Instruction::I32Load(mem_arg(self.ensure_memory(), 8, 2)));
                out.push(Instruction::I32Const(super::constructor_tag(name) as i32));
                out.push(Instruction::I32Eq);
                out.push(Instruction::I32And);
            }
            ir::IrPattern::BitString(_) => return Err(StructuredError::Unsupported),
        }
        Ok(())
    }

    fn bind_patterns(
        &mut self, subjects: &[ir::Expression], patterns: &[ir::IrPattern], out: &mut Vec<Instruction>,
    ) -> StructuredResult<()> {
        for (subject, pattern) in subjects.iter().zip(patterns) {
            self.bind_pattern(subject, pattern, out)?;
        }
        Ok(())
    }

    fn bind_pattern(
        &mut self, subject: &ir::Expression, pattern: &ir::IrPattern, out: &mut Vec<Instruction>,
    ) -> StructuredResult<()> {
        match pattern {
            ir::IrPattern::Discard | ir::IrPattern::Literal(_) => {}
            ir::IrPattern::Binding(local) => self.bind_subject(subject, *local, out)?,
            ir::IrPattern::Alias { pattern, local } => {
                self.bind_pattern(subject, pattern, out)?;
                self.bind_subject(subject, *local, out)?;
            }
            ir::IrPattern::Tuple(elements) => {
                for (index, element) in elements.iter().enumerate() {
                    self.bind_managed_pattern_field(subject, element, 8 + index as u32 * 8, out)?;
                }
            }
            ir::IrPattern::List { elements, tail } => {
                if let Some(head) = elements.first() {
                    self.bind_managed_pattern_field(subject, head, 8, out)?;
                }
                if let Some(local) = tail {
                    self.expression(subject, out)?;
                    out.push(Instruction::I32Load(mem_arg(self.ensure_memory(), 16, 2)));
                    out.push(Instruction::LocalSet { local: self.local(*local, subject.span)?, type_: ValueType::I32 });
                }
            }
            ir::IrPattern::Constructor { arguments, .. } => {
                for (index, argument) in arguments.iter().enumerate() {
                    self.bind_managed_pattern_field(subject, &argument.pattern, 12 + index as u32 * 8, out)?;
                }
            }
            ir::IrPattern::BitString(_) => return Err(StructuredError::Unsupported),
        }
        Ok(())
    }

    fn bind_subject(
        &mut self, subject: &ir::Expression, local: ir::LocalId, out: &mut Vec<Instruction>,
    ) -> StructuredResult<()> {
        self.expression(subject, out)?;
        out.push(Instruction::LocalSet {
            local: self.local(local, subject.span)?,
            type_: value_type(&subject.type_, subject.span)?,
        });
        Ok(())
    }

    fn bind_managed_pattern_field(
        &mut self, subject: &ir::Expression, pattern: &ir::IrPattern, offset: u32, out: &mut Vec<Instruction>,
    ) -> StructuredResult<()> {
        match pattern {
            ir::IrPattern::Binding(local) => {
                self.expression(subject, out)?;
                let type_ = value_type(&local_type(*local, self.source), subject.span)?;
                out.push(load_for_type(self.ensure_memory(), offset, type_));
                out.push(Instruction::LocalSet { local: self.local(*local, subject.span)?, type_ });
            }
            ir::IrPattern::Alias { pattern, local } => {
                self.bind_managed_pattern_field(subject, pattern, offset, out)?;
                self.bind_subject(subject, *local, out)?;
            }
            _ => {}
        }
        Ok(())
    }

    fn managed_field_load(
        &mut self, object: &ir::Expression, index: usize, type_: &Type, out: &mut Vec<Instruction>,
    ) -> StructuredResult<()> {
        self.expression(object, out)?;
        let type_ = value_type(type_, object.span)?;
        out.push(load_for_type(self.ensure_memory(), 8 + index as u32 * 8, type_));
        Ok(())
    }

    fn managed_tag_test(
        &mut self, subject: &ir::Expression, tag: runtime::ObjectTag, size: Option<u32>, out: &mut Vec<Instruction>,
    ) -> StructuredResult<()> {
        self.expression(subject, out)?;
        out.push(Instruction::I32Load(mem_arg(self.ensure_memory(), 0, 2)));
        out.push(Instruction::I32Const(u32::from(tag) as i32));
        out.push(Instruction::I32Eq);
        if let Some(size) = size {
            self.expression(subject, out)?;
            out.push(Instruction::I32Load(mem_arg(self.ensure_memory(), 4, 2)));
            out.push(Instruction::I32Const(size as i32));
            out.push(Instruction::I32Eq);
            out.push(Instruction::I32And);
        }
        Ok(())
    }

    fn memory_operation(
        &mut self, operation: &ir::MemoryOperation, out: &mut Vec<Instruction>,
    ) -> StructuredResult<()> {
        match operation {
            ir::MemoryOperation::Allocate { .. } => Err(StructuredError::Unsupported),
            ir::MemoryOperation::Load { address, type_ } => {
                self.expression(address, out)?;
                out.push(match type_ {
                    ir::RepresentationType::Scalar(ir::ScalarRepresentation::I64) => {
                        Instruction::I64Load(mem_arg(self.ensure_memory(), 0, 3))
                    }
                    ir::RepresentationType::Scalar(ir::ScalarRepresentation::F64) => {
                        Instruction::F64Load(mem_arg(self.ensure_memory(), 0, 3))
                    }
                    _ => Instruction::I32Load(mem_arg(self.ensure_memory(), 0, 2)),
                });
                Ok(())
            }
            ir::MemoryOperation::Store { address, value } => {
                self.expression(address, out)?;
                self.expression(value, out)?;
                out.push(store_for_type(
                    self.ensure_memory(),
                    0,
                    value_type(&value.type_, value.span)?,
                ));
                Ok(())
            }
        }
    }

    fn indirect_call(&mut self, call: &ir::IndirectCall, out: &mut Vec<Instruction>) -> StructuredResult<()> {
        let scratch = self.scratch_local.ok_or(StructuredError::Unsupported)?;
        self.expression(&call.callee, out)?;
        out.push(Instruction::LocalSet { local: scratch, type_: ValueType::I32 });
        let results = call
            .abi
            .return_
            .as_ref()
            .map(|value| result_types(&value.type_, call.callee.span))
            .transpose()?
            .unwrap_or_default();
        let body = self.indirect_call_branch(call, 0, scratch, results)?;
        out.extend(body);
        Ok(())
    }

    fn indirect_call_branch(
        &mut self, call: &ir::IndirectCall, index: usize, scratch: LocalId, results: Vec<ValueType>,
    ) -> StructuredResult<Vec<Instruction>> {
        let Some(name) = self.source.functions.get(index).map(|function| function.name.clone()) else {
            return Ok(vec![Instruction::Unreachable]);
        };
        if !self.function_matches_indirect_call(&name, call) {
            return self.indirect_call_branch(call, index + 1, scratch, results);
        }
        let mut condition = vec![
            Instruction::LocalGet { local: scratch, type_: ValueType::I32 },
            Instruction::I32Load(mem_arg(
                self.ensure_memory(),
                u32::from(ClosureConstants::FunctionIdOffset),
                2,
            )),
            Instruction::I32Const(self.function_id(&name) as i32),
            Instruction::I32Eq,
        ];
        let mut then_body = Vec::new();
        if let Some(function) = self.source.functions.iter().find(|function| function.name == name) {
            for (capture_index, type_) in function.closure_captures.iter().enumerate() {
                then_body.push(Instruction::LocalGet { local: scratch, type_: ValueType::I32 });
                then_body.push(load_for_type(
                    self.ensure_memory(),
                    u32::from(ClosureConstants::CapturesOffset)
                        + capture_index as u32 * u32::from(ClosureConstants::CaptureSlotSize),
                    value_type(type_, call.callee.span)?,
                ));
            }
        }
        for argument in &call.arguments {
            self.expression(&argument.value, &mut then_body)?;
        }
        let signature = self.signatures.get(&name).ok_or(StructuredError::Unsupported)?.clone();
        then_body.push(Instruction::Call { function: self.function_id_structured(&name), type_: signature.type_ });
        let else_body = self.indirect_call_branch(call, index + 1, scratch, results.clone())?;
        condition.push(Instruction::If { type_: BlockType::new([], results), then_body, else_body });
        Ok(condition)
    }

    fn function_matches_indirect_call(&self, name: &str, call: &ir::IndirectCall) -> bool {
        let Some(function) = self.source.functions.iter().find(|function| function.name == name) else {
            return false;
        };
        let params = function.params.iter().skip(function.closure_captures.len());
        params.len() == call.arguments.len()
            && params
                .zip(&call.arguments)
                .all(|(param, argument)| param.type_ == argument.value.type_)
            && call.abi.return_.as_ref().map(|value| &value.type_) == Some(&function.return_type)
    }

    fn static_values<'b>(
        &mut self, expressions: impl IntoIterator<Item = &'b ir::Expression>,
    ) -> StructuredResult<Vec<u64>> {
        expressions
            .into_iter()
            .map(|expression| self.static_value(expression))
            .collect()
    }

    fn static_value(&mut self, expression: &ir::Expression) -> StructuredResult<u64> {
        match &expression.kind {
            ExpressionKind::Literal(literal) => match literal.kind {
                LiteralKind::Int => literal.source.parse::<u64>().map_err(|_| StructuredError::Unsupported),
                LiteralKind::Bool => Ok(if literal.source == "True" { 1 } else { 0 }),
                LiteralKind::Nil => Ok(0),
                LiteralKind::String => {
                    let string = literal.source.trim_matches('"');
                    Ok(self.push_static(runtime::string_object(self.config, self.next_static_offset, string)) as u64)
                }
                LiteralKind::Float => Err(StructuredError::Unsupported),
            },
            ExpressionKind::Tuple(items) => {
                let fields = self.static_values(items)?;
                Ok(self.push_static(runtime::tuple_object(self.config, self.next_static_offset, &fields)) as u64)
            }
            ExpressionKind::List(items) => Ok(self.static_list(items)? as u64),
            ExpressionKind::Record(record) => {
                let fields = self.static_values(record.fields.iter().map(|field| &field.value))?;
                Ok(self.push_static(runtime::record_object(self.config, self.next_static_offset, &fields)) as u64)
            }
            ExpressionKind::Constructor(constructor) => {
                let fields = self.static_values(&constructor.arguments)?;
                Ok(self.push_static(runtime::custom_object(
                    self.config,
                    self.next_static_offset,
                    super::constructor_tag(&constructor.name),
                    &fields,
                )) as u64)
            }
            ExpressionKind::FunctionValue(function) => Ok(self.push_static(runtime::closure_object(
                self.config,
                self.next_static_offset,
                self.function_id(&function.name),
                &[],
            )) as u64),
            _ => Err(StructuredError::Unsupported),
        }
    }

    fn static_list(&mut self, items: &[ir::Expression]) -> StructuredResult<u32> {
        let mut tail = 0;
        for item in items.iter().rev() {
            let head = self.static_value(item)?;
            tail = self.push_static(runtime::list_cons_object(
                self.config,
                self.next_static_offset,
                head,
                tail,
            ));
        }
        Ok(tail)
    }

    fn static_pointer(&mut self, object: runtime::StaticObject, out: &mut Vec<Instruction>) -> StructuredResult<()> {
        let pointer = self.push_static(object);
        out.push(Instruction::I32Const(pointer as i32));
        Ok(())
    }

    fn constant(&mut self, constant: &ir::Constant) -> StructuredResult<()> {
        if let ir::ConstantValue::Literal(ir::Literal { kind: LiteralKind::String, source }) = &constant.value {
            let string = source.trim_matches('"');
            self.push_static(runtime::string_object(self.config, self.next_static_offset, string));
        }
        Ok(())
    }

    fn push_static(&mut self, object: runtime::StaticObject) -> u32 {
        let pointer = object.offset;
        let memory = self.ensure_memory();
        self.next_static_offset = self.config.layout.align_to(object.offset + object.bytes.len() as u32);
        self.module.data_segments.push(DataSegment {
            memory,
            offset: vec![Instruction::I32Const(object.offset as i32)],
            bytes: object.bytes,
        });
        pointer
    }

    fn ensure_memory(&mut self) -> MemoryId {
        if let Some(memory) = self.memory {
            return memory;
        }
        let memory = self
            .module
            .push_memory(Memory { minimum_pages: 1, maximum_pages: None });
        self.memory = Some(memory);
        memory
    }

    fn function_id(&self, name: &str) -> u32 {
        self.source
            .functions
            .iter()
            .position(|function| function.name == name)
            .unwrap_or_default() as u32
    }

    fn function_id_structured(&self, name: &str) -> FunctionId {
        self.function_ids
            .get(name)
            .copied()
            .unwrap_or_else(|| FunctionId(self.function_id(name)))
    }

    fn local(&self, local: ir::LocalId, span: crate::source::Span) -> StructuredResult<LocalId> {
        self.local_indices.get(&local).copied().ok_or_else(|| {
            StructuredError::Diagnostics(vec![
                Diagnostic::new(DiagnosticCode::WasmError, "unknown local in structured Wasm emitter")
                    .with_label(Label::primary(span, "local used here")),
            ])
        })
    }
}

fn result_types(type_: &Type, span: crate::source::Span) -> StructuredResult<Vec<ValueType>> {
    if matches!(type_, Type::Nil) { Ok(Vec::new()) } else { Ok(vec![value_type(type_, span)?]) }
}

fn value_type(type_: &Type, _span: crate::source::Span) -> StructuredResult<ValueType> {
    maybe_value_type(type_).ok_or(StructuredError::Unsupported)
}

fn maybe_value_type(type_: &Type) -> Option<ValueType> {
    match type_ {
        Type::Int => Some(ValueType::I64),
        Type::Float => Some(ValueType::F64),
        Type::Bool
        | Type::String
        | Type::BitArray
        | Type::Tuple(_)
        | Type::List(_)
        | Type::Record { .. }
        | Type::Custom { .. }
        | Type::Opaque { .. }
        | Type::Function { .. } => Some(ValueType::I32),
        Type::Nil | Type::Generic(_) => None,
    }
}

fn mem_arg(memory: MemoryId, offset: u32, align: u32) -> MemoryArg {
    MemoryArg { memory, align, offset }
}

fn load_for_type(memory: MemoryId, offset: u32, type_: ValueType) -> Instruction {
    match type_ {
        ValueType::I64 => Instruction::I64Load(mem_arg(memory, offset, 3)),
        ValueType::F64 => Instruction::F64Load(mem_arg(memory, offset, 3)),
        _ => Instruction::I32Load(mem_arg(memory, offset, 2)),
    }
}

fn store_for_type(memory: MemoryId, offset: u32, type_: ValueType) -> Instruction {
    match type_ {
        ValueType::I64 => Instruction::I64Store(mem_arg(memory, offset, 3)),
        ValueType::F64 => Instruction::F64Store(mem_arg(memory, offset, 3)),
        _ => Instruction::I32Store(mem_arg(memory, offset, 2)),
    }
}

fn local_type(local: ir::LocalId, module: &ir::Module) -> Type {
    module
        .functions
        .iter()
        .flat_map(|function| &function.locals)
        .find(|candidate| candidate.id == local)
        .map(|local| local.type_.clone())
        .unwrap_or(Type::Int)
}

fn needs_scratch(function: &ir::Function) -> bool {
    block_needs_scratch(&function.body)
}

fn needed_debug_imports(function: &ir::Function) -> Vec<DebugImport> {
    let mut imports = Vec::new();
    collect_block_debug_imports(&function.body, &mut imports);
    imports.sort_by_key(|import| match import {
        DebugImport::Bool => 0,
        DebugImport::Value => 1,
        DebugImport::I64 => 2,
        DebugImport::F64 => 3,
    });
    imports.dedup();
    imports
}

fn collect_block_debug_imports(block: &ir::Block, imports: &mut Vec<DebugImport>) {
    for instruction in &block.instructions {
        match instruction {
            ir::Instruction::Evaluate { expression, .. } | ir::Instruction::LocalSet { value: expression, .. } => {
                collect_expression_debug_imports(expression, imports);
            }
            ir::Instruction::AssertMatch { value, .. } => collect_expression_debug_imports(value, imports),
        }
    }
    collect_expression_debug_imports(&block.result, imports);
}

fn collect_expression_debug_imports(expression: &ir::Expression, imports: &mut Vec<DebugImport>) {
    match &expression.kind {
        ExpressionKind::DirectCall(call) if call.function == "__stdlib_gleam_io_debug" => {
            if let Some(argument) = call.arguments.first() {
                match argument.value.type_ {
                    Type::Int => imports.push(DebugImport::I64),
                    Type::Float => imports.push(DebugImport::F64),
                    Type::Bool => imports.push(DebugImport::Bool),
                    Type::String
                    | Type::BitArray
                    | Type::Tuple(_)
                    | Type::List(_)
                    | Type::Record { .. }
                    | Type::Custom { .. }
                    | Type::Opaque { .. }
                    | Type::Function { .. } => imports.push(DebugImport::Value),
                    Type::Nil | Type::Generic(_) => {}
                }
            }
            for argument in &call.arguments {
                collect_expression_debug_imports(&argument.value, imports);
            }
        }
        ExpressionKind::DirectCall(call) => {
            for argument in &call.arguments {
                collect_expression_debug_imports(&argument.value, imports);
            }
        }
        ExpressionKind::Branch(branch) => {
            for subject in &branch.subjects {
                collect_expression_debug_imports(subject, imports);
            }
            for clause in &branch.clauses {
                if let Some(guard) = &clause.guard {
                    collect_expression_debug_imports(guard, imports);
                }
                collect_expression_debug_imports(&clause.body, imports);
            }
        }
        ExpressionKind::Pipeline(pipeline) => {
            collect_expression_debug_imports(&pipeline.input, imports);
            collect_expression_debug_imports(&pipeline.call, imports);
        }
        _ => {}
    }
}

fn block_needs_scratch(block: &ir::Block) -> bool {
    block.instructions.iter().any(|instruction| match instruction {
        ir::Instruction::Evaluate { expression, .. } | ir::Instruction::LocalSet { value: expression, .. } => {
            expression_needs_scratch(expression)
        }
        ir::Instruction::AssertMatch { value, .. } => expression_needs_scratch(value),
    }) || expression_needs_scratch(&block.result)
}

fn expression_needs_scratch(expression: &ir::Expression) -> bool {
    match &expression.kind {
        ExpressionKind::IndirectCall(_) => true,
        ExpressionKind::DirectCall(call) => call
            .arguments
            .iter()
            .any(|argument| expression_needs_scratch(&argument.value)),
        ExpressionKind::Branch(branch) => {
            branch.subjects.iter().any(expression_needs_scratch)
                || branch.clauses.iter().any(|clause| {
                    clause.guard.as_ref().is_some_and(expression_needs_scratch)
                        || expression_needs_scratch(&clause.body)
                })
        }
        ExpressionKind::Pipeline(pipeline) => {
            expression_needs_scratch(&pipeline.input) || expression_needs_scratch(&pipeline.call)
        }
        _ => false,
    }
}
