# neco-pathrel

[日本語](README-ja.md)

String-based path relation helpers for checking subtree membership and rewriting paths through file or directory renames.

## Path handling

`neco-pathrel` compares path strings under an explicit `PathPolicy` instead of consulting the filesystem. Callers choose the separator and case-sensitivity once, then reuse the same policy for subtree checks, parent lookup, path joining, and rename remapping.

The crate does not canonicalize `.`, `..`, symlinks, drive letters, or UNC paths. It stays at the level of runtime path strings so behavior stays deterministic across hosts.

## Usage

```rust
use neco_pathrel::{path_matches_or_contains, remap_path_for_rename, PathPolicy};

let policy = PathPolicy::posix();

assert!(path_matches_or_contains(
    "/workspace/src/lib.rs",
    "/workspace/src",
    &policy,
));

let renamed = remap_path_for_rename(
    "/workspace/src/lib.rs",
    "/workspace/src",
    "/workspace/core",
    &policy,
);
assert_eq!(renamed.as_deref(), Some("/workspace/core/lib.rs"));
```

## API

| Item | Description |
|------|-------------|
| `PathPolicy::new(separator, case_sensitivity)` | Construct an explicit path comparison policy |
| `PathPolicy::posix()` | Construct the default POSIX-like policy |
| `path_matches_or_contains(path, target, policy)` | Return whether `path` equals `target` or lies under `target` |
| `parent_path(path, policy)` | Return the direct parent slice when one exists |
| `join_path(base, name, policy)` | Join two path fragments with one separator |
| `remap_path_for_rename(path, source, target, policy)` | Rewrite one path through a file or subtree rename |

## License

MIT
