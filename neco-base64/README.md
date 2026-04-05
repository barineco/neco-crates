# neco-base64

[Japanese](README-ja.md)

A zero-dependency Base64 encoder and decoder supporting standard and URL-safe alphabets.

## Usage

```rust
use neco_base64::{encode, decode, encode_url, decode_url};

let encoded = encode(b"hello");       // "aGVsbG8="
let decoded = decode("aGVsbG8=").unwrap(); // b"hello"

let url_encoded = encode_url(b"hello");         // "aGVsbG8"
let url_decoded = decode_url("aGVsbG8").unwrap(); // b"hello"
```

## API

### Encode

| Function | Description |
|----------|-------------|
| `encode(input: &[u8]) -> String` | Standard Base64 with padding |
| `encode_url(input: &[u8]) -> String` | URL-safe Base64, no padding |
| `encode_url_padded(input: &[u8]) -> String` | URL-safe Base64 with padding |

### Decode

| Function | Description |
|----------|-------------|
| `decode(input: &str) -> Result<Vec<u8>, Base64Error>` | Standard Base64 (padding optional) |
| `decode_url(input: &str) -> Result<Vec<u8>, Base64Error>` | URL-safe Base64 (padding optional) |
| `decode_url_strict(input: &str) -> Result<Vec<u8>, Base64Error>` | URL-safe, rejects padding chars and non-zero trailing bits |

### Error

```rust
pub enum Base64Error {
    InvalidCharacter,
    InvalidLength,
    NonZeroPaddingBits,
}
```

## License

MIT
