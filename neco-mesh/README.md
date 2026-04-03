# neco-mesh

[日本語](README-ja.md)

1D/2D/3D mesh and voxel discretization primitives for computational geometry and simulation.

A Japanese mathematical note is available in [MATH-ja.md](MATH-ja.md).

## Features

- `mesh1d`: uniform and adaptive edge meshing for line segments and NURBS curves
- `mesh2d`: constrained Delaunay triangulation for polygons and NURBS regions
- `mesh3d`: quality tetrahedral meshing with Delaunay insertion, boundary recovery, refinement, and improvement
- `voxel`: structured voxel grids, wire/frame voxelization helpers, surface occupancy, and solid occupancy entry points
- `immersed`: structured-grid immersed boundary meshing with fill fractions

## Usage

```toml
[dependencies]
neco-mesh = { path = "../neco-mesh" }
```

```rust
use neco_mesh::{generate_quality_mesh, mesh_rect};

let tri = mesh_rect(1.0, 1.0, 0.25);
let tet = generate_quality_mesh(
    &[
        [0.0, 0.0, 0.0],
        [1.0, 0.0, 0.0],
        [1.0, 1.0, 0.0],
        [0.0, 1.0, 0.0],
        [0.0, 0.0, 1.0],
        [1.0, 0.0, 1.0],
        [1.0, 1.0, 1.0],
        [0.0, 1.0, 1.0],
    ],
    &[
        [0, 2, 1], [0, 3, 2], [4, 5, 6], [4, 6, 7],
        [0, 1, 5], [0, 5, 4], [2, 3, 7], [2, 7, 6],
        [0, 4, 7], [0, 7, 3], [1, 2, 6], [1, 6, 5],
    ],
    None,
)?;

println!("triangles={}, tetrahedra={}", tri.triangles.len(), tet.tetrahedra.len());
# Ok::<(), String>(())
```

## API

| Item | Description |
|---|---|
| `EdgeMesh` | 1D edge mesh |
| `TriMesh2D` | 2D triangle mesh |
| `TetMesh3D` | 3D tetrahedral mesh |
| `VoxelGrid<T>` | Common structured voxel grid type with flat storage |
| `FillFractionGrid` | Fill-fraction field on top of `VoxelGrid<f64>` |
| `OccupancyGrid` | Binary occupancy field on top of `VoxelGrid<bool>` |
| `UniformGrid3` | Spatial layout metadata where `origin + [i, j, k] * spacing` gives the grid point for `(i, j, k)` |
| `SpatialVoxelGrid<T>` | Voxel field bundled with the world-coordinate grid points used for sampling |
| `ImmersedMesh` | Tetrahedral mesh with fill fractions |
| `mesh_line(origin, length, direction, max_edge)` | Mesh a line segment |
| `mesh_curve(curve, max_edge)` | Mesh a NURBS curve |
| `mesh_rect(width, height, max_edge)` | Mesh a rectangle |
| `mesh_polygon(boundary, max_edge) -> Result<TriMesh2D, neco_cdt::CdtError>` | Mesh a polygon |
| `mesh_polygon_adaptive(boundary, max_edge, min_nodes_per_width) -> Result<TriMesh2D, neco_cdt::CdtError>` | Adaptive polygon meshing |
| `mesh_region(region, max_edge) -> Result<TriMesh2D, neco_cdt::CdtError>` | Mesh a NURBS region |
| `mesh_region_adaptive(region, max_edge, min_nodes_per_width) -> Result<TriMesh2D, neco_cdt::CdtError>` | Adaptive region meshing |
| `point_in_polygon(point, polygon)` | Point-in-polygon test |
| `generate_quality_mesh(nodes, triangles, params)` | Generate a quality tetrahedral mesh |
| `RodGeometry::fill_fraction(dx, nx, ny, nz, cx, cy, cz)` | Wire helper that rasterizes a rod centerline into a fill-fraction grid |
| `TriangleGeometry::to_rod_geometry()` | Wire/frame helper that converts an equilateral frame into a rod centerline |
| `surface_occupancy(surface_nodes, surface_triangles, max_edge)` | Evaluate occupancy at each grid point and return the field with origin / spacing metadata |
| `solid_occupancy(surface_nodes, surface_triangles, max_edge) -> Result<SpatialVoxelGrid<bool>, SolidOccupancyError>` | Validate closed-surface topology, then evaluate interior occupancy using the same grid layout contract |
| `generate_immersed_mesh(nodes, triangles, max_edge)` | Generate an immersed boundary mesh |

## License

MIT
