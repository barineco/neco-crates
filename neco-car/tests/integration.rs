use neco_car::{parse_v1, write_v1, CarError};
use neco_cid::{Cid, Codec};

fn make_cid(data: &[u8], codec: Codec) -> Cid {
    Cid::compute(codec, data)
}

#[test]
fn roundtrip_single_block() {
    let data = b"hello world";
    let cid = make_cid(data, Codec::Raw);
    let roots = vec![cid.clone()];
    let blocks = vec![(cid.clone(), data.as_slice())];

    let car_bytes = write_v1(&roots, &blocks).unwrap();
    let parsed = parse_v1(&car_bytes).unwrap();

    assert_eq!(parsed.roots(), std::slice::from_ref(&cid));
    assert_eq!(parsed.blocks().len(), 1);
    assert_eq!(parsed.blocks()[0].cid(), &cid);
    assert_eq!(parsed.blocks()[0].data(), data);
}

#[test]
fn roundtrip_multiple_blocks() {
    let data_a = b"block A";
    let data_b = b"block B";
    let data_c = b"block C";
    let cid_a = make_cid(data_a, Codec::Raw);
    let cid_b = make_cid(data_b, Codec::DagCbor);
    let cid_c = make_cid(data_c, Codec::Raw);

    let roots = vec![cid_a.clone(), cid_c.clone()];
    let blocks = vec![
        (cid_a.clone(), data_a.as_slice()),
        (cid_b.clone(), data_b.as_slice()),
        (cid_c.clone(), data_c.as_slice()),
    ];

    let car_bytes = write_v1(&roots, &blocks).unwrap();
    let parsed = parse_v1(&car_bytes).unwrap();

    assert_eq!(parsed.roots(), &[cid_a.clone(), cid_c.clone()]);
    assert_eq!(parsed.blocks().len(), 3);
    assert_eq!(parsed.blocks()[0].cid(), &cid_a);
    assert_eq!(parsed.blocks()[0].data(), data_a.as_slice());
    assert_eq!(parsed.blocks()[1].cid(), &cid_b);
    assert_eq!(parsed.blocks()[1].data(), data_b.as_slice());
    assert_eq!(parsed.blocks()[2].cid(), &cid_c);
    assert_eq!(parsed.blocks()[2].data(), data_c.as_slice());
}

#[test]
fn error_truncated_input() {
    let data = b"hello";
    let cid = make_cid(data, Codec::Raw);
    let roots = [cid.clone()];
    let car_bytes = write_v1(&roots, &[(cid, data.as_slice())]).unwrap();

    let truncated = &car_bytes[..5];
    let result = parse_v1(truncated);
    assert_eq!(result.unwrap_err(), CarError::UnexpectedEnd);
}

#[test]
fn error_unsupported_version() {
    let header = neco_cbor::CborValue::Map(vec![
        (
            neco_cbor::CborValue::Text("roots".into()),
            neco_cbor::CborValue::Array(vec![]),
        ),
        (
            neco_cbor::CborValue::Text("version".into()),
            neco_cbor::CborValue::Unsigned(2),
        ),
    ]);
    let header_bytes = neco_cbor::encode_dag(&header).unwrap();
    let mut car_bytes = Vec::new();
    encode_varint(header_bytes.len() as u64, &mut car_bytes);
    car_bytes.extend_from_slice(&header_bytes);

    let result = parse_v1(&car_bytes);
    assert_eq!(result.unwrap_err(), CarError::UnsupportedVersion(2));
}

#[test]
fn error_missing_roots_field() {
    let header = neco_cbor::CborValue::Map(vec![(
        neco_cbor::CborValue::Text("version".into()),
        neco_cbor::CborValue::Unsigned(1),
    )]);
    let header_bytes = neco_cbor::encode_dag(&header).unwrap();
    let mut car_bytes = Vec::new();
    encode_varint(header_bytes.len() as u64, &mut car_bytes);
    car_bytes.extend_from_slice(&header_bytes);

    let result = parse_v1(&car_bytes);
    assert_eq!(result.unwrap_err(), CarError::MissingHeaderField("roots"));
}

#[test]
fn error_empty_section() {
    let data = b"hello";
    let cid = make_cid(data, Codec::Raw);
    let roots = [cid.clone()];
    let car_bytes = write_v1(&roots, &[(cid, data.as_slice())]).unwrap();

    let mut bad_bytes = car_bytes.clone();
    bad_bytes.push(0x00);

    let result = parse_v1(&bad_bytes);
    assert_eq!(result.unwrap_err(), CarError::EmptySection);
}

#[test]
fn error_invalid_block_cid() {
    let data = b"hello";
    let cid = make_cid(data, Codec::Raw);
    let roots = [cid.clone()];
    let good_car = write_v1(&roots, &[(cid, data.as_slice())]).unwrap();

    let (header_len, varint_size) = read_varint(&good_car);
    let header_end = varint_size + header_len as usize;

    let mut bad_car = good_car[..header_end].to_vec();
    // version=99 は無効な CID
    let bad_cid_bytes = [99u8, 0x55, 0x12, 0x20];
    let section_len = bad_cid_bytes.len() + 3;
    encode_varint(section_len as u64, &mut bad_car);
    bad_car.extend_from_slice(&bad_cid_bytes);
    bad_car.extend_from_slice(b"abc");

    let result = parse_v1(&bad_car);
    assert!(matches!(result.unwrap_err(), CarError::InvalidBlockCid(_)));
}

#[test]
fn error_header_not_map() {
    let header = neco_cbor::CborValue::Array(vec![neco_cbor::CborValue::Unsigned(1)]);
    let header_bytes = neco_cbor::encode_dag(&header).unwrap();
    let mut car_bytes = Vec::new();
    encode_varint(header_bytes.len() as u64, &mut car_bytes);
    car_bytes.extend_from_slice(&header_bytes);

    let result = parse_v1(&car_bytes);
    assert_eq!(result.unwrap_err(), CarError::HeaderNotMap);
}

#[test]
fn error_varint_overflow() {
    // 10 bytes of continuation bits = overflow
    let bad_varint: Vec<u8> = vec![0xFF; 10];
    let result = parse_v1(&bad_varint);
    assert_eq!(result.unwrap_err(), CarError::VarintOverflow);
}

#[test]
fn error_missing_version_field() {
    let header = neco_cbor::CborValue::Map(vec![(
        neco_cbor::CborValue::Text("roots".into()),
        neco_cbor::CborValue::Array(vec![]),
    )]);
    let header_bytes = neco_cbor::encode_dag(&header).unwrap();
    let mut car_bytes = Vec::new();
    encode_varint(header_bytes.len() as u64, &mut car_bytes);
    car_bytes.extend_from_slice(&header_bytes);

    let result = parse_v1(&car_bytes);
    assert_eq!(result.unwrap_err(), CarError::MissingHeaderField("version"));
}

#[test]
fn error_roots_not_array() {
    let header = neco_cbor::CborValue::Map(vec![
        (
            neco_cbor::CborValue::Text("roots".into()),
            neco_cbor::CborValue::Text("not-an-array".into()),
        ),
        (
            neco_cbor::CborValue::Text("version".into()),
            neco_cbor::CborValue::Unsigned(1),
        ),
    ]);
    let header_bytes = neco_cbor::encode_dag(&header).unwrap();
    let mut car_bytes = Vec::new();
    encode_varint(header_bytes.len() as u64, &mut car_bytes);
    car_bytes.extend_from_slice(&header_bytes);

    let result = parse_v1(&car_bytes);
    assert_eq!(result.unwrap_err(), CarError::RootsNotArray);
}

#[test]
fn error_invalid_cid_link_in_roots() {
    // Root is a plain integer, not a tag 42 CID link
    let header = neco_cbor::CborValue::Map(vec![
        (
            neco_cbor::CborValue::Text("roots".into()),
            neco_cbor::CborValue::Array(vec![neco_cbor::CborValue::Unsigned(42)]),
        ),
        (
            neco_cbor::CborValue::Text("version".into()),
            neco_cbor::CborValue::Unsigned(1),
        ),
    ]);
    let header_bytes = neco_cbor::encode_dag(&header).unwrap();
    let mut car_bytes = Vec::new();
    encode_varint(header_bytes.len() as u64, &mut car_bytes);
    car_bytes.extend_from_slice(&header_bytes);

    let result = parse_v1(&car_bytes);
    assert_eq!(result.unwrap_err(), CarError::InvalidCidLink);
}

fn encode_varint(mut value: u64, out: &mut Vec<u8>) {
    loop {
        let lower = (value & 0x7f) as u8;
        value >>= 7;
        if value == 0 {
            out.push(lower);
            return;
        }
        out.push(lower | 0x80);
    }
}

fn read_varint(input: &[u8]) -> (u64, usize) {
    let mut value = 0u64;
    let mut shift = 0u32;
    for (i, &byte) in input.iter().enumerate() {
        value |= u64::from(byte & 0x7f) << shift;
        if byte & 0x80 == 0 {
            return (value, i + 1);
        }
        shift += 7;
    }
    panic!("invalid varint in test helper");
}
