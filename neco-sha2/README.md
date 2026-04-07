# neco-sha2

[日本語](README-ja.md)

Zero-dependency SHA-256, HMAC-SHA256, and HKDF-SHA256 implementation.

- SHA-256: RFC 6234 compliant
- HMAC-SHA256: RFC 2104 compliant
- HKDF-SHA256: RFC 5869 compliant

## Usage

```rust
use neco_sha2::{Sha256, Hmac, Hkdf};

// SHA-256 one-shot
let hash = Sha256::digest(b"hello");

// SHA-256 streaming
let hash = Sha256::new().update(b"hel").update(b"lo").finalize();

// HMAC-SHA256
let mac = Hmac::new(b"key").update(b"message").finalize();

// HKDF-SHA256
let prk = Hkdf::extract(b"salt", b"input keying material");
let okm = prk.expand(b"info", 32).unwrap();
```

## License

MIT
