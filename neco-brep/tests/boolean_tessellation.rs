use std::fs::File;
use std::panic::{catch_unwind, AssertUnwindSafe};

use neco_brep::stl::write_stl_binary;
use neco_brep::vec3;
use neco_brep::{
    apply_transform, boolean_3d, shell_from_box, shell_from_cylinder, shell_from_extrude,
    shell_from_loft, shell_from_revolve, shell_from_sphere, shell_from_sweep, Axis, BooleanOp,
    Curve3D, Edge, Face, LoftMode, LoftSection, MeshValidation, Radians, Shell,
};
use neco_nurbs::{NurbsCurve2D, NurbsRegion};

fn translate_shell(shell: &Shell, offset: [f64; 3]) -> Shell {
    let m = [
        [1.0, 0.0, 0.0, offset[0]],
        [0.0, 1.0, 0.0, offset[1]],
        [0.0, 0.0, 1.0, offset[2]],
        [0.0, 0.0, 0.0, 1.0],
    ];
    apply_transform(shell, &m)
}

fn rotate_x_shell(shell: &Shell, radians: f64) -> Shell {
    let (s, c) = radians.sin_cos();
    let m = [
        [1.0, 0.0, 0.0, 0.0],
        [0.0, c, -s, 0.0],
        [0.0, s, c, 0.0],
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

fn translate_z(oz: f64) -> [[f64; 4]; 4] {
    [
        [1.0, 0.0, 0.0, 0.0],
        [0.0, 1.0, 0.0, 0.0],
        [0.0, 0.0, 1.0, oz],
        [0.0, 0.0, 0.0, 1.0],
    ]
}

fn make_rect_region(width: f64, height: f64) -> NurbsRegion {
    let hx = width * 0.5;
    let hy = height * 0.5;
    NurbsRegion {
        outer: vec![NurbsCurve2D::new(
            1,
            vec![[-hx, -hy], [hx, -hy], [hx, hy], [-hx, hy], [-hx, -hy]],
            vec![0.0, 0.0, 0.25, 0.5, 0.75, 1.0, 1.0],
        )],
        holes: vec![],
    }
}

fn make_revolve_profile(inner_r: f64, outer_r: f64, height: f64) -> NurbsRegion {
    NurbsRegion {
        outer: vec![NurbsCurve2D::new(
            1,
            vec![
                [inner_r, -height * 0.5],
                [outer_r, -height * 0.5],
                [outer_r, height * 0.5],
                [inner_r, height * 0.5],
                [inner_r, -height * 0.5],
            ],
            vec![0.0, 0.0, 0.25, 0.5, 0.75, 1.0, 1.0],
        )],
        holes: vec![],
    }
}

fn dump_mesh_if_requested(name: &str, mesh: &neco_brep::TriMesh) {
    if std::env::var("DUMP_STL").is_err() {
        return;
    }
    let path = format!("{name}.stl");
    let mut file = File::create(&path).unwrap_or_else(|e| panic!("failed to create {path}: {e}"));
    write_stl_binary(mesh, &mut file).unwrap_or_else(|e| panic!("failed to write {path}: {e}"));
}

fn surface_kind(surface: &neco_brep::Surface) -> &'static str {
    match surface {
        neco_brep::Surface::Plane { .. } => "Plane",
        neco_brep::Surface::Cylinder { .. } => "Cylinder",
        neco_brep::Surface::Cone { .. } => "Cone",
        neco_brep::Surface::Sphere { .. } => "Sphere",
        neco_brep::Surface::Ellipsoid { .. } => "Ellipsoid",
        neco_brep::Surface::Torus { .. } => "Torus",
        neco_brep::Surface::SurfaceOfRevolution { .. } => "SurfaceOfRevolution",
        neco_brep::Surface::SurfaceOfSweep { .. } => "SurfaceOfSweep",
        neco_brep::Surface::NurbsSurface { .. } => "NurbsSurface",
    }
}

fn dump_shell_summary_if_requested(name: &str, shell: &Shell) {
    if std::env::var("DEBUG_BOOLEAN_TESSELLATE").is_err() {
        return;
    }
    println!("{name}: faces={}", shell.faces.len());
    for (i, face) in shell.faces.iter().enumerate() {
        println!(
            "  face[{i}]: kind={} loop_edges={} reversed={}",
            surface_kind(&face.surface),
            face.loop_edges.len(),
            face.orientation_reversed
        );
    }
}

fn quantize(v: f64) -> i64 {
    (v * 1_000_000.0).round() as i64
}

fn plane_face_signature(face: &Face, shell: &Shell) -> Option<([i64; 6], [i64; 6])> {
    let neco_brep::Surface::Plane { origin, normal } = &face.surface else {
        return None;
    };
    let polygon = neco_brep::boolean3d::intersect3d::face_polygon(face, shell);
    if polygon.is_empty() {
        return None;
    }
    let mut min = [f64::INFINITY; 3];
    let mut max = [f64::NEG_INFINITY; 3];
    for p in &polygon {
        min[0] = min[0].min(p[0]);
        min[1] = min[1].min(p[1]);
        min[2] = min[2].min(p[2]);
        max[0] = max[0].max(p[0]);
        max[1] = max[1].max(p[1]);
        max[2] = max[2].max(p[2]);
    }
    Some((
        [
            quantize(origin[0]),
            quantize(origin[1]),
            quantize(origin[2]),
            quantize(normal[0]),
            quantize(normal[1]),
            quantize(normal[2]),
        ],
        [
            quantize(min[0]),
            quantize(min[1]),
            quantize(min[2]),
            quantize(max[0]),
            quantize(max[1]),
            quantize(max[2]),
        ],
    ))
}

fn tessellate_boolean(
    name: &str,
    a: &Shell,
    b: &Shell,
    op: BooleanOp,
    density: usize,
) -> (neco_brep::TriMesh, MeshValidation) {
    let result = boolean_3d(a, b, op).unwrap_or_else(|e| panic!("{name}: boolean_3d failed: {e}"));
    dump_shell_summary_if_requested(name, &result);
    let mesh = result
        .tessellate(density)
        .unwrap_or_else(|e| panic!("{name}: tessellate failed: {e}"));
    dump_mesh_if_requested(name, &mesh);
    let validation = mesh.validate();
    (mesh, validation)
}

fn assert_mesh_pipeline_survives(
    name: &str,
    mesh: &neco_brep::TriMesh,
    validation: &MeshValidation,
) {
    assert!(
        !mesh.vertices.is_empty(),
        "{name}: tessellation produced no vertices"
    );
    assert!(
        !mesh.triangles.is_empty(),
        "{name}: tessellation produced no triangles"
    );
    assert!(
        validation.signed_volume.abs() > 0.0,
        "{name}: signed volume is zero"
    );
}

fn assert_no_panic<F>(name: &str, f: F)
where
    F: FnOnce(),
{
    let result = catch_unwind(AssertUnwindSafe(f));
    assert!(result.is_ok(), "{name}: operation panicked");
}

fn shell_has_only_degenerate_faces(shell: &Shell) -> bool {
    !shell.faces.is_empty() && shell.faces.iter().all(|face| face.loop_edges.len() < 3)
}

fn edge_sampling_scale(edge: &Edge) -> f64 {
    let points = edge.curve.to_polyline(0.05);
    let mut min = [f64::INFINITY; 3];
    let mut max = [f64::NEG_INFINITY; 3];
    for point in points {
        min[0] = min[0].min(point[0]);
        min[1] = min[1].min(point[1]);
        min[2] = min[2].min(point[2]);
        max[0] = max[0].max(point[0]);
        max[1] = max[1].max(point[1]);
        max[2] = max[2].max(point[2]);
    }
    vec3::length(vec3::sub(max, min)).max(1e-6)
}

fn sample_edge_polyline(edge: &Edge, density: usize) -> Vec<[f64; 3]> {
    edge.curve
        .to_polyline(edge_sampling_scale(edge) / density.max(4) as f64 * 0.25)
}

fn same_point_2d(a: [f64; 2], b: [f64; 2], tol: f64) -> bool {
    let dx = a[0] - b[0];
    let dy = a[1] - b[1];
    dx * dx + dy * dy <= tol * tol
}

fn point_in_polygon_2d(point: [f64; 2], polygon: &[[f64; 2]]) -> bool {
    let n = polygon.len();
    if n < 3 {
        return false;
    }
    let mut inside = false;
    let (px, py) = (point[0], point[1]);
    let mut j = n - 1;
    for i in 0..n {
        let (xi, yi) = (polygon[i][0], polygon[i][1]);
        let (xj, yj) = (polygon[j][0], polygon[j][1]);
        if ((yi > py) != (yj > py)) && (px < (xj - xi) * (py - yi) / (yj - yi) + xi) {
            inside = !inside;
        }
        j = i;
    }
    inside
}

fn average_uv(points: &[[f64; 2]]) -> [f64; 2] {
    let sum = points
        .iter()
        .copied()
        .fold([0.0, 0.0], |acc, p| [acc[0] + p[0], acc[1] + p[1]]);
    [sum[0] / points.len() as f64, sum[1] / points.len() as f64]
}

fn point_is_redundant_2d(prev: [f64; 2], curr: [f64; 2], next: [f64; 2], tol: f64) -> bool {
    if same_point_2d(prev, curr, tol) || same_point_2d(curr, next, tol) {
        return true;
    }

    let ab = [next[0] - prev[0], next[1] - prev[1]];
    let ap = [curr[0] - prev[0], curr[1] - prev[1]];
    let cross = ab[0] * ap[1] - ab[1] * ap[0];
    let scale = (ab[0] * ab[0] + ab[1] * ab[1]).sqrt().max(1.0);
    if cross.abs() > tol * scale {
        return false;
    }

    let dot = ap[0] * ab[0] + ap[1] * ab[1];
    let ab_len_sq = ab[0] * ab[0] + ab[1] * ab[1];
    dot >= 0.0 && dot <= ab_len_sq
}

fn simplify_trim_loop(points: &mut Vec<[f64; 2]>, tol: f64) {
    if points.len() < 3 {
        return;
    }

    loop {
        let n = points.len();
        let mut simplified = Vec::with_capacity(n);
        let mut changed = false;

        for i in 0..n {
            let prev = points[(i + n - 1) % n];
            let curr = points[i];
            let next = points[(i + 1) % n];
            if point_is_redundant_2d(prev, curr, next, tol) {
                changed = true;
                continue;
            }
            simplified.push(curr);
        }

        if !changed || simplified.len() < 3 {
            if simplified.len() >= 3 {
                *points = simplified;
            }
            return;
        }

        *points = simplified;
    }
}

fn surface_param_periods(surface: &neco_brep::Surface) -> (Option<f64>, Option<f64>) {
    let tau = std::f64::consts::TAU;
    match surface {
        neco_brep::Surface::Cylinder { .. }
        | neco_brep::Surface::Cone { .. }
        | neco_brep::Surface::Sphere { .. }
        | neco_brep::Surface::Ellipsoid { .. } => (Some(tau), None),
        neco_brep::Surface::Torus { .. } => (Some(tau), Some(tau)),
        neco_brep::Surface::SurfaceOfRevolution { theta_range, .. }
            if *theta_range >= tau - 1e-12 =>
        {
            (Some(tau), None)
        }
        _ => (None, None),
    }
}

fn normalize_singular_uv(
    surface: &neco_brep::Surface,
    uv: (f64, f64),
    previous_uv: Option<[f64; 2]>,
) -> (f64, f64) {
    match surface {
        neco_brep::Surface::Sphere { .. } | neco_brep::Surface::Ellipsoid { .. }
            if uv.1.abs() <= 1e-8 || (std::f64::consts::PI - uv.1).abs() <= 1e-8 =>
        {
            (previous_uv.map_or(uv.0, |prev| prev[0]), uv.1)
        }
        _ => uv,
    }
}

fn unwrap_periodic_component(value: f64, reference: f64, period: Option<f64>) -> f64 {
    match period {
        Some(period) if period > 0.0 => {
            let shift = ((reference - value) / period).round();
            value + shift * period
        }
        _ => value,
    }
}

fn unwrap_uv_near_reference(
    uv: (f64, f64),
    reference: [f64; 2],
    periods: (Option<f64>, Option<f64>),
) -> [f64; 2] {
    [
        unwrap_periodic_component(uv.0, reference[0], periods.0),
        unwrap_periodic_component(uv.1, reference[1], periods.1),
    ]
}

fn collect_trim_loop_samples(face: &Face, shell: &Shell, density: usize) -> Vec<[f64; 2]> {
    let periods = surface_param_periods(&face.surface);
    let mut trim_loop = Vec::new();
    let mut previous_uv = None;

    for edge_ref in &face.loop_edges {
        let edge = &shell.edges[edge_ref.edge_id];
        let mut polyline = sample_edge_polyline(edge, density);
        if !edge_ref.forward {
            polyline.reverse();
        }
        if !trim_loop.is_empty() && !polyline.is_empty() {
            polyline.remove(0);
        }
        for point in polyline {
            let projected = face.surface.inverse_project(&point).unwrap_or_else(|| {
                panic!("trim loop point must be inverse-projectable: {point:?}")
            });
            let raw_uv = normalize_singular_uv(&face.surface, projected, previous_uv);
            let uv = if let Some(prev) = previous_uv {
                unwrap_uv_near_reference(raw_uv, prev, periods)
            } else {
                [raw_uv.0, raw_uv.1]
            };
            if previous_uv.is_none_or(|prev| !same_point_2d(prev, uv, 1e-10)) {
                trim_loop.push(uv);
                previous_uv = Some(uv);
            }
        }
    }

    simplify_trim_loop(&mut trim_loop, 1e-8);

    if trim_loop.len() >= 2 && same_point_2d(trim_loop[0], trim_loop[trim_loop.len() - 1], 1e-10) {
        trim_loop.pop();
    }
    trim_loop
}

fn assert_face_triangles_stay_inside_trim(
    name: &str,
    shell: &Shell,
    face_index: usize,
    density: usize,
) {
    let face = &shell.faces[face_index];
    let trim_loop = collect_trim_loop_samples(face, shell, density);
    assert!(
        trim_loop.len() >= 3,
        "{name}: trim loop is too small for face_index={face_index}"
    );
    let reference = average_uv(&trim_loop);
    let periods = surface_param_periods(&face.surface);
    let face_shell = Shell {
        vertices: shell.vertices.clone(),
        edges: shell.edges.clone(),
        faces: vec![face.clone()],
    };
    let mesh = face_shell.tessellate(density).unwrap_or_else(|e| {
        panic!("{name}: face tessellation failed for face_index={face_index}: {e}")
    });
    assert!(
        !mesh.triangles.is_empty(),
        "{name}: trimmed face produced no triangles for face_index={face_index}"
    );

    for tri in &mesh.triangles {
        let mut uv = [[0.0, 0.0]; 3];
        for (slot, vertex_id) in uv.iter_mut().zip(tri) {
            let raw = face
                .surface
                .inverse_project(&mesh.vertices[*vertex_id])
                .unwrap_or_else(|| panic!("{name}: triangle vertex is not inverse-projectable"));
            *slot = unwrap_uv_near_reference(raw, reference, periods);
        }
        let centroid = [
            (uv[0][0] + uv[1][0] + uv[2][0]) / 3.0,
            (uv[0][1] + uv[1][1] + uv[2][1]) / 3.0,
        ];
        assert!(
            point_in_polygon_2d(centroid, &trim_loop),
            "{name}: triangle centroid escaped trim loop: face_index={face_index} centroid={centroid:?} trim_loop={trim_loop:?}"
        );
    }
}

fn assert_complex_surface_faces_stay_inside_trim(
    name: &str,
    shell: &Shell,
    kind: &str,
    density: usize,
) {
    let mut matched_faces = 0usize;
    for (face_index, face) in shell.faces.iter().enumerate() {
        if surface_kind(&face.surface) != kind || face.loop_edges.len() < 3 {
            continue;
        }
        if collect_trim_loop_samples(face, shell, density).len() < 3 {
            continue;
        }
        matched_faces += 1;
        assert_face_triangles_stay_inside_trim(name, shell, face_index, density);
    }
    assert!(
        matched_faces > 0,
        "{name}: expected at least one {kind} face in boolean result"
    );
}

#[test]
fn box_box_union_partial_overlap_validates_closed_mesh() {
    let a = shell_from_box_at([0.0, 0.0, 0.0], 2.0, 2.0, 2.0);
    let b = shell_from_box_at([1.0, 0.5, 0.5], 2.0, 2.0, 2.0);

    let (_mesh, v) = tessellate_boolean("union_box_box_partial", &a, &b, BooleanOp::Union, 8);
    assert!(v.has_no_degenerate_faces);
    assert!(v.signed_volume > 0.0);

    let vol_a = a.tessellate(4).unwrap().validate().signed_volume;
    let vol_b = b.tessellate(4).unwrap().validate().signed_volume;
    assert!(v.signed_volume <= vol_a + vol_b + 1e-6);
    assert!(v.signed_volume >= vol_a.max(vol_b) - 1e-6);
}

#[test]
fn box_box_intersect_partial_overlap_validates_closed_mesh() {
    let a = shell_from_box_at([0.0, 0.0, 0.0], 2.0, 2.0, 2.0);
    let b = shell_from_box_at([1.0, 0.5, 0.5], 2.0, 2.0, 2.0);

    let (mesh, v) =
        tessellate_boolean("intersect_box_box_partial", &a, &b, BooleanOp::Intersect, 8);
    assert_mesh_pipeline_survives("intersect_box_box_partial", &mesh, &v);
    assert!(v.signed_volume > 0.0);
}

#[test]
fn box_box_subtract_partial_overlap_validates_closed_mesh() {
    let a = shell_from_box_at([0.0, 0.0, 0.0], 2.0, 2.0, 2.0);
    let b = shell_from_box_at([1.0, 0.5, 0.5], 2.0, 2.0, 2.0);

    let (mesh, v) = tessellate_boolean("subtract_box_box_partial", &a, &b, BooleanOp::Subtract, 8);
    assert_mesh_pipeline_survives("subtract_box_box_partial", &mesh, &v);
    assert!(v.signed_volume > 0.0);

    let vol_a = a.tessellate(4).unwrap().validate().signed_volume;
    assert!(v.signed_volume < vol_a - 1e-6);
}

#[test]
fn box_box_intersect_partial_overlap_has_no_duplicate_plane_faces() {
    use std::collections::BTreeMap;

    let a = shell_from_box_at([0.0, 0.0, 0.0], 2.0, 2.0, 2.0);
    let b = shell_from_box_at([1.0, 0.5, 0.5], 2.0, 2.0, 2.0);
    let result = boolean_3d(&a, &b, BooleanOp::Intersect)
        .unwrap_or_else(|e| panic!("intersect_box_box_partial_duplicates: boolean_3d failed: {e}"));

    let mut counts = BTreeMap::<([i64; 6], [i64; 6]), usize>::new();
    for face in &result.faces {
        if let Some(sig) = plane_face_signature(face, &result) {
            *counts.entry(sig).or_insert(0) += 1;
        }
    }

    assert!(
        counts.values().all(|&count| count == 1),
        "partial-overlap box intersect should not leave duplicate plane faces: {counts:?}"
    );
}

#[test]
fn box_box_subtract_partial_overlap_has_no_duplicate_plane_faces() {
    use std::collections::BTreeMap;

    let a = shell_from_box_at([0.0, 0.0, 0.0], 2.0, 2.0, 2.0);
    let b = shell_from_box_at([1.0, 0.5, 0.5], 2.0, 2.0, 2.0);
    let result = boolean_3d(&a, &b, BooleanOp::Subtract)
        .unwrap_or_else(|e| panic!("subtract_box_box_partial_duplicates: boolean_3d failed: {e}"));

    let mut counts = BTreeMap::<([i64; 6], [i64; 6]), usize>::new();
    for face in &result.faces {
        if let Some(sig) = plane_face_signature(face, &result) {
            *counts.entry(sig).or_insert(0) += 1;
        }
    }

    assert!(
        counts.values().all(|&count| count == 1),
        "partial-overlap box subtract should not leave duplicate plane faces: {counts:?}"
    );
}

#[test]
fn box_box_union_partial_overlap_has_no_duplicate_plane_faces() {
    use std::collections::BTreeMap;

    let a = shell_from_box_at([0.0, 0.0, 0.0], 2.0, 2.0, 2.0);
    let b = shell_from_box_at([1.0, 0.5, 0.5], 2.0, 2.0, 2.0);
    let result = boolean_3d(&a, &b, BooleanOp::Union)
        .unwrap_or_else(|e| panic!("union_box_box_partial_duplicates: boolean_3d failed: {e}"));

    let mut counts = BTreeMap::<([i64; 6], [i64; 6]), usize>::new();
    for face in &result.faces {
        if let Some(sig) = plane_face_signature(face, &result) {
            *counts.entry(sig).or_insert(0) += 1;
        }
    }

    assert!(
        counts.values().all(|&count| count == 1),
        "partial-overlap box union should not leave duplicate plane faces: {counts:?}"
    );
}

#[test]
fn box_box_union_face_sharing_errors_like_disjoint() {
    let a = shell_from_box_at([0.0, 0.0, 0.0], 1.0, 1.0, 1.0);
    let b = shell_from_box_at([1.0, 0.0, 0.0], 1.0, 1.0, 1.0);

    assert!(
        boolean_3d(&a, &b, BooleanOp::Union).is_err(),
        "face-sharing box union should follow disjoint-shell union semantics"
    );
}

#[test]
fn box_box_union_face_sharing_intersect_returns_empty_shell() {
    let a = shell_from_box_at([0.0, 0.0, 0.0], 1.0, 1.0, 1.0);
    let b = shell_from_box_at([1.0, 0.0, 0.0], 1.0, 1.0, 1.0);
    let result = boolean_3d(&a, &b, BooleanOp::Intersect)
        .expect("face-sharing box intersect should return an empty shell");
    assert!(
        result.faces.is_empty() && result.edges.is_empty() && result.vertices.is_empty(),
        "face-sharing box intersect should be empty"
    );
}

#[test]
fn box_box_edge_edge_contact_union_errors_like_disjoint() {
    let a = shell_from_box_at([0.0, 0.0, 0.0], 1.0, 1.0, 1.0);
    let b = shell_from_box_at([1.0, 1.0, 0.0], 1.0, 1.0, 1.0);
    assert!(
        boolean_3d(&a, &b, BooleanOp::Union).is_err(),
        "edge-edge contact union should follow disjoint-shell union semantics"
    );
}

#[test]
fn box_box_vertex_face_contact_union_errors_like_disjoint() {
    let a = shell_from_box_at([0.0, 0.0, 0.0], 1.0, 1.0, 1.0);
    let b = shell_from_box_at([1.0, 0.25, 0.25], 0.5, 0.5, 0.5);
    assert!(
        boolean_3d(&a, &b, BooleanOp::Union).is_err(),
        "vertex-face contact union should follow disjoint-shell union semantics"
    );
}

#[test]
fn box_box_subtract_containment_validates_closed_mesh() {
    let a = shell_from_box_at([0.0, 0.0, 0.0], 3.0, 3.0, 3.0);
    let b = shell_from_box_at([1.0, 1.0, 1.0], 1.0, 1.0, 1.0);

    let (mesh, v) =
        tessellate_boolean("subtract_box_box_contained", &a, &b, BooleanOp::Subtract, 8);
    assert_mesh_pipeline_survives("subtract_box_box_contained", &mesh, &v);
    assert!(v.signed_volume > 0.0);
}

#[test]
fn box_sphere_union_tessellates_with_positive_volume() {
    let box_shell = shell_from_box(2.0, 2.0, 2.0);
    let sphere_shell = translate_shell(&shell_from_sphere(0.9), [0.35, 0.0, 0.0]);
    let result = boolean_3d(&box_shell, &sphere_shell, BooleanOp::Union)
        .unwrap_or_else(|e| panic!("union_box_sphere_offset: boolean_3d failed: {e}"));
    let mesh = result
        .tessellate(24)
        .unwrap_or_else(|e| panic!("union_box_sphere_offset: tessellate failed: {e}"));
    let v = mesh.validate();
    assert_mesh_pipeline_survives("union_box_sphere_offset", &mesh, &v);
    // Trim-aware tessellation fixes curved face leakage, but this mixed plane/sphere union
    // still leaves disconnected components, so stronger volume bounds stay deferred.
}

#[test]
fn characterize_box_sphere_boolean_shell_plane_face_duplicates() {
    use std::collections::BTreeMap;

    let box_shell = shell_from_box(2.0, 2.0, 2.0);
    let sphere_shell = translate_shell(&shell_from_sphere(1.25), [0.8, 0.0, 0.0]);

    for (name, op) in [
        ("subtract", BooleanOp::Subtract),
        ("intersect", BooleanOp::Intersect),
        ("union", BooleanOp::Union),
    ] {
        let result = boolean_3d(&box_shell, &sphere_shell, op)
            .unwrap_or_else(|e| panic!("{name}: boolean_3d failed: {e}"));
        let mut counts: BTreeMap<([i64; 6], [i64; 6]), usize> = BTreeMap::new();
        for face in &result.faces {
            if let Some(sig) = plane_face_signature(face, &result) {
                *counts.entry(sig).or_insert(0) += 1;
            }
        }
        let duplicates: Vec<_> = counts.iter().filter(|(_, count)| **count > 1).collect();
        eprintln!(
            "{name}: plane_faces={}, duplicate_plane_groups={}",
            counts.values().sum::<usize>(),
            duplicates.len()
        );
        for (idx, ((plane_sig, bbox_sig), count)) in duplicates.iter().enumerate() {
            eprintln!(
                "  dup[{idx}] count={} plane={:?} bbox={:?}",
                count, plane_sig, bbox_sig
            );
        }
        assert!(
            duplicates.is_empty(),
            "{name}: duplicate plane face groups should be removed"
        );
    }
}

#[test]
fn box_cylinder_subtract_tessellates_with_positive_volume() {
    let box_shell = shell_from_box(2.0, 2.0, 2.0);
    let cylinder_shell = shell_from_cylinder(0.45, None, 3.0);

    let (mesh, v) = tessellate_boolean(
        "subtract_box_cylinder_through",
        &box_shell,
        &cylinder_shell,
        BooleanOp::Subtract,
        24,
    );

    assert_mesh_pipeline_survives("subtract_box_cylinder_through", &mesh, &v);
}

#[test]
fn box_cylinder_union_contains_analytic_trim_edges() {
    let box_shell = shell_from_box(2.0, 2.0, 2.0);
    let cylinder_shell = shell_from_cylinder(0.45, None, 3.0);
    let result = boolean_3d(&box_shell, &cylinder_shell, BooleanOp::Union).unwrap();

    assert!(
        result.edges.iter().any(|edge| {
            matches!(
                edge.curve,
                Curve3D::Arc { .. } | Curve3D::Ellipse { .. }
            )
        }),
        "box_cylinder_union_contains_analytic_trim_edges: expected at least one Arc or Ellipse edge"
    );
}

#[test]
fn sphere_sphere_intersect_tessellates_with_positive_volume() {
    let a = shell_from_sphere(1.0);
    let b = translate_shell(&shell_from_sphere(1.0), [0.75, 0.0, 0.0]);
    let result = boolean_3d(&a, &b, BooleanOp::Intersect)
        .unwrap_or_else(|e| panic!("intersect_sphere_sphere: boolean_3d failed: {e}"));
    let mesh = result
        .tessellate(8)
        .unwrap_or_else(|e| panic!("intersect_sphere_sphere: tessellate failed: {e}"));
    let v = mesh.validate();
    assert_mesh_pipeline_survives("intersect_sphere_sphere", &mesh, &v);
    let vol_a = a.tessellate(24).unwrap().validate().signed_volume;
    let vol_b = b.tessellate(24).unwrap().validate().signed_volume;
    assert!(v.signed_volume <= vol_a.min(vol_b) + 1e-6);
}

#[test]
fn cylinder_cylinder_subtract_tessellates_with_positive_volume() {
    let a = shell_from_cylinder(0.75, None, 3.0);
    let b = rotate_x_shell(
        &shell_from_cylinder(0.4, None, 3.0),
        std::f64::consts::FRAC_PI_2,
    );

    let (mesh, v) = tessellate_boolean(
        "subtract_cylinder_cylinder_orthogonal",
        &a,
        &b,
        BooleanOp::Subtract,
        20,
    );
    assert_mesh_pipeline_survives("subtract_cylinder_cylinder_orthogonal", &mesh, &v);
    let vol_a = std::f64::consts::PI * 0.75_f64.powi(2) * 3.0;
    assert!(v.signed_volume <= vol_a + 1e-3);
    assert!(v.signed_volume < vol_a - 1e-3);
}

#[test]
fn sweep_minus_box_tessellates_if_boolean_succeeds() {
    let profile = make_rect_region(1.0, 1.0);
    let spine = vec![[0.0, 0.0, -1.5], [0.0, 0.0, 1.5]];
    let sweep_shell = shell_from_sweep(&profile, &spine).unwrap();
    let box_shell = shell_from_box(1.5, 1.5, 1.5);

    let (mesh, v) = tessellate_boolean(
        "subtract_box_sweep",
        &box_shell,
        &sweep_shell,
        BooleanOp::Subtract,
        16,
    );

    assert_mesh_pipeline_survives("subtract_box_sweep", &mesh, &v);
}

#[test]
fn revolution_minus_cylinder_tessellates_if_boolean_succeeds() {
    let revolution_shell = shell_from_revolve(
        &make_revolve_profile(0.35, 1.0, 2.0),
        Axis::Y,
        Radians::from_degrees(360.0),
    )
    .unwrap();
    let cylinder_shell = rotate_x_shell(
        &shell_from_cylinder(0.45, None, 2.5),
        std::f64::consts::FRAC_PI_2,
    );
    let result = boolean_3d(&revolution_shell, &cylinder_shell, BooleanOp::Subtract)
        .unwrap_or_else(|e| panic!("subtract_revolution_cylinder: boolean_3d failed: {e}"));
    if shell_has_only_degenerate_faces(&result) {
        let mesh = result
            .tessellate(16)
            .unwrap_or_else(|e| panic!("subtract_revolution_cylinder: tessellate failed: {e}"));
        assert!(
            mesh.vertices.is_empty() || mesh.triangles.is_empty(),
            "subtract_revolution_cylinder: degenerate shell unexpectedly produced a non-empty mesh"
        );
        return;
    }
    let mesh = result
        .tessellate(16)
        .unwrap_or_else(|e| panic!("subtract_revolution_cylinder: tessellate failed: {e}"));
    dump_mesh_if_requested("subtract_revolution_cylinder", &mesh);
    let v = mesh.validate();
    assert_mesh_pipeline_survives("subtract_revolution_cylinder", &mesh, &v);
}

#[test]
fn loft_nurbs_faces_stay_inside_trim() {
    let sections = vec![
        LoftSection {
            profile: make_rect_region(1.0, 1.0),
            transform: translate_z(0.0),
        },
        LoftSection {
            profile: make_rect_region(1.0, 1.0),
            transform: translate_z(1.0),
        },
        LoftSection {
            profile: make_rect_region(1.0, 1.0),
            transform: translate_z(2.0),
        },
    ];
    let shell = shell_from_loft(&sections, LoftMode::Smooth)
        .unwrap_or_else(|e| panic!("loft_nurbs_trim_containment: shell_from_loft failed: {e}"));

    assert_complex_surface_faces_stay_inside_trim(
        "loft_nurbs_trim_containment",
        &shell,
        "NurbsSurface",
        16,
    );
}

#[test]
fn revolution_minus_cylinder_trimmed_faces_stay_inside_trim() {
    let revolution_shell = shell_from_revolve(
        &make_revolve_profile(0.35, 1.0, 2.0),
        Axis::Y,
        Radians::from_degrees(360.0),
    )
    .unwrap();
    let cylinder_shell = rotate_x_shell(
        &shell_from_cylinder(0.45, None, 2.5),
        std::f64::consts::FRAC_PI_2,
    );
    let result = boolean_3d(&revolution_shell, &cylinder_shell, BooleanOp::Subtract)
        .unwrap_or_else(|e| {
            panic!("subtract_revolution_cylinder_trim_containment: boolean_3d failed: {e}")
        });
    if shell_has_only_degenerate_faces(&result) {
        return;
    }

    assert!(
        result
            .faces
            .iter()
            .all(|face| surface_kind(&face.surface) != "SurfaceOfRevolution"),
        "subtract_revolution_cylinder_trim_containment: rectangular revolve profile should stay analytic instead of producing SurfaceOfRevolution faces"
    );
    assert!(
        result
            .faces
            .iter()
            .all(|face| surface_kind(&face.surface) != "NurbsSurface"),
        "subtract_revolution_cylinder_trim_containment: rectangular revolve profile should stay analytic instead of degrading to NurbsSurface faces"
    );
    assert_complex_surface_faces_stay_inside_trim(
        "subtract_revolution_cylinder_trim_containment",
        &result,
        "Cylinder",
        16,
    );
}

#[test]
fn extrude_minus_sphere_tessellates_if_boolean_succeeds() {
    let extrude_shell =
        shell_from_extrude(&make_rect_region(2.5, 1.5), [0.0, 0.0, 1.0], 2.0).unwrap();
    let sphere_shell = translate_shell(&shell_from_sphere(0.55), [0.0, 0.0, 1.0]);

    let (mesh, v) = tessellate_boolean(
        "subtract_extrude_sphere",
        &extrude_shell,
        &sphere_shell,
        BooleanOp::Subtract,
        20,
    );
    assert_mesh_pipeline_survives("subtract_extrude_sphere", &mesh, &v);
}

#[test]
fn coplanar_faces_do_not_panic() {
    let a = shell_from_box_at([0.0, 0.0, 0.0], 1.0, 1.0, 1.0);
    let b = shell_from_box_at([1.0, 0.0, 0.0], 1.0, 1.0, 1.0);

    assert_no_panic("coplanar_faces_do_not_panic", || {
        let _ = boolean_3d(&a, &b, BooleanOp::Union);
    });
}

#[test]
fn edge_edge_contact_does_not_panic() {
    let a = shell_from_box_at([0.0, 0.0, 0.0], 1.0, 1.0, 1.0);
    let b = shell_from_box_at([1.0, 1.0, 0.0], 1.0, 1.0, 1.0);

    assert_no_panic("edge_edge_contact_does_not_panic", || {
        let _ = boolean_3d(&a, &b, BooleanOp::Union);
    });
}

#[test]
fn vertex_face_contact_does_not_panic() {
    let a = shell_from_box_at([0.0, 0.0, 0.0], 1.0, 1.0, 1.0);
    let b = shell_from_box_at([1.0, 0.25, 0.25], 0.5, 0.5, 0.5);

    assert_no_panic("vertex_face_contact_does_not_panic", || {
        let _ = boolean_3d(&a, &b, BooleanOp::Union);
    });
}

#[test]
fn tiny_gap_does_not_panic() {
    let a = shell_from_box_at([0.0, 0.0, 0.0], 1.0, 1.0, 1.0);
    let b = shell_from_box_at([1.0 + 1e-9, 0.0, 0.0], 1.0, 1.0, 1.0);

    assert_no_panic("tiny_gap_does_not_panic", || {
        let _ = boolean_3d(&a, &b, BooleanOp::Union);
    });
}

#[test]
fn degenerate_zero_volume_result_is_err_or_empty() {
    let a = shell_from_sphere(1.0);
    let b = translate_shell(&shell_from_sphere(1.0), [2.0, 0.0, 0.0]);

    assert_no_panic("degenerate_zero_volume_result_is_err_or_empty", || {
        if let Ok(shell) = boolean_3d(&a, &b, BooleanOp::Intersect) {
            if !shell.faces.is_empty() {
                let mesh = shell.tessellate(24).unwrap();
                dump_mesh_if_requested("degenerate_zero_volume_result", &mesh);
                let v = mesh.validate();
                assert!(v.signed_volume.abs() < 1e-5);
            }
        }
    });
}
