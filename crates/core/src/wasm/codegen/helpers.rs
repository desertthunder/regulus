use std::collections::{HashMap, HashSet};

use super::{DebugImport, JsAbiBoundary, StructuredError, StructuredResult, WasmTarget};
use crate::ast::LiteralKind;
use crate::diagnostic::{Diagnostic, DiagnosticCode, Diagnostics, Label};
use crate::ir::{self, ExpressionKind};
use crate::source::Span;
use crate::wasm::builder::{Instruction, MemoryArg, MemoryId, ValueType};
use crate::{abi::is_allowed_anything_external, types::Type};

pub fn unsupported_structured_diagnostics(module: &ir::Module) -> Diagnostics {
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

pub fn invariant_diagnostics(module: &ir::Module, message: &str) -> Diagnostics {
    vec![
        Diagnostic::new(DiagnosticCode::WasmError, message.to_string()).with_label(Label::primary(
            module.span,
            "internal Wasm invariant failed while compiling this module",
        )),
    ]
}

pub fn literal_parse_diagnostic(literal: &ir::IrLiteral, span: Span, expected: &'static str) -> StructuredError {
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

pub fn literal_type(literal: &ir::IrLiteral) -> Type {
    match literal.kind {
        LiteralKind::Int => Type::Int,
        LiteralKind::Float => Type::Float,
        LiteralKind::Bool => Type::Bool,
        LiteralKind::String => Type::String,
        LiteralKind::Nil => Type::Nil,
    }
}

pub fn result_types(type_: &Type, span: Span) -> StructuredResult<Vec<ValueType>> {
    if matches!(type_, Type::Nil) { Ok(Vec::new()) } else { Ok(vec![value_type(type_, span)?]) }
}

pub fn validate_anything_boundary_abi(module: &ir::Module) -> StructuredResult<()> {
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

pub fn anything_boundary_abi_diagnostic(
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

pub fn validate_js_host_abi(module: &ir::Module, target: WasmTarget) -> StructuredResult<()> {
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

pub fn validate_js_host_function_shape(
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

pub fn js_host_abi_diagnostic(
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

pub fn is_supported_js_host_parameter(module: &ir::Module, type_: &Type) -> bool {
    matches!(type_, Type::Int | Type::Float | Type::Bool | Type::String) || is_js_host_opaque_handle(module, type_)
}

pub fn is_supported_js_host_return(module: &ir::Module, type_: &Type, structured_allowed: bool) -> bool {
    matches!(type_, Type::Int | Type::Float | Type::Bool | Type::String | Type::Nil)
        || is_js_host_opaque_handle(module, type_)
        || (structured_allowed && is_supported_js_host_structured_return(module, type_))
}

pub fn is_supported_js_host_structured_return(module: &ir::Module, type_: &Type) -> bool {
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

pub fn is_supported_js_host_field(module: &ir::Module, type_: &Type) -> bool {
    is_supported_js_host_parameter(module, type_) || is_supported_js_host_structured_return(module, type_)
}

pub fn is_js_host_opaque_handle(module: &ir::Module, type_: &Type) -> bool {
    match type_ {
        Type::Opaque { .. } => true,
        Type::Custom { name, .. } => module
            .type_declarations
            .iter()
            .any(|type_| type_.name == *name && type_.opaque),
        _ => false,
    }
}

pub fn native_dict_external_name(function: &str, has_local_function: bool) -> Option<&'static str> {
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

pub fn module_exports_arena_scoped_values(module: &ir::Module) -> bool {
    module.functions.iter().any(|function| {
        matches!(function.abi.boundary, ir::CallBoundary::ModuleExport)
            && block_needs_allocation(&function.body)
            && (is_heap_managed_type(&function.return_type)
                || function.params.iter().any(|param| is_heap_managed_type(&param.type_)))
    })
}

pub fn reachable_functions(module: &ir::Module) -> Vec<&ir::Function> {
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

pub fn collect_function_refs(function: &ir::Function, by_name: &HashMap<&str, &ir::Function>, stack: &mut Vec<String>) {
    for instruction in &function.body.instructions {
        collect_expression_refs(instruction.expression(), by_name, stack);
    }
    collect_expression_refs(&function.body.result, by_name, stack);
}

pub fn collect_expression_refs(
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

pub fn is_heap_managed_type(type_: &Type) -> bool {
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

pub fn value_type(type_: &Type, span: Span) -> StructuredResult<ValueType> {
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

pub fn maybe_value_type(type_: &Type) -> Option<ValueType> {
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

pub fn load_for_type(memory: MemoryId, offset: u32, type_: ValueType) -> Instruction {
    match type_ {
        ValueType::I64 => Instruction::I64Load(MemoryArg::new(memory, offset, 3)),
        ValueType::F64 => Instruction::F64Load(MemoryArg::new(memory, offset, 3)),
        _ => Instruction::I32Load(MemoryArg::new(memory, offset, 2)),
    }
}

pub fn store_for_type(memory: MemoryId, offset: u32, type_: ValueType) -> Instruction {
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
pub fn indirect_call_max_arg_depth(block: &ir::Block) -> usize {
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
pub fn expr_indirect_depth(expr: &ir::Expression, depth: usize) -> usize {
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
pub fn needs_bit_string_pattern(block: &ir::Block) -> bool {
    block.instructions.iter().any(|instruction| match instruction {
        ir::Instruction::Evaluate { expression, .. } | ir::Instruction::LocalSet { value: expression, .. } => {
            expression_has_bit_string_pattern(expression)
        }
        ir::Instruction::AssertMatch { pattern, .. } => pattern_has_bit_string(pattern),
    }) || expression_has_bit_string_pattern(&block.result)
}

// TODO: instance method
pub fn pattern_has_bit_string(pattern: &ir::IrPattern) -> bool {
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
pub fn expression_has_bit_string_pattern(expression: &ir::Expression) -> bool {
    match &expression.kind {
        ExpressionKind::Branch(branch) => branch.clauses.iter().any(|clause| {
            clause.patterns.iter().any(pattern_has_bit_string) || expression_has_bit_string_pattern(&clause.body)
        }),
        _ => expression.children().any(expression_has_bit_string_pattern),
    }
}

// TODO: instance method
pub fn needed_debug_imports(function: &ir::Function) -> Vec<DebugImport> {
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

pub fn collect_block_debug_imports(block: &ir::Block, imports: &mut Vec<DebugImport>) {
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

pub fn collect_expression_debug_imports(expression: &ir::Expression, imports: &mut Vec<DebugImport>) {
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

pub fn block_needs_allocation(block: &ir::Block) -> bool {
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

pub fn expression_needs_allocation(expression: &ir::Expression) -> bool {
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

pub fn expression_is_static_allocatable(expression: &ir::Expression) -> bool {
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

pub fn needs_dynamic_decode(block: &ir::Block) -> bool {
    block.instructions.iter().any(|instruction| match instruction {
        ir::Instruction::Evaluate { expression, .. } | ir::Instruction::LocalSet { value: expression, .. } => {
            expression_needs_dynamic_decode(expression)
        }
        ir::Instruction::AssertMatch { value, .. } => expression_needs_dynamic_decode(value),
    }) || expression_needs_dynamic_decode(&block.result)
}

pub fn expression_needs_dynamic_decode(expression: &ir::Expression) -> bool {
    matches!(&expression.kind, ExpressionKind::DirectCall(call) if call.function.starts_with("__stdlib_gleam_dynamic"))
        || expression.children().any(expression_needs_dynamic_decode)
}

pub fn needs_dynamic_closure_dispatch(block: &ir::Block) -> bool {
    block.instructions.iter().any(|instruction| match instruction {
        ir::Instruction::Evaluate { expression, .. } | ir::Instruction::LocalSet { value: expression, .. } => {
            expression_needs_dynamic_closure_dispatch(expression)
        }
        ir::Instruction::AssertMatch { value, .. } => expression_needs_dynamic_closure_dispatch(value),
    }) || expression_needs_dynamic_closure_dispatch(&block.result)
}

pub fn expression_needs_dynamic_closure_dispatch(expression: &ir::Expression) -> bool {
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

pub fn block_needs_scratch(block: &ir::Block) -> bool {
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

pub fn expression_needs_scratch(expression: &ir::Expression) -> bool {
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
