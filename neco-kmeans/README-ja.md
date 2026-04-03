# neco-kmeans

[English](README.md)

乱数初期化のぶれを抑えて点群を再現可能な形で `k` 個へ分ける、決定論的 k-means 実装です。

## クラスタリングの挙動

初期値には決定論的な最大距離版 k-means++ を使い、その後 Lloyd 反復で割当を更新します。同じ入力なら常に同じ結果になるので、再現性が必要な解析パイプラインに向いています。

## 使い方

```rust
use neco_kmeans::kmeans;

let data = [
    0.0, 0.0,
    1.0, 0.0,
    10.0, 10.0,
    11.0, 10.0,
];

let result = kmeans(&data, 2, 2, 100).expect("valid clustering input");

println!("assignments: {:?}", result.assignments);
println!("centroids: {:?}", result.centroids);
println!("iterations: {}", result.iterations);
```

## API

| 項目 | 説明 |
|------|-------------|
| `kmeans(data, dim, k, max_iter)` | 決定論的 k-means を実行して `Result<KmeansResult, KmeansError>` を返す |
| `KmeansResult` | 割当、重心、次元数、反復回数を保持する |
| `KmeansResult::centroid(i)` | `i` 番目の重心をスライスで参照する |
| `KmeansResult::k()` | クラスタ数を返す |

### オプション機能

| 項目 | 説明 |
|---------|-------------|
| `parallel` | rayon による並列割当更新を有効化する |

## ライセンス

MIT
