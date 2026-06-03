use crate::{diagnostic::Diagnostics, source::SourceFile};

/// Tree-sitter concrete syntax tree for a source file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConcreteSyntaxTree {
    pub source: SourceFile,
}

pub fn parse(source: SourceFile) -> Result<ConcreteSyntaxTree, Diagnostics> {
    Ok(ConcreteSyntaxTree { source })
}
