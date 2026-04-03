use alloc::string::String;
use alloc::vec::Vec;

use crate::error::AccessError;

#[derive(Debug, Clone, PartialEq)]
pub enum JsonValue {
    Null,
    Bool(bool),
    Number(f64),
    String(String),
    Array(Vec<JsonValue>),
    Object(Vec<(String, JsonValue)>),
}

impl JsonValue {
    pub fn is_null(&self) -> bool {
        matches!(self, Self::Null)
    }

    pub fn is_bool(&self) -> bool {
        matches!(self, Self::Bool(_))
    }

    pub fn is_number(&self) -> bool {
        matches!(self, Self::Number(_))
    }

    pub fn is_string(&self) -> bool {
        matches!(self, Self::String(_))
    }

    pub fn is_array(&self) -> bool {
        matches!(self, Self::Array(_))
    }

    pub fn is_object(&self) -> bool {
        matches!(self, Self::Object(_))
    }

    pub fn as_bool(&self) -> Option<bool> {
        match self {
            Self::Bool(value) => Some(*value),
            _ => None,
        }
    }

    pub fn as_f64(&self) -> Option<f64> {
        match self {
            Self::Number(value) => Some(*value),
            _ => None,
        }
    }

    pub fn as_str(&self) -> Option<&str> {
        match self {
            Self::String(value) => Some(value.as_str()),
            _ => None,
        }
    }

    pub fn as_array(&self) -> Option<&[JsonValue]> {
        match self {
            Self::Array(values) => Some(values.as_slice()),
            _ => None,
        }
    }

    pub fn as_object(&self) -> Option<&[(String, JsonValue)]> {
        match self {
            Self::Object(fields) => Some(fields.as_slice()),
            _ => None,
        }
    }

    pub fn get(&self, key: &str) -> Option<&JsonValue> {
        match self {
            Self::Object(fields) => fields
                .iter()
                .find_map(|(field, value)| (field == key).then_some(value)),
            _ => None,
        }
    }

    pub fn required_str(&self, key: &str) -> Result<&str, AccessError> {
        self.required_value(key, JsonValue::as_str, "string")
    }

    pub fn required_bool(&self, key: &str) -> Result<bool, AccessError> {
        self.required_value(key, JsonValue::as_bool, "bool")
    }

    pub fn required_f64(&self, key: &str) -> Result<f64, AccessError> {
        self.required_value(key, JsonValue::as_f64, "number")
    }

    pub fn required_array(&self, key: &str) -> Result<&[JsonValue], AccessError> {
        self.required_value(key, JsonValue::as_array, "array")
    }

    pub fn required_object(&self, key: &str) -> Result<&[(String, JsonValue)], AccessError> {
        self.required_value(key, JsonValue::as_object, "object")
    }

    pub fn optional_str(&self, key: &str) -> Result<Option<&str>, AccessError> {
        self.optional_value(key, JsonValue::as_str, "string")
    }

    pub fn optional_bool(&self, key: &str) -> Result<Option<bool>, AccessError> {
        self.optional_value(key, JsonValue::as_bool, "bool")
    }

    pub fn optional_f64(&self, key: &str) -> Result<Option<f64>, AccessError> {
        self.optional_value(key, JsonValue::as_f64, "number")
    }

    pub fn optional_array(&self, key: &str) -> Result<Option<&[JsonValue]>, AccessError> {
        self.optional_value(key, JsonValue::as_array, "array")
    }

    fn required_value<'a, T>(
        &'a self,
        key: &str,
        accessor: impl FnOnce(&'a JsonValue) -> Option<T>,
        expected: &'static str,
    ) -> Result<T, AccessError> {
        let value = self.object_field(key)?;
        accessor(value).ok_or_else(|| AccessError::TypeMismatch {
            field: key.into(),
            expected,
        })
    }

    fn optional_value<'a, T>(
        &'a self,
        key: &str,
        accessor: impl FnOnce(&'a JsonValue) -> Option<T>,
        expected: &'static str,
    ) -> Result<Option<T>, AccessError> {
        let Some(value) = self.object_field_optional(key)? else {
            return Ok(None);
        };

        if value.is_null() {
            return Ok(None);
        }

        accessor(value)
            .map(Some)
            .ok_or_else(|| AccessError::TypeMismatch {
                field: key.into(),
                expected,
            })
    }

    fn object_field(&self, key: &str) -> Result<&JsonValue, AccessError> {
        self.object_field_optional(key)?
            .ok_or_else(|| AccessError::MissingField(key.into()))
    }

    fn object_field_optional(&self, key: &str) -> Result<Option<&JsonValue>, AccessError> {
        match self {
            Self::Object(_) => Ok(self.get(key)),
            _ => Err(AccessError::NotAnObject),
        }
    }
}

#[cfg(test)]
mod tests {
    use alloc::string::String;
    use alloc::vec;

    use super::JsonValue;
    use crate::AccessError;

    fn sample_object() -> JsonValue {
        JsonValue::Object(vec![
            ("name".into(), JsonValue::String("neco".into())),
            ("enabled".into(), JsonValue::Bool(true)),
            ("score".into(), JsonValue::Number(42.5)),
            (
                "items".into(),
                JsonValue::Array(vec![JsonValue::Bool(false), JsonValue::Null]),
            ),
            (
                "meta".into(),
                JsonValue::Object(vec![("nested".into(), JsonValue::String("ok".into()))]),
            ),
            ("maybe".into(), JsonValue::Null),
            ("dup".into(), JsonValue::String("first".into())),
            ("dup".into(), JsonValue::String("second".into())),
        ])
    }

    #[test]
    fn is_methods_match_variants() {
        assert!(JsonValue::Null.is_null());
        assert!(JsonValue::Bool(true).is_bool());
        assert!(JsonValue::Number(1.0).is_number());
        assert!(JsonValue::String("x".into()).is_string());
        assert!(JsonValue::Array(vec![]).is_array());
        assert!(JsonValue::Object(vec![]).is_object());
    }

    #[test]
    fn as_methods_return_expected_values() {
        let value = JsonValue::Bool(true);
        assert_eq!(value.as_bool(), Some(true));
        assert_eq!(value.as_f64(), None);

        let value = JsonValue::Number(3.25);
        assert_eq!(value.as_f64(), Some(3.25));
        assert_eq!(value.as_str(), None);

        let value = JsonValue::String("cat".into());
        assert_eq!(value.as_str(), Some("cat"));
        assert_eq!(value.as_array(), None);

        let array = vec![JsonValue::Null];
        let value = JsonValue::Array(array.clone());
        assert_eq!(value.as_array(), Some(array.as_slice()));
        assert_eq!(value.as_object(), None);

        let object = vec![("k".into(), JsonValue::Bool(false))];
        let value = JsonValue::Object(object.clone());
        assert_eq!(value.as_object(), Some(object.as_slice()));
        assert_eq!(value.as_bool(), None);
    }

    #[test]
    fn get_reads_object_field_and_keeps_first_duplicate() {
        let object = sample_object();

        assert_eq!(object.get("name"), Some(&JsonValue::String("neco".into())));
        assert_eq!(object.get("dup"), Some(&JsonValue::String("first".into())));
        assert_eq!(object.get("missing"), None);
        assert_eq!(JsonValue::Bool(true).get("name"), None);
    }

    #[test]
    fn required_accessors_cover_success_and_errors() {
        let object = sample_object();

        assert_eq!(object.required_str("name"), Ok("neco"));
        assert_eq!(object.required_bool("enabled"), Ok(true));
        assert_eq!(object.required_f64("score"), Ok(42.5));
        assert_eq!(
            object.required_array("items"),
            Ok([JsonValue::Bool(false), JsonValue::Null].as_slice())
        );
        assert_eq!(
            object.required_object("meta"),
            Ok([("nested".into(), JsonValue::String("ok".into()))].as_slice())
        );

        assert_eq!(
            object.required_str("missing"),
            Err(AccessError::MissingField(String::from("missing")))
        );
        assert_eq!(
            object.required_bool("name"),
            Err(AccessError::TypeMismatch {
                field: String::from("name"),
                expected: "bool",
            })
        );
        assert_eq!(
            JsonValue::Null.required_str("name"),
            Err(AccessError::NotAnObject)
        );
    }

    #[test]
    fn optional_accessors_cover_success_none_and_errors() {
        let object = sample_object();

        assert_eq!(object.optional_str("name"), Ok(Some("neco")));
        assert_eq!(object.optional_bool("enabled"), Ok(Some(true)));
        assert_eq!(object.optional_f64("score"), Ok(Some(42.5)));
        assert_eq!(
            object.optional_array("items"),
            Ok(Some([JsonValue::Bool(false), JsonValue::Null].as_slice()))
        );

        assert_eq!(object.optional_str("missing"), Ok(None));
        assert_eq!(object.optional_str("maybe"), Ok(None));
        assert_eq!(
            object.optional_bool("name"),
            Err(AccessError::TypeMismatch {
                field: String::from("name"),
                expected: "bool",
            })
        );
        assert_eq!(
            JsonValue::Null.optional_str("name"),
            Err(AccessError::NotAnObject)
        );
    }
}
