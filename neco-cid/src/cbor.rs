use core::fmt;

use neco_cbor::CborValue;

use crate::{Cid, CidError};

const CID_CBOR_TAG: u64 = 42;
const MULTIBASE_IDENTITY_PREFIX: u8 = 0x00;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CborCidError {
    NotATag,
    WrongTag(u64),
    NotBytes,
    MissingIdentityPrefix,
    InvalidCid(CidError),
    TrailingData,
}

impl fmt::Display for CborCidError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NotATag => f.write_str("expected CBOR tag"),
            Self::WrongTag(tag) => write!(f, "expected tag 42, got tag {tag}"),
            Self::NotBytes => f.write_str("tag 42 payload must be bytes"),
            Self::MissingIdentityPrefix => {
                f.write_str("tag 42 payload missing 0x00 identity prefix")
            }
            Self::InvalidCid(e) => write!(f, "invalid binary CID: {e}"),
            Self::TrailingData => f.write_str("trailing data after CID"),
        }
    }
}

impl std::error::Error for CborCidError {}

impl From<CidError> for CborCidError {
    fn from(e: CidError) -> Self {
        Self::InvalidCid(e)
    }
}

impl Cid {
    /// IPLD 標準の CBOR tag 42 としてエンコードする。
    /// 構造: Tag(42, Bytes([0x00] ++ binary_cid))
    pub fn to_cbor_tag(&self) -> CborValue {
        let cid_bytes = self.to_bytes();
        let mut payload = Vec::with_capacity(cid_bytes.len() + 1);
        payload.push(MULTIBASE_IDENTITY_PREFIX);
        payload.extend_from_slice(&cid_bytes);
        CborValue::Tag(CID_CBOR_TAG, Box::new(CborValue::Bytes(payload)))
    }

    /// CBOR tag 42 から CID をデコードする。
    pub fn from_cbor_tag(value: &CborValue) -> Result<Cid, CborCidError> {
        let (tag, payload) = value.as_tag().ok_or(CborCidError::NotATag)?;
        if tag != CID_CBOR_TAG {
            return Err(CborCidError::WrongTag(tag));
        }
        let bytes = payload.as_bytes().ok_or(CborCidError::NotBytes)?;
        let (prefix, rest) = bytes
            .split_first()
            .ok_or(CborCidError::MissingIdentityPrefix)?;
        if *prefix != MULTIBASE_IDENTITY_PREFIX {
            return Err(CborCidError::MissingIdentityPrefix);
        }
        let (cid, consumed) = Cid::from_bytes(rest).map_err(CborCidError::InvalidCid)?;
        if consumed != rest.len() {
            return Err(CborCidError::TrailingData);
        }
        Ok(cid)
    }

    /// null なら None、tag 42 なら Some(Cid) を返す。
    pub fn from_cbor_tag_optional(value: &CborValue) -> Result<Option<Cid>, CborCidError> {
        if value.is_null() {
            return Ok(None);
        }
        Self::from_cbor_tag(value).map(Some)
    }
}
