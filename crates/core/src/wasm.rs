//! WebAssembly backend.
//!
//! This module owns target selection and backend orchestration.

mod binary;
mod builder;
mod codegen;
mod encode;
mod validator;

pub mod fragments;

#[cfg(test)]
mod tests;

use std::collections::{HashMap, HashSet};
use std::fmt::Write;

use crate::diagnostic::{Diagnostic, DiagnosticCode, Diagnostics, Label};
use crate::ir;
use crate::runtime;
use crate::target::CompileTarget;

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

impl EmitOptions {
    pub fn new(target: WasmTarget) -> Self {
        Self { target }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum WasmTarget {
    #[default]
    Wasmtime,
    Browser,
    Bundler,
    Nodejs,
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
            CompileTarget::Bundler => Self::Bundler,
            CompileTarget::Nodejs => Self::Nodejs,
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
            Self::Bundler => "bundler",
            Self::Nodejs => "nodejs",
            Self::Wasi => "wasi",
        }
    }

    fn host_module(self) -> &'static str {
        match self {
            Self::Wasmtime => "env",
            Self::Browser => "browser",
            Self::Bundler => "regulus/js",
            Self::Nodejs => "nodejs",
            Self::Wasi => "wasi_snapshot_preview1",
        }
    }

    fn is_js_host(self) -> bool {
        matches!(self, Self::Browser | Self::Bundler | Self::Nodejs)
    }

    fn accepts_host_module(self, module: &str) -> bool {
        match self {
            Self::Wasmtime => module == "env",
            Self::Browser => module == "browser" || module == "regulus/js",
            Self::Bundler => module == "regulus/js",
            Self::Nodejs => module == "nodejs" || module == "regulus/js",
            Self::Wasi => module == "wasi_snapshot_preview1",
        }
    }

    fn accepts_host_import(self, module: &str, name: &str) -> bool {
        if !self.accepts_host_module(module) {
            return false;
        }
        match self {
            Self::Browser if module == "browser" => matches!(
                name,
                "fetch"
                    | "localStorage.getItem"
                    | "localStorage.setItem"
                    | "localStorage.removeItem"
                    | "time.now"
                    | "online.isOnline"
                    | "print"
                    | "println"
                    | "debug_i64"
                    | "debug_f64"
                    | "debug_bool"
                    | "debug_value"
            ),
            Self::Nodejs if module == "nodejs" => matches!(name, "env.get" | "time.now"),
            _ => true,
        }
    }
}

// FIXME: this implementation being here doesn't sit right with me.
impl ir::Module {
    pub fn emit_wasm(&self) -> Result<WasmModule, Diagnostics> {
        self.emit_wasm_with_options(EmitOptions::default())
    }

    pub fn emit_wasm_with_options(&self, options: EmitOptions) -> Result<WasmModule, Diagnostics> {
        let module = codegen::emit(self, options)?;
        let wat = module.structured_wat()?;
        let bytes = module.structured_bytes()?;
        Ok(WasmModule { wat, bytes })
    }

    pub fn emit_wat(&self) -> Result<String, Diagnostics> {
        self.emit_wat_with_options(EmitOptions::default())
    }

    pub fn emit_wat_with_options(&self, options: EmitOptions) -> Result<String, Diagnostics> {
        let module = codegen::emit(self, options)?;
        module.structured_wat()
    }
}

// FIXME: this implementation being here doesn't sit right with me.
impl builder::Module {
    fn structured_wat(&self) -> Result<String, Diagnostics> {
        self.to_wat()
            .map_err(|errors| self.structured_validation_diagnostics(errors))
    }

    fn structured_bytes(&self) -> Result<Vec<u8>, Diagnostics> {
        if !self.raw_wat_items.is_empty() {
            let wat = self.structured_wat()?;
            return wat::parse_str(&wat).map_err(|error| {
                vec![Diagnostic::new(
                    DiagnosticCode::WasmError,
                    format!("could not assemble structured WAT with runtime helpers: {error}"),
                )]
            });
        }
        self.to_wasm_bytes()
            .map_err(|errors| self.structured_validation_diagnostics(errors))
    }

    fn structured_validation_diagnostics(&self, errors: Vec<validator::ValidationError>) -> Diagnostics {
        errors
            .into_iter()
            .map(|error| {
                let diagnostic = Diagnostic::new(DiagnosticCode::WasmError, error.message);
                if let Some(span) = self.source_span {
                    diagnostic.with_label(Label::primary(span, "Wasm generated for this source"))
                } else {
                    diagnostic
                }
            })
            .collect()
    }
}

#[derive(Debug, Clone)]
pub struct RuntimeHelperFragment {
    pub name: String,
    wat: String,
    pub deps: HashSet<String>,
}

struct RuntimePrelude {
    wat: String,
    fragments: Vec<RuntimeHelperFragment>,
}

impl RuntimePrelude {
    fn helpers(&mut self, helper_roots: &HashSet<String>) {
        for index in self.required_helper_indices(helper_roots) {
            let wat = self.fragments[index].wat.clone();
            self.lines(&wat);
        }
    }

    fn required_helper_indices(&self, helper_roots: &HashSet<String>) -> Vec<usize> {
        let by_name = self
            .fragments
            .iter()
            .enumerate()
            .map(|(index, fragment)| (fragment.name.as_str(), index))
            .collect::<HashMap<_, _>>();
        let mut required = HashSet::new();
        let mut stack = helper_roots.iter().cloned().collect::<Vec<_>>();
        if helper_roots
            .iter()
            .any(|name| matches!(name.as_str(), "__alloc" | "__panic" | "__match_fail" | "__assert"))
        {
            stack.push("__last_panic".into());
        }
        while let Some(name) = stack.pop() {
            let Some(index) = by_name.get(name.as_str()).copied() else {
                continue;
            };
            if !required.insert(index) {
                continue;
            }
            stack.extend(self.fragments[index].deps.iter().cloned());
        }
        let mut indices = required.into_iter().collect::<Vec<_>>();
        indices.sort_unstable();
        indices
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

fn runtime_helper_roots(wat: &str) -> HashSet<String> {
    wat.lines()
        .filter_map(|line| line.trim().strip_prefix("call $__"))
        .filter_map(|rest| rest.split_whitespace().next())
        .map(|name| name.trim_end_matches(|char: char| !char.is_ascii_alphanumeric() && char != '_'))
        .map(|name| format!("__{name}"))
        .collect()
}

fn runtime_helper_wat(config: runtime::RuntimeConfig, helper_roots: &HashSet<String>) -> String {
    let mut prelude = RuntimePrelude { wat: String::new(), fragments: config.runtime_helper_fragments() };
    prelude.helpers(helper_roots);
    prelude.wat
}

pub fn runtime_helper_fragments_from_block(block: &str) -> Vec<RuntimeHelperFragment> {
    let lines = block.trim_matches('\n').lines().collect::<Vec<_>>();
    let mut fragments = Vec::new();
    let mut index = 0;
    while index < lines.len() {
        let line = lines[index];
        let Some(name) = runtime_helper_name(line).or_else(|| runtime_helper_data_name(line)) else {
            index += 1;
            continue;
        };
        let start = index;
        let mut depth = paren_delta(line);
        index += 1;
        while index < lines.len() && depth > 0 {
            depth += paren_delta(lines[index]);
            index += 1;
        }
        let wat = lines[start..index].join("\n");
        let deps = runtime_helper_roots(&wat)
            .into_iter()
            .filter(|dep| dep != &name)
            .collect();
        fragments.push(RuntimeHelperFragment { name, wat, deps });
    }
    fragments
}

fn runtime_helper_name(line: &str) -> Option<String> {
    let trimmed = line.trim_start();
    let rest = trimmed.strip_prefix("(func $")?;
    let name = rest.split([' ', ')']).next()?;
    name.starts_with("__").then(|| name.to_string())
}

fn runtime_helper_data_name(line: &str) -> Option<String> {
    line.trim_start()
        .starts_with("(data ")
        .then(|| "__float_to_string_dot_data".to_string())
}

fn paren_delta(line: &str) -> i32 {
    line.chars().fold(0, |depth, char| match char {
        '(' => depth + 1,
        ')' => depth - 1,
        _ => depth,
    })
}

fn constructor_tag(name: &str) -> u32 {
    name.bytes().fold(0x811c_9dc5, |hash, byte| {
        hash.wrapping_mul(0x0100_0193) ^ u32::from(byte)
    })
}
