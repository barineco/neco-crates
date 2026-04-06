# neco-argparse

CLI argument parser and validator backed by neco-json.

## Overview

- Define command schemas with `ArgDef` / `CommandMeta`
- Validate `JsonValue` parameters with `parse_and_validate`
- Convert raw `std::env::args()` into structured `JsonValue` with `parse_cli_args`

## Usage

```rust
use neco_argparse::{ArgDef, ArgType, CommandMeta, parse_cli_args, parse_and_validate};
```

## License

MIT
