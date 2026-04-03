//! Zero-dependency minimal JSON codec for no_std environments.
//!
//! Provides [JsonValue] for representing JSON data, [parse] for decoding,
//! and [encode] for encoding.

#![no_std]
extern crate alloc;

mod encode;
mod error;
mod parse;
mod value;

pub use encode::encode;
pub use error::{AccessError, EncodeError, ParseError, ParseErrorKind};
pub use parse::parse;
pub use value::JsonValue;
