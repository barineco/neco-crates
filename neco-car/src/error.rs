use core::fmt;

#[derive(Debug)]
pub enum CarError {
    // parse
    UnexpectedEnd,
    VarintOverflow,
    InvalidHeader(neco_cbor::DecodeErrorKind),
    HeaderNotMap,
    MissingHeaderField(&'static str),
    UnsupportedVersion(u64),
    RootsNotArray,
    InvalidRootCid(neco_cid::CidError),
    InvalidBlockCid(neco_cid::CidError),
    BlockLengthMismatch,
    EmptySection,
    InvalidCidLink,
    // write
    HeaderEncode(neco_cbor::EncodeError),
}

impl fmt::Display for CarError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnexpectedEnd => f.write_str("unexpected end of input"),
            Self::VarintOverflow => f.write_str("varint exceeds 64-bit range"),
            Self::InvalidHeader(kind) => write!(f, "invalid header: {kind}"),
            Self::HeaderNotMap => f.write_str("header is not a map"),
            Self::MissingHeaderField(field) => write!(f, "missing header field: {field}"),
            Self::UnsupportedVersion(version) => {
                write!(f, "unsupported CAR version: {version}")
            }
            Self::RootsNotArray => f.write_str("roots field is not an array"),
            Self::InvalidRootCid(err) => write!(f, "invalid root CID: {err}"),
            Self::InvalidBlockCid(err) => write!(f, "invalid block CID: {err}"),
            Self::BlockLengthMismatch => f.write_str("block length mismatch"),
            Self::EmptySection => f.write_str("empty section"),
            Self::InvalidCidLink => {
                f.write_str("invalid CID link (expected tag 42 with 0x00 prefix)")
            }
            Self::HeaderEncode(err) => write!(f, "header encode error: {err}"),
        }
    }
}

impl std::error::Error for CarError {}

impl PartialEq for CarError {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Self::UnexpectedEnd, Self::UnexpectedEnd) => true,
            (Self::VarintOverflow, Self::VarintOverflow) => true,
            (Self::InvalidHeader(a), Self::InvalidHeader(b)) => a == b,
            (Self::HeaderNotMap, Self::HeaderNotMap) => true,
            (Self::MissingHeaderField(a), Self::MissingHeaderField(b)) => a == b,
            (Self::UnsupportedVersion(a), Self::UnsupportedVersion(b)) => a == b,
            (Self::RootsNotArray, Self::RootsNotArray) => true,
            (Self::InvalidRootCid(a), Self::InvalidRootCid(b)) => a == b,
            (Self::InvalidBlockCid(a), Self::InvalidBlockCid(b)) => a == b,
            (Self::BlockLengthMismatch, Self::BlockLengthMismatch) => true,
            (Self::EmptySection, Self::EmptySection) => true,
            (Self::InvalidCidLink, Self::InvalidCidLink) => true,
            (Self::HeaderEncode(a), Self::HeaderEncode(b)) => a == b,
            _ => false,
        }
    }
}
