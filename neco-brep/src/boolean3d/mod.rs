pub mod classify3d;
pub mod combine3d;
pub mod intersect3d;
pub mod nurbs_intersect;
pub(crate) mod sweep_intersect;
pub mod tet_clip;
pub mod tolerance;

/// Events emitted during boolean processing
#[derive(Debug, Clone)]
pub enum BooleanEvent {
    Info(String),
    Warning(String),
}

use crate::boolean3d::classify3d::point_in_shell;
use crate::boolean3d::combine3d::{
    build_shell_from_subfaces, has_substantial_selection_overlap, normalize_selected_subfaces,
    select_faces,
};
use crate::boolean3d::intersect3d::{face_face_intersection, split_face};
use crate::brep::Curve3D;
use crate::brep::{Shell, SubFace};
use crate::types::BooleanOp;
use crate::vec3;

/// 3D boolean operation orchestrator
pub fn boolean_3d(a: &Shell, b: &Shell, op: BooleanOp) -> Result<Shell, String> {
    // 1. Collect face-face intersection curves
    let na = a.faces.len();
    let nb = b.faces.len();
    let mut cuts_a = vec![Vec::new(); na];
    let mut cuts_b = vec![Vec::new(); nb];
    let mut events = Vec::new();

    for (ia, fa) in a.faces.iter().enumerate() {
        for (ib, fb) in b.faces.iter().enumerate() {
            let curves_a = face_face_intersection(fa, a, fb, b, &mut events);
            let curves_b = face_face_intersection(fb, b, fa, a, &mut events);
            cuts_a[ia].extend(curves_a);
            cuts_b[ib].extend(curves_b);
        }
    }

    for curves in &mut cuts_a {
        dedup_curves(curves);
    }
    for curves in &mut cuts_b {
        dedup_curves(curves);
    }

    let has_intersections = cuts_a.iter().any(|c| !c.is_empty());

    if !has_intersections {
        return handle_no_intersection(a, b, op);
    }

    // 2. Split faces
    let sub_a: Vec<SubFace> = a
        .faces
        .iter()
        .enumerate()
        .flat_map(|(i, f)| split_face(f, a, &cuts_a[i], 0, i))
        .collect();

    let sub_b: Vec<SubFace> = b
        .faces
        .iter()
        .enumerate()
        .flat_map(|(i, f)| split_face(f, b, &cuts_b[i], 1, i))
        .collect();

    if !has_substantial_selection_overlap(&sub_a, &sub_b, a, b) {
        return handle_no_intersection(a, b, op);
    }

    // 3. Select faces
    let selected = normalize_selected_subfaces(select_faces(&sub_a, &sub_b, a, b, op));
    if selected.is_empty() {
        return match op {
            BooleanOp::Intersect => Ok(Shell::new()),
            _ => Err("no faces selected".to_string()),
        };
    }

    // 4. Build shell
    build_shell_from_subfaces(&selected, a, b)
}

fn dedup_curves(curves: &mut Vec<Curve3D>) {
    let mut unique = Vec::new();
    for curve in curves.drain(..) {
        if unique
            .iter()
            .any(|existing| curves_approx_equal(existing, &curve))
        {
            continue;
        }
        unique.push(curve);
    }
    *curves = unique;
}

fn curves_approx_equal(a: &Curve3D, b: &Curve3D) -> bool {
    match (a, b) {
        (
            Curve3D::Line {
                start: a_start,
                end: a_end,
            },
            Curve3D::Line {
                start: b_start,
                end: b_end,
            },
        ) => {
            (points_close(a_start, b_start) && points_close(a_end, b_end))
                || (points_close(a_start, b_end) && points_close(a_end, b_start))
        }
        (
            Curve3D::Arc {
                center: a_center,
                axis: a_axis,
                start: a_start,
                end: a_end,
                radius: a_radius,
            },
            Curve3D::Arc {
                center: b_center,
                axis: b_axis,
                start: b_start,
                end: b_end,
                radius: b_radius,
            },
        ) => {
            points_close(a_center, b_center)
                && approx_eq(*a_radius, *b_radius)
                && ((points_close(a_axis, b_axis)
                    && ((points_close(a_start, b_start) && points_close(a_end, b_end))
                        || (points_close(a_start, b_end) && points_close(a_end, b_start))))
                    || (points_close(a_axis, &crate::vec3::scale(*b_axis, -1.0))
                        && ((points_close(a_start, b_end) && points_close(a_end, b_start))
                            || (points_close(a_start, b_start) && points_close(a_end, b_end)))))
        }
        (
            Curve3D::Ellipse {
                center: a_center,
                axis_u: a_axis_u,
                axis_v: a_axis_v,
                t_start: a_t_start,
                t_end: a_t_end,
            },
            Curve3D::Ellipse {
                center: b_center,
                axis_u: b_axis_u,
                axis_v: b_axis_v,
                t_start: b_t_start,
                t_end: b_t_end,
            },
        ) => {
            points_close(a_center, b_center)
                && points_close(a_axis_u, b_axis_u)
                && points_close(a_axis_v, b_axis_v)
                && ((approx_eq(*a_t_start, *b_t_start) && approx_eq(*a_t_end, *b_t_end))
                    || (approx_eq(*a_t_start, *b_t_end) && approx_eq(*a_t_end, *b_t_start)))
        }
        (Curve3D::NurbsCurve3D { .. }, Curve3D::NurbsCurve3D { .. }) => {
            let pa = a.to_polyline(1e-3);
            let pb = b.to_polyline(1e-3);
            if pa.len() != pb.len() {
                return false;
            }
            points_match_in_order(&pa, &pb)
                || points_match_in_order(&pa, &pb.iter().rev().copied().collect::<Vec<_>>())
        }
        _ => false,
    }
}

fn points_match_in_order(a: &[[f64; 3]], b: &[[f64; 3]]) -> bool {
    a.iter()
        .zip(b.iter())
        .all(|(pa, pb)| crate::vec3::distance(*pa, *pb) <= 1e-6)
}

fn points_close(a: &[f64; 3], b: &[f64; 3]) -> bool {
    crate::vec3::distance(*a, *b) <= 1e-3
}

fn approx_eq(a: f64, b: f64) -> bool {
    (a - b).abs() <= 1e-3
}

fn handle_no_intersection(a: &Shell, b: &Shell, op: BooleanOp) -> Result<Shell, String> {
    let a_center = shell_interior_point(a).unwrap_or_else(|| shell_vertex_centroid(a));
    let b_center = shell_interior_point(b).unwrap_or_else(|| shell_vertex_centroid(b));
    let a_in_b = point_in_shell(&a_center, b);
    let b_in_a = point_in_shell(&b_center, a);

    match op {
        BooleanOp::Union => {
            if a_in_b {
                Ok(b.clone())
            } else if b_in_a {
                Ok(a.clone())
            } else {
                Err("Union: disjoint shells not supported".to_string())
            }
        }
        BooleanOp::Subtract => {
            if b_in_a {
                // B contained in A -> A outer + B inner (flipped)
                let sub_a: Vec<SubFace> = a
                    .faces
                    .iter()
                    .enumerate()
                    .flat_map(|(i, f)| split_face(f, a, &[], 0, i))
                    .collect();
                let sub_b: Vec<SubFace> = b
                    .faces
                    .iter()
                    .enumerate()
                    .flat_map(|(i, f)| split_face(f, b, &[], 1, i))
                    .collect();
                let selected = select_faces(&sub_a, &sub_b, a, b, BooleanOp::Subtract);
                build_shell_from_subfaces(&selected, a, b)
            } else if a_in_b {
                Err("Subtract: A is contained in B".to_string())
            } else {
                Ok(a.clone())
            }
        }
        BooleanOp::Intersect => {
            if a_in_b {
                Ok(a.clone())
            } else if b_in_a {
                Ok(b.clone())
            } else {
                Ok(Shell::new())
            }
        }
    }
}

fn shell_interior_point(shell: &Shell) -> Option<[f64; 3]> {
    crate::shell_view(shell).faces.into_iter().find_map(|face| {
        let sample = face.sample?;
        let normal = sample.normal?;
        Some(vec3::sub(sample.point, vec3::scale(normal, 1e-5)))
    })
}

fn shell_vertex_centroid(shell: &Shell) -> [f64; 3] {
    let n = shell.vertices.len() as f64;
    let (sx, sy, sz) = shell.vertices.iter().fold((0.0, 0.0, 0.0), |(x, y, z), p| {
        (x + p[0], y + p[1], z + p[2])
    });
    [sx / n, sy / n, sz / n]
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::apply_transform;
    use crate::shell_from_box;

    fn shell_from_box_at(corner: [f64; 3], lx: f64, ly: f64, lz: f64) -> Shell {
        let shell = shell_from_box(lx, ly, lz);
        let m = [
            [1.0, 0.0, 0.0, corner[0] + lx * 0.5],
            [0.0, 1.0, 0.0, corner[1] + ly * 0.5],
            [0.0, 0.0, 1.0, corner[2] + lz * 0.5],
            [0.0, 0.0, 0.0, 1.0],
        ];
        apply_transform(&shell, &m)
    }

    #[test]
    fn curves_approx_equal_accepts_analytic_lines_and_reverse_order() {
        let a = Curve3D::Line {
            start: [0.0, 0.0, 0.0],
            end: [1.0, 0.0, 0.0],
        };
        let b = Curve3D::Line {
            start: [1.0, 0.0, 0.0],
            end: [0.0, 0.0, 0.0],
        };
        assert!(curves_approx_equal(&a, &b));
    }

    #[test]
    fn curves_approx_equal_accepts_analytic_arcs() {
        let a = Curve3D::Arc {
            center: [0.0, 0.0, 0.0],
            axis: [0.0, 0.0, 1.0],
            start: [1.0, 0.0, 0.0],
            end: [0.0, 1.0, 0.0],
            radius: 1.0,
        };
        let b = Curve3D::Arc {
            center: [0.0, 0.0, 0.0],
            axis: [0.0, 0.0, 1.0],
            start: [0.0, 1.0, 0.0],
            end: [1.0, 0.0, 0.0],
            radius: 1.0,
        };
        assert!(curves_approx_equal(&a, &b));

        let c = Curve3D::Arc {
            center: [0.0, 0.0, 0.0],
            axis: [0.0, 0.0, -1.0],
            start: [0.0, 1.0, 0.0],
            end: [1.0, 0.0, 0.0],
            radius: 1.0,
        };
        assert!(curves_approx_equal(&a, &c));
    }

    #[test]
    fn curves_approx_equal_accepts_analytic_ellipses() {
        let a = Curve3D::Ellipse {
            center: [0.0, 0.0, 0.0],
            axis_u: [2.0, 0.0, 0.0],
            axis_v: [0.0, 1.0, 0.0],
            t_start: 0.0,
            t_end: std::f64::consts::FRAC_PI_2,
        };
        let b = Curve3D::Ellipse {
            center: [0.0, 0.0, 0.0],
            axis_u: [2.0, 0.0, 0.0],
            axis_v: [0.0, 1.0, 0.0],
            t_start: std::f64::consts::FRAC_PI_2,
            t_end: 0.0,
        };
        assert!(curves_approx_equal(&a, &b));
    }

    #[test]
    fn curves_approx_equal_rejects_different_variants() {
        let line = Curve3D::Line {
            start: [0.0, 0.0, 0.0],
            end: [1.0, 0.0, 0.0],
        };
        let arc = Curve3D::Arc {
            center: [0.0, 0.0, 0.0],
            axis: [0.0, 0.0, 1.0],
            start: [1.0, 0.0, 0.0],
            end: [0.0, 1.0, 0.0],
            radius: 1.0,
        };
        assert!(!curves_approx_equal(&line, &arc));
    }

    #[test]
    #[ignore = "characterization: inspect face-sharing box union boolean path"]
    fn face_sharing_box_union_path_characterization() {
        let a = shell_from_box_at([0.0, 0.0, 0.0], 1.0, 1.0, 1.0);
        let b = shell_from_box_at([1.0, 0.0, 0.0], 1.0, 1.0, 1.0);

        let na = a.faces.len();
        let nb = b.faces.len();
        let mut cuts_a = vec![Vec::new(); na];
        let mut cuts_b = vec![Vec::new(); nb];
        let mut events = Vec::new();

        for (ia, fa) in a.faces.iter().enumerate() {
            for (ib, fb) in b.faces.iter().enumerate() {
                let curves_a = face_face_intersection(fa, &a, fb, &b, &mut events);
                let curves_b = face_face_intersection(fb, &b, fa, &a, &mut events);
                cuts_a[ia].extend(curves_a);
                cuts_b[ib].extend(curves_b);
            }
        }

        for curves in &mut cuts_a {
            dedup_curves(curves);
        }
        for curves in &mut cuts_b {
            dedup_curves(curves);
        }

        let has_intersections = cuts_a.iter().any(|c| !c.is_empty());
        eprintln!("has_intersections={has_intersections}");
        eprintln!(
            "cuts_a={:?}",
            cuts_a.iter().map(std::vec::Vec::len).collect::<Vec<_>>()
        );
        eprintln!(
            "cuts_b={:?}",
            cuts_b.iter().map(std::vec::Vec::len).collect::<Vec<_>>()
        );

        let result = boolean_3d(&a, &b, BooleanOp::Union).expect("boolean_3d");
        eprintln!("result_faces={}", result.faces.len());
    }

    #[test]
    #[ignore = "characterization: inspect normalized subfaces on face-sharing box union actual path"]
    fn face_sharing_box_union_normalized_subfaces_characterization() {
        let a = shell_from_box_at([0.0, 0.0, 0.0], 1.0, 1.0, 1.0);
        let b = shell_from_box_at([1.0, 0.0, 0.0], 1.0, 1.0, 1.0);

        let na = a.faces.len();
        let nb = b.faces.len();
        let mut cuts_a = vec![Vec::new(); na];
        let mut cuts_b = vec![Vec::new(); nb];
        let mut events = Vec::new();

        for (ia, fa) in a.faces.iter().enumerate() {
            for (ib, fb) in b.faces.iter().enumerate() {
                let curves_a = face_face_intersection(fa, &a, fb, &b, &mut events);
                let curves_b = face_face_intersection(fb, &b, fa, &a, &mut events);
                cuts_a[ia].extend(curves_a);
                cuts_b[ib].extend(curves_b);
            }
        }

        for curves in &mut cuts_a {
            dedup_curves(curves);
        }
        for curves in &mut cuts_b {
            dedup_curves(curves);
        }

        let sub_a: Vec<SubFace> = a
            .faces
            .iter()
            .enumerate()
            .flat_map(|(i, f)| split_face(f, &a, &cuts_a[i], 0, i))
            .collect();
        let sub_b: Vec<SubFace> = b
            .faces
            .iter()
            .enumerate()
            .flat_map(|(i, f)| split_face(f, &b, &cuts_b[i], 1, i))
            .collect();

        let selected = select_faces(&sub_a, &sub_b, &a, &b, BooleanOp::Union);
        let normalized = normalize_selected_subfaces(selected);
        eprintln!("normalized_faces={}", normalized.len());
        for (idx, sf) in normalized.iter().enumerate() {
            let mut min = [f64::INFINITY; 3];
            let mut max = [f64::NEG_INFINITY; 3];
            for p in &sf.polygon {
                min[0] = min[0].min(p[0]);
                min[1] = min[1].min(p[1]);
                min[2] = min[2].min(p[2]);
                max[0] = max[0].max(p[0]);
                max[1] = max[1].max(p[1]);
                max[2] = max[2].max(p[2]);
            }
            eprintln!(
                "  sf[{idx}]: shell={} face={} polygon_len={} flipped={} bbox={:?}->{:?} poly={:?}",
                sf.source_shell,
                sf.source_face,
                sf.polygon.len(),
                sf.flipped,
                min,
                max,
                sf.polygon
            );
        }
    }

    #[test]
    #[ignore = "characterization: inspect edge-edge contact boolean path"]
    fn edge_edge_contact_union_path_characterization() {
        let a = shell_from_box_at([0.0, 0.0, 0.0], 1.0, 1.0, 1.0);
        let b = shell_from_box_at([1.0, 1.0, 0.0], 1.0, 1.0, 1.0);

        let na = a.faces.len();
        let nb = b.faces.len();
        let mut cuts_a = vec![Vec::new(); na];
        let mut cuts_b = vec![Vec::new(); nb];
        let mut events = Vec::new();

        for (ia, fa) in a.faces.iter().enumerate() {
            for (ib, fb) in b.faces.iter().enumerate() {
                let curves_a = face_face_intersection(fa, &a, fb, &b, &mut events);
                let curves_b = face_face_intersection(fb, &b, fa, &a, &mut events);
                cuts_a[ia].extend(curves_a);
                cuts_b[ib].extend(curves_b);
            }
        }

        for curves in &mut cuts_a {
            dedup_curves(curves);
        }
        for curves in &mut cuts_b {
            dedup_curves(curves);
        }

        let has_intersections = cuts_a.iter().any(|c| !c.is_empty());
        eprintln!("edge-edge has_intersections={has_intersections}");
        eprintln!(
            "edge-edge cuts_a={:?}",
            cuts_a.iter().map(std::vec::Vec::len).collect::<Vec<_>>()
        );
        eprintln!(
            "edge-edge cuts_b={:?}",
            cuts_b.iter().map(std::vec::Vec::len).collect::<Vec<_>>()
        );

        match boolean_3d(&a, &b, BooleanOp::Union) {
            Ok(shell) => eprintln!("edge-edge union result_faces={}", shell.faces.len()),
            Err(err) => eprintln!("edge-edge union err={err}"),
        }
        match boolean_3d(&a, &b, BooleanOp::Intersect) {
            Ok(shell) => eprintln!("edge-edge intersect result_faces={}", shell.faces.len()),
            Err(err) => eprintln!("edge-edge intersect err={err}"),
        }
    }

    #[test]
    #[ignore = "characterization: inspect vertex-face contact boolean path"]
    fn vertex_face_contact_union_path_characterization() {
        let a = shell_from_box_at([0.0, 0.0, 0.0], 1.0, 1.0, 1.0);
        let b = shell_from_box_at([1.0, 0.25, 0.25], 0.5, 0.5, 0.5);

        let na = a.faces.len();
        let nb = b.faces.len();
        let mut cuts_a = vec![Vec::new(); na];
        let mut cuts_b = vec![Vec::new(); nb];
        let mut events = Vec::new();

        for (ia, fa) in a.faces.iter().enumerate() {
            for (ib, fb) in b.faces.iter().enumerate() {
                let curves_a = face_face_intersection(fa, &a, fb, &b, &mut events);
                let curves_b = face_face_intersection(fb, &b, fa, &a, &mut events);
                cuts_a[ia].extend(curves_a);
                cuts_b[ib].extend(curves_b);
            }
        }

        for curves in &mut cuts_a {
            dedup_curves(curves);
        }
        for curves in &mut cuts_b {
            dedup_curves(curves);
        }

        let has_intersections = cuts_a.iter().any(|c| !c.is_empty());
        eprintln!("vertex-face has_intersections={has_intersections}");
        eprintln!(
            "vertex-face cuts_a={:?}",
            cuts_a.iter().map(std::vec::Vec::len).collect::<Vec<_>>()
        );
        eprintln!(
            "vertex-face cuts_b={:?}",
            cuts_b.iter().map(std::vec::Vec::len).collect::<Vec<_>>()
        );

        match boolean_3d(&a, &b, BooleanOp::Union) {
            Ok(shell) => eprintln!("vertex-face union result_faces={}", shell.faces.len()),
            Err(err) => eprintln!("vertex-face union err={err}"),
        }
        match boolean_3d(&a, &b, BooleanOp::Intersect) {
            Ok(shell) => eprintln!("vertex-face intersect result_faces={}", shell.faces.len()),
            Err(err) => eprintln!("vertex-face intersect err={err}"),
        }
    }
}
