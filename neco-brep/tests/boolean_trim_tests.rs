use neco_brep::boolean3d::classify3d::{point_in_face_polygon, point_in_shell};
use neco_brep::boolean3d::intersect3d::face_polygon;
use neco_brep::vec3;
use neco_brep::{
    apply_transform, boolean_3d, shell_from_box, shell_from_sphere, shell_from_torus, shell_view,
    BooleanOp, Curve3D, Edge, Face, Shell,
};

fn translate_shell(shell: &Shell, offset: [f64; 3]) -> Shell {
    let m = [
        [1.0, 0.0, 0.0, offset[0]],
        [0.0, 1.0, 0.0, offset[1]],
        [0.0, 0.0, 1.0, offset[2]],
        [0.0, 0.0, 0.0, 1.0],
    ];
    apply_transform(shell, &m)
}

fn shell_from_box_at(corner: [f64; 3], lx: f64, ly: f64, lz: f64) -> Shell {
    let shell = shell_from_box(lx, ly, lz);
    translate_shell(
        &shell,
        [
            corner[0] + lx * 0.5,
            corner[1] + ly * 0.5,
            corner[2] + lz * 0.5,
        ],
    )
}

fn polygon_centroid(poly: &[[f64; 3]]) -> [f64; 3] {
    let sum = poly.iter().copied().fold([0.0, 0.0, 0.0], vec3::add);
    vec3::scale(sum, 1.0 / poly.len() as f64)
}

fn triangle_centroid(a: [f64; 3], b: [f64; 3], c: [f64; 3]) -> [f64; 3] {
    vec3::scale(vec3::add(vec3::add(a, b), c), 1.0 / 3.0)
}

fn polygon_normal(poly: &[[f64; 3]]) -> [f64; 3] {
    let mut normal = [0.0, 0.0, 0.0];
    for i in 0..poly.len() {
        let a = poly[i];
        let b = poly[(i + 1) % poly.len()];
        normal[0] += (a[1] - b[1]) * (a[2] + b[2]);
        normal[1] += (a[2] - b[2]) * (a[0] + b[0]);
        normal[2] += (a[0] - b[0]) * (a[1] + b[1]);
    }
    vec3::normalized(normal)
}

fn interior_point(face: &Face, shell: &Shell, poly: &[[f64; 3]]) -> Option<[f64; 3]> {
    let centroid = project_point_to_face_surface(face, &polygon_centroid(poly));
    if point_in_face_polygon(&centroid, face, shell) {
        return Some(centroid);
    }

    for &vertex in poly {
        for fraction in [0.1, 0.25, 0.5, 0.75, 0.9] {
            let toward_centroid = project_point_to_face_surface(
                face,
                &vec3::add(
                    vertex,
                    vec3::scale(vec3::sub(polygon_centroid(poly), vertex), fraction),
                ),
            );
            if point_in_face_polygon(&toward_centroid, face, shell) {
                return Some(toward_centroid);
            }
        }
    }

    if poly.len() >= 3 {
        let root = poly[0];
        for i in 1..(poly.len() - 1) {
            let c =
                project_point_to_face_surface(face, &triangle_centroid(root, poly[i], poly[i + 1]));
            if point_in_face_polygon(&c, face, shell) {
                return Some(c);
            }
            let mid_ab = project_point_to_face_surface(
                face,
                &triangle_centroid(root, poly[i], polygon_centroid(poly)),
            );
            if point_in_face_polygon(&mid_ab, face, shell) {
                return Some(mid_ab);
            }

            for (wa, wb, wc) in [
                (0.6, 0.2, 0.2),
                (0.2, 0.6, 0.2),
                (0.2, 0.2, 0.6),
                (0.4, 0.4, 0.2),
            ] {
                let sample = [
                    root[0] * wa + poly[i][0] * wb + poly[i + 1][0] * wc,
                    root[1] * wa + poly[i][1] * wb + poly[i + 1][1] * wc,
                    root[2] * wa + poly[i][2] * wb + poly[i + 1][2] * wc,
                ];
                let sample = project_point_to_face_surface(face, &sample);
                if point_in_face_polygon(&sample, face, shell) {
                    return Some(sample);
                }
            }
        }
    }

    if let Some(uv_poly) = poly
        .iter()
        .map(|point| face.surface.inverse_project(point).map(|(u, v)| [u, v]))
        .collect::<Option<Vec<_>>>()
    {
        let uv_centroid = uv_poly
            .iter()
            .fold([0.0, 0.0], |acc, uv| [acc[0] + uv[0], acc[1] + uv[1]]);
        let uv_centroid = [
            uv_centroid[0] / uv_poly.len() as f64,
            uv_centroid[1] / uv_poly.len() as f64,
        ];
        let candidate = face.surface.evaluate(uv_centroid[0], uv_centroid[1]);
        if point_in_face_polygon(&candidate, face, shell) {
            return Some(candidate);
        }

        if uv_poly.len() >= 3 {
            let root = uv_poly[0];
            for i in 1..(uv_poly.len() - 1) {
                for (wa, wb, wc) in [(0.6, 0.2, 0.2), (0.2, 0.6, 0.2), (0.2, 0.2, 0.6)] {
                    let uv = [
                        root[0] * wa + uv_poly[i][0] * wb + uv_poly[i + 1][0] * wc,
                        root[1] * wa + uv_poly[i][1] * wb + uv_poly[i + 1][1] * wc,
                    ];
                    let candidate = face.surface.evaluate(uv[0], uv[1]);
                    if point_in_face_polygon(&candidate, face, shell) {
                        return Some(candidate);
                    }
                }
            }
        }
    }

    Some(centroid)
}

fn project_point_to_face_surface(face: &Face, point: &[f64; 3]) -> [f64; 3] {
    match face.surface.inverse_project(point) {
        Some((u, v)) => face.surface.evaluate(u, v),
        None => *point,
    }
}

fn face_surface_normal(face: &Face, poly: &[[f64; 3]], point: &[f64; 3]) -> Option<[f64; 3]> {
    let (u, v) = face.surface.inverse_project(point)?;
    let surf_n = face.surface.normal_at(u, v);
    if face.orientation_reversed {
        Some(vec3::scale(surf_n, -1.0))
    } else {
        let poly_n = polygon_normal(poly);
        Some(if vec3::dot(poly_n, surf_n) < 0.0 {
            vec3::scale(surf_n, -1.0)
        } else {
            surf_n
        })
    }
}

fn assert_loop_closed(face: &Face, shell: &Shell) {
    let n = face.loop_edges.len();
    assert!(n >= 3, "face loop must have at least 3 edges");
    for i in 0..n {
        let cur = face.loop_edges[i];
        let next = face.loop_edges[(i + 1) % n];
        let cur_edge = &shell.edges[cur.edge_id];
        let next_edge = &shell.edges[next.edge_id];
        let cur_end = if cur.forward {
            cur_edge.v_end
        } else {
            cur_edge.v_start
        };
        let next_start = if next.forward {
            next_edge.v_start
        } else {
            next_edge.v_end
        };
        assert_eq!(cur_end, next_start, "face loop is not closed");
    }
}

fn result_membership(op: BooleanOp, in_a: bool, in_b: bool) -> bool {
    match op {
        BooleanOp::Union => in_a || in_b,
        BooleanOp::Subtract => in_a && !in_b,
        BooleanOp::Intersect => in_a && in_b,
    }
}

fn curve_kind(curve: &Curve3D) -> &'static str {
    match curve {
        Curve3D::Line { .. } => "Line",
        Curve3D::Arc { .. } => "Arc",
        Curve3D::Ellipse { .. } => "Ellipse",
        Curve3D::NurbsCurve3D { .. } => "NurbsCurve3D",
    }
}

fn edge_samples_lie_on_face(face: &Face, edge: &Edge) -> bool {
    let (t0, t1) = edge.curve.param_range();
    [0.2, 0.4, 0.6, 0.8].into_iter().all(|frac| {
        let point = edge.curve.evaluate(t0 + (t1 - t0) * frac);
        let Some((u, v)) = face.surface.inverse_project(&point) else {
            return false;
        };
        vec3::distance(point, face.surface.evaluate(u, v)) < 1e-3
    })
}

fn assert_curved_trim_geometry(name: &str, result: &Shell) {
    let mut curved_edges = 0usize;
    let mut summary = Vec::new();
    for (face_index, face) in result.faces.iter().enumerate() {
        let mut face_curve_kinds = Vec::new();
        for edge_ref in &face.loop_edges {
            let edge = &result.edges[edge_ref.edge_id];
            face_curve_kinds.push(curve_kind(&edge.curve));
            if matches!(edge.curve, Curve3D::Line { .. }) {
                continue;
            }
            curved_edges += 1;
            assert!(
                edge_samples_lie_on_face(face, edge),
                "{name}: curved edge escaped its face surface; face_index={face_index} curve_kind={} surface={:?}",
                curve_kind(&edge.curve),
                face.surface
            );
        }
        summary.push(format!(
            "face[{face_index}] surface={:?} curves={face_curve_kinds:?}",
            face.surface
        ));
    }
    assert!(
        curved_edges > 0,
        "{name}: expected at least one retained non-line trim edge in the boolean result; summary={summary:?}"
    );
}

fn assert_shell_trim_semantics(name: &str, result: &Shell, a: &Shell, b: &Shell, op: BooleanOp) {
    assert!(
        !result.faces.is_empty(),
        "{name}: boolean result must contain faces"
    );

    for (face_index, face) in result.faces.iter().enumerate() {
        assert_loop_closed(face, result);

        let poly = face_polygon(face, result);
        assert!(
            poly.len() >= 3,
            "{name}: face {face_index} polygon has too few vertices"
        );

        let centroid = interior_point(face, result, &poly).unwrap_or_else(|| {
            panic!(
                "{name}: face {face_index} has no interior representative point; poly={poly:?} surface={:?}",
                face.surface
            )
        });

        let normal =
            face_surface_normal(face, &poly, &centroid).unwrap_or_else(|| polygon_normal(&poly));
        let eps = 1e-3;
        let inward = vec3::sub(centroid, vec3::scale(normal, eps));
        let outward = vec3::add(centroid, vec3::scale(normal, eps));

        let in_a = point_in_shell(&inward, a);
        let in_b = point_in_shell(&inward, b);
        let expected_inside_result = result_membership(op, in_a, in_b);
        assert!(
            expected_inside_result,
            "{name}: face {face_index} inward sample violates boolean membership: in_a={in_a} in_b={in_b} centroid={centroid:?} normal={normal:?} poly={poly:?} surface={:?}",
            face.surface
        );

        let outward_in_a = point_in_shell(&outward, a);
        let outward_in_b = point_in_shell(&outward, b);
        let outward_in_result = result_membership(op, outward_in_a, outward_in_b);
        assert!(
            !outward_in_result,
            "{name}: face {face_index} outward sample should be outside boolean result; outward_in_a={outward_in_a} outward_in_b={outward_in_b} centroid={centroid:?} normal={normal:?} poly={poly:?} surface={:?}",
            face.surface
        );
    }
}

#[test]
fn union_box_sphere_face_trim_matches_membership() {
    let box_shell = shell_from_box(2.0, 2.0, 2.0);
    let sphere_shell = translate_shell(&shell_from_sphere(0.9), [0.35, 0.0, 0.0]);
    let result = boolean_3d(&box_shell, &sphere_shell, BooleanOp::Union).unwrap();

    assert_shell_trim_semantics(
        "union_box_sphere_face_trim_matches_membership",
        &result,
        &box_shell,
        &sphere_shell,
        BooleanOp::Union,
    );
}

#[test]
fn union_box_sphere_large_overlap_face_trim_matches_membership() {
    let box_shell = shell_from_box_at([0.0, 0.0, 0.0], 2.0, 2.0, 2.0);
    let sphere_shell = translate_shell(&shell_from_sphere(1.25), [0.8, 0.0, 0.0]);
    let result = boolean_3d(&box_shell, &sphere_shell, BooleanOp::Union).unwrap();

    assert_shell_trim_semantics(
        "union_box_sphere_large_overlap_face_trim_matches_membership",
        &result,
        &box_shell,
        &sphere_shell,
        BooleanOp::Union,
    );
}

#[test]
fn union_box_sphere_large_overlap_shell_view_exports_polygons() {
    let box_shell = shell_from_box_at([0.0, 0.0, 0.0], 2.0, 2.0, 2.0);
    let sphere_shell = translate_shell(&shell_from_sphere(1.25), [0.8, 0.0, 0.0]);
    let result = boolean_3d(&box_shell, &sphere_shell, BooleanOp::Union).unwrap();
    let view = shell_view(&result);

    let sphere_faces: Vec<_> = view
        .faces
        .iter()
        .filter(|face| face.surface_kind == "Sphere")
        .collect();
    assert!(
        !sphere_faces.is_empty(),
        "shell view should include sphere faces"
    );
    assert!(sphere_faces.iter().all(|face| !face.polygon_3d.is_empty()));
    assert!(sphere_faces.iter().all(|face| !face.polygon_uv.is_empty()));
}

#[test]
fn union_overlapping_spheres_shell_view_faces_have_polygons() {
    let sphere_a = shell_from_sphere(2.0);
    let sphere_b = translate_shell(&shell_from_sphere(2.0), [1.5, 0.0, 0.0]);
    let result = boolean_3d(&sphere_a, &sphere_b, BooleanOp::Union).unwrap();
    let view = shell_view(&result);

    let sphere_faces: Vec<_> = view
        .faces
        .iter()
        .filter(|face| face.surface_kind == "Sphere")
        .collect();
    assert!(
        !sphere_faces.is_empty(),
        "shell view should include sphere faces for overlapping sphere union"
    );

    for face in &sphere_faces {
        assert!(
            !face.polygon_3d.is_empty(),
            "sphere shell view face must have polygon_3d"
        );
        assert!(
            !face.polygon_uv.is_empty(),
            "sphere shell view face must have polygon_uv"
        );
    }

    let sphere_result_faces: Vec<_> = result
        .faces
        .iter()
        .enumerate()
        .filter(|(_, face)| matches!(face.surface, neco_brep::Surface::Sphere { .. }))
        .collect();
    for (_idx, face) in sphere_result_faces {
        let poly = face_polygon(face, &result);
        let _sample = interior_point(face, &result, &poly).expect("interior point");
    }
}

#[test]
fn subtract_box_torus_face_trim_matches_membership() {
    let box_shell = shell_from_box_at([-2.0, -2.0, -2.0], 4.0, 4.0, 4.0);
    let torus_shell = shell_from_torus(1.0, 0.3);
    let result = boolean_3d(&box_shell, &torus_shell, BooleanOp::Subtract).unwrap();

    assert_shell_trim_semantics(
        "subtract_box_torus_face_trim_matches_membership",
        &result,
        &box_shell,
        &torus_shell,
        BooleanOp::Subtract,
    );
}

#[test]
fn intersect_box_sphere_face_trim_matches_membership() {
    let box_shell = shell_from_box(2.0, 2.0, 2.0);
    let sphere_shell = translate_shell(&shell_from_sphere(1.0), [0.4, 0.0, 0.0]);
    let result = boolean_3d(&box_shell, &sphere_shell, BooleanOp::Intersect).unwrap();

    assert_shell_trim_semantics(
        "intersect_box_sphere_face_trim_matches_membership",
        &result,
        &box_shell,
        &sphere_shell,
        BooleanOp::Intersect,
    );
}

#[test]
fn subtract_box_torus_retains_curved_trim_edges() {
    let box_shell = shell_from_box_at([-2.0, -2.0, -2.0], 4.0, 4.0, 4.0);
    let torus_shell = shell_from_torus(1.0, 0.3);
    let result = boolean_3d(&box_shell, &torus_shell, BooleanOp::Subtract).unwrap();

    assert_curved_trim_geometry("subtract_box_torus_retains_curved_trim_edges", &result);
}

#[test]
fn intersect_box_sphere_retains_curved_trim_edges() {
    let box_shell = shell_from_box(2.0, 2.0, 2.0);
    let sphere_shell = translate_shell(&shell_from_sphere(1.0), [0.4, 0.0, 0.0]);
    let result = boolean_3d(&box_shell, &sphere_shell, BooleanOp::Intersect).unwrap();

    assert_curved_trim_geometry("intersect_box_sphere_retains_curved_trim_edges", &result);
}

#[test]
fn intersect_box_sphere_contains_arc_trim_edges() {
    let box_shell = shell_from_box(2.0, 2.0, 2.0);
    let sphere_shell = translate_shell(&shell_from_sphere(1.0), [0.4, 0.0, 0.0]);
    let result = boolean_3d(&box_shell, &sphere_shell, BooleanOp::Intersect).unwrap();

    assert!(
        result
            .edges
            .iter()
            .any(|edge| matches!(edge.curve, Curve3D::Arc { .. })),
        "intersect_box_sphere_contains_arc_trim_edges: expected at least one Arc edge"
    );
}

#[test]
fn union_box_sphere_retains_curved_trim_edges() {
    let box_shell = shell_from_box(2.0, 2.0, 2.0);
    let sphere_shell = translate_shell(&shell_from_sphere(0.9), [0.35, 0.0, 0.0]);
    let result = boolean_3d(&box_shell, &sphere_shell, BooleanOp::Union).unwrap();

    assert_curved_trim_geometry("union_box_sphere_retains_curved_trim_edges", &result);
}
