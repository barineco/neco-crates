# neco-edge-routing-wasm

[日本語](README-ja.md)

minimum dependency WebAssembly bindings for [neco-edge-routing](../neco-edge-routing) via `wasm-bindgen`.

## Routing surface

`route_edge` computes a 2D edge path between two points and returns the result as a plain JavaScript object containing `style`, `kind`, and a `points` array. NURBS results additionally carry `knots` and `weights`.

Supported style names:

- `bezier`: cubic Bezier route with tangent-scaled handles
- `orthogonal`: axis-aligned route with rounded corners
- `spline`: natural cubic spline route
- `nurbs`: NURBS control path

## Usage

Build with `wasm-pack build --target web`, then import the generated package from JavaScript or TypeScript.

```ts
import init, { route_edge } from "./pkg/neco_edge_routing_wasm.js";

await init();

const path = route_edge("bezier", 0, 0, 100, 50);
// path: { style: "bezier", kind: "cubic", points: [{x, y}, ...] }

const nurbs = route_edge("nurbs", 0, 0, 100, 50);
// nurbs: { style: "nurbs", kind: "nurbs", points, knots, weights }
```

Unsupported style names and non-finite inputs return a thrown `Error` carrying the underlying message.

## API

| Item | Description |
|------|-------------|
| `route_edge(style, from_x, from_y, to_x, to_y)` | Route an edge between two points; returns a plain object with `style`, `kind`, `points`, and NURBS metadata when applicable |

## License

Licensed under the MIT License. See [LICENSE](LICENSE).
