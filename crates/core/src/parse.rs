use tree_sitter::{Parser, Tree};

use crate::{
    diagnostic::{Diagnostic, DiagnosticCode, Diagnostics, Label},
    source::{SourceFile, Span},
};

/// Tree-sitter concrete syntax tree for a source file.
#[derive(Debug)]
pub struct ConcreteSyntaxTree {
    pub source: SourceFile,
    pub tree: Tree,
}

/// Parse a Gleam source file and print its tree-sitter tree.
///
/// ```
/// use compiler_core::{parse, source::{SourceFile, SourceFileId}};
///
/// let source = SourceFile::new(
///     SourceFileId(0),
///     "import gleam/io\npub fn main() { io.println(\"hi\") }",
/// );
/// let cst = parse::parse(source).expect("parse Gleam source");
///
/// println!("{}", cst.tree.root_node().to_sexp());
/// ```
pub fn parse(source: SourceFile) -> Result<ConcreteSyntaxTree, Diagnostics> {
    let mut parser = Parser::new();
    parser
        .set_language(&tree_sitter_gleam::LANGUAGE.into())
        .map_err(|error| {
            vec![Diagnostic::new(
                DiagnosticCode::ParseError,
                format!("could not load Gleam grammar: {error}"),
            )]
        })?;

    let tree = parser.parse(&source.text, None).ok_or_else(|| {
        vec![Diagnostic::new(
            DiagnosticCode::ParseError,
            "tree-sitter could not parse the source file",
        )]
    })?;

    if tree.root_node().has_error() {
        let span = first_error_span(source.id, tree.root_node()).unwrap_or_else(|| source.whole_span());
        return Err(vec![
            Diagnostic::new(DiagnosticCode::ParseError, "Gleam syntax could not be parsed")
                .with_label(Label::primary(span, "parse error here")),
        ]);
    }

    Ok(ConcreteSyntaxTree { source, tree })
}

fn first_error_span(file_id: crate::source::SourceFileId, node: tree_sitter::Node<'_>) -> Option<Span> {
    if node.is_error() || node.is_missing() {
        return Some(Span::new(file_id, node.start_byte(), node.end_byte()));
    }

    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        if child.has_error() {
            return first_error_span(file_id, child);
        }
    }

    None
}
