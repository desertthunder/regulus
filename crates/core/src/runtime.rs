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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn converts_object_tags_to_and_from_u32() {
        assert_eq!(u32::from(ObjectTag::String), 1);
        assert_eq!(ObjectTag::try_from(6), Ok(ObjectTag::Closure));
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
    fn computes_aligned_object_sizes() {
        let layout = Layout::DEFAULT;

        assert_eq!(layout.list_cons_size(8), 24);
        assert_eq!(layout.tuple_size(3, 8), 32);
        assert_eq!(layout.record_size(2, 8), 24);
        assert_eq!(layout.custom_size(2, 8), 32);
        assert_eq!(layout.closure_size(3), 24);
    }
}
