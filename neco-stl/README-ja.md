# neco-stl

[English](README.md)

三角形表面メッシュ向けの STL パーサー / ライター。頂点重複除去に対応。

## 機能

- バイナリ STL のパース
- ASCII STL のパース
- 頂点重複除去による添字化
- 縮退三角形を除去
- バイナリ / ASCII STL の書き出し
- 面法線と特徴エッジの抽出

## 使い方

```toml
[dependencies]
neco-stl = { path = "../neco-stl" }
```

```rust
use neco_stl::parse_stl;

let bytes = std::fs::read("mesh.stl")?;
let surface = parse_stl(&bytes)?;
println!("nodes={}, triangles={}", surface.nodes.len(), surface.triangles.len());
# Ok::<(), Box<dyn std::error::Error>>(())
```

## API

| 項目 | 説明 |
|------|-------------|
| `TriSurface` | インデックス付き三角形表面メッシュ |
| `parse_stl(data)` | STL バイト列を `TriSurface` に変換 |
| `write_stl_binary(nodes, triangles, path)` | バイナリ STL を書き出す |
| `write_stl_ascii(nodes, triangles, path)` | ASCII STL を書き出す |
| `TriSurface::face_normals()` | 面法線を計算 |
| `TriSurface::feature_edges(angle_threshold_deg)` | 境界・鋭角エッジを抽出 |

## ライセンス

MIT
