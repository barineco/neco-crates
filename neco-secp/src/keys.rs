use crate::hex::{hex_decode, hex_encode};
use crate::{EcdsaSignature, SchnorrSignature, SecpError};

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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SecretKey {
    pub(crate) bytes: [u8; 32],
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

    pub(crate) fn signing_key(&self) -> Result<SigningKey, SecpError> {
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
    pub(crate) bytes: [u8; 32],
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

#[cfg(any(feature = "nip04", feature = "nip44"))]
pub(crate) fn decode_xonly_pubkey(pubkey: &XOnlyPublicKey) -> Result<k256::PublicKey, SecpError> {
    let mut sec1 = [0u8; 33];
    sec1[0] = 0x02;
    sec1[1..].copy_from_slice(&pubkey.to_bytes());
    k256::PublicKey::from_sec1_bytes(&sec1).map_err(|_| SecpError::InvalidPublicKey)
}
