use crate::ClosureConstants;
use crate::wasm::{RuntimeHelperFragment, fragments, runtime_helper_fragments_from_block};

pub const WASM_PAGE_SIZE: u32 = 65_536;
pub const DEFAULT_MEMORY_MAX_PAGES: u32 = 256;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Layout {
    pub word_size: u32,
    pub alignment: u32,
    pub header_size: u32,
}

impl Layout {
    pub const DEFAULT: Self = Self { word_size: 4, alignment: 8, header_size: 8 };

    pub fn align_to(self, value: u32) -> u32 {
        value.next_multiple_of(self.alignment)
    }

    pub fn string_size(self, byte_len: u32) -> u32 {
        self.header_size + self.align_to(byte_len)
    }

    pub fn bit_array_size(self, bit_len: u32) -> u32 {
        self.header_size + self.align_to(bit_array_payload_len(bit_len))
    }

    pub fn list_cons_size(self, value_size: u32) -> u32 {
        self.align_to(self.header_size + value_size + self.word_size)
    }

    pub fn tuple_size(self, arity: u32, value_size: u32) -> u32 {
        self.align_to(self.header_size + arity * value_size)
    }

    pub fn record_size(self, field_count: u32, value_size: u32) -> u32 {
        self.tuple_size(field_count, value_size)
    }

    pub fn custom_size(self, field_count: u32, value_size: u32) -> u32 {
        self.align_to(self.header_size + self.word_size + field_count * value_size)
    }

    pub fn closure_size(self, capture_count: u32) -> u32 {
        self.align_to(
            self.header_size + self.word_size + capture_count * u32::from(crate::ClosureConstants::CaptureSlotSize),
        )
    }

    pub fn opaque_size(self) -> u32 {
        self.align_to(self.header_size + 2 * self.word_size)
    }

    pub fn error_size(self, field_count: u32, value_size: u32) -> u32 {
        self.custom_size(field_count, value_size)
    }

    pub fn panic_size(self, field_count: u32, value_size: u32) -> u32 {
        self.custom_size(field_count, value_size)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RuntimeConfig {
    pub layout: Layout,
    pub static_data_start: u32,
    pub heap_start: u32,
    pub memory_max_pages: u32,
}

impl RuntimeConfig {
    pub const DEFAULT: Self = Self {
        layout: Layout::DEFAULT,
        static_data_start: 1024,
        heap_start: 4096,
        memory_max_pages: DEFAULT_MEMORY_MAX_PAGES,
    };

    pub fn memory_limit_bytes(self) -> u32 {
        self.memory_max_pages
            .checked_mul(WASM_PAGE_SIZE)
            .expect("runtime memory limit must fit in wasm32 address space")
    }

    pub fn runtime_helper_fragments(self) -> Vec<RuntimeHelperFragment> {
        let alloc_helper = fragments::allocation::ALLOC_HELPER
            .replace("{alignment_mask}", &(self.layout.alignment - 1).to_string())
            .replace("{alignment}", &self.layout.alignment.to_string())
            .replace("{heap_limit}", &self.memory_limit_bytes().to_string())
            .replace("{allocation_failure_offset}", "64");
        let managed_value_helpers = fragments::managed_values::MANAGED_VALUE_HELPERS
            .replace(
                "{closure_capture_slot_size}",
                &u32::from(ClosureConstants::CaptureSlotSize).to_string(),
            )
            .replace(
                "{closure_function_id_offset}",
                &u32::from(ClosureConstants::FunctionIdOffset).to_string(),
            )
            .replace(
                "{closure_captures_offset}",
                &u32::from(ClosureConstants::CapturesOffset).to_string(),
            );
        let blocks = [
            alloc_helper.as_str(),
            fragments::panic::PANIC_HELPERS,
            fragments::copy::COPY_HELPERS,
            fragments::strings::STRING_HELPERS,
            fragments::bit_arrays::BIT_ARRAY_HELPERS,
            fragments::lists::LIST_HELPERS,
            managed_value_helpers.as_str(),
            fragments::dictionaries::DICTIONARY_HELPERS,
            fragments::dynamic::DYNAMIC_HELPERS,
            fragments::equality_ordering::EQUALITY_AND_ORDERING_HELPERS,
            fragments::debug::DEBUG_HELPERS,
            fragments::host_adapters::HOST_ADAPTER_HELPERS,
        ];
        let mut fragments = blocks
            .into_iter()
            .flat_map(runtime_helper_fragments_from_block)
            .collect::<Vec<_>>();
        if let Some(fragment) = fragments
            .iter_mut()
            .find(|fragment| fragment.name == "__float_to_string")
        {
            fragment.deps.insert("__float_to_string_dot_data".into());
        }
        for name in ["__alloc", "__allocation_fail", "__panic", "__match_fail", "__assert"] {
            if let Some(fragment) = fragments.iter_mut().find(|fragment| fragment.name == name) {
                fragment.deps.insert("__last_panic".into());
            }
        }
        fragments
    }
}

impl Default for RuntimeConfig {
    fn default() -> Self {
        Self::DEFAULT
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ObjectTag {
    String,
    ListCons,
    Tuple,
    Record,
    Custom,
    Closure,
    BitArray,
    Opaque,
    Error,
    Panic,
}

impl From<ObjectTag> for u32 {
    fn from(tag: ObjectTag) -> Self {
        match tag {
            ObjectTag::String => 1,
            ObjectTag::ListCons => 2,
            ObjectTag::Tuple => 3,
            ObjectTag::Record => 4,
            ObjectTag::Custom => 5,
            ObjectTag::Closure => 6,
            ObjectTag::BitArray => 7,
            ObjectTag::Opaque => 8,
            ObjectTag::Error => 9,
            ObjectTag::Panic => 10,
        }
    }
}

impl TryFrom<u32> for ObjectTag {
    type Error = ();

    fn try_from(tag: u32) -> Result<Self, <Self as TryFrom<u32>>::Error> {
        match tag {
            1 => Ok(ObjectTag::String),
            2 => Ok(ObjectTag::ListCons),
            3 => Ok(ObjectTag::Tuple),
            4 => Ok(ObjectTag::Record),
            5 => Ok(ObjectTag::Custom),
            6 => Ok(ObjectTag::Closure),
            7 => Ok(ObjectTag::BitArray),
            8 => Ok(ObjectTag::Opaque),
            9 => Ok(ObjectTag::Error),
            10 => Ok(ObjectTag::Panic),
            _ => Err(()),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ObjectHeader {
    pub tag: ObjectTag,
    pub size: u32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StaticObject {
    pub offset: u32,
    pub bytes: Vec<u8>,
}

pub fn string_object(config: RuntimeConfig, offset: u32, string: &str) -> StaticObject {
    let data = string.as_bytes();
    let size = config.layout.string_size(data.len() as u32);
    let mut bytes = Vec::with_capacity(size as usize);
    bytes.extend_from_slice(&u32::from(ObjectTag::String).to_le_bytes());
    bytes.extend_from_slice(&(data.len() as u32).to_le_bytes());
    bytes.extend_from_slice(data);
    bytes.resize(size as usize, 0);
    StaticObject { offset, bytes }
}

pub fn list_cons_object(config: RuntimeConfig, offset: u32, head: u64, tail: u32) -> StaticObject {
    let size = config.layout.list_cons_size(8);
    let mut bytes = Vec::with_capacity(size as usize);
    bytes.extend_from_slice(&u32::from(ObjectTag::ListCons).to_le_bytes());
    bytes.extend_from_slice(&2u32.to_le_bytes());
    bytes.extend_from_slice(&head.to_le_bytes());
    bytes.extend_from_slice(&tail.to_le_bytes());
    bytes.resize(size as usize, 0);
    StaticObject { offset, bytes }
}

pub fn tuple_object(config: RuntimeConfig, offset: u32, fields: &[u64]) -> StaticObject {
    field_array_object(config, offset, ObjectTag::Tuple, fields)
}

pub fn record_object(config: RuntimeConfig, offset: u32, fields: &[u64]) -> StaticObject {
    field_array_object(config, offset, ObjectTag::Record, fields)
}

pub fn custom_object(config: RuntimeConfig, offset: u32, constructor_tag: u32, fields: &[u64]) -> StaticObject {
    let size = config.layout.custom_size(fields.len() as u32, 8);
    let mut bytes = Vec::with_capacity(size as usize);
    bytes.extend_from_slice(&u32::from(ObjectTag::Custom).to_le_bytes());
    bytes.extend_from_slice(&(fields.len() as u32).to_le_bytes());
    bytes.extend_from_slice(&constructor_tag.to_le_bytes());
    for field in fields {
        bytes.extend_from_slice(&field.to_le_bytes());
    }
    bytes.resize(size as usize, 0);
    StaticObject { offset, bytes }
}

pub fn closure_object(config: RuntimeConfig, offset: u32, function_id: u32, captures: &[u64]) -> StaticObject {
    let size = config.layout.closure_size(captures.len() as u32);
    let mut bytes = Vec::with_capacity(size as usize);
    bytes.extend_from_slice(&u32::from(ObjectTag::Closure).to_le_bytes());
    bytes.extend_from_slice(&(captures.len() as u32).to_le_bytes());
    bytes.extend_from_slice(&function_id.to_le_bytes());
    for capture in captures {
        bytes.extend_from_slice(&capture.to_le_bytes());
    }
    bytes.resize(size as usize, 0);
    StaticObject { offset, bytes }
}

pub fn bit_array_object(config: RuntimeConfig, offset: u32, data: &[u8], bit_len: u32) -> StaticObject {
    assert!(bit_len <= data.len() as u32 * 8, "bit length exceeds payload bytes");
    let size = config.layout.bit_array_size(bit_len);
    let payload_len = bit_array_payload_len(bit_len) as usize;
    let mut bytes = Vec::with_capacity(size as usize);
    bytes.extend_from_slice(&u32::from(ObjectTag::BitArray).to_le_bytes());
    bytes.extend_from_slice(&bit_len.to_le_bytes());
    bytes.extend_from_slice(&data[..payload_len]);
    bytes.resize(size as usize, 0);
    StaticObject { offset, bytes }
}

pub fn opaque_object(config: RuntimeConfig, offset: u32, type_tag: u32, payload: u32) -> StaticObject {
    let size = config.layout.opaque_size();
    let mut bytes = Vec::with_capacity(size as usize);
    bytes.extend_from_slice(&u32::from(ObjectTag::Opaque).to_le_bytes());
    bytes.extend_from_slice(&0u32.to_le_bytes());
    bytes.extend_from_slice(&type_tag.to_le_bytes());
    bytes.extend_from_slice(&payload.to_le_bytes());
    bytes.resize(size as usize, 0);
    StaticObject { offset, bytes }
}

pub fn error_object(config: RuntimeConfig, offset: u32, reason_tag: u32, fields: &[u64]) -> StaticObject {
    tagged_payload_object(
        config,
        offset,
        ObjectTag::Error,
        reason_tag,
        fields,
        config.layout.error_size(fields.len() as u32, 8),
    )
}

pub fn panic_object(config: RuntimeConfig, offset: u32, reason_tag: u32, fields: &[u64]) -> StaticObject {
    tagged_payload_object(
        config,
        offset,
        ObjectTag::Panic,
        reason_tag,
        fields,
        config.layout.panic_size(fields.len() as u32, 8),
    )
}

pub fn bit_array_payload_len(bit_len: u32) -> u32 {
    bit_len.div_ceil(8)
}

pub fn debug_render(memory: &[u8], pointer: u32) -> Option<String> {
    debug_render_at(memory, pointer as usize, 0)
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DebugPayload {
    pub reason: u32,
    pub fields: Vec<String>,
}

impl DebugPayload {
    pub fn panic(memory: &[u8], pointer: u32) -> Option<DebugPayload> {
        Self::new(memory, pointer, ObjectTag::Panic)
    }

    pub fn error(memory: &[u8], pointer: u32) -> Option<DebugPayload> {
        Self::new(memory, pointer, ObjectTag::Error)
    }

    fn new(memory: &[u8], pointer: u32, expected: ObjectTag) -> Option<DebugPayload> {
        let offset = pointer as usize;
        let header = read_header(memory, offset)?;
        if header.tag != expected {
            return None;
        }
        let reason = read_u32(memory, offset + 8)?;
        let fields = (0..header.size)
            .map(|index| render_slot(memory, read_u64(memory, offset + 12 + index as usize * 8)?, 1))
            .collect::<Option<Vec<_>>>()?;
        Some(DebugPayload { reason, fields })
    }
}

fn debug_render_at(memory: &[u8], offset: usize, depth: usize) -> Option<String> {
    if offset == 0 {
        return Some("[]".into());
    }
    if depth > 16 {
        return Some("...".into());
    }
    let header = read_header(memory, offset)?;
    match header.tag {
        ObjectTag::String => {
            let start = offset + 8;
            let end = start.checked_add(header.size as usize)?;
            let bytes = memory.get(start..end)?;
            Some(format!("\"{}\"", escape_debug_string(bytes)))
        }
        ObjectTag::ListCons => {
            let head = render_slot(memory, read_u64(memory, offset + 8)?, depth + 1)?;
            let tail = read_u32(memory, offset + 16)?;
            Some(format!(
                "[{head} | {}]",
                debug_render_at(memory, tail as usize, depth + 1)?
            ))
        }
        ObjectTag::Tuple => render_fields(memory, offset + 8, header.size, "#(", ")", depth),
        ObjectTag::Record => render_fields(memory, offset + 8, header.size, "Record(", ")", depth),
        ObjectTag::Custom => {
            let constructor = read_u32(memory, offset + 8)?;
            let fields = render_fields(memory, offset + 12, header.size, "(", ")", depth)?;
            Some(format!("Custom#{constructor}{fields}"))
        }
        ObjectTag::Closure => Some(format!("<closure:{} captures>", header.size)),
        ObjectTag::BitArray => {
            let bytes = bit_array_payload_len(header.size) as usize;
            let data = memory.get(offset + 8..offset + 8 + bytes)?;
            Some(format!("<<{}:{}>>", hex_bytes(data), header.size))
        }
        ObjectTag::Opaque => Some(format!("<opaque:{}>", read_u32(memory, offset + 8)?)),
        ObjectTag::Error => render_payload_object(memory, offset, header.size, "Error", depth),
        ObjectTag::Panic => render_payload_object(memory, offset, header.size, "Panic", depth),
    }
}

fn render_payload_object(memory: &[u8], offset: usize, fields: u32, name: &str, depth: usize) -> Option<String> {
    let reason = read_u32(memory, offset + 8)?;
    let fields = render_fields(memory, offset + 12, fields, "(", ")", depth)?;
    Some(format!("{name}#{reason}{fields}"))
}

fn render_fields(memory: &[u8], start: usize, count: u32, prefix: &str, suffix: &str, depth: usize) -> Option<String> {
    let fields = (0..count)
        .map(|index| render_slot(memory, read_u64(memory, start + index as usize * 8)?, depth + 1))
        .collect::<Option<Vec<_>>>()?;
    Some(format!("{prefix}{}{suffix}", fields.join(", ")))
}

fn render_slot(memory: &[u8], value: u64, depth: usize) -> Option<String> {
    let pointer = value as usize;
    if pointer != 0 && read_header(memory, pointer).is_some() {
        debug_render_at(memory, pointer, depth)
    } else {
        Some(value.to_string())
    }
}

fn read_header(memory: &[u8], offset: usize) -> Option<ObjectHeader> {
    let tag = ObjectTag::try_from(read_u32(memory, offset)?).ok()?;
    let size = read_u32(memory, offset + 4)?;
    Some(ObjectHeader { tag, size })
}

fn read_u32(memory: &[u8], offset: usize) -> Option<u32> {
    Some(u32::from_le_bytes(memory.get(offset..offset + 4)?.try_into().ok()?))
}

fn read_u64(memory: &[u8], offset: usize) -> Option<u64> {
    Some(u64::from_le_bytes(memory.get(offset..offset + 8)?.try_into().ok()?))
}

fn escape_debug_string(bytes: &[u8]) -> String {
    String::from_utf8_lossy(bytes)
        .chars()
        .flat_map(char::escape_default)
        .collect()
}

fn hex_bytes(bytes: &[u8]) -> String {
    bytes
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect::<Vec<_>>()
        .join(" ")
}

pub fn bit_array_get_bit(data: &[u8], bit_len: u32, index: u32) -> Option<u8> {
    if index >= bit_len {
        return None;
    }
    let byte = data.get((index / 8) as usize)?;
    let shift = 7 - index % 8;
    Some((byte >> shift) & 1)
}

pub fn bit_array_slice(data: &[u8], source_bit_len: u32, start: u32, bit_len: u32) -> Option<Vec<u8>> {
    if start.checked_add(bit_len)? > source_bit_len {
        return None;
    }
    let mut output = vec![0; bit_array_payload_len(bit_len) as usize];
    for offset in 0..bit_len {
        let bit = bit_array_get_bit(data, source_bit_len, start + offset)?;
        bit_array_set_bit(&mut output, offset, bit);
    }
    Some(output)
}

pub fn bit_array_append(left: &[u8], left_bit_len: u32, right: &[u8], right_bit_len: u32) -> Vec<u8> {
    let bit_len = left_bit_len + right_bit_len;
    let mut output = vec![0; bit_array_payload_len(bit_len) as usize];
    for index in 0..left_bit_len {
        let bit = bit_array_get_bit(left, left_bit_len, index).expect("left bit in range");
        bit_array_set_bit(&mut output, index, bit);
    }
    for index in 0..right_bit_len {
        let bit = bit_array_get_bit(right, right_bit_len, index).expect("right bit in range");
        bit_array_set_bit(&mut output, left_bit_len + index, bit);
    }
    output
}

fn field_array_object(config: RuntimeConfig, offset: u32, tag: ObjectTag, fields: &[u64]) -> StaticObject {
    let size = config.layout.tuple_size(fields.len() as u32, 8);
    let mut bytes = Vec::with_capacity(size as usize);
    bytes.extend_from_slice(&u32::from(tag).to_le_bytes());
    bytes.extend_from_slice(&(fields.len() as u32).to_le_bytes());
    for field in fields {
        bytes.extend_from_slice(&field.to_le_bytes());
    }
    bytes.resize(size as usize, 0);
    StaticObject { offset, bytes }
}

fn tagged_payload_object(
    _config: RuntimeConfig, offset: u32, tag: ObjectTag, payload_tag: u32, fields: &[u64], size: u32,
) -> StaticObject {
    let mut bytes = Vec::with_capacity(size as usize);
    bytes.extend_from_slice(&u32::from(tag).to_le_bytes());
    bytes.extend_from_slice(&(fields.len() as u32).to_le_bytes());
    bytes.extend_from_slice(&payload_tag.to_le_bytes());
    for field in fields {
        bytes.extend_from_slice(&field.to_le_bytes());
    }
    bytes.resize(size as usize, 0);
    StaticObject { offset, bytes }
}

fn bit_array_set_bit(data: &mut [u8], index: u32, bit: u8) {
    if bit & 1 == 0 {
        return;
    }
    let byte = &mut data[(index / 8) as usize];
    let shift = 7 - index % 8;
    *byte |= 1 << shift;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn converts_object_tags_to_and_from_u32() {
        assert_eq!(u32::from(ObjectTag::String), 1);
        assert_eq!(u32::from(ObjectTag::BitArray), 7);
        assert_eq!(u32::from(ObjectTag::Panic), 10);
        assert_eq!(ObjectTag::try_from(6), Ok(ObjectTag::Closure));
        assert_eq!(ObjectTag::try_from(7), Ok(ObjectTag::BitArray));
        assert_eq!(ObjectTag::try_from(8), Ok(ObjectTag::Opaque));
        assert_eq!(ObjectTag::try_from(10), Ok(ObjectTag::Panic));
        assert_eq!(ObjectTag::try_from(99), Err(()));
    }

    #[test]
    fn encodes_static_strings_with_header_and_padding() {
        let object = string_object(
            RuntimeConfig::DEFAULT,
            RuntimeConfig::DEFAULT.static_data_start,
            "hello",
        );

        assert_eq!(object.offset, 1024);
        assert_eq!(
            ObjectTag::try_from(u32::from_le_bytes(object.bytes[0..4].try_into().unwrap())),
            Ok(ObjectTag::String)
        );
        assert_eq!(u32::from_le_bytes(object.bytes[4..8].try_into().unwrap()), 5);
        assert_eq!(&object.bytes[8..13], b"hello");
        assert_eq!(object.bytes.len() as u32, 16);
    }

    #[test]
    fn encodes_static_managed_values_with_header_and_padding() {
        let config = RuntimeConfig::DEFAULT;

        let list = list_cons_object(config, 1024, 42, 0);
        assert_header(&list, ObjectTag::ListCons, 2);
        assert_eq!(u64::from_le_bytes(list.bytes[8..16].try_into().unwrap()), 42);
        assert_eq!(u32::from_le_bytes(list.bytes[16..20].try_into().unwrap()), 0);
        assert_eq!(list.bytes.len() as u32, config.layout.list_cons_size(8));

        let tuple = tuple_object(config, 1024, &[1, 2]);
        assert_header(&tuple, ObjectTag::Tuple, 2);
        assert_eq!(u64::from_le_bytes(tuple.bytes[8..16].try_into().unwrap()), 1);
        assert_eq!(u64::from_le_bytes(tuple.bytes[16..24].try_into().unwrap()), 2);

        let record = record_object(config, 1024, &[3, 4]);
        assert_header(&record, ObjectTag::Record, 2);
        assert_eq!(u64::from_le_bytes(record.bytes[8..16].try_into().unwrap()), 3);

        let custom = custom_object(config, 1024, 7, &[5]);
        assert_header(&custom, ObjectTag::Custom, 1);
        assert_eq!(u32::from_le_bytes(custom.bytes[8..12].try_into().unwrap()), 7);
        assert_eq!(u64::from_le_bytes(custom.bytes[12..20].try_into().unwrap()), 5);

        let option = custom_object(config, 1024, 8, &[]);
        assert_header(&option, ObjectTag::Custom, 0);
        assert_eq!(u32::from_le_bytes(option.bytes[8..12].try_into().unwrap()), 8);

        let result = custom_object(config, 1024, 9, &[13]);
        assert_header(&result, ObjectTag::Custom, 1);
        assert_eq!(u64::from_le_bytes(result.bytes[12..20].try_into().unwrap()), 13);

        let closure = closure_object(config, 1024, 9, &[11]);
        assert_header(&closure, ObjectTag::Closure, 1);
        assert_eq!(u32::from_le_bytes(closure.bytes[8..12].try_into().unwrap()), 9);
        assert_eq!(u32::from_le_bytes(closure.bytes[12..16].try_into().unwrap()), 11);
    }

    #[test]
    fn encodes_static_bit_arrays_with_header_and_padding() {
        let object = bit_array_object(
            RuntimeConfig::DEFAULT,
            RuntimeConfig::DEFAULT.static_data_start,
            &[0b1010_0101, 0b1100_0000],
            10,
        );

        assert_eq!(object.offset, 1024);
        assert_eq!(
            ObjectTag::try_from(u32::from_le_bytes(object.bytes[0..4].try_into().unwrap())),
            Ok(ObjectTag::BitArray)
        );
        assert_eq!(u32::from_le_bytes(object.bytes[4..8].try_into().unwrap()), 10);
        assert_eq!(&object.bytes[8..10], &[0b1010_0101, 0b1100_0000]);
        assert_eq!(object.bytes.len() as u32, 16);
    }

    #[test]
    fn slices_and_appends_bit_arrays() {
        let data = [0b1010_0101, 0b1100_0000];

        assert_eq!(bit_array_get_bit(&data, 10, 0), Some(1));
        assert_eq!(bit_array_get_bit(&data, 10, 1), Some(0));
        assert_eq!(bit_array_get_bit(&data, 10, 10), None);
        assert_eq!(bit_array_slice(&data, 10, 2, 5), Some(vec![0b1001_0000]));
        assert_eq!(
            bit_array_append(&[0b1010_0000], 4, &[0b1100_0000], 4),
            vec![0b1010_1100]
        );
    }

    #[test]
    fn renders_debug_strings_for_nested_runtime_objects() {
        let config = RuntimeConfig::DEFAULT;
        let string = string_object(config, 1024, "Ada\n");
        let custom = custom_object(config, 1040, 7, &[1024, 42]);
        let panic = panic_object(config, 1072, 3, &[1040]);
        let bit_array = bit_array_object(config, 1096, &[0b1010_0000], 4);
        let mut memory = vec![0; 2048];
        memory[1024..1024 + string.bytes.len()].copy_from_slice(&string.bytes);
        memory[1040..1040 + custom.bytes.len()].copy_from_slice(&custom.bytes);
        memory[1072..1072 + panic.bytes.len()].copy_from_slice(&panic.bytes);
        memory[1096..1096 + bit_array.bytes.len()].copy_from_slice(&bit_array.bytes);

        assert_eq!(debug_render(&memory, 1024), Some("\"Ada\\n\"".into()));
        assert_eq!(debug_render(&memory, 1040), Some("Custom#7(\"Ada\\n\", 42)".into()));
        assert_eq!(
            debug_render(&memory, 1072),
            Some("Panic#3(Custom#7(\"Ada\\n\", 42))".into())
        );
        assert_eq!(debug_render(&memory, 1096), Some("<<a0:4>>".into()));
    }

    #[test]
    fn exposes_debug_payloads_for_errors_and_panics() {
        let config = RuntimeConfig::DEFAULT;
        let string = string_object(config, 1024, "bad");
        let error = error_object(config, 1040, 5, &[1024, 7]);
        let panic = panic_object(config, 1072, 9, &[1040]);
        let mut memory = vec![0; 2048];
        memory[1024..1024 + string.bytes.len()].copy_from_slice(&string.bytes);
        memory[1040..1040 + error.bytes.len()].copy_from_slice(&error.bytes);
        memory[1072..1072 + panic.bytes.len()].copy_from_slice(&panic.bytes);

        assert_eq!(
            DebugPayload::error(&memory, 1040),
            Some(DebugPayload { reason: 5, fields: vec!["\"bad\"".into(), "7".into()] })
        );
        assert_eq!(
            DebugPayload::panic(&memory, 1072),
            Some(DebugPayload { reason: 9, fields: vec!["Error#5(\"bad\", 7)".into()] })
        );
    }

    #[test]
    fn encodes_runtime_opaque_error_and_panic_values() {
        let config = RuntimeConfig::DEFAULT;

        let opaque = opaque_object(config, 1024, 7, 2048);
        assert_header(&opaque, ObjectTag::Opaque, 0);
        assert_eq!(u32::from_le_bytes(opaque.bytes[8..12].try_into().unwrap()), 7);
        assert_eq!(u32::from_le_bytes(opaque.bytes[12..16].try_into().unwrap()), 2048);

        let error = error_object(config, 1024, 3, &[11, 13]);
        assert_header(&error, ObjectTag::Error, 2);
        assert_eq!(u32::from_le_bytes(error.bytes[8..12].try_into().unwrap()), 3);
        assert_eq!(u64::from_le_bytes(error.bytes[12..20].try_into().unwrap()), 11);
        assert_eq!(u64::from_le_bytes(error.bytes[20..28].try_into().unwrap()), 13);

        let panic = panic_object(config, 1024, 4, &[17]);
        assert_header(&panic, ObjectTag::Panic, 1);
        assert_eq!(u32::from_le_bytes(panic.bytes[8..12].try_into().unwrap()), 4);
        assert_eq!(u64::from_le_bytes(panic.bytes[12..20].try_into().unwrap()), 17);
    }

    #[test]
    fn computes_aligned_object_sizes() {
        let layout = Layout::DEFAULT;

        assert_eq!(layout.bit_array_size(10), 16);
        assert_eq!(layout.list_cons_size(8), 24);
        assert_eq!(layout.tuple_size(3, 8), 32);
        assert_eq!(layout.record_size(2, 8), 24);
        assert_eq!(layout.custom_size(2, 8), 32);
        assert_eq!(layout.closure_size(3), 40);
        assert_eq!(layout.opaque_size(), 16);
        assert_eq!(layout.error_size(2, 8), 32);
        assert_eq!(layout.panic_size(1, 8), 24);
    }

    fn assert_header(object: &StaticObject, tag: ObjectTag, size: u32) {
        assert_eq!(
            ObjectTag::try_from(u32::from_le_bytes(object.bytes[0..4].try_into().unwrap())),
            Ok(tag)
        );
        assert_eq!(u32::from_le_bytes(object.bytes[4..8].try_into().unwrap()), size);
        assert_eq!(object.bytes.len() as u32 % Layout::DEFAULT.alignment, 0);
    }
}
