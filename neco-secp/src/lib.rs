//! Minimal secp256k1 and Nostr signing core.

use core::fmt;

use k256::ecdsa::{
    signature::hazmat::{
        PrehashSigner as EcdsaPrehashSigner, PrehashVerifier as EcdsaPrehashVerifier,
    },
    Signature as K256EcdsaSignature, SigningKey as EcdsaSigningKey,
    VerifyingKey as EcdsaVerifyingKey,
};
use k256::elliptic_curve::rand_core::OsRng;
use k256::elliptic_curve::scalar::IsHigh;
use k256::elliptic_curve::sec1::ToEncodedPoint;
use k256::schnorr::signature::hazmat::{
    PrehashSigner as SchnorrPrehashSigner, PrehashVerifier as SchnorrPrehashVerifier,
};
use k256::schnorr::{Signature, SigningKey, VerifyingKey};
#[cfg(feature = "serde")]
use serde::{Deserialize, Deserializer, Serialize, Serializer};
#[cfg(feature = "nostr")]
use sha2::Digest;
#[cfg(any(feature = "nostr", feature = "nip44"))]
use sha2::Sha256;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SecpError {
    InvalidSecretKey,
    InvalidPublicKey,
    InvalidSignature,
    ExhaustedAttempts,
    InvalidHex(&'static str),
    InvalidEvent(&'static str),
    InvalidNip19(&'static str),
    InvalidNip04(&'static str),
    InvalidNip44(&'static str),
    Json(String),
}

impl fmt::Display for SecpError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidSecretKey => f.write_str("invalid secret key"),
            Self::InvalidPublicKey => f.write_str("invalid public key"),
            Self::InvalidSignature => f.write_str("invalid signature"),
            Self::ExhaustedAttempts => f.write_str("exhausted attempts"),
            Self::InvalidHex(message) => f.write_str(message),
            Self::InvalidEvent(message) => f.write_str(message),
            Self::InvalidNip19(message) => f.write_str(message),
            Self::InvalidNip04(message) => f.write_str(message),
            Self::InvalidNip44(message) => f.write_str(message),
            Self::Json(error) => write!(f, "json error: {error}"),
        }
    }
}

impl std::error::Error for SecpError {}

#[cfg(feature = "nostr")]
impl From<neco_json::ParseError> for SecpError {
    fn from(value: neco_json::ParseError) -> Self {
        Self::Json(value.to_string())
    }
}

#[cfg(feature = "nostr")]
impl From<neco_json::EncodeError> for SecpError {
    fn from(value: neco_json::EncodeError) -> Self {
        Self::Json(value.to_string())
    }
}

fn hex_encode(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut s = String::with_capacity(bytes.len() * 2);
    for &b in bytes {
        s.push(HEX[(b >> 4) as usize] as char);
        s.push(HEX[(b & 0x0f) as usize] as char);
    }
    s
}

fn hex_decode(hex: &str) -> Result<Vec<u8>, SecpError> {
    let hex = hex.as_bytes();
    if hex.len() % 2 != 0 {
        return Err(SecpError::InvalidHex("odd length"));
    }
    let mut out = Vec::with_capacity(hex.len() / 2);
    for chunk in hex.chunks(2) {
        let high = hex_nibble(chunk[0])?;
        let low = hex_nibble(chunk[1])?;
        out.push((high << 4) | low);
    }
    Ok(out)
}

fn hex_nibble(b: u8) -> Result<u8, SecpError> {
    match b {
        b'0'..=b'9' => Ok(b - b'0'),
        b'a'..=b'f' => Ok(b - b'a' + 10),
        b'A'..=b'F' => Ok(b - b'A' + 10),
        _ => Err(SecpError::InvalidHex("invalid character")),
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SecretKey {
    bytes: [u8; 32],
}

impl SecretKey {
    pub fn generate() -> Result<Self, SecpError> {
        let signing_key = SigningKey::random(&mut OsRng);
        let mut bytes = [0u8; 32];
        bytes.copy_from_slice(&signing_key.to_bytes());
        Self::from_bytes(bytes)
    }

    pub fn from_hex(hex: &str) -> Result<Self, SecpError> {
        let bytes = hex_decode(hex)?;
        if bytes.len() != 32 {
            return Err(SecpError::InvalidHex("expected 64 hex characters"));
        }
        let mut arr = [0u8; 32];
        arr.copy_from_slice(&bytes);
        Self::from_bytes(arr)
    }

    pub fn from_bytes(bytes: [u8; 32]) -> Result<Self, SecpError> {
        let _ = SigningKey::from_bytes(&bytes).map_err(|_| SecpError::InvalidSecretKey)?;
        Ok(Self { bytes })
    }

    pub fn to_hex(&self) -> String {
        hex_encode(&self.bytes)
    }

    pub fn to_bytes(&self) -> [u8; 32] {
        self.bytes
    }

    pub fn public_key(&self) -> Result<PublicKey, SecpError> {
        let verifying = self.verifying_key()?;
        Ok(PublicKey {
            sec1_bytes: verifying
                .as_affine()
                .to_encoded_point(true)
                .as_bytes()
                .try_into()
                .map_err(|_| SecpError::InvalidPublicKey)?,
        })
    }

    pub fn xonly_public_key(&self) -> Result<XOnlyPublicKey, SecpError> {
        let verifying = self.verifying_key()?;
        Ok(XOnlyPublicKey {
            bytes: *verifying.to_bytes().as_ref(),
        })
    }

    pub fn sign_schnorr_prehash(&self, digest32: [u8; 32]) -> Result<SchnorrSignature, SecpError> {
        let signing_key = self.signing_key()?;
        let signature = SchnorrPrehashSigner::sign_prehash(&signing_key, &digest32)
            .map_err(|_| SecpError::InvalidSignature)?;
        Ok(SchnorrSignature {
            bytes: signature.to_bytes(),
        })
    }

    pub fn sign_ecdsa_prehash(&self, digest32: [u8; 32]) -> Result<EcdsaSignature, SecpError> {
        let signing_key = self.ecdsa_signing_key()?;
        let signature: K256EcdsaSignature =
            EcdsaPrehashSigner::sign_prehash(&signing_key, &digest32)
                .map_err(|_| SecpError::InvalidSignature)?;
        let signature = signature.normalize_s().unwrap_or(signature);
        Ok(EcdsaSignature {
            bytes: signature.to_bytes().into(),
        })
    }

    fn signing_key(&self) -> Result<SigningKey, SecpError> {
        SigningKey::from_bytes(&self.bytes).map_err(|_| SecpError::InvalidSecretKey)
    }

    fn ecdsa_signing_key(&self) -> Result<EcdsaSigningKey, SecpError> {
        EcdsaSigningKey::from_slice(&self.bytes).map_err(|_| SecpError::InvalidSecretKey)
    }

    fn verifying_key(&self) -> Result<VerifyingKey, SecpError> {
        let signing_key = self.signing_key()?;
        Ok(*signing_key.verifying_key())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PublicKey {
    sec1_bytes: [u8; 33],
}

impl PublicKey {
    pub fn from_hex(hex: &str) -> Result<Self, SecpError> {
        let bytes = hex_decode(hex)?;
        if bytes.len() != 33 {
            return Err(SecpError::InvalidHex("expected 66 hex characters"));
        }
        Self::from_sec1_bytes(&bytes)
    }

    pub fn from_sec1_bytes(bytes: &[u8]) -> Result<Self, SecpError> {
        let key =
            k256::PublicKey::from_sec1_bytes(bytes).map_err(|_| SecpError::InvalidPublicKey)?;
        let sec1 = key.to_sec1_bytes();
        let mut sec1_bytes = [0u8; 33];
        sec1_bytes.copy_from_slice(sec1.as_ref());
        Ok(Self { sec1_bytes })
    }

    pub fn to_hex(&self) -> String {
        hex_encode(&self.sec1_bytes)
    }

    pub fn to_sec1_bytes(&self) -> [u8; 33] {
        self.sec1_bytes
    }

    pub fn verify_ecdsa_prehash(
        &self,
        digest32: [u8; 32],
        sig: &EcdsaSignature,
    ) -> Result<(), SecpError> {
        let verifying_key = EcdsaVerifyingKey::from_sec1_bytes(&self.sec1_bytes)
            .map_err(|_| SecpError::InvalidPublicKey)?;
        let signature =
            K256EcdsaSignature::from_slice(&sig.bytes).map_err(|_| SecpError::InvalidSignature)?;
        if bool::from(signature.s().is_high()) {
            return Err(SecpError::InvalidSignature);
        }
        EcdsaPrehashVerifier::verify_prehash(&verifying_key, &digest32, &signature)
            .map_err(|_| SecpError::InvalidSignature)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct XOnlyPublicKey {
    bytes: [u8; 32],
}

impl XOnlyPublicKey {
    pub fn from_hex(hex: &str) -> Result<Self, SecpError> {
        let bytes = hex_decode(hex)?;
        if bytes.len() != 32 {
            return Err(SecpError::InvalidHex("expected 64 hex characters"));
        }
        let mut arr = [0u8; 32];
        arr.copy_from_slice(&bytes);
        Self::from_bytes(arr)
    }

    pub fn from_bytes(bytes: [u8; 32]) -> Result<Self, SecpError> {
        let _ = VerifyingKey::from_bytes(&bytes).map_err(|_| SecpError::InvalidPublicKey)?;
        Ok(Self { bytes })
    }

    pub fn to_hex(&self) -> String {
        hex_encode(&self.bytes)
    }

    pub fn to_bytes(&self) -> [u8; 32] {
        self.bytes
    }

    pub fn verify_schnorr_prehash(
        &self,
        digest32: [u8; 32],
        sig: &SchnorrSignature,
    ) -> Result<(), SecpError> {
        let verifying_key =
            VerifyingKey::from_bytes(&self.bytes).map_err(|_| SecpError::InvalidPublicKey)?;
        let signature =
            Signature::try_from(sig.bytes.as_slice()).map_err(|_| SecpError::InvalidSignature)?;
        SchnorrPrehashVerifier::verify_prehash(&verifying_key, &digest32, &signature)
            .map_err(|_| SecpError::InvalidSignature)
    }
}

#[cfg(feature = "serde")]
impl Serialize for XOnlyPublicKey {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(&self.to_hex())
    }
}

#[cfg(feature = "serde")]
impl<'de> Deserialize<'de> for XOnlyPublicKey {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let hex = String::deserialize(deserializer)?;
        Self::from_hex(&hex).map_err(serde::de::Error::custom)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EcdsaSignature {
    bytes: [u8; 64],
}

impl EcdsaSignature {
    pub fn from_bytes(bytes: [u8; 64]) -> Self {
        Self { bytes }
    }

    pub fn to_bytes(&self) -> [u8; 64] {
        self.bytes
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SchnorrSignature {
    bytes: [u8; 64],
}

impl SchnorrSignature {
    pub fn to_bytes(&self) -> [u8; 64] {
        self.bytes
    }
}

#[cfg(feature = "serde")]
impl Serialize for SchnorrSignature {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(&hex_encode(&self.bytes))
    }
}

#[cfg(feature = "serde")]
impl<'de> Deserialize<'de> for SchnorrSignature {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let hex = String::deserialize(deserializer)?;
        let bytes = hex_decode(&hex).map_err(serde::de::Error::custom)?;
        if bytes.len() != 64 {
            return Err(serde::de::Error::custom("expected 128 hex characters"));
        }
        let mut arr = [0u8; 64];
        arr.copy_from_slice(&bytes);
        Ok(Self { bytes: arr })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct KeyBundle {
    secret: SecretKey,
    xonly: XOnlyPublicKey,
}

impl KeyBundle {
    pub fn generate() -> Result<Self, SecpError> {
        let secret = SecretKey::generate()?;
        let xonly = secret.xonly_public_key()?;
        Ok(Self { secret, xonly })
    }

    pub fn secret(&self) -> &SecretKey {
        &self.secret
    }

    pub fn xonly_public_key(&self) -> &XOnlyPublicKey {
        &self.xonly
    }

    #[cfg(feature = "batch")]
    pub fn generate_batch(count: usize) -> Result<Vec<Self>, SecpError> {
        let mut bundles = Vec::with_capacity(count);
        for _ in 0..count {
            bundles.push(Self::generate()?);
        }
        Ok(bundles)
    }

    #[cfg(feature = "nip19")]
    pub fn npub(&self) -> Result<String, SecpError> {
        nip19::encode_npub(&self.xonly)
    }

    #[cfg(feature = "nip19")]
    pub fn nsec(&self) -> Result<String, SecpError> {
        nip19::encode_nsec(&self.secret)
    }
}

#[cfg(all(feature = "batch", feature = "nip19"))]
pub fn mine_vanity_npub(prefix: &str, max_attempts: u64) -> Result<KeyBundle, SecpError> {
    for _ in 0..max_attempts {
        let bundle = KeyBundle::generate()?;
        if count_npub_prefix_matches(&bundle.xonly_public_key().to_bytes(), prefix)? == prefix.len()
        {
            return Ok(bundle);
        }
    }
    Err(SecpError::ExhaustedAttempts)
}

#[cfg(all(feature = "batch", feature = "nip19"))]
#[derive(Debug, Clone)]
pub struct VanityCandidate {
    bundle: KeyBundle,
    matched_len: usize,
}

#[cfg(all(feature = "batch", feature = "nip19"))]
impl VanityCandidate {
    pub fn bundle(&self) -> &KeyBundle {
        &self.bundle
    }

    pub fn matched_len(&self) -> usize {
        self.matched_len
    }
}

#[cfg(all(feature = "batch", feature = "nip19"))]
pub fn mine_vanity_npub_candidates(
    prefix: &str,
    max_attempts: u64,
    top_k: usize,
) -> Result<Vec<VanityCandidate>, SecpError> {
    if top_k == 0 {
        return Ok(vec![]);
    }
    let mut candidates: Vec<VanityCandidate> = Vec::new();
    let mut min_matched = 0usize;

    for _ in 0..max_attempts {
        let bundle = KeyBundle::generate()?;
        let matched = count_npub_prefix_matches(&bundle.xonly_public_key().to_bytes(), prefix)?;

        if matched == 0 {
            continue;
        }

        if matched == prefix.len() || matched > min_matched || candidates.len() < top_k {
            candidates.push(VanityCandidate {
                bundle,
                matched_len: matched,
            });
        }

        if candidates.len() > top_k {
            candidates.sort_by(|a, b| b.matched_len.cmp(&a.matched_len));
            candidates.truncate(top_k);
            min_matched = candidates.last().map_or(0, |c| c.matched_len);
        }
    }

    candidates.sort_by(|a, b| b.matched_len.cmp(&a.matched_len));
    candidates.truncate(top_k);
    Ok(candidates)
}

#[cfg(all(feature = "batch", feature = "nip19"))]
const BECH32_CHARSET: &[u8; 32] = b"qpzry9x8gf2tvdw0s3jn54khce6mua7l";

#[cfg(all(feature = "batch", feature = "nip19"))]
fn bech32_value(byte: u8) -> Result<u8, SecpError> {
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
        _ => Err(SecpError::InvalidNip19("invalid npub vanity prefix")),
    }
}

#[cfg(all(feature = "batch", feature = "nip19"))]
fn count_npub_prefix_matches(xonly_bytes: &[u8; 32], prefix: &str) -> Result<usize, SecpError> {
    let prefix_bytes = prefix.as_bytes();
    if prefix_bytes.is_empty() {
        return Ok(0);
    }

    for &byte in prefix_bytes {
        bech32_value(byte)?;
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

#[cfg(feature = "batch")]
pub fn mine_pow(difficulty: u8, max_attempts: u64) -> Result<KeyBundle, SecpError> {
    for _ in 0..max_attempts {
        let bundle = KeyBundle::generate()?;
        if count_leading_zero_nibbles(&bundle.xonly_public_key().to_bytes()) >= difficulty {
            return Ok(bundle);
        }
    }
    Err(SecpError::ExhaustedAttempts)
}

#[cfg(feature = "batch")]
pub fn mine_pow_best(min_difficulty: u8, max_attempts: u64) -> Result<(KeyBundle, u8), SecpError> {
    let mut best: Option<(KeyBundle, u8)> = None;
    for _ in 0..max_attempts {
        let bundle = KeyBundle::generate()?;
        let diff = count_leading_zero_nibbles(&bundle.xonly_public_key().to_bytes());
        if diff >= min_difficulty {
            match best {
                Some((_, best_diff)) if diff <= best_diff => {}
                _ => best = Some((bundle, diff)),
            }
        }
    }
    best.ok_or(SecpError::ExhaustedAttempts)
}

#[cfg(feature = "batch")]
fn count_leading_zero_nibbles(bytes: &[u8]) -> u8 {
    let mut count = 0u8;
    for &byte in bytes {
        let high = byte >> 4;
        if high == 0 {
            count += 1;
        } else {
            break;
        }

        let low = byte & 0x0f;
        if low == 0 {
            count += 1;
        } else {
            break;
        }
    }
    count
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EventId {
    bytes: [u8; 32],
}

impl EventId {
    pub fn from_hex(hex: &str) -> Result<Self, SecpError> {
        let bytes = hex_decode(hex)?;
        if bytes.len() != 32 {
            return Err(SecpError::InvalidHex("expected 64 hex characters"));
        }
        let mut arr = [0u8; 32];
        arr.copy_from_slice(&bytes);
        Ok(Self::from_bytes(arr))
    }

    pub fn from_bytes(bytes: [u8; 32]) -> Self {
        Self { bytes }
    }

    pub fn to_hex(&self) -> String {
        hex_encode(&self.bytes)
    }

    pub fn to_bytes(&self) -> [u8; 32] {
        self.bytes
    }
}

#[cfg(feature = "serde")]
impl Serialize for EventId {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(&self.to_hex())
    }
}

#[cfg(feature = "serde")]
impl<'de> Deserialize<'de> for EventId {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let hex = String::deserialize(deserializer)?;
        Self::from_hex(&hex).map_err(serde::de::Error::custom)
    }
}

#[cfg(feature = "nip19")]
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Nip19 {
    Npub(XOnlyPublicKey),
    Nsec(SecretKey),
    Note(EventId),
    NProfile(NProfile),
    NEvent(NEvent),
    NAddr(NAddr),
    NRelay(NRelay),
}

#[cfg(feature = "nip19")]
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NProfile {
    pub pubkey: XOnlyPublicKey,
    pub relays: Vec<String>,
}

#[cfg(feature = "nip19")]
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NEvent {
    pub id: EventId,
    pub relays: Vec<String>,
    pub author: Option<XOnlyPublicKey>,
    pub kind: Option<u32>,
}

#[cfg(feature = "nip19")]
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NAddr {
    pub identifier: String,
    pub relays: Vec<String>,
    pub author: XOnlyPublicKey,
    pub kind: u32,
}

#[cfg(feature = "nip19")]
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NRelay {
    pub relay: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct UnsignedEvent {
    pub created_at: u64,
    pub kind: u32,
    pub tags: Vec<Vec<String>>,
    pub content: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct SignedEvent {
    pub id: EventId,
    pub pubkey: XOnlyPublicKey,
    pub created_at: u64,
    pub kind: u32,
    pub tags: Vec<Vec<String>>,
    pub content: String,
    pub sig: SchnorrSignature,
}

#[cfg(feature = "nostr")]
pub mod nostr {
    use super::*;
    use neco_json::JsonValue;

    pub fn serialize_event(
        pubkey: &XOnlyPublicKey,
        event: &UnsignedEvent,
    ) -> Result<String, SecpError> {
        let tags = JsonValue::Array(
            event
                .tags
                .iter()
                .map(|tag| {
                    JsonValue::Array(
                        tag.iter()
                            .map(|s| JsonValue::String(s.clone()))
                            .collect(),
                    )
                })
                .collect(),
        );
        let payload = JsonValue::Array(vec![
            JsonValue::Number(0.0),
            JsonValue::String(hex_encode(&pubkey.to_bytes())),
            JsonValue::Number(event.created_at as f64),
            JsonValue::Number(event.kind as f64),
            tags,
            JsonValue::String(event.content.clone()),
        ]);
        let bytes = neco_json::encode(&payload).map_err(SecpError::from)?;
        String::from_utf8(bytes).map_err(|e| SecpError::Json(e.to_string()))
    }

    pub fn compute_event_id(
        pubkey: &XOnlyPublicKey,
        event: &UnsignedEvent,
    ) -> Result<EventId, SecpError> {
        let serialized = serialize_event(pubkey, event)?;
        let mut hasher = Sha256::new();
        hasher.update(serialized.as_bytes());
        let mut bytes = [0u8; 32];
        bytes.copy_from_slice(&hasher.finalize());
        Ok(EventId { bytes })
    }

    pub fn finalize_event(
        event: UnsignedEvent,
        secret: &SecretKey,
    ) -> Result<SignedEvent, SecpError> {
        let pubkey = secret.xonly_public_key()?;
        let id = compute_event_id(&pubkey, &event)?;
        let sig = secret.sign_schnorr_prehash(id.to_bytes())?;
        Ok(SignedEvent {
            id,
            pubkey,
            created_at: event.created_at,
            kind: event.kind,
            tags: event.tags,
            content: event.content,
            sig,
        })
    }

    pub fn serialize_signed_event(event: &SignedEvent) -> Result<String, SecpError> {
        let tags = JsonValue::Array(
            event
                .tags
                .iter()
                .map(|tag| {
                    JsonValue::Array(
                        tag.iter()
                            .map(|s| JsonValue::String(s.clone()))
                            .collect(),
                    )
                })
                .collect(),
        );
        let content_encoded = neco_json::encode(&JsonValue::String(event.content.clone()))
            .map_err(SecpError::from)?;
        let content_str =
            String::from_utf8(content_encoded).map_err(|e| SecpError::Json(e.to_string()))?;
        let tags_encoded = neco_json::encode(&tags).map_err(SecpError::from)?;
        let tags_str =
            String::from_utf8(tags_encoded).map_err(|e| SecpError::Json(e.to_string()))?;
        Ok(format!(
            "{{\"id\":\"{}\",\"pubkey\":\"{}\",\"created_at\":{},\"kind\":{},\"tags\":{},\"content\":{},\"sig\":\"{}\"}}",
            hex_encode(&event.id.to_bytes()),
            hex_encode(&event.pubkey.to_bytes()),
            event.created_at,
            event.kind,
            tags_str,
            content_str,
            hex_encode(&event.sig.to_bytes())
        ))
    }

    pub fn parse_signed_event(json: &str) -> Result<SignedEvent, SecpError> {
        let value = neco_json::parse(json.as_bytes()).map_err(SecpError::from)?;
        if !value.is_object() {
            return Err(SecpError::InvalidEvent("signed event must be a JSON object"));
        }

        let id = parse_hex32(required_string(&value, "id")?, "id")?;
        let pubkey = parse_hex32(required_string(&value, "pubkey")?, "pubkey")?;
        let created_at = required_u64(&value, "created_at")?;
        let kind = required_u32(&value, "kind")?;
        let tags = parse_tags(required_value(&value, "tags")?)?;
        let content = required_string(&value, "content")?.to_string();
        let sig = parse_hex64(required_string(&value, "sig")?, "sig")?;

        Ok(SignedEvent {
            id: EventId::from_bytes(id),
            pubkey: XOnlyPublicKey::from_bytes(pubkey)?,
            created_at,
            kind,
            tags,
            content,
            sig: SchnorrSignature { bytes: sig },
        })
    }

    pub fn verify_event(event: &SignedEvent) -> Result<(), SecpError> {
        let unsigned = UnsignedEvent {
            created_at: event.created_at,
            kind: event.kind,
            tags: event.tags.clone(),
            content: event.content.clone(),
        };
        let expected = compute_event_id(&event.pubkey, &unsigned)?;
        if expected != event.id {
            return Err(SecpError::InvalidEvent("event id mismatch"));
        }
        event
            .pubkey
            .verify_schnorr_prehash(event.id.to_bytes(), &event.sig)
    }

    fn required_value<'a>(
        object: &'a JsonValue,
        field: &'static str,
    ) -> Result<&'a JsonValue, SecpError> {
        object
            .get(field)
            .ok_or(SecpError::InvalidEvent(missing_field(field)))
    }

    fn required_string<'a>(
        object: &'a JsonValue,
        field: &'static str,
    ) -> Result<&'a str, SecpError> {
        required_value(object, field)?
            .as_str()
            .ok_or(SecpError::InvalidEvent(expected_field(field)))
    }

    fn required_u64(object: &JsonValue, field: &'static str) -> Result<u64, SecpError> {
        required_value(object, field)?
            .as_u64()
            .ok_or(SecpError::InvalidEvent(expected_field(field)))
    }

    fn required_u32(object: &JsonValue, field: &'static str) -> Result<u32, SecpError> {
        required_u64(object, field)?
            .try_into()
            .map_err(|_| SecpError::InvalidEvent(expected_field(field)))
    }

    fn parse_tags(value: &JsonValue) -> Result<Vec<Vec<String>>, SecpError> {
        let tags = value
            .as_array()
            .ok_or(SecpError::InvalidEvent("tags must be an array"))?;
        tags.iter()
            .map(|tag| {
                let tag = tag
                    .as_array()
                    .ok_or(SecpError::InvalidEvent("tag must be an array"))?;
                tag.iter()
                    .map(|entry| {
                        entry
                            .as_str()
                            .map(ToOwned::to_owned)
                            .ok_or(SecpError::InvalidEvent("tag entry must be a string"))
                    })
                    .collect()
            })
            .collect()
    }

    fn parse_hex32(hex: &str, field: &'static str) -> Result<[u8; 32], SecpError> {
        let bytes =
            hex_decode(hex).map_err(|_| SecpError::InvalidEvent(invalid_hex_field(field)))?;
        bytes
            .try_into()
            .map_err(|_| SecpError::InvalidEvent(invalid_hex_field(field)))
    }

    fn parse_hex64(hex: &str, field: &'static str) -> Result<[u8; 64], SecpError> {
        let bytes =
            hex_decode(hex).map_err(|_| SecpError::InvalidEvent(invalid_hex_field(field)))?;
        bytes
            .try_into()
            .map_err(|_| SecpError::InvalidEvent(invalid_hex_field(field)))
    }

    fn missing_field(field: &'static str) -> &'static str {
        match field {
            "id" => "missing id",
            "pubkey" => "missing pubkey",
            "created_at" => "missing created_at",
            "kind" => "missing kind",
            "tags" => "missing tags",
            "content" => "missing content",
            "sig" => "missing sig",
            _ => "missing field",
        }
    }

    fn expected_field(field: &'static str) -> &'static str {
        match field {
            "id" => "id must be a string",
            "pubkey" => "pubkey must be a string",
            "created_at" => "created_at must be an integer",
            "kind" => "kind must be an integer",
            "content" => "content must be a string",
            "sig" => "sig must be a string",
            _ => "invalid field type",
        }
    }

    fn invalid_hex_field(field: &'static str) -> &'static str {
        match field {
            "id" => "id must be 64 hex characters",
            "pubkey" => "pubkey must be 64 hex characters",
            "sig" => "sig must be 128 hex characters",
            _ => "invalid hex field",
        }
    }
}

#[cfg(all(feature = "nostr", feature = "nip44"))]
pub mod nip17 {
    use super::*;
    use k256::elliptic_curve::rand_core::RngCore;

    pub fn create_seal(
        inner: UnsignedEvent,
        sender: &SecretKey,
        recipient: &XOnlyPublicKey,
    ) -> Result<SignedEvent, SecpError> {
        let signed_inner = nostr::finalize_event(inner, sender)?;
        let json = nostr::serialize_signed_event(&signed_inner)?;
        let conversation_key = nip44::get_conversation_key(sender, recipient)?;
        let encrypted = nip44::encrypt(&json, &conversation_key, None)?;
        let seal = UnsignedEvent {
            created_at: randomized_timestamp(signed_inner.created_at),
            kind: 13,
            tags: Vec::new(),
            content: encrypted,
        };
        nostr::finalize_event(seal, sender)
    }

    pub fn open_seal(seal: &SignedEvent, recipient: &SecretKey) -> Result<SignedEvent, SecpError> {
        if seal.kind != 13 {
            return Err(SecpError::InvalidEvent("seal must have kind 13"));
        }
        nostr::verify_event(seal)?;
        let conversation_key = nip44::get_conversation_key(recipient, &seal.pubkey)?;
        let json = nip44::decrypt(&seal.content, &conversation_key)?;
        let inner = nostr::parse_signed_event(&json)?;
        nostr::verify_event(&inner)?;
        Ok(inner)
    }

    pub fn create_gift_wrap(
        seal: &SignedEvent,
        recipient: &XOnlyPublicKey,
    ) -> Result<SignedEvent, SecpError> {
        if seal.kind != 13 {
            return Err(SecpError::InvalidEvent("seal must have kind 13"));
        }
        nostr::verify_event(seal)?;
        let ephemeral = SecretKey::generate()?;
        let json = nostr::serialize_signed_event(seal)?;
        let conversation_key = nip44::get_conversation_key(&ephemeral, recipient)?;
        let encrypted = nip44::encrypt(&json, &conversation_key, None)?;
        let wrap = UnsignedEvent {
            created_at: randomized_timestamp(seal.created_at),
            kind: 1059,
            tags: vec![vec!["p".to_string(), recipient.to_hex()]],
            content: encrypted,
        };
        nostr::finalize_event(wrap, &ephemeral)
    }

    pub fn open_gift_wrap(
        gift_wrap: &SignedEvent,
        recipient: &SecretKey,
    ) -> Result<SignedEvent, SecpError> {
        if gift_wrap.kind != 1059 {
            return Err(SecpError::InvalidEvent("gift wrap must have kind 1059"));
        }
        nostr::verify_event(gift_wrap)?;
        let conversation_key = nip44::get_conversation_key(recipient, &gift_wrap.pubkey)?;
        let json = nip44::decrypt(&gift_wrap.content, &conversation_key)?;
        let seal = nostr::parse_signed_event(&json)?;
        open_seal(&seal, recipient)
    }

    fn randomized_timestamp(base: u64) -> u64 {
        let offset = (OsRng.next_u32() % 345_601) as u64;
        base.saturating_sub(172_800) + offset
    }
}

#[cfg(feature = "nostr")]
pub mod nip42 {
    use super::*;

    pub fn create_auth_event(
        challenge: &str,
        relay_url: &str,
        signer: &SecretKey,
        now_unix_seconds: u64,
    ) -> Result<SignedEvent, SecpError> {
        let event = UnsignedEvent {
            created_at: now_unix_seconds,
            kind: 22_242,
            tags: vec![
                vec!["relay".to_string(), relay_url.to_string()],
                vec!["challenge".to_string(), challenge.to_string()],
            ],
            content: String::new(),
        };
        nostr::finalize_event(event, signer)
    }

    pub fn validate_auth_event(
        event: &SignedEvent,
        challenge: &str,
        relay_url: &str,
    ) -> Result<XOnlyPublicKey, SecpError> {
        nostr::verify_event(event)?;
        if event.kind != 22_242 {
            return Err(SecpError::InvalidEvent("auth event must have kind 22242"));
        }
        if tag_value(&event.tags, "relay") != Some(relay_url) {
            return Err(SecpError::InvalidEvent("auth event relay tag mismatch"));
        }
        if tag_value(&event.tags, "challenge") != Some(challenge) {
            return Err(SecpError::InvalidEvent("auth event challenge tag mismatch"));
        }
        Ok(event.pubkey)
    }

    fn tag_value<'a>(tags: &'a [Vec<String>], name: &str) -> Option<&'a str> {
        tags.iter()
            .find(|tag| tag.first().is_some_and(|value| value == name))
            .and_then(|tag| tag.get(1))
            .map(String::as_str)
    }
}

#[cfg(feature = "nip19")]
pub mod nip19 {
    use super::*;
    use bech32::{self, FromBase32, ToBase32, Variant};

    pub fn encode_npub(pubkey: &XOnlyPublicKey) -> Result<String, SecpError> {
        bech32::encode("npub", pubkey.to_bytes().to_base32(), Variant::Bech32)
            .map_err(|_| SecpError::InvalidNip19("failed to encode npub"))
    }

    pub fn encode_nsec(secret: &SecretKey) -> Result<String, SecpError> {
        bech32::encode("nsec", secret.to_bytes().to_base32(), Variant::Bech32)
            .map_err(|_| SecpError::InvalidNip19("failed to encode nsec"))
    }

    pub fn encode_note(id: &EventId) -> Result<String, SecpError> {
        bech32::encode("note", id.to_bytes().to_base32(), Variant::Bech32)
            .map_err(|_| SecpError::InvalidNip19("failed to encode note"))
    }

    pub fn encode_nprofile(profile: &NProfile) -> Result<String, SecpError> {
        encode_tlv_entity(
            "nprofile",
            &[
                (0, vec![profile.pubkey.to_bytes().to_vec()]),
                (
                    1,
                    profile
                        .relays
                        .iter()
                        .map(|relay| relay.as_bytes().to_vec())
                        .collect(),
                ),
            ],
        )
    }

    pub fn encode_nevent(event: &NEvent) -> Result<String, SecpError> {
        let mut fields = vec![
            (0, vec![event.id.to_bytes().to_vec()]),
            (
                1,
                event
                    .relays
                    .iter()
                    .map(|relay| relay.as_bytes().to_vec())
                    .collect(),
            ),
        ];

        if let Some(author) = event.author {
            fields.push((2, vec![author.to_bytes().to_vec()]));
        }
        if let Some(kind) = event.kind {
            fields.push((3, vec![kind.to_be_bytes().to_vec()]));
        }

        encode_tlv_entity("nevent", &fields)
    }

    pub fn encode_naddr(addr: &NAddr) -> Result<String, SecpError> {
        encode_tlv_entity(
            "naddr",
            &[
                (0, vec![addr.identifier.as_bytes().to_vec()]),
                (
                    1,
                    addr.relays
                        .iter()
                        .map(|relay| relay.as_bytes().to_vec())
                        .collect(),
                ),
                (2, vec![addr.author.to_bytes().to_vec()]),
                (3, vec![addr.kind.to_be_bytes().to_vec()]),
            ],
        )
    }

    pub fn encode_nrelay(relay: &NRelay) -> Result<String, SecpError> {
        encode_tlv_entity("nrelay", &[(0, vec![relay.relay.as_bytes().to_vec()])])
    }

    pub fn decode(s: &str) -> Result<Nip19, SecpError> {
        let (hrp, data, variant) =
            bech32::decode(s).map_err(|_| SecpError::InvalidNip19("invalid bech32 string"))?;
        if variant != Variant::Bech32 {
            return Err(SecpError::InvalidNip19("unexpected bech32 variant"));
        }

        let bytes = Vec::<u8>::from_base32(&data)
            .map_err(|_| SecpError::InvalidNip19("invalid bech32 payload"))?;

        match hrp.as_str() {
            "npub" => {
                let payload: [u8; 32] = bytes
                    .as_slice()
                    .try_into()
                    .map_err(|_| SecpError::InvalidNip19("expected 32-byte payload"))?;
                Ok(Nip19::Npub(XOnlyPublicKey::from_bytes(payload)?))
            }
            "nsec" => {
                let payload: [u8; 32] = bytes
                    .as_slice()
                    .try_into()
                    .map_err(|_| SecpError::InvalidNip19("expected 32-byte payload"))?;
                Ok(Nip19::Nsec(SecretKey::from_bytes(payload)?))
            }
            "note" => {
                let payload: [u8; 32] = bytes
                    .as_slice()
                    .try_into()
                    .map_err(|_| SecpError::InvalidNip19("expected 32-byte payload"))?;
                Ok(Nip19::Note(EventId::from_bytes(payload)))
            }
            "nprofile" => decode_nprofile(&bytes),
            "nevent" => decode_nevent(&bytes),
            "naddr" => decode_naddr(&bytes),
            "nrelay" => decode_nrelay(&bytes),
            _ => Err(SecpError::InvalidNip19("unsupported nip19 prefix")),
        }
    }

    fn decode_nprofile(bytes: &[u8]) -> Result<Nip19, SecpError> {
        let tlv = parse_tlv(bytes)?;
        let pubkey = required_bytes32(&tlv, 0, "nprofile")?;
        let relays = utf8_entries(&tlv, 1, "nprofile")?;
        Ok(Nip19::NProfile(NProfile {
            pubkey: XOnlyPublicKey::from_bytes(pubkey)?,
            relays,
        }))
    }

    fn decode_nevent(bytes: &[u8]) -> Result<Nip19, SecpError> {
        let tlv = parse_tlv(bytes)?;
        let id = required_bytes32(&tlv, 0, "nevent")?;
        let relays = utf8_entries(&tlv, 1, "nevent")?;
        let author = optional_bytes32(&tlv, 2, "nevent")?
            .map(XOnlyPublicKey::from_bytes)
            .transpose()?;
        let kind = optional_u32(&tlv, 3, "nevent")?;

        Ok(Nip19::NEvent(NEvent {
            id: EventId::from_bytes(id),
            relays,
            author,
            kind,
        }))
    }

    fn decode_naddr(bytes: &[u8]) -> Result<Nip19, SecpError> {
        let tlv = parse_tlv(bytes)?;
        let identifier = required_utf8(&tlv, 0, "naddr")?;
        let relays = utf8_entries(&tlv, 1, "naddr")?;
        let author = required_bytes32(&tlv, 2, "naddr")?;
        let kind = required_u32(&tlv, 3, "naddr")?;

        Ok(Nip19::NAddr(NAddr {
            identifier,
            relays,
            author: XOnlyPublicKey::from_bytes(author)?,
            kind,
        }))
    }

    fn decode_nrelay(bytes: &[u8]) -> Result<Nip19, SecpError> {
        let tlv = parse_tlv(bytes)?;
        let relay = required_utf8(&tlv, 0, "nrelay")?;
        Ok(Nip19::NRelay(NRelay { relay }))
    }

    fn encode_tlv_entity(prefix: &str, fields: &[(u8, Vec<Vec<u8>>)]) -> Result<String, SecpError> {
        let tlv = encode_tlv(fields)?;
        bech32::encode(prefix, tlv.to_base32(), Variant::Bech32)
            .map_err(|_| SecpError::InvalidNip19("failed to encode tlv entity"))
    }

    fn encode_tlv(fields: &[(u8, Vec<Vec<u8>>)]) -> Result<Vec<u8>, SecpError> {
        let mut out = Vec::new();
        for (tag, values) in fields.iter().rev() {
            for value in values {
                let len: u8 = value
                    .len()
                    .try_into()
                    .map_err(|_| SecpError::InvalidNip19("tlv value too long"))?;
                out.push(*tag);
                out.push(len);
                out.extend_from_slice(value);
            }
        }
        Ok(out)
    }

    fn parse_tlv(bytes: &[u8]) -> Result<Vec<Vec<Vec<u8>>>, SecpError> {
        let mut tlv = vec![Vec::new(); 256];
        let mut offset = 0usize;
        while offset < bytes.len() {
            if offset + 2 > bytes.len() {
                return Err(SecpError::InvalidNip19("truncated tlv header"));
            }
            let tag = bytes[offset] as usize;
            let len = bytes[offset + 1] as usize;
            offset += 2;
            if offset + len > bytes.len() {
                return Err(SecpError::InvalidNip19("not enough data for tlv entry"));
            }
            tlv[tag].push(bytes[offset..offset + len].to_vec());
            offset += len;
        }
        Ok(tlv)
    }

    fn required_bytes32(
        tlv: &[Vec<Vec<u8>>],
        tag: usize,
        entity: &'static str,
    ) -> Result<[u8; 32], SecpError> {
        let value = tlv[tag]
            .first()
            .ok_or(SecpError::InvalidNip19(missing_required_field(entity, tag)))?;
        value
            .as_slice()
            .try_into()
            .map_err(|_| SecpError::InvalidNip19(expected_length(entity, tag, 32)))
    }

    fn optional_bytes32(
        tlv: &[Vec<Vec<u8>>],
        tag: usize,
        entity: &'static str,
    ) -> Result<Option<[u8; 32]>, SecpError> {
        tlv[tag]
            .first()
            .map(|value| {
                value
                    .as_slice()
                    .try_into()
                    .map_err(|_| SecpError::InvalidNip19(expected_length(entity, tag, 32)))
            })
            .transpose()
    }

    fn required_u32(
        tlv: &[Vec<Vec<u8>>],
        tag: usize,
        entity: &'static str,
    ) -> Result<u32, SecpError> {
        optional_u32(tlv, tag, entity)?
            .ok_or(SecpError::InvalidNip19(missing_required_field(entity, tag)))
    }

    fn optional_u32(
        tlv: &[Vec<Vec<u8>>],
        tag: usize,
        entity: &'static str,
    ) -> Result<Option<u32>, SecpError> {
        tlv[tag]
            .first()
            .map(|value| {
                let bytes: [u8; 4] = value
                    .as_slice()
                    .try_into()
                    .map_err(|_| SecpError::InvalidNip19(expected_length(entity, tag, 4)))?;
                Ok(u32::from_be_bytes(bytes))
            })
            .transpose()
    }

    fn required_utf8(
        tlv: &[Vec<Vec<u8>>],
        tag: usize,
        entity: &'static str,
    ) -> Result<String, SecpError> {
        let value = tlv[tag]
            .first()
            .ok_or(SecpError::InvalidNip19(missing_required_field(entity, tag)))?;
        String::from_utf8(value.clone())
            .map_err(|_| SecpError::InvalidNip19("invalid utf-8 payload"))
    }

    fn utf8_entries(
        tlv: &[Vec<Vec<u8>>],
        tag: usize,
        _entity: &'static str,
    ) -> Result<Vec<String>, SecpError> {
        tlv[tag]
            .iter()
            .map(|value| {
                String::from_utf8(value.clone())
                    .map_err(|_| SecpError::InvalidNip19("invalid utf-8 payload"))
            })
            .collect()
    }

    fn missing_required_field(entity: &'static str, tag: usize) -> &'static str {
        match (entity, tag) {
            ("nprofile", 0) => "missing TLV 0 for nprofile",
            ("nevent", 0) => "missing TLV 0 for nevent",
            ("naddr", 0) => "missing TLV 0 for naddr",
            ("naddr", 2) => "missing TLV 2 for naddr",
            ("naddr", 3) => "missing TLV 3 for naddr",
            ("nrelay", 0) => "missing TLV 0 for nrelay",
            _ => "missing required tlv field",
        }
    }

    fn expected_length(entity: &'static str, tag: usize, len: usize) -> &'static str {
        match (entity, tag, len) {
            ("nprofile", 0, 32) => "TLV 0 should be 32 bytes",
            ("nevent", 0, 32) => "TLV 0 should be 32 bytes",
            ("nevent", 2, 32) => "TLV 2 should be 32 bytes",
            ("nevent", 3, 4) => "TLV 3 should be 4 bytes",
            ("naddr", 2, 32) => "TLV 2 should be 32 bytes",
            ("naddr", 3, 4) => "TLV 3 should be 4 bytes",
            _ => "invalid tlv length",
        }
    }
}

#[cfg(feature = "nip44")]
pub mod nip44 {
    use super::*;
    use chacha20::cipher::{KeyIvInit, StreamCipher};
    use hkdf::Hkdf;
    use hmac::{Hmac, Mac};
    use k256::ecdh::diffie_hellman;
    use k256::elliptic_curve::rand_core::RngCore;

    type HmacSha256 = Hmac<Sha256>;

    const VERSION_V2: u8 = 2;
    const MIN_PLAINTEXT_SIZE: usize = 1;
    const MAX_PLAINTEXT_SIZE: usize = 65_535;
    const MIN_RAW_PAYLOAD_SIZE: usize = 99;
    const MAX_RAW_PAYLOAD_SIZE: usize = 65_603;
    const MIN_ENCODED_PAYLOAD_SIZE: usize = 132;
    const MAX_ENCODED_PAYLOAD_SIZE: usize = 87_472;

    pub fn get_conversation_key(
        secret: &SecretKey,
        pubkey: &XOnlyPublicKey,
    ) -> Result<[u8; 32], SecpError> {
        let signing_key = secret.signing_key()?;
        let secp_public = super::decode_xonly_pubkey(pubkey)?;
        let shared = diffie_hellman(signing_key.as_nonzero_scalar(), secp_public.as_affine());
        let hk = Hkdf::<Sha256>::extract(Some(b"nip44-v2"), shared.raw_secret_bytes().as_ref()).0;
        Ok(hk.into())
    }

    pub fn calc_padded_len(len: usize) -> Result<usize, SecpError> {
        if len < MIN_PLAINTEXT_SIZE {
            return Err(SecpError::InvalidNip44("expected positive integer"));
        }
        if len <= 32 {
            return Ok(32);
        }
        let next_power = 1usize << (usize::BITS as usize - (len - 1).leading_zeros() as usize);
        let chunk = if next_power <= 256 {
            32
        } else {
            next_power / 8
        };
        Ok(chunk * ((len - 1) / chunk + 1))
    }

    pub fn encrypt(
        plaintext: &str,
        conversation_key: &[u8; 32],
        nonce: Option<[u8; 32]>,
    ) -> Result<String, SecpError> {
        let nonce = nonce.unwrap_or_else(random_nonce);
        let padded = pad(plaintext)?;
        let keys = get_message_keys(conversation_key, &nonce)?;
        let ciphertext = chacha20_xor(&keys.chacha_key, &keys.chacha_nonce, &padded);
        let mac = hmac_aad(&keys.hmac_key, &ciphertext, &nonce)?;

        let mut payload = Vec::with_capacity(1 + nonce.len() + ciphertext.len() + mac.len());
        payload.push(VERSION_V2);
        payload.extend_from_slice(&nonce);
        payload.extend_from_slice(&ciphertext);
        payload.extend_from_slice(&mac);
        Ok(neco_base64::encode(&payload))
    }

    pub fn decrypt(payload: &str, conversation_key: &[u8; 32]) -> Result<String, SecpError> {
        let decoded = decode_payload(payload)?;
        let keys = get_message_keys(conversation_key, &decoded.nonce)?;
        let calculated_mac = hmac_aad(&keys.hmac_key, &decoded.ciphertext, &decoded.nonce)?;
        if calculated_mac != decoded.mac {
            return Err(SecpError::InvalidNip44("invalid MAC"));
        }

        let padded = chacha20_xor(&keys.chacha_key, &keys.chacha_nonce, &decoded.ciphertext);
        unpad(&padded)
    }

    struct MessageKeys {
        chacha_key: [u8; 32],
        chacha_nonce: [u8; 12],
        hmac_key: [u8; 32],
    }

    struct DecodedPayload {
        nonce: [u8; 32],
        ciphertext: Vec<u8>,
        mac: [u8; 32],
    }

    fn get_message_keys(
        conversation_key: &[u8; 32],
        nonce: &[u8; 32],
    ) -> Result<MessageKeys, SecpError> {
        let hk = Hkdf::<Sha256>::from_prk(conversation_key)
            .map_err(|_| SecpError::InvalidNip44("invalid conversation key"))?;
        let mut keys = [0u8; 76];
        hk.expand(nonce, &mut keys)
            .map_err(|_| SecpError::InvalidNip44("failed to derive message keys"))?;

        let mut chacha_key = [0u8; 32];
        chacha_key.copy_from_slice(&keys[..32]);

        let mut chacha_nonce = [0u8; 12];
        chacha_nonce.copy_from_slice(&keys[32..44]);

        let mut hmac_key = [0u8; 32];
        hmac_key.copy_from_slice(&keys[44..76]);

        Ok(MessageKeys {
            chacha_key,
            chacha_nonce,
            hmac_key,
        })
    }

    fn random_nonce() -> [u8; 32] {
        let mut nonce = [0u8; 32];
        OsRng.fill_bytes(&mut nonce);
        nonce
    }

    fn pad(plaintext: &str) -> Result<Vec<u8>, SecpError> {
        let unpadded = plaintext.as_bytes();
        let len = unpadded.len();
        if !(MIN_PLAINTEXT_SIZE..=MAX_PLAINTEXT_SIZE).contains(&len) {
            return Err(SecpError::InvalidNip44(
                "invalid plaintext size: must be between 1 and 65535 bytes",
            ));
        }

        let padded_len = calc_padded_len(len)?;
        let mut out = Vec::with_capacity(2 + padded_len);
        let len_u16 = u16::try_from(len).expect("validated plaintext size <= u16::MAX");
        out.extend_from_slice(&len_u16.to_be_bytes());
        out.extend_from_slice(unpadded);
        out.resize(2 + padded_len, 0);
        Ok(out)
    }

    fn unpad(padded: &[u8]) -> Result<String, SecpError> {
        if padded.len() < 2 {
            return Err(SecpError::InvalidNip44("invalid padding"));
        }
        let len = u16::from_be_bytes([padded[0], padded[1]]) as usize;
        if !(MIN_PLAINTEXT_SIZE..=MAX_PLAINTEXT_SIZE).contains(&len) {
            return Err(SecpError::InvalidNip44("invalid padding"));
        }
        let expected = 2 + calc_padded_len(len)?;
        if padded.len() != expected || 2 + len > padded.len() {
            return Err(SecpError::InvalidNip44("invalid padding"));
        }
        let unpadded = &padded[2..2 + len];
        String::from_utf8(unpadded.to_vec())
            .map_err(|_| SecpError::InvalidNip44("invalid utf-8 payload"))
    }

    fn hmac_aad(key: &[u8; 32], message: &[u8], aad: &[u8; 32]) -> Result<[u8; 32], SecpError> {
        let mut mac = HmacSha256::new_from_slice(key)
            .map_err(|_| SecpError::InvalidNip44("invalid HMAC key"))?;
        mac.update(aad);
        mac.update(message);
        let bytes = mac.finalize().into_bytes();
        let mut out = [0u8; 32];
        out.copy_from_slice(&bytes);
        Ok(out)
    }

    fn chacha20_xor(key: &[u8; 32], nonce: &[u8; 12], data: &[u8]) -> Vec<u8> {
        let mut out = data.to_vec();
        let mut cipher = chacha20::ChaCha20::new(key.into(), nonce.into());
        cipher.apply_keystream(&mut out);
        out
    }

    fn decode_payload(payload: &str) -> Result<DecodedPayload, SecpError> {
        let len = payload.len();
        if !(MIN_ENCODED_PAYLOAD_SIZE..=MAX_ENCODED_PAYLOAD_SIZE).contains(&len) {
            return Err(SecpError::InvalidNip44("invalid payload length"));
        }
        if payload.starts_with('#') {
            return Err(SecpError::InvalidNip44("unknown encryption version"));
        }

        let data = neco_base64::decode(payload)
            .map_err(|_| SecpError::InvalidNip44("invalid base64"))?;
        if !(MIN_RAW_PAYLOAD_SIZE..=MAX_RAW_PAYLOAD_SIZE).contains(&data.len()) {
            return Err(SecpError::InvalidNip44("invalid data length"));
        }
        if data[0] != VERSION_V2 {
            return Err(SecpError::InvalidNip44("unknown encryption version"));
        }
        if data.len() < 65 {
            return Err(SecpError::InvalidNip44("invalid data length"));
        }

        let mut nonce = [0u8; 32];
        nonce.copy_from_slice(&data[1..33]);
        let mut mac = [0u8; 32];
        mac.copy_from_slice(&data[data.len() - 32..]);
        let ciphertext = data[33..data.len() - 32].to_vec();

        Ok(DecodedPayload {
            nonce,
            ciphertext,
            mac,
        })
    }
}

#[cfg(feature = "nip04")]
pub mod nip04 {
    use super::*;
    use aes::Aes256;
    use cbc::cipher::block_padding::Pkcs7;
    use cbc::cipher::{BlockDecryptMut, BlockEncryptMut, KeyIvInit};
    use k256::ecdh::diffie_hellman;
    use k256::elliptic_curve::rand_core::RngCore;

    type Aes256CbcEnc = cbc::Encryptor<Aes256>;
    type Aes256CbcDec = cbc::Decryptor<Aes256>;

    pub fn encrypt(
        secret: &SecretKey,
        pubkey: &XOnlyPublicKey,
        plaintext: &str,
        iv: Option<[u8; 16]>,
    ) -> Result<String, SecpError> {
        let key = get_shared_secret_x(secret, pubkey)?;
        let iv = iv.unwrap_or_else(random_iv);
        let plaintext = plaintext.as_bytes();
        let mut buf = plaintext.to_vec();
        let msg_len = buf.len();
        buf.resize(msg_len + 16, 0);
        let ciphertext = Aes256CbcEnc::new((&key).into(), (&iv).into())
            .encrypt_padded_mut::<Pkcs7>(&mut buf, msg_len)
            .map_err(|_| SecpError::InvalidNip04("failed to encrypt"))?;

        Ok(format!(
            "{}?iv={}",
            neco_base64::encode(ciphertext),
            neco_base64::encode(&iv)
        ))
    }

    pub fn decrypt(
        secret: &SecretKey,
        pubkey: &XOnlyPublicKey,
        payload: &str,
    ) -> Result<String, SecpError> {
        let (ciphertext_b64, iv_b64) = payload
            .split_once("?iv=")
            .ok_or(SecpError::InvalidNip04("invalid payload"))?;
        let key = get_shared_secret_x(secret, pubkey)?;
        let iv = neco_base64::decode(iv_b64)
            .map_err(|_| SecpError::InvalidNip04("invalid iv"))?;
        let ciphertext = neco_base64::decode(ciphertext_b64)
            .map_err(|_| SecpError::InvalidNip04("invalid ciphertext"))?;
        let iv: [u8; 16] = iv
            .try_into()
            .map_err(|_| SecpError::InvalidNip04("invalid iv"))?;

        let mut buf = ciphertext;
        let plaintext = Aes256CbcDec::new((&key).into(), (&iv).into())
            .decrypt_padded_mut::<Pkcs7>(&mut buf)
            .map_err(|_| SecpError::InvalidNip04("failed to decrypt"))?;
        String::from_utf8(plaintext.to_vec())
            .map_err(|_| SecpError::InvalidNip04("invalid utf-8 payload"))
    }

    fn get_shared_secret_x(
        secret: &SecretKey,
        pubkey: &XOnlyPublicKey,
    ) -> Result<[u8; 32], SecpError> {
        let signing_key = secret.signing_key()?;
        let secp_public = super::decode_xonly_pubkey(pubkey)?;
        let shared = diffie_hellman(signing_key.as_nonzero_scalar(), secp_public.as_affine());
        let mut out = [0u8; 32];
        out.copy_from_slice(shared.raw_secret_bytes().as_ref());
        Ok(out)
    }

    fn random_iv() -> [u8; 16] {
        let mut iv = [0u8; 16];
        OsRng.fill_bytes(&mut iv);
        iv
    }
}

#[cfg(any(feature = "nip04", feature = "nip44"))]
fn decode_xonly_pubkey(pubkey: &XOnlyPublicKey) -> Result<k256::PublicKey, SecpError> {
    let mut sec1 = [0u8; 33];
    sec1[0] = 0x02;
    sec1[1..].copy_from_slice(&pubkey.to_bytes());
    k256::PublicKey::from_sec1_bytes(&sec1).map_err(|_| SecpError::InvalidPublicKey)
}

#[cfg(test)]
mod tests {
    use super::*;

    // Keep oracle fixtures stable across branding renames.
    #[cfg(feature = "nip04")]
    const ORACLE_NIP04_PLAINTEXT: &str = "cyphercat nip04 oracle";

    #[cfg(feature = "nip44")]
    const ORACLE_NIP44_PLAINTEXT: &str = "cyphercat nip44 oracle";

    const SECP256K1_ORDER: [u8; 32] = [
        0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff,
        0xfe, 0xba, 0xae, 0xdc, 0xe6, 0xaf, 0x48, 0xa0, 0x3b, 0xbf, 0xd2, 0x5e, 0x8c, 0xd0, 0x36,
        0x41, 0x41,
    ];

    fn scalar_sub(minuend: [u8; 32], subtrahend: [u8; 32]) -> [u8; 32] {
        let mut out = [0u8; 32];
        let mut borrow = 0u16;

        for i in (0..32).rev() {
            let lhs = u16::from(minuend[i]);
            let rhs = u16::from(subtrahend[i]) + borrow;
            if lhs >= rhs {
                out[i] = u8::try_from(lhs - rhs).expect("byte subtraction stays in range");
                borrow = 0;
            } else {
                out[i] =
                    u8::try_from((lhs + 256) - rhs).expect("borrowed subtraction stays in range");
                borrow = 1;
            }
        }

        assert_eq!(borrow, 0, "subtraction must not underflow");
        out
    }

    #[test]
    fn secret_key_roundtrip() {
        let secret = SecretKey::generate().expect("secret key");
        let restored = SecretKey::from_bytes(secret.to_bytes()).expect("restore");
        assert_eq!(secret.to_bytes(), restored.to_bytes());
    }

    #[test]
    fn schnorr_sign_and_verify() {
        let secret = SecretKey::generate().expect("secret key");
        let pubkey = secret.xonly_public_key().expect("pubkey");
        let digest = [7u8; 32];
        let sig = secret.sign_schnorr_prehash(digest).expect("sign");
        pubkey.verify_schnorr_prehash(digest, &sig).expect("verify");
    }

    #[test]
    fn invalid_secret_key_is_rejected() {
        let error = SecretKey::from_bytes([0u8; 32]).expect_err("must reject zero secret key");
        assert!(matches!(error, SecpError::InvalidSecretKey));
    }

    #[test]
    fn xonly_public_key_roundtrip() {
        let secret = SecretKey::generate().expect("secret key");
        let public = secret.xonly_public_key().expect("public key");
        let restored = XOnlyPublicKey::from_bytes(public.to_bytes()).expect("restored public key");
        assert_eq!(public.to_bytes(), restored.to_bytes());
    }

    #[test]
    fn public_key_sec1_roundtrip() {
        let secret = SecretKey::generate().expect("secret key");
        let public = secret.public_key().expect("public key");
        let restored =
            PublicKey::from_sec1_bytes(&public.to_sec1_bytes()).expect("restored public key");
        assert_eq!(public.to_sec1_bytes(), restored.to_sec1_bytes());
    }

    #[test]
    fn secret_key_hex_roundtrip() {
        let key = SecretKey::generate().expect("generate");
        let hex = key.to_hex();
        assert_eq!(hex.len(), 64);
        let decoded = SecretKey::from_hex(&hex).expect("from_hex");
        assert_eq!(key, decoded);
    }

    #[test]
    fn xonly_hex_roundtrip() {
        let key = SecretKey::generate().expect("generate");
        let xonly = key.xonly_public_key().expect("xonly");
        let hex = xonly.to_hex();
        assert_eq!(hex.len(), 64);
        let decoded = XOnlyPublicKey::from_hex(&hex).expect("from_hex");
        assert_eq!(xonly, decoded);
    }

    #[test]
    fn public_key_hex_roundtrip() {
        let key = SecretKey::generate().expect("generate");
        let pubkey = key.public_key().expect("pubkey");
        let hex = pubkey.to_hex();
        assert_eq!(hex.len(), 66);
        let decoded = PublicKey::from_hex(&hex).expect("from_hex");
        assert_eq!(pubkey, decoded);
    }

    #[test]
    fn event_id_hex_roundtrip() {
        let id = EventId::from_bytes([0xab; 32]);
        let hex = id.to_hex();
        assert_eq!(hex, "ab".repeat(32));
        let decoded = EventId::from_hex(&hex).expect("from_hex");
        assert_eq!(id, decoded);
    }

    #[test]
    fn hex_decode_case_insensitive() {
        let upper = "ABCDEF0123456789ABCDEF0123456789ABCDEF0123456789ABCDEF0123456789";
        let lower = upper.to_lowercase();
        let id_upper = EventId::from_hex(upper).expect("upper");
        let id_lower = EventId::from_hex(&lower).expect("lower");
        assert_eq!(id_upper, id_lower);
    }

    #[test]
    fn hex_decode_odd_length_fails() {
        assert!(matches!(
            EventId::from_hex("abc"),
            Err(SecpError::InvalidHex(_))
        ));
    }

    #[test]
    fn hex_decode_invalid_char_fails() {
        assert!(matches!(
            EventId::from_hex("zz00000000000000000000000000000000000000000000000000000000000000"),
            Err(SecpError::InvalidHex(_))
        ));
    }

    #[test]
    fn hex_decode_wrong_length_fails() {
        assert!(matches!(
            SecretKey::from_hex("aabb"),
            Err(SecpError::InvalidHex(_))
        ));
    }

    #[test]
    fn hex_encode_is_lowercase() {
        let id = EventId::from_bytes([0xAB; 32]);
        let hex = id.to_hex();
        assert_eq!(hex, hex.to_lowercase());
    }

    #[test]
    fn keybundle_generate() {
        let bundle = KeyBundle::generate().expect("key bundle");
        let derived = bundle
            .secret()
            .xonly_public_key()
            .expect("derived public key");
        assert_eq!(*bundle.xonly_public_key(), derived);
    }

    #[cfg(feature = "batch")]
    #[test]
    fn keybundle_generate_batch() {
        let bundles = KeyBundle::generate_batch(4).expect("batch");
        assert_eq!(bundles.len(), 4);
        for bundle in bundles {
            let derived = bundle
                .secret()
                .xonly_public_key()
                .expect("derived public key");
            assert_eq!(*bundle.xonly_public_key(), derived);
        }
    }

    #[cfg(feature = "batch")]
    #[test]
    fn keybundle_generate_batch_zero() {
        let bundles = KeyBundle::generate_batch(0).expect("batch");
        assert!(bundles.is_empty());
    }

    #[cfg(all(feature = "batch", feature = "nip19"))]
    #[test]
    #[ignore]
    fn mine_vanity_npub_finds_match() {
        let bundle = mine_vanity_npub("q", 100_000).expect("vanity match");
        assert!(bundle.npub().expect("npub")[5..].starts_with("q"));
    }

    #[cfg(all(feature = "batch", feature = "nip19"))]
    #[test]
    fn mine_vanity_npub_exhausted() {
        let error = mine_vanity_npub("zzzzzzzzzz", 10).expect_err("must exhaust attempts");
        assert!(matches!(error, SecpError::ExhaustedAttempts));
    }

    #[test]
    #[cfg(all(feature = "batch", feature = "nip19"))]
    #[ignore]
    fn vanity_candidates_returns_top_k() {
        let candidates = mine_vanity_npub_candidates("q", 10_000, 3).expect("candidates");
        assert!(candidates.len() <= 3);
        for c in &candidates {
            assert!(c.matched_len() >= 1);
        }
        for w in candidates.windows(2) {
            assert!(w[0].matched_len() >= w[1].matched_len());
        }
    }

    #[test]
    #[cfg(all(feature = "batch", feature = "nip19"))]
    fn vanity_candidates_zero_top_k() {
        let candidates = mine_vanity_npub_candidates("q", 100, 0).expect("candidates");
        assert!(candidates.is_empty());
    }

    #[test]
    #[cfg(all(feature = "batch", feature = "nip19"))]
    #[ignore]
    fn vanity_candidates_exact_match_included() {
        let candidates = mine_vanity_npub_candidates("q", 100_000, 5).expect("candidates");
        let has_exact = candidates.iter().any(|c| c.matched_len() == 1);
        assert!(
            has_exact,
            "should find at least one exact single-char match"
        );
    }

    #[test]
    #[cfg(all(feature = "batch", feature = "nip19"))]
    fn vanity_prefix_match_counter_matches_encoded_npub() {
        let secret = SecretKey::from_bytes([0x31; 32]).expect("secret");
        let xonly = secret.xonly_public_key().expect("pubkey");
        let npub = nip19::encode_npub(&xonly).expect("npub");
        let npub_data = &npub[5..];

        for len in [0usize, 1, 2, 3, 5, 8, 12] {
            let prefix = &npub_data[..len];
            let matched = count_npub_prefix_matches(&xonly.to_bytes(), prefix).expect("matched");
            assert_eq!(matched, len);
        }

        let mismatched = format!("{}x", &npub_data[..4]);
        let matched = count_npub_prefix_matches(&xonly.to_bytes(), &mismatched).expect("matched");
        assert_eq!(matched, 4);
    }

    #[test]
    #[cfg(all(feature = "batch", feature = "nip19"))]
    fn vanity_prefix_rejects_invalid_bech32_chars() {
        let err = mine_vanity_npub("!", 1).expect_err("invalid prefix");
        assert!(matches!(
            err,
            SecpError::InvalidNip19("invalid npub vanity prefix")
        ));

        let err = mine_vanity_npub_candidates("I", 1, 1).expect_err("invalid prefix");
        assert!(matches!(
            err,
            SecpError::InvalidNip19("invalid npub vanity prefix")
        ));
    }

    #[cfg(feature = "batch")]
    #[test]
    #[ignore]
    fn mine_pow_finds_match() {
        let bundle = mine_pow(1, 100_000).expect("pow match");
        assert!(count_leading_zero_nibbles(&bundle.xonly_public_key().to_bytes()) >= 1);
    }

    #[cfg(feature = "batch")]
    #[test]
    fn mine_pow_exhausted() {
        let error = mine_pow(64, 10).expect_err("must exhaust attempts");
        assert!(matches!(error, SecpError::ExhaustedAttempts));
    }

    #[test]
    #[cfg(feature = "batch")]
    #[ignore]
    fn mine_pow_best_returns_best() {
        let (bundle, diff) = mine_pow_best(1, 100_000).expect("pow best");
        assert!(diff >= 1);
        let actual = count_leading_zero_nibbles(&bundle.xonly_public_key().to_bytes());
        assert_eq!(diff, actual);
    }

    #[test]
    #[cfg(feature = "batch")]
    fn mine_pow_best_exhausted() {
        let err = mine_pow_best(64, 10).expect_err("should exhaust");
        assert!(matches!(err, SecpError::ExhaustedAttempts));
    }

    #[cfg(feature = "batch")]
    #[test]
    fn count_leading_zero_nibbles_cases() {
        assert_eq!(count_leading_zero_nibbles(&[0x00, 0x0a, 0xff]), 3);
        assert_eq!(count_leading_zero_nibbles(&[0x00, 0x00]), 4);
        assert_eq!(count_leading_zero_nibbles(&[0xab]), 0);
        assert_eq!(count_leading_zero_nibbles(&[0x0f]), 1);
    }

    #[test]
    fn schnorr_verify_rejects_wrong_digest() {
        let secret = SecretKey::generate().expect("secret key");
        let pubkey = secret.xonly_public_key().expect("pubkey");
        let sig = secret
            .sign_schnorr_prehash([1u8; 32])
            .expect("signature for digest");
        let error = pubkey
            .verify_schnorr_prehash([2u8; 32], &sig)
            .expect_err("must reject wrong digest");
        assert!(matches!(error, SecpError::InvalidSignature));
    }

    #[test]
    fn schnorr_verify_rejects_wrong_public_key() {
        let secret_a = SecretKey::generate().expect("secret key a");
        let secret_b = SecretKey::generate().expect("secret key b");
        let pubkey_b = secret_b.xonly_public_key().expect("pubkey b");
        let sig = secret_a
            .sign_schnorr_prehash([3u8; 32])
            .expect("signature for digest");
        let error = pubkey_b
            .verify_schnorr_prehash([3u8; 32], &sig)
            .expect_err("must reject wrong public key");
        assert!(matches!(error, SecpError::InvalidSignature));
    }

    #[test]
    fn ecdsa_sign_verify_roundtrip() {
        let secret = SecretKey::generate().expect("secret key");
        let public = secret.public_key().expect("public key");
        let digest = [9u8; 32];

        let sig = secret.sign_ecdsa_prehash(digest).expect("sign");
        assert_eq!(sig.to_bytes().len(), 64);
        public.verify_ecdsa_prehash(digest, &sig).expect("verify");
    }

    #[test]
    fn ecdsa_verify_wrong_message() {
        let secret = SecretKey::generate().expect("secret key");
        let public = secret.public_key().expect("public key");
        let sig = secret
            .sign_ecdsa_prehash([0x21; 32])
            .expect("signature for digest");

        let error = public
            .verify_ecdsa_prehash([0x22; 32], &sig)
            .expect_err("must reject wrong digest");
        assert!(matches!(error, SecpError::InvalidSignature));
    }

    #[test]
    fn ecdsa_reject_high_s() {
        let secret = SecretKey::from_bytes([0x11; 32]).expect("secret key");
        let public = secret.public_key().expect("public key");
        let digest = [0x33; 32];
        let low_s = secret.sign_ecdsa_prehash(digest).expect("sign");

        let low_signature =
            K256EcdsaSignature::from_slice(&low_s.to_bytes()).expect("low-S signature");
        let (r_bytes, s_bytes) = low_signature.split_bytes();
        let high_s_bytes = scalar_sub(SECP256K1_ORDER, s_bytes.into());
        let high_signature =
            K256EcdsaSignature::from_scalars(r_bytes, high_s_bytes).expect("high-S signature");
        assert!(bool::from(high_signature.s().is_high()));
        let high_s = EcdsaSignature::from_bytes(high_signature.to_bytes().into());

        let error = public
            .verify_ecdsa_prehash(digest, &high_s)
            .expect_err("must reject high-S signature");
        assert!(matches!(error, SecpError::InvalidSignature));
    }

    #[cfg(feature = "nostr")]
    #[test]
    fn finalize_and_verify_event() {
        let secret = SecretKey::generate().expect("secret key");
        let event = UnsignedEvent {
            created_at: 1_700_000_000,
            kind: 1,
            tags: vec![vec!["t".to_string(), "rust".to_string()]],
            content: "hello".to_string(),
        };

        let signed = nostr::finalize_event(event, &secret).expect("finalize");
        nostr::verify_event(&signed).expect("verify");
    }

    #[cfg(feature = "nostr")]
    #[test]
    fn nostr_matches_known_nostr_tools_fixture() {
        let secret = SecretKey::from_hex(
            "d217c1ff2f8a65c3e3a1740db3b9f58b\
             8c848bb45e26d00ed4714e4a0f4ceecf",
        )
        .expect("secret");
        let pubkey = secret.xonly_public_key().expect("pubkey");
        let unsigned = UnsignedEvent {
            created_at: 1_617_932_115,
            kind: 1,
            tags: vec![],
            content: "Hello, world!".to_string(),
        };

        let serialized = nostr::serialize_event(&pubkey, &unsigned).expect("serialize");
        let hash = nostr::compute_event_id(&pubkey, &unsigned).expect("hash");
        let signed = nostr::finalize_event(unsigned, &secret).expect("finalize");

        assert_eq!(
            pubkey.to_hex(),
            "6af0f9de588f2c53cedcba26c5e2402e0d0aa64ec7b47c9f8d97b5bc562bab5f"
        );
        assert_eq!(
            serialized,
            r#"[0,"6af0f9de588f2c53cedcba26c5e2402e0d0aa64ec7b47c9f8d97b5bc562bab5f",1617932115,1,[],"Hello, world!"]"#
        );
        assert_eq!(
            hash.to_hex(),
            "b2a44af84ca99b14820ae91c44e1ef0908f8aadc4e10620a6e6caa344507f03c"
        );
        assert_eq!(signed.id.to_hex(), hash.to_hex());
        assert_eq!(
            signed
                .sig
                .to_bytes()
                .iter()
                .map(|byte| format!("{byte:02x}"))
                .collect::<String>()
                .len(),
            128
        );
        nostr::verify_event(&signed).expect("verify");
    }

    #[cfg(feature = "nostr")]
    #[test]
    fn serialize_event_matches_expected_shape() {
        let secret = SecretKey::generate().expect("secret key");
        let public = secret.xonly_public_key().expect("public key");
        let event = UnsignedEvent {
            created_at: 1_700_000_123,
            kind: 7,
            tags: vec![vec!["p".to_string(), "abcd".to_string()]],
            content: "hello\nworld".to_string(),
        };

        let serialized = nostr::serialize_event(&public, &event).expect("serialize");
        let value = neco_json::parse(serialized.as_bytes()).expect("json");
        let array = value.as_array().expect("array payload");
        assert_eq!(array.len(), 6);
        assert_eq!(array[0].as_u64(), Some(0));
        assert_eq!(array[2].as_u64(), Some(event.created_at));
        assert_eq!(array[3].as_u64(), Some(event.kind as u64));
        let expected_tags = neco_json::JsonValue::Array(
            event
                .tags
                .iter()
                .map(|tag| {
                    neco_json::JsonValue::Array(
                        tag.iter()
                            .map(|s| neco_json::JsonValue::String(s.clone()))
                            .collect(),
                    )
                })
                .collect(),
        );
        assert_eq!(array[4], expected_tags);
        assert_eq!(array[5].as_str(), Some(event.content.as_str()));
    }

    #[cfg(feature = "nostr")]
    #[test]
    fn compute_event_id_is_reproducible() {
        let secret = SecretKey::generate().expect("secret key");
        let public = secret.xonly_public_key().expect("public key");
        let event = UnsignedEvent {
            created_at: 10,
            kind: 1,
            tags: vec![vec!["e".to_string(), "1".to_string()]],
            content: "same".to_string(),
        };

        let id_a = nostr::compute_event_id(&public, &event).expect("id a");
        let id_b = nostr::compute_event_id(&public, &event).expect("id b");
        assert_eq!(id_a, id_b);
    }

    #[cfg(feature = "nostr")]
    #[test]
    fn verify_rejects_tampered_content() {
        let secret = SecretKey::generate().expect("secret key");
        let event = UnsignedEvent {
            created_at: 1,
            kind: 1,
            tags: Vec::new(),
            content: "hello".to_string(),
        };
        let mut signed = nostr::finalize_event(event, &secret).expect("finalize");
        signed.content = "tampered".to_string();
        let error = nostr::verify_event(&signed).expect_err("must fail");
        assert!(matches!(error, SecpError::InvalidEvent(_)));
    }

    #[cfg(feature = "nostr")]
    #[test]
    fn verify_rejects_tampered_tags() {
        let secret = SecretKey::generate().expect("secret key");
        let event = UnsignedEvent {
            created_at: 2,
            kind: 1,
            tags: vec![vec!["t".to_string(), "rust".to_string()]],
            content: "hello".to_string(),
        };
        let mut signed = nostr::finalize_event(event, &secret).expect("finalize");
        signed.tags.push(vec!["p".to_string(), "peer".to_string()]);
        let error = nostr::verify_event(&signed).expect_err("must fail");
        assert!(matches!(error, SecpError::InvalidEvent(_)));
    }

    #[cfg(feature = "nostr")]
    #[test]
    fn verify_rejects_tampered_created_at() {
        let secret = SecretKey::generate().expect("secret key");
        let event = UnsignedEvent {
            created_at: 3,
            kind: 1,
            tags: Vec::new(),
            content: "hello".to_string(),
        };
        let mut signed = nostr::finalize_event(event, &secret).expect("finalize");
        signed.created_at += 1;
        let error = nostr::verify_event(&signed).expect_err("must fail");
        assert!(matches!(error, SecpError::InvalidEvent(_)));
    }

    #[cfg(feature = "nostr")]
    #[test]
    fn verify_rejects_tampered_kind() {
        let secret = SecretKey::generate().expect("secret key");
        let event = UnsignedEvent {
            created_at: 4,
            kind: 1,
            tags: Vec::new(),
            content: "hello".to_string(),
        };
        let mut signed = nostr::finalize_event(event, &secret).expect("finalize");
        signed.kind = 42;
        let error = nostr::verify_event(&signed).expect_err("must fail");
        assert!(matches!(error, SecpError::InvalidEvent(_)));
    }

    #[cfg(feature = "nostr")]
    #[test]
    fn verify_rejects_tampered_id() {
        let secret = SecretKey::generate().expect("secret key");
        let event = UnsignedEvent {
            created_at: 5,
            kind: 1,
            tags: Vec::new(),
            content: "hello".to_string(),
        };
        let mut signed = nostr::finalize_event(event, &secret).expect("finalize");
        signed.id = EventId { bytes: [9u8; 32] };
        let error = nostr::verify_event(&signed).expect_err("must fail");
        assert!(matches!(error, SecpError::InvalidEvent(_)));
    }

    #[cfg(feature = "nostr")]
    #[test]
    fn verify_rejects_tampered_signature() {
        let secret = SecretKey::generate().expect("secret key");
        let event = UnsignedEvent {
            created_at: 6,
            kind: 1,
            tags: Vec::new(),
            content: "hello".to_string(),
        };
        let mut signed = nostr::finalize_event(event, &secret).expect("finalize");
        let mut sig = signed.sig.to_bytes();
        sig[0] ^= 0x01;
        signed.sig = SchnorrSignature { bytes: sig };
        let error = nostr::verify_event(&signed).expect_err("must fail");
        assert!(matches!(error, SecpError::InvalidSignature));
    }

    #[cfg(feature = "nostr")]
    #[test]
    fn signed_event_serialize_parse_roundtrip() {
        let secret = SecretKey::generate().expect("secret key");
        let signed = nostr::finalize_event(
            UnsignedEvent {
                created_at: 1_700_000_222,
                kind: 14,
                tags: vec![vec!["p".to_string(), "peer".to_string()]],
                content: "sealed".to_string(),
            },
            &secret,
        )
        .expect("finalize");

        let json = nostr::serialize_signed_event(&signed).expect("serialize signed");
        let parsed = nostr::parse_signed_event(&json).expect("parse signed");
        assert_eq!(parsed, signed);
    }

    #[cfg(feature = "nostr")]
    #[test]
    fn nip42_create_and_validate() {
        let secret = SecretKey::generate().expect("secret key");
        let expected = secret.xonly_public_key().expect("pubkey");
        let signed = nip42::create_auth_event(
            "challenge-123",
            "wss://relay.example.com",
            &secret,
            1_700_000_666,
        )
        .expect("create auth event");

        assert_eq!(signed.kind, 22_242);
        assert_eq!(signed.content, "");
        assert_eq!(
            signed.tags,
            vec![
                vec!["relay".to_string(), "wss://relay.example.com".to_string()],
                vec!["challenge".to_string(), "challenge-123".to_string()],
            ]
        );
        assert_eq!(
            nip42::validate_auth_event(&signed, "challenge-123", "wss://relay.example.com")
                .expect("validate auth event"),
            expected
        );
    }

    #[cfg(feature = "nostr")]
    #[test]
    fn nip42_wrong_challenge_fails() {
        let secret = SecretKey::generate().expect("secret key");
        let signed = nip42::create_auth_event(
            "challenge-123",
            "wss://relay.example.com",
            &secret,
            1_700_000_777,
        )
        .expect("create auth event");

        let error =
            nip42::validate_auth_event(&signed, "wrong-challenge", "wss://relay.example.com")
                .expect_err("wrong challenge must fail");
        assert!(matches!(
            error,
            SecpError::InvalidEvent("auth event challenge tag mismatch")
        ));
    }

    #[cfg(feature = "nostr")]
    #[test]
    fn nip42_wrong_relay_fails() {
        let secret = SecretKey::generate().expect("secret key");
        let signed = nip42::create_auth_event(
            "challenge-123",
            "wss://relay.example.com",
            &secret,
            1_700_000_888,
        )
        .expect("create auth event");

        let error = nip42::validate_auth_event(&signed, "challenge-123", "wss://other.example.com")
            .expect_err("wrong relay must fail");
        assert!(matches!(
            error,
            SecpError::InvalidEvent("auth event relay tag mismatch")
        ));
    }

    #[cfg(all(feature = "nostr", feature = "nip44"))]
    #[test]
    fn nip17_seal_roundtrip() {
        let sender = SecretKey::generate().expect("sender");
        let recipient = SecretKey::generate().expect("recipient");
        let recipient_pubkey = recipient.xonly_public_key().expect("recipient pubkey");
        let inner = UnsignedEvent {
            created_at: 1_700_000_333,
            kind: 14,
            tags: vec![vec!["p".to_string(), recipient_pubkey.to_hex()]],
            content: "hello seal".to_string(),
        };

        let seal = nip17::create_seal(inner.clone(), &sender, &recipient_pubkey).expect("seal");
        let opened = nip17::open_seal(&seal, &recipient).expect("open seal");

        assert_eq!(seal.kind, 13);
        assert!(seal.tags.is_empty());
        assert_eq!(
            seal.pubkey,
            sender.xonly_public_key().expect("sender pubkey")
        );
        assert_eq!(opened.created_at, inner.created_at);
        assert_eq!(opened.kind, inner.kind);
        assert_eq!(opened.tags, inner.tags);
        assert_eq!(opened.content, inner.content);
    }

    #[cfg(all(feature = "nostr", feature = "nip44"))]
    #[test]
    fn nip17_gift_wrap_roundtrip() {
        let sender = SecretKey::generate().expect("sender");
        let recipient = SecretKey::generate().expect("recipient");
        let recipient_pubkey = recipient.xonly_public_key().expect("recipient pubkey");
        let inner = UnsignedEvent {
            created_at: 1_700_000_444,
            kind: 14,
            tags: vec![vec!["p".to_string(), recipient_pubkey.to_hex()]],
            content: "hello wrap".to_string(),
        };

        let seal = nip17::create_seal(inner.clone(), &sender, &recipient_pubkey).expect("seal");
        let gift_wrap = nip17::create_gift_wrap(&seal, &recipient_pubkey).expect("gift wrap");
        let opened = nip17::open_gift_wrap(&gift_wrap, &recipient).expect("open gift wrap");

        assert_eq!(gift_wrap.kind, 1059);
        assert_eq!(
            gift_wrap.tags,
            vec![vec!["p".to_string(), recipient_pubkey.to_hex()]]
        );
        assert_ne!(
            gift_wrap.pubkey,
            sender.xonly_public_key().expect("sender pubkey")
        );
        assert_eq!(opened.created_at, inner.created_at);
        assert_eq!(opened.kind, inner.kind);
        assert_eq!(opened.tags, inner.tags);
        assert_eq!(opened.content, inner.content);
    }

    #[cfg(all(feature = "nostr", feature = "nip44"))]
    #[test]
    fn nip17_wrong_recipient_fails() {
        let sender = SecretKey::generate().expect("sender");
        let recipient = SecretKey::generate().expect("recipient");
        let wrong_recipient = SecretKey::generate().expect("wrong recipient");
        let recipient_pubkey = recipient.xonly_public_key().expect("recipient pubkey");
        let inner = UnsignedEvent {
            created_at: 1_700_000_555,
            kind: 14,
            tags: vec![vec!["p".to_string(), recipient_pubkey.to_hex()]],
            content: "secret".to_string(),
        };

        let seal = nip17::create_seal(inner, &sender, &recipient_pubkey).expect("seal");
        let gift_wrap = nip17::create_gift_wrap(&seal, &recipient_pubkey).expect("gift wrap");
        let error =
            nip17::open_gift_wrap(&gift_wrap, &wrong_recipient).expect_err("wrong recipient");
        assert!(matches!(
            error,
            SecpError::InvalidNip44(_) | SecpError::InvalidEvent(_)
        ));
    }

    #[cfg(feature = "nip19")]
    #[test]
    fn nip19_nsec_roundtrip() {
        let secret = SecretKey::generate().expect("secret key");
        let encoded = nip19::encode_nsec(&secret).expect("encode");
        let decoded = nip19::decode(&encoded).expect("decode");
        assert_eq!(decoded, Nip19::Nsec(secret));
    }

    #[cfg(feature = "nip19")]
    #[test]
    fn nip19_npub_roundtrip() {
        let secret = SecretKey::generate().expect("secret key");
        let public = secret.xonly_public_key().expect("public key");
        let encoded = nip19::encode_npub(&public).expect("encode");
        let decoded = nip19::decode(&encoded).expect("decode");
        assert_eq!(decoded, Nip19::Npub(public));
    }

    #[cfg(feature = "nip19")]
    #[test]
    fn keybundle_nip19() {
        let bundle = KeyBundle::generate().expect("key bundle");
        let npub = bundle.npub().expect("npub");
        let nsec = bundle.nsec().expect("nsec");
        assert_eq!(
            nip19::decode(&npub).expect("decode npub"),
            Nip19::Npub(*bundle.xonly_public_key())
        );
        assert_eq!(
            nip19::decode(&nsec).expect("decode nsec"),
            Nip19::Nsec(*bundle.secret())
        );
    }

    #[cfg(feature = "nip19")]
    #[test]
    fn nip19_note_roundtrip() {
        let id = EventId::from_bytes([0x42; 32]);
        let encoded = nip19::encode_note(&id).expect("encode");
        let decoded = nip19::decode(&encoded).expect("decode");
        assert_eq!(decoded, Nip19::Note(id));
    }

    #[cfg(feature = "nip19")]
    #[test]
    fn nip19_decodes_known_valid_npub_fixture() {
        let public = XOnlyPublicKey::from_bytes([
            0x6e, 0x46, 0x84, 0x22, 0xdf, 0xb7, 0x4a, 0x57, 0x38, 0x70, 0x2a, 0x88, 0x23, 0xb9,
            0xb2, 0x81, 0x68, 0xab, 0xab, 0x86, 0x55, 0xfa, 0xac, 0xb6, 0x85, 0x3c, 0xd0, 0xee,
            0x15, 0xde, 0xee, 0x93,
        ])
        .expect("fixture pubkey");
        let decoded =
            nip19::decode("npub1dergggklka99wwrs92yz8wdjs952h2ux2ha2ed598ngwu9w7a6fsh9xzpc")
                .expect("decode fixture");
        assert_eq!(decoded, Nip19::Npub(public));
    }

    #[cfg(feature = "nip19")]
    #[test]
    fn nip19_matches_known_nostr_tools_fixture() {
        let pubkey = XOnlyPublicKey::from_hex(
            "6af0f9de588f2c53cedcba26c5e2402e0d0aa64ec7b47c9f8d97b5bc562bab5f",
        )
        .expect("pubkey");
        let event_id =
            EventId::from_hex("b2a44af84ca99b14820ae91c44e1ef0908f8aadc4e10620a6e6caa344507f03c")
                .expect("event id");
        let profile = NProfile {
            pubkey,
            relays: vec![
                "wss://relay.damus.io".to_string(),
                "wss://nostr.example.com".to_string(),
            ],
        };
        let event = NEvent {
            id: event_id,
            relays: vec![
                "wss://relay.example.com".to_string(),
                "wss://relay2.example.com".to_string(),
            ],
            author: Some(pubkey),
            kind: Some(1),
        };
        let addr = NAddr {
            identifier: "article-1".to_string(),
            relays: vec![
                "wss://relay.example.com".to_string(),
                "wss://relay2.example.com".to_string(),
            ],
            author: pubkey,
            kind: 30_023,
        };

        assert_eq!(
            nip19::encode_npub(&pubkey).expect("npub"),
            "npub1dtc0nhjc3uk98nkuhgnvtcjq9cxs4fjwc768e8udj76mc43t4d0sw73h32"
        );
        assert_eq!(
            nip19::encode_nprofile(&profile).expect("nprofile"),
            "nprofile1qy28wumn8ghj7un9d3shjtnyv9kh2uewd9hsz9mhwden5te0dehhxarj9ejhsctdwpkx2tnrdaksqgr27ruauky093fuah96ymz7yspwp592vnk8k37flrvhkk79v2attuwkrkwx"
        );
        assert_eq!(
            nip19::encode_nevent(&event).expect("nevent"),
            "nevent1qvzqqqqqqypzq6hsl8093rev208dew3xch3yqtsdp2nya3a50j0cm9a4h3tzh26lqythwumn8ghj7un9d3shjtn90psk6urvv5hxxmmdqyv8wumn8ghj7un9d3shjv3wv4uxzmtsd3jjucm0d5qzpv4yftuye2vmzjpq46gugns77zgglz4dcnssvg9xum92x3zs0upuv94f3z"
        );
        assert_eq!(
            nip19::encode_naddr(&addr).expect("naddr"),
            "naddr1qvzqqqr4gupzq6hsl8093rev208dew3xch3yqtsdp2nya3a50j0cm9a4h3tzh26lqythwumn8ghj7un9d3shjtn90psk6urvv5hxxmmdqyv8wumn8ghj7un9d3shjv3wv4uxzmtsd3jjucm0d5qqjctjw35kxmr995csn0d4es"
        );
    }

    #[cfg(feature = "nip19")]
    #[test]
    fn nip19_nprofile_roundtrip_with_multiple_relays() {
        let pubkey = XOnlyPublicKey::from_bytes([
            0x6e, 0x46, 0x84, 0x22, 0xdf, 0xb7, 0x4a, 0x57, 0x38, 0x70, 0x2a, 0x88, 0x23, 0xb9,
            0xb2, 0x81, 0x68, 0xab, 0xab, 0x86, 0x55, 0xfa, 0xac, 0xb6, 0x85, 0x3c, 0xd0, 0xee,
            0x15, 0xde, 0xee, 0x93,
        ])
        .expect("pubkey");
        let profile = NProfile {
            pubkey,
            relays: vec![
                "wss://relay.example".to_string(),
                "wss://relay2.example".to_string(),
            ],
        };
        let encoded = nip19::encode_nprofile(&profile).expect("encode");
        let decoded = nip19::decode(&encoded).expect("decode");
        assert_eq!(decoded, Nip19::NProfile(profile));
    }

    #[cfg(feature = "nip19")]
    #[test]
    fn nip19_nevent_roundtrip_with_author_and_kind() {
        let event = NEvent {
            id: EventId::from_bytes([0x34; 32]),
            relays: vec!["wss://relay.example".to_string()],
            author: Some(
                XOnlyPublicKey::from_bytes([
                    0x4f, 0x35, 0x5b, 0xdc, 0xb7, 0xcc, 0x0a, 0xf7, 0x28, 0xef, 0x3c, 0xce, 0xb9,
                    0x61, 0x5d, 0x90, 0x68, 0x4b, 0xb5, 0xb2, 0xca, 0x5f, 0x85, 0x9a, 0xb0, 0xf0,
                    0xb7, 0x04, 0x07, 0x58, 0x71, 0xaa,
                ])
                .expect("author"),
            ),
            kind: Some(30_023),
        };
        let encoded = nip19::encode_nevent(&event).expect("encode");
        let decoded = nip19::decode(&encoded).expect("decode");
        assert_eq!(decoded, Nip19::NEvent(event));
    }

    #[cfg(feature = "nip19")]
    #[test]
    fn nip19_naddr_roundtrip_allows_empty_identifier() {
        let addr = NAddr {
            identifier: String::new(),
            relays: vec!["wss://relay.example".to_string()],
            author: XOnlyPublicKey::from_bytes([
                0x6e, 0x46, 0x84, 0x22, 0xdf, 0xb7, 0x4a, 0x57, 0x38, 0x70, 0x2a, 0x88, 0x23, 0xb9,
                0xb2, 0x81, 0x68, 0xab, 0xab, 0x86, 0x55, 0xfa, 0xac, 0xb6, 0x85, 0x3c, 0xd0, 0xee,
                0x15, 0xde, 0xee, 0x93,
            ])
            .expect("author"),
            kind: 30_023,
        };
        let encoded = nip19::encode_naddr(&addr).expect("encode");
        let decoded = nip19::decode(&encoded).expect("decode");
        assert_eq!(decoded, Nip19::NAddr(addr));
    }

    #[cfg(feature = "nip19")]
    #[test]
    fn nip19_nrelay_roundtrip() {
        let relay = NRelay {
            relay: "wss://relay.example".to_string(),
        };
        let encoded = nip19::encode_nrelay(&relay).expect("encode");
        let decoded = nip19::decode(&encoded).expect("decode");
        assert_eq!(decoded, Nip19::NRelay(relay));
    }

    #[cfg(feature = "nip19")]
    #[test]
    fn nip19_decode_ignores_unknown_tlv_entries() {
        use bech32::ToBase32;

        let mut payload = vec![9, 3, b'x', b'y', b'z', 0, 32];
        payload.extend_from_slice(&[
            0x6e, 0x46, 0x84, 0x22, 0xdf, 0xb7, 0x4a, 0x57, 0x38, 0x70, 0x2a, 0x88, 0x23, 0xb9,
            0xb2, 0x81, 0x68, 0xab, 0xab, 0x86, 0x55, 0xfa, 0xac, 0xb6, 0x85, 0x3c, 0xd0, 0xee,
            0x15, 0xde, 0xee, 0x93,
        ]);
        let encoded = bech32::encode("nprofile", payload.to_base32(), bech32::Variant::Bech32)
            .expect("bech32");
        let decoded = nip19::decode(&encoded).expect("decode");
        assert_eq!(
            decoded,
            Nip19::NProfile(NProfile {
                pubkey: XOnlyPublicKey::from_bytes([
                    0x6e, 0x46, 0x84, 0x22, 0xdf, 0xb7, 0x4a, 0x57, 0x38, 0x70, 0x2a, 0x88, 0x23,
                    0xb9, 0xb2, 0x81, 0x68, 0xab, 0xab, 0x86, 0x55, 0xfa, 0xac, 0xb6, 0x85, 0x3c,
                    0xd0, 0xee, 0x15, 0xde, 0xee, 0x93,
                ])
                .expect("pubkey"),
                relays: Vec::new(),
            })
        );
    }

    #[cfg(feature = "nip19")]
    #[test]
    fn nip19_decode_rejects_missing_required_nprofile_pubkey() {
        use bech32::ToBase32;

        let payload = vec![1, 5, b'r', b'e', b'l', b'a', b'y'];
        let encoded = bech32::encode("nprofile", payload.to_base32(), bech32::Variant::Bech32)
            .expect("bech32");
        let error = nip19::decode(&encoded).expect_err("missing pubkey must fail");
        assert!(matches!(
            error,
            SecpError::InvalidNip19("missing TLV 0 for nprofile")
        ));
    }

    #[cfg(feature = "nip19")]
    #[test]
    fn nip19_decode_rejects_missing_required_naddr_fields() {
        use bech32::ToBase32;

        let mut payload = vec![0, 0];
        payload.extend_from_slice(&[3, 4, 0, 0, 0x75, 0x37]);
        let encoded =
            bech32::encode("naddr", payload.to_base32(), bech32::Variant::Bech32).expect("bech32");
        let error = nip19::decode(&encoded).expect_err("missing author must fail");
        assert!(matches!(
            error,
            SecpError::InvalidNip19("missing TLV 2 for naddr")
        ));
    }

    #[cfg(feature = "nip19")]
    #[test]
    fn nip19_decode_rejects_invalid_nevent_kind_length() {
        use bech32::ToBase32;

        let mut payload = vec![0, 32];
        payload.extend_from_slice(&[0x34; 32]);
        payload.extend_from_slice(&[3, 3, 0, 0x75, 0x37]);
        let encoded =
            bech32::encode("nevent", payload.to_base32(), bech32::Variant::Bech32).expect("bech32");
        let error = nip19::decode(&encoded).expect_err("invalid kind length must fail");
        assert!(matches!(
            error,
            SecpError::InvalidNip19("TLV 3 should be 4 bytes")
        ));
    }

    #[cfg(feature = "nip19")]
    #[test]
    fn nip19_nevent_kind_is_big_endian() {
        let event = NEvent {
            id: EventId::from_bytes([0x12; 32]),
            relays: Vec::new(),
            author: None,
            kind: Some(30_023),
        };
        let encoded = nip19::encode_nevent(&event).expect("encode");
        let decoded = nip19::decode(&encoded).expect("decode");
        assert_eq!(decoded, Nip19::NEvent(event));
    }

    #[cfg(feature = "nip19")]
    #[test]
    fn nip19_decode_rejects_invalid_checksum() {
        let error =
            nip19::decode("npub1de5gss7lkafc0pe2s2sz8wjsx6v4hvxxg8l6e60v8uuguvs7m5fsq4qwxn")
                .expect_err("checksum must fail");
        assert!(matches!(error, SecpError::InvalidNip19(_)));
    }

    #[cfg(feature = "nip19")]
    #[test]
    fn nip19_decode_rejects_invalid_length() {
        use bech32::ToBase32;

        let short = bech32::encode("npub", vec![1u8; 31].to_base32(), bech32::Variant::Bech32)
            .expect("fixture encode");
        let error = nip19::decode(&short).expect_err("short payload must fail");
        assert!(matches!(error, SecpError::InvalidNip19(_)));
    }

    #[cfg(feature = "nip44")]
    #[test]
    fn nip44_conversation_key_matches_from_both_sides() {
        let secret_a = SecretKey::from_bytes([0x11; 32]).expect("secret a");
        let secret_b = SecretKey::from_bytes([0x22; 32]).expect("secret b");
        let pubkey_a = secret_a.xonly_public_key().expect("pubkey a");
        let pubkey_b = secret_b.xonly_public_key().expect("pubkey b");

        let key_ab = nip44::get_conversation_key(&secret_a, &pubkey_b).expect("key ab");
        let key_ba = nip44::get_conversation_key(&secret_b, &pubkey_a).expect("key ba");
        assert_eq!(key_ab, key_ba);
    }

    #[cfg(feature = "nip44")]
    #[test]
    fn nip44_encrypt_and_decrypt_roundtrip() {
        let secret = SecretKey::from_bytes([0x11; 32]).expect("secret");
        let peer = SecretKey::from_bytes([0x22; 32]).expect("peer");
        let peer_pubkey = peer.xonly_public_key().expect("peer pubkey");
        let conversation_key =
            nip44::get_conversation_key(&secret, &peer_pubkey).expect("conversation key");
        let nonce = [0x33; 32];

        let payload = nip44::encrypt("hello from neco-secp", &conversation_key, Some(nonce))
            .expect("encrypt");
        let plaintext = nip44::decrypt(&payload, &conversation_key).expect("decrypt");
        assert_eq!(plaintext, "hello from neco-secp");
    }

    #[cfg(feature = "nip44")]
    #[test]
    fn nip44_matches_known_oracle_fixture() {
        let secret = SecretKey::from_bytes([0x11; 32]).expect("secret");
        let peer_pubkey = XOnlyPublicKey::from_bytes([
            0x46, 0x6d, 0x7f, 0xca, 0xe5, 0x63, 0xe5, 0xcb, 0x09, 0xa0, 0xd1, 0x87, 0x0b, 0xb5,
            0x80, 0x34, 0x48, 0x04, 0x61, 0x78, 0x79, 0xa1, 0x49, 0x49, 0xcf, 0x22, 0x28, 0x5f,
            0x1b, 0xae, 0x3f, 0x27,
        ])
        .expect("peer pubkey");
        let conversation_key =
            nip44::get_conversation_key(&secret, &peer_pubkey).expect("conversation key");
        assert_eq!(
            conversation_key,
            [
                0x2c, 0xbd, 0xf0, 0x74, 0xf6, 0x01, 0x17, 0x8c, 0x24, 0xda, 0x3f, 0x82, 0x9b, 0x50,
                0x45, 0x07, 0xa1, 0xf5, 0x50, 0xf9, 0x7d, 0x47, 0x2a, 0xf0, 0xf3, 0xf2, 0xcc, 0x59,
                0xab, 0x77, 0x57, 0xd1,
            ]
        );

        let payload = nip44::encrypt(ORACLE_NIP44_PLAINTEXT, &conversation_key, Some([0x33; 32]))
            .expect("encrypt");
        assert_eq!(
            payload,
            "AjMzMzMzMzMzMzMzMzMzMzMzMzMzMzMzMzMzMzMzMzMzJDZzpLQFdobg13n/RufVeG0ps8acSBfghr22oozB/q91IVexzbaA/lxkSa0R+6Dly9F1gKsZLCy1tzW4LPplhuWg"
        );
        let plaintext = nip44::decrypt(&payload, &conversation_key).expect("decrypt");
        assert_eq!(plaintext, ORACLE_NIP44_PLAINTEXT);
    }

    #[cfg(feature = "nip44")]
    #[test]
    fn nip44_calc_padded_len_contract() {
        assert_eq!(nip44::calc_padded_len(1).expect("len"), 32);
        assert_eq!(nip44::calc_padded_len(32).expect("len"), 32);
        assert_eq!(nip44::calc_padded_len(33).expect("len"), 64);
        assert_eq!(nip44::calc_padded_len(300).expect("len"), 320);
    }

    #[cfg(feature = "nip44")]
    #[test]
    fn nip44_rejects_invalid_mac() {
        let secret = SecretKey::from_bytes([0x11; 32]).expect("secret");
        let peer = SecretKey::from_bytes([0x22; 32]).expect("peer");
        let peer_pubkey = peer.xonly_public_key().expect("peer pubkey");
        let conversation_key =
            nip44::get_conversation_key(&secret, &peer_pubkey).expect("conversation key");
        let nonce = [0x33; 32];

        let payload = nip44::encrypt("hello", &conversation_key, Some(nonce)).expect("encrypt");
        let mut raw = neco_base64::decode(&payload).expect("decode");
        let last = raw.len() - 1;
        raw[last] ^= 0x01;
        let tampered = neco_base64::encode(&raw);

        let error = nip44::decrypt(&tampered, &conversation_key).expect_err("invalid mac");
        assert!(matches!(error, SecpError::InvalidNip44("invalid MAC")));
    }

    #[cfg(feature = "nip44")]
    #[test]
    fn nip44_rejects_invalid_version() {
        let secret = SecretKey::from_bytes([0x11; 32]).expect("secret");
        let peer = SecretKey::from_bytes([0x22; 32]).expect("peer");
        let peer_pubkey = peer.xonly_public_key().expect("peer pubkey");
        let conversation_key =
            nip44::get_conversation_key(&secret, &peer_pubkey).expect("conversation key");
        let nonce = [0x33; 32];

        let payload = nip44::encrypt("hello", &conversation_key, Some(nonce)).expect("encrypt");
        let mut raw = neco_base64::decode(&payload).expect("decode");
        raw[0] = 3;
        let tampered = neco_base64::encode(&raw);

        let error = nip44::decrypt(&tampered, &conversation_key).expect_err("invalid version");
        assert!(matches!(
            error,
            SecpError::InvalidNip44("unknown encryption version")
        ));
    }

    #[cfg(feature = "nip44")]
    #[test]
    fn nip44_rejects_invalid_payload_length() {
        let error = nip44::decrypt("short", &[0u8; 32]).expect_err("invalid payload");
        assert!(matches!(
            error,
            SecpError::InvalidNip44("invalid payload length")
        ));
    }

    #[cfg(feature = "nip04")]
    #[test]
    fn nip04_encrypt_and_decrypt_roundtrip() {
        let secret = SecretKey::from_bytes([0x11; 32]).expect("secret");
        let peer = SecretKey::from_bytes([0x22; 32]).expect("peer");
        let peer_pubkey = peer.xonly_public_key().expect("peer pubkey");
        let payload = nip04::encrypt(
            &secret,
            &peer_pubkey,
            "hello from neco-secp",
            Some([0x44; 16]),
        )
        .expect("encrypt");
        let plaintext =
            nip04::decrypt(&peer, &secret.xonly_public_key().expect("pubkey"), &payload)
                .expect("decrypt");
        assert_eq!(plaintext, "hello from neco-secp");
    }

    #[cfg(feature = "nip04")]
    #[test]
    fn nip04_matches_known_oracle_fixture() {
        let secret = SecretKey::from_bytes([0x11; 32]).expect("secret");
        let peer_pubkey = XOnlyPublicKey::from_bytes([
            0x46, 0x6d, 0x7f, 0xca, 0xe5, 0x63, 0xe5, 0xcb, 0x09, 0xa0, 0xd1, 0x87, 0x0b, 0xb5,
            0x80, 0x34, 0x48, 0x04, 0x61, 0x78, 0x79, 0xa1, 0x49, 0x49, 0xcf, 0x22, 0x28, 0x5f,
            0x1b, 0xae, 0x3f, 0x27,
        ])
        .expect("peer pubkey");
        let payload = nip04::encrypt(
            &secret,
            &peer_pubkey,
            ORACLE_NIP04_PLAINTEXT,
            Some([0x44; 16]),
        )
        .expect("encrypt");
        assert_eq!(
            payload,
            "xftPpDirMJGDoq3ktNutZsG6W+lmUsILU9XMp06pYmM=?iv=RERERERERERERERERERERA=="
        );
    }

    #[cfg(feature = "nip04")]
    #[test]
    fn nip04_rejects_invalid_payload() {
        let secret = SecretKey::from_bytes([0x11; 32]).expect("secret");
        let peer_pubkey = SecretKey::from_bytes([0x22; 32])
            .expect("peer")
            .xonly_public_key()
            .expect("pubkey");
        let error = nip04::decrypt(&secret, &peer_pubkey, "invalid").expect_err("invalid payload");
        assert!(matches!(error, SecpError::InvalidNip04("invalid payload")));
    }

    #[cfg(feature = "nip04")]
    #[test]
    fn nip04_rejects_invalid_iv() {
        let secret = SecretKey::from_bytes([0x11; 32]).expect("secret");
        let peer_pubkey = SecretKey::from_bytes([0x22; 32])
            .expect("peer")
            .xonly_public_key()
            .expect("pubkey");
        let error = nip04::decrypt(&secret, &peer_pubkey, "abcd?iv=bad!").expect_err("invalid iv");
        assert!(matches!(error, SecpError::InvalidNip04("invalid iv")));
    }
}
