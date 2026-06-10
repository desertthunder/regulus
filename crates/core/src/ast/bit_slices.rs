use tree_sitter::Node;

use crate::{diagnostic::Diagnostics, source::Span};

use super::{AstBuilder, Expression};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BitArray {
    pub span: Span,
    pub segments: Vec<BitArraySegment>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BitArraySegment {
    pub span: Span,
    pub value: Expression,
    pub options: Vec<BitArraySegmentOption>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BitArraySegmentOption {
    pub span: Span,
    pub value: Option<Expression>,
    pub source: String,
}

impl AstBuilder<'_> {
    pub fn bit_array(&self, node: Node<'_>) -> Result<BitArray, Diagnostics> {
        Ok(BitArray {
            span: self.span(node),
            segments: self
                .named_children(node)
                .into_iter()
                .filter(|child| child.kind() == "bit_string_segment")
                .map(|child| self.bit_array_segment(child))
                .collect::<Result<Vec<_>, _>>()?,
        })
    }

    fn bit_array_segment(&self, node: Node<'_>) -> Result<BitArraySegment, Diagnostics> {
        let value = node
            .child_by_field_name("value")
            .ok_or_else(|| vec![self.missing(node, "bit array segment value")])?;
        let options = self
            .named_children(node)
            .into_iter()
            .find(|child| child.kind() == "bit_string_segment_options")
            .map(|options| self.bit_array_segment_options(options))
            .transpose()?
            .unwrap_or_default();
        Ok(BitArraySegment { span: self.span(node), value: self.expression(value)?, options })
    }

    fn bit_array_segment_options(&self, node: Node<'_>) -> Result<Vec<BitArraySegmentOption>, Diagnostics> {
        self.named_children(node)
            .into_iter()
            .map(|child| {
                let value = self
                    .named_children(child)
                    .into_iter()
                    .next()
                    .map(|value| self.expression(value))
                    .transpose()?;
                Ok(BitArraySegmentOption { span: self.span(child), value, source: self.text(child).to_string() })
            })
            .collect()
    }
}
