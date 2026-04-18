# neco-view2d-svg

[日本語](README-ja.md)

necosystems series SVG attribute emitters for neco-view2d world coordinates

## Overview

`neco-view2d-svg` converts `neco_view2d::View2d` world coordinates into SVG attribute strings for `transform`, `points`, and `d` values without DOM or SVG library dependencies.

## Usage

```rust
use neco_view2d::View2d;
use neco_view2d_svg::world_transform_attr;

let transform = world_transform_attr(&View2d::default(), 800.0, 600.0);
```

```rust
use neco_view2d::View2d;
use neco_view2d_svg::world_points_to_polyline;

let points = world_points_to_polyline(&View2d::default(), &[(0.0, 0.0), (1.0, 1.0)], 800.0, 600.0);
```

```rust
use neco_view2d::View2d;
use neco_view2d_svg::world_points_to_svg_d;

let path_d = world_points_to_svg_d(&View2d::default(), &[(0.0, 0.0), (1.0, 1.0)], 800.0, 600.0);
```

## API

- `world_transform_attr(view, canvas_w, canvas_h)` returns `translate(tx,ty) scale(sx,sy)` for a world-space `<g>`.
- `world_points_to_polyline(view, points, canvas_w, canvas_h)` returns a `<polyline points="...">` attribute string.
- `world_points_to_svg_d(view, points, canvas_w, canvas_h)` returns a `<path d="...">` attribute string.
- Public functions expect finite floating-point inputs. Non-finite values may be emitted as `NaN` or `inf` strings.

## License

MIT

## Related

- [`neco-view2d`](https://docs.rs/neco-view2d)
