# neco-view2d

[日本語](README-ja.md)

A minimal 2D camera / viewport transform for moving between world space and canvas space. Useful for image viewers, 2D editors, and any UI where the user pans and zooms over a plane.

## Coordinate transform

The transform maps between **world space** and **canvas space** (screen / CSS pixels).

`View2d` stores `center_x`, `center_y`, and `view_size`, where `view_size` is the visible world-space height on the canvas.

- `canvas_to_world(cx, cy, canvas_width, canvas_height)` converts a canvas point to world coordinates.
- `world_to_canvas(wx, wy, canvas_width, canvas_height)` converts a world point to canvas coordinates.
- `pan(dx, dy, canvas_height)` shifts the view by a pixel delta scaled by `view_size / canvas_height`.
- `zoom_at(delta, canvas_x, canvas_y, canvas_width, canvas_height)` keeps the world point under the cursor fixed while changing `view_size`.

`view_size` is clamped to stay positive. Smaller `view_size` means more zoom.

## Usage

```rust
use neco_view2d::View2d;

let mut view = View2d::default();

// pan by a pixel delta on a 600px-tall canvas
view.pan(50.0, 30.0, 600.0);

// zoom in around canvas center
view.zoom_at(120.0, 400.0, 300.0, 800.0, 600.0);

// coordinate conversion
let (cx, cy) = view.world_to_canvas(100.0, 200.0, 800.0, 600.0);
let (wx, wy) = view.canvas_to_world(cx, cy, 800.0, 600.0);
```

## API

| Item | Description |
|------|-------------|
| `View2d::default()` | Default view centered at `(0, 0)` with `view_size = 1.0` |
| `set(center_x, center_y, view_size)` | Set the view center and visible world-space height |
| `pan(dx, dy, canvas_height)` | Translate the view by a pixel delta |
| `zoom_at(delta, canvas_x, canvas_y, canvas_width, canvas_height)` | Zoom around a canvas point while keeping its world position fixed |
| `world_to_canvas(wx, wy, canvas_width, canvas_height) -> (f64, f64)` | World coordinates to canvas coordinates |
| `canvas_to_world(cx, cy, canvas_width, canvas_height) -> (f64, f64)` | Canvas coordinates to world coordinates |
| `fit(world_width, world_height, canvas_width, canvas_height)` | Fit a world-space rectangle into the canvas |
| `zoom_factor(reference_view_size)` | Return zoom relative to a reference `view_size` |

### Optional features

| Feature | Description |
|---------|-------------|
| `serde` | Enables `Serialize` / `Deserialize` on `View2d` |

## License

MIT
