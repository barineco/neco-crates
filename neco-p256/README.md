# neco-p256

[日本語](README-ja.md)

A minimal P-256 (NIST) ECDSA sign/verify crate. The only dependency is the [RustCrypto p256](https://crates.io/crates/p256) crate.

## Features

- P-256 ECDSA prehash signing and verification
- Low-S normalization on the signing side; high-S rejection on the verification side
- Public keys in SEC1 compressed form (33 bytes)
- No unsafe code

## Usage

```rust
use neco_p256::{SecretKey, EcdsaSignature};

let secret = SecretKey::generate().unwrap();
let public = secret.public_key().unwrap();
let digest: [u8; 32] = [0x42; 32]; // SHA-256 digest

let sig: EcdsaSignature = secret.sign_ecdsa_prehash(digest).unwrap();
public.verify_ecdsa_prehash(digest, &sig).unwrap();
```

Keys can also be handled as hex strings.

```rust
use neco_p256::SecretKey;

let secret = SecretKey::generate().unwrap();
let hex = secret.to_hex();
let restored = SecretKey::from_hex(&hex).unwrap();
assert_eq!(secret, restored);
```

Public keys round-trip through SEC1 compressed bytes.

```rust
use neco_p256::PublicKey;

let secret = neco_p256::SecretKey::generate().unwrap();
let public = secret.public_key().unwrap();
let bytes: [u8; 33] = public.to_sec1_bytes();
let restored = PublicKey::from_sec1_bytes(&bytes).unwrap();
assert_eq!(public, restored);
```

## API

### `SecretKey`

| Method | Description |
|--------|-------------|
| `generate() -> Result<Self, P256Error>` | Generate a random secret key |
| `from_bytes(bytes: [u8; 32]) -> Result<Self, P256Error>` | Construct from a 32-byte array |
| `from_hex(hex: &str) -> Result<Self, P256Error>` | Construct from a 64-character hex string |
| `to_bytes() -> [u8; 32]` | Return as a 32-byte array |
| `to_hex() -> String` | Return as a hex string |
| `public_key() -> Result<PublicKey, P256Error>` | Derive the corresponding public key |
| `sign_ecdsa_prehash(digest32: [u8; 32]) -> Result<EcdsaSignature, P256Error>` | P-256 ECDSA prehash signing with low-S normalization |

### `PublicKey`

| Method | Description |
|--------|-------------|
| `from_sec1_bytes(bytes: &[u8]) -> Result<Self, P256Error>` | Construct from SEC1 compressed bytes |
| `from_hex(hex: &str) -> Result<Self, P256Error>` | Construct from a 66-character hex string |
| `to_sec1_bytes() -> [u8; 33]` | Return as SEC1 compressed bytes |
| `to_hex() -> String` | Return as a hex string |
| `verify_ecdsa_prehash(digest32: [u8; 32], sig: &EcdsaSignature) -> Result<(), P256Error>` | P-256 ECDSA prehash verification with high-S rejection |

### `EcdsaSignature`

64-byte ECDSA signature in raw r\|\|s compact format.

| Method | Description |
|--------|-------------|
| `from_bytes(bytes: [u8; 64]) -> Self` | Construct from a 64-byte array |
| `from_hex(hex: &str) -> Result<Self, P256Error>` | Construct from a 128-character hex string |
| `to_bytes() -> [u8; 64]` | Return as a 64-byte array |
| `to_hex() -> String` | Return as a hex string |

### `P256Error`

| Variant | Description |
|---------|-------------|
| `InvalidSecretKey` | Secret key bytes are invalid |
| `InvalidPublicKey` | Public key bytes are invalid |
| `InvalidSignature` | Signature is invalid (verification failure or high-S) |
| `InvalidHex(&'static str)` | Hex decoding failed |

## License

MIT
