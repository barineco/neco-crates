# neco-color

[日本語](README-ja.md)

Color conversion utilities for graphics pipelines, covering sRGB gamma transfer, HSL conversion, and color-temperature-based white balance.

A Japanese mathematical note is available in [MATH-ja.md](MATH-ja.md).

## Color operations

The crate handles sRGB gamma conversion (exact and LUT-accelerated), HSL round-trips for color manipulation, and correlated color temperature (`CCT`) to white-balance transforms. All channel values stay in floating point, making it easy to slot into rendering, image processing, and preprocessing pipelines.

## Usage

### Gamma conversion

```rust
use neco_color::{srgb_to_linear, linear_to_srgb, to_u8};

let linear = srgb_to_linear(0.5);
let srgb = linear_to_srgb(linear);
let byte = to_u8(srgb);
```

### LUT-accelerated conversion

```rust
use neco_color::{srgb_to_linear_lut, linear_to_srgb_lut};

let linear = srgb_to_linear_lut(0.5);
let srgb = linear_to_srgb_lut(linear);
```

### HSL and white balance

```rust
use neco_color::{build_wb_matrix, cct_to_xy, hsl_to_srgb, srgb_to_hsl};

let (h, s, l) = srgb_to_hsl(1.0, 0.0, 0.0);
let (r, g, b) = hsl_to_srgb(h, s, l);
let xy = cct_to_xy(5600.0);
let wb = build_wb_matrix(6500.0, 0.0);
# let _ = (r, g, b, xy, wb);
```

## API

| Item | Description |
|------|-------------|
| `srgb_to_linear` / `linear_to_srgb` | Exact IEC 61966-2-1 transfer functions |
| `srgb_to_linear_lut` / `linear_to_srgb_lut` | LUT-accelerated transfer functions |
| `to_u8` | Clamp `[0, 1]` and convert to byte |
| `srgb_to_hsl` / `hsl_to_srgb` | Convert between sRGB and HSL |
| `cct_to_xy` | Convert correlated color temperature to CIE xy |
| `build_wb_matrix` | Build a 3x3 white-balance matrix toward D65 |

## License

MIT
