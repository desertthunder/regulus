//! Incremental IR-to-structured-Wasm code generation.

use std::collections::HashMap;

use super::{
    EmitOptions,
    builder::{
        BlockType, Export, ExportDesc, Function, FunctionId, FunctionType, Instruction, Local, LocalId, Module, TypeId,
        ValueType,
    },
};
use crate::{
    ast::LiteralKind,
    diagnostic::{Diagnostic, DiagnosticCode, Diagnostics, Label},
    ir::{self, ExpressionKind},
    types::Type,
};

pub(super) fn emit(module: &ir::Module, _options: EmitOptions) -> Result<Option<Module>, Diagnostics> {
    let emitter = StructuredEmitter::new(module);
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
}

#[derive(Debug, Clone)]
struct FunctionSignature {
    type_id: TypeId,
    type_: FunctionType,
}

#[derive(Debug)]
enum StructuredError {
    Unsupported,
    Diagnostics(Diagnostics),
}

type StructuredResult<T> = Result<T, StructuredError>;

impl<'a> StructuredEmitter<'a> {
    fn new(source: &'a ir::Module) -> Self {
        Self {
            source,
            module: Module::new(),
            signatures: HashMap::new(),
            function_ids: HashMap::new(),
            local_indices: HashMap::new(),
        }
    }

    fn module(mut self, source: &ir::Module) -> StructuredResult<Module> {
        if !source.constants.is_empty() {
            return Err(StructuredError::Unsupported);
        }

        for function in &source.functions {
            let signature = self.function_signature(function)?;
            self.signatures.insert(function.name.clone(), signature);
        }

        for function in &source.functions {
            match &function.abi.boundary {
                ir::CallBoundary::HostImport { .. } | ir::CallBoundary::ModuleImport { .. } => {
                    return Err(StructuredError::Unsupported);
                }
                ir::CallBoundary::Internal | ir::CallBoundary::ModuleExport => {
                    let name = function.name.clone();
                    let function = self.function(function)?;
                    let id = self.module.push_function(function);
                    self.function_ids.insert(name, id);
                }
            }
        }

        for (index, function) in source.functions.iter().enumerate() {
            if matches!(function.abi.boundary, ir::CallBoundary::ModuleExport) {
                self.module
                    .exports
                    .push(Export { name: function.name.clone(), desc: ExportDesc::Function(FunctionId(index as u32)) });
            }
        }

        Ok(self.module)
    }

    fn function_signature(&mut self, function: &ir::Function) -> StructuredResult<FunctionSignature> {
        if !function.closure_captures.is_empty() {
            return Err(StructuredError::Unsupported);
        }
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
                ir::Instruction::AssertMatch { .. } => return Err(StructuredError::Unsupported),
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
            LiteralKind::String => return Err(StructuredError::Unsupported),
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
            "__stdlib_gleam_function_identity" | "__stdlib_gleam_function_constant" => {
                self.expression(&call.arguments[0].value, out)?;
            }
            _ => {
                let id = self
                    .function_ids
                    .get(&call.function)
                    .copied()
                    .or_else(|| {
                        self.source
                            .functions
                            .iter()
                            .position(|f| f.name == call.function)
                            .map(|i| FunctionId(i as u32))
                    })
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
                    Type::Nil => Instruction::I32Const(1),
                    _ => return Err(StructuredError::Unsupported),
                });
            }
            ir::IrPattern::Tuple(_)
            | ir::IrPattern::List { .. }
            | ir::IrPattern::Constructor { .. }
            | ir::IrPattern::BitString(_) => return Err(StructuredError::Unsupported),
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
            ir::IrPattern::Tuple(_)
            | ir::IrPattern::List { .. }
            | ir::IrPattern::Constructor { .. }
            | ir::IrPattern::BitString(_) => return Err(StructuredError::Unsupported),
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
        Type::Bool => Some(ValueType::I32),
        Type::Nil => None,
        _ => None,
    }
}
