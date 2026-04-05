#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Base64Error {
    InvalidCharacter,
    InvalidLength,
    NonZeroPaddingBits,
}

impl core::fmt::Display for Base64Error {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::InvalidCharacter => f.write_str("invalid base64 character"),
            Self::InvalidLength => f.write_str("invalid base64 input length"),
            Self::NonZeroPaddingBits => f.write_str("non-zero padding bits"),
        }
    }
}

impl std::error::Error for Base64Error {}
