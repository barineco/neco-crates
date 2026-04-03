# neco-stl

[日本語](README-ja.md)

STL file parser and writer with vertex deduplication for triangle surface meshes.

## Features

- Parse binary STL
- Parse ASCII STL
- Deduplicate vertices into indexed triangles
- Filter degenerate triangles
- Write binary and ASCII STL files
- Extract face normals and sharp feature edges

## Usage

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

| Item | Description |
|---|---|
| `TriSurface` | Indexed triangle surface mesh |
| `parse_stl(data)` | Parse STL bytes into `TriSurface` |
| `write_stl_binary(nodes, triangles, path)` | Write binary STL |
| `write_stl_ascii(nodes, triangles, path)` | Write ASCII STL |
| `TriSurface::face_normals()` | Compute per-face normals |
| `TriSurface::feature_edges(angle_threshold_deg)` | Extract boundary and sharp edges |

## License

MIT
