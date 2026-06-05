use crate::{
    ast::{self, Expression as AstExpression, LiteralKind},
    source::Span,
    types::Type,
};

use super::FunctionContext;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BitArrayLiteral {
    pub segments: Vec<BitArraySegment>,
    pub bit_len: u32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BitArraySegment {
    pub value: u64,
    pub bit_size: u32,
    pub type_: BitSegmentType,
    pub options: Vec<BitSegmentOption>,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BitStringPatternSegment {
    pub binding: Option<super::LocalId>,
    pub bit_size: Option<u32>,
    pub type_: BitSegmentType,
    pub options: Vec<BitSegmentOption>,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BitSegmentType {
    Integer,
    Float,
    Binary,
    Utf8,
    Utf16,
    Utf32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BitSegmentOption {
    Size(u32),
    Unit(u32),
    Signed,
    Unsigned,
    BigEndian,
    LittleEndian,
    NativeEndian,
}

pub(super) fn bit_array_literal(raw: &ast::RawSyntax) -> BitArrayLiteral {
    let segments = bit_array_segments(raw);
    let bit_len = segments.iter().map(|segment| segment.bit_size).sum();
    BitArrayLiteral { segments, bit_len }
}

pub(super) fn ast_bit_array_literal(bit_array: &ast::BitArray) -> BitArrayLiteral {
    let segments = bit_array
        .segments
        .iter()
        .filter_map(|segment| {
            let value = match &segment.value {
                AstExpression::Literal(literal) if literal.kind == LiteralKind::Int => {
                    literal.source.parse::<u64>().ok()?
                }
                _ => return None,
            };
            let options = segment
                .options
                .iter()
                .filter_map(|option| bit_segment_options(&option.source).into_iter().next())
                .collect::<Vec<_>>();
            let bit_size = options
                .iter()
                .find_map(|option| match option {
                    BitSegmentOption::Size(size) => Some(*size),
                    _ => None,
                })
                .unwrap_or(8);
            Some(BitArraySegment { value, bit_size, type_: BitSegmentType::Integer, options, span: segment.span })
        })
        .collect::<Vec<_>>();
    let bit_len = segments.iter().map(|segment| segment.bit_size).sum();
    BitArrayLiteral { segments, bit_len }
}

fn bit_array_segments(raw: &ast::RawSyntax) -> Vec<BitArraySegment> {
    let Some(inner) = raw
        .source
        .trim()
        .strip_prefix("<<")
        .and_then(|source| source.strip_suffix(">>"))
    else {
        return Vec::new();
    };
    inner
        .split(',')
        .filter_map(|segment| bit_array_segment(segment.trim(), raw.span))
        .collect()
}

fn bit_array_segment(source: &str, span: Span) -> Option<BitArraySegment> {
    if source.is_empty() {
        return None;
    }
    let (value, options) = source.split_once(':').unwrap_or((source, ""));
    let value = value.trim().parse::<u64>().ok()?;
    let options = bit_segment_options(options);
    let bit_size = options
        .iter()
        .find_map(|option| match option {
            BitSegmentOption::Size(size) => Some(*size),
            _ => None,
        })
        .unwrap_or(8);
    Some(BitArraySegment { value, bit_size, type_: BitSegmentType::Integer, options, span })
}

fn bit_segment_options(source: &str) -> Vec<BitSegmentOption> {
    source
        .split('-')
        .filter_map(|option| {
            let option = option.trim();
            if option.is_empty() {
                return None;
            }
            match option {
                "signed" => Some(BitSegmentOption::Signed),
                "unsigned" => Some(BitSegmentOption::Unsigned),
                "big" => Some(BitSegmentOption::BigEndian),
                "little" => Some(BitSegmentOption::LittleEndian),
                "native" => Some(BitSegmentOption::NativeEndian),
                "float" => None,
                "binary" | "bytes" | "bits" | "bit_string" | "utf8" | "utf16" | "utf32" => None,
                _ if let Some(size) = option.strip_prefix("size(").and_then(|value| value.strip_suffix(')')) => {
                    size.parse().ok().map(BitSegmentOption::Size)
                }
                _ if let Some(unit) = option.strip_prefix("unit(").and_then(|value| value.strip_suffix(')')) => {
                    unit.parse().ok().map(BitSegmentOption::Unit)
                }
                _ => option.parse().ok().map(BitSegmentOption::Size),
            }
        })
        .collect()
}

pub(super) fn bit_string_pattern_segments(
    context: &mut FunctionContext, raw: &ast::RawSyntax,
) -> Vec<BitStringPatternSegment> {
    let Some(inner) = raw
        .source
        .trim()
        .strip_prefix("<<")
        .and_then(|source| source.strip_suffix(">>"))
    else {
        return Vec::new();
    };
    inner
        .split(',')
        .map(|segment| bit_string_pattern_segment(context, segment.trim(), raw.span))
        .collect()
}

fn bit_string_pattern_segment(context: &mut FunctionContext, source: &str, span: Span) -> BitStringPatternSegment {
    let (value, options) = source.split_once(':').unwrap_or((source, ""));
    let options = bit_segment_options(options);
    let bit_size = options.iter().find_map(|option| match option {
        BitSegmentOption::Size(size) => Some(*size),
        _ => None,
    });
    let binding = value.trim().chars().next().filter(|char| char.is_lowercase()).map(|_| {
        let name = ast::Name { span, text: value.trim().into() };
        let local = context.allocate(&name, Type::Int);
        context.bind(name.text, local.id);
        local.id
    });
    BitStringPatternSegment { binding, bit_size, type_: BitSegmentType::Integer, options, span }
}
