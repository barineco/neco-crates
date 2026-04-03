# neco-cdt

[English](README.md)

退化に強い 2D メッシュ生成と計算幾何向けに、crate 内の適応精度付き制約 Delaunay 三角形分割を実装したものです。

詳細な数理背景は [MATH-ja.md](MATH-ja.md) を参照してください。

## 三角形分割フロー

`Cdt` は 2D 点群から逐次的に三角形分割を構築し、その後で境界や穴に対応する制約辺を回復します。向き判定と外接円判定は、Shewchuk 系の拡張演算を元にした crate 内の適応精度演算で行うので、退化に近い配置でも素朴な `f64` 幾何より安定です。

三角形分割以外でも使えるよう、単独の厳密判定 API も公開し、`orient3d` は「4 点目が最初の 3 点の平面の下側なら正」という既存の符号約束を保ち、`insphere` は最初の 4 点が正の `orient3d` 順になることを前提にしています。

## 使い方

### 三角形分割

```rust
use neco_cdt::{Cdt, CdtError};

let mut cdt = Cdt::new((0.0, 0.0, 10.0, 10.0));
cdt.insert(1.0, 1.0);
cdt.insert(5.0, 1.0);
cdt.insert(5.0, 5.0);
cdt.insert(1.0, 5.0);

let triangles = cdt.triangles();
let vertices = cdt.user_vertices();
# let _ = (triangles, vertices);
```

### 制約辺追加

```rust
use neco_cdt::{Cdt, CdtError};

let boundary = [[0.0, 0.0], [4.0, 0.0], [4.0, 3.0], [0.0, 3.0]];
let mut cdt = Cdt::new((-1.0, -1.0, 5.0, 4.0));
cdt.add_constraint_edges(&boundary, true)?;
assert_eq!(cdt.triangles().len(), 2);
# Ok::<(), CdtError>(())
```

### 厳密判定の直接利用

```rust
use neco_cdt::{incircle, orient2d};

assert!(orient2d([0.0, 0.0], [1.0, 0.0], [0.5, 1.0]) > 0.0);
assert!(incircle([0.0, 0.0], [1.0, 0.0], [0.5, 1.0], [0.5, 0.3]) > 0.0);
```

## API

| 項目 | 説明 |
|------|-------------|
| `Cdt::new(bounds)` | 既定境界を覆うスーパー三角形付きで初期化する |
| `Cdt::insert(x, y)` | 点を 1 つ挿入してインデックスを返す |
| `Cdt::add_constraint_edges(points, closed)` | 点を挿入し、連続する制約辺を回復する |
| `Cdt::triangles()` / `user_vertices()` | 出力三角形とユーザー頂点を取得する |
| `CdtError` | 制約辺回復失敗を表す構造化エラー |
| `orient2d` / `incircle` | 厳密 2D 判定 |
| `orient3d` / `insphere` | 厳密 3D 判定 |

## ライセンス

このクレートは MIT ライセンスで配布します。

`src/robust_impl.rs` には `robust` 由来の実装を含みます。このファイルには upstream の著作権表示と dual-license の notice をそのまま残しており、リポジトリにも provenance のため `LICENSE-MIT` と `LICENSE-APACHE` を併せて置いています。
