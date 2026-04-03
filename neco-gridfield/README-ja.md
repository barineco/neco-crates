# neco-gridfield

[English](README.md)

時間領域ソルバー向けの一様 2D 格子と三重バッファ状態をまとめた crate です。

大きな波動シミュレーション実装から格子定義と場の状態管理だけを切り出したもので、2D buffer 表現は crate 内で再実装せず `neco-array2` を共有しています。

## API

| 項目 | 説明 |
|------|------|
| `Grid2D::new(lx, ly, dx)` | 一様間隔の 2D 格子を構築し、不正な幾何入力なら `GridError` を返す |
| `Grid2D::coords()` | 中心基準の `x` / `y` 座標配列を返す |
| `Grid2D::radius_map()` | 格子中心からの半径マップを返す |
| `Grid2D::interior_mask(geom)` | 境界形状に応じて内部セル用のマスクを作り、明示マスク形状が不正なら `GridError` を返す |
| `BoundaryGeometry` | 円形、矩形、または明示マスクの境界形状 |
| `FieldSet::new(nx, ny)` | `w`, `u`, `v` 向けの三重バッファを確保 |
| `FieldSet::split_bufs()` | 1 ステップ更新向けに現在・前回・次の 3 バッファを同時借用 |
| `FieldSet::advance()` | 世代カウンタを進めて O(1) でバッファを回転 |
| `FieldSet::to_checkpoint()` | 3 系列の全バッファを保存状態に変換する（`serde` 有効時は直列化可能） |
| `FieldSet::restore_checkpoint(cp)` | 保存状態から全バッファを復元し、形状が不正なら `CheckpointError` を返す |

## 前提条件

- 格子座標は中央セル基準
- `Grid2D::new` は有限な `dx > 0` と有限かつ非負の `lx` / `ly` を前提にし、不正値は `GridError` で返す
- `BoundaryGeometry::Rectangular` は 2 セル幅の外周を無効化する
- `FieldSet` は各場ごとに 3 バッファを持ち、Störmer-Verlet 系で `current` / `previous` を読みつつ `next` を書ける
- `FieldSetCheckpoint` は `neco-array2::Array2` の行優先 flatten を使う

## ライセンス

MIT
