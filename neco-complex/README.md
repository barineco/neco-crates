# neco-complex

[日本語](README-ja.md)

Lightweight complex-number foundation for FFT- and solver-adjacent crates.

This crate provides a narrow `Complex<T>` used by spectrum buffers, FFT facades, and solver-side helpers. General-purpose complex numerics are handled by ecosystem crates that target those APIs.

## API

| Item | Description |
|------|-------------|
| `Complex::new(re, im)` | Construct a complex value |
| `Complex::zero()` | Construct the origin for `f32` / `f64` |
| `Complex::conj()` | Return the complex conjugate |
| `Complex::norm_sqr()` | Return squared magnitude |
| `Complex::arg()` | Return phase angle |
| `+`, `-`, `*`, `/ scalar` | Minimal arithmetic needed by FFT and spectrum code |

## Preconditions

- The API is intentionally narrow and focused on FFT / spectrum / lightweight solver needs.
- The type exposes plain `re` / `im` fields so backend bridges can stay small.
- High-level complex analysis helpers and matrix abstractions are handled by dedicated crates outside this core.

## License

MIT
