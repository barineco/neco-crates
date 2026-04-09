# neco-color

[English](README.md)

グラフィクス処理向けの色変換ユーティリティです。sRGB のガンマ変換、HSL 変換、色温度にもとづくホワイトバランスをまとめています。

詳細な数理背景は [MATH-ja.md](MATH-ja.md) を参照してください。

## 色操作

sRGB のガンマ変換は厳密式と LUT 高速化の両方を用意し、加えて HSL の往復変換と相関色温度 (`CCT`) から作るホワイトバランス行列も提供しています。チャネル値は浮動小数点のまま扱うので、レンダリングや画像処理の前段にそのまま組み込みやすい構成です。

## 使い方

### ガンマ変換

```rust
use neco_color::{srgb_to_linear, linear_to_srgb, to_u8};

let linear = srgb_to_linear(0.5);
let srgb = linear_to_srgb(linear);
let byte = to_u8(srgb);
```

### LUT 高速化

```rust
use neco_color::{srgb_to_linear_lut, linear_to_srgb_lut};

let linear = srgb_to_linear_lut(0.5);
let srgb = linear_to_srgb_lut(linear);
```

### HSL とホワイトバランス

```rust
use neco_color::{build_wb_matrix, cct_to_xy, hsl_to_srgb, srgb_to_hsl};

let (h, s, l) = srgb_to_hsl(1.0, 0.0, 0.0);
let (r, g, b) = hsl_to_srgb(h, s, l);
let xy = cct_to_xy(5600.0);
let wb = build_wb_matrix(6500.0, 0.0);
# let _ = (r, g, b, xy, wb);
```

## API

| 項目 | 説明 |
|------|-------------|
| `srgb_to_linear` / `linear_to_srgb` | IEC 61966-2-1 の厳密な伝達関数 |
| `srgb_to_linear_lut` / `linear_to_srgb_lut` | LUT 高速化版の伝達関数 |
| `to_u8` | `[0, 1]` にクランプしてバイトへ変換する |
| `srgb_to_hsl` / `hsl_to_srgb` | sRGB と HSL を相互変換する |
| `cct_to_xy` | 相関色温度を CIE xy に変換する |
| `build_wb_matrix` | D65 へ寄せる 3x3 ホワイトバランス行列を作る |

## ライセンス

MIT
