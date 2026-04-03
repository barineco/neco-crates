# neco-view2d-wasm

[English](README.md)

[neco-view2d](../neco-view2d) を JavaScript から使うための WebAssembly バインディングです。`wasm-bindgen` 経由でパン、ズーム、座標変換を呼べます。

## JavaScript バインディング

`WasmView2d` は `neco-view2d` のパン、ズーム、フィット、座標変換を薄く包んだ型です。返り値は固定長の小さな配列にそろえています。

## 使い方

まず `wasm-pack build --target web` でビルドし、生成されたパッケージを JavaScript からインポートします。

```js
import init, { WasmView2d } from "./pkg/neco_view2d_wasm.js";

await init();

const view = new WasmView2d();

// パン (dx, dy: キャンバスピクセル, canvas_height)
view.pan(50, 30, 600);

// キャンバス上の点を中心にズーム (delta, cx, cy, canvas_width, canvas_height)
view.zoom_at(100, 400, 300, 800, 600);

// 座標変換 ([x, y] を返す)
const [wx, wy] = view.canvas_to_world(400, 300, 800, 600);
const [cx, cy] = view.world_to_canvas(wx, wy, 800, 600);

// ワールド領域をキャンバスにフィット
view.fit(1920, 1080, 800, 600);

// 状態の取得・設定: [center_x, center_y, view_size]
const [centerX, centerY, viewSize] = view.get_state();
view.set_state(0, 0, 10);

// 基準 view_size に対するズーム倍率
const factor = view.zoom_factor(viewSize);
```

## API

| 項目 | 説明 |
|------|-------------|
| `new WasmView2d()` | 既定のビュー作成、中心 `(0, 0)`、`view_size 1.0` |
| `pan(dx, dy, canvas_height)` | ビューをキャンバスピクセル単位で `(dx, dy)` 平行移動 |
| `zoom_at(delta, cx, cy, cw, ch)` | キャンバス上の点 `(cx, cy)` を中心にズーム。`delta > 0` でズームイン |
| `canvas_to_world(cx, cy, cw, ch)` | キャンバス座標からワールド座標 `[wx, wy]` への変換 |
| `world_to_canvas(wx, wy, cw, ch)` | ワールド座標からキャンバス座標 `[cx, cy]` への変換 |
| `fit(ww, wh, cw, ch)` | ワールド領域を余白付きでキャンバス内に収まるよう調整 |
| `get_state()` | `[center_x, center_y, view_size]` の取得 |
| `set_state(cx, cy, vs)` | ビュー状態の直接設定 |
| `zoom_factor(ref_view_size)` | 基準ビューサイズに対する現在のズーム倍率 |

## ライセンス

MIT
