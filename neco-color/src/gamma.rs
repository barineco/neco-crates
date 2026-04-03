use std::sync::OnceLock;

const GAMMA_LUT_SIZE: usize = 4096;

pub fn srgb_to_linear(c: f32) -> f32 {
    if c <= 0.04045 {
        c / 12.92
    } else {
        ((c + 0.055) / 1.055).powf(2.4)
    }
}

pub fn linear_to_srgb(c: f32) -> f32 {
    if c <= 0.0031308 {
        c * 12.92
    } else {
        1.055 * c.powf(1.0 / 2.4) - 0.055
    }
}

static SRGB_TO_LINEAR_LUT: OnceLock<[f32; GAMMA_LUT_SIZE]> = OnceLock::new();
static LINEAR_TO_SRGB_LUT: OnceLock<[f32; GAMMA_LUT_SIZE]> = OnceLock::new();

fn build_srgb_to_linear_lut() -> [f32; GAMMA_LUT_SIZE] {
    let mut lut = [0.0f32; GAMMA_LUT_SIZE];
    for (i, item) in lut.iter_mut().enumerate() {
        let x = i as f32 / (GAMMA_LUT_SIZE - 1) as f32;
        *item = srgb_to_linear(x);
    }
    lut
}

fn build_linear_to_srgb_lut() -> [f32; GAMMA_LUT_SIZE] {
    let mut lut = [0.0f32; GAMMA_LUT_SIZE];
    for (i, item) in lut.iter_mut().enumerate() {
        let x = i as f32 / (GAMMA_LUT_SIZE - 1) as f32;
        *item = linear_to_srgb(x);
    }
    lut
}

/// sRGB to linear conversion using LUT with linear interpolation.
pub fn srgb_to_linear_lut(c: f32) -> f32 {
    let lut = SRGB_TO_LINEAR_LUT.get_or_init(build_srgb_to_linear_lut);
    gamma_lut_lookup(lut, c)
}

/// Linear to sRGB conversion using LUT with linear interpolation.
pub fn linear_to_srgb_lut(c: f32) -> f32 {
    let lut = LINEAR_TO_SRGB_LUT.get_or_init(build_linear_to_srgb_lut);
    gamma_lut_lookup(lut, c)
}

fn gamma_lut_lookup(lut: &[f32; GAMMA_LUT_SIZE], value: f32) -> f32 {
    let idx = (value * (GAMMA_LUT_SIZE - 1) as f32).clamp(0.0, (GAMMA_LUT_SIZE - 1) as f32);
    let lo = idx as usize;
    let hi = (lo + 1).min(GAMMA_LUT_SIZE - 1);
    let frac = idx - lo as f32;
    lut[lo] * (1.0 - frac) + lut[hi] * frac
}

pub fn to_u8(v: f32) -> u8 {
    let rounded = (v.clamp(0.0, 1.0) * 255.0 + 0.5).floor();
    rounded.clamp(0.0, 255.0) as u8
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trip() {
        for &x in &[0.0f32, 0.5, 1.0] {
            let rt = srgb_to_linear(linear_to_srgb(x));
            assert!((rt - x).abs() < 1e-5, "round-trip failed for {x}: got {rt}");
        }
    }

    #[test]
    fn lut_matches_formula() {
        for i in 0..=10 {
            let x = i as f32 / 10.0;
            let formula = srgb_to_linear(x);
            let lut = srgb_to_linear_lut(x);
            assert!(
                (formula - lut).abs() < 1e-3,
                "srgb_to_linear mismatch at {x}: formula={formula}, lut={lut}"
            );

            let formula = linear_to_srgb(x);
            let lut = linear_to_srgb_lut(x);
            assert!(
                (formula - lut).abs() < 1e-3,
                "linear_to_srgb mismatch at {x}: formula={formula}, lut={lut}"
            );
        }
    }

    #[test]
    fn to_u8_boundary() {
        assert_eq!(to_u8(0.0), 0);
        assert_eq!(to_u8(1.0), 255);
        assert_eq!(to_u8(-0.5), 0);
        assert_eq!(to_u8(1.5), 255);
    }
}
