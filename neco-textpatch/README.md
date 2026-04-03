# neco-textpatch

[日本語](README-ja.md)

Deterministic text patch helpers for replacing byte ranges and rewriting small named blocks without a full parser.

## Patch behavior

`neco-textpatch` works on UTF-8 strings and validates all byte ranges against the original source before rewriting. It rejects overlapping replacements, invalid UTF-8 boundaries, and ambiguous keyed entries instead of guessing how to recover.

For simple structured text, the crate also finds named brace blocks and rewrites their contents. This is useful when a caller wants to update one config-like section while keeping the rest of the source untouched.

## Usage

### Replace one range

```rust
use neco_textpatch::{apply_patch, TextPatch};

let patch = TextPatch::replace(6, 11, "there").expect("valid patch");
let updated = apply_patch("hello world", &patch).expect("patch should apply");

assert_eq!(updated, "hello there");
```

### Replace one named block

```rust
use neco_textpatch::{apply_patch, replace_block};

let source = "settings {\n  mode = \"fast\"\n}\n";
let patch = replace_block(source, "settings", "\n  mode = \"safe\"\n")
    .expect("settings block should exist");
let updated = apply_patch(source, &patch).expect("patch should apply");

assert_eq!(updated, "settings {\n  mode = \"safe\"\n}\n");
```

## API

| Item | Description |
|------|-------------|
| `TextPatch` | One validated byte-range replacement |
| `TextPatch::new(start, end, replacement)` | Build a checked patch and return `Err` on invalid ranges |
| `TextPatch::insert(offset, replacement)` | Insert text at one byte offset |
| `TextPatch::delete(start, end)` / `replace(start, end, replacement)` | Convenience constructors for deletion and replacement |
| `TextPatchError` | Reports invalid ranges, bounds, UTF-8 boundaries, conflicts, and block lookup failures |
| `PatchConflict` | Describes which two patches overlapped during validation |
| `validate_patches(source, patches)` | Check bounds and overlap rules against the original source |
| `apply_patch(source, patch)` / `apply_patches(source, patches)` | Apply one or more validated patches deterministically |
| `find_block_range(source, block_name)` | Locate a named brace block and return its byte range |
| `replace_block(source, block_name, replacement)` | Build a patch that replaces one block body |
| `KnownEntry` | One keyed replacement entry for block merging |
| `merge_known_entries(source, block_name, entries)` | Replace known keyed entries and append missing ones |

## License

MIT
