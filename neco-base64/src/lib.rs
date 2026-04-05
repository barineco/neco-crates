//! Zero-dependency Base64 encoder and decoder.
//!
//! Supports standard (RFC 4648 section 4) and URL-safe (RFC 4648 section 5)
//! alphabets with configurable padding.

mod decode;
mod encode;
mod error;

pub use decode::{decode, decode_url, decode_url_strict};
pub use encode::{encode, encode_url, encode_url_padded};
pub use error::Base64Error;
