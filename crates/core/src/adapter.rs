use crate::ir::{self, CallBoundary, ExportKind};
use crate::types::Type;

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
        if let CallBoundary::HostImport { module, name } = &function.abi.boundary
            && let Some(shape) = function_shape_json(&function.params, &function.return_type)
        {
            imports.push(format!("{}:{shape}", json_string(&format!("{module}.{name}"))));
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
        if let Some(shape) = function_shape_json(&function.params, &function.return_type) {
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

fn function_shape_json(params: &[ir::Local], return_type: &Type) -> Option<String> {
    let params = params
        .iter()
        .map(|param| js_abi_type(&param.type_).map(json_string))
        .collect::<Option<Vec<_>>>()?;
    let result = js_abi_return_type(return_type)?;
    Some(format!(
        "{{\"params\":[{}],\"result\":{}}}",
        params.join(","),
        json_string(result)
    ))
}

fn js_abi_return_type(type_: &Type) -> Option<&'static str> {
    if matches!(type_, Type::Nil) { Some("Nil") } else { js_abi_type(type_) }
}

fn js_abi_type(type_: &Type) -> Option<&'static str> {
    match type_ {
        Type::Int => Some("Int"),
        Type::Float => Some("Float"),
        Type::Bool => Some("Bool"),
        Type::String => Some("String"),
        _ => None,
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
