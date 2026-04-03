# neco-watchnorm

[日本語](README-ja.md)

`neco-watchnorm` normalizes host-specific file watcher events into a deterministic batch API for downstream runtime logic.

## Event normalization

The crate accepts host-side watcher events as `RawWatchEvent`, then converts them into a smaller normalized event stream during `drain()`. Callers provide a generation number, and the normalizer drops stale events, joins rename halves when possible, and keeps incomplete rename information as `PartialRename` instead of guessing.

`Modify` coalescing is conservative. Repeated modifies for the same path collapse, an immediate modify after create is absorbed, and removes discard earlier modifies for the same path in the current batch.

## Usage

```rust
use neco_watchnorm::{
    NormalizedWatchKind, RawWatchEvent, RawWatchKind, RenameHint, WatchBatchNormalizer,
};

let mut normalizer = WatchBatchNormalizer::new();
normalizer.push(RawWatchEvent {
    kind: RawWatchKind::Rename,
    paths: vec!["/old.txt".into()],
    rename_from: Some("/old.txt".into()),
    rename_to: None,
    rename_hint: Some(RenameHint::From),
    generation: 2,
});
normalizer.push(RawWatchEvent {
    kind: RawWatchKind::Rename,
    paths: vec!["/new.txt".into()],
    rename_from: None,
    rename_to: Some("/new.txt".into()),
    rename_hint: Some(RenameHint::To),
    generation: 2,
});

let result = normalizer.drain(2);
assert_eq!(result.events.len(), 1);
assert!(matches!(
    result.events[0].kind,
    NormalizedWatchKind::Rename { .. }
));
```

## API

| Item | Description |
|------|-------------|
| `RawWatchEvent` | Host-agnostic batch input event with generation metadata |
| `RawWatchKind` | Raw create / remove / modify / rename discriminator |
| `RenameHint` | Optional rename-side hint from the host bridge |
| `NormalizedWatchEvent` | Consumer-facing normalized event with generation |
| `NormalizedWatchKind` | `Create`, `Remove`, `Modify`, `Rename`, and `PartialRename` |
| `WatchBatchNormalizer` | Stateful batch normalizer with `push` and `drain` |
| `FlushResult` | Normalized output events and stale discard count |

## License

MIT
