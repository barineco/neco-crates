use std::collections::{BTreeMap, BTreeSet};
use std::f64::consts::TAU;

use neco_nurbs::fit_nurbs_curve;

use crate::boolean3d::classify3d::{
    point_in_face_polygon, point_in_shell, Location3D, OverlapClass,
};
use crate::boolean3d::intersect3d::project_to_nurbs;
use crate::brep::{find_closest_v_on_profile, Curve3D, EdgeRef, Face, Shell, SubFace, Surface};
use crate::types::BooleanOp;
use crate::vec3;

use super::tolerance::BOUNDARY_TOL;

// ─── SurfaceOfRevolution inverse projection ────────────────────

/// Inverse projection onto SurfaceOfRevolution.
/// Returns (theta, v, nearest_point) for the closest point on the surface.
#[allow(clippy::too_many_arguments)]
fn project_to_revolution(
    center: &[f64; 3],
    axis: &[f64; 3],
    frame_u: &[f64; 3],
    frame_v: &[f64; 3],
    profile_control_points: &[[f64; 2]],
    profile_weights: &[f64],
    profile_degree: u32,
    n_profile_spans: u32,
    theta_start: f64,
    theta_range: f64,
    surface: &Surface,
    p: &[f64; 3],
) -> (f64, f64, [f64; 3]) {
    let axis_n = vec3::normalized(*axis);
    let bu = vec3::normalized(*frame_u);
    let bv = vec3::normalized(*frame_v);

    // Compute theta
    let q = vec3::sub(*p, *center);
    let dot_u = vec3::dot(q, bu);
    let dot_v = vec3::dot(q, bv);
    let theta_raw = dot_v.atan2(dot_u);

    // Normalize theta_raw into [theta_start, theta_start + theta_range]
    let theta = if theta_range >= TAU - 1e-12 {
        // Full revolution: normalize relative to theta_start
        let offset = (theta_raw - theta_start).rem_euclid(TAU);
        theta_start + offset
    } else {
        // Partial revolution: clamp to nearest end if beyond range
        let offset = (theta_raw - theta_start).rem_euclid(TAU);
        if offset <= theta_range {
            theta_start + offset
        } else {
            // Clamp to nearest end
            let dist_to_start = offset.min(TAU - offset);
            let dist_to_end = (offset - theta_range).min(TAU - offset + theta_range);
            if dist_to_start <= dist_to_end {
                theta_start
            } else {
                theta_start + theta_range
            }
        }
    };

    // Project p onto meridional plane: (r_target, z_target)
    let cos_t = theta.cos();
    let sin_t = theta.sin();
    let radial_dir = vec3::add(vec3::scale(bu, cos_t), vec3::scale(bv, sin_t));
    let r_target = vec3::dot(q, radial_dir);
    let z_target = vec3::dot(q, axis_n);

    let best_v = find_closest_v_on_profile(
        r_target,
        z_target,
        profile_control_points,
        profile_weights,
        profile_degree,
        n_profile_spans,
    );

    let nearest = surface.evaluate(theta, best_v);
    (theta, best_v, nearest)
}

// ─── Surface distance / normal helpers ────────────────────────

/// Unsigned distance from a point to a surface.
pub(crate) fn surface_distance_to(surface: &Surface, point: &[f64; 3]) -> Option<f64> {
    match surface {
        Surface::Plane { origin, normal } => {
            let d = vec3::sub(*point, *origin);
            Some(vec3::dot(*normal, d).abs())
        }
        Surface::Cylinder {
            origin,
            axis,
            radius,
        } => {
            let a = vec3::normalized(*axis);
            let q = vec3::sub(*point, *origin);
            let along = vec3::dot(q, a);
            let q_perp = vec3::sub(q, vec3::scale(a, along));
            Some((vec3::length(q_perp) - radius).abs())
        }
        Surface::Cone {
            origin,
            axis,
            half_angle,
        } => {
            let a = vec3::normalized(*axis);
            let q = vec3::sub(*point, *origin);
            let s = vec3::dot(q, a);
            let q_perp = vec3::sub(q, vec3::scale(a, s));
            let r_perp = vec3::length(q_perp);
            let expected_r = s * half_angle.tan();
            Some((r_perp - expected_r).abs())
        }
        Surface::Sphere { center, radius } => {
            let d = vec3::sub(*point, *center);
            Some((vec3::length(d) - radius).abs())
        }
        Surface::Ellipsoid { center, rx, ry, rz } => {
            let dx = (point[0] - center[0]) / rx;
            let dy = (point[1] - center[1]) / ry;
            let dz = (point[2] - center[2]) / rz;
            let r_max = rx.max(*ry).max(*rz);
            Some(((dx * dx + dy * dy + dz * dz).sqrt() - 1.0).abs() * r_max)
        }
        Surface::Torus {
            center,
            axis,
            major_radius,
            minor_radius,
        } => {
            let q = vec3::sub(*point, *center);
            let s = vec3::dot(q, *axis);
            let q_perp = vec3::sub(q, vec3::scale(*axis, s));
            let r_perp = vec3::length(q_perp);
            // Distance from tube center minus minor_radius
            let dx = r_perp - major_radius;
            let dist_to_tube_center = (dx * dx + s * s).sqrt();
            Some((dist_to_tube_center - minor_radius).abs())
        }
        Surface::SurfaceOfRevolution {
            center,
            axis,
            frame_u,
            frame_v,
            profile_control_points,
            profile_weights,
            profile_degree,
            n_profile_spans,
            theta_start,
            theta_range,
        } => {
            let (_theta, _v, nearest) = project_to_revolution(
                center,
                axis,
                frame_u,
                frame_v,
                profile_control_points,
                profile_weights,
                *profile_degree,
                *n_profile_spans,
                *theta_start,
                *theta_range,
                surface,
                point,
            );
            Some(vec3::length(vec3::sub(*point, nearest)))
        }
        Surface::SurfaceOfSweep { .. } => {
            let ns = surface.to_nurbs_surface()?;
            let (u, v) = project_to_nurbs(&ns, point);
            let closest = ns.evaluate(u, v);
            let diff = vec3::sub(*point, closest);
            Some(vec3::length(diff))
        }
        Surface::NurbsSurface { data } => {
            let (u, v) = project_to_nurbs(data, point);
            let closest = data.evaluate(u, v);
            let diff = vec3::sub(*point, closest);
            Some(vec3::length(diff))
        }
    }
}

/// Outward normal at a point on (or near) the surface.
pub(crate) fn surface_normal_at(surface: &Surface, point: &[f64; 3]) -> Option<[f64; 3]> {
    match surface {
        Surface::Plane { normal, .. } => Some(*normal),
        Surface::Cylinder { origin, axis, .. } => {
            let a = vec3::normalized(*axis);
            let q = vec3::sub(*point, *origin);
            let along = vec3::dot(q, a);
            let q_perp = vec3::sub(q, vec3::scale(a, along));
            let len = vec3::length(q_perp);
            if len < 1e-30 {
                return None;
            }
            Some(vec3::scale(q_perp, 1.0 / len))
        }
        Surface::Cone {
            origin,
            axis,
            half_angle,
        } => {
            let a = vec3::normalized(*axis);
            let q = vec3::sub(*point, *origin);
            let along = vec3::dot(q, a);
            let q_perp = vec3::sub(q, vec3::scale(a, along));
            let len = vec3::length(q_perp);
            if len < 1e-30 {
                return None;
            }
            let cos_a = half_angle.cos();
            let sin_a = half_angle.sin();
            let radial = vec3::scale(q_perp, 1.0 / len);
            // normal = cos(a)*radial - sin(a)*axis
            let n = vec3::sub(vec3::scale(radial, cos_a), vec3::scale(a, sin_a));
            let n_len = vec3::length(n);
            if n_len < 1e-30 {
                return None;
            }
            Some(vec3::scale(n, 1.0 / n_len))
        }
        Surface::Sphere { center, .. } => {
            let d = vec3::sub(*point, *center);
            let len = vec3::length(d);
            if len < 1e-30 {
                return None;
            }
            Some(vec3::scale(d, 1.0 / len))
        }
        Surface::Ellipsoid { center, rx, ry, rz } => {
            let nx = (point[0] - center[0]) / (rx * rx);
            let ny = (point[1] - center[1]) / (ry * ry);
            let nz = (point[2] - center[2]) / (rz * rz);
            let len = (nx * nx + ny * ny + nz * nz).sqrt();
            if len < 1e-30 {
                return None;
            }
            Some([nx / len, ny / len, nz / len])
        }
        Surface::Torus {
            center,
            axis,
            major_radius,
            ..
        } => {
            let q = vec3::sub(*point, *center);
            let s = vec3::dot(q, *axis);
            let q_perp = vec3::sub(q, vec3::scale(*axis, s));
            let r_perp = vec3::length(q_perp);
            if r_perp < 1e-30 {
                return None;
            }
            // tube_center: closest point on the major circle
            let tube_center = vec3::add(*center, vec3::scale(q_perp, *major_radius / r_perp));
            let n = vec3::sub(*point, tube_center);
            let n_len = vec3::length(n);
            if n_len < 1e-30 {
                return None;
            }
            Some(vec3::scale(n, 1.0 / n_len))
        }
        Surface::SurfaceOfRevolution {
            center,
            axis,
            frame_u,
            frame_v,
            profile_control_points,
            profile_weights,
            profile_degree,
            n_profile_spans,
            theta_start,
            theta_range,
        } => {
            let (theta, v, _nearest) = project_to_revolution(
                center,
                axis,
                frame_u,
                frame_v,
                profile_control_points,
                profile_weights,
                *profile_degree,
                *n_profile_spans,
                *theta_start,
                *theta_range,
                surface,
                point,
            );
            Some(surface.normal_at(theta, v))
        }
        Surface::SurfaceOfSweep { .. } => {
            let ns = surface.to_nurbs_surface()?;
            let (u, v) = project_to_nurbs(&ns, point);
            Some(ns.normal(u, v))
        }
        Surface::NurbsSurface { data } => {
            let (u, v) = project_to_nurbs(data, point);
            Some(data.normal(u, v))
        }
    }
}

// ─── classify_subface ───────────────────────────────────────

/// Classify a SubFace centroid against the other shell
fn classify_subface(sf: &SubFace, other_shell: &Shell) -> Location3D {
    let c = find_interior_point(&sf.polygon, &sf.surface);
    let inward_probe = surface_normal_at(&sf.surface, &c)
        .map(|n| vec3::sub(c, vec3::scale(n, BOUNDARY_TOL * 4.0)))
        .unwrap_or(c);

    // Boundary check: is the centroid on any face of the other shell?
    for face in &other_shell.faces {
        let dist = match surface_distance_to(&face.surface, &c) {
            Some(d) => d,
            None => continue,
        };
        if dist < BOUNDARY_TOL && point_in_face_polygon(&c, face, other_shell) {
            let face_normal = match surface_normal_at(&face.surface, &c) {
                Some(n) => n,
                None => continue,
            };
            let sf_normal = match surface_normal_at(&sf.surface, &c) {
                Some(n) => n,
                None => continue,
            };
            let dot = vec3::dot(sf_normal, face_normal);
            if dot > 0.0 {
                return Location3D::Boundary(OverlapClass::SameDirection);
            } else {
                return Location3D::Boundary(OverlapClass::OppositeDirection);
            }
        }
    }

    if point_in_shell(&inward_probe, other_shell) {
        Location3D::Inside
    } else {
        Location3D::Outside
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum BoundaryKind {
    Duplicate,
    PartialOverlap,
    Contact,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SelectionRelation {
    Inside,
    Outside,
    Boundary(OverlapClass, BoundaryKind),
}

fn find_interior_point(polygon: &[[f64; 3]], surface: &Surface) -> [f64; 3] {
    let centroid = project_point_to_surface(surface, &polygon_centroid(polygon));
    if point_in_polygon_on_surface(&centroid, polygon, surface) {
        return centroid;
    }

    for &vertex in polygon {
        for fraction in [0.25, 0.5, 0.75] {
            let candidate = vec3::add(vertex, vec3::scale(vec3::sub(centroid, vertex), fraction));
            let candidate = project_point_to_surface(surface, &candidate);
            if point_in_polygon_on_surface(&candidate, polygon, surface) {
                return candidate;
            }
        }
    }

    if polygon.len() >= 3 {
        let root = polygon[0];
        for i in 1..(polygon.len() - 1) {
            let candidate = project_point_to_surface(
                surface,
                &triangle_centroid(root, polygon[i], polygon[i + 1]),
            );
            if point_in_polygon_on_surface(&candidate, polygon, surface) {
                return candidate;
            }

            let candidate =
                project_point_to_surface(surface, &triangle_centroid(root, polygon[i], centroid));
            if point_in_polygon_on_surface(&candidate, polygon, surface) {
                return candidate;
            }
        }
    }

    centroid
}

fn project_point_to_surface(surface: &Surface, point: &[f64; 3]) -> [f64; 3] {
    surface
        .inverse_project(point)
        .map(|(u, v)| surface.evaluate(u, v))
        .unwrap_or(*point)
}

fn point_in_polygon_on_surface(point: &[f64; 3], polygon: &[[f64; 3]], surface: &Surface) -> bool {
    if polygon.len() < 3 {
        return false;
    }

    let normal = surface_normal_at(surface, point).unwrap_or_else(|| polygon_normal(polygon));
    let axis = dominant_axis(normal);
    let point_2d = project_point_2d(point, axis);
    let polygon_2d: Vec<[f64; 2]> = polygon
        .iter()
        .map(|vertex| project_point_2d(vertex, axis))
        .collect();
    point_in_polygon_2d(point_2d, &polygon_2d)
}

fn dominant_axis(normal: [f64; 3]) -> usize {
    let ax = normal[0].abs();
    let ay = normal[1].abs();
    let az = normal[2].abs();
    if ax >= ay && ax >= az {
        0
    } else if ay >= az {
        1
    } else {
        2
    }
}

fn project_point_2d(point: &[f64; 3], axis: usize) -> [f64; 2] {
    match axis {
        0 => [point[1], point[2]],
        1 => [point[0], point[2]],
        _ => [point[0], point[1]],
    }
}

fn point_in_polygon_2d(point: [f64; 2], polygon: &[[f64; 2]]) -> bool {
    let mut inside = false;
    let n = polygon.len();
    if n < 3 {
        return false;
    }

    let mut j = n - 1;
    for i in 0..n {
        let a = polygon[i];
        let b = polygon[j];
        if point_on_segment_2d(point, a, b) {
            return true;
        }
        let intersects = ((a[1] > point[1]) != (b[1] > point[1]))
            && (point[0] < (b[0] - a[0]) * (point[1] - a[1]) / (b[1] - a[1]) + a[0]);
        if intersects {
            inside = !inside;
        }
        j = i;
    }

    inside
}

fn point_on_segment_2d(point: [f64; 2], a: [f64; 2], b: [f64; 2]) -> bool {
    let ab = [b[0] - a[0], b[1] - a[1]];
    let ap = [point[0] - a[0], point[1] - a[1]];
    let cross = ab[0] * ap[1] - ab[1] * ap[0];
    if cross.abs() > 1e-10 {
        return false;
    }
    let dot = ap[0] * ab[0] + ap[1] * ab[1];
    if dot < -1e-10 {
        return false;
    }
    let len_sq = ab[0] * ab[0] + ab[1] * ab[1];
    dot <= len_sq + 1e-10
}

fn polygon_centroid(polygon: &[[f64; 3]]) -> [f64; 3] {
    let sum = polygon.iter().copied().fold([0.0, 0.0, 0.0], vec3::add);
    vec3::scale(sum, 1.0 / polygon.len() as f64)
}

fn triangle_centroid(a: [f64; 3], b: [f64; 3], c: [f64; 3]) -> [f64; 3] {
    vec3::scale(vec3::add(vec3::add(a, b), c), 1.0 / 3.0)
}

/// Select SubFaces according to the boolean operation type.
pub fn select_faces(
    sub_a: &[SubFace],
    sub_b: &[SubFace],
    shell_a: &Shell,
    shell_b: &Shell,
    op: BooleanOp,
) -> Vec<SubFace> {
    let mut result = Vec::new();

    for sf in sub_a {
        let relation = classify_selection_relation(sf, sub_b, shell_b);
        let keep = match op {
            BooleanOp::Union => matches!(
                relation,
                SelectionRelation::Outside
                    | SelectionRelation::Boundary(OverlapClass::SameDirection, _)
            ),
            BooleanOp::Subtract => matches!(
                relation,
                SelectionRelation::Outside
                    | SelectionRelation::Boundary(OverlapClass::OppositeDirection, _)
            ),
            BooleanOp::Intersect => matches!(
                relation,
                SelectionRelation::Inside
                    | SelectionRelation::Boundary(OverlapClass::SameDirection, _)
            ),
        };
        if keep {
            result.push(sf.clone());
        }
    }

    for sf in sub_b {
        let relation = classify_selection_relation(sf, sub_a, shell_a);
        let keep = match op {
            BooleanOp::Union => matches!(
                relation,
                SelectionRelation::Outside
                    | SelectionRelation::Boundary(
                        OverlapClass::SameDirection,
                        BoundaryKind::Contact
                    )
            ),
            BooleanOp::Subtract => {
                if relation == SelectionRelation::Inside {
                    // B inner faces are flipped and added
                    let mut flipped = sf.clone();
                    flip_subface(&mut flipped);
                    result.push(flipped);
                    continue;
                }
                false
            }
            BooleanOp::Intersect => matches!(
                relation,
                SelectionRelation::Inside
                    | SelectionRelation::Boundary(
                        OverlapClass::SameDirection,
                        BoundaryKind::Contact
                    )
            ),
        };
        if keep {
            result.push(sf.clone());
        }
    }

    result
}

fn classify_selection_relation(
    sf: &SubFace,
    other_subfaces: &[SubFace],
    other_shell: &Shell,
) -> SelectionRelation {
    match classify_subface(sf, other_shell) {
        Location3D::Inside => SelectionRelation::Inside,
        Location3D::Outside => SelectionRelation::Outside,
        Location3D::Boundary(overlap) => SelectionRelation::Boundary(
            overlap,
            classify_boundary_kind(sf, other_subfaces, overlap),
        ),
    }
}

fn classify_boundary_kind(
    sf: &SubFace,
    other_subfaces: &[SubFace],
    _overlap: OverlapClass,
) -> BoundaryKind {
    if has_duplicate_counterpart(sf, other_subfaces) {
        BoundaryKind::Duplicate
    } else if has_partial_overlap_counterpart(sf, other_subfaces) {
        BoundaryKind::PartialOverlap
    } else {
        BoundaryKind::Contact
    }
}

pub(crate) fn has_substantial_selection_overlap(
    sub_a: &[SubFace],
    sub_b: &[SubFace],
    shell_a: &Shell,
    shell_b: &Shell,
) -> bool {
    sub_a.iter().any(|sf| {
        matches!(
            classify_selection_relation(sf, sub_b, shell_b),
            SelectionRelation::Inside
                | SelectionRelation::Boundary(
                    _,
                    BoundaryKind::Duplicate | BoundaryKind::PartialOverlap
                )
        )
    }) || sub_b.iter().any(|sf| {
        matches!(
            classify_selection_relation(sf, sub_a, shell_a),
            SelectionRelation::Inside
                | SelectionRelation::Boundary(
                    _,
                    BoundaryKind::Duplicate | BoundaryKind::PartialOverlap
                )
        )
    })
}

fn has_duplicate_counterpart(sf: &SubFace, other_subfaces: &[SubFace]) -> bool {
    let Some(sig) = subface_boundary_signature(sf) else {
        return false;
    };
    other_subfaces
        .iter()
        .filter_map(subface_boundary_signature)
        .any(|other_sig| other_sig == sig)
}

fn has_partial_overlap_counterpart(sf: &SubFace, other_subfaces: &[SubFace]) -> bool {
    let surface_sig = subface_surface_signature(sf);
    other_subfaces.iter().any(|other| {
        subface_surface_signature(other) == surface_sig
            && polygons_overlap_with_area(&sf.polygon, &other.polygon)
    })
}

fn polygons_overlap_with_area(polygon_a: &[[f64; 3]], polygon_b: &[[f64; 3]]) -> bool {
    if polygon_a.len() < 3 || polygon_b.len() < 3 {
        return false;
    }

    let normal = polygon_normal(polygon_a);
    let axis = dominant_axis(normal);
    let poly_a: Vec<[f64; 2]> = polygon_a
        .iter()
        .map(|vertex| project_point_2d(vertex, axis))
        .collect();
    let poly_b: Vec<[f64; 2]> = polygon_b
        .iter()
        .map(|vertex| project_point_2d(vertex, axis))
        .collect();

    poly_a
        .iter()
        .any(|&p| point_strictly_in_polygon_2d(p, &poly_b))
        || poly_b
            .iter()
            .any(|&p| point_strictly_in_polygon_2d(p, &poly_a))
        || polygon_edges_2d(&poly_a).iter().any(|&(a0, a1)| {
            polygon_edges_2d(&poly_b)
                .iter()
                .any(|&(b0, b1)| segments_overlap_with_area_2d(a0, a1, b0, b1))
        })
}

fn polygon_edges_2d(polygon: &[[f64; 2]]) -> Vec<([f64; 2], [f64; 2])> {
    (0..polygon.len())
        .map(|i| (polygon[i], polygon[(i + 1) % polygon.len()]))
        .collect()
}

fn point_strictly_in_polygon_2d(point: [f64; 2], polygon: &[[f64; 2]]) -> bool {
    if polygon.len() < 3 {
        return false;
    }
    let mut inside = false;
    let mut j = polygon.len() - 1;
    for i in 0..polygon.len() {
        let a = polygon[i];
        let b = polygon[j];
        if point_on_segment_2d(point, a, b) {
            return false;
        }
        let intersects = ((a[1] > point[1]) != (b[1] > point[1]))
            && (point[0] < (b[0] - a[0]) * (point[1] - a[1]) / (b[1] - a[1]) + a[0]);
        if intersects {
            inside = !inside;
        }
        j = i;
    }
    inside
}

fn segments_overlap_with_area_2d(a0: [f64; 2], a1: [f64; 2], b0: [f64; 2], b1: [f64; 2]) -> bool {
    let eps = 1e-10;
    let oa = orient_2d(a0, a1, b0);
    let ob = orient_2d(a0, a1, b1);
    let oc = orient_2d(b0, b1, a0);
    let od = orient_2d(b0, b1, a1);

    if oa.abs() < eps && ob.abs() < eps && oc.abs() < eps && od.abs() < eps {
        let axis = if (a1[0] - a0[0]).abs() >= (a1[1] - a0[1]).abs() {
            0
        } else {
            1
        };
        let (a_min, a_max) = if a0[axis] <= a1[axis] {
            (a0[axis], a1[axis])
        } else {
            (a1[axis], a0[axis])
        };
        let (b_min, b_max) = if b0[axis] <= b1[axis] {
            (b0[axis], b1[axis])
        } else {
            (b1[axis], b0[axis])
        };
        return a_max.min(b_max) - a_min.max(b_min) > eps;
    }

    oa * ob < -eps && oc * od < -eps
}

fn orient_2d(a: [f64; 2], b: [f64; 2], c: [f64; 2]) -> f64 {
    (b[0] - a[0]) * (c[1] - a[1]) - (b[1] - a[1]) * (c[0] - a[0])
}

fn quantize(v: f64) -> i64 {
    (v * 1_000_000.0).round() as i64
}

fn min_rotation(values: &[[i64; 2]]) -> Vec<[i64; 2]> {
    let n = values.len();
    let mut best = values.to_vec();
    for shift in 1..n {
        let rotated: Vec<[i64; 2]> = (0..n).map(|i| values[(i + shift) % n]).collect();
        if rotated < best {
            best = rotated;
        }
    }
    best
}

fn canonical_uv_ring(surface: &Surface, polygon: &[[f64; 3]]) -> Option<Vec<[i64; 2]>> {
    if polygon.len() < 3 {
        return None;
    }
    let uv: Vec<[i64; 2]> = polygon
        .iter()
        .map(|point| {
            surface
                .inverse_project(point)
                .map(|(u, v)| [quantize(u), quantize(v)])
        })
        .collect::<Option<Vec<_>>>()?;
    let forward = min_rotation(&uv);
    let mut reversed = uv;
    reversed.reverse();
    let reversed = min_rotation(&reversed);
    Some(if reversed < forward {
        reversed
    } else {
        forward
    })
}

fn subface_surface_signature(sf: &SubFace) -> Vec<i64> {
    match &sf.surface {
        Surface::Plane { origin, normal } => vec![
            0,
            quantize(origin[0]),
            quantize(origin[1]),
            quantize(origin[2]),
            quantize(normal[0]),
            quantize(normal[1]),
            quantize(normal[2]),
        ],
        Surface::Sphere { center, radius } => vec![
            1,
            quantize(center[0]),
            quantize(center[1]),
            quantize(center[2]),
            quantize(*radius),
        ],
        Surface::Ellipsoid { center, rx, ry, rz } => vec![
            2,
            quantize(center[0]),
            quantize(center[1]),
            quantize(center[2]),
            quantize(*rx),
            quantize(*ry),
            quantize(*rz),
        ],
        Surface::Cylinder {
            origin,
            axis,
            radius,
        } => vec![
            3,
            quantize(origin[0]),
            quantize(origin[1]),
            quantize(origin[2]),
            quantize(axis[0]),
            quantize(axis[1]),
            quantize(axis[2]),
            quantize(*radius),
        ],
        Surface::Cone {
            origin,
            axis,
            half_angle,
        } => vec![
            4,
            quantize(origin[0]),
            quantize(origin[1]),
            quantize(origin[2]),
            quantize(axis[0]),
            quantize(axis[1]),
            quantize(axis[2]),
            quantize(*half_angle),
        ],
        Surface::Torus {
            center,
            axis,
            major_radius,
            minor_radius,
        } => vec![
            5,
            quantize(center[0]),
            quantize(center[1]),
            quantize(center[2]),
            quantize(axis[0]),
            quantize(axis[1]),
            quantize(axis[2]),
            quantize(*major_radius),
            quantize(*minor_radius),
        ],
        Surface::SurfaceOfRevolution { center, axis, .. } => vec![
            6,
            quantize(center[0]),
            quantize(center[1]),
            quantize(center[2]),
            quantize(axis[0]),
            quantize(axis[1]),
            quantize(axis[2]),
        ],
        Surface::SurfaceOfSweep {
            spine_control_points,
            ..
        } => {
            let first = spine_control_points
                .first()
                .copied()
                .unwrap_or([0.0, 0.0, 0.0]);
            let last = spine_control_points
                .last()
                .copied()
                .unwrap_or([0.0, 0.0, 0.0]);
            vec![
                7,
                quantize(first[0]),
                quantize(first[1]),
                quantize(first[2]),
                quantize(last[0]),
                quantize(last[1]),
                quantize(last[2]),
            ]
        }
        Surface::NurbsSurface { .. } => vec![8],
    }
}

fn subface_boundary_signature(sf: &SubFace) -> Option<(Vec<i64>, Vec<[i64; 2]>)> {
    let ring = canonical_uv_ring(&sf.surface, &sf.polygon)?;
    Some((subface_surface_signature(sf), ring))
}

fn subface_signature(sf: &SubFace) -> Option<(Vec<i64>, Vec<[i64; 2]>, bool)> {
    let (surface_sig, ring) = subface_boundary_signature(sf)?;
    Some((surface_sig, ring, sf.flipped))
}

pub fn normalize_selected_subfaces(sub_faces: Vec<SubFace>) -> Vec<SubFace> {
    let mut seen = BTreeSet::new();
    let mut result = Vec::new();
    for sf in sub_faces {
        let Some(sig) = subface_signature(&sf) else {
            result.push(sf);
            continue;
        };
        if seen.insert(sig) {
            result.push(sf);
        }
    }
    let merged = merge_coplanar_plane_subfaces(result);
    dedup_coplanar_plane_loops(merged)
}

fn plane_group_key(sf: &SubFace) -> Option<(Vec<i64>, bool)> {
    let Surface::Plane { origin, normal } = &sf.surface else {
        return None;
    };
    let n = vec3::normalized(*normal);
    let d = vec3::dot(n, *origin);
    Some((
        vec![quantize(n[0]), quantize(n[1]), quantize(n[2]), quantize(d)],
        sf.flipped,
    ))
}

fn uv_key(p: [f64; 2]) -> [i64; 2] {
    [quantize(p[0]), quantize(p[1])]
}

fn simplify_uv_ring(poly: &[[f64; 2]]) -> Vec<[i64; 2]> {
    let mut pts: Vec<[i64; 2]> = poly.iter().copied().map(uv_key).collect();
    if pts.len() >= 2 && pts.first() == pts.last() {
        pts.pop();
    }

    let mut changed = true;
    while changed {
        changed = false;

        let mut deduped = Vec::with_capacity(pts.len());
        for p in pts {
            if deduped.last().copied() != Some(p) {
                deduped.push(p);
            } else {
                changed = true;
            }
        }
        if deduped.len() >= 2 && deduped.first() == deduped.last() {
            deduped.pop();
            changed = true;
        }
        pts = deduped;

        if pts.len() < 3 {
            break;
        }

        let mut reduced = Vec::with_capacity(pts.len());
        let n = pts.len();
        for i in 0..n {
            let prev = pts[(i + n - 1) % n];
            let curr = pts[i];
            let next = pts[(i + 1) % n];
            if prev == next {
                changed = true;
                continue;
            }
            reduced.push(curr);
        }
        pts = reduced;
    }

    pts
}

fn canonical_uv_loop(poly: &[[f64; 2]]) -> Vec<[i64; 2]> {
    let uv = simplify_uv_ring(poly);
    let forward = min_rotation(&uv);
    let mut reversed = uv;
    reversed.reverse();
    let reversed = min_rotation(&reversed);
    if reversed < forward {
        reversed
    } else {
        forward
    }
}

fn plane_group_reference_surface(group: &[SubFace]) -> Option<Surface> {
    let first = group.first()?;
    let Surface::Plane { origin, normal } = &first.surface else {
        return None;
    };
    let n = vec3::normalized(*normal);
    let d = vec3::dot(n, *origin);
    let ref_origin = vec3::scale(n, d);
    Some(Surface::Plane {
        origin: ref_origin,
        normal: n,
    })
}

fn dedup_plane_group_loops(group: &[SubFace]) -> Option<Vec<Vec<[f64; 2]>>> {
    let ref_surface = plane_group_reference_surface(group)?;
    let mut seen = BTreeSet::<Vec<[i64; 2]>>::new();
    let mut unique = Vec::new();
    for sf in group {
        if sf.polygon.len() < 3 {
            continue;
        }
        let uv_ring: Vec<[f64; 2]> = sf
            .polygon
            .iter()
            .map(|p| ref_surface.inverse_project(p).map(|(u, v)| [u, v]))
            .collect::<Option<Vec<_>>>()?;
        let key = canonical_uv_loop(&uv_ring);
        if seen.insert(key) {
            unique.push(uv_ring);
        }
    }
    Some(unique)
}

#[cfg(test)]
type QuantizedUvEdge = ([i64; 2], [i64; 2]);

#[cfg(test)]
fn plane_group_edge_multiplicities(group: &[SubFace]) -> Option<BTreeMap<QuantizedUvEdge, usize>> {
    let mut counts = BTreeMap::<QuantizedUvEdge, usize>::new();
    for uv_ring in dedup_plane_group_loops(group)? {
        for i in 0..uv_ring.len() {
            let a = uv_key(uv_ring[i]);
            let b = uv_key(uv_ring[(i + 1) % uv_ring.len()]);
            let key = if a <= b { (a, b) } else { (b, a) };
            *counts.entry(key).or_insert(0) += 1;
        }
    }
    Some(counts)
}

fn reconstruct_plane_union_loops(group: &[SubFace]) -> Option<Vec<Vec<[f64; 2]>>> {
    #[derive(Clone)]
    struct DirectedEdge {
        start: [f64; 2],
        end: [f64; 2],
    }

    let mut edges: BTreeMap<([i64; 2], [i64; 2]), DirectedEdge> = BTreeMap::new();

    for uv_ring in dedup_plane_group_loops(group)? {
        for i in 0..uv_ring.len() {
            let a = uv_ring[i];
            let b = uv_ring[(i + 1) % uv_ring.len()];
            let ab = (uv_key(a), uv_key(b));
            let ba = (uv_key(b), uv_key(a));
            if edges.remove(&ba).is_none() {
                edges.insert(ab, DirectedEdge { start: a, end: b });
            }
        }
    }

    if edges.is_empty() {
        return None;
    }

    let mut outgoing: BTreeMap<[i64; 2], Vec<DirectedEdge>> = BTreeMap::new();
    for edge in edges.into_values() {
        outgoing.entry(uv_key(edge.start)).or_default().push(edge);
    }

    let mut loops = Vec::new();
    while let Some((&start_key, _)) = outgoing.iter().next() {
        let first = outgoing.get_mut(&start_key)?.pop()?;
        if outgoing.get(&start_key).is_some_and(|v| v.is_empty()) {
            outgoing.remove(&start_key);
        }
        let mut loop_pts = vec![first.start];
        let mut current = first;
        let guard = outgoing.values().map(|v| v.len()).sum::<usize>() + 4;
        for _ in 0..guard {
            loop_pts.push(current.end);
            let next_key = uv_key(current.end);
            if next_key == start_key {
                break;
            }
            let next = outgoing.get_mut(&next_key)?.pop()?;
            if outgoing.get(&next_key).is_some_and(|v| v.is_empty()) {
                outgoing.remove(&next_key);
            }
            current = next;
        }
        if loop_pts.len() >= 4 {
            loop_pts.pop();
            loops.push(loop_pts);
        }
    }

    if loops.is_empty() {
        None
    } else {
        Some(loops)
    }
}

fn merge_coplanar_plane_subfaces(sub_faces: Vec<SubFace>) -> Vec<SubFace> {
    let mut grouped: BTreeMap<(Vec<i64>, bool), Vec<SubFace>> = BTreeMap::new();
    let mut others = Vec::new();

    for sf in sub_faces {
        if let Some(key) = plane_group_key(&sf) {
            grouped.entry(key).or_default().push(sf);
        } else {
            others.push(sf);
        }
    }

    let mut result = others;
    for (_key, group) in grouped {
        if group.len() <= 1 {
            result.extend(group);
            continue;
        }
        let Some(loops) = reconstruct_plane_union_loops(&group) else {
            result.extend(group);
            continue;
        };
        if loops.len() != 1 {
            result.extend(group);
            continue;
        }
        let template = &group[0];
        let loop_uv = &loops[0];
        let polygon: Vec<[f64; 3]> = loop_uv
            .iter()
            .map(|uv| template.surface.evaluate(uv[0], uv[1]))
            .collect();
        let mut candidate_curves = Vec::new();
        for sf in &group {
            for curve in &sf.candidate_curves {
                candidate_curves.push(curve.clone());
            }
        }
        result.push(SubFace {
            surface: template.surface.clone(),
            polygon,
            candidate_curves,
            flipped: template.flipped,
            source_shell: template.source_shell,
            source_face: template.source_face,
        });
    }

    result
}

fn dedup_coplanar_plane_loops(sub_faces: Vec<SubFace>) -> Vec<SubFace> {
    let mut seen = BTreeSet::<(Vec<i64>, bool, Vec<[i64; 2]>)>::new();
    let mut result = Vec::new();
    for sf in sub_faces {
        let Some((plane_key, flipped)) = plane_group_key(&sf) else {
            result.push(sf);
            continue;
        };
        let normal = [
            plane_key[0] as f64 / 1_000_000.0,
            plane_key[1] as f64 / 1_000_000.0,
            plane_key[2] as f64 / 1_000_000.0,
        ];
        let d = plane_key[3] as f64 / 1_000_000.0;
        let ref_surface = Surface::Plane {
            origin: vec3::scale(normal, d),
            normal,
        };
        let Some(uv_ring) = sf
            .polygon
            .iter()
            .map(|p| ref_surface.inverse_project(p).map(|(u, v)| [u, v]))
            .collect::<Option<Vec<_>>>()
        else {
            result.push(sf);
            continue;
        };
        let sig = (plane_key, flipped, canonical_uv_loop(&uv_ring));
        if seen.insert(sig) {
            result.push(sf);
        }
    }
    result
}

fn flip_subface(sf: &mut SubFace) {
    sf.polygon.reverse();
    sf.flipped = !sf.flipped;
    match &mut sf.surface {
        Surface::Plane { normal, .. } => *normal = vec3::scale(*normal, -1.0),
        Surface::Cylinder { .. }
        | Surface::Cone { .. }
        | Surface::Sphere { .. }
        | Surface::Ellipsoid { .. }
        | Surface::Torus { .. }
        | Surface::SurfaceOfRevolution { .. }
        | Surface::SurfaceOfSweep { .. }
        | Surface::NurbsSurface { .. } => {
            // Curved surface flip: polygon winding only (normal implicitly flips)
        }
    }
}

/// Build a Shell from SubFaces.
pub fn build_shell_from_subfaces(
    sub_faces: &[SubFace],
    shell_a: &Shell,
    shell_b: &Shell,
) -> Result<Shell, String> {
    if sub_faces.is_empty() {
        return Err("empty SubFace list".to_string());
    }

    let mut shell = Shell::new();

    for sf in sub_faces {
        let mut polygon = sf.polygon.clone();
        orient_polygon_for_surface(&mut polygon, &sf.surface, sf.flipped);
        let source_shell = if sf.source_shell == 0 {
            shell_a
        } else {
            shell_b
        };
        let source_face = &source_shell.faces[sf.source_face];

        let vids: Vec<usize> = polygon.iter().map(|p| shell.add_vertex(*p)).collect();
        let n = vids.len();

        let mut edge_refs = Vec::with_capacity(n);
        for i in 0..n {
            let j = (i + 1) % n;
            let curve = reconstruct_edge_curve(
                polygon[i],
                polygon[j],
                source_face,
                source_shell,
                &sf.candidate_curves,
            )
            .unwrap_or(Curve3D::Line {
                start: polygon[i],
                end: polygon[j],
            });
            let eid = shell.add_edge(vids[i], vids[j], curve);
            edge_refs.push(EdgeRef {
                edge_id: eid,
                forward: true,
            });
        }

        shell.faces.push(Face {
            loop_edges: edge_refs,
            surface: sf.surface.clone(),
            orientation_reversed: sf.flipped,
        });
    }

    Ok(shell)
}

fn orient_polygon_for_surface(polygon: &mut [[f64; 3]], surface: &Surface, flipped: bool) {
    if polygon.len() < 3 {
        return;
    }
    let centroid = vec3::scale(
        polygon.iter().copied().fold([0.0, 0.0, 0.0], vec3::add),
        1.0 / polygon.len() as f64,
    );
    let sample_point = surface
        .inverse_project(&centroid)
        .map(|(u, v)| surface.evaluate(u, v))
        .unwrap_or(centroid);
    let Some(surface_normal) = surface_normal_at(surface, &sample_point) else {
        return;
    };
    let polygon_normal = polygon_normal(polygon);
    let target_normal = if flipped {
        vec3::scale(surface_normal, -1.0)
    } else {
        surface_normal
    };
    if vec3::dot(polygon_normal, target_normal) < 0.0 {
        polygon.reverse();
    }
}

fn polygon_normal(polygon: &[[f64; 3]]) -> [f64; 3] {
    let mut normal = [0.0, 0.0, 0.0];
    for i in 0..polygon.len() {
        let a = polygon[i];
        let b = polygon[(i + 1) % polygon.len()];
        normal[0] += (a[1] - b[1]) * (a[2] + b[2]);
        normal[1] += (a[2] - b[2]) * (a[0] + b[0]);
        normal[2] += (a[0] - b[0]) * (a[1] + b[1]);
    }
    vec3::normalized(normal)
}

#[derive(Debug)]
struct CurveMatch {
    curve: Curve3D,
    surface_error: f64,
    orientation_penalty: f64,
    deviation_error: f64,
    endpoint_error: f64,
    source_priority: u8,
}

fn reconstruct_edge_curve(
    start: [f64; 3],
    end: [f64; 3],
    source_face: &Face,
    source_shell: &Shell,
    candidate_curves: &[Curve3D],
) -> Option<Curve3D> {
    let mut best_match = None;
    for edge_ref in &source_face.loop_edges {
        let curve = &source_shell.edges[edge_ref.edge_id].curve;
        consider_curve_match(&mut best_match, curve, start, end, &source_face.surface, 0);
    }
    for curve in candidate_curves {
        consider_curve_match(&mut best_match, curve, start, end, &source_face.surface, 1);
    }
    best_match.map(|matched| matched.curve)
}

fn consider_curve_match(
    best_match: &mut Option<CurveMatch>,
    curve: &Curve3D,
    start: [f64; 3],
    end: [f64; 3],
    surface: &Surface,
    source_priority: u8,
) {
    let Some(matched) = match_curve_to_edge(curve, start, end, surface, source_priority) else {
        return;
    };
    if best_match
        .as_ref()
        .is_none_or(|current| is_better_curve_match(&matched, current))
    {
        *best_match = Some(matched);
    }
}

fn match_curve_to_edge(
    curve: &Curve3D,
    start: [f64; 3],
    end: [f64; 3],
    surface: &Surface,
    source_priority: u8,
) -> Option<CurveMatch> {
    let clipped = clip_curve_between_points_with_mode(curve, start, end, source_priority != 0)?;
    Some(CurveMatch {
        surface_error: curve_surface_error(surface, &clipped.curve),
        orientation_penalty: curve_orientation_penalty(&clipped.curve, start, end),
        deviation_error: curve_segment_deviation_error(
            curve,
            clipped.t_start,
            clipped.t_end,
            &clipped.curve,
        ),
        endpoint_error: curve_endpoint_error(curve, clipped.t_start, clipped.t_end, &clipped.curve),
        curve: clipped.curve,
        source_priority,
    })
}

fn is_better_curve_match(candidate: &CurveMatch, current: &CurveMatch) -> bool {
    compare_metric(candidate.surface_error, current.surface_error, 1e-7)
        .or_else(|| {
            compare_metric(
                candidate.orientation_penalty,
                current.orientation_penalty,
                1e-7,
            )
        })
        .or_else(|| compare_metric(candidate.deviation_error, current.deviation_error, 1e-7))
        .or_else(|| compare_metric(candidate.endpoint_error, current.endpoint_error, 1e-7))
        .or_else(|| compare_curve_rank(&candidate.curve, &current.curve))
        .unwrap_or(candidate.source_priority < current.source_priority)
}

fn compare_curve_rank(candidate: &Curve3D, current: &Curve3D) -> Option<bool> {
    let rank = |curve: &Curve3D| match curve {
        Curve3D::Line { .. } => 1u8,
        Curve3D::Arc { .. } | Curve3D::Ellipse { .. } | Curve3D::NurbsCurve3D { .. } => 0u8,
    };
    match rank(candidate).cmp(&rank(current)) {
        std::cmp::Ordering::Less => Some(true),
        std::cmp::Ordering::Greater => Some(false),
        std::cmp::Ordering::Equal => None,
    }
}

fn compare_metric(lhs: f64, rhs: f64, tol: f64) -> Option<bool> {
    if lhs + tol < rhs {
        Some(true)
    } else if rhs + tol < lhs {
        Some(false)
    } else {
        None
    }
}

struct ClippedCurve {
    curve: Curve3D,
    t_start: f64,
    t_end: f64,
}

#[cfg(test)]
fn clip_curve_between_points(
    curve: &Curve3D,
    start: [f64; 3],
    end: [f64; 3],
) -> Option<ClippedCurve> {
    clip_curve_between_points_with_mode(curve, start, end, true)
}

fn clip_curve_between_points_with_mode(
    curve: &Curve3D,
    start: [f64; 3],
    end: [f64; 3],
    allow_projection: bool,
) -> Option<ClippedCurve> {
    let t0 = point_parameter_on_curve(curve, &start, None, allow_projection)?;
    let t1 = point_parameter_on_curve(curve, &end, Some(t0), allow_projection)?;
    let curve_start = curve.evaluate(t0);
    let curve_end = curve.evaluate(t1);
    match curve {
        Curve3D::Line { .. } => Some(ClippedCurve {
            curve: Curve3D::Line { start, end },
            t_start: t0,
            t_end: t1,
        }),
        Curve3D::Arc {
            center,
            axis,
            radius,
            ..
        } => Some(ClippedCurve {
            curve: Curve3D::Arc {
                center: *center,
                axis: if t1 >= t0 {
                    *axis
                } else {
                    vec3::scale(*axis, -1.0)
                },
                start: curve_start,
                end: curve_end,
                radius: *radius,
            },
            t_start: t0,
            t_end: t1,
        }),
        Curve3D::Ellipse {
            center,
            axis_u,
            axis_v,
            ..
        } => Some(ClippedCurve {
            curve: Curve3D::Ellipse {
                center: *center,
                axis_u: *axis_u,
                axis_v: *axis_v,
                t_start: t0,
                t_end: t1,
            },
            t_start: t0,
            t_end: t1,
        }),
        Curve3D::NurbsCurve3D { .. } => clip_nurbs_curve_between_points(curve, start, end, t0, t1),
    }
}

fn clip_nurbs_curve_between_points(
    curve: &Curve3D,
    start: [f64; 3],
    end: [f64; 3],
    t0: f64,
    t1: f64,
) -> Option<ClippedCurve> {
    let sample_count = 17usize;
    let mut samples = Vec::with_capacity(sample_count);
    for i in 0..sample_count {
        let frac = i as f64 / (sample_count - 1) as f64;
        samples.push(curve.evaluate(t0 + (t1 - t0) * frac));
    }
    samples[0] = start;
    samples[sample_count - 1] = end;
    let tolerance = vec3::distance(start, end).max(1.0) * 5e-4;
    let clipped_curve = match fit_nurbs_curve(&samples, tolerance) {
        Ok(fitted) => Curve3D::NurbsCurve3D {
            degree: fitted.degree,
            control_points: fitted.control_points,
            weights: fitted.weights,
            knots: fitted.knots,
        },
        Err(_) => polyline_nurbs_curve(&samples),
    };
    Some(ClippedCurve {
        curve: clipped_curve,
        t_start: t0,
        t_end: t1,
    })
}

fn polyline_nurbs_curve(samples: &[[f64; 3]]) -> Curve3D {
    let n = samples.len();
    let mut knots = Vec::with_capacity(n + 2);
    knots.push(0.0);
    knots.push(0.0);
    if n > 2 {
        for i in 1..=(n - 2) {
            knots.push(i as f64 / (n - 1) as f64);
        }
    }
    knots.push(1.0);
    knots.push(1.0);
    Curve3D::NurbsCurve3D {
        degree: 1,
        control_points: samples.to_vec(),
        weights: vec![1.0; n],
        knots,
    }
}

fn curve_surface_error(surface: &Surface, curve: &Curve3D) -> f64 {
    sample_curve(curve, 5)
        .into_iter()
        .skip(1)
        .take(3)
        .filter_map(|point| surface_distance_to(surface, &point))
        .fold(0.0, f64::max)
}

fn curve_orientation_penalty(curve: &Curve3D, start: [f64; 3], end: [f64; 3]) -> f64 {
    let chord = vec3::sub(end, start);
    let chord_len = vec3::length(chord);
    if chord_len < 1e-12 {
        return 0.0;
    }
    let chord_dir = vec3::scale(chord, 1.0 / chord_len);
    [true, false]
        .into_iter()
        .filter_map(|at_start| {
            let tangent = curve_tangent_along_traversal(curve, at_start)?;
            Some((-vec3::dot(tangent, chord_dir)).max(0.0))
        })
        .sum()
}

fn curve_segment_deviation_error(
    original: &Curve3D,
    t_start: f64,
    t_end: f64,
    clipped: &Curve3D,
) -> f64 {
    let (u0, u1) = clipped.param_range();
    [0.25, 0.5, 0.75]
        .into_iter()
        .map(|frac| {
            let original_point = original.evaluate(t_start + (t_end - t_start) * frac);
            let clipped_point = clipped.evaluate(u0 + (u1 - u0) * frac);
            vec3::distance(original_point, clipped_point)
        })
        .fold(0.0, f64::max)
}

fn curve_endpoint_error(original: &Curve3D, t_start: f64, t_end: f64, clipped: &Curve3D) -> f64 {
    let (u0, u1) = clipped.param_range();
    vec3::distance(original.evaluate(t_start), clipped.evaluate(u0)).max(vec3::distance(
        original.evaluate(t_end),
        clipped.evaluate(u1),
    ))
}

fn sample_curve(curve: &Curve3D, sample_count: usize) -> Vec<[f64; 3]> {
    if sample_count <= 1 {
        return vec![curve.evaluate(curve.param_range().0)];
    }
    let (t0, t1) = curve.param_range();
    (0..sample_count)
        .map(|i| {
            let frac = i as f64 / (sample_count - 1) as f64;
            curve.evaluate(t0 + (t1 - t0) * frac)
        })
        .collect()
}

fn curve_tangent_along_traversal(curve: &Curve3D, at_start: bool) -> Option<[f64; 3]> {
    let (t0, t1) = curve.param_range();
    let delta = t1 - t0;
    if delta.abs() < 1e-10 {
        return None;
    }
    let step = delta.signum() * delta.abs().max(1.0) * 1e-4;
    let (ta, tb) = if at_start {
        (t0, clamp_parameter_to_segment(t0 + step, t0, t1))
    } else {
        (clamp_parameter_to_segment(t1 - step, t0, t1), t1)
    };
    let tangent = vec3::sub(curve.evaluate(tb), curve.evaluate(ta));
    let len = vec3::length(tangent);
    (len > 1e-12).then_some(vec3::scale(tangent, 1.0 / len))
}

fn clamp_parameter_to_segment(value: f64, t0: f64, t1: f64) -> f64 {
    if t0 <= t1 {
        value.clamp(t0, t1)
    } else {
        value.clamp(t1, t0)
    }
}

fn point_parameter_on_curve(
    curve: &Curve3D,
    point: &[f64; 3],
    reference: Option<f64>,
    allow_projection: bool,
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
            if !allow_projection
                && (vec3::distance(projected, *point) >= 1e-4 || !(-1e-4..=1.0 + 1e-4).contains(&t))
            {
                return None;
            }
            Some(t.clamp(0.0, 1.0))
        }
        Curve3D::Arc {
            center,
            axis,
            start,
            radius,
            ..
        } => {
            let axis_n = vec3::normalized(*axis);
            let r_raw = vec3::sub(*point, *center);
            if !allow_projection
                && ((vec3::length(r_raw) - *radius).abs() > 1e-3
                    || vec3::dot(r_raw, axis_n).abs() > 1e-3)
            {
                return None;
            }
            let r = vec3::sub(r_raw, vec3::scale(axis_n, vec3::dot(r_raw, axis_n)));
            let r_len = vec3::length(r);
            if r_len < 1e-12 || *radius <= 1e-12 {
                return None;
            }
            let r0 = vec3::sub(*start, *center);
            let tangent = vec3::scale(vec3::normalized(vec3::cross(axis_n, r0)), *radius);
            let x = vec3::dot(r, r0) / (r_len * *radius);
            let y = vec3::dot(r, tangent) / (r_len * *radius);
            closest_periodic_parameter(y.atan2(x), TAU, range, reference)
        }
        Curve3D::Ellipse {
            center,
            axis_u,
            axis_v,
            ..
        } => {
            let rel = vec3::sub(*point, *center);
            if !allow_projection && vec3::dot(rel, vec3::cross(*axis_u, *axis_v)).abs() > 1e-3 {
                return None;
            }
            let u_len_sq = vec3::dot(*axis_u, *axis_u);
            let v_len_sq = vec3::dot(*axis_v, *axis_v);
            if u_len_sq < 1e-20 || v_len_sq < 1e-20 {
                return None;
            }
            let cos_t = (vec3::dot(rel, *axis_u) / u_len_sq).clamp(-1.0, 1.0);
            let sin_t = (vec3::dot(rel, *axis_v) / v_len_sq).clamp(-1.0, 1.0);
            closest_periodic_parameter(sin_t.atan2(cos_t), TAU, range, reference)
        }
        Curve3D::NurbsCurve3D { .. } => point_parameter_on_nurbs_curve(curve, point),
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

fn point_parameter_on_nurbs_curve(curve: &Curve3D, point: &[f64; 3]) -> Option<f64> {
    let (t0, t1) = curve.param_range();
    let samples = 64usize;
    let mut best_t = t0;
    let mut best_distance = f64::INFINITY;
    for i in 0..=samples {
        let frac = i as f64 / samples as f64;
        let t = t0 + (t1 - t0) * frac;
        let distance = vec3::distance(curve.evaluate(t), *point);
        if distance < best_distance {
            best_distance = distance;
            best_t = t;
        }
    }

    let step = (t1 - t0).abs() / samples as f64;
    let mut left = clamp_parameter_to_segment(best_t - step, t0, t1);
    let mut right = clamp_parameter_to_segment(best_t + step, t0, t1);
    if (right - left).abs() < 1e-12 {
        return (best_distance < 1e-3).then_some(best_t);
    }
    for _ in 0..24 {
        let m1 = left + (right - left) / 3.0;
        let m2 = right - (right - left) / 3.0;
        let d1 = vec3::distance(curve.evaluate(m1), *point);
        let d2 = vec3::distance(curve.evaluate(m2), *point);
        if d1 <= d2 {
            right = m2;
        } else {
            left = m1;
        }
    }
    let refined = (left + right) * 0.5;
    let refined_distance = vec3::distance(curve.evaluate(refined), *point);
    (refined_distance < 5e-3).then_some(refined)
}

// ─── Tests ─────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::boolean3d::intersect3d::split_face;
    use crate::types::BooleanOp;

    fn shell_from_box_at(corner: [f64; 3], lx: f64, ly: f64, lz: f64) -> Shell {
        let shell = crate::shell_from_box(lx, ly, lz);
        let m = [
            [1.0, 0.0, 0.0, corner[0] + lx * 0.5],
            [0.0, 1.0, 0.0, corner[1] + ly * 0.5],
            [0.0, 0.0, 1.0, corner[2] + lz * 0.5],
            [0.0, 0.0, 0.0, 1.0],
        ];
        crate::apply_transform(&shell, &m)
    }

    fn plane_support_key(sf: &SubFace) -> Option<(Vec<i64>, bool)> {
        let Surface::Plane { origin, normal } = &sf.surface else {
            return None;
        };
        let d = normal[0] * origin[0] + normal[1] * origin[1] + normal[2] * origin[2];
        Some((
            vec![
                quantize(normal[0]),
                quantize(normal[1]),
                quantize(normal[2]),
                quantize(d),
            ],
            sf.flipped,
        ))
    }

    fn uv_polygon_area(poly: &[[f64; 2]]) -> f64 {
        let mut area = 0.0;
        for i in 0..poly.len() {
            let a = poly[i];
            let b = poly[(i + 1) % poly.len()];
            area += a[0] * b[1] - b[0] * a[1];
        }
        area.abs() * 0.5
    }

    fn subface_uv_polygon(sf: &SubFace) -> Option<Vec<[f64; 2]>> {
        sf.polygon
            .iter()
            .map(|p| sf.surface.inverse_project(p).map(|(u, v)| [u, v]))
            .collect()
    }

    #[test]
    fn surface_distance_to_plane() {
        let plane = Surface::Plane {
            origin: [0.0, 0.0, 0.0],
            normal: [0.0, 1.0, 0.0],
        };
        let on = [3.0, 0.0, 2.0];
        assert!(surface_distance_to(&plane, &on).unwrap() < 1e-10);
        let off = [0.0, 0.5, 0.0];
        assert!((surface_distance_to(&plane, &off).unwrap() - 0.5).abs() < 1e-10);
    }

    #[test]
    fn canonical_uv_loop_simplifies_backtracking_ring() {
        let ring = vec![
            [1.0, 1.0],
            [1.0, 0.0],
            [1.0, 1.0],
            [2.0, 1.0],
            [2.0, 0.0],
            [1.0, 0.0],
            [1.0, 1.0],
        ];
        let simplified = canonical_uv_loop(&ring);
        assert_eq!(
            simplified,
            vec![
                [1000000, 0],
                [1000000, 1000000],
                [2000000, 1000000],
                [2000000, 0]
            ]
        );
    }

    #[test]
    fn surface_distance_to_cylinder() {
        let cyl = Surface::Cylinder {
            origin: [0.0, 0.0, 0.0],
            axis: [0.0, 1.0, 0.0],
            radius: 1.0,
        };
        // Point on cylinder surface
        let on_surface = [1.0, 0.5, 0.0];
        assert!(surface_distance_to(&cyl, &on_surface).unwrap() < 1e-10);
        // Point 0.5 away from cylinder surface
        let off = [1.5, 0.5, 0.0];
        assert!((surface_distance_to(&cyl, &off).unwrap() - 0.5).abs() < 1e-10);
    }

    #[test]
    fn surface_distance_to_sphere() {
        let sph = Surface::Sphere {
            center: [0.0, 0.0, 0.0],
            radius: 1.0,
        };
        let on = [1.0, 0.0, 0.0];
        assert!(surface_distance_to(&sph, &on).unwrap() < 1e-10);
        let off = [2.0, 0.0, 0.0];
        assert!((surface_distance_to(&sph, &off).unwrap() - 1.0).abs() < 1e-10);
    }

    #[test]
    fn surface_distance_to_cone() {
        let cone = Surface::Cone {
            origin: [0.0, 0.0, 0.0],
            axis: [0.0, 1.0, 0.0],
            half_angle: std::f64::consts::FRAC_PI_4, // 45°
        };
        // tan(45deg) = 1, so radius at y=1 is 1.0
        let on = [1.0, 1.0, 0.0];
        assert!(surface_distance_to(&cone, &on).unwrap() < 1e-10);
        // radius at y=2 is 2.0
        let on2 = [0.0, 2.0, 2.0];
        assert!(surface_distance_to(&cone, &on2).unwrap() < 1e-10);
    }

    #[test]
    fn surface_distance_to_torus() {
        let torus = Surface::Torus {
            center: [0.0, 0.0, 0.0],
            axis: [0.0, 1.0, 0.0],
            major_radius: 3.0,
            minor_radius: 1.0,
        };
        // x-axis, outer: major + minor = 4.0
        let outer = [4.0, 0.0, 0.0];
        assert!(surface_distance_to(&torus, &outer).unwrap() < 1e-10);
        // x-axis, inner: major - minor = 2.0
        let inner = [2.0, 0.0, 0.0];
        assert!(surface_distance_to(&torus, &inner).unwrap() < 1e-10);
        // Off-surface point
        let off = [5.0, 0.0, 0.0];
        assert!((surface_distance_to(&torus, &off).unwrap() - 1.0).abs() < 1e-10);
    }

    #[test]
    fn surface_normal_at_sphere() {
        let sph = Surface::Sphere {
            center: [0.0, 0.0, 0.0],
            radius: 1.0,
        };
        let n = surface_normal_at(&sph, &[1.0, 0.0, 0.0]).unwrap();
        assert!((n[0] - 1.0).abs() < 1e-10);
        assert!(n[1].abs() < 1e-10);
        assert!(n[2].abs() < 1e-10);
    }

    #[test]
    fn surface_normal_at_cylinder() {
        let cyl = Surface::Cylinder {
            origin: [0.0, 0.0, 0.0],
            axis: [0.0, 1.0, 0.0],
            radius: 1.0,
        };
        let n = surface_normal_at(&cyl, &[1.0, 5.0, 0.0]).unwrap();
        assert!((n[0] - 1.0).abs() < 1e-10);
        assert!(n[1].abs() < 1e-10);
        assert!(n[2].abs() < 1e-10);
    }

    #[test]
    fn surface_normal_at_cone() {
        let cone = Surface::Cone {
            origin: [0.0, 0.0, 0.0],
            axis: [0.0, 1.0, 0.0],
            half_angle: std::f64::consts::FRAC_PI_4,
        };
        // 45deg cone: normal = radial*cos(45) - axis*sin(45) = (1/sqrt2, -1/sqrt2, 0)
        let n = surface_normal_at(&cone, &[1.0, 1.0, 0.0]).unwrap();
        let expected_x = 1.0_f64 / 2.0_f64.sqrt();
        let expected_y = -1.0_f64 / 2.0_f64.sqrt();
        assert!((n[0] - expected_x).abs() < 1e-10);
        assert!((n[1] - expected_y).abs() < 1e-10);
        assert!(n[2].abs() < 1e-10);
    }

    #[test]
    fn surface_normal_at_torus() {
        let torus = Surface::Torus {
            center: [0.0, 0.0, 0.0],
            axis: [0.0, 1.0, 0.0],
            major_radius: 3.0,
            minor_radius: 1.0,
        };
        // Outer point (4,0,0) -> tube_center (3,0,0) -> normal (1,0,0)
        let n = surface_normal_at(&torus, &[4.0, 0.0, 0.0]).unwrap();
        assert!((n[0] - 1.0).abs() < 1e-10);
        assert!(n[1].abs() < 1e-10);
        assert!(n[2].abs() < 1e-10);
    }

    #[test]
    fn reconstruct_edge_curve_prefers_curve_on_source_surface() {
        let mut shell = Shell::new();
        let v0 = shell.add_vertex([1.0, 0.0, 0.0]);
        let v1 = shell.add_vertex([0.0, 1.0, 0.0]);
        let line_edge = shell.add_edge(
            v0,
            v1,
            Curve3D::Line {
                start: [1.0, 0.0, 0.0],
                end: [0.0, 1.0, 0.0],
            },
        );
        let face = Face {
            loop_edges: vec![EdgeRef {
                edge_id: line_edge,
                forward: true,
            }],
            surface: Surface::Sphere {
                center: [0.0, 0.0, 0.0],
                radius: 1.0,
            },
            orientation_reversed: false,
        };
        let candidate = Curve3D::Ellipse {
            center: [0.0, 0.0, 0.0],
            axis_u: [1.0, 0.0, 0.0],
            axis_v: [0.0, 1.0, 0.0],
            t_start: 0.0,
            t_end: std::f64::consts::FRAC_PI_2,
        };

        let reconstructed = reconstruct_edge_curve(
            [1.0, 0.0, 0.0],
            [0.0, 1.0, 0.0],
            &face,
            &shell,
            &[candidate],
        )
        .unwrap();

        assert!(
            matches!(reconstructed, Curve3D::Ellipse { .. }),
            "expected curve on sphere surface to win over source line, got {reconstructed:?}"
        );
    }

    #[test]
    fn clip_curve_between_points_reverses_arc_by_flipping_axis() {
        let curve = Curve3D::Arc {
            center: [0.0, 0.0, 0.0],
            axis: [0.0, 0.0, 1.0],
            start: [1.0, 0.0, 0.0],
            end: [0.0, 1.0, 0.0],
            radius: 1.0,
        };

        let clipped = clip_curve_between_points(&curve, [0.0, 1.0, 0.0], [1.0, 0.0, 0.0]).unwrap();
        let Curve3D::Arc { axis, .. } = clipped.curve else {
            panic!("expected arc");
        };
        assert_eq!(axis, [0.0, 0.0, -1.0]);
        let (t_start, t_end) = clipped.curve.param_range();
        assert!(t_start.abs() < 1e-12);
        assert!((t_end - std::f64::consts::FRAC_PI_2).abs() < 1e-8);
        let midpoint = clipped.curve.evaluate((t_start + t_end) * 0.5);
        let expected = [
            std::f64::consts::FRAC_1_SQRT_2,
            std::f64::consts::FRAC_1_SQRT_2,
            0.0,
        ];
        assert!(vec3::distance(midpoint, expected) < 1e-6);
    }

    #[test]
    fn clip_curve_between_points_preserves_ellipse_wraparound() {
        let curve = Curve3D::Ellipse {
            center: [0.0, 0.0, 0.0],
            axis_u: [2.0, 0.0, 0.0],
            axis_v: [0.0, 1.0, 0.0],
            t_start: 5.5,
            t_end: 6.5,
        };
        let start = curve.evaluate(6.1);
        let end = curve.evaluate(6.35);

        let clipped = clip_curve_between_points(&curve, start, end).unwrap();
        let Curve3D::Ellipse { t_start, t_end, .. } = clipped.curve else {
            panic!("expected ellipse");
        };
        assert!((t_start - 6.1).abs() < 1e-6, "unexpected t_start={t_start}");
        assert!((t_end - 6.35).abs() < 1e-6, "unexpected t_end={t_end}");
        assert!(vec3::distance(clipped.curve.evaluate(t_start), start) < 1e-6);
        assert!(vec3::distance(clipped.curve.evaluate(t_end), end) < 1e-6);
    }

    #[test]
    fn clip_curve_between_points_fits_nurbs_segment() {
        let curve = Curve3D::NurbsCurve3D {
            degree: 3,
            control_points: vec![
                [0.0, 0.0, 0.0],
                [0.3, 0.5, 0.0],
                [0.7, 0.5, 0.0],
                [1.0, 0.0, 0.0],
            ],
            weights: vec![1.0; 4],
            knots: vec![0.0, 0.0, 0.0, 0.0, 1.0, 1.0, 1.0, 1.0],
        };
        let start = curve.evaluate(0.2);
        let end = curve.evaluate(0.8);

        let clipped = clip_curve_between_points(&curve, start, end).unwrap();
        assert!(
            matches!(clipped.curve, Curve3D::NurbsCurve3D { .. }),
            "expected NURBS clip, got {:?}",
            clipped.curve
        );
        let (u0, u1) = clipped.curve.param_range();
        assert!(vec3::distance(clipped.curve.evaluate(u0), start) < 5e-3);
        assert!(vec3::distance(clipped.curve.evaluate(u1), end) < 5e-3);
        let midpoint = clipped.curve.evaluate((u0 + u1) * 0.5);
        let expected = curve.evaluate(0.5);
        assert!(vec3::distance(midpoint, expected) < 5e-3);
    }

    #[test]
    fn find_interior_point_prefers_inside_candidate() {
        let surface = Surface::Plane {
            origin: [0.0, 0.0, 0.0],
            normal: [0.0, 0.0, 1.0],
        };
        let polygon = vec![
            [0.0, 0.0, 0.0],
            [4.0, 0.0, 0.0],
            [4.0, 4.0, 0.0],
            [3.0, 4.0, 0.0],
            [3.0, 1.0, 0.0],
            [1.0, 1.0, 0.0],
            [1.0, 4.0, 0.0],
            [0.0, 4.0, 0.0],
        ];

        let centroid = project_point_to_surface(&surface, &polygon_centroid(&polygon));
        assert!(!point_in_polygon_on_surface(&centroid, &polygon, &surface));

        let point = find_interior_point(&polygon, &surface);
        assert!(point_in_polygon_on_surface(&point, &polygon, &surface));
        assert!(vec3::distance(point, centroid) > 1e-6);
    }

    #[test]
    fn face_sharing_box_union_source_spanning_plane_groups_preserve_area_in_uv() {
        let shell_a = shell_from_box_at([0.0, 0.0, 0.0], 1.0, 1.0, 1.0);
        let shell_b = shell_from_box_at([1.0, 0.0, 0.0], 1.0, 1.0, 1.0);

        let sub_a: Vec<_> = shell_a
            .faces
            .iter()
            .enumerate()
            .flat_map(|(i, f)| split_face(f, &shell_a, &[], 0, i))
            .collect();
        let sub_b: Vec<_> = shell_b
            .faces
            .iter()
            .enumerate()
            .flat_map(|(i, f)| split_face(f, &shell_b, &[], 1, i))
            .collect();

        let selected = select_faces(&sub_a, &sub_b, &shell_a, &shell_b, BooleanOp::Union);
        let mut groups = BTreeMap::<(Vec<i64>, bool), Vec<SubFace>>::new();
        for sf in selected {
            if let Some(key) = plane_support_key(&sf) {
                groups.entry(key).or_default().push(sf);
            }
        }

        let spanning_groups: Vec<_> = groups
            .into_iter()
            .filter(|(_, group)| {
                group.len() > 1
                    && group
                        .iter()
                        .map(|sf| sf.source_shell)
                        .collect::<BTreeSet<_>>()
                        .len()
                        > 1
            })
            .collect();

        assert!(
            !spanning_groups.is_empty(),
            "face-sharing union should contain source-spanning coplanar plane groups"
        );

        for ((_plane_key, _flipped), group) in spanning_groups {
            let loops = reconstruct_plane_union_loops(&group).expect("reconstruct loops");
            assert_eq!(
                loops.len(),
                1,
                "source-spanning group should form a single loop"
            );
            let loop_area = uv_polygon_area(&loops[0]);
            let part_area: f64 = group
                .iter()
                .map(|sf| subface_uv_polygon(sf).expect("uv polygon"))
                .map(|poly| uv_polygon_area(&poly))
                .sum();
            assert!(
                (loop_area - part_area).abs() < 1e-8,
                "source-spanning plane loop should preserve area: loop_area={loop_area} part_area={part_area}"
            );
        }
    }

    #[test]
    #[ignore = "characterization: inspect edge multiplicities for source-spanning plane groups"]
    fn face_sharing_box_union_source_spanning_plane_groups_edge_multiplicities() {
        let shell_a = shell_from_box_at([0.0, 0.0, 0.0], 1.0, 1.0, 1.0);
        let shell_b = shell_from_box_at([1.0, 0.0, 0.0], 1.0, 1.0, 1.0);

        let sub_a: Vec<_> = shell_a
            .faces
            .iter()
            .enumerate()
            .flat_map(|(i, f)| split_face(f, &shell_a, &[], 0, i))
            .collect();
        let sub_b: Vec<_> = shell_b
            .faces
            .iter()
            .enumerate()
            .flat_map(|(i, f)| split_face(f, &shell_b, &[], 1, i))
            .collect();

        let selected = select_faces(&sub_a, &sub_b, &shell_a, &shell_b, BooleanOp::Union);
        let mut groups = BTreeMap::<(Vec<i64>, bool), Vec<SubFace>>::new();
        for sf in selected {
            if let Some(key) = plane_support_key(&sf) {
                groups.entry(key).or_default().push(sf);
            }
        }

        let spanning_groups: Vec<_> = groups
            .into_iter()
            .filter(|(_, group)| {
                group.len() > 1
                    && group
                        .iter()
                        .map(|sf| sf.source_shell)
                        .collect::<BTreeSet<_>>()
                        .len()
                        > 1
            })
            .collect();

        assert!(
            !spanning_groups.is_empty(),
            "face-sharing union should contain source-spanning coplanar plane groups"
        );

        for (group_index, ((_plane_key, _flipped), group)) in
            spanning_groups.into_iter().enumerate()
        {
            let counts = plane_group_edge_multiplicities(&group).expect("edge multiplicities");
            let ones = counts.values().filter(|&&c| c == 1).count();
            let twos = counts.values().filter(|&&c| c == 2).count();
            let many: Vec<_> = counts.iter().filter(|(_, &c)| c > 2).collect();
            eprintln!(
                "group {group_index}: faces={} unique_edges={} count1={} count2={} count>2={}",
                group.len(),
                counts.len(),
                ones,
                twos,
                many.len()
            );
            for (edge, count) in many {
                eprintln!("  edge {:?} count={}", edge, count);
            }
        }
    }

    #[test]
    #[ignore = "characterization: inspect plane groups after normalization for face-sharing box union"]
    fn face_sharing_box_union_normalized_plane_groups() {
        let shell_a = shell_from_box_at([0.0, 0.0, 0.0], 1.0, 1.0, 1.0);
        let shell_b = shell_from_box_at([1.0, 0.0, 0.0], 1.0, 1.0, 1.0);

        let sub_a: Vec<_> = shell_a
            .faces
            .iter()
            .enumerate()
            .flat_map(|(i, f)| split_face(f, &shell_a, &[], 0, i))
            .collect();
        let sub_b: Vec<_> = shell_b
            .faces
            .iter()
            .enumerate()
            .flat_map(|(i, f)| split_face(f, &shell_b, &[], 1, i))
            .collect();

        let selected = select_faces(&sub_a, &sub_b, &shell_a, &shell_b, BooleanOp::Union);
        let normalized = normalize_selected_subfaces(selected);
        let mut groups = BTreeMap::<(Vec<i64>, bool), Vec<SubFace>>::new();
        for sf in normalized {
            if let Some(key) = plane_support_key(&sf) {
                groups.entry(key).or_default().push(sf);
            }
        }

        for (group_index, ((_plane_key, _flipped), group)) in groups.into_iter().enumerate() {
            let bboxes = group
                .iter()
                .map(|sf| {
                    let poly = subface_uv_polygon(sf).expect("uv polygon");
                    let mut min = [f64::INFINITY; 2];
                    let mut max = [f64::NEG_INFINITY; 2];
                    for p in poly {
                        min[0] = min[0].min(p[0]);
                        min[1] = min[1].min(p[1]);
                        max[0] = max[0].max(p[0]);
                        max[1] = max[1].max(p[1]);
                    }
                    (min, max)
                })
                .collect::<Vec<_>>();
            eprintln!(
                "normalized group {group_index}: faces={} bboxes={bboxes:?}",
                group.len()
            );
        }
    }

    #[test]
    #[ignore = "characterization: inspect plane groups after actual boolean cuts for face-sharing box union"]
    fn face_sharing_box_union_normalized_plane_groups_with_actual_cuts() {
        use crate::boolean3d::intersect3d::face_face_intersection;

        let shell_a = shell_from_box_at([0.0, 0.0, 0.0], 1.0, 1.0, 1.0);
        let shell_b = shell_from_box_at([1.0, 0.0, 0.0], 1.0, 1.0, 1.0);

        let mut cuts_a = vec![Vec::new(); shell_a.faces.len()];
        let mut cuts_b = vec![Vec::new(); shell_b.faces.len()];

        for (ia, fa) in shell_a.faces.iter().enumerate() {
            for (ib, fb) in shell_b.faces.iter().enumerate() {
                cuts_a[ia].extend(face_face_intersection(
                    fa,
                    &shell_a,
                    fb,
                    &shell_b,
                    &mut vec![],
                ));
                cuts_b[ib].extend(face_face_intersection(
                    fb,
                    &shell_b,
                    fa,
                    &shell_a,
                    &mut vec![],
                ));
            }
        }

        let sub_a: Vec<_> = shell_a
            .faces
            .iter()
            .enumerate()
            .flat_map(|(i, f)| split_face(f, &shell_a, &cuts_a[i], 0, i))
            .collect();
        let sub_b: Vec<_> = shell_b
            .faces
            .iter()
            .enumerate()
            .flat_map(|(i, f)| split_face(f, &shell_b, &cuts_b[i], 1, i))
            .collect();

        let selected = select_faces(&sub_a, &sub_b, &shell_a, &shell_b, BooleanOp::Union);
        let normalized = normalize_selected_subfaces(selected);
        let mut groups = BTreeMap::<(Vec<i64>, bool), Vec<SubFace>>::new();
        for sf in normalized {
            if let Some(key) = plane_support_key(&sf) {
                groups.entry(key).or_default().push(sf);
            }
        }

        for (group_index, ((_plane_key, _flipped), group)) in groups.into_iter().enumerate() {
            let source_count = group
                .iter()
                .map(|sf| (sf.source_shell, sf.source_face))
                .collect::<Vec<_>>();
            let bboxes = group
                .iter()
                .map(|sf| {
                    let poly = subface_uv_polygon(sf).expect("uv polygon");
                    let mut min = [f64::INFINITY; 2];
                    let mut max = [f64::NEG_INFINITY; 2];
                    for p in poly {
                        min[0] = min[0].min(p[0]);
                        min[1] = min[1].min(p[1]);
                        max[0] = max[0].max(p[0]);
                        max[1] = max[1].max(p[1]);
                    }
                    (min, max)
                })
                .collect::<Vec<_>>();
            eprintln!(
                "actual-cut normalized group {group_index}: faces={} sources={source_count:?} bboxes={bboxes:?}",
                group.len()
            );
        }
    }
}
