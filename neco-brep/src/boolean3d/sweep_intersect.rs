/// Common framework for Quadric x Quadric intersection via parameter sweep.
///
/// Parametrize one surface, substitute into the other's implicit function,
/// and solve by sweeping theta.
use std::f64::consts::TAU;

use crate::boolean3d::intersect3d::{classify_axes, clip_polyline_to_both_faces};
use crate::boolean3d::BooleanEvent;
use crate::brep::{Curve3D, Face, Shell, Surface};
use crate::vec3;
use neco_nurbs::solve_polynomial;
use neco_nurbs::solve_quadratic;

/// Discriminant threshold
const DISC_EPS: f64 = 1e-12;

/// Closed-loop detection distance threshold
const CLOSE_LOOP_TOL: f64 = 1e-4;

/// Adaptive refinement angle threshold (radians): 5 deg
const ANGLE_THRESHOLD: f64 = 5.0 * std::f64::consts::PI / 180.0;

/// Maximum adaptive refinement depth
const MAX_REFINE_DEPTH: usize = 10;

/// Initial subdivision count
const N_THETA_INITIAL: usize = 360;

/// Newton refinement max iterations
const NEWTON_MAX_ITER: usize = 8;

/// Newton refinement convergence threshold
const NEWTON_TOL: f64 = 1e-14;

type SweepPoint = (f64, [f64; 3]);
type ThetaGroup = (f64, Vec<SweepPoint>);

// ─── Core 1: sweep_quadric_intersection ──────────────

/// Construct intersection curves via parameter sweep.
///
/// Parametrize one surface as (theta, h), substitute into the other's implicit
/// to get alpha*h^2 + 2*beta*h + gamma = 0, sweep theta and solve for h.
pub fn sweep_quadric_intersection<F, E, H>(
    coeff_fn: F,
    eval_fn: E,
    theta_range: (f64, f64),
    h_bounds: H,
    char_len: f64,
) -> Vec<Vec<[f64; 3]>>
where
    F: Fn(f64) -> (f64, f64, f64),
    E: Fn(f64, f64) -> [f64; 3],
    H: Fn(f64) -> (f64, f64),
{
    // 1. Initial sampling
    let n = N_THETA_INITIAL;
    let (t0, t1) = theta_range;
    let dt = (t1 - t0) / n as f64;

    let mut samples: Vec<(f64, Vec<f64>)> = Vec::with_capacity(n + 1);
    for i in 0..=n {
        let theta = t0 + dt * i as f64;
        let hs = solve_h_at_theta(&coeff_fn, &h_bounds, theta);
        samples.push((theta, hs));
    }

    // 2. Refine discriminant zeros
    refine_discriminant_zeros(&coeff_fn, &h_bounds, &mut samples);

    // 3. Adaptive refinement
    adaptive_refine_quadric(&coeff_fn, &eval_fn, &h_bounds, &mut samples);

    // 4. Branch tracing and point collection
    let mut raw_points: Vec<(f64, f64, [f64; 3])> = Vec::new();
    for (theta, hs) in &samples {
        for &h in hs {
            let pt = eval_fn(*theta, h);
            raw_points.push((*theta, h, pt));
        }
    }

    classify_branches(&raw_points, theta_range, char_len)
}

/// Solve alpha*h^2 + 2*beta*h + gamma = 0 at each theta
fn solve_h_at_theta<F, H>(coeff_fn: &F, h_bounds: &H, theta: f64) -> Vec<f64>
where
    F: Fn(f64) -> (f64, f64, f64),
    H: Fn(f64) -> (f64, f64),
{
    let (alpha, beta, gamma) = coeff_fn(theta);
    let (h_min, h_max) = h_bounds(theta);

    // αh² + 2βh + γ = 0
    let roots = solve_quadratic(alpha, 2.0 * beta, gamma);

    // Apply Newton refinement near tangent points, return only in-range solutions
    roots
        .into_iter()
        .filter_map(|h| {
            let h_refined = newton_refine_h(alpha, beta, gamma, h);
            if h_refined >= h_min - DISC_EPS && h_refined <= h_max + DISC_EPS {
                Some(h_refined.clamp(h_min, h_max))
            } else {
                None
            }
        })
        .collect()
}

/// Newton refinement for small discriminant.
/// f(h) = αh² + 2βh + γ, f'(h) = 2αh + 2β
fn newton_refine_h(alpha: f64, beta: f64, gamma: f64, h0: f64) -> f64 {
    if alpha.abs() < DISC_EPS {
        return h0;
    }
    let mut h = h0;
    for _ in 0..NEWTON_MAX_ITER {
        let f = alpha * h * h + 2.0 * beta * h + gamma;
        let fp = 2.0 * alpha * h + 2.0 * beta;
        if fp.abs() < NEWTON_TOL {
            break;
        }
        let dh = f / fp;
        h -= dh;
        if dh.abs() < NEWTON_TOL {
            break;
        }
    }
    h
}

/// Refine discriminant zeros: detect sign changes between adjacent samples and bisect.
fn refine_discriminant_zeros<F, H>(coeff_fn: &F, h_bounds: &H, samples: &mut Vec<(f64, Vec<f64>)>)
where
    F: Fn(f64) -> (f64, f64, f64),
    H: Fn(f64) -> (f64, f64),
{
    let mut insertions: Vec<(usize, f64, Vec<f64>)> = Vec::new();

    for i in 0..samples.len() - 1 {
        let theta_lo = samples[i].0;
        let theta_hi = samples[i + 1].0;
        let disc_lo = discriminant_at(coeff_fn, theta_lo);
        let disc_hi = discriminant_at(coeff_fn, theta_hi);

        // Refine if sign change detected
        if disc_lo * disc_hi < 0.0 {
            let theta_zero = bisect_discriminant_zero(coeff_fn, theta_lo, theta_hi);
            let hs = solve_h_at_theta(coeff_fn, h_bounds, theta_zero);
            insertions.push((i + 1, theta_zero, hs));
        }
    }

    // Insert from back to preserve indices
    for (idx, theta, hs) in insertions.into_iter().rev() {
        samples.insert(idx, (theta, hs));
    }
}

/// Discriminant: beta^2 - alpha*gamma
fn discriminant_at<F>(coeff_fn: &F, theta: f64) -> f64
where
    F: Fn(f64) -> (f64, f64, f64),
{
    let (alpha, beta, gamma) = coeff_fn(theta);
    beta * beta - alpha * gamma
}

/// Bisection refinement of discriminant zero
fn bisect_discriminant_zero<F>(coeff_fn: &F, theta_lo: f64, theta_hi: f64) -> f64
where
    F: Fn(f64) -> (f64, f64, f64),
{
    let mut lo = theta_lo;
    let mut hi = theta_hi;
    let disc_lo = discriminant_at(coeff_fn, lo);

    for _ in 0..60 {
        let mid = 0.5 * (lo + hi);
        if (hi - lo) < 1e-15 {
            return mid;
        }
        let disc_mid = discriminant_at(coeff_fn, mid);
        if disc_mid * disc_lo < 0.0 {
            hi = mid;
        } else {
            lo = mid;
        }
    }
    0.5 * (lo + hi)
}

/// Adaptive refinement: bisect intervals where direction change exceeds threshold.
fn adaptive_refine_quadric<F, E, H>(
    coeff_fn: &F,
    eval_fn: &E,
    h_bounds: &H,
    samples: &mut Vec<(f64, Vec<f64>)>,
) where
    F: Fn(f64) -> (f64, f64, f64),
    E: Fn(f64, f64) -> [f64; 3],
    H: Fn(f64) -> (f64, f64),
{
    for _ in 0..MAX_REFINE_DEPTH {
        let mut insertions: Vec<(usize, f64, Vec<f64>)> = Vec::new();

        for i in 0..samples.len().saturating_sub(2) {
            let (t0, ref hs0) = samples[i];
            let (t1, ref hs1) = samples[i + 1];
            let (t2, ref hs2) = samples[i + 2];

            let needs = check_refinement_needed(eval_fn, t0, hs0, t1, hs1, t2, hs2);
            if needs {
                let mid_01 = 0.5 * (t0 + t1);
                let mid_12 = 0.5 * (t1 + t2);
                let hs_01 = solve_h_at_theta(coeff_fn, h_bounds, mid_01);
                let hs_12 = solve_h_at_theta(coeff_fn, h_bounds, mid_12);
                insertions.push((i + 1, mid_01, hs_01));
                insertions.push((i + 2, mid_12, hs_12));
            }
        }

        if insertions.is_empty() {
            break;
        }

        // Dedup indices, insert from back
        insertions.dedup_by_key(|item| item.0);
        for (idx, theta, hs) in insertions.into_iter().rev() {
            let already_exists = samples.iter().any(|(t, _)| (t - theta).abs() < 1e-15);
            if !already_exists {
                samples.insert(idx, (theta, hs));
            }
        }
    }
}

/// Check if direction change between 3 consecutive samples exceeds threshold.
fn check_refinement_needed<E>(
    eval_fn: &E,
    t0: f64,
    hs0: &[f64],
    t1: f64,
    hs1: &[f64],
    t2: f64,
    hs2: &[f64],
) -> bool
where
    E: Fn(f64, f64) -> [f64; 3],
{
    if hs0.is_empty() || hs1.is_empty() || hs2.is_empty() {
        return false;
    }

    for &h0 in hs0 {
        let p0 = eval_fn(t0, h0);
        if let Some(&h1) = nearest_h(hs1, h0) {
            let p1 = eval_fn(t1, h1);
            if let Some(&h2) = nearest_h(hs2, h1) {
                let p2 = eval_fn(t2, h2);
                if needs_refinement(p0, p1, p2, ANGLE_THRESHOLD) {
                    return true;
                }
            }
        }
    }
    false
}

/// Return nearest value in h array to target.
fn nearest_h(hs: &[f64], target: f64) -> Option<&f64> {
    hs.iter().min_by(|a, b| {
        let da = (*a - target).abs();
        let db = (*b - target).abs();
        da.total_cmp(&db)
    })
}

/// Check if direction change of 3 points exceeds angle threshold.
fn needs_refinement(p0: [f64; 3], p1: [f64; 3], p2: [f64; 3], angle_threshold: f64) -> bool {
    let v01 = vec3::sub(p1, p0);
    let v12 = vec3::sub(p2, p1);
    let len01 = vec3::length(v01);
    let len12 = vec3::length(v12);

    if len01 < 1e-14 || len12 < 1e-14 {
        return false;
    }

    let cos_angle = vec3::dot(v01, v12) / (len01 * len12);
    let cos_clamped = cos_angle.clamp(-1.0, 1.0);
    let angle = cos_clamped.acos();

    angle > angle_threshold
}

/// Classify a sequence of points into branches by nearest-neighbor matching.
///
/// `char_len` scales the branch matching distance threshold.
pub(crate) fn classify_branches(
    points: &[(f64, f64, [f64; 3])],
    theta_range: (f64, f64),
    char_len: f64,
) -> Vec<Vec<[f64; 3]>> {
    if points.is_empty() {
        return vec![];
    }

    // Group by theta
    let mut theta_groups: Vec<ThetaGroup> = Vec::new();
    for &(theta, h, pt) in points {
        match theta_groups.last_mut() {
            Some((last_theta, group)) if (*last_theta - theta).abs() < 1e-15 => {
                group.push((h, pt));
            }
            _ => {
                theta_groups.push((theta, vec![(h, pt)]));
            }
        }
    }

    // Sort by theta
    theta_groups.sort_by(|a, b| a.0.total_cmp(&b.0));

    // Branch tracing: nearest-neighbor matching at each theta step
    let mut branches: Vec<Vec<[f64; 3]>> = Vec::new();
    let mut branch_tips: Vec<[f64; 3]> = Vec::new();

    for (_theta, group) in &theta_groups {
        let mut used = vec![false; group.len()];

        // Nearest-neighbor matching against existing branches
        for (bi, tip) in branch_tips.iter_mut().enumerate() {
            let mut best_idx = None;
            let mut best_dist = f64::MAX;

            for (gi, (_, pt)) in group.iter().enumerate() {
                if used[gi] {
                    continue;
                }
                let d = vec3::distance(*tip, *pt);
                if d < best_dist {
                    best_dist = d;
                    best_idx = Some(gi);
                }
            }

            if let Some(idx) = best_idx {
                // Scale distance threshold by surface characteristic length (min 0.01)
                let branch_tol = (char_len * 0.2).max(0.01);
                if best_dist < branch_tol {
                    used[idx] = true;
                    let pt = group[idx].1;
                    branches[bi].push(pt);
                    *tip = pt;
                }
            }
        }

        // Unassigned points start new branches
        for (gi, &(_, pt)) in group.iter().enumerate() {
            if !used[gi] {
                branches.push(vec![pt]);
                branch_tips.push(pt);
            }
        }
    }

    // Detect closed curves: connect first and last points if theta is periodic
    let is_periodic = {
        let span = (theta_range.1 - theta_range.0).abs();
        (span - std::f64::consts::TAU).abs() < 1e-6
    };

    if is_periodic {
        for branch in &mut branches {
            if branch.len() >= 3 {
                let first = branch[0];
                // len() >= 3 so last() is always Some
                let last = *branch.last().unwrap();
                if vec3::distance(first, last) < CLOSE_LOOP_TOL {
                    *branch.last_mut().unwrap() = first;
                }
            }
        }
    }

    // Remove branches with too few points
    branches.retain(|b| b.len() >= 2);

    branches
}

// ─── Core 2: sweep_spherical_intersection ────────────

/// Spherical parameter sweep for surface intersection.
///
/// Parametrize as (theta, phi), apply Weierstrass substitution t = tan(phi/2),
/// solve polynomial at each theta.
pub fn sweep_spherical_intersection<F, E>(
    phi_eq_fn: F,
    eval_fn: E,
    theta_range: (f64, f64),
    char_len: f64,
) -> Vec<Vec<[f64; 3]>>
where
    F: Fn(f64) -> Vec<f64>,
    E: Fn(f64, f64) -> [f64; 3],
{
    let n = N_THETA_INITIAL;
    let (t0, t1) = theta_range;
    let dt = (t1 - t0) / n as f64;
    let phi_range = (-std::f64::consts::FRAC_PI_2, std::f64::consts::FRAC_PI_2);

    // 1. Initial sampling: solve phi at each theta
    let mut samples: Vec<(f64, Vec<f64>)> = Vec::with_capacity(n + 1);
    for i in 0..=n {
        let theta = t0 + dt * i as f64;
        let phis = solve_phi_at_theta(&phi_eq_fn, theta, phi_range);
        samples.push((theta, phis));
    }

    // 2. Adaptive refinement
    adaptive_refine_spherical(&phi_eq_fn, &eval_fn, phi_range, &mut samples);

    // 3. Point collection
    let mut raw_points: Vec<(f64, f64, [f64; 3])> = Vec::new();
    for (theta, phis) in &samples {
        for &phi in phis {
            let pt = eval_fn(*theta, phi);
            raw_points.push((*theta, phi, pt));
        }
    }

    classify_branches(&raw_points, theta_range, char_len)
}

/// Solve Weierstrass-substituted polynomial at theta, return valid phi values.
fn solve_phi_at_theta<F>(phi_eq_fn: &F, theta: f64, phi_range: (f64, f64)) -> Vec<f64>
where
    F: Fn(f64) -> Vec<f64>,
{
    let coeffs = phi_eq_fn(theta);
    if coeffs.is_empty() {
        return vec![];
    }

    let t_roots = solve_polynomial(&coeffs).expect("polynomial-highorder feature enabled");

    // Inverse Weierstrass: t -> phi = 2 * atan(t)
    t_roots
        .into_iter()
        .filter_map(|t| {
            let phi = 2.0 * t.atan();
            if phi >= phi_range.0 - DISC_EPS && phi <= phi_range.1 + DISC_EPS {
                Some(phi.clamp(phi_range.0, phi_range.1))
            } else {
                None
            }
        })
        .collect()
}

/// Adaptive refinement for spherical sweep.
fn adaptive_refine_spherical<F, E>(
    phi_eq_fn: &F,
    eval_fn: &E,
    phi_range: (f64, f64),
    samples: &mut Vec<(f64, Vec<f64>)>,
) where
    F: Fn(f64) -> Vec<f64>,
    E: Fn(f64, f64) -> [f64; 3],
{
    for _ in 0..MAX_REFINE_DEPTH {
        let mut insertions: Vec<(usize, f64, Vec<f64>)> = Vec::new();

        for i in 0..samples.len().saturating_sub(2) {
            let (t0, ref phis0) = samples[i];
            let (t1, ref phis1) = samples[i + 1];
            let (t2, ref phis2) = samples[i + 2];

            let needs = check_refinement_needed(eval_fn, t0, phis0, t1, phis1, t2, phis2);
            if needs {
                let mid_01 = 0.5 * (t0 + t1);
                let mid_12 = 0.5 * (t1 + t2);
                let phis_01 = solve_phi_at_theta(phi_eq_fn, mid_01, phi_range);
                let phis_12 = solve_phi_at_theta(phi_eq_fn, mid_12, phi_range);
                insertions.push((i + 1, mid_01, phis_01));
                insertions.push((i + 2, mid_12, phis_12));
            }
        }

        if insertions.is_empty() {
            break;
        }

        insertions.dedup_by_key(|item| item.0);
        for (idx, theta, phis) in insertions.into_iter().rev() {
            let already_exists = samples.iter().any(|(t, _)| (t - theta).abs() < 1e-15);
            if !already_exists {
                samples.insert(idx, (theta, phis));
            }
        }
    }
}

/// Length of cylinder axis vector (= height).
fn cyl_axis_length(surface: &Surface) -> f64 {
    match surface {
        Surface::Cylinder { axis, .. } => vec3::length(*axis),
        _ => 0.0,
    }
}

/// Convert polyline to Line segment sequence.
fn polyline_to_line_segments(polyline: &[[f64; 3]]) -> Vec<Curve3D> {
    polyline
        .windows(2)
        .map(|w| Curve3D::Line {
            start: w[0],
            end: w[1],
        })
        .collect()
}

/// Clip sweep polylines to face boundaries and convert to Curve3D.
///
/// Skips clipping if faces have no edge loops.
fn clip_or_convert_polylines(
    polylines: &[Vec<[f64; 3]>],
    face_a: &Face,
    shell_a: &Shell,
    face_b: &Face,
    shell_b: &Shell,
) -> Vec<Curve3D> {
    let has_bounds = !face_a.loop_edges.is_empty() && !face_b.loop_edges.is_empty();
    let mut result = Vec::new();

    for polyline in polylines {
        if polyline.len() < 2 {
            continue;
        }
        if has_bounds {
            let clipped = clip_polyline_to_both_faces(polyline, face_a, shell_a, face_b, shell_b);
            result.extend(clipped);
        } else {
            result.extend(polyline_to_line_segments(polyline));
        }
    }
    result
}

// ─── B1: Sphere × Cylinder ───────────────────────────────

/// Sphere x Cylinder face intersection.
pub(crate) fn sphere_cylinder_face_intersection(
    sphere_face: &Face,
    sphere_shell: &Shell,
    cyl_face: &Face,
    cyl_shell: &Shell,
    _events: &mut Vec<BooleanEvent>,
) -> Vec<Curve3D> {
    let (sc, rs) = match &sphere_face.surface {
        Surface::Sphere { center, radius } => (*center, *radius),
        _ => return vec![],
    };
    let (co, ca, cr) = match &cyl_face.surface {
        Surface::Cylinder {
            origin,
            axis,
            radius,
        } => (*origin, vec3::normalized(*axis), *radius),
        _ => return vec![],
    };

    let char_len = rs.min(cr);
    let (e1, e2) = vec3::orthonormal_basis(ca);
    let h_len = cyl_axis_length(&cyl_face.surface);

    let d = vec3::sub(co, sc);
    let d_a = vec3::dot(d, ca);
    let d_e1 = vec3::dot(d, e1);
    let d_e2 = vec3::dot(d, e2);
    let d2 = vec3::dot(d, d);

    let coeff_fn = |theta: f64| -> (f64, f64, f64) {
        let cos_t = theta.cos();
        let sin_t = theta.sin();
        let alpha = 1.0;
        let beta = d_a;
        let gamma = cr * cr + 2.0 * cr * (d_e1 * cos_t + d_e2 * sin_t) + d2 - rs * rs;
        (alpha, beta, gamma)
    };

    let eval_fn = |theta: f64, h: f64| -> [f64; 3] {
        vec3::add(
            vec3::add(
                vec3::add(co, vec3::scale(e1, cr * theta.cos())),
                vec3::scale(e2, cr * theta.sin()),
            ),
            vec3::scale(ca, h),
        )
    };

    let h_bounds = |_theta: f64| -> (f64, f64) { (0.0, h_len) };

    let polylines = sweep_quadric_intersection(coeff_fn, eval_fn, (0.0, TAU), h_bounds, char_len);
    clip_or_convert_polylines(&polylines, sphere_face, sphere_shell, cyl_face, cyl_shell)
}

// ─── B4: Ellipsoid × Cylinder ────────────────────────────

/// Ellipsoid x Cylinder face intersection.
pub(crate) fn ellipsoid_cylinder_face_intersection(
    ellipsoid_face: &Face,
    ellipsoid_shell: &Shell,
    cyl_face: &Face,
    cyl_shell: &Shell,
    _events: &mut Vec<BooleanEvent>,
) -> Vec<Curve3D> {
    let (ec, rx, ry, rz) = match &ellipsoid_face.surface {
        Surface::Ellipsoid { center, rx, ry, rz } => (*center, *rx, *ry, *rz),
        _ => return vec![],
    };
    let (co, ca, cr) = match &cyl_face.surface {
        Surface::Cylinder {
            origin,
            axis,
            radius,
        } => (*origin, vec3::normalized(*axis), *radius),
        _ => return vec![],
    };

    let char_len = rx.min(ry).min(rz).min(cr);
    let (e1, e2) = vec3::orthonormal_basis(ca);
    let h_len = cyl_axis_length(&cyl_face.surface);

    let inv_r2 = [1.0 / (rx * rx), 1.0 / (ry * ry), 1.0 / (rz * rz)];
    let a_comp = [ca[0], ca[1], ca[2]];
    let e1_comp = [e1[0], e1[1], e1[2]];
    let e2_comp = [e2[0], e2[1], e2[2]];
    let d_comp = [co[0] - ec[0], co[1] - ec[1], co[2] - ec[2]];

    // alpha = sum(a_i^2/r_i^2) (constant)
    let alpha: f64 = (0..3).map(|i| a_comp[i] * a_comp[i] * inv_r2[i]).sum();

    let coeff_fn = |theta: f64| -> (f64, f64, f64) {
        let cos_t = theta.cos();
        let sin_t = theta.sin();

        let mut beta = 0.0;
        let mut gamma = -1.0; // -1 from implicit RHS
        for i in 0..3 {
            let p_i = cr * (e1_comp[i] * cos_t + e2_comp[i] * sin_t) + d_comp[i];
            beta += a_comp[i] * p_i * inv_r2[i];
            gamma += p_i * p_i * inv_r2[i];
        }
        (alpha, beta, gamma)
    };

    let eval_fn = |theta: f64, h: f64| -> [f64; 3] {
        vec3::add(
            vec3::add(
                vec3::add(co, vec3::scale(e1, cr * theta.cos())),
                vec3::scale(e2, cr * theta.sin()),
            ),
            vec3::scale(ca, h),
        )
    };

    let h_bounds = |_theta: f64| -> (f64, f64) { (0.0, h_len) };

    let polylines = sweep_quadric_intersection(coeff_fn, eval_fn, (0.0, TAU), h_bounds, char_len);
    clip_or_convert_polylines(
        &polylines,
        ellipsoid_face,
        ellipsoid_shell,
        cyl_face,
        cyl_shell,
    )
}

// ─── B7: Cylinder x Cylinder ───────────────────

/// Cylinder x Cylinder face intersection.
///
/// Parallel/coaxial cases use analytic solutions; non-parallel uses sweep.
pub(crate) fn cylinder_cylinder_face_intersection(
    cyl_a_face: &Face,
    cyl_a_shell: &Shell,
    cyl_b_face: &Face,
    cyl_b_shell: &Shell,
    _events: &mut Vec<BooleanEvent>,
) -> Vec<Curve3D> {
    let (oa, aa, ra) = match &cyl_a_face.surface {
        Surface::Cylinder {
            origin,
            axis,
            radius,
        } => (*origin, vec3::normalized(*axis), *radius),
        _ => return vec![],
    };
    let (ob, ab, rb) = match &cyl_b_face.surface {
        Surface::Cylinder {
            origin,
            axis,
            radius,
        } => (*origin, vec3::normalized(*axis), *radius),
        _ => return vec![],
    };

    let char_len = ra.min(rb);
    let (is_parallel, d) = classify_axes(&oa, &aa, &ob, &ab);

    if is_parallel {
        return cylinder_cylinder_parallel(
            &oa,
            &aa,
            ra,
            cyl_axis_length(&cyl_a_face.surface),
            &ob,
            &ab,
            rb,
            cyl_axis_length(&cyl_b_face.surface),
            d,
            cyl_a_face,
            cyl_a_shell,
            cyl_b_face,
            cyl_b_shell,
        );
    }

    // Non-parallel: parametrize Cylinder_A, substitute into Cylinder_B implicit
    let (e1, e2) = vec3::orthonormal_basis(aa);
    let h_len_a = cyl_axis_length(&cyl_a_face.surface);

    // d = oa - ob
    let dv = vec3::sub(oa, ob);

    let d_cross_ab = vec3::cross(dv, ab);
    let e1_cross_ab = vec3::cross(e1, ab);
    let e2_cross_ab = vec3::cross(e2, ab);
    let b_vec = vec3::cross(aa, ab);
    let alpha = vec3::dot(b_vec, b_vec);

    let coeff_fn = |theta: f64| -> (f64, f64, f64) {
        let cos_t = theta.cos();
        let sin_t = theta.sin();
        let ax = d_cross_ab[0] + ra * (e1_cross_ab[0] * cos_t + e2_cross_ab[0] * sin_t);
        let ay = d_cross_ab[1] + ra * (e1_cross_ab[1] * cos_t + e2_cross_ab[1] * sin_t);
        let az = d_cross_ab[2] + ra * (e1_cross_ab[2] * cos_t + e2_cross_ab[2] * sin_t);
        let a_dot_b = ax * b_vec[0] + ay * b_vec[1] + az * b_vec[2];
        let a_sq = ax * ax + ay * ay + az * az;
        (alpha, a_dot_b, a_sq - rb * rb)
    };

    let eval_fn = |theta: f64, h: f64| -> [f64; 3] {
        vec3::add(
            vec3::add(
                vec3::add(oa, vec3::scale(e1, ra * theta.cos())),
                vec3::scale(e2, ra * theta.sin()),
            ),
            vec3::scale(aa, h),
        )
    };

    let h_bounds = |_theta: f64| -> (f64, f64) { (0.0, h_len_a) };

    let polylines = sweep_quadric_intersection(coeff_fn, eval_fn, (0.0, TAU), h_bounds, char_len);
    clip_or_convert_polylines(&polylines, cyl_a_face, cyl_a_shell, cyl_b_face, cyl_b_shell)
}

/// Analytic intersection of two parallel cylinders.
#[allow(clippy::too_many_arguments)]
fn cylinder_cylinder_parallel(
    oa: &[f64; 3],
    aa: &[f64; 3],
    ra: f64,
    h_len_a: f64,
    ob: &[f64; 3],
    _ab: &[f64; 3],
    rb: f64,
    h_len_b: f64,
    d: f64,
    face_a: &Face,
    shell_a: &Shell,
    face_b: &Face,
    shell_b: &Shell,
) -> Vec<Curve3D> {
    let tol = 1e-10;

    // Coaxial
    if d < tol {
        return vec![];
    }

    // Too far apart
    if d > ra + rb + tol {
        return vec![];
    }

    // Containment (no intersection)
    if d < (ra - rb).abs() - tol {
        return vec![];
    }

    let diff = vec3::sub(*ob, *oa);
    let along = vec3::dot(diff, *aa);
    let perp = vec3::sub(diff, vec3::scale(*aa, along));
    let u = vec3::normalized(perp);
    let w = vec3::normalized(vec3::cross(*aa, u));

    // Effective h range: axial overlap of two cylinders
    let h_min = 0.0_f64.max(-along);
    let h_max = h_len_a.min(h_len_b - along);
    if h_max - h_min < tol {
        return vec![];
    }

    let mut lines = Vec::new();

    // Circumscribed: 1 line
    if (d - (ra + rb)).abs() < tol || (d - (ra - rb).abs()).abs() < tol {
        let pt = vec3::add(*oa, vec3::scale(u, ra));
        let line = Curve3D::Line {
            start: vec3::add(pt, vec3::scale(*aa, h_min)),
            end: vec3::add(pt, vec3::scale(*aa, h_max)),
        };
        lines.push(line);
    } else {
        // 2 lines
        let x = (d * d + ra * ra - rb * rb) / (2.0 * d);
        let y_sq = ra * ra - x * x;
        if y_sq < 0.0 {
            return vec![];
        }
        let y = y_sq.sqrt();

        let base = vec3::add(*oa, vec3::scale(u, x));
        let p1 = vec3::add(base, vec3::scale(w, y));
        let p2 = vec3::sub(base, vec3::scale(w, y));

        lines.push(Curve3D::Line {
            start: vec3::add(p1, vec3::scale(*aa, h_min)),
            end: vec3::add(p1, vec3::scale(*aa, h_max)),
        });
        lines.push(Curve3D::Line {
            start: vec3::add(p2, vec3::scale(*aa, h_min)),
            end: vec3::add(p2, vec3::scale(*aa, h_max)),
        });
    }

    // Clip to face boundaries
    let has_bounds = !face_a.loop_edges.is_empty() && !face_b.loop_edges.is_empty();
    if has_bounds {
        let mut result = Vec::new();
        for line in &lines {
            let polyline = line.to_polyline(1e-3);
            if polyline.len() >= 2 {
                let clipped =
                    clip_polyline_to_both_faces(&polyline, face_a, shell_a, face_b, shell_b);
                result.extend(clipped);
            }
        }
        result
    } else {
        lines
    }
}

/// Length of cone axis vector (= h upper bound).
fn cone_axis_length(surface: &Surface) -> f64 {
    match surface {
        Surface::Cone { axis, .. } => vec3::length(*axis),
        _ => 0.0,
    }
}

// ─── B2: Sphere × Cone ──────────────────────────────────

/// Sphere x Cone face intersection.
pub(crate) fn sphere_cone_face_intersection(
    sphere_face: &Face,
    sphere_shell: &Shell,
    cone_face: &Face,
    cone_shell: &Shell,
    _events: &mut Vec<BooleanEvent>,
) -> Vec<Curve3D> {
    let (sc, rs) = match &sphere_face.surface {
        Surface::Sphere { center, radius } => (*center, *radius),
        _ => return vec![],
    };
    let (cv, ca_raw, half_angle) = match &cone_face.surface {
        Surface::Cone {
            origin,
            axis,
            half_angle,
        } => (*origin, *axis, *half_angle),
        _ => return vec![],
    };

    let ca = vec3::normalized(ca_raw);
    let cone_axis_len = vec3::length(ca_raw);
    let char_len = rs.min(cone_axis_len.max(1.0));
    let (e1, e2) = vec3::orthonormal_basis(ca);
    let h_len = cone_axis_length(&cone_face.surface);
    let cos_a = half_angle.cos();
    let sin_a = half_angle.sin();

    // d = v - sc
    let d = vec3::sub(cv, sc);
    let d_a = vec3::dot(d, ca);
    let d_e1 = vec3::dot(d, e1);
    let d_e2 = vec3::dot(d, e2);
    let d2 = vec3::dot(d, d);

    let coeff_fn = |theta: f64| -> (f64, f64, f64) {
        let cos_t = theta.cos();
        let sin_t = theta.sin();
        let alpha_coeff = 1.0;
        let beta = cos_a * d_a + sin_a * (d_e1 * cos_t + d_e2 * sin_t);
        let gamma = d2 - rs * rs;
        (alpha_coeff, beta, gamma)
    };

    let eval_fn = |theta: f64, h: f64| -> [f64; 3] {
        let r = h * sin_a;
        vec3::add(
            vec3::add(cv, vec3::scale(ca, h * cos_a)),
            vec3::add(
                vec3::scale(e1, r * theta.cos()),
                vec3::scale(e2, r * theta.sin()),
            ),
        )
    };

    let h_bounds = |_theta: f64| -> (f64, f64) { (0.0, h_len) };

    let polylines = sweep_quadric_intersection(coeff_fn, eval_fn, (0.0, TAU), h_bounds, char_len);
    clip_or_convert_polylines(&polylines, sphere_face, sphere_shell, cone_face, cone_shell)
}

// ─── B5: Ellipsoid × Cone ───────────────────────────────

/// Ellipsoid x Cone face intersection.
pub(crate) fn ellipsoid_cone_face_intersection(
    ellipsoid_face: &Face,
    ellipsoid_shell: &Shell,
    cone_face: &Face,
    cone_shell: &Shell,
    _events: &mut Vec<BooleanEvent>,
) -> Vec<Curve3D> {
    let (ec, rx, ry, rz) = match &ellipsoid_face.surface {
        Surface::Ellipsoid { center, rx, ry, rz } => (*center, *rx, *ry, *rz),
        _ => return vec![],
    };
    let (cv, ca_raw, half_angle) = match &cone_face.surface {
        Surface::Cone {
            origin,
            axis,
            half_angle,
        } => (*origin, *axis, *half_angle),
        _ => return vec![],
    };

    let ca = vec3::normalized(ca_raw);
    let cone_axis_len = vec3::length(ca_raw);
    let char_len = rx.min(ry).min(rz).min(cone_axis_len.max(1.0));
    let (e1, e2) = vec3::orthonormal_basis(ca);
    let h_len = cone_axis_length(&cone_face.surface);
    let cos_a = half_angle.cos();
    let sin_a = half_angle.sin();

    let inv_r2 = [1.0 / (rx * rx), 1.0 / (ry * ry), 1.0 / (rz * rz)];
    let a_comp = [ca[0], ca[1], ca[2]];
    let e1_comp = [e1[0], e1[1], e1[2]];
    let e2_comp = [e2[0], e2[1], e2[2]];
    let d_comp = [cv[0] - ec[0], cv[1] - ec[1], cv[2] - ec[2]];

    let coeff_fn = |theta: f64| -> (f64, f64, f64) {
        let cos_t = theta.cos();
        let sin_t = theta.sin();

        let mut alpha_coeff = 0.0;
        let mut beta = 0.0;
        let mut gamma = -1.0; // Implicit RHS
        for i in 0..3 {
            let q_i = cos_a * a_comp[i] + sin_a * (e1_comp[i] * cos_t + e2_comp[i] * sin_t);
            alpha_coeff += q_i * q_i * inv_r2[i];
            beta += d_comp[i] * q_i * inv_r2[i];
            gamma += d_comp[i] * d_comp[i] * inv_r2[i];
        }
        (alpha_coeff, beta, gamma)
    };

    let eval_fn = |theta: f64, h: f64| -> [f64; 3] {
        let r = h * sin_a;
        vec3::add(
            vec3::add(cv, vec3::scale(ca, h * cos_a)),
            vec3::add(
                vec3::scale(e1, r * theta.cos()),
                vec3::scale(e2, r * theta.sin()),
            ),
        )
    };

    let h_bounds = |_theta: f64| -> (f64, f64) { (0.0, h_len) };

    let polylines = sweep_quadric_intersection(coeff_fn, eval_fn, (0.0, TAU), h_bounds, char_len);
    clip_or_convert_polylines(
        &polylines,
        ellipsoid_face,
        ellipsoid_shell,
        cone_face,
        cone_shell,
    )
}

// ─── B8: Cylinder × Cone ────────────────────────────────

/// Cylinder x Cone face intersection.
pub(crate) fn cylinder_cone_face_intersection(
    cyl_face: &Face,
    cyl_shell: &Shell,
    cone_face: &Face,
    cone_shell: &Shell,
    _events: &mut Vec<BooleanEvent>,
) -> Vec<Curve3D> {
    let (co, ca_cyl, cr) = match &cyl_face.surface {
        Surface::Cylinder {
            origin,
            axis,
            radius,
        } => (*origin, vec3::normalized(*axis), *radius),
        _ => return vec![],
    };
    let (cv, ca_cone_raw, half_angle) = match &cone_face.surface {
        Surface::Cone {
            origin,
            axis,
            half_angle,
        } => (*origin, *axis, *half_angle),
        _ => return vec![],
    };

    let ca_cone = vec3::normalized(ca_cone_raw);
    let cone_axis_len = vec3::length(ca_cone_raw);
    let char_len = cr.min(cone_axis_len.max(1.0));
    let (e1, e2) = vec3::orthonormal_basis(ca_cone);
    let h_len = cone_axis_length(&cone_face.surface);
    let cos_a = half_angle.cos();
    let sin_a = half_angle.sin();

    let dv = vec3::sub(cv, co);
    let d_cross_ac = vec3::cross(dv, ca_cyl);

    let a_cone_cross_ac = vec3::cross(ca_cone, ca_cyl);
    let e1_cross_ac = vec3::cross(e1, ca_cyl);
    let e2_cross_ac = vec3::cross(e2, ca_cyl);

    let coeff_fn = |theta: f64| -> (f64, f64, f64) {
        let cos_t = theta.cos();
        let sin_t = theta.sin();

        let bx =
            cos_a * a_cone_cross_ac[0] + sin_a * (e1_cross_ac[0] * cos_t + e2_cross_ac[0] * sin_t);
        let by =
            cos_a * a_cone_cross_ac[1] + sin_a * (e1_cross_ac[1] * cos_t + e2_cross_ac[1] * sin_t);
        let bz =
            cos_a * a_cone_cross_ac[2] + sin_a * (e1_cross_ac[2] * cos_t + e2_cross_ac[2] * sin_t);

        let b_sq = bx * bx + by * by + bz * bz;
        let a_dot_b = d_cross_ac[0] * bx + d_cross_ac[1] * by + d_cross_ac[2] * bz;
        let a_sq = d_cross_ac[0] * d_cross_ac[0]
            + d_cross_ac[1] * d_cross_ac[1]
            + d_cross_ac[2] * d_cross_ac[2];

        (b_sq, a_dot_b, a_sq - cr * cr)
    };

    let eval_fn = |theta: f64, h: f64| -> [f64; 3] {
        let r = h * sin_a;
        vec3::add(
            vec3::add(cv, vec3::scale(ca_cone, h * cos_a)),
            vec3::add(
                vec3::scale(e1, r * theta.cos()),
                vec3::scale(e2, r * theta.sin()),
            ),
        )
    };

    let h_bounds = |_theta: f64| -> (f64, f64) { (0.0, h_len) };

    let polylines = sweep_quadric_intersection(coeff_fn, eval_fn, (0.0, TAU), h_bounds, char_len);
    clip_or_convert_polylines(&polylines, cyl_face, cyl_shell, cone_face, cone_shell)
}

// ─── B9: Cone × Cone ────────────────────────────────────

/// Cone x Cone face intersection.
pub(crate) fn cone_cone_face_intersection(
    cone_a_face: &Face,
    cone_a_shell: &Shell,
    cone_b_face: &Face,
    cone_b_shell: &Shell,
    _events: &mut Vec<BooleanEvent>,
) -> Vec<Curve3D> {
    let (va, ca_a_raw, ha_a) = match &cone_a_face.surface {
        Surface::Cone {
            origin,
            axis,
            half_angle,
        } => (*origin, *axis, *half_angle),
        _ => return vec![],
    };
    let (vb, ca_b_raw, ha_b) = match &cone_b_face.surface {
        Surface::Cone {
            origin,
            axis,
            half_angle,
        } => (*origin, *axis, *half_angle),
        _ => return vec![],
    };

    let ca_a = vec3::normalized(ca_a_raw);
    let ca_b = vec3::normalized(ca_b_raw);
    let char_len = vec3::length(ca_a_raw)
        .max(1.0)
        .min(vec3::length(ca_b_raw).max(1.0));
    let h_len_a = cone_axis_length(&cone_a_face.surface);

    // Coaxial check
    let dot_ab = vec3::dot(ca_a, ca_b).abs();
    if dot_ab > 1.0 - 1e-10 {
        let diff = vec3::sub(vb, va);
        let along = vec3::dot(diff, ca_a);
        let perp = vec3::sub(diff, vec3::scale(ca_a, along));
        if vec3::length(perp) < 1e-10 {
            return vec![];
        }
    }

    let (e1, e2) = vec3::orthonormal_basis(ca_a);
    let cos_a = ha_a.cos();
    let sin_a = ha_a.sin();

    let cos_b = ha_b.cos();
    let sin_b = ha_b.sin();
    let cos2_b = cos_b * cos_b;
    let sin2_b = sin_b * sin_b;

    let dv = vec3::sub(va, vb);
    let p_vec = vec3::cross(dv, ca_b);
    let s = vec3::dot(dv, ca_b);

    let a_a_cross_ab = vec3::cross(ca_a, ca_b);
    let e1_cross_ab = vec3::cross(e1, ca_b);
    let e2_cross_ab = vec3::cross(e2, ca_b);
    let a_a_dot_ab = vec3::dot(ca_a, ca_b);
    let e1_dot_ab = vec3::dot(e1, ca_b);
    let e2_dot_ab = vec3::dot(e2, ca_b);

    let coeff_fn = |theta: f64| -> (f64, f64, f64) {
        let cos_t = theta.cos();
        let sin_t = theta.sin();

        let qx =
            cos_a * a_a_cross_ab[0] + sin_a * (e1_cross_ab[0] * cos_t + e2_cross_ab[0] * sin_t);
        let qy =
            cos_a * a_a_cross_ab[1] + sin_a * (e1_cross_ab[1] * cos_t + e2_cross_ab[1] * sin_t);
        let qz =
            cos_a * a_a_cross_ab[2] + sin_a * (e1_cross_ab[2] * cos_t + e2_cross_ab[2] * sin_t);

        let q_sq = qx * qx + qy * qy + qz * qz;
        let p_dot_q = p_vec[0] * qx + p_vec[1] * qy + p_vec[2] * qz;
        let p_sq = vec3::dot(p_vec, p_vec);

        let t = cos_a * a_a_dot_ab + sin_a * (e1_dot_ab * cos_t + e2_dot_ab * sin_t);

        let alpha_coeff = cos2_b * q_sq - sin2_b * t * t;
        let beta = cos2_b * p_dot_q - sin2_b * s * t;
        let gamma = cos2_b * p_sq - sin2_b * s * s;

        (alpha_coeff, beta, gamma)
    };

    let eval_fn = |theta: f64, h: f64| -> [f64; 3] {
        let r = h * sin_a;
        vec3::add(
            vec3::add(va, vec3::scale(ca_a, h * cos_a)),
            vec3::add(
                vec3::scale(e1, r * theta.cos()),
                vec3::scale(e2, r * theta.sin()),
            ),
        )
    };

    let h_bounds = |_theta: f64| -> (f64, f64) { (0.0, h_len_a) };

    // Filter by Cone_B half-space: (x - v_B) * a_B >= 0
    let polylines = sweep_quadric_intersection(coeff_fn, eval_fn, (0.0, TAU), h_bounds, char_len);

    // Keep only points in Cone_B half-space
    let filtered: Vec<Vec<[f64; 3]>> = polylines
        .into_iter()
        .flat_map(|pl| {
            let mut segments: Vec<Vec<[f64; 3]>> = Vec::new();
            let mut current: Vec<[f64; 3]> = Vec::new();
            for pt in pl {
                let w = vec3::sub(pt, vb);
                if vec3::dot(w, ca_b) >= -DISC_EPS {
                    current.push(pt);
                } else if current.len() >= 2 {
                    segments.push(std::mem::take(&mut current));
                } else {
                    current.clear();
                }
            }
            if current.len() >= 2 {
                segments.push(current);
            }
            segments
        })
        .collect();

    clip_or_convert_polylines(
        &filtered,
        cone_a_face,
        cone_a_shell,
        cone_b_face,
        cone_b_shell,
    )
}

// ─── B3: Sphere × Ellipsoid ─────────────────────────────

/// Sphere x Ellipsoid face intersection.
pub(crate) fn sphere_ellipsoid_face_intersection(
    sphere_face: &Face,
    sphere_shell: &Shell,
    ellipsoid_face: &Face,
    ellipsoid_shell: &Shell,
    _events: &mut Vec<BooleanEvent>,
) -> Vec<Curve3D> {
    let (sc, rs) = match &sphere_face.surface {
        Surface::Sphere { center, radius } => (*center, *radius),
        _ => return vec![],
    };
    let (ec, rx, ry, rz) = match &ellipsoid_face.surface {
        Surface::Ellipsoid { center, rx, ry, rz } => (*center, *rx, *ry, *rz),
        _ => return vec![],
    };

    let char_len = rs.min(rx.min(ry).min(rz));

    let dx = ec[0] - sc[0];
    let dy = ec[1] - sc[1];
    let dz = ec[2] - sc[2];
    let d2 = dx * dx + dy * dy + dz * dz;
    let rz2 = rz * rz;

    let c_const = d2 - rs * rs;
    let e_coeff = 4.0 * dz * rz;

    let phi_eq_fn = |theta: f64| -> Vec<f64> {
        let cos_t = theta.cos();
        let sin_t = theta.sin();

        let a = rx * rx * cos_t * cos_t + ry * ry * sin_t * sin_t;
        let b = 2.0 * (dx * rx * cos_t + dy * ry * sin_t);

        vec![
            a + b + c_const,
            e_coeff,
            -2.0 * a + 4.0 * rz2 + 2.0 * c_const,
            e_coeff,
            a - b + c_const,
        ]
    };

    let eval_fn = |theta: f64, phi: f64| -> [f64; 3] {
        let cos_phi = phi.cos();
        let sin_phi = phi.sin();
        [
            ec[0] + rx * theta.cos() * cos_phi,
            ec[1] + ry * theta.sin() * cos_phi,
            ec[2] + rz * sin_phi,
        ]
    };

    let polylines = sweep_spherical_intersection(phi_eq_fn, eval_fn, (0.0, TAU), char_len);
    clip_or_convert_polylines(
        &polylines,
        sphere_face,
        sphere_shell,
        ellipsoid_face,
        ellipsoid_shell,
    )
}

// ─── B6: Ellipsoid × Ellipsoid ──────────────────────────

/// Ellipsoid x Ellipsoid face intersection.
pub(crate) fn ellipsoid_ellipsoid_face_intersection(
    ellipsoid_a_face: &Face,
    ellipsoid_a_shell: &Shell,
    ellipsoid_b_face: &Face,
    ellipsoid_b_shell: &Shell,
    _events: &mut Vec<BooleanEvent>,
) -> Vec<Curve3D> {
    let (ca, rxa, rya, rza) = match &ellipsoid_a_face.surface {
        Surface::Ellipsoid { center, rx, ry, rz } => (*center, *rx, *ry, *rz),
        _ => return vec![],
    };
    let (cb, rxb, ryb, rzb) = match &ellipsoid_b_face.surface {
        Surface::Ellipsoid { center, rx, ry, rz } => (*center, *rx, *ry, *rz),
        _ => return vec![],
    };

    let char_len = rxa.min(rya).min(rza).min(rxb).min(ryb).min(rzb);

    let dx = cb[0] - ca[0];
    let dy = cb[1] - ca[1];
    let dz = cb[2] - ca[2];

    let inv_rxa2 = 1.0 / (rxa * rxa);
    let inv_rya2 = 1.0 / (rya * rya);
    let inv_rza2 = 1.0 / (rza * rza);

    let d_coeff = rzb * rzb * inv_rza2;
    let e_coeff = 2.0 * dz * rzb * inv_rza2;
    let c_const = dx * dx * inv_rxa2 + dy * dy * inv_rya2 + dz * dz * inv_rza2 - 1.0;

    let phi_eq_fn = |theta: f64| -> Vec<f64> {
        let cos_t = theta.cos();
        let sin_t = theta.sin();

        let a = rxb * rxb * cos_t * cos_t * inv_rxa2 + ryb * ryb * sin_t * sin_t * inv_rya2;
        let b = 2.0 * (dx * rxb * cos_t * inv_rxa2 + dy * ryb * sin_t * inv_rya2);

        let two_e = 2.0 * e_coeff;

        vec![
            a + b + c_const,
            two_e,
            -2.0 * a + 4.0 * d_coeff + 2.0 * c_const,
            two_e,
            a - b + c_const,
        ]
    };

    let eval_fn = |theta: f64, phi: f64| -> [f64; 3] {
        let cos_phi = phi.cos();
        let sin_phi = phi.sin();
        [
            cb[0] + rxb * theta.cos() * cos_phi,
            cb[1] + ryb * theta.sin() * cos_phi,
            cb[2] + rzb * sin_phi,
        ]
    };

    let polylines = sweep_spherical_intersection(phi_eq_fn, eval_fn, (0.0, TAU), char_len);
    clip_or_convert_polylines(
        &polylines,
        ellipsoid_a_face,
        ellipsoid_a_shell,
        ellipsoid_b_face,
        ellipsoid_b_shell,
    )
}

// ─── Core 3: sweep_torus_intersection ─────────────────

/// Shift to avoid Weierstrass singularity at phi = +/-pi
const WEIERSTRASS_EPS: f64 = 0.01;

/// Torus parameter sweep for surface intersection.
pub fn sweep_torus_intersection<F, E>(
    phi_eq_fn: F,
    eval_fn: E,
    theta_range: (f64, f64),
    char_len: f64,
) -> Vec<Vec<[f64; 3]>>
where
    F: Fn(f64) -> Vec<f64>,
    E: Fn(f64, f64) -> [f64; 3],
{
    let n = N_THETA_INITIAL;
    let (t0, t1) = theta_range;
    let dt = (t1 - t0) / n as f64;
    let pi = std::f64::consts::PI;
    let phi_main = (-pi + WEIERSTRASS_EPS, pi - WEIERSTRASS_EPS);

    // 1. Main interval: initial sampling
    let mut samples: Vec<(f64, Vec<f64>)> = Vec::with_capacity(n + 1);
    for i in 0..=n {
        let theta = t0 + dt * i as f64;
        let phis = solve_phi_at_theta_torus(&phi_eq_fn, theta, phi_main);
        samples.push((theta, phis));
    }

    // 2. Main interval: adaptive refinement
    adaptive_refine_torus(&phi_eq_fn, &eval_fn, phi_main, &mut samples);

    // 3. Complement: direct sampling near phi ~ +/-pi
    let complement_phis = [
        -pi,
        -pi + WEIERSTRASS_EPS * 0.25,
        -pi + WEIERSTRASS_EPS * 0.5,
        -pi + WEIERSTRASS_EPS * 0.75,
        pi - WEIERSTRASS_EPS * 0.75,
        pi - WEIERSTRASS_EPS * 0.5,
        pi - WEIERSTRASS_EPS * 0.25,
        pi,
    ];

    for sample in &mut samples {
        let theta = sample.0;
        let coeffs = phi_eq_fn(theta);
        if coeffs.is_empty() {
            continue;
        }
        for &phi in &complement_phis {
            let half = phi / 2.0;
            let cos_half = half.cos();
            if cos_half.abs() < 1e-15 {
                continue;
            }
            let t = half.sin() / cos_half;
            let val = eval_poly(&coeffs, t);
            if val.abs() < 1e-6 {
                let dominated = sample
                    .1
                    .iter()
                    .any(|&existing| (existing - phi).abs() < 1e-4);
                if !dominated {
                    sample.1.push(phi);
                }
            }
        }
        sample.1.sort_by(|a, b| a.total_cmp(b));
    }

    // 4. Point collection
    let mut raw_points: Vec<(f64, f64, [f64; 3])> = Vec::new();
    for (theta, phis) in &samples {
        for &phi in phis {
            let pt = eval_fn(*theta, phi);
            raw_points.push((*theta, phi, pt));
        }
    }

    classify_branches(&raw_points, theta_range, char_len)
}

/// Polynomial evaluation: coeffs[0] + coeffs[1]*t + coeffs[2]*t^2 + ...
fn eval_poly(coeffs: &[f64], t: f64) -> f64 {
    let mut result = 0.0;
    let mut t_pow = 1.0;
    for &c in coeffs {
        result += c * t_pow;
        t_pow *= t;
    }
    result
}

/// Solve Weierstrass polynomial for torus at theta, return valid phi in [-pi, pi].
fn solve_phi_at_theta_torus<F>(phi_eq_fn: &F, theta: f64, phi_range: (f64, f64)) -> Vec<f64>
where
    F: Fn(f64) -> Vec<f64>,
{
    let coeffs = phi_eq_fn(theta);
    if coeffs.is_empty() {
        return vec![];
    }

    let t_roots = solve_polynomial(&coeffs).expect("polynomial-highorder feature enabled");

    let max_coeff = coeffs
        .iter()
        .map(|c| c.abs())
        .fold(0.0_f64, f64::max)
        .max(1.0);
    let residual_tol = 1e-6 * max_coeff;

    t_roots
        .into_iter()
        .filter_map(|t| {
            let residual = eval_poly(&coeffs, t).abs();
            if residual > residual_tol {
                return None;
            }
            let phi = 2.0 * t.atan();
            if phi >= phi_range.0 - DISC_EPS && phi <= phi_range.1 + DISC_EPS {
                Some(phi.clamp(phi_range.0, phi_range.1))
            } else {
                None
            }
        })
        .collect()
}

/// Adaptive refinement for torus sweep.
fn adaptive_refine_torus<F, E>(
    phi_eq_fn: &F,
    eval_fn: &E,
    phi_range: (f64, f64),
    samples: &mut Vec<(f64, Vec<f64>)>,
) where
    F: Fn(f64) -> Vec<f64>,
    E: Fn(f64, f64) -> [f64; 3],
{
    const MAX_TORUS_SAMPLES: usize = 1500;

    for _ in 0..MAX_REFINE_DEPTH {
        if samples.len() >= MAX_TORUS_SAMPLES {
            break;
        }
        let mut insertions: Vec<(usize, f64, Vec<f64>)> = Vec::new();

        for i in 0..samples.len().saturating_sub(2) {
            let (t0, ref phis0) = samples[i];
            let (t1, ref phis1) = samples[i + 1];
            let (t2, ref phis2) = samples[i + 2];

            let needs = check_refinement_needed(eval_fn, t0, phis0, t1, phis1, t2, phis2);
            if needs {
                let mid_01 = 0.5 * (t0 + t1);
                let mid_12 = 0.5 * (t1 + t2);
                let phis_01 = solve_phi_at_theta_torus(phi_eq_fn, mid_01, phi_range);
                let phis_12 = solve_phi_at_theta_torus(phi_eq_fn, mid_12, phi_range);
                insertions.push((i + 1, mid_01, phis_01));
                insertions.push((i + 2, mid_12, phis_12));
            }
        }

        if insertions.is_empty() {
            break;
        }

        insertions.dedup_by_key(|item| item.0);
        for (idx, theta, phis) in insertions.into_iter().rev() {
            let already_exists = samples.iter().any(|(t, _)| (t - theta).abs() < 1e-15);
            if !already_exists {
                samples.insert(idx, (theta, phis));
            }
        }
    }
}

// ─── Sphere × Torus ─────────────────────────────────────

/// Sphere x Torus face intersection.
pub(crate) fn sphere_torus_face_intersection(
    sphere_face: &Face,
    sphere_shell: &Shell,
    torus_face: &Face,
    torus_shell: &Shell,
    _events: &mut Vec<BooleanEvent>,
) -> Vec<Curve3D> {
    let (sc, sr) = match &sphere_face.surface {
        Surface::Sphere { center, radius } => (*center, *radius),
        _ => return vec![],
    };
    let (tc, ta_raw, big_r, little_r) = match &torus_face.surface {
        Surface::Torus {
            center,
            axis,
            major_radius,
            minor_radius,
        } => (*center, *axis, *major_radius, *minor_radius),
        _ => return vec![],
    };

    let char_len = sr.min(little_r);
    let ta = vec3::normalized(ta_raw);
    let (bu, bv) = vec3::orthonormal_basis(ta_raw);

    let delta = vec3::sub(tc, sc);

    let d_dot_d = vec3::dot(delta, delta);
    let d_dot_a = vec3::dot(delta, ta);
    let d_dot_bu = vec3::dot(delta, bu);
    let d_dot_bv = vec3::dot(delta, bv);

    let phi_eq_fn = |theta: f64| -> Vec<f64> {
        let cos_t = theta.cos();
        let sin_t = theta.sin();

        let delta_dot_d = d_dot_bu * cos_t + d_dot_bv * sin_t;

        let c0 =
            d_dot_d + big_r * big_r + little_r * little_r + 2.0 * big_r * delta_dot_d - sr * sr;
        let c1 = 2.0 * little_r * (big_r + delta_dot_d);
        let c2 = 2.0 * little_r * d_dot_a;

        let a_coeff = c0 - c1;
        let b_coeff = 2.0 * c2;
        let c_coeff = c0 + c1;

        vec![c_coeff, b_coeff, a_coeff]
    };

    let eval_fn = |theta: f64, phi: f64| -> [f64; 3] {
        let cos_t = theta.cos();
        let sin_t = theta.sin();
        let dir = vec3::add(vec3::scale(bu, cos_t), vec3::scale(bv, sin_t));
        let r = big_r + little_r * phi.cos();
        vec3::add(
            vec3::add(tc, vec3::scale(dir, r)),
            vec3::scale(ta, little_r * phi.sin()),
        )
    };

    let polylines = sweep_torus_intersection(phi_eq_fn, eval_fn, (0.0, TAU), char_len);
    clip_or_convert_polylines(
        &polylines,
        sphere_face,
        sphere_shell,
        torus_face,
        torus_shell,
    )
}

// ─── Cylinder × Torus ───────────────────────────────────

/// Cylinder x Torus face intersection.
pub(crate) fn cylinder_torus_face_intersection(
    cyl_face: &Face,
    cyl_shell: &Shell,
    torus_face: &Face,
    torus_shell: &Shell,
    _events: &mut Vec<BooleanEvent>,
) -> Vec<Curve3D> {
    let (co, ca_raw, cr) = match &cyl_face.surface {
        Surface::Cylinder {
            origin,
            axis,
            radius,
        } => (*origin, *axis, *radius),
        _ => return vec![],
    };
    let (tc, ta_raw, big_r, little_r) = match &torus_face.surface {
        Surface::Torus {
            center,
            axis,
            major_radius,
            minor_radius,
        } => (*center, *axis, *major_radius, *minor_radius),
        _ => return vec![],
    };

    let char_len = cr.min(little_r);
    let ca = vec3::normalized(ca_raw);
    let ta = vec3::normalized(ta_raw);
    let (bu, bv) = vec3::orthonormal_basis(ta_raw);

    let delta = vec3::sub(tc, co);

    let d_dot_ca = vec3::dot(delta, ca);
    let a_dot_ca = vec3::dot(ta, ca);
    let bu_dot_ca = vec3::dot(bu, ca);
    let bv_dot_ca = vec3::dot(bv, ca);

    let delta_perp = vec3::sub(delta, vec3::scale(ca, d_dot_ca));
    let a_perp = vec3::sub(ta, vec3::scale(ca, a_dot_ca));
    let bu_perp = vec3::sub(bu, vec3::scale(ca, bu_dot_ca));
    let bv_perp = vec3::sub(bv, vec3::scale(ca, bv_dot_ca));

    let phi_eq_fn = |theta: f64| -> Vec<f64> {
        let cos_t = theta.cos();
        let sin_t = theta.sin();

        let d_perp = vec3::add(vec3::scale(bu_perp, cos_t), vec3::scale(bv_perp, sin_t));

        let p = vec3::add(delta_perp, vec3::scale(d_perp, big_r));
        let p_sq = vec3::dot(p, p);
        let d_perp_sq = vec3::dot(d_perp, d_perp);
        let a_perp_sq = vec3::dot(a_perp, a_perp);
        let p_dot_dp = vec3::dot(p, d_perp);
        let p_dot_ap = vec3::dot(p, a_perp);
        let dp_dot_ap = vec3::dot(d_perp, a_perp);

        let k0 = p_sq - cr * cr;
        let r2 = little_r * little_r;

        let pd = 2.0 * little_r * p_dot_dp;
        let pa = 2.0 * little_r * p_dot_ap;
        let da = 2.0 * r2 * dp_dot_ap;

        let c0 = k0 + r2 * d_perp_sq + pd;
        let c1 = 2.0 * pa + 2.0 * da;
        let c2 = 2.0 * k0 - 2.0 * r2 * d_perp_sq + 4.0 * r2 * a_perp_sq;
        let c3 = 2.0 * pa - 2.0 * da;
        let c4 = k0 + r2 * d_perp_sq - pd;

        vec![c0, c1, c2, c3, c4]
    };

    let eval_fn = |theta: f64, phi: f64| -> [f64; 3] {
        let cos_t = theta.cos();
        let sin_t = theta.sin();
        let dir = vec3::add(vec3::scale(bu, cos_t), vec3::scale(bv, sin_t));
        let r = big_r + little_r * phi.cos();
        vec3::add(
            vec3::add(tc, vec3::scale(dir, r)),
            vec3::scale(ta, little_r * phi.sin()),
        )
    };

    let polylines = sweep_torus_intersection(phi_eq_fn, eval_fn, (0.0, TAU), char_len);
    clip_or_convert_polylines(&polylines, cyl_face, cyl_shell, torus_face, torus_shell)
}

// ─── Cone × Torus ───────────────────────────────────────

/// Cone x Torus face intersection.
pub(crate) fn cone_torus_face_intersection(
    cone_face: &Face,
    cone_shell: &Shell,
    torus_face: &Face,
    torus_shell: &Shell,
    _events: &mut Vec<BooleanEvent>,
) -> Vec<Curve3D> {
    let (cv, ca_raw, half_angle) = match &cone_face.surface {
        Surface::Cone {
            origin,
            axis,
            half_angle,
        } => (*origin, *axis, *half_angle),
        _ => return vec![],
    };
    let (tc, ta_raw, big_r, little_r) = match &torus_face.surface {
        Surface::Torus {
            center,
            axis,
            major_radius,
            minor_radius,
        } => (*center, *axis, *major_radius, *minor_radius),
        _ => return vec![],
    };

    let char_len = little_r;
    let ca = vec3::normalized(ca_raw);
    let ta = vec3::normalized(ta_raw);
    let (bu, bv) = vec3::orthonormal_basis(ta_raw);
    let cos_a = half_angle.cos();
    let sin_a = half_angle.sin();
    let cos2_a = cos_a * cos_a;
    let sin2_a = sin_a * sin_a;

    let delta = vec3::sub(tc, cv);

    let d_dot_ca = vec3::dot(delta, ca);
    let a_dot_ca = vec3::dot(ta, ca);
    let bu_dot_ca = vec3::dot(bu, ca);
    let bv_dot_ca = vec3::dot(bv, ca);

    let delta_perp = vec3::sub(delta, vec3::scale(ca, d_dot_ca));
    let a_perp = vec3::sub(ta, vec3::scale(ca, a_dot_ca));
    let bu_perp = vec3::sub(bu, vec3::scale(ca, bu_dot_ca));
    let bv_perp = vec3::sub(bv, vec3::scale(ca, bv_dot_ca));

    let phi_eq_fn = |theta: f64| -> Vec<f64> {
        let cos_t = theta.cos();
        let sin_t = theta.sin();

        let d_perp = vec3::add(vec3::scale(bu_perp, cos_t), vec3::scale(bv_perp, sin_t));
        let d_dot_ca_theta = bu_dot_ca * cos_t + bv_dot_ca * sin_t;

        let p = vec3::add(delta_perp, vec3::scale(d_perp, big_r));
        let p_sq = vec3::dot(p, p);
        let d_perp_sq = vec3::dot(d_perp, d_perp);
        let a_perp_sq = vec3::dot(a_perp, a_perp);
        let p_dot_dp = vec3::dot(p, d_perp);
        let p_dot_ap = vec3::dot(p, a_perp);
        let dp_dot_ap = vec3::dot(d_perp, a_perp);

        let q0 = d_dot_ca + big_r * d_dot_ca_theta;
        let q1 = little_r * d_dot_ca_theta;
        let q2 = little_r * a_dot_ca;

        let r2 = little_r * little_r;
        let k0 = cos2_a * p_sq - sin2_a * q0 * q0;
        let k_cc = cos2_a * r2 * d_perp_sq - sin2_a * q1 * q1;
        let k_ss = cos2_a * r2 * a_perp_sq - sin2_a * q2 * q2;
        let k_c = cos2_a * 2.0 * little_r * p_dot_dp - sin2_a * 2.0 * q0 * q1;
        let k_s = cos2_a * 2.0 * little_r * p_dot_ap - sin2_a * 2.0 * q0 * q2;
        let k_cs = cos2_a * 2.0 * r2 * dp_dot_ap - sin2_a * 2.0 * q1 * q2;

        let c0 = k0 + k_cc + k_c;
        let c1 = 2.0 * k_s + 2.0 * k_cs;
        let c2 = 2.0 * k0 - 2.0 * k_cc + 4.0 * k_ss;
        let c3 = 2.0 * k_s - 2.0 * k_cs;
        let c4 = k0 + k_cc - k_c;

        vec![c0, c1, c2, c3, c4]
    };

    let eval_fn = |theta: f64, phi: f64| -> [f64; 3] {
        let cos_t = theta.cos();
        let sin_t = theta.sin();
        let dir = vec3::add(vec3::scale(bu, cos_t), vec3::scale(bv, sin_t));
        let r = big_r + little_r * phi.cos();
        vec3::add(
            vec3::add(tc, vec3::scale(dir, r)),
            vec3::scale(ta, little_r * phi.sin()),
        )
    };

    let polylines = sweep_torus_intersection(phi_eq_fn, eval_fn, (0.0, TAU), char_len);
    clip_or_convert_polylines(&polylines, cone_face, cone_shell, torus_face, torus_shell)
}

// ─── Ellipsoid × Torus ──────────────────────────────────

/// Ellipsoid x Torus face intersection.
pub(crate) fn ellipsoid_torus_face_intersection(
    ellipsoid_face: &Face,
    ellipsoid_shell: &Shell,
    torus_face: &Face,
    torus_shell: &Shell,
    _events: &mut Vec<BooleanEvent>,
) -> Vec<Curve3D> {
    let (ec, erx, ery, erz) = match &ellipsoid_face.surface {
        Surface::Ellipsoid { center, rx, ry, rz } => (*center, *rx, *ry, *rz),
        _ => return vec![],
    };
    let (tc, ta_raw, big_r, little_r) = match &torus_face.surface {
        Surface::Torus {
            center,
            axis,
            major_radius,
            minor_radius,
        } => (*center, *axis, *major_radius, *minor_radius),
        _ => return vec![],
    };

    let char_len = erx.min(ery).min(erz).min(little_r);
    let ta = vec3::normalized(ta_raw);
    let (bu, bv) = vec3::orthonormal_basis(ta_raw);

    let delta = vec3::sub(tc, ec);

    let inv_rx2 = 1.0 / (erx * erx);
    let inv_ry2 = 1.0 / (ery * ery);
    let inv_rz2 = 1.0 / (erz * erz);

    let scaled_dot = |u: [f64; 3], v: [f64; 3]| -> f64 {
        u[0] * v[0] * inv_rx2 + u[1] * v[1] * inv_ry2 + u[2] * v[2] * inv_rz2
    };

    let a_s_a = scaled_dot(ta, ta);
    let a_s_bu = scaled_dot(ta, bu);
    let a_s_bv = scaled_dot(ta, bv);
    let d_s_bu = scaled_dot(delta, bu);
    let d_s_bv = scaled_dot(delta, bv);
    let d_s_a = scaled_dot(delta, ta);
    let d_s_d = scaled_dot(delta, delta);
    let bu_s_bu = scaled_dot(bu, bu);
    let bv_s_bv = scaled_dot(bv, bv);
    let bu_s_bv = scaled_dot(bu, bv);

    let phi_eq_fn = |theta: f64| -> Vec<f64> {
        let cos_t = theta.cos();
        let sin_t = theta.sin();

        let d_s_dtheta =
            bu_s_bu * cos_t * cos_t + 2.0 * bu_s_bv * cos_t * sin_t + bv_s_bv * sin_t * sin_t;
        let d_s_atheta = a_s_bu * cos_t + a_s_bv * sin_t;

        let delta_dot_d_s = d_s_bu * cos_t + d_s_bv * sin_t;
        let p_dot_d_s = delta_dot_d_s + big_r * d_s_dtheta;
        let p_dot_a_s = d_s_a + big_r * d_s_atheta;
        let p_s_p = d_s_d + 2.0 * big_r * delta_dot_d_s + big_r * big_r * d_s_dtheta;

        let r2 = little_r * little_r;

        let k0 = p_s_p - 1.0;
        let k_cc = r2 * d_s_dtheta;
        let k_ss = r2 * a_s_a;
        let k_c = 2.0 * little_r * p_dot_d_s;
        let k_s = 2.0 * little_r * p_dot_a_s;
        let k_cs = 2.0 * r2 * d_s_atheta;

        let c0 = k0 + k_cc + k_c;
        let c1 = 2.0 * k_s + 2.0 * k_cs;
        let c2 = 2.0 * k0 - 2.0 * k_cc + 4.0 * k_ss;
        let c3 = 2.0 * k_s - 2.0 * k_cs;
        let c4 = k0 + k_cc - k_c;

        vec![c0, c1, c2, c3, c4]
    };

    let eval_fn = |theta: f64, phi: f64| -> [f64; 3] {
        let cos_t = theta.cos();
        let sin_t = theta.sin();
        let dir = vec3::add(vec3::scale(bu, cos_t), vec3::scale(bv, sin_t));
        let r = big_r + little_r * phi.cos();
        vec3::add(
            vec3::add(tc, vec3::scale(dir, r)),
            vec3::scale(ta, little_r * phi.sin()),
        )
    };

    let polylines = sweep_torus_intersection(phi_eq_fn, eval_fn, (0.0, TAU), char_len);
    clip_or_convert_polylines(
        &polylines,
        ellipsoid_face,
        ellipsoid_shell,
        torus_face,
        torus_shell,
    )
}

// ─── Torus × Torus ─────────────────────────────────────

/// Torus x Torus face intersection.
pub(crate) fn torus_torus_face_intersection(
    torus_a_face: &Face,
    torus_a_shell: &Shell,
    torus_b_face: &Face,
    torus_b_shell: &Shell,
    _events: &mut Vec<BooleanEvent>,
) -> Vec<Curve3D> {
    let (a_center, a_axis_raw, a_big_r, a_little_r) = match &torus_a_face.surface {
        Surface::Torus {
            center,
            axis,
            major_radius,
            minor_radius,
        } => (*center, *axis, *major_radius, *minor_radius),
        _ => return vec![],
    };
    let (b_center, b_axis_raw, b_big_r, b_little_r) = match &torus_b_face.surface {
        Surface::Torus {
            center,
            axis,
            major_radius,
            minor_radius,
        } => (*center, *axis, *major_radius, *minor_radius),
        _ => return vec![],
    };

    // Early return via bounding spheres
    let dist = vec3::length(vec3::sub(a_center, b_center));
    let bound_a = a_big_r + a_little_r;
    let bound_b = b_big_r + b_little_r;
    if dist > bound_a + bound_b + 1e-10 {
        return vec![];
    }

    let char_len = a_little_r.min(b_little_r);
    let a_axis = vec3::normalized(a_axis_raw);
    let (a_bu, a_bv) = vec3::orthonormal_basis(a_axis_raw);

    let b_axis = vec3::normalized(b_axis_raw);
    let (b_bx, b_by) = vec3::orthonormal_basis(b_axis_raw);

    let delta = vec3::sub(a_center, b_center);

    let abu_lx = vec3::dot(a_bu, b_bx);
    let abu_ly = vec3::dot(a_bu, b_by);
    let abu_lz = vec3::dot(a_bu, b_axis);
    let abv_lx = vec3::dot(a_bv, b_bx);
    let abv_ly = vec3::dot(a_bv, b_by);
    let abv_lz = vec3::dot(a_bv, b_axis);

    let aa_lx = vec3::dot(a_axis, b_bx);
    let aa_ly = vec3::dot(a_axis, b_by);
    let aa_lz = vec3::dot(a_axis, b_axis);

    let dl_x = vec3::dot(delta, b_bx);
    let dl_y = vec3::dot(delta, b_by);
    let dl_z = vec3::dot(delta, b_axis);

    let b_rr = b_big_r * b_big_r;
    let b_r2 = b_little_r * b_little_r;

    let phi_eq_fn = |theta: f64| -> Vec<f64> {
        let cos_t = theta.cos();
        let sin_t = theta.sin();

        let dx = abu_lx * cos_t + abv_lx * sin_t;
        let dy = abu_ly * cos_t + abv_ly * sin_t;
        let dz = abu_lz * cos_t + abv_lz * sin_t;

        let px = dl_x + a_big_r * dx;
        let py = dl_y + a_big_r * dy;
        let pz = dl_z + a_big_r * dz;

        let cx = a_little_r * dx;
        let cy = a_little_r * dy;
        let cz = a_little_r * dz;

        let sx = a_little_r * aa_lx;
        let sy = a_little_r * aa_ly;
        let sz = a_little_r * aa_lz;

        let pp = px * px + py * py + pz * pz;
        let cc = cx * cx + cy * cy + cz * cz;
        let ss = sx * sx + sy * sy + sz * sz;
        let pc = px * cx + py * cy + pz * cz;
        let ps = px * sx + py * sy + pz * sz;
        let cs = cx * sx + cy * sy + cz * sz;

        let pp_xy = px * px + py * py;
        let cc_xy = cx * cx + cy * cy;
        let ss_xy = sx * sx + sy * sy;
        let pc_xy = px * cx + py * cy;
        let ps_xy = px * sx + py * sy;
        let cs_xy = cx * sx + cy * sy;

        let q0 = pp + b_rr - b_r2;
        let q_cc = cc;
        let q_ss = ss;
        let q_c = 2.0 * pc;
        let q_s = 2.0 * ps;
        let q_cs = 2.0 * cs;

        let m0 = pp_xy;
        let m_cc = cc_xy;
        let m_ss = ss_xy;
        let m_c = 2.0 * pc_xy;
        let m_s = 2.0 * ps_xy;
        let m_cs = 2.0 * cs_xy;

        let qp0 = q0 + q_cc + q_c;
        let qp1 = 2.0 * q_s + 2.0 * q_cs;
        let qp2 = 2.0 * q0 - 2.0 * q_cc + 4.0 * q_ss;
        let qp3 = 2.0 * q_s - 2.0 * q_cs;
        let qp4 = q0 + q_cc - q_c;

        let mp0 = m0 + m_cc + m_c;
        let mp1 = 2.0 * m_s + 2.0 * m_cs;
        let mp2 = 2.0 * m0 - 2.0 * m_cc + 4.0 * m_ss;
        let mp3 = 2.0 * m_s - 2.0 * m_cs;
        let mp4 = m0 + m_cc - m_c;

        let qp = [qp0, qp1, qp2, qp3, qp4];

        let mut qp_sq = [0.0_f64; 9];
        for i in 0..5 {
            for j in 0..5 {
                qp_sq[i + j] += qp[i] * qp[j];
            }
        }

        let u = [1.0, 0.0, 2.0, 0.0, 1.0_f64];
        let mp_arr = [mp0, mp1, mp2, mp3, mp4];

        let mut mp_u = [0.0_f64; 9];
        for i in 0..5 {
            for j in 0..5 {
                mp_u[i + j] += mp_arr[i] * u[j];
            }
        }

        let four_rr = 4.0 * b_rr;
        let mut coeffs = Vec::with_capacity(9);
        for k in 0..9 {
            coeffs.push(qp_sq[k] - four_rr * mp_u[k]);
        }

        coeffs
    };

    let eval_fn = |theta: f64, phi: f64| -> [f64; 3] {
        let cos_t = theta.cos();
        let sin_t = theta.sin();
        let dir = vec3::add(vec3::scale(a_bu, cos_t), vec3::scale(a_bv, sin_t));
        let r = a_big_r + a_little_r * phi.cos();
        vec3::add(
            vec3::add(a_center, vec3::scale(dir, r)),
            vec3::scale(a_axis, a_little_r * phi.sin()),
        )
    };

    let polylines = sweep_torus_intersection(phi_eq_fn, eval_fn, (0.0, TAU), char_len);
    clip_or_convert_polylines(
        &polylines,
        torus_a_face,
        torus_a_shell,
        torus_b_face,
        torus_b_shell,
    )
}

// ─── Tests ──────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use std::f64::consts::TAU;

    #[test]
    fn sweep_sphere_cylinder_known_case() {
        let coeff_fn = |theta: f64| -> (f64, f64, f64) {
            let gamma = 0.5 * theta.cos() - 0.5;
            (1.0, 0.0, gamma)
        };

        let eval_fn =
            |theta: f64, h: f64| -> [f64; 3] { [0.5 + 0.5 * theta.cos(), 0.5 * theta.sin(), h] };

        let h_bounds = |_theta: f64| -> (f64, f64) { (-1.0, 1.0) };

        let curves = sweep_quadric_intersection(coeff_fn, eval_fn, (0.0, TAU), h_bounds, 1.0);

        assert!(
            !curves.is_empty(),
            "intersection curves detected ({} curves)",
            curves.len()
        );

        for curve in &curves {
            for pt in curve {
                let r2 = pt[0] * pt[0] + pt[1] * pt[1] + pt[2] * pt[2];
                assert!(
                    (r2 - 1.0).abs() < 1e-6,
                    "point ({}, {}, {}) not on sphere: r^2={r2}",
                    pt[0],
                    pt[1],
                    pt[2],
                );
            }
        }

        for curve in &curves {
            for pt in curve {
                let dx = pt[0] - 0.5;
                let cyl_r2 = dx * dx + pt[1] * pt[1];
                assert!(
                    (cyl_r2 - 0.25).abs() < 1e-6,
                    "point not on cylinder: cyl_r^2={cyl_r2}",
                );
            }
        }
    }

    #[test]
    fn discriminant_zeros_basic() {
        let coeff_fn = |theta: f64| -> (f64, f64, f64) { (1.0, 0.0, theta.cos()) };

        let zero = bisect_discriminant_zero(&coeff_fn, 1.0, 2.0);
        assert!(
            (zero - std::f64::consts::FRAC_PI_2).abs() < 1e-10,
            "discriminant zero near theta=pi/2: theta={zero}",
        );

        let three_half_pi = 3.0 * std::f64::consts::FRAC_PI_2;
        let zero2 = bisect_discriminant_zero(&coeff_fn, 4.0, 5.0);
        assert!(
            (zero2 - three_half_pi).abs() < 1e-10,
            "discriminant zero near theta=3pi/2: theta={zero2}",
        );
    }

    #[test]
    fn needs_refinement_basic() {
        let p0 = [0.0, 0.0, 0.0];
        let p1 = [1.0, 0.0, 0.0];
        let p2 = [2.0, 0.0, 0.0];
        assert!(!needs_refinement(p0, p1, p2, ANGLE_THRESHOLD));

        let p2_bent = [1.0, 1.0, 0.0];
        assert!(needs_refinement(p0, p1, p2_bent, ANGLE_THRESHOLD));
    }

    #[test]
    fn sweep_spherical_basic() {
        let phi_eq_fn = |_theta: f64| -> Vec<f64> { vec![1.0, -4.0, 1.0] };

        let eval_fn = |theta: f64, phi: f64| -> [f64; 3] {
            [phi.cos() * theta.cos(), phi.cos() * theta.sin(), phi.sin()]
        };

        let curves = sweep_spherical_intersection(phi_eq_fn, eval_fn, (0.0, TAU), 1.0);

        assert!(!curves.is_empty(), "intersection curves detected");

        for curve in &curves {
            for pt in curve {
                let r2 = pt[0] * pt[0] + pt[1] * pt[1] + pt[2] * pt[2];
                assert!(
                    (r2 - 1.0).abs() < 1e-6,
                    "point not on unit sphere: r^2={r2}"
                );
            }
        }
    }
}
