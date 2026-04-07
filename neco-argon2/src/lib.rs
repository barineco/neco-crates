//! Zero-dependency Argon2id password hashing with Blake2b.
//!
//! Provides Argon2id (RFC 9106) password hashing with Blake2b (RFC 7693) as
//! the internal hash function. Supports PHC string format for encoded
//! password storage and verification.
//!
//! # Example
//!
//! ```rust
//! use neco_argon2::{Argon2Params, argon2id_hash_encoded, argon2id_verify};
//!
//! let password = b"my secret password";
//! let params = Argon2Params::default();
//! let encoded = argon2id_hash_encoded(password, params);
//! assert!(argon2id_verify(&encoded, password));
//! ```

// Argon2id は仕様上多数の独立パラメータ (password, salt, m_cost, t_cost, p_cost,
// secret, associated_data, output_len, ...) を取るため、内部関数の引数数は RFC 9106
// に対応する形で設計されている。Block (1024 bytes Copy) の to_bytes は大きな値コピーを
// 避けるため `&self` を維持する。
#![allow(clippy::too_many_arguments, clippy::wrong_self_convention)]

mod argon2id;
mod blake2b;

pub use argon2id::{
    argon2id_hash, argon2id_hash_encoded, argon2id_hash_with_secret, argon2id_verify, Argon2Params,
};
