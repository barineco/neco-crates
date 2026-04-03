# neco-nurbs

[English](README.md)

CAD 境界、プロファイル形状、正確で滑らかな曲線表現に使う 2D / 3D 有理 NURBS 曲線・曲面・領域ライブラリです。

詳細な数理背景は [MATH-ja.md](MATH-ja.md) を参照してください。

## 曲線と領域の操作

`NurbsCurve2D` は重み付き B-spline 曲線を表し、検証、評価、サンプリング、ノット挿入、Bezier 分解、分割を行える一方、`circle` で円を近似ではなく正確な有理曲線として扱えます。

`NurbsRegion` は区分的閉曲線を外側境界と穴でまとめる型です。2D CAD 領域やメッシュ生成、B-Rep 生成の入力に向いています。

## 使い方

### 曲線の構築と評価

```rust
use neco_nurbs::NurbsCurve2D;

let curve = NurbsCurve2D::new(
    2,
    vec![[0.0, 0.0], [1.0, 2.0], [3.0, 2.0], [4.0, 0.0]],
    vec![0.0, 0.0, 0.0, 0.5, 1.0, 1.0, 1.0],
);
curve.validate().unwrap();

let point = curve.evaluate(0.5);
```

### 曲線のサンプリングと分割

```rust
let uniform = curve.sample(100);
let adaptive = curve.adaptive_sample(0.1);
let refined = curve.insert_knot(0.25);
let (left, right) = refined.split_at(0.5);
# let _ = (uniform, adaptive, left, right);
```

### 領域生成

```rust
use neco_nurbs::{NurbsCurve2D, NurbsRegion};

let region = NurbsRegion {
    outer: vec![NurbsCurve2D::circle([0.0, 0.0], 5.0)],
    holes: vec![vec![NurbsCurve2D::circle([1.0, 0.0], 1.0)]],
};

let points = region.outer_adaptive_sample(0.2);
# let _ = points;
```

## API

| 項目 | 説明 |
|------|-------------|
| `NurbsCurve2D` | 2D 有理 NURBS 曲線 |
| `NurbsCurve3D` / `NurbsSurface3D` | 3D 有理 NURBS 曲線と曲面 |
| `NurbsCurve2D::new` / `new_rational` | 非有理または有理曲線を構築する |
| `NurbsCurve2D::circle` | 正確な円を作る |
| `validate` / `evaluate` | 整合性検証と一点評価 |
| `sample` / `adaptive_sample` / `evaluate_samples` | 一様評価、適応評価、まとめて評価 |
| `insert_knot` / `split_at` / `split_at_params` | 曲線の細分化と分割 |
| `to_bezier_spans` / `reverse` / `bounding_box` / `is_closed` | 曲線ユーティリティ |
| `NurbsRegion` | 外側境界と穴を持つ 2D 領域 |
| `NurbsRegion::outer_adaptive_sample` | 下流処理向けに境界をサンプルする |
| `fit_nurbs_curve(...)` | nalgebra ベースの曲線あてはめ補助（`fitting`） |

### オプション機能

| 項目 | 説明 |
|---------|-------------|
| `polynomial-highorder` | nalgebra ベースの高次多項式補助を有効化する |
| `fitting` | nalgebra ベースの曲線あてはめ補助を有効化する |

## ライセンス

MIT
