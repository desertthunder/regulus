pub mod ast;
pub mod diagnostic;
pub mod inference;
pub mod ir;
pub mod parse;
pub mod project;
pub mod resolve;
pub mod runtime;
pub mod source;
pub mod types;
pub mod wasm;

use diagnostic::Diagnostics;
use source::{SourceFile, SourceFileId};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ClosureConstants {
    CaptureSlotSize,
    FunctionIdOffset,
    CapturesOffset,
}

impl From<ClosureConstants> for u32 {
    fn from(value: ClosureConstants) -> Self {
        match value {
            ClosureConstants::CaptureSlotSize => 8,
            ClosureConstants::FunctionIdOffset => 8,
            ClosureConstants::CapturesOffset => 12,
        }
    }
}

/// Output from a full compile pipeline run.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CompileOutput {
    pub wasm: wasm::WasmModule,
}

/// Compile a Gleam source string.
pub fn compile(source: impl Into<String>) -> Result<CompileOutput, Diagnostics> {
    let source = SourceFile::new(SourceFileId(0), source);
    compile_source(source)
}

/// Compile an already-created source file.
pub fn compile_source(source: SourceFile) -> Result<CompileOutput, Diagnostics> {
    let cst = parse::parse(source)?;
    let ast = ast::build(&cst)?;
    let resolved = resolve::resolve(ast)?;
    let typed = types::check(resolved)?;
    let ir = ir::lower(typed)?;
    let wasm = wasm::emit(&ir)?;

    Ok(CompileOutput { wasm })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn compiles_nil_function_to_wasm() {
        let output = compile("pub fn main() { Nil }").expect("pipeline should run");
        assert!(!output.wasm.bytes.is_empty());
    }

    #[test]
    fn compiles_add_function_end_to_end() {
        let output = compile("pub fn add(a, b) { a + b }").expect("compile add function");
        assert!(!output.wasm.bytes.is_empty());
    }
}
