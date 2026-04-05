use std::collections::HashMap;

use neco_cid::{Base, Cid, CidError, Codec};

const HELLO_WORLD_DAG_CBOR: &str = "bafyreifzjut3te2nhyekklss27nh3k72ysco7y32koao5eei66wof36n5e";
const EMPTY_DAG_CBOR: &str = "bafyreihdwdcefgh4dqkjv67uzcmw7ojee6xedzdetojuzjevtenxquvyku";
const HELLO_WORLD_DAG_CBOR_BYTES: [u8; 36] = [
    0x01, 0x71, 0x12, 0x20, 0xb9, 0x4d, 0x27, 0xb9, 0x93, 0x4d, 0x3e, 0x08, 0xa5, 0x2e, 0x52, 0xd7,
    0xda, 0x7d, 0xab, 0xfa, 0xc4, 0x84, 0xef, 0xe3, 0x7a, 0x53, 0x80, 0xee, 0x90, 0x88, 0xf7, 0xac,
    0xe2, 0xef, 0xcd, 0xe9,
];

#[test]
fn multibase_roundtrip_is_identity() {
    let cid = Cid::compute(Codec::DagCbor, b"hello world");
    let encoded = cid.to_multibase(Base::Base32Lower);
    let decoded = Cid::from_multibase(&encoded).expect("multibase should decode");

    assert_eq!(encoded, HELLO_WORLD_DAG_CBOR);
    assert_eq!(decoded.to_multibase(Base::Base32Lower), encoded);
    assert_eq!(decoded.codec(), Codec::DagCbor);
    assert_eq!(decoded.digest(), cid.digest());
}

#[test]
fn binary_roundtrip_is_identity() {
    let cid = Cid::compute(Codec::DagCbor, b"hello world");
    let encoded = cid.to_bytes();
    let (decoded, consumed) = Cid::from_bytes(&encoded).expect("bytes should decode");

    assert_eq!(consumed, encoded.len());
    assert_eq!(decoded.to_bytes(), encoded);
}

#[test]
fn from_bytes_reports_consumed_length_when_extra_bytes_follow() {
    let cid = Cid::compute(Codec::Raw, b"hello world");
    let mut encoded = cid.to_bytes();
    encoded.extend_from_slice(b"tail");

    let (decoded, consumed) = Cid::from_bytes(&encoded).expect("bytes should decode");

    assert_eq!(consumed, encoded.len() - 4);
    assert_eq!(decoded.to_bytes(), encoded[..consumed]);
}

#[test]
fn known_values_match_expected_strings() {
    assert_eq!(
        Cid::compute(Codec::DagCbor, b"").to_multibase(Base::Base32Lower),
        EMPTY_DAG_CBOR
    );
    assert_eq!(
        Cid::compute(Codec::DagCbor, b"hello world").to_multibase(Base::Base32Lower),
        HELLO_WORLD_DAG_CBOR
    );
}

#[test]
fn known_value_matches_expected_binary_structure() {
    assert_eq!(
        Cid::compute(Codec::DagCbor, b"hello world").to_bytes(),
        HELLO_WORLD_DAG_CBOR_BYTES
    );
}

#[test]
fn cid_is_hashable() {
    let cid = Cid::compute(Codec::DagCbor, b"hello world");
    let mut map = HashMap::new();
    map.insert(cid.clone(), "value");

    assert_eq!(map.get(&cid), Some(&"value"));
}

#[test]
fn rejects_cidv0_binary_multihash() {
    let bytes = [
        0x12, 0x20, 0xb9, 0x4d, 0x27, 0xb9, 0x93, 0x4d, 0x3e, 0x08, 0xa5, 0x2e, 0x52, 0xd7, 0xda,
        0x7d, 0xab, 0xfa, 0xc4, 0x84, 0xef, 0xe3, 0x7a, 0x53, 0x80, 0xee, 0x90, 0x88, 0xf7, 0xac,
        0xe2, 0xef, 0xcd, 0xe9,
    ];

    assert_eq!(Cid::from_bytes(&bytes), Err(CidError::InvalidVersion(0x12)));
}

#[test]
fn rejects_unsupported_codec() {
    let bytes = [
        0x01, 0x70, 0x12, 0x20, 0xb9, 0x4d, 0x27, 0xb9, 0x93, 0x4d, 0x3e, 0x08, 0xa5, 0x2e, 0x52,
        0xd7, 0xda, 0x7d, 0xab, 0xfa, 0xc4, 0x84, 0xef, 0xe3, 0x7a, 0x53, 0x80, 0xee, 0x90, 0x88,
        0xf7, 0xac, 0xe2, 0xef, 0xcd, 0xe9,
    ];

    assert_eq!(
        Cid::from_bytes(&bytes),
        Err(CidError::UnsupportedCodec(0x70))
    );
}

#[test]
fn rejects_unsupported_hash_code() {
    let bytes = [
        0x01, 0x71, 0x13, 0x20, 0xb9, 0x4d, 0x27, 0xb9, 0x93, 0x4d, 0x3e, 0x08, 0xa5, 0x2e, 0x52,
        0xd7, 0xda, 0x7d, 0xab, 0xfa, 0xc4, 0x84, 0xef, 0xe3, 0x7a, 0x53, 0x80, 0xee, 0x90, 0x88,
        0xf7, 0xac, 0xe2, 0xef, 0xcd, 0xe9,
    ];

    assert_eq!(
        Cid::from_bytes(&bytes),
        Err(CidError::UnsupportedHashCode(0x13))
    );
}

#[test]
fn rejects_invalid_digest_length() {
    let bytes = [
        0x01, 0x71, 0x12, 0x1f, 0xb9, 0x4d, 0x27, 0xb9, 0x93, 0x4d, 0x3e, 0x08, 0xa5, 0x2e, 0x52,
        0xd7, 0xda, 0x7d, 0xab, 0xfa, 0xc4, 0x84, 0xef, 0xe3, 0x7a, 0x53, 0x80, 0xee, 0x90, 0x88,
        0xf7, 0xac, 0xe2, 0xef, 0xcd,
    ];

    assert_eq!(Cid::from_bytes(&bytes), Err(CidError::InvalidDigestLength));
}

#[test]
fn rejects_invalid_multibase() {
    assert_eq!(
        Cid::from_multibase("Qmabcdef"),
        Err(CidError::InvalidMultibase)
    );
    assert_eq!(
        Cid::from_multibase("bafyreifzjut3te2nhyekklss27nh3k72ysco7y32koao5eei66wof36n5!"),
        Err(CidError::InvalidMultibase)
    );
}
