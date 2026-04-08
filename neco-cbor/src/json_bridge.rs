use alloc::borrow::ToOwned;
use alloc::format;
use alloc::string::{String, ToString};
use alloc::vec;
use alloc::vec::Vec;
use core::fmt;

use crate::cid_tag::{decode_cid_tag, encode_cid_tag};
use crate::CborValue;
use neco_cid::Cid;
use neco_json::JsonValue;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum JsonBridgeError {
    NumberOverflow,
    NonTextMapKey,
    UnsupportedTag(u64),
    InvalidBase64(String),
    InvalidCidLink(String),
    FloatNotSupported,
}

impl fmt::Display for JsonBridgeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NumberOverflow => f.write_str("number overflow"),
            Self::NonTextMapKey => f.write_str("CBOR map key must be text"),
            Self::UnsupportedTag(tag) => write!(f, "unsupported CBOR tag {tag}"),
            Self::InvalidBase64(value) => write!(f, "invalid base64 bytes: {value}"),
            Self::InvalidCidLink(value) => write!(f, "invalid CID link: {value}"),
            Self::FloatNotSupported => f.write_str("floating-point JSON numbers are not supported"),
        }
    }
}

impl core::error::Error for JsonBridgeError {}

pub fn cbor_from_json(json: &JsonValue) -> Result<CborValue, JsonBridgeError> {
    match json {
        JsonValue::Null => Ok(CborValue::Null),
        JsonValue::Bool(value) => Ok(CborValue::Bool(*value)),
        JsonValue::String(value) => Ok(CborValue::Text(value.clone())),
        JsonValue::Number(value) => cbor_from_number(*value),
        JsonValue::Array(values) => values
            .iter()
            .map(cbor_from_json)
            .collect::<Result<Vec<_>, _>>()
            .map(CborValue::Array),
        JsonValue::Object(fields) => {
            if let Some(single) = decode_special_object(fields)? {
                return Ok(single);
            }
            fields
                .iter()
                .map(|(key, value)| Ok((CborValue::Text(key.clone()), cbor_from_json(value)?)))
                .collect::<Result<Vec<_>, JsonBridgeError>>()
                .map(CborValue::Map)
        }
    }
}

pub fn cbor_to_json(cbor: &CborValue) -> Result<JsonValue, JsonBridgeError> {
    match cbor {
        CborValue::Unsigned(value) => {
            if *value > i64::MAX as u64 {
                return Err(JsonBridgeError::NumberOverflow);
            }
            Ok(JsonValue::Number(*value as f64))
        }
        CborValue::Negative(value) => Ok(JsonValue::Number(*value as f64)),
        CborValue::Bytes(value) => Ok(JsonValue::Object(vec![(
            "$bytes".to_owned(),
            JsonValue::String(neco_base64::encode(value)),
        )])),
        CborValue::Text(value) => Ok(JsonValue::String(value.clone())),
        CborValue::Array(values) => values
            .iter()
            .map(cbor_to_json)
            .collect::<Result<Vec<_>, _>>()
            .map(JsonValue::Array),
        CborValue::Map(entries) => entries
            .iter()
            .map(|(key, value)| {
                let key = match key {
                    CborValue::Text(text) => text.clone(),
                    _ => return Err(JsonBridgeError::NonTextMapKey),
                };
                Ok((key, cbor_to_json(value)?))
            })
            .collect::<Result<Vec<_>, JsonBridgeError>>()
            .map(JsonValue::Object),
        CborValue::Tag(tag, _) => {
            if *tag != 42 {
                return Err(JsonBridgeError::UnsupportedTag(*tag));
            }
            let cid = decode_cid_tag(cbor)
                .map_err(|error| JsonBridgeError::InvalidCidLink(error.to_string()))?;
            Ok(JsonValue::Object(vec![(
                "$link".to_owned(),
                JsonValue::String(cid.to_multibase(neco_cid::Base::Base32Lower)),
            )]))
        }
        CborValue::Bool(value) => Ok(JsonValue::Bool(*value)),
        CborValue::Null => Ok(JsonValue::Null),
    }
}

fn cbor_from_number(value: f64) -> Result<CborValue, JsonBridgeError> {
    if !value.is_finite() {
        return Err(JsonBridgeError::NumberOverflow);
    }
    if value.fract() != 0.0 {
        return Err(JsonBridgeError::FloatNotSupported);
    }
    if value < i64::MIN as f64 || value > u64::MAX as f64 {
        return Err(JsonBridgeError::NumberOverflow);
    }
    if value < 0.0 {
        return Ok(CborValue::Negative(value as i64));
    }
    Ok(CborValue::Unsigned(value as u64))
}

fn decode_special_object(
    fields: &[(String, JsonValue)],
) -> Result<Option<CborValue>, JsonBridgeError> {
    if fields.len() != 1 {
        return Ok(None);
    }

    let (key, value) = &fields[0];
    match key.as_str() {
        "$bytes" => {
            let encoded = value
                .as_str()
                .ok_or_else(|| JsonBridgeError::InvalidBase64(format!("{value:?}")))?;
            let bytes = neco_base64::decode(encoded)
                .map_err(|error| JsonBridgeError::InvalidBase64(error.to_string()))?;
            Ok(Some(CborValue::Bytes(bytes)))
        }
        "$link" => {
            let cid = value
                .as_str()
                .ok_or_else(|| JsonBridgeError::InvalidCidLink(format!("{value:?}")))?;
            let cid = Cid::from_multibase(cid)
                .map_err(|error| JsonBridgeError::InvalidCidLink(error.to_string()))?;
            Ok(Some(encode_cid_tag(&cid)))
        }
        _ => Ok(None),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloc::borrow::ToOwned;
    use alloc::vec;

    fn link_json(cid: &str) -> JsonValue {
        JsonValue::Object(vec![(
            "$link".to_owned(),
            JsonValue::String(cid.to_owned()),
        )])
    }

    #[test]
    fn roundtrip_integers_strings_bools_null() {
        let value = JsonValue::Array(vec![
            JsonValue::Number(7.0),
            JsonValue::Number(-2.0),
            JsonValue::String("hello".to_owned()),
            JsonValue::Bool(true),
            JsonValue::Null,
        ]);

        let encoded = cbor_from_json(&value).expect("encode");
        let decoded = cbor_to_json(&encoded).expect("decode");

        assert_eq!(decoded, value);
    }

    #[test]
    fn roundtrip_array_and_object() {
        let value = JsonValue::Object(vec![
            ("name".to_owned(), JsonValue::String("alice".to_owned())),
            (
                "items".to_owned(),
                JsonValue::Array(vec![
                    JsonValue::Number(1.0),
                    JsonValue::Object(vec![("ok".to_owned(), JsonValue::Bool(false))]),
                ]),
            ),
        ]);

        let encoded = cbor_from_json(&value).expect("encode");
        let decoded = cbor_to_json(&encoded).expect("decode");

        assert_eq!(decoded, value);
    }

    #[test]
    fn roundtrip_bytes_via_base64() {
        let value = JsonValue::Object(vec![(
            "$bytes".to_owned(),
            JsonValue::String("aGVsbG8=".to_owned()),
        )]);

        let encoded = cbor_from_json(&value).expect("encode");
        assert_eq!(encoded, CborValue::Bytes(b"hello".to_vec()));

        let decoded = cbor_to_json(&encoded).expect("decode");
        assert_eq!(decoded, value);
    }

    #[test]
    fn roundtrip_cid_link() {
        let cid = neco_cid::Cid::compute(neco_cid::Codec::Raw, b"json-cbor-link")
            .to_multibase(neco_cid::Base::Base32Lower);
        let value = link_json(&cid);

        let encoded = cbor_from_json(&value).expect("encode");
        let decoded = cbor_to_json(&encoded).expect("decode");

        assert_eq!(decoded, value);
    }

    #[test]
    fn error_float_not_supported() {
        let error = cbor_from_json(&JsonValue::Number(1.5)).expect_err("error");
        assert_eq!(error, JsonBridgeError::FloatNotSupported);
    }

    #[test]
    fn error_number_overflow_to_cbor() {
        let error = cbor_from_json(&JsonValue::Number(1e40)).expect_err("error");
        assert_eq!(error, JsonBridgeError::NumberOverflow);
    }

    #[test]
    fn error_number_overflow_from_cbor() {
        let error = cbor_to_json(&CborValue::Unsigned((i64::MAX as u64) + 1)).expect_err("error");
        assert_eq!(error, JsonBridgeError::NumberOverflow);
    }

    #[test]
    fn error_non_text_map_key() {
        let error = cbor_to_json(&CborValue::Map(vec![(
            CborValue::Unsigned(1),
            CborValue::Text("value".to_owned()),
        )]))
        .expect_err("error");
        assert_eq!(error, JsonBridgeError::NonTextMapKey);
    }

    #[test]
    fn roundtrip_negative_integer() {
        let value = JsonValue::Number(-42.0);
        let encoded = cbor_from_json(&value).expect("encode");
        assert_eq!(encoded, CborValue::Negative(-42));
        let decoded = cbor_to_json(&encoded).expect("decode");
        assert_eq!(decoded, value);
    }
}
