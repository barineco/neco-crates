# neco-spectral

[English](README.md)

重み付きグラフのスペクトルクラスタリングと、非重み付きグラフの再帰分割をまとめた crate です。

## クラスタリングと分割

重み付き隣接行列 `W` から非正規化ラプラシアン

$$L = D - W$$

を組み立て、`neco-eigensolve` で情報を持つ最小固有ベクトルを取り出し、行正規化した埋め込みを `neco-kmeans` に渡します。

非重み付きの隣接リストには、スペクトル二分割、Kernighan-Lin 改良、再帰分割を使えます。

## 使い方

### 対称グラフのクラスタリング

```rust
use neco_sparse::{CooMat, CsrMat};
use neco_spectral::spectral_cluster;

let mut coo = CooMat::new(100, 100);
for (i, j, w) in edges {
    coo.push(i, j, w);
    coo.push(j, i, w);
}
let adj = CsrMat::from(&coo);

let result = spectral_cluster(&adj, 3, 1e-6, 500, 100);
println!("clusters: {}", result.n_clusters);
println!("assignments: {}", result.assignments.len());
```

### 埋め込み確認

```rust
let result = spectral_cluster(&adj, 3, 1e-6, 500, 100);

// 行正規化済みのスペクトル埋め込み
for row in &result.eigenvectors {
    println!("{row:?}");
}
```

## API

| 項目 | 説明 |
|------|-------------|
| `spectral_cluster(adjacency, n_clusters, tol, max_eigen_iter, max_kmeans_iter)` | スペクトルクラスタリング全体を実行する |
| `spectral_bisect(graph)` | 現行の二分割実装で使う normalized adjacency の第 2 ベクトルで非重み付きグラフを二分割する |
| `kl_refine(graph, part_a, part_b)` | Kernighan-Lin 交換で二分割を改善する |
| `recursive_partition(graph, target_size)` | 各分割が目標サイズ以下になるまで再帰二分割する |
| `count_cut_edges(graph, part_a, part_b)` | 2 分割間をまたぐ辺数を数える |
| `SpectralResult` | 割当結果、クラスタ数、埋め込み、反復回数を返す |
| `SpectralResult::assignments` | 各ノードのクラスタ ID |
| `SpectralResult::eigenvectors` | k-means に渡した行正規化済みの埋め込み |
| `SpectralResult::eigen_iterations` | LOBPCG の反復回数 |
| `SpectralResult::kmeans_iterations` | k-means の反復回数 |

## ライセンス

MIT
