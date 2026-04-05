//! Minimal CIDv1 and multibase core.

use core::fmt;

use sha2::{Digest, Sha256};

#[cfg(feature = "cbor")]
mod cbor;
#[cfg(feature = "cbor")]
pub use cbor::CborCidError;

const CID_VERSION_V1: u64 = 1;
const SHA2_256_CODE: u64 = 0x12;
const SHA2_256_DIGEST_LEN: usize = 32;
const BASE32_ALPHABET: &[u8; 32] = b"abcdefghijklmnopqrstuvwxyz234567";

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Cid {
    version: u64,
    codec: Codec,
    hash_code: u64,
    digest: [u8; SHA2_256_DIGEST_LEN],
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u64)]
pub enum Codec {
    DagCbor = 0x71,
    Raw = 0x55,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Base {
    Base32Lower,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CidError {
    InvalidVersion(u64),
    UnsupportedCodec(u64),
    UnsupportedHashCode(u64),
    InvalidDigestLength,
    InvalidMultibase,
    UnexpectedEnd,
    VarintOverflow,
}

impl fmt::Display for CidError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidVersion(version) => write!(f, "invalid CID version: {version}"),
            Self::UnsupportedCodec(codec) => write!(f, "unsupported codec: {codec}"),
            Self::UnsupportedHashCode(code) => write!(f, "unsupported hash code: {code}"),
            Self::InvalidDigestLength => f.write_str("invalid digest length"),
            Self::InvalidMultibase => f.write_str("invalid multibase"),
            Self::UnexpectedEnd => f.write_str("unexpected end of input"),
            Self::VarintOverflow => f.write_str("varint exceeds 64-bit range"),
        }
    }
}

impl std::error::Error for CidError {}

impl Cid {
    pub fn compute(codec: Codec, data: &[u8]) -> Self {
        let digest = Sha256::digest(data);
        let mut digest_bytes = [0u8; SHA2_256_DIGEST_LEN];
        digest_bytes.copy_from_slice(&digest);

        Self {
            version: CID_VERSION_V1,
            codec,
            hash_code: SHA2_256_CODE,
            digest: digest_bytes,
        }
    }

    pub fn from_bytes(input: &[u8]) -> Result<(Self, usize), CidError> {
        let (version, mut offset) = decode_varint(input)?;
        if version != CID_VERSION_V1 {
            return Err(CidError::InvalidVersion(version));
        }

        let (codec_raw, consumed) = decode_varint(&input[offset..])?;
        offset += consumed;
        let codec = decode_codec(codec_raw)?;

        let (hash_code, consumed) = decode_varint(&input[offset..])?;
        offset += consumed;
        if hash_code != SHA2_256_CODE {
            return Err(CidError::UnsupportedHashCode(hash_code));
        }

        let (digest_len, consumed) = decode_varint(&input[offset..])?;
        offset += consumed;
        if digest_len != SHA2_256_DIGEST_LEN as u64 {
            return Err(CidError::InvalidDigestLength);
        }

        let end = offset + SHA2_256_DIGEST_LEN;
        if input.len() < end {
            return Err(CidError::UnexpectedEnd);
        }

        let mut digest = [0u8; SHA2_256_DIGEST_LEN];
        digest.copy_from_slice(&input[offset..end]);

        Ok((
            Self {
                version,
                codec,
                hash_code,
                digest,
            },
            end,
        ))
    }

    pub fn to_bytes(&self) -> Vec<u8> {
        let mut out = Vec::with_capacity(4 + SHA2_256_DIGEST_LEN);
        encode_varint_into(self.version, &mut out);
        encode_varint_into(self.codec as u64, &mut out);
        encode_varint_into(self.hash_code, &mut out);
        encode_varint_into(SHA2_256_DIGEST_LEN as u64, &mut out);
        out.extend_from_slice(&self.digest);
        out
    }

    pub fn to_multibase(&self, base: Base) -> String {
        match base {
            Base::Base32Lower => {
                let binary = self.to_bytes();
                let mut encoded = String::with_capacity(1 + (binary.len() * 8).div_ceil(5));
                encoded.push('b');
                encoded.push_str(&base32lower_encode(&binary));
                encoded
            }
        }
    }

    pub fn from_multibase(input: &str) -> Result<Self, CidError> {
        let payload = input.strip_prefix('b').ok_or(CidError::InvalidMultibase)?;
        let bytes = base32lower_decode(payload)?;
        let (cid, consumed) = Self::from_bytes(&bytes).map_err(|error| match error {
            CidError::UnexpectedEnd => CidError::InvalidMultibase,
            other => other,
        })?;
        if consumed != bytes.len() {
            return Err(CidError::InvalidMultibase);
        }
        Ok(cid)
    }

    pub fn codec(&self) -> Codec {
        self.codec
    }

    pub fn digest(&self) -> &[u8; SHA2_256_DIGEST_LEN] {
        &self.digest
    }
}

fn decode_codec(value: u64) -> Result<Codec, CidError> {
    match value {
        0x71 => Ok(Codec::DagCbor),
        0x55 => Ok(Codec::Raw),
        other => Err(CidError::UnsupportedCodec(other)),
    }
}

fn encode_varint_into(mut value: u64, out: &mut Vec<u8>) {
    loop {
        let lower = (value & 0x7f) as u8;
        value >>= 7;
        if value == 0 {
            out.push(lower);
            return;
        }
        out.push(lower | 0x80);
    }
}

fn decode_varint(input: &[u8]) -> Result<(u64, usize), CidError> {
    let mut value = 0u64;
    let mut shift = 0u32;

    for (index, &byte) in input.iter().enumerate() {
        let chunk = u64::from(byte & 0x7f);
        value |= chunk << shift;
        if byte & 0x80 == 0 {
            return Ok((value, index + 1));
        }
        shift += 7;
        if shift >= 64 {
            return Err(CidError::VarintOverflow);
        }
    }

    Err(CidError::UnexpectedEnd)
}

pub fn base32lower_encode(input: &[u8]) -> String {
    if input.is_empty() {
        return String::new();
    }

    let mut output = String::with_capacity((input.len() * 8).div_ceil(5));
    let mut buffer = 0u16;
    let mut bits = 0u8;

    for &byte in input {
        buffer = (buffer << 8) | u16::from(byte);
        bits += 8;
        while bits >= 5 {
            bits -= 5;
            let index = ((buffer >> bits) & 0x1f) as usize;
            output.push(BASE32_ALPHABET[index] as char);
        }
    }

    if bits > 0 {
        let index = ((buffer << (5 - bits)) & 0x1f) as usize;
        output.push(BASE32_ALPHABET[index] as char);
    }

    output
}

fn base32lower_decode(input: &str) -> Result<Vec<u8>, CidError> {
    if input.is_empty() {
        return Err(CidError::InvalidMultibase);
    }

    let mut output = Vec::with_capacity((input.len() * 5) / 8);
    let mut buffer = 0u32;
    let mut bits = 0u8;

    for byte in input.bytes() {
        let value = decode_base32_char(byte)?;
        buffer = (buffer << 5) | u32::from(value);
        bits += 5;
        while bits >= 8 {
            bits -= 8;
            output.push(((buffer >> bits) & 0xff) as u8);
        }
    }

    if bits > 0 {
        let mask = (1u32 << bits) - 1;
        if buffer & mask != 0 {
            return Err(CidError::InvalidMultibase);
        }
    }

    Ok(output)
}

fn decode_base32_char(byte: u8) -> Result<u8, CidError> {
    match byte {
        b'a'..=b'z' => Ok(byte - b'a'),
        b'2'..=b'7' => Ok(byte - b'2' + 26),
        _ => Err(CidError::InvalidMultibase),
    }
}
