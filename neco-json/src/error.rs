use alloc::string::String;
use core::fmt;

/// JSON parse error with position information.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParseError {
    pub kind: ParseErrorKind,
    pub position: usize,
}

/// Kinds of parse errors.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ParseErrorKind {
    UnexpectedCharacter(u8),
    UnexpectedEnd,
    InvalidNumber,
    InvalidEscape,
    InvalidUnicodeEscape,
    NestingTooDeep,
    TrailingContent,
    InvalidUtf8,
}

/// Accessor error.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AccessError {
    NotAnObject,
    MissingField(String),
    TypeMismatch {
        field: String,
        expected: &'static str,
    },
}

/// JSON encoding error.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EncodeError {
    NonFiniteNumber,
}

impl fmt::Display for ParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "JSON parse error at position {}: {}",
            self.position, self.kind
        )
    }
}

impl fmt::Display for ParseErrorKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnexpectedCharacter(ch) => write!(f, "unexpected character 0x{ch:02X}"),
            Self::UnexpectedEnd => f.write_str("unexpected end of input"),
            Self::InvalidNumber => f.write_str("invalid number"),
            Self::InvalidEscape => f.write_str("invalid escape sequence"),
            Self::InvalidUnicodeEscape => f.write_str("invalid unicode escape"),
            Self::NestingTooDeep => f.write_str("nesting too deep"),
            Self::TrailingContent => f.write_str("trailing content after JSON value"),
            Self::InvalidUtf8 => f.write_str("invalid UTF-8"),
        }
    }
}

impl fmt::Display for AccessError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NotAnObject => f.write_str("value is not an object"),
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
            Self::NonFiniteNumber => f.write_str("cannot encode non-finite number"),
        }
    }
}

impl core::error::Error for ParseError {}
impl core::error::Error for AccessError {}
impl core::error::Error for EncodeError {}
