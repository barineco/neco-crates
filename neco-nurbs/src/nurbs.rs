//! 2D rational NURBS curves and region representation.
//!
//! Coordinates use `[f64; 2]`.

use crate::deboor::deboor_1d_control_points_2d;

/// Distance between two points.
#[inline]
fn dist(a: [f64; 2], b: [f64; 2]) -> f64 {
    ((a[0] - b[0]).powi(2) + (a[1] - b[1]).powi(2)).sqrt()
}

/// A 2D rational NURBS curve defined by control points, weights, and a knot vector.
///
/// Evaluation uses De Boor's algorithm in homogeneous coordinates:
/// 1. Lift control points to (w*x, w*y, w)
/// 2. Apply standard De Boor recursion
/// 3. Project back: (X/W, Y/W)
#[derive(Debug, Clone)]
pub struct NurbsCurve2D {
    pub degree: usize,
    pub control_points: Vec<[f64; 2]>,
    pub weights: Vec<f64>,
    pub knots: Vec<f64>,
}

impl NurbsCurve2D {
    /// Create a new NURBS curve with uniform weights (non-rational B-spline).
    pub fn new(degree: usize, control_points: Vec<[f64; 2]>, knots: Vec<f64>) -> Self {
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
        control_points: Vec<[f64; 2]>,
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

    /// Validate the NURBS curve parameters.
    ///
    /// Checks:
    /// - At least degree+1 control points
    /// - weights.len() == control_points.len()
    /// - knots.len() == control_points.len() + degree + 1
    /// - Knot vector is non-decreasing
    /// - All weights are positive
    pub fn validate(&self) -> Result<(), String> {
        let n = self.control_points.len();
        let p = self.degree;

        if n < p + 1 {
            return Err(format!(
                "need at least {} control points for degree {}, got {}",
                p + 1,
                p,
                n
            ));
        }

        if self.weights.len() != n {
            return Err(format!(
                "weights.len() ({}) != control_points.len() ({})",
                self.weights.len(),
                n
            ));
        }

        let expected_knots = n + p + 1;
        if self.knots.len() != expected_knots {
            return Err(format!(
                "knots.len() ({}) != n + p + 1 ({})",
                self.knots.len(),
                expected_knots
            ));
        }

        for i in 1..self.knots.len() {
            if self.knots[i] < self.knots[i - 1] {
                return Err(format!(
                    "knot vector not non-decreasing at index {}: {} < {}",
                    i,
                    self.knots[i],
                    self.knots[i - 1]
                ));
            }
        }

        for (i, &w) in self.weights.iter().enumerate() {
            if w <= 0.0 {
                return Err(format!("weight[{}] = {} is not positive", i, w));
            }
        }

        Ok(())
    }

    /// Check whether the curve is closed (start == end within tolerance).
    pub fn is_closed(&self, tol: f64) -> bool {
        if self.control_points.len() < 2 {
            return false;
        }
        let first = self.control_points[0];
        let last = self.control_points[self.control_points.len() - 1];
        (first[0] - last[0]).abs() < tol && (first[1] - last[1]).abs() < tol
    }

    /// Evaluate the curve at parameter `t` using De Boor's algorithm.
    ///
    /// `t` should be in the range [knots[degree], knots[n]] where n = control_points.len().
    ///
    /// Panics when `degree > 10` because the fast path uses a fixed-size stack buffer.
    pub fn evaluate(&self, t: f64) -> [f64; 2] {
        let (fx, fy, fw, _) = deboor_1d_control_points_2d(
            self.degree,
            &self.knots,
            &self.control_points,
            &self.weights,
            t,
            None,
        );

        if fw.abs() < 1e-30 {
            return [0.0, 0.0];
        }
        [fx / fw, fy / fw]
    }

    /// Evaluate sorted samples while reusing the previous knot span as a hint.
    ///
    /// Panics when `degree > 10` because the fast path uses a fixed-size stack buffer.
    pub fn evaluate_samples(&self, ts: &[f64], out: &mut Vec<[f64; 2]>) {
        out.clear();
        out.reserve(ts.len());

        if ts.is_empty() {
            return;
        }

        let mut hint = self.degree.min(self.control_points.len().saturating_sub(1));
        for &t in ts {
            let (fx, fy, fw, span) = deboor_1d_control_points_2d(
                self.degree,
                &self.knots,
                &self.control_points,
                &self.weights,
                t,
                Some(hint),
            );
            hint = span;
            if fw.abs() < 1e-30 {
                out.push([0.0, 0.0]);
            } else {
                out.push([fx / fw, fy / fw]);
            }
        }
    }

    /// Sample the curve uniformly at `n_points` parameter values.
    pub fn sample(&self, n_points: usize) -> Vec<[f64; 2]> {
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
        let ts: Vec<f64> = (0..n_points)
            .map(|i| t_min + (t_max - t_min) * i as f64 / (n_points - 1) as f64)
            .collect();
        let mut out = Vec::with_capacity(n_points);
        self.evaluate_samples(&ts, &mut out);
        out
    }

    /// Adaptively sample the curve so that no edge in the resulting polygon
    /// exceeds `max_edge` in length.
    pub fn adaptive_sample(&self, max_edge: f64) -> Vec<[f64; 2]> {
        let n = self.control_points.len();
        let t_max = self.knots[n];

        let mut breaks: Vec<f64> = self.knots[self.degree..=n].to_vec();
        breaks.dedup_by(|a, b| (*a - *b).abs() < 1e-14);

        let mut params: Vec<f64> = Vec::new();
        for pair in breaks.windows(2) {
            let (ta, tb) = (pair[0], pair[1]);
            let pa = self.evaluate(ta);
            let pb = self.evaluate(tb);
            let seg_len = dist(pa, pb);
            let seg_n = ((seg_len / max_edge).ceil() as usize).max(1);
            for j in 0..seg_n {
                params.push(ta + (tb - ta) * j as f64 / seg_n as f64);
            }
        }
        params.push(t_max);
        let mut points: Vec<[f64; 2]> = params.iter().map(|&t| self.evaluate(t)).collect();

        let max_iterations = 12;
        for _ in 0..max_iterations {
            let mut new_params = Vec::with_capacity(params.len() * 2);
            let mut new_points = Vec::with_capacity(points.len() * 2);
            let mut refined = false;

            new_params.push(params[0]);
            new_points.push(points[0]);

            for i in 1..params.len() {
                let d = dist(points[i - 1], points[i]);
                if d > max_edge {
                    let t_mid = 0.5 * (params[i - 1] + params[i]);
                    let p_mid = self.evaluate(t_mid);
                    new_params.push(t_mid);
                    new_points.push(p_mid);
                    refined = true;
                }
                new_params.push(params[i]);
                new_points.push(points[i]);
            }

            if !refined {
                break;
            }
            params = new_params;
            points = new_points;
        }

        points
    }

    /// Approximate length of the control polygon.
    pub fn control_polygon_length(&self) -> f64 {
        self.control_points
            .windows(2)
            .map(|w| dist(w[0], w[1]))
            .sum()
    }

    /// Knot insertion (De Boor algorithm).
    ///
    /// Insert a single knot at parameter `t` without changing the curve shape.
    pub fn insert_knot(&self, t: f64) -> NurbsCurve2D {
        let p = self.degree;
        let n = self.control_points.len();
        let k = crate::deboor::find_knot_span(&self.knots, p, n, t);
        debug_assert!(k >= p, "find_knot_span returned k={} < p={}", k, p);

        let mut new_knots = Vec::with_capacity(self.knots.len() + 1);
        new_knots.extend_from_slice(&self.knots[..=k]);
        new_knots.push(t);
        new_knots.extend_from_slice(&self.knots[k + 1..]);

        let mut new_pts = Vec::with_capacity(n + 1);
        let mut new_weights = Vec::with_capacity(n + 1);

        for i in 0..=n {
            if i <= k.saturating_sub(p) {
                new_pts.push(self.control_points[i]);
                new_weights.push(self.weights[i]);
            } else if i > k {
                new_pts.push(self.control_points[i - 1]);
                new_weights.push(self.weights[i - 1]);
            } else {
                let denom = self.knots[i + p] - self.knots[i];
                let alpha = if denom.abs() < 1e-30 {
                    0.0
                } else {
                    (t - self.knots[i]) / denom
                };

                let w_prev = self.weights[i - 1];
                let w_curr = self.weights[i];
                let w_new = (1.0 - alpha) * w_prev + alpha * w_curr;

                let x_new = ((1.0 - alpha) * self.control_points[i - 1][0] * w_prev
                    + alpha * self.control_points[i][0] * w_curr)
                    / w_new;
                let y_new = ((1.0 - alpha) * self.control_points[i - 1][1] * w_prev
                    + alpha * self.control_points[i][1] * w_curr)
                    / w_new;

                new_pts.push([x_new, y_new]);
                new_weights.push(w_new);
            }
        }

        NurbsCurve2D::new_rational(p, new_pts, new_weights, new_knots)
    }

    /// Decompose the curve into Bezier spans.
    pub fn to_bezier_spans(&self) -> Vec<NurbsCurve2D> {
        let p = self.degree;

        let mut curve = self.clone();

        let n = curve.control_points.len();
        let t_min = curve.knots[p];
        let t_max = curve.knots[n];

        let mut internal_knots: Vec<(f64, usize)> = Vec::new();
        let mut i = p + 1;
        while i < curve.knots.len() - p - 1 {
            let t = curve.knots[i];
            if t > t_min && t < t_max {
                let mut mult = 0;
                let mut j = i;
                while j < curve.knots.len() && (curve.knots[j] - t).abs() < 1e-14 {
                    mult += 1;
                    j += 1;
                }
                internal_knots.push((t, mult));
                i = j;
            } else {
                i += 1;
            }
        }

        for (t, mult) in &internal_knots {
            let insertions_needed = p - mult;
            for _ in 0..insertions_needed {
                curve = curve.insert_knot(*t);
            }
        }

        let num_spans = (curve.control_points.len() - 1) / p;
        if num_spans == 0 {
            return vec![curve];
        }

        let mut spans = Vec::with_capacity(num_spans);
        for s in 0..num_spans {
            let start_cp = s * p;
            let end_cp = start_cp + p + 1;
            if end_cp > curve.control_points.len() {
                break;
            }

            let pts: Vec<[f64; 2]> = curve.control_points[start_cp..end_cp].to_vec();
            let wts: Vec<f64> = curve.weights[start_cp..end_cp].to_vec();

            let knot_start = curve.knots[start_cp + p];
            let knot_end = curve.knots[end_cp];
            let mut knots = vec![knot_start; p + 1];
            knots.extend(vec![knot_end; p + 1]);

            spans.push(NurbsCurve2D::new_rational(p, pts, wts, knots));
        }

        spans
    }

    /// Split the curve into two at parameter `t`.
    pub fn split_at(&self, t: f64) -> (NurbsCurve2D, NurbsCurve2D) {
        let p = self.degree;

        let mut mult = 0;
        for &knot in &self.knots {
            if (knot - t).abs() < 1e-14 {
                mult += 1;
            }
        }

        let mut curve = self.clone();
        let insertions_needed = (p + 1).saturating_sub(mult);
        for _ in 0..insertions_needed {
            curve = curve.insert_knot(t);
        }

        let mut first_t_idx = 0;
        let mut last_t_idx = 0;
        let mut found_first = false;
        for (i, &knot) in curve.knots.iter().enumerate() {
            if (knot - t).abs() < 1e-14 {
                if !found_first {
                    first_t_idx = i;
                    found_first = true;
                }
                last_t_idx = i;
            }
        }

        let n_left = last_t_idx - p;

        let left_pts = curve.control_points[..n_left].to_vec();
        let left_wts = curve.weights[..n_left].to_vec();
        let left_knots = curve.knots[..=last_t_idx].to_vec();

        let right_pts = curve.control_points[n_left..].to_vec();
        let right_wts = curve.weights[n_left..].to_vec();
        let right_knots = curve.knots[first_t_idx..].to_vec();

        let left = NurbsCurve2D::new_rational(p, left_pts, left_wts, left_knots);
        let right = NurbsCurve2D::new_rational(p, right_pts, right_wts, right_knots);

        (left, right)
    }

    /// Split the curve at multiple sorted parameter values.
    pub fn split_at_params(&self, ts: &[f64]) -> Vec<NurbsCurve2D> {
        if ts.is_empty() {
            return vec![self.clone()];
        }

        let mut result = Vec::with_capacity(ts.len() + 1);
        let mut remaining = self.clone();

        for (i, &t) in ts.iter().enumerate() {
            let (left, right) = remaining.split_at(t);
            result.push(left);
            if i == ts.len() - 1 {
                result.push(right);
            } else {
                remaining = right;
            }
        }

        result
    }

    /// Reverse the curve (flip parameter direction).
    pub fn reverse(&self) -> NurbsCurve2D {
        let p = self.degree;
        let mut pts = self.control_points.clone();
        pts.reverse();
        let mut wts = self.weights.clone();
        wts.reverse();

        let knot_min = self.knots[0];
        let knot_max = self.knots[self.knots.len() - 1];
        let n = self.knots.len();
        let mut new_knots = Vec::with_capacity(n);
        for i in 0..n {
            new_knots.push(knot_max + knot_min - self.knots[n - 1 - i]);
        }

        NurbsCurve2D::new_rational(p, pts, wts, new_knots)
    }

    /// Axis-aligned bounding box from control points: (min, max).
    pub fn bounding_box(&self) -> ([f64; 2], [f64; 2]) {
        let mut min_x = f64::INFINITY;
        let mut min_y = f64::INFINITY;
        let mut max_x = f64::NEG_INFINITY;
        let mut max_y = f64::NEG_INFINITY;

        for p in &self.control_points {
            if p[0] < min_x {
                min_x = p[0];
            }
            if p[1] < min_y {
                min_y = p[1];
            }
            if p[0] > max_x {
                max_x = p[0];
            }
            if p[1] > max_y {
                max_y = p[1];
            }
        }

        ([min_x, min_y], [max_x, max_y])
    }

    /// Exact circle as a degree-2 rational NURBS (4 quadrants, 9 control points).
    pub fn circle(center: [f64; 2], radius: f64) -> Self {
        let w = std::f64::consts::FRAC_1_SQRT_2;
        let r = radius;
        let (cx, cy) = (center[0], center[1]);
        NurbsCurve2D::new_rational(
            2,
            vec![
                [cx + r, cy],
                [cx + r, cy + r],
                [cx, cy + r],
                [cx - r, cy + r],
                [cx - r, cy],
                [cx - r, cy - r],
                [cx, cy - r],
                [cx + r, cy - r],
                [cx + r, cy],
            ],
            vec![1.0, w, 1.0, w, 1.0, w, 1.0, w, 1.0],
            vec![
                0.0, 0.0, 0.0, 0.25, 0.25, 0.5, 0.5, 0.75, 0.75, 1.0, 1.0, 1.0,
            ],
        )
    }
}

/// A 2D region bounded by closed NURBS boundaries.
///
/// Both outer boundary and holes are piecewise curves (`Vec<NurbsCurve2D>`).
#[derive(Clone, Debug)]
pub struct NurbsRegion {
    /// Outer boundary (counter-clockwise) -- one or more segments.
    pub outer: Vec<NurbsCurve2D>,
    /// Holes (each clockwise) -- each hole is one or more segments.
    pub holes: Vec<Vec<NurbsCurve2D>>,
}

impl NurbsRegion {
    pub fn outer_curves(&self) -> &[NurbsCurve2D] {
        &self.outer
    }

    pub fn hole_curves(&self, i: usize) -> &[NurbsCurve2D] {
        &self.holes[i]
    }

    pub fn holes_count(&self) -> usize {
        self.holes.len()
    }

    pub fn outer_is_closed(&self, tol: f64) -> bool {
        piecewise_is_closed(&self.outer, tol)
    }

    pub fn outer_bounding_box(&self) -> ([f64; 2], [f64; 2]) {
        piecewise_bounding_box(&self.outer)
    }

    pub fn outer_control_points_flat(&self) -> Vec<[f64; 2]> {
        piecewise_control_points_flat(&self.outer)
    }

    pub fn outer_adaptive_sample(&self, max_edge: f64) -> Vec<[f64; 2]> {
        dedup_piecewise_sample(self.outer.iter(), max_edge)
    }

    pub fn hole_adaptive_sample(&self, i: usize, max_edge: f64) -> Vec<[f64; 2]> {
        dedup_piecewise_sample(self.holes[i].iter(), max_edge)
    }
}

fn piecewise_is_closed(curves: &[NurbsCurve2D], tol: f64) -> bool {
    if curves.is_empty() {
        return false;
    }
    if curves.len() == 1 {
        return curves[0].is_closed(tol);
    }
    let first_start = curves.first().and_then(|c| c.control_points.first());
    let last_end = curves.last().and_then(|c| c.control_points.last());
    matches!((first_start, last_end), (Some(a), Some(b)) if dist(*a, *b) < tol)
}

fn piecewise_bounding_box(curves: &[NurbsCurve2D]) -> ([f64; 2], [f64; 2]) {
    let mut min = [f64::INFINITY, f64::INFINITY];
    let mut max = [f64::NEG_INFINITY, f64::NEG_INFINITY];
    for curve in curves {
        let (cmin, cmax) = curve.bounding_box();
        min[0] = min[0].min(cmin[0]);
        min[1] = min[1].min(cmin[1]);
        max[0] = max[0].max(cmax[0]);
        max[1] = max[1].max(cmax[1]);
    }
    (min, max)
}

fn piecewise_control_points_flat(curves: &[NurbsCurve2D]) -> Vec<[f64; 2]> {
    let mut result = Vec::new();
    for (i, curve) in curves.iter().enumerate() {
        if i == 0 {
            result.extend_from_slice(&curve.control_points);
        } else if !curve.control_points.is_empty() {
            let start = if let Some(last) = result.last() {
                if dist(*last, curve.control_points[0]) < 1e-10 {
                    1
                } else {
                    0
                }
            } else {
                0
            };
            result.extend_from_slice(&curve.control_points[start..]);
        }
    }
    result
}

/// Adaptively sample a piecewise curve sequence, deduplicating shared endpoints.
pub fn dedup_piecewise_sample<'a>(
    curves: impl Iterator<Item = &'a NurbsCurve2D>,
    max_edge: f64,
) -> Vec<[f64; 2]> {
    let mut result: Vec<[f64; 2]> = Vec::new();
    for curve in curves {
        let pts = curve.adaptive_sample(max_edge);
        if result.is_empty() {
            result.extend(pts);
        } else {
            let start = if let (Some(last), Some(first)) = (result.last(), pts.first()) {
                if dist(*last, *first) < 1e-10 {
                    1
                } else {
                    0
                }
            } else {
                0
            };
            result.extend_from_slice(&pts[start..]);
        }
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use approx::assert_relative_eq;
    use std::f64::consts::PI;

    fn quarter_circle() -> NurbsCurve2D {
        let w = std::f64::consts::FRAC_1_SQRT_2;
        NurbsCurve2D::new_rational(
            2,
            vec![[1.0, 0.0], [1.0, 1.0], [0.0, 1.0]],
            vec![1.0, w, 1.0],
            vec![0.0, 0.0, 0.0, 1.0, 1.0, 1.0],
        )
    }

    fn unit_circle() -> NurbsCurve2D {
        let w = std::f64::consts::FRAC_1_SQRT_2;
        NurbsCurve2D::new_rational(
            2,
            vec![
                [1.0, 0.0],
                [1.0, 1.0],
                [0.0, 1.0],
                [-1.0, 1.0],
                [-1.0, 0.0],
                [-1.0, -1.0],
                [0.0, -1.0],
                [1.0, -1.0],
                [1.0, 0.0],
            ],
            vec![1.0, w, 1.0, w, 1.0, w, 1.0, w, 1.0],
            vec![
                0.0, 0.0, 0.0, 0.25, 0.25, 0.5, 0.5, 0.75, 0.75, 1.0, 1.0, 1.0,
            ],
        )
    }

    fn assert_curves_match_on_unit_interval(lhs: &NurbsCurve2D, rhs: &NurbsCurve2D) {
        for i in 0..=32 {
            let t = i as f64 / 32.0;
            let p_lhs = lhs.evaluate(t);
            let p_rhs = rhs.evaluate(t);
            assert_relative_eq!(p_lhs[0], p_rhs[0], epsilon = 1e-10);
            assert_relative_eq!(p_lhs[1], p_rhs[1], epsilon = 1e-10);
        }
    }

    fn translated_curve(curve: &NurbsCurve2D, delta: [f64; 2]) -> NurbsCurve2D {
        let mut control_points = curve.control_points.clone();
        for point in &mut control_points {
            point[0] += delta[0];
            point[1] += delta[1];
        }

        NurbsCurve2D::new_rational(
            curve.degree,
            control_points,
            curve.weights.clone(),
            curve.knots.clone(),
        )
    }

    #[test]
    fn validate_good_curve() {
        let c = quarter_circle();
        assert!(c.validate().is_ok());
    }

    #[test]
    fn validate_bad_knot_count() {
        let c = NurbsCurve2D::new(
            2,
            vec![[0.0, 0.0], [1.0, 0.0], [1.0, 1.0]],
            vec![0.0, 0.0, 1.0, 1.0],
        );
        assert!(c.validate().is_err());
    }

    #[test]
    fn validate_non_decreasing_knots() {
        let c = NurbsCurve2D::new(
            2,
            vec![[0.0, 0.0], [1.0, 0.0], [1.0, 1.0]],
            vec![0.0, 0.0, 0.0, 0.5, 0.3, 1.0],
        );
        assert!(c.validate().is_err());
    }

    #[test]
    fn validate_negative_weight() {
        let c = NurbsCurve2D::new_rational(
            2,
            vec![[0.0, 0.0], [1.0, 0.0], [1.0, 1.0]],
            vec![1.0, -1.0, 1.0],
            vec![0.0, 0.0, 0.0, 1.0, 1.0, 1.0],
        );
        assert!(c.validate().is_err());
    }

    #[test]
    fn validate_too_few_control_points() {
        let c = NurbsCurve2D::new(3, vec![[0.0, 0.0], [1.0, 0.0]], vec![0.0; 6]);
        assert!(c.validate().is_err());
    }

    #[test]
    fn line_degree1_exact() {
        let c = NurbsCurve2D::new(1, vec![[0.0, 0.0], [4.0, 6.0]], vec![0.0, 0.0, 1.0, 1.0]);
        assert!(c.validate().is_ok());

        let p0 = c.evaluate(0.0);
        assert_relative_eq!(p0[0], 0.0, epsilon = 1e-12);
        assert_relative_eq!(p0[1], 0.0, epsilon = 1e-12);

        let p1 = c.evaluate(1.0);
        assert_relative_eq!(p1[0], 4.0, epsilon = 1e-12);
        assert_relative_eq!(p1[1], 6.0, epsilon = 1e-12);

        let pm = c.evaluate(0.5);
        assert_relative_eq!(pm[0], 2.0, epsilon = 1e-12);
        assert_relative_eq!(pm[1], 3.0, epsilon = 1e-12);

        let pq = c.evaluate(0.25);
        assert_relative_eq!(pq[0], 1.0, epsilon = 1e-12);
        assert_relative_eq!(pq[1], 1.5, epsilon = 1e-12);
    }

    #[test]
    fn quarter_circle_endpoints() {
        let c = quarter_circle();
        let p0 = c.evaluate(0.0);
        assert_relative_eq!(p0[0], 1.0, epsilon = 1e-12);
        assert_relative_eq!(p0[1], 0.0, epsilon = 1e-12);

        let p1 = c.evaluate(1.0);
        assert_relative_eq!(p1[0], 0.0, epsilon = 1e-12);
        assert_relative_eq!(p1[1], 1.0, epsilon = 1e-12);
    }

    #[test]
    fn quarter_circle_midpoint_on_unit_circle() {
        let c = quarter_circle();
        let pm = c.evaluate(0.5);
        let r = (pm[0] * pm[0] + pm[1] * pm[1]).sqrt();
        assert_relative_eq!(r, 1.0, epsilon = 1e-12);
    }

    #[test]
    fn unit_circle_points_on_circle() {
        let c = unit_circle();
        assert!(c.validate().is_ok());

        for i in 0..=20 {
            let t = i as f64 / 20.0;
            let n = c.control_points.len();
            let t_val = c.knots[c.degree] + t * (c.knots[n] - c.knots[c.degree]);
            let p = c.evaluate(t_val);
            let r = (p[0] * p[0] + p[1] * p[1]).sqrt();
            assert_relative_eq!(r, 1.0, epsilon = 1e-10, max_relative = 1e-10);
        }
    }

    #[test]
    fn unit_circle_area_from_adaptive_sample() {
        let c = unit_circle();
        let polygon = c.adaptive_sample(0.05);

        let n = polygon.len();
        assert!(n > 10, "expected many polygon vertices, got {}", n);
        let mut area = 0.0;
        for i in 0..n {
            let j = (i + 1) % n;
            area += polygon[i][0] * polygon[j][1];
            area -= polygon[j][0] * polygon[i][1];
        }
        let area = area.abs() * 0.5;

        assert_relative_eq!(area, PI, epsilon = 0.01);
    }

    #[test]
    fn sample_count() {
        let c = quarter_circle();
        let pts = c.sample(10);
        assert_eq!(pts.len(), 10);
    }

    #[test]
    fn evaluate_samples_matches_pointwise_evaluate() {
        let c = unit_circle();
        let ts = vec![0.0, 0.125, 0.25, 0.5, 0.75, 1.0];
        let mut out = Vec::new();
        c.evaluate_samples(&ts, &mut out);
        assert_eq!(out.len(), ts.len());
        for (idx, &t) in ts.iter().enumerate() {
            let expected = c.evaluate(t);
            assert_relative_eq!(out[idx][0], expected[0], epsilon = 1e-12);
            assert_relative_eq!(out[idx][1], expected[1], epsilon = 1e-12);
        }
    }

    #[test]
    #[should_panic(expected = "exceeds MAX_DEGREE")]
    fn evaluate_panics_above_max_degree() {
        let curve = NurbsCurve2D::new(11, vec![[0.0, 0.0]; 12], vec![0.0; 12 + 11 + 1]);
        let _ = curve.evaluate(0.5);
    }

    #[test]
    fn bounding_box_quarter_circle() {
        let c = quarter_circle();
        let (min, max) = c.bounding_box();
        assert_relative_eq!(min[0], 0.0, epsilon = 1e-12);
        assert_relative_eq!(min[1], 0.0, epsilon = 1e-12);
        assert_relative_eq!(max[0], 1.0, epsilon = 1e-12);
        assert_relative_eq!(max[1], 1.0, epsilon = 1e-12);
    }

    #[test]
    fn control_polygon_length_line() {
        let c = NurbsCurve2D::new(1, vec![[0.0, 0.0], [3.0, 4.0]], vec![0.0, 0.0, 1.0, 1.0]);
        assert_relative_eq!(c.control_polygon_length(), 5.0, epsilon = 1e-12);
    }

    #[test]
    fn insert_knot_uniform_preserves_shape() {
        let c = NurbsCurve2D::new(
            3,
            vec![[0.0, 0.0], [1.0, 2.0], [3.0, 3.0], [4.0, 1.0], [5.0, 0.0]],
            vec![0.0, 0.0, 0.0, 0.0, 0.5, 1.0, 1.0, 1.0, 1.0],
        );
        assert!(c.validate().is_ok());

        let c2 = c.insert_knot(0.3);
        assert!(c2.validate().is_ok());
        assert_eq!(c2.control_points.len(), c.control_points.len() + 1);

        for i in 0..=4 {
            let t = i as f64 / 4.0;
            let p_orig = c.evaluate(t);
            let p_new = c2.evaluate(t);
            assert_relative_eq!(p_orig[0], p_new[0], epsilon = 1e-10);
            assert_relative_eq!(p_orig[1], p_new[1], epsilon = 1e-10);
        }
    }

    #[test]
    fn insert_knot_rational_preserves_shape() {
        let c = quarter_circle();
        let c2 = c.insert_knot(0.5);
        assert!(c2.validate().is_ok());
        assert_curves_match_on_unit_interval(&c, &c2);
    }

    #[test]
    fn to_bezier_spans_single_span() {
        let c = quarter_circle();
        let spans = c.to_bezier_spans();
        assert_eq!(spans.len(), 1);
        assert_eq!(spans[0].control_points.len(), 3);

        for i in 0..=50 {
            let t = i as f64 / 50.0;
            let p_orig = c.evaluate(t);
            let p_span = spans[0].evaluate(t);
            assert_relative_eq!(p_orig[0], p_span[0], epsilon = 1e-10);
            assert_relative_eq!(p_orig[1], p_span[1], epsilon = 1e-10);
        }
    }

    #[test]
    fn to_bezier_spans_unit_circle() {
        let c = unit_circle();
        let spans = c.to_bezier_spans();
        assert_eq!(spans.len(), 4);

        for span in &spans {
            assert!(span.validate().is_ok());
            assert_eq!(span.control_points.len(), 3);
        }

        let n = c.control_points.len();
        let t_min = c.knots[c.degree];
        let t_max = c.knots[n];
        for i in 0..=50 {
            let t = t_min + (t_max - t_min) * i as f64 / 50.0;
            let p_orig = c.evaluate(t);

            let mut found = false;
            for span in &spans {
                let sn = span.control_points.len();
                let s_min = span.knots[span.degree];
                let s_max = span.knots[sn];
                if t >= s_min - 1e-14 && t <= s_max + 1e-14 {
                    let p_span = span.evaluate(t);
                    assert_relative_eq!(p_orig[0], p_span[0], epsilon = 1e-10);
                    assert_relative_eq!(p_orig[1], p_span[1], epsilon = 1e-10);
                    found = true;
                    break;
                }
            }
            assert!(found, "no span found for parameter t={}", t);
        }
    }

    #[test]
    fn to_bezier_spans_degree3_three_spans() {
        let c = NurbsCurve2D::new(
            3,
            vec![
                [0.0, 0.0],
                [1.0, 2.0],
                [2.0, 3.0],
                [3.0, 2.0],
                [4.0, 1.0],
                [5.0, 0.0],
            ],
            vec![0.0, 0.0, 0.0, 0.0, 0.33, 0.67, 1.0, 1.0, 1.0, 1.0],
        );
        assert!(c.validate().is_ok());

        let spans = c.to_bezier_spans();
        assert_eq!(spans.len(), 3);

        for span in &spans {
            assert!(span.validate().is_ok());
            assert_eq!(span.control_points.len(), 4);
        }

        for i in 0..=50 {
            let t = i as f64 / 50.0;
            let p_orig = c.evaluate(t);
            let mut found = false;
            for span in &spans {
                let sn = span.control_points.len();
                let s_min = span.knots[span.degree];
                let s_max = span.knots[sn];
                if t >= s_min - 1e-14 && t <= s_max + 1e-14 {
                    let p_span = span.evaluate(t);
                    assert_relative_eq!(p_orig[0], p_span[0], epsilon = 1e-10);
                    assert_relative_eq!(p_orig[1], p_span[1], epsilon = 1e-10);
                    found = true;
                    break;
                }
            }
            assert!(found, "no span found for parameter t={}", t);
        }
    }

    #[test]
    fn split_at_preserves_shape() {
        let c = quarter_circle();
        let t_split = 0.5;
        let (left, right) = c.split_at(t_split);

        assert!(left.validate().is_ok());
        assert!(right.validate().is_ok());

        let p_orig = c.evaluate(t_split);
        let n_left = left.control_points.len();
        let p_left_end = left.evaluate(left.knots[n_left]);
        let p_right_start = right.evaluate(right.knots[right.degree]);

        assert_relative_eq!(p_orig[0], p_left_end[0], epsilon = 1e-10);
        assert_relative_eq!(p_orig[1], p_left_end[1], epsilon = 1e-10);
        assert_relative_eq!(p_orig[0], p_right_start[0], epsilon = 1e-10);
        assert_relative_eq!(p_orig[1], p_right_start[1], epsilon = 1e-10);

        let t_mid = 0.25;
        let p_orig_mid = c.evaluate(t_mid);
        let p_left_mid = left.evaluate(t_mid);
        assert_relative_eq!(p_orig_mid[0], p_left_mid[0], epsilon = 1e-10);
        assert_relative_eq!(p_orig_mid[1], p_left_mid[1], epsilon = 1e-10);
    }

    #[test]
    fn reverse_endpoints() {
        let c = quarter_circle();
        let r = c.reverse();
        assert!(r.validate().is_ok());

        let p0 = c.evaluate(0.0);
        let p1 = c.evaluate(1.0);
        let r0 = r.evaluate(0.0);
        let r1 = r.evaluate(1.0);

        assert_relative_eq!(r0[0], p1[0], epsilon = 1e-10);
        assert_relative_eq!(r0[1], p1[1], epsilon = 1e-10);
        assert_relative_eq!(r1[0], p0[0], epsilon = 1e-10);
        assert_relative_eq!(r1[1], p0[1], epsilon = 1e-10);
    }

    #[test]
    fn reverse_intermediate_point() {
        let c = quarter_circle();
        let r = c.reverse();

        let p = c.evaluate(0.3);
        let rp = r.evaluate(0.7);
        assert_relative_eq!(p[0], rp[0], epsilon = 1e-10);
        assert_relative_eq!(p[1], rp[1], epsilon = 1e-10);
    }

    #[test]
    fn geometric_equivalence_after_insert_knot_dense_samples() {
        let c = NurbsCurve2D::new(
            3,
            vec![[0.0, 0.0], [1.0, 2.0], [3.0, 3.0], [4.0, 1.0], [5.0, 0.0]],
            vec![0.0, 0.0, 0.0, 0.0, 0.5, 1.0, 1.0, 1.0, 1.0],
        );
        assert!(c.validate().is_ok());

        let c2 = c.insert_knot(0.3);
        assert!(c2.validate().is_ok());
        assert_curves_match_on_unit_interval(&c, &c2);
    }

    #[test]
    fn geometric_equivalence_after_reverse_dense_samples() {
        let c = unit_circle();
        let r = c.reverse();
        assert!(r.validate().is_ok());

        for i in 0..=32 {
            let t = i as f64 / 32.0;
            let p = c.evaluate(t);
            let rp = r.evaluate(1.0 - t);
            assert_relative_eq!(p[0], rp[0], epsilon = 1e-10);
            assert_relative_eq!(p[1], rp[1], epsilon = 1e-10);
        }
    }

    #[test]
    fn translation_covariance_evaluate_dense_samples() {
        let c = quarter_circle();
        let delta = [2.5, -1.75];
        let translated = translated_curve(&c, delta);
        assert!(translated.validate().is_ok());

        for i in 0..=32 {
            let t = i as f64 / 32.0;
            let p = c.evaluate(t);
            let q = translated.evaluate(t);
            assert_relative_eq!(q[0], p[0] + delta[0], epsilon = 1e-10);
            assert_relative_eq!(q[1], p[1] + delta[1], epsilon = 1e-10);
        }
    }

    #[test]
    fn adaptive_sample_degree1_includes_corners() {
        let curve = NurbsCurve2D::new(
            1,
            vec![
                [0.3746, -0.3456],
                [3.2724, 0.4308],
                [2.6254, 2.8456],
                [-0.2724, 2.0692],
                [0.3746, -0.3456],
            ],
            vec![0.0, 0.0, 1.0, 2.0, 3.0, 4.0, 4.0],
        );
        let pts = curve.adaptive_sample(0.2);

        let corners = [
            [0.3746, -0.3456],
            [3.2724, 0.4308],
            [2.6254, 2.8456],
            [-0.2724, 2.0692],
        ];
        for (ci, corner) in corners.iter().enumerate() {
            let min_dist = pts
                .iter()
                .map(|p| dist(*p, *corner))
                .fold(f64::MAX, f64::min);
            assert!(min_dist < 1e-12, "Corner {ci} missing: dist={min_dist}");
        }
    }
}
