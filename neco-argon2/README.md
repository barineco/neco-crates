# neco-argon2

[日本語](README-ja.md)

Zero-dependency Argon2id password hashing using Blake2b as the internal hash function.

- Argon2id: RFC 9106 compliant
- Blake2b: RFC 7693 compliant
- PHC string format: `$argon2id$v=19$m=...,t=...,p=...$...`

## Usage

```rust
use neco_argon2::{Argon2Params, argon2id_hash_encoded, argon2id_verify};

let password = b"hunter2";
let params = Argon2Params::default(); // m=19456, t=2, p=1

// Hash (random salt is generated automatically via getrandom)
let encoded = argon2id_hash_encoded(password, params);
// => "$argon2id$v=19$m=19456,t=2,p=1$<salt_b64>$<hash_b64>"

// Verify
assert!(argon2id_verify(&encoded, password));
assert!(!argon2id_verify(&encoded, b"wrong"));
```

### Low-level API

```rust
use neco_argon2::argon2id_hash;

let hash = argon2id_hash(
    b"password",
    b"randomsalt12345!",  // minimum 8 bytes
    19456,               // m_cost (KiB)
    2,                   // t_cost (iterations)
    1,                   // p_cost (parallelism)
    32,                  // output length (bytes)
);
```

## Parameter Guidelines

| Parameter | Description | Recommended |
|-----------|-------------|-------------|
| `m_cost` | Memory cost (KiB) | 19456 (19 MiB) |
| `t_cost` | Iterations | 2 |
| `p_cost` | Parallelism | 1 |
| `output_len` | Output length (bytes) | 32 |

See [MATH.md](MATH.md) for the mathematical background and detailed parameter guidance.

## Dependencies

- [`getrandom`](https://docs.rs/getrandom): salt generation (OS entropy source)
- [`neco-base64`](https://docs.rs/neco-base64): base64 encoding for PHC strings

## License

MIT
