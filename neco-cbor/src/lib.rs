//! necosystems series CBOR and DAG-CBOR codec.

#![no_std]
extern crate alloc;

pub mod cid_tag;
mod decode;
mod encode;
mod error;
pub mod json_bridge;
mod value;

pub use decode::{decode, decode_dag, decode_one};
pub use encode::{encode, encode_dag};
pub use error::{AccessError, DecodeError, DecodeErrorKind, EncodeError};
pub use value::CborValue;
