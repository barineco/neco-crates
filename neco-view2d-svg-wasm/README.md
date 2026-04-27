# neco-view2d-svg-wasm

[日本語](README-ja.md)

minimum dependency WebAssembly bindings for neco-view2d-svg via wasm-bindgen

## Overview

WebAssembly bindings for [neco-view2d-svg](../neco-view2d-svg). Generates SVG attribute strings from `neco-view2d` view state and world coordinates, exposed to JavaScript via `wasm-bindgen`.

## Features

The crate exposes three functions: `emit_transform`, `emit_polyline`, and `emit_path`. Each takes a view (center and view size) and canvas dimensions, projects world coordinates onto canvas space, and returns a string ready to embed as an SVG attribute value.

`emit_transform` produces a `transform` attribute value for an SVG root or group element. `emit_polyline` produces the `points` attribute value of a polyline, and `emit_path` produces the `d` attribute value of a path. Both polyline and path consume a flat `[x0, y0, x1, y1, ...]` world-coordinate array.

## Usage

Build with `wasm-pack build --target web`, then import the generated package from JavaScript.

```js
import init, { emit_transform, emit_polyline, emit_path } from "./pkg/neco_view2d_svg_wasm.js";

await init();

const cx = 0;
const cy = 0;
const vs = 10;
const cw = 800;
const ch = 600;

// SVG transform attribute value
const transform = emit_transform(cx, cy, vs, cw, ch);

// polyline points attribute value
const points = new Float64Array([0, 0, 1, 1, 2, 0]);
const polyline = emit_polyline(cx, cy, vs, points, cw, ch);

// path d attribute value
const d = emit_path(cx, cy, vs, points, cw, ch);
```

## API

| Item | Description |
|------|-------------|
| `emit_transform(center_x, center_y, view_size, canvas_w, canvas_h)` | Build the SVG `transform` attribute value from view state and canvas size |
| `emit_polyline(center_x, center_y, view_size, points, canvas_w, canvas_h)` | Build a polyline `points` attribute value from a flat `[x0, y0, x1, y1, ...]` world-coordinate array |
| `emit_path(center_x, center_y, view_size, points, canvas_w, canvas_h)` | Build a path `d` attribute value from a flat `[x0, y0, x1, y1, ...]` world-coordinate array |

## License

Licensed under the MIT License. See [LICENSE](LICENSE).
