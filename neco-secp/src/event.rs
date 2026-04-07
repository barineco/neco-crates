use crate::hex::{hex_decode, hex_encode};
use crate::{SchnorrSignature, SecpError, XOnlyPublicKey};
#[cfg(feature = "serde")]
use serde::{Deserialize, Deserializer, Serialize, Serializer};

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
    Nsec(crate::SecretKey),
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
