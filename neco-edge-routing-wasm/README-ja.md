# neco-edge-routing-wasm

[English](README.md)

[neco-edge-routing](../neco-edge-routing) を JavaScript から使うための minimum dependency WebAssembly バインディングです。`wasm-bindgen` 経由でエッジ経路の計算を呼べます。

## ルーティング機能

`route_edge` は 2 点間のエッジ経路を計算し、`style`、`kind`、`points` 配列を持つ JavaScript オブジェクトを返します。NURBS の場合は `knots` と `weights` も付加されます。

対応するスタイル名:

- `bezier`: 接線スケールハンドル付きの 3 次ベジエ経路
- `orthogonal`: 角丸付きの軸並行経路
- `spline`: 自然 3 次スプライン経路
- `nurbs`: NURBS 制御点列

## 使い方

まず `wasm-pack build --target web` でビルドし、生成されたパッケージを JavaScript または TypeScript からインポートします。

```ts
import init, { route_edge } from "./pkg/neco_edge_routing_wasm.js";

await init();

const path = route_edge("bezier", 0, 0, 100, 50);
// path: { style: "bezier", kind: "cubic", points: [{x, y}, ...] }

const nurbs = route_edge("nurbs", 0, 0, 100, 50);
// nurbs: { style: "nurbs", kind: "nurbs", points, knots, weights }
```

未対応のスタイル名や非有限入力に対してはエラーが投げられ、原因メッセージが含まれます。

## API

| 項目 | 説明 |
|------|-------------|
| `route_edge(style, from_x, from_y, to_x, to_y)` | 2 点間のエッジ経路計算。`style`、`kind`、`points`、NURBS の場合は追加メタデータを含むオブジェクトを返す |

## ライセンス

MIT ライセンスのもとで配布しています。詳細は [LICENSE](LICENSE) を参照してください。
