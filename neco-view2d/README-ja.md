# neco-view2d

[English](README.md)

world 座標と canvas 座標を相互変換する軽量な 2D カメラ / ビューポート変換です。画像ビューアや 2D エディタのように、パンとズームで平面を操作する用途に向きます。

## 座標変換

**world 空間** と **canvas 空間**（画面 / CSS ピクセル）を相互変換します。

`View2d` は `center_x`, `center_y`, `view_size` を保持し、`view_size` は canvas 全高に対応する world 空間の高さを表します。

- `canvas_to_world(cx, cy, canvas_width, canvas_height)` は canvas 点を world 座標へ変換します。
- `world_to_canvas(wx, wy, canvas_width, canvas_height)` は world 点を canvas 座標へ変換します。
- `pan(dx, dy, canvas_height)` は `view_size / canvas_height` を使ってピクセル差分を world 空間の移動量へ変換します。
- `zoom_at(delta, canvas_x, canvas_y, canvas_width, canvas_height)` はカーソル下の world 点を固定したまま `view_size` を変更します。

`view_size` は常に正値に保たれます。値が小さいほどズームインです。

## 使い方

```rust
use neco_view2d::View2d;

let mut view = View2d::default();

// 高さ 600px の canvas 上でピクセル差分だけパンする
view.pan(50.0, 30.0, 600.0);

// canvas 中心を基点にズームする
view.zoom_at(120.0, 400.0, 300.0, 800.0, 600.0);

// 座標変換
let (cx, cy) = view.world_to_canvas(100.0, 200.0, 800.0, 600.0);
let (wx, wy) = view.canvas_to_world(cx, cy, 800.0, 600.0);
```

## API

| 項目 | 説明 |
|------|-------------|
| `View2d::default()` | 中心 `(0, 0)`、`view_size = 1.0` の既定ビュー |
| `set(center_x, center_y, view_size)` | ビュー中心と可視 world 高さを設定する |
| `pan(dx, dy, canvas_height)` | ピクセル差分でビューを移動する |
| `zoom_at(delta, canvas_x, canvas_y, canvas_width, canvas_height)` | canvas 上の点を固定したままズームする |
| `world_to_canvas(wx, wy, canvas_width, canvas_height) -> (f64, f64)` | world 座標を canvas 座標へ変換する |
| `canvas_to_world(cx, cy, canvas_width, canvas_height) -> (f64, f64)` | canvas 座標を world 座標へ変換する |
| `fit(world_width, world_height, canvas_width, canvas_height)` | world 空間矩形全体を canvas に収める |
| `zoom_factor(reference_view_size)` | 基準 `view_size` に対するズーム率を返す |

### オプション機能

| 項目 | 説明 |
|---------|-------------|
| `serde` | `View2d` に `Serialize` / `Deserialize` を有効化する |

## ライセンス

MIT
