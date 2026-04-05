use alloc::string::String;
use core::fmt;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DecodeError {
    kind: DecodeErrorKind,
    position: usize,
}

impl DecodeError {
    pub fn new(kind: DecodeErrorKind, position: usize) -> Self {
        Self { kind, position }
    }

    pub fn kind(&self) -> &DecodeErrorKind {
        &self.kind
    }

    pub fn position(&self) -> usize {
        self.position
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DecodeErrorKind {
    UnexpectedEnd,
    InvalidMajorType(u8),
    NestingTooDeep,
    IndefiniteLength,
    FloatNotAllowed,
    UnsortedMapKeys,
    NonCanonicalInteger,
    DuplicateMapKey,
    TrailingContent,
    InvalidUtf8,
    NonTextMapKey,
    UnsupportedTag(u64),
    IntegerOverflow,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AccessError {
    NotAMap,
    MissingField(String),
    TypeMismatch {
        field: String,
        expected: &'static str,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EncodeError {
    NonTextKeyInDagMode,
    DuplicateKeyInDagMode,
    UnsupportedTag(u64),
    InvalidTag42Payload,
    InvalidNegativeValue(i64),
}

impl fmt::Display for DecodeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "CBOR decode error at position {}: {}",
            self.position,
            self.kind()
        )
    }
}

impl fmt::Display for DecodeErrorKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnexpectedEnd => f.write_str("unexpected end of input"),
            Self::InvalidMajorType(major) => write!(f, "invalid major type {major}"),
            Self::NestingTooDeep => f.write_str("nesting too deep"),
            Self::IndefiniteLength => f.write_str("indefinite-length item is not supported"),
            Self::FloatNotAllowed => f.write_str("floating-point values are not supported"),
            Self::UnsortedMapKeys => f.write_str("map keys are not sorted in DAG-CBOR order"),
            Self::NonCanonicalInteger => f.write_str("integer is not encoded canonically"),
            Self::DuplicateMapKey => f.write_str("duplicate map key"),
            Self::TrailingContent => f.write_str("trailing content after CBOR value"),
            Self::InvalidUtf8 => f.write_str("invalid UTF-8"),
            Self::NonTextMapKey => f.write_str("map key must be text in DAG-CBOR mode"),
            Self::UnsupportedTag(tag) => write!(f, "unsupported tag {tag}"),
            Self::IntegerOverflow => f.write_str("integer does not fit in target representation"),
        }
    }
}

impl fmt::Display for AccessError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NotAMap => f.write_str("value is not a map"),
            Self::MissingField(field) => write!(f, "missing field \"{field}\""),
            Self::TypeMismatch { field, expected } => {
                write!(f, "field \"{field}\": expected {expected}")
            }
        }
    }
}

impl fmt::Display for EncodeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NonTextKeyInDagMode => f.write_str("map key must be text in DAG-CBOR mode"),
            Self::DuplicateKeyInDagMode => f.write_str("duplicate map key in DAG-CBOR mode"),
            Self::UnsupportedTag(tag) => write!(f, "unsupported tag {tag}"),
            Self::InvalidTag42Payload => {
                f.write_str("tag 42 payload must be a byte string prefixed with 0x00")
            }
            Self::InvalidNegativeValue(value) => {
                write!(f, "negative integer value must be less than zero: {value}")
            }
        }
    }
}

impl core::error::Error for DecodeError {}
impl core::error::Error for AccessError {}
impl core::error::Error for EncodeError {}
