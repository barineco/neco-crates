# neco-cdt

[日本語](README-ja.md)

Constrained Delaunay triangulation with crate-local adaptive predicates for robust 2D meshing and computational geometry.

A Japanese mathematical note is available in [MATH-ja.md](MATH-ja.md).

## Triangulation workflow

`Cdt` incrementally builds a triangulation from 2D points, then recovers constraint edges for boundaries and holes. All orientation and in-circle tests use crate-local adaptive predicates derived from Shewchuk-style expansion arithmetic, so near-degenerate cases are handled more reliably than raw `f64` geometry.

The crate also exposes standalone exact predicates, which are useful outside the triangulation pipeline. `orient3d` keeps the existing sign convention where a positive value means the fourth point lies below the plane through the first three points, and `insphere` expects the first four points in positive `orient3d` order.

## Usage

### Build a triangulation

```rust
use neco_cdt::{Cdt, CdtError};

let mut cdt = Cdt::new((0.0, 0.0, 10.0, 10.0));
cdt.insert(1.0, 1.0);
cdt.insert(5.0, 1.0);
cdt.insert(5.0, 5.0);
cdt.insert(1.0, 5.0);

let triangles = cdt.triangles();
let vertices = cdt.user_vertices();
# let _ = (triangles, vertices);
```

### Add constraint edges

```rust
use neco_cdt::{Cdt, CdtError};

let boundary = [[0.0, 0.0], [4.0, 0.0], [4.0, 3.0], [0.0, 3.0]];
let mut cdt = Cdt::new((-1.0, -1.0, 5.0, 4.0));
cdt.add_constraint_edges(&boundary, true)?;
assert_eq!(cdt.triangles().len(), 2);
# Ok::<(), CdtError>(())
```

### Use exact predicates directly

```rust
use neco_cdt::{incircle, orient2d};

assert!(orient2d([0.0, 0.0], [1.0, 0.0], [0.5, 1.0]) > 0.0);
assert!(incircle([0.0, 0.0], [1.0, 0.0], [0.5, 1.0], [0.5, 0.3]) > 0.0);
```

## API

| Item | Description |
|------|-------------|
| `Cdt::new(bounds)` | Create a triangulation with a super-triangle covering the bounds |
| `Cdt::insert(x, y)` | Insert one point and return its index |
| `Cdt::add_constraint_edges(points, closed)` | Insert points and recover consecutive constraint edges |
| `Cdt::triangles()` / `user_vertices()` | Access output triangles and user vertices |
| `CdtError` | Structured error for recovery failures |
| `orient2d` / `incircle` | Exact 2D predicates |
| `orient3d` / `insphere` | Exact 3D predicates |

## License

This crate is distributed under the MIT license.

`src/robust_impl.rs` includes code derived from `robust`. That file keeps its upstream copyright notice and dual-license notice, and the repository keeps `LICENSE-MIT` and `LICENSE-APACHE` alongside it for provenance.
