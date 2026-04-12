# neco-sha1

[日本語](README-ja.md)

Zero-dependency SHA-1 hash function implementation.

- SHA-1: RFC 3174 compliant

## Usage

```rust
use neco_sha1::Sha1;

// SHA-1 one-shot
let hash = Sha1::digest(b"hello");

// SHA-1 streaming
let mut h = Sha1::new();
h.update(b"hel");
h.update(b"lo");
let hash = h.finalize();
```

## License

MIT
