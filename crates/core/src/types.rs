use crate::{diagnostic::Diagnostics, resolve::ResolvedModule};

/// The initial set of compiler types.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Type {
    Int,
    Float,
    String,
    Bool,
    Nil,
    Function { params: Vec<Type>, return_type: Box<Type> },
}

/// Resolved module annotated with type information.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TypedModule {
    pub resolved: ResolvedModule,
}

pub fn check(module: ResolvedModule) -> Result<TypedModule, Diagnostics> {
    Ok(TypedModule { resolved: module })
}
