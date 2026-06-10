mod builder;
mod helpers;

use std::{
    collections::{HashMap, HashSet},
    fmt::Write,
};

use crate::diagnostic::{Diagnostic, DiagnosticCode, Diagnostics, Label};
use crate::ir::{self, ExpressionKind, Instruction};
use crate::{
    ClosureConstants, ast::LiteralKind, runtime, stdlib::STDLIB_IO_HOST_MODULE, target::CompileTarget, types::Type,
};

/// WebAssembly output from the backend.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WasmModule {
    pub wat: String,
    pub bytes: Vec<u8>,
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct EmitOptions {
    pub target: WasmTarget,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum WasmTarget {
    #[default]
    Wasmtime,
    Browser,
    Wasi,
}

impl From<CompileTarget> for EmitOptions {
    fn from(target: CompileTarget) -> Self {
        Self { target: WasmTarget::from(target) }
    }
}

impl From<CompileTarget> for WasmTarget {
    fn from(target: CompileTarget) -> Self {
        match target {
            CompileTarget::Wasmtime => Self::Wasmtime,
            CompileTarget::Browser => Self::Browser,
            CompileTarget::Wasi => Self::Wasi,
            CompileTarget::Wasm => Self::Wasmtime,
        }
    }
}

impl WasmTarget {
    fn name(self) -> &'static str {
        match self {
            Self::Wasmtime => "wasmtime",
            Self::Browser => "browser",
            Self::Wasi => "wasi",
        }
    }

    fn host_module(self) -> &'static str {
        match self {
            Self::Wasmtime => "env",
            Self::Browser => "browser",
            Self::Wasi => "wasi_snapshot_preview1",
        }
    }
}

pub fn emit(module: &ir::Module) -> Result<WasmModule, Diagnostics> {
    emit_with_options(module, EmitOptions::default())
}

pub fn emit_with_options(module: &ir::Module, options: EmitOptions) -> Result<WasmModule, Diagnostics> {
    let wat = emit_wat_with_options(module, options)?;
    let bytes = wat::parse_str(&wat).map_err(|error| {
        vec![Diagnostic::new(
            DiagnosticCode::WasmError,
            format!("could not assemble WAT: {error}"),
        )]
    })?;

    Ok(WasmModule { wat, bytes })
}

pub fn emit_wat(module: &ir::Module) -> Result<String, Diagnostics> {
    emit_wat_with_options(module, EmitOptions::default())
}

pub fn emit_wat_with_options(module: &ir::Module, options: EmitOptions) -> Result<String, Diagnostics> {
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
                        function
                            .params
                            .iter()
                            .skip(function.closure_captures.len())
                            .map(|param| param.type_.clone())
                            .collect(),
                        function.return_type.clone(),
                    ),
                )
            })
            .collect(),
        closure_captures: module
            .functions
            .iter()
            .map(|function| (function.name.clone(), function.closure_captures.clone()))
            .collect(),
        current: CurrentEmission::default(),
        debug_imports: HashSet::new(),
        options,
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
    wat.push_str(&emitter.imports);
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

#[derive(Clone, Default)]
struct CurrentEmission {
    scratch: Option<String>,
    capture_slots: Option<String>,
    record_update_source: Option<String>,
    record_update_slots: Option<String>,
    debug_i32: Option<String>,
    debug_i64: Option<String>,
    debug_f64: Option<String>,
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
    closure_captures: HashMap<String, Vec<Type>>,
    current: CurrentEmission,
    debug_imports: HashSet<DebugImport>,
    options: EmitOptions,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ConcreteHostImport {
    module: String,
    name: String,
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

    fn wasm_type(self) -> &'static str {
        match self {
            Self::Bool | Self::Value => "i32",
            Self::I64 => "i64",
            Self::F64 => "f64",
        }
    }
}

impl Emitter {
    fn constant(&mut self, constant: &ir::Constant) {
        if let ir::ConstantValue::Literal(ir::Literal { kind: LiteralKind::String, source }) = &constant.value {
            self.static_string(source);
        }
    }

    fn concrete_import(&mut self, function: &ir::Function) -> Option<ConcreteHostImport> {
        match &function.abi.boundary {
            ir::CallBoundary::HostImport { module, name } if module == STDLIB_IO_HOST_MODULE => {
                self.stdlib_io_import(name, function.span)
            }
            ir::CallBoundary::HostImport { module, name } if module == self.options.target.host_module() => {
                Some(ConcreteHostImport { module: module.clone(), name: name.clone() })
            }
            ir::CallBoundary::HostImport { module, .. } => {
                self.unsupported_target_import(module, function.span, &function.name);
                None
            }
            ir::CallBoundary::ModuleImport { module } => {
                Some(ConcreteHostImport { module: module.clone(), name: function.name.clone() })
            }
            ir::CallBoundary::Internal | ir::CallBoundary::ModuleExport => None,
        }
    }

    fn stdlib_io_import(&mut self, name: &str, span: crate::source::Span) -> Option<ConcreteHostImport> {
        match self.options.target {
            WasmTarget::Wasmtime => Some(ConcreteHostImport { module: "env".into(), name: name.into() }),
            WasmTarget::Browser => Some(ConcreteHostImport { module: "browser".into(), name: name.into() }),
            WasmTarget::Wasi => {
                self.unsupported_stdlib_host_call("gleam/io", name, span);
                None
            }
        }
    }

    fn import_function(&mut self, function: &ir::Function) {
        let Some(import) = self.concrete_import(function) else {
            return;
        };
        if !self.validate_host_abi(function) {
            return;
        }

        write!(
            self.imports,
            "  (import \"{}\" \"{}\" (func ${}",
            import.module, import.name, function.name
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

        let previous_current = self.current.clone();
        if block_contains_indirect_call(&function.body) {
            writeln!(self.functions, "    (local $__callee i32)").expect("write WAT");
            self.current.scratch = Some("__callee".into());
        }
        if block_contains_anonymous_function(&function.body) {
            writeln!(self.functions, "    (local $__capture_slots i32)").expect("write WAT");
            self.current.capture_slots = Some("__capture_slots".into());
        }
        if block_contains_record_update(&function.body) || block_contains_constructor(&function.body) {
            writeln!(self.functions, "    (local $__record_update_slots i32)").expect("write WAT");
            self.current.record_update_slots = Some("__record_update_slots".into());
        }
        if block_contains_record_update(&function.body) {
            writeln!(self.functions, "    (local $__record_update_source i32)").expect("write WAT");
            self.current.record_update_source = Some("__record_update_source".into());
        }
        let debug_locals = block_debug_local_types(&function.body);
        if debug_locals.i32 {
            writeln!(self.functions, "    (local $__debug_i32 i32)").expect("write WAT");
            self.current.debug_i32 = Some("__debug_i32".into());
        }
        if debug_locals.i64 {
            writeln!(self.functions, "    (local $__debug_i64 i64)").expect("write WAT");
            self.current.debug_i64 = Some("__debug_i64".into());
        }
        if debug_locals.f64 {
            writeln!(self.functions, "    (local $__debug_f64 f64)").expect("write WAT");
            self.current.debug_f64 = Some("__debug_f64".into());
        }

        self.block(&function.body);
        self.current = previous_current;
        self.functions.push_str("  )\n");
        self.export_adapters(function);
    }

    fn export_adapters(&mut self, function: &ir::Function) {
        if !matches!(function.abi.boundary, ir::CallBoundary::ModuleExport) {
            return;
        }
        if function.params.is_empty() && function.return_type == Type::String {
            self.uses_runtime = true;
            writeln!(
                self.functions,
                "  (func ${}__data (export \"{}__data\") (result i32)",
                function.name, function.name
            )
            .expect("write WAT");
            writeln!(self.functions, "    call ${}", function.name).expect("write WAT");
            writeln!(self.functions, "    call $__string_data").expect("write WAT");
            self.functions.push_str("  )\n");
            writeln!(
                self.functions,
                "  (func ${}__len (export \"{}__len\") (result i32)",
                function.name, function.name
            )
            .expect("write WAT");
            writeln!(self.functions, "    call ${}", function.name).expect("write WAT");
            writeln!(self.functions, "    call $__string_len").expect("write WAT");
            self.functions.push_str("  )\n");
        }
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
            ExpressionKind::DirectCall(call) => self.direct_call(call),
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
            ExpressionKind::Constructor(constructor) => self.constructor_value(constructor, expression.span),
            ExpressionKind::FunctionValue(function) => {
                let pointer = self.static_closure(self.function_id(&function.name), &[]);
                writeln!(self.functions, "    i32.const {pointer}").expect("write WAT");
            }
            ExpressionKind::AnonymousFunction(function) => self.closure_allocation(function, expression.span),
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
            ExpressionKind::RuntimeEquality { left, right } => self.runtime_equality(left, right),
            ExpressionKind::FieldAccess { record, .. } => self.managed_field_load(record, 0, &expression.type_),
            ExpressionKind::TupleElement { tuple, index } => self.managed_field_load(tuple, *index, &expression.type_),
            ExpressionKind::Failure(failure) => self.failure(failure),
            ExpressionKind::Memory(operation) => self.memory_operation(operation),
            ExpressionKind::IndirectCall(call) => self.indirect_call(call, expression.span),
            ExpressionKind::Compare { op, left, right } => self.compare(*op, left, right),
            ExpressionKind::Pipeline(pipeline) => self.pipeline(pipeline),
            ExpressionKind::Use(_) => self.unsupported_residual_use(expression.span),
            ExpressionKind::BitStringDeconstruct { bit_array, .. } => {
                self.managed_tag_test(bit_array, runtime::ObjectTag::BitArray, None)
            }
            ExpressionKind::ListDeconstruct { list, head, tail } => self.list_deconstruct(list, *head, *tail),
            ExpressionKind::RecordUpdate { record, constructor, fields } => {
                self.record_update(record, constructor, fields, &expression.type_, expression.span)
            }
        }
    }

    fn direct_call(&mut self, call: &ir::DirectCall) {
        match call.function.as_str() {
            "__op_add" | "__op_subtract" | "__op_multiply" | "__op_divide" | "__op_remainder" => {
                self.binary_scalar_op(call, integer_operator_instruction(&call.function));
            }
            "__op_float_add" | "__op_float_subtract" | "__op_float_multiply" | "__op_float_divide" => {
                self.binary_scalar_op(call, float_operator_instruction(&call.function));
            }
            "__op_not" => {
                self.expression(&call.arguments[0].value);
                writeln!(self.functions, "    i32.eqz").expect("write WAT");
            }
            "__op_negate" => {
                writeln!(self.functions, "    i64.const 0").expect("write WAT");
                self.expression(&call.arguments[0].value);
                writeln!(self.functions, "    i64.sub").expect("write WAT");
            }
            "__op_and" => self.short_circuit_bool(call, false),
            "__op_or" => self.short_circuit_bool(call, true),
            "__op_string_concat" | "__stdlib_gleam_string_append" => {
                self.expression(&call.arguments[0].value);
                self.expression(&call.arguments[1].value);
                writeln!(self.functions, "    call $__string_concat").expect("write WAT");
                self.uses_runtime = true;
            }
            "__stdlib_gleam_string_concat" => {
                self.expression(&call.arguments[0].value);
                writeln!(self.functions, "    call $__string_concat_list").expect("write WAT");
                self.uses_runtime = true;
            }
            "__stdlib_gleam_string_length" => {
                self.expression(&call.arguments[0].value);
                writeln!(self.functions, "    call $__string_len").expect("write WAT");
                writeln!(self.functions, "    i64.extend_i32_u").expect("write WAT");
                self.uses_runtime = true;
            }
            "__stdlib_gleam_string_is_empty" => {
                self.expression(&call.arguments[0].value);
                writeln!(self.functions, "    call $__string_len").expect("write WAT");
                writeln!(self.functions, "    i32.eqz").expect("write WAT");
                self.uses_runtime = true;
            }
            "__stdlib_gleam_int_to_string" => {
                self.expression(&call.arguments[0].value);
                writeln!(self.functions, "    call $__int_to_string").expect("write WAT");
                self.uses_runtime = true;
            }
            "__stdlib_gleam_list_length" => {
                self.expression(&call.arguments[0].value);
                writeln!(self.functions, "    call $__list_length").expect("write WAT");
                self.uses_runtime = true;
            }
            "__stdlib_gleam_list_reverse" => {
                self.expression(&call.arguments[0].value);
                writeln!(self.functions, "    call $__list_reverse").expect("write WAT");
                self.uses_runtime = true;
            }
            "__stdlib_gleam_bit_array_append" => {
                self.expression(&call.arguments[0].value);
                self.expression(&call.arguments[1].value);
                writeln!(self.functions, "    call $__bit_array_append").expect("write WAT");
                self.uses_runtime = true;
            }
            "__stdlib_gleam_bit_array_concat" => {
                self.expression(&call.arguments[0].value);
                writeln!(self.functions, "    call $__bit_array_concat_list").expect("write WAT");
                self.uses_runtime = true;
            }
            "__stdlib_gleam_bit_array_bit_size" => {
                self.expression(&call.arguments[0].value);
                writeln!(self.functions, "    call $__bit_array_len").expect("write WAT");
                writeln!(self.functions, "    i64.extend_i32_u").expect("write WAT");
                self.uses_runtime = true;
            }
            "__stdlib_gleam_bit_array_byte_size" => {
                self.expression(&call.arguments[0].value);
                writeln!(self.functions, "    call $__bit_array_len").expect("write WAT");
                writeln!(self.functions, "    call $__bit_array_payload_len").expect("write WAT");
                writeln!(self.functions, "    i64.extend_i32_u").expect("write WAT");
                self.uses_runtime = true;
            }
            "__stdlib_gleam_bit_array_is_empty" => {
                self.expression(&call.arguments[0].value);
                writeln!(self.functions, "    call $__bit_array_len").expect("write WAT");
                writeln!(self.functions, "    i32.eqz").expect("write WAT");
                self.uses_runtime = true;
            }
            "__stdlib_gleam_bit_array_starts_with" => {
                self.expression(&call.arguments[0].value);
                writeln!(self.functions, "    i32.const 0").expect("write WAT");
                self.expression(&call.arguments[1].value);
                writeln!(self.functions, "    call $__bit_array_match").expect("write WAT");
                self.uses_runtime = true;
            }
            "__stdlib_gleam_bool_to_string" => self.stdlib_bool_to_string(call),
            "__stdlib_gleam_bool_negate" => {
                self.expression(&call.arguments[0].value);
                writeln!(self.functions, "    i32.eqz").expect("write WAT");
            }
            "__stdlib_gleam_bool_compare" => {
                self.expression(&call.arguments[0].value);
                self.expression(&call.arguments[1].value);
                writeln!(self.functions, "    i32.sub").expect("write WAT");
                self.order_from_compare_result();
            }
            "__stdlib_gleam_dict_new" => {
                writeln!(self.functions, "    call $__dict_new").expect("write WAT");
                self.uses_runtime = true;
            }
            "__stdlib_gleam_dict_size" => {
                self.expression(&call.arguments[0].value);
                writeln!(self.functions, "    call $__dict_size").expect("write WAT");
                self.uses_runtime = true;
            }
            "__stdlib_gleam_dict_is_empty" => {
                self.expression(&call.arguments[0].value);
                writeln!(self.functions, "    call $__dict_is_empty").expect("write WAT");
                self.uses_runtime = true;
            }
            "__stdlib_gleam_dict_insert" => self.stdlib_dict_insert(call),
            "__stdlib_gleam_dict_get" => self.stdlib_dict_get(call),
            "__stdlib_gleam_dict_has_key" => self.stdlib_dict_has_key(call),
            "__stdlib_gleam_dict_delete" => self.stdlib_dict_delete(call),
            "__stdlib_gleam_float_compare" => {
                self.expression(&call.arguments[0].value);
                self.expression(&call.arguments[1].value);
                writeln!(self.functions, "    call $__compare_f64").expect("write WAT");
                self.order_from_compare_result();
                self.uses_runtime = true;
            }
            "__stdlib_gleam_float_to_string" => {
                self.expression(&call.arguments[0].value);
                writeln!(self.functions, "    call $__float_to_string").expect("write WAT");
                self.uses_runtime = true;
            }
            "__stdlib_gleam_float_max" => {
                self.expression(&call.arguments[0].value);
                self.expression(&call.arguments[1].value);
                writeln!(self.functions, "    f64.max").expect("write WAT");
            }
            "__stdlib_gleam_float_min" => {
                self.expression(&call.arguments[0].value);
                self.expression(&call.arguments[1].value);
                writeln!(self.functions, "    f64.min").expect("write WAT");
            }
            "__stdlib_gleam_float_negate" => {
                writeln!(self.functions, "    f64.const -0").expect("write WAT");
                self.expression(&call.arguments[0].value);
                writeln!(self.functions, "    f64.sub").expect("write WAT");
            }
            "__stdlib_gleam_function_identity" | "__stdlib_gleam_function_constant" => {
                self.expression(&call.arguments[0].value);
            }
            "__stdlib_gleam_io_debug" => self.stdlib_io_debug(call),
            _ => {
                for argument in &call.arguments {
                    self.expression(&argument.value);
                }
                writeln!(self.functions, "    call ${}", call.function).expect("write WAT");
            }
        }
    }

    fn stdlib_bool_to_string(&mut self, call: &ir::DirectCall) {
        let true_ptr = self.static_string("True");
        let false_ptr = self.static_string("False");
        self.expression(&call.arguments[0].value);
        writeln!(self.functions, "    if (result i32)").expect("write WAT");
        writeln!(self.functions, "      i32.const {true_ptr}").expect("write WAT");
        writeln!(self.functions, "    else").expect("write WAT");
        writeln!(self.functions, "      i32.const {false_ptr}").expect("write WAT");
        writeln!(self.functions, "    end").expect("write WAT");
    }

    fn order_from_compare_result(&mut self) {
        writeln!(self.functions, "    call $__order_from_compare").expect("write WAT");
        self.uses_runtime = true;
    }

    fn stdlib_dict_insert(&mut self, call: &ir::DirectCall) {
        self.expression(&call.arguments[0].value);
        self.expression_slot_value(&call.arguments[1].value, &call.arguments[1].value.type_);
        self.expression_slot_value(&call.arguments[2].value, &call.arguments[2].value.type_);
        writeln!(self.functions, "    call $__dict_insert").expect("write WAT");
        self.uses_runtime = true;
    }

    fn stdlib_dict_get(&mut self, call: &ir::DirectCall) {
        self.expression(&call.arguments[0].value);
        self.expression_slot_value(&call.arguments[1].value, &call.arguments[1].value.type_);
        writeln!(self.functions, "    call $__dict_get").expect("write WAT");
        self.uses_runtime = true;
    }

    fn stdlib_dict_has_key(&mut self, call: &ir::DirectCall) {
        self.expression(&call.arguments[0].value);
        self.expression_slot_value(&call.arguments[1].value, &call.arguments[1].value.type_);
        writeln!(self.functions, "    call $__dict_has_key").expect("write WAT");
        self.uses_runtime = true;
    }

    fn stdlib_dict_delete(&mut self, call: &ir::DirectCall) {
        self.expression(&call.arguments[0].value);
        self.expression_slot_value(&call.arguments[1].value, &call.arguments[1].value.type_);
        writeln!(self.functions, "    call $__dict_delete").expect("write WAT");
        self.uses_runtime = true;
    }

    fn stdlib_io_debug(&mut self, call: &ir::DirectCall) {
        let value = &call.arguments[0].value;
        match value.type_ {
            Type::Int => self.debug_scalar(value, DebugImport::I64),
            Type::Float => self.debug_scalar(value, DebugImport::F64),
            Type::Bool => self.debug_scalar(value, DebugImport::Bool),
            Type::String
            | Type::BitArray
            | Type::Tuple(_)
            | Type::List(_)
            | Type::Record { .. }
            | Type::Custom { .. }
            | Type::Opaque { .. }
            | Type::Function { .. } => self.debug_scalar(value, DebugImport::Value),
            Type::Nil => {}
            Type::Generic(_) => self.unsupported_type(&value.type_, value.span),
        }
    }

    fn debug_scalar(&mut self, value: &ir::Expression, import: DebugImport) {
        let Some(local) = self.debug_local(import, value.span) else {
            return;
        };
        self.ensure_debug_import(import, value.span);
        self.expression(value);
        writeln!(self.functions, "    local.tee ${local}").expect("write WAT");
        writeln!(self.functions, "    call ${}", import.name()).expect("write WAT");
        writeln!(self.functions, "    local.get ${local}").expect("write WAT");
    }

    fn debug_local(&mut self, import: DebugImport, span: crate::source::Span) -> Option<String> {
        let local = match import {
            DebugImport::Bool | DebugImport::Value => &self.current.debug_i32,
            DebugImport::I64 => &self.current.debug_i64,
            DebugImport::F64 => &self.current.debug_f64,
        };
        if let Some(local) = local {
            return Some(local.clone());
        }
        self.diagnostics.push(
            Diagnostic::new(DiagnosticCode::WasmError, "debug intrinsic needs a temporary local")
                .with_label(Label::primary(span, "debug value here")),
        );
        None
    }

    fn ensure_debug_import(&mut self, import: DebugImport, span: crate::source::Span) {
        if self.options.target == WasmTarget::Wasi {
            self.unsupported_stdlib_host_call("gleam/io", "debug", span);
            return;
        }
        if !self.debug_imports.insert(import) {
            return;
        }
        let module = self.options.target.host_module();
        writeln!(
            self.imports,
            "  (import \"{module}\" \"{}\" (func ${} (param {})))",
            import.name(),
            import.name(),
            import.wasm_type(),
        )
        .expect("write WAT");
    }

    fn binary_scalar_op(&mut self, call: &ir::DirectCall, instruction: &'static str) {
        self.expression(&call.arguments[0].value);
        self.expression(&call.arguments[1].value);
        writeln!(self.functions, "    {instruction}").expect("write WAT");
    }

    fn short_circuit_bool(&mut self, call: &ir::DirectCall, is_or: bool) {
        self.expression(&call.arguments[0].value);
        if is_or {
            writeln!(self.functions, "    if (result i32)").expect("write WAT");
            writeln!(self.functions, "      i32.const 1").expect("write WAT");
            writeln!(self.functions, "    else").expect("write WAT");
            self.expression(&call.arguments[1].value);
            writeln!(self.functions, "    end").expect("write WAT");
        } else {
            writeln!(self.functions, "    if (result i32)").expect("write WAT");
            self.expression(&call.arguments[1].value);
            writeln!(self.functions, "    else").expect("write WAT");
            writeln!(self.functions, "      i32.const 0").expect("write WAT");
            writeln!(self.functions, "    end").expect("write WAT");
        }
    }

    fn runtime_equality(&mut self, left: &ir::Expression, right: &ir::Expression) {
        self.expression(left);
        self.expression(right);
        match left.type_ {
            Type::Int => writeln!(self.functions, "    i64.eq").expect("write WAT"),
            Type::Float => writeln!(self.functions, "    f64.eq").expect("write WAT"),
            Type::Bool => writeln!(self.functions, "    i32.eq").expect("write WAT"),
            Type::String
            | Type::BitArray
            | Type::Tuple(_)
            | Type::List(_)
            | Type::Record { .. }
            | Type::Custom { .. }
            | Type::Opaque { .. }
            | Type::Function { .. } => {
                writeln!(self.functions, "    call $__equal_value").expect("write WAT");
                self.uses_runtime = true;
            }
            Type::Nil | Type::Generic(_) => writeln!(self.functions, "    i32.const 1").expect("write WAT"),
        }
    }

    fn compare(&mut self, op: ir::ComparisonOp, left: &ir::Expression, right: &ir::Expression) {
        match op {
            ir::ComparisonOp::Equal | ir::ComparisonOp::NotEqual => self.runtime_equality(left, right),
            ir::ComparisonOp::Less
            | ir::ComparisonOp::LessEqual
            | ir::ComparisonOp::Greater
            | ir::ComparisonOp::GreaterEqual => {
                self.expression(left);
                self.expression(right);
                match left.type_ {
                    Type::Int => {
                        writeln!(self.functions, "    {}", comparison_instruction(op, "i64")).expect("write WAT")
                    }
                    Type::Float => {
                        writeln!(self.functions, "    {}", comparison_instruction(op, "f64")).expect("write WAT")
                    }
                    Type::Bool => {
                        writeln!(self.functions, "    {}", comparison_instruction(op, "i32")).expect("write WAT")
                    }
                    Type::String => {
                        writeln!(self.functions, "    call $__string_compare").expect("write WAT");
                        writeln!(self.functions, "    i32.const 0").expect("write WAT");
                        writeln!(self.functions, "    {}", comparison_instruction(op, "i32")).expect("write WAT");
                        self.uses_runtime = true;
                    }
                    _ => self.diagnostics.push(
                        Diagnostic::new(DiagnosticCode::WasmError, "comparison type is not supported")
                            .with_label(Label::primary(left.span, "unsupported comparison here")),
                    ),
                }
            }
        }
        if matches!(op, ir::ComparisonOp::NotEqual) {
            writeln!(self.functions, "    i32.eqz").expect("write WAT");
        }
    }

    fn pipeline(&mut self, pipeline: &ir::PipelineLowering) {
        match &pipeline.call.kind {
            ExpressionKind::DirectCall(call) => {
                let mut call = call.clone();
                call.arguments.insert(
                    pipeline.inserted_argument,
                    ir::CallArgument { label: None, value: pipeline.input.as_ref().clone(), span: pipeline.input.span },
                );
                self.direct_call(&call);
            }
            _ => self.expression(&pipeline.call),
        }
    }

    fn list_deconstruct(&mut self, list: &ir::Expression, head: ir::LocalId, tail: ir::LocalId) {
        self.expression(list);
        writeln!(self.functions, "    call $__list_head").expect("write WAT");
        writeln!(self.functions, "    local.set ${}", head.0).expect("write WAT");
        self.expression(list);
        writeln!(self.functions, "    call $__list_tail").expect("write WAT");
        writeln!(self.functions, "    local.set ${}", tail.0).expect("write WAT");
        self.uses_runtime = true;
    }

    fn failure(&mut self, failure: &ir::FailurePath) {
        match failure.reason {
            ir::FailureReason::AssertMatch | ir::FailureReason::BranchFallthrough => {
                writeln!(self.functions, "    call $__match_fail").expect("write WAT")
            }
            ir::FailureReason::Panic | ir::FailureReason::Todo | ir::FailureReason::Assert => {
                writeln!(self.functions, "    call $__panic").expect("write WAT")
            }
        }
        writeln!(self.functions, "    unreachable").expect("write WAT");
        self.uses_runtime = true;
    }

    fn closure_allocation(&mut self, function: &ir::AnonymousFunction, span: crate::source::Span) {
        let Some(slots) = self.current.capture_slots.clone() else {
            self.diagnostics.push(
                Diagnostic::new(
                    DiagnosticCode::WasmError,
                    "closure allocation needs a capture-slot local",
                )
                .with_label(Label::primary(span, "closure allocated here")),
            );
            writeln!(self.functions, "    i32.const 0").expect("write WAT");
            return;
        };
        let byte_len = function.captures.len() * closure_constant_usize(ClosureConstants::CaptureSlotSize);
        writeln!(self.functions, "    i32.const {byte_len}").expect("write WAT");
        writeln!(self.functions, "    call $__alloc").expect("write WAT");
        writeln!(self.functions, "    local.set ${slots}").expect("write WAT");
        for (index, capture) in function.captures.iter().enumerate() {
            writeln!(self.functions, "    local.get ${slots}").expect("write WAT");
            writeln!(
                self.functions,
                "    i32.const {}",
                index * closure_constant_usize(ClosureConstants::CaptureSlotSize)
            )
            .expect("write WAT");
            writeln!(self.functions, "    i32.add").expect("write WAT");
            writeln!(self.functions, "    local.get ${}", capture.source.0).expect("write WAT");
            match capture.type_ {
                Type::Int => writeln!(self.functions, "    i64.store").expect("write WAT"),
                Type::Float => writeln!(self.functions, "    f64.store").expect("write WAT"),
                _ => writeln!(self.functions, "    i32.store").expect("write WAT"),
            }
        }
        writeln!(self.functions, "    i32.const {}", self.function_id(&function.name)).expect("write WAT");
        writeln!(self.functions, "    i32.const {}", function.captures.len()).expect("write WAT");
        writeln!(self.functions, "    local.get ${slots}").expect("write WAT");
        writeln!(self.functions, "    call $__closure_new").expect("write WAT");
        self.uses_runtime = true;
    }

    fn indirect_call(&mut self, call: &ir::IndirectCall, span: crate::source::Span) {
        let Some(scratch) = self.current.scratch.clone() else {
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
        writeln!(
            self.functions,
            "    i32.const {}",
            u32::from(ClosureConstants::FunctionIdOffset)
        )
        .expect("write WAT");
        writeln!(self.functions, "    i32.add").expect("write WAT");
        writeln!(self.functions, "    i32.load").expect("write WAT");
        writeln!(self.functions, "    i32.const {id}").expect("write WAT");
        writeln!(self.functions, "    i32.eq").expect("write WAT");
        if result_type.is_empty() {
            writeln!(self.functions, "    if").expect("write WAT");
        } else {
            writeln!(self.functions, "    if (result {result_type})").expect("write WAT");
        }
        if let Some(captures) = self.closure_captures.get(&name).cloned() {
            for (index, type_) in captures.iter().enumerate() {
                writeln!(self.functions, "    local.get ${scratch}").expect("write WAT");
                writeln!(
                    self.functions,
                    "    i32.const {}",
                    closure_constant_usize(ClosureConstants::CapturesOffset)
                        + index * closure_constant_usize(ClosureConstants::CaptureSlotSize)
                )
                .expect("write WAT");
                writeln!(self.functions, "    i32.add").expect("write WAT");
                match type_ {
                    Type::Int => writeln!(self.functions, "    i64.load").expect("write WAT"),
                    Type::Float => writeln!(self.functions, "    f64.load").expect("write WAT"),
                    _ => writeln!(self.functions, "    i32.load").expect("write WAT"),
                }
            }
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
            self.failure(&branch.fallthrough);
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
            ir::IrPattern::Discard | ir::IrPattern::Literal(_) => {}
            ir::IrPattern::BitString(segments) => self.bind_bit_string_pattern(subject, segments),
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
                    Type::Bool => writeln!(self.functions, "    i32.eq").expect("write WAT"),
                    Type::String => {
                        writeln!(self.functions, "    call $__equal_value").expect("write WAT");
                        self.uses_runtime = true;
                    }
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
            ir::IrPattern::BitString(segments) => self.bit_string_pattern_test(subject, segments, span),
        }
    }

    fn bit_string_pattern_test(
        &mut self, subject: &ir::Expression, segments: &[ir::BitStringPatternSegment], span: crate::source::Span,
    ) {
        self.managed_tag_test(subject, runtime::ObjectTag::BitArray, None);
        if !self.validate_bit_string_pattern_segments(segments, span) {
            writeln!(self.functions, "    i32.const 0").expect("write WAT");
            return;
        }
        let fixed_bit_len = segments.iter().filter_map(|segment| segment.bit_size).sum::<u32>();
        let has_variable_tail = segments.last().is_some_and(|segment| segment.bit_size.is_none());
        self.expression(subject);
        writeln!(self.functions, "    i32.const 4").expect("write WAT");
        writeln!(self.functions, "    i32.add").expect("write WAT");
        writeln!(self.functions, "    i32.load").expect("write WAT");
        writeln!(self.functions, "    i32.const {fixed_bit_len}").expect("write WAT");
        if has_variable_tail {
            writeln!(self.functions, "    i32.ge_u").expect("write WAT");
        } else {
            writeln!(self.functions, "    i32.eq").expect("write WAT");
        }
        writeln!(self.functions, "    i32.and").expect("write WAT");

        let mut offset = 0;
        for segment in segments {
            if let Some(value) = segment.value {
                self.bit_string_integer_segment_test(subject, offset, segment.bit_size.unwrap_or(8), value, span);
                writeln!(self.functions, "    i32.and").expect("write WAT");
            }
            offset += segment.bit_size.unwrap_or(0);
        }
    }

    fn validate_bit_string_pattern_segments(
        &mut self, segments: &[ir::BitStringPatternSegment], span: crate::source::Span,
    ) -> bool {
        let mut valid = true;
        for (index, segment) in segments.iter().enumerate() {
            match segment.type_ {
                ir::BitSegmentType::Integer => {}
                ir::BitSegmentType::Binary if segment.bit_size.is_some() || index + 1 == segments.len() => {}
                _ => {
                    self.diagnostics.push(
                        Diagnostic::new(DiagnosticCode::WasmError, "unsupported bit-string pattern segment type")
                            .with_label(Label::primary(span, "unsupported segment type")),
                    );
                    valid = false;
                }
            }
        }
        valid
    }

    fn bit_string_integer_segment_test(
        &mut self, subject: &ir::Expression, offset: u32, bit_size: u32, value: u64, span: crate::source::Span,
    ) {
        if bit_size > 64 {
            self.diagnostics.push(
                Diagnostic::new(DiagnosticCode::WasmError, "bit-string integer segment is too large")
                    .with_label(Label::primary(span, "segment too large")),
            );
            writeln!(self.functions, "    i32.const 0").expect("write WAT");
            return;
        }
        let mut emitted_any = false;
        for bit in 0..bit_size {
            self.expression(subject);
            writeln!(self.functions, "    i32.const {}", offset + bit).expect("write WAT");
            writeln!(self.functions, "    call $__bit_array_get_bit").expect("write WAT");
            let shift = bit_size - bit - 1;
            writeln!(self.functions, "    i32.const {}", (value >> shift) & 1).expect("write WAT");
            writeln!(self.functions, "    i32.eq").expect("write WAT");
            if emitted_any {
                writeln!(self.functions, "    i32.and").expect("write WAT");
            }
            emitted_any = true;
        }
        if !emitted_any {
            writeln!(self.functions, "    i32.const 1").expect("write WAT");
        }
        self.uses_runtime = true;
    }

    fn bind_bit_string_pattern(&mut self, subject: &ir::Expression, segments: &[ir::BitStringPatternSegment]) {
        let mut offset = 0;
        for segment in segments {
            if let Some(local) = segment.binding {
                match segment.type_ {
                    ir::BitSegmentType::Binary => self.extract_bit_string_binary_segment(subject, offset),
                    _ => self.extract_bit_string_integer_segment(subject, offset, segment.bit_size.unwrap_or(8)),
                }
                writeln!(self.functions, "    local.set ${}", local.0).expect("write WAT");
            }
            offset += segment.bit_size.unwrap_or(0);
        }
    }

    fn extract_bit_string_integer_segment(&mut self, subject: &ir::Expression, offset: u32, bit_size: u32) {
        writeln!(self.functions, "    i64.const 0").expect("write WAT");
        for bit in 0..bit_size.min(64) {
            writeln!(self.functions, "    i64.const 1").expect("write WAT");
            writeln!(self.functions, "    i64.shl").expect("write WAT");
            self.expression(subject);
            writeln!(self.functions, "    i32.const {}", offset + bit).expect("write WAT");
            writeln!(self.functions, "    call $__bit_array_get_bit").expect("write WAT");
            writeln!(self.functions, "    i64.extend_i32_u").expect("write WAT");
            writeln!(self.functions, "    i64.or").expect("write WAT");
        }
        self.uses_runtime = true;
    }

    fn extract_bit_string_binary_segment(&mut self, subject: &ir::Expression, offset: u32) {
        self.expression(subject);
        writeln!(self.functions, "    i32.const {offset}").expect("write WAT");
        self.expression(subject);
        writeln!(self.functions, "    i32.const 4").expect("write WAT");
        writeln!(self.functions, "    i32.add").expect("write WAT");
        writeln!(self.functions, "    i32.load").expect("write WAT");
        writeln!(self.functions, "    i32.const {offset}").expect("write WAT");
        writeln!(self.functions, "    i32.sub").expect("write WAT");
        writeln!(self.functions, "    call $__bit_array_slice").expect("write WAT");
        self.uses_runtime = true;
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

    fn constructor_value(&mut self, constructor: &ir::ConstructorValue, span: crate::source::Span) {
        let Some(slots) = self.current.record_update_slots.clone() else {
            self.diagnostics.push(
                Diagnostic::new(
                    DiagnosticCode::WasmError,
                    "constructor allocation needs a field-slot local",
                )
                .with_label(Label::primary(span, "constructor allocated here")),
            );
            return;
        };
        writeln!(self.functions, "    i32.const {}", constructor.arguments.len() * 8).expect("write WAT");
        writeln!(self.functions, "    call $__alloc").expect("write WAT");
        writeln!(self.functions, "    local.set ${slots}").expect("write WAT");
        self.uses_runtime = true;
        for (index, argument) in constructor.arguments.iter().enumerate() {
            writeln!(self.functions, "    local.get ${slots}").expect("write WAT");
            writeln!(self.functions, "    i32.const {}", index * 8).expect("write WAT");
            writeln!(self.functions, "    i32.add").expect("write WAT");
            self.expression_slot_value(argument, &argument.type_);
            self.store_slot(&argument.type_);
        }
        writeln!(self.functions, "    i32.const {}", constructor_tag(&constructor.name)).expect("write WAT");
        writeln!(self.functions, "    i32.const {}", constructor.arguments.len()).expect("write WAT");
        writeln!(self.functions, "    local.get ${slots}").expect("write WAT");
        writeln!(self.functions, "    call $__custom_new").expect("write WAT");
    }

    fn record_update(
        &mut self, record: &ir::Expression, constructor: &str, fields: &[ir::RecordFieldUpdate], type_: &Type,
        span: crate::source::Span,
    ) {
        let (Some(source), Some(slots)) = (
            self.current.record_update_source.clone(),
            self.current.record_update_slots.clone(),
        ) else {
            self.diagnostics.push(
                Diagnostic::new(DiagnosticCode::WasmError, "record update needs scratch locals")
                    .with_label(Label::primary(span, "record update here")),
            );
            return;
        };
        self.expression(record);
        writeln!(self.functions, "    local.set ${source}").expect("write WAT");
        writeln!(self.functions, "    i32.const {}", fields.len() * 8).expect("write WAT");
        writeln!(self.functions, "    call $__alloc").expect("write WAT");
        writeln!(self.functions, "    local.set ${slots}").expect("write WAT");
        self.uses_runtime = true;

        let source_offset = if matches!(type_, Type::Record { .. }) { 8 } else { 12 };
        for (index, field) in fields.iter().enumerate() {
            writeln!(self.functions, "    local.get ${slots}").expect("write WAT");
            writeln!(self.functions, "    i32.const {}", index * 8).expect("write WAT");
            writeln!(self.functions, "    i32.add").expect("write WAT");
            match &field.value {
                Some(value) => self.expression_slot_value(value, &field.type_),
                None => self.source_slot_value(&source, source_offset + index * 8, &field.type_),
            }
            self.store_slot(&field.type_);
        }

        match type_ {
            Type::Record { .. } => {
                writeln!(self.functions, "    i32.const {}", fields.len()).expect("write WAT");
                writeln!(self.functions, "    local.get ${slots}").expect("write WAT");
                writeln!(self.functions, "    call $__record_new").expect("write WAT");
            }
            _ => {
                writeln!(self.functions, "    i32.const {}", constructor_tag(constructor)).expect("write WAT");
                writeln!(self.functions, "    i32.const {}", fields.len()).expect("write WAT");
                writeln!(self.functions, "    local.get ${slots}").expect("write WAT");
                writeln!(self.functions, "    call $__custom_new").expect("write WAT");
            }
        }
    }

    fn expression_slot_value(&mut self, expression: &ir::Expression, type_: &Type) {
        match type_ {
            Type::Nil => writeln!(self.functions, "    i64.const 0").expect("write WAT"),
            Type::Int | Type::Float => self.expression(expression),
            _ => {
                self.expression(expression);
                writeln!(self.functions, "    i64.extend_i32_u").expect("write WAT");
            }
        }
    }

    fn source_slot_value(&mut self, source: &str, offset: usize, type_: &Type) {
        match type_ {
            Type::Nil => writeln!(self.functions, "    i64.const 0").expect("write WAT"),
            Type::Int => {
                writeln!(self.functions, "    local.get ${source}").expect("write WAT");
                writeln!(self.functions, "    i32.const {offset}").expect("write WAT");
                writeln!(self.functions, "    i32.add").expect("write WAT");
                writeln!(self.functions, "    i64.load").expect("write WAT");
            }
            Type::Float => {
                writeln!(self.functions, "    local.get ${source}").expect("write WAT");
                writeln!(self.functions, "    i32.const {offset}").expect("write WAT");
                writeln!(self.functions, "    i32.add").expect("write WAT");
                writeln!(self.functions, "    f64.load").expect("write WAT");
            }
            _ => {
                writeln!(self.functions, "    local.get ${source}").expect("write WAT");
                writeln!(self.functions, "    i32.const {offset}").expect("write WAT");
                writeln!(self.functions, "    i32.add").expect("write WAT");
                writeln!(self.functions, "    i32.load").expect("write WAT");
                writeln!(self.functions, "    i64.extend_i32_u").expect("write WAT");
            }
        }
    }

    fn store_slot(&mut self, type_: &Type) {
        match type_ {
            Type::Float => writeln!(self.functions, "    f64.store").expect("write WAT"),
            _ => writeln!(self.functions, "    i64.store").expect("write WAT"),
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

    fn static_closure(&mut self, function_id: u32, captures: &[u64]) -> u32 {
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
        if matches!(function.abi.boundary, ir::CallBoundary::Internal) {
            return true;
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

    fn unsupported_target_import(&mut self, module: &str, span: crate::source::Span, function: &str) {
        self.diagnostics.push(
            Diagnostic::new(
                DiagnosticCode::WasmError,
                format!(
                    "function `{function}` imports host module `{module}`, but target {:?} expects `{}`",
                    self.options.target,
                    self.options.target.host_module()
                ),
            )
            .with_label(Label::primary(span, "unsupported target import here")),
        );
    }

    fn unsupported_stdlib_host_call(&mut self, module: &str, member: &str, span: crate::source::Span) {
        self.diagnostics.push(
            Diagnostic::new(
                DiagnosticCode::WasmError,
                format!(
                    "stdlib host call `{module}.{member}` is not supported for target `{}`",
                    self.options.target.name()
                ),
            )
            .with_label(Label::primary(span, "unsupported host call for this target"))
            .with_note("supported targets for `gleam/io` host calls are `wasmtime` and `browser`"),
        );
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

    fn unsupported_residual_use(&mut self, span: crate::source::Span) {
        self.diagnostics.push(
            Diagnostic::new(DiagnosticCode::WasmError, "raw `use` IR reached the Wasm backend")
                .with_label(Label::primary(span, "lower `use` before backend emission"))
                .with_note("`use` must lower to callback-passing call IR before WAT emission"),
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
        writeln!(self.wat, "  (global $__last_panic_payload (mut i32) (i32.const 0))").expect("write WAT");
    }

    fn alloc(&mut self, config: runtime::RuntimeConfig) {
        let helper = helpers::ALLOC_HELPER
            .replace("{alignment_mask}", &(config.layout.alignment - 1).to_string())
            .replace("{alignment}", &config.layout.alignment.to_string())
            .replace("{allocation_failure_offset}", "64");
        self.lines(&helper);
    }

    fn helpers(&mut self) {
        self.lines(helpers::PANIC_HELPERS);
        self.lines(helpers::COPY_HELPERS);
        self.lines(helpers::STRING_HELPERS);
        self.lines(helpers::BIT_ARRAY_HELPERS);
        let managed_value_helpers = helpers::MANAGED_VALUE_HELPERS
            .replace(
                "{closure_capture_slot_size}",
                &u32::from(ClosureConstants::CaptureSlotSize).to_string(),
            )
            .replace(
                "{closure_function_id_offset}",
                &u32::from(ClosureConstants::FunctionIdOffset).to_string(),
            )
            .replace(
                "{closure_captures_offset}",
                &u32::from(ClosureConstants::CapturesOffset).to_string(),
            );
        self.lines(&managed_value_helpers);
        self.lines(helpers::EQUALITY_AND_ORDERING_HELPERS);
        self.lines(helpers::DEBUG_HELPERS);
        self.lines(helpers::HOST_ADAPTER_HELPERS);
    }

    fn lines(&mut self, block: &str) {
        for line in block.trim_matches('\n').split('\n') {
            self.line(line);
        }
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

fn closure_constant_usize(value: ClosureConstants) -> usize {
    u32::from(value) as usize
}

#[derive(Default)]
struct DebugLocalTypes {
    i32: bool,
    i64: bool,
    f64: bool,
}

fn block_debug_local_types(block: &ir::Block) -> DebugLocalTypes {
    let mut locals = DebugLocalTypes::default();
    for instruction in &block.instructions {
        match instruction {
            Instruction::Evaluate { expression, .. }
            | Instruction::LocalSet { value: expression, .. }
            | Instruction::AssertMatch { value: expression, .. } => {
                expression_debug_local_types(expression, &mut locals)
            }
        }
    }
    expression_debug_local_types(&block.result, &mut locals);
    locals
}

fn expression_debug_local_types(expression: &ir::Expression, locals: &mut DebugLocalTypes) {
    if let ExpressionKind::DirectCall(call) = &expression.kind
        && call.function == "__stdlib_gleam_io_debug"
        && let Some(argument) = call.arguments.first()
    {
        match argument.value.type_ {
            Type::Int => locals.i64 = true,
            Type::Float => locals.f64 = true,
            Type::Bool
            | Type::String
            | Type::BitArray
            | Type::Tuple(_)
            | Type::List(_)
            | Type::Record { .. }
            | Type::Custom { .. }
            | Type::Opaque { .. }
            | Type::Function { .. } => locals.i32 = true,
            Type::Nil | Type::Generic(_) => {}
        }
    }

    match &expression.kind {
        ExpressionKind::DirectCall(call) => {
            for argument in &call.arguments {
                expression_debug_local_types(&argument.value, locals);
            }
        }
        ExpressionKind::IndirectCall(call) => {
            expression_debug_local_types(&call.callee, locals);
            for argument in &call.arguments {
                expression_debug_local_types(&argument.value, locals);
            }
        }
        ExpressionKind::Branch(branch) => {
            for subject in &branch.subjects {
                expression_debug_local_types(subject, locals);
            }
            for clause in &branch.clauses {
                if let Some(guard) = &clause.guard {
                    expression_debug_local_types(guard, locals);
                }
                expression_debug_local_types(&clause.body, locals);
            }
        }
        ExpressionKind::Tuple(items) | ExpressionKind::List(items) => {
            for item in items {
                expression_debug_local_types(item, locals);
            }
        }
        ExpressionKind::BitArrayConcat { left, right }
        | ExpressionKind::Compare { left, right, .. }
        | ExpressionKind::RuntimeEquality { left, right }
        | ExpressionKind::ListCons { head: left, tail: right } => {
            expression_debug_local_types(left, locals);
            expression_debug_local_types(right, locals);
        }
        ExpressionKind::BitStringDeconstruct { bit_array, .. }
        | ExpressionKind::FieldAccess { record: bit_array, .. }
        | ExpressionKind::TupleElement { tuple: bit_array, .. }
        | ExpressionKind::ListDeconstruct { list: bit_array, .. } => expression_debug_local_types(bit_array, locals),
        ExpressionKind::Record(record) => {
            for field in &record.fields {
                expression_debug_local_types(&field.value, locals);
            }
        }
        ExpressionKind::RecordUpdate { record, fields, .. } => {
            expression_debug_local_types(record, locals);
            for field in fields {
                if let Some(value) = &field.value {
                    expression_debug_local_types(value, locals);
                }
            }
        }
        ExpressionKind::Memory(operation) => match operation {
            ir::MemoryOperation::Allocate { bytes } => expression_debug_local_types(bytes, locals),
            ir::MemoryOperation::Load { address, .. } => expression_debug_local_types(address, locals),
            ir::MemoryOperation::Store { address, value } => {
                expression_debug_local_types(address, locals);
                expression_debug_local_types(value, locals);
            }
        },
        ExpressionKind::Pipeline(pipeline) => {
            expression_debug_local_types(&pipeline.input, locals);
            expression_debug_local_types(&pipeline.call, locals);
        }
        ExpressionKind::Use(use_) => {
            expression_debug_local_types(&use_.callback, locals);
            expression_debug_local_types(&use_.call, locals);
        }
        ExpressionKind::Literal(_)
        | ExpressionKind::LocalGet(_)
        | ExpressionKind::BitArray(_)
        | ExpressionKind::Constructor(_)
        | ExpressionKind::FunctionValue(_)
        | ExpressionKind::AnonymousFunction(_)
        | ExpressionKind::Failure(_) => {}
    }
}

fn block_contains_indirect_call(block: &ir::Block) -> bool {
    block.instructions.iter().any(|instruction| match instruction {
        Instruction::Evaluate { expression, .. }
        | Instruction::LocalSet { value: expression, .. }
        | Instruction::AssertMatch { value: expression, .. } => expression_contains_indirect_call(expression),
    }) || expression_contains_indirect_call(&block.result)
}

fn block_contains_anonymous_function(block: &ir::Block) -> bool {
    block.instructions.iter().any(|instruction| match instruction {
        Instruction::Evaluate { expression, .. }
        | Instruction::LocalSet { value: expression, .. }
        | Instruction::AssertMatch { value: expression, .. } => expression_contains_anonymous_function(expression),
    }) || expression_contains_anonymous_function(&block.result)
}

fn block_contains_constructor(block: &ir::Block) -> bool {
    block.instructions.iter().any(|instruction| match instruction {
        Instruction::Evaluate { expression, .. }
        | Instruction::LocalSet { value: expression, .. }
        | Instruction::AssertMatch { value: expression, .. } => expression_contains_constructor(expression),
    }) || expression_contains_constructor(&block.result)
}

fn expression_contains_constructor(expression: &ir::Expression) -> bool {
    match &expression.kind {
        ExpressionKind::Constructor(_) => true,
        ExpressionKind::DirectCall(call) => call
            .arguments
            .iter()
            .any(|argument| expression_contains_constructor(&argument.value)),
        ExpressionKind::IndirectCall(call) => {
            expression_contains_constructor(&call.callee)
                || call
                    .arguments
                    .iter()
                    .any(|argument| expression_contains_constructor(&argument.value))
        }
        ExpressionKind::Branch(branch) => {
            branch.subjects.iter().any(expression_contains_constructor)
                || branch.clauses.iter().any(|clause| {
                    clause.guard.as_ref().is_some_and(expression_contains_constructor)
                        || expression_contains_constructor(&clause.body)
                })
        }
        ExpressionKind::Tuple(items) | ExpressionKind::List(items) => items.iter().any(expression_contains_constructor),
        ExpressionKind::BitArrayConcat { left, right }
        | ExpressionKind::Compare { left, right, .. }
        | ExpressionKind::RuntimeEquality { left, right } => {
            expression_contains_constructor(left) || expression_contains_constructor(right)
        }
        ExpressionKind::BitStringDeconstruct { bit_array, .. }
        | ExpressionKind::FieldAccess { record: bit_array, .. }
        | ExpressionKind::TupleElement { tuple: bit_array, .. }
        | ExpressionKind::ListDeconstruct { list: bit_array, .. } => expression_contains_constructor(bit_array),
        ExpressionKind::Record(record) => record
            .fields
            .iter()
            .any(|field| expression_contains_constructor(&field.value)),
        ExpressionKind::RecordUpdate { record, fields, .. } => {
            expression_contains_constructor(record)
                || fields
                    .iter()
                    .filter_map(|field| field.value.as_ref())
                    .any(expression_contains_constructor)
        }
        ExpressionKind::ListCons { head, tail } => {
            expression_contains_constructor(head) || expression_contains_constructor(tail)
        }
        ExpressionKind::Memory(operation) => match operation {
            ir::MemoryOperation::Allocate { bytes } => expression_contains_constructor(bytes),
            ir::MemoryOperation::Load { address, .. } => expression_contains_constructor(address),
            ir::MemoryOperation::Store { address, value } => {
                expression_contains_constructor(address) || expression_contains_constructor(value)
            }
        },
        ExpressionKind::Pipeline(pipeline) => {
            expression_contains_constructor(&pipeline.input) || expression_contains_constructor(&pipeline.call)
        }
        ExpressionKind::Use(use_) => {
            expression_contains_constructor(&use_.callback) || expression_contains_constructor(&use_.call)
        }
        _ => false,
    }
}

fn block_contains_record_update(block: &ir::Block) -> bool {
    block.instructions.iter().any(|instruction| match instruction {
        Instruction::Evaluate { expression, .. }
        | Instruction::LocalSet { value: expression, .. }
        | Instruction::AssertMatch { value: expression, .. } => expression_contains_record_update(expression),
    }) || expression_contains_record_update(&block.result)
}

fn expression_contains_record_update(expression: &ir::Expression) -> bool {
    match &expression.kind {
        ExpressionKind::RecordUpdate { .. } => true,
        ExpressionKind::DirectCall(call) => call
            .arguments
            .iter()
            .any(|argument| expression_contains_record_update(&argument.value)),
        ExpressionKind::IndirectCall(call) => {
            expression_contains_record_update(&call.callee)
                || call
                    .arguments
                    .iter()
                    .any(|argument| expression_contains_record_update(&argument.value))
        }
        ExpressionKind::Branch(branch) => {
            branch.subjects.iter().any(expression_contains_record_update)
                || branch.clauses.iter().any(|clause| {
                    clause.guard.as_ref().is_some_and(expression_contains_record_update)
                        || expression_contains_record_update(&clause.body)
                })
        }
        ExpressionKind::Tuple(items) | ExpressionKind::List(items) => {
            items.iter().any(expression_contains_record_update)
        }
        ExpressionKind::BitArrayConcat { left, right }
        | ExpressionKind::Compare { left, right, .. }
        | ExpressionKind::RuntimeEquality { left, right } => {
            expression_contains_record_update(left) || expression_contains_record_update(right)
        }
        ExpressionKind::BitStringDeconstruct { bit_array, .. }
        | ExpressionKind::FieldAccess { record: bit_array, .. }
        | ExpressionKind::TupleElement { tuple: bit_array, .. }
        | ExpressionKind::ListDeconstruct { list: bit_array, .. } => expression_contains_record_update(bit_array),
        ExpressionKind::Record(record) => record
            .fields
            .iter()
            .any(|field| expression_contains_record_update(&field.value)),
        ExpressionKind::Constructor(constructor) => constructor.arguments.iter().any(expression_contains_record_update),
        ExpressionKind::ListCons { head, tail } => {
            expression_contains_record_update(head) || expression_contains_record_update(tail)
        }
        ExpressionKind::Memory(operation) => match operation {
            ir::MemoryOperation::Allocate { bytes } => expression_contains_record_update(bytes),
            ir::MemoryOperation::Load { address, .. } => expression_contains_record_update(address),
            ir::MemoryOperation::Store { address, value } => {
                expression_contains_record_update(address) || expression_contains_record_update(value)
            }
        },
        ExpressionKind::Pipeline(pipeline) => {
            expression_contains_record_update(&pipeline.input) || expression_contains_record_update(&pipeline.call)
        }
        ExpressionKind::Use(use_) => {
            expression_contains_record_update(&use_.callback) || expression_contains_record_update(&use_.call)
        }
        _ => false,
    }
}

fn expression_contains_anonymous_function(expression: &ir::Expression) -> bool {
    match &expression.kind {
        ExpressionKind::AnonymousFunction(_) => true,
        ExpressionKind::DirectCall(call) => call
            .arguments
            .iter()
            .any(|argument| expression_contains_anonymous_function(&argument.value)),
        ExpressionKind::IndirectCall(call) => {
            expression_contains_anonymous_function(&call.callee)
                || call
                    .arguments
                    .iter()
                    .any(|argument| expression_contains_anonymous_function(&argument.value))
        }
        ExpressionKind::Branch(branch) => {
            branch.subjects.iter().any(expression_contains_anonymous_function)
                || branch.clauses.iter().any(|clause| {
                    clause
                        .guard
                        .as_ref()
                        .is_some_and(expression_contains_anonymous_function)
                        || expression_contains_anonymous_function(&clause.body)
                })
        }
        ExpressionKind::Tuple(items) | ExpressionKind::List(items) => {
            items.iter().any(expression_contains_anonymous_function)
        }
        ExpressionKind::BitArrayConcat { left, right }
        | ExpressionKind::Compare { left, right, .. }
        | ExpressionKind::RuntimeEquality { left, right } => {
            expression_contains_anonymous_function(left) || expression_contains_anonymous_function(right)
        }
        ExpressionKind::BitStringDeconstruct { bit_array, .. }
        | ExpressionKind::FieldAccess { record: bit_array, .. }
        | ExpressionKind::TupleElement { tuple: bit_array, .. }
        | ExpressionKind::ListDeconstruct { list: bit_array, .. } => expression_contains_anonymous_function(bit_array),
        ExpressionKind::Record(record) => record
            .fields
            .iter()
            .any(|field| expression_contains_anonymous_function(&field.value)),
        ExpressionKind::Constructor(constructor) => {
            constructor.arguments.iter().any(expression_contains_anonymous_function)
        }
        ExpressionKind::RecordUpdate { record, fields, .. } => {
            expression_contains_anonymous_function(record)
                || fields
                    .iter()
                    .filter_map(|field| field.value.as_ref())
                    .any(expression_contains_anonymous_function)
        }
        ExpressionKind::ListCons { head, tail } => {
            expression_contains_anonymous_function(head) || expression_contains_anonymous_function(tail)
        }
        ExpressionKind::Memory(operation) => match operation {
            ir::MemoryOperation::Allocate { bytes } => expression_contains_anonymous_function(bytes),
            ir::MemoryOperation::Load { address, .. } => expression_contains_anonymous_function(address),
            ir::MemoryOperation::Store { address, value } => {
                expression_contains_anonymous_function(address) || expression_contains_anonymous_function(value)
            }
        },
        ExpressionKind::Pipeline(pipeline) => {
            expression_contains_anonymous_function(&pipeline.input)
                || expression_contains_anonymous_function(&pipeline.call)
        }
        ExpressionKind::Use(use_) => {
            expression_contains_anonymous_function(&use_.callback) || expression_contains_anonymous_function(&use_.call)
        }
        _ => false,
    }
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
        ExpressionKind::RecordUpdate { record, fields, .. } => {
            expression_contains_indirect_call(record)
                || fields
                    .iter()
                    .filter_map(|field| field.value.as_ref())
                    .any(expression_contains_indirect_call)
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

fn integer_operator_instruction(function: &str) -> &'static str {
    match function {
        "__op_add" => "i64.add",
        "__op_subtract" => "i64.sub",
        "__op_multiply" => "i64.mul",
        "__op_divide" => "i64.div_s",
        "__op_remainder" => "i64.rem_s",
        _ => unreachable!("unknown integer operator function"),
    }
}

fn float_operator_instruction(function: &str) -> &'static str {
    match function {
        "__op_float_add" => "f64.add",
        "__op_float_subtract" => "f64.sub",
        "__op_float_multiply" => "f64.mul",
        "__op_float_divide" => "f64.div",
        _ => unreachable!("unknown float operator function"),
    }
}

fn comparison_instruction(op: ir::ComparisonOp, prefix: &str) -> &'static str {
    match (prefix, op) {
        ("i64", ir::ComparisonOp::Less) => "i64.lt_s",
        ("i64", ir::ComparisonOp::LessEqual) => "i64.le_s",
        ("i64", ir::ComparisonOp::Greater) => "i64.gt_s",
        ("i64", ir::ComparisonOp::GreaterEqual) => "i64.ge_s",
        ("i32", ir::ComparisonOp::Less) => "i32.lt_s",
        ("i32", ir::ComparisonOp::LessEqual) => "i32.le_s",
        ("i32", ir::ComparisonOp::Greater) => "i32.gt_s",
        ("i32", ir::ComparisonOp::GreaterEqual) => "i32.ge_s",
        ("f64", ir::ComparisonOp::Less) => "f64.lt",
        ("f64", ir::ComparisonOp::LessEqual) => "f64.le",
        ("f64", ir::ComparisonOp::Greater) => "f64.gt",
        ("f64", ir::ComparisonOp::GreaterEqual) => "f64.ge",
        _ => unreachable!("unknown comparison instruction"),
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
    use crate::runtime::ObjectTag;
    use crate::source::{SourceFile, SourceFileId, Span};
    use crate::{ast, ir, parse, resolve, types};
    use wasmtime::{Caller, Engine, Instance, Linker, Module, Store};

    fn compile_wasm(source: &str) -> WasmModule {
        let source = SourceFile::new(SourceFileId(0), source);
        let cst = parse::parse(source).expect("parse source");
        let ast = ast::build(&cst).expect("build ast");
        let resolved = resolve::resolve(ast).expect("resolve names");
        let typed = types::check(resolved).expect("type check source");
        let ir = ir::lower(typed).expect("lower source");
        emit(&ir).expect("emit wasm")
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
        assert!(wasm.wat.contains("(export \"__regulus_string_len\")"));
        assert!(wasm.wat.contains("(export \"__regulus_value_tag\")"));
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
    fn emits_string_export_adapters_for_host_boundaries() {
        let wasm = compile_wasm("pub fn greeting() { \"hello\" }");

        assert!(wasm.wat.contains("(func $greeting__data (export \"greeting__data\")"));
        assert!(wasm.wat.contains("(func $greeting__len (export \"greeting__len\")"));
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
        let diagnostics = emit_wat_with_options(&module, EmitOptions { target: WasmTarget::Browser })
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
        let diagnostics = emit_wat(&ir_module(vec![function], span)).expect_err("residual use should fail");

        assert!(
            diagnostics
                .iter()
                .any(|diagnostic| diagnostic.message.contains("raw `use` IR reached the Wasm backend"))
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
                .contains("(import \"browser\" \"println\" (func $__stdlib_gleam_io_println"),
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
        let engine = Engine::default();
        let module = Module::new(&engine, &wasm.bytes).expect("compile wasm module");
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
        let number = instance
            .get_typed_func::<(), i32>(&mut store, "number")
            .expect("get number export");
        let pointer = number.call(&mut store, ()).expect("call number") as usize;
        let mut bytes = [0; 16];
        memory.read(&store, pointer, &mut bytes).expect("read number string");
        assert_eq!(u32::from_le_bytes(bytes[4..8].try_into().unwrap()), 3);
        assert_eq!(&bytes[8..11], b"-42");

        let text = instance
            .get_typed_func::<(), i32>(&mut store, "text")
            .expect("get text export");
        let pointer = text.call(&mut store, ()).expect("call text") as usize;
        memory.read(&store, pointer, &mut bytes).expect("read text string");
        assert_eq!(&bytes[8..10], b"ab");

        let text_len = instance
            .get_typed_func::<(), i64>(&mut store, "text_len")
            .expect("get text_len export");
        assert_eq!(text_len.call(&mut store, ()).expect("call text_len"), 3);
        let empty = instance
            .get_typed_func::<(), i32>(&mut store, "empty")
            .expect("get empty export");
        assert_eq!(empty.call(&mut store, ()).expect("call empty"), 1);
        let item_count = instance
            .get_typed_func::<(), i64>(&mut store, "item_count")
            .expect("get item_count export");
        assert_eq!(item_count.call(&mut store, ()).expect("call item_count"), 3);
        let reversed_head = instance
            .get_typed_func::<(), i64>(&mut store, "reversed_head")
            .expect("get reversed_head export");
        assert_eq!(reversed_head.call(&mut store, ()).expect("call reversed_head"), 3);
        let debugged = instance
            .get_typed_func::<(), i64>(&mut store, "debugged")
            .expect("get debugged export");
        assert_eq!(debugged.call(&mut store, ()).expect("call debugged"), 42);
        let debugged_text = instance
            .get_typed_func::<(), i32>(&mut store, "debugged_text")
            .expect("get debugged_text export");
        let pointer = debugged_text.call(&mut store, ()).expect("call debugged_text") as usize;
        memory
            .read(&store, pointer, &mut bytes)
            .expect("read debugged text string");
        assert_eq!(&bytes[8..10], b"ok");
        assert_eq!(store.data(), "42\nok\n");

        let bool_text = instance
            .get_typed_func::<(), i32>(&mut store, "bool_text")
            .expect("get bool_text export");
        let pointer = bool_text.call(&mut store, ()).expect("call bool_text") as usize;
        memory.read(&store, pointer, &mut bytes).expect("read bool text");
        assert_eq!(&bytes[8..12], b"True");
        let bool_rank = instance
            .get_typed_func::<(), i64>(&mut store, "bool_rank")
            .expect("get bool_rank export");
        assert_eq!(bool_rank.call(&mut store, ()).expect("call bool_rank"), -1);
        let dict_value = instance
            .get_typed_func::<(), i64>(&mut store, "dict_value")
            .expect("get dict_value export");
        assert_eq!(dict_value.call(&mut store, ()).expect("call dict_value"), 42);
        let dict_missing = instance
            .get_typed_func::<(), i32>(&mut store, "dict_missing")
            .expect("get dict_missing export");
        assert_eq!(dict_missing.call(&mut store, ()).expect("call dict_missing"), 0);
        let dict_persistent_size = instance
            .get_typed_func::<(), i64>(&mut store, "dict_persistent_size")
            .expect("get dict_persistent_size export");
        assert_eq!(
            dict_persistent_size
                .call(&mut store, ())
                .expect("call dict_persistent_size"),
            1
        );
        let float_rank = instance
            .get_typed_func::<(), i64>(&mut store, "float_rank")
            .expect("get float_rank export");
        assert_eq!(float_rank.call(&mut store, ()).expect("call float_rank"), -1);
        let float_larger = instance
            .get_typed_func::<(), f64>(&mut store, "float_larger")
            .expect("get float_larger export");
        assert_eq!(float_larger.call(&mut store, ()).expect("call float_larger"), 2.5);
        let float_text = instance
            .get_typed_func::<(), i32>(&mut store, "float_text")
            .expect("get float_text export");
        let pointer = float_text.call(&mut store, ()).expect("call float_text") as usize;
        memory.read(&store, pointer, &mut bytes).expect("read float text");
        assert_eq!(&bytes[8..16], b"1.500000");
        let same_value = instance
            .get_typed_func::<(), i64>(&mut store, "same_value")
            .expect("get same_value export");
        assert_eq!(same_value.call(&mut store, ()).expect("call same_value"), 9);
        let constant_value = instance
            .get_typed_func::<(), i64>(&mut store, "constant_value")
            .expect("get constant_value export");
        assert_eq!(constant_value.call(&mut store, ()).expect("call constant_value"), 7);
        let bits_size = instance
            .get_typed_func::<(), i64>(&mut store, "bits_size")
            .expect("get bits_size export");
        assert_eq!(bits_size.call(&mut store, ()).expect("call bits_size"), 24);
        let bytes_size = instance
            .get_typed_func::<(), i64>(&mut store, "bytes_size")
            .expect("get bytes_size export");
        assert_eq!(bytes_size.call(&mut store, ()).expect("call bytes_size"), 2);
        let bits_empty = instance
            .get_typed_func::<(), i32>(&mut store, "bits_empty")
            .expect("get bits_empty export");
        assert_eq!(bits_empty.call(&mut store, ()).expect("call bits_empty"), 1);
        let bits_start = instance
            .get_typed_func::<(), i32>(&mut store, "bits_start")
            .expect("get bits_start export");
        assert_eq!(bits_start.call(&mut store, ()).expect("call bits_start"), 1);
        let bits_append_size = instance
            .get_typed_func::<(), i64>(&mut store, "bits_append_size")
            .expect("get bits_append_size export");
        assert_eq!(
            bits_append_size.call(&mut store, ()).expect("call bits_append_size"),
            16
        );
        let bits_joined = instance
            .get_typed_func::<(), i32>(&mut store, "bits_joined")
            .expect("get bits_joined export");
        let pointer = bits_joined.call(&mut store, ()).expect("call bits_joined") as usize;
        memory.read(&store, pointer, &mut bytes).expect("read joined bit array");
        assert_eq!(u32::from_le_bytes(bytes[4..8].try_into().unwrap()), 24);
        assert_eq!(&bytes[8..11], &[1, 2, 3]);
    }

    #[test]
    fn compiles_common_stdlib_fixture() {
        let wasm = compile_wasm(include_str!("../../../fixtures/wasm/common_stdlib.gleam"));

        assert!(!wasm.wat.contains("(import \"env\" \"print\""), "{}", wasm.wat);
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
        let prelude =
            runtime_prelude(runtime::RuntimeConfig::DEFAULT).replace("  (memory (export \"memory\") 1)", memory_wat);
        let wat = format!("(module\n{prelude}{extra_wat})\n");
        let bytes = wat::parse_str(&wat).expect("parse runtime helper wat");
        let engine = Engine::default();
        let module = Module::new(&engine, bytes).expect("compile helper module");
        let mut store = Store::new(&engine, ());
        let instance = Instance::new(&mut store, &module, &[]).expect("instantiate helper module");
        (engine, store, instance)
    }
}
