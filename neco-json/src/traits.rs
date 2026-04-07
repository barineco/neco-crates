use alloc::boxed::Box;
use alloc::string::{String, ToString};
use alloc::vec::Vec;

use crate::error::AccessError;
use crate::value::JsonValue;

/// Converts `self` into a [`JsonValue`].
pub trait ToJson {
    fn to_json(&self) -> JsonValue;
}

/// Constructs `Self` from a [`JsonValue`] reference.
pub trait FromJson: Sized {
    fn from_json(value: &JsonValue) -> Result<Self, AccessError>;
}

impl ToJson for bool {
    fn to_json(&self) -> JsonValue {
        JsonValue::Bool(*self)
    }
}

impl ToJson for i64 {
    fn to_json(&self) -> JsonValue {
        JsonValue::Number(*self as f64)
    }
}

impl ToJson for u64 {
    fn to_json(&self) -> JsonValue {
        JsonValue::Number(*self as f64)
    }
}

impl ToJson for f64 {
    fn to_json(&self) -> JsonValue {
        JsonValue::Number(*self)
    }
}

impl ToJson for String {
    fn to_json(&self) -> JsonValue {
        JsonValue::String(self.clone())
    }
}

impl ToJson for str {
    fn to_json(&self) -> JsonValue {
        JsonValue::String(self.into())
    }
}

impl<T: ToJson> ToJson for Vec<T> {
    fn to_json(&self) -> JsonValue {
        JsonValue::Array(self.iter().map(|v| v.to_json()).collect())
    }
}

impl<T: ToJson> ToJson for Option<T> {
    fn to_json(&self) -> JsonValue {
        match self {
            Some(v) => v.to_json(),
            None => JsonValue::Null,
        }
    }
}

impl<T: ToJson> ToJson for Box<T> {
    fn to_json(&self) -> JsonValue {
        self.as_ref().to_json()
    }
}

impl FromJson for bool {
    fn from_json(value: &JsonValue) -> Result<Self, AccessError> {
        value.as_bool().ok_or_else(|| AccessError::TypeMismatch {
            field: String::new(),
            expected: "bool",
        })
    }
}

impl FromJson for i64 {
    fn from_json(value: &JsonValue) -> Result<Self, AccessError> {
        value.as_i64().ok_or_else(|| AccessError::TypeMismatch {
            field: String::new(),
            expected: "integer",
        })
    }
}

impl FromJson for u64 {
    fn from_json(value: &JsonValue) -> Result<Self, AccessError> {
        value.as_u64().ok_or_else(|| AccessError::TypeMismatch {
            field: String::new(),
            expected: "non-negative integer",
        })
    }
}

impl FromJson for f64 {
    fn from_json(value: &JsonValue) -> Result<Self, AccessError> {
        value.as_f64().ok_or_else(|| AccessError::TypeMismatch {
            field: String::new(),
            expected: "number",
        })
    }
}

impl FromJson for String {
    fn from_json(value: &JsonValue) -> Result<Self, AccessError> {
        value
            .as_str()
            .map(|s| s.to_string())
            .ok_or_else(|| AccessError::TypeMismatch {
                field: String::new(),
                expected: "string",
            })
    }
}

impl<T: FromJson> FromJson for Vec<T> {
    fn from_json(value: &JsonValue) -> Result<Self, AccessError> {
        let items = value.as_array().ok_or_else(|| AccessError::TypeMismatch {
            field: String::new(),
            expected: "array",
        })?;
        items.iter().map(T::from_json).collect()
    }
}

impl<T: FromJson> FromJson for Option<T> {
    fn from_json(value: &JsonValue) -> Result<Self, AccessError> {
        if value.is_null() {
            Ok(None)
        } else {
            T::from_json(value).map(Some)
        }
    }
}

impl<T: FromJson> FromJson for Box<T> {
    fn from_json(value: &JsonValue) -> Result<Self, AccessError> {
        T::from_json(value).map(Box::new)
    }
}

#[cfg(test)]
mod tests {
    use alloc::boxed::Box;
    use alloc::string::{String, ToString};
    use alloc::vec;

    use super::{FromJson, ToJson};
    use crate::value::JsonValue;

    fn roundtrip<T: ToJson + FromJson + PartialEq + core::fmt::Debug>(value: T) {
        let json = value.to_json();
        let restored = T::from_json(&json).expect("from_json failed");
        assert_eq!(value, restored);
    }

    #[test]
    fn roundtrip_bool_true() {
        roundtrip(true);
    }

    #[test]
    fn roundtrip_bool_false() {
        roundtrip(false);
    }

    #[test]
    fn roundtrip_i64_positive() {
        roundtrip(42_i64);
    }

    #[test]
    fn roundtrip_i64_negative() {
        roundtrip(-1_i64);
    }

    #[test]
    fn roundtrip_u64_zero() {
        roundtrip(0_u64);
    }

    #[test]
    fn roundtrip_u64_large() {
        roundtrip(1_000_000_u64);
    }

    #[test]
    fn roundtrip_f64() {
        roundtrip(2.5_f64);
    }

    #[test]
    fn roundtrip_string() {
        roundtrip("hello world".to_string());
    }

    #[test]
    fn roundtrip_vec_i64() {
        roundtrip(vec![1_i64, 2, 3]);
    }

    #[test]
    fn roundtrip_vec_string() {
        roundtrip(vec!["a".to_string(), "b".to_string()]);
    }

    #[test]
    fn roundtrip_option_some() {
        roundtrip(Some(99_i64));
    }

    #[test]
    fn roundtrip_option_none() {
        let json = None::<i64>.to_json();
        assert_eq!(json, JsonValue::Null);
        let restored = Option::<i64>::from_json(&json).expect("from_json failed");
        assert_eq!(restored, None);
    }

    #[test]
    fn roundtrip_box_string() {
        let original = Box::new("boxed".to_string());
        let json = original.to_json();
        let restored = Box::<String>::from_json(&json).expect("from_json failed");
        assert_eq!(original, restored);
    }

    #[test]
    fn roundtrip_vec_bool() {
        roundtrip(vec![true, false, true]);
    }

    #[test]
    fn str_to_json_produces_string_variant() {
        let json = "neco".to_json();
        assert_eq!(json, JsonValue::String("neco".into()));
    }

    #[test]
    fn type_mismatch_returns_error() {
        let json = JsonValue::Bool(true);
        assert!(i64::from_json(&json).is_err());
        assert!(String::from_json(&json).is_err());
        assert!(f64::from_json(&json).is_err());
    }

    #[test]
    fn as_i64_and_as_u64_added_to_json_value() {
        let n = JsonValue::Number(7.0);
        assert_eq!(n.as_i64(), Some(7_i64));
        assert_eq!(n.as_u64(), Some(7_u64));

        let neg = JsonValue::Number(-3.0);
        assert_eq!(neg.as_i64(), Some(-3_i64));
        assert_eq!(neg.as_u64(), None);

        let frac = JsonValue::Number(1.5);
        assert_eq!(frac.as_i64(), None);
        assert_eq!(frac.as_u64(), None);
    }
}
