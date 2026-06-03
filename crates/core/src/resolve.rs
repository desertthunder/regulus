use crate::{ast, diagnostic::Diagnostics};

/// AST after name resolution.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolvedModule {
    pub ast: ast::Module,
}

pub fn resolve(module: ast::Module) -> Result<ResolvedModule, Diagnostics> {
    Ok(ResolvedModule { ast: module })
}
