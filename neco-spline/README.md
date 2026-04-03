# neco-spline

[日本語](README-ja.md)

Natural cubic spline interpolation with no dependencies beyond the standard library by default, plus an optional `serde` feature. Useful when you need a smooth curve through a set of data points.

A Japanese mathematical note is available in [MATH-ja.md](MATH-ja.md).

## Natural cubic spline

Given data points $(x_0, y_0), \dots, (x_n, y_n)$, the spline constructs a piecewise cubic polynomial that passes through every point while maintaining $C^2$ continuity. On each interval the polynomial is:

$$S_i(x) = a_i + b_i(x - x_i) + c_i(x - x_i)^2 + d_i(x - x_i)^3$$

The *natural* boundary conditions ($S'' = 0$ at both endpoints) yield a tridiagonal system solved in $O(n)$ time by the Thomas algorithm. Among all interpolating splines, this is the unique one that minimizes total curvature $\int |f''(x)|^2 \, dx$.

## Usage

```toml
[dependencies]
neco-spline = "0.1"
```

### Basic interpolation

```rust
use neco_spline::CubicSpline;

let spline = CubicSpline::new(&[
    (0.0, 0.0),
    (0.25, 0.4),
    (0.75, 0.9),
    (1.0, 1.0),
]).unwrap();

let y = spline.evaluate(0.5);
```

### Boundary clamping

Values outside the data range return the nearest endpoint value:

```rust
let spline = CubicSpline::new(&[(0.0, 1.0), (1.0, 2.0)]).unwrap();

assert_eq!(spline.evaluate(-1.0), 1.0); // clamped to left endpoint
assert_eq!(spline.evaluate(5.0),  2.0); // clamped to right endpoint
```

### Error handling

```rust
use neco_spline::{CubicSpline, SplineError};

// Fewer than 2 points
let err = CubicSpline::new(&[(0.0, 0.0)]);
assert!(matches!(err, Err(SplineError::InsufficientPoints)));

// Non-ascending x values
let err = CubicSpline::new(&[(1.0, 0.0), (0.5, 1.0)]);
assert!(matches!(err, Err(SplineError::NonAscendingX)));
```

## API

| Item | Description |
|------|-------------|
| `CubicSpline` | Natural cubic spline interpolator |
| `CubicSpline::new(points)` | Build a spline from `&[(f32, f32)]` (returns `Result`) |
| `CubicSpline::evaluate(x)` | Evaluate the spline at a given $x$ |
| `SplineError::InsufficientPoints` | Fewer than 2 control points |
| `SplineError::NonAscendingX` | Control points not in strictly ascending $x$ order |

### Optional features

| Feature | Description |
|---------|-------------|
| `serde` | Enables `Serialize` / `Deserialize` for `CubicSpline` |

## License

MIT
