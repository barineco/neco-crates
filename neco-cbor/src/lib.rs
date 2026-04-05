//! Zero-dependency minimal CBOR and DAG-CBOR codec.

#![no_std]
extern crate alloc;

mod decode;
mod encode;
mod error;
mod value;

pub use decode::{decode, decode_dag, decode_one};
pub use encode::{encode, encode_dag};
pub use error::{AccessError, DecodeError, DecodeErrorKind, EncodeError};
pub use value::CborValue;
