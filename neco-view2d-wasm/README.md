# neco-view2d-wasm

[日本語](README-ja.md)

WebAssembly bindings for [neco-view2d](../neco-view2d). Exposes the `View2d` pan/zoom transform to JavaScript via `wasm-bindgen`.

## JavaScript binding surface

`WasmView2d` mirrors the core pan, zoom, fit, and coordinate-conversion operations of `neco-view2d` and returns small fixed-shape arrays for JavaScript interop.

## Usage

Build with `wasm-pack build --target web`, then import the generated package from JavaScript.

```js
import init, { WasmView2d } from "./pkg/neco_view2d_wasm.js";

await init();

const view = new WasmView2d();

// pan (dx, dy in canvas pixels, canvas_height)
view.pan(50, 30, 600);

// zoom at canvas point (delta, cx, cy, canvas_width, canvas_height)
view.zoom_at(100, 400, 300, 800, 600);

// coordinate conversion (returns [x, y])
const [wx, wy] = view.canvas_to_world(400, 300, 800, 600);
const [cx, cy] = view.world_to_canvas(wx, wy, 800, 600);

// fit world region into canvas
view.fit(1920, 1080, 800, 600);

// get/set state: [center_x, center_y, view_size]
const [centerX, centerY, viewSize] = view.get_state();
view.set_state(0, 0, 10);

// zoom factor relative to a reference view_size
const factor = view.zoom_factor(viewSize);
```

## API

| Item | Description |
|------|-------------|
| `new WasmView2d()` | Create a default view -- center `(0, 0)`, view_size `1.0` |
| `pan(dx, dy, canvas_height)` | Translate the view by `(dx, dy)` in canvas pixels |
| `zoom_at(delta, cx, cy, cw, ch)` | Zoom at canvas point `(cx, cy)`; `delta > 0` zooms in |
| `canvas_to_world(cx, cy, cw, ch)` | Canvas coordinates to world coordinates; returns `[wx, wy]` |
| `world_to_canvas(wx, wy, cw, ch)` | World coordinates to canvas coordinates; returns `[cx, cy]` |
| `fit(ww, wh, cw, ch)` | Fit a world region into the canvas with margin |
| `get_state()` | Returns `[center_x, center_y, view_size]` |
| `set_state(cx, cy, vs)` | Set view state directly |
| `zoom_factor(ref_view_size)` | Current zoom relative to a reference view_size |

## License

MIT
