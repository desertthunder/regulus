use crate::{diagnostic::Diagnostics, ir};

/// WebAssembly output from the backend.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WasmModule {
    pub bytes: Vec<u8>,
}

pub fn emit(_module: ir::Module) -> Result<WasmModule, Diagnostics> {
    Ok(WasmModule { bytes: Vec::new() })
}
