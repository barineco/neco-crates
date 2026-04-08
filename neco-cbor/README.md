# neco-cbor

[日本語](README-ja.md)

A necosystems series CBOR / DAG-CBOR codec that runs in `no_std` + `alloc` environments, suitable for embedded and WASM targets dealing with AT Protocol or IPLD.

## Features

- No external crates. Runtime dependencies are limited to neco-* crates (`no_std` + `alloc`)
- CBOR decode and encode (RFC 7049)
- DAG-CBOR decode and encode (IPLD deterministic encoding)
- Tag 42 (CID link) support
- DAG-CBOR mode validates map key sort order, duplicates, and non-text keys

## Usage

### Decode CBOR

```rust
use neco_cbor::{decode, CborValue};

let bytes = &[0xa1, 0x61, 0x78, 0x01]; // {"x": 1}
let value = decode(bytes).unwrap();

let x = value.required_unsigned("x").unwrap(); // 1u64
```

### Decode DAG-CBOR

```rust
use neco_cbor::{decode_dag, CborValue};

let value = decode_dag(dag_cbor_bytes).unwrap();
```

### Encode CBOR

```rust
use neco_cbor::{encode, CborValue};
use alloc::vec;

let value = CborValue::Map(vec![
    (CborValue::Text("n".into()), CborValue::Unsigned(42)),
]);
let bytes = encode(&value).unwrap();
```

### Encode DAG-CBOR

```rust
use neco_cbor::{encode_dag, CborValue};

let bytes = encode_dag(&value).unwrap();
```

### Read Tag 42 (CID link)

```rust
use neco_cbor::CborValue;

if let Some((42, inner)) = value.as_tag() {
    let cid_bytes = inner.as_bytes().unwrap(); // byte string prefixed with 0x00
}
```

## API

### Top-level functions

| Item | Description |
|------|-------------|
| `decode(input: &[u8]) -> Result<CborValue, DecodeError>` | Decode a byte slice as CBOR |
| `decode_dag(input: &[u8]) -> Result<CborValue, DecodeError>` | Decode a byte slice as DAG-CBOR (validates constraints) |
| `encode(value: &CborValue) -> Result<Vec<u8>, EncodeError>` | Encode a `CborValue` to CBOR bytes |
| `encode_dag(value: &CborValue) -> Result<Vec<u8>, EncodeError>` | Encode a `CborValue` to DAG-CBOR bytes (sorts map keys) |

### `CborValue`

Represents any CBOR value.

```
Unsigned(u64) | Negative(i64) | Bytes(Vec<u8>) | Text(String)
| Array(Vec<CborValue>) | Map(Vec<(CborValue, CborValue)>)
| Tag(u64, Box<CborValue>) | Bool(bool) | Null
```

#### Value extraction (`Option`)

| Item | Description |
|------|-------------|
| `as_unsigned() -> Option<u64>` | Extract unsigned integer |
| `as_negative() -> Option<i64>` | Extract negative integer |
| `as_bytes() -> Option<&[u8]>` | Extract byte slice |
| `as_text() -> Option<&str>` | Extract text slice |
| `as_array() -> Option<&[CborValue]>` | Extract array slice |
| `as_map() -> Option<&[(CborValue, CborValue)]>` | Extract map entry slice |
| `as_tag() -> Option<(u64, &CborValue)>` | Extract tag number and inner value |
| `as_bool() -> Option<bool>` | Extract bool |
| `is_null() -> bool` | Returns `true` if `Null` |
| `get(key: &str) -> Option<&CborValue>` | Look up a map field by text key |

#### Required field accessors

Return `Err(AccessError)` when the value is not a map, the field is missing, or the type does not match.

| Item | Description |
|------|-------------|
| `required_text(key) -> Result<&str, AccessError>` | Required text field |
| `required_bytes(key) -> Result<&[u8], AccessError>` | Required bytes field |
| `required_unsigned(key) -> Result<u64, AccessError>` | Required unsigned integer field |
| `required_negative(key) -> Result<i64, AccessError>` | Required negative integer field |
| `required_bool(key) -> Result<bool, AccessError>` | Required bool field |
| `required_array(key) -> Result<&[CborValue], AccessError>` | Required array field |
| `required_map(key) -> Result<&[(CborValue, CborValue)], AccessError>` | Required map field |
| `required_tag(key) -> Result<(u64, &CborValue), AccessError>` | Required tag field |

### Error types

| Item | Description |
|------|-------------|
| `DecodeError` | Decode failure with byte `position` and `kind` |
| `DecodeErrorKind` | Specific decode failure reason (see below) |
| `EncodeError` | Encode failure (see below) |
| `AccessError` | Field access failure: `NotAMap`, `MissingField`, `TypeMismatch` |

#### `DecodeErrorKind` variants

| Variant | Description |
|---------|-------------|
| `UnexpectedEnd` | Input ended prematurely |
| `InvalidMajorType(u8)` | Unknown major type |
| `NestingTooDeep` | Nesting depth exceeded |
| `IndefiniteLength` | Indefinite-length items are not supported |
| `FloatNotAllowed` | Floating-point values are not supported |
| `UnsortedMapKeys` | Map keys are not sorted in DAG-CBOR order |
| `NonCanonicalInteger` | Integer is not minimally encoded |
| `DuplicateMapKey` | Duplicate map key |
| `TrailingContent` | Trailing bytes after the CBOR value |
| `InvalidUtf8` | Text string is not valid UTF-8 |
| `NonTextMapKey` | Map key is not text in DAG-CBOR mode |
| `UnsupportedTag(u64)` | Unsupported tag number |
| `IntegerOverflow` | Integer does not fit in the target representation |

#### `EncodeError` variants

| Variant | Description |
|---------|-------------|
| `NonTextKeyInDagMode` | Map key is not text in DAG-CBOR mode |
| `DuplicateKeyInDagMode` | Duplicate map key in DAG-CBOR mode |
| `UnsupportedTag(u64)` | Unsupported tag number |
| `InvalidTag42Payload` | Tag 42 payload must be a byte string prefixed with `0x00` |
| `InvalidNegativeValue(i64)` | `Negative` value is not less than zero |

## License

MIT
