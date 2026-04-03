# neco-contact

[English](README.md)

一様 2D 場向けの Hertz 接触力学と小さな空間補助をまとめた crate です。

大きなソルバー crate から、再利用しやすい接触モデルと空間マスク処理だけを切り出しています。PDE 更新本体から分離して単体でテストでき、2D バッファ表現は `neco-array2` に分けているため、配列保持のためだけに `neco-gridfield` へ依存しません。

## API

| 項目 | 説明 |
|------|------|
| `find_nearest(x, y, tx, ty)` | 目標点に最も近い格子セルを返す |
| `build_spatial_mask(x, y, hx, hy, width, interior)` | 正規化余弦テーパー関数でマスクを作る |
| `collect_interior(interior, margin)` | 境界から `margin` セル以上離れた内部セルを集める |
| `HertzContact::new(...)` | 単純な Hertz ビーター模型を構築 |
| `HertzContact::step(w_surface, dt)` | 1 ステップ進めて接触力を返す |
| `HertzContact::energy()` | 現在のビータ運動エネルギーを返す |
| `HertzContact::contact_ended()` | 反発により接触が終わったかを返す |
| `HertzContact::set_contact_ended(ended)` | 接触終了フラグを上書きする |

## 前提条件

- `build_spatial_mask` は非ゼロマスクを和 1 に正規化する
- `collect_interior` は与えられたブールマスクと margin だけを使い、幾何の推定はしない
- `HertzContact` はソルバーの復元点処理を簡単にするため、ビータの位置と速度を公開状態として保持する

## ライセンス

MIT
