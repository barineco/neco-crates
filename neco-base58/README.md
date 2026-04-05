# neco-base58

[日本語](README-ja.md)

A zero-dependency Base58BTC encoder and decoder.

## Usage

```rust
use neco_base58::{encode, decode};

let encoded = encode(b"abc");
assert_eq!(encoded, "ZiCa");

let decoded = decode("ZiCa").unwrap();
assert_eq!(decoded, b"abc");
```

## API

| Item | Description |
|------|-------------|
| `encode(input: &[u8]) -> String` | Encode a byte slice to a Base58BTC string |
| `decode(input: &str) -> Result<Vec<u8>, Base58Error>` | Decode a Base58BTC string to bytes |
| `Base58Error` | Error type: `InvalidCharacter(char)` |

## License

MIT
