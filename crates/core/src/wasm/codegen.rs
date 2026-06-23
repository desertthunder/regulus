//! Incremental IR-to-structured-Wasm code generation.

use std::collections::{HashMap, HashSet};

use super::builder::{
    BlockType, DataSegment, ElementSegment, Export, ExportDesc, Function, FunctionId, FunctionType, Global, GlobalId,
    Import, ImportDesc, Instruction, Local, LocalId, Memory, MemoryArg, MemoryId, Module, ReferenceType, Table,
    TableId, TypeId, ValueType,
};
use super::{EmitOptions, WasmTarget};
use crate::ast::LiteralKind;
use crate::diagnostic::{Diagnostic, DiagnosticCode, Diagnostics, Label};
use crate::ir::{self, ExpressionKind};
use crate::source::Span;
use crate::{
    ClosureConstants,
    abi::{STDLIB_IO_HOST_MODULE, is_allowed_anything_external},
    runtime,
    types::Type,
};

#[derive(Debug, Clone, Copy)]
enum JsAbiBoundary<'a> {
    Import { module: &'a str, name: &'a str },
    Export { name: &'a str },
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

#[derive(Debug, Clone)]
struct FunctionSignature {
    type_id: TypeId,
    type_: FunctionType,
}

#[derive(Debug, Clone, Copy)]
struct DecodeLocals {
    result: LocalId,
    kind: LocalId,
    tag: LocalId,
    field: LocalId,
    data: LocalId,
}

#[derive(Debug)]
enum StructuredError {
    Unsupported,
    Invariant(String),
    Diagnostics(Diagnostics),
}

type StructuredResult<T> = Result<T, StructuredError>;

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
    list_tail_local: Option<LocalId>,
    aggregate_local: Option<LocalId>,
    alloc_local: Option<LocalId>,
    alloc_end_local: Option<LocalId>,
    alloc_pages_local: Option<LocalId>,
    bit_i_local: Option<LocalId>,
    bit_value_local: Option<LocalId>,
    dynamic_data_local: Option<LocalId>,
    dynamic_decoder_local: Option<LocalId>,
    dynamic_kind_local: Option<LocalId>,
    dynamic_tag_local: Option<LocalId>,
    dynamic_field_local: Option<LocalId>,
    dynamic_result_local: Option<LocalId>,
    dynamic_original_local: Option<LocalId>,
    funcid_locals: Vec<LocalId>,
    indirect_call_depth: usize,
    options: EmitOptions,
    config: runtime::RuntimeConfig,
    next_static_offset: u32,
    memory: Option<MemoryId>,
    heap_global: Option<GlobalId>,
    imported_functions: u32,
    func_table: Option<TableId>,
    runtime_helper_roots: HashSet<String>,
}

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
            list_tail_local: None,
            aggregate_local: None,
            alloc_local: None,
            alloc_end_local: None,
            alloc_pages_local: None,
            bit_i_local: None,
            bit_value_local: None,
            dynamic_data_local: None,
            dynamic_decoder_local: None,
            dynamic_kind_local: None,
            dynamic_tag_local: None,
            dynamic_field_local: None,
            dynamic_result_local: None,
            dynamic_original_local: None,
            funcid_locals: Vec::new(),
            indirect_call_depth: 0,
            options,
            config: runtime::RuntimeConfig::DEFAULT,
            next_static_offset: runtime::RuntimeConfig::DEFAULT.static_data_start,
            memory: None,
            heap_global: None,
            imported_functions: 0,
            func_table: None,
            runtime_helper_roots: HashSet::new(),
        }
    }

    fn module(mut self, source: &ir::Module) -> StructuredResult<Module> {
        self.module.source_span = source.functions.first().map(|function| function.span);
        validate_anything_boundary_abi(source)?;
        if self.options.target.is_js_host() {
            validate_js_host_abi(source, self.options.target)?;
        }
        let emitted_functions = reachable_functions(source);
        for function in &emitted_functions {
            let signature = self.function_signature(function)?;
            self.signatures.insert(function.name.clone(), signature);
        }

        let needs_table = emitted_functions.iter().any(|f| {
            f.body.instructions.iter().any(|instr| match instr {
                ir::Instruction::Evaluate { expression, .. } | ir::Instruction::LocalSet { value: expression, .. } => {
                    expression.contains_indirect_call() || expression_needs_dynamic_closure_dispatch(expression)
                }
                ir::Instruction::AssertMatch { value, .. } => {
                    value.contains_indirect_call() || expression_needs_dynamic_closure_dispatch(value)
                }
            }) || f.body.result.contains_indirect_call()
                || expression_needs_dynamic_closure_dispatch(&f.body.result)
        });
        if needs_table {
            let n = emitted_functions.len().max(1) as u32;
            let table =
                self.module
                    .push_table(Table { element_type: ReferenceType::FuncRef, minimum: n, maximum: Some(n) });
            self.func_table = Some(table);
        }

        for function in &emitted_functions {
            if matches!(
                function.abi.boundary,
                ir::CallBoundary::HostImport { .. } | ir::CallBoundary::ModuleImport { .. }
            ) {
                self.import_function(function)?;
            }
        }

        for function in &emitted_functions {
            for import in needed_debug_imports(function) {
                match self.options.target {
                    WasmTarget::Wasi => {
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
                    _ => {
                        self.ensure_debug_import(import);
                    }
                }
            }
        }

        for function in &emitted_functions {
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

        for function in &emitted_functions {
            if matches!(function.abi.boundary, ir::CallBoundary::ModuleExport) {
                let export_name = source
                    .exports
                    .iter()
                    .find(|export| export.kind == ir::ExportKind::Function && export.backend_name() == function.name)
                    .map(|export| export.name.clone())
                    .unwrap_or_else(|| function.name.clone());
                let function_id = self.function_id_structured(&function.name);
                self.module
                    .exports
                    .push(Export { name: export_name.clone(), desc: ExportDesc::Function(function_id) });
                self.export_adapters(function, &export_name)?;
            }
        }

        if self.options.target.is_js_host() {
            self.emit_js_host_abi_helpers();
        }
        if self.options.target == WasmTarget::Wasmtime && module_exports_arena_scoped_values(source) {
            self.runtime_helper_roots.insert("__arena_mark".into());
            self.runtime_helper_roots.insert("__arena_reset".into());
        }

        if !self.runtime_helper_roots.is_empty() {
            let helper_bundle = super::runtime_helper_bundle(self.config, &self.runtime_helper_roots)
                .map_err(StructuredError::Invariant)?;
            debug_assert!(!helper_bundle.bytes.is_empty());
            self.ensure_memory();
            self.ensure_heap_global();
            self.name_heap_global();
            self.ensure_last_panic_global();
            self.module.raw_wat_items.push(helper_bundle.wat);
        }

        if let Some(memory) = self.memory {
            self.module
                .exports
                .push(Export { name: "memory".into(), desc: ExportDesc::Memory(memory) });
        }

        if self.func_table.is_some() {
            self.emit_function_table(&emitted_functions)?;
        }

        Ok(self.module)
    }

    fn export_adapters(&mut self, function: &ir::Function, export_name: &str) -> StructuredResult<()> {
        if !function.params.is_empty() || function.return_type != Type::String {
            return Ok(());
        }
        let string_result = FunctionType::new([], [ValueType::I32]);
        let string_type = self.required_signature(&function.name)?.type_;
        let original = self.function_id_structured(&function.name);
        let memory = self.ensure_memory();

        let data_type = self.module.push_type(FunctionType::new([], [ValueType::I32]));
        let mut data = Function::new(data_type);
        data.name = Some(format!("{}__data", function.name));
        data.body = vec![
            Instruction::Call { function: original, type_: string_type },
            Instruction::I32Const(8),
            Instruction::I32Add,
        ];
        let data_id = self.module.push_function(data);
        self.module
            .exports
            .push(Export { name: format!("{export_name}__data"), desc: ExportDesc::Function(data_id) });

        let len_type = self.module.push_type(FunctionType::new([], [ValueType::I32]));
        let mut len = Function::new(len_type);
        len.name = Some(format!("{}__len", function.name));
        len.body = vec![
            Instruction::Call { function: original, type_: string_result },
            Instruction::I32Load(MemoryArg::new(memory, 4, 2)),
        ];
        let len_id = self.module.push_function(len);
        self.module
            .exports
            .push(Export { name: format!("{export_name}__len"), desc: ExportDesc::Function(len_id) });
        Ok(())
    }

    fn concrete_import(&self, function: &ir::Function) -> StructuredResult<Option<(String, String)>> {
        match &function.abi.boundary {
            ir::CallBoundary::HostImport { module, name } if module == STDLIB_IO_HOST_MODULE => {
                Ok(Some(self.stdlib_io_import(name, function.span)?))
            }
            ir::CallBoundary::HostImport { module, name }
                if self.options.target.accepts_host_import(module, name) =>
            {
                Ok(Some((module.clone(), name.clone())))
            }
            ir::CallBoundary::HostImport { module, name } if self.options.target.accepts_host_module(module) => {
                Err(StructuredError::Diagnostics(vec![
                    Diagnostic::new(
                        DiagnosticCode::WasmError,
                        format!(
                            "function `{}` imports host function `{module}.{name}`, but target `{}` does not allow that import name",
                            function.name,
                            self.options.target.name()
                        ),
                    )
                    .with_label(Label::primary(function.span, "unsupported target import here"))
                    .with_note("choose an import name supported by this target profile or select a different target"),
                ]))
            }
            ir::CallBoundary::HostImport { module, .. } => Err(StructuredError::Diagnostics(vec![
                Diagnostic::new(
                    DiagnosticCode::WasmError,
                    format!(
                        "function `{}` imports host module `{module}`, but target `{}` expects `{}`",
                        function.name,
                        self.options.target.name(),
                        self.options.target.host_module()
                    ),
                )
                .with_label(Label::primary(function.span, "unsupported target import here"))
                .with_note("change the external module or compile for the target that provides it"),
            ])),
            ir::CallBoundary::ModuleImport { module, name } => Ok(Some((module.clone(), name.clone()))),
            ir::CallBoundary::Internal | ir::CallBoundary::ModuleExport => Ok(None),
        }
    }

    fn emit_js_host_abi_helpers(&mut self) {
        self.ensure_memory();
        self.runtime_helper_roots.insert("__alloc".into());
        self.runtime_helper_roots.insert("__arena_mark".into());
        self.runtime_helper_roots.insert("__arena_reset".into());
        self.runtime_helper_roots.insert("__string_new".into());
        self.runtime_helper_roots.insert("__string_len".into());
        self.runtime_helper_roots.insert("__string_data".into());
        self.runtime_helper_roots.insert("__opaque_new".into());
        self.module.raw_wat_items.push(
            r#"
  (func $__regulus_alloc (param $size i32) (result i32)
    local.get $size
    call $__alloc
  )
  (export "__regulus_alloc" (func $__regulus_alloc))
  (func $__regulus_arena_mark (result i32)
    call $__arena_mark
  )
  (export "__regulus_arena_mark" (func $__regulus_arena_mark))
  (func $__regulus_arena_reset (param $mark i32)
    local.get $mark
    call $__arena_reset
  )
  (export "__regulus_arena_reset" (func $__regulus_arena_reset))
  (func $__regulus_string_new (param $data i32) (param $len i32) (result i32)
    local.get $data
    local.get $len
    call $__string_new
  )
  (export "__regulus_string_new" (func $__regulus_string_new))
  (func $__regulus_trap_if (param $condition i32)
    local.get $condition
    if
      unreachable
    end
  )
  (func $__regulus_memory_bytes (result i64)
    memory.size
    i64.extend_i32_u
    i64.const 65536
    i64.mul
  )
  (func $__regulus_validate_range (param $start i32) (param $len i64)
    (local $start64 i64) (local $end i64)
    local.get $start
    i64.extend_i32_u
    local.set $start64
    local.get $start64
    local.get $len
    i64.add
    local.set $end
    local.get $end
    local.get $start64
    i64.lt_u
    call $__regulus_trap_if
    local.get $end
    call $__regulus_memory_bytes
    i64.gt_u
    call $__regulus_trap_if
  )
  (func $__regulus_validate_object (param $ptr i32)
    (local $tag i32) (local $size i32) (local $payload_len i64)
    local.get $ptr
    i32.eqz
    call $__regulus_trap_if
    local.get $ptr
    i64.const 8
    call $__regulus_validate_range
    local.get $ptr
    i32.load
    local.set $tag
    local.get $ptr
    i32.const 4
    i32.add
    i32.load
    local.set $size
    local.get $tag
    i32.const 1
    i32.lt_u
    local.get $tag
    i32.const 10
    i32.gt_u
    i32.or
    call $__regulus_trap_if
    local.get $tag
    i32.const 1
    i32.eq
    if
      local.get $size
      i64.extend_i32_u
      local.set $payload_len
    else
      local.get $tag
      i32.const 2
      i32.eq
      if
        local.get $size
        i32.const 2
        i32.ne
        call $__regulus_trap_if
        i64.const 12
        local.set $payload_len
      else
        local.get $tag
        i32.const 3
        i32.eq
        local.get $tag
        i32.const 4
        i32.eq
        i32.or
        if
          local.get $size
          i64.extend_i32_u
          i64.const 8
          i64.mul
          local.set $payload_len
        else
          local.get $tag
          i32.const 5
          i32.eq
          local.get $tag
          i32.const 9
          i32.eq
          i32.or
          local.get $tag
          i32.const 10
          i32.eq
          i32.or
          if
            local.get $size
            i64.extend_i32_u
            i64.const 8
            i64.mul
            i64.const 4
            i64.add
            local.set $payload_len
          else
            local.get $tag
            i32.const 6
            i32.eq
            if
              local.get $size
              i64.extend_i32_u
              i64.const 4
              i64.mul
              i64.const 4
              i64.add
              local.set $payload_len
            else
              local.get $tag
              i32.const 7
              i32.eq
              if
                local.get $size
                i64.extend_i32_u
                i64.const 7
                i64.add
                i64.const 8
                i64.div_u
                local.set $payload_len
              else
                local.get $size
                i32.const 0
                i32.ne
                call $__regulus_trap_if
                i64.const 8
                local.set $payload_len
              end
            end
          end
        end
      end
    end
    local.get $ptr
    i32.const 8
    i32.add
    local.get $payload_len
    call $__regulus_validate_range
  )
  (func $__regulus_validate_tag (param $ptr i32) (param $expected i32)
    local.get $ptr
    call $__regulus_validate_object
    local.get $ptr
    i32.load
    local.get $expected
    i32.ne
    call $__regulus_trap_if
  )
  (func $__regulus_value_payload_offset (param $tag i32) (result i32)
    local.get $tag
    i32.const 5
    i32.eq
    local.get $tag
    i32.const 9
    i32.eq
    i32.or
    local.get $tag
    i32.const 10
    i32.eq
    i32.or
    if (result i32)
      i32.const 12
    else
      i32.const 8
    end
  )
  (func $__regulus_string_len (param $ptr i32) (result i32)
    local.get $ptr
    i32.const 1
    call $__regulus_validate_tag
    local.get $ptr
    call $__string_len
  )
  (export "__regulus_string_len" (func $__regulus_string_len))
  (func $__regulus_string_data (param $ptr i32) (result i32)
    local.get $ptr
    i32.const 1
    call $__regulus_validate_tag
    local.get $ptr
    call $__string_data
  )
  (export "__regulus_string_data" (func $__regulus_string_data))
  (func $__regulus_value_tag (param $ptr i32) (result i32)
    local.get $ptr
    i32.eqz
    if (result i32)
      i32.const 0
    else
      local.get $ptr
      call $__regulus_validate_object
      local.get $ptr
      i32.load
    end
  )
  (export "__regulus_value_tag" (func $__regulus_value_tag))
  (func $__regulus_value_arity (param $ptr i32) (result i32)
    local.get $ptr
    call $__regulus_validate_object
    local.get $ptr
    i32.load
    i32.const 1
    i32.eq
    local.get $ptr
    i32.load
    i32.const 7
    i32.eq
    i32.or
    if (result i32)
      i32.const 0
    else
      local.get $ptr
      i32.const 4
      i32.add
      i32.load
    end
  )
  (export "__regulus_value_arity" (func $__regulus_value_arity))
  (func $__regulus_value_constructor (param $ptr i32) (result i32)
    (local $tag i32)
    local.get $ptr
    call $__regulus_validate_object
    local.get $ptr
    i32.load
    local.set $tag
    local.get $tag
    i32.const 5
    i32.eq
    local.get $tag
    i32.const 9
    i32.eq
    i32.or
    local.get $tag
    i32.const 10
    i32.eq
    i32.or
    if (result i32)
      local.get $ptr
      i32.const 8
      i32.add
      i32.load
    else
      i32.const 0
    end
  )
  (export "__regulus_value_constructor" (func $__regulus_value_constructor))
  (func $__regulus_value_field (param $ptr i32) (param $index i32) (result i64)
    (local $tag i32) (local $arity i32)
    local.get $ptr
    call $__regulus_validate_object
    local.get $ptr
    i32.load
    local.set $tag
    local.get $ptr
    i32.const 4
    i32.add
    i32.load
    local.set $arity
    local.get $tag
    i32.const 2
    i32.eq
    local.get $tag
    i32.const 3
    i32.eq
    i32.or
    local.get $tag
    i32.const 4
    i32.eq
    i32.or
    local.get $tag
    i32.const 5
    i32.eq
    i32.or
    local.get $tag
    i32.const 9
    i32.eq
    i32.or
    local.get $tag
    i32.const 10
    i32.eq
    i32.or
    i32.eqz
    call $__regulus_trap_if
    local.get $index
    local.get $arity
    i32.ge_u
    call $__regulus_trap_if
    local.get $ptr
    local.get $tag
    call $__regulus_value_payload_offset
    i32.add
    local.get $index
    i32.const 8
    i32.mul
    i32.add
    i64.load
  )
  (export "__regulus_value_field" (func $__regulus_value_field))
  (func $__regulus_handle_new (param $type_tag i32) (param $handle_id i32) (result i32)
    local.get $type_tag
    local.get $handle_id
    call $__opaque_new
  )
  (export "__regulus_handle_new" (func $__regulus_handle_new))
  (func $__regulus_handle_type (param $ptr i32) (result i32)
    local.get $ptr
    i32.const 8
    call $__regulus_validate_tag
    local.get $ptr
    i32.const 8
    i32.add
    i32.load
  )
  (export "__regulus_handle_type" (func $__regulus_handle_type))
  (func $__regulus_handle_id (param $ptr i32) (result i32)
    local.get $ptr
    i32.const 8
    call $__regulus_validate_tag
    local.get $ptr
    i32.const 12
    i32.add
    i32.load
  )
  (export "__regulus_handle_id" (func $__regulus_handle_id))
"#
            .into(),
        );
    }

    fn stdlib_io_import(&self, name: &str, span: Span) -> StructuredResult<(String, String)> {
        match self.options.target {
            WasmTarget::Wasmtime => Ok(("env".into(), name.into())),
            WasmTarget::Browser | WasmTarget::Bundler => Ok(("browser".into(), name.into())),
            WasmTarget::Wasi | WasmTarget::Nodejs => Err(StructuredError::Diagnostics(vec![
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
        self.list_tail_local = None;
        self.aggregate_local = None;
        self.alloc_local = None;
        self.alloc_end_local = None;
        self.alloc_pages_local = None;
        self.bit_i_local = None;
        self.bit_value_local = None;
        self.dynamic_data_local = None;
        self.dynamic_decoder_local = None;
        self.dynamic_kind_local = None;
        self.dynamic_tag_local = None;
        self.dynamic_field_local = None;
        self.dynamic_result_local = None;
        self.dynamic_original_local = None;
        self.funcid_locals.clear();
        self.indirect_call_depth = 0;

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

        if block_needs_scratch(&function.body) {
            let id = LocalId((structured.params.len() + structured.locals.len()) as u32);
            structured
                .locals
                .push(Local { name: Some("__scratch".into()), type_: ValueType::I32 });
            self.scratch_local = Some(id);

            let depth = indirect_call_max_arg_depth(&function.body)
                .max(if needs_dynamic_closure_dispatch(&function.body) { 1 } else { 0 });
            self.funcid_locals.clear();

            for d in 0..depth {
                let fid = LocalId((structured.params.len() + structured.locals.len()) as u32);
                structured
                    .locals
                    .push(Local { name: Some(format!("__funcid_{d}")), type_: ValueType::I32 });
                self.funcid_locals.push(fid);
            }
        }
        if block_needs_allocation(&function.body) {
            self.runtime_helper_roots.insert("__allocation_fail".into());
            let id = LocalId((structured.params.len() + structured.locals.len()) as u32);
            structured
                .locals
                .push(Local { name: Some("__list_tail".into()), type_: ValueType::I32 });
            self.list_tail_local = Some(id);
            let id = LocalId((structured.params.len() + structured.locals.len()) as u32);
            structured
                .locals
                .push(Local { name: Some("__aggregate".into()), type_: ValueType::I32 });
            self.aggregate_local = Some(id);
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
        if needs_dynamic_decode(&function.body) {
            let id = LocalId((structured.params.len() + structured.locals.len()) as u32);
            structured
                .locals
                .push(Local { name: Some("__dynamic_data".into()), type_: ValueType::I32 });
            self.dynamic_data_local = Some(id);
            let id = LocalId((structured.params.len() + structured.locals.len()) as u32);
            structured
                .locals
                .push(Local { name: Some("__dynamic_decoder".into()), type_: ValueType::I32 });
            self.dynamic_decoder_local = Some(id);
            let id = LocalId((structured.params.len() + structured.locals.len()) as u32);
            structured
                .locals
                .push(Local { name: Some("__dynamic_kind".into()), type_: ValueType::I64 });
            self.dynamic_kind_local = Some(id);
            let id = LocalId((structured.params.len() + structured.locals.len()) as u32);
            structured
                .locals
                .push(Local { name: Some("__dynamic_tag".into()), type_: ValueType::I32 });
            self.dynamic_tag_local = Some(id);
            let id = LocalId((structured.params.len() + structured.locals.len()) as u32);
            structured
                .locals
                .push(Local { name: Some("__dynamic_field".into()), type_: ValueType::I64 });
            self.dynamic_field_local = Some(id);
            let id = LocalId((structured.params.len() + structured.locals.len()) as u32);
            structured
                .locals
                .push(Local { name: Some("__dynamic_result".into()), type_: ValueType::I32 });
            self.dynamic_result_local = Some(id);
            let id = LocalId((structured.params.len() + structured.locals.len()) as u32);
            structured
                .locals
                .push(Local { name: Some("__dynamic_original".into()), type_: ValueType::I32 });
            self.dynamic_original_local = Some(id);
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
            ExpressionKind::Literal(literal) => self.literal(literal, expression.span, out),
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
            ExpressionKind::Tuple(items) => match self.static_values(items) {
                Ok(fields) => {
                    let object = runtime::tuple_object(self.config, self.next_static_offset, &fields);
                    self.static_pointer(object, out)
                }
                Err(StructuredError::Unsupported) => {
                    self.field_array_value(runtime::ObjectTag::Tuple, items.iter(), out)
                }
                Err(error) => Err(error),
            },
            ExpressionKind::List(items) => match self.static_list(items) {
                Ok(pointer) => {
                    out.push(Instruction::I32Const(pointer as i32));
                    Ok(())
                }
                Err(StructuredError::Unsupported) => self.list_value(items, out),
                Err(error) => Err(error),
            },
            ExpressionKind::Record(record) => {
                match self.static_values(record.fields.iter().map(|field| &field.value)) {
                    Ok(fields) => {
                        let object = runtime::record_object(self.config, self.next_static_offset, &fields);
                        self.static_pointer(object, out)
                    }
                    Err(StructuredError::Unsupported) => self.field_array_value(
                        runtime::ObjectTag::Record,
                        record.fields.iter().map(|field| &field.value),
                        out,
                    ),
                    Err(error) => Err(error),
                }
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
            ExpressionKind::BitArrayConcat { left, right } => self.bit_array_concat(left, right, out),
            ExpressionKind::BitStringDeconstruct { bit_array, .. } => {
                self.managed_tag_test(bit_array, runtime::ObjectTag::BitArray, None, out)
            }
            ExpressionKind::ListDeconstruct { list, head, tail } => self.list_deconstruct(list, *head, *tail, out),
            ExpressionKind::Failure(failure) => self.failure(failure, out),
            ExpressionKind::Memory(operation) => self.memory_operation(operation, out),
            ExpressionKind::IndirectCall(call) => self.indirect_call(call, out),
            ExpressionKind::BitArray(bit_array) => {
                let bytes = bit_array.bytes();
                self.static_pointer(
                    runtime::bit_array_object(self.config, self.next_static_offset, &bytes, bit_array.bit_len),
                    out,
                )
            }
            ExpressionKind::Use(_) => Err(StructuredError::Diagnostics(vec![
                Diagnostic::new(DiagnosticCode::WasmError, "raw `use` IR reached the Wasm backend")
                    .with_label(Label::primary(expression.span, "residual use expression here"))
                    .with_note("`use` must lower to callback-passing call IR before Wasm emission"),
            ])),
        }
    }

    fn literal(&mut self, literal: &ir::IrLiteral, span: Span, out: &mut Vec<Instruction>) -> StructuredResult<()> {
        match literal.kind {
            LiteralKind::Int => {
                let value = literal
                    .source
                    .parse::<i64>()
                    .map_err(|_| literal_parse_diagnostic(literal, span, "signed 64-bit integer"))?;
                out.push(Instruction::I64Const(value));
            }
            LiteralKind::Float => {
                let value = literal
                    .source
                    .parse::<f64>()
                    .map_err(|_| literal_parse_diagnostic(literal, span, "64-bit float"))?;
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
        if self.native_dict_external(call, out)? {
            return Ok(());
        }
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
            "__op_not" => {
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
            "__stdlib_gleam_string_concat" => {
                self.expression(&call.arguments[0].value, out)?;
                self.call_runtime_helper("__string_concat_list", [ValueType::I32], [ValueType::I32], out);
            }
            "__regulus_int_to_string" => {
                self.expression(&call.arguments[0].value, out)?;
                self.call_runtime_helper("__int_to_string", [ValueType::I64], [ValueType::I32], out);
            }
            "__regulus_float_to_string" => {
                self.expression(&call.arguments[0].value, out)?;
                self.call_runtime_helper("__float_to_string", [ValueType::F64], [ValueType::I32], out);
            }
            "__stdlib_gleam_string_length" => {
                self.expression(&call.arguments[0].value, out)?;
                out.push(Instruction::I32Load(MemoryArg::new(self.ensure_memory(), 4, 2)));
                out.push(Instruction::I64ExtendI32U);
            }
            "__stdlib_gleam_string_is_empty" => {
                self.expression(&call.arguments[0].value, out)?;
                out.push(Instruction::I32Load(MemoryArg::new(self.ensure_memory(), 4, 2)));
                out.push(Instruction::I32Eqz);
            }
            "__stdlib_gleam_bit_array_bit_size" => {
                self.expression(&call.arguments[0].value, out)?;
                out.push(Instruction::I32Load(MemoryArg::new(self.ensure_memory(), 4, 2)));
                out.push(Instruction::I64ExtendI32U);
            }
            "__stdlib_gleam_bit_array_byte_size" => {
                self.expression(&call.arguments[0].value, out)?;
                out.push(Instruction::I32Load(MemoryArg::new(self.ensure_memory(), 4, 2)));
                out.push(Instruction::I32Const(7));
                out.push(Instruction::I32Add);
                out.push(Instruction::I32Const(8));
                out.push(Instruction::I32DivS);
                out.push(Instruction::I64ExtendI32U);
            }
            "__stdlib_gleam_bit_array_is_empty" => {
                self.expression(&call.arguments[0].value, out)?;
                out.push(Instruction::I32Load(MemoryArg::new(self.ensure_memory(), 4, 2)));
                out.push(Instruction::I32Eqz);
            }
            "__stdlib_gleam_bit_array_append" => {
                self.binary_arguments(call, out)?;
                self.call_runtime_helper(
                    "__bit_array_append",
                    [ValueType::I32, ValueType::I32],
                    [ValueType::I32],
                    out,
                );
            }
            "__stdlib_gleam_bit_array_concat" => {
                self.expression(&call.arguments[0].value, out)?;
                self.call_runtime_helper("__bit_array_concat_list", [ValueType::I32], [ValueType::I32], out);
            }
            "__stdlib_gleam_bit_array_starts_with" => {
                self.expression(&call.arguments[0].value, out)?;
                out.push(Instruction::I32Const(0));
                self.expression(&call.arguments[1].value, out)?;
                self.call_runtime_helper(
                    "__bit_array_match",
                    [ValueType::I32, ValueType::I32, ValueType::I32],
                    [ValueType::I32],
                    out,
                );
            }
            "__stdlib_gleam_dict_new" => {
                self.call_runtime_helper("__dict_new", [], [ValueType::I32], out);
            }
            "__stdlib_gleam_dict_size" => {
                self.expression(&call.arguments[0].value, out)?;
                self.call_runtime_helper("__dict_size", [ValueType::I32], [ValueType::I64], out);
            }
            "__stdlib_gleam_dict_is_empty" => {
                self.expression(&call.arguments[0].value, out)?;
                self.call_runtime_helper("__dict_is_empty", [ValueType::I32], [ValueType::I32], out);
            }
            "__stdlib_gleam_dict_insert" => self.dict_insert(call, out)?,
            "__stdlib_gleam_dict_get" => self.dict_get(call, out)?,
            "__stdlib_gleam_dict_has_key" => self.dict_has_key(call, out)?,
            "__stdlib_gleam_dict_delete" => self.dict_delete(call, out)?,
            "__stdlib_gleam_dynamic_int" => self.dynamic_i64(call, 1, out)?,
            "__stdlib_gleam_dynamic_float" => self.dynamic_float(call, out)?,
            "__stdlib_gleam_dynamic_bool" => self.dynamic_i32(call, 3, out)?,
            "__stdlib_gleam_dynamic_string" => self.dynamic_i32(call, 4, out)?,
            "__stdlib_gleam_dynamic_bit_array" => self.dynamic_i32(call, 5, out)?,
            "__stdlib_gleam_dynamic_list" => self.dynamic_i32(call, 6, out)?,
            "__stdlib_gleam_dynamic_array" => self.dynamic_i32(call, 9, out)?,
            "__stdlib_gleam_dynamic_nil" => self.dynamic_empty(7, out)?,
            "__stdlib_gleam_dynamic_properties" => {
                self.expression(&call.arguments[0].value, out)?;
                self.call_runtime_helper("__dynamic_properties", [ValueType::I32], [ValueType::I32], out);
            }
            "__stdlib_gleam_dynamic_classify" => self.dynamic_classify(call, out)?,
            "__stdlib_gleam_dynamic_decode_dynamic" => self.decoder(100, 0, out)?,
            "__stdlib_gleam_dynamic_decode_int" => self.decoder(101, 0, out)?,
            "__stdlib_gleam_dynamic_decode_float" => self.decoder(102, 0, out)?,
            "__stdlib_gleam_dynamic_decode_bool" => self.decoder(103, 0, out)?,
            "__stdlib_gleam_dynamic_decode_string" => self.decoder(104, 0, out)?,
            "__stdlib_gleam_dynamic_decode_bit_array" => self.decoder(105, 0, out)?,
            "__stdlib_gleam_dynamic_decode_list" => {
                self.expression(&call.arguments[0].value, out)?;
                self.decoder_from_stack(106, out)?;
            }
            "__stdlib_gleam_dynamic_decode_optional" => {
                self.expression(&call.arguments[0].value, out)?;
                self.decoder_from_stack(107, out)?;
            }
            "__stdlib_gleam_dynamic_decode_at" => {
                self.expression(&call.arguments[0].value, out)?;
                self.expression(&call.arguments[1].value, out)?;
                self.decoder_two_ptrs_from_stack(114, out)?;
            }
            "__stdlib_gleam_dynamic_decode_field" => {
                self.expression_slot_value(&call.arguments[0].value, out)?;
                self.expression(&call.arguments[1].value, out)?;
                self.expression(&call.arguments[2].value, out)?;
                self.decoder_three_slots_from_stack(115, out)?;
            }
            "__stdlib_gleam_dynamic_decode_subfield" => {
                self.expression(&call.arguments[0].value, out)?;
                self.expression(&call.arguments[1].value, out)?;
                self.expression(&call.arguments[2].value, out)?;
                self.decoder_three_slots_from_stack(116, out)?;
            }
            "__stdlib_gleam_dynamic_decode_success" => {
                self.expression_slot_value(&call.arguments[0].value, out)?;
                self.decoder_slot_from_stack(108, out)?;
            }
            "__stdlib_gleam_dynamic_decode_failure" => {
                self.decoder(109, 0, out)?;
            }
            "__stdlib_gleam_dynamic_decode_map" => {
                self.expression(&call.arguments[0].value, out)?;
                self.expression(&call.arguments[1].value, out)?;
                self.decoder_two_ptrs_from_stack(110, out)?;
            }
            "__stdlib_gleam_dynamic_decode_then" => {
                self.expression(&call.arguments[0].value, out)?;
                self.expression(&call.arguments[1].value, out)?;
                self.decoder_two_ptrs_from_stack(111, out)?;
            }
            "__stdlib_gleam_dynamic_decode_one_of" => {
                self.expression(&call.arguments[0].value, out)?;
                self.expression(&call.arguments[1].value, out)?;
                self.decoder_two_ptrs_from_stack(112, out)?;
            }
            "__stdlib_gleam_dynamic_decode_recursive" => {
                self.expression(&call.arguments[0].value, out)?;
                self.decoder_from_stack(113, out)?;
            }
            "__stdlib_gleam_dynamic_decode_run" => {
                if !self.decode_run_inline_combinator(call, out)? {
                    self.decode_run(call, out)?;
                }
            }
            "__stdlib_gleam_io_debug" => self.stdlib_io_debug(call, out)?,
            _ => {
                let signature = self.required_signature(&call.function)?;
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
        let ptr = self.required_local(self.alloc_local, "allocation pointer")?;
        let end = self.required_local(self.alloc_end_local, "allocation end")?;
        let pages = self.required_local(self.alloc_pages_local, "allocation pages")?;
        let heap = self.ensure_heap_global();
        let memory = self.ensure_memory();

        out.push(Instruction::GlobalGet { global: heap, type_: ValueType::I32 });
        out.push(Instruction::LocalSet { local: ptr, type_: ValueType::I32 });
        out.push(Instruction::LocalGet { local: ptr, type_: ValueType::I32 });
        out.push(Instruction::I32Const((u32::MAX - bytes) as i32));
        out.push(Instruction::I32GtU);
        out.push(Instruction::If {
            type_: BlockType::empty(),
            then_body: self.allocation_failure_body(vec![Instruction::I32Const(bytes as i32)]),
            else_body: Vec::new(),
        });
        out.push(Instruction::LocalGet { local: ptr, type_: ValueType::I32 });
        out.push(Instruction::I32Const(bytes as i32));
        out.push(Instruction::I32Add);
        out.push(Instruction::LocalSet { local: end, type_: ValueType::I32 });
        out.push(Instruction::LocalGet { local: end, type_: ValueType::I32 });
        out.push(Instruction::I32Const(-(self.config.layout.alignment as i32)));
        out.push(Instruction::I32GtU);
        out.push(Instruction::If {
            type_: BlockType::empty(),
            then_body: self.allocation_failure_body(vec![Instruction::I32Const(bytes as i32)]),
            else_body: Vec::new(),
        });
        out.push(Instruction::LocalGet { local: end, type_: ValueType::I32 });
        out.push(Instruction::I32Const((self.config.layout.alignment - 1) as i32));
        out.push(Instruction::I32Add);
        out.push(Instruction::I32Const(-(self.config.layout.alignment as i32)));
        out.push(Instruction::I32And);
        out.push(Instruction::LocalSet { local: end, type_: ValueType::I32 });

        self.check_heap_limit(end, vec![Instruction::I32Const(bytes as i32)], out);

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
                    then_body: self.allocation_failure_body(vec![Instruction::I32Const(bytes as i32)]),
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

    fn check_heap_limit(&self, end: LocalId, size: Vec<Instruction>, out: &mut Vec<Instruction>) {
        out.push(Instruction::LocalGet { local: end, type_: ValueType::I32 });
        out.push(Instruction::I32Const(self.config.memory_limit_bytes() as i32));
        out.push(Instruction::I32GtU);
        out.push(Instruction::If {
            type_: BlockType::empty(),
            then_body: self.allocation_failure_body(size),
            else_body: Vec::new(),
        });
    }

    fn allocation_failure_body(&self, size: Vec<Instruction>) -> Vec<Instruction> {
        let mut body = size;
        body.extend([
            Instruction::LocalGet {
                local: self.alloc_local.expect("allocation pointer local must be present"),
                type_: ValueType::I32,
            },
            Instruction::CallName {
                name: "__allocation_fail".into(),
                type_: FunctionType::new([ValueType::I32, ValueType::I32], [ValueType::I32]),
            },
            Instruction::Drop(ValueType::I32),
        ]);
        body
    }

    fn allocate_dynamic(&mut self, out: &mut Vec<Instruction>) -> StructuredResult<()> {
        let ptr = self.required_local(self.alloc_local, "allocation pointer")?;
        let end = self.required_local(self.alloc_end_local, "allocation end")?;
        let pages = self.required_local(self.alloc_pages_local, "allocation pages")?;
        let heap = self.ensure_heap_global();
        let memory = self.ensure_memory();
        out.push(Instruction::LocalSet { local: pages, type_: ValueType::I32 });
        out.push(Instruction::GlobalGet { global: heap, type_: ValueType::I32 });
        out.push(Instruction::LocalSet { local: ptr, type_: ValueType::I32 });
        out.push(Instruction::LocalGet { local: ptr, type_: ValueType::I32 });
        out.push(Instruction::I32Const(-1));
        out.push(Instruction::LocalGet { local: pages, type_: ValueType::I32 });
        out.push(Instruction::I32Sub);
        out.push(Instruction::I32GtU);
        out.push(Instruction::If {
            type_: BlockType::empty(),
            then_body: self
                .allocation_failure_body(vec![Instruction::LocalGet { local: pages, type_: ValueType::I32 }]),
            else_body: Vec::new(),
        });
        out.push(Instruction::LocalGet { local: ptr, type_: ValueType::I32 });
        out.push(Instruction::LocalGet { local: pages, type_: ValueType::I32 });
        out.push(Instruction::I32Add);
        out.push(Instruction::LocalSet { local: end, type_: ValueType::I32 });
        out.push(Instruction::LocalGet { local: end, type_: ValueType::I32 });
        out.push(Instruction::I32Const(-(self.config.layout.alignment as i32)));
        out.push(Instruction::I32GtU);
        out.push(Instruction::If {
            type_: BlockType::empty(),
            then_body: self
                .allocation_failure_body(vec![Instruction::LocalGet { local: pages, type_: ValueType::I32 }]),
            else_body: Vec::new(),
        });
        out.push(Instruction::LocalGet { local: end, type_: ValueType::I32 });
        out.push(Instruction::I32Const((self.config.layout.alignment - 1) as i32));
        out.push(Instruction::I32Add);
        out.push(Instruction::I32Const(-(self.config.layout.alignment as i32)));
        out.push(Instruction::I32And);
        out.push(Instruction::LocalSet { local: end, type_: ValueType::I32 });
        self.check_heap_limit(
            end,
            vec![Instruction::LocalGet { local: pages, type_: ValueType::I32 }],
            out,
        );
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
                Instruction::MemoryGrow(memory),
                Instruction::I32Const(-1),
                Instruction::I32Eq,
                Instruction::If {
                    type_: BlockType::empty(),
                    then_body: self
                        .allocation_failure_body(vec![Instruction::LocalGet { local: pages, type_: ValueType::I32 }]),
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
        self.expression(&call.arguments[0].value, out)?;
        self.expression(&call.arguments[1].value, out)?;
        self.call_runtime_helper(
            "__string_concat",
            [ValueType::I32, ValueType::I32],
            [ValueType::I32],
            out,
        );
        Ok(())
    }

    fn call_runtime_helper(
        &mut self, name: &str, params: impl Into<Vec<ValueType>>, results: impl Into<Vec<ValueType>>,
        out: &mut Vec<Instruction>,
    ) {
        self.runtime_helper_roots.insert(name.into());
        out.push(Instruction::CallName { name: name.into(), type_: FunctionType::new(params, results) });
    }

    fn native_dict_external(&mut self, call: &ir::DirectCall, out: &mut Vec<Instruction>) -> StructuredResult<bool> {
        match native_dict_external_name(&call.function, self.function_ids.contains_key(&call.function)) {
            Some("make") => self.call_runtime_helper("__dict_new", [], [ValueType::I32], out),
            Some("size") => {
                self.expression(&call.arguments[0].value, out)?;
                self.call_runtime_helper("__dict_size", [ValueType::I32], [ValueType::I64], out);
            }
            Some("has") => {
                self.expression(&call.arguments[0].value, out)?;
                self.expression_slot_value(&call.arguments[1].value, out)?;
                self.call_runtime_helper(
                    "__dict_has_key",
                    [ValueType::I32, ValueType::I64],
                    [ValueType::I32],
                    out,
                );
            }
            Some("get") => {
                self.expression(&call.arguments[0].value, out)?;
                self.expression_slot_value(&call.arguments[1].value, out)?;
                self.call_runtime_helper(
                    "__dict_get_result",
                    [ValueType::I32, ValueType::I64],
                    [ValueType::I32],
                    out,
                );
            }
            Some("insert") => self.dict_insert(call, out)?,
            Some("toTransient") | Some("fromTransient") => {
                self.expression(&call.arguments[0].value, out)?;
            }
            Some("destructiveTransientInsert") => {
                self.expression_slot_value(&call.arguments[0].value, out)?;
                self.expression_slot_value(&call.arguments[1].value, out)?;
                self.expression(&call.arguments[2].value, out)?;
                self.call_runtime_helper(
                    "__dict_transient_insert",
                    [ValueType::I64, ValueType::I64, ValueType::I32],
                    [ValueType::I32],
                    out,
                );
            }
            Some("destructiveTransientDelete") => {
                self.expression_slot_value(&call.arguments[0].value, out)?;
                self.expression(&call.arguments[1].value, out)?;
                self.call_runtime_helper(
                    "__dict_transient_delete",
                    [ValueType::I64, ValueType::I32],
                    [ValueType::I32],
                    out,
                );
            }
            Some("destructiveTransientUpdateWith") => return Ok(false),
            Some(_) | None => return Ok(false),
        }
        Ok(true)
    }

    fn dict_insert(&mut self, call: &ir::DirectCall, out: &mut Vec<Instruction>) -> StructuredResult<()> {
        self.expression(&call.arguments[0].value, out)?;
        self.expression_slot_value(&call.arguments[1].value, out)?;
        self.expression_slot_value(&call.arguments[2].value, out)?;
        self.call_runtime_helper(
            "__dict_insert",
            [ValueType::I32, ValueType::I64, ValueType::I64],
            [ValueType::I32],
            out,
        );
        Ok(())
    }

    fn dict_get(&mut self, call: &ir::DirectCall, out: &mut Vec<Instruction>) -> StructuredResult<()> {
        self.expression(&call.arguments[0].value, out)?;
        self.expression_slot_value(&call.arguments[1].value, out)?;
        self.call_runtime_helper("__dict_get", [ValueType::I32, ValueType::I64], [ValueType::I32], out);
        Ok(())
    }

    fn dict_has_key(&mut self, call: &ir::DirectCall, out: &mut Vec<Instruction>) -> StructuredResult<()> {
        self.expression(&call.arguments[0].value, out)?;
        self.expression_slot_value(&call.arguments[1].value, out)?;
        self.call_runtime_helper(
            "__dict_has_key",
            [ValueType::I32, ValueType::I64],
            [ValueType::I32],
            out,
        );
        Ok(())
    }

    fn dict_delete(&mut self, call: &ir::DirectCall, out: &mut Vec<Instruction>) -> StructuredResult<()> {
        self.expression(&call.arguments[0].value, out)?;
        self.expression_slot_value(&call.arguments[1].value, out)?;
        self.call_runtime_helper("__dict_delete", [ValueType::I32, ValueType::I64], [ValueType::I32], out);
        Ok(())
    }

    fn dynamic_classify(&mut self, call: &ir::DirectCall, out: &mut Vec<Instruction>) -> StructuredResult<()> {
        let names = [
            (1, "Int"),
            (2, "Float"),
            (3, "Bool"),
            (4, "String"),
            (5, "BitArray"),
            (6, "List"),
            (7, "Nil"),
            (8, "Dict"),
            (9, "Array"),
        ];
        let unknown = self.push_static(runtime::string_object(self.config, self.next_static_offset, "Unknown"));
        let mut else_body = vec![Instruction::I32Const(unknown as i32)];
        for (tag, name) in names.into_iter().rev() {
            let ptr = self.push_static(runtime::string_object(self.config, self.next_static_offset, name));
            let mut condition = Vec::new();
            self.expression(&call.arguments[0].value, &mut condition)?;
            condition.push(Instruction::I32Load(MemoryArg::new(self.ensure_memory(), 8, 2)));
            condition.push(Instruction::I32Const(tag));
            condition.push(Instruction::I32Eq);
            condition.push(Instruction::If {
                type_: BlockType::new([], [ValueType::I32]),
                then_body: vec![Instruction::I32Const(ptr as i32)],
                else_body,
            });
            else_body = condition;
        }
        out.extend(else_body);
        Ok(())
    }

    fn dynamic_i64(&mut self, call: &ir::DirectCall, tag: i32, out: &mut Vec<Instruction>) -> StructuredResult<()> {
        self.expression(&call.arguments[0].value, out)?;
        self.dynamic_i64_from_stack(tag, out)
    }

    fn dynamic_float(&mut self, call: &ir::DirectCall, out: &mut Vec<Instruction>) -> StructuredResult<()> {
        self.expression(&call.arguments[0].value, out)?;
        out.push(Instruction::I64ReinterpretF64);
        self.dynamic_i64_from_stack(2, out)
    }

    fn dynamic_i32(&mut self, call: &ir::DirectCall, tag: i32, out: &mut Vec<Instruction>) -> StructuredResult<()> {
        self.expression(&call.arguments[0].value, out)?;
        out.push(Instruction::I64ExtendI32U);
        self.dynamic_i64_from_stack(tag, out)
    }

    fn dynamic_i64_from_stack(&mut self, tag: i32, out: &mut Vec<Instruction>) -> StructuredResult<()> {
        let field = self.required_local(self.dynamic_field_local, "dynamic field")?;
        out.push(Instruction::LocalSet { local: field, type_: ValueType::I64 });
        self.custom_value(tag, 1, [(12, field)], out)
    }

    fn dynamic_empty(&mut self, tag: i32, out: &mut Vec<Instruction>) -> StructuredResult<()> {
        self.custom_value(tag, 0, [], out)
    }

    fn decoder(&mut self, kind: i64, inner: i32, out: &mut Vec<Instruction>) -> StructuredResult<()> {
        out.push(Instruction::I32Const(inner));
        self.decoder_from_stack(kind, out)
    }

    fn decoder_from_stack(&mut self, kind: i64, out: &mut Vec<Instruction>) -> StructuredResult<()> {
        let decoder = self.required_local(self.dynamic_decoder_local, "dynamic decoder")?;
        out.push(Instruction::LocalSet { local: decoder, type_: ValueType::I32 });
        let field = self.required_local(self.dynamic_field_local, "dynamic field")?;
        out.push(Instruction::I64Const(kind));
        out.push(Instruction::LocalSet { local: field, type_: ValueType::I64 });
        self.custom_value(200, 2, [(12, field)], out)?;
        let ptr = self.required_local(self.alloc_local, "allocation pointer")?;
        out.push(Instruction::LocalSet { local: ptr, type_: ValueType::I32 });
        out.push(Instruction::LocalGet { local: ptr, type_: ValueType::I32 });
        out.push(Instruction::I32Const(20));
        out.push(Instruction::I32Add);
        out.push(Instruction::LocalGet { local: decoder, type_: ValueType::I32 });
        out.push(Instruction::I64ExtendI32U);
        out.push(Instruction::I64Store(MemoryArg::new(self.ensure_memory(), 0, 3)));
        out.push(Instruction::LocalGet { local: ptr, type_: ValueType::I32 });
        Ok(())
    }

    fn decoder_slot_from_stack(&mut self, kind: i64, out: &mut Vec<Instruction>) -> StructuredResult<()> {
        let field = self.required_local(self.dynamic_field_local, "dynamic field")?;
        out.push(Instruction::LocalSet { local: field, type_: ValueType::I64 });
        let decoder = self.required_local(self.dynamic_kind_local, "dynamic kind")?;
        out.push(Instruction::I64Const(kind));
        out.push(Instruction::LocalSet { local: decoder, type_: ValueType::I64 });
        self.custom_value(200, 2, [(12, decoder)], out)?;
        let ptr = self.required_local(self.alloc_local, "allocation pointer")?;
        out.push(Instruction::LocalSet { local: ptr, type_: ValueType::I32 });
        out.push(Instruction::LocalGet { local: ptr, type_: ValueType::I32 });
        out.push(Instruction::I32Const(20));
        out.push(Instruction::I32Add);
        out.push(Instruction::LocalGet { local: field, type_: ValueType::I64 });
        out.push(Instruction::I64Store(MemoryArg::new(self.ensure_memory(), 0, 3)));
        out.push(Instruction::LocalGet { local: ptr, type_: ValueType::I32 });
        Ok(())
    }

    fn decoder_two_ptrs_from_stack(&mut self, kind: i64, out: &mut Vec<Instruction>) -> StructuredResult<()> {
        let data = self.required_local(self.dynamic_data_local, "dynamic data")?;
        let decoder = self.required_local(self.dynamic_decoder_local, "dynamic decoder")?;
        out.push(Instruction::LocalSet { local: decoder, type_: ValueType::I32 });
        out.push(Instruction::LocalSet { local: data, type_: ValueType::I32 });
        let field = self.required_local(self.dynamic_field_local, "dynamic field")?;
        out.push(Instruction::I64Const(kind));
        out.push(Instruction::LocalSet { local: field, type_: ValueType::I64 });
        self.custom_value(200, 3, [(12, field)], out)?;
        let ptr = self.required_local(self.alloc_local, "allocation pointer")?;
        out.push(Instruction::LocalSet { local: ptr, type_: ValueType::I32 });
        out.push(Instruction::LocalGet { local: ptr, type_: ValueType::I32 });
        out.push(Instruction::I32Const(20));
        out.push(Instruction::I32Add);
        out.push(Instruction::LocalGet { local: data, type_: ValueType::I32 });
        out.push(Instruction::I64ExtendI32U);
        out.push(Instruction::I64Store(MemoryArg::new(self.ensure_memory(), 0, 3)));
        out.push(Instruction::LocalGet { local: ptr, type_: ValueType::I32 });
        out.push(Instruction::I32Const(28));
        out.push(Instruction::I32Add);
        out.push(Instruction::LocalGet { local: decoder, type_: ValueType::I32 });
        out.push(Instruction::I64ExtendI32U);
        out.push(Instruction::I64Store(MemoryArg::new(self.ensure_memory(), 0, 3)));
        out.push(Instruction::LocalGet { local: ptr, type_: ValueType::I32 });
        Ok(())
    }

    fn decoder_three_slots_from_stack(&mut self, kind: i64, out: &mut Vec<Instruction>) -> StructuredResult<()> {
        let data = self.required_local(self.dynamic_data_local, "dynamic data")?;
        let decoder = self.required_local(self.dynamic_decoder_local, "dynamic decoder")?;
        let field = self.required_local(self.dynamic_field_local, "dynamic field")?;
        out.push(Instruction::LocalSet { local: decoder, type_: ValueType::I32 });
        out.push(Instruction::LocalSet { local: data, type_: ValueType::I32 });
        out.push(Instruction::LocalSet { local: field, type_: ValueType::I64 });
        let kind_local = self.required_local(self.dynamic_kind_local, "dynamic kind")?;
        out.push(Instruction::I64Const(kind));
        out.push(Instruction::LocalSet { local: kind_local, type_: ValueType::I64 });
        self.custom_value(200, 4, [(12, kind_local)], out)?;
        let ptr = self.required_local(self.alloc_local, "allocation pointer")?;
        out.push(Instruction::LocalSet { local: ptr, type_: ValueType::I32 });
        for (offset, local, type_) in [
            (20, field, ValueType::I64),
            (28, data, ValueType::I32),
            (36, decoder, ValueType::I32),
        ] {
            out.push(Instruction::LocalGet { local: ptr, type_: ValueType::I32 });
            out.push(Instruction::I32Const(offset));
            out.push(Instruction::I32Add);
            out.push(Instruction::LocalGet { local, type_ });
            if type_ == ValueType::I32 {
                out.push(Instruction::I64ExtendI32U);
            }
            out.push(Instruction::I64Store(MemoryArg::new(self.ensure_memory(), 0, 3)));
        }
        out.push(Instruction::LocalGet { local: ptr, type_: ValueType::I32 });
        Ok(())
    }

    fn custom_value<const N: usize>(
        &mut self, tag: i32, fields: u32, values: [(u32, LocalId); N], out: &mut Vec<Instruction>,
    ) -> StructuredResult<()> {
        let size = self.config.layout.custom_size(fields, 8);
        self.allocate(size, out)?;
        let ptr = self.required_local(self.alloc_local, "allocation pointer")?;
        out.push(Instruction::LocalSet { local: ptr, type_: ValueType::I32 });
        out.push(Instruction::LocalGet { local: ptr, type_: ValueType::I32 });
        out.push(Instruction::I32Const(u32::from(runtime::ObjectTag::Custom) as i32));
        out.push(Instruction::I32Store(MemoryArg::new(self.ensure_memory(), 0, 2)));
        out.push(Instruction::LocalGet { local: ptr, type_: ValueType::I32 });
        out.push(Instruction::I32Const(fields as i32));
        out.push(Instruction::I32Store(MemoryArg::new(self.ensure_memory(), 4, 2)));
        out.push(Instruction::LocalGet { local: ptr, type_: ValueType::I32 });
        out.push(Instruction::I32Const(tag));
        out.push(Instruction::I32Store(MemoryArg::new(self.ensure_memory(), 8, 2)));
        for (offset, local) in values {
            out.push(Instruction::LocalGet { local: ptr, type_: ValueType::I32 });
            out.push(Instruction::LocalGet { local, type_: ValueType::I64 });
            out.push(Instruction::I64Store(MemoryArg::new(self.ensure_memory(), offset, 3)));
        }
        out.push(Instruction::LocalGet { local: ptr, type_: ValueType::I32 });
        Ok(())
    }

    fn decode_run(&mut self, call: &ir::DirectCall, out: &mut Vec<Instruction>) -> StructuredResult<()> {
        let data = self.required_local(self.dynamic_data_local, "dynamic data")?;
        let decoder = self.required_local(self.dynamic_decoder_local, "dynamic decoder")?;
        self.expression(&call.arguments[0].value, out)?;
        out.push(Instruction::LocalSet { local: data, type_: ValueType::I32 });
        self.expression(&call.arguments[1].value, out)?;
        out.push(Instruction::LocalSet { local: decoder, type_: ValueType::I32 });
        self.decode_run_loaded(out)
    }

    fn decode_run_loaded(&mut self, out: &mut Vec<Instruction>) -> StructuredResult<()> {
        self.decode_run_loaded_with_depth(out, 4)
    }

    fn decode_run_loaded_with_depth(&mut self, out: &mut Vec<Instruction>, depth: usize) -> StructuredResult<()> {
        let data = self.required_local(self.dynamic_data_local, "dynamic data")?;
        let decoder = self.required_local(self.dynamic_decoder_local, "dynamic decoder")?;
        let kind = self.required_local(self.dynamic_kind_local, "dynamic kind")?;
        let tag = self.required_local(self.dynamic_tag_local, "dynamic tag")?;
        let field = self.required_local(self.dynamic_field_local, "dynamic field")?;
        let result = self.required_local(self.dynamic_result_local, "dynamic result")?;
        out.push(Instruction::LocalGet { local: decoder, type_: ValueType::I32 });
        out.push(Instruction::I64Load(MemoryArg::new(self.ensure_memory(), 12, 3)));
        out.push(Instruction::LocalSet { local: kind, type_: ValueType::I64 });
        let mut primitive_then = Vec::new();
        self.decode_error(&mut primitive_then)?;
        primitive_then.push(Instruction::LocalSet { local: result, type_: ValueType::I32 });
        let mut primitive_else = vec![
            Instruction::LocalGet { local: data, type_: ValueType::I32 },
            Instruction::I32Load(MemoryArg::new(self.ensure_memory(), 8, 2)),
            Instruction::LocalSet { local: tag, type_: ValueType::I32 },
            Instruction::LocalGet { local: data, type_: ValueType::I32 },
            Instruction::I64Load(MemoryArg::new(self.ensure_memory(), 12, 3)),
            Instruction::LocalSet { local: field, type_: ValueType::I64 },
        ];
        self.decode_kind_chain(
            &mut primitive_else,
            DecodeLocals { result, kind, tag, field, data },
            &[(100, 0), (101, 1), (102, 2), (103, 3), (104, 4), (105, 5), (106, 6)],
        )?;
        let primitive_body = vec![
            Instruction::LocalGet { local: data, type_: ValueType::I32 },
            Instruction::I32Eqz,
            Instruction::If { type_: BlockType::empty(), then_body: primitive_then, else_body: primitive_else },
        ];
        let mut failure_body = Vec::new();
        self.decode_error(&mut failure_body)?;
        failure_body.push(Instruction::LocalSet { local: result, type_: ValueType::I32 });
        let failure_or_primitive = vec![
            Instruction::LocalGet { local: kind, type_: ValueType::I64 },
            Instruction::I64Const(109),
            Instruction::I64Eq,
            Instruction::If { type_: BlockType::empty(), then_body: failure_body, else_body: primitive_body },
        ];
        let fallback = if depth > 0 {
            let mut at_body = Vec::new();
            self.decode_run_loaded_at(&mut at_body, depth - 1)?;
            at_body.push(Instruction::LocalSet { local: result, type_: ValueType::I32 });
            vec![
                Instruction::LocalGet { local: kind, type_: ValueType::I64 },
                Instruction::I64Const(114),
                Instruction::I64Eq,
                Instruction::If { type_: BlockType::empty(), then_body: at_body, else_body: failure_or_primitive },
            ]
        } else {
            failure_or_primitive
        };
        let mut success_body = vec![
            Instruction::LocalGet { local: decoder, type_: ValueType::I32 },
            Instruction::I64Load(MemoryArg::new(self.ensure_memory(), 20, 3)),
        ];
        self.decode_ok_from_stack(&mut success_body)?;
        success_body.push(Instruction::LocalSet { local: result, type_: ValueType::I32 });
        out.extend([
            Instruction::LocalGet { local: kind, type_: ValueType::I64 },
            Instruction::I64Const(108),
            Instruction::I64Eq,
            Instruction::If { type_: BlockType::empty(), then_body: success_body, else_body: fallback },
        ]);
        out.push(Instruction::LocalGet { local: result, type_: ValueType::I32 });
        Ok(())
    }

    fn decode_run_loaded_at(&mut self, out: &mut Vec<Instruction>, depth: usize) -> StructuredResult<()> {
        let data = self.required_local(self.dynamic_data_local, "dynamic data")?;
        let decoder = self.required_local(self.dynamic_decoder_local, "dynamic decoder")?;
        let path = self.required_local(self.aggregate_local, "decoder path")?;
        out.extend([
            Instruction::LocalGet { local: decoder, type_: ValueType::I32 },
            Instruction::I64Load(MemoryArg::new(self.ensure_memory(), 20, 3)),
            Instruction::I32WrapI64,
            Instruction::LocalSet { local: path, type_: ValueType::I32 },
        ]);
        let mut loop_body = Vec::new();
        loop_body.extend([
            Instruction::LocalGet { local: path, type_: ValueType::I32 },
            Instruction::I32Eqz,
            Instruction::BrIf { depth: 1, results: Vec::new() },
            Instruction::LocalGet { local: data, type_: ValueType::I32 },
            Instruction::LocalGet { local: path, type_: ValueType::I32 },
        ]);
        self.call_runtime_helper("__list_head", [ValueType::I32], [ValueType::I64], &mut loop_body);
        loop_body.push(Instruction::I32Const(0));
        self.call_runtime_helper(
            "__dynamic_lookup",
            [ValueType::I32, ValueType::I64, ValueType::I32],
            [ValueType::I32],
            &mut loop_body,
        );
        loop_body.extend([
            Instruction::LocalSet { local: data, type_: ValueType::I32 },
            Instruction::LocalGet { local: path, type_: ValueType::I32 },
        ]);
        self.call_runtime_helper("__list_tail", [ValueType::I32], [ValueType::I32], &mut loop_body);
        loop_body.extend([
            Instruction::LocalSet { local: path, type_: ValueType::I32 },
            Instruction::Br { depth: 0, results: Vec::new() },
        ]);
        out.push(Instruction::Block {
            type_: BlockType::empty(),
            body: vec![Instruction::Loop { type_: BlockType::empty(), body: loop_body }],
        });
        out.extend([
            Instruction::LocalGet { local: decoder, type_: ValueType::I32 },
            Instruction::I64Load(MemoryArg::new(self.ensure_memory(), 28, 3)),
            Instruction::I32WrapI64,
            Instruction::LocalSet { local: decoder, type_: ValueType::I32 },
        ]);
        self.decode_run_loaded_with_depth(out, depth)
    }

    fn decode_run_inline_combinator(
        &mut self, call: &ir::DirectCall, out: &mut Vec<Instruction>,
    ) -> StructuredResult<bool> {
        let Some(decoder) = call.arguments.get(1).map(|argument| &argument.value) else {
            return Ok(false);
        };
        let ExpressionKind::DirectCall(combinator) = &decoder.kind else {
            return Ok(false);
        };
        match combinator.function.as_str() {
            "__stdlib_gleam_dynamic_decode_at" => {
                if !matches!(
                    combinator.arguments.first().map(|argument| &argument.value.kind),
                    Some(ExpressionKind::List(_))
                ) {
                    return Ok(false);
                }
                self.decode_run_at(&call.arguments[0].value, combinator, out)?;
                Ok(true)
            }
            "__stdlib_gleam_dynamic_decode_field" => {
                self.decode_run_field(&call.arguments[0].value, combinator, false, out)?;
                Ok(true)
            }
            "__stdlib_gleam_dynamic_decode_subfield" => {
                if !matches!(
                    combinator.arguments.first().map(|argument| &argument.value.kind),
                    Some(ExpressionKind::List(_))
                ) {
                    return Ok(false);
                }
                self.decode_run_field(&call.arguments[0].value, combinator, true, out)?;
                Ok(true)
            }
            "__stdlib_gleam_dynamic_decode_map" => {
                self.decode_run_map(&call.arguments[0].value, combinator, out)?;
                Ok(true)
            }
            "__stdlib_gleam_dynamic_decode_then" => {
                self.decode_run_then(&call.arguments[0].value, combinator, out)?;
                Ok(true)
            }
            "__stdlib_gleam_dynamic_decode_recursive" => {
                self.decode_run_recursive(&call.arguments[0].value, combinator, out)?;
                Ok(true)
            }
            "__stdlib_gleam_dynamic_decode_one_of" => {
                if !matches!(
                    combinator.arguments.get(1).map(|argument| &argument.value.kind),
                    Some(ExpressionKind::List(_))
                ) {
                    return Ok(false);
                }
                self.decode_run_one_of(&call.arguments[0].value, combinator, out)?;
                Ok(true)
            }
            _ => Ok(false),
        }
    }

    fn decode_run_map(
        &mut self, data_expr: &ir::Expression, map_call: &ir::DirectCall, out: &mut Vec<Instruction>,
    ) -> StructuredResult<()> {
        let data = self.required_local(self.dynamic_data_local, "dynamic data")?;
        let decoder = self.required_local(self.dynamic_decoder_local, "dynamic decoder")?;
        let result = self.required_local(self.dynamic_result_local, "dynamic result")?;
        self.expression(data_expr, out)?;
        out.push(Instruction::LocalSet { local: data, type_: ValueType::I32 });
        self.expression(&map_call.arguments[0].value, out)?;
        out.push(Instruction::LocalSet { local: decoder, type_: ValueType::I32 });
        self.decode_run_loaded(out)?;
        out.push(Instruction::LocalSet { local: result, type_: ValueType::I32 });
        out.extend([
            Instruction::LocalGet { local: result, type_: ValueType::I32 },
            Instruction::I32Load(MemoryArg::new(self.ensure_memory(), 8, 2)),
            Instruction::I32Const(1115088027),
            Instruction::I32Eq,
        ]);
        let mut then_body = Vec::new();
        self.decode_map_ok(&map_call.arguments[1].value, result, &mut then_body)?;
        let else_body = vec![Instruction::LocalGet { local: result, type_: ValueType::I32 }];
        out.push(Instruction::If { type_: BlockType::new([], [ValueType::I32]), then_body, else_body });
        Ok(())
    }

    fn decode_run_at(
        &mut self, data_expr: &ir::Expression, at_call: &ir::DirectCall, out: &mut Vec<Instruction>,
    ) -> StructuredResult<()> {
        let path = self.static_path_items(&at_call.arguments[0].value)?;
        let data = self.required_local(self.dynamic_data_local, "dynamic data")?;
        let decoder = self.required_local(self.dynamic_decoder_local, "dynamic decoder")?;
        self.expression(data_expr, out)?;
        out.push(Instruction::LocalSet { local: data, type_: ValueType::I32 });
        for key in path {
            self.dynamic_lookup(data, key, out)?;
            out.push(Instruction::LocalSet { local: data, type_: ValueType::I32 });
        }
        self.expression(&at_call.arguments[1].value, out)?;
        out.push(Instruction::LocalSet { local: decoder, type_: ValueType::I32 });
        self.decode_run_loaded(out)
    }

    fn decode_run_field(
        &mut self, data_expr: &ir::Expression, field_call: &ir::DirectCall, subfield: bool, out: &mut Vec<Instruction>,
    ) -> StructuredResult<()> {
        let data = self.required_local(self.dynamic_data_local, "dynamic data")?;
        let original = self.required_local(self.dynamic_original_local, "dynamic original")?;
        let decoder = self.required_local(self.dynamic_decoder_local, "dynamic decoder")?;
        let result = self.required_local(self.dynamic_result_local, "dynamic result")?;
        self.expression(data_expr, out)?;
        out.push(Instruction::LocalTee { local: original, type_: ValueType::I32 });
        out.push(Instruction::LocalSet { local: data, type_: ValueType::I32 });
        let path = if subfield {
            self.static_path_items(&field_call.arguments[0].value)?
        } else {
            vec![&field_call.arguments[0].value]
        };
        for key in path {
            self.dynamic_lookup(data, key, out)?;
            out.push(Instruction::LocalSet { local: data, type_: ValueType::I32 });
        }
        self.expression(&field_call.arguments[1].value, out)?;
        out.push(Instruction::LocalSet { local: decoder, type_: ValueType::I32 });
        self.decode_run_loaded(out)?;
        out.push(Instruction::LocalSet { local: result, type_: ValueType::I32 });
        out.extend([
            Instruction::LocalGet { local: result, type_: ValueType::I32 },
            Instruction::I32Load(MemoryArg::new(self.ensure_memory(), 8, 2)),
            Instruction::I32Const(1115088027),
            Instruction::I32Eq,
        ]);
        let mut then_body = Vec::new();
        self.decode_field_ok(&field_call.arguments[2].value, result, &mut then_body)?;
        then_body.push(Instruction::LocalSet { local: decoder, type_: ValueType::I32 });
        then_body.push(Instruction::LocalGet { local: original, type_: ValueType::I32 });
        then_body.push(Instruction::LocalSet { local: data, type_: ValueType::I32 });
        self.decode_run_loaded(&mut then_body)?;
        let else_body = vec![Instruction::LocalGet { local: result, type_: ValueType::I32 }];
        out.push(Instruction::If { type_: BlockType::new([], [ValueType::I32]), then_body, else_body });
        Ok(())
    }

    fn decode_field_ok(
        &mut self, next: &ir::Expression, result: LocalId, out: &mut Vec<Instruction>,
    ) -> StructuredResult<()> {
        let Type::Function { params, return_type } = &next.type_ else {
            return Err(StructuredError::Invariant(
                "decode.field callback must have function type".into(),
            ));
        };
        let Some(param) = params.first() else {
            return Err(StructuredError::Invariant(
                "decode.field callback must accept one parameter".into(),
            ));
        };
        self.call_decoder_closure(next, result, param, return_type, out)
    }

    fn static_path_items<'b>(&self, path: &'b ir::Expression) -> StructuredResult<Vec<&'b ir::Expression>> {
        match &path.kind {
            ExpressionKind::List(items) => Ok(items.iter().collect()),
            _ => Err(StructuredError::Invariant(
                "dynamic decoder path direct path requires a static list".into(),
            )),
        }
    }

    fn dynamic_lookup(
        &mut self, container: LocalId, key: &ir::Expression, out: &mut Vec<Instruction>,
    ) -> StructuredResult<()> {
        out.push(Instruction::LocalGet { local: container, type_: ValueType::I32 });
        match &key.type_ {
            Type::Int => {
                self.expression(key, out)?;
                out.push(Instruction::I32Const(1));
            }
            Type::String => {
                self.expression(key, out)?;
                out.push(Instruction::I64ExtendI32U);
                out.push(Instruction::I32Const(4));
            }
            Type::Custom { name, .. } if name == "Dynamic" => {
                self.expression(key, out)?;
                out.push(Instruction::I64ExtendI32U);
                out.push(Instruction::I32Const(0));
            }
            _ => {
                return Err(StructuredError::Diagnostics(vec![Diagnostic::spanned(
                    DiagnosticCode::WasmError,
                    "unsupported dynamic decoder path segment",
                    key.span,
                    "path segment must be Int, String, or Dynamic here",
                )]));
            }
        }
        self.call_runtime_helper(
            "__dynamic_lookup",
            [ValueType::I32, ValueType::I64, ValueType::I32],
            [ValueType::I32],
            out,
        );
        Ok(())
    }

    fn decode_run_then(
        &mut self, data_expr: &ir::Expression, then_call: &ir::DirectCall, out: &mut Vec<Instruction>,
    ) -> StructuredResult<()> {
        let data = self.required_local(self.dynamic_data_local, "dynamic data")?;
        let decoder = self.required_local(self.dynamic_decoder_local, "dynamic decoder")?;
        let result = self.required_local(self.dynamic_result_local, "dynamic result")?;
        self.expression(data_expr, out)?;
        out.push(Instruction::LocalSet { local: data, type_: ValueType::I32 });
        self.expression(&then_call.arguments[0].value, out)?;
        out.push(Instruction::LocalSet { local: decoder, type_: ValueType::I32 });
        self.decode_run_loaded(out)?;
        out.push(Instruction::LocalSet { local: result, type_: ValueType::I32 });
        out.extend([
            Instruction::LocalGet { local: result, type_: ValueType::I32 },
            Instruction::I32Load(MemoryArg::new(self.ensure_memory(), 8, 2)),
            Instruction::I32Const(1115088027),
            Instruction::I32Eq,
        ]);
        let mut then_body = Vec::new();
        self.decode_then_ok(&then_call.arguments[1].value, result, &mut then_body)?;
        let else_body = vec![Instruction::LocalGet { local: result, type_: ValueType::I32 }];
        out.push(Instruction::If { type_: BlockType::new([], [ValueType::I32]), then_body, else_body });
        Ok(())
    }

    fn decode_run_recursive(
        &mut self, data_expr: &ir::Expression, recursive_call: &ir::DirectCall, out: &mut Vec<Instruction>,
    ) -> StructuredResult<()> {
        let data = self.required_local(self.dynamic_data_local, "dynamic data")?;
        let decoder = self.required_local(self.dynamic_decoder_local, "dynamic decoder")?;
        self.expression(data_expr, out)?;
        out.push(Instruction::LocalSet { local: data, type_: ValueType::I32 });
        self.call_decoder_thunk(&recursive_call.arguments[0].value, out)?;
        out.push(Instruction::LocalSet { local: decoder, type_: ValueType::I32 });
        self.decode_run_loaded(out)
    }

    fn decode_run_one_of(
        &mut self, data_expr: &ir::Expression, one_of_call: &ir::DirectCall, out: &mut Vec<Instruction>,
    ) -> StructuredResult<()> {
        let fallbacks = match &one_of_call.arguments[1].value.kind {
            ExpressionKind::List(items) => items.as_slice(),
            _ => {
                return Err(StructuredError::Invariant(
                    "decode.one_of direct path requires a static fallback list".into(),
                ));
            }
        };
        let data = self.required_local(self.dynamic_data_local, "dynamic data")?;
        let decoder = self.required_local(self.dynamic_decoder_local, "dynamic decoder")?;
        let result = self.required_local(self.dynamic_result_local, "dynamic result")?;
        self.expression(data_expr, out)?;
        out.push(Instruction::LocalSet { local: data, type_: ValueType::I32 });
        self.expression(&one_of_call.arguments[0].value, out)?;
        out.push(Instruction::LocalSet { local: decoder, type_: ValueType::I32 });
        self.decode_run_loaded(out)?;
        out.push(Instruction::LocalSet { local: result, type_: ValueType::I32 });
        for fallback in fallbacks {
            out.extend([
                Instruction::LocalGet { local: result, type_: ValueType::I32 },
                Instruction::I32Load(MemoryArg::new(self.ensure_memory(), 8, 2)),
                Instruction::I32Const(1115088027),
                Instruction::I32Eq,
                Instruction::I32Eqz,
            ]);
            let mut then_body = Vec::new();
            self.expression(fallback, &mut then_body)?;
            then_body.push(Instruction::LocalSet { local: decoder, type_: ValueType::I32 });
            self.decode_run_loaded(&mut then_body)?;
            then_body.push(Instruction::LocalSet { local: result, type_: ValueType::I32 });
            out.push(Instruction::If { type_: BlockType::empty(), then_body, else_body: Vec::new() });
        }
        out.push(Instruction::LocalGet { local: result, type_: ValueType::I32 });
        Ok(())
    }

    fn decode_map_ok(
        &mut self, mapper: &ir::Expression, result: LocalId, out: &mut Vec<Instruction>,
    ) -> StructuredResult<()> {
        let Type::Function { params, return_type } = &mapper.type_ else {
            return Err(StructuredError::Invariant(
                "decode.map mapper must have function type".into(),
            ));
        };
        let Some(param) = params.first() else {
            return Err(StructuredError::Invariant(
                "decode.map mapper must accept one parameter".into(),
            ));
        };
        self.call_decoder_closure(mapper, result, param, return_type, out)?;
        self.value_to_slot(return_type, mapper.span, out)?;
        self.decode_ok_from_stack(out)
    }

    fn decode_then_ok(
        &mut self, next: &ir::Expression, result: LocalId, out: &mut Vec<Instruction>,
    ) -> StructuredResult<()> {
        let Type::Function { params, return_type } = &next.type_ else {
            return Err(StructuredError::Invariant(
                "decode.then callback must have function type".into(),
            ));
        };
        let Some(param) = params.first() else {
            return Err(StructuredError::Invariant(
                "decode.then callback must accept one parameter".into(),
            ));
        };
        self.call_decoder_closure(next, result, param, return_type, out)?;
        out.push(Instruction::LocalSet {
            local: self.required_local(self.dynamic_decoder_local, "dynamic decoder")?,
            type_: ValueType::I32,
        });
        self.decode_run_loaded(out)
    }

    fn call_decoder_closure(
        &mut self, closure: &ir::Expression, result: LocalId, param: &Type, return_type: &Type,
        out: &mut Vec<Instruction>,
    ) -> StructuredResult<()> {
        let scratch = self.required_local(self.scratch_local, "scratch")?;
        let funcid = self.funcid_locals.first().copied().ok_or_else(|| {
            StructuredError::Invariant("dynamic decoder closure dispatch needs function id local".into())
        })?;
        let table = self.func_table.ok_or_else(|| {
            StructuredError::Invariant("dynamic decoder closure dispatch needs function table".into())
        })?;
        self.expression(closure, out)?;
        out.push(Instruction::LocalSet { local: scratch, type_: ValueType::I32 });
        out.push(Instruction::LocalGet { local: scratch, type_: ValueType::I32 });
        out.push(Instruction::I32Load(MemoryArg::new(
            self.ensure_memory(),
            u32::from(ClosureConstants::FunctionIdOffset),
            2,
        )));
        out.push(Instruction::LocalSet { local: funcid, type_: ValueType::I32 });
        out.push(Instruction::LocalGet { local: scratch, type_: ValueType::I32 });
        out.push(Instruction::LocalGet { local: result, type_: ValueType::I32 });
        out.push(Instruction::I64Load(MemoryArg::new(self.ensure_memory(), 12, 3)));
        self.slot_to_value(param, closure.span, out)?;
        out.push(Instruction::LocalGet { local: funcid, type_: ValueType::I32 });
        let mut params = vec![ValueType::I32];
        if !matches!(param, Type::Nil) {
            params.push(value_type(param, closure.span)?);
        }
        let results = result_types(return_type, closure.span)?;
        let type_ = FunctionType::new(params, results);
        let type_id = self.module.intern_type(type_.clone());
        out.push(Instruction::CallIndirect { table, type_id, type_ });
        Ok(())
    }

    fn call_decoder_thunk(&mut self, closure: &ir::Expression, out: &mut Vec<Instruction>) -> StructuredResult<()> {
        let Type::Function { params, return_type } = &closure.type_ else {
            return Err(StructuredError::Invariant(
                "decode.recursive callback must have function type".into(),
            ));
        };
        if !params.is_empty() {
            return Err(StructuredError::Invariant(
                "decode.recursive callback must accept no parameters".into(),
            ));
        }
        let scratch = self.required_local(self.scratch_local, "scratch")?;
        let funcid = self.funcid_locals.first().copied().ok_or_else(|| {
            StructuredError::Invariant("dynamic decoder thunk dispatch needs function id local".into())
        })?;
        let table = self
            .func_table
            .ok_or_else(|| StructuredError::Invariant("dynamic decoder thunk dispatch needs function table".into()))?;
        self.expression(closure, out)?;
        out.push(Instruction::LocalSet { local: scratch, type_: ValueType::I32 });
        out.push(Instruction::LocalGet { local: scratch, type_: ValueType::I32 });
        out.push(Instruction::I32Load(MemoryArg::new(
            self.ensure_memory(),
            u32::from(ClosureConstants::FunctionIdOffset),
            2,
        )));
        out.push(Instruction::LocalSet { local: funcid, type_: ValueType::I32 });
        out.push(Instruction::LocalGet { local: scratch, type_: ValueType::I32 });
        out.push(Instruction::LocalGet { local: funcid, type_: ValueType::I32 });
        let results = result_types(return_type, closure.span)?;
        let type_ = FunctionType::new([ValueType::I32], results);
        let type_id = self.module.intern_type(type_.clone());
        out.push(Instruction::CallIndirect { table, type_id, type_ });
        Ok(())
    }

    fn slot_to_value(&mut self, type_: &Type, span: Span, out: &mut Vec<Instruction>) -> StructuredResult<()> {
        match type_ {
            Type::Nil => {
                out.push(Instruction::Drop(ValueType::I64));
                Ok(())
            }
            Type::Int => Ok(()),
            Type::Float => {
                out.push(Instruction::F64ReinterpretI64);
                Ok(())
            }
            Type::Bool
            | Type::String
            | Type::BitArray
            | Type::Tuple(_)
            | Type::List(_)
            | Type::Record { .. }
            | Type::Custom { .. }
            | Type::Opaque { .. }
            | Type::Function { .. }
            | Type::Anything => {
                out.push(Instruction::I32WrapI64);
                Ok(())
            }
            Type::Generic(_) => Err(StructuredError::Diagnostics(vec![Diagnostic::spanned(
                DiagnosticCode::WasmError,
                "unsupported dynamic decoder callback",
                span,
                "generic callback parameter cannot cross decoder primitive here",
            )])),
        }
    }

    fn value_to_slot(&mut self, type_: &Type, span: Span, out: &mut Vec<Instruction>) -> StructuredResult<()> {
        match type_ {
            Type::Nil => {
                out.push(Instruction::I64Const(0));
                Ok(())
            }
            Type::Int => Ok(()),
            Type::Float => {
                out.push(Instruction::I64ReinterpretF64);
                Ok(())
            }
            Type::Bool
            | Type::String
            | Type::BitArray
            | Type::Tuple(_)
            | Type::List(_)
            | Type::Record { .. }
            | Type::Custom { .. }
            | Type::Opaque { .. }
            | Type::Function { .. }
            | Type::Anything => {
                out.push(Instruction::I64ExtendI32U);
                Ok(())
            }
            Type::Generic(_) => Err(StructuredError::Diagnostics(vec![Diagnostic::spanned(
                DiagnosticCode::WasmError,
                "unsupported dynamic decoder callback",
                span,
                "generic callback return cannot cross decoder primitive here",
            )])),
        }
    }

    fn decode_kind_chain(
        &mut self, out: &mut Vec<Instruction>, locals: DecodeLocals, cases: &[(i64, i32)],
    ) -> StructuredResult<()> {
        let Some(((decoder_kind, dynamic_tag), rest)) = cases.split_first() else {
            let mut optional_then = Vec::new();
            self.decode_optional(&mut optional_then, locals.result, locals.tag, locals.data)?;
            let mut else_body = Vec::new();
            self.decode_error(&mut else_body)?;
            else_body.push(Instruction::LocalSet { local: locals.result, type_: ValueType::I32 });
            out.extend([
                Instruction::LocalGet { local: locals.kind, type_: ValueType::I64 },
                Instruction::I64Const(107),
                Instruction::I64Eq,
                Instruction::If { type_: BlockType::empty(), then_body: optional_then, else_body },
            ]);
            return Ok(());
        };
        let mut condition = vec![
            Instruction::LocalGet { local: locals.kind, type_: ValueType::I64 },
            Instruction::I64Const(*decoder_kind),
            Instruction::I64Eq,
        ];
        if *decoder_kind != 100 {
            condition.extend([
                Instruction::LocalGet { local: locals.tag, type_: ValueType::I32 },
                Instruction::I32Const(*dynamic_tag),
                Instruction::I32Eq,
                Instruction::I32And,
            ]);
        }
        out.extend(condition);
        let ok_value = if *decoder_kind == 100 { locals.data } else { locals.field };
        let ok_type = if *decoder_kind == 100 { ValueType::I32 } else { ValueType::I64 };
        let mut then_body = vec![Instruction::LocalGet { local: ok_value, type_: ok_type }];
        if ok_type == ValueType::I32 {
            then_body.push(Instruction::I64ExtendI32U);
        }
        self.decode_ok_from_stack(&mut then_body)?;
        then_body.push(Instruction::LocalSet { local: locals.result, type_: ValueType::I32 });
        let mut else_body = Vec::new();
        self.decode_kind_chain(&mut else_body, locals, rest)?;
        out.push(Instruction::If { type_: BlockType::empty(), then_body, else_body });
        Ok(())
    }

    fn decode_optional(
        &mut self, out: &mut Vec<Instruction>, result: LocalId, tag: LocalId, data: LocalId,
    ) -> StructuredResult<()> {
        out.extend([
            Instruction::LocalGet { local: tag, type_: ValueType::I32 },
            Instruction::I32Const(7),
            Instruction::I32Eq,
        ]);
        let mut none_body = Vec::new();
        self.custom_value(2443824955u32 as i32, 0, [], &mut none_body)?;
        none_body.push(Instruction::I64ExtendI32U);
        self.decode_ok_from_stack(&mut none_body)?;
        none_body.push(Instruction::LocalSet { local: result, type_: ValueType::I32 });
        let mut some_body = vec![
            Instruction::LocalGet { local: data, type_: ValueType::I32 },
            Instruction::I64ExtendI32U,
        ];
        let field = self.required_local(self.dynamic_field_local, "dynamic field")?;
        some_body.push(Instruction::LocalSet { local: field, type_: ValueType::I64 });
        self.custom_value(2407843793u32 as i32, 1, [(12, field)], &mut some_body)?;
        some_body.push(Instruction::I64ExtendI32U);
        self.decode_ok_from_stack(&mut some_body)?;
        some_body.push(Instruction::LocalSet { local: result, type_: ValueType::I32 });
        out.push(Instruction::If { type_: BlockType::empty(), then_body: none_body, else_body: some_body });
        Ok(())
    }

    fn decode_ok_from_stack(&mut self, out: &mut Vec<Instruction>) -> StructuredResult<()> {
        let field = self.required_local(self.dynamic_field_local, "dynamic field")?;
        out.push(Instruction::LocalSet { local: field, type_: ValueType::I64 });
        self.custom_value(1115088027, 1, [(12, field)], out)
    }

    fn decode_error(&mut self, out: &mut Vec<Instruction>) -> StructuredResult<()> {
        self.custom_value(4031082741u32 as i32, 1, [], out)
    }

    fn constructor_value(
        &mut self, constructor: &ir::ConstructorValue, out: &mut Vec<Instruction>,
    ) -> StructuredResult<()> {
        let ptr = self.required_local(self.alloc_local, "allocation pointer")?;
        let size = self.config.layout.custom_size(constructor.arguments.len() as u32, 8);
        self.allocate(size, out)?;
        out.push(Instruction::LocalTee { local: ptr, type_: ValueType::I32 });
        out.push(Instruction::I32Const(u32::from(runtime::ObjectTag::Custom) as i32));
        out.push(Instruction::I32Store(MemoryArg::new(self.ensure_memory(), 0, 2)));
        out.push(Instruction::LocalGet { local: ptr, type_: ValueType::I32 });
        out.push(Instruction::I32Const(constructor.arguments.len() as i32));
        out.push(Instruction::I32Store(MemoryArg::new(self.ensure_memory(), 4, 2)));
        out.push(Instruction::LocalGet { local: ptr, type_: ValueType::I32 });
        out.push(Instruction::I32Const(super::constructor_tag(&constructor.name) as i32));
        out.push(Instruction::I32Store(MemoryArg::new(self.ensure_memory(), 8, 2)));
        for (index, argument) in constructor.arguments.iter().enumerate() {
            out.push(Instruction::LocalGet { local: ptr, type_: ValueType::I32 });
            self.expression_slot_value(argument, out)?;
            out.push(Instruction::I64Store(MemoryArg::new(
                self.ensure_memory(),
                12 + index as u32 * 8,
                3,
            )));
        }
        out.push(Instruction::LocalGet { local: ptr, type_: ValueType::I32 });
        Ok(())
    }

    fn field_array_value<'b>(
        &mut self, tag: runtime::ObjectTag, fields: impl IntoIterator<Item = &'b ir::Expression>,
        out: &mut Vec<Instruction>,
    ) -> StructuredResult<()> {
        let fields = fields.into_iter().collect::<Vec<_>>();
        let ptr = self.required_local(self.alloc_local, "allocation pointer")?;
        let aggregate = self.required_local(self.aggregate_local, "aggregate pointer")?;
        let size = match tag {
            runtime::ObjectTag::Tuple => self.config.layout.tuple_size(fields.len() as u32, 8),
            runtime::ObjectTag::Record => self.config.layout.record_size(fields.len() as u32, 8),
            _ => {
                return Err(StructuredError::Invariant(format!(
                    "internal Wasm codegen invariant failed: unsupported field array tag {tag:?}"
                )));
            }
        };
        self.allocate(size, out)?;
        out.push(Instruction::LocalTee { local: ptr, type_: ValueType::I32 });
        out.push(Instruction::LocalSet { local: aggregate, type_: ValueType::I32 });
        out.push(Instruction::LocalGet { local: aggregate, type_: ValueType::I32 });
        out.push(Instruction::I32Const(u32::from(tag) as i32));
        out.push(Instruction::I32Store(MemoryArg::new(self.ensure_memory(), 0, 2)));
        out.push(Instruction::LocalGet { local: aggregate, type_: ValueType::I32 });
        out.push(Instruction::I32Const(fields.len() as i32));
        out.push(Instruction::I32Store(MemoryArg::new(self.ensure_memory(), 4, 2)));
        for (index, field) in fields.iter().enumerate() {
            out.push(Instruction::LocalGet { local: aggregate, type_: ValueType::I32 });
            out.push(Instruction::LocalGet { local: aggregate, type_: ValueType::I32 });
            self.expression_slot_value(field, out)?;
            out.push(Instruction::I64Store(MemoryArg::new(
                self.ensure_memory(),
                8 + index as u32 * 8,
                3,
            )));
            out.push(Instruction::LocalSet { local: aggregate, type_: ValueType::I32 });
        }
        out.push(Instruction::LocalGet { local: aggregate, type_: ValueType::I32 });
        Ok(())
    }

    fn list_value(&mut self, items: &[ir::Expression], out: &mut Vec<Instruction>) -> StructuredResult<()> {
        let tail = self.required_local(self.list_tail_local, "list tail")?;
        let ptr = self.required_local(self.alloc_local, "allocation pointer")?;
        out.push(Instruction::I32Const(0));
        out.push(Instruction::LocalSet { local: tail, type_: ValueType::I32 });
        for item in items.iter().rev() {
            self.allocate(self.config.layout.list_cons_size(8), out)?;
            out.push(Instruction::LocalTee { local: ptr, type_: ValueType::I32 });
            out.push(Instruction::I32Const(u32::from(runtime::ObjectTag::ListCons) as i32));
            out.push(Instruction::I32Store(MemoryArg::new(self.ensure_memory(), 0, 2)));
            out.push(Instruction::LocalGet { local: ptr, type_: ValueType::I32 });
            out.push(Instruction::I32Const(2));
            out.push(Instruction::I32Store(MemoryArg::new(self.ensure_memory(), 4, 2)));
            out.push(Instruction::LocalGet { local: ptr, type_: ValueType::I32 });
            out.push(Instruction::LocalGet { local: tail, type_: ValueType::I32 });
            out.push(Instruction::I32Store(MemoryArg::new(self.ensure_memory(), 16, 2)));
            out.push(Instruction::LocalGet { local: ptr, type_: ValueType::I32 });
            out.push(Instruction::LocalGet { local: ptr, type_: ValueType::I32 });
            self.expression_slot_value(item, out)?;
            out.push(Instruction::I64Store(MemoryArg::new(self.ensure_memory(), 8, 3)));
            out.push(Instruction::LocalSet { local: tail, type_: ValueType::I32 });
        }
        out.push(Instruction::LocalGet { local: tail, type_: ValueType::I32 });
        Ok(())
    }

    fn record_update(
        &mut self, record: &ir::Expression, constructor: &str, fields: &[ir::RecordFieldUpdate], type_: &Type,
        out: &mut Vec<Instruction>,
    ) -> StructuredResult<()> {
        let source = self.required_local(self.scratch_local, "scratch")?;
        let ptr = self.required_local(self.alloc_local, "allocation pointer")?;
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
        out.push(Instruction::I32Store(MemoryArg::new(self.ensure_memory(), 0, 2)));
        out.push(Instruction::LocalGet { local: ptr, type_: ValueType::I32 });
        out.push(Instruction::I32Const(header_fields));
        out.push(Instruction::I32Store(MemoryArg::new(self.ensure_memory(), 4, 2)));
        if !matches!(type_, Type::Record { .. }) {
            out.push(Instruction::LocalGet { local: ptr, type_: ValueType::I32 });
            out.push(Instruction::I32Const(super::constructor_tag(constructor) as i32));
            out.push(Instruction::I32Store(MemoryArg::new(self.ensure_memory(), 8, 2)));
        }
        for (index, field) in fields.iter().enumerate() {
            out.push(Instruction::LocalGet { local: ptr, type_: ValueType::I32 });
            match &field.value {
                Some(value) => self.expression_slot_value(value, out)?,
                None => {
                    out.push(Instruction::LocalGet { local: source, type_: ValueType::I32 });
                    out.push(Instruction::I64Load(MemoryArg::new(
                        self.ensure_memory(),
                        slot_offset + index as u32 * 8,
                        3,
                    )));
                }
            }
            out.push(Instruction::I64Store(MemoryArg::new(
                self.ensure_memory(),
                slot_offset + index as u32 * 8,
                3,
            )));
        }
        out.push(Instruction::LocalGet { local: ptr, type_: ValueType::I32 });
        Ok(())
    }

    fn bit_array_concat(
        &mut self, left: &ir::Expression, right: &ir::Expression, out: &mut Vec<Instruction>,
    ) -> StructuredResult<()> {
        self.expression(left, out)?;
        self.expression(right, out)?;
        self.call_runtime_helper(
            "__bit_array_append",
            [ValueType::I32, ValueType::I32],
            [ValueType::I32],
            out,
        );
        Ok(())
    }

    fn list_cons(
        &mut self, head: &ir::Expression, tail: &ir::Expression, out: &mut Vec<Instruction>,
    ) -> StructuredResult<()> {
        let ptr = self.required_local(self.alloc_local, "allocation pointer")?;
        self.allocate(self.config.layout.list_cons_size(8), out)?;
        out.push(Instruction::LocalTee { local: ptr, type_: ValueType::I32 });
        out.push(Instruction::I32Const(u32::from(runtime::ObjectTag::ListCons) as i32));
        out.push(Instruction::I32Store(MemoryArg::new(self.ensure_memory(), 0, 2)));
        out.push(Instruction::LocalGet { local: ptr, type_: ValueType::I32 });
        out.push(Instruction::I32Const(2));
        out.push(Instruction::I32Store(MemoryArg::new(self.ensure_memory(), 4, 2)));
        out.push(Instruction::LocalGet { local: ptr, type_: ValueType::I32 });
        self.expression_slot_value(head, out)?;
        out.push(Instruction::I64Store(MemoryArg::new(self.ensure_memory(), 8, 3)));
        out.push(Instruction::LocalGet { local: ptr, type_: ValueType::I32 });
        self.expression(tail, out)?;
        out.push(Instruction::I32Store(MemoryArg::new(self.ensure_memory(), 16, 2)));
        out.push(Instruction::LocalGet { local: ptr, type_: ValueType::I32 });
        Ok(())
    }

    fn closure_allocation(
        &mut self, function: &ir::AnonymousFunction, out: &mut Vec<Instruction>,
    ) -> StructuredResult<()> {
        let ptr = self.required_local(self.alloc_local, "allocation pointer")?;
        let size = self.config.layout.closure_size(function.captures.len() as u32);
        self.allocate(size, out)?;
        out.push(Instruction::LocalTee { local: ptr, type_: ValueType::I32 });
        out.push(Instruction::I32Const(u32::from(runtime::ObjectTag::Closure) as i32));
        out.push(Instruction::I32Store(MemoryArg::new(self.ensure_memory(), 0, 2)));
        out.push(Instruction::LocalGet { local: ptr, type_: ValueType::I32 });
        out.push(Instruction::I32Const(function.captures.len() as i32));
        out.push(Instruction::I32Store(MemoryArg::new(self.ensure_memory(), 4, 2)));
        out.push(Instruction::LocalGet { local: ptr, type_: ValueType::I32 });
        out.push(Instruction::I32Const(self.function_id(&function.name) as i32));
        out.push(Instruction::I32Store(MemoryArg::new(
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
            out.push(Instruction::I64Store(MemoryArg::new(
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
            | Type::Anything
            | Type::BitArray
            | Type::Tuple(_)
            | Type::List(_)
            | Type::Record { .. }
            | Type::Custom { .. }
            | Type::Opaque { .. }
            | Type::Function { .. } => DebugImport::Value,
            Type::Nil => return Ok(()),
            Type::Generic(_) => {
                return Err(StructuredError::Diagnostics(vec![
                    Diagnostic::new(
                        DiagnosticCode::WasmError,
                        "debug intrinsic does not support generic values",
                    )
                    .with_label(Label::primary(value.span, "generic value passed to debug here")),
                ]));
            }
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
        let local = self.debug_locals.get(&import).copied().ok_or_else(|| {
            StructuredError::Invariant(format!(
                "internal Wasm codegen invariant failed: missing {} debug local",
                import.name()
            ))
        })?;
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
                    _ => {
                        return Err(StructuredError::Diagnostics(vec![
                            Diagnostic::new(DiagnosticCode::WasmError, "comparison type is not supported")
                                .with_label(Label::primary(left.span, "unsupported comparison operand here")),
                        ]));
                    }
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
        match left.type_ {
            Type::Int => out.push(Instruction::I64Eq),
            Type::Float => out.push(Instruction::F64Eq),
            Type::Bool => out.push(Instruction::I32Eq),
            Type::Nil => out.push(Instruction::I32Const(1)),
            Type::String
            | Type::Anything
            | Type::BitArray
            | Type::Tuple(_)
            | Type::List(_)
            | Type::Record { .. }
            | Type::Custom { .. }
            | Type::Opaque { .. }
            | Type::Function { .. } => {
                self.call_runtime_helper("__equal_value", [ValueType::I32, ValueType::I32], [ValueType::I32], out);
            }
            Type::Generic(_) => {
                return Err(StructuredError::Diagnostics(vec![
                    Diagnostic::new(
                        DiagnosticCode::WasmError,
                        "runtime equality does not support generic values",
                    )
                    .with_label(Label::primary(left.span, "generic equality operand here")),
                ]));
            }
        }
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
        &mut self, branch: &ir::Branch, type_: &Type, span: Span, out: &mut Vec<Instruction>,
    ) -> StructuredResult<()> {
        let results = result_types(type_, span)?;
        let body = self.branch_clause(branch, 0, type_, span, results)?;
        out.extend(body);
        Ok(())
    }

    fn branch_clause(
        &mut self, branch: &ir::Branch, index: usize, type_: &Type, span: Span, results: Vec<ValueType>,
    ) -> StructuredResult<Vec<Instruction>> {
        let Some(clause) = branch.clauses.get(index) else {
            let mut failure = Vec::new();
            if !matches!(type_, Type::Nil) && results.is_empty() {
                return Err(StructuredError::Invariant(
                    "internal Wasm codegen invariant failed: non-nil branch has no Wasm result type".into(),
                ));
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
                out.push(Instruction::I32Load(MemoryArg::new(self.ensure_memory(), 8, 2)));
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
        &mut self, subject: &PatternSubject<'_>, literal: &ir::IrLiteral, out: &mut Vec<Instruction>,
    ) -> StructuredResult<()> {
        if subject.path.is_empty() {
            self.expression(subject.root, out)?;
        } else {
            self.slot_address(subject, out)?;
            out.push(match literal.kind {
                LiteralKind::Int => Instruction::I64Load(MemoryArg::new(self.ensure_memory(), 0, 3)),
                LiteralKind::Float => Instruction::F64Load(MemoryArg::new(self.ensure_memory(), 0, 3)),
                LiteralKind::Bool | LiteralKind::String => {
                    Instruction::I32Load(MemoryArg::new(self.ensure_memory(), 0, 2))
                }
                LiteralKind::Nil => Instruction::I64Load(MemoryArg::new(self.ensure_memory(), 0, 3)),
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

    fn managed_tag_test(
        &mut self, expression: &ir::Expression, tag: runtime::ObjectTag, size: Option<u32>, out: &mut Vec<Instruction>,
    ) -> StructuredResult<()> {
        self.managed_tag_test_subject(&PatternSubject { root: expression, path: Vec::new() }, tag, size, out)
    }

    fn managed_tag_test_subject(
        &mut self, subject: &PatternSubject<'_>, tag: runtime::ObjectTag, size: Option<u32>, out: &mut Vec<Instruction>,
    ) -> StructuredResult<()> {
        self.subject_pointer(subject, out)?;
        out.push(Instruction::I32Load(MemoryArg::new(self.ensure_memory(), 0, 2)));
        out.push(Instruction::I32Const(u32::from(tag) as i32));
        out.push(Instruction::I32Eq);
        if let Some(size) = size {
            self.subject_pointer(subject, out)?;
            out.push(Instruction::I32Load(MemoryArg::new(self.ensure_memory(), 4, 2)));
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
            out.push(Instruction::I32Load(MemoryArg::new(self.ensure_memory(), 0, 2)));
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
        out.push(Instruction::I32Load(MemoryArg::new(self.ensure_memory(), 4, 2)));
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
        &self, segments: &[ir::BitStringPatternSegment], span: Span,
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
        let value = self.required_local(self.bit_value_local, "bit-string value")?;
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
        if !offset.is_multiple_of(8) {
            return Err(StructuredError::Diagnostics(vec![Diagnostic::new(
                DiagnosticCode::WasmError,
                "structured binary bit-string binding requires a byte-aligned offset",
            )]));
        }
        let bit_len = self.required_local(self.scratch_local, "scratch")?;
        let ptr = self.required_local(self.alloc_local, "allocation pointer")?;
        let i = self.required_local(self.bit_i_local, "bit-string index")?;
        self.subject_pointer(subject, out)?;
        out.push(Instruction::I32Load(MemoryArg::new(self.ensure_memory(), 4, 2)));
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
        out.push(Instruction::I32Store(MemoryArg::new(self.ensure_memory(), 0, 2)));
        out.push(Instruction::LocalGet { local: ptr, type_: ValueType::I32 });
        out.push(Instruction::LocalGet { local: bit_len, type_: ValueType::I32 });
        out.push(Instruction::I32Store(MemoryArg::new(self.ensure_memory(), 4, 2)));
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
        copy_body.push(Instruction::I32Load8U(MemoryArg::new(self.ensure_memory(), 0, 0)));
        copy_body.push(Instruction::I32Store8(MemoryArg::new(self.ensure_memory(), 0, 0)));
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
        let ptr = self.required_local(self.scratch_local, "scratch")?;
        self.subject_pointer(subject, out)?;
        out.push(Instruction::LocalSet { local: ptr, type_: ValueType::I32 });
        out.push(Instruction::LocalGet { local: ptr, type_: ValueType::I32 });
        out.push(Instruction::I32Const(8 + (index / 8) as i32));
        out.push(Instruction::I32Add);
        out.push(Instruction::I32Load8U(MemoryArg::new(self.ensure_memory(), 0, 0)));
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
            out.push(Instruction::I32Load(MemoryArg::new(self.ensure_memory(), *offset, 2)));
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

    fn list_deconstruct(
        &mut self, list: &ir::Expression, head: ir::LocalId, tail: ir::LocalId, out: &mut Vec<Instruction>,
    ) -> StructuredResult<()> {
        self.expression(list, out)?;
        self.call_runtime_helper("__list_head", [ValueType::I32], [ValueType::I64], out);
        let head_type = self.local_types.get(&head).cloned().unwrap_or(Type::Int);
        self.slot_bits_to_value(&head_type, list.span, out)?;
        out.push(Instruction::LocalSet {
            local: self.local(head, list.span)?,
            type_: value_type(&head_type, list.span)?,
        });
        self.expression(list, out)?;
        self.call_runtime_helper("__list_tail", [ValueType::I32], [ValueType::I32], out);
        out.push(Instruction::LocalSet { local: self.local(tail, list.span)?, type_: ValueType::I32 });
        Ok(())
    }

    fn slot_bits_to_value(&mut self, type_: &Type, span: Span, out: &mut Vec<Instruction>) -> StructuredResult<()> {
        match value_type(type_, span)? {
            ValueType::I64 => {}
            ValueType::I32 => out.push(Instruction::I32WrapI64),
            ValueType::F64 => out.push(Instruction::F64ReinterpretI64),
            ValueType::F32 | ValueType::FuncRef | ValueType::ExternRef => {
                return Err(StructuredError::Invariant(
                    "internal Wasm codegen invariant failed: unsupported slot value type".into(),
                ));
            }
        }
        Ok(())
    }

    fn failure(&mut self, failure: &ir::FailurePath, out: &mut Vec<Instruction>) -> StructuredResult<()> {
        let helper = match failure.reason {
            ir::FailureReason::AssertMatch | ir::FailureReason::BranchFallthrough => "__match_fail",
            ir::FailureReason::Panic | ir::FailureReason::Todo | ir::FailureReason::Assert => "__panic",
        };
        self.call_runtime_helper(helper, [], [], out);
        out.push(Instruction::Unreachable);
        Ok(())
    }

    fn memory_operation(
        &mut self, operation: &ir::MemoryOperation, out: &mut Vec<Instruction>,
    ) -> StructuredResult<()> {
        match operation {
            ir::MemoryOperation::Allocate { bytes } => {
                self.expression(bytes, out)?;
                if value_type(&bytes.type_, bytes.span)? == ValueType::I64 {
                    out.push(Instruction::I32WrapI64);
                }
                self.allocate_dynamic(out)
            }
            ir::MemoryOperation::Load { address, type_ } => {
                self.expression(address, out)?;
                out.push(match type_ {
                    ir::RepresentationType::Scalar(ir::ScalarRepresentation::I64) => {
                        Instruction::I64Load(MemoryArg::new(self.ensure_memory(), 0, 3))
                    }
                    ir::RepresentationType::Scalar(ir::ScalarRepresentation::F64) => {
                        Instruction::F64Load(MemoryArg::new(self.ensure_memory(), 0, 3))
                    }
                    _ => Instruction::I32Load(MemoryArg::new(self.ensure_memory(), 0, 2)),
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
        let scratch = self.required_local(self.scratch_local, "scratch")?;
        let depth = self.indirect_call_depth;
        let funcid = self.funcid_locals.get(depth).copied().ok_or_else(|| {
            StructuredError::Invariant(format!(
                "indirect call at depth {depth} but only {} funcid locals were allocated",
                self.funcid_locals.len()
            ))
        })?;
        let table = self.func_table.ok_or_else(|| {
            StructuredError::Invariant("indirect call reached codegen before function table was created".into())
        })?;

        self.expression(&call.callee, out)?;
        out.push(Instruction::LocalSet { local: scratch, type_: ValueType::I32 });

        out.push(Instruction::LocalGet { local: scratch, type_: ValueType::I32 });
        out.push(Instruction::I32Load(MemoryArg::new(
            self.ensure_memory(),
            u32::from(ClosureConstants::FunctionIdOffset),
            2,
        )));
        out.push(Instruction::LocalSet { local: funcid, type_: ValueType::I32 });

        let mut param_types = vec![ValueType::I32];
        for argument in &call.arguments {
            param_types.push(value_type(&argument.value.type_, argument.span)?);
        }
        let dispatch_results = call
            .abi
            .return_
            .as_ref()
            .map(|value| result_types(&value.type_, call.callee.span))
            .transpose()?
            .unwrap_or_default();
        let dispatch_type = FunctionType::new(param_types, dispatch_results);
        let type_id = self.module.intern_type(dispatch_type.clone());

        out.push(Instruction::LocalGet { local: scratch, type_: ValueType::I32 });
        self.indirect_call_depth += 1;
        for argument in &call.arguments {
            self.expression(&argument.value, out)?;
        }
        self.indirect_call_depth -= 1;
        out.push(Instruction::LocalGet { local: funcid, type_: ValueType::I32 });
        out.push(Instruction::CallIndirect { table, type_id, type_: dispatch_type });
        Ok(())
    }

    /// Emit trampoline functions for all source functions and populate the element segment.
    ///
    /// The funcref table itself is pre-declared at the start of `module()` so that
    /// `call_indirect` can reference it during function body emission.  This method
    /// fills in the trampolines and the active element segment that populates the table.
    fn emit_function_table(&mut self, functions: &[&ir::Function]) -> StructuredResult<()> {
        let table = self
            .func_table
            .expect("func_table must be set before emit_function_table");
        let n = functions.len();
        if n == 0 {
            return Ok(());
        }

        let mut table_entries: Vec<FunctionId> = Vec::with_capacity(n);
        for ir_function in functions {
            let id = self.emit_trampoline(ir_function)?;
            table_entries.push(id);
        }

        self.module
            .push_element(ElementSegment { table, offset: 0, functions: table_entries });

        Ok(())
    }

    /// Build a trampoline for `ir_function`.
    ///
    /// The trampoline signature is `(i32 closure_ptr, non_capture_params...) -> result`.
    /// Its body loads captures from the closure object, then calls the real Wasm function.
    ///
    /// If any type cannot be represented (e.g. a residual Generic), an `unreachable` trap
    /// trampoline is emitted instead so the table slot is still occupied by a valid function.
    fn emit_trampoline(&mut self, ir_function: &ir::Function) -> StructuredResult<FunctionId> {
        let non_capture_params: Vec<&ir::Local> = ir_function
            .params
            .iter()
            .skip(ir_function.closure_captures.len())
            .collect();

        let mut trampoline_param_types = vec![ValueType::I32];
        for param in &non_capture_params {
            match maybe_value_type(&param.type_) {
                Some(t) => trampoline_param_types.push(t),
                None => return self.emit_trap_trampoline(),
            }
        }

        let trampoline_result_types = match result_types(&ir_function.return_type, ir_function.span) {
            Ok(r) => r,
            Err(_) => return self.emit_trap_trampoline(),
        };

        for cap_type in &ir_function.closure_captures {
            if maybe_value_type(cap_type).is_none() {
                return self.emit_trap_trampoline();
            }
        }

        let trampoline_type = FunctionType::new(trampoline_param_types.clone(), trampoline_result_types);
        let type_id = self.module.intern_type(trampoline_type);

        let mut body: Vec<Instruction> = Vec::new();

        let memory = self.ensure_memory();
        for (i, cap_type) in ir_function.closure_captures.iter().enumerate() {
            let wasm_type = maybe_value_type(cap_type).unwrap();
            let offset =
                u32::from(ClosureConstants::CapturesOffset) + i as u32 * u32::from(ClosureConstants::CaptureSlotSize);
            body.push(Instruction::LocalGet { local: LocalId(0), type_: ValueType::I32 });
            body.push(load_for_type(memory, offset, wasm_type));
        }

        for (j, param) in non_capture_params.iter().enumerate() {
            let wasm_type = maybe_value_type(&param.type_).unwrap();
            body.push(Instruction::LocalGet { local: LocalId(1 + j as u32), type_: wasm_type });
        }

        let callee_id = self.function_id_structured(&ir_function.name);
        let callee_type = self
            .signatures
            .get(&ir_function.name)
            .map(|sig| sig.type_.clone())
            .unwrap_or_else(|| {
                FunctionType::new(
                    ir_function
                        .params
                        .iter()
                        .filter_map(|p| maybe_value_type(&p.type_))
                        .collect::<Vec<_>>(),
                    result_types(&ir_function.return_type, ir_function.span).unwrap_or_default(),
                )
            });
        body.push(Instruction::Call { function: callee_id, type_: callee_type });

        let mut trampoline =
            Function { name: Some(format!("{}__trampoline", ir_function.name)), ..Function::new(type_id) };

        for &t in &trampoline_param_types {
            trampoline.params.push(Local { name: None, type_: t });
        }
        trampoline.body = body;

        Ok(self.module.push_function(trampoline))
    }

    /// Emit a single `unreachable` function used as a placeholder for table slots
    /// whose source function has types that cannot be trampolined.
    fn emit_trap_trampoline(&mut self) -> StructuredResult<FunctionId> {
        let type_id = self.module.intern_type(FunctionType::new([ValueType::I32], []));
        let mut trampoline = Function::new(type_id);
        trampoline.name = Some("__trap_trampoline".into());
        trampoline.params = vec![Local { name: None, type_: ValueType::I32 }];
        trampoline.body = vec![Instruction::Unreachable];
        Ok(self.module.push_function(trampoline))
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
                LiteralKind::Int => literal
                    .source
                    .parse::<i64>()
                    .map(|value| value as u64)
                    .map_err(|_| literal_parse_diagnostic(literal, expression.span, "signed 64-bit integer")),
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
                    .map_err(|_| literal_parse_diagnostic(literal, expression.span, "64-bit float")),
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
            ExpressionKind::BitArray(bit_array) => {
                let bytes = bit_array.bytes();
                Ok(self.push_static(runtime::bit_array_object(
                    self.config,
                    self.next_static_offset,
                    &bytes,
                    bit_array.bit_len,
                )) as u64)
            }
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
        if let ir::ConstantValue::Literal(ir::IrLiteral { kind: LiteralKind::String, source }) = &constant.value {
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
            .push_memory(Memory { minimum_pages: 1, maximum_pages: Some(self.config.memory_max_pages) });
        self.memory = Some(memory);
        memory
    }

    fn ensure_heap_global(&mut self) -> GlobalId {
        if let Some(global) = self.heap_global {
            return global;
        }
        let global = self.module.push_global(Global {
            name: None,
            type_: ValueType::I32,
            mutable: true,
            init: vec![Instruction::I32Const(self.config.heap_start as i32)],
        });
        self.heap_global = Some(global);
        global
    }

    fn name_heap_global(&mut self) {
        if let Some(global) = self.heap_global {
            self.module.globals[global.0 as usize].name = Some("__heap".into());
        }
    }

    fn ensure_last_panic_global(&mut self) {
        if self
            .module
            .globals
            .iter()
            .any(|global| global.name.as_deref() == Some("__last_panic_payload"))
        {
            return;
        }
        self.module.push_global(Global {
            name: Some("__last_panic_payload".into()),
            type_: ValueType::I32,
            mutable: true,
            init: vec![Instruction::I32Const(0)],
        });
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

    fn required_signature(&self, name: &str) -> StructuredResult<FunctionSignature> {
        self.signatures.get(name).cloned().ok_or_else(|| {
            StructuredError::Invariant(format!(
                "internal Wasm codegen invariant failed: missing signature for `{name}`"
            ))
        })
    }

    fn required_local(&self, local: Option<LocalId>, name: &str) -> StructuredResult<LocalId> {
        local.ok_or_else(|| {
            StructuredError::Invariant(format!("internal Wasm codegen invariant failed: missing {name} local"))
        })
    }

    fn local(&self, local: ir::LocalId, span: Span) -> StructuredResult<LocalId> {
        self.local_indices.get(&local).copied().ok_or_else(|| {
            StructuredError::Diagnostics(vec![
                Diagnostic::new(DiagnosticCode::WasmError, "unknown local in structured Wasm emitter")
                    .with_label(Label::primary(span, "local used here")),
            ])
        })
    }
}

pub fn emit(module: &ir::Module, options: EmitOptions) -> Result<Module, Diagnostics> {
    let emitter = StructuredEmitter::new(module, options);
    match emitter.module(module) {
        Ok(module) => Ok(module),
        Err(StructuredError::Unsupported) => Err(unsupported_structured_diagnostics(module)),
        Err(StructuredError::Invariant(message)) => Err(invariant_diagnostics(module, &message)),
        Err(StructuredError::Diagnostics(diagnostics)) => Err(diagnostics),
    }
}

fn unsupported_structured_diagnostics(module: &ir::Module) -> Diagnostics {
    let diagnostic = Diagnostic::new(
        DiagnosticCode::WasmError,
        "structured Wasm emitter does not support this IR yet",
    )
    .with_note("the fallback WAT emitter is disabled; port this IR form to structured codegen");
    if let Some(function) = module.functions.first() {
        vec![diagnostic.with_label(Label::primary(function.span, "module lowered to unsupported IR here"))]
    } else {
        vec![diagnostic.with_label(Label::primary(module.span, "module lowered to unsupported IR here"))]
    }
}

fn invariant_diagnostics(module: &ir::Module, message: &str) -> Diagnostics {
    vec![
        Diagnostic::new(DiagnosticCode::WasmError, message.to_string()).with_label(Label::primary(
            module.span,
            "internal Wasm invariant failed while compiling this module",
        )),
    ]
}

fn literal_parse_diagnostic(literal: &ir::IrLiteral, span: Span, expected: &'static str) -> StructuredError {
    let kind = match literal.kind {
        LiteralKind::Int => "int",
        LiteralKind::Float => "float",
        LiteralKind::Bool => "bool",
        LiteralKind::Nil => "nil",
        LiteralKind::String => "string",
    };
    StructuredError::Diagnostics(vec![
        Diagnostic::new(
            DiagnosticCode::WasmError,
            format!("invalid {kind} literal in Wasm backend"),
        )
        .with_label(Label::primary(
            span,
            format!("could not parse `{}` as {expected}", literal.source),
        )),
    ])
}

fn literal_type(literal: &ir::IrLiteral) -> Type {
    match literal.kind {
        LiteralKind::Int => Type::Int,
        LiteralKind::Float => Type::Float,
        LiteralKind::Bool => Type::Bool,
        LiteralKind::String => Type::String,
        LiteralKind::Nil => Type::Nil,
    }
}

fn result_types(type_: &Type, span: Span) -> StructuredResult<Vec<ValueType>> {
    if matches!(type_, Type::Nil) { Ok(Vec::new()) } else { Ok(vec![value_type(type_, span)?]) }
}

fn validate_anything_boundary_abi(module: &ir::Module) -> StructuredResult<()> {
    let mut diagnostics = Vec::new();

    for function in &module.functions {
        let allow_anything = match &function.abi.boundary {
            ir::CallBoundary::HostImport { module, name } => is_allowed_anything_external(true, module, name),
            ir::CallBoundary::Internal => true,
            ir::CallBoundary::ModuleExport | ir::CallBoundary::ModuleImport { .. } => false,
        };
        if allow_anything {
            continue;
        }

        for (index, param) in function.params.iter().enumerate() {
            if param.type_.contains_anything() {
                diagnostics.push(anything_boundary_abi_diagnostic(
                    module,
                    function,
                    &format!("parameter {}", index + 1),
                    &param.type_,
                    param.span,
                ));
            }
        }

        if function.return_type.contains_anything() {
            diagnostics.push(anything_boundary_abi_diagnostic(
                module,
                function,
                "return",
                &function.return_type,
                function.span,
            ));
        }
    }

    if diagnostics.is_empty() { Ok(()) } else { Err(StructuredError::Diagnostics(diagnostics)) }
}

fn anything_boundary_abi_diagnostic(
    module: &ir::Module, function: &ir::Function, pos: &str, type_: &Type, span: Span,
) -> Diagnostic {
    let name = match &function.abi.boundary {
        ir::CallBoundary::ModuleExport => module
            .exports
            .iter()
            .find(|export| export.kind == ir::ExportKind::Function && export.backend_name() == function.name)
            .map(|export| export.name.as_str())
            .unwrap_or(function.name.as_str()),
        _ => function.name.as_str(),
    };
    let boundary = match &function.abi.boundary {
        ir::CallBoundary::ModuleExport => "export",
        ir::CallBoundary::ModuleImport { .. } => "module import",
        ir::CallBoundary::HostImport { .. } => "host import",
        ir::CallBoundary::Internal => "internal function",
    };

    Diagnostic::spanned(
        DiagnosticCode::WasmError,
        format!(
            "Wasm {boundary} `{name}` {pos} uses unsupported dynamic boundary type `{}`",
            type_.display()
        ),
        span,
        "unsupported `anything` ABI shape here",
    )
    .with_note("`anything` is reserved for stdlib-native dynamic and inspection boundaries")
}

fn validate_js_host_abi(module: &ir::Module, target: WasmTarget) -> StructuredResult<()> {
    let mut diagnostics = Vec::new();

    for function in &module.functions {
        match &function.abi.boundary {
            ir::CallBoundary::HostImport { module: import_module, name } => {
                validate_js_host_function_shape(
                    module,
                    function,
                    JsAbiBoundary::Import { module: import_module, name },
                    target,
                    &mut diagnostics,
                );
            }
            ir::CallBoundary::ModuleExport => {
                let export_name = module
                    .exports
                    .iter()
                    .find(|export| export.kind == ir::ExportKind::Function && export.backend_name() == function.name)
                    .map(|export| export.name.as_str())
                    .unwrap_or(function.name.as_str());
                validate_js_host_function_shape(
                    module,
                    function,
                    JsAbiBoundary::Export { name: export_name },
                    target,
                    &mut diagnostics,
                );
            }
            ir::CallBoundary::Internal | ir::CallBoundary::ModuleImport { .. } => {}
        }
    }

    if diagnostics.is_empty() { Ok(()) } else { Err(StructuredError::Diagnostics(diagnostics)) }
}

fn validate_js_host_function_shape(
    module: &ir::Module, function: &ir::Function, boundary: JsAbiBoundary<'_>, target: WasmTarget,
    diagnostics: &mut Diagnostics,
) {
    for (index, param) in function.params.iter().enumerate() {
        if is_supported_js_host_parameter(module, &param.type_) {
            continue;
        }
        diagnostics.push(js_host_abi_diagnostic(
            function,
            boundary,
            target,
            &format!("parameter {}", index + 1),
            &param.type_,
            param.span,
        ));
    }

    if !is_supported_js_host_return(
        module,
        &function.return_type,
        matches!(boundary, JsAbiBoundary::Export { .. }),
    ) {
        diagnostics.push(js_host_abi_diagnostic(
            function,
            boundary,
            target,
            "return",
            &function.return_type,
            function.span,
        ));
    }
}

fn js_host_abi_diagnostic(
    func: &ir::Function, boundary: JsAbiBoundary<'_>, target: WasmTarget, pos: &str, type_: &Type, span: Span,
) -> Diagnostic {
    if type_.contains_anything() {
        let name = match boundary {
            JsAbiBoundary::Import { .. } => func.name.as_str(),
            JsAbiBoundary::Export { name } => name,
        };
        return Diagnostic::spanned(
            DiagnosticCode::WasmError,
            format!(
                "JS host function `{name}` {pos} uses unsupported dynamic boundary type `{}` for target `{}`",
                type_.display(),
                target.name()
            ),
            span,
            "unsupported `anything` ABI shape here",
        )
        .with_note("`anything` is reserved for stdlib-native dynamic and inspection boundaries");
    }

    let (message, note) = match boundary {
        JsAbiBoundary::Import { module, name } => (
            format!(
                "JS host import `{}` {} uses unsupported ABI shape `{:?}` for target `{}`",
                func.name,
                pos,
                type_,
                target.name()
            ),
            format!(
                "host import `{module}.{name}` must use Int, Float, Bool, String, or Nil returns until structured writers and opaque handles are stable"
            ),
        ),
        JsAbiBoundary::Export { name } => (
            format!(
                "JS host export `{name}` {} uses unsupported ABI shape `{:?}` for target `{}`",
                pos,
                type_,
                target.name()
            ),
            "public JS host exports must use Int, Float, Bool, String, Nil, or supported structured managed returns"
                .into(),
        ),
    };

    Diagnostic::new(DiagnosticCode::WasmError, message)
        .with_label(Label::primary(span, "unsupported JS host ABI shape here"))
        .with_note(note)
}

fn is_supported_js_host_parameter(module: &ir::Module, type_: &Type) -> bool {
    matches!(type_, Type::Int | Type::Float | Type::Bool | Type::String) || is_js_host_opaque_handle(module, type_)
}

fn is_supported_js_host_return(module: &ir::Module, type_: &Type, structured_allowed: bool) -> bool {
    matches!(type_, Type::Int | Type::Float | Type::Bool | Type::String | Type::Nil)
        || is_js_host_opaque_handle(module, type_)
        || (structured_allowed && is_supported_js_host_structured_return(module, type_))
}

fn is_supported_js_host_structured_return(module: &ir::Module, type_: &Type) -> bool {
    match type_ {
        Type::Tuple(items) => items.iter().all(|item| is_supported_js_host_field(module, item)),
        Type::List(item) => is_supported_js_host_field(module, item),
        Type::Record { fields, .. } => fields
            .iter()
            .all(|field| is_supported_js_host_field(module, &field.type_)),
        Type::Custom { args, .. } => args.iter().all(|arg| is_supported_js_host_field(module, arg)),
        _ => false,
    }
}

fn is_supported_js_host_field(module: &ir::Module, type_: &Type) -> bool {
    is_supported_js_host_parameter(module, type_) || is_supported_js_host_structured_return(module, type_)
}

fn is_js_host_opaque_handle(module: &ir::Module, type_: &Type) -> bool {
    match type_ {
        Type::Opaque { .. } => true,
        Type::Custom { name, .. } => module
            .type_declarations
            .iter()
            .any(|type_| type_.name == *name && type_.opaque),
        _ => false,
    }
}

fn native_dict_external_name(function: &str, has_local_function: bool) -> Option<&'static str> {
    let public = function
        .strip_prefix("gleam_stdlib:gleam/dict.")
        .or_else(|| function.strip_prefix("gleam/dict."));
    if let Some(name) = public {
        return match name {
            "new" | "make" => Some("make"),
            "size" => Some("size"),
            "get" => Some("get"),
            "has_key" | "has" => Some("has"),
            "insert" => Some("insert"),
            _ => None,
        };
    }
    if has_local_function {
        return None;
    }
    match function {
        "to_transient" => Some("toTransient"),
        "from_transient" => Some("fromTransient"),
        "transient_insert" => Some("destructiveTransientInsert"),
        "transient_delete" => Some("destructiveTransientDelete"),
        "transient_update_with" => Some("destructiveTransientUpdateWith"),
        _ => None,
    }
}

fn module_exports_arena_scoped_values(module: &ir::Module) -> bool {
    module.functions.iter().any(|function| {
        matches!(function.abi.boundary, ir::CallBoundary::ModuleExport)
            && block_needs_allocation(&function.body)
            && (is_heap_managed_type(&function.return_type)
                || function.params.iter().any(|param| is_heap_managed_type(&param.type_)))
    })
}

fn reachable_functions(module: &ir::Module) -> Vec<&ir::Function> {
    let has_indirect_call = module.functions.iter().any(|function| {
        function
            .body
            .instructions
            .iter()
            .any(|instruction| instruction.expression().contains_indirect_call())
            || function.body.result.contains_indirect_call()
    });
    let has_linked_stdlib_source = module
        .linked_names
        .iter()
        .any(|name| name.source_name.starts_with("gleam_stdlib:"));
    if has_indirect_call && !has_linked_stdlib_source {
        return module.functions.iter().collect();
    }

    let by_name = module
        .functions
        .iter()
        .map(|function| (function.name.as_str(), function))
        .collect::<HashMap<_, _>>();
    let roots = module
        .functions
        .iter()
        .filter(|function| !matches!(function.abi.boundary, ir::CallBoundary::Internal))
        .map(|function| function.name.clone())
        .collect::<Vec<_>>();
    if roots.is_empty() {
        return module.functions.iter().collect();
    }

    let mut reachable = HashSet::new();
    let mut stack = roots;
    while let Some(name) = stack.pop() {
        if !reachable.insert(name.clone()) {
            continue;
        }
        let Some(function) = by_name.get(name.as_str()) else {
            continue;
        };
        collect_function_refs(function, &by_name, &mut stack);
    }

    module
        .functions
        .iter()
        .filter(|function| reachable.contains(&function.name))
        .collect()
}

fn collect_function_refs(function: &ir::Function, by_name: &HashMap<&str, &ir::Function>, stack: &mut Vec<String>) {
    for instruction in &function.body.instructions {
        collect_expression_refs(instruction.expression(), by_name, stack);
    }
    collect_expression_refs(&function.body.result, by_name, stack);
}

fn collect_expression_refs(
    expression: &ir::Expression, by_name: &HashMap<&str, &ir::Function>, stack: &mut Vec<String>,
) {
    match &expression.kind {
        ExpressionKind::DirectCall(call) if by_name.contains_key(call.function.as_str()) => {
            stack.push(call.function.clone());
        }
        ExpressionKind::FunctionValue(function) if by_name.contains_key(function.name.as_str()) => {
            stack.push(function.name.clone());
        }
        ExpressionKind::AnonymousFunction(function) if by_name.contains_key(function.name.as_str()) => {
            stack.push(function.name.clone());
        }
        _ => {}
    }
    for child in expression.children() {
        collect_expression_refs(child, by_name, stack);
    }
}

fn is_heap_managed_type(type_: &Type) -> bool {
    matches!(
        type_,
        Type::String
            | Type::BitArray
            | Type::Tuple(_)
            | Type::List(_)
            | Type::Record { .. }
            | Type::Custom { .. }
            | Type::Opaque { .. }
            | Type::Function { .. }
            | Type::Generic(_)
    )
}

fn value_type(type_: &Type, span: Span) -> StructuredResult<ValueType> {
    maybe_value_type(type_).ok_or_else(|| {
        StructuredError::Diagnostics(vec![
            Diagnostic::spanned(
                DiagnosticCode::WasmError,
                "unsupported host ABI",
                span,
                "unsupported ABI value here",
            )
            .with_notes([
                "Wasm boundaries require concrete scalar or managed runtime types",
                "generic return values and unsupported public exports need an explicit supported ABI shape",
            ]),
        ])
    })
}

fn maybe_value_type(type_: &Type) -> Option<ValueType> {
    match type_ {
        Type::Int => Some(ValueType::I64),
        Type::Float => Some(ValueType::F64),
        Type::Bool
        | Type::Anything
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

fn load_for_type(memory: MemoryId, offset: u32, type_: ValueType) -> Instruction {
    match type_ {
        ValueType::I64 => Instruction::I64Load(MemoryArg::new(memory, offset, 3)),
        ValueType::F64 => Instruction::F64Load(MemoryArg::new(memory, offset, 3)),
        _ => Instruction::I32Load(MemoryArg::new(memory, offset, 2)),
    }
}

fn store_for_type(memory: MemoryId, offset: u32, type_: ValueType) -> Instruction {
    match type_ {
        ValueType::I64 => Instruction::I64Store(MemoryArg::new(memory, offset, 3)),
        ValueType::F64 => Instruction::F64Store(MemoryArg::new(memory, offset, 3)),
        _ => Instruction::I32Store(MemoryArg::new(memory, offset, 2)),
    }
}

/// Compute how many depth-specific `__funcid_N` locals are needed.
///
/// Each level of indirect calls nested as arguments of another indirect
/// call needs its own local so that the outer call's saved table index
/// is not clobbered when an inner call saves its own.
///
/// Returns the number of unique depth levels (= max nesting depth + 1).
fn indirect_call_max_arg_depth(block: &ir::Block) -> usize {
    block
        .instructions
        .iter()
        .map(|i| match i {
            ir::Instruction::Evaluate { expression, .. } | ir::Instruction::LocalSet { value: expression, .. } => {
                expr_indirect_depth(expression, 0)
            }
            ir::Instruction::AssertMatch { value, .. } => expr_indirect_depth(value, 0),
        })
        .chain(std::iter::once(expr_indirect_depth(&block.result, 0)))
        .max()
        .unwrap_or(0)
}

/// Returns the minimum number of depth-indexed funcid locals the expression
/// tree requires (= max nesting level + 1, where outermost = level 0).
fn expr_indirect_depth(expr: &ir::Expression, depth: usize) -> usize {
    match &expr.kind {
        ExpressionKind::IndirectCall(call) => {
            let from_callee = expr_indirect_depth(&call.callee, depth);
            let from_args = call
                .arguments
                .iter()
                .map(|a| expr_indirect_depth(&a.value, depth + 1))
                .max()
                .unwrap_or(0);
            (depth + 1).max(from_callee).max(from_args)
        }
        _ => expr
            .children()
            .map(|c| expr_indirect_depth(c, depth))
            .max()
            .unwrap_or(0),
    }
}

// TODO: instance method
fn needs_bit_string_pattern(block: &ir::Block) -> bool {
    block.instructions.iter().any(|instruction| match instruction {
        ir::Instruction::Evaluate { expression, .. } | ir::Instruction::LocalSet { value: expression, .. } => {
            expression_has_bit_string_pattern(expression)
        }
        ir::Instruction::AssertMatch { pattern, .. } => pattern_has_bit_string(pattern),
    }) || expression_has_bit_string_pattern(&block.result)
}

// TODO: instance method
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

// TODO: instance method
fn expression_has_bit_string_pattern(expression: &ir::Expression) -> bool {
    match &expression.kind {
        ExpressionKind::Branch(branch) => branch.clauses.iter().any(|clause| {
            clause.patterns.iter().any(pattern_has_bit_string) || expression_has_bit_string_pattern(&clause.body)
        }),
        _ => expression.children().any(expression_has_bit_string_pattern),
    }
}

// TODO: instance method
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
                    | Type::Anything
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

// TODO: all of the below functions can be instance methods

fn block_needs_allocation(block: &ir::Block) -> bool {
    needs_bit_string_pattern(block)
        || needs_dynamic_decode(block)
        || block.instructions.iter().any(|instruction| match instruction {
            ir::Instruction::Evaluate { expression, .. } | ir::Instruction::LocalSet { value: expression, .. } => {
                expression_needs_allocation(expression)
            }
            ir::Instruction::AssertMatch { value, .. } => expression_needs_allocation(value),
        })
        || expression_needs_allocation(&block.result)
}

fn expression_needs_allocation(expression: &ir::Expression) -> bool {
    expression_needs_dynamic_decode(expression)
        || matches!(
            expression.kind,
            ExpressionKind::AnonymousFunction(_)
                | ExpressionKind::ListCons { .. }
                | ExpressionKind::RecordUpdate { .. }
                | ExpressionKind::Memory(_)
        )
        || matches!(&expression.kind, ExpressionKind::DirectCall(call) if matches!(call.function.as_str(), "__op_string_concat" | "__stdlib_gleam_string_append"))
        || matches!(&expression.kind, ExpressionKind::Tuple(_) | ExpressionKind::List(_) | ExpressionKind::Record(_) if !expression_is_static_allocatable(expression))
        || matches!(&expression.kind, ExpressionKind::Constructor(constructor) if !constructor.arguments.iter().all(expression_is_static_allocatable))
        || expression.children().any(expression_needs_allocation)
}

fn expression_is_static_allocatable(expression: &ir::Expression) -> bool {
    match &expression.kind {
        ExpressionKind::Literal(_) | ExpressionKind::FunctionValue(_) | ExpressionKind::BitArray(_) => true,
        ExpressionKind::Tuple(items) | ExpressionKind::List(items) => {
            items.iter().all(expression_is_static_allocatable)
        }
        ExpressionKind::Record(record) => record
            .fields
            .iter()
            .all(|field| expression_is_static_allocatable(&field.value)),
        ExpressionKind::Constructor(constructor) => constructor.arguments.iter().all(expression_is_static_allocatable),
        _ => false,
    }
}

fn needs_dynamic_decode(block: &ir::Block) -> bool {
    block.instructions.iter().any(|instruction| match instruction {
        ir::Instruction::Evaluate { expression, .. } | ir::Instruction::LocalSet { value: expression, .. } => {
            expression_needs_dynamic_decode(expression)
        }
        ir::Instruction::AssertMatch { value, .. } => expression_needs_dynamic_decode(value),
    }) || expression_needs_dynamic_decode(&block.result)
}

fn expression_needs_dynamic_decode(expression: &ir::Expression) -> bool {
    matches!(&expression.kind, ExpressionKind::DirectCall(call) if call.function.starts_with("__stdlib_gleam_dynamic"))
        || expression.children().any(expression_needs_dynamic_decode)
}

fn needs_dynamic_closure_dispatch(block: &ir::Block) -> bool {
    block.instructions.iter().any(|instruction| match instruction {
        ir::Instruction::Evaluate { expression, .. } | ir::Instruction::LocalSet { value: expression, .. } => {
            expression_needs_dynamic_closure_dispatch(expression)
        }
        ir::Instruction::AssertMatch { value, .. } => expression_needs_dynamic_closure_dispatch(value),
    }) || expression_needs_dynamic_closure_dispatch(&block.result)
}

fn expression_needs_dynamic_closure_dispatch(expression: &ir::Expression) -> bool {
    matches!(
        &expression.kind,
        ExpressionKind::DirectCall(call)
            if call.function == "__stdlib_gleam_dynamic_decode_run"
                && call.arguments.get(1).is_some_and(|argument| {
                    matches!(
                        &argument.value.kind,
                        ExpressionKind::DirectCall(decoder)
                            if matches!(
                                decoder.function.as_str(),
                                "__stdlib_gleam_dynamic_decode_map"
                                    | "__stdlib_gleam_dynamic_decode_then"
                                    | "__stdlib_gleam_dynamic_decode_recursive"
                                    | "__stdlib_gleam_dynamic_decode_field"
                                    | "__stdlib_gleam_dynamic_decode_subfield"
                            )
                    )
                })
    ) || expression.children().any(expression_needs_dynamic_closure_dispatch)
}

fn block_needs_scratch(block: &ir::Block) -> bool {
    needs_bit_string_pattern(block)
        || needs_dynamic_closure_dispatch(block)
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
        ExpressionKind::List(_) if !expression_is_static_allocatable(expression) => true,
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
