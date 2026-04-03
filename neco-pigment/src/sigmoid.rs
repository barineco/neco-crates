use crate::colorimetry::illuminant_d65;
use crate::illuminant::{LAMBDAS, LAMBDA_MAX, LAMBDA_MIN, N_SPECTRAL};
use crate::PigmentError;

/// Jakob sigmoid coefficients (12 bytes per color).
///
/// Defined over normalized wavelength t = (lambda - 380) / 400 in [0, 1].
#[derive(Clone, Copy, Debug)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct SigmoidCoeffs {
    pub c0: f32,
    pub c1: f32,
    pub c2: f32,
}

/// Max Gauss-Newton iterations.
const MAX_ITERATIONS: usize = 50;
/// Convergence threshold (L2 norm of round-trip RGB residual).
const CONVERGENCE_THRESHOLD: f64 = 1e-5;
/// Accept threshold when stalled (gamut extremes cannot converge exactly).
const STALL_ACCEPT_THRESHOLD: f64 = 2e-3;
/// Stall detection: relative improvement below this ratio.
const STALL_RATIO: f64 = 0.999;

/// Normalize wavelength to [0, 1].
fn normalize_lambda(lambda_nm: f64) -> f64 {
    (lambda_nm - LAMBDA_MIN as f64) / (LAMBDA_MAX as f64 - LAMBDA_MIN as f64)
}

/// Evaluate the Jakob sigmoid: S(t) = 1/2 + x / (2*sqrt(1 + x^2)).
fn sigmoid_eval(c0: f64, c1: f64, c2: f64, t: f64) -> f64 {
    let x = c0 * t * t + c1 * t + c2;
    0.5 + x / (2.0 * (1.0 + x * x).sqrt())
}

/// Sigmoid value and partial derivatives w.r.t. coefficients.
fn sigmoid_deriv(c0: f64, c1: f64, c2: f64, t: f64) -> (f64, [f64; 3]) {
    let x = c0 * t * t + c1 * t + c2;
    let denom = (1.0 + x * x).sqrt();
    let s = 0.5 + x / (2.0 * denom);
    let ds_dx = 1.0 / (2.0 * denom * denom * denom);
    let ds = [ds_dx * t * t, ds_dx * t, ds_dx];
    (s, ds)
}

/// Convert sigmoid coefficients to a reflectance spectrum.
pub fn sigmoid_to_spectrum(coeffs: &SigmoidCoeffs) -> [f32; N_SPECTRAL] {
    let mut refl = [0.0f32; N_SPECTRAL];
    for (i, refl_i) in refl.iter_mut().enumerate().take(N_SPECTRAL) {
        let t = normalize_lambda(LAMBDAS[i] as f64);
        *refl_i = sigmoid_eval(coeffs.c0 as f64, coeffs.c1 as f64, coeffs.c2 as f64, t) as f32;
    }
    refl
}

/// Fit Jakob sigmoid coefficients to an sRGB color via Gauss-Newton optimization.
pub fn rgb_to_sigmoid(r: f32, g: f32, b: f32) -> Result<SigmoidCoeffs, PigmentError> {
    // Target in linear RGB
    let target = [
        neco_color::srgb_to_linear(r) as f64,
        neco_color::srgb_to_linear(g) as f64,
        neco_color::srgb_to_linear(b) as f64,
    ];

    let transform = illuminant_d65();

    // Initial guess: flat spectrum (R=0.5)
    let mut c = [0.0f64; 3];
    let mut best_c = c;
    let mut best_residual = f64::MAX;
    let mut prev_residual = f64::MAX;

    for _iter in 0..MAX_ITERATIONS {
        // Compute predicted RGB and Jacobian
        let mut rgb_pred = [0.0f64; 3];
        let mut jacobian = [[0.0f64; 3]; 3]; // J[rgb_ch][coeff_idx]

        for (i, &lambda_nm) in LAMBDAS.iter().enumerate().take(N_SPECTRAL) {
            let t = normalize_lambda(lambda_nm as f64);
            let (s, ds) = sigmoid_deriv(c[0], c[1], c[2], t);

            for (ch, rgb_pred_ch) in rgb_pred.iter_mut().enumerate() {
                let w = transform.a_rgb_f64(ch, i);
                *rgb_pred_ch += w * s;
                for (j_entry, &ds_j) in jacobian[ch].iter_mut().zip(ds.iter()) {
                    *j_entry += w * ds_j;
                }
            }
        }

        let residual = [
            rgb_pred[0] - target[0],
            rgb_pred[1] - target[1],
            rgb_pred[2] - target[2],
        ];

        let residual_norm =
            (residual[0] * residual[0] + residual[1] * residual[1] + residual[2] * residual[2])
                .sqrt();

        if residual_norm < best_residual {
            best_residual = residual_norm;
            best_c = c;
        }

        // Exact convergence
        if residual_norm < CONVERGENCE_THRESHOLD {
            return Ok(SigmoidCoeffs {
                c0: c[0] as f32,
                c1: c[1] as f32,
                c2: c[2] as f32,
            });
        }

        // Stall detection: sigmoid's (0,1) range causes stalling at gamut extremes
        if residual_norm > prev_residual * STALL_RATIO && best_residual < STALL_ACCEPT_THRESHOLD {
            return Ok(SigmoidCoeffs {
                c0: best_c[0] as f32,
                c1: best_c[1] as f32,
                c2: best_c[2] as f32,
            });
        }
        prev_residual = residual_norm;

        // Gauss-Newton: J^T J Δc = -J^T r
        let jt_j = jtj(&jacobian);
        let jt_r = jtr(&jacobian, &residual);

        if let Some(delta) = solve_3x3(&jt_j, &jt_r) {
            c[0] += delta[0];
            c[1] += delta[1];
            c[2] += delta[2];
        } else {
            break;
        }
    }

    // Accept best result if good enough after loop exit
    if best_residual < STALL_ACCEPT_THRESHOLD {
        return Ok(SigmoidCoeffs {
            c0: best_c[0] as f32,
            c1: best_c[1] as f32,
            c2: best_c[2] as f32,
        });
    }

    Err(PigmentError::ConvergenceFailure {
        residual: best_residual,
    })
}

/// Compute J^T J.
fn jtj(j: &[[f64; 3]; 3]) -> [[f64; 3]; 3] {
    let mut m = [[0.0f64; 3]; 3];
    for i in 0..3 {
        for k in 0..3 {
            for row in j.iter().take(3) {
                m[i][k] += row[i] * row[k];
            }
        }
    }
    m
}

/// Compute -J^T r.
fn jtr(j: &[[f64; 3]; 3], r: &[f64; 3]) -> [f64; 3] {
    let mut v = [0.0f64; 3];
    for i in 0..3 {
        for (row, &r_ch) in j.iter().zip(r.iter()) {
            v[i] -= row[i] * r_ch;
        }
    }
    v
}

/// Solve a 3x3 linear system using Cramer's rule.
fn solve_3x3(a: &[[f64; 3]; 3], b: &[f64; 3]) -> Option<[f64; 3]> {
    let det = a[0][0] * (a[1][1] * a[2][2] - a[1][2] * a[2][1])
        - a[0][1] * (a[1][0] * a[2][2] - a[1][2] * a[2][0])
        + a[0][2] * (a[1][0] * a[2][1] - a[1][1] * a[2][0]);

    if det.abs() < 1e-30 {
        return None;
    }

    let inv_det = 1.0 / det;

    let x0 = (b[0] * (a[1][1] * a[2][2] - a[1][2] * a[2][1])
        - a[0][1] * (b[1] * a[2][2] - a[1][2] * b[2])
        + a[0][2] * (b[1] * a[2][1] - a[1][1] * b[2]))
        * inv_det;

    let x1 = (a[0][0] * (b[1] * a[2][2] - a[1][2] * b[2])
        - b[0] * (a[1][0] * a[2][2] - a[1][2] * a[2][0])
        + a[0][2] * (a[1][0] * b[2] - b[1] * a[2][0]))
        * inv_det;

    let x2 = (a[0][0] * (a[1][1] * b[2] - b[1] * a[2][1])
        - a[0][1] * (a[1][0] * b[2] - b[1] * a[2][0])
        + b[0] * (a[1][0] * a[2][1] - a[1][1] * a[2][0]))
        * inv_det;

    Some([x0, x1, x2])
}

// Accessor helper for RgbTransform
impl crate::colorimetry::RgbTransform {
    /// Access a_rgb element as f64.
    pub(crate) fn a_rgb_f64(&self, channel: usize, wavelength_idx: usize) -> f64 {
        self.a_rgb[channel * N_SPECTRAL + wavelength_idx] as f64
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn representative_colors() -> [(f32, f32, f32); 8] {
        [
            (1.0, 0.0, 0.0),
            (0.0, 1.0, 0.0),
            (0.0, 0.0, 1.0),
            (1.0, 1.0, 1.0),
            (0.0, 0.0, 0.0),
            (0.5, 0.5, 0.5),
            (0.9, 0.3, 0.2),
            (0.2, 0.7, 0.8),
        ]
    }

    #[test]
    fn sigmoid_spectrum_range() {
        let coeffs = SigmoidCoeffs {
            c0: 0.0,
            c1: 0.0,
            c2: 0.0,
        };
        let refl = sigmoid_to_spectrum(&coeffs);
        // c0=c1=c2=0 -> S(lambda) = 0.5 for all wavelengths
        for &value in refl.iter().take(N_SPECTRAL) {
            assert!((value - 0.5).abs() < 1e-6);
        }
    }

    #[test]
    fn gn_convergence_gray() {
        let coeffs = rgb_to_sigmoid(0.5, 0.5, 0.5).expect("GN should converge for gray");
        assert!(coeffs.c0.abs() < 1.0);
    }

    #[test]
    fn gn_convergence_primaries() {
        // Verify convergence for 6 primary/secondary colors
        let colors = [
            (1.0, 0.0, 0.0), // Red
            (0.0, 1.0, 0.0), // Green
            (0.0, 0.0, 1.0), // Blue
            (1.0, 1.0, 0.0), // Yellow
            (1.0, 0.0, 1.0), // Magenta
            (0.0, 1.0, 1.0), // Cyan
        ];
        for (r, g, b) in colors {
            rgb_to_sigmoid(r, g, b).unwrap_or_else(|e| panic!("GN failed for ({r},{g},{b}): {e}"));
        }
    }

    #[test]
    fn representative_sigmoid_spectrum_is_bounded() {
        for (r, g, b) in representative_colors() {
            let coeffs = rgb_to_sigmoid(r, g, b)
                .unwrap_or_else(|e| panic!("rgb_to_sigmoid failed for ({r},{g},{b}): {e}"));
            let refl = sigmoid_to_spectrum(&coeffs);
            for (i, &value) in refl.iter().enumerate() {
                assert!(value.is_finite(), "non-finite reflectance i={i}");
                assert!(
                    (-1e-6..=1.0 + 1e-6).contains(&value),
                    "out of range i={i}: {value}"
                );
            }
        }
    }

    #[test]
    fn rgb_to_sigmoid_is_deterministic() {
        let (r, g, b) = (0.23, 0.61, 0.84);
        let a = rgb_to_sigmoid(r, g, b).unwrap();
        let b2 = rgb_to_sigmoid(r, g, b).unwrap();
        assert!((a.c0 - b2.c0).abs() < 1e-7);
        assert!((a.c1 - b2.c1).abs() < 1e-7);
        assert!((a.c2 - b2.c2).abs() < 1e-7);
    }

    #[test]
    fn sigmoid_to_ks_path_is_finite() {
        for (r, g, b) in representative_colors() {
            let coeffs = rgb_to_sigmoid(r, g, b)
                .unwrap_or_else(|e| panic!("rgb_to_sigmoid failed for ({r},{g},{b}): {e}"));
            let refl = sigmoid_to_spectrum(&coeffs);
            let ks = crate::reflectance_to_ks(&refl);
            for (i, &value) in ks.ks.iter().enumerate() {
                assert!(value.is_finite(), "non-finite ks i={i}: {value}");
                assert!(value >= 0.0, "negative ks i={i}: {value}");
            }
        }
    }
}
