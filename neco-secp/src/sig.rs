#[cfg(feature = "serde")]
use crate::hex::{hex_decode, hex_encode};
#[cfg(feature = "serde")]
use serde::{Deserialize, Deserializer, Serialize, Serializer};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EcdsaSignature {
    pub(crate) bytes: [u8; 64],
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
    pub(crate) bytes: [u8; 64],
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
