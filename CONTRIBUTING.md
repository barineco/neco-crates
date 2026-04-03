# Contributing

## Scope

`neco-crates` is maintained as a crates.io-oriented monorepo for reusable mathematical and geometry libraries. Small, focused changes are preferred over broad speculative rewrites.

## Development Checks

Before opening a pull request, run the repository-level checks from the workspace root:

```sh
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```

## Pull Requests

- Keep changes narrowly scoped and technically justified.
- Update crate-level README files when public behavior changes.
- Avoid introducing silent fallbacks at public API boundaries.
- Prefer adding tests for bug fixes and new public behavior.

## Workspace Notes

- Crates in this repository are intended to remain publishable independently.
- Path dependencies should keep a version fallback when they point to another workspace crate.
- Public-facing metadata in each `Cargo.toml` should remain suitable for crates.io.
