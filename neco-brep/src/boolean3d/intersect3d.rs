use crate::bezier_decompose::decompose_to_bezier_patches;
use crate::brep::{Curve3D, Face, Shell, SubFace, Surface};
use crate::vec3::{self, orthonormal_basis};
use neco_nurbs::NurbsSurface3D;

use super::tolerance::GEO_TOL;
use super::BooleanEvent;
const CLIP_TOL: f64 = GEO_TOL;
type PolygonSplit3D = (Vec<[f64; 3]>, Vec<[f64; 3]>);

/// Newton closest-point projection onto a NURBS surface.
///
/// Uses Bezier patch decomposition for initial guess to avoid seam convergence issues.
pub(crate) fn project_to_nurbs(surface: &NurbsSurface3D, p: &[f64; 3]) -> (f64, f64) {
    let patches = decompose_to_bezier_patches(surface);
    if patches.is_empty() {
        // Fallback: global grid search if patch decomposition fails
        return project_to_nurbs_global(surface, p);
    }

    // Compute squared distance from point to each patch AABB
    let mut patch_dists: Vec<(usize, f64)> = patches
        .iter()
        .enumerate()
        .map(|(i, patch)| {
            let (bb_min, bb_max) = patch.aabb();
            let dist_sq = aabb_point_dist_sq(&bb_min, &bb_max, p);
            (i, dist_sq)
        })
        .collect();
    patch_dists.sort_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal));

    // Try top 3 patches (or fewer) for initial guess + Newton
    let n_candidates = patch_dists.len().min(3);
    let mut best_u = 0.0;
    let mut best_v = 0.0;
    let mut best_dist_sq = f64::INFINITY;

    let n_samples = 8;
    for &(patch_idx, _) in &patch_dists[..n_candidates] {
        let patch = &patches[patch_idx];
        let pu_min = patch.u_min;
        let pu_max = patch.u_max;
        let pv_min = patch.v_min;
        let pv_max = patch.v_max;

        // Grid sampling within patch for initial guess
        let mut init_u = (pu_min + pu_max) * 0.5;
        let mut init_v = (pv_min + pv_max) * 0.5;
        let mut init_dist_sq = f64::INFINITY;

        for i in 0..=n_samples {
            let u = pu_min + (pu_max - pu_min) * i as f64 / n_samples as f64;
            for j in 0..=n_samples {
                let v = pv_min + (pv_max - pv_min) * j as f64 / n_samples as f64;
                let q = surface.evaluate(u, v);
                let d = vec3::sub(*p, q);
                let dist_sq = vec3::dot(d, d);
                if dist_sq < init_dist_sq {
                    init_dist_sq = dist_sq;
                    init_u = u;
                    init_v = v;
                }
            }
        }

        // Newton iteration clamped to patch u/v range
        let (u, v, dist_sq) =
            newton_project(surface, p, init_u, init_v, pu_min, pu_max, pv_min, pv_max);
        if dist_sq < best_dist_sq {
            best_dist_sq = dist_sq;
            best_u = u;
            best_v = v;
        }
    }

    (best_u, best_v)
}

/// Squared distance from AABB to point
fn aabb_point_dist_sq(bb_min: &[f64; 3], bb_max: &[f64; 3], p: &[f64; 3]) -> f64 {
    let dx = if p[0] < bb_min[0] {
        bb_min[0] - p[0]
    } else if p[0] > bb_max[0] {
        p[0] - bb_max[0]
    } else {
        0.0
    };
    let dy = if p[1] < bb_min[1] {
        bb_min[1] - p[1]
    } else if p[1] > bb_max[1] {
        p[1] - bb_max[1]
    } else {
        0.0
    };
    let dz = if p[2] < bb_min[2] {
        bb_min[2] - p[2]
    } else if p[2] > bb_max[2] {
        p[2] - bb_max[2]
    } else {
        0.0
    };
    dx * dx + dy * dy + dz * dz
}

/// Gauss-Newton closest-point projection with clampable range.
#[allow(clippy::too_many_arguments)]
fn newton_project(
    surface: &NurbsSurface3D,
    p: &[f64; 3],
    init_u: f64,
    init_v: f64,
    u_min: f64,
    u_max: f64,
    v_min: f64,
    v_max: f64,
) -> (f64, f64, f64) {
    let mut u = init_u;
    let mut v = init_v;

    for _ in 0..20 {
        let s = surface.evaluate(u, v);
        let diff = vec3::sub(s, *p);
        let su = surface.partial_u(u, v);
        let sv = surface.partial_v(u, v);

        let gu = vec3::dot(diff, su);
        let gv = vec3::dot(diff, sv);

        if gu.abs() < 1e-12 && gv.abs() < 1e-12 {
            break;
        }

        let h11 = vec3::dot(su, su);
        let h12 = vec3::dot(su, sv);
        let h22 = vec3::dot(sv, sv);
        let det = h11 * h22 - h12 * h12;
        if det.abs() < 1e-30 {
            break;
        }

        let du = -(h22 * gu - h12 * gv) / det;
        let dv = -(h11 * gv - h12 * gu) / det;

        u = (u + du).clamp(u_min, u_max);
        v = (v + dv).clamp(v_min, v_max);
    }

    let s = surface.evaluate(u, v);
    let diff = vec3::sub(s, *p);
    let dist_sq = vec3::dot(diff, diff);
    (u, v, dist_sq)
}

/// Fallback: global 8x8 grid search + Newton
fn project_to_nurbs_global(surface: &NurbsSurface3D, p: &[f64; 3]) -> (f64, f64) {
    let (u_min, u_max) = surface.u_range();
    let (v_min, v_max) = surface.v_range();

    let n_samples = 8;
    let mut best_u = (u_min + u_max) * 0.5;
    let mut best_v = (v_min + v_max) * 0.5;
    let mut best_dist_sq = f64::INFINITY;

    for i in 0..=n_samples {
        let u = u_min + (u_max - u_min) * i as f64 / n_samples as f64;
        for j in 0..=n_samples {
            let v = v_min + (v_max - v_min) * j as f64 / n_samples as f64;
            let q = surface.evaluate(u, v);
            let d = vec3::sub(*p, q);
            let dist_sq = vec3::dot(d, d);
            if dist_sq < best_dist_sq {
                best_dist_sq = dist_sq;
                best_u = u;
                best_v = v;
            }
        }
    }

    let (u, v, _) = newton_project(surface, p, best_u, best_v, u_min, u_max, v_min, v_max);
    (u, v)
}

// ─── Analytic surface intersection ───

#[derive(Debug, Clone)]
pub enum SurfaceIntersection {
    Line {
        point: [f64; 3],
        direction: [f64; 3],
    },
    Coplanar,
    Circle {
        center: [f64; 3],
        axis: [f64; 3],
        radius: f64,
    },
    Ellipse {
        center: [f64; 3],
        axis_u: [f64; 3],
        axis_v: [f64; 3],
    },
    TwoLines {
        line1_point: [f64; 3],
        line1_dir: [f64; 3],
        line2_point: [f64; 3],
        line2_dir: [f64; 3],
    },
}

/// Plane-plane intersection. Returns None if parallel, Coplanar if coincident, else a line.
pub fn plane_plane_intersect(a: &Surface, b: &Surface) -> Option<SurfaceIntersection> {
    let (oa, na) = match a {
        Surface::Plane { origin, normal } => (origin, normal),
        _ => return None,
    };
    let (ob, nb) = match b {
        Surface::Plane { origin, normal } => (origin, normal),
        _ => return None,
    };

    let dir = vec3::cross(*na, *nb);
    let dir_len = vec3::length(dir);

    if dir_len < CLIP_TOL {
        // Parallel -- check coplanarity: (ob - oa) * na ~ 0
        let d = vec3::dot(vec3::sub(*ob, *oa), *na);
        if d.abs() < CLIP_TOL {
            return Some(SurfaceIntersection::Coplanar);
        }
        return None;
    }

    let direction = vec3::scale(dir, 1.0 / dir_len);

    // Find a point on the intersection line by solving in 2 components orthogonal to dir
    let da = vec3::dot(*na, *oa);
    let db = vec3::dot(*nb, *ob);

    // Drop largest dir component and solve 2x2 system
    let abs_dir = [direction[0].abs(), direction[1].abs(), direction[2].abs()];
    let drop_axis = if abs_dir[0] >= abs_dir[1] && abs_dir[0] >= abs_dir[2] {
        0
    } else if abs_dir[1] >= abs_dir[2] {
        1
    } else {
        2
    };

    // 2x2 system: [na_i na_j; nb_i nb_j] [pi; pj] = [da; db]
    let (na_i, na_j, nb_i, nb_j) = match drop_axis {
        0 => (na[1], na[2], nb[1], nb[2]),
        1 => (na[0], na[2], nb[0], nb[2]),
        _ => (na[0], na[1], nb[0], nb[1]),
    };

    let det = na_i * nb_j - na_j * nb_i;
    let pi = (da * nb_j - db * na_j) / det;
    let pj = (na_i * db - nb_i * da) / det;

    let point = match drop_axis {
        0 => [0.0, pi, pj],
        1 => [pi, 0.0, pj],
        _ => [pi, pj, 0.0],
    };

    Some(SurfaceIntersection::Line { point, direction })
}

/// Plane-cylinder intersection.
/// Perpendicular -> Circle, parallel -> TwoLines, oblique -> Ellipse.
pub fn plane_cylinder_intersect(plane: &Surface, cyl: &Surface) -> Option<SurfaceIntersection> {
    let (origin_p, normal_p) = match plane {
        Surface::Plane { origin, normal } => (origin, normal),
        _ => return None,
    };
    let (origin_c, axis_c_raw, radius) = match cyl {
        Surface::Cylinder {
            origin,
            axis,
            radius,
        } => (origin, axis, *radius),
        _ => return None,
    };
    let axis_n = vec3::normalized(*axis_c_raw);

    let n_dot_a = vec3::dot(*normal_p, axis_n);

    // Case 1: perpendicular (|normal * axis| ~ 1)
    if n_dot_a.abs() > 1.0 - CLIP_TOL {
        let t = vec3::dot(*normal_p, vec3::sub(*origin_p, *origin_c)) / n_dot_a;
        let center = vec3::add(*origin_c, vec3::scale(axis_n, t));
        return Some(SurfaceIntersection::Circle {
            center,
            axis: axis_n,
            radius,
        });
    }

    // Case 2: parallel (|normal * axis| ~ 0)
    if n_dot_a.abs() < CLIP_TOL {
        // Signed distance from axis to plane
        let d = vec3::dot(*normal_p, vec3::sub(*origin_c, *origin_p));
        let abs_d = d.abs();

        if abs_d > radius + CLIP_TOL {
            return None; // No intersection
        }

        // Line direction: axis itself
        let line_dir = axis_n;

        if (abs_d - radius).abs() < CLIP_TOL {
            // Tangent: single line
            let offset = vec3::scale(*normal_p, -d.signum() * radius);
            let pt = vec3::add(*origin_c, offset);
            return Some(SurfaceIntersection::TwoLines {
                line1_point: pt,
                line1_dir: line_dir,
                line2_point: pt,
                line2_dir: line_dir,
            });
        }

        // Two parallel lines on cross-section
        let n_perp = vec3::normalized(*normal_p);
        let lateral = vec3::normalized(vec3::cross(axis_n, n_perp));

        // Cross-section: offset d in normal direction, +/-sqrt(r^2-d^2) laterally
        let h = (radius * radius - d * d).sqrt();
        let base = vec3::add(*origin_c, vec3::scale(n_perp, -d)); // Offset -d in normal direction
        let p1 = vec3::add(base, vec3::scale(lateral, h));
        let p2 = vec3::sub(base, vec3::scale(lateral, h));

        return Some(SurfaceIntersection::TwoLines {
            line1_point: p1,
            line1_dir: line_dir,
            line2_point: p2,
            line2_dir: line_dir,
        });
    }

    // Case 3: oblique -> ellipse
    let u = {
        let candidate = if axis_n[0].abs() < 0.9 {
            [1.0, 0.0, 0.0]
        } else {
            [0.0, 1.0, 0.0]
        };
        let raw = vec3::cross(axis_n, candidate);
        vec3::normalized(raw)
    };
    let v = vec3::normalized(vec3::cross(axis_n, u));

    // P(θ) = origin_c + h*axis + cos(θ)*(r*u + α*axis) + sin(θ)*(r*v + β*axis)
    let h = vec3::dot(*normal_p, vec3::sub(*origin_p, *origin_c)) / n_dot_a;
    let alpha = -radius * vec3::dot(*normal_p, u) / n_dot_a;
    let beta = -radius * vec3::dot(*normal_p, v) / n_dot_a;

    let center = vec3::add(*origin_c, vec3::scale(axis_n, h));
    let ellipse_u = vec3::add(vec3::scale(u, radius), vec3::scale(axis_n, alpha));
    let ellipse_v = vec3::add(vec3::scale(v, radius), vec3::scale(axis_n, beta));

    Some(SurfaceIntersection::Ellipse {
        center,
        axis_u: ellipse_u,
        axis_v: ellipse_v,
    })
}

/// Plane-sphere intersection. Returns Circle or None.
pub fn plane_sphere_intersect(plane: &Surface, sphere: &Surface) -> Option<SurfaceIntersection> {
    let (origin_p, normal) = match plane {
        Surface::Plane { origin, normal } => (origin, normal),
        _ => return None,
    };
    let (center, radius) = match sphere {
        Surface::Sphere { center, radius } => (center, *radius),
        _ => return None,
    };
    // Signed distance from sphere center to plane
    let d = vec3::dot(*normal, vec3::sub(*center, *origin_p));
    if d.abs() > radius + CLIP_TOL {
        return None;
    }
    let circle_radius = (radius * radius - d * d).max(0.0).sqrt();
    let circle_center = vec3::sub(*center, vec3::scale(*normal, d));
    Some(SurfaceIntersection::Circle {
        center: circle_center,
        axis: *normal,
        radius: circle_radius,
    })
}

/// Sphere x Sphere intersection (analytic circle).
pub fn sphere_sphere_intersect(a: &Surface, b: &Surface) -> Option<SurfaceIntersection> {
    let (c1, r1) = match a {
        Surface::Sphere { center, radius } => (center, *radius),
        _ => return None,
    };
    let (c2, r2) = match b {
        Surface::Sphere { center, radius } => (center, *radius),
        _ => return None,
    };
    let diff = vec3::sub(*c2, *c1);
    let d = vec3::length(diff);

    // Same center and radius -> coincident
    if d < CLIP_TOL && (r1 - r2).abs() < CLIP_TOL {
        return None;
    }
    // Too far apart -> no intersection
    if d > r1 + r2 - CLIP_TOL {
        return None;
    }
    // Containment -> no intersection
    if d < (r1 - r2).abs() + CLIP_TOL {
        return None;
    }

    let n = vec3::scale(diff, 1.0 / d);
    let h = (d * d + r1 * r1 - r2 * r2) / (2.0 * d);
    let circle_r_sq = r1 * r1 - h * h;
    if circle_r_sq < 0.0 {
        return None;
    }
    let circle_radius = circle_r_sq.sqrt();
    let circle_center = vec3::add(*c1, vec3::scale(n, h));

    Some(SurfaceIntersection::Circle {
        center: circle_center,
        axis: n,
        radius: circle_radius,
    })
}

/// Plane x Ellipsoid intersection (analytic ellipse).
pub fn plane_ellipsoid_intersect(
    plane: &Surface,
    ellipsoid: &Surface,
) -> Option<SurfaceIntersection> {
    let (origin_p, normal) = match plane {
        Surface::Plane { origin, normal } => (origin, vec3::normalized(*normal)),
        _ => return None,
    };
    let (center, rx, ry, rz) = match ellipsoid {
        Surface::Ellipsoid { center, rx, ry, rz } => (center, *rx, *ry, *rz),
        _ => return None,
    };
    // Scale to unit sphere space
    let sc = [
        (center[0] - origin_p[0]) / rx,
        (center[1] - origin_p[1]) / ry,
        (center[2] - origin_p[2]) / rz,
    ];
    // Covariant normal in scaled space
    let sn = [normal[0] * rx, normal[1] * ry, normal[2] * rz];
    let sn_len = vec3::length(sn);
    if sn_len < 1e-30 {
        return None;
    }
    let sn_hat = vec3::scale(sn, 1.0 / sn_len);

    // Distance from sphere center to plane in scaled space
    let d = vec3::dot(sn_hat, sc);
    if d.abs() > 1.0 + CLIP_TOL {
        return None;
    }

    // Intersection circle in scaled space
    let circle_r = (1.0 - d * d).max(0.0).sqrt();
    if circle_r < 1e-12 {
        return None; // Tangent point only -- degenerate, skip
    }
    let circle_c_s = vec3::sub(sc, vec3::scale(sn_hat, d));

    // Circle basis vectors in scaled space
    let u_s = perpendicular_unit(&sn_hat);
    let v_s = vec3::cross(sn_hat, u_s);

    // Transform back to original space
    let ellipse_center = [
        origin_p[0] + circle_c_s[0] * rx,
        origin_p[1] + circle_c_s[1] * ry,
        origin_p[2] + circle_c_s[2] * rz,
    ];
    let axis_u = vec3::scale([u_s[0] * rx, u_s[1] * ry, u_s[2] * rz], circle_r);
    let axis_v = vec3::scale([v_s[0] * rx, v_s[1] * ry, v_s[2] * rz], circle_r);

    Some(SurfaceIntersection::Ellipse {
        center: ellipse_center,
        axis_u,
        axis_v,
    })
}

/// Plane-Ellipsoid face intersection
fn plane_ellipsoid_face_intersection(
    plane_face: &Face,
    plane_shell: &Shell,
    ellipsoid_face: &Face,
    _ellipsoid_shell: &Shell,
) -> Vec<Curve3D> {
    let si = match plane_ellipsoid_intersect(&plane_face.surface, &ellipsoid_face.surface) {
        Some(si) => si,
        None => return vec![],
    };

    match si {
        SurfaceIntersection::Ellipse {
            center,
            axis_u,
            axis_v,
        } => {
            let ellipse = Curve3D::Ellipse {
                center,
                axis_u,
                axis_v,
                t_start: 0.0,
                t_end: std::f64::consts::TAU,
            };
            clip_closed_curve_or_keep(ellipse, plane_face, plane_shell)
        }
        _ => vec![],
    }
}

/// Plane x Cone analytic intersection.
///
/// Classifies conic section by angle between plane normal and cone axis.
pub fn plane_cone_intersect(plane: &Surface, cone: &Surface) -> Option<Vec<SurfaceIntersection>> {
    let (origin_p, normal_p) = match plane {
        Surface::Plane { origin, normal } => (origin, vec3::normalized(*normal)),
        _ => return None,
    };
    let (origin_c, axis_c, half_angle) = match cone {
        Surface::Cone {
            origin,
            axis,
            half_angle,
        } => (origin, axis, *half_angle),
        _ => return None,
    };

    let axis_len = vec3::length(*axis_c);
    if axis_len < GEO_TOL {
        return None;
    }
    let axis_n = vec3::normalized(*axis_c);
    let sin_a = half_angle.sin();
    let cos_a = half_angle.cos();

    // Angle between plane normal and cone axis
    let n_dot_a = vec3::dot(normal_p, axis_n);
    let beta = n_dot_a.abs().acos(); // β ∈ [0, π/2]

    // Signed distance from cone apex to plane
    let d_apex = vec3::dot(normal_p, vec3::sub(*origin_c, *origin_p));

    // Apex on plane -> two generator lines or empty
    if d_apex.abs() < GEO_TOL {
        // Two generators if beta > alpha, degenerate if beta = alpha
        if beta < half_angle - GEO_TOL {
            return Some(vec![]); // Apex only -> no finite intersection
        }

        // Find cone generator directions on the plane

        let e1 = perpendicular_unit(&axis_n);
        let e2 = vec3::normalized(vec3::cross(axis_n, e1));

        let c_coeff = cos_a * n_dot_a; // cos(α)·(n·a)
        let a_coeff = sin_a * vec3::dot(normal_p, e1); // sin(α)·(n·e1)
        let b_coeff = sin_a * vec3::dot(normal_p, e2); // sin(α)·(n·e2)

        let amplitude = (a_coeff * a_coeff + b_coeff * b_coeff).sqrt();
        if amplitude < GEO_TOL {
            return Some(vec![]); // Degenerate
        }

        let ratio = -c_coeff / amplitude;
        if ratio.abs() > 1.0 + GEO_TOL {
            return Some(vec![]); // No solution
        }
        let ratio_clamped = ratio.clamp(-1.0, 1.0);
        let delta = b_coeff.atan2(a_coeff);
        let phi = ratio_clamped.acos();

        let theta1 = delta + phi;
        let theta2 = delta - phi;
        let dir1 = vec3::add(
            vec3::add(
                vec3::scale(axis_n, cos_a),
                vec3::scale(e1, sin_a * theta1.cos()),
            ),
            vec3::scale(e2, sin_a * theta1.sin()),
        );
        let dir2 = vec3::add(
            vec3::add(
                vec3::scale(axis_n, cos_a),
                vec3::scale(e1, sin_a * theta2.cos()),
            ),
            vec3::scale(e2, sin_a * theta2.sin()),
        );

        return Some(vec![SurfaceIntersection::TwoLines {
            line1_point: *origin_c,
            line1_dir: dir1,
            line2_point: *origin_c,
            line2_dir: dir2,
        }]);
    }

    // Apex not on plane: classify the conic

    // beta ~ 0: plane perpendicular to axis -> circle
    if beta < GEO_TOL {
        if n_dot_a.abs() < GEO_TOL {
            return None;
        }
        let t = -d_apex / n_dot_a;
        if t < -GEO_TOL || t > axis_len + GEO_TOL {
            return None; // Outside cone range
        }
        let r = t * half_angle.tan();
        let center = vec3::add(*origin_c, vec3::scale(axis_n, t));
        return Some(vec![SurfaceIntersection::Circle {
            center,
            axis: normal_p,
            radius: r,
        }]);
    }

    // General case: eigenvalue decomposition of implicit matrix

    // Orthonormal basis on plane
    let (pl_u, pl_w) = orthonormal_basis(normal_p);
    // Origin on plane (closest to cone apex)
    let p0 = vec3::add(*origin_c, vec3::scale(normal_p, -d_apex));

    // Build quadratic form coefficients of cone implicit
    let q0 = vec3::sub(p0, *origin_c); // P0 - apex
    let q0_dot_a = vec3::dot(q0, axis_n);
    let u_dot_a = vec3::dot(pl_u, axis_n);
    let w_dot_a = vec3::dot(pl_w, axis_n);
    let cos2a = cos_a * cos_a;

    let q0_dot_u = vec3::dot(q0, pl_u);
    let q0_dot_w = vec3::dot(q0, pl_w);
    let q0_sq = vec3::dot(q0, q0);

    let m00 = u_dot_a * u_dot_a - cos2a;
    let m11 = w_dot_a * w_dot_a - cos2a;
    let m01 = u_dot_a * w_dot_a;

    let coeff_d = q0_dot_a * u_dot_a - cos2a * q0_dot_u;
    let coeff_e = q0_dot_a * w_dot_a - cos2a * q0_dot_w;

    let coeff_f = q0_dot_a * q0_dot_a - cos2a * q0_sq;

    // 2x2 eigenvalue decomposition
    let trace = m00 + m11;
    let disc_m = (m00 - m11) * (m00 - m11) + 4.0 * m01 * m01;
    let sqrt_disc = disc_m.max(0.0).sqrt();

    let lambda1 = (trace + sqrt_disc) / 2.0;
    let lambda2 = (trace - sqrt_disc) / 2.0;

    // Ellipse: both eigenvalues have same sign and nonzero
    if lambda1.abs() < GEO_TOL || lambda2.abs() < GEO_TOL {
        // Parabola case: sampling fallback
        return None;
    }
    if lambda1 * lambda2 < 0.0 {
        // Hyperbola case: sampling fallback
        return None;
    }

    // Compute eigenvectors
    let (v1x, v1y) = if m01.abs() > GEO_TOL {
        let vx = m01;
        let vy = lambda1 - m00;
        let len = (vx * vx + vy * vy).sqrt();
        (vx / len, vy / len)
    } else if (m00 - lambda1).abs() < (m11 - lambda1).abs() {
        (1.0, 0.0)
    } else {
        (0.0, 1.0)
    };
    let (v2x, v2y) = (-v1y, v1x); // Orthogonal eigenvector

    // Linear terms in eigenvector coordinates
    let d_prime = coeff_d * v1x + coeff_e * v1y;
    let e_prime = coeff_d * v2x + coeff_e * v2y;

    // Complete the square
    let rhs = -(coeff_f - d_prime * d_prime / lambda1 - e_prime * e_prime / lambda2);

    if rhs * lambda1 < -GEO_TOL {
        // No real solution
        return Some(vec![]);
    }
    if rhs.abs() < GEO_TOL {
        // Point (degenerate ellipse)
        return Some(vec![]);
    }

    // Semi-axes
    let a_sq = rhs / lambda1;
    let b_sq = rhs / lambda2;
    if a_sq < -GEO_TOL || b_sq < -GEO_TOL {
        return Some(vec![]);
    }
    let semi_a = a_sq.max(0.0).sqrt();
    let semi_b = b_sq.max(0.0).sqrt();

    // Ellipse center in eigenvector coordinates
    let s0_prime = -d_prime / lambda1;
    let t0_prime = -e_prime / lambda2;

    // Back to original (s,t) coordinates
    let s0 = v1x * s0_prime + v2x * t0_prime;
    let t0 = v1y * s0_prime + v2y * t0_prime;

    // 3D ellipse center
    let center = vec3::add(vec3::add(p0, vec3::scale(pl_u, s0)), vec3::scale(pl_w, t0));

    // 3D ellipse axis directions
    let dir1 = vec3::add(vec3::scale(pl_u, v1x), vec3::scale(pl_w, v1y));
    let dir2 = vec3::add(vec3::scale(pl_u, v2x), vec3::scale(pl_w, v2y));
    let axis_u = vec3::scale(dir1, semi_a);
    let axis_v = vec3::scale(dir2, semi_b);

    // Verify ellipse is within cone height range
    let check_points = [
        center,
        vec3::add(center, axis_u),
        vec3::sub(center, axis_u),
        vec3::add(center, axis_v),
        vec3::sub(center, axis_v),
    ];
    let mut any_in_range = false;
    for pt in &check_points {
        let q = vec3::sub(*pt, *origin_c);
        let t_axis = vec3::dot(q, axis_n);
        if t_axis > -GEO_TOL && t_axis < axis_len + GEO_TOL {
            any_in_range = true;
            break;
        }
    }
    if !any_in_range {
        return Some(vec![]);
    }

    Some(vec![SurfaceIntersection::Ellipse {
        center,
        axis_u,
        axis_v,
    }])
}

/// Polyline sampling fallback for Plane x Cone when analytic solution fails.
fn plane_cone_sample_intersection(
    origin_p: &[f64; 3],
    normal_p: &[f64; 3],
    origin_c: &[f64; 3],
    axis_c: &[f64; 3],
    half_angle: f64,
) -> Vec<Vec<[f64; 3]>> {
    let axis_len = vec3::length(*axis_c);
    if axis_len < GEO_TOL {
        return vec![];
    }
    let axis_n = vec3::normalized(*axis_c);
    let sin_a = half_angle.sin();
    let cos_a = half_angle.cos();
    let n = vec3::normalized(*normal_p);

    let d_apex = vec3::dot(n, vec3::sub(*origin_c, *origin_p));

    let e1 = perpendicular_unit(&axis_n);
    let e2 = vec3::normalized(vec3::cross(axis_n, e1));

    let n_samples = 720;
    let mut points = Vec::new();

    for i in 0..=n_samples {
        let theta = std::f64::consts::TAU * i as f64 / n_samples as f64;
        let cos_t = theta.cos();
        let sin_t = theta.sin();

        let gen_dir = vec3::add(
            vec3::add(vec3::scale(axis_n, cos_a), vec3::scale(e1, sin_a * cos_t)),
            vec3::scale(e2, sin_a * sin_t),
        );

        let n_dot_gen = vec3::dot(n, gen_dir);
        if n_dot_gen.abs() < GEO_TOL {
            continue;
        }

        let h = -d_apex / n_dot_gen;
        let t_axis = h * cos_a;
        if t_axis < -GEO_TOL || t_axis > axis_len + GEO_TOL {
            continue;
        }
        if h < -GEO_TOL {
            continue;
        }

        let point = vec3::add(*origin_c, vec3::scale(gen_dir, h));
        points.push(point);
    }

    if points.len() < 2 {
        return vec![];
    }

    // Split point cloud into curves by angular gap
    let mut curves: Vec<Vec<[f64; 3]>> = Vec::new();
    let mut current = vec![points[0]];
    let gap_threshold = axis_len * 0.5; // Split at large gaps

    for i in 1..points.len() {
        let dist = vec3::length(vec3::sub(points[i], points[i - 1]));
        if dist > gap_threshold {
            if current.len() >= 2 {
                curves.push(current);
            }
            current = vec![points[i]];
        } else {
            current.push(points[i]);
        }
    }
    if current.len() >= 2 {
        curves.push(current);
    }

    curves
}

/// Plane x Cone face intersection
fn plane_cone_face_intersection(
    plane_face: &Face,
    plane_shell: &Shell,
    cone_face: &Face,
    _cone_shell: &Shell,
    events: &mut Vec<BooleanEvent>,
) -> Vec<Curve3D> {
    let (p_origin, p_normal) = match &plane_face.surface {
        Surface::Plane { origin, normal } => (*origin, vec3::normalized(*normal)),
        _ => return vec![],
    };
    let (c_origin, c_axis, c_half_angle) = match &cone_face.surface {
        Surface::Cone {
            origin,
            axis,
            half_angle,
        } => (origin, axis, *half_angle),
        _ => return vec![],
    };

    // Try analytic intersection
    let has_boundary = !plane_face.loop_edges.is_empty();
    if let Some(intersections) = plane_cone_intersect(&plane_face.surface, &cone_face.surface) {
        let mut result = Vec::new();
        for si in intersections {
            match si {
                SurfaceIntersection::Circle {
                    center,
                    axis,
                    radius,
                } => {
                    let u = perpendicular_unit(&axis);
                    let start = vec3::add(center, vec3::scale(u, radius));
                    let arc = Curve3D::Arc {
                        center,
                        axis,
                        start,
                        end: start,
                        radius,
                    };
                    if has_boundary {
                        result.extend(clip_closed_curve_or_keep(arc, plane_face, plane_shell));
                    } else {
                        result.push(arc);
                    }
                }
                SurfaceIntersection::Ellipse {
                    center,
                    axis_u,
                    axis_v,
                } => {
                    let ellipse = Curve3D::Ellipse {
                        center,
                        axis_u,
                        axis_v,
                        t_start: 0.0,
                        t_end: std::f64::consts::TAU,
                    };
                    if has_boundary {
                        result.extend(clip_closed_curve_or_keep(ellipse, plane_face, plane_shell));
                    } else {
                        result.push(ellipse);
                    }
                }
                SurfaceIntersection::TwoLines {
                    line1_point,
                    line1_dir,
                    line2_point,
                    line2_dir,
                } => {
                    if has_boundary {
                        if let Some((t0, t1)) =
                            clip_line_to_face(&line1_point, &line1_dir, plane_face, plane_shell)
                        {
                            let start = vec3::add(line1_point, vec3::scale(line1_dir, t0));
                            let end = vec3::add(line1_point, vec3::scale(line1_dir, t1));
                            if vec3::length(vec3::sub(start, end)) > CLIP_TOL {
                                result.push(Curve3D::Line { start, end });
                            }
                        }
                        if let Some((t0, t1)) =
                            clip_line_to_face(&line2_point, &line2_dir, plane_face, plane_shell)
                        {
                            let start = vec3::add(line2_point, vec3::scale(line2_dir, t0));
                            let end = vec3::add(line2_point, vec3::scale(line2_dir, t1));
                            if vec3::length(vec3::sub(start, end)) > CLIP_TOL {
                                result.push(Curve3D::Line { start, end });
                            }
                        }
                    } else {
                        // No boundary: clip to cone range
                        let len = vec3::length(*c_axis);
                        let l1 = vec3::add(line1_point, vec3::scale(line1_dir, len));
                        result.push(Curve3D::Line {
                            start: line1_point,
                            end: l1,
                        });
                        let l2 = vec3::add(line2_point, vec3::scale(line2_dir, len));
                        result.push(Curve3D::Line {
                            start: line2_point,
                            end: l2,
                        });
                    }
                }
                _ => {}
            }
        }
        if !result.is_empty() {
            events.push(BooleanEvent::Info(format!(
                "Plane-Cone analytic intersection: {} curves detected",
                result.len()
            )));
            return result;
        }
    }

    // Fallback: polyline sampling for parabola/hyperbola
    let polylines =
        plane_cone_sample_intersection(&p_origin, &p_normal, c_origin, c_axis, c_half_angle);

    if polylines.is_empty() {
        return vec![];
    }

    events.push(BooleanEvent::Info(format!(
        "Plane-Cone sampling intersection: {} curves detected",
        polylines.len()
    )));

    if !plane_face.loop_edges.is_empty() {
        let mut result = Vec::new();
        for polyline in &polylines {
            let clipped = clip_polyline_to_face(polyline, plane_face, plane_shell);
            result.extend(clipped);
        }
        if !result.is_empty() {
            return result;
        }
    }

    polylines_to_curves(polylines, events)
}

/// Clip an infinite line to a convex face boundary, returning parameter interval (t_min, t_max).
/// line(t) = line_point + t * line_dir
pub fn clip_line_to_face(
    line_point: &[f64; 3],
    line_dir: &[f64; 3],
    face: &Face,
    shell: &Shell,
) -> Option<(f64, f64)> {
    let face_normal = match &face.surface {
        Surface::Plane { normal, .. } => normal,
        _ => return None,
    };

    let mut t_min = f64::NEG_INFINITY;
    let mut t_max = f64::INFINITY;

    let loop_edges = &face.loop_edges;
    for edge_ref in loop_edges {
        let edge = &shell.edges[edge_ref.edge_id];
        let (va, vb) = if edge_ref.forward {
            (edge.v_start, edge.v_end)
        } else {
            (edge.v_end, edge.v_start)
        };
        let pa = &shell.vertices[va];
        let pb = &shell.vertices[vb];

        let edge_dir = vec3::sub(*pb, *pa);
        // Inward normal = edge_dir x face_normal (CCW loop)
        let inward = vec3::cross(edge_dir, *face_normal);

        let denom = vec3::dot(inward, *line_dir);
        let numer = vec3::dot(inward, vec3::sub(*line_point, *pa));

        if denom.abs() < CLIP_TOL {
            // Line parallel to half-plane boundary
            if numer > CLIP_TOL {
                return None; // Outside
            }
            continue;
        }

        let t = -numer / denom;
        if denom < 0.0 {
            // Entering
            if t > t_min {
                t_min = t;
            }
        } else {
            // Exiting
            if t < t_max {
                t_max = t;
            }
        }
    }

    if t_min > t_max + CLIP_TOL {
        return None;
    }

    // Filter empty interval
    if t_min > t_max {
        return None;
    }

    Some((t_min, t_max))
}

/// Fit SSI polylines to NurbsCurve3D; fall back to Line segments on failure.
const NURBS_FIT_TOL: f64 = 1e-4;

/// Convert fit_nurbs_curve result to Curve3D.
fn fit_nurbs_curve_to_curve3d(
    points: &[[f64; 3]],
    tolerance: f64,
) -> Result<Curve3D, neco_nurbs::NurbsFitError> {
    let result = neco_nurbs::fit_nurbs_curve(points, tolerance)?;
    Ok(Curve3D::NurbsCurve3D {
        degree: result.degree,
        control_points: result.control_points,
        weights: result.weights,
        knots: result.knots,
    })
}

fn arc_from_three_points(points: &[[f64; 3]]) -> Option<Curve3D> {
    if points.len() != 3 {
        return None;
    }
    let p0 = points[0];
    let p1 = points[1];
    let p2 = points[2];
    let a = vec3::sub(p1, p0);
    let b = vec3::sub(p2, p0);
    let normal = vec3::cross(a, b);
    let normal_len_sq = vec3::dot(normal, normal);
    if normal_len_sq < 1e-12 {
        return None;
    }

    let a_len_sq = vec3::dot(a, a);
    let b_len_sq = vec3::dot(b, b);
    let term_a = vec3::scale(vec3::cross(normal, b), a_len_sq);
    let term_b = vec3::scale(vec3::cross(a, normal), b_len_sq);
    let center = vec3::add(
        p0,
        vec3::scale(vec3::add(term_a, term_b), 0.5 / normal_len_sq),
    );
    let radius = vec3::distance(center, p0);
    if radius < 1e-8 {
        return None;
    }

    let mut best = None;
    for axis in [
        vec3::normalized(normal),
        vec3::scale(vec3::normalized(normal), -1.0),
    ] {
        let curve = Curve3D::Arc {
            center,
            axis,
            start: p0,
            end: p2,
            radius,
        };
        let (_, t_end) = curve.param_range();
        if t_end <= 1e-8 || t_end >= std::f64::consts::TAU - 1e-8 {
            continue;
        }
        let midpoint = curve.evaluate(t_end * 0.5);
        let error = vec3::distance(midpoint, p1);
        if best
            .as_ref()
            .is_none_or(|(_, best_error)| error < *best_error)
        {
            best = Some((curve, error));
        }
    }

    let (curve, error) = best?;
    (error < radius * 1e-2 + 1e-4).then_some(curve)
}

pub(crate) fn polylines_to_curves(
    polylines: Vec<Vec<[f64; 3]>>,
    events: &mut Vec<BooleanEvent>,
) -> Vec<Curve3D> {
    let mut result = Vec::new();
    for (idx, polyline) in polylines.into_iter().enumerate() {
        if polyline.len() == 3 {
            if let Some(curve) = arc_from_three_points(&polyline) {
                events.push(BooleanEvent::Info(format!(
                    "SSI polyline[{idx}]: 3 points -> Arc reconstruction success"
                )));
                result.push(curve);
                continue;
            }
        }
        if polyline.len() >= 4 {
            match fit_nurbs_curve_to_curve3d(&polyline, NURBS_FIT_TOL) {
                Ok(curve) => {
                    events.push(BooleanEvent::Info(format!(
                        "SSI polyline[{idx}]: {} points -> NurbsCurve3D fit success",
                        polyline.len()
                    )));
                    result.push(curve);
                    continue;
                }
                Err(e) => {
                    events.push(BooleanEvent::Warning(format!(
                        "SSI polyline[{idx}]: NurbsCurve3D fit failed ({e}), falling back to Line segments"
                    )));
                }
            }
        }
        // Fallback: Line segments
        for w in polyline.windows(2) {
            result.push(Curve3D::Line {
                start: w[0],
                end: w[1],
            });
        }
    }
    result
}

/// Compute intersection curves between two faces.
///
/// SSI polylines are fit to NurbsCurve3D when possible, with Line segment fallback.
pub fn face_face_intersection(
    face_a: &Face,
    shell_a: &Shell,
    face_b: &Face,
    shell_b: &Shell,
    events: &mut Vec<BooleanEvent>,
) -> Vec<Curve3D> {
    match (&face_a.surface, &face_b.surface) {
        (Surface::Plane { .. }, Surface::Plane { .. }) => {
            plane_plane_face_intersection(face_a, shell_a, face_b, shell_b)
        }
        (Surface::Plane { .. }, Surface::Cylinder { .. }) => {
            plane_cylinder_face_intersection(face_a, shell_a, face_b, shell_b)
        }
        (Surface::Cylinder { .. }, Surface::Plane { .. }) => {
            plane_cylinder_face_intersection(face_b, shell_b, face_a, shell_a)
        }
        (Surface::Cylinder { .. }, Surface::Cylinder { .. }) => {
            super::sweep_intersect::cylinder_cylinder_face_intersection(
                face_a, shell_a, face_b, shell_b, events,
            )
        }
        (Surface::Sphere { .. }, Surface::Cylinder { .. }) => {
            super::sweep_intersect::sphere_cylinder_face_intersection(
                face_a, shell_a, face_b, shell_b, events,
            )
        }
        (Surface::Cylinder { .. }, Surface::Sphere { .. }) => {
            super::sweep_intersect::sphere_cylinder_face_intersection(
                face_b, shell_b, face_a, shell_a, events,
            )
        }
        (Surface::Ellipsoid { .. }, Surface::Cylinder { .. }) => {
            super::sweep_intersect::ellipsoid_cylinder_face_intersection(
                face_a, shell_a, face_b, shell_b, events,
            )
        }
        (Surface::Cylinder { .. }, Surface::Ellipsoid { .. }) => {
            super::sweep_intersect::ellipsoid_cylinder_face_intersection(
                face_b, shell_b, face_a, shell_a, events,
            )
        }
        (Surface::Plane { .. }, Surface::Sphere { .. }) => {
            plane_sphere_face_intersection(face_a, shell_a, face_b, shell_b)
        }
        (Surface::Sphere { .. }, Surface::Plane { .. }) => {
            sphere_plane_face_intersection(face_a, shell_a, face_b, shell_b)
        }
        (Surface::Sphere { .. }, Surface::Sphere { .. }) => {
            sphere_sphere_face_intersection(face_a, shell_a, face_b, shell_b)
        }
        (Surface::Plane { .. }, Surface::Ellipsoid { .. }) => {
            plane_ellipsoid_face_intersection(face_a, shell_a, face_b, shell_b)
        }
        (Surface::Ellipsoid { .. }, Surface::Plane { .. }) => {
            plane_ellipsoid_face_intersection(face_b, shell_b, face_a, shell_a)
        }
        (Surface::Plane { .. }, Surface::Torus { .. }) => {
            plane_torus_face_intersection(face_a, shell_a, face_b, shell_b, events)
        }
        (Surface::Torus { .. }, Surface::Plane { .. }) => {
            plane_torus_face_intersection(face_b, shell_b, face_a, shell_a, events)
        }
        (Surface::Plane { .. }, Surface::Cone { .. }) => {
            plane_cone_face_intersection(face_a, shell_a, face_b, shell_b, events)
        }
        (Surface::Cone { .. }, Surface::Plane { .. }) => {
            plane_cone_face_intersection(face_b, shell_b, face_a, shell_a, events)
        }
        // B2: Sphere × Cone
        (Surface::Sphere { .. }, Surface::Cone { .. }) => {
            super::sweep_intersect::sphere_cone_face_intersection(
                face_a, shell_a, face_b, shell_b, events,
            )
        }
        (Surface::Cone { .. }, Surface::Sphere { .. }) => {
            super::sweep_intersect::sphere_cone_face_intersection(
                face_b, shell_b, face_a, shell_a, events,
            )
        }
        // B3: Sphere × Ellipsoid
        (Surface::Sphere { .. }, Surface::Ellipsoid { .. }) => {
            super::sweep_intersect::sphere_ellipsoid_face_intersection(
                face_a, shell_a, face_b, shell_b, events,
            )
        }
        (Surface::Ellipsoid { .. }, Surface::Sphere { .. }) => {
            super::sweep_intersect::sphere_ellipsoid_face_intersection(
                face_b, shell_b, face_a, shell_a, events,
            )
        }
        // B5: Ellipsoid × Cone
        (Surface::Ellipsoid { .. }, Surface::Cone { .. }) => {
            super::sweep_intersect::ellipsoid_cone_face_intersection(
                face_a, shell_a, face_b, shell_b, events,
            )
        }
        (Surface::Cone { .. }, Surface::Ellipsoid { .. }) => {
            super::sweep_intersect::ellipsoid_cone_face_intersection(
                face_b, shell_b, face_a, shell_a, events,
            )
        }
        // B6: Ellipsoid × Ellipsoid
        (Surface::Ellipsoid { .. }, Surface::Ellipsoid { .. }) => {
            super::sweep_intersect::ellipsoid_ellipsoid_face_intersection(
                face_a, shell_a, face_b, shell_b, events,
            )
        }
        // B8: Cylinder × Cone
        (Surface::Cylinder { .. }, Surface::Cone { .. }) => {
            super::sweep_intersect::cylinder_cone_face_intersection(
                face_a, shell_a, face_b, shell_b, events,
            )
        }
        (Surface::Cone { .. }, Surface::Cylinder { .. }) => {
            super::sweep_intersect::cylinder_cone_face_intersection(
                face_b, shell_b, face_a, shell_a, events,
            )
        }
        // B9: Cone × Cone
        (Surface::Cone { .. }, Surface::Cone { .. }) => {
            super::sweep_intersect::cone_cone_face_intersection(
                face_a, shell_a, face_b, shell_b, events,
            )
        }
        // Sphere × Torus
        (Surface::Sphere { .. }, Surface::Torus { .. }) => {
            super::sweep_intersect::sphere_torus_face_intersection(
                face_a, shell_a, face_b, shell_b, events,
            )
        }
        (Surface::Torus { .. }, Surface::Sphere { .. }) => {
            super::sweep_intersect::sphere_torus_face_intersection(
                face_b, shell_b, face_a, shell_a, events,
            )
        }
        // Cylinder × Torus
        (Surface::Cylinder { .. }, Surface::Torus { .. }) => {
            super::sweep_intersect::cylinder_torus_face_intersection(
                face_a, shell_a, face_b, shell_b, events,
            )
        }
        (Surface::Torus { .. }, Surface::Cylinder { .. }) => {
            super::sweep_intersect::cylinder_torus_face_intersection(
                face_b, shell_b, face_a, shell_a, events,
            )
        }
        // Cone × Torus
        (Surface::Cone { .. }, Surface::Torus { .. }) => {
            super::sweep_intersect::cone_torus_face_intersection(
                face_a, shell_a, face_b, shell_b, events,
            )
        }
        (Surface::Torus { .. }, Surface::Cone { .. }) => {
            super::sweep_intersect::cone_torus_face_intersection(
                face_b, shell_b, face_a, shell_a, events,
            )
        }
        // Ellipsoid × Torus
        (Surface::Ellipsoid { .. }, Surface::Torus { .. }) => {
            super::sweep_intersect::ellipsoid_torus_face_intersection(
                face_a, shell_a, face_b, shell_b, events,
            )
        }
        (Surface::Torus { .. }, Surface::Ellipsoid { .. }) => {
            super::sweep_intersect::ellipsoid_torus_face_intersection(
                face_b, shell_b, face_a, shell_a, events,
            )
        }
        // Torus × Torus
        (Surface::Torus { .. }, Surface::Torus { .. }) => {
            super::sweep_intersect::torus_torus_face_intersection(
                face_a, shell_a, face_b, shell_b, events,
            )
        }
        // NurbsSurface × Plane
        (Surface::NurbsSurface { data }, Surface::Plane { origin, normal })
        | (Surface::Plane { origin, normal }, Surface::NurbsSurface { data }) => {
            let polylines =
                super::nurbs_intersect::nurbs_plane_intersection(data.as_ref(), origin, normal);
            polylines_to_curves(polylines, events)
        }
        // NurbsSurface × Quadric (Sphere/Ellipsoid/Cylinder/Cone)
        (
            Surface::NurbsSurface { data },
            quadric @ (Surface::Sphere { .. }
            | Surface::Ellipsoid { .. }
            | Surface::Cylinder { .. }
            | Surface::Cone { .. }),
        )
        | (
            quadric @ (Surface::Sphere { .. }
            | Surface::Ellipsoid { .. }
            | Surface::Cylinder { .. }
            | Surface::Cone { .. }),
            Surface::NurbsSurface { data },
        ) => {
            let polylines =
                super::nurbs_intersect::nurbs_quadric_intersection(data.as_ref(), quadric);
            polylines_to_curves(polylines, events)
        }
        // NurbsSurface × Torus
        (
            Surface::NurbsSurface { data },
            Surface::Torus {
                center,
                axis,
                major_radius,
                minor_radius,
            },
        )
        | (
            Surface::Torus {
                center,
                axis,
                major_radius,
                minor_radius,
            },
            Surface::NurbsSurface { data },
        ) => {
            let polylines = super::nurbs_intersect::nurbs_torus_intersection(
                data.as_ref(),
                center,
                axis,
                *major_radius,
                *minor_radius,
            );
            polylines_to_curves(polylines, events)
        }
        // NurbsSurface × NurbsSurface
        (Surface::NurbsSurface { data: data_a }, Surface::NurbsSurface { data: data_b }) => {
            let polylines =
                super::nurbs_intersect::nurbs_nurbs_intersection(data_a.as_ref(), data_b.as_ref());
            polylines_to_curves(polylines, events)
        }
        // SurfaceOfRevolution × Plane
        (Surface::SurfaceOfRevolution { .. }, Surface::Plane { origin, normal })
        | (Surface::Plane { origin, normal }, Surface::SurfaceOfRevolution { .. }) => {
            let rev_surf = if matches!(face_a.surface, Surface::SurfaceOfRevolution { .. }) {
                &face_a.surface
            } else {
                &face_b.surface
            };
            match rev_surf.to_nurbs_surface() {
                Some(ns) => {
                    let polylines =
                        super::nurbs_intersect::nurbs_plane_intersection(&ns, origin, normal);
                    polylines_to_curves(polylines, events)
                }
                None => {
                    events.push(BooleanEvent::Warning(
                        "SurfaceOfRevolution to NurbsSurface conversion failed".to_string(),
                    ));
                    vec![]
                }
            }
        }
        // SurfaceOfRevolution × Quadric (Sphere/Ellipsoid/Cylinder/Cone)
        (
            Surface::SurfaceOfRevolution { .. },
            quadric @ (Surface::Sphere { .. }
            | Surface::Ellipsoid { .. }
            | Surface::Cylinder { .. }
            | Surface::Cone { .. }),
        )
        | (
            quadric @ (Surface::Sphere { .. }
            | Surface::Ellipsoid { .. }
            | Surface::Cylinder { .. }
            | Surface::Cone { .. }),
            Surface::SurfaceOfRevolution { .. },
        ) => {
            let rev_surf = if matches!(face_a.surface, Surface::SurfaceOfRevolution { .. }) {
                &face_a.surface
            } else {
                &face_b.surface
            };
            match rev_surf.to_nurbs_surface() {
                Some(ns) => {
                    let polylines =
                        super::nurbs_intersect::nurbs_quadric_intersection(&ns, quadric);
                    polylines_to_curves(polylines, events)
                }
                None => {
                    events.push(BooleanEvent::Warning(
                        "SurfaceOfRevolution to NurbsSurface conversion failed".to_string(),
                    ));
                    vec![]
                }
            }
        }
        // SurfaceOfRevolution × Torus
        (
            Surface::SurfaceOfRevolution { .. },
            Surface::Torus {
                center,
                axis,
                major_radius,
                minor_radius,
            },
        )
        | (
            Surface::Torus {
                center,
                axis,
                major_radius,
                minor_radius,
            },
            Surface::SurfaceOfRevolution { .. },
        ) => {
            let rev_surf = if matches!(face_a.surface, Surface::SurfaceOfRevolution { .. }) {
                &face_a.surface
            } else {
                &face_b.surface
            };
            match rev_surf.to_nurbs_surface() {
                Some(ns) => {
                    let polylines = super::nurbs_intersect::nurbs_torus_intersection(
                        &ns,
                        center,
                        axis,
                        *major_radius,
                        *minor_radius,
                    );
                    polylines_to_curves(polylines, events)
                }
                None => {
                    events.push(BooleanEvent::Warning(
                        "SurfaceOfRevolution to NurbsSurface conversion failed".to_string(),
                    ));
                    vec![]
                }
            }
        }
        // SurfaceOfRevolution × NurbsSurface
        (Surface::SurfaceOfRevolution { .. }, Surface::NurbsSurface { data })
        | (Surface::NurbsSurface { data }, Surface::SurfaceOfRevolution { .. }) => {
            let rev_surf = if matches!(face_a.surface, Surface::SurfaceOfRevolution { .. }) {
                &face_a.surface
            } else {
                &face_b.surface
            };
            match rev_surf.to_nurbs_surface() {
                Some(ns) => {
                    let polylines =
                        super::nurbs_intersect::nurbs_nurbs_intersection(&ns, data.as_ref());
                    polylines_to_curves(polylines, events)
                }
                None => {
                    events.push(BooleanEvent::Warning(
                        "SurfaceOfRevolution to NurbsSurface conversion failed".to_string(),
                    ));
                    vec![]
                }
            }
        }
        // SurfaceOfRevolution × SurfaceOfSweep
        (Surface::SurfaceOfRevolution { .. }, Surface::SurfaceOfSweep { .. })
        | (Surface::SurfaceOfSweep { .. }, Surface::SurfaceOfRevolution { .. }) => {
            match (
                face_a.surface.to_nurbs_surface(),
                face_b.surface.to_nurbs_surface(),
            ) {
                (Some(ns_a), Some(ns_b)) => {
                    let polylines = super::nurbs_intersect::nurbs_nurbs_intersection(&ns_a, &ns_b);
                    polylines_to_curves(polylines, events)
                }
                _ => {
                    events.push(BooleanEvent::Warning(
                        "SurfaceOfRevolution/SurfaceOfSweep to NurbsSurface conversion failed"
                            .to_string(),
                    ));
                    vec![]
                }
            }
        }
        // SurfaceOfRevolution × SurfaceOfRevolution
        (Surface::SurfaceOfRevolution { .. }, Surface::SurfaceOfRevolution { .. }) => {
            match (
                face_a.surface.to_nurbs_surface(),
                face_b.surface.to_nurbs_surface(),
            ) {
                (Some(ns_a), Some(ns_b)) => {
                    let polylines = super::nurbs_intersect::nurbs_nurbs_intersection(&ns_a, &ns_b);
                    polylines_to_curves(polylines, events)
                }
                _ => {
                    events.push(BooleanEvent::Warning(
                        "SurfaceOfRevolution to NurbsSurface conversion failed".to_string(),
                    ));
                    vec![]
                }
            }
        }
        // SurfaceOfSweep × Plane
        (Surface::SurfaceOfSweep { .. }, Surface::Plane { origin, normal })
        | (Surface::Plane { origin, normal }, Surface::SurfaceOfSweep { .. }) => {
            let sweep_surf = if matches!(face_a.surface, Surface::SurfaceOfSweep { .. }) {
                &face_a.surface
            } else {
                &face_b.surface
            };
            match sweep_surf.to_nurbs_surface() {
                Some(ns) => {
                    let polylines =
                        super::nurbs_intersect::nurbs_plane_intersection(&ns, origin, normal);
                    polylines_to_curves(polylines, events)
                }
                None => {
                    events.push(BooleanEvent::Warning(
                        "SurfaceOfSweep to NurbsSurface conversion failed".to_string(),
                    ));
                    vec![]
                }
            }
        }
        // SurfaceOfSweep × Quadric (Sphere/Ellipsoid/Cylinder/Cone)
        (
            Surface::SurfaceOfSweep { .. },
            quadric @ (Surface::Sphere { .. }
            | Surface::Ellipsoid { .. }
            | Surface::Cylinder { .. }
            | Surface::Cone { .. }),
        )
        | (
            quadric @ (Surface::Sphere { .. }
            | Surface::Ellipsoid { .. }
            | Surface::Cylinder { .. }
            | Surface::Cone { .. }),
            Surface::SurfaceOfSweep { .. },
        ) => {
            let sweep_surf = if matches!(face_a.surface, Surface::SurfaceOfSweep { .. }) {
                &face_a.surface
            } else {
                &face_b.surface
            };
            match sweep_surf.to_nurbs_surface() {
                Some(ns) => {
                    let polylines =
                        super::nurbs_intersect::nurbs_quadric_intersection(&ns, quadric);
                    polylines_to_curves(polylines, events)
                }
                None => {
                    events.push(BooleanEvent::Warning(
                        "SurfaceOfSweep to NurbsSurface conversion failed".to_string(),
                    ));
                    vec![]
                }
            }
        }
        // SurfaceOfSweep × Torus
        (
            Surface::SurfaceOfSweep { .. },
            Surface::Torus {
                center,
                axis,
                major_radius,
                minor_radius,
            },
        )
        | (
            Surface::Torus {
                center,
                axis,
                major_radius,
                minor_radius,
            },
            Surface::SurfaceOfSweep { .. },
        ) => {
            let sweep_surf = if matches!(face_a.surface, Surface::SurfaceOfSweep { .. }) {
                &face_a.surface
            } else {
                &face_b.surface
            };
            match sweep_surf.to_nurbs_surface() {
                Some(ns) => {
                    let polylines = super::nurbs_intersect::nurbs_torus_intersection(
                        &ns,
                        center,
                        axis,
                        *major_radius,
                        *minor_radius,
                    );
                    polylines_to_curves(polylines, events)
                }
                None => {
                    events.push(BooleanEvent::Warning(
                        "SurfaceOfSweep to NurbsSurface conversion failed".to_string(),
                    ));
                    vec![]
                }
            }
        }
        // SurfaceOfSweep × NurbsSurface
        (Surface::SurfaceOfSweep { .. }, Surface::NurbsSurface { data })
        | (Surface::NurbsSurface { data }, Surface::SurfaceOfSweep { .. }) => {
            let sweep_surf = if matches!(face_a.surface, Surface::SurfaceOfSweep { .. }) {
                &face_a.surface
            } else {
                &face_b.surface
            };
            match sweep_surf.to_nurbs_surface() {
                Some(ns) => {
                    let polylines =
                        super::nurbs_intersect::nurbs_nurbs_intersection(&ns, data.as_ref());
                    polylines_to_curves(polylines, events)
                }
                None => {
                    events.push(BooleanEvent::Warning(
                        "SurfaceOfSweep to NurbsSurface conversion failed".to_string(),
                    ));
                    vec![]
                }
            }
        }
        // SurfaceOfSweep × SurfaceOfSweep
        (Surface::SurfaceOfSweep { .. }, Surface::SurfaceOfSweep { .. }) => {
            match (
                face_a.surface.to_nurbs_surface(),
                face_b.surface.to_nurbs_surface(),
            ) {
                (Some(ns_a), Some(ns_b)) => {
                    let polylines = super::nurbs_intersect::nurbs_nurbs_intersection(&ns_a, &ns_b);
                    polylines_to_curves(polylines, events)
                }
                _ => {
                    events.push(BooleanEvent::Warning(
                        "SurfaceOfSweep to NurbsSurface conversion failed".to_string(),
                    ));
                    vec![]
                }
            }
        }
        // All surface pairs covered -- unreachable
        #[allow(unreachable_patterns)]
        _ => {
            events.push(BooleanEvent::Warning(
                "unsupported surface pair".to_string(),
            ));
            vec![]
        }
    }
}

/// Plane-Plane intersection
fn plane_plane_face_intersection(
    face_a: &Face,
    shell_a: &Shell,
    face_b: &Face,
    shell_b: &Shell,
) -> Vec<Curve3D> {
    let si = match plane_plane_intersect(&face_a.surface, &face_b.surface) {
        Some(SurfaceIntersection::Line { point, direction }) => (point, direction),
        _ => return Vec::new(),
    };
    let (line_pt, line_dir) = si;

    let interval_a = clip_line_to_face(&line_pt, &line_dir, face_a, shell_a);
    let interval_b = clip_line_to_face(&line_pt, &line_dir, face_b, shell_b);

    match (interval_a, interval_b) {
        (Some((a_min, a_max)), Some((b_min, b_max))) => {
            let t0 = a_min.max(b_min);
            let t1 = a_max.min(b_max);
            if t1 - t0 < CLIP_TOL {
                return Vec::new();
            }
            let start = vec3::add(line_pt, vec3::scale(line_dir, t0));
            let end = vec3::add(line_pt, vec3::scale(line_dir, t1));
            vec![Curve3D::Line { start, end }]
        }
        _ => Vec::new(),
    }
}

/// Plane-Cylinder intersection
fn plane_cylinder_face_intersection(
    plane_face: &Face,
    plane_shell: &Shell,
    cyl_face: &Face,
    _cyl_shell: &Shell,
) -> Vec<Curve3D> {
    let si = match plane_cylinder_intersect(&plane_face.surface, &cyl_face.surface) {
        Some(si) => si,
        None => return vec![],
    };

    match si {
        SurfaceIntersection::Circle {
            center,
            axis,
            radius,
        } => {
            // Clip circle to plane face boundary
            let u = perpendicular_unit(&axis);
            let start = vec3::add(center, vec3::scale(u, radius));
            let arc = Curve3D::Arc {
                center,
                axis,
                start,
                end: start,
                radius,
            };
            clip_closed_curve_or_keep(arc, plane_face, plane_shell)
        }
        SurfaceIntersection::Ellipse {
            center,
            axis_u,
            axis_v,
        } => {
            let ellipse = Curve3D::Ellipse {
                center,
                axis_u,
                axis_v,
                t_start: 0.0,
                t_end: std::f64::consts::TAU,
            };
            clip_closed_curve_or_keep(ellipse, plane_face, plane_shell)
        }
        SurfaceIntersection::TwoLines {
            line1_point,
            line1_dir,
            line2_point,
            line2_dir,
        } => {
            let mut result = vec![];
            if let Some((t0, t1)) =
                clip_line_to_face(&line1_point, &line1_dir, plane_face, plane_shell)
            {
                let start = vec3::add(line1_point, vec3::scale(line1_dir, t0));
                let end = vec3::add(line1_point, vec3::scale(line1_dir, t1));
                if vec3::length(vec3::sub(start, end)) > CLIP_TOL {
                    result.push(Curve3D::Line { start, end });
                }
            }
            if let Some((t0, t1)) =
                clip_line_to_face(&line2_point, &line2_dir, plane_face, plane_shell)
            {
                let start = vec3::add(line2_point, vec3::scale(line2_dir, t0));
                let end = vec3::add(line2_point, vec3::scale(line2_dir, t1));
                if vec3::length(vec3::sub(start, end)) > CLIP_TOL {
                    result.push(Curve3D::Line { start, end });
                }
            }
            result
        }
        _ => vec![],
    }
}

/// Plane-Torus face intersection.
///
/// Substitutes plane implicit into torus parametrization,
/// uses Weierstrass substitution for phi, solved by `sweep_torus_intersection`.
fn plane_torus_face_intersection(
    plane_face: &Face,
    plane_shell: &Shell,
    torus_face: &Face,
    _torus_shell: &Shell,
    events: &mut Vec<BooleanEvent>,
) -> Vec<Curve3D> {
    use std::f64::consts::TAU;

    let (p_origin, p_normal) = match &plane_face.surface {
        Surface::Plane { origin, normal } => (*origin, vec3::normalized(*normal)),
        _ => return vec![],
    };
    let (tc, t_axis, big_r, little_r) = match &torus_face.surface {
        Surface::Torus {
            center,
            axis,
            major_radius,
            minor_radius,
        } => (
            *center,
            vec3::normalized(*axis),
            *major_radius,
            *minor_radius,
        ),
        _ => return vec![],
    };

    // Torus orthonormal basis
    let (bu, bv) = orthonormal_basis(t_axis);

    // Plane implicit: n * x = d
    let n = p_normal;
    let d_val = vec3::dot(n, p_origin);

    // Precompute: n*a
    let n_dot_a = vec3::dot(n, t_axis);
    // n*tc - d
    let n_dot_tc_minus_d = vec3::dot(n, tc) - d_val;

    // n*bu, n*bv
    let n_dot_bu = vec3::dot(n, bu);
    let n_dot_bv = vec3::dot(n, bv);

    let char_len = little_r;

    // Torus parametrization
    let eval_fn = |theta: f64, phi: f64| -> [f64; 3] {
        let cos_t = theta.cos();
        let sin_t = theta.sin();
        let dir = vec3::add(vec3::scale(bu, cos_t), vec3::scale(bv, sin_t));
        let r = big_r + little_r * phi.cos();
        vec3::add(
            vec3::add(tc, vec3::scale(dir, r)),
            vec3::scale(t_axis, little_r * phi.sin()),
        )
    };

    // Degenerate case: plane normal perpendicular to torus axis (n*a ~ 0).
    // When n*d(theta) = 0, the entire minor circle lies on the plane.
    if n_dot_a.abs() < 1e-10 {
        let amplitude = (n_dot_bu * n_dot_bu + n_dot_bv * n_dot_bv).sqrt();
        if amplitude < 1e-14 {
            // n orthogonal to both bu, bv -> contradiction -> no intersection
            return vec![];
        }

        // Find theta where n*d(theta) = 0
        let theta0 = (-n_dot_bu).atan2(n_dot_bv);
        let mut coplanar_circles = Vec::new();

        for &theta in &[theta0, theta0 + std::f64::consts::PI] {
            if n_dot_tc_minus_d.abs() > little_r + 1e-10 {
                continue;
            }

            if n_dot_tc_minus_d.abs() < 1e-10 {
                // Entire minor circle on plane -> sample it
                let n_pts = 64;
                let mut circle = Vec::with_capacity(n_pts + 1);
                for j in 0..=n_pts {
                    let phi = TAU * j as f64 / n_pts as f64;
                    circle.push(eval_fn(theta, phi));
                }
                coplanar_circles.push(circle);
            }
            // Nonzero case handled by sweep
        }

        if !coplanar_circles.is_empty() {
            // Also run sweep to combine results
            let phi_eq_fn_inner = |theta_: f64| -> Vec<f64> {
                let cos_t = theta_.cos();
                let sin_t = theta_.sin();
                let n_dot_d = n_dot_bu * cos_t + n_dot_bv * sin_t;
                let a = little_r * n_dot_d;
                let b = little_r * n_dot_a;
                let c = big_r * n_dot_d + n_dot_tc_minus_d;
                let norm = a.abs() + b.abs() + c.abs();
                if norm < 1e-14 {
                    return vec![];
                }
                vec![c + a, 2.0 * b, c - a]
            };
            let sweep_polylines = super::sweep_intersect::sweep_torus_intersection(
                phi_eq_fn_inner,
                eval_fn,
                (0.0, TAU),
                char_len,
            );

            let mut all_polylines = coplanar_circles;
            all_polylines.extend(sweep_polylines);

            events.push(BooleanEvent::Info(format!(
                "Plane-Torus degenerate case: {} curves detected",
                all_polylines.len()
            )));

            if !plane_face.loop_edges.is_empty() {
                let mut result = Vec::new();
                for polyline in &all_polylines {
                    let clipped = clip_polyline_to_face(polyline, plane_face, plane_shell);
                    result.extend(clipped);
                }
                if !result.is_empty() {
                    return result;
                }
            }
            return polylines_to_curves(all_polylines, events);
        }
    }

    // Closure returning phi equation coefficients

    let phi_eq_fn = |theta: f64| -> Vec<f64> {
        let cos_t = theta.cos();
        let sin_t = theta.sin();
        let n_dot_d = n_dot_bu * cos_t + n_dot_bv * sin_t;

        let a = little_r * n_dot_d;
        let b = little_r * n_dot_a;
        let c = big_r * n_dot_d + n_dot_tc_minus_d;

        // Near-zero polynomial (entire minor circle on plane) -> empty
        let norm = a.abs() + b.abs() + c.abs();
        if norm < 1e-14 {
            return vec![];
        }

        vec![c + a, 2.0 * b, c - a]
    };

    let polylines =
        super::sweep_intersect::sweep_torus_intersection(phi_eq_fn, eval_fn, (0.0, TAU), char_len);

    if polylines.is_empty() {
        return vec![];
    }

    events.push(BooleanEvent::Info(format!(
        "Plane-Torus sweep intersection: {} curves detected",
        polylines.len()
    )));

    // Clip polylines to plane face if it has bounds
    if !plane_face.loop_edges.is_empty() {
        let mut result = Vec::new();
        for polyline in &polylines {
            let clipped = clip_polyline_to_face(polyline, plane_face, plane_shell);
            result.extend(clipped);
        }
        if !result.is_empty() {
            return result;
        }
    }

    // No bounds (empty loop_edges): try NURBS fit
    polylines_to_curves(polylines, events)
}

/// Plane-Sphere intersection
fn plane_sphere_face_intersection(
    plane_face: &Face,
    plane_shell: &Shell,
    _sphere_face: &Face,
    _sphere_shell: &Shell,
) -> Vec<Curve3D> {
    let si = match plane_sphere_intersect(&plane_face.surface, &_sphere_face.surface) {
        Some(si) => si,
        None => return vec![],
    };

    match si {
        SurfaceIntersection::Circle {
            center,
            axis,
            radius,
        } => {
            let u = perpendicular_unit(&axis);
            let start = vec3::add(center, vec3::scale(u, radius));
            let arc = Curve3D::Arc {
                center,
                axis,
                start,
                end: start,
                radius,
            };
            clip_closed_curve_or_keep(arc, plane_face, plane_shell)
        }
        _ => vec![],
    }
}

fn sphere_plane_face_intersection(
    sphere_face: &Face,
    sphere_shell: &Shell,
    plane_face: &Face,
    plane_shell: &Shell,
) -> Vec<Curve3D> {
    use crate::boolean3d::classify3d::point_in_face_polygon;

    let si = match plane_sphere_intersect(&plane_face.surface, &sphere_face.surface) {
        Some(si) => si,
        None => return vec![],
    };

    match si {
        SurfaceIntersection::Circle {
            center,
            axis,
            radius,
        } => {
            let u = perpendicular_unit(&axis);
            let start = vec3::add(center, vec3::scale(u, radius));
            let full_arc = Curve3D::Arc {
                center,
                axis,
                start,
                end: start,
                radius,
            };
            let polyline = full_arc.to_polyline(1e-3);
            if polyline.len() < 2 {
                return vec![];
            }

            let mut result = Vec::new();
            let mut inside_run: Vec<[f64; 3]> = Vec::new();
            for pt in polyline {
                if point_in_face_polygon(&pt, sphere_face, sphere_shell)
                    && point_in_face_polygon(&pt, plane_face, plane_shell)
                {
                    inside_run.push(pt);
                } else if inside_run.len() >= 2 {
                    let start = inside_run[0];
                    let end = inside_run[inside_run.len() - 1];
                    if vec3::length(vec3::sub(start, end)) > CLIP_TOL {
                        result.push(Curve3D::Arc {
                            center,
                            axis,
                            start,
                            end,
                            radius,
                        });
                    }
                    inside_run.clear();
                } else {
                    inside_run.clear();
                }
            }
            if inside_run.len() >= 2 {
                let start = inside_run[0];
                let end = inside_run[inside_run.len() - 1];
                if vec3::length(vec3::sub(start, end)) > CLIP_TOL {
                    result.push(Curve3D::Arc {
                        center,
                        axis,
                        start,
                        end,
                        radius,
                    });
                }
            }
            result
        }
        _ => vec![],
    }
}

/// Sphere-Sphere face intersection
fn sphere_sphere_face_intersection(
    face_a: &Face,
    shell_a: &Shell,
    face_b: &Face,
    shell_b: &Shell,
) -> Vec<Curve3D> {
    let si = match sphere_sphere_intersect(&face_a.surface, &face_b.surface) {
        Some(si) => si,
        None => return vec![],
    };

    match si {
        SurfaceIntersection::Circle {
            center,
            axis,
            radius,
        } => {
            let u = perpendicular_unit(&axis);
            let start = vec3::add(center, vec3::scale(u, radius));
            let arc = Curve3D::Arc {
                center,
                axis,
                start,
                end: start,
                radius,
            };
            let polyline = arc.to_polyline(1e-3);
            if polyline.len() < 2 {
                return vec![];
            }
            clip_curve_polyline_to_both_faces(&arc, &polyline, face_a, shell_a, face_b, shell_b)
        }
        _ => vec![],
    }
}

/// Clip a polyline to both face boundaries, returning Line segments inside both faces.
pub(crate) fn clip_polyline_to_both_faces(
    polyline: &[[f64; 3]],
    face_a: &Face,
    shell_a: &Shell,
    face_b: &Face,
    shell_b: &Shell,
) -> Vec<Curve3D> {
    use crate::boolean3d::classify3d::point_in_face_polygon;

    let mut result = Vec::new();
    let mut inside_run: Vec<[f64; 3]> = Vec::new();

    for pt in polyline {
        if point_in_face_polygon(pt, face_a, shell_a) && point_in_face_polygon(pt, face_b, shell_b)
        {
            inside_run.push(*pt);
        } else {
            if inside_run.len() >= 2 {
                let start = inside_run[0];
                let end = inside_run[inside_run.len() - 1];
                if vec3::length(vec3::sub(start, end)) > CLIP_TOL {
                    result.push(Curve3D::Line { start, end });
                }
            }
            inside_run.clear();
        }
    }

    if inside_run.len() >= 2 {
        let start = inside_run[0];
        let end = inside_run[inside_run.len() - 1];
        if vec3::length(vec3::sub(start, end)) > CLIP_TOL {
            result.push(Curve3D::Line { start, end });
        }
    }

    result
}

fn clip_curve_polyline_to_both_faces(
    curve: &Curve3D,
    polyline: &[[f64; 3]],
    face_a: &Face,
    shell_a: &Shell,
    face_b: &Face,
    shell_b: &Shell,
) -> Vec<Curve3D> {
    use crate::boolean3d::classify3d::point_in_face_polygon;

    let mut result = Vec::new();
    let mut inside_run: Vec<[f64; 3]> = Vec::new();

    for pt in polyline {
        if point_in_face_polygon(pt, face_a, shell_a) && point_in_face_polygon(pt, face_b, shell_b)
        {
            inside_run.push(*pt);
        } else {
            flush_curve_run(&mut result, curve, &inside_run);
            inside_run.clear();
        }
    }
    flush_curve_run(&mut result, curve, &inside_run);
    result
}

/// Classify parallelism and distance between two cylinder axes.
pub(crate) fn classify_axes(
    origin_a: &[f64; 3],
    axis_a: &[f64; 3],
    origin_b: &[f64; 3],
    axis_b: &[f64; 3],
) -> (bool, f64) {
    let is_parallel = vec3::dot(*axis_a, *axis_b).abs() > 1.0 - 1e-10;

    if is_parallel {
        // Parallel: project inter-origin vector onto axis-perpendicular component
        let diff = vec3::sub(*origin_b, *origin_a);
        let along = vec3::dot(diff, *axis_a);
        let perp = vec3::sub(diff, vec3::scale(*axis_a, along));
        (true, vec3::length(perp))
    } else {
        // Non-parallel: shortest distance between two lines
        let n = vec3::normalized(vec3::cross(*axis_a, *axis_b));
        let diff = vec3::sub(*origin_b, *origin_a);
        let dist = vec3::dot(diff, n).abs();
        (false, dist)
    }
}

/// Return a unit vector perpendicular to the given axis.
fn perpendicular_unit(axis: &[f64; 3]) -> [f64; 3] {
    orthonormal_basis(*axis).0
}

/// Clip a polyline to a face boundary, returning Line segments inside the face.
fn clip_polyline_to_face(polyline: &[[f64; 3]], face: &Face, shell: &Shell) -> Vec<Curve3D> {
    use crate::boolean3d::classify3d::point_in_face_polygon;

    let mut result = Vec::new();
    let mut inside_run: Vec<[f64; 3]> = Vec::new();

    for pt in polyline {
        if point_in_face_polygon(pt, face, shell) {
            inside_run.push(*pt);
        } else {
            if inside_run.len() >= 2 {
                let start = inside_run[0];
                let end = inside_run[inside_run.len() - 1];
                if vec3::length(vec3::sub(start, end)) > CLIP_TOL {
                    result.push(Curve3D::Line { start, end });
                }
            }
            inside_run.clear();
        }
    }

    // Remaining tail
    if inside_run.len() >= 2 {
        let start = inside_run[0];
        let end = inside_run[inside_run.len() - 1];
        if vec3::length(vec3::sub(start, end)) > CLIP_TOL {
            result.push(Curve3D::Line { start, end });
        }
    }

    result
}

fn clip_curve_to_face(
    curve: &Curve3D,
    polyline: &[[f64; 3]],
    face: &Face,
    shell: &Shell,
) -> Vec<Curve3D> {
    use crate::boolean3d::classify3d::point_in_face_polygon;

    let mut result = Vec::new();
    let mut inside_run: Vec<[f64; 3]> = Vec::new();

    for pt in polyline {
        if point_in_face_polygon(pt, face, shell) {
            inside_run.push(*pt);
        } else {
            flush_curve_run(&mut result, curve, &inside_run);
            inside_run.clear();
        }
    }

    flush_curve_run(&mut result, curve, &inside_run);
    result
}

fn clip_analytic_curve_to_face(
    curve: &Curve3D,
    face: &Face,
    shell: &Shell,
) -> Option<Vec<Curve3D>> {
    use crate::boolean3d::classify3d::point_in_face_polygon;

    let (plane_origin, plane_normal) = match &face.surface {
        Surface::Plane { origin, normal } => (*origin, vec3::normalized(*normal)),
        _ => return None,
    };
    let polygon = face_polygon(face, shell);
    if polygon.len() < 3 {
        return Some(vec![]);
    }

    let (basis_u, basis_v) = orthonormal_basis(plane_normal);
    let mut hits = analytic_curve_face_hits(curve, &polygon, &plane_origin, &basis_u, &basis_v)?;
    if hits.is_empty() {
        return Some(if point_in_face_polygon(&curve.midpoint(), face, shell) {
            vec![curve.clone()]
        } else {
            vec![]
        });
    }

    let (_, period) = curve.param_range();
    hits.sort_by(|a, b| a.0.total_cmp(&b.0));
    let mut clipped = Vec::new();
    for i in 0..hits.len() {
        let start_t = hits[i].0;
        let mut end_t = hits[(i + 1) % hits.len()].0;
        if i + 1 == hits.len() {
            end_t += period;
        }
        if end_t - start_t <= 1e-6 {
            continue;
        }
        let mut mid_t = start_t + (end_t - start_t) * 0.5;
        if mid_t > period {
            mid_t -= period;
        }
        let mid = curve.evaluate(mid_t);
        if !point_in_face_polygon(&mid, face, shell) {
            continue;
        }
        let start = curve.evaluate(start_t);
        let end = curve.evaluate(if end_t > period {
            end_t - period
        } else {
            end_t
        });
        if let Some(segment) = clip_curve_between_points(curve, start, end) {
            clipped.push(segment);
        }
    }
    Some(clipped)
}

fn analytic_curve_face_hits(
    curve: &Curve3D,
    polygon: &[[f64; 3]],
    plane_origin: &[f64; 3],
    basis_u: &[f64; 3],
    basis_v: &[f64; 3],
) -> Option<Vec<(f64, [f64; 3])>> {
    let center = match curve {
        Curve3D::Arc { center, .. } | Curve3D::Ellipse { center, .. } => *center,
        _ => return None,
    };
    let center_2d = plane_coords(center, plane_origin, basis_u, basis_v);
    let mut hits: Vec<(f64, [f64; 3])> = Vec::new();
    for i in 0..polygon.len() {
        let a = plane_coords(polygon[i], plane_origin, basis_u, basis_v);
        let b = plane_coords(
            polygon[(i + 1) % polygon.len()],
            plane_origin,
            basis_u,
            basis_v,
        );
        for point in analytic_segment_intersections(curve, center_2d, a, b, basis_u, basis_v)? {
            let point3 = lift_plane_coords(point, plane_origin, basis_u, basis_v);
            let t = point_parameter_on_curve(curve, &point3, None)?;
            if hits.iter().any(|(existing_t, existing_p)| {
                (*existing_t - t).abs() <= 1e-4 || vec3::distance(*existing_p, point3) <= 1e-4
            }) {
                continue;
            }
            hits.push((t, point3));
        }
    }
    Some(hits)
}

fn analytic_segment_intersections(
    curve: &Curve3D,
    center_2d: [f64; 2],
    a: [f64; 2],
    b: [f64; 2],
    basis_u: &[f64; 3],
    basis_v: &[f64; 3],
) -> Option<Vec<[f64; 2]>> {
    let d = [b[0] - a[0], b[1] - a[1]];
    let rel = [a[0] - center_2d[0], a[1] - center_2d[1]];
    let roots = match curve {
        Curve3D::Arc { radius, .. } => quadratic_segment_roots(
            dot2(d, d),
            2.0 * dot2(rel, d),
            dot2(rel, rel) - radius * radius,
        ),
        Curve3D::Ellipse { axis_u, axis_v, .. } => {
            let axis_u_2d = [vec3::dot(*axis_u, *basis_u), vec3::dot(*axis_u, *basis_v)];
            let axis_v_2d = [vec3::dot(*axis_v, *basis_u), vec3::dot(*axis_v, *basis_v)];
            let inv = invert_2x2(axis_u_2d, axis_v_2d)?;
            let q0 = mat2_mul_vec(inv, rel);
            let qd = mat2_mul_vec(inv, d);
            quadratic_segment_roots(dot2(qd, qd), 2.0 * dot2(q0, qd), dot2(q0, q0) - 1.0)
        }
        _ => return None,
    };
    Some(
        roots
            .into_iter()
            .map(|s| [a[0] + d[0] * s, a[1] + d[1] * s])
            .collect(),
    )
}

fn quadratic_segment_roots(a: f64, b: f64, c: f64) -> Vec<f64> {
    if a.abs() <= 1e-12 {
        if b.abs() <= 1e-12 {
            return Vec::new();
        }
        let s = -c / b;
        return if (-1e-8..=1.0 + 1e-8).contains(&s) {
            vec![s.clamp(0.0, 1.0)]
        } else {
            Vec::new()
        };
    }
    let disc = b * b - 4.0 * a * c;
    if disc < -1e-10 {
        return Vec::new();
    }
    let disc = disc.max(0.0).sqrt();
    let mut roots: Vec<f64> = Vec::new();
    for s in [(-b - disc) / (2.0 * a), (-b + disc) / (2.0 * a)] {
        if (-1e-8..=1.0 + 1e-8).contains(&s)
            && !roots.iter().any(|existing| (*existing - s).abs() <= 1e-8)
        {
            roots.push(s.clamp(0.0, 1.0));
        }
    }
    roots
}

fn invert_2x2(col0: [f64; 2], col1: [f64; 2]) -> Option<[[f64; 2]; 2]> {
    let det = col0[0] * col1[1] - col1[0] * col0[1];
    if det.abs() <= 1e-12 {
        return None;
    }
    Some([
        [col1[1] / det, -col1[0] / det],
        [-col0[1] / det, col0[0] / det],
    ])
}

fn mat2_mul_vec(m: [[f64; 2]; 2], v: [f64; 2]) -> [f64; 2] {
    [
        m[0][0] * v[0] + m[0][1] * v[1],
        m[1][0] * v[0] + m[1][1] * v[1],
    ]
}

fn dot2(a: [f64; 2], b: [f64; 2]) -> f64 {
    a[0] * b[0] + a[1] * b[1]
}

fn plane_coords(
    point: [f64; 3],
    plane_origin: &[f64; 3],
    basis_u: &[f64; 3],
    basis_v: &[f64; 3],
) -> [f64; 2] {
    let rel = vec3::sub(point, *plane_origin);
    [vec3::dot(rel, *basis_u), vec3::dot(rel, *basis_v)]
}

fn lift_plane_coords(
    point: [f64; 2],
    plane_origin: &[f64; 3],
    basis_u: &[f64; 3],
    basis_v: &[f64; 3],
) -> [f64; 3] {
    vec3::add(
        vec3::add(*plane_origin, vec3::scale(*basis_u, point[0])),
        vec3::scale(*basis_v, point[1]),
    )
}

fn clip_closed_curve_or_keep(curve: Curve3D, face: &Face, shell: &Shell) -> Vec<Curve3D> {
    use crate::boolean3d::classify3d::point_in_face_polygon;

    if matches!(curve, Curve3D::Arc { .. } | Curve3D::Ellipse { .. }) {
        if let Some(clipped) = clip_analytic_curve_to_face(&curve, face, shell) {
            return clipped;
        }
    }

    let polyline = curve.to_polyline(1e-3);
    if polyline.len() < 2 {
        return vec![];
    }
    if polyline
        .iter()
        .all(|pt| point_in_face_polygon(pt, face, shell))
    {
        return vec![curve];
    }
    clip_curve_to_face(&curve, &polyline, face, shell)
}

fn flush_curve_run(result: &mut Vec<Curve3D>, curve: &Curve3D, inside_run: &[[f64; 3]]) {
    if inside_run.len() < 2 {
        return;
    }
    let start = inside_run[0];
    let end = inside_run[inside_run.len() - 1];
    if vec3::length(vec3::sub(start, end)) <= CLIP_TOL {
        return;
    }
    if let Some(clipped) = clip_curve_between_points(curve, start, end) {
        result.push(clipped);
    } else {
        result.push(Curve3D::Line { start, end });
    }
}

fn clip_curve_between_points(curve: &Curve3D, start: [f64; 3], end: [f64; 3]) -> Option<Curve3D> {
    match curve {
        Curve3D::Line { .. } => Some(Curve3D::Line { start, end }),
        Curve3D::Arc {
            center,
            axis,
            radius,
            ..
        } => {
            let t0 = point_parameter_on_curve(curve, &start, None)?;
            let t1 = point_parameter_on_curve(curve, &end, Some(t0))?;
            Some(Curve3D::Arc {
                center: *center,
                axis: if t1 >= t0 {
                    *axis
                } else {
                    vec3::scale(*axis, -1.0)
                },
                start,
                end,
                radius: *radius,
            })
        }
        Curve3D::Ellipse {
            center,
            axis_u,
            axis_v,
            ..
        } => {
            let t0 = point_parameter_on_curve(curve, &start, None)?;
            let t1 = point_parameter_on_curve(curve, &end, Some(t0))?;
            Some(Curve3D::Ellipse {
                center: *center,
                axis_u: *axis_u,
                axis_v: *axis_v,
                t_start: t0,
                t_end: t1,
            })
        }
        Curve3D::NurbsCurve3D { .. } => None,
    }
}

fn point_parameter_on_curve(
    curve: &Curve3D,
    point: &[f64; 3],
    reference: Option<f64>,
) -> Option<f64> {
    let range = curve.param_range();
    match curve {
        Curve3D::Line { start, end } => {
            let dir = vec3::sub(*end, *start);
            let len_sq = vec3::dot(dir, dir);
            if len_sq < 1e-20 {
                return None;
            }
            let t = vec3::dot(vec3::sub(*point, *start), dir) / len_sq;
            let projected = vec3::add(*start, vec3::scale(dir, t));
            (vec3::distance(projected, *point) < 1e-4 && (-1e-4..=1.0 + 1e-4).contains(&t))
                .then_some(t.clamp(0.0, 1.0))
        }
        Curve3D::Arc {
            center,
            axis,
            start,
            radius,
            ..
        } => {
            let r = vec3::sub(*point, *center);
            if (vec3::length(r) - *radius).abs() > 1e-3 {
                return None;
            }
            if vec3::dot(r, *axis).abs() > 1e-3 {
                return None;
            }
            let r0 = vec3::sub(*start, *center);
            let tangent = vec3::scale(vec3::normalized(vec3::cross(*axis, r0)), *radius);
            let x = vec3::dot(r, r0) / (*radius * *radius);
            let y = vec3::dot(r, tangent) / (*radius * *radius);
            closest_periodic_parameter(y.atan2(x), std::f64::consts::TAU, range, reference)
        }
        Curve3D::Ellipse {
            center,
            axis_u,
            axis_v,
            ..
        } => {
            let rel = vec3::sub(*point, *center);
            if vec3::dot(rel, vec3::cross(*axis_u, *axis_v)).abs() > 1e-3 {
                return None;
            }
            let u_len_sq = vec3::dot(*axis_u, *axis_u);
            let v_len_sq = vec3::dot(*axis_v, *axis_v);
            if u_len_sq < 1e-20 || v_len_sq < 1e-20 {
                return None;
            }
            let cos_t = (vec3::dot(rel, *axis_u) / u_len_sq).clamp(-1.0, 1.0);
            let sin_t = (vec3::dot(rel, *axis_v) / v_len_sq).clamp(-1.0, 1.0);
            closest_periodic_parameter(sin_t.atan2(cos_t), std::f64::consts::TAU, range, reference)
        }
        Curve3D::NurbsCurve3D { .. } => None,
    }
}

fn closest_periodic_parameter(
    raw: f64,
    period: f64,
    range: (f64, f64),
    reference: Option<f64>,
) -> Option<f64> {
    let min = range.0.min(range.1) - 1e-4;
    let max = range.0.max(range.1) + 1e-4;
    let mut candidates = Vec::new();
    for shift in -2..=2 {
        let candidate = raw + shift as f64 * period;
        if (min..=max).contains(&candidate) {
            candidates.push(candidate);
        }
    }
    if candidates.is_empty() {
        for shift in -2..=2 {
            candidates.push(raw + shift as f64 * period);
        }
    }
    if let Some(reference) = reference {
        candidates
            .into_iter()
            .min_by(|a, b| (a - reference).abs().total_cmp(&(b - reference).abs()))
    } else {
        let anchor = if range.0 <= range.1 { range.0 } else { range.1 };
        candidates
            .into_iter()
            .min_by(|a, b| (a - anchor).abs().total_cmp(&(b - anchor).abs()))
    }
}

/// Extract vertex coordinates from face edge loop.
pub fn face_polygon(face: &Face, shell: &Shell) -> Vec<[f64; 3]> {
    face.loop_edges
        .iter()
        .map(|er| {
            let edge = &shell.edges[er.edge_id];
            let vid = if er.forward { edge.v_start } else { edge.v_end };
            shell.vertices[vid]
        })
        .collect()
}

/// Sample a face boundary as a UV polyline on the underlying surface.
#[allow(dead_code)]
pub(crate) fn face_boundary_uv_polyline(
    face: &Face,
    shell: &Shell,
    samples_per_edge: usize,
) -> Option<Vec<[f64; 2]>> {
    let mut uv = Vec::new();

    for edge_ref in &face.loop_edges {
        let edge = &shell.edges[edge_ref.edge_id];
        let (t0, t1) = edge.curve.param_range();
        let n = samples_per_edge.max(1);
        for k in 0..=n {
            if !uv.is_empty() && k == 0 {
                continue;
            }
            let frac = k as f64 / n as f64;
            let t = if edge_ref.forward {
                t0 + frac * (t1 - t0)
            } else {
                t1 - frac * (t1 - t0)
            };
            let pt = edge.curve.evaluate(t);
            let (u, v) = face.surface.inverse_project(&pt)?;
            uv.push([u, v]);
        }
    }

    Some(uv)
}

/// Split a face by cut curves into SubFaces.
pub fn split_face(
    face: &Face,
    shell: &Shell,
    cuts: &[Curve3D],
    source_shell: usize,
    source_face: usize,
) -> Vec<SubFace> {
    let polygon = face_polygon(face, shell);

    if cuts.is_empty() {
        return vec![SubFace {
            surface: face.surface.clone(),
            polygon,
            candidate_curves: cuts.to_vec(),
            flipped: false,
            source_shell,
            source_face,
        }];
    }

    let mut current_polygons = vec![polygon];

    for cut in cuts {
        let mut next_polygons = Vec::new();
        for poly in &current_polygons {
            next_polygons.extend(split_polygon_by_curve(poly, cut, &face.surface));
        }
        if next_polygons.is_empty() {
            next_polygons = current_polygons.clone();
        }
        current_polygons = next_polygons;
    }

    current_polygons
        .into_iter()
        .map(|polygon| SubFace {
            surface: face.surface.clone(),
            polygon,
            candidate_curves: cuts.to_vec(),
            flipped: false,
            source_shell,
            source_face,
        })
        .collect()
}

/// Split polygon by a curve. Non-line curves use polyline endpoint approximation.
///
/// For NurbsSurface, projects onto an approximate plane using centroid normal.
fn split_polygon_by_curve(
    polygon: &[[f64; 3]],
    cut: &Curve3D,
    surface: &Surface,
) -> Vec<Vec<[f64; 3]>> {
    if let Some(split) = split_polygon_by_support_plane(polygon, cut, surface) {
        let (left, right) = split;
        return vec![
            project_polygon_to_surface(&left, surface),
            project_polygon_to_surface(&right, surface),
        ];
    }

    if let Some(split) = split_polygon_by_curve_uv(polygon, cut, surface) {
        return split;
    }

    // For NurbsSurface, build approximate plane from centroid normal
    let effective_surface: Option<Surface> = match surface {
        Surface::NurbsSurface { data } => {
            let n = polygon.len() as f64;
            if n < 1.0 {
                None
            } else {
                let cx = polygon.iter().map(|p| p[0]).sum::<f64>() / n;
                let cy = polygon.iter().map(|p| p[1]).sum::<f64>() / n;
                let cz = polygon.iter().map(|p| p[2]).sum::<f64>() / n;
                let centroid = [cx, cy, cz];
                // Project centroid to surface to get normal
                let (u, v) = project_to_nurbs(data, &centroid);
                let normal = data.normal(u, v);
                let origin = data.evaluate(u, v);
                Some(Surface::Plane { origin, normal })
            }
        }
        _ => None,
    };
    let surf = effective_surface.as_ref().unwrap_or(surface);

    match cut {
        Curve3D::Line { .. } => polygons_from_pair(split_polygon_by_line(polygon, cut, surf)),
        _ => {
            // Convert curve to polyline and split by line through endpoints
            let polyline = cut.to_polyline(1e-3);
            if polyline.len() < 2 {
                return vec![polygon.to_vec()];
            }
            let approx = Curve3D::Line {
                start: polyline[0],
                end: polyline[polyline.len() - 1],
            };
            polygons_from_pair(split_polygon_by_line(polygon, &approx, surf))
        }
    }
}

fn project_polygon_to_surface(polygon: &[[f64; 3]], surface: &Surface) -> Vec<[f64; 3]> {
    polygon
        .iter()
        .map(|point| match surface.inverse_project(point) {
            Some((u, v)) => surface.evaluate(u, v),
            None => *point,
        })
        .collect()
}

fn polygons_from_pair(split: (Vec<[f64; 3]>, Vec<[f64; 3]>)) -> Vec<Vec<[f64; 3]>> {
    let (left, right) = split;
    let mut polygons = Vec::new();
    if !left.is_empty() {
        polygons.push(left);
    }
    if !right.is_empty() {
        polygons.push(right);
    }
    polygons
}

/// Split polygon by a cut line using Sutherland-Hodgman with orient2d exact predicates.
fn split_polygon_by_line(
    polygon: &[[f64; 3]],
    cut: &Curve3D,
    surface: &Surface,
) -> (Vec<[f64; 3]>, Vec<[f64; 3]>) {
    let (cut_start, cut_end) = match cut {
        Curve3D::Line { start, end } => (start, end),
        _ => return (polygon.to_vec(), Vec::new()),
    };
    let surface_normal = match surface {
        Surface::Plane { normal, .. } => normal,
        _ => return (polygon.to_vec(), Vec::new()),
    };

    // Check for degenerate cut direction
    let cut_dir = vec3::sub(*cut_end, *cut_start);
    if vec3::length(cut_dir) < 1e-15 {
        return (polygon.to_vec(), Vec::new());
    }

    // Drop largest normal component for 2D projection
    let abs_n = [
        surface_normal[0].abs(),
        surface_normal[1].abs(),
        surface_normal[2].abs(),
    ];
    let drop_axis = if abs_n[0] >= abs_n[1] && abs_n[0] >= abs_n[2] {
        0
    } else if abs_n[1] >= abs_n[2] {
        1
    } else {
        2
    };

    let project = |pt: &[f64; 3]| -> [f64; 2] {
        match drop_axis {
            0 => [pt[1], pt[2]],
            1 => [pt[0], pt[2]],
            _ => [pt[0], pt[1]],
        }
    };

    let cs2d = project(cut_start);
    let ce2d = project(cut_end);

    // Classify vertices via orient2d: positive=left, negative=right, 0=on-line
    let orients: Vec<f64> = polygon
        .iter()
        .map(|p| neco_cdt::orient2d(cs2d, ce2d, project(p)))
        .collect();

    let n = polygon.len();
    let mut left = Vec::new();
    let mut right = Vec::new();

    for i in 0..n {
        let j = (i + 1) % n;
        let oi = orients[i];
        let oj = orients[j];
        let pi = &polygon[i];
        let pj = &polygon[j];

        // orient >= 0 -> left (on-line included in both)
        if oi >= 0.0 {
            left.push(*pi);
        }
        // orient <= 0 → right
        if oi <= 0.0 {
            right.push(*pi);
        }

        // Edge crosses cut line (strict sign reversal)
        if (oi > 0.0 && oj < 0.0) || (oi < 0.0 && oj > 0.0) {
            // Linear interpolation for intersection point (inexact construction)
            let cut_normal = vec3::cross(cut_dir, *surface_normal);
            let cut_normal_len = vec3::length(cut_normal);
            let cut_normal = vec3::scale(cut_normal, 1.0 / cut_normal_len);
            let di = vec3::dot(vec3::sub(*pi, *cut_start), cut_normal);
            let dj = vec3::dot(vec3::sub(*pj, *cut_start), cut_normal);
            let t = di / (di - dj);
            let ix = vec3::add(*pi, vec3::scale(vec3::sub(*pj, *pi), t));
            left.push(ix);
            right.push(ix);
        }
    }

    // Remove degenerate polygons (< 3 vertices)
    if left.len() < 3 {
        left.clear();
    }
    if right.len() < 3 {
        right.clear();
    }

    (left, right)
}

fn split_polygon_by_support_plane(
    polygon: &[[f64; 3]],
    cut: &Curve3D,
    surface: &Surface,
) -> Option<PolygonSplit3D> {
    let (plane_o, plane_n) = support_plane_for_split(surface, cut)?;
    let (pos, neg) = split_polygon_by_plane(polygon, &plane_o, &plane_n);
    if pos.len() >= 3 && neg.len() >= 3 {
        Some((pos, neg))
    } else {
        None
    }
}

fn support_plane_for_split(surface: &Surface, cut: &Curve3D) -> Option<([f64; 3], [f64; 3])> {
    match (surface, cut) {
        (Surface::Sphere { .. } | Surface::Plane { .. }, Curve3D::Arc { center, axis, .. }) => {
            Some((*center, vec3::normalized(*axis)))
        }
        (
            Surface::Plane { .. } | Surface::Cylinder { .. } | Surface::Cone { .. },
            Curve3D::Ellipse {
                center,
                axis_u,
                axis_v,
                ..
            },
        ) => {
            let plane_n = vec3::cross(*axis_u, *axis_v);
            (vec3::length(plane_n) > 1e-12).then_some((*center, vec3::normalized(plane_n)))
        }
        (
            Surface::Cone {
                axis: surface_axis, ..
            },
            Curve3D::Arc { center, axis, .. },
        ) => {
            let surface_axis = vec3::normalized(*surface_axis);
            let curve_axis = vec3::normalized(*axis);
            let aligned = vec3::dot(surface_axis, curve_axis).abs() >= 1.0 - 1e-6;
            aligned.then_some((*center, curve_axis))
        }
        _ => None,
    }
}

fn split_polygon_by_plane(
    polygon: &[[f64; 3]],
    plane_origin: &[f64; 3],
    plane_normal: &[f64; 3],
) -> (Vec<[f64; 3]>, Vec<[f64; 3]>) {
    let mut pos = Vec::new();
    let mut neg = Vec::new();
    let n = polygon.len();
    let dists: Vec<f64> = polygon
        .iter()
        .map(|p| vec3::dot(vec3::sub(*p, *plane_origin), *plane_normal))
        .collect();

    for i in 0..n {
        let j = (i + 1) % n;
        let pi = polygon[i];
        let pj = polygon[j];
        let di = dists[i];
        let dj = dists[j];

        if di >= -1e-9 {
            pos.push(pi);
        }
        if di <= 1e-9 {
            neg.push(pi);
        }

        if (di > 0.0 && dj < 0.0) || (di < 0.0 && dj > 0.0) {
            let t = di / (di - dj);
            let hit = vec3::add(pi, vec3::scale(vec3::sub(pj, pi), t));
            pos.push(hit);
            neg.push(hit);
        }
    }

    (dedup_open_3d(&pos), dedup_open_3d(&neg))
}

fn split_polygon_by_curve_uv(
    polygon: &[[f64; 3]],
    cut: &Curve3D,
    surface: &Surface,
) -> Option<Vec<Vec<[f64; 3]>>> {
    if polygon.len() < 3 {
        return None;
    }

    let polygon_uv = polygon_to_uv(surface, polygon)?;
    let cut_polyline = sample_curve_for_split(surface, cut, 1e-3)?;
    if cut_polyline.len() < 2 {
        return None;
    }
    let cut_uv = polygon_to_uv(surface, &cut_polyline)?;
    if cut_uv.len() < 2 {
        return None;
    }

    if curve_is_closed(cut) {
        let closed_uv = dedup_uv_ring(&cut_uv);
        if closed_uv.len() < 3 || !point_in_polygon_2d(uv_centroid(&closed_uv), &polygon_uv) {
            return None;
        }
        return match surface {
            Surface::Plane { .. } => {
                split_polygon_by_closed_uv_loop(surface, polygon, &polygon_uv, &closed_uv)
            }
            _ => {
                let closed_3d: Vec<[f64; 3]> = closed_uv
                    .iter()
                    .map(|[u, v]| surface.evaluate(*u, *v))
                    .collect();
                if closed_3d.len() < 3 {
                    None
                } else {
                    Some(vec![closed_3d, polygon.to_vec()])
                }
            }
        };
    }

    split_polygon_by_open_uv_path(polygon, &polygon_uv, &cut_polyline, &cut_uv)
        .map(polygons_from_pair)
}

fn sample_curve_for_split(
    surface: &Surface,
    curve: &Curve3D,
    chord_tol: f64,
) -> Option<Vec<[f64; 3]>> {
    match curve {
        Curve3D::Line { start, end } => Some(vec![*start, *end]),
        Curve3D::Arc { .. } | Curve3D::Ellipse { .. } => {
            let (t0, t1) = curve.param_range();
            let mut points = vec![curve.evaluate(t0)];
            adaptive_sample_curve_on_surface(surface, curve, t0, t1, chord_tol, &mut points)?;
            Some(points)
        }
        Curve3D::NurbsCurve3D { .. } => {
            let polyline = curve.to_polyline(chord_tol);
            (polyline.len() >= 2).then_some(polyline)
        }
    }
}

fn adaptive_sample_curve_on_surface(
    surface: &Surface,
    curve: &Curve3D,
    t0: f64,
    t1: f64,
    tol: f64,
    points: &mut Vec<[f64; 3]>,
) -> Option<()> {
    if (t1 - t0).abs() < 1e-10 {
        let point = curve.evaluate(t1);
        surface.inverse_project(&point)?;
        points.push(point);
        return Some(());
    }

    let p0 = curve.evaluate(t0);
    let p1 = curve.evaluate(t1);
    surface.inverse_project(&p0)?;
    surface.inverse_project(&p1)?;
    let mid_t = (t0 + t1) * 0.5;
    let mid_curve = curve.evaluate(mid_t);
    surface.inverse_project(&mid_curve)?;
    let mid_chord = vec3::scale(vec3::add(p0, p1), 0.5);
    if vec3::length(vec3::sub(mid_curve, mid_chord)) > tol {
        adaptive_sample_curve_on_surface(surface, curve, t0, mid_t, tol, points)?;
        adaptive_sample_curve_on_surface(surface, curve, mid_t, t1, tol, points)?;
    } else {
        points.push(p1);
    }
    Some(())
}

fn curve_is_closed(curve: &Curve3D) -> bool {
    match curve {
        Curve3D::Arc { start, end, .. } => vec3::distance(*start, *end) <= 1e-8,
        Curve3D::Ellipse { t_start, t_end, .. } => {
            (t_end - t_start).abs() >= std::f64::consts::TAU - 1e-8
        }
        Curve3D::Line { start, end } => vec3::distance(*start, *end) <= 1e-8,
        Curve3D::NurbsCurve3D { .. } => {
            let (t0, t1) = curve.param_range();
            vec3::distance(curve.evaluate(t0), curve.evaluate(t1)) <= 1e-8
        }
    }
}

fn polygon_to_uv(surface: &Surface, polygon: &[[f64; 3]]) -> Option<Vec<[f64; 2]>> {
    polygon
        .iter()
        .map(|p| surface.inverse_project(p).map(|(u, v)| [u, v]))
        .collect()
}

fn split_polygon_by_open_uv_path(
    polygon_3d: &[[f64; 3]],
    polygon_uv: &[[f64; 2]],
    cut_3d: &[[f64; 3]],
    cut_uv: &[[f64; 2]],
) -> Option<PolygonSplit3D> {
    let n = polygon_uv.len();
    if n < 3 || cut_uv.len() < 2 {
        return None;
    }

    let start_hit = locate_point_on_polygon_edge(cut_uv[0], polygon_uv, polygon_3d)?;
    let end_hit = locate_point_on_polygon_edge(cut_uv[cut_uv.len() - 1], polygon_uv, polygon_3d)?;

    let chain_forward_uv = boundary_chain_uv(
        polygon_uv,
        start_hit.edge_index,
        end_hit.edge_index,
        start_hit.point,
        end_hit.point,
    );
    let chain_backward_uv = boundary_chain_uv(
        polygon_uv,
        end_hit.edge_index,
        start_hit.edge_index,
        end_hit.point,
        start_hit.point,
    );
    let chain_forward_3d = boundary_chain_3d(
        polygon_3d,
        start_hit.edge_index,
        end_hit.edge_index,
        start_hit.point3d,
        end_hit.point3d,
    );
    let chain_backward_3d = boundary_chain_3d(
        polygon_3d,
        end_hit.edge_index,
        start_hit.edge_index,
        end_hit.point3d,
        start_hit.point3d,
    );

    let mut left_uv = cut_uv.to_vec();
    left_uv.extend_from_slice(&chain_backward_uv[1..]);
    let mut right_uv = cut_uv.iter().rev().copied().collect::<Vec<_>>();
    right_uv.extend_from_slice(&chain_forward_uv[1..]);

    let mut left_3d = cut_3d.to_vec();
    left_3d.extend_from_slice(&chain_backward_3d[1..]);
    let mut right_3d = cut_3d.iter().rev().copied().collect::<Vec<_>>();
    right_3d.extend_from_slice(&chain_forward_3d[1..]);

    left_uv = dedup_open_uv(&left_uv);
    right_uv = dedup_open_uv(&right_uv);
    left_3d = dedup_open_3d(&left_3d);
    right_3d = dedup_open_3d(&right_3d);

    if left_uv.len() < 3 || right_uv.len() < 3 {
        return None;
    }
    if polygon_area_2d(&left_uv).abs() < 1e-10 || polygon_area_2d(&right_uv).abs() < 1e-10 {
        return None;
    }

    Some((left_3d, right_3d))
}

#[derive(Clone, Copy)]
struct RingHit {
    point: [f64; 2],
}

fn split_polygon_by_closed_uv_loop(
    surface: &Surface,
    _polygon_3d: &[[f64; 3]],
    polygon_uv: &[[f64; 2]],
    closed_uv: &[[f64; 2]],
) -> Option<Vec<Vec<[f64; 3]>>> {
    let mut outer_uv = dedup_uv_ring(polygon_uv);
    if outer_uv.len() < 3 {
        return None;
    }
    if polygon_area_2d(&outer_uv) < 0.0 {
        outer_uv.reverse();
    }

    let mut hole_uv = closed_uv.to_vec();
    if polygon_area_2d(&hole_uv) > 0.0 {
        hole_uv.reverse();
    }

    let hole_center = uv_centroid(&hole_uv);
    let hole_hits: Vec<RingHit> = outer_uv
        .iter()
        .map(|vertex| intersect_ray_with_closed_uv_ring(hole_center, *vertex, &hole_uv))
        .collect::<Option<_>>()?;

    let mut polygons = Vec::new();
    for i in 0..outer_uv.len() {
        let j = (i + 1) % outer_uv.len();
        let mut poly_uv = vec![
            outer_uv[i],
            outer_uv[j],
            hole_hits[j].point,
            hole_hits[i].point,
        ];
        poly_uv = dedup_open_uv(&poly_uv);
        if poly_uv.len() < 3 || polygon_area_2d(&poly_uv).abs() < 1e-10 {
            continue;
        }

        let polygon: Vec<[f64; 3]> = poly_uv
            .iter()
            .map(|[u, v]| surface.evaluate(*u, *v))
            .collect();
        if polygon.len() >= 3 {
            polygons.push(polygon);
        }
    }

    if polygons.is_empty() {
        None
    } else {
        Some(polygons)
    }
}

fn intersect_ray_with_closed_uv_ring(
    origin: [f64; 2],
    target: [f64; 2],
    ring: &[[f64; 2]],
) -> Option<RingHit> {
    let ray = [target[0] - origin[0], target[1] - origin[1]];
    let mut best: Option<(f64, RingHit)> = None;
    for i in 0..ring.len() {
        let a = ring[i];
        let b = ring[(i + 1) % ring.len()];
        let seg = [b[0] - a[0], b[1] - a[1]];
        let denom = cross2d(ray, seg);
        if denom.abs() <= 1e-12 {
            continue;
        }
        let qp = [a[0] - origin[0], a[1] - origin[1]];
        let t = cross2d(qp, seg) / denom;
        let u = cross2d(qp, ray) / denom;
        if !(1e-9..=1.0 + 1e-9).contains(&t) || !(-1e-9..=1.0 + 1e-9).contains(&u) {
            continue;
        }

        let hit = RingHit {
            point: [origin[0] + ray[0] * t, origin[1] + ray[1] * t],
        };
        if best.map(|(best_t, _)| t < best_t).unwrap_or(true) {
            best = Some((t, hit));
        }
    }
    best.map(|(_, hit)| hit)
}

#[derive(Clone, Copy)]
struct EdgeHit {
    edge_index: usize,
    point: [f64; 2],
    point3d: [f64; 3],
}

fn locate_point_on_polygon_edge(
    point: [f64; 2],
    polygon_uv: &[[f64; 2]],
    polygon_3d: &[[f64; 3]],
) -> Option<EdgeHit> {
    let mut best: Option<(usize, f64, [f64; 2], [f64; 3])> = None;
    for i in 0..polygon_uv.len() {
        let a = polygon_uv[i];
        let b = polygon_uv[(i + 1) % polygon_uv.len()];
        let a3 = polygon_3d[i];
        let b3 = polygon_3d[(i + 1) % polygon_3d.len()];
        let (dist_sq, t) = point_segment_distance_sq_2d(point, a, b);
        if best
            .map(|(_, best_dist, _, _)| dist_sq < best_dist)
            .unwrap_or(true)
        {
            let proj = [a[0] + (b[0] - a[0]) * t, a[1] + (b[1] - a[1]) * t];
            let proj3 = vec3::add(a3, vec3::scale(vec3::sub(b3, a3), t));
            best = Some((i, dist_sq, proj, proj3));
        }
    }
    let (edge_index, dist_sq, point, point3d) = best?;
    if dist_sq > 2.5e-3 {
        return None;
    }
    Some(EdgeHit {
        edge_index,
        point,
        point3d,
    })
}

fn boundary_chain_uv(
    polygon: &[[f64; 2]],
    start_edge: usize,
    end_edge: usize,
    start_point: [f64; 2],
    end_point: [f64; 2],
) -> Vec<[f64; 2]> {
    let n = polygon.len();
    let mut chain = vec![start_point];
    let mut i = (start_edge + 1) % n;
    loop {
        chain.push(polygon[i]);
        if i == end_edge {
            break;
        }
        i = (i + 1) % n;
    }
    chain.push(end_point);
    chain
}

fn boundary_chain_3d(
    polygon: &[[f64; 3]],
    start_edge: usize,
    end_edge: usize,
    start_point: [f64; 3],
    end_point: [f64; 3],
) -> Vec<[f64; 3]> {
    let n = polygon.len();
    let mut chain = vec![start_point];
    let mut i = (start_edge + 1) % n;
    loop {
        chain.push(polygon[i]);
        if i == end_edge {
            break;
        }
        i = (i + 1) % n;
    }
    chain.push(end_point);
    chain
}

fn point_segment_distance_sq_2d(p: [f64; 2], a: [f64; 2], b: [f64; 2]) -> (f64, f64) {
    let ab = [b[0] - a[0], b[1] - a[1]];
    let ap = [p[0] - a[0], p[1] - a[1]];
    let ab_len_sq = ab[0] * ab[0] + ab[1] * ab[1];
    if ab_len_sq <= 1e-16 {
        let dx = p[0] - a[0];
        let dy = p[1] - a[1];
        return (dx * dx + dy * dy, 0.0);
    }
    let t = ((ap[0] * ab[0] + ap[1] * ab[1]) / ab_len_sq).clamp(0.0, 1.0);
    let proj = [a[0] + ab[0] * t, a[1] + ab[1] * t];
    let dx = p[0] - proj[0];
    let dy = p[1] - proj[1];
    (dx * dx + dy * dy, t)
}

fn uv_distance_sq(a: [f64; 2], b: [f64; 2]) -> f64 {
    let dx = a[0] - b[0];
    let dy = a[1] - b[1];
    dx * dx + dy * dy
}

fn cross2d(a: [f64; 2], b: [f64; 2]) -> f64 {
    a[0] * b[1] - a[1] * b[0]
}

fn dedup_uv_ring(points: &[[f64; 2]]) -> Vec<[f64; 2]> {
    let mut deduped = dedup_open_uv(points);
    if deduped.len() >= 2 && uv_distance_sq(deduped[0], *deduped.last().unwrap()) <= 1e-10 {
        deduped.pop();
    }
    deduped
}

fn dedup_open_uv(points: &[[f64; 2]]) -> Vec<[f64; 2]> {
    let mut result = Vec::new();
    for &point in points {
        if result
            .last()
            .map(|last| uv_distance_sq(*last, point) > 1e-10)
            .unwrap_or(true)
        {
            result.push(point);
        }
    }
    result
}

fn dedup_open_3d(points: &[[f64; 3]]) -> Vec<[f64; 3]> {
    let mut result = Vec::new();
    for &point in points {
        if result
            .last()
            .map(|last| vec3::length(vec3::sub(*last, point)) > 1e-10)
            .unwrap_or(true)
        {
            result.push(point);
        }
    }
    result
}

fn polygon_area_2d(points: &[[f64; 2]]) -> f64 {
    let mut area = 0.0;
    for i in 0..points.len() {
        let a = points[i];
        let b = points[(i + 1) % points.len()];
        area += a[0] * b[1] - b[0] * a[1];
    }
    area * 0.5
}

fn uv_centroid(points: &[[f64; 2]]) -> [f64; 2] {
    let mut sum = [0.0, 0.0];
    for point in points {
        sum[0] += point[0];
        sum[1] += point[1];
    }
    [sum[0] / points.len() as f64, sum[1] / points.len() as f64]
}

fn point_in_polygon_2d(point: [f64; 2], polygon: &[[f64; 2]]) -> bool {
    if polygon.len() < 3 {
        return false;
    }
    let mut inside = false;
    let (px, py) = (point[0], point[1]);
    for i in 0..polygon.len() {
        let a = polygon[i];
        let b = polygon[(i + 1) % polygon.len()];
        let intersects = ((a[1] > py) != (b[1] > py))
            && (px < (b[0] - a[0]) * (py - a[1]) / ((b[1] - a[1]).abs().max(1e-30)) + a[0]);
        if intersects {
            inside = !inside;
        }
    }
    inside
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plane_cylinder_perpendicular_gives_circle() {
        let plane = Surface::Plane {
            origin: [0.0, 0.5, 0.0],
            normal: [0.0, 1.0, 0.0],
        };
        let cyl = Surface::Cylinder {
            origin: [0.0, 0.0, 0.0],
            axis: [0.0, 1.0, 0.0],
            radius: 1.0,
        };
        let result = plane_cylinder_intersect(&plane, &cyl);
        match result {
            Some(SurfaceIntersection::Circle { center, radius, .. }) => {
                assert!((center[1] - 0.5).abs() < 1e-10);
                assert!((radius - 1.0).abs() < 1e-10);
            }
            other => panic!("Expected Circle, got {other:?}"),
        }
    }

    #[test]
    fn plane_cylinder_parallel_gives_two_lines() {
        let plane = Surface::Plane {
            origin: [0.5, 0.0, 0.0],
            normal: [1.0, 0.0, 0.0],
        };
        let cyl = Surface::Cylinder {
            origin: [0.0, 0.0, 0.0],
            axis: [0.0, 1.0, 0.0],
            radius: 1.0,
        };
        let result = plane_cylinder_intersect(&plane, &cyl);
        assert!(
            matches!(result, Some(SurfaceIntersection::TwoLines { .. })),
            "Expected TwoLines, got {result:?}"
        );
    }

    #[test]
    fn plane_cylinder_oblique_gives_ellipse() {
        let plane = Surface::Plane {
            origin: [0.0, 0.0, 0.0],
            normal: vec3::normalized([0.0, 1.0, 1.0]),
        };
        let cyl = Surface::Cylinder {
            origin: [0.0, 0.0, 0.0],
            axis: [0.0, 1.0, 0.0],
            radius: 1.0,
        };
        match plane_cylinder_intersect(&plane, &cyl) {
            Some(SurfaceIntersection::Ellipse {
                center,
                axis_u,
                axis_v,
            }) => {
                // Verify point lies on both plane and cylinder
                let p = vec3::add(center, axis_u);
                // On plane: normal * (p - origin) ~ 0
                let n = vec3::normalized([0.0, 1.0, 1.0]);
                assert!(vec3::dot(n, p).abs() < 1e-10, "ellipse point not on plane");
                // On cylinder: distance from axis ~ radius
                let q = [p[0], 0.0, p[2]]; // remove axis component
                assert!(
                    (vec3::length(q) - 1.0).abs() < 1e-10,
                    "ellipse point not on cylinder"
                );

                // Also verify axis_v direction
                let p2 = vec3::add(center, axis_v);
                assert!(
                    vec3::dot(n, p2).abs() < 1e-10,
                    "ellipse point (v) not on plane"
                );
                let q2 = [p2[0], 0.0, p2[2]];
                assert!(
                    (vec3::length(q2) - 1.0).abs() < 1e-10,
                    "ellipse point (v) not on cylinder"
                );
            }
            other => panic!("Expected Ellipse, got {other:?}"),
        }
    }

    #[test]
    fn plane_cylinder_no_intersection() {
        let plane = Surface::Plane {
            origin: [2.0, 0.0, 0.0],
            normal: [1.0, 0.0, 0.0],
        };
        let cyl = Surface::Cylinder {
            origin: [0.0, 0.0, 0.0],
            axis: [0.0, 1.0, 0.0],
            radius: 1.0,
        };
        assert!(plane_cylinder_intersect(&plane, &cyl).is_none());
    }

    #[test]
    fn plane_sphere_intersect_basic() {
        let plane = Surface::Plane {
            origin: [0.0, 0.5, 0.0],
            normal: [0.0, 1.0, 0.0],
        };
        let sphere = Surface::Sphere {
            center: [0.0, 0.0, 0.0],
            radius: 1.0,
        };
        match plane_sphere_intersect(&plane, &sphere) {
            Some(SurfaceIntersection::Circle { center, radius, .. }) => {
                assert!((center[1] - 0.5).abs() < 1e-10);
                let expected_r = (1.0_f64 - 0.25).sqrt();
                assert!((radius - expected_r).abs() < 1e-10);
            }
            other => panic!("Expected Circle, got {other:?}"),
        }
    }

    #[test]
    fn plane_sphere_no_intersection() {
        let plane = Surface::Plane {
            origin: [0.0, 2.0, 0.0],
            normal: [0.0, 1.0, 0.0],
        };
        let sphere = Surface::Sphere {
            center: [0.0, 0.0, 0.0],
            radius: 1.0,
        };
        assert!(plane_sphere_intersect(&plane, &sphere).is_none());
    }

    #[test]
    fn sphere_sphere_intersect_partial_overlap() {
        // Partially overlapping spheres
        let s1 = Surface::Sphere {
            center: [0.0, 0.0, 0.0],
            radius: 1.0,
        };
        let s2 = Surface::Sphere {
            center: [1.0, 0.0, 0.0],
            radius: 1.0,
        };
        match sphere_sphere_intersect(&s1, &s2) {
            Some(SurfaceIntersection::Circle {
                center,
                axis,
                radius,
            }) => {
                // h = (1 + 1 - 1) / 2 = 0.5
                assert!((center[0] - 0.5).abs() < 1e-10, "circle center x = 0.5");
                assert!(center[1].abs() < 1e-10);
                assert!(center[2].abs() < 1e-10);
                // Axis along (1,0,0)
                assert!((axis[0] - 1.0).abs() < 1e-10, "axis direction is +X");
                // r = sqrt(1 - 0.25) = sqrt(0.75)
                let expected_r = (0.75_f64).sqrt();
                assert!((radius - expected_r).abs() < 1e-10, "circle radius");
            }
            other => panic!("expected Circle for partial overlap: {other:?}"),
        }
    }

    #[test]
    fn sphere_sphere_intersect_no_overlap() {
        // Separated spheres
        let s1 = Surface::Sphere {
            center: [0.0, 0.0, 0.0],
            radius: 1.0,
        };
        let s2 = Surface::Sphere {
            center: [3.0, 0.0, 0.0],
            radius: 1.0,
        };
        assert!(
            sphere_sphere_intersect(&s1, &s2).is_none(),
            "separated spheres have no intersection"
        );
    }

    #[test]
    fn sphere_sphere_intersect_containment() {
        // Contained spheres
        let s1 = Surface::Sphere {
            center: [0.0, 0.0, 0.0],
            radius: 2.0,
        };
        let s2 = Surface::Sphere {
            center: [0.0, 0.0, 0.0],
            radius: 0.5,
        };
        assert!(
            sphere_sphere_intersect(&s1, &s2).is_none(),
            "contained spheres have no intersection"
        );
    }

    #[test]
    fn sphere_sphere_intersect_coincident() {
        // Identical spheres
        let s1 = Surface::Sphere {
            center: [1.0, 2.0, 3.0],
            radius: 1.0,
        };
        let s2 = Surface::Sphere {
            center: [1.0, 2.0, 3.0],
            radius: 1.0,
        };
        assert!(
            sphere_sphere_intersect(&s1, &s2).is_none(),
            "coincident spheres have no intersection"
        );
    }

    #[test]
    fn sphere_sphere_intersect_tangent() {
        // Externally tangent
        let s1 = Surface::Sphere {
            center: [0.0, 0.0, 0.0],
            radius: 1.0,
        };
        let s2 = Surface::Sphere {
            center: [2.0, 0.0, 0.0],
            radius: 1.0,
        };
        assert!(
            sphere_sphere_intersect(&s1, &s2).is_none(),
            "tangent spheres have no intersection"
        );
    }

    // ─── Plane x Cone tests ───

    #[test]
    fn plane_cone_perpendicular_gives_circle() {
        // Perpendicular plane -> circle
        let plane = Surface::Plane {
            origin: [0.0, 1.0, 0.0],
            normal: [0.0, 1.0, 0.0],
        };
        let cone = Surface::Cone {
            origin: [0.0, 0.0, 0.0],
            axis: [0.0, 2.0, 0.0],
            half_angle: std::f64::consts::FRAC_PI_4,
        };
        let result = plane_cone_intersect(&plane, &cone);
        assert!(result.is_some(), "intersection expected");
        let intersections = result.unwrap();
        assert_eq!(intersections.len(), 1, "one intersection");
        match &intersections[0] {
            SurfaceIntersection::Circle { radius, .. } => {
                // h=1, half_angle=π/4 → r = 1·tan(π/4) = 1
                assert!(
                    (*radius - 1.0).abs() < 1e-8,
                    "radius = tan(pi/4) = 1: {radius}"
                );
            }
            other => panic!("expected Circle: {other:?}"),
        }
    }

    #[test]
    fn plane_cone_through_apex_gives_two_lines() {
        // Plane through apex -> 2 lines
        let plane = Surface::Plane {
            origin: [0.0, 0.0, 0.0],
            normal: [1.0, 0.0, 0.0],
        };
        let cone = Surface::Cone {
            origin: [0.0, 0.0, 0.0],
            axis: [0.0, 2.0, 0.0],
            half_angle: std::f64::consts::FRAC_PI_4,
        };
        let result = plane_cone_intersect(&plane, &cone);
        assert!(result.is_some(), "intersection expected");
        let intersections = result.unwrap();
        assert_eq!(intersections.len(), 1, "one TwoLines");
        assert!(
            matches!(&intersections[0], SurfaceIntersection::TwoLines { .. }),
            "expected TwoLines"
        );
    }

    #[test]
    fn plane_cone_oblique_gives_ellipse() {
        // Oblique plane -> ellipse
        let plane = Surface::Plane {
            origin: [0.0, 1.0, 0.0],
            normal: vec3::normalized([0.0, 1.0, 0.3]),
        };
        let cone = Surface::Cone {
            origin: [0.0, 0.0, 0.0],
            axis: [0.0, 3.0, 0.0],
            half_angle: 0.3, // ≈ 17°
        };
        let result = plane_cone_intersect(&plane, &cone);
        assert!(result.is_some(), "intersection expected");
        let intersections = result.unwrap();
        assert!(!intersections.is_empty(), "intersection curves expected");
        // Should return ellipse
        assert!(
            matches!(&intersections[0], SurfaceIntersection::Ellipse { .. }),
            "expected Ellipse: {:?}",
            intersections[0]
        );
    }

    #[test]
    fn plane_cone_no_intersection() {
        // Plane outside cone range -> no intersection
        let plane = Surface::Plane {
            origin: [0.0, 5.0, 0.0],
            normal: [0.0, 1.0, 0.0],
        };
        let cone = Surface::Cone {
            origin: [0.0, 0.0, 0.0],
            axis: [0.0, 2.0, 0.0],
            half_angle: 0.3,
        };
        let result = plane_cone_intersect(&plane, &cone);
        // Plane outside cone height range -> None or empty
        match result {
            None => {} // OK
            Some(v) => assert!(v.is_empty(), "expected no intersection"),
        }
    }

    #[test]
    fn plane_cone_through_apex_axial() {
        // Plane through apex, normal parallel to axis -> degenerate
        let plane = Surface::Plane {
            origin: [0.0, 0.0, 0.0],
            normal: [0.0, 0.0, 1.0],
        };
        let cone = Surface::Cone {
            origin: [0.0, 0.0, 0.0],
            axis: [0.0, 0.0, 1.0],
            half_angle: std::f64::consts::FRAC_PI_4,
        };
        // Should not panic
        let _ = plane_cone_intersect(&plane, &cone);
    }

    #[test]
    fn plane_cone_sample_hyperbola() {
        // Steep plane -> hyperbola (sampling)
        let origin_p = [0.0, 0.5, 2.0];
        let normal_p = [0.0, 0.0, 1.0];
        let origin_c = [0.0, 0.0, 0.0];
        let axis_c = [0.0, 5.0, 0.0];
        let half_angle = 0.3;

        let polylines =
            plane_cone_sample_intersection(&origin_p, &normal_p, &origin_c, &axis_c, half_angle);
        // Plane z=2 cutting cone (half-angle 0.3)
        // Sampling should produce curves
        if !polylines.is_empty() {
            for polyline in &polylines {
                assert!(
                    polyline.len() >= 2,
                    "sampling curve should have >= 2 points"
                );
                // All points should be on the plane
                for p in polyline {
                    let d = (p[2] - 2.0).abs();
                    assert!(d < 0.1, "on plane: z={}, expected=2.0", p[2]);
                }
            }
        }
    }

    #[test]
    fn split_face_nurbs_surface_line_cut() {
        // Bilinear NurbsSurface at y=0, split by line; verify centroid normal projection works
        let surf_data = NurbsSurface3D {
            degree_u: 1,
            degree_v: 1,
            control_points: vec![
                vec![[0.0, 0.0, 0.0], [0.0, 0.0, 1.0]],
                vec![[1.0, 0.0, 0.0], [1.0, 0.0, 1.0]],
            ],
            weights: vec![vec![1.0, 1.0], vec![1.0, 1.0]],
            knots_u: vec![0.0, 0.0, 1.0, 1.0],
            knots_v: vec![0.0, 0.0, 1.0, 1.0],
        };
        let surface = Surface::NurbsSurface {
            data: Box::new(surf_data),
        };

        // Square polygon [0,1]x[0,1] at y=0
        let polygon = vec![
            [0.0, 0.0, 0.0],
            [1.0, 0.0, 0.0],
            [1.0, 0.0, 1.0],
            [0.0, 0.0, 1.0],
        ];

        // Cut vertically at x=0.5
        let cut = Curve3D::Line {
            start: [0.5, 0.0, -0.1],
            end: [0.5, 0.0, 1.1],
        };

        let _shell = Shell {
            vertices: vec![],
            edges: vec![],
            faces: vec![],
        };
        let _face = Face {
            surface: surface.clone(),
            loop_edges: vec![],
            orientation_reversed: false,
        };

        // Test split_polygon_by_curve directly
        let split = split_polygon_by_curve(&polygon, &cut, &surface);
        assert_eq!(
            split.len(),
            2,
            "expected open cut to split into two polygons"
        );
        let left = &split[0];
        let right = &split[1];
        assert!(
            left.len() >= 3,
            "left polygon should be non-empty: len={}",
            left.len()
        );
        assert!(
            right.len() >= 3,
            "right polygon should be non-empty: len={}",
            right.len()
        );
        // Left side contains vertices with x < 0.5
        let max_x_left = left.iter().map(|p| p[0]).fold(f64::NEG_INFINITY, f64::max);
        assert!(
            max_x_left <= 0.5 + 1e-9,
            "left max x should be <= 0.5: {max_x_left}"
        );
        // Right side contains vertices with x > 0.5
        let min_x_right = right.iter().map(|p| p[0]).fold(f64::INFINITY, f64::min);
        assert!(
            min_x_right >= 0.5 - 1e-9,
            "right min x should be >= 0.5: {min_x_right}"
        );
    }

    #[test]
    fn split_polygon_by_closed_curve_uv_plane_creates_annulus_fan() {
        let surface = Surface::Plane {
            origin: [0.0, 0.0, 0.0],
            normal: [0.0, 0.0, 1.0],
        };
        let polygon = vec![
            [-1.0, -1.0, 0.0],
            [1.0, -1.0, 0.0],
            [1.0, 1.0, 0.0],
            [-1.0, 1.0, 0.0],
        ];
        let cut = Curve3D::Ellipse {
            center: [0.0, 0.0, 0.0],
            axis_u: [0.45, 0.0, 0.0],
            axis_v: [0.0, 0.35, 0.0],
            t_start: 0.0,
            t_end: std::f64::consts::TAU,
        };

        let split = split_polygon_by_curve(&polygon, &cut, &surface);
        assert!(
            split.len() >= 2,
            "closed plane cut should decompose annulus into multiple polygons"
        );
        for poly in &split {
            assert!(
                poly.len() >= 4,
                "fan polygon must keep outer and inner boundary samples"
            );
            let area = polygon_area_2d(&polygon_to_uv(&surface, poly).unwrap());
            assert!(area.abs() > 1e-8, "fan polygon must have non-zero area");
        }
    }

    #[test]
    fn support_plane_split_enables_cylinder_ellipse_case() {
        let surface = Surface::Cylinder {
            origin: [0.0, 0.0, 0.0],
            axis: [0.0, 2.0, 0.0],
            radius: 1.0,
        };
        let polygon = vec![
            [(-1.0_f64).cos(), 0.0, (-1.0_f64).sin()],
            [1.0_f64.cos(), 0.0, 1.0_f64.sin()],
            [1.0_f64.cos(), 2.0, 1.0_f64.sin()],
            [(-1.0_f64).cos(), 2.0, (-1.0_f64).sin()],
        ];
        let cut = match plane_cylinder_intersect(
            &Surface::Plane {
                origin: [0.0, 1.0, 0.0],
                normal: vec3::normalized([0.6, 1.0, 0.0]),
            },
            &surface,
        ) {
            Some(SurfaceIntersection::Ellipse {
                center,
                axis_u,
                axis_v,
            }) => Curve3D::Ellipse {
                center,
                axis_u,
                axis_v,
                t_start: 0.0,
                t_end: std::f64::consts::TAU,
            },
            other => panic!("expected ellipse intersection, got {other:?}"),
        };

        let split = split_polygon_by_support_plane(&polygon, &cut, &surface);
        assert!(
            split.is_some(),
            "cylinder ellipse case should use support-plane split"
        );
    }

    #[test]
    fn support_plane_split_enables_cone_arc_case() {
        let surface = Surface::Cone {
            origin: [0.0, 0.0, 0.0],
            axis: [0.0, 3.0, 0.0],
            half_angle: 0.35,
        };
        let tan_a = 0.35_f64.tan();
        let radius_lo = 1.0 * tan_a;
        let radius_hi = 2.0 * tan_a;
        let polygon = vec![
            [
                radius_lo * (-0.9_f64).cos(),
                1.0,
                radius_lo * (-0.9_f64).sin(),
            ],
            [radius_lo * 0.9_f64.cos(), 1.0, radius_lo * 0.9_f64.sin()],
            [radius_hi * 0.9_f64.cos(), 2.0, radius_hi * 0.9_f64.sin()],
            [
                radius_hi * (-0.9_f64).cos(),
                2.0,
                radius_hi * (-0.9_f64).sin(),
            ],
        ];
        let cut = Curve3D::Arc {
            center: [0.0, 1.5, 0.0],
            axis: [0.0, 1.0, 0.0],
            start: [1.5 * tan_a, 1.5, 0.0],
            end: [1.5 * tan_a * 0.8, 1.5, 1.5 * tan_a * 0.6],
            radius: 1.5 * tan_a,
        };

        let split = split_polygon_by_support_plane(&polygon, &cut, &surface);
        assert!(
            split.is_some(),
            "cone arc case should use support-plane split"
        );
    }

    #[test]
    fn face_boundary_uv_polyline_box_face() {
        let shell = crate::primitives::shell_from_box(2.0, 2.0, 2.0);
        let uv = face_boundary_uv_polyline(&shell.faces[0], &shell, 1).unwrap();
        assert!(uv.len() >= 4);
        for p in uv {
            assert!(p[0].is_finite());
            assert!(p[1].is_finite());
        }
    }

    #[test]
    fn face_boundary_uv_polyline_sphere_face() {
        let shell = crate::primitives::shell_from_sphere(1.0);
        let uv = face_boundary_uv_polyline(&shell.faces[0], &shell, 4).unwrap();
        assert!(uv.len() >= 9);
        for p in uv {
            assert!(p[0].is_finite());
            assert!(p[1].is_finite());
        }
    }
}
