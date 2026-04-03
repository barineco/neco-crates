# neco-array2

[日本語](README-ja.md)

Lightweight row-major 2D array foundation for grid-oriented crates.

This crate provides a narrow `Array2<T>` for masks, field buffers, and checkpoint-friendly flattened storage. It is shared by grid-oriented crates such as `neco-gridfield` and `neco-contact`, and it serves as a focused array foundation for those crates.

## API

| Item | Description |
|------|-------------|
| `Array2::from_shape_vec((nrows, ncols), data)` | Construct an array from row-major owned storage with shape validation |
| `Array2::from_elem((nrows, ncols), value)` | Fill a new array with one repeated value |
| `Array2::zeros((nrows, ncols))` | Construct a zero-initialized array when `T: Default` |
| `Array2::dim()` | Return `(nrows, ncols)` |
| `Array2::shape()` | Return `[nrows, ncols]` |
| `Array2::as_slice()` | Expose the internal row-major storage |
| `Array2::iter()` / `iter_mut()` | Iterate over the row-major storage |
| `array[[row, col]]` | Read or write one cell |

## Preconditions

- Storage order is row-major.
- The API is intentionally narrow: linear algebra, slicing, and broadcasting helpers belong to other array libraries.
- The type is meant to be shared by internal grid-oriented crates, not to replace a full array framework.
- Serialization support exists so row-major checkpoint data can round-trip without an additional array dependency.

## License

MIT
