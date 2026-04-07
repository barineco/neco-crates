# neco-galois

[日本語](README-ja.md)

Necosystems series finite field arithmetic for secp256k1 and P-256.

This crate provides a 256-bit big integer (`U256`), a generic prime field (`Fp<P>`) backed by Montgomery multiplication, the secp256k1 and P-256 field and order constants, and an HMAC-DRBG deterministic nonce generator (RFC 6979).

## Modules

- `bigint`: `U256` 4-limb little-endian big integer
- `fp`: `Fp<P>`, `PrimeField`, and the `redc` Montgomery reduction
- `secp256k1`: `Secp256k1Field`, `Secp256k1Order`, and the `SQRT_EXP_*` constants
- `p256`: `P256Field`, `P256Order`, and the `SQRT_EXP_*` constants
- `rfc6979`: `generate_k` deterministic nonce generation

## Usage

```rust
use neco_galois::{Fp, PrimeField, Secp256k1Field, U256};

// big integer arithmetic (carry-aware)
let a = U256::from_u64(0xdead_beef);
let b = U256::from_u64(0xcafe_babe);
let (sum, _carry) = U256::add(a, b);

// prime field element via Montgomery form
let one = Fp::<Secp256k1Field>::one();
let two = Fp::<Secp256k1Field>::add(one, one);
let four = Fp::<Secp256k1Field>::mul(two, two);
# let _ = (sum, four);
```

## License

MIT
