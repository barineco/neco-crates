# neco-gridfield

[日本語](README-ja.md)

Uniform 2D grids and triple-buffered field state for time-domain solvers.

This crate extracts the grid and field-state layer from larger wave-simulation code so it can be tested independently and reused without pulling in solver-specific equations. Its 2D buffer storage is shared through `neco-array2` rather than reimplemented locally.

## API

| Item | Description |
|------|-------------|
| `Grid2D::new(lx, ly, dx)` | Construct a uniform square-spaced 2D grid, or return `GridError` for invalid geometry |
| `Grid2D::coords()` | Return centered `x` / `y` coordinate arrays |
| `Grid2D::radius_map()` | Return radial distance from the grid center |
| `Grid2D::interior_mask(geom)` | Build an active-cell mask for a boundary geometry, or return `GridError` if an explicit mask shape is wrong |
| `BoundaryGeometry` | Circular, rectangular, or explicit mask boundary |
| `FieldSet::new(nx, ny)` | Allocate triple buffers for `w`, `u`, and `v` fields |
| `FieldSet::split_bufs()` | Borrow current / previous / next buffers for one update step |
| `FieldSet::advance()` | Rotate buffers in O(1) by advancing the generation counter |
| `FieldSet::to_checkpoint()` | Snapshot all triple buffers into a checkpoint (`serde` makes the checkpoint serializable) |
| `FieldSet::restore_checkpoint(cp)` | Restore all buffers from a checkpoint, or return `CheckpointError` if the shape is invalid |

## Preconditions

- Grid coordinates are centered around the midpoint cell.
- `Grid2D::new` requires finite `dx > 0` and finite non-negative `lx` / `ly`.
- `BoundaryGeometry::Rectangular` disables a two-cell border.
- `FieldSet` stores three buffers for each field so Störmer-Verlet style stepping can update `next` while reading `current` and `previous`.
- `FieldSetCheckpoint` uses row-major flattened data from `neco-array2::Array2`.

## License

MIT
