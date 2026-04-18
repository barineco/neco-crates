# neco-edge-routing

[English](README.md)

2 つの端点と接線方向からエッジ経路を計算する、描画非依存の 2D ルーティングプリミティブです。描画エンジンや UI フレームワーク、ノードグラフのデータモデルには依存せず、SVG や canvas など後段の描画処理が消費できる中立な `PathData` を返します。

## Feature

| Feature | 説明 |
|---------|------|
| `default` | 追加依存なし。`Linear`、`Bezier`、`Orthogonal` を提供 |
| `spline` | `neco-spline` 経由で `RouteStyle::Spline` を有効化し、3 次 Bezier セグメントを返す |
| `nurbs` | `neco-nurbs` 経由で `RouteStyle::Nurbs` を有効化し、NURBS 制御データを返す |

`spline` / `nurbs` を有効にしなくても対応する `RouteStyle` バリアントは常に存在し、`route()` はサイレントに別スタイルへ落とさず `RoutingError::FeatureDisabled` を返します。

## 使い方

```rust
use neco_edge_routing::{route, RouteRequest, RouteStyle};

let path = route(&RouteRequest {
    from: (0.0, 0.0),
    to: (120.0, 40.0),
    from_tangent: (1.0, 0.0),
    to_tangent: (-1.0, 0.0),
    style: RouteStyle::Linear,
})?;

assert_eq!(path.points, vec![(0.0, 0.0), (120.0, 40.0)]);
# Ok::<(), neco_edge_routing::RoutingError>(())
```

```rust
use neco_edge_routing::{route, RouteRequest, RouteStyle};

let path = route(&RouteRequest {
    from: (0.0, 0.0),
    to: (120.0, 40.0),
    from_tangent: (1.0, 0.0),
    to_tangent: (-1.0, 0.0),
    style: RouteStyle::Bezier { curvature: 0.25 },
})?;

assert_eq!(path.points.len(), 4);
# Ok::<(), neco_edge_routing::RoutingError>(())
```

```rust
use neco_edge_routing::{route, RouteRequest, RouteStyle};

let path = route(&RouteRequest {
    from: (0.0, 0.0),
    to: (120.0, 40.0),
    from_tangent: (1.0, 0.0),
    to_tangent: (-1.0, 0.0),
    style: RouteStyle::Orthogonal { corner_radius: 8.0 },
})?;

assert!(!path.points.is_empty());
# Ok::<(), neco_edge_routing::RoutingError>(())
```

Spline と NURBS の経路は対応する Cargo feature を有効にすると利用できます。

## API

| 項目 | 説明 |
|------|------|
| `RouteStyle` | `Linear`、`Bezier`、`Orthogonal`、`Spline`、`Nurbs` のルーティング戦略 |
| `RouteRequest` | 端点、接線方向、要求するスタイルをまとめた入力 |
| `PathData` | ルーティング結果の制御点列と `PathKind` |
| `PathKind` | `Polyline`、`Cubic`、`Quadratic`、`Nurbs { knots, weights }` を表す出力種別 |
| `route(&RouteRequest)` | `Result<PathData, RoutingError>` を返すルーティング関数 |
| `RoutingError` | 無効な入力、または feature 未有効のスタイル指定時に返すエラー型 |

## ライセンス

MIT
