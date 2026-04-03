# neco-pigment

[English](README.md)

RGB 補間ではなく、Kubelka-Munk スペクトルにもとづいて絵具色を混ぜる顔料混色ライブラリです。

詳細な数理背景は [MATH-ja.md](MATH-ja.md) を参照してください。

## 混色モデル

neco-pigment は sRGB 色を 41 点で標本化したスペクトル表現へ変換し、K/S 空間で混合して選んだ光源で表示 RGB に戻します。減法混色では、青と黄を混ぜても線形 RGB 補間のような灰色にはならず、緑に近い結果になります。繰り返し使う場合でも `Pigment` が計算コストの高い sRGB からスペクトルへの変換結果を保持し、K/S スペクトルを再利用できます。

## 使い方

### 2 色混色

```rust
use neco_pigment::{rgb_to_ks, ks_mix, ks_to_srgb, illuminant_d65};

let blue = rgb_to_ks(0.0, 0.0, 1.0).unwrap();
let yellow = rgb_to_ks(1.0, 1.0, 0.0).unwrap();

let mixed = ks_mix(&blue, &yellow, 0.5);
let rgb = ks_to_srgb(&mixed, &illuminant_d65());
```

### 重み付き複数色混色

```rust
use neco_pigment::{rgb_to_ks, ks_mix_weighted, ks_to_srgb, illuminant_d65};

let red = rgb_to_ks(1.0, 0.0, 0.0).unwrap();
let white = rgb_to_ks(1.0, 1.0, 1.0).unwrap();

let mixed = ks_mix_weighted(&[(&red, 0.3), (&white, 0.7)]);
let rgb = ks_to_srgb(&mixed, &illuminant_d65());
# let _ = rgb;
```

### 顔料キャッシュ再利用混色

```rust
use neco_pigment::{Pigment, ks_mix, ks_to_srgb, illuminant_d65};

let blue = Pigment::from_srgb(0.0, 0.0, 1.0).unwrap();
let yellow = Pigment::from_srgb(1.0, 1.0, 0.0).unwrap();

let mixed = ks_mix(&blue.ks, &yellow.ks, 0.5);
let rgb = ks_to_srgb(&mixed, illuminant_d65());
# let _ = rgb;
```

## API

| 項目 | 説明 |
|------|-------------|
| `Pigment` | 係数と K/S スペクトルを保持する再利用向け顔料 |
| `Pigment::from_srgb(r, g, b)` | sRGB 色からキャッシュ済み顔料を作る |
| `Pigment::spectrum()` | キャッシュ済み係数から反射率を復元する |
| `KsSpectrum` | 41 サンプルの K/S スペクトル |
| `SigmoidCoeffs` | シグモイド持ち上げモデルの係数 |
| `RgbTransform` | 光源ごとの前計算済み RGB 変換 |
| `rgb_to_ks(r, g, b)` | sRGB を K/S 空間へ変換する |
| `ks_mix(a, b, t)` / `ks_mix_weighted(colors)` | K/S 空間でスペクトルを混合する |
| `ks_to_srgb(ks, transform)` | 混合後の K/S スペクトルを sRGB に戻す |
| `illuminant_d65()` / `illuminant_d50()` / `illuminant_a()` / `illuminant_e()` | 組み込み光源変換 |

### オプション機能

| 項目 | 説明 |
|---------|-------------|
| `serde` | シリアライズ可能な顔料型に `Serialize` / `Deserialize` を有効化する |

## ライセンス

MIT
