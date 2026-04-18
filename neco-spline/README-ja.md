# neco-spline

[English](README.md)

外部依存なしの自然三次スプライン補間器です。データ点を通る滑らかな曲線を構築します。

詳細な数理背景は [MATH-ja.md](MATH-ja.md) を参照してください。

## 自然三次スプライン

データ点 $(x_0, y_0), \dots, (x_n, y_n)$ に対し、各点を通り $C^2$ 連続な区分的三次多項式を構築します。各区間での多項式は次のとおりです。

$$S_i(x) = a_i + b_i(x - x_i) + c_i(x - x_i)^2 + d_i(x - x_i)^3$$

自然境界条件（両端点で $S'' = 0$）により三重対角系が得られ、Thomas アルゴリズムで $O(n)$ で解けます。補間スプラインの中で全曲率 $\int |f''(x)|^2 \, dx$ を最小化する唯一の補間関数です。

## 使い方

```toml
[dependencies]
neco-spline = "0.2"
```

### 基本的な補間

```rust
use neco_spline::CubicSpline;

let spline = CubicSpline::new(&[
    (0.0, 0.0),
    (0.25, 0.4),
    (0.75, 0.9),
    (1.0, 1.0),
]).unwrap();

let y = spline.evaluate(0.5);
```

### 境界クランプ

データ範囲外では最近傍の端点値を返します。

```rust
let spline = CubicSpline::new(&[(0.0, 1.0), (1.0, 2.0)]).unwrap();

assert_eq!(spline.evaluate(-1.0), 1.0); // 左端点でクランプ
assert_eq!(spline.evaluate(5.0),  2.0); // 右端点でクランプ
```

### エラー処理

```rust
use neco_spline::{CubicSpline, SplineError};

// 2 点未満
let err = CubicSpline::new(&[(0.0, 0.0)]);
assert!(matches!(err, Err(SplineError::InsufficientPoints)));

// x が昇順でない
let err = CubicSpline::new(&[(1.0, 0.0), (0.5, 1.0)]);
assert!(matches!(err, Err(SplineError::NonAscendingX)));
```

## API

| 項目 | 説明 |
|------|------|
| `CubicSpline` | 自然三次スプライン補間器 |
| `CubicSpline::new(points)` | `&[(f32, f32)]` からスプラインを構築 (`Result` を返す) |
| `CubicSpline::evaluate(x)` | 指定した $x$ でスプラインを評価する |
| `CubicSpline::to_bezier_segments()` | スプラインを 3 次 Bezier セグメント列へ変換する |
| `BezierCubic` | 4 つの制御点を持つ 3 次 Bezier セグメント |
| `SplineError::InsufficientPoints` | 制御点が 2 点未満 |
| `SplineError::NonAscendingX` | 制御点の $x$ が狭義昇順でない |

## ライセンス

MIT
