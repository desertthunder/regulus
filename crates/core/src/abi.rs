use std::fmt::Display;

use crate::diagnostic::{Diagnostic, DiagnosticCode, Diagnostics, Label};
use crate::{ast, source::Span, types::Type};

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
            AbiPosition::Parameter { index },
            type_,
            span,
            false,
            &mut diagnostics,
        );
    }

    validate_value(
        func,
        AbiPosition::Return,
        return_type,
        func.return_type.span,
        true,
        &mut diagnostics,
    );

    diagnostics
}

fn validate_value(
    func: &ast::ExternalFunction, pos: AbiPosition, type_: &Type, span: Span, nil_allowed: bool,
    diagnostics: &mut Diagnostics,
) {
    if is_supported_extern_abi_value(type_, nil_allowed) {
        return;
    }

    diagnostics.push(
        Diagnostic::new(
            DiagnosticCode::LoweringError,
            format!(
                "external function `{}` {} uses unsupported ABI shape `{:?}`",
                func.name.text,
                pos.description(),
                type_
            ),
        )
        .with_label(Label::primary(span, "unsupported external ABI shape here"))
        .with_note(format!(
            "host import `{}.{}` must use concrete scalar values, managed values, or Nil returns",
            unquote(&func.body.module.source),
            unquote(&func.body.function.source)
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

fn unquote(source: &str) -> String {
    source.trim_matches('"').to_string()
}
