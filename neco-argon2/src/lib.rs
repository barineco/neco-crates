//! Argon2id によるパスワードハッシュを提供します。
//!
//! パスワードのハッシュ化、PHC 形式の文字列生成、検証を提供します。

#![allow(clippy::too_many_arguments, clippy::wrong_self_convention)]

mod argon2id;
mod blake2b;

pub use argon2id::{
    argon2id_hash, argon2id_hash_encoded, argon2id_hash_with_secret, argon2id_verify, Argon2Params,
};
