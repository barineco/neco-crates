//! 3D rational NURBS curves.

use crate::deboor::deboor_1d_control_points_3d;

/// A 3D rational NURBS curve defined by control points, weights, and a knot vector.
#[derive(Debug, Clone)]
pub struct NurbsCurve3D {
    pub degree: usize,
    pub control_points: Vec<[f64; 3]>,
    pub weights: Vec<f64>,
    pub knots: Vec<f64>,
}

impl NurbsCurve3D {
    /// Create a new NURBS curve with uniform weights (non-rational B-spline).
    pub fn new(degree: usize, control_points: Vec<[f64; 3]>, knots: Vec<f64>) -> Self {
        let n = control_points.len();
        Self {
            degree,
            control_points,
            weights: vec![1.0; n],
            knots,
        }
    }

    /// Create a new rational NURBS curve with explicit weights.
    pub fn new_rational(
        degree: usize,
        control_points: Vec<[f64; 3]>,
        weights: Vec<f64>,
        knots: Vec<f64>,
    ) -> Self {
        Self {
            degree,
            control_points,
            weights,
            knots,
        }
    }

    /// Validate that the curve parameters are consistent.
    pub fn validate(&self) -> Result<(), String> {
        let n = self.control_points.len();
        if n < self.degree + 1 {
            return Err(format!(
                "control point count {} is less than degree+1={}",
                n,
                self.degree + 1
            ));
        }
        if self.weights.len() != n {
            return Err(format!(
                "weight count {} does not match control point count {}",
                self.weights.len(),
                n
            ));
        }
        let expected_knots = n + self.degree + 1;
        if self.knots.len() != expected_knots {
            return Err(format!(
                "knot count {} does not match expected {}",
                self.knots.len(),
                expected_knots
            ));
        }
        for w in self.knots.windows(2) {
            if w[1] < w[0] - 1e-14 {
                return Err(format!(
                    "knot vector not non-decreasing: {} > {}",
                    w[0], w[1]
                ));
            }
        }
        if self.weights.iter().any(|&w| w < 0.0) {
            return Err("negative weight exists".into());
        }
        Ok(())
    }

    /// Evaluate the curve at parameter `t` using De Boor's algorithm.
    pub fn evaluate(&self, t: f64) -> [f64; 3] {
        let (fx, fy, fz, fw, _) = deboor_1d_control_points_3d(
            self.degree,
            &self.knots,
            &self.control_points,
            &self.weights,
            t,
            None,
        );

        if fw.abs() < 1e-30 {
            return [0.0, 0.0, 0.0];
        }
        [fx / fw, fy / fw, fz / fw]
    }

    /// Sample the curve uniformly at `n_points` parameter values.
    pub fn sample(&self, n_points: usize) -> Vec<[f64; 3]> {
        if n_points == 0 {
            return Vec::new();
        }
        if n_points == 1 {
            let t_min = self.knots[self.degree];
            return vec![self.evaluate(t_min)];
        }

        let n = self.control_points.len();
        let t_min = self.knots[self.degree];
        let t_max = self.knots[n];

        let mut out = Vec::with_capacity(n_points);
        let mut hint = self.degree.min(n.saturating_sub(1));
        for i in 0..n_points {
            let t = t_min + (t_max - t_min) * i as f64 / (n_points - 1) as f64;
            let (fx, fy, fz, fw, span) = deboor_1d_control_points_3d(
                self.degree,
                &self.knots,
                &self.control_points,
                &self.weights,
                t,
                Some(hint),
            );
            hint = span;
            if fw.abs() < 1e-30 {
                out.push([0.0, 0.0, 0.0]);
            } else {
                out.push([fx / fw, fy / fw, fz / fw]);
            }
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn line_evaluate() {
        let curve = NurbsCurve3D::new(
            1,
            vec![[0.0, 0.0, 0.0], [3.0, 6.0, 9.0]],
            vec![0.0, 0.0, 1.0, 1.0],
        );

        let p0 = curve.evaluate(0.0);
        assert!((p0[0]).abs() < 1e-12);
        assert!((p0[1]).abs() < 1e-12);
        assert!((p0[2]).abs() < 1e-12);

        let p1 = curve.evaluate(1.0);
        assert!((p1[0] - 3.0).abs() < 1e-12);
        assert!((p1[1] - 6.0).abs() < 1e-12);
        assert!((p1[2] - 9.0).abs() < 1e-12);

        let pm = curve.evaluate(0.5);
        assert!((pm[0] - 1.5).abs() < 1e-12);
        assert!((pm[1] - 3.0).abs() < 1e-12);
        assert!((pm[2] - 4.5).abs() < 1e-12);
    }

    #[test]
    fn circle_arc_in_3d_plane() {
        let w = std::f64::consts::FRAC_1_SQRT_2;
        let curve = NurbsCurve3D::new_rational(
            2,
            vec![[1.0, 0.0, 5.0], [1.0, 1.0, 5.0], [0.0, 1.0, 5.0]],
            vec![1.0, w, 1.0],
            vec![0.0, 0.0, 0.0, 1.0, 1.0, 1.0],
        );

        let p0 = curve.evaluate(0.0);
        assert!((p0[0] - 1.0).abs() < 1e-12);
        assert!((p0[1] - 0.0).abs() < 1e-12);
        assert!((p0[2] - 5.0).abs() < 1e-12);

        let p1 = curve.evaluate(1.0);
        assert!((p1[0] - 0.0).abs() < 1e-12);
        assert!((p1[1] - 1.0).abs() < 1e-12);
        assert!((p1[2] - 5.0).abs() < 1e-12);

        let pm = curve.evaluate(0.5);
        let r = (pm[0] * pm[0] + pm[1] * pm[1]).sqrt();
        assert!((r - 1.0).abs() < 1e-10, "radius at midpoint: {}", r);
        assert!((pm[2] - 5.0).abs() < 1e-12);
    }

    #[test]
    fn validate_good_curve() {
        let curve = NurbsCurve3D::new(
            2,
            vec![[0.0, 0.0, 0.0], [1.0, 1.0, 0.0], [2.0, 0.0, 0.0]],
            vec![0.0, 0.0, 0.0, 1.0, 1.0, 1.0],
        );
        assert!(curve.validate().is_ok());
    }

    #[test]
    fn validate_bad_knot_count() {
        let curve = NurbsCurve3D::new(
            2,
            vec![[0.0, 0.0, 0.0], [1.0, 1.0, 0.0], [2.0, 0.0, 0.0]],
            vec![0.0, 0.0, 1.0, 1.0], // should be 6
        );
        assert!(curve.validate().is_err());
    }

    #[test]
    fn degenerate_weight_returns_zero() {
        let curve = NurbsCurve3D::new_rational(
            1,
            vec![[1.0, 2.0, 3.0], [4.0, 5.0, 6.0]],
            vec![0.0, 0.0], // all zero weights
            vec![0.0, 0.0, 1.0, 1.0],
        );
        let p = curve.evaluate(0.5);
        assert_eq!(p, [0.0, 0.0, 0.0]);
    }
}
