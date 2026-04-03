use neco_json::{encode, parse, JsonValue};

#[test]
fn acceptance_1_and_2_crate_build_contract_is_visible_from_public_api() {
    // 受け入れ基準 1: no_std + alloc の公開 API で crate を利用できる。
    // 受け入れ基準 2: 外部依存ゼロでも外部クレートとして parse/encode/accessor を使える。
    let value = parse(br#"{"ok":true}"#).expect("public parse should work");
    assert_eq!(value.required_bool("ok"), Ok(true));
    assert_eq!(
        encode(&value).expect("public encode should work"),
        br#"{"ok":true}"#
    );
}

#[test]
fn acceptance_3_and_7_parse_complex_structure_and_read_with_accessors() {
    // 受け入れ基準 3: RFC 8259 の基本構文を含む複雑な JSON を正しく parse できる。
    // 受け入れ基準 7: accessor API で required/optional の用途をカバーできる。
    let payload = br#"{
        "repo":"did:plc:alice",
        "active":true,
        "score":42.5,
        "profile":{"handle":"alice.test","labels":["dev",null,"cat"]},
        "posts":[
            {"id":"p1","counts":{"likes":3,"replies":0}},
            {"id":"p2","counts":{"likes":5,"replies":1}}
        ]
    }"#;

    let value = parse(payload).expect("complex payload should parse");
    assert_eq!(value.required_str("repo"), Ok("did:plc:alice"));
    assert_eq!(value.required_bool("active"), Ok(true));
    assert_eq!(value.required_f64("score"), Ok(42.5));

    let profile = value.get("profile").expect("profile should exist");
    assert_eq!(profile.required_str("handle"), Ok("alice.test"));
    let labels = profile
        .required_array("labels")
        .expect("labels should be array");
    assert_eq!(labels.len(), 3);
    assert_eq!(labels[0].as_str(), Some("dev"));
    assert!(labels[1].is_null());
    assert_eq!(labels[2].as_str(), Some("cat"));

    let posts = value
        .required_array("posts")
        .expect("posts should be array");
    assert_eq!(posts.len(), 2);
    let first = posts[0].as_object().expect("post should be object");
    let first = JsonValue::Object(first.to_vec());
    assert_eq!(first.required_str("id"), Ok("p1"));
    let counts = first.get("counts").expect("counts should exist");
    assert_eq!(counts.required_f64("likes"), Ok(3.0));
    assert_eq!(counts.required_f64("replies"), Ok(0.0));
}

#[test]
fn acceptance_4_unicode_surrogate_pair_roundtrips_through_public_api() {
    // 受け入れ基準 4: \uXXXX サロゲートペアを decode できる。
    let value = parse(br#"{"emoji":"\uD83D\uDE80"}"#).expect("unicode payload should parse");
    assert_eq!(value.required_str("emoji"), Ok("🚀"));
}

#[test]
fn acceptance_5_roundtrip_preserves_complex_value_equivalence() {
    // 受け入れ基準 5: encode -> parse roundtrip が JsonValue の等価性を保つ。
    let value = JsonValue::Object(vec![
        (
            "meta".into(),
            JsonValue::Object(vec![
                ("version".into(), JsonValue::Number(1.0)),
                ("cursor".into(), JsonValue::Null),
            ]),
        ),
        (
            "items".into(),
            JsonValue::Array(vec![
                JsonValue::Object(vec![
                    ("id".into(), JsonValue::String("one".into())),
                    ("enabled".into(), JsonValue::Bool(true)),
                ]),
                JsonValue::Object(vec![
                    ("id".into(), JsonValue::String("two".into())),
                    ("enabled".into(), JsonValue::Bool(false)),
                    ("score".into(), JsonValue::Number(-1.5e-3)),
                ]),
            ]),
        ),
        (
            "message".into(),
            JsonValue::String("hello\u{0000}world".into()),
        ),
    ]);

    let encoded = encode(&value).expect("encode should succeed");
    let reparsed = parse(&encoded).expect("encoded value should parse");
    assert_eq!(reparsed, value);
    let items = reparsed
        .required_array("items")
        .expect("items should be array");
    assert_eq!(items.len(), 2);
}

#[test]
fn acceptance_6_depth_limit_is_enforced() {
    // 受け入れ基準 6: 深すぎる入力で NestingTooDeep を返す。
    let mut input = vec![b'['; 129];
    input.extend(vec![b']'; 129]);
    let error = parse(&input).expect_err("depth overflow should fail");
    assert!(matches!(
        error.kind,
        neco_json::ParseErrorKind::NestingTooDeep
    ));
}

#[test]
fn acceptance_8_end_to_end_atproto_like_payload_parses_and_accesses() {
    // 受け入れ基準 8: cargo test 対象の統合テストとして end-to-end を検証する。
    let response = br#"{
        "did":"did:plc:alice",
        "handle":"alice.test",
        "active":true,
        "viewer":{"muted":false,"blockedBy":null},
        "followersCount":128,
        "labels":[{"val":"verified","src":"did:plc:labeler"}]
    }"#;

    let value = parse(response).expect("ATProto-like response should parse");
    assert_eq!(value.required_str("did"), Ok("did:plc:alice"));
    assert_eq!(value.required_str("handle"), Ok("alice.test"));
    assert_eq!(value.required_bool("active"), Ok(true));
    assert_eq!(value.required_f64("followersCount"), Ok(128.0));

    let viewer = value.get("viewer").expect("viewer should exist");
    assert_eq!(viewer.required_bool("muted"), Ok(false));
    assert_eq!(viewer.optional_bool("blockedBy"), Ok(None));

    let labels = value
        .required_array("labels")
        .expect("labels should be array");
    assert_eq!(labels.len(), 1);
    let label = labels[0].as_object().expect("label should be object");
    let label = JsonValue::Object(label.to_vec());
    assert_eq!(label.required_str("val"), Ok("verified"));
    assert_eq!(label.required_str("src"), Ok("did:plc:labeler"));
}
