use alloc::boxed::Box;
use alloc::string::String;
use alloc::vec::Vec;

use crate::error::AccessError;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CborValue {
    Unsigned(u64),
    Negative(i64),
    Bytes(Vec<u8>),
    Text(String),
    Array(Vec<CborValue>),
    Map(Vec<(CborValue, CborValue)>),
    Tag(u64, Box<CborValue>),
    Bool(bool),
    Null,
}

impl CborValue {
    pub fn as_unsigned(&self) -> Option<u64> {
        match self {
            Self::Unsigned(value) => Some(*value),
            _ => None,
        }
    }

    pub fn as_negative(&self) -> Option<i64> {
        match self {
            Self::Negative(value) => Some(*value),
            _ => None,
        }
    }

    pub fn as_bytes(&self) -> Option<&[u8]> {
        match self {
            Self::Bytes(value) => Some(value.as_slice()),
            _ => None,
        }
    }

    pub fn as_text(&self) -> Option<&str> {
        match self {
            Self::Text(value) => Some(value.as_str()),
            _ => None,
        }
    }

    pub fn as_array(&self) -> Option<&[CborValue]> {
        match self {
            Self::Array(values) => Some(values.as_slice()),
            _ => None,
        }
    }

    pub fn as_map(&self) -> Option<&[(CborValue, CborValue)]> {
        match self {
            Self::Map(values) => Some(values.as_slice()),
            _ => None,
        }
    }

    pub fn as_tag(&self) -> Option<(u64, &CborValue)> {
        match self {
            Self::Tag(tag, value) => Some((*tag, value.as_ref())),
            _ => None,
        }
    }

    pub fn as_bool(&self) -> Option<bool> {
        match self {
            Self::Bool(value) => Some(*value),
            _ => None,
        }
    }

    pub fn is_null(&self) -> bool {
        matches!(self, Self::Null)
    }

    pub fn get(&self, key: &str) -> Option<&CborValue> {
        match self {
            Self::Map(entries) => entries
                .iter()
                .find_map(|(entry_key, value)| match entry_key {
                    Self::Text(text) if text == key => Some(value),
                    _ => None,
                }),
            _ => None,
        }
    }

    pub fn required_text(&self, key: &str) -> Result<&str, AccessError> {
        self.required_value(key, CborValue::as_text, "text")
    }

    pub fn required_bytes(&self, key: &str) -> Result<&[u8], AccessError> {
        self.required_value(key, CborValue::as_bytes, "bytes")
    }

    pub fn required_unsigned(&self, key: &str) -> Result<u64, AccessError> {
        self.required_value(key, CborValue::as_unsigned, "unsigned")
    }

    pub fn required_negative(&self, key: &str) -> Result<i64, AccessError> {
        self.required_value(key, CborValue::as_negative, "negative")
    }

    pub fn required_bool(&self, key: &str) -> Result<bool, AccessError> {
        self.required_value(key, CborValue::as_bool, "bool")
    }

    pub fn required_array(&self, key: &str) -> Result<&[CborValue], AccessError> {
        self.required_value(key, CborValue::as_array, "array")
    }

    pub fn required_map(&self, key: &str) -> Result<&[(CborValue, CborValue)], AccessError> {
        self.required_value(key, CborValue::as_map, "map")
    }

    pub fn required_tag(&self, key: &str) -> Result<(u64, &CborValue), AccessError> {
        self.required_value(key, CborValue::as_tag, "tag")
    }

    fn required_value<'a, T>(
        &'a self,
        key: &str,
        accessor: impl FnOnce(&'a CborValue) -> Option<T>,
        expected: &'static str,
    ) -> Result<T, AccessError> {
        let value = self.object_field(key)?;
        accessor(value).ok_or_else(|| AccessError::TypeMismatch {
            field: key.into(),
            expected,
        })
    }

    fn object_field(&self, key: &str) -> Result<&CborValue, AccessError> {
        match self {
            Self::Map(_) => self
                .get(key)
                .ok_or_else(|| AccessError::MissingField(key.into())),
            _ => Err(AccessError::NotAMap),
        }
    }
}
