use neco_brep::boolean3d::classify3d::point_in_shell;
use neco_brep::boolean3d::intersect3d::{
    face_face_intersection, face_polygon, plane_plane_intersect, split_face, SurfaceIntersection,
};
use neco_brep::vec3::{self, tet_volume};
use neco_brep::{
    apply_transform, shell_from_box, shell_from_extrude, shell_from_revolve, shell_from_sphere,
    shell_from_torus, Axis, BooleanOp, Curve3D, Radians, Shell, Surface,
};
use neco_nurbs::{NurbsCurve2D, NurbsRegion};

/// Local replacement for `shell_from_box_at`: build a box shell with `corner` at `(x, y, z)`.
fn shell_from_box_at(corner: [f64; 3], lx: f64, ly: f64, lz: f64) -> Shell {
    let s = shell_from_box(lx, ly, lz);
    let m = [
        [1.0, 0.0, 0.0, corner[0] + lx / 2.0],
        [0.0, 1.0, 0.0, corner[1] + ly / 2.0],
        [0.0, 0.0, 1.0, corner[2] + lz / 2.0],
        [0.0, 0.0, 0.0, 1.0],
    ];
    apply_transform(&s, &m)
}

fn polygon_area_3d(poly: &[[f64; 3]]) -> f64 {
    if poly.len() < 3 {
        return 0.0;
    }
    let root = poly[0];
    let mut area = 0.0;
    for i in 1..(poly.len() - 1) {
        area += vec3::tri_area(root, poly[i], poly[i + 1]);
    }
    area
}

fn assert_shell_matches_exactly(actual: &Shell, expected: &Shell) {
    assert_eq!(
        actual.vertices, expected.vertices,
        "vertices should match exactly"
    );
    assert_eq!(
        actual.edges.len(),
        expected.edges.len(),
        "edge count should match"
    );
    assert_eq!(
        actual.faces.len(),
        expected.faces.len(),
        "face count should match"
    );
}

#[test]
fn from_box_topology() {
    let s = shell_from_box_at([0.0, 0.0, 0.0], 2.0, 3.0, 4.0);
    assert_eq!(s.vertices.len(), 8);
    assert_eq!(s.edges.len(), 12);
    assert_eq!(s.faces.len(), 6);
}

#[test]
fn from_box_normals_point_outward() {
    let origin = [1.0, 2.0, 3.0];
    let s = shell_from_box_at(origin, 2.0, 3.0, 4.0);
    let center = [origin[0] + 1.0, origin[1] + 1.5, origin[2] + 2.0];

    for face in &s.faces {
        if let Surface::Plane { origin: fo, normal } = &face.surface {
            let outward = vec3::sub(*fo, center);
            assert!(
                vec3::dot(outward, *normal) >= 0.0,
                "normal {:?} not outward from center at face origin {:?}",
                normal,
                fo
            );
        }
    }
}

#[test]
fn from_box_face_loops_closed() {
    let s = shell_from_box_at([0.0, 0.0, 0.0], 1.0, 1.0, 1.0);

    for (fi, face) in s.faces.iter().enumerate() {
        let edges = &face.loop_edges;
        for i in 0..edges.len() {
            let cur = &edges[i];
            let next = &edges[(i + 1) % edges.len()];

            let cur_edge = &s.edges[cur.edge_id];
            let next_edge = &s.edges[next.edge_id];

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

            assert_eq!(
                cur_end,
                next_start,
                "face {} edge {} → {} not connected",
                fi,
                i,
                (i + 1) % edges.len()
            );
        }
    }
}

// --- Plane-plane intersection ---

#[test]
fn plane_plane_intersection_perpendicular() {
    let xy = Surface::Plane {
        origin: [0.0, 0.0, 0.0],
        normal: [0.0, 0.0, 1.0],
    };
    let xz = Surface::Plane {
        origin: [0.0, 0.0, 0.0],
        normal: [0.0, 1.0, 0.0],
    };
    let result = plane_plane_intersect(&xy, &xz).unwrap();
    match result {
        SurfaceIntersection::Line { point, direction } => {
            assert!(direction[1].abs() < 1e-9, "dir.y should be ~0");
            assert!(direction[2].abs() < 1e-9, "dir.z should be ~0");
            assert!(direction[0].abs() > 0.9, "dir.x should be ~±1");
            assert!(point[1].abs() < 1e-9);
            assert!(point[2].abs() < 1e-9);
        }
        other => panic!("expected Line, got {other:?}"),
    }
}

#[test]
fn plane_plane_parallel_no_intersection() {
    let a = Surface::Plane {
        origin: [0.0, 0.0, 0.0],
        normal: [0.0, 0.0, 1.0],
    };
    let b = Surface::Plane {
        origin: [0.0, 0.0, 5.0],
        normal: [0.0, 0.0, 1.0],
    };
    assert!(plane_plane_intersect(&a, &b).is_none());
}

#[test]
fn plane_plane_coplanar() {
    let a = Surface::Plane {
        origin: [0.0, 0.0, 0.0],
        normal: [0.0, 0.0, 1.0],
    };
    let b = Surface::Plane {
        origin: [3.0, 4.0, 0.0],
        normal: [0.0, 0.0, 1.0],
    };
    match plane_plane_intersect(&a, &b) {
        Some(SurfaceIntersection::Coplanar) => {}
        other => panic!("expected Coplanar, got {:?}", other),
    }
}

// --- Box-box integration tests ---

#[test]
fn two_boxes_intersection_edges() {
    let shell_a = shell_from_box_at([0.0, 0.0, 0.0], 2.0, 1.0, 1.0);
    let shell_b = shell_from_box_at([1.0, 0.0, 0.0], 2.0, 1.0, 1.0);

    let mut segments = Vec::new();
    let mut events = Vec::new();
    for face_a in &shell_a.faces {
        for face_b in &shell_b.faces {
            let curves = face_face_intersection(face_a, &shell_a, face_b, &shell_b, &mut events);
            segments.extend(curves);
        }
    }

    assert!(
        segments.len() >= 8,
        "expected >= 8 intersection segments, got {}",
        segments.len()
    );
}

// --- Ray-cast inside/outside classification ---

#[test]
fn point_inside_box_shell() {
    let shell = shell_from_box_at([0.0, 0.0, 0.0], 2.0, 2.0, 2.0);
    let inside = [1.0, 1.0, 1.0];
    let outside = [5.0, 5.0, 5.0];

    assert!(point_in_shell(&inside, &shell), "center should be inside");
    assert!(
        !point_in_shell(&outside, &shell),
        "far point should be outside"
    );
}

#[test]
fn point_on_face_of_box() {
    let shell = shell_from_box_at([0.0, 0.0, 0.0], 2.0, 2.0, 2.0);
    let on_face = [2.0, 1.0, 1.0];
    assert!(
        !point_in_shell(&on_face, &shell),
        "point on face should be outside"
    );
}

// --- face_polygon + SubFace ---

#[test]
fn face_polygon_extracts_vertices() {
    let shell = shell_from_box_at([0.0, 0.0, 0.0], 1.0, 1.0, 1.0);
    let poly = face_polygon(&shell.faces[0], &shell);
    assert_eq!(poly.len(), 4, "box face should have 4 vertices");
}

#[test]
fn subface_from_unsplit_face() {
    let shell = shell_from_box_at([0.0, 0.0, 0.0], 1.0, 1.0, 1.0);
    let subs = split_face(&shell.faces[0], &shell, &[], 0, 0);
    assert_eq!(subs.len(), 1);
    assert_eq!(subs[0].polygon.len(), 4);
}

// --- Polygon line splitting ---

#[test]
fn split_rectangular_face_by_line() {
    let shell = shell_from_box_at([0.0, 0.0, 0.0], 1.0, 1.0, 1.0);
    let cut = Curve3D::Line {
        start: [0.0, 0.5, 0.0],
        end: [1.0, 0.5, 0.0],
    };
    let subs = split_face(&shell.faces[0], &shell, &[cut], 0, 0);
    assert_eq!(subs.len(), 2, "should split into 2 SubFaces");
    let total_area: f64 = subs.iter().map(|sf| polygon_area_3d(&sf.polygon)).sum();
    for sf in &subs {
        assert!(
            sf.polygon.len() >= 4,
            "each half should retain a valid polygon after split"
        );
        assert!(
            polygon_area_3d(&sf.polygon) > 0.0,
            "each half should have positive area"
        );
    }
    assert!((total_area - 1.0).abs() < 1e-9, "split must preserve area");
}

#[test]
fn split_face_by_two_cuts() {
    let shell = shell_from_box_at([0.0, 0.0, 0.0], 1.0, 1.0, 1.0);
    let cut1 = Curve3D::Line {
        start: [0.0, 0.3, 0.0],
        end: [1.0, 0.3, 0.0],
    };
    let cut2 = Curve3D::Line {
        start: [0.0, 0.7, 0.0],
        end: [1.0, 0.7, 0.0],
    };
    let subs = split_face(&shell.faces[0], &shell, &[cut1, cut2], 0, 0);
    assert_eq!(subs.len(), 3, "2 parallel cuts should yield 3 SubFaces");
}

// --- Six-plane clipping of a whole mesh ---
// NOTE: `clip_mesh_subtract_box` requires `Vec<[f64; 3]>` nodes instead of `Point3`.
// generate_box_mesh is not available in neco-brep (it's in mfp-types).
// These tests are marked #[ignore].

#[test]
#[ignore = "generate_box_mesh (mfp-types) does not exist in neco-brep"]
fn clip_mesh_subtract_box_removes_interior() {}

#[test]
#[ignore = "generate_box_mesh (mfp-types) does not exist in neco-brep"]
fn clip_mesh_volume_conservation() {}

// --- Curve3D::Ellipse tests ---

#[test]
fn ellipse_evaluate_circle() {
    let ellipse = Curve3D::Ellipse {
        center: [0.0, 0.0, 0.0],
        axis_u: [1.0, 0.0, 0.0],
        axis_v: [0.0, 1.0, 0.0],
        t_start: 0.0,
        t_end: std::f64::consts::TAU,
    };
    let p0 = ellipse.evaluate(0.0);
    assert!((p0[0] - 1.0).abs() < 1e-12);
    assert!(p0[1].abs() < 1e-12);
    let p_half_pi = ellipse.evaluate(std::f64::consts::FRAC_PI_2);
    assert!(p_half_pi[0].abs() < 1e-12);
    assert!((p_half_pi[1] - 1.0).abs() < 1e-12);
}

#[test]
fn ellipse_evaluate_non_circular() {
    let ellipse = Curve3D::Ellipse {
        center: [0.0, 0.0, 0.0],
        axis_u: [2.0, 0.0, 0.0],
        axis_v: [0.0, 1.0, 0.0],
        t_start: 0.0,
        t_end: std::f64::consts::TAU,
    };
    let p0 = ellipse.evaluate(0.0);
    assert!((p0[0] - 2.0).abs() < 1e-12);
    let p_half_pi = ellipse.evaluate(std::f64::consts::FRAC_PI_2);
    assert!((p_half_pi[1] - 1.0).abs() < 1e-12);
}

#[test]
fn ellipse_to_polyline_adapts() {
    let circle = Curve3D::Ellipse {
        center: [0.0, 0.0, 0.0],
        axis_u: [1.0, 0.0, 0.0],
        axis_v: [0.0, 1.0, 0.0],
        t_start: 0.0,
        t_end: std::f64::consts::TAU,
    };
    let coarse = circle.to_polyline(0.1);
    let fine = circle.to_polyline(0.01);
    assert!(
        coarse.len() >= 4,
        "coarse sampling should still produce at least 4 points"
    );
    assert!(
        fine.len() > coarse.len(),
        "finer tolerance should produce more sample points"
    );
}

#[test]
#[ignore = "generate_box_mesh (mfp-types) does not exist in neco-brep"]
fn clip_mesh_no_cracks() {}

// --- Face selection ---

#[test]
fn select_faces_subtract() {
    use neco_brep::boolean3d::combine3d::select_faces;

    let shell_a = shell_from_box_at([0.0, 0.0, 0.0], 2.0, 2.0, 2.0);
    let shell_b = shell_from_box_at([0.5, 0.5, 0.5], 1.0, 1.0, 1.0);

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

    let selected = select_faces(&sub_a, &sub_b, &shell_a, &shell_b, BooleanOp::Subtract);
    assert_eq!(
        selected.len(),
        12,
        "subtract contained box should yield 12 faces, got {}",
        selected.len()
    );
}

#[test]
fn select_faces_identical_boxes_current_boundary_ownership() {
    use neco_brep::boolean3d::combine3d::select_faces;

    let shell_a = shell_from_box_at([0.0, 0.0, 0.0], 2.0, 2.0, 2.0);
    let shell_b = shell_from_box_at([0.0, 0.0, 0.0], 2.0, 2.0, 2.0);

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

    let union_selected = select_faces(&sub_a, &sub_b, &shell_a, &shell_b, BooleanOp::Union);
    let intersect_selected = select_faces(&sub_a, &sub_b, &shell_a, &shell_b, BooleanOp::Intersect);
    let subtract_selected = select_faces(&sub_a, &sub_b, &shell_a, &shell_b, BooleanOp::Subtract);

    assert_eq!(
        union_selected.len(),
        shell_a.faces.len(),
        "current union ownership keeps only A-side coincident boundary faces"
    );
    assert_eq!(
        intersect_selected.len(),
        shell_a.faces.len(),
        "current intersect ownership keeps only A-side coincident boundary faces"
    );
    assert!(
        subtract_selected.is_empty(),
        "current subtract ownership drops coincident same-direction boundaries from both sides"
    );
}

#[test]
fn select_faces_face_sharing_boxes_current_boundary_ownership() {
    use neco_brep::boolean3d::combine3d::select_faces;

    let shell_a = shell_from_box_at([0.0, 0.0, 0.0], 2.0, 2.0, 2.0);
    let shell_b = shell_from_box_at([2.0, 0.0, 0.0], 2.0, 2.0, 2.0);

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

    let union_selected = select_faces(&sub_a, &sub_b, &shell_a, &shell_b, BooleanOp::Union);
    let intersect_selected = select_faces(&sub_a, &sub_b, &shell_a, &shell_b, BooleanOp::Intersect);
    let subtract_selected = select_faces(&sub_a, &sub_b, &shell_a, &shell_b, BooleanOp::Subtract);

    assert_eq!(
        union_selected.len(),
        10,
        "current union ownership drops the shared opposite-direction face from both shells"
    );
    assert!(
        intersect_selected.is_empty(),
        "current intersect ownership drops face-sharing opposite-direction boundaries"
    );
    assert_eq!(
        subtract_selected.len(),
        6,
        "current subtract ownership keeps A-side opposite-direction boundary and drops B-side boundary"
    );
}

fn plane_subface_on_x_face(
    shell: &Shell,
    x: f64,
    y0: f64,
    y1: f64,
    z0: f64,
    z1: f64,
    source_shell: usize,
) -> neco_brep::SubFace {
    let (face_index, face) = shell
        .faces
        .iter()
        .enumerate()
        .find(|(_, face)| match &face.surface {
            neco_brep::Surface::Plane { normal, .. } => normal[0] > 0.9,
            _ => false,
        })
        .expect("positive-x plane face");

    neco_brep::SubFace {
        surface: face.surface.clone(),
        polygon: vec![[x, y0, z0], [x, y1, z0], [x, y1, z1], [x, y0, z1]],
        candidate_curves: Vec::new(),
        flipped: false,
        source_shell,
        source_face: face_index,
    }
}

#[test]
fn select_faces_same_direction_partial_overlap_is_not_forced_to_a_side() {
    use neco_brep::boolean3d::combine3d::select_faces;

    let shell_a = shell_from_box_at([0.0, 0.0, 0.0], 2.0, 2.0, 2.0);
    let shell_b = shell_from_box_at([0.0, 0.5, 0.0], 2.0, 2.0, 2.0);

    let sub_a = vec![plane_subface_on_x_face(
        &shell_a, 2.0, 0.5, 1.5, 0.0, 2.0, 0,
    )];
    let sub_b = vec![plane_subface_on_x_face(
        &shell_b, 2.0, 1.0, 2.0, 0.0, 2.0, 1,
    )];

    let union_selected = select_faces(&sub_a, &sub_b, &shell_a, &shell_b, BooleanOp::Union);
    let intersect_selected = select_faces(&sub_a, &sub_b, &shell_a, &shell_b, BooleanOp::Intersect);

    assert_eq!(
        union_selected.len(),
        2,
        "same-direction partial overlap without duplicate should keep both shells in union selection"
    );
    assert!(
        union_selected.iter().any(|sf| sf.source_shell == 0)
            && union_selected.iter().any(|sf| sf.source_shell == 1),
        "union selection should retain both A-side and B-side partially overlapping subfaces"
    );
    assert_eq!(
        intersect_selected.len(),
        2,
        "same-direction partial overlap without duplicate should not be collapsed into one owner in intersect selection"
    );
}

#[test]
fn select_faces_same_direction_edge_contact_is_not_collapsed() {
    use neco_brep::boolean3d::combine3d::select_faces;

    let shell_a = shell_from_box_at([0.0, 0.0, 0.0], 2.0, 2.0, 2.0);
    let shell_b = shell_from_box_at([0.0, 1.0, 0.0], 2.0, 2.0, 2.0);

    let sub_a = vec![plane_subface_on_x_face(
        &shell_a, 2.0, 0.5, 1.5, 0.0, 2.0, 0,
    )];
    let sub_b = vec![plane_subface_on_x_face(
        &shell_b, 2.0, 1.5, 2.0, 0.0, 2.0, 1,
    )];

    let union_selected = select_faces(&sub_a, &sub_b, &shell_a, &shell_b, BooleanOp::Union);
    let intersect_selected = select_faces(&sub_a, &sub_b, &shell_a, &shell_b, BooleanOp::Intersect);

    assert_eq!(
        union_selected.len(),
        2,
        "same-direction edge contact keeps both shells in union selection"
    );
    assert!(
        union_selected.iter().any(|sf| sf.source_shell == 0)
            && union_selected.iter().any(|sf| sf.source_shell == 1),
        "union selection should retain both A-side and B-side edge-contact subfaces"
    );
    assert_eq!(
        intersect_selected.len(),
        2,
        "same-direction edge contact keeps both shells in intersect selection"
    );
}

// --- `boolean_3d()` orchestrator ---

#[test]
fn boolean3d_subtract_overlapping_boxes() {
    use neco_brep::boolean_3d;

    let a = shell_from_box_at([0.0, 0.0, 0.0], 2.0, 1.0, 1.0);
    let b = shell_from_box_at([1.0, 0.0, 0.0], 2.0, 1.0, 1.0);
    let result = boolean_3d(&a, &b, BooleanOp::Subtract).unwrap();
    assert!(
        result.faces.len() >= 6,
        "subtract overlapping boxes should have >= 6 faces, got {}",
        result.faces.len()
    );
}

#[test]
fn boolean3d_union_overlapping_boxes() {
    use neco_brep::boolean_3d;

    let a = shell_from_box_at([0.0, 0.0, 0.0], 2.0, 1.0, 1.0);
    let b = shell_from_box_at([1.0, 0.0, 0.0], 2.0, 1.0, 1.0);
    let result = boolean_3d(&a, &b, BooleanOp::Union).unwrap();
    assert!(
        result.faces.len() >= 6,
        "union overlapping boxes should have >= 6 faces, got {}",
        result.faces.len()
    );
}

#[test]
fn boolean3d_intersect_overlapping_boxes() {
    use neco_brep::boolean_3d;

    let a = shell_from_box_at([0.0, 0.0, 0.0], 2.0, 1.0, 1.0);
    let b = shell_from_box_at([1.0, 0.0, 0.0], 2.0, 1.0, 1.0);
    let result = boolean_3d(&a, &b, BooleanOp::Intersect).unwrap();
    assert!(
        result.faces.len() >= 6,
        "intersect overlapping boxes should have >= 6 faces, got {}",
        result.faces.len()
    );
}

#[test]
fn boolean3d_subtract_contained_box() {
    use neco_brep::boolean_3d;

    let a = shell_from_box_at([0.0, 0.0, 0.0], 3.0, 3.0, 3.0);
    let b = shell_from_box_at([1.0, 1.0, 1.0], 1.0, 1.0, 1.0);
    let result = boolean_3d(&a, &b, BooleanOp::Subtract).unwrap();
    assert_eq!(
        result.faces.len(),
        12,
        "subtract contained box should yield 12 faces, got {}",
        result.faces.len()
    );
}

#[test]
fn boolean3d_disjoint_boxes_union_error() {
    use neco_brep::boolean_3d;

    let a = shell_from_box_at([0.0, 0.0, 0.0], 1.0, 1.0, 1.0);
    let b = shell_from_box_at([5.0, 5.0, 5.0], 1.0, 1.0, 1.0);
    assert!(
        boolean_3d(&a, &b, BooleanOp::Union).is_err(),
        "disjoint boxes union should error"
    );
}

#[test]
fn boolean3d_disjoint_boxes_intersect_returns_empty_shell() {
    use neco_brep::boolean_3d;

    let a = shell_from_box_at([0.0, 0.0, 0.0], 1.0, 1.0, 1.0);
    let b = shell_from_box_at([5.0, 5.0, 5.0], 1.0, 1.0, 1.0);
    let result = boolean_3d(&a, &b, BooleanOp::Intersect)
        .expect("disjoint boxes intersect should return an empty shell");
    assert!(
        result.faces.is_empty(),
        "disjoint intersect should be empty"
    );
    assert!(
        result.edges.is_empty(),
        "disjoint intersect should be empty"
    );
    assert!(
        result.vertices.is_empty(),
        "disjoint intersect should be empty"
    );
}

#[test]
fn boolean3d_edge_edge_contact_intersect_returns_empty_shell() {
    use neco_brep::boolean_3d;

    let a = shell_from_box_at([0.0, 0.0, 0.0], 1.0, 1.0, 1.0);
    let b = shell_from_box_at([1.0, 1.0, 0.0], 1.0, 1.0, 1.0);
    let result = boolean_3d(&a, &b, BooleanOp::Intersect)
        .expect("edge-edge contact intersect should return an empty shell");
    assert!(
        result.faces.is_empty(),
        "edge-edge contact intersect should be empty"
    );
}

#[test]
fn boolean3d_vertex_face_contact_intersect_returns_empty_shell() {
    use neco_brep::boolean_3d;

    let a = shell_from_box_at([0.0, 0.0, 0.0], 1.0, 1.0, 1.0);
    let b = shell_from_box_at([1.0, 0.25, 0.25], 0.5, 0.5, 0.5);
    let result = boolean_3d(&a, &b, BooleanOp::Intersect)
        .expect("vertex-face contact intersect should return an empty shell");
    assert!(
        result.faces.is_empty(),
        "vertex-face contact intersect should be empty"
    );
}

#[test]
fn boolean3d_face_sharing_boxes_subtract_keeps_minuend() {
    use neco_brep::boolean_3d;

    let a = shell_from_box_at([0.0, 0.0, 0.0], 1.0, 1.0, 1.0);
    let b = shell_from_box_at([1.0, 0.0, 0.0], 1.0, 1.0, 1.0);
    let result = boolean_3d(&a, &b, BooleanOp::Subtract)
        .expect("face-sharing box subtract should keep the minuend unchanged");
    assert_shell_matches_exactly(&result, &a);
}

#[test]
fn boolean3d_edge_edge_contact_subtract_keeps_minuend() {
    use neco_brep::boolean_3d;

    let a = shell_from_box_at([0.0, 0.0, 0.0], 1.0, 1.0, 1.0);
    let b = shell_from_box_at([1.0, 1.0, 0.0], 1.0, 1.0, 1.0);
    let result = boolean_3d(&a, &b, BooleanOp::Subtract)
        .expect("edge-edge contact subtract should keep the minuend unchanged");
    assert_shell_matches_exactly(&result, &a);
}

#[test]
fn boolean3d_vertex_face_contact_subtract_keeps_minuend() {
    use neco_brep::boolean_3d;

    let a = shell_from_box_at([0.0, 0.0, 0.0], 1.0, 1.0, 1.0);
    let b = shell_from_box_at([1.0, 0.25, 0.25], 0.5, 0.5, 0.5);
    let result = boolean_3d(&a, &b, BooleanOp::Subtract)
        .expect("vertex-face contact subtract should keep the minuend unchanged");
    assert_shell_matches_exactly(&result, &a);
}

// --- Shell::bounding_box ---

#[test]
fn shell_bounding_box() {
    let shell = shell_from_box_at([1.0, 2.0, 3.0], 4.0, 5.0, 6.0);
    let (min, max) = shell.bounding_box();
    assert!((min[0] - 1.0).abs() < 1e-10);
    assert!((min[1] - 2.0).abs() < 1e-10);
    assert!((min[2] - 3.0).abs() < 1e-10);
    assert!((max[0] - 5.0).abs() < 1e-10);
    assert!((max[1] - 7.0).abs() < 1e-10);
    assert!((max[2] - 9.0).abs() < 1e-10);
}

// --- shell_to_immersed_mesh / shell_to_clipped_mesh / boolean_mesh ---
// These depend on mfp-types (generate_box_mesh, Mesh3D) and are not available in neco-brep.

#[test]
#[ignore = "shell_to_immersed_mesh does not exist in neco-brep (mfp-geo specific)"]
fn shell_to_immersed_mesh_unit_box() {}

// `boolean3d_subtract_to_mesh` moved to `tests/boolean_tessellation.rs`.

// --- shell_from_extrude ---

fn make_rect_region_2d(lx: f64, lz: f64) -> NurbsRegion {
    let pts = vec![[0.0, 0.0], [lx, 0.0], [lx, lz], [0.0, lz], [0.0, 0.0]];
    let n = pts.len();
    NurbsRegion {
        outer: vec![NurbsCurve2D {
            degree: 1,
            control_points: pts,
            weights: vec![1.0; n],
            knots: vec![0.0, 0.0, 1.0, 2.0, 3.0, 4.0, 4.0],
        }],
        holes: vec![],
    }
}

#[test]
fn from_extrude_rect_topology() {
    let profile = make_rect_region_2d(1.0, 1.0);
    let shell = shell_from_extrude(&profile, [0.0, 0.0, 1.0], 2.0).unwrap();
    assert_eq!(shell.faces.len(), 6);
    assert_eq!(shell.vertices.len(), 8);
}

#[test]
fn from_extrude_triangle_topology() {
    let pts = vec![[0.0, 0.0], [1.0, 0.0], [0.5, 1.0], [0.0, 0.0]];
    let region = NurbsRegion {
        outer: vec![NurbsCurve2D {
            degree: 1,
            control_points: pts,
            weights: vec![1.0; 4],
            knots: vec![0.0, 0.0, 1.0, 2.0, 3.0, 3.0],
        }],
        holes: vec![],
    };
    let shell = shell_from_extrude(&region, [0.0, 0.0, 1.0], 1.0).unwrap();
    assert_eq!(shell.faces.len(), 5);
    assert_eq!(shell.vertices.len(), 6);
}

#[test]
fn from_extrude_normals_outward() {
    let profile = make_rect_region_2d(1.0, 1.0);
    let shell = shell_from_extrude(&profile, [0.0, 0.0, 1.0], 1.0).unwrap();
    let (bb_min, bb_max) = shell.bounding_box();
    let center = [
        (bb_min[0] + bb_max[0]) / 2.0,
        (bb_min[1] + bb_max[1]) / 2.0,
        (bb_min[2] + bb_max[2]) / 2.0,
    ];
    for face in &shell.faces {
        if let Surface::Plane { origin, normal } = &face.surface {
            let to_center = vec3::sub(center, *origin);
            assert!(
                vec3::dot(*normal, to_center) <= 1e-10,
                "normal points inward: normal={:?}",
                normal
            );
        }
    }
}

#[test]
fn from_extrude_point_in_shell() {
    let profile = make_rect_region_2d(1.0, 1.0);
    let shell = shell_from_extrude(&profile, [0.0, 0.0, 1.0], 1.0).unwrap();
    assert!(point_in_shell(&[0.5, 0.5, 0.5], &shell));
    assert!(!point_in_shell(&[2.0, 0.5, 0.5], &shell));
}

// --- shell_from_revolve ---

fn make_revolve_rect_profile() -> NurbsRegion {
    let pts = vec![[0.5, 0.0], [1.0, 0.0], [1.0, 1.0], [0.5, 1.0], [0.5, 0.0]];
    NurbsRegion {
        outer: vec![NurbsCurve2D {
            degree: 1,
            control_points: pts,
            weights: vec![1.0; 5],
            knots: vec![0.0, 0.0, 1.0, 2.0, 3.0, 4.0, 4.0],
        }],
        holes: vec![],
    }
}

#[test]
fn from_revolve_full_rotation() {
    let region = make_revolve_rect_profile();
    let shell = shell_from_revolve(&region, Axis::Y, Radians::from_degrees(360.0)).unwrap();
    assert_eq!(
        shell.faces.len(),
        4,
        "expected 4 analytic-surface faces, got {}",
        shell.faces.len()
    );
    let n_cyl = shell
        .faces
        .iter()
        .filter(|f| matches!(f.surface, Surface::Cylinder { .. }))
        .count();
    let n_plane = shell
        .faces
        .iter()
        .filter(|f| matches!(f.surface, Surface::Plane { .. }))
        .count();
    assert_eq!(n_cyl, 2, "expected 2 cylinder faces (inner + outer)");
    assert_eq!(n_plane, 2, "expected 2 plane faces (top + bottom)");
}

#[test]
fn from_revolve_partial_has_caps() {
    let region = make_revolve_rect_profile();
    let shell = shell_from_revolve(&region, Axis::Y, Radians::from_degrees(180.0)).unwrap();
    assert_eq!(
        shell.faces.len(),
        6,
        "partial revolve should have 4 curved faces plus 2 caps = 6 faces, got {}",
        shell.faces.len()
    );
    let full_shell = shell_from_revolve(&region, Axis::Y, Radians::from_degrees(360.0)).unwrap();
    assert!(
        shell.faces.len() > full_shell.faces.len(),
        "partial revolve should have more faces than full revolve because of cap faces: partial={}, full={}",
        shell.faces.len(),
        full_shell.faces.len()
    );
}

#[test]
fn from_revolve_normals_outward() {
    let region = make_revolve_rect_profile();
    let shell = shell_from_revolve(&region, Axis::Y, Radians::from_degrees(360.0)).unwrap();
    assert!(!shell.faces.is_empty());
    for face in &shell.faces {
        if let Surface::Plane { normal, .. } = &face.surface {
            assert!(
                normal[1].abs() > 0.9,
                "plane face normal should align with the Y axis: {:?}",
                normal
            );
        }
    }
    for face in &shell.faces {
        if let Surface::Cylinder { axis, .. } = &face.surface {
            assert!(
                axis[1].abs() > 0.9,
                "cylinder axis should align with Y: {:?}",
                axis
            );
        }
    }
}

// --- End-to-end: Extrude/Revolve boolean ---

#[test]
fn extrude_vs_box_subtract() {
    use neco_brep::boolean_3d;

    let profile = make_rect_region_2d(3.0, 2.0);
    let shell_a = shell_from_extrude(&profile, [0.0, 0.0, 1.0], 2.0).unwrap();
    let shell_b = shell_from_box_at([0.5, 0.5, 0.5], 1.0, 1.0, 1.0);
    let result = boolean_3d(&shell_a, &shell_b, BooleanOp::Subtract).unwrap();
    assert!(
        result.faces.len() >= 6,
        "result should contain at least 6 faces, got {}",
        result.faces.len()
    );
}

#[test]
fn extrude_vs_extrude_intersect() {
    use neco_brep::boolean_3d;

    let profile_a = make_rect_region_2d(3.0, 2.0);
    let shell_a = shell_from_extrude(&profile_a, [0.0, 0.0, 1.0], 2.0).unwrap();
    let shell_b = shell_from_box_at([0.5, 0.5, 0.5], 1.0, 1.0, 1.0);
    let result = boolean_3d(&shell_a, &shell_b, BooleanOp::Intersect).unwrap();
    assert!(result.faces.len() >= 6);
}

#[test]
fn revolve_point_in_shell_works() {
    let region = make_revolve_rect_profile();
    let shell = shell_from_revolve(&region, Axis::Y, Radians::from_degrees(360.0)).unwrap();

    assert_eq!(shell.faces.len(), 4);
    let n_arc = shell
        .edges
        .iter()
        .filter(|e| matches!(e.curve, Curve3D::Arc { .. }))
        .count();
    assert_eq!(n_arc, 8, "expected 8 arc edges, got {}", n_arc);
}

#[test]
fn revolve_rect_profile_cylinder_faces() {
    let profile = make_revolve_rect_profile();
    let shell = shell_from_revolve(&profile, Axis::Y, Radians::from_degrees(360.0)).unwrap();

    let n_cyl = shell
        .faces
        .iter()
        .filter(|f| matches!(f.surface, Surface::Cylinder { .. }))
        .count();
    let n_plane = shell
        .faces
        .iter()
        .filter(|f| matches!(f.surface, Surface::Plane { .. }))
        .count();
    assert_eq!(n_cyl, 2, "expected 2 cylinder faces (inner + outer)");
    assert_eq!(n_plane, 2, "expected 2 plane faces (top + bottom)");

    let n_arc = shell
        .edges
        .iter()
        .filter(|e| matches!(e.curve, Curve3D::Arc { .. }))
        .count();
    assert!(n_arc >= 4, "expected at least 4 arc edges");

    for face in &shell.faces {
        if let Surface::Cylinder { radius, .. } = &face.surface {
            assert!(
                (*radius - 0.5).abs() < 1e-10 || (*radius - 1.0).abs() < 1e-10,
                "cylinder radius should be either 0.5 or 1.0, got {}",
                radius
            );
        }
    }
}

#[test]
fn box_subtract_revolve_to_mesh() {
    // Validate only the boolean result. Mesh generation depends on
    // `shell_to_immersed_mesh`, which does not exist in neco-brep.
    use neco_brep::boolean_3d;

    let shell_a = shell_from_box_at([-1.0, -1.0, -1.0], 2.0, 2.0, 2.0);
    let region = make_revolve_rect_profile();
    let shell_b = shell_from_revolve(&region, Axis::Y, Radians::from_degrees(360.0)).unwrap();

    let result = boolean_3d(&shell_a, &shell_b, BooleanOp::Subtract).unwrap();
    assert!(
        result.faces.len() > 6,
        "box-minus-cylinder result produced {} faces",
        result.faces.len()
    );
}

// --- Geometry3D / shell_from_geometry3d ---
// These depend on mfp-geo's Geometry3D/BooleanOperand/Placement types.

#[test]
#[ignore = "Geometry3D does not exist in neco-brep (mfp-geo specific)"]
fn shell_from_geometry3d_extrude() {}

#[test]
fn lshape_extrude_concavity_outside() {
    let pts = vec![
        [0.0, 0.0],
        [1.0, 0.0],
        [1.0, 0.3],
        [0.3, 0.3],
        [0.3, 1.0],
        [0.0, 1.0],
        [0.0, 0.0],
    ];
    let region = NurbsRegion {
        outer: vec![NurbsCurve2D {
            degree: 1,
            control_points: pts,
            weights: vec![1.0; 7],
            knots: vec![0.0, 0.0, 1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 6.0],
        }],
        holes: vec![],
    };
    let shell = shell_from_extrude(&region, [0.0, 0.0, 1.0], 0.5).unwrap();
    assert!(
        point_in_shell(&[0.15, 0.15, 0.25], &shell),
        "lower branch of the L-shape should be inside"
    );
    assert!(
        point_in_shell(&[0.15, 0.65, 0.25], &shell),
        "left branch of the L-shape should be inside"
    );
    assert!(
        !point_in_shell(&[0.65, 0.65, 0.25], &shell),
        "L-shape notch at (0.65, 0.65, 0.25) should be outside"
    );
    assert!(
        !point_in_shell(&[0.5, 0.5, 0.25], &shell),
        "L-shape notch at (0.5, 0.5, 0.25) should be outside"
    );
}

#[test]
#[ignore = "Geometry3D/BooleanOperand/Placement do not exist in neco-brep (mfp-geo specific)"]
fn lshape_extrude_boolean_subtract_mesh() {}

// --- predicates ---

#[test]
#[ignore = "predicates module does not exist in neco-brep (mfp-geo specific)"]
fn orient2d_counterclockwise() {}

#[test]
#[ignore = "predicates module does not exist in neco-brep (mfp-geo specific)"]
fn orient2d_collinear() {}

#[test]
#[ignore = "predicates module does not exist in neco-brep (mfp-geo specific)"]
fn orient3d_above_plane() {}

#[test]
#[ignore = "predicates module does not exist in neco-brep (mfp-geo specific)"]
fn orient3d_on_plane() {}

#[test]
fn point_in_shell_on_vertex() {
    let shell = shell_from_box_at([0.0, 0.0, 0.0], 1.0, 1.0, 1.0);
    let near_vertex = [1e-15, 1e-15, 1e-15];
    let _ = point_in_shell(&near_vertex, &shell);
}

#[test]
fn point_in_shell_on_face() {
    let shell = shell_from_box_at([0.0, 0.0, 0.0], 1.0, 1.0, 1.0);
    let on_face = [0.5, 0.0, 0.5];
    let _ = point_in_shell(&on_face, &shell);
}

// --- `tet_clip` tests ---

#[test]
fn clip_tet_all_positive() {
    use neco_brep::boolean3d::tet_clip::*;
    let nodes = vec![
        [0.0, 0.0, 0.0],
        [1.0, 0.0, 0.0],
        [0.0, 1.0, 0.0],
        [0.0, 0.0, 1.0],
    ];
    let mut ws = TetClipWorkspace::new(nodes, vec![[0, 1, 2, 3]]);
    let plane = ClipPlane::from_origin_normal([0.0, 0.0, -1.0], [0.0, 0.0, 1.0]);
    let (pos, neg) = clip_tet(&mut ws, [0, 1, 2, 3], &plane);
    assert_eq!(pos.len(), 1);
    assert_eq!(neg.len(), 0);
}

#[test]
fn clip_tet_1_3_split() {
    use neco_brep::boolean3d::tet_clip::*;
    let nodes = vec![
        [0.0, 0.0, 0.0],
        [1.0, 0.0, 0.0],
        [0.0, 1.0, 0.0],
        [0.0, 0.0, 1.0],
    ];
    let mut ws = TetClipWorkspace::new(nodes, vec![[0, 1, 2, 3]]);
    let plane = ClipPlane::from_origin_normal([0.0, 0.0, 0.5], [0.0, 0.0, 1.0]);
    let (pos, neg) = clip_tet(&mut ws, [0, 1, 2, 3], &plane);
    assert_eq!(pos.len(), 1);
    assert_eq!(neg.len(), 3);
    let vol_orig = tet_volume(&ws.nodes, &[0, 1, 2, 3]);
    let vol_pos: f64 = pos.iter().map(|t| tet_volume(&ws.nodes, t)).sum();
    let vol_neg: f64 = neg.iter().map(|t| tet_volume(&ws.nodes, t)).sum();
    assert!(
        (vol_orig - vol_pos - vol_neg).abs() < 1e-12,
        "volume is not conserved: orig={vol_orig}, pos={vol_pos}, neg={vol_neg}"
    );
}

#[test]
fn clip_tet_2_2_split() {
    use neco_brep::boolean3d::tet_clip::*;
    let nodes = vec![
        [0.0, 0.0, 0.0],
        [1.0, 0.0, 0.0],
        [0.0, 1.0, 0.5],
        [0.0, 0.0, 1.0],
    ];
    let mut ws = TetClipWorkspace::new(nodes, vec![[0, 1, 2, 3]]);
    let plane = ClipPlane::from_origin_normal([0.0, 0.0, 0.3], [0.0, 0.0, 1.0]);
    let (pos, neg) = clip_tet(&mut ws, [0, 1, 2, 3], &plane);
    assert!(pos.len() >= 2);
    assert!(neg.len() >= 2);
    let vol_orig = tet_volume(&ws.nodes, &[0, 1, 2, 3]);
    let vol_pos: f64 = pos.iter().map(|t| tet_volume(&ws.nodes, t)).sum();
    let vol_neg: f64 = neg.iter().map(|t| tet_volume(&ws.nodes, t)).sum();
    assert!(
        (vol_orig - vol_pos - vol_neg).abs() < 1e-12,
        "volume is not conserved: orig={vol_orig}, pos={vol_pos}, neg={vol_neg}"
    );
}

#[test]
fn clip_tet_vertex_on_plane() {
    use neco_brep::boolean3d::tet_clip::*;
    let nodes = vec![
        [0.0, 0.0, 0.0],
        [1.0, 0.0, 0.0],
        [0.0, 1.0, 0.0],
        [0.0, 0.0, 0.5],
    ];
    let mut ws = TetClipWorkspace::new(nodes, vec![[0, 1, 2, 3]]);
    let plane = ClipPlane::from_origin_normal([0.0, 0.0, 0.5], [0.0, 0.0, 1.0]);
    let (pos, neg) = clip_tet(&mut ws, [0, 1, 2, 3], &plane);
    assert_eq!(pos.len(), 0);
    assert_eq!(neg.len(), 1);
}

#[test]
#[ignore = "generate_box_mesh (mfp-types) does not exist in neco-brep"]
fn flip_does_not_worsen_quality() {}

#[test]
#[ignore = "generate_box_mesh (mfp-types) does not exist in neco-brep"]
fn smoothing_preserves_boundary() {}

// --- shell_to_clipped_mesh / boolean_mesh ---

#[test]
#[ignore = "shell_to_clipped_mesh (mfp-geo specific) does not exist in neco-brep"]
fn clipped_mesh_box_subtract_sharp_edges() {}

#[test]
#[ignore = "shell_to_clipped_mesh (mfp-geo specific) does not exist in neco-brep"]
fn clipped_vs_immersed_volume_comparison() {}

#[test]
#[ignore = "boolean_mesh (mfp-geo specific) does not exist in neco-brep"]
fn boolean_mesh_selects_clipped_for_box() {}

// --- `point_in_shell`: cylinder/cone coverage ---

#[test]
fn point_in_shell_cylinder_full_rotation() {
    let region = make_revolve_rect_profile();
    let shell = shell_from_revolve(&region, Axis::Y, Radians::from_degrees(360.0)).unwrap();

    let inside = [0.75, 0.5, 0.0];
    assert!(
        point_in_shell(&inside, &shell),
        "point inside the wall should be inside"
    );

    let inside2 = [0.0, 0.5, 0.75];
    assert!(
        point_in_shell(&inside2, &shell),
        "point inside the wall along z should be inside"
    );

    let outside = [1.5, 0.5, 0.0];
    assert!(
        !point_in_shell(&outside, &shell),
        "point outside the outer cylinder should be outside"
    );

    let in_hole = [0.25, 0.5, 0.0];
    assert!(
        !point_in_shell(&in_hole, &shell),
        "point inside the inner hole should be outside"
    );

    let above = [0.75, 1.5, 0.0];
    assert!(
        !point_in_shell(&above, &shell),
        "point above the shell should be outside"
    );

    let below = [0.75, -0.5, 0.0];
    assert!(
        !point_in_shell(&below, &shell),
        "point below the shell should be outside"
    );
}

#[test]
fn point_in_shell_cylinder_partial_rotation() {
    let region = make_revolve_rect_profile();
    let shell = shell_from_revolve(&region, Axis::Y, Radians::from_degrees(90.0)).unwrap();

    let angle = std::f64::consts::FRAC_PI_4;
    let r = 0.75;
    let inside = [r * angle.cos(), 0.5, r * angle.sin()];
    assert!(
        point_in_shell(&inside, &shell),
        "point inside the 90-degree revolve wall should be inside"
    );

    let above = [0.75, 2.0, 0.0];
    assert!(
        !point_in_shell(&above, &shell),
        "point above the shell should be outside"
    );
}

// --- Integration tests ---

#[test]
#[ignore = "shell_to_immersed_mesh does not exist in neco-brep (mfp-geo specific)"]
fn immersed_mesh_from_cylinder_shell() {}

#[test]
fn face_face_intersection_plane_cylinder() {
    let box_shell = shell_from_box_at([-1.0, -1.0, -1.0], 2.0, 2.0, 2.0);
    let region = make_revolve_rect_profile();
    let cyl_shell = shell_from_revolve(&region, Axis::Y, Radians::from_degrees(360.0)).unwrap();

    let mut total_curves = 0;
    let mut events = Vec::new();
    for fa in &box_shell.faces {
        for fb in &cyl_shell.faces {
            let curves = face_face_intersection(fa, &box_shell, fb, &cyl_shell, &mut events);
            total_curves += curves.len();
        }
    }
    assert!(
        total_curves > 0,
        "no Plane-Cylinder intersection curves were produced"
    );
}

// --- NurbsSurface tests ---

#[test]
fn nurbs_surface_evaluate_plane() {
    use neco_nurbs::NurbsSurface3D;
    let surf = NurbsSurface3D {
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
    let p = surf.evaluate(0.5, 0.5);
    assert!((p[0] - 0.5).abs() < 1e-10);
    assert!(p[1].abs() < 1e-10);
    assert!((p[2] - 0.5).abs() < 1e-10);

    let p00 = surf.evaluate(0.0, 0.0);
    assert!(p00[0].abs() < 1e-10);
    assert!(p00[2].abs() < 1e-10);

    let p11 = surf.evaluate(1.0, 1.0);
    assert!((p11[0] - 1.0).abs() < 1e-10);
    assert!((p11[2] - 1.0).abs() < 1e-10);
}

#[test]
fn nurbs_surface_normal_plane() {
    use neco_nurbs::NurbsSurface3D;
    let surf = NurbsSurface3D {
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
    let n = surf.normal(0.5, 0.5);
    assert!(
        (n[1].abs() - 1.0).abs() < 1e-4,
        "normal should align with Y: {:?}",
        n
    );
}

#[test]
fn nurbs_surface_evaluate_cylinder_quarter() {
    use neco_nurbs::NurbsSurface3D;
    let w = std::f64::consts::FRAC_1_SQRT_2;
    let surf = NurbsSurface3D {
        degree_u: 2,
        degree_v: 1,
        control_points: vec![
            vec![[1.0, 0.0, 0.0], [1.0, 1.0, 0.0]],
            vec![[1.0, 0.0, 1.0], [1.0, 1.0, 1.0]],
            vec![[0.0, 0.0, 1.0], [0.0, 1.0, 1.0]],
        ],
        weights: vec![vec![1.0, 1.0], vec![w, w], vec![1.0, 1.0]],
        knots_u: vec![0.0, 0.0, 0.0, 1.0, 1.0, 1.0],
        knots_v: vec![0.0, 0.0, 1.0, 1.0],
    };
    let p = surf.evaluate(0.5, 0.0);
    let r = (p[0] * p[0] + p[2] * p[2]).sqrt();
    assert!(
        (r - 1.0).abs() < 1e-6,
        "point should lie on the unit cylinder: r={r}"
    );
}

#[test]
fn nurbs_curve3d_evaluate_line() {
    let curve = Curve3D::NurbsCurve3D {
        degree: 1,
        control_points: vec![[0.0, 0.0, 0.0], [1.0, 1.0, 1.0]],
        weights: vec![1.0, 1.0],
        knots: vec![0.0, 0.0, 1.0, 1.0],
    };
    let mid = curve.evaluate(0.5);
    assert!((mid[0] - 0.5).abs() < 1e-10);
    assert!((mid[1] - 0.5).abs() < 1e-10);
    assert!((mid[2] - 0.5).abs() < 1e-10);
}

// --- Surface boolean integration tests ---

#[test]
fn boolean3d_box_subtract_sphere_e2e() {
    use neco_brep::boolean_3d;

    let box_shell = shell_from_box_at([0.0, 0.0, 0.0], 2.0, 2.0, 2.0);
    let sphere = {
        let s = shell_from_sphere(0.5);
        let m = [
            [1.0, 0.0, 0.0, 1.0],
            [0.0, 1.0, 0.0, 1.0],
            [0.0, 0.0, 1.0, 1.0],
            [0.0, 0.0, 0.0, 1.0],
        ];
        apply_transform(&s, &m)
    };
    // B-Rep boolean should not panic
    let _ = boolean_3d(&box_shell, &sphere, BooleanOp::Subtract);
}

#[test]
fn boolean3d_box_subtract_torus_e2e() {
    use neco_brep::boolean_3d;
    use neco_brep::shell_from_torus;

    let box_shell = shell_from_box_at([-2.0, -2.0, -2.0], 4.0, 4.0, 4.0);
    // In neco-brep, `shell_from_torus` is centered at the origin around the Z
    // axis. The original mfp-geo case used a Y-axis torus, but this test only
    // needs to ensure the operation does not panic.
    let torus = shell_from_torus(1.0, 0.3);
    let _ = boolean_3d(&box_shell, &torus, BooleanOp::Subtract);
}

#[test]
fn boolean3d_revolve_vs_revolve_perpendicular() {
    use neco_brep::boolean_3d;

    let profile = make_revolve_rect_profile();
    let cyl_y = shell_from_revolve(&profile, Axis::Y, Radians::from_degrees(360.0)).unwrap();

    let profile2 = {
        let pts = vec![[0.3, 0.0], [1.2, 0.0], [1.2, 1.0], [0.3, 1.0], [0.3, 0.0]];
        NurbsRegion {
            outer: vec![NurbsCurve2D {
                degree: 1,
                control_points: pts,
                weights: vec![1.0; 5],
                knots: vec![0.0, 0.0, 1.0, 2.0, 3.0, 4.0, 4.0],
            }],
            holes: vec![],
        }
    };
    let cyl_y2 = shell_from_revolve(&profile2, Axis::Y, Radians::from_degrees(360.0)).unwrap();

    // No panic is the primary requirement here.
    let _ = boolean_3d(&cyl_y, &cyl_y2, BooleanOp::Subtract);
}

#[test]
fn point_in_shell_cone_revolve() {
    let profile = make_revolve_rect_profile();
    let shell = shell_from_revolve(&profile, Axis::Y, Radians::from_degrees(360.0)).unwrap();
    let inside = [0.75, 0.5, 0.0];
    assert!(
        point_in_shell(&inside, &shell),
        "point should be inside the cylinder"
    );
    let outside = [1.5, 0.5, 0.0];
    assert!(
        !point_in_shell(&outside, &shell),
        "point should be outside the cylinder"
    );
    let in_hole = [0.25, 0.5, 0.0];
    assert!(
        !point_in_shell(&in_hole, &shell),
        "point inside the hole should be outside"
    );
}

#[test]
#[ignore = "shell_to_immersed_mesh does not exist in neco-brep (mfp-geo specific)"]
fn immersed_mesh_sphere_has_reasonable_volume() {}

#[test]
#[ignore = "generate_box_mesh (mfp-types) does not exist in neco-brep"]
fn tet_clip_quality_metrics() {}

#[test]
fn boolean3d_tangent_spheres() {
    use neco_brep::boolean_3d;

    let s1 = shell_from_sphere(1.0);
    let s2 = {
        let s = shell_from_sphere(1.0);
        let m = [
            [1.0, 0.0, 0.0, 2.0],
            [0.0, 1.0, 0.0, 0.0],
            [0.0, 0.0, 1.0, 0.0],
            [0.0, 0.0, 0.0, 1.0],
        ];
        apply_transform(&s, &m)
    };

    assert!(
        boolean_3d(&s1, &s2, BooleanOp::Union).is_err(),
        "tangent spheres union should follow disjoint-shell union semantics"
    );

    let intersect = boolean_3d(&s1, &s2, BooleanOp::Intersect)
        .expect("tangent spheres intersect should return an empty shell");
    assert!(
        intersect.faces.is_empty() && intersect.edges.is_empty() && intersect.vertices.is_empty(),
        "tangent spheres intersect should be empty"
    );

    let subtract = boolean_3d(&s1, &s2, BooleanOp::Subtract)
        .expect("tangent spheres subtract should keep the minuend unchanged");
    assert_shell_matches_exactly(&subtract, &s1);

    let reverse_subtract = boolean_3d(&s2, &s1, BooleanOp::Subtract)
        .expect("tangent spheres reverse subtract should keep the minuend unchanged");
    assert_shell_matches_exactly(&reverse_subtract, &s2);
}

#[test]
fn boolean3d_line_contact_sphere_in_torus_behaves_like_disjoint() {
    use neco_brep::boolean_3d;

    let torus = shell_from_torus(1.0, 0.3);
    let sphere = shell_from_sphere(0.7);

    assert!(
        boolean_3d(&sphere, &torus, BooleanOp::Union).is_err(),
        "line-contact sphere-torus union should follow disjoint-shell union semantics"
    );

    let intersect = boolean_3d(&sphere, &torus, BooleanOp::Intersect)
        .expect("line-contact sphere-torus intersect should return an empty shell");
    assert!(
        intersect.faces.is_empty() && intersect.edges.is_empty() && intersect.vertices.is_empty(),
        "line-contact sphere-torus intersect should be empty"
    );

    let sphere_subtract = boolean_3d(&sphere, &torus, BooleanOp::Subtract)
        .expect("line-contact sphere minus torus should keep the sphere unchanged");
    assert_shell_matches_exactly(&sphere_subtract, &sphere);

    let torus_subtract = boolean_3d(&torus, &sphere, BooleanOp::Subtract)
        .expect("line-contact torus minus sphere should keep the torus unchanged");
    assert_shell_matches_exactly(&torus_subtract, &torus);
}

#[test]
fn point_in_shell_thin_box_y_fallback() {
    let shell = shell_from_box_at([0.0, 0.0, 0.0], 0.001, 1.0, 1.0);
    let inside = [0.0005, 0.5, 0.5];
    assert!(
        point_in_shell(&inside, &shell),
        "point inside the thin box should be classified as inside"
    );
}

// --- insert_steiner_point tests ---

#[test]
fn insert_steiner_point_basic() {
    use neco_brep::boolean3d::tet_clip::insert_steiner_point;

    let mut nodes: Vec<[f64; 3]> = vec![
        [0.0, 0.0, 0.0],
        [1.0, 0.0, 0.0],
        [0.0, 1.0, 0.0],
        [0.0, 0.0, 1.0],
    ];
    let mut tets = vec![[0, 1, 2, 3]];
    let vol_before: f64 = tets.iter().map(|t| tet_volume(&nodes, t)).sum();

    let centroid = [0.25, 0.25, 0.25];
    let result = insert_steiner_point(&mut nodes, &mut tets, centroid);
    assert!(result.is_some(), "insertion should succeed");

    assert!(
        tets.len() >= 4,
        "should have at least 4 tets, got {}",
        tets.len()
    );

    let vol_after: f64 = tets.iter().map(|t| tet_volume(&nodes, t)).sum();
    assert!(
        (vol_before - vol_after).abs() < 1e-10,
        "volume not preserved: {vol_before} -> {vol_after}"
    );

    for (i, tet) in tets.iter().enumerate() {
        assert!(
            tet_volume(&nodes, tet) > 1e-30,
            "degenerate tet {i}: {:?}",
            tet
        );
    }
}

#[test]
fn insert_steiner_point_two_tets() {
    use neco_brep::boolean3d::tet_clip::insert_steiner_point;

    let mut nodes: Vec<[f64; 3]> = vec![
        [0.0, 0.0, 0.0],
        [1.0, 0.0, 0.0],
        [0.5, 1.0, 0.0],
        [0.5, 0.5, 1.0],
        [0.5, 0.5, -1.0],
    ];
    let mut tets = vec![[0, 1, 2, 3], [0, 1, 2, 4]];
    let vol_before: f64 = tets.iter().map(|t| tet_volume(&nodes, t)).sum();

    let p = [0.4, 0.3, 0.3];
    let result = insert_steiner_point(&mut nodes, &mut tets, p);
    assert!(result.is_some());

    let vol_after: f64 = tets.iter().map(|t| tet_volume(&nodes, t)).sum();
    assert!(
        (vol_before - vol_after).abs() < 1e-10,
        "volume not preserved: {vol_before} -> {vol_after}"
    );
}

#[test]
fn insert_steiner_point_outside_returns_none() {
    use neco_brep::boolean3d::tet_clip::insert_steiner_point;

    let mut nodes: Vec<[f64; 3]> = vec![
        [0.0, 0.0, 0.0],
        [1.0, 0.0, 0.0],
        [0.0, 1.0, 0.0],
        [0.0, 0.0, 1.0],
    ];
    let mut tets = vec![[0, 1, 2, 3]];
    let result = insert_steiner_point(&mut nodes, &mut tets, [5.0, 5.0, 5.0]);
    assert!(
        result.is_none(),
        "insertion outside mesh should return None"
    );
    assert_eq!(tets.len(), 1);
}

#[test]
fn revolve_x_axis_bounding_box_not_degenerate() {
    let profile = make_revolve_rect_profile();
    let shell = shell_from_revolve(&profile, Axis::X, Radians::from_degrees(360.0)).unwrap();

    let (bb_min, bb_max) = shell.bounding_box();
    let dx = bb_max[0] - bb_min[0];
    let dy = bb_max[1] - bb_min[1];
    let dz = bb_max[2] - bb_min[2];

    assert!(dx > 0.1, "insufficient thickness along X: {dx}");
    assert!(dy > 0.1, "insufficient thickness along Y: {dy}");
    assert!(dz > 0.1, "insufficient thickness along Z: {dz}");
}

// --- Integration tests: surface tetra-clip pipeline ---

#[test]
#[ignore = "generate_box_mesh / boolean_mesh (mfp-types / mfp-geo specific) do not exist in neco-brep"]
fn test_box_minus_sphere_tet_clip() {}

#[test]
#[ignore = "generate_box_mesh / boolean_mesh (mfp-types / mfp-geo specific) do not exist in neco-brep"]
fn test_box_intersect_sphere_tet_clip() {}

#[test]
#[ignore = "generate_box_mesh / boolean_mesh (mfp-types / mfp-geo specific) do not exist in neco-brep"]
fn test_box_minus_box_regression() {}

/// Ensure ellipsoid boolean subtract does not panic in degenerate tangency cases.
#[test]
fn box_subtract_ellipsoid_near_tangent() {
    use neco_brep::{boolean_3d, shell_from_ellipsoid};

    let box_shell = shell_from_box_at([0.0, 0.0, 0.0], 3.0, 2.0, 2.0);
    // Sweep `rz` across 0.9..1.1 to reproduce tangency and penetration against the box faces.
    for i in 0..=20 {
        let rz = 0.9 + (i as f64) * 0.01;
        let ell = {
            let e = shell_from_ellipsoid(1.2, 0.6, rz);
            let m = [
                [1.0, 0.0, 0.0, 1.5],
                [0.0, 1.0, 0.0, 1.0],
                [0.0, 0.0, 1.0, 1.0],
                [0.0, 0.0, 0.0, 1.0],
            ];
            apply_transform(&e, &m)
        };
        let _ = boolean_3d(&box_shell, &ell, BooleanOp::Subtract);
        // No panic is sufficient for this regression case.
    }
}

/// Ensure box x torus boolean subtract does not hang.
#[test]
fn box_subtract_torus_no_hang() {
    use neco_brep::{boolean_3d, shell_from_torus};

    let box_shell = shell_from_box_at([0.0, 0.0, 0.0], 1.0, 1.0, 1.0);
    let torus_shell = {
        let t = shell_from_torus(0.3, 0.1);
        let m = [
            [1.0, 0.0, 0.0, 0.5],
            [0.0, 1.0, 0.0, 0.5],
            [0.0, 0.0, 1.0, 0.5],
            [0.0, 0.0, 0.0, 1.0],
        ];
        apply_transform(&t, &m)
    };
    let _ = boolean_3d(&box_shell, &torus_shell, BooleanOp::Subtract);
}

#[test]
#[ignore = "extract_impostor_faces does not exist in neco-brep (mfp-geo specific)"]
fn boolean_subtract_impostor_trim_generated() {}
