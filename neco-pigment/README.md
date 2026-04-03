# neco-pigment

[日本語](README-ja.md)

Physically based paint-color mixing using Kubelka-Munk spectra instead of RGB interpolation.

A Japanese mathematical note is available in [MATH-ja.md](MATH-ja.md).

## Mixing model

neco-pigment converts sRGB colors into 41-sample spectral representations, mixes them in K/S space, and converts the result back to display RGB under a chosen illuminant. This models subtractive pigment mixing, so blue plus yellow produces green rather than the gray typical of linear RGB blending.

For repeated use, `Pigment` caches the expensive sRGB-to-spectrum conversion and keeps the K/S spectrum ready for later mixes.

## Usage

### Mix two colors

```rust
use neco_pigment::{rgb_to_ks, ks_mix, ks_to_srgb, illuminant_d65};

let blue = rgb_to_ks(0.0, 0.0, 1.0).unwrap();
let yellow = rgb_to_ks(1.0, 1.0, 0.0).unwrap();

let mixed = ks_mix(&blue, &yellow, 0.5);
let rgb = ks_to_srgb(&mixed, &illuminant_d65());
```

### Mix multiple colors with weights

```rust
use neco_pigment::{rgb_to_ks, ks_mix_weighted, ks_to_srgb, illuminant_d65};

let red = rgb_to_ks(1.0, 0.0, 0.0).unwrap();
let white = rgb_to_ks(1.0, 1.0, 1.0).unwrap();

let mixed = ks_mix_weighted(&[(&red, 0.3), (&white, 0.7)]);
let rgb = ks_to_srgb(&mixed, &illuminant_d65());
# let _ = rgb;
```

### Cache pigments for repeated mixing

```rust
use neco_pigment::{Pigment, ks_mix, ks_to_srgb, illuminant_d65};

let blue = Pigment::from_srgb(0.0, 0.0, 1.0).unwrap();
let yellow = Pigment::from_srgb(1.0, 1.0, 0.0).unwrap();

let mixed = ks_mix(&blue.ks, &yellow.ks, 0.5);
let rgb = ks_to_srgb(&mixed, illuminant_d65());
# let _ = rgb;
```

## API

| Item | Description |
|------|-------------|
| `Pigment` | Cached pigment representation with coefficients and K/S spectrum |
| `Pigment::from_srgb(r, g, b)` | Build a cached pigment from an sRGB color |
| `Pigment::spectrum()` | Reconstruct reflectance from cached coefficients |
| `KsSpectrum` | 41-sample K/S spectrum |
| `SigmoidCoeffs` | Coefficients for the sigmoid uplift model |
| `RgbTransform` | Precomputed illuminant-specific RGB transform |
| `rgb_to_ks(r, g, b)` | Convert sRGB to K/S space |
| `ks_mix(a, b, t)` / `ks_mix_weighted(colors)` | Mix spectra in K/S space |
| `ks_to_srgb(ks, transform)` | Convert mixed K/S spectra back to sRGB |
| `illuminant_d65()` / `illuminant_d50()` / `illuminant_a()` / `illuminant_e()` | Built-in illuminant transforms |

### Optional features

| Feature | Description |
|---------|-------------|
| `serde` | Enables `Serialize` / `Deserialize` for serializable pigment data types |

## License

MIT
