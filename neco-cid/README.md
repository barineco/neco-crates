# neco-cid

[日本語](README-ja.md)

A minimal CIDv1 library with only a `sha2` dependency, providing CID computation and multibase encode/decode for IPLD and AT Protocol use cases.

## Features

- Single dependency: `sha2` (core functionality)
- CIDv1 compute, byte serialization, and deserialization
- Multibase encode/decode (base32lower, `b` prefix)
- Supported codecs: `dag-cbor` (0x71), `raw` (0x55)
- Hash function: SHA-256 only
- Optional `cbor` feature: CBOR tag 42 encode/decode via `neco-cbor`

## Usage

### Compute a CID

```rust
use neco_cid::{Cid, Codec, Base};

let data = b"hello world";
let cid = Cid::compute(Codec::DagCbor, data);

let s = cid.to_multibase(Base::Base32Lower);
// base32lower string like "bafyrei..."
```

### Decode from a CID string

```rust
use neco_cid::Cid;

let cid = Cid::from_multibase("bafyreib...").unwrap();
```

### Round-trip through bytes

```rust
use neco_cid::Cid;

let bytes = cid.to_bytes();
let (cid2, consumed) = Cid::from_bytes(&bytes).unwrap();
assert_eq!(cid, cid2);
assert_eq!(consumed, bytes.len());
```

## API

### `Cid`

Represents a CIDv1. Implements `Debug`, `Clone`, `PartialEq`, `Eq`, `Hash`.

| Item | Description |
|------|-------------|
| `Cid::compute(codec: Codec, data: &[u8]) -> Cid` | Compute a CID from the SHA-256 hash of data |
| `Cid::from_bytes(input: &[u8]) -> Result<(Cid, usize), CidError>` | Deserialize a CID from bytes, returning the number of bytes consumed |
| `Cid::to_bytes(&self) -> Vec<u8>` | Serialize the CID to bytes |
| `Cid::to_multibase(&self, base: Base) -> String` | Encode the CID as a multibase string in the given base |
| `Cid::from_multibase(input: &str) -> Result<Cid, CidError>` | Decode a CID from a multibase string |
| `Cid::codec(&self) -> Codec` | Return the codec |
| `Cid::digest(&self) -> &[u8; 32]` | Return the SHA-256 digest |

### `Codec`

| Variant | Value | Description |
|---------|-------|-------------|
| `Codec::DagCbor` | `0x71` | DAG-CBOR |
| `Codec::Raw` | `0x55` | Raw bytes |

### `Base`

| Variant | Prefix | Description |
|---------|--------|-------------|
| `Base::Base32Lower` | `b` | RFC 4648 base32 lowercase, no padding |

### CBOR tag 42 (requires `cbor` feature)

Enable the `cbor` feature to use CBOR tag 42 encode/decode for IPLD link representation.

```toml
[dependencies]
neco-cid = { version = "0.1", features = ["cbor"] }
```

```rust
use neco_cid::{Cid, Codec};

let cid = Cid::compute(Codec::DagCbor, b"hello");

let tag = cid.to_cbor_tag();
let decoded = Cid::from_cbor_tag(&tag).unwrap();
assert_eq!(cid, decoded);
```

| Item | Description |
|------|-------------|
| `Cid::to_cbor_tag(&self) -> CborValue` | Encode as CBOR tag 42 |
| `Cid::from_cbor_tag(value: &CborValue) -> Result<Cid, CborCidError>` | Decode from CBOR tag 42 |
| `Cid::from_cbor_tag_optional(value: &CborValue) -> Result<Option<Cid>, CborCidError>` | Decode optional CID (null to None) |

### `CidError`

| Variant | Description |
|---------|-------------|
| `InvalidVersion(u64)` | Version other than CIDv1 |
| `UnsupportedCodec(u64)` | Unsupported codec code |
| `UnsupportedHashCode(u64)` | Unsupported hash function code |
| `InvalidDigestLength` | Digest length is not 32 bytes |
| `InvalidMultibase` | Malformed multibase string |
| `UnexpectedEnd` | Input ended prematurely |

### `CborCidError` (requires `cbor` feature)

| Variant | Description |
|---------|-------------|
| `NotATag` | Value is not a CBOR tag |
| `WrongTag(u64)` | Tag number is not 42 |
| `NotBytes` | Tag payload is not a byte string |
| `MissingIdentityPrefix` | Missing 0x00 multibase identity prefix |
| `InvalidCid(CidError)` | CID binary parsing failed |
| `TrailingData` | Extra bytes after the CID |

## License

MIT
