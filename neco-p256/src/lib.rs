//! Minimal P-256 ECDSA signing core.

use core::fmt;

use p256::ecdsa::signature::hazmat::{PrehashSigner, PrehashVerifier};
use p256::ecdsa::{Signature as P256Signature, SigningKey, VerifyingKey};
use p256::elliptic_curve::rand_core::OsRng;
use p256::elliptic_curve::scalar::IsHigh;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum P256Error {
    InvalidSecretKey,
    InvalidPublicKey,
    InvalidSignature,
    InvalidHex(&'static str),
}

impl fmt::Display for P256Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidSecretKey => f.write_str("invalid secret key"),
            Self::InvalidPublicKey => f.write_str("invalid public key"),
            Self::InvalidSignature => f.write_str("invalid signature"),
            Self::InvalidHex(message) => f.write_str(message),
        }
    }
}

impl std::error::Error for P256Error {}

fn hex_encode(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut s = String::with_capacity(bytes.len() * 2);
    for &b in bytes {
        s.push(HEX[(b >> 4) as usize] as char);
        s.push(HEX[(b & 0x0f) as usize] as char);
    }
    s
}

fn hex_decode(hex: &str) -> Result<Vec<u8>, P256Error> {
    let hex = hex.as_bytes();
    if hex.len() % 2 != 0 {
        return Err(P256Error::InvalidHex("odd length"));
    }

    let mut out = Vec::with_capacity(hex.len() / 2);
    for chunk in hex.chunks(2) {
        let high = hex_nibble(chunk[0])?;
        let low = hex_nibble(chunk[1])?;
        out.push((high << 4) | low);
    }
    Ok(out)
}

fn hex_nibble(b: u8) -> Result<u8, P256Error> {
    match b {
        b'0'..=b'9' => Ok(b - b'0'),
        b'a'..=b'f' => Ok(b - b'a' + 10),
        b'A'..=b'F' => Ok(b - b'A' + 10),
        _ => Err(P256Error::InvalidHex("invalid character")),
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SecretKey {
    bytes: [u8; 32],
}

impl SecretKey {
    pub fn generate() -> Result<Self, P256Error> {
        let signing_key = SigningKey::random(&mut OsRng);
        let mut bytes = [0u8; 32];
        bytes.copy_from_slice(&signing_key.to_bytes());
        Self::from_bytes(bytes)
    }

    pub fn from_hex(hex: &str) -> Result<Self, P256Error> {
        let bytes = hex_decode(hex)?;
        if bytes.len() != 32 {
            return Err(P256Error::InvalidHex("expected 64 hex characters"));
        }

        let mut arr = [0u8; 32];
        arr.copy_from_slice(&bytes);
        Self::from_bytes(arr)
    }

    pub fn from_bytes(bytes: [u8; 32]) -> Result<Self, P256Error> {
        let _ = SigningKey::from_bytes(&bytes.into()).map_err(|_| P256Error::InvalidSecretKey)?;
        Ok(Self { bytes })
    }

    pub fn to_hex(&self) -> String {
        hex_encode(&self.bytes)
    }

    pub fn to_bytes(&self) -> [u8; 32] {
        self.bytes
    }

    pub fn public_key(&self) -> Result<PublicKey, P256Error> {
        let verifying = self.verifying_key()?;
        let encoded = verifying.to_encoded_point(true);
        let sec1_bytes = encoded
            .as_bytes()
            .try_into()
            .map_err(|_| P256Error::InvalidPublicKey)?;
        Ok(PublicKey { sec1_bytes })
    }

    pub fn sign_ecdsa_prehash(&self, digest32: [u8; 32]) -> Result<EcdsaSignature, P256Error> {
        let signing_key = self.signing_key()?;
        let signature: P256Signature = signing_key
            .sign_prehash(&digest32)
            .map_err(|_| P256Error::InvalidSignature)?;
        let signature = signature.normalize_s().unwrap_or(signature);
        let mut bytes = [0u8; 64];
        bytes.copy_from_slice(&signature.to_bytes());
        Ok(EcdsaSignature { bytes })
    }

    fn signing_key(&self) -> Result<SigningKey, P256Error> {
        SigningKey::from_bytes(&self.bytes.into()).map_err(|_| P256Error::InvalidSecretKey)
    }

    fn verifying_key(&self) -> Result<VerifyingKey, P256Error> {
        let signing_key = self.signing_key()?;
        Ok(*signing_key.verifying_key())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PublicKey {
    sec1_bytes: [u8; 33],
}

impl PublicKey {
    pub fn from_hex(hex: &str) -> Result<Self, P256Error> {
        let bytes = hex_decode(hex)?;
        if bytes.len() != 33 {
            return Err(P256Error::InvalidHex("expected 66 hex characters"));
        }
        Self::from_sec1_bytes(&bytes)
    }

    pub fn from_sec1_bytes(bytes: &[u8]) -> Result<Self, P256Error> {
        let verifying =
            VerifyingKey::from_sec1_bytes(bytes).map_err(|_| P256Error::InvalidPublicKey)?;
        let encoded = verifying.to_encoded_point(true);
        let sec1_bytes = encoded
            .as_bytes()
            .try_into()
            .map_err(|_| P256Error::InvalidPublicKey)?;
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
    ) -> Result<(), P256Error> {
        let verifying_key = self.verifying_key()?;
        let signature =
            P256Signature::from_slice(&sig.bytes).map_err(|_| P256Error::InvalidSignature)?;
        if bool::from(signature.s().is_high()) {
            return Err(P256Error::InvalidSignature);
        }
        verifying_key
            .verify_prehash(&digest32, &signature)
            .map_err(|_| P256Error::InvalidSignature)
    }

    fn verifying_key(&self) -> Result<VerifyingKey, P256Error> {
        VerifyingKey::from_sec1_bytes(&self.sec1_bytes).map_err(|_| P256Error::InvalidPublicKey)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EcdsaSignature {
    bytes: [u8; 64],
}

impl EcdsaSignature {
    pub fn from_hex(hex: &str) -> Result<Self, P256Error> {
        let bytes = hex_decode(hex)?;
        if bytes.len() != 64 {
            return Err(P256Error::InvalidHex("expected 128 hex characters"));
        }

        let mut arr = [0u8; 64];
        arr.copy_from_slice(&bytes);
        Ok(Self::from_bytes(arr))
    }

    pub fn from_bytes(bytes: [u8; 64]) -> Self {
        Self { bytes }
    }

    pub fn to_hex(&self) -> String {
        hex_encode(&self.bytes)
    }

    pub fn to_bytes(&self) -> [u8; 64] {
        self.bytes
    }
}
