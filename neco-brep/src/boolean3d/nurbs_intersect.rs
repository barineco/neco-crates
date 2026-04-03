//! NurbsSurface x Plane / Quadric intersection.
//!
//! Fixes v, solves u as a rational Bezier polynomial, and traces branches into polylines.
//! Quadric uses implicit function sampling + bisection.

use neco_nurbs::NurbsSurface3D;

use crate::bezier_decompose::{decompose_to_bezier_patches, BezierPatch};
use crate::boolean3d::sweep_intersect::classify_branches;
use crate::brep::Surface;
use crate::vec3;
use neco_nurbs::solve_polynomial;

// ─── Constants ────────────────────────────────────────────

/// Minimum v sample count
const V_SAMPLES_MIN: usize = 120;

/// Maximum v sample count
const V_SAMPLES_MAX: usize = 720;

/// v samples per span
const V_SAMPLES_PER_SPAN: usize = 120;

/// Root parameter range tolerance
const ROOT_TOL: f64 = 1e-10;

/// u-direction samples for quadric intersection
const U_SAMPLES_QUADRIC: usize = 64;

/// Newton projection convergence tolerance
const NEWTON_CONVERGE_TOL: f64 = 1e-8;

/// Two-stage sampling: coarse pass count
const COARSE_SAMPLES: usize = 16;
/// Two-stage sampling: fine samples per interval
const FINE_SAMPLES_PER_INTERVAL: usize = 8;

/// Maximum bisection iterations
const BISECT_MAX_ITER: usize = 50;

// ─── Public API ────────────────────────────────────────

/// NurbsSurface x Plane intersection curves as polylines.
pub fn nurbs_plane_intersection(
    surface: &NurbsSurface3D,
    plane_origin: &[f64; 3],
    plane_normal: &[f64; 3],
) -> Vec<Vec<[f64; 3]>> {
    let n = plane_normal;
    let d = n[0] * plane_origin[0] + n[1] * plane_origin[1] + n[2] * plane_origin[2];

    // 1. Decompose into Bezier patches
    let patches = decompose_to_bezier_patches(surface);

    // 2. Prune by control-point sign test
    let active_patches: Vec<&BezierPatch> = patches
        .iter()
        .filter(|patch| {
            debug_assert!(
                patch.weights.iter().all(|row| row.iter().all(|&w| w > 0.0)),
                "all rational NURBS weights must be positive"
            );

            let mut has_positive = false;
            let mut has_negative = false;
            for row in &patch.control_points {
                for pt in row {
                    let val = n[0] * pt[0] + n[1] * pt[1] + n[2] * pt[2] - d;
                    if val > 0.0 {
                        has_positive = true;
                    } else if val < 0.0 {
                        has_negative = true;
                    } else {
                        return true;
                    }
                    if has_positive && has_negative {
                        return true;
                    }
                }
            }
            false
        })
        .collect();

    if active_patches.is_empty() {
        return vec![];
    }

    // 3. Determine v sample count
    let n_v_samples = v_sample_count(surface);

    // 4. Sweep v to collect intersections
    let v_min = surface.knots_v[surface.degree_v];
    let v_max = surface.knots_v[surface.control_points[0].len()];
    let v_range = v_max - v_min;

    let mut raw_points: Vec<(f64, f64, [f64; 3])> = Vec::new();

    for vi in 0..=n_v_samples {
        let v = v_min + v_range * (vi as f64 / n_v_samples as f64);

        for patch in &active_patches {
            let (u_lo, u_hi) = (patch.u_min, patch.u_max);

            // Compute u-direction Bernstein values via De Casteljau at fixed v
            let bernstein = bezier_plane_u_polynomial(patch, n, d, v);

            // Bernstein to power basis
            let power = bernstein_to_power_basis(&bernstein, u_lo, u_hi);

            // Find polynomial roots
            let roots = solve_polynomial(&power).expect("polynomial-highorder feature enabled");

            // Record roots within parameter range
            for root in roots {
                if root >= u_lo - ROOT_TOL && root <= u_hi + ROOT_TOL {
                    let u_clamped = root.clamp(u_lo, u_hi);
                    let pt = patch.evaluate(u_clamped, v);
                    raw_points.push((v, u_clamped, pt));
                }
            }
        }
    }

    // 5. Branch classification and polyline construction
    let char_len = estimate_char_len(surface);
    classify_branches(&raw_points, (v_min, v_max), char_len)
}

// ─── Helper functions ────────────────────────────────────

/// Compute u-direction Bernstein plane-distance coefficients at fixed v.
///
/// Uses numerator form w_i*(n*P_i - d) to eliminate W(u) > 0 denominator.
pub fn bezier_plane_u_polynomial(
    patch: &BezierPatch,
    normal: &[f64; 3],
    d: f64,
    v: f64,
) -> Vec<f64> {
    let q = patch.degree_v;
    let n_u = patch.degree_u + 1;

    // Normalize v to [0,1]
    let (v_lo, v_hi) = (patch.v_min, patch.v_max);
    let t = if (v_hi - v_lo).abs() < 1e-30 {
        0.0
    } else {
        ((v - v_lo) / (v_hi - v_lo)).clamp(0.0, 1.0)
    };

    let mut result = Vec::with_capacity(n_u);

    for i in 0..n_u {
        // De Casteljau along v-direction control points
        let cp_v = &patch.control_points[i];
        let w_v = &patch.weights[i];
        let (pt, w) = de_casteljau_3d(cp_v, w_v, q, t);

        // Numerator form: w * (n*P - d)
        let signed_dist = normal[0] * pt[0] + normal[1] * pt[1] + normal[2] * pt[2] - d;
        result.push(w * signed_dist);
    }

    result
}

/// De Casteljau evaluation of a rational Bezier curve.
/// Returns (point, weight) in dehomogenized coordinates.
pub fn de_casteljau_3d(
    pts: &[[f64; 3]],
    weights: &[f64],
    degree: usize,
    t: f64,
) -> ([f64; 3], f64) {
    debug_assert_eq!(pts.len(), degree + 1);
    debug_assert_eq!(weights.len(), degree + 1);

    // Copy to homogeneous coordinates
    let n = degree + 1;
    let mut hx: Vec<f64> = pts.iter().zip(weights).map(|(p, &w)| p[0] * w).collect();
    let mut hy: Vec<f64> = pts.iter().zip(weights).map(|(p, &w)| p[1] * w).collect();
    let mut hz: Vec<f64> = pts.iter().zip(weights).map(|(p, &w)| p[2] * w).collect();
    let mut hw: Vec<f64> = weights.to_vec();

    // De Casteljau recursion
    for r in 1..n {
        for j in (r..n).rev() {
            let s = 1.0 - t;
            hx[j] = s * hx[j - 1] + t * hx[j];
            hy[j] = s * hy[j - 1] + t * hy[j];
            hz[j] = s * hz[j - 1] + t * hz[j];
            hw[j] = s * hw[j - 1] + t * hw[j];
        }
    }

    let w = hw[n - 1];
    if w.abs() < 1e-30 {
        ([0.0, 0.0, 0.0], 0.0)
    } else {
        ([hx[n - 1] / w, hy[n - 1] / w, hz[n - 1] / w], w)
    }
}

/// Bernstein to power basis conversion.
pub fn bernstein_to_power_basis(bernstein: &[f64], u_min: f64, u_max: f64) -> Vec<f64> {
    let n = bernstein.len() - 1;
    if n == 0 {
        return bernstein.to_vec();
    }

    // First compute power basis on [0,1]
    let mut power_01 = vec![0.0; n + 1];
    for (k, power_coeff) in power_01.iter_mut().enumerate().take(n + 1) {
        let mut delta = 0.0;
        for (j, &bernstein_coeff) in bernstein.iter().enumerate().take(k + 1) {
            let sign = if (k - j) % 2 == 0 { 1.0 } else { -1.0 };
            delta += sign * binomial(k as u64, j as u64) as f64 * bernstein_coeff;
        }
        *power_coeff = binomial(n as u64, k as u64) as f64 * delta;
    }

    // Remap [0,1] -> [u_min, u_max]
    let span = u_max - u_min;
    if span.abs() < 1e-30 {
        return power_01;
    }

    let mut result = vec![0.0; n + 1];
    for (k, &power_coeff) in power_01.iter().enumerate().take(n + 1) {
        let coeff = power_coeff / span.powi(i32::try_from(k).expect("power fits in i32"));
        for (m, result_coeff) in result.iter_mut().enumerate().take(k + 1) {
            let binom = binomial(k as u64, m as u64) as f64;
            let shift_power = (-u_min).powi(i32::try_from(k - m).expect("power fits in i32"));
            *result_coeff += coeff * binom * shift_power;
        }
    }

    result
}

/// Binomial coefficient C(n, k). Safe for n <= 62.
pub fn binomial(n: u64, k: u64) -> u64 {
    if k > n {
        return 0;
    }
    if k == 0 || k == n {
        return 1;
    }
    let k = k.min(n - k) as u128;
    let n = n as u128;
    let mut result: u128 = 1;
    for i in 0..k {
        result = result * (n - i) / (i + 1);
    }
    result as u64
}

/// Determine v sample count, proportional to span count.
pub fn v_sample_count(surface: &NurbsSurface3D) -> usize {
    let q = surface.degree_v;
    let n_v = surface.control_points[0].len();
    let n_spans_v = if n_v > q { (n_v - 1) / q } else { 1 };

    let count = n_spans_v * V_SAMPLES_PER_SPAN;
    count.clamp(V_SAMPLES_MIN, V_SAMPLES_MAX)
}

/// Estimate characteristic length from AABB diagonal.
fn estimate_char_len(surface: &NurbsSurface3D) -> f64 {
    let (bb_min, bb_max) = surface.aabb();
    let dx = bb_max[0] - bb_min[0];
    let dy = bb_max[1] - bb_min[1];
    let dz = bb_max[2] - bb_min[2];
    (dx * dx + dy * dy + dz * dz).sqrt().max(0.01)
}

// ─── Quadric implicit ─────────────────────────────────

/// Evaluate quadric implicit function.
pub fn quadric_implicit(surface: &Surface, pt: &[f64; 3]) -> f64 {
    match surface {
        Surface::Sphere { center, radius } => {
            let dx = pt[0] - center[0];
            let dy = pt[1] - center[1];
            let dz = pt[2] - center[2];
            dx * dx + dy * dy + dz * dz - radius * radius
        }
        Surface::Ellipsoid { center, rx, ry, rz } => {
            let dx = pt[0] - center[0];
            let dy = pt[1] - center[1];
            let dz = pt[2] - center[2];
            dx * dx / (rx * rx) + dy * dy / (ry * ry) + dz * dz / (rz * rz) - 1.0
        }
        Surface::Cylinder {
            origin,
            axis,
            radius,
        } => {
            let a = vec3::normalized(*axis);
            let dx = pt[0] - origin[0];
            let dy = pt[1] - origin[1];
            let dz = pt[2] - origin[2];
            let h = dx * a[0] + dy * a[1] + dz * a[2];
            let perp_sq =
                (dx - h * a[0]).powi(2) + (dy - h * a[1]).powi(2) + (dz - h * a[2]).powi(2);
            perp_sq - radius * radius
        }
        Surface::Cone {
            origin,
            axis,
            half_angle,
        } => {
            let a = vec3::normalized(*axis);
            let dx = pt[0] - origin[0];
            let dy = pt[1] - origin[1];
            let dz = pt[2] - origin[2];
            let h = dx * a[0] + dy * a[1] + dz * a[2];
            let perp_sq =
                (dx - h * a[0]).powi(2) + (dy - h * a[1]).powi(2) + (dz - h * a[2]).powi(2);
            let tan_a = half_angle.tan();
            perp_sq - (h * tan_a).powi(2)
        }
        _ => unreachable!("quadric_implicit: non-quadric surface"),
    }
}

/// Prune patches by control-point sign test.
pub fn patch_may_intersect_quadric(patch: &BezierPatch, quadric: &Surface) -> bool {
    let mut has_positive = false;
    let mut has_negative = false;
    for row in &patch.control_points {
        for pt in row {
            let val = quadric_implicit(quadric, pt);
            if val > 0.0 {
                has_positive = true;
            } else if val < 0.0 {
                has_negative = true;
            } else {
                return true;
            }
            if has_positive && has_negative {
                return true;
            }
        }
    }
    false
}

/// Generic bisection for implicit zero along u at fixed v.
pub fn bisect_nurbs_implicit<F>(u_lo: f64, u_hi: f64, f_lo: f64, implicit_fn: &F) -> f64
where
    F: Fn(f64) -> f64,
{
    let mut lo = u_lo;
    let mut hi = u_hi;
    let mut f_l = f_lo;

    for _ in 0..BISECT_MAX_ITER {
        let mid = 0.5 * (lo + hi);
        if (hi - lo) < 1e-14 {
            return mid;
        }
        let f_mid = implicit_fn(mid);
        if f_mid.is_nan() {
            return mid;
        }
        if (f_l < 0.0 && f_mid < 0.0) || (f_l > 0.0 && f_mid > 0.0) {
            lo = mid;
            f_l = f_mid;
        } else {
            hi = mid;
        }
    }
    0.5 * (lo + hi)
}

/// NurbsSurface x Quadric intersection curves as polylines.
pub fn nurbs_quadric_intersection(
    surface: &NurbsSurface3D,
    quadric: &Surface,
) -> Vec<Vec<[f64; 3]>> {
    // 1. Decompose into Bezier patches
    let patches = decompose_to_bezier_patches(surface);

    // 2. Prune by control-point sign test
    let active_patches: Vec<&BezierPatch> = patches
        .iter()
        .filter(|patch| patch_may_intersect_quadric(patch, quadric))
        .collect();

    if active_patches.is_empty() {
        return vec![];
    }

    // 3. Determine v sample count
    let n_v_samples = v_sample_count(surface);

    // 4. v sweep + u sampling + bisection
    let v_min = surface.knots_v[surface.degree_v];
    let v_max = surface.knots_v[surface.control_points[0].len()];
    let v_range = v_max - v_min;

    let mut raw_points: Vec<(f64, f64, [f64; 3])> = Vec::new();

    for vi in 0..=n_v_samples {
        let v = v_min + v_range * (vi as f64 / n_v_samples as f64);

        for patch in &active_patches {
            let (u_lo, u_hi) = (patch.u_min, patch.u_max);
            let u_span = u_hi - u_lo;

            let n_u = U_SAMPLES_QUADRIC;
            let mut prev_val = {
                let pt = patch.evaluate(u_lo, v);
                quadric_implicit(quadric, &pt)
            };

            for ui in 1..=n_u {
                let u = u_lo + u_span * (ui as f64 / n_u as f64);
                let pt = patch.evaluate(u, v);
                let val = quadric_implicit(quadric, &pt);

                if (prev_val < 0.0 && val > 0.0) || (prev_val > 0.0 && val < 0.0) {
                    let u_prev = u_lo + u_span * ((ui - 1) as f64 / n_u as f64);
                    let implicit_fn = |u_param: f64| -> f64 {
                        let p = patch.evaluate(u_param, v);
                        quadric_implicit(quadric, &p)
                    };
                    let u_root = bisect_nurbs_implicit(u_prev, u, prev_val, &implicit_fn);
                    let pt_root = patch.evaluate(u_root, v);
                    raw_points.push((v, u_root, pt_root));
                }

                prev_val = val;
            }
        }
    }

    // 5. Branch classification and polyline construction
    let char_len = estimate_char_len(surface);
    classify_branches(&raw_points, (v_min, v_max), char_len)
}

// ─── Torus implicit ─────────────────────────────────

/// Evaluate torus implicit function.
pub fn torus_implicit(
    pt: &[f64; 3],
    center: &[f64; 3],
    axis: &[f64; 3],
    major_r: f64,
    minor_r: f64,
) -> f64 {
    let a = vec3::normalized(*axis);
    let dx = pt[0] - center[0];
    let dy = pt[1] - center[1];
    let dz = pt[2] - center[2];
    let h = dx * a[0] + dy * a[1] + dz * a[2];
    let perp_x = dx - h * a[0];
    let perp_y = dy - h * a[1];
    let perp_z = dz - h * a[2];
    let rho = (perp_x * perp_x + perp_y * perp_y + perp_z * perp_z).sqrt();
    (rho - major_r).powi(2) + h * h - minor_r * minor_r
}

/// Prune patches by AABB-torus distance.
pub fn patch_may_intersect_torus(
    patch: &BezierPatch,
    center: &[f64; 3],
    axis: &[f64; 3],
    major_r: f64,
    minor_r: f64,
) -> bool {
    let (bb_min, bb_max) = patch.aabb();

    let cx = 0.5 * (bb_min[0] + bb_max[0]);
    let cy = 0.5 * (bb_min[1] + bb_max[1]);
    let cz = 0.5 * (bb_min[2] + bb_max[2]);

    let hx = 0.5 * (bb_max[0] - bb_min[0]);
    let hy = 0.5 * (bb_max[1] - bb_min[1]);
    let hz = 0.5 * (bb_max[2] - bb_min[2]);
    let half_diag = (hx * hx + hy * hy + hz * hz).sqrt();

    let a = vec3::normalized(*axis);
    let dx = cx - center[0];
    let dy = cy - center[1];
    let dz = cz - center[2];
    let h = dx * a[0] + dy * a[1] + dz * a[2];
    let perp_x = dx - h * a[0];
    let perp_y = dy - h * a[1];
    let perp_z = dz - h * a[2];
    let rho = (perp_x * perp_x + perp_y * perp_y + perp_z * perp_z).sqrt();

    let dist_to_tube = ((rho - major_r).powi(2) + h * h).sqrt();

    dist_to_tube <= minor_r + half_diag
}

/// NurbsSurface x Torus intersection curves as polylines.
pub fn nurbs_torus_intersection(
    surface: &NurbsSurface3D,
    center: &[f64; 3],
    axis: &[f64; 3],
    major_r: f64,
    minor_r: f64,
) -> Vec<Vec<[f64; 3]>> {
    // 1. Decompose into Bezier patches
    let patches = decompose_to_bezier_patches(surface);

    // 2. Prune by AABB-torus distance
    let active_patches: Vec<&BezierPatch> = patches
        .iter()
        .filter(|patch| patch_may_intersect_torus(patch, center, axis, major_r, minor_r))
        .collect();

    if active_patches.is_empty() {
        return vec![];
    }

    // 3. Determine v sample count
    let n_v_samples = v_sample_count(surface);

    // 4. v sweep + u sampling + bisection
    let v_min = surface.knots_v[surface.degree_v];
    let v_max = surface.knots_v[surface.control_points[0].len()];
    let v_range = v_max - v_min;

    let mut raw_points: Vec<(f64, f64, [f64; 3])> = Vec::new();

    for vi in 0..=n_v_samples {
        let v = v_min + v_range * (vi as f64 / n_v_samples as f64);

        for patch in &active_patches {
            let (u_lo, u_hi) = (patch.u_min, patch.u_max);
            let u_span = u_hi - u_lo;

            let n_u = U_SAMPLES_QUADRIC;
            let mut prev_val = {
                let pt = patch.evaluate(u_lo, v);
                torus_implicit(&pt, center, axis, major_r, minor_r)
            };

            for ui in 1..=n_u {
                let u = u_lo + u_span * (ui as f64 / n_u as f64);
                let pt = patch.evaluate(u, v);
                let val = torus_implicit(&pt, center, axis, major_r, minor_r);

                if (prev_val < 0.0 && val > 0.0) || (prev_val > 0.0 && val < 0.0) {
                    let u_prev = u_lo + u_span * ((ui - 1) as f64 / n_u as f64);
                    let implicit_fn = |u_param: f64| -> f64 {
                        let p = patch.evaluate(u_param, v);
                        torus_implicit(&p, center, axis, major_r, minor_r)
                    };
                    let u_root = bisect_nurbs_implicit(u_prev, u, prev_val, &implicit_fn);
                    let pt_root = patch.evaluate(u_root, v);
                    raw_points.push((v, u_root, pt_root));
                }

                prev_val = val;
            }
        }
    }

    // 5. Branch classification and polyline construction
    let char_len = estimate_char_len(surface);
    classify_branches(&raw_points, (v_min, v_max), char_len)
}

// ─── NurbsSurface x NurbsSurface intersection ─────

/// Test whether two AABBs overlap.
pub fn aabb_overlap(
    a_min: &[f64; 3],
    a_max: &[f64; 3],
    b_min: &[f64; 3],
    b_max: &[f64; 3],
) -> bool {
    a_min[0] <= b_max[0]
        && a_max[0] >= b_min[0]
        && a_min[1] <= b_max[1]
        && a_max[1] >= b_min[1]
        && a_min[2] <= b_max[2]
        && a_max[2] >= b_min[2]
}

/// 3-variable Newton for NurbsSurface x NurbsSurface.
pub fn newton_nurbs_nurbs(
    surf_a: &NurbsSurface3D,
    surf_b: &NurbsSurface3D,
    u_a: f64,
    v_a: f64,
    u_b: f64,
    v_b: f64,
) -> Option<[f64; 3]> {
    let (ua_min, ua_max) = surf_a.u_range();
    let (ub_min, ub_max) = surf_b.u_range();
    let (vb_min, vb_max) = surf_b.v_range();

    let mut ua = u_a;
    let mut ub = u_b;
    let mut vb = v_b;

    for _ in 0..20 {
        let pa = surf_a.evaluate(ua, v_a);
        let pb = surf_b.evaluate(ub, vb);

        let rx = pa[0] - pb[0];
        let ry = pa[1] - pb[1];
        let rz = pa[2] - pb[2];

        if (rx * rx + ry * ry + rz * rz).sqrt() < 1e-6 {
            return Some(pa);
        }

        let da_u = surf_a.partial_u(ua, v_a);
        let db_u = surf_b.partial_u(ub, vb);
        let db_v = surf_b.partial_v(ub, vb);

        let j = [
            [da_u[0], -db_u[0], -db_v[0]],
            [da_u[1], -db_u[1], -db_v[1]],
            [da_u[2], -db_u[2], -db_v[2]],
        ];

        let mut jtj = [[0.0f64; 3]; 3];
        for i in 0..3 {
            for k in 0..3 {
                jtj[i][k] = j[0][i] * j[0][k] + j[1][i] * j[1][k] + j[2][i] * j[2][k];
            }
        }

        let r = [rx, ry, rz];
        let mut jtr = [0.0f64; 3];
        for i in 0..3 {
            jtr[i] = j[0][i] * r[0] + j[1][i] * r[1] + j[2][i] * r[2];
        }

        let det = det3(&jtj);
        if det.abs() < 1e-30 {
            return None;
        }

        let d0 = det3_col(&jtj, &jtr, 0);
        let d1 = det3_col(&jtj, &jtr, 1);
        let d2 = det3_col(&jtj, &jtr, 2);

        let delta_ua = d0 / det;
        let delta_ub = d1 / det;
        let delta_vb = d2 / det;

        ua = (ua - delta_ua).clamp(ua_min, ua_max);
        ub = (ub - delta_ub).clamp(ub_min, ub_max);
        vb = (vb - delta_vb).clamp(vb_min, vb_max);
    }

    // Convergence check
    let pa = surf_a.evaluate(ua, v_a);
    let pb = surf_b.evaluate(ub, vb);
    let dx = pa[0] - pb[0];
    let dy = pa[1] - pb[1];
    let dz = pa[2] - pb[2];
    if (dx * dx + dy * dy + dz * dz).sqrt() < 1e-6 {
        Some(pa)
    } else {
        None
    }
}

/// 3x3 determinant.
fn det3(m: &[[f64; 3]; 3]) -> f64 {
    m[0][0] * (m[1][1] * m[2][2] - m[1][2] * m[2][1])
        - m[0][1] * (m[1][0] * m[2][2] - m[1][2] * m[2][0])
        + m[0][2] * (m[1][0] * m[2][1] - m[1][1] * m[2][0])
}

/// Cramer's rule: determinant with column replaced by rhs.
fn det3_col(m: &[[f64; 3]; 3], rhs: &[f64; 3], col: usize) -> f64 {
    let mut tmp = *m;
    for row in 0..3 {
        tmp[row][col] = rhs[row];
    }
    det3(&tmp)
}

/// Precomputed grid for fast nearest-neighbor projection.
struct SurfaceGrid {
    points: Vec<(f64, f64, [f64; 3])>,
}

impl SurfaceGrid {
    fn build(surface: &NurbsSurface3D, n: usize) -> Self {
        let (u_min, u_max) = surface.u_range();
        let (v_min, v_max) = surface.v_range();
        let mut points = Vec::with_capacity((n + 1) * (n + 1));
        for i in 0..=n {
            let u = u_min + (u_max - u_min) * (i as f64 / n as f64);
            for j in 0..=n {
                let v = v_min + (v_max - v_min) * (j as f64 / n as f64);
                let pt = surface.evaluate(u, v);
                points.push((u, v, pt));
            }
        }
        Self { points }
    }

    fn nearest_uv(&self, target: &[f64; 3]) -> (f64, f64) {
        let mut best_idx = 0;
        let mut best_dist_sq = f64::INFINITY;
        for (idx, (_, _, pt)) in self.points.iter().enumerate() {
            let dx = pt[0] - target[0];
            let dy = pt[1] - target[1];
            let dz = pt[2] - target[2];
            let d2 = dx * dx + dy * dy + dz * dz;
            if d2 < best_dist_sq {
                best_dist_sq = d2;
                best_idx = idx;
            }
        }
        let (u, v, _) = self.points[best_idx];
        (u, v)
    }
}

/// Newton projection onto surface. Returns (u, v).
fn project_to_surface_newton(
    surface: &NurbsSurface3D,
    target: &[f64; 3],
    u_init: f64,
    v_init: f64,
) -> (f64, f64) {
    let (u_min, u_max) = surface.u_range();
    let (v_min, v_max) = surface.v_range();

    let mut u = u_init;
    let mut v = v_init;

    for _ in 0..20 {
        let pt = surface.evaluate(u, v);
        let du = surface.partial_u(u, v);
        let dv = surface.partial_v(u, v);

        let rx = pt[0] - target[0];
        let ry = pt[1] - target[1];
        let rz = pt[2] - target[2];

        let a11 = du[0] * du[0] + du[1] * du[1] + du[2] * du[2];
        let a12 = du[0] * dv[0] + du[1] * dv[1] + du[2] * dv[2];
        let a22 = dv[0] * dv[0] + dv[1] * dv[1] + dv[2] * dv[2];
        let b1 = du[0] * rx + du[1] * ry + du[2] * rz;
        let b2 = dv[0] * rx + dv[1] * ry + dv[2] * rz;

        let det = a11 * a22 - a12 * a12;
        if det.abs() < 1e-30 {
            break;
        }

        let delta_u = (a22 * b1 - a12 * b2) / det;
        let delta_v = (a11 * b2 - a12 * b1) / det;

        u = (u - delta_u).clamp(u_min, u_max);
        v = (v - delta_v).clamp(v_min, v_max);

        if delta_u.abs() < NEWTON_CONVERGE_TOL && delta_v.abs() < NEWTON_CONVERGE_TOL {
            break;
        }
    }

    (u, v)
}

/// NurbsSurface x NurbsSurface intersection curves as polylines.
pub fn nurbs_nurbs_intersection(
    surf_a: &NurbsSurface3D,
    surf_b: &NurbsSurface3D,
) -> Vec<Vec<[f64; 3]>> {
    // 1. Decompose into Bezier patches
    let patches_a = decompose_to_bezier_patches(surf_a);
    let patches_b = decompose_to_bezier_patches(surf_b);

    // 2. Prune patch pairs by AABB overlap
    let mut active_pairs: Vec<(usize, usize)> = Vec::new();
    let aabbs_a: Vec<([f64; 3], [f64; 3])> = patches_a.iter().map(|p| p.aabb()).collect();
    let aabbs_b: Vec<([f64; 3], [f64; 3])> = patches_b.iter().map(|p| p.aabb()).collect();

    for (ia, (a_min, a_max)) in aabbs_a.iter().enumerate() {
        for (ib, (b_min, b_max)) in aabbs_b.iter().enumerate() {
            if aabb_overlap(a_min, a_max, b_min, b_max) {
                active_pairs.push((ia, ib));
            }
        }
    }

    if active_pairs.is_empty() {
        return vec![];
    }

    // Precompute grid for surf_b
    let grid_b = SurfaceGrid::build(surf_b, 32);

    // 3. Sweep v_a
    let n_v_samples = v_sample_count(surf_a);
    let v_min = surf_a.knots_v[surf_a.degree_v];
    let v_max = surf_a.knots_v[surf_a.control_points[0].len()];
    let v_range = v_max - v_min;

    let char_len = estimate_char_len(surf_a);
    let warm_start_tol_sq = (char_len * 0.1) * (char_len * 0.1);

    let mut raw_points: Vec<(f64, f64, [f64; 3])> = Vec::new();

    for vi in 0..=n_v_samples {
        let v_a = v_min + v_range * (vi as f64 / n_v_samples as f64);

        for &(ia, _ib) in &active_pairs {
            let patch_a = &patches_a[ia];
            let (u_lo, u_hi) = (patch_a.u_min, patch_a.u_max);
            let u_span = u_hi - u_lo;

            let mut prev_uv_b: Option<(f64, f64)> = None;

            let compute_sd = |u_a: f64, prev: Option<(f64, f64)>| -> (f64, f64, f64) {
                let pt_a = patch_a.evaluate(u_a, v_a);

                let (u_b, v_b) = if let Some((pu, pv)) = prev {
                    let (u_b_w, v_b_w) = project_to_surface_newton(surf_b, &pt_a, pu, pv);
                    let pt_b_w = surf_b.evaluate(u_b_w, v_b_w);
                    let dx = pt_a[0] - pt_b_w[0];
                    let dy = pt_a[1] - pt_b_w[1];
                    let dz = pt_a[2] - pt_b_w[2];
                    if dx * dx + dy * dy + dz * dz < warm_start_tol_sq {
                        (u_b_w, v_b_w)
                    } else {
                        let (u_b_g, v_b_g) = grid_b.nearest_uv(&pt_a);
                        project_to_surface_newton(surf_b, &pt_a, u_b_g, v_b_g)
                    }
                } else {
                    let (u_b_g, v_b_g) = grid_b.nearest_uv(&pt_a);
                    project_to_surface_newton(surf_b, &pt_a, u_b_g, v_b_g)
                };

                let pt_b = surf_b.evaluate(u_b, v_b);
                let nb = surf_b.normal(u_b, v_b);
                let nb_len = (nb[0] * nb[0] + nb[1] * nb[1] + nb[2] * nb[2]).sqrt();
                let diff_x = pt_a[0] - pt_b[0];
                let diff_y = pt_a[1] - pt_b[1];
                let diff_z = pt_a[2] - pt_b[2];
                let sd = if nb_len > 1e-15 {
                    (diff_x * nb[0] + diff_y * nb[1] + diff_z * nb[2]) / nb_len
                } else {
                    (diff_x * diff_x + diff_y * diff_y + diff_z * diff_z).sqrt()
                };
                (sd, u_b, v_b)
            };

            // Coarse pass
            let mut coarse_sd = Vec::with_capacity(COARSE_SAMPLES + 1);
            let mut coarse_uv_b = Vec::with_capacity(COARSE_SAMPLES + 1);

            for ci in 0..=COARSE_SAMPLES {
                let u_a = u_lo + u_span * (ci as f64 / COARSE_SAMPLES as f64);
                let (sd, u_b, v_b) = compute_sd(u_a, prev_uv_b);
                coarse_sd.push(sd);
                coarse_uv_b.push((u_b, v_b));
                prev_uv_b = Some((u_b, v_b));
            }

            // Identify candidate intervals
            let mut candidate_intervals = [false; COARSE_SAMPLES];
            for ci in 0..COARSE_SAMPLES {
                if (coarse_sd[ci] < 0.0 && coarse_sd[ci + 1] > 0.0)
                    || (coarse_sd[ci] > 0.0 && coarse_sd[ci + 1] < 0.0)
                {
                    candidate_intervals[ci] = true;
                    if ci > 0 {
                        candidate_intervals[ci - 1] = true;
                    }
                    if ci + 1 < COARSE_SAMPLES {
                        candidate_intervals[ci + 1] = true;
                    }
                }
            }

            // Fine pass
            for ci in 0..COARSE_SAMPLES {
                if !candidate_intervals[ci] {
                    continue;
                }

                let u_left = u_lo + u_span * (ci as f64 / COARSE_SAMPLES as f64);
                let u_right = u_lo + u_span * ((ci + 1) as f64 / COARSE_SAMPLES as f64);
                let fine_span = u_right - u_left;

                let mut fine_prev_uv_b = Some(coarse_uv_b[ci]);
                let mut prev_fine_sd = coarse_sd[ci];

                for fi in 1..=FINE_SAMPLES_PER_INTERVAL {
                    let u_a = u_left + fine_span * (fi as f64 / FINE_SAMPLES_PER_INTERVAL as f64);
                    let (sd, u_b, v_b) = compute_sd(u_a, fine_prev_uv_b);
                    fine_prev_uv_b = Some((u_b, v_b));

                    if (prev_fine_sd < 0.0 && sd > 0.0) || (prev_fine_sd > 0.0 && sd < 0.0) {
                        let u_prev_fine = u_left
                            + fine_span * ((fi - 1) as f64 / FINE_SAMPLES_PER_INTERVAL as f64);
                        let u_mid = 0.5 * (u_prev_fine + u_a);
                        let pt_mid = patch_a.evaluate(u_mid, v_a);

                        let (u_b_init, v_b_init) = {
                            let Some((pu, pv)) = fine_prev_uv_b else {
                                continue;
                            };
                            project_to_surface_newton(surf_b, &pt_mid, pu, pv)
                        };

                        if let Some(pt_int) =
                            newton_nurbs_nurbs(surf_a, surf_b, u_mid, v_a, u_b_init, v_b_init)
                        {
                            raw_points.push((v_a, u_mid, pt_int));
                        }
                    }

                    prev_fine_sd = sd;
                }
            }
        }
    }

    // Deduplicate
    raw_points.sort_by(|a, b| a.0.total_cmp(&b.0).then(a.1.total_cmp(&b.1)));
    let dedup_tol = char_len * 0.01;
    let mut deduped: Vec<(f64, f64, [f64; 3])> = Vec::new();
    for p in &raw_points {
        let dominated = deduped.last().is_some_and(|last| {
            (last.0 - p.0).abs() < 1e-15 && {
                let dx = last.2[0] - p.2[0];
                let dy = last.2[1] - p.2[1];
                let dz = last.2[2] - p.2[2];
                (dx * dx + dy * dy + dz * dz).sqrt() < dedup_tol
            }
        });
        if !dominated {
            deduped.push(*p);
        }
    }

    // 4. Branch classification and polyline construction
    classify_branches(&deduped, (v_min, v_max), char_len)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn binomial_basic() {
        assert_eq!(binomial(0, 0), 1);
        assert_eq!(binomial(5, 0), 1);
        assert_eq!(binomial(5, 5), 1);
        assert_eq!(binomial(5, 2), 10);
        assert_eq!(binomial(10, 3), 120);
        assert_eq!(binomial(62, 31), 465428353255261088);
    }

    #[test]
    fn bernstein_to_power_identity() {
        let b = vec![1.0, 0.0];
        let p = bernstein_to_power_basis(&b, 0.0, 1.0);
        assert!((p[0] - 1.0).abs() < 1e-12);
        assert!((p[1] - (-1.0)).abs() < 1e-12);
    }

    #[test]
    fn de_casteljau_endpoints() {
        let pts = vec![[0.0, 0.0, 0.0], [1.0, 1.0, 0.0], [2.0, 0.0, 0.0]];
        let weights = vec![1.0, 1.0, 1.0];

        let (p0, _) = de_casteljau_3d(&pts, &weights, 2, 0.0);
        assert!((p0[0] - 0.0).abs() < 1e-12);
        assert!((p0[1] - 0.0).abs() < 1e-12);

        let (p1, _) = de_casteljau_3d(&pts, &weights, 2, 1.0);
        assert!((p1[0] - 2.0).abs() < 1e-12);
        assert!((p1[1] - 0.0).abs() < 1e-12);
    }
}
