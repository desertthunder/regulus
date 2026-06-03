use crate::{diagnostic::Diagnostics, types::TypedModule};

/// Core IR module.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Module {
    pub typed: TypedModule,
}

pub fn lower(module: TypedModule) -> Result<Module, Diagnostics> {
    Ok(Module { typed: module })
}
