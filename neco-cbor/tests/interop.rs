//! ATProto data-model interop tests (DAG-CBOR round-trip + CID verification).

use neco_cbor::{decode_dag, encode_dag, CborValue};
use neco_cid::{Base, Cid, Codec};
use neco_json::{parse, JsonValue};

const FIXTURES: &str = include_str!("atproto-interop/data-model-fixtures.json");

// --- base64 decoder (standard, no padding required) ---

fn decode_base64(input: &str) -> Vec<u8> {
    const TABLE: [u8; 128] = {
        let mut t = [0xffu8; 128];
        let mut i = 0u8;
        while i < 26 {
            t[(b'A' + i) as usize] = i;
            t[(b'a' + i) as usize] = i + 26;
            i += 1;
        }
        let mut d = 0u8;
        while d < 10 {
            t[(b'0' + d) as usize] = d + 52;
            d += 1;
        }
        t[b'+' as usize] = 62;
        t[b'/' as usize] = 63;
        t
    };

    let input = input.trim_end_matches('=');
    let mut out = Vec::with_capacity(input.len() * 3 / 4);
    let mut buf = 0u32;
    let mut bits = 0u8;

    for &b in input.as_bytes() {
        let val = TABLE[b as usize];
        assert_ne!(val, 0xff, "invalid base64 char: {}", b as char);
        buf = (buf << 6) | val as u32;
        bits += 6;
        if bits >= 8 {
            bits -= 8;
            out.push((buf >> bits) as u8);
        }
    }
    out
}

fn json_get<'a>(value: &'a JsonValue, key: &str) -> &'a JsonValue {
    value
        .get(key)
        .unwrap_or_else(|| panic!("missing key: {key}"))
}

fn json_str<'a>(value: &'a JsonValue, key: &str) -> &'a str {
    json_get(value, key)
        .as_str()
        .unwrap_or_else(|| panic!("expected string for key: {key}"))
}

// --- fixture-level assertions ---

/// 各 fixture の JSON 値と DAG-CBOR デコード結果を再帰的に比較し、
/// `$link` → Tag(42, ...) および `$bytes` → Bytes(...) の対応を検証する。
fn assert_json_cbor_structure(json: &JsonValue, cbor: &CborValue) {
    match json {
        JsonValue::Object(fields) => {
            // `{"$link": "..."}` は DAG-CBOR では Tag(42, Bytes(...))
            if let Some(link_val) = json.get("$link") {
                let link_str = link_val.as_str().expect("$link must be string");
                let (tag, inner) = cbor.as_tag().expect("$link must map to Tag");
                assert_eq!(tag, 42, "$link must be tag 42");
                let cid_bytes = inner.as_bytes().expect("tag 42 inner must be bytes");
                // CID bytes の先頭は 0x00 prefix、残りが CID binary
                assert_eq!(cid_bytes[0], 0x00, "CID link must start with 0x00");
                let cid = Cid::from_bytes(&cid_bytes[1..])
                    .expect("CID parse failed")
                    .0;
                assert_eq!(
                    cid.to_multibase(Base::Base32Lower),
                    link_str,
                    "CID multibase mismatch"
                );
                return;
            }
            // `{"$bytes": "..."}` は DAG-CBOR では Bytes(...)
            if let Some(bytes_val) = json.get("$bytes") {
                let _b64 = bytes_val.as_str().expect("$bytes must be string");
                let decoded_bytes = decode_base64(_b64);
                let cbor_bytes = cbor.as_bytes().expect("$bytes must map to Bytes");
                assert_eq!(
                    cbor_bytes,
                    decoded_bytes.as_slice(),
                    "$bytes content mismatch"
                );
                return;
            }
            // 通常のオブジェクト
            let cbor_map = cbor.as_map().expect("expected CBOR map");
            assert_eq!(fields.len(), cbor_map.len(), "map length mismatch");
            for (key, json_val) in fields {
                let cbor_val = cbor
                    .get(key)
                    .unwrap_or_else(|| panic!("missing CBOR key: {key}"));
                assert_json_cbor_structure(json_val, cbor_val);
            }
        }
        JsonValue::Array(items) => {
            let cbor_arr = cbor.as_array().expect("expected CBOR array");
            assert_eq!(items.len(), cbor_arr.len(), "array length mismatch");
            for (json_item, cbor_item) in items.iter().zip(cbor_arr.iter()) {
                assert_json_cbor_structure(json_item, cbor_item);
            }
        }
        JsonValue::String(s) => {
            let cbor_text = cbor.as_text().expect("expected CBOR text");
            assert_eq!(cbor_text, s.as_str(), "text mismatch");
        }
        JsonValue::Number(n) => {
            // fixture の数値はすべて非負整数
            let v = *n as u64;
            let cbor_u = cbor.as_unsigned().expect("expected CBOR unsigned");
            assert_eq!(cbor_u, v, "integer mismatch");
        }
        JsonValue::Bool(b) => {
            let cbor_b = cbor.as_bool().expect("expected CBOR bool");
            assert_eq!(cbor_b, *b, "bool mismatch");
        }
        JsonValue::Null => {
            assert!(cbor.is_null(), "expected CBOR null");
        }
    }
}

#[test]
fn atproto_data_model_roundtrip_and_cid() {
    let fixtures = parse(FIXTURES.as_bytes()).expect("fixture JSON parse failed");
    let items = fixtures.as_array().expect("fixtures must be array");
    assert_eq!(items.len(), 3, "expected 3 fixtures");

    for (i, fixture) in items.iter().enumerate() {
        let cbor_b64 = json_str(fixture, "cbor_base64");
        let expected_cid = json_str(fixture, "cid");
        let json_val = json_get(fixture, "json");

        // 1. base64 → bytes → decode_dag
        let cbor_bytes = decode_base64(cbor_b64);
        let decoded = decode_dag(&cbor_bytes)
            .unwrap_or_else(|e| panic!("fixture {i}: decode_dag failed: {e}"));

        // 2. encode_dag round-trip
        let re_encoded =
            encode_dag(&decoded).unwrap_or_else(|e| panic!("fixture {i}: encode_dag failed: {e}"));
        assert_eq!(
            re_encoded, cbor_bytes,
            "fixture {i}: DAG-CBOR round-trip mismatch"
        );

        // 3. CID 検証
        let cid = Cid::compute(Codec::DagCbor, &cbor_bytes);
        assert_eq!(
            cid.to_multibase(Base::Base32Lower),
            expected_cid,
            "fixture {i}: CID mismatch"
        );

        // 4. JSON 構造と CBOR 構造の対応検証 ($link, $bytes)
        assert_json_cbor_structure(json_val, &decoded);
    }
}
