use crate::{diagnostic::Diagnostics, parse::ConcreteSyntaxTree, source::Span};

/// Compiler-owned AST module.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Module {
    pub span: Span,
}

pub fn build(cst: ConcreteSyntaxTree) -> Result<Module, Diagnostics> {
    Ok(Module { span: cst.source.whole_span() })
}
