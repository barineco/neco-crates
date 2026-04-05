#![cfg(feature = "cbor")]

use neco_cbor::CborValue;
use neco_cid::{CborCidError, Cid, CidError, Codec};

#[test]
fn roundtrip_dag_cbor() {
    let cid = Cid::compute(Codec::DagCbor, b"hello world");
    let tag = cid.to_cbor_tag();
    let decoded = Cid::from_cbor_tag(&tag).expect("should decode tag 42");
    assert_eq!(decoded, cid);
}

#[test]
fn roundtrip_raw() {
    let cid = Cid::compute(Codec::Raw, b"raw payload");
    let tag = cid.to_cbor_tag();
    let decoded = Cid::from_cbor_tag(&tag).expect("should decode tag 42");
    assert_eq!(decoded, cid);
}

#[test]
fn ipld_structure_has_correct_format() {
    let cid = Cid::compute(Codec::DagCbor, b"hello world");
    let cid_bytes = cid.to_bytes();
    let tag = cid.to_cbor_tag();

    match &tag {
        CborValue::Tag(42, inner) => match inner.as_ref() {
            CborValue::Bytes(payload) => {
                assert_eq!(payload[0], 0x00, "first byte must be 0x00 identity prefix");
                assert_eq!(&payload[1..], &cid_bytes);
            }
            other => panic!("expected Bytes, got {other:?}"),
        },
        other => panic!("expected Tag(42, _), got {other:?}"),
    }
}

#[test]
fn rejects_non_tag_value() {
    let value = CborValue::Unsigned(42);
    assert_eq!(Cid::from_cbor_tag(&value), Err(CborCidError::NotATag));
}

#[test]
fn rejects_wrong_tag_number() {
    let value = CborValue::Tag(99, Box::new(CborValue::Bytes(vec![0x00])));
    assert_eq!(Cid::from_cbor_tag(&value), Err(CborCidError::WrongTag(99)));
}

#[test]
fn rejects_non_bytes_payload() {
    let value = CborValue::Tag(42, Box::new(CborValue::Text("not bytes".into())));
    assert_eq!(Cid::from_cbor_tag(&value), Err(CborCidError::NotBytes));
}

#[test]
fn rejects_missing_identity_prefix() {
    // tag 42 with empty bytes
    let value = CborValue::Tag(42, Box::new(CborValue::Bytes(vec![])));
    assert_eq!(
        Cid::from_cbor_tag(&value),
        Err(CborCidError::MissingIdentityPrefix)
    );
}

#[test]
fn rejects_wrong_identity_prefix() {
    let cid = Cid::compute(Codec::DagCbor, b"data");
    let mut payload = vec![0x01]; // wrong prefix
    payload.extend_from_slice(&cid.to_bytes());
    let value = CborValue::Tag(42, Box::new(CborValue::Bytes(payload)));
    assert_eq!(
        Cid::from_cbor_tag(&value),
        Err(CborCidError::MissingIdentityPrefix)
    );
}

#[test]
fn rejects_trailing_data() {
    let cid = Cid::compute(Codec::DagCbor, b"data");
    let mut payload = vec![0x00];
    payload.extend_from_slice(&cid.to_bytes());
    payload.extend_from_slice(b"extra");
    let value = CborValue::Tag(42, Box::new(CborValue::Bytes(payload)));
    assert_eq!(Cid::from_cbor_tag(&value), Err(CborCidError::TrailingData));
}

#[test]
fn rejects_invalid_cid_bytes() {
    // 0x00 prefix + invalid CID (version 0x02)
    let value = CborValue::Tag(42, Box::new(CborValue::Bytes(vec![0x00, 0x02])));
    assert!(matches!(
        Cid::from_cbor_tag(&value),
        Err(CborCidError::InvalidCid(CidError::InvalidVersion(2)))
    ));
}

#[test]
fn optional_null_returns_none() {
    let value = CborValue::Null;
    assert_eq!(Cid::from_cbor_tag_optional(&value), Ok(None));
}

#[test]
fn optional_tag_returns_some() {
    let cid = Cid::compute(Codec::DagCbor, b"optional");
    let tag = cid.to_cbor_tag();
    let result = Cid::from_cbor_tag_optional(&tag).expect("should decode");
    assert_eq!(result, Some(cid));
}

#[test]
fn optional_non_tag_non_null_returns_error() {
    let value = CborValue::Unsigned(123);
    assert_eq!(
        Cid::from_cbor_tag_optional(&value),
        Err(CborCidError::NotATag)
    );
}
