# neco-brep

[日本語](README-ja.md)

Analytical B-Rep modeling for constructive solid geometry, profile-driven solid generation, and mesh export.

A Japanese mathematical note is available in [MATH-ja.md](MATH-ja.md).

## Modeling workflow

`Shell` is the main boundary representation. It stores vertices, edge curves, and face surfaces. Surfaces can stay analytic (`Plane`, `Cylinder`, `Cone`, `Sphere`, `Ellipsoid`, `Torus`) or remain NURBS-backed for revolve, sweep, and loft workflows.

The crate covers both primitive construction and profile-driven solids. Primitive shells include `shell_from_box`, `shell_from_cylinder`, `shell_from_cone`, `shell_from_sphere`, `shell_from_ellipsoid`, and `shell_from_torus`. Profile-driven shells use `shell_from_extrude`, `shell_from_revolve`, `shell_from_sweep`, or `shell_from_loft` with `neco-nurbs` profiles.

`shell_from_sweep` expects the spine as Bezier-decomposed control points. `shell_from_loft` operates on `&[LoftSection]` with a `LoftMode`, and corresponding sections must decompose to the same number of Bezier spans.

## Boolean and export

`boolean_2d_all` and `boolean_3d` keep the result in B-Rep form, so downstream steps can stay analytic until tessellation is needed. `Shell::tessellate` converts the shell to triangles, and the resulting mesh can be exported with `write_stl_binary` or `write_stl_ascii`.

General solid representation is stable for ordinary analytic primitives and standard profile-driven construction. `shell_from_extrude` and `shell_from_revolve` are in better shape than the more complex loft and sweep routes, which still need more hardening.

For 2D boolean, the main result type is `RegionSet`, which can represent empty results, a single region, or multiple disjoint regions. The older `boolean_2d` function remains as a compatibility helper for callers that only accept single-region results.

For 3D boolean, lower-dimensional contact is treated as non-overlap. Point contact, line contact, and other zero-volume contact states produce an empty shell for `Intersect` and keep the minuend unchanged for `Subtract`. `Union` still requires a single connected shell result.

3D boolean operations are experimental. Result completeness is not yet guaranteed, and the tessellation / rendering path still has known bugs. Use them for evaluation, controlled workflows, and incremental validation rather than as a fully reliable production boolean pipeline.

## Usage

### Primitive boolean and tessellation

```rust
use neco_brep::{
    boolean_3d, shell_from_box, shell_from_cylinder, BooleanOp,
};
use neco_brep::stl::write_stl_binary;

let a = shell_from_box(2.0, 2.0, 2.0);
let b = shell_from_cylinder(0.4, None, 2.0);

let result = boolean_3d(&a, &b, BooleanOp::Subtract)?;
let mesh = result.tessellate(24)?;

let mut bytes = Vec::new();
write_stl_binary(&mesh, &mut bytes)?;
# Ok::<(), Box<dyn std::error::Error>>(())
```

### Extrude a NURBS profile

```rust
use neco_brep::shell_from_extrude;
use neco_nurbs::{NurbsCurve2D, NurbsRegion};

let profile = NurbsRegion {
    outer: vec![NurbsCurve2D::circle([0.0, 0.0], 1.0)],
    holes: vec![],
};

let shell = shell_from_extrude(&profile, [0.0, 0.0, 1.0], 2.0)?;
# let _ = shell;
# Ok::<(), String>(())
```

## API

| Item | Description |
|------|-------------|
| `Shell` | Boundary representation with vertices, edges, and faces |
| `Surface` | Analytic or NURBS-backed face geometry |
| `Curve3D` | 3D edge curve types |
| `shell_from_box` / `shell_from_cylinder` / `shell_from_cone` / `shell_from_sphere` / `shell_from_ellipsoid` / `shell_from_torus` | Primitive solid constructors |
| `shell_from_extrude` / `shell_from_revolve` / `shell_from_sweep` / `shell_from_loft` | Profile-driven solid constructors |
| `boolean_2d_all` / `boolean_3d` | Boolean operations for region sets and shells |
| `boolean_2d` | Compatibility helper that succeeds only for single-region 2D results |
| `RegionSet` | Zero, one, or many 2D boolean result regions |
| `BooleanOp` | `Union`, `Subtract`, and `Intersect` |
| `Shell::tessellate(density)` | Convert a shell to triangles |
| `TriMesh` | Triangle mesh output for rendering and export |
| `stl::write_stl_binary` / `stl::write_stl_ascii` | Export the tessellated mesh as STL |

## License

MIT
