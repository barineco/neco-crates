use core::fmt;

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
