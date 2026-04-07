//! ATProto signature interop tests (ES256K / secp256k1).

use neco_json::{parse, JsonValue};
use neco_secp::{EcdsaSignature, PublicKey, SecretKey};
use neco_sha2::Sha256;

const SIGNATURE_FIXTURES: &str = include_str!("atproto-interop/signature-fixtures.json");
const W3C_DIDKEY_K256: &str = include_str!("atproto-interop/w3c_didkey_K256.json");

// --- base64 decoder ---

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

// --- base58btc decoder ---

fn decode_base58btc(input: &str) -> Vec<u8> {
    const ALPHABET: &[u8] = b"123456789ABCDEFGHJKLMNPQRSTUVWXYZabcdefghijkmnopqrstuvwxyz";

    let leading_ones = input.bytes().take_while(|&b| b == b'1').count();

    let mut acc = Vec::new();
    for &ch in input.as_bytes() {
        let carry_init = ALPHABET
            .iter()
            .position(|&a| a == ch)
            .unwrap_or_else(|| panic!("invalid base58 char: {}", ch as char));
        let mut carry = carry_init;
        for byte in acc.iter_mut().rev() {
            carry += 58 * (*byte as usize);
            *byte = (carry % 256) as u8;
            carry /= 256;
        }
        while carry > 0 {
            acc.insert(0, (carry % 256) as u8);
            carry /= 256;
        }
    }

    let mut result = vec![0u8; leading_ones];
    result.extend_from_slice(&acc);
    result
}

// --- hex helpers ---

fn hex_encode(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut s = String::with_capacity(bytes.len() * 2);
    for &b in bytes {
        s.push(HEX[(b >> 4) as usize] as char);
        s.push(HEX[(b & 0x0f) as usize] as char);
    }
    s
}

// --- JSON helpers ---

fn json_str<'a>(value: &'a JsonValue, key: &str) -> &'a str {
    value
        .get(key)
        .and_then(|v| v.as_str())
        .unwrap_or_else(|| panic!("missing or non-string key: {key}"))
}

fn json_bool(value: &JsonValue, key: &str) -> bool {
    value
        .get(key)
        .and_then(|v| v.as_bool())
        .unwrap_or_else(|| panic!("missing or non-bool key: {key}"))
}

/// publicKeyMultibase (z + base58btc) → raw 33-byte SEC1 compressed public key
fn pubkey_from_multibase(multibase: &str) -> PublicKey {
    let without_prefix = multibase
        .strip_prefix('z')
        .expect("multibase must start with 'z' (base58btc)");
    let decoded = decode_base58btc(without_prefix);
    assert_eq!(
        decoded.len(),
        33,
        "expected 33-byte compressed SEC1 key, got {}",
        decoded.len()
    );
    PublicKey::from_sec1_bytes(&decoded).expect("invalid secp256k1 public key")
}

/// did:key (multicodec varint 0xe7 0x01 for secp256k1) → 33-byte SEC1 compressed public key
fn pubkey_from_did_key(did_key: &str) -> PublicKey {
    let multibase = did_key
        .strip_prefix("did:key:")
        .expect("expected did:key: prefix");
    let without_z = multibase
        .strip_prefix('z')
        .expect("did:key multibase must start with 'z'");
    let decoded = decode_base58btc(without_z);
    assert!(
        decoded.len() >= 2,
        "did:key decoded too short: {}",
        decoded.len()
    );
    // secp256k1 multicodec: varint 0xe7 0x01
    assert_eq!(
        decoded[0], 0xe7,
        "expected secp256k1 multicodec byte 0, got 0x{:02x}",
        decoded[0]
    );
    assert_eq!(
        decoded[1], 0x01,
        "expected secp256k1 multicodec byte 1, got 0x{:02x}",
        decoded[1]
    );
    let pubkey_bytes = &decoded[2..];
    assert_eq!(
        pubkey_bytes.len(),
        33,
        "expected 33-byte compressed SEC1 key, got {}",
        pubkey_bytes.len()
    );
    PublicKey::from_sec1_bytes(pubkey_bytes).expect("invalid secp256k1 public key")
}

#[test]
fn atproto_signature_verification_k256() {
    let fixtures = parse(SIGNATURE_FIXTURES.as_bytes()).expect("fixture JSON parse failed");
    let items = fixtures.as_array().expect("fixtures must be array");

    let mut tested = 0;
    for fixture in items {
        let algorithm = json_str(fixture, "algorithm");
        if algorithm != "ES256K" {
            continue;
        }

        let comment = json_str(fixture, "comment");
        let message_b64 = json_str(fixture, "messageBase64");
        let multibase = json_str(fixture, "publicKeyMultibase");
        let sig_b64 = json_str(fixture, "signatureBase64");
        let valid = json_bool(fixture, "validSignature");

        let tags = fixture
            .get("tags")
            .and_then(|v| v.as_array())
            .unwrap_or(&[]);
        let is_der = tags.iter().any(|t| t.as_str() == Some("der-encoded"));

        // 公開鍵: publicKeyMultibase は z + base58btc(33-byte SEC1)
        let pubkey = pubkey_from_multibase(multibase);

        // メッセージ → SHA-256
        let message = decode_base64(message_b64);
        let digest = Sha256::digest(&message);

        // 署名のデコード
        let sig_bytes = decode_base64(sig_b64);

        if is_der {
            // DER-encoded: 64 バイトではないので from_bytes に渡せない → 不正扱い
            assert_ne!(
                sig_bytes.len(),
                64,
                "{comment}: DER signature should not be 64 bytes"
            );
            assert!(!valid, "{comment}: DER signature should be invalid");
            tested += 1;
            continue;
        }

        assert_eq!(
            sig_bytes.len(),
            64,
            "{comment}: expected 64-byte signature, got {}",
            sig_bytes.len()
        );
        let mut sig_arr = [0u8; 64];
        sig_arr.copy_from_slice(&sig_bytes);
        let sig = EcdsaSignature::from_bytes(sig_arr);

        let result = pubkey.verify_ecdsa_prehash(digest, &sig);
        if valid {
            result.unwrap_or_else(|e| panic!("{comment}: expected valid, got error: {e}"));
        } else {
            result.expect_err(&format!("{comment}: expected invalid signature"));
        }

        tested += 1;
    }
    assert!(
        tested >= 3,
        "expected at least 3 K-256 fixtures, got {tested}"
    );
}

#[test]
fn w3c_didkey_k256_private_to_public() {
    let fixtures = parse(W3C_DIDKEY_K256.as_bytes()).expect("fixture JSON parse failed");
    let items = fixtures.as_array().expect("fixtures must be array");
    assert!(!items.is_empty(), "no w3c didkey K256 fixtures");

    for fixture in items {
        let priv_hex = json_str(fixture, "privateKeyBytesHex");
        let public_did_key = json_str(fixture, "publicDidKey");

        // did:key から ECDSA 公開鍵を抽出（multicodec prefix 付き）
        let expected_pk = pubkey_from_did_key(public_did_key);
        let expected_sec1 = expected_pk.to_sec1_bytes();

        // 秘密鍵 → 公開鍵
        // neco_secp::SecretKey::public_key() は Schnorr 用に even-y 正規化するため、
        // ECDSA 公開鍵と parity が異なる場合がある。
        // x 座標（SEC1 の先頭バイト以降 32 バイト）が一致すれば同じ点。
        let sk = SecretKey::from_hex(priv_hex).expect("invalid secret key hex");
        let pk = sk.public_key().expect("failed to derive public key");
        let pk_sec1 = pk.to_sec1_bytes();

        // x 座標の一致を検証（先頭 1 バイトは圧縮 prefix 02/03）
        assert_eq!(
            hex_encode(&pk_sec1[1..]),
            hex_encode(&expected_sec1[1..]),
            "x-coordinate mismatch for private key {priv_hex}"
        );

        // did:key から取得した公開鍵で ECDSA 署名検証が可能なことを確認
        // （鍵自体が有効な secp256k1 点であること）
        let _ = PublicKey::from_sec1_bytes(&expected_sec1).expect("did:key pubkey should be valid");
    }
}
