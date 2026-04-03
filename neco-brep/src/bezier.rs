//! Bezier curve evaluation and frame interpolation utilities.

/// Rational De Casteljau algorithm (2D).
pub fn de_casteljau_rational_2d(
    control_points: &[[f64; 2]],
    weights: &[f64],
    t: f64,
) -> (f64, f64) {
    let n = control_points.len();
    let mut pts: Vec<[f64; 2]> = control_points.to_vec();
    let mut wts: Vec<f64> = weights.to_vec();
    for level in 1..n {
        for i in 0..n - level {
            let w0 = wts[i] * (1.0 - t);
            let w1 = wts[i + 1] * t;
            let w_sum = w0 + w1;
            pts[i][0] = (pts[i][0] * w0 + pts[i + 1][0] * w1) / w_sum;
            pts[i][1] = (pts[i][1] * w0 + pts[i + 1][1] * w1) / w_sum;
            wts[i] = w_sum;
        }
    }
    (pts[0][0], pts[0][1])
}

/// Rational De Casteljau algorithm (3D).
pub fn de_casteljau_rational_3d(control_points: &[[f64; 3]], weights: &[f64], t: f64) -> [f64; 3] {
    let n = control_points.len();
    let mut pts: Vec<[f64; 3]> = control_points.to_vec();
    let mut wts: Vec<f64> = weights.to_vec();
    for level in 1..n {
        for i in 0..n - level {
            let w0 = wts[i] * (1.0 - t);
            let w1 = wts[i + 1] * t;
            let w_sum = w0 + w1;
            pts[i] = [
                (pts[i][0] * w0 + pts[i + 1][0] * w1) / w_sum,
                (pts[i][1] * w0 + pts[i + 1][1] * w1) / w_sum,
                (pts[i][2] * w0 + pts[i + 1][2] * w1) / w_sum,
            ];
            wts[i] = w_sum;
        }
    }
    pts[0]
}

/// Normalize a vector, returning `fallback` if near-zero.
fn normalize_with_fallback(v: [f64; 3], fallback: [f64; 3]) -> [f64; 3] {
    let len = (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt();
    if len < 1e-30 {
        return fallback;
    }
    let inv = 1.0 / len;
    [v[0] * inv, v[1] * inv, v[2] * inv]
}

/// Linearly interpolate between two frames, returning normalized (normal, binormal).
///
/// `frames` layout: `[[normal, binormal, tangent]; 2]`.
/// Falls back to `[0, 0, 1]` on zero vectors.
pub fn interpolate_frame(frames: &[[[f64; 3]; 3]], u: f64) -> ([f64; 3], [f64; 3]) {
    let n0 = frames[0][0];
    let b0 = frames[0][1];
    let n1 = frames[1][0];
    let b1 = frames[1][1];

    let fallback = [0.0, 0.0, 1.0];

    let normal = normalize_with_fallback(
        [
            n0[0] * (1.0 - u) + n1[0] * u,
            n0[1] * (1.0 - u) + n1[1] * u,
            n0[2] * (1.0 - u) + n1[2] * u,
        ],
        fallback,
    );
    let binormal = normalize_with_fallback(
        [
            b0[0] * (1.0 - u) + b1[0] * u,
            b0[1] * (1.0 - u) + b1[1] * u,
            b0[2] * (1.0 - u) + b1[2] * u,
        ],
        fallback,
    );
    (normal, binormal)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_de_casteljau_2d_linear() {
        let pts = [[0.0, 0.0], [1.0, 1.0]];
        let ws = [1.0, 1.0];
        let (x, y) = de_casteljau_rational_2d(&pts, &ws, 0.5);
        assert!((x - 0.5).abs() < 1e-12);
        assert!((y - 0.5).abs() < 1e-12);
    }

    #[test]
    fn test_de_casteljau_3d_linear() {
        let pts = [[0.0, 0.0, 0.0], [2.0, 4.0, 6.0]];
        let ws = [1.0, 1.0];
        let result = de_casteljau_rational_3d(&pts, &ws, 0.25);
        assert!((result[0] - 0.5).abs() < 1e-12);
        assert!((result[1] - 1.0).abs() < 1e-12);
        assert!((result[2] - 1.5).abs() < 1e-12);
    }

    #[test]
    fn test_interpolate_frame_midpoint() {
        let frames = [
            [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]],
            [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]],
        ];
        let (n, b) = interpolate_frame(&frames, 0.5);
        assert!((n[0] - 1.0).abs() < 1e-12);
        assert!((b[1] - 1.0).abs() < 1e-12);
    }

    #[test]
    fn test_interpolate_frame_zero_fallback() {
        let frames = [
            [[0.0, 0.0, 0.0], [0.0, 0.0, 0.0], [0.0, 0.0, 1.0]],
            [[0.0, 0.0, 0.0], [0.0, 0.0, 0.0], [0.0, 0.0, 1.0]],
        ];
        let (n, b) = interpolate_frame(&frames, 0.5);
        assert_eq!(n, [0.0, 0.0, 1.0]);
        assert_eq!(b, [0.0, 0.0, 1.0]);
    }
}
