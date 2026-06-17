use std::collections::HashMap;

use crate::ir::{self, CallBoundary, ExportKind};
use crate::types::{FieldInfo, Type};

const BUNDLER_ADAPTER: &str = include_str!("adapter.js");

pub fn bundler_adapter(wasm_file: &str) -> String {
    bundler_adapter_with_metadata(wasm_file, None)
}

pub fn bundler_adapter_for_module(wasm_file: &str, module: &ir::Module) -> String {
    bundler_adapter_with_metadata(wasm_file, Some(&js_abi_metadata(module)))
}

fn bundler_adapter_with_metadata(wasm_file: &str, metadata: Option<&str>) -> String {
    BUNDLER_ADAPTER.replace("__WASM_FILE__", wasm_file).replace(
        "__REGULUS_JS_ABI_METADATA__",
        metadata.unwrap_or("{\"imports\":{},\"exports\":{}}"),
    )
}

fn js_abi_metadata(module: &ir::Module) -> String {
    let mut imports = Vec::new();
    let mut exports = Vec::new();

    for function in &module.functions {
        if let CallBoundary::HostImport { module: import_module, name } = &function.abi.boundary
            && let Some(shape) = function_shape_json(module, &function.params, &function.return_type, false)
        {
            imports.push(format!("{}:{shape}", json_string(&format!("{import_module}.{name}"))));
        }
    }

    for export in module
        .exports
        .iter()
        .filter(|export| export.kind == ExportKind::Function)
    {
        let Some(function) = module
            .functions
            .iter()
            .find(|function| function.name == export.backend_name())
        else {
            continue;
        };
        if let Some(shape) = function_shape_json(module, &function.params, &function.return_type, true) {
            exports.push(format!("{}:{shape}", json_string(&export.name)));
        }
    }

    imports.sort();
    exports.sort();
    format!(
        "{{\"imports\":{{{}}},\"exports\":{{{}}}}}",
        imports.join(","),
        exports.join(",")
    )
}

fn function_shape_json(
    module: &ir::Module, params: &[ir::Local], return_type: &Type, structured_return: bool,
) -> Option<String> {
    let params = params
        .iter()
        .map(|param| simple_js_abi_type(module, &param.type_).map(json_string))
        .collect::<Option<Vec<_>>>()?;
    let result = return_shape_json(module, return_type, structured_return)?;
    Some(format!("{{\"params\":[{}],\"result\":{result}}}", params.join(",")))
}

fn return_shape_json(module: &ir::Module, type_: &Type, structured: bool) -> Option<String> {
    match type_ {
        Type::Nil => Some(json_string("Nil")),
        _ => match simple_js_abi_type(module, type_) {
            Some(type_) => Some(json_string(type_)),
            None if structured => structured_shape_json(module, type_),
            _ => None,
        },
    }
}

fn structured_shape_json(module: &ir::Module, type_: &Type) -> Option<String> {
    match type_ {
        Type::Tuple(items) => {
            let items = items
                .iter()
                .map(|item| field_shape_json(module, item))
                .collect::<Option<Vec<_>>>()?;
            Some(format!("{{\"kind\":\"Tuple\",\"items\":[{}]}}", items.join(",")))
        }
        Type::List(item) => Some(format!(
            "{{\"kind\":\"List\",\"item\":{}}}",
            field_shape_json(module, item)?
        )),
        Type::Record { fields, .. } => {
            let fields = fields
                .iter()
                .map(|field| {
                    Some(format!(
                        "{{\"name\":{},\"type\":{}}}",
                        json_string(&field.name),
                        field_shape_json(module, &field.type_)?
                    ))
                })
                .collect::<Option<Vec<_>>>()?;
            Some(format!("{{\"kind\":\"Record\",\"fields\":[{}]}}", fields.join(",")))
        }
        Type::Custom { name, args } if name == "Result" && args.len() == 2 => Some(format!(
            "{{\"kind\":\"Result\",\"ok\":{},\"error\":{}}}",
            field_shape_json(module, &args[0])?,
            field_shape_json(module, &args[1])?
        )),
        Type::Custom { name, args } if name == "Option" && args.len() == 1 => Some(format!(
            "{{\"kind\":\"Option\",\"some\":{}}}",
            field_shape_json(module, &args[0])?
        )),
        Type::Custom { name, args } => custom_shape_json(module, name, args),
        _ => None,
    }
}

fn custom_shape_json(module: &ir::Module, name: &str, args: &[Type]) -> Option<String> {
    let type_ = module.type_declarations.iter().find(|type_| type_.name == name)?;
    let substitutions = type_
        .parameters
        .iter()
        .cloned()
        .zip(args.iter().cloned())
        .collect::<HashMap<_, _>>();
    let variants = type_
        .constructors
        .iter()
        .map(|constructor| {
            let fields = constructor
                .fields
                .iter()
                .map(|field| {
                    let type_ = substitute_generics(&field.type_, &substitutions);
                    let shape = field_shape_json(module, &type_)?;
                    match &field.name {
                        Some(name) => Some(format!("{{\"name\":{},\"type\":{shape}}}", json_string(name))),
                        None => Some(shape),
                    }
                })
                .collect::<Option<Vec<_>>>()?;
            Some(format!(
                "{}:{{\"fields\":[{}]}}",
                json_string(&constructor.name),
                fields.join(",")
            ))
        })
        .collect::<Option<Vec<_>>>()?;
    Some(format!(
        "{{\"kind\":\"Custom\",\"name\":{},\"variants\":{{{}}}}}",
        json_string(name),
        variants.join(",")
    ))
}

fn field_shape_json(module: &ir::Module, type_: &Type) -> Option<String> {
    match simple_js_abi_type(module, type_) {
        Some(type_) => Some(json_string(type_)),
        None => structured_shape_json(module, type_),
    }
}

fn simple_js_abi_type(module: &ir::Module, type_: &Type) -> Option<&'static str> {
    match type_ {
        Type::Int => Some("Int"),
        Type::Float => Some("Float"),
        Type::Bool => Some("Bool"),
        Type::String => Some("String"),
        Type::Opaque { .. } => Some("Handle"),
        Type::Custom { name, .. } if is_opaque_type(module, name) => Some("Handle"),
        _ => None,
    }
}

fn is_opaque_type(module: &ir::Module, name: &str) -> bool {
    module
        .type_declarations
        .iter()
        .any(|type_| type_.name == name && type_.opaque)
}

fn substitute_generics(type_: &Type, substitutions: &HashMap<String, Type>) -> Type {
    match type_ {
        Type::Generic(name) => substitutions.get(name).cloned().unwrap_or_else(|| type_.clone()),
        Type::Tuple(items) => Type::Tuple(
            items
                .iter()
                .map(|item| substitute_generics(item, substitutions))
                .collect(),
        ),
        Type::List(item) => Type::List(Box::new(substitute_generics(item, substitutions))),
        Type::Record { name, fields } => Type::Record {
            name: name.clone(),
            fields: fields
                .iter()
                .map(|field| FieldInfo::new(field.name.clone(), substitute_generics(&field.type_, substitutions)))
                .collect(),
        },
        Type::Custom { name, args } => Type::Custom {
            name: name.clone(),
            args: args.iter().map(|arg| substitute_generics(arg, substitutions)).collect(),
        },
        Type::Opaque { name, args } => Type::Opaque {
            name: name.clone(),
            args: args.iter().map(|arg| substitute_generics(arg, substitutions)).collect(),
        },
        Type::Function { params, return_type } => Type::Function {
            params: params
                .iter()
                .map(|param| substitute_generics(param, substitutions))
                .collect(),
            return_type: Box::new(substitute_generics(return_type, substitutions)),
        },
        Type::Int | Type::Float | Type::String | Type::BitArray | Type::Bool | Type::Nil => type_.clone(),
    }
}

// TODO: use serde for this?
fn json_string(value: &str) -> String {
    let mut out = String::from("\"");
    for ch in value.chars() {
        match ch {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            ch if ch.is_control() => out.push_str(&format!("\\u{:04x}", ch as u32)),
            ch => out.push(ch),
        }
    }
    out.push('"');
    out
}
