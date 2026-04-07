use crate::types::{
    js_error, nip19_to_json, parse_event_id, parse_event_with_pubkey, parse_naddr, parse_nevent,
    parse_nprofile, parse_public_key, parse_secret_key, parse_signed_event, parse_unsigned_event,
    parse_xonly_public_key, stringify_signed_event,
};
use bech32::{ToBase32, Variant};
use wasm_bindgen::prelude::*;

#[wasm_bindgen]
pub fn encode_npub(hex: &str) -> Result<String, JsValue> {
    let pubkey = parse_xonly_public_key(hex)?;
    neco_secp::nip19::encode_npub(&pubkey).map_err(js_error)
}

#[wasm_bindgen]
pub fn encode_note(hex: &str) -> Result<String, JsValue> {
    let id = parse_event_id(hex)?;
    neco_secp::nip19::encode_note(&id).map_err(js_error)
}

#[wasm_bindgen]
pub fn encode_nevent(json: &str) -> Result<String, JsValue> {
    let event = parse_nevent(json)?;
    neco_secp::nip19::encode_nevent(&event).map_err(js_error)
}

#[wasm_bindgen]
pub fn encode_nprofile(json: &str) -> Result<String, JsValue> {
    let profile = parse_nprofile(json)?;
    neco_secp::nip19::encode_nprofile(&profile).map_err(js_error)
}

#[wasm_bindgen]
pub fn encode_naddr(json: &str) -> Result<String, JsValue> {
    let addr = parse_naddr(json)?;
    neco_secp::nip19::encode_naddr(&addr).map_err(js_error)
}

#[wasm_bindgen]
pub fn encode_nsec(secret_hex: &str) -> Result<String, JsValue> {
    let secret = parse_secret_key(secret_hex)?;
    neco_secp::nip19::encode_nsec(&secret).map_err(js_error)
}

#[wasm_bindgen]
pub fn decode_nsec(nsec: &str) -> Result<String, JsValue> {
    match neco_secp::nip19::decode(nsec).map_err(js_error)? {
        neco_secp::Nip19::Nsec(secret) => Ok(secret.to_hex()),
        _ => Err(JsValue::from_str("expected nsec string")),
    }
}

#[wasm_bindgen]
pub fn encode_lnurl(url: &str) -> Result<String, JsValue> {
    bech32::encode("lnurl", url.as_bytes().to_base32(), Variant::Bech32).map_err(js_error)
}

#[wasm_bindgen]
pub fn decode_bech32(bech32: &str) -> Result<String, JsValue> {
    let decoded = neco_secp::nip19::decode(bech32).map_err(js_error)?;
    nip19_to_json(decoded, false)
}

#[wasm_bindgen]
pub fn generate_secret_key() -> Result<String, JsValue> {
    Err(JsValue::from_str(
        "plaintext secret export is disabled; use vanity mining output if needed",
    ))
}

#[wasm_bindgen]
pub fn derive_public_key(secret_hex: &str) -> Result<String, JsValue> {
    let secret = parse_secret_key(secret_hex)?;
    secret
        .xonly_public_key()
        .map(|pubkey| pubkey.to_hex())
        .map_err(js_error)
}

#[wasm_bindgen]
pub fn derive_public_key_sec1(secret_hex: &str) -> Result<String, JsValue> {
    let secret = parse_secret_key(secret_hex)?;
    secret
        .public_key()
        .map(|pubkey| pubkey.to_hex())
        .map_err(js_error)
}

#[wasm_bindgen]
pub fn parse_public_key_hex(public_key_hex: &str) -> Result<String, JsValue> {
    let public = parse_public_key(public_key_hex)?;
    Ok(public.to_hex())
}

#[wasm_bindgen]
pub fn finalize_event(json: &str, secret_hex: &str) -> Result<String, JsValue> {
    let event = parse_unsigned_event(json)?;
    let secret = parse_secret_key(secret_hex)?;
    let signed = neco_secp::nostr::finalize_event(event, &secret).map_err(js_error)?;
    stringify_signed_event(&signed)
}

#[wasm_bindgen]
pub fn verify_event(json: &str) -> Result<bool, JsValue> {
    let event = parse_signed_event(json)?;
    Ok(neco_secp::nostr::verify_event(&event).is_ok())
}

#[wasm_bindgen]
pub fn serialize_event(json: &str) -> Result<String, JsValue> {
    let (pubkey, event) = parse_event_with_pubkey(json)?;
    neco_secp::nostr::serialize_event(&pubkey, &event).map_err(js_error)
}

#[wasm_bindgen]
pub fn get_event_hash(json: &str) -> Result<String, JsValue> {
    let (pubkey, event) = parse_event_with_pubkey(json)?;
    neco_secp::nostr::compute_event_id(&pubkey, &event)
        .map(|id| id.to_hex())
        .map_err(js_error)
}

#[wasm_bindgen]
pub fn validate_auth_event(
    json: &str,
    challenge: &str,
    relay_url: &str,
) -> Result<String, JsValue> {
    let event = parse_signed_event(json)?;
    neco_secp::nip42::validate_auth_event(&event, challenge, relay_url)
        .map(|pubkey| pubkey.to_hex())
        .map_err(js_error)
}

#[cfg(test)]
mod tests {
    use super::*;
    use neco_secp::{nip19, EventId, NAddr, NEvent, NProfile, NRelay, SecretKey, UnsignedEvent};
    use serde_json::Value;

    #[test]
    fn finalize_and_verify_event_roundtrip() {
        let secret = SecretKey::generate().expect("secret");
        let unsigned = r#"{"created_at":10,"kind":1,"tags":[],"content":"hello"}"#;
        let signed = finalize_event(unsigned, &secret.to_hex()).expect("finalize");
        assert!(verify_event(&signed).expect("verify"));
    }

    #[test]
    fn finalize_event_matches_core_output() {
        // wasm binding は BIP-340 推奨のランダム aux_rand を使う通常経路で動作するため、
        // sig のバイト列は呼び出しごとに異なる。よって wasm 経路と core 経路の等価性は
        // 「id/pubkey/created_at/kind/tags/content が一致し、かつ両方とも verify OK」
        // という意味不変量で検証する。決定モードそのものの再現性は neco-secp 側の
        // finalize_event_deterministic_is_reproducible テストで担保している。
        let secret = SecretKey::from_bytes([0x11; 32]).expect("secret");
        let unsigned = UnsignedEvent {
            created_at: 1_700_000_000,
            kind: 1,
            tags: vec![vec!["p".to_string(), "abc".to_string()]],
            content: "hello".to_string(),
        };
        let json = r#"{"created_at":1700000000,"kind":1,"tags":[["p","abc"]],"content":"hello"}"#;

        let wasm_signed = finalize_event(json, &secret.to_hex()).expect("wasm finalize");
        let wasm_signed = neco_secp::nostr::parse_signed_event(&wasm_signed).expect("parse wasm");
        let core_signed = neco_secp::nostr::finalize_event(unsigned, &secret).expect("core");

        assert_eq!(wasm_signed.id, core_signed.id);
        assert_eq!(wasm_signed.pubkey, core_signed.pubkey);
        assert_eq!(wasm_signed.created_at, core_signed.created_at);
        assert_eq!(wasm_signed.kind, core_signed.kind);
        assert_eq!(wasm_signed.tags, core_signed.tags);
        assert_eq!(wasm_signed.content, core_signed.content);
        neco_secp::nostr::verify_event(&wasm_signed).expect("verify wasm");
        neco_secp::nostr::verify_event(&core_signed).expect("verify core");
    }

    #[test]
    fn wasm_matches_known_nostr_tools_fixture() {
        let secret = SecretKey::from_hex(
            "d217c1ff2f8a65c3e3a1740db3b9f58b\
             8c848bb45e26d00ed4714e4a0f4ceecf",
        )
        .expect("secret");
        let pubkey = secret.xonly_public_key().expect("pubkey");
        let unsigned_json =
            r#"{"created_at":1617932115,"kind":1,"tags":[],"content":"Hello, world!"}"#;
        let with_pubkey = format!(
            r#"{{"pubkey":"{}","created_at":1617932115,"kind":1,"tags":[],"content":"Hello, world!"}}"#,
            pubkey.to_hex()
        );

        let finalized = finalize_event(unsigned_json, &secret.to_hex()).expect("finalize");
        let serialized = serialize_event(&with_pubkey).expect("serialize");
        let hash = get_event_hash(&with_pubkey).expect("hash");
        let decoded_npub =
            decode_bech32("npub1dtc0nhjc3uk98nkuhgnvtcjq9cxs4fjwc768e8udj76mc43t4d0sw73h32")
                .expect("decode npub");
        let decoded_npub: Value = serde_json::from_str(&decoded_npub).expect("npub json");
        let decoded_nprofile = decode_bech32(
            "nprofile1qy28wumn8ghj7un9d3shjtnyv9kh2uewd9hsz9mhwden5te0dehhxarj9ejhsctdwpkx2tnrdaksqgr27ruauky093fuah96ymz7yspwp592vnk8k37flrvhkk79v2attuwkrkwx",
        )
        .expect("decode nprofile");
        let decoded_nprofile: Value = serde_json::from_str(&decoded_nprofile).expect("json");

        let finalized: Value = serde_json::from_str(&finalized).expect("signed json");
        assert_eq!(
            serialized,
            r#"[0,"6af0f9de588f2c53cedcba26c5e2402e0d0aa64ec7b47c9f8d97b5bc562bab5f",1617932115,1,[],"Hello, world!"]"#
        );
        assert_eq!(
            hash,
            "b2a44af84ca99b14820ae91c44e1ef0908f8aadc4e10620a6e6caa344507f03c"
        );
        assert_eq!(
            finalized["id"],
            "b2a44af84ca99b14820ae91c44e1ef0908f8aadc4e10620a6e6caa344507f03c"
        );
        assert_eq!(finalized["sig"].as_str().expect("sig").len(), 128);
        assert_eq!(
            decoded_npub,
            serde_json::json!({
                "kind": "npub",
                "pubkey_hex": "6af0f9de588f2c53cedcba26c5e2402e0d0aa64ec7b47c9f8d97b5bc562bab5f",
            })
        );
        assert_eq!(decoded_nprofile["kind"], "nprofile");
        assert_eq!(
            decoded_nprofile["pubkey_hex"],
            "6af0f9de588f2c53cedcba26c5e2402e0d0aa64ec7b47c9f8d97b5bc562bab5f"
        );
        assert_eq!(
            decoded_nprofile["relays"],
            serde_json::json!(["wss://relay.damus.io", "wss://nostr.example.com"])
        );
        assert!(verify_event(&finalized.to_string()).expect("verify"));
    }

    #[test]
    fn serialize_and_hash_match_core() {
        let secret = SecretKey::from_bytes([0x12; 32]).expect("secret");
        let pubkey = secret.xonly_public_key().expect("pubkey");
        let json = format!(
            r#"{{"pubkey":"{}","created_at":42,"kind":1,"tags":[["e","deadbeef"]],"content":"hi"}}"#,
            pubkey.to_hex()
        );
        let unsigned = UnsignedEvent {
            created_at: 42,
            kind: 1,
            tags: vec![vec!["e".to_string(), "deadbeef".to_string()]],
            content: "hi".to_string(),
        };

        let wasm_serialized = serialize_event(&json).expect("wasm serialize");
        let wasm_hash = get_event_hash(&json).expect("wasm hash");
        let core_serialized = neco_secp::nostr::serialize_event(&pubkey, &unsigned).expect("core");
        let core_hash = neco_secp::nostr::compute_event_id(&pubkey, &unsigned).expect("core id");

        assert_eq!(wasm_serialized, core_serialized);
        assert_eq!(wasm_hash, core_hash.to_hex());
    }

    #[test]
    fn decode_bech32_returns_full_nprofile_json() {
        let pubkey = SecretKey::from_bytes([0x13; 32])
            .expect("secret")
            .xonly_public_key()
            .expect("pubkey");
        let encoded = nip19::encode_nprofile(&NProfile {
            pubkey,
            relays: vec![
                "wss://relay.example".to_string(),
                "wss://relay2.example".to_string(),
            ],
        })
        .expect("encode");

        let decoded = decode_bech32(&encoded).expect("decode");
        let decoded: Value = serde_json::from_str(&decoded).expect("json");
        assert_eq!(decoded["kind"], "nprofile");
        assert_eq!(decoded["pubkey_hex"], pubkey.to_hex());
        assert_eq!(
            decoded["relays"],
            serde_json::json!(["wss://relay.example", "wss://relay2.example"])
        );
    }

    #[test]
    fn decode_bech32_returns_full_nevent_json() {
        let author = SecretKey::from_bytes([0x14; 32])
            .expect("secret")
            .xonly_public_key()
            .expect("author");
        let encoded = nip19::encode_nevent(&NEvent {
            id: EventId::from_bytes([0x22; 32]),
            relays: vec!["wss://relay.example".to_string()],
            author: Some(author),
            kind: Some(30_023),
        })
        .expect("encode");

        let decoded = decode_bech32(&encoded).expect("decode");
        let decoded: Value = serde_json::from_str(&decoded).expect("json");
        assert_eq!(decoded["kind"], "nevent");
        assert_eq!(decoded["id_hex"], "22".repeat(32));
        assert_eq!(decoded["author_hex"], author.to_hex());
        assert_eq!(decoded["event_kind"], 30_023);
        assert_eq!(
            decoded["relays"],
            serde_json::json!(["wss://relay.example"])
        );
    }

    #[test]
    fn decode_bech32_returns_full_naddr_and_nrelay_json() {
        let author = SecretKey::from_bytes([0x15; 32])
            .expect("secret")
            .xonly_public_key()
            .expect("author");
        let naddr = nip19::encode_naddr(&NAddr {
            identifier: "article".to_string(),
            relays: vec!["wss://relay.example".to_string()],
            author,
            kind: 30_023,
        })
        .expect("naddr");
        let nrelay = nip19::encode_nrelay(&NRelay {
            relay: "wss://relay.example".to_string(),
        })
        .expect("nrelay");

        let naddr: Value = serde_json::from_str(&decode_bech32(&naddr).expect("decode naddr"))
            .expect("naddr json");
        let nrelay: Value = serde_json::from_str(&decode_bech32(&nrelay).expect("decode nrelay"))
            .expect("nrelay json");

        assert_eq!(naddr["kind"], "naddr");
        assert_eq!(naddr["identifier"], "article");
        assert_eq!(naddr["author_hex"], author.to_hex());
        assert_eq!(naddr["event_kind"], 30_023);
        assert_eq!(nrelay["kind"], "nrelay");
        assert_eq!(nrelay["relay"], "wss://relay.example");
    }

    #[test]
    fn encode_profile_addr_and_nsec_match_core() {
        let secret = SecretKey::from_bytes([0x31; 32]).expect("secret");
        let pubkey = secret.xonly_public_key().expect("pubkey");

        let nprofile_json = format!(
            r#"{{"pubkey":"{}","relays":["wss://relay.example","wss://relay2.example"]}}"#,
            pubkey.to_hex()
        );
        let naddr_json = format!(
            r#"{{"identifier":"article","pubkey":"{}","kind":30023,"relays":["wss://relay.example"]}}"#,
            pubkey.to_hex()
        );

        let wasm_nprofile = encode_nprofile(&nprofile_json).expect("wasm nprofile");
        let wasm_naddr = encode_naddr(&naddr_json).expect("wasm naddr");
        let wasm_nsec = encode_nsec(&secret.to_hex()).expect("wasm nsec");

        let core_nprofile = neco_secp::nip19::encode_nprofile(&NProfile {
            pubkey,
            relays: vec![
                "wss://relay.example".to_string(),
                "wss://relay2.example".to_string(),
            ],
        })
        .expect("core nprofile");
        let core_naddr = neco_secp::nip19::encode_naddr(&NAddr {
            identifier: "article".to_string(),
            relays: vec!["wss://relay.example".to_string()],
            author: pubkey,
            kind: 30_023,
        })
        .expect("core naddr");
        let core_nsec = neco_secp::nip19::encode_nsec(&secret).expect("core nsec");

        assert_eq!(wasm_nprofile, core_nprofile);
        assert_eq!(wasm_naddr, core_naddr);
        assert_eq!(wasm_nsec, core_nsec);
        assert_eq!(
            decode_nsec(&wasm_nsec).expect("decode nsec"),
            secret.to_hex()
        );
    }

    #[test]
    fn validate_auth_event_returns_signer_pubkey() {
        let secret = SecretKey::from_bytes([0x16; 32]).expect("secret");
        let signed = neco_secp::nip42::create_auth_event(
            "challenge-123",
            "wss://relay.example.com",
            &secret,
            1_700_000_000,
        )
        .expect("create auth");
        let json = neco_secp::nostr::serialize_signed_event(&signed).expect("json");

        let validated =
            validate_auth_event(&json, "challenge-123", "wss://relay.example.com").expect("ok");
        assert_eq!(
            validated,
            secret.xonly_public_key().expect("pubkey").to_hex()
        );
    }

    #[test]
    fn encode_lnurl_matches_known_output() {
        let lnurl = encode_lnurl("https://example.com/.well-known/lnurlp/alice").expect("lnurl");
        assert_eq!(
            lnurl,
            "lnurl1dp68gurn8ghj7etcv9khqmr99e3k7mf09emk2mrv944kummhdchkcmn4wfk8qtmpd35kxeg9saevq"
        );
    }
}
