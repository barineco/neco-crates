# neco-edge-routing

[日本語](README-ja.md)

necosystems series 2D edge routing primitives for node graphs

## Overview

`neco-edge-routing` computes edge paths from two endpoints and their tangents without taking a dependency on any renderer, UI framework, or node graph data model.

The output stays in a neutral `PathData` form that SVG, canvas, or other emitters can consume later.

## Features

| Feature | Description |
|---------|-------------|
| `default` | No optional dependencies. Provides `Linear`, `Bezier`, and `Orthogonal` routing |
| `spline` | Enables `RouteStyle::Spline` via `neco-spline` and emits cubic Bezier segments |
| `nurbs` | Enables `RouteStyle::Nurbs` via `neco-nurbs` and returns NURBS control data |

When `spline` or `nurbs` is not enabled, the corresponding `RouteStyle` variant still exists, and `route()` returns `RoutingError::FeatureDisabled` instead of falling back silently.

## Usage

```rust
use neco_edge_routing::{route, RouteRequest, RouteStyle};

let path = route(&RouteRequest {
    from: (0.0, 0.0),
    to: (120.0, 40.0),
    from_tangent: (1.0, 0.0),
    to_tangent: (-1.0, 0.0),
    style: RouteStyle::Linear,
})?;

assert_eq!(path.points, vec![(0.0, 0.0), (120.0, 40.0)]);
# Ok::<(), neco_edge_routing::RoutingError>(())
```

```rust
use neco_edge_routing::{route, RouteRequest, RouteStyle};

let path = route(&RouteRequest {
    from: (0.0, 0.0),
    to: (120.0, 40.0),
    from_tangent: (1.0, 0.0),
    to_tangent: (-1.0, 0.0),
    style: RouteStyle::Bezier { curvature: 0.25 },
})?;

assert_eq!(path.points.len(), 4);
# Ok::<(), neco_edge_routing::RoutingError>(())
```

```rust
use neco_edge_routing::{route, RouteRequest, RouteStyle};

let path = route(&RouteRequest {
    from: (0.0, 0.0),
    to: (120.0, 40.0),
    from_tangent: (1.0, 0.0),
    to_tangent: (-1.0, 0.0),
    style: RouteStyle::Orthogonal { corner_radius: 8.0 },
})?;

assert!(!path.points.is_empty());
# Ok::<(), neco_edge_routing::RoutingError>(())
```

Spline and NURBS routes are available through the corresponding Cargo features.

## API

| Item | Description |
|------|-------------|
| `RouteStyle` | Routing strategy: `Linear`, `Bezier`, `Orthogonal`, `Spline`, or `Nurbs` |
| `RouteRequest` | Input endpoints, tangents, and the requested style |
| `PathData` | Routed control points plus the semantic `PathKind` |
| `PathKind` | `Polyline`, `Cubic`, `Quadratic`, or `Nurbs { knots, weights }` |
| `route(&RouteRequest)` | Pure routing entry point returning `Result<PathData, RoutingError>` |
| `RoutingError` | Invalid input or feature-disabled style request |

## License

MIT
