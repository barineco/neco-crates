# neco-car

[日本語](README-ja.md)

A CAR v1 (Content Addressable aRchive) parser and writer for the IPFS/IPLD ecosystem. Depends on [neco-cbor](../neco-cbor) for DAG-CBOR header decoding and [neco-cid](../neco-cid) for CID parsing.

## Features

- Parse CAR v1 files: varint-prefixed DAG-CBOR header + varint-prefixed CID+data blocks
- Write CAR v1 files from a list of roots and raw blocks
- Comprehensive `CarError` enum covering all parse and write failure cases
- No unsafe code

## Usage

### Parse a CAR v1 file

```rust
use neco_car::{parse_v1, CarV1};

let bytes: &[u8] = /* raw CAR bytes */ &[];
let car = parse_v1(bytes).unwrap();

for root in car.roots() {
    println!("root: {root}");
}

for block in car.blocks() {
    println!("cid={}, len={}", block.cid(), block.data().len());
}
```

### Decompose into owned parts

```rust
let (roots, blocks) = car.into_parts();
for (cid, data) in blocks.into_iter().map(|e| e.into_parts()) {
    // cid: Cid, data: Vec<u8>
}
```

### Write a CAR v1 file

```rust
use neco_car::write_v1;
use neco_cid::Cid;

let root: Cid = /* your root CID */ todo!();
let block_data: &[u8] = b"...";

let car_bytes = write_v1(&[root.clone()], &[(root, block_data)]).unwrap();
```

## API

### Top-level functions

| Item | Description |
|------|-------------|
| `parse_v1(input: &[u8]) -> Result<CarV1, CarError>` | Parse a byte slice as a CAR v1 archive |
| `write_v1(roots: &[Cid], blocks: &[(Cid, &[u8])]) -> Result<Vec<u8>, CarError>` | Encode roots and blocks as a CAR v1 archive |

### `CarV1`

Represents a parsed CAR v1 archive.

| Method | Description |
|--------|-------------|
| `roots() -> &[Cid]` | Slice of root CIDs declared in the header |
| `blocks() -> &[CarEntry]` | Slice of all blocks in the archive |
| `into_parts() -> (Vec<Cid>, Vec<CarEntry>)` | Consume and decompose into owned roots and blocks |

### `CarEntry`

A single block within a CAR archive.

| Method | Description |
|--------|-------------|
| `cid() -> &Cid` | The CID identifying this block |
| `data() -> &[u8]` | The raw block data |
| `into_parts() -> (Cid, Vec<u8>)` | Consume and decompose into owned CID and data |

### `CarError`

| Variant | Description |
|---------|-------------|
| `UnexpectedEnd` | Input ended prematurely |
| `VarintOverflow` | Varint value exceeds 64-bit range |
| `InvalidHeader(DecodeErrorKind)` | DAG-CBOR header failed to decode |
| `HeaderNotMap` | Header is not a CBOR map |
| `MissingHeaderField(&'static str)` | Required header field (`version` or `roots`) is absent |
| `UnsupportedVersion(u64)` | CAR version is not 1 |
| `RootsNotArray` | `roots` field is not a CBOR array |
| `InvalidRootCid(CidError)` | A root CID could not be parsed |
| `InvalidBlockCid(CidError)` | A block CID could not be parsed |
| `BlockLengthMismatch` | Block section length is inconsistent with CID size |
| `EmptySection` | A block section has zero length |
| `InvalidCidLink` | Root CID link is not a valid tag-42 DAG-CBOR link |
| `HeaderEncode(EncodeError)` | DAG-CBOR header encoding failed during write |

## License

MIT
