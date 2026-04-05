use core::fmt;

pub const PREPARED_CANDIDATE_FORMAT_VERSION: u16 = 1;
pub const PREPARED_CANDIDATE_ALGORITHM_VERSION: u16 = 2;
pub const PREPARED_CANDIDATE_HEADER_LEN: usize = 16;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PreparedCandidateHeader {
    pub format_version: u16,
    pub algorithm_version: u16,
    pub fingerprint: u64,
    pub text_len: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EncodeError {
    BufferTooSmall { required: usize },
    TextTooLong { len: usize },
}

impl fmt::Display for EncodeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::BufferTooSmall { required } => {
                write!(
                    f,
                    "buffer too small for prepared candidate archive: need {required} bytes"
                )
            }
            Self::TextTooLong { len } => {
                write!(
                    f,
                    "prepared candidate text length {len} exceeds u32 archive header"
                )
            }
        }
    }
}

impl std::error::Error for EncodeError {}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DecodeError {
    Truncated { required: usize, actual: usize },
    UnsupportedFormatVersion { expected: u16, actual: u16 },
    UnsupportedAlgorithmVersion { expected: u16, actual: u16 },
    FingerprintMismatch { expected: u64, actual: u64 },
    PlatformLengthOverflow { text_len: u32 },
    InvalidUtf8,
}

impl fmt::Display for DecodeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Truncated { required, actual } => {
                write!(
                    f,
                    "prepared candidate archive truncated: need {required} bytes, got {actual}"
                )
            }
            Self::UnsupportedFormatVersion { expected, actual } => {
                write!(
                    f,
                    "unsupported prepared candidate format version: expected {expected}, got {actual}"
                )
            }
            Self::UnsupportedAlgorithmVersion { expected, actual } => {
                write!(
                    f,
                    "unsupported prepared candidate algorithm version: expected {expected}, got {actual}"
                )
            }
            Self::FingerprintMismatch { expected, actual } => {
                write!(
                    f,
                    "prepared candidate fingerprint mismatch: expected {expected}, got {actual}"
                )
            }
            Self::PlatformLengthOverflow { text_len } => {
                write!(
                    f,
                    "prepared candidate archive length {text_len} does not fit on this platform"
                )
            }
            Self::InvalidUtf8 => write!(f, "prepared candidate archive contains invalid utf-8"),
        }
    }
}

impl std::error::Error for DecodeError {}

pub(crate) fn checked_archive_text_len(len: usize) -> Result<u32, EncodeError> {
    u32::try_from(len).map_err(|_| EncodeError::TextTooLong { len })
}

pub(crate) fn decode_header(bytes: &[u8]) -> Result<PreparedCandidateHeader, DecodeError> {
    if bytes.len() < PREPARED_CANDIDATE_HEADER_LEN {
        return Err(DecodeError::Truncated {
            required: PREPARED_CANDIDATE_HEADER_LEN,
            actual: bytes.len(),
        });
    }

    let format_version = u16::from_le_bytes([bytes[0], bytes[1]]);
    let algorithm_version = u16::from_le_bytes([bytes[2], bytes[3]]);
    let fingerprint = u64::from_le_bytes([
        bytes[4], bytes[5], bytes[6], bytes[7], bytes[8], bytes[9], bytes[10], bytes[11],
    ]);
    let text_len = u32::from_le_bytes([bytes[12], bytes[13], bytes[14], bytes[15]]);
    Ok(PreparedCandidateHeader {
        format_version,
        algorithm_version,
        fingerprint,
        text_len,
    })
}

pub(crate) fn encode_candidate(
    text: &str,
    fingerprint: u64,
    out: &mut [u8],
) -> Result<usize, EncodeError> {
    let required = encoded_len(text.len());
    if out.len() < required {
        return Err(EncodeError::BufferTooSmall { required });
    }

    let header = PreparedCandidateHeader {
        format_version: PREPARED_CANDIDATE_FORMAT_VERSION,
        algorithm_version: PREPARED_CANDIDATE_ALGORITHM_VERSION,
        fingerprint,
        text_len: checked_archive_text_len(text.len())?,
    };
    out[0..2].copy_from_slice(&header.format_version.to_le_bytes());
    out[2..4].copy_from_slice(&header.algorithm_version.to_le_bytes());
    out[4..12].copy_from_slice(&header.fingerprint.to_le_bytes());
    out[12..16].copy_from_slice(&header.text_len.to_le_bytes());
    out[16..required].copy_from_slice(text.as_bytes());
    Ok(required)
}

pub(crate) fn encoded_len(text_len: usize) -> usize {
    PREPARED_CANDIDATE_HEADER_LEN + text_len
}

pub(crate) fn decode_candidate_text(
    bytes: &[u8],
) -> Result<(&str, PreparedCandidateHeader), DecodeError> {
    let header = decode_header(bytes)?;
    if header.format_version != PREPARED_CANDIDATE_FORMAT_VERSION {
        return Err(DecodeError::UnsupportedFormatVersion {
            expected: PREPARED_CANDIDATE_FORMAT_VERSION,
            actual: header.format_version,
        });
    }
    if header.algorithm_version != PREPARED_CANDIDATE_ALGORITHM_VERSION {
        return Err(DecodeError::UnsupportedAlgorithmVersion {
            expected: PREPARED_CANDIDATE_ALGORITHM_VERSION,
            actual: header.algorithm_version,
        });
    }

    let text_len =
        usize::try_from(header.text_len).map_err(|_| DecodeError::PlatformLengthOverflow {
            text_len: header.text_len,
        })?;
    let required = PREPARED_CANDIDATE_HEADER_LEN + text_len;
    if bytes.len() < required {
        return Err(DecodeError::Truncated {
            required,
            actual: bytes.len(),
        });
    }

    let text_bytes = &bytes[PREPARED_CANDIDATE_HEADER_LEN..required];
    let text = core::str::from_utf8(text_bytes).map_err(|_| DecodeError::InvalidUtf8)?;
    Ok((text, header))
}
