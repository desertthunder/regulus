use super::FunctionContext;
use crate::ast::{self, LiteralKind};
use crate::{runtime, source::Span, types::Type};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BitArrayLiteral {
    pub segments: Vec<BitArraySegment>,
    pub bit_len: u32,
}

impl BitArrayLiteral {
    pub fn bytes(&self) -> Vec<u8> {
        let mut bytes = vec![0; runtime::bit_array_payload_len(self.bit_len) as usize];
        let mut offset = 0;
        for segment in &self.segments {
            for bit_index in 0..segment.bit_size {
                let source_shift = segment.bit_size - bit_index - 1;
                let bit = if source_shift < u64::BITS { (segment.value >> source_shift) & 1 } else { 0 };
                if bit == 1 {
                    let byte = &mut bytes[(offset / 8) as usize];
                    let target_shift = 7 - offset % 8;
                    *byte |= 1 << target_shift;
                }
                offset += 1;
            }
        }
        bytes
    }
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
    pub value: Option<u64>,
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

// TODO: this could be from/into
pub fn bit_array_literal(raw: &ast::RawSyntax) -> BitArrayLiteral {
    let segments = bit_array_segments(raw);
    let bit_len = segments.iter().map(|segment| segment.bit_size).sum();
    BitArrayLiteral { segments, bit_len }
}

// TODO: this could be from/into
pub fn ast_bit_array_literal(bit_array: &ast::BitArray) -> BitArrayLiteral {
    let segments = bit_array
        .segments
        .iter()
        .filter_map(|segment| {
            let value = match &segment.value {
                ast::Expression::Literal(literal) if literal.kind == LiteralKind::Int => {
                    literal.source.parse::<u64>().ok()?
                }
                _ => return None,
            };
            let option_source = segment
                .options
                .iter()
                .map(|option| option.source.as_str())
                .collect::<Vec<_>>()
                .join("-");
            let options = segment
                .options
                .iter()
                .filter_map(|option| bit_segment_options(&option.source).into_iter().next())
                .collect::<Vec<_>>();
            let bit_size = bit_size_from_options(&options).unwrap_or(8);
            Some(BitArraySegment {
                value,
                bit_size,
                type_: bit_segment_type(&option_source),
                options,
                span: segment.span,
            })
        })
        .collect::<Vec<_>>();
    let bit_len = segments.iter().map(|segment| segment.bit_size).sum();
    BitArrayLiteral { segments, bit_len }
}

pub fn bit_string_pattern_segments(ctx: &mut FunctionContext, raw: &ast::RawSyntax) -> Vec<BitStringPatternSegment> {
    let source = raw.source.trim();
    match source.strip_prefix("<<").and_then(|source| source.strip_suffix(">>")) {
        Some(inner) => inner
            .split(',')
            .map(|segment| bit_string_pattern_segment(ctx, segment.trim(), raw.span))
            .collect(),
        _ => Vec::new(),
    }
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
    let (value, option_source) = source.split_once(':').unwrap_or((source, ""));
    let value = value.trim().parse::<u64>().ok()?;
    let options = bit_segment_options(option_source);
    let bit_size = bit_size_from_options(&options).unwrap_or(8);
    Some(BitArraySegment { value, bit_size, type_: bit_segment_type(option_source), options, span })
}

fn bit_segment_type(source: &str) -> BitSegmentType {
    source
        .split('-')
        .find_map(|option| match option.trim() {
            "float" => Some(BitSegmentType::Float),
            "binary" | "bytes" | "bits" | "bit_string" => Some(BitSegmentType::Binary),
            "utf8" => Some(BitSegmentType::Utf8),
            "utf16" => Some(BitSegmentType::Utf16),
            "utf32" => Some(BitSegmentType::Utf32),
            _ => None,
        })
        .unwrap_or(BitSegmentType::Integer)
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

fn bit_size_from_options(options: &[BitSegmentOption]) -> Option<u32> {
    let size = options.iter().find_map(|option| match option {
        BitSegmentOption::Size(size) => Some(*size),
        _ => None,
    })?;
    let unit = options
        .iter()
        .find_map(|option| match option {
            BitSegmentOption::Unit(unit) => Some(*unit),
            _ => None,
        })
        .unwrap_or(1);
    size.checked_mul(unit)
}

fn bit_string_pattern_segment(context: &mut FunctionContext, source: &str, span: Span) -> BitStringPatternSegment {
    let (value, option_source) = source.split_once(':').unwrap_or((source, ""));
    let options = bit_segment_options(option_source);
    let type_ = bit_segment_type(option_source);
    let bit_size = bit_size_from_options(&options).or(match type_ {
        BitSegmentType::Binary => None,
        _ => Some(8),
    });
    let value = value.trim();
    let literal = value.parse::<u64>().ok();
    let binding = (literal.is_none() && value.chars().next().is_some_and(char::is_lowercase)).then(|| {
        let name = ast::Name { span, text: value.into() };
        let local = context.allocate(
            &name,
            match type_ {
                BitSegmentType::Binary => Type::BitArray,
                _ => Type::Int,
            },
        );
        context.bind(name.text, local.id);
        local.id
    });
    BitStringPatternSegment { value: literal, binding, bit_size, type_, options, span }
}
