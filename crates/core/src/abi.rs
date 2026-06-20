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
}

impl<'a> ValidateParams<'a> {
    fn new(pos: AbiPosition, type_: &'a Type, span: Span, allow_nil: bool) -> Self {
        Self { pos, type_, span, allow_nil }
    }
}

pub fn validate_extern_function_abi(func: &ast::ExternalFunction, type_: &Type) -> Diagnostics {
    let mut diagnostics = Vec::new();
    let Type::Function { params, return_type } = type_ else {
        return diagnostics;
    };

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
            &ValidateParams::new(AbiPosition::Parameter { index }, type_, span, false),
        );
    }

    validate_value(
        func,
        &mut diagnostics,
        &ValidateParams::new(AbiPosition::Return, return_type, func.return_type.span, true),
    );

    diagnostics
}

pub fn validate_external_info_abi(name: &str, info: &ExternalFunctionInfo, type_: &Type) -> Diagnostics {
    let mut diagnostics = Vec::new();
    let Type::Function { params, return_type } = type_ else {
        return diagnostics;
    };

    for (index, type_) in params.iter().enumerate() {
        validate_named_value(
            NamedFunction::new(name, &info.module, &info.function),
            &mut diagnostics,
            ValidateParams::new(AbiPosition::Parameter { index }, type_, info.span, false),
        );
    }

    validate_named_value(
        NamedFunction::new(name, &info.module, &info.function),
        &mut diagnostics,
        ValidateParams::new(AbiPosition::Return, return_type, info.span, true),
    );

    diagnostics
}

fn validate_value(func: &ast::ExternalFunction, diagnostics: &mut Diagnostics, params: &ValidateParams) {
    if is_supported_extern_abi_value(params.type_, params.allow_nil) {
        return;
    }

    diagnostics.push(
        Diagnostic::spanned(
            DiagnosticCode::LoweringError,
            format!(
                "external function `{}` {} uses unsupported ABI shape `{:?}`",
                func.name.text,
                params.pos.description(),
                params.type_
            ),
            params.span,
            "unsupported external ABI shape here",
        )
        .with_note(format!(
            "host import `{}.{}` must use concrete scalar values, managed values, or Nil returns",
            unquote(&func.body.module.source),
            unquote(&func.body.function.source)
        )),
    );
}

fn validate_named_value(named_func: NamedFunction, diagnostics: &mut Diagnostics, params: ValidateParams) {
    if is_supported_extern_abi_value(params.type_, params.allow_nil) {
        return;
    }

    diagnostics.push(
        Diagnostic::spanned(
            DiagnosticCode::LoweringError,
            format!(
                "external function `{}` {} uses unsupported ABI shape `{:?}`",
                named_func.func_name,
                params.pos.description(),
                params.type_
            ),
            params.span,
            "unsupported external ABI shape here",
        )
        .with_note(format!(
            "host import `{}.{}` must use concrete scalar values, managed values, or Nil returns",
            named_func.module, named_func.function
        )),
    );
}

fn is_supported_extern_abi_value(type_: &Type, nil_allowed: bool) -> bool {
    match type_ {
        Type::Nil => nil_allowed,
        Type::Generic(_) => false,
        Type::Tuple(items) => items.iter().all(|item| is_supported_extern_abi_value(item, false)),
        Type::List(item) => is_supported_extern_abi_value(item, false),
        Type::Record { fields, .. } => fields
            .iter()
            .all(|field| is_supported_extern_abi_value(&field.type_, false)),
        Type::Custom { args, .. } | Type::Opaque { args, .. } => {
            args.iter().all(|arg| is_supported_extern_abi_value(arg, false))
        }
        Type::Function { params, return_type } => {
            params.iter().all(|param| is_supported_extern_abi_value(param, false))
                && is_supported_extern_abi_value(return_type, true)
        }
        Type::Int | Type::Float | Type::String | Type::BitArray | Type::Bool => true,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn records_stdlib_host_adapters_in_abi_table() {
        assert_eq!(
            stdlib_host_adapter("gleam/io", "println"),
            Some(StdlibHostAdapter { import_module: STDLIB_IO_HOST_MODULE, import_name: "println" })
        );
        assert_eq!(stdlib_host_adapter("gleam/int", "to_string"), None);
    }
}
