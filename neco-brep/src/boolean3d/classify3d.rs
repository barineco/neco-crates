//! Point-in-face and point-in-shell tests via ray casting

use crate::brep::{eval_revolution_profile, find_closest_v_on_profile, Face, Shell, Surface};
use crate::vec3::{self, newton_root, orthonormal_basis};
use neco_nurbs::solve_quadratic;
use neco_nurbs::NurbsSurface3D;

use super::tolerance::GEO_TOL;
const PARALLEL_TOL: f64 = GEO_TOL;
const EPS: f64 = 1e-9;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OverlapClass {
    SameDirection,
    OppositeDirection,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Location3D {
    Inside,
    Outside,
    Boundary(OverlapClass),
}

/// Axis-aligned ray direction
#[derive(Debug, Clone, Copy)]
enum RayAxis {
    X,
    Y,
    Z,
}

impl RayAxis {
    /// Unit vector in the ray direction
    fn dir(&self) -> [f64; 3] {
        match self {
            RayAxis::X => [1.0, 0.0, 0.0],
            RayAxis::Y => [0.0, 1.0, 0.0],
            RayAxis::Z => [0.0, 0.0, 1.0],
        }
    }

    /// Point on ray: origin + t * dir
    fn hit(&self, origin: &[f64; 3], t: f64) -> [f64; 3] {
        match self {
            RayAxis::X => [origin[0] + t, origin[1], origin[2]],
            RayAxis::Y => [origin[0], origin[1] + t, origin[2]],
            RayAxis::Z => [origin[0], origin[1], origin[2] + t],
        }
    }

    /// Ray-direction component of a vector
    fn component(&self, v: &[f64; 3]) -> f64 {
        match self {
            RayAxis::X => v[0],
            RayAxis::Y => v[1],
            RayAxis::Z => v[2],
        }
    }

    /// Two perpendicular components (perp1, perp2)
    fn perp_components(&self, v: &[f64; 3]) -> (f64, f64) {
        match self {
            RayAxis::X => (v[1], v[2]),
            RayAxis::Y => (v[0], v[2]),
            RayAxis::Z => (v[0], v[1]),
        }
    }

    /// Check if perpendicular components are within AABB range
    fn perp_in_range(&self, origin: &[f64; 3], bb_min: &[f64; 3], bb_max: &[f64; 3]) -> bool {
        let (o1, o2) = self.perp_components(origin);
        let (min1, min2) = self.perp_components(bb_min);
        let (max1, max2) = self.perp_components(bb_max);
        o1 >= min1 && o1 <= max1 && o2 >= min2 && o2 <= max2
    }
}

// ────────── Ray-surface intersection helpers ──────────

/// Ray-sphere intersection parameter t
fn ray_sphere_intersect(
    ray_origin: &[f64; 3],
    center: &[f64; 3],
    radius: f64,
    axis: RayAxis,
) -> Vec<f64> {
    let o = vec3::sub(*ray_origin, *center);
    let dir = axis.dir();
    let b = 2.0 * vec3::dot(o, dir);
    let c = vec3::dot(o, o) - radius * radius;
    solve_quadratic(1.0, b, c)
        .into_iter()
        .filter(|&t| t > EPS)
        .collect()
}

/// Ray-ellipsoid intersection parameter t
fn ray_ellipsoid_intersect(
    ray_origin: &[f64; 3],
    center: &[f64; 3],
    rx: f64,
    ry: f64,
    rz: f64,
    axis: RayAxis,
) -> Vec<f64> {
    // Transform to scaled space and reduce to unit sphere intersection
    let o = [
        (ray_origin[0] - center[0]) / rx,
        (ray_origin[1] - center[1]) / ry,
        (ray_origin[2] - center[2]) / rz,
    ];
    let raw_dir = axis.dir();
    let d = [raw_dir[0] / rx, raw_dir[1] / ry, raw_dir[2] / rz];
    let a = vec3::dot(d, d);
    let b = 2.0 * vec3::dot(o, d);
    let c = vec3::dot(o, o) - 1.0;
    solve_quadratic(a, b, c)
        .into_iter()
        .filter(|&t| t > EPS)
        .collect()
}

/// Ray-torus intersection parameter t
fn ray_torus_intersect(
    ray_origin: &[f64; 3],
    torus_center: &[f64; 3],
    torus_axis: &[f64; 3],
    major_radius: f64,
    minor_radius: f64,
    axis: RayAxis,
) -> Vec<f64> {
    let o = vec3::sub(*ray_origin, *torus_center);

    // Local frame: axis -> e_y, u -> e_x, v -> e_z
    let u = if torus_axis[0].abs() < 0.9 {
        vec3::normalized(vec3::cross(*torus_axis, [1.0, 0.0, 0.0]))
    } else {
        vec3::normalized(vec3::cross(*torus_axis, [0.0, 0.0, 1.0]))
    };
    let v = vec3::cross(*torus_axis, u);

    let local_o = [vec3::dot(o, u), vec3::dot(o, *torus_axis), vec3::dot(o, v)];
    let ray_dir = axis.dir();
    let local_d = [
        vec3::dot(ray_dir, u),
        vec3::dot(ray_dir, *torus_axis),
        vec3::dot(ray_dir, v),
    ];

    solve_ray_torus_quartic(&local_o, &local_d, major_radius, minor_radius)
}

/// Numerically solve ray-torus intersection
fn solve_ray_torus_quartic(o: &[f64; 3], d: &[f64; 3], big_r: f64, small_r: f64) -> Vec<f64> {
    let f = |t: f64| -> f64 {
        let px = o[0] + t * d[0];
        let py = o[1] + t * d[1];
        let pz = o[2] + t * d[2];
        let sum_sq = px * px + py * py + pz * pz;
        let s = sum_sq + big_r * big_r - small_r * small_r;
        s * s - 4.0 * big_r * big_r * (px * px + pz * pz)
    };

    let df = |t: f64| -> f64 {
        let px = o[0] + t * d[0];
        let py = o[1] + t * d[1];
        let pz = o[2] + t * d[2];
        let sum_sq = px * px + py * py + pz * pz;
        let s = sum_sq + big_r * big_r - small_r * small_r;
        let ds = 2.0 * (px * d[0] + py * d[1] + pz * d[2]);
        let d_planar = 2.0 * (px * d[0] + pz * d[2]);
        2.0 * s * ds - 4.0 * big_r * big_r * d_planar
    };

    // Sample along ray to detect sign changes
    let t_max = 2.0 * (big_r + small_r + vec3::length(*o));
    let n_samples = 100;
    let mut roots = Vec::new();

    let mut prev_f = f(0.0);
    for i in 1..=n_samples {
        let t = t_max * i as f64 / n_samples as f64;
        let cur_f = f(t);
        if prev_f * cur_f < 0.0 {
            let t0 = t_max * (i - 1) as f64 / n_samples as f64;
            if let Some(root) = newton_root(t0, t, &f, &df) {
                if root > EPS {
                    roots.push(root);
                }
            }
        }
        prev_f = cur_f;
    }

    roots
}

/// Ray-SurfaceOfRevolution intersection parameters.
///
/// Converts ray to local frame, samples v, solves circle-ray per v,
/// detects z-residual sign changes, and refines via Newton-bisection.
#[allow(clippy::too_many_arguments)]
fn ray_revolution_intersect(
    ray_origin: &[f64; 3],
    center: &[f64; 3],
    axis_vec: &[f64; 3],
    frame_u: &[f64; 3],
    frame_v: &[f64; 3],
    profile_control_points: &[[f64; 2]],
    profile_weights: &[f64],
    profile_degree: u32,
    n_profile_spans: u32,
    ray_axis: RayAxis,
) -> Vec<f64> {
    let o = vec3::sub(*ray_origin, *center);
    let axis_n = vec3::normalized(*axis_vec);
    let ray_dir = ray_axis.dir();

    // Transform to local frame
    let local_ox = vec3::dot(o, *frame_u);
    let local_oy = vec3::dot(o, *frame_v);
    let local_oz = vec3::dot(o, axis_n);

    let local_dx = vec3::dot(ray_dir, *frame_u);
    let local_dy = vec3::dot(ray_dir, *frame_v);
    let local_dz = vec3::dot(ray_dir, axis_n);

    // z(t) = oz + dz*t
    let z_ray = |t: f64| -> f64 { local_oz + local_dz * t };

    // Estimate r,z range of profile and build bounding sphere
    let n_sample_bound = 32;
    let mut r_max: f64 = 0.0;
    let mut z_min = f64::INFINITY;
    let mut z_max = f64::NEG_INFINITY;
    for i in 0..=n_sample_bound {
        let v = i as f64 / n_sample_bound as f64;
        let (r, z) = eval_revolution_profile(
            profile_control_points,
            profile_weights,
            profile_degree,
            n_profile_spans,
            v,
        );
        r_max = r_max.max(r.abs());
        z_min = z_min.min(z);
        z_max = z_max.max(z);
    }

    // Bounding sphere: center (0, 0, z_mid), radius = sqrt(r_max^2 + half_z^2)
    let z_mid = (z_min + z_max) * 0.5;
    let half_z = (z_max - z_min) * 0.5;
    let bound_r = (r_max * r_max + half_z * half_z).sqrt() * 1.1 + 1e-6;

    // Intersection interval with bounding sphere
    let o_shifted_z = local_oz - z_mid;
    let oo = local_ox * local_ox + local_oy * local_oy + o_shifted_z * o_shifted_z;
    let od = local_ox * local_dx + local_oy * local_dy + o_shifted_z * local_dz;
    let dd = local_dx * local_dx + local_dy * local_dy + local_dz * local_dz;

    let disc = od * od - dd * (oo - bound_r * bound_r);
    if disc < 0.0 {
        return Vec::new();
    }
    let sqrt_disc = disc.sqrt();
    let inv_dd = 1.0 / dd;
    let t_entry = ((-od - sqrt_disc) * inv_dd).max(EPS);
    let t_exit = (-od + sqrt_disc) * inv_dd;
    if t_exit < EPS {
        return Vec::new();
    }

    // Compute t candidates for each v sample
    // Circle-ray: (ox+dx*t)^2 + (oy+dy*t)^2 = r(v)^2
    let a_coeff = local_dx * local_dx + local_dy * local_dy;
    let b_half = local_ox * local_dx + local_oy * local_dy;
    let c_base = local_ox * local_ox + local_oy * local_oy;

    // For each v sample, solve circle intersection for t, detect z-residual sign changes
    let n_v_samples: usize = 16;
    let n_prof = n_profile_spans as usize;

    let mut roots: Vec<f64> = Vec::new();

    // Process each span independently
    for span_idx in 0..n_prof {
        let v_lo = span_idx as f64 / n_prof as f64;
        let v_hi = (span_idx + 1) as f64 / n_prof as f64;

        // Special case: ray passes through axis (r ~ 0)
        if a_coeff.abs() < 1e-15 && c_base.sqrt() < 1e-8 && local_dz.abs() > 1e-15 {
            for iv in 0..=n_v_samples {
                let v = v_lo + (v_hi - v_lo) * iv as f64 / n_v_samples as f64;
                let (r_v, z_v) = eval_revolution_profile(
                    profile_control_points,
                    profile_weights,
                    profile_degree,
                    n_profile_spans,
                    v,
                );
                if r_v.abs() < 1e-15 {
                    let t_cand = (z_v - local_oz) / local_dz;
                    if t_cand > EPS && t_cand >= t_entry && t_cand <= t_exit {
                        roots.push(t_cand);
                    }
                }
            }
        }

        // Scan v for each sign (-1, +1), detect z_residual sign changes
        for &sign in &[-1.0_f64, 1.0] {
            let mut prev: Option<(f64, f64, f64)> = None; // (v, t, z_residual)

            for iv in 0..=n_v_samples {
                let v = v_lo + (v_hi - v_lo) * iv as f64 / n_v_samples as f64;
                let (r_v, z_v) = eval_revolution_profile(
                    profile_control_points,
                    profile_weights,
                    profile_degree,
                    n_profile_spans,
                    v,
                );

                if r_v.abs() < 1e-15 {
                    prev = None;
                    continue;
                }

                let c_coeff = c_base - r_v * r_v;
                let disc_cyl = b_half * b_half - a_coeff * c_coeff;
                if disc_cyl < 0.0 {
                    prev = None;
                    continue;
                }

                let sqrt_disc_cyl = disc_cyl.sqrt();
                let inv_a = if a_coeff.abs() > 1e-15 {
                    1.0 / a_coeff
                } else {
                    prev = None;
                    continue;
                };

                let t_cand = (-b_half + sign * sqrt_disc_cyl) * inv_a;
                let z_residual = z_ray(t_cand) - z_v;

                if let Some((prev_v, prev_t, prev_res)) = prev {
                    if prev_res * z_residual < 0.0 && t_cand > EPS && prev_t > EPS {
                        // Sign change detected -> refine via Newton-bisection
                        let v_lo_bracket = prev_v;
                        let v_hi_bracket = v;
                        if let Some(root_t) = refine_revolution_root(
                            local_ox,
                            local_oy,
                            local_oz,
                            local_dx,
                            local_dy,
                            local_dz,
                            a_coeff,
                            b_half,
                            c_base,
                            sign,
                            profile_control_points,
                            profile_weights,
                            profile_degree,
                            n_profile_spans,
                            v_lo_bracket,
                            v_hi_bracket,
                        ) {
                            if root_t > EPS {
                                roots.push(root_t);
                            }
                        }
                    }
                }

                prev = Some((v, t_cand, z_residual));
            }
        }
    }

    // Deduplicate and filter positive only
    roots.sort_by(|a, b| a.total_cmp(b));
    roots.dedup_by(|a, b| (*a - *b).abs() < 1e-8);
    roots.into_iter().filter(|&t| t > EPS).collect()
}

/// Newton-bisection refinement for revolution intersection.
/// Given a bracket [v_lo, v_hi] with z_residual sign change, find precise (t, v).
#[allow(clippy::too_many_arguments)]
fn refine_revolution_root(
    _ox: f64,
    _oy: f64,
    oz: f64,
    _dx: f64,
    _dy: f64,
    dz: f64,
    a_coeff: f64,
    b_half: f64,
    c_base: f64,
    sign: f64,
    profile_control_points: &[[f64; 2]],
    profile_weights: &[f64],
    profile_degree: u32,
    n_profile_spans: u32,
    mut v_lo: f64,
    mut v_hi: f64,
) -> Option<f64> {
    let z_residual_at_v = |v: f64| -> Option<(f64, f64)> {
        let (r_v, z_v) = eval_revolution_profile(
            profile_control_points,
            profile_weights,
            profile_degree,
            n_profile_spans,
            v,
        );
        if r_v.abs() < 1e-15 {
            return None;
        }
        let c_coeff = c_base - r_v * r_v;
        let disc = b_half * b_half - a_coeff * c_coeff;
        if disc < 0.0 {
            return None;
        }
        let inv_a = if a_coeff.abs() > 1e-15 {
            1.0 / a_coeff
        } else {
            return None;
        };
        let t = (-b_half + sign * disc.sqrt()) * inv_a;
        let z_res = (oz + dz * t) - z_v;
        Some((t, z_res))
    };

    let (_, res_lo) = z_residual_at_v(v_lo)?;
    let (_, res_hi) = z_residual_at_v(v_hi)?;

    // Verify bracket condition (same sign -> cannot refine)
    if res_lo * res_hi > 0.0 {
        return None;
    }

    // Normalize so res_lo < 0
    if res_lo > 0.0 {
        std::mem::swap(&mut v_lo, &mut v_hi);
    }

    for _ in 0..30 {
        let v_mid = (v_lo + v_hi) * 0.5;
        let (t_mid, res_mid) = z_residual_at_v(v_mid)?;
        if res_mid.abs() < 1e-10 {
            return Some(t_mid);
        }
        if res_mid < 0.0 {
            v_lo = v_mid;
        } else {
            v_hi = v_mid;
        }
        if (v_hi - v_lo).abs() < 1e-12 {
            return Some(t_mid);
        }
    }

    let v_mid = (v_lo + v_hi) * 0.5;
    z_residual_at_v(v_mid).map(|(t, _)| t)
}

/// Test if a point on SurfaceOfRevolution is inside the face boundary.
/// Inverse-projects to (theta, v) parameter space and uses 2D ray casting.
#[allow(clippy::too_many_arguments)]
fn point_in_face_revolution(
    p: &[f64; 3],
    face: &Face,
    shell: &Shell,
    center: &[f64; 3],
    axis_vec: &[f64; 3],
    frame_u: &[f64; 3],
    frame_v: &[f64; 3],
    profile_control_points: &[[f64; 2]],
    profile_weights: &[f64],
    profile_degree: u32,
    n_profile_spans: u32,
) -> bool {
    // Full-revolution surface with single seam edge -> entire face is valid
    if face.loop_edges.len() == 1 {
        let edge = &shell.edges[face.loop_edges[0].edge_id];
        if edge.v_start == edge.v_end {
            return true;
        }
    }

    let axis_n = vec3::normalized(*axis_vec);

    // 3D -> (theta, v) conversion
    let to_theta_v = |pt: &[f64; 3]| -> (f64, f64) {
        let q = vec3::sub(*pt, *center);
        let qu = vec3::dot(q, *frame_u);
        let qv = vec3::dot(q, *frame_v);
        let theta = qv.atan2(qu);

        // v: find closest profile parameter via Newton
        let rho = (qu * qu + qv * qv).sqrt();
        let z = vec3::dot(q, axis_n);

        let v = find_closest_v_on_profile(
            rho,
            z,
            profile_control_points,
            profile_weights,
            profile_degree,
            n_profile_spans,
        );

        (theta, v)
    };

    // Project face loop vertices to (theta, v) space
    let mut raw_poly = Vec::new();
    for edge_ref in &face.loop_edges {
        let edge = &shell.edges[edge_ref.edge_id];
        let vid = if edge_ref.forward {
            edge.v_start
        } else {
            edge.v_end
        };
        raw_poly.push(to_theta_v(&shell.vertices[vid]));

        // Sample curved or clipped line edges so trim boundaries contribute to the polygon.
        if matches!(
            &edge.curve,
            crate::brep::Curve3D::Arc { .. } | crate::brep::Curve3D::Line { .. }
        ) {
            let (t0, t1) = edge.curve.param_range();
            let n_samples = 16;
            for k in 1..n_samples {
                let frac = k as f64 / n_samples as f64;
                let t = if edge_ref.forward {
                    t0 + frac * (t1 - t0)
                } else {
                    t1 - frac * (t1 - t0)
                };
                let pt = edge.curve.evaluate(t);
                raw_poly.push(to_theta_v(&pt));
            }
        }
    }

    if raw_poly.is_empty() {
        return false;
    }

    // Unwrap angles for continuity
    let ref_theta = raw_poly[0].0;
    let unwrap_angle = |angle: f64, reference: f64| -> f64 {
        let mut d = angle - reference;
        while d > std::f64::consts::PI {
            d -= std::f64::consts::TAU;
        }
        while d < -std::f64::consts::PI {
            d += std::f64::consts::TAU;
        }
        reference + d
    };

    let poly: Vec<(f64, f64)> = raw_poly
        .iter()
        .map(|&(t, v)| (unwrap_angle(t, ref_theta), v))
        .collect();

    let (p_theta, p_v) = to_theta_v(p);
    let p_theta = unwrap_angle(p_theta, ref_theta);

    ray_cast_2d((p_theta, p_v), &poly)
}

/// Ray-infinite-cylinder intersection parameter t
fn ray_cylinder_intersect(
    ray_origin: &[f64; 3],
    cyl_origin: &[f64; 3],
    cyl_axis: &[f64; 3],
    radius: f64,
    axis: RayAxis,
) -> Vec<f64> {
    let d = axis.dir();
    let o = vec3::sub(*ray_origin, *cyl_origin);

    let d_dot_a = vec3::dot(d, *cyl_axis);
    let o_dot_a = vec3::dot(o, *cyl_axis);

    // d_perp = d - (d·a)*a
    let d_perp = vec3::sub(d, vec3::scale(*cyl_axis, d_dot_a));
    // o_perp = o - (o·a)*a
    let o_perp = vec3::sub(o, vec3::scale(*cyl_axis, o_dot_a));

    let a_coeff = vec3::dot(d_perp, d_perp);
    let b_coeff = 2.0 * vec3::dot(d_perp, o_perp);
    let c_coeff = vec3::dot(o_perp, o_perp) - radius * radius;

    solve_quadratic(a_coeff, b_coeff, c_coeff)
        .into_iter()
        .filter(|&t| t > EPS)
        .collect()
}

/// Ray-infinite-cone intersection parameter t
fn ray_cone_intersect(
    ray_origin: &[f64; 3],
    cone_origin: &[f64; 3],
    cone_axis: &[f64; 3],
    half_angle: f64,
    axis: RayAxis,
) -> Vec<f64> {
    let d = axis.dir();
    let o = vec3::sub(*ray_origin, *cone_origin);

    let k = half_angle.tan().powi(2);

    let d_dot_a = vec3::dot(d, *cone_axis);
    let o_dot_a = vec3::dot(o, *cone_axis);

    let d_perp = vec3::sub(d, vec3::scale(*cone_axis, d_dot_a));
    let o_perp = vec3::sub(o, vec3::scale(*cone_axis, o_dot_a));

    let a_coeff = vec3::dot(d_perp, d_perp) - k * d_dot_a * d_dot_a;
    let b_coeff = 2.0 * (vec3::dot(d_perp, o_perp) - k * d_dot_a * o_dot_a);
    let c_coeff = vec3::dot(o_perp, o_perp) - k * o_dot_a * o_dot_a;

    let ts = solve_quadratic(a_coeff, b_coeff, c_coeff);

    // Cone extends only in axis direction from apex -> q*a > 0
    ts.into_iter()
        .filter(|&t| {
            t > EPS && {
                let hit = axis.hit(ray_origin, t);
                let q = vec3::sub(hit, *cone_origin);
                vec3::dot(q, *cone_axis) > 0.0
            }
        })
        .collect()
}

// ────────── Point-in-face for curved surfaces ──────────

/// Test if a point on a cylinder is within the face boundary.
fn point_in_face_cylinder(
    p: &[f64; 3],
    face: &Face,
    shell: &Shell,
    cyl_origin: &[f64; 3],
    cyl_axis: &[f64; 3],
    _radius: f64,
) -> bool {
    let q = vec3::sub(*p, *cyl_origin);
    let s = vec3::dot(q, *cyl_axis);

    // Collect s range from face boundary vertices
    let (s_min, s_max) = face_s_range(face, shell, cyl_origin, cyl_axis);

    if is_full_rotation_face(face, shell) {
        // Full-revolution: s-range check only
        return s >= s_min - EPS && s <= s_max + EPS;
    }

    // Partial revolution: 2D ray cast in (theta, s) space
    let (u_basis, v_basis) = orthonormal_basis(*cyl_axis);
    let theta = compute_theta(&q, cyl_axis, &u_basis, &v_basis);

    point_in_face_parametric(
        theta, s, face, shell, cyl_origin, cyl_axis, &u_basis, &v_basis,
    )
}

/// Test if a point on a cone is within the face boundary.
fn point_in_face_cone(
    p: &[f64; 3],
    face: &Face,
    shell: &Shell,
    cone_origin: &[f64; 3],
    cone_axis: &[f64; 3],
    _half_angle: f64,
) -> bool {
    let q = vec3::sub(*p, *cone_origin);
    let s = vec3::dot(q, *cone_axis);

    let (s_min, s_max) = face_s_range(face, shell, cone_origin, cone_axis);

    if is_full_rotation_face(face, shell) {
        return s >= s_min - EPS && s <= s_max + EPS;
    }

    let (u_basis, v_basis) = orthonormal_basis(*cone_axis);
    let theta = compute_theta(&q, cone_axis, &u_basis, &v_basis);

    point_in_face_parametric(
        theta,
        s,
        face,
        shell,
        cone_origin,
        cone_axis,
        &u_basis,
        &v_basis,
    )
}

/// Test if a point on a sphere is within the face boundary.
/// Builds a spherical polygon via Arc sampling, then projects to tangent plane for 2D ray cast.
fn point_in_face_sphere(
    p: &[f64; 3],
    face: &Face,
    shell: &Shell,
    center: &[f64; 3],
    _radius: f64,
) -> bool {
    let q = vec3::normalized(vec3::sub(*p, *center));

    // Build polygon on unit sphere by sampling Arc edges
    let mut poly = Vec::new();
    for edge_ref in &face.loop_edges {
        let edge = &shell.edges[edge_ref.edge_id];
        let vid = if edge_ref.forward {
            edge.v_start
        } else {
            edge.v_end
        };
        poly.push(vec3::normalized(vec3::sub(shell.vertices[vid], *center)));

        // Sample Arc edges at intermediate points
        if let crate::brep::Curve3D::Arc { .. } = &edge.curve {
            let (t0, t1) = edge.curve.param_range();
            let n_samples = 16;
            for k in 1..n_samples {
                let frac = k as f64 / n_samples as f64;
                let t = if edge_ref.forward {
                    t0 + frac * (t1 - t0)
                } else {
                    t1 - frac * (t1 - t0)
                };
                let pt = edge.curve.evaluate(t);
                poly.push(vec3::normalized(vec3::sub(pt, *center)));
            }
        }
    }

    point_in_spherical_polygon(&q, &poly)
}

/// Test if a point on an ellipsoid is within the face boundary.
/// Transforms to scaled unit-sphere space.
fn point_in_face_ellipsoid(
    p: &[f64; 3],
    face: &Face,
    shell: &Shell,
    center: &[f64; 3],
    rx: f64,
    ry: f64,
    rz: f64,
) -> bool {
    let q = vec3::normalized([
        (p[0] - center[0]) / rx,
        (p[1] - center[1]) / ry,
        (p[2] - center[2]) / rz,
    ]);

    // Build polygon on scaled unit sphere by sampling Arc edges
    let mut poly = Vec::new();
    for edge_ref in &face.loop_edges {
        let edge = &shell.edges[edge_ref.edge_id];
        let vid = if edge_ref.forward {
            edge.v_start
        } else {
            edge.v_end
        };
        let v = &shell.vertices[vid];
        poly.push(vec3::normalized([
            (v[0] - center[0]) / rx,
            (v[1] - center[1]) / ry,
            (v[2] - center[2]) / rz,
        ]));

        // Sample Arc edges at intermediate points
        if let crate::brep::Curve3D::Arc { .. } = &edge.curve {
            let (t0, t1) = edge.curve.param_range();
            let n_samples = 16;
            for k in 1..n_samples {
                let frac = k as f64 / n_samples as f64;
                let t = if edge_ref.forward {
                    t0 + frac * (t1 - t0)
                } else {
                    t1 - frac * (t1 - t0)
                };
                let pt = edge.curve.evaluate(t);
                poly.push(vec3::normalized([
                    (pt[0] - center[0]) / rx,
                    (pt[1] - center[1]) / ry,
                    (pt[2] - center[2]) / rz,
                ]));
            }
        }
    }

    point_in_spherical_polygon(&q, &poly)
}

/// Spherical polygon containment test via tangent-plane projection + 2D ray cast.
fn point_in_spherical_polygon(q: &[f64; 3], poly: &[[f64; 3]]) -> bool {
    if poly.len() < 3 {
        return false;
    }

    // Reject if polygon centroid is on the opposite hemisphere from q
    let n = poly.len() as f64;
    let centroid = poly
        .iter()
        .fold([0.0, 0.0, 0.0], |acc, p| vec3::add(acc, *p));
    let centroid = vec3::normalized(vec3::scale(centroid, 1.0 / n));
    if vec3::dot(*q, centroid) < 0.0 {
        return false;
    }

    // Build orthonormal basis for the tangent plane at q
    let u = if q[0].abs() < 0.9 {
        vec3::normalized(vec3::cross(*q, [1.0, 0.0, 0.0]))
    } else {
        vec3::normalized(vec3::cross(*q, [0.0, 1.0, 0.0]))
    };
    let v = vec3::cross(*q, u);

    let poly_2d: Vec<(f64, f64)> = poly
        .iter()
        .map(|p| (vec3::dot(*p, u), vec3::dot(*p, v)))
        .collect();
    let q_2d = (vec3::dot(*q, u), vec3::dot(*q, v));

    ray_cast_2d(q_2d, &poly_2d)
}

/// Test if a point on a torus is within the face boundary.
/// Uses (theta, phi) parametric space with angle unwrapping and 2D ray cast.
fn point_in_face_torus(
    p: &[f64; 3],
    face: &Face,
    shell: &Shell,
    center: &[f64; 3],
    axis: &[f64; 3],
    major_radius: f64,
    _minor_radius: f64,
) -> bool {
    let (u_basis, v_basis) = orthonormal_basis(*axis);

    let torus_params = |pt: &[f64; 3]| -> (f64, f64) {
        let q = vec3::sub(*pt, *center);
        let qx = vec3::dot(q, u_basis);
        let qz = vec3::dot(q, v_basis);
        let theta = qz.atan2(qx);

        let radial_dir = vec3::add(
            vec3::scale(u_basis, theta.cos()),
            vec3::scale(v_basis, theta.sin()),
        );
        let tube_center = vec3::add(*center, vec3::scale(radial_dir, major_radius));
        let to_pt = vec3::sub(*pt, tube_center);
        let pr = vec3::dot(to_pt, radial_dir);
        let pa = vec3::dot(to_pt, *axis);
        let phi = pa.atan2(pr);
        (theta, phi)
    };

    // Build polygon in (theta, phi) space by sampling Arc edges
    let mut raw_poly = Vec::new();
    for edge_ref in &face.loop_edges {
        let edge = &shell.edges[edge_ref.edge_id];
        let vid = if edge_ref.forward {
            edge.v_start
        } else {
            edge.v_end
        };
        raw_poly.push(torus_params(&shell.vertices[vid]));

        if let crate::brep::Curve3D::Arc { .. } = &edge.curve {
            let (t0, t1) = edge.curve.param_range();
            let n_samples = 16;
            for k in 1..n_samples {
                let frac = k as f64 / n_samples as f64;
                let t = if edge_ref.forward {
                    t0 + frac * (t1 - t0)
                } else {
                    t1 - frac * (t1 - t0)
                };
                let pt = edge.curve.evaluate(t);
                raw_poly.push(torus_params(&pt));
            }
        }
    }

    if raw_poly.is_empty() {
        return false;
    }

    // Unwrap angles relative to the first point
    let ref_theta = raw_poly[0].0;
    let ref_phi = raw_poly[0].1;

    let unwrap_angle = |angle: f64, reference: f64| -> f64 {
        let mut d = angle - reference;
        while d > std::f64::consts::PI {
            d -= std::f64::consts::TAU;
        }
        while d < -std::f64::consts::PI {
            d += std::f64::consts::TAU;
        }
        reference + d
    };

    let poly: Vec<(f64, f64)> = raw_poly
        .iter()
        .map(|&(t, p)| (unwrap_angle(t, ref_theta), unwrap_angle(p, ref_phi)))
        .collect();

    let (p_theta, p_phi) = torus_params(p);
    let p_theta = unwrap_angle(p_theta, ref_theta);
    let p_phi = unwrap_angle(p_phi, ref_phi);

    ray_cast_2d((p_theta, p_phi), &poly)
}

/// Test if a point on a NurbsSurface is within the face boundary.
///
/// Uses u/v bounding-box estimation from edge projections, since
/// UV polygon construction fails near axis singularities.
fn point_in_face_nurbs(p: &[f64; 3], face: &Face, shell: &Shell, surface: &NurbsSurface3D) -> bool {
    use crate::boolean3d::intersect3d::project_to_nurbs;

    let (pu, pv) = project_to_nurbs(surface, p);

    // Distance check: points not on the surface are outside
    let on_surf = surface.evaluate(pu, pv);
    if vec3::distance(on_surf, *p) > 1e-3 {
        return false;
    }

    let (surf_u_min, surf_u_max) = surface.u_range();
    let (surf_v_min, surf_v_max) = surface.v_range();

    // Full-revolution NurbsSurface face: check u range only
    if is_full_rotation_nurbs_face(face, shell) {
        let (u_min, u_max) = face_u_range_nurbs(face, shell, surface);
        return pu >= u_min - EPS && pu <= u_max + EPS;
    }

    // Partial revolution / general: project non-singular edge points to get u,v bounds
    let mut u_min = surf_u_max;
    let mut u_max = surf_u_min;
    let mut v_min = surf_v_max;
    let mut v_max = surf_v_min;

    for edge_ref in &face.loop_edges {
        let edge = &shell.edges[edge_ref.edge_id];
        let (t0, t1) = edge.curve.param_range();

        // Project multiple samples from each edge
        let n = 8;
        for k in 0..=n {
            let frac = k as f64 / n as f64;
            let t = t0 + frac * (t1 - t0);
            let pt = edge.curve.evaluate(t);
            let (u, v) = project_to_nurbs(surface, &pt);

            // Exclude axis singularities where all v collapse to a single point
            let on_surf = surface.evaluate(u, v);
            if vec3::distance(on_surf, pt) > 1e-4 {
                continue;
            }

            u_min = u_min.min(u);
            u_max = u_max.max(u);
            v_min = v_min.min(v);
            v_max = v_max.max(v);
        }
    }

    // Rectangular u/v range test
    pu >= u_min - EPS && pu <= u_max + EPS && pv >= v_min - EPS && pv <= v_max + EPS
}

/// Check if a NurbsSurface face is full-revolution (closed Arc or seam-closed edge).
fn is_full_rotation_nurbs_face(face: &Face, shell: &Shell) -> bool {
    for edge_ref in &face.loop_edges {
        let edge = &shell.edges[edge_ref.edge_id];
        match &edge.curve {
            crate::brep::Curve3D::Arc { start, end, .. } => {
                if vec3::distance(*start, *end) < 1e-8 {
                    return true;
                }
            }
            crate::brep::Curve3D::NurbsCurve3D { .. } => {
                let (t0, t1) = edge.curve.param_range();
                let s = edge.curve.evaluate(t0);
                let e = edge.curve.evaluate(t1);
                if vec3::distance(s, e) < 1e-8 {
                    return true;
                }
            }
            _ => {}
        }
    }
    false
}

/// Get u parameter range of a full-revolution NurbsSurface face from its edges.
fn face_u_range_nurbs(face: &Face, shell: &Shell, surface: &NurbsSurface3D) -> (f64, f64) {
    use crate::boolean3d::intersect3d::project_to_nurbs;

    let (surf_u_min, surf_u_max) = surface.u_range();
    let mut u_values = Vec::new();

    for edge_ref in &face.loop_edges {
        let edge = &shell.edges[edge_ref.edge_id];
        let (t0, t1) = edge.curve.param_range();

        // Project midpoint of each edge to get u
        let t_mid = (t0 + t1) * 0.5;
        let pt_mid = edge.curve.evaluate(t_mid);
        let (u, _) = project_to_nurbs(surface, &pt_mid);
        u_values.push(u);
    }

    match u_values.len() {
        0 => (surf_u_min, surf_u_max),
        1 => {
            // Single edge: use full surface range
            let _u = u_values[0];
            (surf_u_min, surf_u_max) // full range
        }
        _ => {
            let u_min = u_values.iter().cloned().fold(f64::INFINITY, f64::min);
            let u_max = u_values.iter().cloned().fold(f64::NEG_INFINITY, f64::max);
            (u_min, u_max)
        }
    }
}

/// Test if a point is within a SurfaceOfSweep face boundary.
///
/// Uses UV bounding-box from edge sample projections. Only accurate for UV-convex faces.
fn point_in_face_sweep(p: &[f64; 3], face: &Face, shell: &Shell) -> bool {
    let surface = &face.surface;
    let (u0, u1, v0, v1) = surface.param_range();

    // Inverse-project test point
    let (pu, pv) = match surface.inverse_project(p) {
        Some(uv) => uv,
        None => return false,
    };

    // Distance check
    let on_surf = surface.evaluate(pu, pv);
    if vec3::distance(on_surf, *p) > 1e-3 {
        return false;
    }

    // Get UV range from edge sample projections
    let mut eu_min = u1;
    let mut eu_max = u0;
    let mut ev_min = v1;
    let mut ev_max = v0;

    for edge_ref in &face.loop_edges {
        let edge = &shell.edges[edge_ref.edge_id];
        let (t0, t1) = edge.curve.param_range();
        let n_samples = 8;
        for k in 0..=n_samples {
            let frac = k as f64 / n_samples as f64;
            let t = t0 + frac * (t1 - t0);
            let pt = edge.curve.evaluate(t);
            let (eu, ev) = match surface.inverse_project(&pt) {
                Some(uv) => uv,
                None => continue,
            };
            let on_surf = surface.evaluate(eu, ev);
            if vec3::distance(on_surf, pt) > 1e-4 {
                continue;
            }
            eu_min = eu_min.min(eu);
            eu_max = eu_max.max(eu);
            ev_min = ev_min.min(ev);
            ev_max = ev_max.max(ev);
        }
    }

    pu >= eu_min - EPS && pu <= eu_max + EPS && pv >= ev_min - EPS && pv <= ev_max + EPS
}

/// 2D ray cast (+x direction)
fn ray_cast_2d(point: (f64, f64), poly: &[(f64, f64)]) -> bool {
    let n = poly.len();
    let mut inside = false;
    let mut j = n - 1;
    for i in 0..n {
        let (yi, yj) = (poly[i].1, poly[j].1);
        if ((yi > point.1) != (yj > point.1))
            && (point.0 < (poly[j].0 - poly[i].0) * (point.1 - yi) / (yj - yi) + poly[i].0)
        {
            inside = !inside;
        }
        j = i;
    }
    inside
}

/// Min/max s parameter across all face boundary vertices.
fn face_s_range(face: &Face, shell: &Shell, origin: &[f64; 3], axis: &[f64; 3]) -> (f64, f64) {
    let mut s_min = f64::INFINITY;
    let mut s_max = f64::NEG_INFINITY;
    for edge_ref in &face.loop_edges {
        let edge = &shell.edges[edge_ref.edge_id];
        for &vid in &[edge.v_start, edge.v_end] {
            let q = vec3::sub(shell.vertices[vid], *origin);
            let s = vec3::dot(q, *axis);
            s_min = s_min.min(s);
            s_max = s_max.max(s);
        }
    }
    (s_min, s_max)
}

/// Check if face is full-revolution (has Arc edge with start == end).
fn is_full_rotation_face(face: &Face, shell: &Shell) -> bool {
    for edge_ref in &face.loop_edges {
        let edge = &shell.edges[edge_ref.edge_id];
        if let crate::brep::Curve3D::Arc { start, end, .. } = &edge.curve {
            if vec3::distance(*start, *end) < 1e-8 {
                return true;
            }
        }
    }
    false
}

/// Compute theta from perpendicular component of q.
fn compute_theta(q: &[f64; 3], axis: &[f64; 3], u: &[f64; 3], v: &[f64; 3]) -> f64 {
    let s = vec3::dot(*q, *axis);
    let q_perp = vec3::sub(*q, vec3::scale(*axis, s));
    let cu = vec3::dot(q_perp, *u);
    let cv = vec3::dot(q_perp, *v);
    cv.atan2(cu)
}

/// 2D ray cast in (theta, s) parametric space.
#[allow(clippy::too_many_arguments)]
fn point_in_face_parametric(
    theta: f64,
    s: f64,
    face: &Face,
    shell: &Shell,
    origin: &[f64; 3],
    axis: &[f64; 3],
    u_basis: &[f64; 3],
    v_basis: &[f64; 3],
) -> bool {
    // Project face loop vertices to (theta, s) space
    let mut poly = Vec::new();
    for edge_ref in &face.loop_edges {
        let edge = &shell.edges[edge_ref.edge_id];
        let vid = if edge_ref.forward {
            edge.v_start
        } else {
            edge.v_end
        };
        let q = vec3::sub(shell.vertices[vid], *origin);
        let sv = vec3::dot(q, *axis);
        let tv = compute_theta(&q, axis, u_basis, v_basis);
        poly.push((tv, sv));
    }

    // Refine polygon by sampling Arc edges at midpoints
    let mut refined_poly = Vec::new();
    for (idx, edge_ref) in face.loop_edges.iter().enumerate() {
        refined_poly.push(poly[idx]);
        let edge = &shell.edges[edge_ref.edge_id];
        if let crate::brep::Curve3D::Arc { .. } = &edge.curve {
            // Interpolate Arc at midpoint samples
            let (t0, t1) = edge.curve.param_range();
            let n_samples = 16;
            for k in 1..n_samples {
                let frac = k as f64 / n_samples as f64;
                let t = if edge_ref.forward {
                    t0 + frac * (t1 - t0)
                } else {
                    t1 - frac * (t1 - t0)
                };
                let pt = edge.curve.evaluate(t);
                let q = vec3::sub(pt, *origin);
                let sv = vec3::dot(q, *axis);
                let tv = compute_theta(&q, axis, u_basis, v_basis);
                refined_poly.push((tv, sv));
            }
        }
    }

    // 2D ray cast (+theta direction)
    let n = refined_poly.len();
    let mut inside = false;
    let mut j = n - 1;
    for i in 0..n {
        let (_, yi) = refined_poly[i];
        let (_, yj) = refined_poly[j];
        if ((yi > s) != (yj > s))
            && (theta
                < (refined_poly[j].0 - refined_poly[i].0) * (s - yi) / (yj - yi)
                    + refined_poly[i].0)
        {
            inside = !inside;
        }
        j = i;
    }
    inside
}

// ────────── Ray-NURBS surface intersection ──────────

/// Ray-NURBS surface intersection via recursive parameter subdivision + Newton.
fn ray_nurbs_intersect(ray_origin: &[f64; 3], surface: &NurbsSurface3D, axis: RayAxis) -> Vec<f64> {
    let mut results = Vec::new();
    let (u0, u1) = surface.u_range();
    let (v0, v1) = surface.v_range();

    ray_nurbs_subdivide(
        ray_origin,
        surface,
        u0,
        u1,
        v0,
        v1,
        0,
        20,
        axis,
        &mut results,
    );

    results.sort_by(|a, b| a.total_cmp(b));
    results.dedup_by(|a, b| (*a - *b).abs() < 1e-8);
    results
}

#[allow(clippy::too_many_arguments)]
fn ray_nurbs_subdivide(
    ray_origin: &[f64; 3],
    surface: &NurbsSurface3D,
    u0: f64,
    u1: f64,
    v0: f64,
    v1: f64,
    depth: usize,
    max_depth: usize,
    axis: RayAxis,
    results: &mut Vec<f64>,
) {
    // 1. Sample parameter domain and compute AABB
    let n_samples = 5;
    let mut bb_min = [f64::INFINITY; 3];
    let mut bb_max = [f64::NEG_INFINITY; 3];
    for i in 0..=n_samples {
        for j in 0..=n_samples {
            let u = u0 + (u1 - u0) * i as f64 / n_samples as f64;
            let v = v0 + (v1 - v0) * j as f64 / n_samples as f64;
            let p = surface.evaluate(u, v);
            bb_min = [
                bb_min[0].min(p[0]),
                bb_min[1].min(p[1]),
                bb_min[2].min(p[2]),
            ];
            bb_max = [
                bb_max[0].max(p[0]),
                bb_max[1].max(p[1]),
                bb_max[2].max(p[2]),
            ];
        }
    }

    // 2. Ray-AABB test
    if !ray_intersects_aabb(ray_origin, &bb_min, &bb_max, axis) {
        return;
    }

    // 3. Small enough -> Newton convergence
    let diag_x = bb_max[0] - bb_min[0];
    let diag_y = bb_max[1] - bb_min[1];
    let diag_z = bb_max[2] - bb_min[2];
    let max_dim = diag_x.max(diag_y).max(diag_z);
    if max_dim < 1e-6 || depth >= max_depth {
        let u_mid = (u0 + u1) * 0.5;
        let v_mid = (v0 + v1) * 0.5;
        if let Some(t) = newton_ray_surface(ray_origin, surface, u_mid, v_mid, axis) {
            if t > 1e-10 {
                results.push(t);
            }
        }
        return;
    }

    // 4. Quadrisect and recurse
    let u_mid = (u0 + u1) * 0.5;
    let v_mid = (v0 + v1) * 0.5;
    ray_nurbs_subdivide(
        ray_origin,
        surface,
        u0,
        u_mid,
        v0,
        v_mid,
        depth + 1,
        max_depth,
        axis,
        results,
    );
    ray_nurbs_subdivide(
        ray_origin,
        surface,
        u_mid,
        u1,
        v0,
        v_mid,
        depth + 1,
        max_depth,
        axis,
        results,
    );
    ray_nurbs_subdivide(
        ray_origin,
        surface,
        u0,
        u_mid,
        v_mid,
        v1,
        depth + 1,
        max_depth,
        axis,
        results,
    );
    ray_nurbs_subdivide(
        ray_origin,
        surface,
        u_mid,
        u1,
        v_mid,
        v1,
        depth + 1,
        max_depth,
        axis,
        results,
    );
}

/// Axis-aligned ray AABB intersection test
fn ray_intersects_aabb(
    ray_origin: &[f64; 3],
    bb_min: &[f64; 3],
    bb_max: &[f64; 3],
    axis: RayAxis,
) -> bool {
    // Perpendicular components must be within AABB range
    if !axis.perp_in_range(ray_origin, bb_min, bb_max) {
        return false;
    }
    // Ray origin component <= bb_max component
    axis.component(ray_origin) <= axis.component(bb_max)
}

/// Newton method for ray-NURBS intersection.
/// Matches perpendicular components, derives t from ray-direction component.
fn newton_ray_surface(
    ray_origin: &[f64; 3],
    surface: &NurbsSurface3D,
    u_init: f64,
    v_init: f64,
    axis: RayAxis,
) -> Option<f64> {
    let mut u = u_init;
    let mut v = v_init;
    let (u_lo, u_hi) = surface.u_range();
    let (v_lo, v_hi) = surface.v_range();

    for _ in 0..20 {
        let p = surface.evaluate(u, v);
        let (p1, p2) = axis.perp_components(&p);
        let (o1, o2) = axis.perp_components(ray_origin);
        let f1 = p1 - o1;
        let f2 = p2 - o2;

        if f1.abs() < 1e-10 && f2.abs() < 1e-10 {
            let t = axis.component(&p) - axis.component(ray_origin);
            return Some(t);
        }

        let du = surface.partial_u(u, v);
        let dv = surface.partial_v(u, v);

        let (du1, du2) = axis.perp_components(&du);
        let (dv1, dv2) = axis.perp_components(&dv);

        let det = du1 * dv2 - du2 * dv1;
        if det.abs() < 1e-15 {
            return None;
        }

        let delta_u = (dv2 * f1 - dv1 * f2) / det;
        let delta_v = (-du2 * f1 + du1 * f2) / det;

        u -= delta_u;
        v -= delta_v;

        u = u.clamp(u_lo, u_hi);
        v = v.clamp(v_lo, v_hi);
    }

    None
}

// ────────── Main entry point ──────────

/// Count ray crossings along a given axis.
fn count_ray_crossings(p: &[f64; 3], shell: &Shell, axis: RayAxis) -> u32 {
    let mut hits = Vec::new();

    for face in &shell.faces {
        match &face.surface {
            Surface::Plane { origin, normal } => {
                let n_comp = axis.component(normal);
                if n_comp.abs() < PARALLEL_TOL {
                    continue;
                }
                let t = vec3::dot(*normal, vec3::sub(*origin, *p)) / n_comp;
                if t <= 0.0 {
                    continue;
                }
                let hit = axis.hit(p, t);
                if point_in_face_polygon(&hit, face, shell) {
                    hits.push(t);
                }
            }
            Surface::Cylinder {
                origin,
                axis: cyl_axis,
                radius,
            } => {
                let cyl_axis_n = vec3::normalized(*cyl_axis);
                let ts = ray_cylinder_intersect(p, origin, &cyl_axis_n, *radius, axis);
                for t in ts {
                    let hit = axis.hit(p, t);
                    if point_in_face_cylinder(&hit, face, shell, origin, &cyl_axis_n, *radius) {
                        hits.push(t);
                    }
                }
            }
            Surface::Cone {
                origin,
                axis: cone_axis,
                half_angle,
            } => {
                let cone_axis_n = vec3::normalized(*cone_axis);
                let ts = ray_cone_intersect(p, origin, &cone_axis_n, *half_angle, axis);
                for t in ts {
                    let hit = axis.hit(p, t);
                    if point_in_face_cone(&hit, face, shell, origin, &cone_axis_n, *half_angle) {
                        hits.push(t);
                    }
                }
            }
            Surface::Sphere { center, radius } => {
                let ts = ray_sphere_intersect(p, center, *radius, axis);
                for t in ts {
                    let hit = axis.hit(p, t);
                    if point_in_face_sphere(&hit, face, shell, center, *radius) {
                        hits.push(t);
                    }
                }
            }
            Surface::Ellipsoid { center, rx, ry, rz } => {
                let ts = ray_ellipsoid_intersect(p, center, *rx, *ry, *rz, axis);
                for t in ts {
                    let hit = axis.hit(p, t);
                    if point_in_face_ellipsoid(&hit, face, shell, center, *rx, *ry, *rz) {
                        hits.push(t);
                    }
                }
            }
            Surface::Torus {
                center,
                axis: torus_axis,
                major_radius,
                minor_radius,
            } => {
                let ts =
                    ray_torus_intersect(p, center, torus_axis, *major_radius, *minor_radius, axis);
                for t in ts {
                    let hit = axis.hit(p, t);
                    if point_in_face_torus(
                        &hit,
                        face,
                        shell,
                        center,
                        torus_axis,
                        *major_radius,
                        *minor_radius,
                    ) {
                        hits.push(t);
                    }
                }
            }
            Surface::SurfaceOfRevolution {
                center,
                axis: rev_axis,
                frame_u,
                frame_v,
                ref profile_control_points,
                ref profile_weights,
                profile_degree,
                n_profile_spans,
                ..
            } => {
                let ts = ray_revolution_intersect(
                    p,
                    center,
                    rev_axis,
                    frame_u,
                    frame_v,
                    profile_control_points,
                    profile_weights,
                    *profile_degree,
                    *n_profile_spans,
                    axis,
                );
                for t in ts {
                    let hit = axis.hit(p, t);
                    if point_in_face_revolution(
                        &hit,
                        face,
                        shell,
                        center,
                        rev_axis,
                        frame_u,
                        frame_v,
                        profile_control_points,
                        profile_weights,
                        *profile_degree,
                        *n_profile_spans,
                    ) {
                        hits.push(t);
                    }
                }
            }
            Surface::SurfaceOfSweep { .. } => {
                let ns = face
                    .surface
                    .to_nurbs_surface()
                    .expect("SurfaceOfSweep -> NurbsSurface conversion failed");
                let ts = ray_nurbs_intersect(p, &ns, axis);
                for &t in &ts {
                    let hit = axis.hit(p, t);
                    if point_in_face_nurbs(&hit, face, shell, &ns) {
                        hits.push(t);
                    }
                }
            }
            Surface::NurbsSurface { data } => {
                let ts = ray_nurbs_intersect(p, data, axis);
                for &t in &ts {
                    let hit = axis.hit(p, t);
                    if point_in_face_nurbs(&hit, face, shell, data) {
                        hits.push(t);
                    }
                }
            }
        }
    }

    if hits.is_empty() {
        return 0;
    }

    hits.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    let mut unique = 1u32;
    let mut last = hits[0];
    for &t in &hits[1..] {
        if (t - last).abs() > 1e-7 {
            unique += 1;
            last = t;
        }
    }

    unique
}

/// Point-in-shell test via ray casting (X->Y->Z fallback).
pub fn point_in_shell(p: &[f64; 3], shell: &Shell) -> bool {
    let (bb_min, bb_max) = shell.bounding_box();
    if p[0] < bb_min[0] - EPS
        || p[0] > bb_max[0] + EPS
        || p[1] < bb_min[1] - EPS
        || p[1] > bb_max[1] + EPS
        || p[2] < bb_min[2] - EPS
        || p[2] > bb_max[2] + EPS
    {
        return false;
    }

    for &axis in &[RayAxis::X, RayAxis::Y, RayAxis::Z] {
        let c = count_ray_crossings(p, shell, axis);
        if c > 0 {
            return c % 2 == 1;
        }
    }
    false
}

/// Test if a 3D point is inside a face polygon.
/// Drops the axis with largest normal component for 2D ray cast.
pub fn point_in_face_polygon(p: &[f64; 3], face: &Face, shell: &Shell) -> bool {
    let normal = match &face.surface {
        Surface::Plane { normal, .. } => *normal,
        Surface::Sphere { center, .. } => {
            // Approximate normal from centroid direction
            let mut centroid = [0.0, 0.0, 0.0];
            let mut count = 0usize;
            for edge_ref in &face.loop_edges {
                let edge = &shell.edges[edge_ref.edge_id];
                let vid = if edge_ref.forward {
                    edge.v_start
                } else {
                    edge.v_end
                };
                let v = shell.vertices[vid];
                centroid = vec3::add(centroid, v);
                count += 1;
            }
            if count == 0 {
                return false;
            }
            let centroid = vec3::scale(centroid, 1.0 / count as f64);
            let n = vec3::sub(centroid, *center);
            if vec3::length(n) < 1e-15 {
                return false;
            }
            vec3::normalized(n)
        }
        Surface::Cylinder {
            origin,
            axis,
            radius,
        } => {
            return point_in_face_cylinder(
                p,
                face,
                shell,
                origin,
                &vec3::normalized(*axis),
                *radius,
            )
        }
        Surface::Cone {
            origin,
            axis,
            half_angle,
        } => {
            return point_in_face_cone(
                p,
                face,
                shell,
                origin,
                &vec3::normalized(*axis),
                *half_angle,
            )
        }
        Surface::Ellipsoid { center, rx, ry, rz } => {
            return point_in_face_ellipsoid(p, face, shell, center, *rx, *ry, *rz)
        }
        Surface::Torus {
            center,
            axis,
            major_radius,
            minor_radius,
        } => {
            return point_in_face_torus(p, face, shell, center, axis, *major_radius, *minor_radius)
        }
        Surface::SurfaceOfRevolution {
            center,
            axis: rev_axis,
            frame_u,
            frame_v,
            ref profile_control_points,
            ref profile_weights,
            profile_degree,
            n_profile_spans,
            ..
        } => {
            return point_in_face_revolution(
                p,
                face,
                shell,
                center,
                rev_axis,
                frame_u,
                frame_v,
                profile_control_points,
                profile_weights,
                *profile_degree,
                *n_profile_spans,
            )
        }
        Surface::SurfaceOfSweep { .. } => {
            return point_in_face_sweep(p, face, shell);
        }
        Surface::NurbsSurface { ref data } => {
            return point_in_face_nurbs(p, face, shell, data);
        }
    };

    // Drop axis with largest normal component for projection
    let abs_n = [normal[0].abs(), normal[1].abs(), normal[2].abs()];
    let drop_axis = if abs_n[0] >= abs_n[1] && abs_n[0] >= abs_n[2] {
        0
    } else if abs_n[1] >= abs_n[2] {
        1
    } else {
        2
    };

    let project = |pt: &[f64; 3]| -> (f64, f64) {
        match drop_axis {
            0 => (pt[1], pt[2]),
            1 => (pt[0], pt[2]),
            _ => (pt[0], pt[1]),
        }
    };

    let (px, py) = project(p);

    // Collect face loop vertices
    let mut poly = Vec::new();
    for edge_ref in &face.loop_edges {
        let edge = &shell.edges[edge_ref.edge_id];
        let v = if edge_ref.forward {
            edge.v_start
        } else {
            edge.v_end
        };
        poly.push(project(&shell.vertices[v]));
    }

    // 2D ray cast (+u direction)
    let n = poly.len();
    let mut inside = false;
    let mut j = n - 1;
    for i in 0..n {
        let (yi, yj) = (poly[i].1, poly[j].1);
        if ((yi > py) != (yj > py))
            && (px < (poly[j].0 - poly[i].0) * (py - yi) / (yj - yi) + poly[i].0)
        {
            inside = !inside;
        }
        j = i;
    }
    inside
}
