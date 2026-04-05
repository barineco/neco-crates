//! Zero-dependency Base58BTC encoder and decoder.

mod decode;
mod encode;
mod error;

pub(crate) const ALPHABET: &[u8; 58] =
    b"123456789ABCDEFGHJKLMNPQRSTUVWXYZabcdefghijkmnopqrstuvwxyz";

pub use decode::decode;
pub use encode::encode;
pub use error::Base58Error;
