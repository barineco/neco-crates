# neco-kdl

[日本語](README-ja.md)

Zero-dependency KDL v2 parser. Suitable for configuration files and DSL parsing.

## Features

- Full KDL v2 specification parsing
  - Multiline strings, raw strings, escline
  - Type annotations: `(type)node`
  - Slashdash comments (`/-`), block comments (`/* ... */`), nested comments
  - `#true` / `#false` / `#null` / `#inf` / `#-inf` / `#nan` keywords
  - Hex, octal, binary literals with underscore separators
  - Version marker (`/- kdl-version 2`)
- Normalized output (matches official test suite `expected_kdl`)
- Zero external dependencies
- Passes the full official test suite

## Usage

```toml
[dependencies]
neco-kdl = "0.1"
```

```rust
use neco_kdl::{parse, normalize};

fn main() {
    let src = r#"
        node "hello" key=#true {
            child 42
        }
    "#;

    let doc = parse(src).unwrap();

    // Iterate over nodes
    for node in doc.nodes() {
        println!("{}: {} entries", node.name(), node.entries().len());
    }

    // Convert to normalized form
    let normalized = normalize(&doc);
    print!("{}", normalized);
}
```

## API

### `parse`

```rust
pub fn parse(input: &str) -> Result<KdlDocument, KdlError>
```

Parses a KDL v2 document and returns a `KdlDocument`.

### `normalize`

```rust
pub fn normalize(doc: &KdlDocument) -> String
```

Converts a `KdlDocument` to its normalized string form. Normalization rules:

- Strips comments
- Sorts properties by key in alphabetical order
- Deduplicates properties (last occurrence wins)
- Converts all strings to quoted strings
- Unquotes strings that are valid identifiers
- Indents with 4 spaces
- Converts numbers to decimal, strips underscores
- Adds trailing newline

### Types

| Item | Description |
|------|-------------|
| `KdlDocument` | Parse result root. Access nodes via `nodes()` |
| `KdlNode` | Node with `ty()`, `name()`, `entries()`, `children()` accessors |
| `KdlEntry` | `Argument` (positional) or `Property` (named) |
| `KdlValue` | `String(String)`, `Number(KdlNumber)`, `Bool(bool)`, `Null` |
| `KdlNumber` | Provides `raw()`, `as_i64()`, `as_f64()`. No upper bound on numeric size |
| `KdlError` | Error with `line()`, `col()` (1-based), `kind()` |
| `KdlErrorKind` | Error variant: `UnexpectedChar`, `InvalidEscape`, `UnclosedString`, etc. |

## License

MIT
