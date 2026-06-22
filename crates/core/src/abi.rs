use std::fmt::Display;

use crate::diagnostic::{Diagnostic, DiagnosticCode, Diagnostics};
use crate::shared::unquote;
use crate::types::{ExternalFunctionInfo, Type};
use crate::{ast, source::Span};

pub const STDLIB_IO_HOST_MODULE: &str = "__regulus_stdlib_io";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StdlibHostAdapter {
    pub import_module: &'static str,
    pub import_name: &'static str,
}

const STDLIB_HOST_ADAPTERS: &[(&str, &str, StdlibHostAdapter)] = &[
    (
        "gleam/io",
        "print",
        StdlibHostAdapter { import_module: STDLIB_IO_HOST_MODULE, import_name: "print" },
    ),
    (
        "gleam/io",
        "println",
        StdlibHostAdapter { import_module: STDLIB_IO_HOST_MODULE, import_name: "println" },
    ),
];

pub fn stdlib_host_adapter(module: &str, member: &str) -> Option<StdlibHostAdapter> {
    STDLIB_HOST_ADAPTERS
        .iter()
        .find_map(|(adapter_module, adapter_member, adapter)| {
            (*adapter_module == module && *adapter_member == member).then_some(*adapter)
        })
}

pub fn stdlib_host_adapters() -> impl Iterator<Item = (&'static str, &'static str, StdlibHostAdapter)> {
    STDLIB_HOST_ADAPTERS.iter().copied()
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum AbiPosition {
    Parameter { index: usize },
    Return,
}

impl Display for AbiPosition {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Parameter { index } => {
                let data = format!("parameter {}", index + 1);
                f.write_str(&data)
            }
            Self::Return => f.write_str("return"),
        }
    }
}

impl AbiPosition {
    fn description(self) -> String {
        self.to_string()
    }
}

#[derive(Clone, Copy)]
struct NamedFunction<'a> {
    func_name: &'a str,
    module: &'a str,
    function: &'a str,
}

impl<'a> NamedFunction<'a> {
    fn new(func_name: &'a str, module: &'a str, function: &'a str) -> Self {
        Self { func_name, module, function }
    }
}

#[derive(Clone, Copy)]
struct ValidateParams<'a> {
    pos: AbiPosition,
    type_: &'a Type,
    span: Span,
    allow_nil: bool,
    allow_anything: bool,
}

impl<'a> ValidateParams<'a> {
    fn new(pos: AbiPosition, type_: &'a Type, span: Span, allow_nil: bool) -> Self {
        Self { pos, type_, span, allow_nil, allow_anything: false }
    }

    fn allowing_anything(mut self, allowed: bool) -> Self {
        self.allow_anything = allowed;
        self
    }
}

pub fn validate_extern_function_abi(
    func: &ast::ExternalFunction, type_: &Type, allow_stdlib_anything: bool,
) -> Diagnostics {
    let mut diagnostics = Vec::new();
    let Type::Function { params, return_type } = type_ else {
        return diagnostics;
    };
    let allow_anything = is_allowed_anything_external(
        allow_stdlib_anything,
        &unquote(&func.body.module.source),
        &unquote(&func.body.function.source),
    );

    for (index, type_) in params.iter().enumerate() {
        let span = func
            .parameters
            .get(index)
            .and_then(|parameter| parameter.type_annotation.as_ref())
            .map(|annotation| annotation.span)
            .unwrap_or(func.span);
        validate_value(
            func,
            &mut diagnostics,
            &ValidateParams::new(AbiPosition::Parameter { index }, type_, span, false)
                .allowing_anything(allow_anything),
        );
    }

    validate_value(
        func,
        &mut diagnostics,
        &ValidateParams::new(AbiPosition::Return, return_type, func.return_type.span, true)
            .allowing_anything(allow_anything),
    );

    diagnostics
}

pub fn validate_external_info_abi(name: &str, info: &ExternalFunctionInfo, type_: &Type) -> Diagnostics {
    let mut diagnostics = Vec::new();
    let Type::Function { params, return_type } = type_ else {
        return diagnostics;
    };
    let allow_anything = is_allowed_anything_external(true, &info.module, &info.function);

    for (index, type_) in params.iter().enumerate() {
        validate_named_value(
            NamedFunction::new(name, &info.module, &info.function),
            &mut diagnostics,
            ValidateParams::new(AbiPosition::Parameter { index }, type_, info.span, false)
                .allowing_anything(allow_anything),
        );
    }

    validate_named_value(
        NamedFunction::new(name, &info.module, &info.function),
        &mut diagnostics,
        ValidateParams::new(AbiPosition::Return, return_type, info.span, true).allowing_anything(allow_anything),
    );

    diagnostics
}

fn validate_value(func: &ast::ExternalFunction, diagnostics: &mut Diagnostics, params: &ValidateParams) {
    if is_supported_extern_abi_value(params.type_, params.allow_nil, params.allow_anything) {
        return;
    }

    let mut diagnostic = Diagnostic::spanned(
        DiagnosticCode::LoweringError,
        format!(
            "external function `{}` {} uses unsupported ABI shape `{}`",
            func.name.text,
            params.pos.description(),
            abi_shape(params.type_)
        ),
        params.span,
        unsupported_abi_label(params.type_),
    )
    .with_note(format!(
        "host import `{}.{}` must use concrete scalar values, managed values, or Nil returns",
        unquote(&func.body.module.source),
        unquote(&func.body.function.source)
    ));
    if params.type_.contains_anything() {
        diagnostic =
            diagnostic.with_note("`anything` is only supported at stdlib-native dynamic and inspection boundaries");
    }
    diagnostics.push(diagnostic);
}

fn validate_named_value(named_func: NamedFunction, diagnostics: &mut Diagnostics, params: ValidateParams) {
    if is_supported_extern_abi_value(params.type_, params.allow_nil, params.allow_anything) {
        return;
    }

    let mut diagnostic = Diagnostic::spanned(
        DiagnosticCode::LoweringError,
        format!(
            "external function `{}` {} uses unsupported ABI shape `{}`",
            named_func.func_name,
            params.pos.description(),
            abi_shape(params.type_)
        ),
        params.span,
        unsupported_abi_label(params.type_),
    )
    .with_note(format!(
        "host import `{}.{}` must use concrete scalar values, managed values, or Nil returns",
        named_func.module, named_func.function
    ));
    if params.type_.contains_anything() {
        diagnostic =
            diagnostic.with_note("`anything` is only supported at stdlib-native dynamic and inspection boundaries");
    }
    diagnostics.push(diagnostic);
}

fn is_supported_extern_abi_value(type_: &Type, nil_allowed: bool, anything_allowed: bool) -> bool {
    match type_ {
        Type::Nil => nil_allowed,
        Type::Anything => anything_allowed,
        Type::Generic(_) => false,
        Type::Tuple(items) => items
            .iter()
            .all(|item| is_supported_extern_abi_value(item, false, anything_allowed)),
        Type::List(item) => is_supported_extern_abi_value(item, false, anything_allowed),
        Type::Record { fields, .. } => fields
            .iter()
            .all(|field| is_supported_extern_abi_value(&field.type_, false, anything_allowed)),
        Type::Custom { args, .. } | Type::Opaque { args, .. } => args
            .iter()
            .all(|arg| is_supported_extern_abi_value(arg, false, anything_allowed)),
        Type::Function { params, return_type } => {
            params
                .iter()
                .all(|param| is_supported_extern_abi_value(param, false, anything_allowed))
                && is_supported_extern_abi_value(return_type, true, anything_allowed)
        }
        Type::Int | Type::Float | Type::String | Type::BitArray | Type::Bool => true,
    }
}

pub fn is_allowed_anything_external(allow_stdlib_anything: bool, module: &str, function: &str) -> bool {
    if !allow_stdlib_anything {
        return false;
    }
    let is_stdlib_asset = module.ends_with("gleam_stdlib.mjs");
    is_stdlib_asset && matches!(function, "identity" | "index" | "inspect")
}

fn abi_shape(type_: &Type) -> String {
    if type_.contains_anything() { type_.display() } else { format!("{type_:?}") }
}

fn unsupported_abi_label(type_: &Type) -> &'static str {
    if type_.contains_anything() {
        "unsupported `anything` ABI shape here"
    } else {
        "unsupported external ABI shape here"
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::source::SourceFileId;

    const TEST_SPAN: Span = Span { file_id: SourceFileId(u32::MAX), start: 0, end: 0 };

    #[test]
    fn records_stdlib_host_adapters_in_abi_table() {
        assert_eq!(
            stdlib_host_adapter("gleam/io", "println"),
            Some(StdlibHostAdapter { import_module: STDLIB_IO_HOST_MODULE, import_name: "println" })
        );
        assert_eq!(stdlib_host_adapter("gleam/int", "to_string"), None);
    }

    #[test]
    fn allows_anything_for_stdlib_native_dynamic_and_inspect_externals() {
        let type_ = Type::Function {
            params: vec![Type::Anything],
            return_type: Box::new(Type::Custom { name: "Dynamic".into(), args: Vec::new() }),
        };
        let info = ExternalFunctionInfo {
            target: Some("javascript".into()),
            module: "../gleam_stdlib.mjs".into(),
            function: "identity".into(),
            span: TEST_SPAN,
        };

        assert!(validate_external_info_abi("cast", &info, &type_).is_empty());

        let inspect = Type::Function {
            params: vec![Type::Anything],
            return_type: Box::new(Type::Custom { name: "StringTree".into(), args: Vec::new() }),
        };
        let info = ExternalFunctionInfo { function: "inspect".into(), ..info };

        assert!(validate_external_info_abi("do_inspect", &info, &inspect).is_empty());
    }

    #[test]
    fn rejects_anything_for_general_externals() {
        let type_ = Type::Function { params: vec![Type::Anything], return_type: Box::new(Type::String) };
        let info = ExternalFunctionInfo {
            target: Some("javascript".into()),
            module: "regulus/js".into(),
            function: "inspect".into(),
            span: TEST_SPAN,
        };

        let diagnostics = validate_external_info_abi("inspect", &info, &type_);

        assert_eq!(diagnostics.len(), 1);
        assert!(
            diagnostics[0]
                .message
                .contains("external function `inspect` parameter 1 uses unsupported ABI shape `anything`")
        );
    }
}
