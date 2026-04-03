# neco-mesh

[English](README.md)

計算幾何と数値シミュレーション向けの 1D / 2D / 3D メッシュと voxel 離散化ライブラリです。

詳細な数理背景は [MATH-ja.md](MATH-ja.md) を参照してください。

## 機能

- `mesh1d`: 線分 / NURBS 曲線の 1D 線分メッシュ
- `mesh2d`: polygon / NURBS region の制約付き Delaunay 三角形分割
- `mesh3d`: Delaunay insertion、boundary recovery、refinement、improvement による品質テトラメッシュ生成
- `voxel`: 構造 voxel 格子、ワイヤー / フレームのボクセル化補助、サーフェス占有率、ソリッド占有率への入口
- `immersed`: 充填率付きの埋め込み境界メッシュ

## 使い方

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

| 項目 | 説明 |
|---|---|
| `EdgeMesh` | 1D 線分メッシュ |
| `TriMesh2D` | 2D 三角形メッシュ |
| `TetMesh3D` | 3D 四面体メッシュ |
| `VoxelGrid<T>` | 連続配置を持つ共通のボクセル格子 |
| `FillFractionGrid` | `VoxelGrid<f64>` 上の充填率場 |
| `OccupancyGrid` | `VoxelGrid<bool>` 上の二値占有場 |
| `UniformGrid3` | `origin + [i, j, k] * spacing` で `(i, j, k)` に対応する格子点座標を与える一様格子レイアウト |
| `SpatialVoxelGrid<T>` | 占有判定に用いた格子点情報を束ねたボクセル場 |
| `ImmersedMesh` | 充填率付き四面体メッシュ |
| `mesh_line(origin, length, direction, max_edge)` | 線分をメッシュ化する |
| `mesh_curve(curve, max_edge)` | NURBS 曲線をメッシュ化する |
| `mesh_rect(width, height, max_edge)` | 矩形をメッシュ化する |
| `mesh_polygon(boundary, max_edge) -> Result<TriMesh2D, neco_cdt::CdtError>` | 多角形をメッシュ化する |
| `mesh_polygon_adaptive(boundary, max_edge, min_nodes_per_width) -> Result<TriMesh2D, neco_cdt::CdtError>` | 幅にもとづく適応多角形メッシュ |
| `mesh_region(region, max_edge) -> Result<TriMesh2D, neco_cdt::CdtError>` | NURBS 領域をメッシュ化する |
| `mesh_region_adaptive(region, max_edge, min_nodes_per_width) -> Result<TriMesh2D, neco_cdt::CdtError>` | 適応 NURBS 領域メッシュ |
| `point_in_polygon(point, polygon)` | 点が多角形の内側にあるか判定する |
| `generate_quality_mesh(nodes, triangles, params)` | 品質テトラメッシュ生成 |
| `RodGeometry::fill_fraction(dx, nx, ny, nz, cx, cy, cz)` | 棒材中心線を充填率格子へ焼き込む |
| `TriangleGeometry::to_rod_geometry()` | 正三角フレームを `rod centerline` からロッド中心線へ変換する |
| `surface_occupancy(surface_nodes, surface_triangles, max_edge)` | 各格子点で占有判定し、原点・間隔を含めて返す |
| `solid_occupancy(surface_nodes, surface_triangles, max_edge) -> Result<SpatialVoxelGrid<bool>, SolidOccupancyError>` | 閉曲面検証を行ったうえで、内部占有率を同一の格子レイアウト規約で返す |
| `generate_immersed_mesh(nodes, triangles, max_edge)` | 埋め込み境界メッシュ生成 |

## ライセンス

MIT
