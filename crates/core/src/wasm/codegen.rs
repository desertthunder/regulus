//! Incremental IR-to-structured-Wasm code generation.

use std::collections::HashMap;

use super::builder::{
    BlockType, DataSegment, Export, ExportDesc, Function, FunctionId, FunctionType, Global, GlobalId, Import,
    ImportDesc, Instruction, Local, LocalId, Memory, MemoryArg, MemoryId, Module, TypeId, ValueType,
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

#[derive(Clone)]
struct PatternSubject<'a> {
    root: &'a ir::Expression,
    path: Vec<u32>,
}

impl<'a> PatternSubject<'a> {
    fn field(&self, offset: u32) -> Self {
        let mut path = self.path.clone();
        path.push(offset);
        Self { root: self.root, path }
    }

    fn list_element(&self, index: usize) -> Self {
        let mut path = self.path.clone();
        path.extend(std::iter::repeat_n(16, index));
        path.push(8);
        Self { root: self.root, path }
    }

    fn list_tail(&self, elements: usize) -> Self {
        let mut path = self.path.clone();
        path.extend(std::iter::repeat_n(16, elements));
        Self { root: self.root, path }
    }
}

struct StructuredEmitter<'a> {
    source: &'a ir::Module,
    module: Module,
    signatures: HashMap<String, FunctionSignature>,
    function_ids: HashMap<String, FunctionId>,
    local_indices: HashMap<ir::LocalId, LocalId>,
    local_types: HashMap<ir::LocalId, Type>,
    debug_imports: HashMap<DebugImport, FunctionId>,
    debug_locals: HashMap<DebugImport, LocalId>,
    scratch_local: Option<LocalId>,
    alloc_local: Option<LocalId>,
    alloc_end_local: Option<LocalId>,
    alloc_pages_local: Option<LocalId>,
    string_ptr_local: Option<LocalId>,
    string_left_len_local: Option<LocalId>,
    string_right_len_local: Option<LocalId>,
    string_i_local: Option<LocalId>,
    bit_i_local: Option<LocalId>,
    bit_value_local: Option<LocalId>,
    options: EmitOptions,
    config: runtime::RuntimeConfig,
    next_static_offset: u32,
    memory: Option<MemoryId>,
    heap_global: Option<GlobalId>,
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
            local_types: HashMap::new(),
            debug_imports: HashMap::new(),
            debug_locals: HashMap::new(),
            scratch_local: None,
            alloc_local: None,
            alloc_end_local: None,
            alloc_pages_local: None,
            string_ptr_local: None,
            string_left_len_local: None,
            string_right_len_local: None,
            string_i_local: None,
            bit_i_local: None,
            bit_value_local: None,
            options,
            config: runtime::RuntimeConfig::DEFAULT,
            next_static_offset: runtime::RuntimeConfig::DEFAULT.static_data_start,
            memory: None,
            heap_global: None,
            imported_functions: 0,
        }
    }

    fn module(mut self, source: &ir::Module) -> StructuredResult<Module> {
        self.module.source_span = source.functions.first().map(|function| function.span);
        for function in &source.functions {
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
                self.export_adapters(function)?;
            }
        }

        if let Some(memory) = self.memory {
            self.module
                .exports
                .push(Export { name: "memory".into(), desc: ExportDesc::Memory(memory) });
        }

        Ok(self.module)
    }

    fn export_adapters(&mut self, function: &ir::Function) -> StructuredResult<()> {
        if !function.params.is_empty() || function.return_type != Type::String {
            return Ok(());
        }
        let string_result = FunctionType::new([], [ValueType::I32]);
        let string_type = self
            .signatures
            .get(&function.name)
            .ok_or(StructuredError::Unsupported)?
            .type_
            .clone();
        let original = self.function_id_structured(&function.name);
        let memory = self.ensure_memory();

        let data_type = self.module.push_type(FunctionType::new([], [ValueType::I32]));
        let mut data = Function::new(data_type);
        data.name = Some(format!("{}__data", function.name));
        data.body = vec![
            Instruction::Call { function: original, type_: string_type.clone() },
            Instruction::I32Const(8),
            Instruction::I32Add,
        ];
        let data_id = self.module.push_function(data);
        self.module
            .exports
            .push(Export { name: format!("{}__data", function.name), desc: ExportDesc::Function(data_id) });

        let len_type = self.module.push_type(FunctionType::new([], [ValueType::I32]));
        let mut len = Function::new(len_type);
        len.name = Some(format!("{}__len", function.name));
        len.body = vec![
            Instruction::Call { function: original, type_: string_result },
            Instruction::I32Load(mem_arg(memory, 4, 2)),
        ];
        let len_id = self.module.push_function(len);
        self.module
            .exports
            .push(Export { name: format!("{}__len", function.name), desc: ExportDesc::Function(len_id) });
        Ok(())
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
        self.local_types = function
            .locals
            .iter()
            .map(|local| (local.id, local.type_.clone()))
            .collect();
        self.debug_locals.clear();
        self.scratch_local = None;
        self.alloc_local = None;
        self.alloc_end_local = None;
        self.alloc_pages_local = None;
        self.string_ptr_local = None;
        self.string_left_len_local = None;
        self.string_right_len_local = None;
        self.string_i_local = None;
        self.bit_i_local = None;
        self.bit_value_local = None;

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
        if needs_allocation(function) {
            let id = LocalId((structured.params.len() + structured.locals.len()) as u32);
            structured
                .locals
                .push(Local { name: Some("__alloc_ptr".into()), type_: ValueType::I32 });
            self.alloc_local = Some(id);
            let id = LocalId((structured.params.len() + structured.locals.len()) as u32);
            structured
                .locals
                .push(Local { name: Some("__alloc_end".into()), type_: ValueType::I32 });
            self.alloc_end_local = Some(id);
            let id = LocalId((structured.params.len() + structured.locals.len()) as u32);
            structured
                .locals
                .push(Local { name: Some("__alloc_pages".into()), type_: ValueType::I32 });
            self.alloc_pages_local = Some(id);
        }
        if needs_string_concat(function) {
            let id = LocalId((structured.params.len() + structured.locals.len()) as u32);
            structured
                .locals
                .push(Local { name: Some("__string_ptr".into()), type_: ValueType::I32 });
            self.string_ptr_local = Some(id);
            let id = LocalId((structured.params.len() + structured.locals.len()) as u32);
            structured
                .locals
                .push(Local { name: Some("__string_left_len".into()), type_: ValueType::I32 });
            self.string_left_len_local = Some(id);
            let id = LocalId((structured.params.len() + structured.locals.len()) as u32);
            structured
                .locals
                .push(Local { name: Some("__string_right_len".into()), type_: ValueType::I32 });
            self.string_right_len_local = Some(id);
            let id = LocalId((structured.params.len() + structured.locals.len()) as u32);
            structured
                .locals
                .push(Local { name: Some("__string_i".into()), type_: ValueType::I32 });
            self.string_i_local = Some(id);
        }
        if needs_bit_string_pattern(&function.body) {
            let id = LocalId((structured.params.len() + structured.locals.len()) as u32);
            structured
                .locals
                .push(Local { name: Some("__bit_i".into()), type_: ValueType::I32 });
            self.bit_i_local = Some(id);
            let id = LocalId((structured.params.len() + structured.locals.len()) as u32);
            structured
                .locals
                .push(Local { name: Some("__bit_value".into()), type_: ValueType::I64 });
            self.bit_value_local = Some(id);
        }
        for debug_import in needed_debug_imports(function) {
            let id = LocalId((structured.params.len() + structured.locals.len()) as u32);
            structured
                .locals
                .push(Local { name: Some(format!("__{}", debug_import.name())), type_: debug_import.value_type() });
            self.debug_locals.insert(debug_import, id);
        }
        structured.body = self.block(&function.body)?;
        self.local_types.clear();
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
            ExpressionKind::RecordUpdate { record, constructor, fields } => {
                self.record_update(record, constructor, fields, &expression.type_, out)
            }
            ExpressionKind::Constructor(constructor) => match self.static_values(&constructor.arguments) {
                Ok(fields) => {
                    let object = runtime::custom_object(
                        self.config,
                        self.next_static_offset,
                        super::constructor_tag(&constructor.name),
                        &fields,
                    );
                    self.static_pointer(object, out)
                }
                Err(StructuredError::Unsupported) => self.constructor_value(constructor, out),
                Err(error) => Err(error),
            },
            ExpressionKind::FunctionValue(function) => self.static_pointer(
                runtime::closure_object(
                    self.config,
                    self.next_static_offset,
                    self.function_id(&function.name),
                    &[],
                ),
                out,
            ),
            ExpressionKind::AnonymousFunction(function) => self.closure_allocation(function, out),
            ExpressionKind::FieldAccess { record, .. } => self.managed_field_load(record, 0, &expression.type_, out),
            ExpressionKind::TupleElement { tuple, index } => {
                self.managed_field_load(tuple, *index, &expression.type_, out)
            }
            ExpressionKind::ListCons { head, tail } => self.list_cons(head, tail, out),
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
            "__op_string_concat" | "__stdlib_gleam_string_append" => self.string_concat(call, out)?,
            "__stdlib_gleam_float_negate" => {
                out.push(Instruction::F64Const((-0.0f64).to_bits()));
                self.expression(&call.arguments[0].value, out)?;
                out.push(Instruction::F64Sub);
            }
            "__stdlib_gleam_float_max" | "__stdlib_gleam_float_min" => {
                self.binary_arguments(call, out)?;
                out.push(if call.function == "__stdlib_gleam_float_max" {
                    Instruction::F64Max
                } else {
                    Instruction::F64Min
                });
            }
            "__stdlib_gleam_string_length" => {
                self.expression(&call.arguments[0].value, out)?;
                out.push(Instruction::I32Load(mem_arg(self.ensure_memory(), 4, 2)));
                out.push(Instruction::I64ExtendI32U);
            }
            "__stdlib_gleam_string_is_empty" => {
                self.expression(&call.arguments[0].value, out)?;
                out.push(Instruction::I32Load(mem_arg(self.ensure_memory(), 4, 2)));
                out.push(Instruction::I32Eqz);
            }
            "__stdlib_gleam_function_identity" | "__stdlib_gleam_function_constant" => {
                self.expression(&call.arguments[0].value, out)?;
            }
            "__stdlib_gleam_io_debug" => self.stdlib_io_debug(call, out)?,
            _ => {
                let signature = self
                    .signatures
                    .get(&call.function)
                    .ok_or(StructuredError::Unsupported)?
                    .clone();
                let id = self.function_id_structured(&call.function);
                for argument in &call.arguments {
                    self.expression(&argument.value, out)?;
                }
                out.push(Instruction::Call { function: id, type_: signature.type_ });
            }
        }
        Ok(())
    }

    fn allocate(&mut self, bytes: u32, out: &mut Vec<Instruction>) -> StructuredResult<()> {
        let ptr = self.alloc_local.ok_or(StructuredError::Unsupported)?;
        let end = self.alloc_end_local.ok_or(StructuredError::Unsupported)?;
        let pages = self.alloc_pages_local.ok_or(StructuredError::Unsupported)?;
        let heap = self.ensure_heap_global();
        let memory = self.ensure_memory();

        out.push(Instruction::GlobalGet { global: heap, type_: ValueType::I32 });
        out.push(Instruction::LocalSet { local: ptr, type_: ValueType::I32 });
        out.push(Instruction::GlobalGet { global: heap, type_: ValueType::I32 });
        out.push(Instruction::I32Const(bytes as i32));
        out.push(Instruction::I32Add);
        out.push(Instruction::I32Const((self.config.layout.alignment - 1) as i32));
        out.push(Instruction::I32Add);
        out.push(Instruction::I32Const(-(self.config.layout.alignment as i32)));
        out.push(Instruction::I32And);
        out.push(Instruction::LocalSet { local: end, type_: ValueType::I32 });

        out.push(Instruction::LocalGet { local: end, type_: ValueType::I32 });
        out.push(Instruction::LocalGet { local: ptr, type_: ValueType::I32 });
        out.push(Instruction::I32LtU);
        out.push(Instruction::If {
            type_: BlockType::empty(),
            then_body: vec![Instruction::Unreachable],
            else_body: Vec::new(),
        });

        out.push(Instruction::LocalGet { local: end, type_: ValueType::I32 });
        out.push(Instruction::MemorySize(memory));
        out.push(Instruction::I32Const(65536));
        out.push(Instruction::I32Mul);
        out.push(Instruction::I32GtU);
        out.push(Instruction::If {
            type_: BlockType::empty(),
            then_body: vec![
                Instruction::LocalGet { local: end, type_: ValueType::I32 },
                Instruction::MemorySize(memory),
                Instruction::I32Const(65536),
                Instruction::I32Mul,
                Instruction::I32Sub,
                Instruction::I32Const(65535),
                Instruction::I32Add,
                Instruction::I32Const(16),
                Instruction::I32ShrU,
                Instruction::LocalTee { local: pages, type_: ValueType::I32 },
                Instruction::MemoryGrow(memory),
                Instruction::I32Const(-1),
                Instruction::I32Eq,
                Instruction::If {
                    type_: BlockType::empty(),
                    then_body: vec![Instruction::Unreachable],
                    else_body: Vec::new(),
                },
            ],
            else_body: Vec::new(),
        });

        out.push(Instruction::LocalGet { local: end, type_: ValueType::I32 });
        out.push(Instruction::GlobalSet { global: heap, type_: ValueType::I32 });
        out.push(Instruction::LocalGet { local: ptr, type_: ValueType::I32 });
        Ok(())
    }

    fn allocate_dynamic(&mut self, out: &mut Vec<Instruction>) -> StructuredResult<()> {
        let ptr = self.alloc_local.ok_or(StructuredError::Unsupported)?;
        let end = self.alloc_end_local.ok_or(StructuredError::Unsupported)?;
        let pages = self.alloc_pages_local.ok_or(StructuredError::Unsupported)?;
        let heap = self.ensure_heap_global();
        let memory = self.ensure_memory();
        out.push(Instruction::GlobalGet { global: heap, type_: ValueType::I32 });
        out.push(Instruction::LocalSet { local: ptr, type_: ValueType::I32 });
        out.push(Instruction::GlobalGet { global: heap, type_: ValueType::I32 });
        out.push(Instruction::I32Add);
        out.push(Instruction::I32Const((self.config.layout.alignment - 1) as i32));
        out.push(Instruction::I32Add);
        out.push(Instruction::I32Const(-(self.config.layout.alignment as i32)));
        out.push(Instruction::I32And);
        out.push(Instruction::LocalSet { local: end, type_: ValueType::I32 });
        out.push(Instruction::LocalGet { local: end, type_: ValueType::I32 });
        out.push(Instruction::LocalGet { local: ptr, type_: ValueType::I32 });
        out.push(Instruction::I32LtU);
        out.push(Instruction::If {
            type_: BlockType::empty(),
            then_body: vec![Instruction::Unreachable],
            else_body: Vec::new(),
        });
        out.push(Instruction::LocalGet { local: end, type_: ValueType::I32 });
        out.push(Instruction::MemorySize(memory));
        out.push(Instruction::I32Const(65536));
        out.push(Instruction::I32Mul);
        out.push(Instruction::I32GtU);
        out.push(Instruction::If {
            type_: BlockType::empty(),
            then_body: vec![
                Instruction::LocalGet { local: end, type_: ValueType::I32 },
                Instruction::MemorySize(memory),
                Instruction::I32Const(65536),
                Instruction::I32Mul,
                Instruction::I32Sub,
                Instruction::I32Const(65535),
                Instruction::I32Add,
                Instruction::I32Const(16),
                Instruction::I32ShrU,
                Instruction::LocalTee { local: pages, type_: ValueType::I32 },
                Instruction::MemoryGrow(memory),
                Instruction::I32Const(-1),
                Instruction::I32Eq,
                Instruction::If {
                    type_: BlockType::empty(),
                    then_body: vec![Instruction::Unreachable],
                    else_body: Vec::new(),
                },
            ],
            else_body: Vec::new(),
        });
        out.push(Instruction::LocalGet { local: end, type_: ValueType::I32 });
        out.push(Instruction::GlobalSet { global: heap, type_: ValueType::I32 });
        out.push(Instruction::LocalGet { local: ptr, type_: ValueType::I32 });
        Ok(())
    }

    fn string_concat(&mut self, call: &ir::DirectCall, out: &mut Vec<Instruction>) -> StructuredResult<()> {
        let ptr = self.string_ptr_local.ok_or(StructuredError::Unsupported)?;
        let left_len = self.string_left_len_local.ok_or(StructuredError::Unsupported)?;
        let right_len = self.string_right_len_local.ok_or(StructuredError::Unsupported)?;
        self.expression(&call.arguments[0].value, out)?;
        out.push(Instruction::I32Load(mem_arg(self.ensure_memory(), 4, 2)));
        out.push(Instruction::LocalSet { local: left_len, type_: ValueType::I32 });
        self.expression(&call.arguments[1].value, out)?;
        out.push(Instruction::I32Load(mem_arg(self.ensure_memory(), 4, 2)));
        out.push(Instruction::LocalSet { local: right_len, type_: ValueType::I32 });
        out.push(Instruction::I32Const(8));
        out.push(Instruction::LocalGet { local: left_len, type_: ValueType::I32 });
        out.push(Instruction::LocalGet { local: right_len, type_: ValueType::I32 });
        out.push(Instruction::I32Add);
        out.push(Instruction::I32Add);
        self.allocate_dynamic(out)?;
        out.push(Instruction::LocalTee { local: ptr, type_: ValueType::I32 });
        out.push(Instruction::I32Const(u32::from(runtime::ObjectTag::String) as i32));
        out.push(Instruction::I32Store(mem_arg(self.ensure_memory(), 0, 2)));
        out.push(Instruction::LocalGet { local: ptr, type_: ValueType::I32 });
        out.push(Instruction::LocalGet { local: left_len, type_: ValueType::I32 });
        out.push(Instruction::LocalGet { local: right_len, type_: ValueType::I32 });
        out.push(Instruction::I32Add);
        out.push(Instruction::I32Store(mem_arg(self.ensure_memory(), 4, 2)));
        self.copy_string_bytes(&call.arguments[0].value, ptr, left_len, None, out)?;
        self.copy_string_bytes(&call.arguments[1].value, ptr, right_len, Some(left_len), out)?;
        out.push(Instruction::LocalGet { local: ptr, type_: ValueType::I32 });
        Ok(())
    }

    fn copy_string_bytes(
        &mut self, source: &ir::Expression, dest: LocalId, len: LocalId, dest_extra: Option<LocalId>,
        out: &mut Vec<Instruction>,
    ) -> StructuredResult<()> {
        let i = self.string_i_local.ok_or(StructuredError::Unsupported)?;
        let memory = self.ensure_memory();
        out.push(Instruction::I32Const(0));
        out.push(Instruction::LocalSet { local: i, type_: ValueType::I32 });
        let mut body = vec![
            Instruction::LocalGet { local: i, type_: ValueType::I32 },
            Instruction::LocalGet { local: len, type_: ValueType::I32 },
            Instruction::I32GeS,
            Instruction::BrIf { depth: 1, results: Vec::new() },
            Instruction::LocalGet { local: dest, type_: ValueType::I32 },
            Instruction::I32Const(8),
            Instruction::I32Add,
        ];
        if let Some(extra) = dest_extra {
            body.push(Instruction::LocalGet { local: extra, type_: ValueType::I32 });
            body.push(Instruction::I32Add);
        }
        body.push(Instruction::LocalGet { local: i, type_: ValueType::I32 });
        body.push(Instruction::I32Add);
        self.expression(source, &mut body)?;
        body.push(Instruction::I32Const(8));
        body.push(Instruction::I32Add);
        body.push(Instruction::LocalGet { local: i, type_: ValueType::I32 });
        body.push(Instruction::I32Add);
        body.push(Instruction::I32Load8U(mem_arg(memory, 0, 0)));
        body.push(Instruction::I32Store8(mem_arg(memory, 0, 0)));
        body.push(Instruction::LocalGet { local: i, type_: ValueType::I32 });
        body.push(Instruction::I32Const(1));
        body.push(Instruction::I32Add);
        body.push(Instruction::LocalSet { local: i, type_: ValueType::I32 });
        body.push(Instruction::Br { depth: 0, results: Vec::new() });
        out.push(Instruction::Block {
            type_: BlockType::empty(),
            body: vec![Instruction::Loop { type_: BlockType::empty(), body }],
        });
        Ok(())
    }

    fn constructor_value(
        &mut self, constructor: &ir::ConstructorValue, out: &mut Vec<Instruction>,
    ) -> StructuredResult<()> {
        let ptr = self.alloc_local.ok_or(StructuredError::Unsupported)?;
        let size = self.config.layout.custom_size(constructor.arguments.len() as u32, 8);
        self.allocate(size, out)?;
        out.push(Instruction::LocalTee { local: ptr, type_: ValueType::I32 });
        out.push(Instruction::I32Const(u32::from(runtime::ObjectTag::Custom) as i32));
        out.push(Instruction::I32Store(mem_arg(self.ensure_memory(), 0, 2)));
        out.push(Instruction::LocalGet { local: ptr, type_: ValueType::I32 });
        out.push(Instruction::I32Const(constructor.arguments.len() as i32));
        out.push(Instruction::I32Store(mem_arg(self.ensure_memory(), 4, 2)));
        out.push(Instruction::LocalGet { local: ptr, type_: ValueType::I32 });
        out.push(Instruction::I32Const(super::constructor_tag(&constructor.name) as i32));
        out.push(Instruction::I32Store(mem_arg(self.ensure_memory(), 8, 2)));
        for (index, argument) in constructor.arguments.iter().enumerate() {
            out.push(Instruction::LocalGet { local: ptr, type_: ValueType::I32 });
            self.expression_slot_value(argument, out)?;
            out.push(Instruction::I64Store(mem_arg(
                self.ensure_memory(),
                12 + index as u32 * 8,
                3,
            )));
        }
        out.push(Instruction::LocalGet { local: ptr, type_: ValueType::I32 });
        Ok(())
    }

    fn record_update(
        &mut self, record: &ir::Expression, constructor: &str, fields: &[ir::RecordFieldUpdate], type_: &Type,
        out: &mut Vec<Instruction>,
    ) -> StructuredResult<()> {
        let source = self.scratch_local.ok_or(StructuredError::Unsupported)?;
        let ptr = self.alloc_local.ok_or(StructuredError::Unsupported)?;
        self.expression(record, out)?;
        out.push(Instruction::LocalSet { local: source, type_: ValueType::I32 });
        let (size, tag, header_fields, slot_offset) = if matches!(type_, Type::Record { .. }) {
            (
                self.config.layout.record_size(fields.len() as u32, 8),
                runtime::ObjectTag::Record,
                fields.len() as i32,
                8,
            )
        } else {
            (
                self.config.layout.custom_size(fields.len() as u32, 8),
                runtime::ObjectTag::Custom,
                fields.len() as i32,
                12,
            )
        };
        self.allocate(size, out)?;
        out.push(Instruction::LocalTee { local: ptr, type_: ValueType::I32 });
        out.push(Instruction::I32Const(u32::from(tag) as i32));
        out.push(Instruction::I32Store(mem_arg(self.ensure_memory(), 0, 2)));
        out.push(Instruction::LocalGet { local: ptr, type_: ValueType::I32 });
        out.push(Instruction::I32Const(header_fields));
        out.push(Instruction::I32Store(mem_arg(self.ensure_memory(), 4, 2)));
        if !matches!(type_, Type::Record { .. }) {
            out.push(Instruction::LocalGet { local: ptr, type_: ValueType::I32 });
            out.push(Instruction::I32Const(super::constructor_tag(constructor) as i32));
            out.push(Instruction::I32Store(mem_arg(self.ensure_memory(), 8, 2)));
        }
        for (index, field) in fields.iter().enumerate() {
            out.push(Instruction::LocalGet { local: ptr, type_: ValueType::I32 });
            match &field.value {
                Some(value) => self.expression_slot_value(value, out)?,
                None => {
                    out.push(Instruction::LocalGet { local: source, type_: ValueType::I32 });
                    out.push(Instruction::I64Load(mem_arg(
                        self.ensure_memory(),
                        slot_offset + index as u32 * 8,
                        3,
                    )));
                }
            }
            out.push(Instruction::I64Store(mem_arg(
                self.ensure_memory(),
                slot_offset + index as u32 * 8,
                3,
            )));
        }
        out.push(Instruction::LocalGet { local: ptr, type_: ValueType::I32 });
        Ok(())
    }

    fn list_cons(
        &mut self, head: &ir::Expression, tail: &ir::Expression, out: &mut Vec<Instruction>,
    ) -> StructuredResult<()> {
        let ptr = self.alloc_local.ok_or(StructuredError::Unsupported)?;
        self.allocate(self.config.layout.list_cons_size(8), out)?;
        out.push(Instruction::LocalTee { local: ptr, type_: ValueType::I32 });
        out.push(Instruction::I32Const(u32::from(runtime::ObjectTag::ListCons) as i32));
        out.push(Instruction::I32Store(mem_arg(self.ensure_memory(), 0, 2)));
        out.push(Instruction::LocalGet { local: ptr, type_: ValueType::I32 });
        out.push(Instruction::I32Const(2));
        out.push(Instruction::I32Store(mem_arg(self.ensure_memory(), 4, 2)));
        out.push(Instruction::LocalGet { local: ptr, type_: ValueType::I32 });
        self.expression_slot_value(head, out)?;
        out.push(Instruction::I64Store(mem_arg(self.ensure_memory(), 8, 3)));
        out.push(Instruction::LocalGet { local: ptr, type_: ValueType::I32 });
        self.expression(tail, out)?;
        out.push(Instruction::I32Store(mem_arg(self.ensure_memory(), 16, 2)));
        out.push(Instruction::LocalGet { local: ptr, type_: ValueType::I32 });
        Ok(())
    }

    fn closure_allocation(
        &mut self, function: &ir::AnonymousFunction, out: &mut Vec<Instruction>,
    ) -> StructuredResult<()> {
        let ptr = self.alloc_local.ok_or(StructuredError::Unsupported)?;
        let size = self.config.layout.closure_size(function.captures.len() as u32);
        self.allocate(size, out)?;
        out.push(Instruction::LocalTee { local: ptr, type_: ValueType::I32 });
        out.push(Instruction::I32Const(u32::from(runtime::ObjectTag::Closure) as i32));
        out.push(Instruction::I32Store(mem_arg(self.ensure_memory(), 0, 2)));
        out.push(Instruction::LocalGet { local: ptr, type_: ValueType::I32 });
        out.push(Instruction::I32Const(function.captures.len() as i32));
        out.push(Instruction::I32Store(mem_arg(self.ensure_memory(), 4, 2)));
        out.push(Instruction::LocalGet { local: ptr, type_: ValueType::I32 });
        out.push(Instruction::I32Const(self.function_id(&function.name) as i32));
        out.push(Instruction::I32Store(mem_arg(
            self.ensure_memory(),
            u32::from(ClosureConstants::FunctionIdOffset),
            2,
        )));
        for (index, capture) in function.captures.iter().enumerate() {
            out.push(Instruction::LocalGet { local: ptr, type_: ValueType::I32 });
            out.push(Instruction::LocalGet {
                local: self.local(capture.source, capture.span)?,
                type_: value_type(&capture.type_, capture.span)?,
            });
            self.extend_slot_value(&capture.type_, out);
            out.push(Instruction::I64Store(mem_arg(
                self.ensure_memory(),
                u32::from(ClosureConstants::CapturesOffset)
                    + index as u32 * u32::from(ClosureConstants::CaptureSlotSize),
                3,
            )));
        }
        out.push(Instruction::LocalGet { local: ptr, type_: ValueType::I32 });
        Ok(())
    }

    fn expression_slot_value(
        &mut self, expression: &ir::Expression, out: &mut Vec<Instruction>,
    ) -> StructuredResult<()> {
        self.expression(expression, out)?;
        self.extend_slot_value(&expression.type_, out);
        Ok(())
    }

    fn extend_slot_value(&mut self, type_: &Type, out: &mut Vec<Instruction>) {
        match type_ {
            Type::Float => out.push(Instruction::I64ReinterpretF64),
            Type::Int => {}
            Type::Nil => out.push(Instruction::I64Const(0)),
            _ => out.push(Instruction::I64ExtendI32U),
        }
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
        self.pattern_test_subject(&PatternSubject { root: subject, path: Vec::new() }, pattern, out)
    }

    fn pattern_test_subject(
        &mut self, subject: &PatternSubject<'_>, pattern: &ir::IrPattern, out: &mut Vec<Instruction>,
    ) -> StructuredResult<()> {
        match pattern {
            ir::IrPattern::Discard | ir::IrPattern::Binding(_) => out.push(Instruction::I32Const(1)),
            ir::IrPattern::Alias { pattern, .. } => self.pattern_test_subject(subject, pattern, out)?,
            ir::IrPattern::Literal(literal) => self.pattern_literal_test(subject, literal, out)?,
            ir::IrPattern::Tuple(elements) => {
                self.managed_tag_test_subject(subject, runtime::ObjectTag::Tuple, Some(elements.len() as u32), out)?;
                for (index, element) in elements.iter().enumerate() {
                    self.pattern_test_subject(&subject.field(8 + index as u32 * 8), element, out)?;
                    out.push(Instruction::I32And);
                }
            }
            ir::IrPattern::List { elements, .. } if elements.is_empty() => {
                self.subject_pointer(subject, out)?;
                out.push(Instruction::I32Eqz);
            }
            ir::IrPattern::List { elements, .. } => {
                for index in 0..elements.len() {
                    self.managed_tag_test_subject(&subject.list_tail(index), runtime::ObjectTag::ListCons, None, out)?;
                    if index > 0 {
                        out.push(Instruction::I32And);
                    }
                }
                for (index, element) in elements.iter().enumerate() {
                    self.pattern_test_subject(&subject.list_element(index), element, out)?;
                    out.push(Instruction::I32And);
                }
            }
            ir::IrPattern::Constructor { name, arguments } => {
                self.managed_tag_test_subject(subject, runtime::ObjectTag::Custom, None, out)?;
                self.subject_pointer(subject, out)?;
                out.push(Instruction::I32Load(mem_arg(self.ensure_memory(), 8, 2)));
                out.push(Instruction::I32Const(super::constructor_tag(name) as i32));
                out.push(Instruction::I32Eq);
                out.push(Instruction::I32And);
                for (index, argument) in arguments.iter().enumerate() {
                    self.pattern_test_subject(&subject.field(12 + index as u32 * 8), &argument.pattern, out)?;
                    out.push(Instruction::I32And);
                }
            }
            ir::IrPattern::BitString(segments) => self.bit_string_pattern_test_subject(subject, segments, out)?,
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
        self.bind_pattern_subject(&PatternSubject { root: subject, path: Vec::new() }, pattern, out)
    }

    fn bind_pattern_subject(
        &mut self, subject: &PatternSubject<'_>, pattern: &ir::IrPattern, out: &mut Vec<Instruction>,
    ) -> StructuredResult<()> {
        match pattern {
            ir::IrPattern::Discard | ir::IrPattern::Literal(_) => {}
            ir::IrPattern::Binding(local) => self.bind_subject_to_local(subject, *local, out)?,
            ir::IrPattern::Alias { pattern, local } => {
                self.bind_pattern_subject(subject, pattern, out)?;
                self.bind_subject_to_local(subject, *local, out)?;
            }
            ir::IrPattern::Tuple(elements) => {
                for (index, element) in elements.iter().enumerate() {
                    self.bind_pattern_subject(&subject.field(8 + index as u32 * 8), element, out)?;
                }
            }
            ir::IrPattern::List { elements, tail } => {
                for (index, element) in elements.iter().enumerate() {
                    self.bind_pattern_subject(&subject.list_element(index), element, out)?;
                }
                if let Some(local) = tail {
                    self.bind_subject_to_local(&subject.list_tail(elements.len()), *local, out)?;
                }
            }
            ir::IrPattern::Constructor { arguments, .. } => {
                for (index, argument) in arguments.iter().enumerate() {
                    self.bind_pattern_subject(&subject.field(12 + index as u32 * 8), &argument.pattern, out)?;
                }
            }
            ir::IrPattern::BitString(segments) => self.bind_bit_string_pattern_subject(subject, segments, out)?,
        }
        Ok(())
    }

    fn bind_subject_to_local(
        &mut self, subject: &PatternSubject<'_>, local: ir::LocalId, out: &mut Vec<Instruction>,
    ) -> StructuredResult<()> {
        let type_ = value_type(self.local_types.get(&local).unwrap_or(&Type::Int), subject.root.span)?;
        if subject.path.is_empty() {
            self.expression(subject.root, out)?;
        } else {
            self.slot_address(subject, out)?;
            out.push(load_for_type(self.ensure_memory(), 0, type_));
        }
        out.push(Instruction::LocalSet { local: self.local(local, subject.root.span)?, type_ });
        Ok(())
    }

    fn pattern_literal_test(
        &mut self, subject: &PatternSubject<'_>, literal: &ir::Literal, out: &mut Vec<Instruction>,
    ) -> StructuredResult<()> {
        if subject.path.is_empty() {
            self.expression(subject.root, out)?;
        } else {
            self.slot_address(subject, out)?;
            out.push(match literal.kind {
                LiteralKind::Int => Instruction::I64Load(mem_arg(self.ensure_memory(), 0, 3)),
                LiteralKind::Float => Instruction::F64Load(mem_arg(self.ensure_memory(), 0, 3)),
                LiteralKind::Bool | LiteralKind::String => Instruction::I32Load(mem_arg(self.ensure_memory(), 0, 2)),
                LiteralKind::Nil => Instruction::I64Load(mem_arg(self.ensure_memory(), 0, 3)),
            });
        }
        let literal_expression = ir::Expression {
            type_: literal_type(literal),
            span: subject.root.span,
            kind: ExpressionKind::Literal(literal.clone()),
        };
        self.expression(&literal_expression, out)?;
        out.push(match literal.kind {
            LiteralKind::Int => Instruction::I64Eq,
            LiteralKind::Float => Instruction::F64Eq,
            LiteralKind::Bool | LiteralKind::String => Instruction::I32Eq,
            LiteralKind::Nil => Instruction::I32Const(1),
        });
        Ok(())
    }

    fn managed_tag_test_subject(
        &mut self, subject: &PatternSubject<'_>, tag: runtime::ObjectTag, size: Option<u32>, out: &mut Vec<Instruction>,
    ) -> StructuredResult<()> {
        self.subject_pointer(subject, out)?;
        out.push(Instruction::I32Load(mem_arg(self.ensure_memory(), 0, 2)));
        out.push(Instruction::I32Const(u32::from(tag) as i32));
        out.push(Instruction::I32Eq);
        if let Some(size) = size {
            self.subject_pointer(subject, out)?;
            out.push(Instruction::I32Load(mem_arg(self.ensure_memory(), 4, 2)));
            out.push(Instruction::I32Const(size as i32));
            out.push(Instruction::I32Eq);
            out.push(Instruction::I32And);
        }
        Ok(())
    }

    fn subject_pointer(&mut self, subject: &PatternSubject<'_>, out: &mut Vec<Instruction>) -> StructuredResult<()> {
        if subject.path.is_empty() {
            self.expression(subject.root, out)?;
        } else {
            self.slot_address(subject, out)?;
            out.push(Instruction::I32Load(mem_arg(self.ensure_memory(), 0, 2)));
        }
        Ok(())
    }

    fn bit_string_pattern_test_subject(
        &mut self, subject: &PatternSubject<'_>, segments: &[ir::BitStringPatternSegment], out: &mut Vec<Instruction>,
    ) -> StructuredResult<()> {
        self.validate_bit_string_pattern_segments(segments, subject.root.span)?;
        let fixed_bit_len = segments.iter().filter_map(|segment| segment.bit_size).sum::<u32>();
        let has_variable_tail = segments.last().is_some_and(|segment| segment.bit_size.is_none());
        self.managed_tag_test_subject(subject, runtime::ObjectTag::BitArray, None, out)?;
        self.subject_pointer(subject, out)?;
        out.push(Instruction::I32Load(mem_arg(self.ensure_memory(), 4, 2)));
        out.push(Instruction::I32Const(fixed_bit_len as i32));
        out.push(if has_variable_tail { Instruction::I32GeS } else { Instruction::I32Eq });
        out.push(Instruction::I32And);
        let mut offset = 0;
        for segment in segments {
            if let Some(value) = segment.value {
                self.bit_string_integer_segment_test(subject, offset, segment.bit_size.unwrap_or(8), value, out)?;
                out.push(Instruction::I32And);
            }
            offset += segment.bit_size.unwrap_or(0);
        }
        Ok(())
    }

    fn validate_bit_string_pattern_segments(
        &self, segments: &[ir::BitStringPatternSegment], span: crate::source::Span,
    ) -> StructuredResult<()> {
        for (index, segment) in segments.iter().enumerate() {
            match segment.type_ {
                ir::BitSegmentType::Integer => {}
                ir::BitSegmentType::Binary if segment.bit_size.is_some() || index + 1 == segments.len() => {}
                _ => {
                    return Err(StructuredError::Diagnostics(vec![
                        Diagnostic::new(DiagnosticCode::WasmError, "unsupported bit-string pattern segment type")
                            .with_label(Label::primary(span, "bit-string pattern here")),
                    ]));
                }
            }
        }
        Ok(())
    }

    fn bit_string_integer_segment_test(
        &mut self, subject: &PatternSubject<'_>, offset: u32, bit_size: u32, value: u64, out: &mut Vec<Instruction>,
    ) -> StructuredResult<()> {
        if bit_size > 64 {
            return Err(StructuredError::Diagnostics(vec![
                Diagnostic::new(DiagnosticCode::WasmError, "bit-string integer segment is too large")
                    .with_label(Label::primary(subject.root.span, "bit-string pattern here")),
            ]));
        }
        for bit in 0..bit_size {
            self.bit_array_get_const_bit_subject(subject, offset + bit, out)?;
            let shift = bit_size - bit - 1;
            out.push(Instruction::I32Const(if shift < 64 && ((value >> shift) & 1) == 1 {
                1
            } else {
                0
            }));
            out.push(Instruction::I32Eq);
            if bit > 0 {
                out.push(Instruction::I32And);
            }
        }
        Ok(())
    }

    fn bind_bit_string_pattern_subject(
        &mut self, subject: &PatternSubject<'_>, segments: &[ir::BitStringPatternSegment], out: &mut Vec<Instruction>,
    ) -> StructuredResult<()> {
        self.validate_bit_string_pattern_segments(segments, subject.root.span)?;
        let mut offset = 0;
        for segment in segments {
            if let Some(local) = segment.binding {
                match segment.type_ {
                    ir::BitSegmentType::Binary => {
                        self.extract_bit_string_binary_segment(subject, offset, local, out)?
                    }
                    _ => self.extract_bit_string_integer_segment(
                        subject,
                        offset,
                        segment.bit_size.unwrap_or(8),
                        local,
                        out,
                    )?,
                }
            }
            offset += segment.bit_size.unwrap_or(0);
        }
        Ok(())
    }

    fn extract_bit_string_integer_segment(
        &mut self, subject: &PatternSubject<'_>, offset: u32, bit_size: u32, local: ir::LocalId,
        out: &mut Vec<Instruction>,
    ) -> StructuredResult<()> {
        let value = self.bit_value_local.ok_or(StructuredError::Unsupported)?;
        out.push(Instruction::I64Const(0));
        out.push(Instruction::LocalSet { local: value, type_: ValueType::I64 });
        for bit in 0..bit_size.min(64) {
            out.push(Instruction::LocalGet { local: value, type_: ValueType::I64 });
            out.push(Instruction::I64Const(2));
            out.push(Instruction::I64Mul);
            self.bit_array_get_const_bit_subject(subject, offset + bit, out)?;
            out.push(Instruction::I64ExtendI32U);
            out.push(Instruction::I64Add);
            out.push(Instruction::LocalSet { local: value, type_: ValueType::I64 });
        }
        out.push(Instruction::LocalGet { local: value, type_: ValueType::I64 });
        out.push(Instruction::LocalSet { local: self.local(local, subject.root.span)?, type_: ValueType::I64 });
        Ok(())
    }

    fn extract_bit_string_binary_segment(
        &mut self, subject: &PatternSubject<'_>, offset: u32, local: ir::LocalId, out: &mut Vec<Instruction>,
    ) -> StructuredResult<()> {
        if offset % 8 != 0 {
            return Err(StructuredError::Diagnostics(vec![Diagnostic::new(
                DiagnosticCode::WasmError,
                "structured binary bit-string binding requires a byte-aligned offset",
            )]));
        }
        let bit_len = self.scratch_local.ok_or(StructuredError::Unsupported)?;
        let ptr = self.alloc_local.ok_or(StructuredError::Unsupported)?;
        let i = self.bit_i_local.ok_or(StructuredError::Unsupported)?;
        self.subject_pointer(subject, out)?;
        out.push(Instruction::I32Load(mem_arg(self.ensure_memory(), 4, 2)));
        out.push(Instruction::I32Const(offset as i32));
        out.push(Instruction::I32Sub);
        out.push(Instruction::LocalSet { local: bit_len, type_: ValueType::I32 });
        out.push(Instruction::I32Const(8));
        out.push(Instruction::LocalGet { local: bit_len, type_: ValueType::I32 });
        out.push(Instruction::I32Const(3));
        out.push(Instruction::I32ShrU);
        out.push(Instruction::I32Add);
        self.allocate_dynamic(out)?;
        out.push(Instruction::LocalTee { local: ptr, type_: ValueType::I32 });
        out.push(Instruction::I32Const(u32::from(runtime::ObjectTag::BitArray) as i32));
        out.push(Instruction::I32Store(mem_arg(self.ensure_memory(), 0, 2)));
        out.push(Instruction::LocalGet { local: ptr, type_: ValueType::I32 });
        out.push(Instruction::LocalGet { local: bit_len, type_: ValueType::I32 });
        out.push(Instruction::I32Store(mem_arg(self.ensure_memory(), 4, 2)));
        out.push(Instruction::I32Const(0));
        out.push(Instruction::LocalSet { local: i, type_: ValueType::I32 });
        let mut copy_body = vec![
            Instruction::LocalGet { local: i, type_: ValueType::I32 },
            Instruction::LocalGet { local: bit_len, type_: ValueType::I32 },
            Instruction::I32Const(3),
            Instruction::I32ShrU,
            Instruction::I32GeS,
            Instruction::BrIf { depth: 1, results: Vec::new() },
            Instruction::LocalGet { local: ptr, type_: ValueType::I32 },
            Instruction::I32Const(8),
            Instruction::I32Add,
            Instruction::LocalGet { local: i, type_: ValueType::I32 },
            Instruction::I32Add,
        ];
        self.subject_pointer(subject, &mut copy_body)?;
        copy_body.push(Instruction::I32Const((8 + offset / 8) as i32));
        copy_body.push(Instruction::I32Add);
        copy_body.push(Instruction::LocalGet { local: i, type_: ValueType::I32 });
        copy_body.push(Instruction::I32Add);
        copy_body.push(Instruction::I32Load8U(mem_arg(self.ensure_memory(), 0, 0)));
        copy_body.push(Instruction::I32Store8(mem_arg(self.ensure_memory(), 0, 0)));
        copy_body.push(Instruction::LocalGet { local: i, type_: ValueType::I32 });
        copy_body.push(Instruction::I32Const(1));
        copy_body.push(Instruction::I32Add);
        copy_body.push(Instruction::LocalSet { local: i, type_: ValueType::I32 });
        copy_body.push(Instruction::Br { depth: 0, results: Vec::new() });
        out.push(Instruction::Block {
            type_: BlockType::empty(),
            body: vec![Instruction::Loop { type_: BlockType::empty(), body: copy_body }],
        });
        out.push(Instruction::LocalGet { local: ptr, type_: ValueType::I32 });
        out.push(Instruction::LocalSet { local: self.local(local, subject.root.span)?, type_: ValueType::I32 });
        Ok(())
    }

    fn bit_array_get_const_bit_subject(
        &mut self, subject: &PatternSubject<'_>, index: u32, out: &mut Vec<Instruction>,
    ) -> StructuredResult<()> {
        let ptr = self.scratch_local.ok_or(StructuredError::Unsupported)?;
        self.subject_pointer(subject, out)?;
        out.push(Instruction::LocalSet { local: ptr, type_: ValueType::I32 });
        out.push(Instruction::LocalGet { local: ptr, type_: ValueType::I32 });
        out.push(Instruction::I32Const(8 + (index / 8) as i32));
        out.push(Instruction::I32Add);
        out.push(Instruction::I32Load8U(mem_arg(self.ensure_memory(), 0, 0)));
        out.push(Instruction::I32Const(7 - (index % 8) as i32));
        out.push(Instruction::I32ShrU);
        out.push(Instruction::I32Const(1));
        out.push(Instruction::I32And);
        Ok(())
    }

    fn slot_address(&mut self, subject: &PatternSubject<'_>, out: &mut Vec<Instruction>) -> StructuredResult<()> {
        let Some((last, parents)) = subject.path.split_last() else {
            self.expression(subject.root, out)?;
            return Ok(());
        };
        self.expression(subject.root, out)?;
        for offset in parents {
            out.push(Instruction::I32Load(mem_arg(self.ensure_memory(), *offset, 2)));
        }
        out.push(Instruction::I32Const(*last as i32));
        out.push(Instruction::I32Add);
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
                LiteralKind::Float => literal
                    .source
                    .parse::<f64>()
                    .map(f64::to_bits)
                    .map_err(|_| StructuredError::Unsupported),
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

    fn ensure_heap_global(&mut self) -> GlobalId {
        if let Some(global) = self.heap_global {
            return global;
        }
        let global = self.module.push_global(Global {
            type_: ValueType::I32,
            mutable: true,
            init: vec![Instruction::I32Const(self.config.heap_start as i32)],
        });
        self.heap_global = Some(global);
        global
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

fn literal_type(literal: &ir::Literal) -> Type {
    match literal.kind {
        LiteralKind::Int => Type::Int,
        LiteralKind::Float => Type::Float,
        LiteralKind::Bool => Type::Bool,
        LiteralKind::String => Type::String,
        LiteralKind::Nil => Type::Nil,
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

fn needs_scratch(function: &ir::Function) -> bool {
    block_needs_scratch(&function.body)
}

fn needs_allocation(function: &ir::Function) -> bool {
    block_needs_allocation(&function.body)
}

fn needs_string_concat(function: &ir::Function) -> bool {
    block_needs_string_concat(&function.body)
}

fn needs_bit_string_pattern(block: &ir::Block) -> bool {
    block.instructions.iter().any(|instruction| match instruction {
        ir::Instruction::Evaluate { expression, .. } | ir::Instruction::LocalSet { value: expression, .. } => {
            expression_has_bit_string_pattern(expression)
        }
        ir::Instruction::AssertMatch { pattern, .. } => pattern_has_bit_string(pattern),
    }) || expression_has_bit_string_pattern(&block.result)
}

fn pattern_has_bit_string(pattern: &ir::IrPattern) -> bool {
    match pattern {
        ir::IrPattern::BitString(_) => true,
        ir::IrPattern::Alias { pattern, .. } => pattern_has_bit_string(pattern),
        ir::IrPattern::Tuple(elements) => elements.iter().any(pattern_has_bit_string),
        ir::IrPattern::List { elements, .. } => elements.iter().any(pattern_has_bit_string),
        ir::IrPattern::Constructor { arguments, .. } => arguments
            .iter()
            .any(|argument| pattern_has_bit_string(&argument.pattern)),
        ir::IrPattern::Discard | ir::IrPattern::Binding(_) | ir::IrPattern::Literal(_) => false,
    }
}

fn expression_has_bit_string_pattern(expression: &ir::Expression) -> bool {
    match &expression.kind {
        ExpressionKind::Branch(branch) => branch.clauses.iter().any(|clause| {
            clause.patterns.iter().any(pattern_has_bit_string) || expression_has_bit_string_pattern(&clause.body)
        }),
        _ => expression.children().any(expression_has_bit_string_pattern),
    }
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

fn block_needs_string_concat(block: &ir::Block) -> bool {
    block.instructions.iter().any(|instruction| match instruction {
        ir::Instruction::Evaluate { expression, .. } | ir::Instruction::LocalSet { value: expression, .. } => {
            expression_needs_string_concat(expression)
        }
        ir::Instruction::AssertMatch { value, .. } => expression_needs_string_concat(value),
    }) || expression_needs_string_concat(&block.result)
}

fn expression_needs_string_concat(expression: &ir::Expression) -> bool {
    matches!(&expression.kind, ExpressionKind::DirectCall(call) if matches!(call.function.as_str(), "__op_string_concat" | "__stdlib_gleam_string_append"))
        || expression.children().any(expression_needs_string_concat)
}

fn block_needs_allocation(block: &ir::Block) -> bool {
    needs_bit_string_pattern(block)
        || block.instructions.iter().any(|instruction| match instruction {
            ir::Instruction::Evaluate { expression, .. } | ir::Instruction::LocalSet { value: expression, .. } => {
                expression_needs_allocation(expression)
            }
            ir::Instruction::AssertMatch { value, .. } => expression_needs_allocation(value),
        })
        || expression_needs_allocation(&block.result)
}

fn expression_needs_allocation(expression: &ir::Expression) -> bool {
    matches!(
        expression.kind,
        ExpressionKind::AnonymousFunction(_)
            | ExpressionKind::ListCons { .. }
            | ExpressionKind::RecordUpdate { .. }
            | ExpressionKind::Memory(_)
    ) || matches!(&expression.kind, ExpressionKind::DirectCall(call) if matches!(call.function.as_str(), "__op_string_concat" | "__stdlib_gleam_string_append"))
        || matches!(&expression.kind, ExpressionKind::Constructor(constructor) if !constructor.arguments.iter().all(|arg| matches!(arg.kind, ExpressionKind::Literal(_) | ExpressionKind::Tuple(_) | ExpressionKind::List(_) | ExpressionKind::Record(_) | ExpressionKind::Constructor(_) | ExpressionKind::FunctionValue(_) | ExpressionKind::BitArray(_))))
        || expression.children().any(expression_needs_allocation)
}

fn block_needs_scratch(block: &ir::Block) -> bool {
    needs_bit_string_pattern(block)
        || block.instructions.iter().any(|instruction| match instruction {
            ir::Instruction::Evaluate { expression, .. } | ir::Instruction::LocalSet { value: expression, .. } => {
                expression_needs_scratch(expression)
            }
            ir::Instruction::AssertMatch { value, .. } => expression_needs_scratch(value),
        })
        || expression_needs_scratch(&block.result)
}

fn expression_needs_scratch(expression: &ir::Expression) -> bool {
    match &expression.kind {
        ExpressionKind::IndirectCall(_) | ExpressionKind::RecordUpdate { .. } => true,
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
        _ => expression.children().any(expression_needs_scratch),
    }
}
