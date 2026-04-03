# neco-nurbs

[日本語](README-ja.md)

2D and 3D rational NURBS curves, surfaces, and regions for CAD boundaries, profile geometry, and exact smooth-curve work.

A Japanese mathematical note is available in [MATH-ja.md](MATH-ja.md).

## Curve and region operations

`NurbsCurve2D` represents weighted B-spline curves and supports validation, evaluation, sampling, knot insertion, Bezier decomposition, and splitting. Rational circles are provided out of the box, so exact circular profiles can stay analytic.

`NurbsRegion` groups piecewise closed curves into an outer boundary plus optional holes. It works well for 2D CAD domains and as input to meshing or B-Rep construction.

## Usage

### Create and evaluate a curve

```rust
use neco_nurbs::NurbsCurve2D;

let curve = NurbsCurve2D::new(
    2,
    vec![[0.0, 0.0], [1.0, 2.0], [3.0, 2.0], [4.0, 0.0]],
    vec![0.0, 0.0, 0.0, 0.5, 1.0, 1.0, 1.0],
);
curve.validate().unwrap();

let point = curve.evaluate(0.5);
```

### Sample and refine a curve

```rust
let uniform = curve.sample(100);
let adaptive = curve.adaptive_sample(0.1);
let refined = curve.insert_knot(0.25);
let (left, right) = refined.split_at(0.5);
# let _ = (uniform, adaptive, left, right);
```

### Build a region

```rust
use neco_nurbs::{NurbsCurve2D, NurbsRegion};

let region = NurbsRegion {
    outer: vec![NurbsCurve2D::circle([0.0, 0.0], 5.0)],
    holes: vec![vec![NurbsCurve2D::circle([1.0, 0.0], 1.0)]],
};

let points = region.outer_adaptive_sample(0.2);
# let _ = points;
```

## API

| Item | Description |
|------|-------------|
| `NurbsCurve2D` | Rational 2D NURBS curve |
| `NurbsCurve3D` / `NurbsSurface3D` | Rational 3D NURBS curve and surface types |
| `NurbsCurve2D::new` / `new_rational` | Construct non-rational or rational curves |
| `NurbsCurve2D::circle` | Create an exact circle |
| `validate` / `evaluate` | Check consistency and evaluate a point |
| `sample` / `adaptive_sample` / `evaluate_samples` | Uniform, adaptive, and batched evaluation |
| `insert_knot` / `split_at` / `split_at_params` | Refine or split a curve |
| `to_bezier_spans` / `reverse` / `bounding_box` / `is_closed` | Curve utilities |
| `NurbsRegion` | Outer boundary plus optional holes |
| `NurbsRegion::outer_adaptive_sample` | Sample region boundaries for downstream use |
| `fit_nurbs_curve(...)` | Optional nalgebra-backed curve fitting utility (`fitting`) |

### Optional features

| Feature | Description |
|---------|-------------|
| `polynomial-highorder` | Enable nalgebra-backed high-order polynomial routines |
| `fitting` | Enables nalgebra-backed curve fitting utilities |

## License

MIT
