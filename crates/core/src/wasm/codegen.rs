//! Incremental IR-to-structured-Wasm code generation.

mod emitter;
mod helpers;

use super::builder::{FunctionType, LocalId, Module, TypeId, ValueType};
use super::{EmitOptions, WasmTarget};
use crate::diagnostic::Diagnostics;
use crate::ir;
use emitter::*;

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

pub fn emit(module: &ir::Module, options: EmitOptions) -> Result<Module, Diagnostics> {
    let emitter = StructuredEmitter::new(module, options);
    match emitter.module(module) {
        Ok(module) => Ok(module),
        Err(StructuredError::Unsupported) => Err(helpers::unsupported_structured_diagnostics(module)),
        Err(StructuredError::Invariant(message)) => Err(helpers::invariant_diagnostics(module, &message)),
        Err(StructuredError::Diagnostics(diagnostics)) => Err(diagnostics),
    }
}
