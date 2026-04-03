/// Spline interpolation error type.
#[derive(Debug, Clone)]
pub enum SplineError {
    /// X values are not strictly ascending.
    NonAscendingX,
    /// Fewer than 2 points provided.
    InsufficientPoints,
}

impl std::fmt::Display for SplineError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SplineError::NonAscendingX => {
                write!(f, "control point x values must be strictly ascending")
            }
            SplineError::InsufficientPoints => {
                write!(f, "spline requires at least 2 points")
            }
        }
    }
}

impl std::error::Error for SplineError {}

/// Natural cubic spline interpolation.
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct CubicSpline {
    /// Control points, sorted by ascending x.
    points: Vec<(f32, f32)>,
    /// Per-segment coefficients [a, b, c, d]: y = a + b*(x-xi) + c*(x-xi)^2 + d*(x-xi)^3
    coefficients: Vec<[f32; 4]>,
}

impl CubicSpline {
    pub fn new(points: &[(f32, f32)]) -> Result<Self, SplineError> {
        if points.len() < 2 {
            return Err(SplineError::InsufficientPoints);
        }
        for i in 1..points.len() {
            if points[i].0 <= points[i - 1].0 {
                return Err(SplineError::NonAscendingX);
            }
        }

        if points.len() == 2 {
            let (x0, y0) = points[0];
            let (x1, y1) = points[1];
            let slope = (y1 - y0) / (x1 - x0);
            return Ok(Self {
                points: points.to_vec(),
                coefficients: vec![[y0, slope, 0.0, 0.0]],
            });
        }

        let n = points.len() - 1;
        let h: Vec<f32> = (0..n).map(|i| points[i + 1].0 - points[i].0).collect();
        let f: Vec<f32> = (0..n)
            .map(|i| (points[i + 1].1 - points[i].1) / h[i])
            .collect();

        let m = n - 1;
        if m == 0 {
            unreachable!();
        }

        let mut diag: Vec<f32> = Vec::with_capacity(m);
        let mut sup: Vec<f32> = Vec::with_capacity(m);
        let mut sub: Vec<f32> = Vec::with_capacity(m);
        let mut rhs: Vec<f32> = Vec::with_capacity(m);

        for i in 1..n {
            let idx = i - 1;
            diag.push(2.0 * (h[i - 1] + h[i]));
            rhs.push(3.0 * (f[i] - f[i - 1]));
            if idx > 0 {
                sub.push(h[i - 1]);
            }
            if idx < m - 1 {
                sup.push(h[i]);
            }
        }

        for i in 1..m {
            let factor = sub[i - 1] / diag[i - 1];
            diag[i] -= factor * sup[i - 1];
            rhs[i] -= factor * rhs[i - 1];
        }

        let mut c_inner = vec![0.0f32; m];
        c_inner[m - 1] = rhs[m - 1] / diag[m - 1];
        for i in (0..m - 1).rev() {
            c_inner[i] = (rhs[i] - sup[i] * c_inner[i + 1]) / diag[i];
        }

        let mut c = vec![0.0f32; n + 1];
        c[1..(m + 1)].copy_from_slice(&c_inner[..m]);

        let mut coefficients = Vec::with_capacity(n);
        for i in 0..n {
            let a = points[i].1;
            let b = f[i] - h[i] * (2.0 * c[i] + c[i + 1]) / 3.0;
            let d = (c[i + 1] - c[i]) / (3.0 * h[i]);
            coefficients.push([a, b, c[i], d]);
        }

        Ok(Self {
            points: points.to_vec(),
            coefficients,
        })
    }

    pub fn evaluate(&self, x: f32) -> f32 {
        self.evaluate_with_index(x, None).0
    }

    /// Evaluate the spline for multiple x values.
    ///
    /// When `xs` is sorted in non-decreasing order, span lookup is amortized O(n).
    /// Unsorted input falls back to pointwise binary-search evaluation.
    pub fn evaluate_batch(&self, xs: &[f32]) -> Vec<f32> {
        if xs.is_empty() {
            return Vec::new();
        }
        if xs.windows(2).all(|w| w[0] <= w[1]) {
            let mut out = Vec::with_capacity(xs.len());
            let mut segment = 0usize;
            for &x in xs {
                let (value, next_segment) = self.evaluate_with_index(x, Some(segment));
                out.push(value);
                segment = next_segment;
            }
            out
        } else {
            xs.iter().map(|&x| self.evaluate(x)).collect()
        }
    }

    fn evaluate_with_index(&self, x: f32, start_segment: Option<usize>) -> (f32, usize) {
        if x <= self.points[0].0 {
            return (self.points[0].1, 0);
        }
        let last = self.points.len() - 1;
        if x >= self.points[last].0 {
            return (self.points[last].1, self.coefficients.len() - 1);
        }

        let segment = match start_segment {
            Some(mut idx) if idx < self.coefficients.len() => {
                while idx + 1 < self.points.len() && x >= self.points[idx + 1].0 {
                    idx += 1;
                }
                idx
            }
            _ => self.find_segment(x),
        };

        (self.evaluate_segment(segment, x), segment)
    }

    fn find_segment(&self, x: f32) -> usize {
        let mut lo = 0;
        let mut hi = self.points.len() - 1;
        while lo < hi - 1 {
            let mid = (lo + hi) / 2;
            if x < self.points[mid].0 {
                hi = mid;
            } else {
                lo = mid;
            }
        }
        lo
    }

    fn evaluate_segment(&self, segment: usize, x: f32) -> f32 {
        let dx = x - self.points[segment].0;
        let [a, b, c, d] = self.coefficients[segment];
        a + b * dx + c * dx * dx + d * dx * dx * dx
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn linear_two_points() {
        let spline = CubicSpline::new(&[(0.0, 0.0), (1.0, 1.0)]).unwrap();
        assert!((spline.evaluate(0.5) - 0.5).abs() < 1e-6);
        assert!((spline.evaluate(0.25) - 0.25).abs() < 1e-6);
        assert!((spline.evaluate(0.75) - 0.75).abs() < 1e-6);
    }

    #[test]
    fn three_points_passes_through_control_points() {
        let points = [(0.0, 0.0), (0.5, 0.8), (1.0, 1.0)];
        let spline = CubicSpline::new(&points).unwrap();
        for &(x, y) in &points {
            assert!(
                (spline.evaluate(x) - y).abs() < 1e-5,
                "value mismatch at control point ({}, {}): {}",
                x,
                y,
                spline.evaluate(x)
            );
        }
    }

    #[test]
    fn three_points_interpolation() {
        let spline = CubicSpline::new(&[(0.0, 0.0), (0.5, 0.8), (1.0, 1.0)]).unwrap();
        let val = spline.evaluate(0.25);
        assert!(val > 0.0 && val < 0.8, "unexpected value {} at 0.25", val);
        let val_mid = spline.evaluate(0.75);
        assert!(
            val_mid > 0.7 && val_mid < 1.1,
            "unexpected value {} at 0.75",
            val_mid
        );
    }

    #[test]
    fn clamp_outside_range() {
        let spline = CubicSpline::new(&[(0.2, 0.3), (0.8, 0.9)]).unwrap();
        assert!((spline.evaluate(0.0) - 0.3).abs() < 1e-6);
        assert!((spline.evaluate(-1.0) - 0.3).abs() < 1e-6);
        assert!((spline.evaluate(1.0) - 0.9).abs() < 1e-6);
        assert!((spline.evaluate(10.0) - 0.9).abs() < 1e-6);
    }

    #[test]
    fn error_on_non_ascending_x() {
        let result = CubicSpline::new(&[(0.5, 0.0), (0.3, 1.0)]);
        assert!(result.is_err());
    }

    #[test]
    fn error_on_duplicate_x() {
        let result = CubicSpline::new(&[(0.0, 0.0), (0.5, 0.5), (0.5, 0.8), (1.0, 1.0)]);
        assert!(result.is_err());
    }

    #[test]
    fn error_on_single_point() {
        let result = CubicSpline::new(&[(0.5, 0.5)]);
        assert!(result.is_err());
    }

    #[test]
    fn four_points_smoothness() {
        let spline = CubicSpline::new(&[(0.0, 0.0), (0.25, 0.4), (0.75, 0.9), (1.0, 1.0)]).unwrap();
        assert!((spline.evaluate(0.0) - 0.0).abs() < 1e-5);
        assert!((spline.evaluate(0.25) - 0.4).abs() < 1e-5);
        assert!((spline.evaluate(0.75) - 0.9).abs() < 1e-5);
        assert!((spline.evaluate(1.0) - 1.0).abs() < 1e-5);
        let mut prev = spline.evaluate(0.0);
        for i in 1..=100 {
            let x = i as f32 / 100.0;
            let val = spline.evaluate(x);
            assert!(
                val >= prev - 1e-5,
                "monotonicity broken at x={}: prev={}, val={}",
                x,
                prev,
                val
            );
            prev = val;
        }
    }

    #[test]
    fn evaluate_batch_matches_pointwise_for_sorted_inputs() {
        let spline = CubicSpline::new(&[(0.0, 0.0), (0.25, 0.4), (0.75, 0.9), (1.0, 1.0)]).unwrap();
        let xs = [0.0, 0.1, 0.25, 0.5, 0.75, 1.0];
        let batch = spline.evaluate_batch(&xs);
        let pointwise: Vec<f32> = xs.iter().map(|&x| spline.evaluate(x)).collect();
        assert_eq!(batch.len(), pointwise.len());
        for (actual, expected) in batch.iter().zip(pointwise.iter()) {
            assert!((actual - expected).abs() < 1e-6);
        }
    }

    #[test]
    fn evaluate_batch_falls_back_for_unsorted_inputs() {
        let spline = CubicSpline::new(&[(0.0, 0.0), (0.5, 0.8), (1.0, 1.0)]).unwrap();
        let xs = [0.75, 0.25, 1.0, -1.0, 0.5];
        let batch = spline.evaluate_batch(&xs);
        let pointwise: Vec<f32> = xs.iter().map(|&x| spline.evaluate(x)).collect();
        assert_eq!(batch.len(), pointwise.len());
        for (actual, expected) in batch.iter().zip(pointwise.iter()) {
            assert!((actual - expected).abs() < 1e-6);
        }
    }
}
