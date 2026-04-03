//! NURBS tensor-product surface evaluation and differentiation.

use crate::deboor::{
    deboor_1d_control_points_3d, deboor_1d_homogeneous_3d, find_knot_span, knot_insert_1d_3d,
};

const MAX_CONTROL_ROWS_STACK: usize = 64;

/// A rational NURBS tensor-product surface.
#[derive(Clone, Debug)]
pub struct NurbsSurface3D {
    pub degree_u: usize,
    pub degree_v: usize,
    /// Control point grid [n_u][n_v].
    pub control_points: Vec<Vec<[f64; 3]>>,
    /// Weight grid [n_u][n_v].
    pub weights: Vec<Vec<f64>>,
    /// U-direction knot vector (length = n_u + degree_u + 1).
    pub knots_u: Vec<f64>,
    /// V-direction knot vector (length = n_v + degree_v + 1).
    pub knots_v: Vec<f64>,
}

impl NurbsSurface3D {
    /// Validate that the surface parameters are consistent.
    pub fn validate(&self) -> Result<(), String> {
        let n_u = self.control_points.len();
        if n_u < self.degree_u + 1 {
            return Err(format!(
                "u-direction control point count {} is less than degree_u+1={}",
                n_u,
                self.degree_u + 1
            ));
        }
        if self.control_points.is_empty() {
            return Err("control point grid is empty".into());
        }
        let n_v = self.control_points[0].len();
        if n_v < self.degree_v + 1 {
            return Err(format!(
                "v-direction control point count {} is less than degree_v+1={}",
                n_v,
                self.degree_v + 1
            ));
        }
        for (i, row) in self.control_points.iter().enumerate() {
            if row.len() != n_v {
                return Err(format!(
                    "control point row {} has {} columns, expected {}",
                    i,
                    row.len(),
                    n_v
                ));
            }
        }
        if self.weights.len() != n_u {
            return Err(format!(
                "weight row count {} does not match control point row count {}",
                self.weights.len(),
                n_u
            ));
        }
        for (i, row) in self.weights.iter().enumerate() {
            if row.len() != n_v {
                return Err(format!(
                    "weight row {} has {} columns, expected {}",
                    i,
                    row.len(),
                    n_v
                ));
            }
        }
        let expected_knots_u = n_u + self.degree_u + 1;
        if self.knots_u.len() != expected_knots_u {
            return Err(format!(
                "u knot count {} does not match expected {}",
                self.knots_u.len(),
                expected_knots_u
            ));
        }
        let expected_knots_v = n_v + self.degree_v + 1;
        if self.knots_v.len() != expected_knots_v {
            return Err(format!(
                "v knot count {} does not match expected {}",
                self.knots_v.len(),
                expected_knots_v
            ));
        }
        Ok(())
    }

    /// Evaluate a point on the surface via tensor-product De Boor.
    pub fn evaluate(&self, u: f64, v: f64) -> [f64; 3] {
        let n_u = self.control_points.len();
        let mut stack_hx = [0.0; MAX_CONTROL_ROWS_STACK];
        let mut stack_hy = [0.0; MAX_CONTROL_ROWS_STACK];
        let mut stack_hz = [0.0; MAX_CONTROL_ROWS_STACK];
        let mut stack_hw = [0.0; MAX_CONTROL_ROWS_STACK];
        let mut heap_buffers = if n_u <= MAX_CONTROL_ROWS_STACK {
            None
        } else {
            Some((
                vec![0.0; n_u],
                vec![0.0; n_u],
                vec![0.0; n_u],
                vec![0.0; n_u],
            ))
        };

        let (inter_hx, inter_hy, inter_hz, inter_hw): (
            &mut [f64],
            &mut [f64],
            &mut [f64],
            &mut [f64],
        ) = if n_u <= MAX_CONTROL_ROWS_STACK {
            (
                &mut stack_hx[..n_u],
                &mut stack_hy[..n_u],
                &mut stack_hz[..n_u],
                &mut stack_hw[..n_u],
            )
        } else {
            let (heap_hx, heap_hy, heap_hz, heap_hw) =
                heap_buffers.as_mut().expect("heap buffers must exist");
            (
                &mut heap_hx[..],
                &mut heap_hy[..],
                &mut heap_hz[..],
                &mut heap_hw[..],
            )
        };

        let mut v_hint = self
            .degree_v
            .min(self.control_points[0].len().saturating_sub(1));
        for i in 0..n_u {
            let (rx, ry, rz, rw, span) = deboor_1d_control_points_3d(
                self.degree_v,
                &self.knots_v,
                &self.control_points[i],
                &self.weights[i],
                v,
                Some(v_hint),
            );
            v_hint = span;
            inter_hx[i] = rx;
            inter_hy[i] = ry;
            inter_hz[i] = rz;
            inter_hw[i] = rw;
        }

        let (fx, fy, fz, fw) = deboor_1d_homogeneous_3d(
            self.degree_u,
            &self.knots_u,
            inter_hx,
            inter_hy,
            inter_hz,
            inter_hw,
            u,
        );

        if fw.abs() < 1e-30 {
            return [0.0, 0.0, 0.0];
        }
        [fx / fw, fy / fw, fz / fw]
    }

    /// Partial derivative in the u-direction (central difference, h=1e-7).
    pub fn partial_u(&self, u: f64, v: f64) -> [f64; 3] {
        let h = 1e-7;
        let (u_min, u_max) = self.u_range();
        let u0 = (u - h).max(u_min);
        let u1 = (u + h).min(u_max);
        let dh = u1 - u0;
        if dh.abs() < 1e-30 {
            return [0.0, 0.0, 0.0];
        }
        let p0 = self.evaluate(u0, v);
        let p1 = self.evaluate(u1, v);
        [
            (p1[0] - p0[0]) / dh,
            (p1[1] - p0[1]) / dh,
            (p1[2] - p0[2]) / dh,
        ]
    }

    /// Partial derivative in the v-direction (central difference, h=1e-7).
    pub fn partial_v(&self, u: f64, v: f64) -> [f64; 3] {
        let h = 1e-7;
        let (v_min, v_max) = self.v_range();
        let v0 = (v - h).max(v_min);
        let v1 = (v + h).min(v_max);
        let dh = v1 - v0;
        if dh.abs() < 1e-30 {
            return [0.0, 0.0, 0.0];
        }
        let p0 = self.evaluate(u, v0);
        let p1 = self.evaluate(u, v1);
        [
            (p1[0] - p0[0]) / dh,
            (p1[1] - p0[1]) / dh,
            (p1[2] - p0[2]) / dh,
        ]
    }

    /// Surface normal (normalized cross product of partial derivatives).
    ///
    /// Returns `[0.0, 0.0, 0.0]` at degenerate points.
    pub fn normal(&self, u: f64, v: f64) -> [f64; 3] {
        let du = self.partial_u(u, v);
        let dv = self.partial_v(u, v);
        let nx = du[1] * dv[2] - du[2] * dv[1];
        let ny = du[2] * dv[0] - du[0] * dv[2];
        let nz = du[0] * dv[1] - du[1] * dv[0];
        let len = (nx * nx + ny * ny + nz * nz).sqrt();
        if len < 1e-30 {
            return [0.0, 0.0, 0.0];
        }
        [nx / len, ny / len, nz / len]
    }

    /// Valid parameter range in the u-direction.
    pub fn u_range(&self) -> (f64, f64) {
        let n_u = self.control_points.len();
        (self.knots_u[self.degree_u], self.knots_u[n_u])
    }

    /// Valid parameter range in the v-direction.
    pub fn v_range(&self) -> (f64, f64) {
        let n_v = self.control_points[0].len();
        (self.knots_v[self.degree_v], self.knots_v[n_v])
    }

    /// Insert a single knot in the u-direction (Boehm's algorithm).
    pub fn insert_knot_u(&self, t: f64) -> NurbsSurface3D {
        let p = self.degree_u;
        let n_u = self.control_points.len();
        let n_v = self.control_points[0].len();
        let k = find_knot_span(&self.knots_u, p, n_u, t);

        let mut new_knots_u = Vec::with_capacity(self.knots_u.len() + 1);
        new_knots_u.extend_from_slice(&self.knots_u[..=k]);
        new_knots_u.push(t);
        new_knots_u.extend_from_slice(&self.knots_u[k + 1..]);

        let mut new_cp = vec![vec![[0.0, 0.0, 0.0]; n_v]; n_u + 1];
        let mut new_w = vec![vec![0.0; n_v]; n_u + 1];

        for j in 0..n_v {
            let mut hx = Vec::with_capacity(n_u);
            let mut hy = Vec::with_capacity(n_u);
            let mut hz = Vec::with_capacity(n_u);
            let mut hw = Vec::with_capacity(n_u);
            for i in 0..n_u {
                let w = self.weights[i][j];
                hx.push(self.control_points[i][j][0] * w);
                hy.push(self.control_points[i][j][1] * w);
                hz.push(self.control_points[i][j][2] * w);
                hw.push(w);
            }

            let [nhx, nhy, nhz, nhw] =
                knot_insert_1d_3d(p, &self.knots_u, [&hx, &hy, &hz, &hw], k, t);

            for i in 0..n_u + 1 {
                let w = nhw[i];
                new_w[i][j] = w;
                new_cp[i][j] = [nhx[i] / w, nhy[i] / w, nhz[i] / w];
            }
        }

        NurbsSurface3D {
            degree_u: self.degree_u,
            degree_v: self.degree_v,
            control_points: new_cp,
            weights: new_w,
            knots_u: new_knots_u,
            knots_v: self.knots_v.clone(),
        }
    }

    /// Insert a single knot in the v-direction (Boehm's algorithm).
    pub fn insert_knot_v(&self, t: f64) -> NurbsSurface3D {
        let p = self.degree_v;
        let n_u = self.control_points.len();
        let n_v = self.control_points[0].len();
        let k = find_knot_span(&self.knots_v, p, n_v, t);

        let mut new_knots_v = Vec::with_capacity(self.knots_v.len() + 1);
        new_knots_v.extend_from_slice(&self.knots_v[..=k]);
        new_knots_v.push(t);
        new_knots_v.extend_from_slice(&self.knots_v[k + 1..]);

        let mut new_cp = vec![vec![[0.0, 0.0, 0.0]; n_v + 1]; n_u];
        let mut new_w = vec![vec![0.0; n_v + 1]; n_u];

        for i in 0..n_u {
            let mut hx = Vec::with_capacity(n_v);
            let mut hy = Vec::with_capacity(n_v);
            let mut hz = Vec::with_capacity(n_v);
            let mut hw = Vec::with_capacity(n_v);
            for j in 0..n_v {
                let w = self.weights[i][j];
                hx.push(self.control_points[i][j][0] * w);
                hy.push(self.control_points[i][j][1] * w);
                hz.push(self.control_points[i][j][2] * w);
                hw.push(w);
            }

            let [nhx, nhy, nhz, nhw] =
                knot_insert_1d_3d(p, &self.knots_v, [&hx, &hy, &hz, &hw], k, t);

            for j in 0..n_v + 1 {
                let w = nhw[j];
                new_w[i][j] = w;
                new_cp[i][j] = [nhx[j] / w, nhy[j] / w, nhz[j] / w];
            }
        }

        NurbsSurface3D {
            degree_u: self.degree_u,
            degree_v: self.degree_v,
            control_points: new_cp,
            weights: new_w,
            knots_u: self.knots_u.clone(),
            knots_v: new_knots_v,
        }
    }

    /// Axis-aligned bounding box from control points.
    pub fn aabb(&self) -> ([f64; 3], [f64; 3]) {
        let mut min = [f64::INFINITY; 3];
        let mut max = [f64::NEG_INFINITY; 3];

        for row in &self.control_points {
            for pt in row {
                for d in 0..3 {
                    min[d] = min[d].min(pt[d]);
                    max[d] = max[d].max(pt[d]);
                }
            }
        }

        (min, max)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_flat_surface() -> NurbsSurface3D {
        NurbsSurface3D {
            degree_u: 1,
            degree_v: 1,
            control_points: vec![
                vec![[0.0, 0.0, 0.0], [1.0, 0.0, 0.0]],
                vec![[0.0, 1.0, 0.0], [1.0, 1.0, 0.0]],
            ],
            weights: vec![vec![1.0, 1.0], vec![1.0, 1.0]],
            knots_u: vec![0.0, 0.0, 1.0, 1.0],
            knots_v: vec![0.0, 0.0, 1.0, 1.0],
        }
    }

    fn make_translated_surface(delta: [f64; 3]) -> NurbsSurface3D {
        let mut surface = make_flat_surface();
        for row in &mut surface.control_points {
            for point in row {
                point[0] += delta[0];
                point[1] += delta[1];
                point[2] += delta[2];
            }
        }
        surface
    }

    fn assert_points_close(actual: [f64; 3], expected: [f64; 3], tol: f64, context: &str) {
        assert!(
            (actual[0] - expected[0]).abs() < tol
                && (actual[1] - expected[1]).abs() < tol
                && (actual[2] - expected[2]).abs() < tol,
            "{}: {:?} vs {:?}",
            context,
            actual,
            expected
        );
    }

    fn assert_surfaces_match_on_grid(
        left: &NurbsSurface3D,
        right: &NurbsSurface3D,
        grid: &[f64],
        tol: f64,
        label: &str,
    ) {
        for &u in grid {
            for &v in grid {
                let lp = left.evaluate(u, v);
                let rp = right.evaluate(u, v);
                assert_points_close(lp, rp, tol, &format!("{} at u={}, v={}", label, u, v));
            }
        }
    }

    #[test]
    fn flat_surface_evaluate() {
        let s = make_flat_surface();
        let p00 = s.evaluate(0.0, 0.0);
        assert!((p00[0]).abs() < 1e-12);
        assert!((p00[1]).abs() < 1e-12);
        assert!((p00[2]).abs() < 1e-12);

        let p11 = s.evaluate(1.0, 1.0);
        assert!((p11[0] - 1.0).abs() < 1e-12);
        assert!((p11[1] - 1.0).abs() < 1e-12);
        assert!((p11[2]).abs() < 1e-12);

        let pm = s.evaluate(0.5, 0.5);
        assert!((pm[0] - 0.5).abs() < 1e-12);
        assert!((pm[1] - 0.5).abs() < 1e-12);
        assert!((pm[2]).abs() < 1e-12);
    }

    #[test]
    fn translation_covariance_evaluate() {
        let s = make_flat_surface();
        let delta = [1.25, -2.0, 0.75];
        let translated = make_translated_surface(delta);
        let grid = [0.0, 0.25, 0.5, 0.75, 1.0];

        for &u in &grid {
            for &v in &grid {
                let base = s.evaluate(u, v);
                let moved = translated.evaluate(u, v);
                let expected = [base[0] + delta[0], base[1] + delta[1], base[2] + delta[2]];
                assert_points_close(
                    moved,
                    expected,
                    1e-10,
                    &format!("translation covariance at u={}, v={}", u, v),
                );
            }
        }
    }

    #[test]
    fn insert_knot_u_preserves_shape() {
        let s = make_flat_surface();
        let s2 = s.insert_knot_u(0.5);
        for &u in &[0.0, 0.25, 0.5, 0.75, 1.0] {
            for &v in &[0.0, 0.5, 1.0] {
                let p1 = s.evaluate(u, v);
                let p2 = s2.evaluate(u, v);
                assert!(
                    (p1[0] - p2[0]).abs() < 1e-10
                        && (p1[1] - p2[1]).abs() < 1e-10
                        && (p1[2] - p2[2]).abs() < 1e-10,
                    "mismatch at u={}, v={}: {:?} vs {:?}",
                    u,
                    v,
                    p1,
                    p2
                );
            }
        }
    }

    #[test]
    fn insert_knot_dense_grid_equivalence() {
        let s = make_flat_surface();
        let grid = [0.0, 0.25, 0.5, 0.75, 1.0];

        let u_inserted = s.insert_knot_u(0.5);
        assert_surfaces_match_on_grid(&s, &u_inserted, &grid, 1e-10, "insert_knot_u");

        let v_inserted = s.insert_knot_v(0.3);
        assert_surfaces_match_on_grid(&s, &v_inserted, &grid, 1e-10, "insert_knot_v");
    }

    #[test]
    fn insert_knot_v_preserves_shape() {
        let s = make_flat_surface();
        let s2 = s.insert_knot_v(0.3);
        for &u in &[0.0, 0.5, 1.0] {
            for &v in &[0.0, 0.25, 0.5, 0.75, 1.0] {
                let p1 = s.evaluate(u, v);
                let p2 = s2.evaluate(u, v);
                assert!(
                    (p1[0] - p2[0]).abs() < 1e-10
                        && (p1[1] - p2[1]).abs() < 1e-10
                        && (p1[2] - p2[2]).abs() < 1e-10,
                    "mismatch at u={}, v={}: {:?} vs {:?}",
                    u,
                    v,
                    p1,
                    p2
                );
            }
        }
    }

    #[test]
    fn aabb_matches_control_points() {
        let s = make_flat_surface();
        let (min, max) = s.aabb();
        assert!((min[0]).abs() < 1e-12);
        assert!((min[1]).abs() < 1e-12);
        assert!((min[2]).abs() < 1e-12);
        assert!((max[0] - 1.0).abs() < 1e-12);
        assert!((max[1] - 1.0).abs() < 1e-12);
        assert!((max[2]).abs() < 1e-12);
    }

    #[test]
    fn validate_good_surface() {
        let s = make_flat_surface();
        assert!(s.validate().is_ok());
    }

    #[test]
    fn validate_bad_knots() {
        let mut s = make_flat_surface();
        s.knots_u = vec![0.0, 1.0]; // too few
        assert!(s.validate().is_err());
    }
}
