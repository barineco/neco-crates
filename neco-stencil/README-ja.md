# neco-stencil

[English](README.md)

`neco-stencil` は、一様 2D 格子に差分ステンシル演算をかける crate です。

API は行優先に並べた `&[f64]` / `&mut [f64]` に絞っています。

## 格子演算

基本演算としてラプラシアン、1 階微分、2 階微分をそろえています。

複合演算用の補助関数は作業用バッファを再利用し、板の双調和演算を行列型なしの一次元スライス上に組み立てます。

## 使い方

```rust
use neco_stencil::laplacian;

let nx = 5;
let ny = 5;
let dx = 0.1;
let mut field = vec![0.0; nx * ny];
field[2 * ny + 2] = 1.0;

let mut out = vec![0.0; nx * ny];
laplacian(&field, nx, ny, dx, &mut out).expect("valid stencil input");

assert!(out[2 * ny + 2] < 0.0);
assert_eq!(out[0], 0.0);
```

## API

| 項目 | 説明 |
|------|------|
| `laplacian(u, nx, ny, dx, out)` | 一様格子に 5 点 Laplacian を適用する |
| `gradient_x(u, nx, ny, dx, out)` | x 方向の中心差分勾配を計算する |
| `gradient_y(u, nx, ny, dx, out)` | y 方向の中心差分勾配を計算する |
| `d2_dx2(u, nx, ny, dx, out)` | x 方向 2 階微分を計算する |
| `d2_dy2(u, nx, ny, dx, out)` | y 方向 2 階微分を計算する |
| `d2_dxdy(u, nx, ny, dx, out)` | 混合 2 階微分を計算する |
| `w_derivatives(...)` | 単一場の 1 階・2 階微分を 1 回の走査で計算する |
| `uv_gradients(...)` | 2 つの場の勾配を 1 回の走査で計算する |
| `biharmonic_pass1_fused(...)` | `D * Laplacian(w)` と微分用作業バッファを 1 回の走査で計算する |
| `biharmonic(...)` | 作業バッファを使って `Laplacian(D * Laplacian(w))` を適用する |
| `bilaplacian_uniform(w, nx, ny, d, dx, bilap)` | 一様剛性向け 13 点双ラプラシアンを適用する |
| `bilaplacian_ortho_uniform(...)` | 選択した内部セルに直交異方の双ラプラシアンを適用する |

### 前提条件

- 入出力配列の長さは `nx * ny` でなければならない。不一致時は `StencilError` を返す
- データ配置は行優先で、添字は `(i, j) -> i * ny + j`
- 必要な stencil 近傍を持たない境界は 0 を書き込む
- `laplacian`, `gradient_*`, `d2_*`, `w_derivatives`, `uv_gradients`, `biharmonic`, `bilaplacian_uniform` は、必要 stencil を組めない小さな格子では全ゼロを返す
- `bilaplacian_ortho_uniform` は呼び出し側が与える内部 `cells` をそのまま使い、境界は自動ではなく呼び出し側設定を優先

## オプション機能

| 項目 | 説明 |
|---------|-------------|
| `rayon` | 大きい格子の内部ループ並列化。公開結果は非 `rayon` ビルドと一致想定 |

## ライセンス

MIT
