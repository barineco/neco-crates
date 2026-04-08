use alloc::boxed::Box;
use alloc::vec::Vec;
use core::fmt;

use crate::CborValue;
use neco_cid::{Cid, CidError};

const CID_TAG: u64 = 42;
const CID_IDENTITY_PREFIX: u8 = 0x00;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CidTagError {
    NotATag,
    WrongTag(u64),
    NotBytes,
    MissingIdentityPrefix,
    InvalidCid(CidError),
    TrailingData,
}

impl fmt::Display for CidTagError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NotATag => f.write_str("expected CBOR tag"),
            Self::WrongTag(tag) => write!(f, "expected tag 42, got tag {tag}"),
            Self::NotBytes => f.write_str("tag 42 payload must be bytes"),
            Self::MissingIdentityPrefix => {
                f.write_str("tag 42 payload missing 0x00 identity prefix")
            }
            Self::InvalidCid(error) => write!(f, "invalid binary CID: {error}"),
            Self::TrailingData => f.write_str("trailing data after CID"),
        }
    }
}

impl core::error::Error for CidTagError {}

pub fn encode_cid_tag(cid: &Cid) -> CborValue {
    let cid_bytes = cid.to_bytes();
    let mut payload = Vec::with_capacity(cid_bytes.len() + 1);
    payload.push(CID_IDENTITY_PREFIX);
    payload.extend_from_slice(&cid_bytes);
    CborValue::Tag(CID_TAG, Box::new(CborValue::Bytes(payload)))
}

pub fn decode_cid_tag(value: &CborValue) -> Result<Cid, CidTagError> {
    let (tag, payload) = value.as_tag().ok_or(CidTagError::NotATag)?;
    if tag != CID_TAG {
        return Err(CidTagError::WrongTag(tag));
    }
    let bytes = payload.as_bytes().ok_or(CidTagError::NotBytes)?;
    let (prefix, rest) = bytes
        .split_first()
        .ok_or(CidTagError::MissingIdentityPrefix)?;
    if *prefix != CID_IDENTITY_PREFIX {
        return Err(CidTagError::MissingIdentityPrefix);
    }
    let (cid, consumed) = Cid::from_bytes(rest).map_err(CidTagError::InvalidCid)?;
    if consumed != rest.len() {
        return Err(CidTagError::TrailingData);
    }
    Ok(cid)
}

pub fn decode_optional_cid_tag(value: &CborValue) -> Result<Option<Cid>, CidTagError> {
    if value.is_null() {
        return Ok(None);
    }
    decode_cid_tag(value).map(Some)
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloc::boxed::Box;
    use alloc::vec;
    use neco_cid::Codec;

    #[test]
    fn roundtrip_dag_cbor_tag() {
        let cid = Cid::compute(Codec::DagCbor, b"hello");
        let tag = encode_cid_tag(&cid);
        let decoded = decode_cid_tag(&tag).expect("decode tag 42");
        assert_eq!(decoded, cid);
    }

    #[test]
    fn roundtrip_raw_tag() {
        let cid = Cid::compute(Codec::Raw, b"bytes");
        let tag = encode_cid_tag(&cid);
        let decoded = decode_cid_tag(&tag).expect("decode tag 42");
        assert_eq!(decoded, cid);
    }

    #[test]
    fn tag_payload_has_identity_prefix() {
        let cid = Cid::compute(Codec::DagCbor, b"prefix");
        let tag = encode_cid_tag(&cid);

        match tag {
            CborValue::Tag(42, inner) => match inner.as_ref() {
                CborValue::Bytes(payload) => {
                    assert_eq!(payload[0], 0x00);
                    assert_eq!(&payload[1..], cid.to_bytes().as_slice());
                }
                other => panic!("expected bytes payload, got {other:?}"),
            },
            other => panic!("expected tag 42, got {other:?}"),
        }
    }

    #[test]
    fn decode_rejects_non_tag() {
        let value = CborValue::Unsigned(42);
        assert_eq!(decode_cid_tag(&value), Err(CidTagError::NotATag));
    }

    #[test]
    fn decode_rejects_wrong_tag() {
        let value = CborValue::Tag(99, Box::new(CborValue::Bytes(vec![0x00])));
        assert_eq!(decode_cid_tag(&value), Err(CidTagError::WrongTag(99)));
    }

    #[test]
    fn decode_rejects_non_bytes_payload() {
        let value = CborValue::Tag(42, Box::new(CborValue::Text("not bytes".into())));
        assert_eq!(decode_cid_tag(&value), Err(CidTagError::NotBytes));
    }

    #[test]
    fn decode_rejects_missing_prefix() {
        let value = CborValue::Tag(42, Box::new(CborValue::Bytes(vec![])));
        assert_eq!(
            decode_cid_tag(&value),
            Err(CidTagError::MissingIdentityPrefix)
        );
    }

    #[test]
    fn decode_rejects_wrong_prefix() {
        let cid = Cid::compute(Codec::DagCbor, b"wrong-prefix");
        let mut payload = vec![0x01];
        payload.extend_from_slice(&cid.to_bytes());
        let value = CborValue::Tag(42, Box::new(CborValue::Bytes(payload)));
        assert_eq!(
            decode_cid_tag(&value),
            Err(CidTagError::MissingIdentityPrefix)
        );
    }

    #[test]
    fn decode_rejects_trailing_data() {
        let cid = Cid::compute(Codec::Raw, b"trailing");
        let mut payload = vec![0x00];
        payload.extend_from_slice(&cid.to_bytes());
        payload.push(0xff);
        let value = CborValue::Tag(42, Box::new(CborValue::Bytes(payload)));
        assert_eq!(decode_cid_tag(&value), Err(CidTagError::TrailingData));
    }

    #[test]
    fn decode_rejects_invalid_cid() {
        let value = CborValue::Tag(42, Box::new(CborValue::Bytes(vec![0x00, 0x02])));
        assert_eq!(
            decode_cid_tag(&value),
            Err(CidTagError::InvalidCid(CidError::InvalidVersion(2)))
        );
    }

    #[test]
    fn decode_optional_null() {
        assert_eq!(decode_optional_cid_tag(&CborValue::Null), Ok(None));
    }

    #[test]
    fn decode_optional_tag() {
        let cid = Cid::compute(Codec::DagCbor, b"optional");
        let tag = encode_cid_tag(&cid);
        assert_eq!(decode_optional_cid_tag(&tag), Ok(Some(cid)));
    }

    #[test]
    fn decode_optional_reuses_tag_validation() {
        let value = CborValue::Unsigned(123);
        assert_eq!(decode_optional_cid_tag(&value), Err(CidTagError::NotATag));
    }
}
