# neco-spectral

[English](README.md)

重み付きグラフと非重み付きグラフのスペクトルクラスタリング、再帰分割を提供する crate です。

## クラスタリングと分割

重み付きグラフでは、隣接行列から非正規化 Laplacian を構成します。

$$L = D - W$$

次数行列を mass matrix とする一般化固有値問題を解き、得られた埋め込みを行ごとに正規化してクラスタへ割り当てます。孤立 node には `1.0` の mass を置きます。

非重み付き隣接リストには、スペクトル二分割、Kernighan-Lin 改良、再帰分割もあります。

## 使い方

### 対称グラフのクラスタリング

```rust
use neco_eigensolve::EigensolveConfig;
use neco_linear_types::Shape;
use neco_sparse::{CooMatrix, CsrMatrix};
use neco_spectral::spectral_cluster;

let shape = Shape::new(6, 6);
let mut adjacency = CooMatrix::new(shape);
for (left, right, weight) in [
    (0, 1, 4.0),
    (1, 2, 4.0),
    (0, 2, 4.0),
    (3, 4, 4.0),
    (4, 5, 4.0),
    (3, 5, 4.0),
    (2, 3, 0.01),
] {
    adjacency.push(shape.row_index(left)?, shape.column_index(right)?, weight)?;
    adjacency.push(shape.row_index(right)?, shape.column_index(left)?, weight)?;
}
let adjacency: CsrMatrix<f64> = adjacency.to_csr()?;
let config = EigensolveConfig::new(2, 1.0e-9, 1.0e-9, 64)?;
let result = spectral_cluster(&adjacency, 2, config, 32)?;

println!("clusters: {}", result.cluster_count());
println!("assignments: {}", result.assignments().len());
# Ok::<(), Box<dyn std::error::Error>>(())
```

### 埋め込みの確認

```rust
# use neco_eigensolve::EigensolveConfig;
# use neco_linear_types::Shape;
# use neco_sparse::CooMatrix;
# use neco_spectral::spectral_cluster;
# let shape = Shape::new(2, 2);
# let mut adjacency = CooMatrix::new(shape);
# adjacency.push(shape.row_index(0)?, shape.column_index(1)?, 1.0)?;
# adjacency.push(shape.row_index(1)?, shape.column_index(0)?, 1.0)?;
# let adjacency = adjacency.to_csr()?;
# let config = EigensolveConfig::new(1, 1.0e-9, 1.0e-9, 64)?;
let result = spectral_cluster(&adjacency, 1, config, 32)?;

for row in result.embedding() {
    println!("{row:?}");
}
# Ok::<(), Box<dyn std::error::Error>>(())
```

## API

クラスタリング API は次のとおりです。

- `spectral_cluster`
- `SpectralResult::assignments()`
- `SpectralResult::cluster_count()`
- `SpectralResult::embedding()`
- `SpectralResult::convergence()`
- `SpectralResult::kmeans_iterations()`
- `SpectralError`

```text
spectral_cluster(adjacency, cluster_count, eigensolve_config, max_kmeans_iterations)
```

グラフ分割 API は次のとおりです。

- `spectral_bisect(graph)`
- `kl_refine(graph, part_a, part_b)`
- `recursive_partition(graph, target_size)`
- `count_cut_edges(graph, part_a, part_b)`

## 事前条件

- 隣接行列: 正方、有限値、対称
- cluster 数: `1..=node_count`
- 固有値計算の mode 数: cluster 数と一致

## ライセンス

MIT
