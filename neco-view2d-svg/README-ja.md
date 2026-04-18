# neco-view2d-svg

[English](README.md)

`neco-view2d` のワールド座標を SVG 属性文字列へ変換する補助 crate です。`View2d` から `transform`、`points`、`d` に入る属性文字列を生成します。DOM や SVG ライブラリには依存しません。

## 使い方

```rust
use neco_view2d::View2d;
use neco_view2d_svg::world_transform_attr;

let transform = world_transform_attr(&View2d::default(), 800.0, 600.0);
```

```rust
use neco_view2d::View2d;
use neco_view2d_svg::world_points_to_polyline;

let points = world_points_to_polyline(&View2d::default(), &[(0.0, 0.0), (1.0, 1.0)], 800.0, 600.0);
```

```rust
use neco_view2d::View2d;
use neco_view2d_svg::world_points_to_svg_d;

let path_d = world_points_to_svg_d(&View2d::default(), &[(0.0, 0.0), (1.0, 1.0)], 800.0, 600.0);
```

## API

| 項目 | 説明 |
|------|------|
| `world_transform_attr(view, canvas_w, canvas_h)` | ワールド座標の `<g>` に使う `translate(tx,ty) scale(sx,sy)` 文字列を返す |
| `world_points_to_polyline(view, points, canvas_w, canvas_h)` | `<polyline points="...">` 用の属性文字列を返す |
| `world_points_to_svg_d(view, points, canvas_w, canvas_h)` | `<path d="...">` 用の属性文字列を返す |

公開関数は有限の浮動小数入力を前提とします。非有限値が入ると `NaN` や `inf` がそのまま出力されることがあります。

## 関連

- [`neco-view2d`](https://docs.rs/neco-view2d)

## ライセンス

MIT
