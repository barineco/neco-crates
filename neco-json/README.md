# neco-json

[日本語](README-ja.md)

A zero-dependency JSON codec for `no_std` + `alloc` environments, providing parse, encode, typed field access, and lightweight `ToJson` / `FromJson` traits over `JsonValue`.

## Usage

### Parse

```rust
use neco_json::{parse, JsonValue};

let json = br#"{"name":"neco","score":42.5,"active":true}"#;
let value = parse(json).unwrap();
```

### Access fields

```rust
use neco_json::{parse, JsonValue};

let json = br#"{"name":"neco","score":42.5,"active":true,"tag":null}"#;
let value = parse(json).unwrap();

// required fields — error if missing or wrong type
let name   = value.required_str("name").unwrap();   // "neco"
let score  = value.required_f64("score").unwrap();  // 42.5
let active = value.required_bool("active").unwrap(); // true

// optional fields — Ok(None) when absent or null
let tag = value.optional_str("tag").unwrap(); // None
```

### Encode

```rust
use neco_json::{encode, JsonValue};
use alloc::vec;

let value = JsonValue::Object(vec![
    ("x".into(), JsonValue::Number(1.0)),
    ("ok".into(), JsonValue::Bool(true)),
]);
let bytes = encode(&value).unwrap(); // b"{\"x\":1.0,\"ok\":true}"
```

### `ToJson` / `FromJson`

```rust
use neco_json::{FromJson, ToJson};

let json = vec![1_u64, 2, 3].to_json();
let restored = Vec::<u64>::from_json(&json).unwrap();
assert_eq!(restored, vec![1, 2, 3]);
```

## API

### Top-level functions

| Item | Description |
|------|-------------|
| `parse(input: &[u8]) -> Result<JsonValue, ParseError>` | Parse a byte slice as JSON |
| `encode(value: &JsonValue) -> Result<Vec<u8>, EncodeError>` | Encode a `JsonValue` to compact JSON bytes |
| `ToJson` / `FromJson` | Lightweight traits for converting Rust values to and from `JsonValue` |

### `JsonValue`

Represents any JSON value.

```
Null | Bool(bool) | Number(f64) | String(String) | Array(Vec<JsonValue>) | Object(Vec<(String, JsonValue)>)
```

#### Type checks

| Item | Description |
|------|-------------|
| `is_null()` | Returns `true` if `Null` |
| `is_bool()` | Returns `true` if `Bool` |
| `is_number()` | Returns `true` if `Number` |
| `is_string()` | Returns `true` if `String` |
| `is_array()` | Returns `true` if `Array` |
| `is_object()` | Returns `true` if `Object` |

#### Value extraction (`Option`)

| Item | Description |
|------|-------------|
| `as_bool() -> Option<bool>` | Extract bool |
| `as_f64() -> Option<f64>` | Extract number |
| `as_str() -> Option<&str>` | Extract string slice |
| `as_array() -> Option<&[JsonValue]>` | Extract array slice |
| `as_object() -> Option<&[(String, JsonValue)]>` | Extract object field slice |
| `get(key) -> Option<&JsonValue>` | Look up an object field by key |

#### Required field accessors

Return `Err(AccessError)` when the value is not an object, the field is missing, or the type does not match.

| Item | Description |
|------|-------------|
| `required_str(key) -> Result<&str, AccessError>` | Required string field |
| `required_bool(key) -> Result<bool, AccessError>` | Required bool field |
| `required_f64(key) -> Result<f64, AccessError>` | Required number field |
| `required_array(key) -> Result<&[JsonValue], AccessError>` | Required array field |
| `required_object(key) -> Result<&[(String, JsonValue)], AccessError>` | Required object field |

#### Optional field accessors

Return `Ok(None)` when the field is absent or `null`; `Err(AccessError)` on type mismatch.

| Item | Description |
|------|-------------|
| `optional_str(key) -> Result<Option<&str>, AccessError>` | Optional string field |
| `optional_bool(key) -> Result<Option<bool>, AccessError>` | Optional bool field |
| `optional_f64(key) -> Result<Option<f64>, AccessError>` | Optional number field |
| `optional_array(key) -> Result<Option<&[JsonValue]>, AccessError>` | Optional array field |

### Error types

| Item | Description |
|------|-------------|
| `ParseError` | Parse failure with byte `position` and `kind` |
| `ParseErrorKind` | Specific parse failure reason |
| `EncodeError` | Encode failure — currently only `NonFiniteNumber` |
| `AccessError` | Field access failure: `NotAnObject`, `MissingField`, `TypeMismatch` |

## License

MIT
