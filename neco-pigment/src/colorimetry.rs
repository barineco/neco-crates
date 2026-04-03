use std::sync::OnceLock;

use crate::illuminant::*;

static D65_TRANSFORM: OnceLock<RgbTransform> = OnceLock::new();
static D50_TRANSFORM: OnceLock<RgbTransform> = OnceLock::new();
static A_TRANSFORM: OnceLock<RgbTransform> = OnceLock::new();
static E_TRANSFORM: OnceLock<RgbTransform> = OnceLock::new();

/// Precomputed spectral-to-linear-RGB transform matrix for a given illuminant.
///
/// 3 x N_SPECTRAL row-major matrix: [R_row, G_row, B_row].
#[derive(Clone)]
pub struct RgbTransform {
    pub(crate) a_rgb: [f32; 3 * N_SPECTRAL],
}

// Manual serde impl: derive not available for [f32; N] where N > 32.
#[cfg(feature = "serde")]
impl serde::Serialize for RgbTransform {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        self.a_rgb.as_slice().serialize(serializer)
    }
}

#[cfg(feature = "serde")]
impl<'de> serde::Deserialize<'de> for RgbTransform {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let v: Vec<f32> = Vec::deserialize(deserializer)?;
        let a_rgb: [f32; 3 * N_SPECTRAL] = v.try_into().map_err(|_| {
            serde::de::Error::custom(format!("expected array length {}", 3 * N_SPECTRAL))
        })?;
        Ok(RgbTransform { a_rgb })
    }
}

/// XYZ to linear sRGB matrix (D65 white point, IEC 61966-2-1).
const XYZ_TO_SRGB: [[f64; 3]; 3] = [
    [
        3.2404541621141054,
        -1.5371385940306089,
        -0.49853140955601579,
    ],
    [
        -0.969_266_030_505_183_1,
        1.8760108454466942,
        0.041556017530349834,
    ],
    [
        0.055643430959114726,
        -0.203_976_959_873_057_3,
        1.0572251882231791,
    ],
];

/// Build the 3 x N_SPECTRAL RGB transform matrix from CMF and illuminant SPD.
fn build_transform(illuminant_spd: &[f32; N_SPECTRAL]) -> RgbTransform {
    // Y normalization: k = 1 / sum(y_bar * S)
    let mut sum_y_s = 0.0f64;
    for i in 0..N_SPECTRAL {
        sum_y_s += CMF_Y[i] as f64 * illuminant_spd[i] as f64;
    }
    let k = 1.0 / sum_y_s;

    // Per-wavelength XYZ contribution, then XYZ-to-RGB transform
    let mut a_rgb = [0.0f32; 3 * N_SPECTRAL];
    for i in 0..N_SPECTRAL {
        let s = illuminant_spd[i] as f64;
        let x = CMF_X[i] as f64 * s * k;
        let y = CMF_Y[i] as f64 * s * k;
        let z = CMF_Z[i] as f64 * s * k;

        // XYZ to linear sRGB
        let r = XYZ_TO_SRGB[0][0] * x + XYZ_TO_SRGB[0][1] * y + XYZ_TO_SRGB[0][2] * z;
        let g = XYZ_TO_SRGB[1][0] * x + XYZ_TO_SRGB[1][1] * y + XYZ_TO_SRGB[1][2] * z;
        let b = XYZ_TO_SRGB[2][0] * x + XYZ_TO_SRGB[2][1] * y + XYZ_TO_SRGB[2][2] * z;

        a_rgb[i] = r as f32;
        a_rgb[N_SPECTRAL + i] = g as f32;
        a_rgb[2 * N_SPECTRAL + i] = b as f32;
    }

    RgbTransform { a_rgb }
}

/// Convert a reflectance spectrum to linear sRGB via dot product with the precomputed matrix.
pub fn spectrum_to_linear_rgb(refl: &[f32; N_SPECTRAL], transform: &RgbTransform) -> [f32; 3] {
    let mut rgb = [0.0f32; 3];
    for (c, rgb_ch) in rgb.iter_mut().enumerate() {
        let offset = c * N_SPECTRAL;
        let mut sum = 0.0f32;
        for (i, &refl_i) in refl.iter().enumerate().take(N_SPECTRAL) {
            sum += transform.a_rgb[offset + i] * refl_i;
        }
        *rgb_ch = sum;
    }
    rgb
}

/// Return the cached RGB transform matrix for illuminant D65.
pub fn illuminant_d65() -> &'static RgbTransform {
    D65_TRANSFORM.get_or_init(|| build_transform(&ILLUMINANT_D65))
}

/// Return the cached RGB transform matrix for illuminant D50.
pub fn illuminant_d50() -> &'static RgbTransform {
    D50_TRANSFORM.get_or_init(|| build_transform(&ILLUMINANT_D50))
}

/// Return the cached RGB transform matrix for illuminant A.
pub fn illuminant_a() -> &'static RgbTransform {
    A_TRANSFORM.get_or_init(|| build_transform(&ILLUMINANT_A))
}

/// Return the cached RGB transform matrix for equal-energy illuminant E.
pub fn illuminant_e() -> &'static RgbTransform {
    E_TRANSFORM.get_or_init(|| build_transform(&ILLUMINANT_E))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn d65_flat_spectrum_gives_white() {
        let transform = illuminant_d65();
        let refl = [1.0f32; N_SPECTRAL];
        let rgb = spectrum_to_linear_rgb(&refl, transform);
        for (c, &value) in rgb.iter().enumerate() {
            assert!(
                (value - 1.0).abs() < 0.05,
                "ch {c}: expected ~1.0, got {}",
                value
            );
        }
    }

    #[test]
    fn zero_spectrum_gives_black() {
        let transform = illuminant_d65();
        let refl = [0.0f32; N_SPECTRAL];
        let rgb = spectrum_to_linear_rgb(&refl, transform);
        for &value in &rgb {
            assert!(value.abs() < 1e-6);
        }
    }

    #[test]
    fn illuminant_cache_returns_same_instance() {
        assert!(std::ptr::eq(illuminant_d65(), illuminant_d65()));
        assert!(std::ptr::eq(illuminant_d50(), illuminant_d50()));
        assert!(std::ptr::eq(illuminant_a(), illuminant_a()));
        assert!(std::ptr::eq(illuminant_e(), illuminant_e()));
    }
}
