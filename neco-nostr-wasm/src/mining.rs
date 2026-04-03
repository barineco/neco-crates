use crate::types::{js_error, public_key_json};
use serde_json::{json, to_string, Value};
#[cfg(target_arch = "wasm32")]
use std::arch::wasm32::*;
use wasm_bindgen::prelude::*;

fn bundle_public_json(bundle: &neco_secp::KeyBundle) -> Result<Value, JsValue> {
    public_key_json(bundle.secret())
}

fn bundle_secret_json(bundle: &neco_secp::KeyBundle) -> Result<Value, JsValue> {
    let mut value = bundle_public_json(bundle)?;
    value["secret_hex"] = Value::String(bundle.secret().to_hex());
    Ok(value)
}

#[cfg(any(test, target_arch = "wasm32"))]
const BECH32_CHARSET: &[u8; 32] = b"qpzry9x8gf2tvdw0s3jn54khce6mua7l";

#[cfg(any(test, target_arch = "wasm32"))]
fn bech32_prefix_value(byte: u8) -> Result<u8, neco_secp::SecpError> {
    match byte {
        b'q' => Ok(0),
        b'p' => Ok(1),
        b'z' => Ok(2),
        b'r' => Ok(3),
        b'y' => Ok(4),
        b'9' => Ok(5),
        b'x' => Ok(6),
        b'8' => Ok(7),
        b'g' => Ok(8),
        b'f' => Ok(9),
        b'2' => Ok(10),
        b't' => Ok(11),
        b'v' => Ok(12),
        b'd' => Ok(13),
        b'w' => Ok(14),
        b'0' => Ok(15),
        b's' => Ok(16),
        b'3' => Ok(17),
        b'j' => Ok(18),
        b'n' => Ok(19),
        b'5' => Ok(20),
        b'4' => Ok(21),
        b'k' => Ok(22),
        b'h' => Ok(23),
        b'c' => Ok(24),
        b'e' => Ok(25),
        b'6' => Ok(26),
        b'm' => Ok(27),
        b'u' => Ok(28),
        b'a' => Ok(29),
        b'7' => Ok(30),
        b'l' => Ok(31),
        _ => Err(neco_secp::SecpError::InvalidNip19(
            "invalid npub vanity prefix",
        )),
    }
}

#[cfg(any(test, target_arch = "wasm32"))]
fn bech32_prefix_to_byte_pattern(prefix: &str) -> Result<Vec<u8>, neco_secp::SecpError> {
    let mut pattern = Vec::new();
    let mut acc = 0u32;
    let mut bits = 0u32;

    for &byte in prefix.as_bytes() {
        let value = u32::from(bech32_prefix_value(byte)?);
        acc = (acc << 5) | value;
        bits += 5;

        while bits >= 8 {
            bits -= 8;
            pattern.push(u8::try_from((acc >> bits) & 0xff).expect("masked to one byte"));
        }
    }

    Ok(pattern)
}

#[cfg(test)]
fn xonly_matches_pattern_scalar(xonly: &[u8; 32], pattern: &[u8]) -> bool {
    &xonly[..pattern.len()] == pattern
}

#[cfg(any(test, target_arch = "wasm32"))]
fn count_npub_prefix_matches_local(
    xonly_bytes: &[u8; 32],
    prefix: &str,
) -> Result<usize, neco_secp::SecpError> {
    let prefix_bytes = prefix.as_bytes();
    if prefix_bytes.is_empty() {
        return Ok(0);
    }

    for &byte in prefix_bytes {
        bech32_prefix_value(byte)?;
    }

    let mut matched = 0usize;
    let mut acc = 0u16;
    let mut bits = 0u8;

    for &byte in xonly_bytes {
        acc = (acc << 8) | u16::from(byte);
        bits += 8;

        while bits >= 5 && matched < prefix_bytes.len() {
            bits -= 5;
            let value = ((acc >> bits) & 0x1f) as usize;
            if BECH32_CHARSET[value] != prefix_bytes[matched] {
                return Ok(matched);
            }
            matched += 1;
        }

        if matched == prefix_bytes.len() {
            return Ok(matched);
        }
    }

    if bits > 0 && matched < prefix_bytes.len() {
        let value = ((acc << (5 - bits)) & 0x1f) as usize;
        if BECH32_CHARSET[value] == prefix_bytes[matched] {
            matched += 1;
        }
    }

    Ok(matched)
}

#[cfg(target_arch = "wasm32")]
fn xonly_matches_pattern_simd(xonly: &[u8; 32], pattern: &[u8]) -> bool {
    if pattern.is_empty() {
        return true;
    }

    let mut offset = 0usize;

    unsafe {
        while offset + 16 <= pattern.len() {
            let left = v128_load(xonly.as_ptr().add(offset) as *const v128);
            let right = v128_load(pattern.as_ptr().add(offset) as *const v128);
            let cmp = i8x16_eq(left, right);
            if i8x16_bitmask(cmp) != 0xffff {
                return false;
            }
            offset += 16;
        }
    }

    xonly[offset..pattern.len()] == pattern[offset..]
}

#[cfg(target_arch = "wasm32")]
fn mine_vanity_batch_wasm32(prefix: &str, max_attempts: u32) -> Result<String, JsValue> {
    let pattern = bech32_prefix_to_byte_pattern(prefix).map_err(js_error)?;

    for _ in 0..max_attempts {
        let bundle = neco_secp::KeyBundle::generate().map_err(js_error)?;
        if !xonly_matches_pattern_simd(&bundle.xonly_public_key().to_bytes(), &pattern) {
            continue;
        }

        let npub = bundle.npub().map_err(js_error)?;
        if npub[5..].starts_with(prefix) {
            return to_string(&bundle_secret_json(&bundle)?).map_err(js_error);
        }
    }

    Err(js_error(neco_secp::SecpError::ExhaustedAttempts))
}

#[cfg(target_arch = "wasm32")]
fn mine_vanity_with_candidates_wasm32(
    prefix: &str,
    max_attempts: u32,
    top_k: u32,
) -> Result<String, JsValue> {
    if top_k == 0 {
        return Ok("[]".to_owned());
    }

    let prefilter_chars = prefix.len().min(4);
    let prefilter_pattern = if prefilter_chars == 0 {
        None
    } else {
        Some(bech32_prefix_to_byte_pattern(&prefix[..prefilter_chars]).map_err(js_error)?)
    };

    let mut candidates: Vec<(neco_secp::KeyBundle, usize)> = Vec::new();
    let mut min_matched = 0usize;

    for _ in 0..max_attempts {
        let bundle = neco_secp::KeyBundle::generate().map_err(js_error)?;
        let xonly = bundle.xonly_public_key().to_bytes();

        if let Some(pattern) = prefilter_pattern.as_ref() {
            if !xonly_matches_pattern_simd(&xonly, pattern) {
                continue;
            }
        }

        let matched = count_npub_prefix_matches_local(&xonly, prefix).map_err(js_error)?;
        if matched == 0 {
            continue;
        }

        if matched == prefix.len() || matched > min_matched || candidates.len() < top_k as usize {
            candidates.push((bundle, matched));
        }

        if candidates.len() > top_k as usize {
            candidates.sort_by(|a, b| b.1.cmp(&a.1));
            candidates.truncate(top_k as usize);
            min_matched = candidates.last().map_or(0, |candidate| candidate.1);
        }
    }

    candidates.sort_by(|a, b| b.1.cmp(&a.1));
    candidates.truncate(top_k as usize);
    let items: Result<Vec<_>, _> = candidates
        .iter()
        .map(|(bundle, matched_len)| {
            let mut value = bundle_secret_json(bundle)?;
            value["matched_len"] = json!(matched_len);
            Ok::<Value, JsValue>(value)
        })
        .collect();
    to_string(&items?).map_err(js_error)
}

#[wasm_bindgen]
pub fn generate_keypair() -> Result<String, JsValue> {
    let bundle = neco_secp::KeyBundle::generate().map_err(js_error)?;
    to_string(&bundle_public_json(&bundle)?).map_err(js_error)
}

#[wasm_bindgen]
pub fn generate_keypairs_batch(count: u32) -> Result<String, JsValue> {
    let bundles = neco_secp::KeyBundle::generate_batch(count as usize).map_err(js_error)?;
    let items: Result<Vec<_>, _> = bundles.iter().map(bundle_public_json).collect();
    to_string(&items?).map_err(js_error)
}

#[wasm_bindgen]
pub fn mine_pow_batch(difficulty: u8, max_attempts: u32) -> Result<String, JsValue> {
    let (bundle, actual_difficulty) =
        neco_secp::mine_pow_best(difficulty, max_attempts.into()).map_err(js_error)?;
    let mut value = bundle_public_json(&bundle)?;
    value["difficulty"] = json!(actual_difficulty);
    to_string(&value).map_err(js_error)
}

#[wasm_bindgen]
pub fn mine_vanity_batch(prefix: &str, max_attempts: u32) -> Result<String, JsValue> {
    #[cfg(target_arch = "wasm32")]
    {
        mine_vanity_batch_wasm32(prefix, max_attempts)
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        let bundle = neco_secp::mine_vanity_npub(prefix, max_attempts.into()).map_err(js_error)?;
        to_string(&bundle_secret_json(&bundle)?).map_err(js_error)
    }
}

#[wasm_bindgen]
pub fn mine_vanity_with_candidates(
    prefix: &str,
    max_attempts: u32,
    top_k: u32,
) -> Result<String, JsValue> {
    #[cfg(target_arch = "wasm32")]
    {
        mine_vanity_with_candidates_wasm32(prefix, max_attempts, top_k)
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        let candidates =
            neco_secp::mine_vanity_npub_candidates(prefix, max_attempts.into(), top_k as usize)
                .map_err(js_error)?;
        let items: Result<Vec<_>, _> = candidates
            .iter()
            .map(|candidate| {
                let mut value = bundle_secret_json(candidate.bundle())?;
                value["matched_len"] = json!(candidate.matched_len());
                Ok::<Value, JsValue>(value)
            })
            .collect();
        to_string(&items?).map_err(js_error)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn prefix_pattern_matches_known_npub_prefix_bytes() {
        let bundle = neco_secp::KeyBundle::generate().expect("bundle");
        let xonly = bundle.xonly_public_key().to_bytes();
        let npub = bundle.npub().expect("npub");

        for prefix_len in 0..=12 {
            let prefix = &npub[5..5 + prefix_len];
            let pattern = bech32_prefix_to_byte_pattern(prefix).expect("pattern");
            assert!(xonly_matches_pattern_scalar(&xonly, &pattern));
        }
    }

    #[test]
    fn prefix_pattern_rejects_invalid_bech32_chars() {
        let err = bech32_prefix_to_byte_pattern("I").expect_err("invalid prefix");
        assert!(matches!(
            err,
            neco_secp::SecpError::InvalidNip19("invalid npub vanity prefix")
        ));
    }

    #[test]
    fn local_prefix_match_counter_matches_encoded_npub() {
        let bundle = neco_secp::KeyBundle::generate().expect("bundle");
        let xonly = bundle.xonly_public_key().to_bytes();
        let npub = bundle.npub().expect("npub");

        for prefix_len in 0..=16 {
            let prefix = &npub[5..5 + prefix_len];
            assert_eq!(
                count_npub_prefix_matches_local(&xonly, prefix).expect("matched"),
                prefix_len
            );
        }
    }

    #[test]
    fn scalar_prefix_match_rejects_mutated_pattern() {
        let bundle = neco_secp::KeyBundle::generate().expect("bundle");
        let xonly = bundle.xonly_public_key().to_bytes();
        let npub = bundle.npub().expect("npub");
        let mut pattern = bech32_prefix_to_byte_pattern(&npub[5..10]).expect("pattern");
        assert!(!pattern.is_empty());
        pattern[0] ^= 0b0000_0001;
        assert!(!xonly_matches_pattern_scalar(&xonly, &pattern));
    }

    #[test]
    fn generate_keypair_hides_secret() {
        let json = generate_keypair().expect("generate");
        let value: Value = serde_json::from_str(&json).expect("json");
        assert!(value.get("secret_hex").is_none());
        assert!(value.get("pubkey_hex").is_some());
    }

    #[test]
    fn vanity_result_exposes_secret_hex() {
        let json = mine_vanity_batch("", 1).expect("mine vanity");
        let value: Value = serde_json::from_str(&json).expect("json");
        assert!(value.get("secret_hex").is_some());
    }

    #[test]
    fn pow_result_hides_secret_and_reports_difficulty() {
        let json = mine_pow_batch(0, 1).expect("mine pow");
        let value: Value = serde_json::from_str(&json).expect("json");
        assert!(value.get("secret_hex").is_none());
        assert!(value.get("pubkey_hex").is_some());
        assert!(value.get("difficulty").is_some());
    }

    #[test]
    fn vanity_candidates_include_matched_len_and_secret_hex() {
        let json = mine_vanity_with_candidates("q", 10_000, 3).expect("candidates");
        let value: Value = serde_json::from_str(&json).expect("json");
        let items = value.as_array().expect("array");
        assert!(!items.is_empty());
        assert!(items[0].get("secret_hex").is_some());
        assert!(items[0].get("matched_len").is_some());
    }
}

#[cfg(all(test, target_arch = "wasm32"))]
mod wasm_tests {
    use super::*;
    use wasm_bindgen_test::*;

    #[wasm_bindgen_test]
    fn simd_prefix_match_agrees_with_scalar() {
        let bundle = neco_secp::KeyBundle::generate().expect("bundle");
        let xonly = bundle.xonly_public_key().to_bytes();
        let npub = bundle.npub().expect("npub");

        for prefix_len in 0..=20 {
            let prefix = &npub[5..5 + prefix_len];
            let pattern = bech32_prefix_to_byte_pattern(prefix).expect("pattern");
            assert_eq!(
                xonly_matches_pattern_simd(&xonly, &pattern),
                xonly_matches_pattern_scalar(&xonly, &pattern)
            );
        }
    }

    #[wasm_bindgen_test]
    fn simd_prefix_match_rejects_mutated_pattern() {
        let bundle = neco_secp::KeyBundle::generate().expect("bundle");
        let xonly = bundle.xonly_public_key().to_bytes();
        let npub = bundle.npub().expect("npub");
        let mut pattern = bech32_prefix_to_byte_pattern(&npub[5..15]).expect("pattern");
        assert!(!pattern.is_empty());
        pattern[0] ^= 0b0000_0001;

        assert_eq!(
            xonly_matches_pattern_simd(&xonly, &pattern),
            xonly_matches_pattern_scalar(&xonly, &pattern)
        );
        assert!(!xonly_matches_pattern_simd(&xonly, &pattern));
    }

    #[wasm_bindgen_test]
    fn wasm_candidates_contract_returns_sorted_matches() {
        let json = mine_vanity_with_candidates("q", 10_000, 3).expect("candidates");
        let value: Value = serde_json::from_str(&json).expect("json");
        let items = value.as_array().expect("array");
        assert!(!items.is_empty());
        for item in items {
            assert!(item.get("secret_hex").is_some());
            assert!(item.get("matched_len").is_some());
        }
        for pair in items.windows(2) {
            let left = pair[0]["matched_len"].as_u64().expect("left");
            let right = pair[1]["matched_len"].as_u64().expect("right");
            assert!(left >= right);
        }
    }
}
