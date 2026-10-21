use neco_cbor::{decode, decode_dag, encode, encode_dag, CborValue, DecodeErrorKind, EncodeError};

#[test]
fn roundtrip_standard_cbor_preserves_value() {
    let value = CborValue::Map(vec![
        (CborValue::Text("key".into()), CborValue::Unsigned(42)),
        (
            CborValue::Text("nested".into()),
            CborValue::Array(vec![
                CborValue::Bool(true),
                CborValue::Null,
                CborValue::Negative(-100),
                CborValue::Bytes(vec![0xDE, 0xAD]),
            ]),
        ),
    ]);
    let encoded = encode(&value).expect("encode should succeed");
    let decoded = decode(&encoded).expect("decode should succeed");
    assert_eq!(decoded, value);
}

#[test]
fn roundtrip_dag_cbor_sorts_keys() {
    let value = CborValue::Map(vec![
        (CborValue::Text("bb".into()), CborValue::Unsigned(2)),
        (CborValue::Text("a".into()), CborValue::Unsigned(1)),
        (CborValue::Text("b".into()), CborValue::Unsigned(3)),
    ]);

    let encoded = encode_dag(&value).expect("dag encode should succeed");
    assert_eq!(
        encoded,
        vec![0xA3, 0x61, b'a', 0x01, 0x61, b'b', 0x03, 0x62, b'b', b'b', 0x02]
    );

    let decoded = decode_dag(&encoded).expect("dag decode should succeed");
    let sorted = CborValue::Map(vec![
        (CborValue::Text("a".into()), CborValue::Unsigned(1)),
        (CborValue::Text("b".into()), CborValue::Unsigned(3)),
        (CborValue::Text("bb".into()), CborValue::Unsigned(2)),
    ]);
    assert_eq!(decoded, sorted);
}

#[test]
fn decode_dag_rejects_float() {
    let error = decode_dag(&[0xF9, 0x3C, 0x00]).expect_err("float must fail");
    assert!(matches!(error.kind(), DecodeErrorKind::FloatNotAllowed));
}

#[test]
fn decode_dag_rejects_indefinite_length_bytes() {
    let error = decode_dag(&[0x5F, 0x41, 0x01, 0xFF]).expect_err("indefinite must fail");
    assert!(matches!(error.kind(), DecodeErrorKind::IndefiniteLength));
}

#[test]
fn decode_dag_rejects_unsorted_map_keys() {
    let bytes = encode(&CborValue::Map(vec![
        (CborValue::Text("b".into()), CborValue::Unsigned(1)),
        (CborValue::Text("a".into()), CborValue::Unsigned(2)),
    ]))
    .unwrap();
    let error = decode_dag(&bytes).expect_err("unsorted keys must fail");
    assert!(matches!(error.kind(), DecodeErrorKind::UnsortedMapKeys));
}

#[test]
fn decode_dag_rejects_non_text_map_key() {
    let bytes = encode(&CborValue::Map(vec![(
        CborValue::Unsigned(1),
        CborValue::Unsigned(2),
    )]))
    .unwrap();
    let error = decode_dag(&bytes).expect_err("non-text key must fail");
    assert!(matches!(error.kind(), DecodeErrorKind::NonTextMapKey));
}

#[test]
fn decode_dag_rejects_non_canonical_integer() {
    let error = decode_dag(&[0x18, 0x00]).expect_err("non-canonical must fail");
    assert!(matches!(error.kind(), DecodeErrorKind::NonCanonicalInteger));
}

#[test]
fn decode_dag_rejects_duplicate_map_key() {
    let error = decode_dag(&[0xA2, 0x61, 0x61, 0x01, 0x61, 0x61, 0x02])
        .expect_err("duplicate key must fail");
    assert!(matches!(error.kind(), DecodeErrorKind::DuplicateMapKey));
}

#[test]
fn decode_dag_rejects_unsupported_tag() {
    let error = decode_dag(&[0xD8, 0x63, 0x00]).expect_err("unsupported tag must fail");
    assert!(matches!(error.kind(), DecodeErrorKind::UnsupportedTag(99)));
}

#[test]
fn tag_42_cid_link_roundtrip() {
    let cid_binary = vec![
        0x00, 0x01, 0x71, 0x12, 0x20, 0x00, 0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x08, 0x09,
        0x0A, 0x0B, 0x0C, 0x0D, 0x0E, 0x0F, 0x10, 0x11, 0x12, 0x13, 0x14, 0x15, 0x16, 0x17, 0x18,
        0x19, 0x1A, 0x1B, 0x1C, 0x1D, 0x1E, 0x1F,
    ];
    let value = CborValue::Tag(42, Box::new(CborValue::Bytes(cid_binary.clone())));

    let encoded = encode(&value).unwrap();
    let decoded = decode(&encoded).unwrap();
    assert_eq!(decoded, value);

    let dag_encoded = encode_dag(&value).unwrap();
    let dag_decoded = decode_dag(&dag_encoded).unwrap();
    assert_eq!(dag_decoded, value);
}

#[test]
fn encode_dag_rejects_non_text_key() {
    let value = CborValue::Map(vec![(CborValue::Unsigned(1), CborValue::Unsigned(2))]);
    assert_eq!(encode_dag(&value), Err(EncodeError::NonTextKeyInDagMode));
}

#[test]
fn encode_dag_rejects_duplicate_key() {
    let value = CborValue::Map(vec![
        (CborValue::Text("a".into()), CborValue::Unsigned(1)),
        (CborValue::Text("a".into()), CborValue::Unsigned(2)),
    ]);
    assert_eq!(encode_dag(&value), Err(EncodeError::DuplicateKeyInDagMode));
}

#[test]
fn dag_cbor_commit_block_structure_roundtrip() {
    let cid_bytes = vec![
        0x00, 0x01, 0x71, 0x12, 0x20, 0xAA, 0xBB, 0xCC, 0xDD, 0xEE, 0xFF, 0x00, 0x11, 0x22, 0x33,
        0x44, 0x55, 0x66, 0x77, 0x88, 0x99, 0xAA, 0xBB, 0xCC, 0xDD, 0xEE, 0xFF, 0x00, 0x11, 0x22,
        0x33, 0x44, 0x55, 0x66, 0x77, 0x88, 0x99,
    ];
    let commit = CborValue::Map(vec![
        (
            CborValue::Text("did".into()),
            CborValue::Text("did:plc:example".into()),
        ),
        (
            CborValue::Text("rev".into()),
            CborValue::Text("2224574000000".into()),
        ),
        (
            CborValue::Text("sig".into()),
            CborValue::Bytes(vec![0x01; 64]),
        ),
        (
            CborValue::Text("data".into()),
            CborValue::Tag(42, Box::new(CborValue::Bytes(cid_bytes.clone()))),
        ),
        (CborValue::Text("prev".into()), CborValue::Null),
        (CborValue::Text("version".into()), CborValue::Unsigned(3)),
    ]);

    let encoded = encode_dag(&commit).unwrap();
    let decoded = decode_dag(&encoded).unwrap();
    assert_eq!(decoded, commit);
}
