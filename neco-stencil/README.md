# neco-stencil

[日本語](README-ja.md)

Finite-difference stencil operators for uniform 2D grids on row-major `&[f64]` / `&mut [f64]` slices.

## Grid operators

The basic operators cover Laplacian, first derivatives, and second derivatives on a uniform 2D grid.

The fused routines reuse work buffers so plate-style biharmonic assembly can stay in flat slices without introducing a matrix type.

## Usage

```rust
use neco_stencil::laplacian;

let nx = 5;
let ny = 5;
let dx = 0.1;
let mut field = vec![0.0; nx * ny];
field[2 * ny + 2] = 1.0;

let mut out = vec![0.0; nx * ny];
laplacian(&field, nx, ny, dx, &mut out).expect("valid stencil input");

assert!(out[2 * ny + 2] < 0.0);
assert_eq!(out[0], 0.0);
```

## API

| Item | Description |
|------|-------------|
| `laplacian(u, nx, ny, dx, out)` | Apply the 5-point Laplacian on a uniform grid |
| `gradient_x(u, nx, ny, dx, out)` | Compute the central-difference x-gradient |
| `gradient_y(u, nx, ny, dx, out)` | Compute the central-difference y-gradient |
| `d2_dx2(u, nx, ny, dx, out)` | Compute the second x-derivative |
| `d2_dy2(u, nx, ny, dx, out)` | Compute the second y-derivative |
| `d2_dxdy(u, nx, ny, dx, out)` | Compute the mixed second derivative |
| `w_derivatives(...)` | Compute first and second derivatives of one field in one pass |
| `uv_gradients(...)` | Compute gradients of two fields in one pass |
| `biharmonic_pass1_fused(...)` | Compute `D * Laplacian(w)` and derivative work buffers in one pass |
| `biharmonic(...)` | Apply `Laplacian(D * Laplacian(w))` using work buffers |
| `bilaplacian_uniform(w, nx, ny, d, dx, bilap)` | Apply the 13-point bilaplacian for uniform stiffness |
| `bilaplacian_ortho_uniform(...)` | Apply an orthotropic bilaplacian on selected interior cells |

### Preconditions

- Inputs and outputs must have length `nx * ny`; invalid lengths return `StencilError`.
- Data layout is row-major with index `(i, j) -> i * ny + j`.
- Operators write zeros on boundaries that do not have the required stencil neighborhood.
- `laplacian`, `gradient_*`, `d2_*`, `w_derivatives`, `uv_gradients`, `biharmonic`, and `bilaplacian_uniform` return all zeros when the grid is too small for the required stencil.
- `bilaplacian_ortho_uniform` expects caller-provided interior `cells`; boundary filtering happens before this call.

## Optional features

| Feature | Description |
|---------|-------------|
| `rayon` | Enables internal parallel loops for larger grids only; public results are intended to stay identical to the non-`rayon` build |

## License

MIT
