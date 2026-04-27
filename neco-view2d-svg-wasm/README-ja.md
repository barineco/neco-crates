# neco-view2d-svg-wasm

[English](README.md)

minimum dependency WebAssembly bindings for neco-view2d-svg via wasm-bindgen

## 概要

[neco-view2d-svg](../neco-view2d-svg) を JavaScript から使うための WebAssembly バインディングです。`wasm-bindgen` 経由で、`neco-view2d` のビュー状態とワールド座標から SVG 属性値の文字列を生成できます。

## 機能説明

公開関数は `emit_transform`, `emit_polyline`, `emit_path` の 3 つです。いずれもビュー中心とビューサイズ、キャンバス寸法からワールド座標をキャンバス座標に射影し、SVG 属性として直接埋め込める文字列を返します。

`emit_transform` は SVG ルートまたはグループ要素に与える `transform` 属性値を生成します。`emit_polyline` は折れ線の `points` 属性値、`emit_path` はパスの `d` 属性値を、いずれも `[x0, y0, x1, y1, ...]` 形式のフラット配列から生成します。

## 使い方

`wasm-pack build --target web` でビルドし、生成されたパッケージを JavaScript からインポートします。

```js
import init, { emit_transform, emit_polyline, emit_path } from "./pkg/neco_view2d_svg_wasm.js";

await init();

const cx = 0;
const cy = 0;
const vs = 10;
const cw = 800;
const ch = 600;

// SVG transform 属性値
const transform = emit_transform(cx, cy, vs, cw, ch);

// 折れ線の points 属性値
const points = new Float64Array([0, 0, 1, 1, 2, 0]);
const polyline = emit_polyline(cx, cy, vs, points, cw, ch);

// path の d 属性値
const d = emit_path(cx, cy, vs, points, cw, ch);
```

## API

| 項目 | 説明 |
|------|------|
| `emit_transform(center_x, center_y, view_size, canvas_w, canvas_h)` | ビュー状態とキャンバス寸法から SVG `transform` 属性値の文字列を生成 |
| `emit_polyline(center_x, center_y, view_size, points, canvas_w, canvas_h)` | ワールド座標のフラット配列 `[x0, y0, x1, y1, ...]` から `polyline` の `points` 属性値を生成 |
| `emit_path(center_x, center_y, view_size, points, canvas_w, canvas_h)` | ワールド座標のフラット配列 `[x0, y0, x1, y1, ...]` から `path` の `d` 属性値を生成 |

## ライセンス

MIT License で配布されます。詳細は [LICENSE](LICENSE) を参照してください。
