# neco-contact

[日本語](README-ja.md)

Hertz contact dynamics and small spatial helper routines for uniform 2D fields.

This crate isolates reusable contact and mask logic from larger solver crates so the helpers can be tested independently of any specific PDE update loop. The array storage layer uses `neco-array2`, and 2D buffers stay on that representation rather than `neco-gridfield`.

## API

| Item | Description |
|------|-------------|
| `find_nearest(x, y, tx, ty)` | Find the grid cell closest to a target point |
| `build_spatial_mask(x, y, hx, hy, width, interior)` | Build a normalized cosine-taper mask |
| `collect_interior(interior, margin)` | Collect active cells that are at least `margin` cells from the border |
| `HertzContact::new(...)` | Construct a simple Hertz beater model |
| `HertzContact::step(w_surface, dt)` | Advance one step and return the contact force |
| `HertzContact::energy()` | Return the current beater kinetic energy |
| `HertzContact::contact_ended()` | Report whether rebound ended the contact |
| `HertzContact::set_contact_ended(ended)` | Override the end-of-contact flag |

## Preconditions

- `build_spatial_mask` normalizes nonzero masks to sum to 1.
- `collect_interior` only filters by the provided boolean mask and integer margin; geometry inference is a caller-side operation.
- `HertzContact` stores beater position and velocity as public state to keep solver checkpoint integration simple.

## License

MIT
