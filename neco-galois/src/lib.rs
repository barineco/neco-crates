//! Necosystems series finite field arithmetic for secp256k1 and P-256.
//!
//! Provides `U256`, `Fp<P>` Montgomery-form prime field, secp256k1 / P-256
//! field and order constants, and RFC 6979 deterministic nonce generation.

// `U256::add(a, b)` style associated functions are intentional, not trait impls.
#![allow(clippy::should_implement_trait)]

pub mod bigint;
pub mod fp;
pub mod p256;
pub mod rfc6979;
pub mod secp256k1;

pub use bigint::U256;
pub use fp::{redc, Fp, PrimeField};
pub use p256::{P256Field, P256Order, SQRT_EXP_P256, SQRT_EXP_P256_ORDER};
pub use rfc6979::generate_k;
pub use secp256k1::{Secp256k1Field, Secp256k1Order, SQRT_EXP_SECP256K1, SQRT_EXP_SECP256K1_ORDER};
