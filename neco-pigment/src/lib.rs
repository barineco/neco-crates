//! Kubelka-Munk spectral pigment mixing crate.

mod colorimetry;
mod illuminant;
mod km;
mod sigmoid;

pub use colorimetry::{
    illuminant_a, illuminant_d50, illuminant_d65, illuminant_e, spectrum_to_linear_rgb,
    RgbTransform,
};
pub use illuminant::{LAMBDAS, LAMBDA_MAX, LAMBDA_MIN, LAMBDA_STEP, N_SPECTRAL};
pub use km::{ks_mix, ks_mix_weighted, ks_to_reflectance, reflectance_to_ks, KsSpectrum};
pub use sigmoid::{rgb_to_sigmoid, sigmoid_to_spectrum, SigmoidCoeffs};

// Re-export neco-color gamma functions.
pub use neco_color::{
    linear_to_srgb, linear_to_srgb_lut, srgb_to_linear, srgb_to_linear_lut, to_u8,
};

/// Error type for neco-pigment.
#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum PigmentError {
    /// Gauss-Newton optimization did not converge.
    ConvergenceFailure {
        /// Final residual L2 norm.
        residual: f64,
    },
}

impl std::fmt::Display for PigmentError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            PigmentError::ConvergenceFailure { residual } => {
                write!(
                    f,
                    "Gauss-Newton optimization did not converge (residual: {residual:.6e})"
                )
            }
        }
    }
}

impl std::error::Error for PigmentError {}

const BLACK_THRESHOLD: f32 = 1e-6;
const WHITE_THRESHOLD: f32 = 1.0 - 1e-6;

/// Bundled pigment holding the original sRGB, sigmoid coefficients, and K/S spectrum.
#[derive(Clone, Debug)]
pub struct Pigment {
    pub srgb: [f32; 3],
    pub coeffs: SigmoidCoeffs,
    pub ks: KsSpectrum,
}

impl Pigment {
    /// Create a pigment from sRGB components.
    pub fn from_srgb(r: f32, g: f32, b: f32) -> Result<Pigment, PigmentError> {
        let max_ch = r.max(g).max(b);
        let min_ch = r.min(g).min(b);

        if max_ch < BLACK_THRESHOLD {
            return Ok(Pigment {
                srgb: [r, g, b],
                coeffs: SigmoidCoeffs {
                    c0: 0.0,
                    c1: 0.0,
                    c2: -1e6,
                },
                ks: KsSpectrum {
                    ks: [km::KS_MAX; N_SPECTRAL],
                },
            });
        }
        if min_ch > WHITE_THRESHOLD {
            return Ok(Pigment {
                srgb: [r, g, b],
                coeffs: SigmoidCoeffs {
                    c0: 0.0,
                    c1: 0.0,
                    c2: 1e6,
                },
                ks: KsSpectrum {
                    ks: [0.0; N_SPECTRAL],
                },
            });
        }

        let coeffs = rgb_to_sigmoid(r, g, b)?;
        let refl = sigmoid_to_spectrum(&coeffs);
        let ks = reflectance_to_ks(&refl);
        Ok(Pigment {
            srgb: [r, g, b],
            coeffs,
            ks,
        })
    }

    /// Reconstruct the reflectance spectrum from sigmoid coefficients.
    pub fn spectrum(&self) -> [f32; N_SPECTRAL] {
        sigmoid_to_spectrum(&self.coeffs)
    }
}

/// Convert sRGB to K/S spectrum (handles black/white analytically).
pub fn rgb_to_ks(r: f32, g: f32, b: f32) -> Result<KsSpectrum, PigmentError> {
    let max_ch = r.max(g).max(b);
    let min_ch = r.min(g).min(b);

    if max_ch < BLACK_THRESHOLD {
        return Ok(KsSpectrum {
            ks: [km::KS_MAX; N_SPECTRAL],
        });
    }
    if min_ch > WHITE_THRESHOLD {
        return Ok(KsSpectrum {
            ks: [0.0; N_SPECTRAL],
        });
    }

    let coeffs = rgb_to_sigmoid(r, g, b)?;
    let refl = sigmoid_to_spectrum(&coeffs);
    Ok(reflectance_to_ks(&refl))
}

/// Convert K/S spectrum to sRGB (using LUT gamma).
pub fn ks_to_srgb(ks: &KsSpectrum, transform: &RgbTransform) -> [f32; 3] {
    let refl = ks_to_reflectance(ks);
    let linear = spectrum_to_linear_rgb(&refl, transform);
    [
        neco_color::linear_to_srgb_lut(linear[0]),
        neco_color::linear_to_srgb_lut(linear[1]),
        neco_color::linear_to_srgb_lut(linear[2]),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    // CIEDE2000 test utilities

    /// sRGB → XYZ (D65)
    fn srgb_to_xyz(r: f32, g: f32, b: f32) -> [f64; 3] {
        let rl = neco_color::srgb_to_linear(r) as f64;
        let gl = neco_color::srgb_to_linear(g) as f64;
        let bl = neco_color::srgb_to_linear(b) as f64;
        // sRGB to XYZ (D65)
        let x = 0.4124564 * rl + 0.3575761 * gl + 0.1804375 * bl;
        let y = 0.2126729 * rl + 0.7151522 * gl + 0.0721750 * bl;
        let z = 0.0193339 * rl + 0.1191920 * gl + 0.9503041 * bl;
        [x, y, z]
    }

    /// XYZ → Lab (D65)
    fn xyz_to_lab(xyz: [f64; 3]) -> [f64; 3] {
        // D65 white point
        let xn = 0.95047;
        let yn = 1.0;
        let zn = 1.08883;

        let f = |t: f64| -> f64 {
            if t > (6.0 / 29.0_f64).powi(3) {
                t.cbrt()
            } else {
                t / (3.0 * (6.0 / 29.0_f64).powi(2)) + 4.0 / 29.0
            }
        };

        let fx = f(xyz[0] / xn);
        let fy = f(xyz[1] / yn);
        let fz = f(xyz[2] / zn);

        let l = 116.0 * fy - 16.0;
        let a = 500.0 * (fx - fy);
        let b = 200.0 * (fy - fz);
        [l, a, b]
    }

    /// CIEDE2000 color difference.
    fn delta_e_2000(lab1: [f64; 3], lab2: [f64; 3]) -> f64 {
        let (l1, a1, b1) = (lab1[0], lab1[1], lab1[2]);
        let (l2, a2, b2) = (lab2[0], lab2[1], lab2[2]);

        let c1_star = (a1 * a1 + b1 * b1).sqrt();
        let c2_star = (a2 * a2 + b2 * b2).sqrt();
        let c_bar = (c1_star + c2_star) / 2.0;

        let c_bar_7 = c_bar.powi(7);
        let g = 0.5 * (1.0 - (c_bar_7 / (c_bar_7 + 25.0_f64.powi(7))).sqrt());

        let a1p = a1 * (1.0 + g);
        let a2p = a2 * (1.0 + g);

        let c1p = (a1p * a1p + b1 * b1).sqrt();
        let c2p = (a2p * a2p + b2 * b2).sqrt();

        let h1p = b1.atan2(a1p).to_degrees().rem_euclid(360.0);
        let h2p = b2.atan2(a2p).to_degrees().rem_euclid(360.0);

        let dl = l2 - l1;
        let dc = c2p - c1p;

        let dh_deg = if c1p * c2p == 0.0 {
            0.0
        } else if (h2p - h1p).abs() <= 180.0 {
            h2p - h1p
        } else if h2p - h1p > 180.0 {
            h2p - h1p - 360.0
        } else {
            h2p - h1p + 360.0
        };
        let dh = 2.0 * (c1p * c2p).sqrt() * (dh_deg.to_radians() / 2.0).sin();

        let l_bar = (l1 + l2) / 2.0;
        let c_bar_p = (c1p + c2p) / 2.0;

        let h_bar_p = if c1p * c2p == 0.0 {
            h1p + h2p
        } else if (h1p - h2p).abs() <= 180.0 {
            (h1p + h2p) / 2.0
        } else if h1p + h2p < 360.0 {
            (h1p + h2p + 360.0) / 2.0
        } else {
            (h1p + h2p - 360.0) / 2.0
        };

        let t = 1.0 - 0.17 * ((h_bar_p - 30.0).to_radians()).cos()
            + 0.24 * ((2.0 * h_bar_p).to_radians()).cos()
            + 0.32 * ((3.0 * h_bar_p + 6.0).to_radians()).cos()
            - 0.20 * ((4.0 * h_bar_p - 63.0).to_radians()).cos();

        let sl = 1.0 + 0.015 * (l_bar - 50.0).powi(2) / (20.0 + (l_bar - 50.0).powi(2)).sqrt();
        let sc = 1.0 + 0.045 * c_bar_p;
        let sh = 1.0 + 0.015 * c_bar_p * t;

        let c_bar_p_7 = c_bar_p.powi(7);
        let rt = -2.0
            * (c_bar_p_7 / (c_bar_p_7 + 25.0_f64.powi(7))).sqrt()
            * (60.0 * (-((h_bar_p - 275.0) / 25.0).powi(2)).exp())
                .to_radians()
                .sin();

        ((dl / sl).powi(2) + (dc / sc).powi(2) + (dh / sh).powi(2) + rt * (dc / sc) * (dh / sh))
            .sqrt()
    }

    /// Compute CIEDE2000 between two sRGB colors.
    fn srgb_delta_e(rgb1: [f32; 3], rgb2: [f32; 3]) -> f64 {
        let lab1 = xyz_to_lab(srgb_to_xyz(rgb1[0], rgb1[1], rgb1[2]));
        let lab2 = xyz_to_lab(srgb_to_xyz(rgb2[0], rgb2[1], rgb2[2]));
        delta_e_2000(lab1, lab2)
    }

    // Pipeline smoke test

    #[test]
    fn pipeline_basic() {
        let transform = illuminant_d65();
        let ks = rgb_to_ks(0.8, 0.2, 0.3).expect("pipeline should work");
        let result = ks_to_srgb(&ks, transform);
        // Output should be in valid range
        for (c, &value) in result.iter().enumerate() {
            assert!((0.0..=1.0).contains(&value), "ch {c}: {}", value);
        }
    }

    // Round-trip test (22+ colors)

    fn representative_colors() -> Vec<(f32, f32, f32)> {
        vec![
            // 6 primaries
            (1.0, 0.0, 0.0),
            (0.0, 1.0, 0.0),
            (0.0, 0.0, 1.0),
            (1.0, 1.0, 0.0),
            (1.0, 0.0, 1.0),
            (0.0, 1.0, 1.0),
            // Black & white
            (1.0, 1.0, 1.0),
            (0.0, 0.0, 0.0),
            // Grays
            (0.25, 0.25, 0.25),
            (0.5, 0.5, 0.5),
            (0.75, 0.75, 0.75),
            // 12 intermediate colors
            (1.0, 0.5, 0.0), // Orange
            (0.5, 1.0, 0.0), // Lime
            (0.0, 1.0, 0.5), // Spring green
            (0.0, 0.5, 1.0), // Azure
            (0.5, 0.0, 1.0), // Violet
            (1.0, 0.0, 0.5), // Rose
            (0.8, 0.4, 0.2), // Brown
            (0.2, 0.6, 0.4), // Teal
            (0.6, 0.2, 0.8), // Purple
            (0.9, 0.9, 0.1), // Bright yellow
            (0.1, 0.1, 0.9), // Dark blue
            (0.4, 0.7, 0.3), // Grass green
        ]
    }

    fn round_trip_stats(colors: &[(f32, f32, f32)]) -> (f64, f64) {
        let transform = illuminant_d65();
        let mut sum_de = 0.0;
        let mut max_de = 0.0f64;
        for &(r, g, b) in colors {
            let ks = rgb_to_ks(r, g, b).unwrap();
            let out = ks_to_srgb(&ks, transform);
            let de = srgb_delta_e([r, g, b], out);
            sum_de += de;
            max_de = max_de.max(de);
        }
        (sum_de / colors.len() as f64, max_de)
    }

    fn normalize_weighted_mix(colors: &[(&KsSpectrum, f32)]) -> KsSpectrum {
        let mut mix = ks_mix_weighted(colors);
        let sum_w: f32 = colors.iter().map(|(_, w)| *w).sum();
        let inv = 1.0 / sum_w;
        for value in mix.ks.iter_mut().take(N_SPECTRAL) {
            *value *= inv;
        }
        mix
    }

    #[test]
    fn round_trip_representative_colors() {
        let colors = representative_colors();
        assert!(colors.len() >= 22, "need at least 22 representative colors");

        let (mean_de, max_de) = round_trip_stats(&colors);
        eprintln!("Round-trip: mean ΔE₀₀ = {mean_de:.4}, max ΔE₀₀ = {max_de:.4}");

        assert!(
            mean_de < 0.01,
            "mean ΔE₀₀ = {mean_de:.6} (required: < 0.01)"
        );
        assert!(max_de < 0.5, "max ΔE₀₀ = {max_de:.6} (required: < 0.5)");
    }

    #[test]
    fn round_trip_stats_invariant_to_color_order() {
        let mut colors = representative_colors();
        let (mean_a, max_a) = round_trip_stats(&colors);
        colors.reverse();
        let (mean_b, max_b) = round_trip_stats(&colors);
        assert!((mean_a - mean_b).abs() < 1e-12);
        assert!((max_a - max_b).abs() < 1e-12);
    }

    // Mixing tests

    #[test]
    fn mixing_blue_yellow_gives_green_direction() {
        let transform = illuminant_d65();
        let blue_ks = rgb_to_ks(0.0, 0.0, 1.0).unwrap();
        let yellow_ks = rgb_to_ks(1.0, 1.0, 0.0).unwrap();
        let mixed = ks_mix(&blue_ks, &yellow_ks, 0.5);
        let result = ks_to_srgb(&mixed, transform);
        eprintln!(
            "blue+yellow = [{:.3}, {:.3}, {:.3}]",
            result[0], result[1], result[2]
        );
        // KM mixing: blue+yellow should produce green/teal, not gray
        assert!(
            result[1] + result[2] > result[0] * 5.0,
            "not green-ish (gray like RGB linear mixing): {:?}",
            result
        );
        assert!(result[0] < 0.05, "R too high: {:?}", result);
    }

    #[test]
    fn mixing_red_blue_gives_purple() {
        let transform = illuminant_d65();
        let red_ks = rgb_to_ks(1.0, 0.0, 0.0).unwrap();
        let blue_ks = rgb_to_ks(0.0, 0.0, 1.0).unwrap();
        let mixed = ks_mix(&red_ks, &blue_ks, 0.5);
        let result = ks_to_srgb(&mixed, transform);
        eprintln!(
            "red+blue = [{:.3}, {:.3}, {:.3}]",
            result[0], result[1], result[2]
        );
        // Purple: R and B should exceed G
        assert!(
            result[0] > result[1] && result[2] > result[1],
            "not purple-ish: {:?}",
            result
        );
    }

    #[test]
    fn mixing_red_white_gives_pink() {
        let transform = illuminant_d65();
        let red_ks = rgb_to_ks(1.0, 0.0, 0.0).unwrap();
        let white_ks = rgb_to_ks(1.0, 1.0, 1.0).unwrap();
        let mixed = ks_mix(&red_ks, &white_ks, 0.5);
        let result = ks_to_srgb(&mixed, transform);
        eprintln!(
            "red+white = [{:.3}, {:.3}, {:.3}]",
            result[0], result[1], result[2]
        );
        // Pink: R should be dominant
        assert!(
            result[0] > result[1] && result[0] > result[2],
            "R not dominant: {:?}",
            result
        );
        assert!(
            result[1] > 0.0 && result[2] > 0.0,
            "white mixing had no effect: {:?}",
            result
        );
    }

    #[test]
    fn mixing_blue_white_gives_light_blue() {
        let transform = illuminant_d65();
        let blue_ks = rgb_to_ks(0.0, 0.0, 1.0).unwrap();
        let white_ks = rgb_to_ks(1.0, 1.0, 1.0).unwrap();
        let mixed = ks_mix(&blue_ks, &white_ks, 0.5);
        let result = ks_to_srgb(&mixed, transform);
        eprintln!(
            "blue+white = [{:.3}, {:.3}, {:.3}]",
            result[0], result[1], result[2]
        );
        // Light blue: B should be dominant
        assert!(
            result[2] > result[0] && result[2] > result[1],
            "B not dominant: {:?}",
            result
        );
        assert!(result[1] > 0.0, "white mixing had no effect: {:?}", result);
    }

    #[test]
    fn mixing_commutative_relation_in_srgb_space() {
        let transform = illuminant_d65();
        let red = rgb_to_ks(1.0, 0.0, 0.0).unwrap();
        let blue = rgb_to_ks(0.0, 0.0, 1.0).unwrap();
        let left = ks_to_srgb(&ks_mix(&red, &blue, 0.37), transform);
        let right = ks_to_srgb(&ks_mix(&blue, &red, 0.63), transform);
        let de = srgb_delta_e(left, right);
        assert!(de < 1e-3, "ΔE₀₀={de}");
    }

    #[test]
    fn weighted_mix_normalization_invariant_in_srgb_space() {
        let transform = illuminant_d65();
        let c1 = rgb_to_ks(1.0, 0.0, 0.0).unwrap();
        let c2 = rgb_to_ks(0.0, 1.0, 0.0).unwrap();
        let c3 = rgb_to_ks(0.0, 0.0, 1.0).unwrap();

        let base = normalize_weighted_mix(&[(&c1, 0.2), (&c2, 0.3), (&c3, 0.5)]);
        let scaled = normalize_weighted_mix(&[(&c1, 2.0), (&c2, 3.0), (&c3, 5.0)]);
        let out_a = ks_to_srgb(&base, transform);
        let out_b = ks_to_srgb(&scaled, transform);
        let de = srgb_delta_e(out_a, out_b);
        assert!(de < 1e-3, "ΔE₀₀={de}");
    }

    // Quantitative mixing test: N=41 vs N=401 ground truth

    /// Ground-truth mixing at 1nm resolution (N=401).
    fn mixing_gt_401(coeffs_a: &SigmoidCoeffs, coeffs_b: &SigmoidCoeffs, t: f32) -> [f32; 3] {
        use crate::illuminant::{CMF_X, CMF_Y, CMF_Z, ILLUMINANT_D65};

        const N_GT: usize = 401;

        // Evaluate sigmoid spectrum at 1nm resolution
        let eval_401 = |coeffs: &SigmoidCoeffs| -> Vec<f64> {
            (0..N_GT)
                .map(|i| {
                    let lambda_nm = 380.0 + i as f64;
                    let t_norm = (lambda_nm - 380.0) / 400.0;
                    let x = coeffs.c0 as f64 * t_norm * t_norm
                        + coeffs.c1 as f64 * t_norm
                        + coeffs.c2 as f64;
                    0.5 + x / (2.0 * (1.0 + x * x).sqrt())
                })
                .collect()
        };

        let refl_a = eval_401(coeffs_a);
        let refl_b = eval_401(coeffs_b);

        // KM mixing in K/S space
        let ks_max = 1000.0f64;
        let refl_to_ks = |r: f64| -> f64 {
            let r = r.clamp(0.0, 1.0);
            if r < 1e-10 {
                ks_max
            } else {
                ((1.0 - r) * (1.0 - r)) / (2.0 * r)
            }
        };
        let ks_to_r = |ks: f64| -> f64 {
            if ks >= ks_max {
                0.0
            } else if ks < 1e-6 {
                (1.0 - (2.0 * ks).sqrt()).max(0.0)
            } else {
                (1.0 + ks - (ks * ks + 2.0 * ks).sqrt()).clamp(0.0, 1.0)
            }
        };

        let t_f64 = t as f64;
        let mixed_refl: Vec<f64> = refl_a
            .iter()
            .zip(refl_b.iter())
            .map(|(&refl_a_i, &refl_b_i)| {
                let ks_a = refl_to_ks(refl_a_i);
                let ks_b = refl_to_ks(refl_b_i);
                let ks_mix = (1.0 - t_f64) * ks_a + t_f64 * ks_b;
                ks_to_r(ks_mix)
            })
            .collect();

        // XYZ integration at 401 points (linearly interpolated 10nm CMF to 1nm)
        let interp_cmf = |cmf: &[f32; 41], i_1nm: usize| -> f64 {
            let idx_f = i_1nm as f64 / 10.0;
            let idx = idx_f as usize;
            if idx >= 40 {
                return cmf[40] as f64;
            }
            let frac = idx_f - idx as f64;
            cmf[idx] as f64 * (1.0 - frac) + cmf[idx + 1] as f64 * frac
        };
        let interp_illum = |i_1nm: usize| -> f64 {
            let idx_f = i_1nm as f64 / 10.0;
            let idx = idx_f as usize;
            if idx >= 40 {
                return ILLUMINANT_D65[40] as f64;
            }
            let frac = idx_f - idx as f64;
            ILLUMINANT_D65[idx] as f64 * (1.0 - frac) + ILLUMINANT_D65[idx + 1] as f64 * frac
        };

        // Y normalization
        let mut sum_y_s = 0.0f64;
        for i in 0..N_GT {
            sum_y_s += interp_cmf(&CMF_Y, i) * interp_illum(i);
        }
        let k = 1.0 / sum_y_s;

        // XYZ to linear sRGB (D65)
        let xyz_to_srgb: [[f64; 3]; 3] = [
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

        let mut xyz = [0.0f64; 3];
        for (i, &r) in mixed_refl.iter().enumerate().take(N_GT) {
            let s = interp_illum(i);
            xyz[0] += interp_cmf(&CMF_X, i) * s * r * k;
            xyz[1] += interp_cmf(&CMF_Y, i) * s * r * k;
            xyz[2] += interp_cmf(&CMF_Z, i) * s * r * k;
        }

        let linear_r =
            xyz_to_srgb[0][0] * xyz[0] + xyz_to_srgb[0][1] * xyz[1] + xyz_to_srgb[0][2] * xyz[2];
        let linear_g =
            xyz_to_srgb[1][0] * xyz[0] + xyz_to_srgb[1][1] * xyz[1] + xyz_to_srgb[1][2] * xyz[2];
        let linear_b =
            xyz_to_srgb[2][0] * xyz[0] + xyz_to_srgb[2][1] * xyz[1] + xyz_to_srgb[2][2] * xyz[2];

        [
            neco_color::linear_to_srgb(linear_r.max(0.0) as f32).min(1.0),
            neco_color::linear_to_srgb(linear_g.max(0.0) as f32).min(1.0),
            neco_color::linear_to_srgb(linear_b.max(0.0) as f32).min(1.0),
        ]
    }

    #[test]
    fn mixing_quantitative_gt_comparison() {
        let transform = illuminant_d65();
        type ColorPairCase = ((f32, f32, f32), (f32, f32, f32), &'static str);
        let pairs: &[ColorPairCase] = &[
            ((0.0, 0.0, 1.0), (1.0, 1.0, 0.0), "blue+yellow"),
            ((1.0, 0.0, 0.0), (0.0, 0.0, 1.0), "red+blue"),
            ((1.0, 0.0, 0.0), (1.0, 1.0, 1.0), "red+white"),
            ((0.0, 0.0, 1.0), (1.0, 1.0, 1.0), "blue+white"),
        ];

        for &((r1, g1, b1), (r2, g2, b2), name) in pairs {
            let coeffs_a = rgb_to_sigmoid(r1, g1, b1).unwrap();
            let coeffs_b = rgb_to_sigmoid(r2, g2, b2).unwrap();

            // N=41 result
            let ks_a = rgb_to_ks(r1, g1, b1).unwrap();
            let ks_b = rgb_to_ks(r2, g2, b2).unwrap();
            let mixed_41 = ks_mix(&ks_a, &ks_b, 0.5);
            let result_41 = ks_to_srgb(&mixed_41, transform);

            // N=401 ground truth
            let result_401 = mixing_gt_401(&coeffs_a, &coeffs_b, 0.5);

            let de = srgb_delta_e(result_41, result_401);
            eprintln!(
                "{name}: N=41 [{:.3},{:.3},{:.3}] vs GT [{:.3},{:.3},{:.3}], ΔE₀₀ = {de:.4}",
                result_41[0],
                result_41[1],
                result_41[2],
                result_401[0],
                result_401[1],
                result_401[2]
            );

            assert!(de < 1.0, "{name}: ΔE₀₀ = {de:.4} (required: < 1.0)");
        }
    }

    // Numerical stability tests

    #[test]
    fn stability_black() {
        let transform = illuminant_d65();
        let ks = rgb_to_ks(0.0, 0.0, 0.0).unwrap();
        let _result = ks_to_srgb(&ks, transform);
        // Should not panic
    }

    #[test]
    fn stability_white() {
        let transform = illuminant_d65();
        let ks = rgb_to_ks(1.0, 1.0, 1.0).unwrap();
        let _result = ks_to_srgb(&ks, transform);
    }

    #[test]
    fn stability_high_saturation() {
        let transform = illuminant_d65();
        let saturated = [(1.0, 0.0, 0.0), (0.0, 1.0, 0.0), (0.0, 0.0, 1.0)];
        for (r, g, b) in saturated {
            let ks = rgb_to_ks(r, g, b).unwrap();
            let result = ks_to_srgb(&ks, transform);
            for (c, &value) in result.iter().enumerate() {
                assert!(
                    value.is_finite(),
                    "({r},{g},{b}) ch {c} is not finite: {}",
                    value
                );
            }
        }
    }

    // GN convergence test for all representative colors

    #[test]
    fn gn_convergence_all_representative_colors() {
        let colors = representative_colors();
        for &(r, g, b) in &colors {
            rgb_to_sigmoid(r, g, b).unwrap_or_else(|e| panic!("GN failed for ({r},{g},{b}): {e}"));
        }
    }

    // Pigment tests

    #[test]
    fn pigment_from_srgb_basic() {
        let colors = representative_colors();
        for &(r, g, b) in &colors {
            let p = Pigment::from_srgb(r, g, b)
                .unwrap_or_else(|e| panic!("Pigment::from_srgb({r},{g},{b}) failed: {e}"));
            assert_eq!(p.srgb, [r, g, b]);
        }
    }

    #[test]
    fn pigment_black_sentinel() {
        let p = Pigment::from_srgb(0.0, 0.0, 0.0).unwrap();
        assert_eq!(p.srgb, [0.0, 0.0, 0.0]);
        assert_eq!(p.coeffs.c0, 0.0);
        assert_eq!(p.coeffs.c1, 0.0);
        assert_eq!(p.coeffs.c2, -1e6);
        assert_eq!(p.ks.ks, [1000.0; N_SPECTRAL]);
    }

    #[test]
    fn pigment_white_sentinel() {
        let p = Pigment::from_srgb(1.0, 1.0, 1.0).unwrap();
        assert_eq!(p.srgb, [1.0, 1.0, 1.0]);
        assert_eq!(p.coeffs.c0, 0.0);
        assert_eq!(p.coeffs.c1, 0.0);
        assert_eq!(p.coeffs.c2, 1e6);
        assert_eq!(p.ks.ks, [0.0; N_SPECTRAL]);
    }

    #[test]
    fn pigment_spectrum_matches_sigmoid_to_spectrum() {
        let colors = representative_colors();
        for &(r, g, b) in &colors {
            let p = Pigment::from_srgb(r, g, b).unwrap();
            let direct = sigmoid_to_spectrum(&p.coeffs);
            assert_eq!(p.spectrum(), direct, "mismatch for ({r},{g},{b})");
        }
    }

    #[test]
    fn pigment_ks_matches_rgb_to_ks() {
        let colors = representative_colors();
        for &(r, g, b) in &colors {
            let p = Pigment::from_srgb(r, g, b).unwrap();
            let ks = rgb_to_ks(r, g, b).unwrap();
            assert_eq!(p.ks.ks, ks.ks, "K/S mismatch for ({r},{g},{b})");
        }
    }

    #[test]
    fn pigment_clone_and_debug() {
        let p = Pigment::from_srgb(0.5, 0.3, 0.7).unwrap();
        let p2 = p.clone();
        assert_eq!(p.srgb, p2.srgb);
        assert_eq!(p.ks.ks, p2.ks.ks);
        // Debug trait produces non-empty output
        let dbg = format!("{:?}", p);
        assert!(dbg.contains("Pigment"));
    }
}
