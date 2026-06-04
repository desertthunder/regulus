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
        self.align_to(self.header_size + self.word_size + capture_count * self.word_size)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RuntimeConfig {
    pub layout: Layout,
    pub static_data_start: u32,
    pub heap_start: u32,
}

impl RuntimeConfig {
    pub const DEFAULT: Self = Self { layout: Layout::DEFAULT, static_data_start: 1024, heap_start: 4096 };
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
        }
    }
}

impl TryFrom<u32> for ObjectTag {
    type Error = ();

    fn try_from(tag: u32) -> Result<Self, Self::Error> {
        match tag {
            1 => Ok(ObjectTag::String),
            2 => Ok(ObjectTag::ListCons),
            3 => Ok(ObjectTag::Tuple),
            4 => Ok(ObjectTag::Record),
            5 => Ok(ObjectTag::Custom),
            6 => Ok(ObjectTag::Closure),
            7 => Ok(ObjectTag::BitArray),
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

pub fn closure_object(config: RuntimeConfig, offset: u32, function_id: u32, captures: &[u32]) -> StaticObject {
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

pub fn bit_array_payload_len(bit_len: u32) -> u32 {
    bit_len.div_ceil(8)
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
        assert_eq!(ObjectTag::try_from(6), Ok(ObjectTag::Closure));
        assert_eq!(ObjectTag::try_from(7), Ok(ObjectTag::BitArray));
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
        assert_eq!(
            ObjectTag::try_from(u32::from_le_bytes(list.bytes[0..4].try_into().unwrap())),
            Ok(ObjectTag::ListCons)
        );
        assert_eq!(u64::from_le_bytes(list.bytes[8..16].try_into().unwrap()), 42);

        let tuple = tuple_object(config, 1024, &[1, 2]);
        assert_eq!(
            ObjectTag::try_from(u32::from_le_bytes(tuple.bytes[0..4].try_into().unwrap())),
            Ok(ObjectTag::Tuple)
        );
        assert_eq!(u32::from_le_bytes(tuple.bytes[4..8].try_into().unwrap()), 2);

        let record = record_object(config, 1024, &[3, 4]);
        assert_eq!(
            ObjectTag::try_from(u32::from_le_bytes(record.bytes[0..4].try_into().unwrap())),
            Ok(ObjectTag::Record)
        );

        let custom = custom_object(config, 1024, 7, &[5]);
        assert_eq!(
            ObjectTag::try_from(u32::from_le_bytes(custom.bytes[0..4].try_into().unwrap())),
            Ok(ObjectTag::Custom)
        );
        assert_eq!(u32::from_le_bytes(custom.bytes[8..12].try_into().unwrap()), 7);

        let closure = closure_object(config, 1024, 9, &[11]);
        assert_eq!(
            ObjectTag::try_from(u32::from_le_bytes(closure.bytes[0..4].try_into().unwrap())),
            Ok(ObjectTag::Closure)
        );
        assert_eq!(u32::from_le_bytes(closure.bytes[8..12].try_into().unwrap()), 9);
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
    fn computes_aligned_object_sizes() {
        let layout = Layout::DEFAULT;

        assert_eq!(layout.bit_array_size(10), 16);
        assert_eq!(layout.list_cons_size(8), 24);
        assert_eq!(layout.tuple_size(3, 8), 32);
        assert_eq!(layout.record_size(2, 8), 24);
        assert_eq!(layout.custom_size(2, 8), 32);
        assert_eq!(layout.closure_size(3), 24);
    }
}
