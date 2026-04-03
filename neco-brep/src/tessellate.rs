//! Tessellation: B-Rep Shell to triangle mesh conversion.

use neco_cdt::CdtError;

use crate::brep::{Curve3D, Edge, Face, Shell, Surface};
use crate::vec3;

/// Triangle mesh.
#[derive(Debug, Clone)]
pub struct TriMesh {
    pub vertices: Vec<[f64; 3]>,
    pub normals: Vec<[f64; 3]>,     // per-vertex
    pub triangles: Vec<[usize; 3]>, // vertex indices
}

#[derive(Debug, Clone, Copy)]
struct TrimSample {
    uv: [f64; 2],
    point: [f64; 3],
}

impl TriMesh {
    /// Create an empty mesh.
    pub fn new() -> Self {
        Self {
            vertices: Vec::new(),
            normals: Vec::new(),
            triangles: Vec::new(),
        }
    }

    /// Merge another mesh, offsetting indices.
    pub fn merge(&mut self, other: &TriMesh) {
        let offset = self.vertices.len();
        self.vertices.extend_from_slice(&other.vertices);
        self.normals.extend_from_slice(&other.normals);
        for tri in &other.triangles {
            self.triangles
                .push([tri[0] + offset, tri[1] + offset, tri[2] + offset]);
        }
    }
}

impl Default for TriMesh {
    fn default() -> Self {
        Self::new()
    }
}

impl TriMesh {
    /// Weld vertices within the given distance tolerance to produce a watertight mesh.
    pub fn weld_vertices(&mut self, tolerance: f64) {
        let n = self.vertices.len();
        if n == 0 {
            return;
        }

        // Compute vertex remap
        let mut remap = vec![0usize; n];
        let mut new_vertices: Vec<[f64; 3]> = Vec::new();
        let mut new_normals: Vec<[f64; 3]> = Vec::new();
        let tol_sq = tolerance * tolerance;

        for (i, remap_slot) in remap.iter_mut().enumerate().take(n) {
            let mut found = None;
            for (j, nv) in new_vertices.iter().enumerate() {
                let d = vec3::sub(self.vertices[i], *nv);
                if d[0] * d[0] + d[1] * d[1] + d[2] * d[2] < tol_sq {
                    found = Some(j);
                    break;
                }
            }
            match found {
                Some(j) => {
                    *remap_slot = j;
                }
                None => {
                    *remap_slot = new_vertices.len();
                    new_vertices.push(self.vertices[i]);
                    new_normals.push(self.normals[i]);
                }
            }
        }

        // Remap triangle indices
        for tri in &mut self.triangles {
            tri[0] = remap[tri[0]];
            tri[1] = remap[tri[1]];
            tri[2] = remap[tri[2]];
        }

        self.vertices = new_vertices;
        self.normals = new_normals;
    }
}

impl Shell {
    /// Convert the Shell to a triangle mesh.
    ///
    /// `density` is the number of parametric subdivisions per axis.
    /// Plane faces use constrained Delaunay triangulation (CDT).
    pub fn tessellate(&self, density: usize) -> Result<TriMesh, CdtError> {
        let density = density.max(2);
        let mut mesh = TriMesh::new();

        let debug = std::env::var("DEBUG_FACE_TESSELLATE").is_ok();
        for (face_index, face) in self.faces.iter().enumerate() {
            if debug {
                println!(
                    "tessellate face[{face_index}]: kind={} loop_edges={} reversed={}",
                    tessellation_surface_kind(&face.surface),
                    face.loop_edges.len(),
                    face.orientation_reversed
                );
            }
            let face_mesh = tessellate_face(face, &self.vertices, &self.edges, density)?;
            if debug {
                println!(
                    "  -> vertices={} triangles={}",
                    face_mesh.vertices.len(),
                    face_mesh.triangles.len()
                );
            }
            mesh.merge(&face_mesh);
        }

        // Weld duplicate vertices for watertightness
        mesh.weld_vertices(1e-10);

        Ok(mesh)
    }
}

fn tessellation_surface_kind(surface: &Surface) -> &'static str {
    match surface {
        Surface::Plane { .. } => "Plane",
        Surface::Cylinder { .. } => "Cylinder",
        Surface::Cone { .. } => "Cone",
        Surface::Sphere { .. } => "Sphere",
        Surface::Ellipsoid { .. } => "Ellipsoid",
        Surface::Torus { .. } => "Torus",
        Surface::SurfaceOfRevolution { .. } => "SurfaceOfRevolution",
        Surface::SurfaceOfSweep { .. } => "SurfaceOfSweep",
        Surface::NurbsSurface { .. } => "NurbsSurface",
    }
}

/// Tessellate a single Face.
fn tessellate_face(
    face: &Face,
    vertices: &[[f64; 3]],
    edges: &[Edge],
    density: usize,
) -> Result<TriMesh, CdtError> {
    match &face.surface {
        Surface::Plane { .. } => tessellate_plane(face, vertices, edges, density),
        _ => tessellate_parametric(face, edges, density),
    }
}

/// Tessellate a Plane face using CDT.
///
/// Treats loop_edges as a single outer loop. Inner loops (holes) are not yet supported.
fn tessellate_plane(
    face: &Face,
    vertices: &[[f64; 3]],
    edges: &[Edge],
    density: usize,
) -> Result<TriMesh, CdtError> {
    // Sample curved boundary edges so plane trims follow arc / spline boundaries too.
    let pts_3d = collect_loop_points(face, edges, density);
    match tessellate_plane_polygon(face, &pts_3d) {
        Ok(mesh) if !mesh.triangles.is_empty() => Ok(mesh),
        Ok(_) | Err(_) => {
            let fallback_pts = collect_loop_vertex_points(face, vertices, edges);
            tessellate_plane_polygon(face, &fallback_pts)
        }
    }
}

fn tessellate_plane_polygon(face: &Face, pts_3d: &[[f64; 3]]) -> Result<TriMesh, CdtError> {
    let mut normal = face.surface.normal_at(0.0, 0.0);
    if face.orientation_reversed {
        normal = vec3::scale(normal, -1.0);
    }

    // Build local coordinate frame
    let n = vec3::normalized(normal);
    let up = if n[0].abs() < 0.9 {
        [1.0, 0.0, 0.0]
    } else {
        [0.0, 1.0, 0.0]
    };
    let u_vec = vec3::normalized(vec3::cross(n, up));
    let v_vec = vec3::cross(n, u_vec);

    if pts_3d.len() < 3 {
        return Ok(TriMesh::new());
    }

    // Use first vertex as local origin
    let origin = pts_3d[0];

    // Project 3D to 2D
    let pts_2d: Vec<[f64; 2]> = pts_3d
        .iter()
        .map(|p| {
            let d = vec3::sub(*p, origin);
            [vec3::dot(d, u_vec), vec3::dot(d, v_vec)]
        })
        .collect();

    // Compute bounding box
    let mut min_x = f64::INFINITY;
    let mut min_y = f64::INFINITY;
    let mut max_x = f64::NEG_INFINITY;
    let mut max_y = f64::NEG_INFINITY;
    for p in &pts_2d {
        min_x = min_x.min(p[0]);
        min_y = min_y.min(p[1]);
        max_x = max_x.max(p[0]);
        max_y = max_y.max(p[1]);
    }
    let margin = ((max_x - min_x).max(max_y - min_y)) * 0.01;
    let bounds = (
        min_x - margin,
        min_y - margin,
        max_x + margin,
        max_y + margin,
    );

    // Triangulate via CDT
    let mut cdt = neco_cdt::Cdt::new(bounds);
    cdt.add_constraint_edges(&pts_2d, true)?;
    let cdt_tris = cdt.triangles();

    // Build result mesh
    let mut mesh = TriMesh::new();

    // Map CDT user_vertices back to 3D
    let user_verts = cdt.user_vertices();
    for uv in user_verts {
        let p3d = vec3::add(
            vec3::add(origin, vec3::scale(u_vec, uv[0])),
            vec3::scale(v_vec, uv[1]),
        );
        mesh.vertices.push(p3d);
        mesh.normals.push(normal);
    }

    // Add only triangles whose centroid lies inside the polygon
    for tri in &cdt_tris {
        // Centroid-in-polygon test
        let centroid_2d = [
            (user_verts[tri[0]][0] + user_verts[tri[1]][0] + user_verts[tri[2]][0]) / 3.0,
            (user_verts[tri[0]][1] + user_verts[tri[1]][1] + user_verts[tri[2]][1]) / 3.0,
        ];
        if point_in_polygon_2d(centroid_2d, &pts_2d) {
            mesh.triangles.push(*tri);
        }
    }

    Ok(mesh)
}

/// Tessellate a parametric surface on a uniform grid.
///
/// If the face has trim edges, triangulate its UV trim loop via CDT.
/// Otherwise, fall back to the full surface parameter range.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum TrimmedParametricFailure {
    TrimLoopUnavailable,
    CdtFailure,
    EmptyMesh,
    TriangleLeakage,
}

fn tessellate_parametric(face: &Face, edges: &[Edge], density: usize) -> Result<TriMesh, CdtError> {
    if face.loop_edges.is_empty() {
        return Ok(tessellate_parametric_full(face, density));
    }

    let trim_samples = match try_collect_trim_loop_samples(face, edges, density) {
        Ok(trim_samples) => trim_samples,
        Err(failure) => {
            return Ok(parametric_failure_mesh(
                face,
                density,
                failure,
                TriMesh::new(),
            ));
        }
    };

    let trimmed = if matches!(
        face.surface,
        Surface::Sphere { .. } | Surface::Ellipsoid { .. }
    ) && (face.loop_edges.len() == 3
        || trim_loop_touches_parametric_singularity(&trim_samples))
    {
        Ok(tessellate_parametric_fan(
            face,
            density.max(2),
            &trim_samples,
        ))
    } else {
        tessellate_parametric_trimmed(face, density, &trim_samples)
    };

    match trimmed {
        Ok(mesh) => {
            if mesh.triangles.is_empty() {
                return Ok(parametric_failure_mesh(
                    face,
                    density,
                    TrimmedParametricFailure::EmptyMesh,
                    mesh,
                ));
            }
            if triangles_stay_inside_trim(face, &mesh, &trim_samples) {
                Ok(mesh)
            } else {
                Ok(parametric_failure_mesh(
                    face,
                    density,
                    TrimmedParametricFailure::TriangleLeakage,
                    mesh,
                ))
            }
        }
        Err(err) => {
            if allow_parametric_full_fallback(&face.surface, TrimmedParametricFailure::CdtFailure) {
                Ok(tessellate_parametric_full(face, density))
            } else {
                Err(err)
            }
        }
    }
}

fn allow_parametric_full_fallback(surface: &Surface, failure: TrimmedParametricFailure) -> bool {
    matches!(
        surface,
        Surface::SurfaceOfRevolution { .. }
            | Surface::SurfaceOfSweep { .. }
            | Surface::NurbsSurface { .. }
    ) && matches!(
        failure,
        TrimmedParametricFailure::TrimLoopUnavailable | TrimmedParametricFailure::CdtFailure
    )
}

fn parametric_failure_mesh(
    face: &Face,
    density: usize,
    failure: TrimmedParametricFailure,
    default_mesh: TriMesh,
) -> TriMesh {
    if allow_parametric_full_fallback(&face.surface, failure) {
        tessellate_parametric_full(face, density)
    } else {
        default_mesh
    }
}

/// Tessellate an untrimmed parametric surface on a uniform grid.
fn tessellate_parametric_full(face: &Face, density: usize) -> TriMesh {
    let (u_min, u_max, v_min, v_max) = face.surface.param_range();
    let nu = density;
    let nv = density;

    let mut mesh = TriMesh::new();

    // Generate grid vertices and normals
    for iv in 0..=nv {
        let v = v_min + (v_max - v_min) * iv as f64 / nv as f64;
        for iu in 0..=nu {
            let u = u_min + (u_max - u_min) * iu as f64 / nu as f64;
            let p = face.surface.evaluate(u, v);
            let mut n = face.surface.normal_at(u, v);
            if face.orientation_reversed {
                n = vec3::scale(n, -1.0);
            }
            mesh.vertices.push(p);
            mesh.normals.push(n);
        }
    }

    // Split quad grid into triangles
    let cols = nu + 1;
    for iv in 0..nv {
        for iu in 0..nu {
            let i00 = iv * cols + iu;
            let i10 = iv * cols + (iu + 1);
            let i01 = (iv + 1) * cols + iu;
            let i11 = (iv + 1) * cols + (iu + 1);
            mesh.triangles.push([i00, i10, i11]);
            mesh.triangles.push([i00, i11, i01]);
        }
    }

    mesh
}

fn tessellate_parametric_trimmed(
    face: &Face,
    density: usize,
    trim_samples: &[TrimSample],
) -> Result<TriMesh, CdtError> {
    let trim_loop: Vec<[f64; 2]> = trim_samples.iter().map(|sample| sample.uv).collect();

    if trim_loop.len() == 3 {
        return Ok(tessellate_parametric_triangle(
            face,
            density.max(2),
            [trim_loop[0], trim_loop[1], trim_loop[2]],
        ));
    }

    let (min_u, max_u, min_v, max_v) = bounds_2d(&trim_loop);
    let extent = (max_u - min_u).max(max_v - min_v).max(1e-9);
    let margin = extent * 0.01;
    let bounds = (
        min_u - margin,
        min_v - margin,
        max_u + margin,
        max_v + margin,
    );

    let mut cdt = neco_cdt::Cdt::new(bounds);
    cdt.add_constraint_edges(&trim_loop, true)?;

    let nu = density.max(2);
    let nv = density.max(2);
    let du = (max_u - min_u) / nu as f64;
    let dv = (max_v - min_v) / nv as f64;
    if du.is_finite() && dv.is_finite() && du > 0.0 && dv > 0.0 {
        for iv in 0..nv {
            let v = min_v + (iv as f64 + 0.5) * dv;
            for iu in 0..nu {
                let u = min_u + (iu as f64 + 0.5) * du;
                let uv = [u, v];
                if point_in_polygon_2d(uv, &trim_loop) {
                    cdt.insert(u, v);
                }
            }
        }
    }

    let user_verts = cdt.user_vertices();
    let cdt_tris = cdt.triangles();
    let mut mesh = TriMesh::new();
    for uv in user_verts {
        push_parametric_vertex_with_boundary(&mut mesh, face, *uv, trim_samples);
    }

    for tri in &cdt_tris {
        let centroid = [
            (user_verts[tri[0]][0] + user_verts[tri[1]][0] + user_verts[tri[2]][0]) / 3.0,
            (user_verts[tri[0]][1] + user_verts[tri[1]][1] + user_verts[tri[2]][1]) / 3.0,
        ];
        if point_in_polygon_2d(centroid, &trim_loop) {
            mesh.triangles.push(*tri);
        }
    }

    Ok(mesh)
}

fn tessellate_parametric_triangle(
    face: &Face,
    density: usize,
    triangle_uv: [[f64; 2]; 3],
) -> TriMesh {
    let mut mesh = TriMesh::new();
    let mut row_starts = Vec::with_capacity(density + 1);
    let n = density as f64;

    for i in 0..=density {
        row_starts.push(mesh.vertices.len());
        for j in 0..=(density - i) {
            let w0 = (density - i - j) as f64 / n;
            let w1 = i as f64 / n;
            let w2 = j as f64 / n;
            let uv = [
                w0 * triangle_uv[0][0] + w1 * triangle_uv[1][0] + w2 * triangle_uv[2][0],
                w0 * triangle_uv[0][1] + w1 * triangle_uv[1][1] + w2 * triangle_uv[2][1],
            ];
            push_parametric_vertex(&mut mesh, face, uv);
        }
    }

    let ccw = polygon_signed_area_2d(&triangle_uv) >= 0.0;
    for i in 0..density {
        let row_len = density - i + 1;
        for j in 0..(row_len - 1) {
            let a = row_starts[i] + j;
            let b = row_starts[i] + j + 1;
            let c = row_starts[i + 1] + j;
            push_parametric_triangle(&mut mesh, [a, c, b], ccw);

            if j < row_len - 2 {
                let d = row_starts[i + 1] + j + 1;
                push_parametric_triangle(&mut mesh, [b, c, d], ccw);
            }
        }
    }

    mesh
}

fn tessellate_parametric_fan(face: &Face, density: usize, trim_samples: &[TrimSample]) -> TriMesh {
    let trim_loop: Vec<[f64; 2]> = trim_samples.iter().map(|sample| sample.uv).collect();
    let mut mesh = TriMesh::new();
    let ccw = polygon_signed_area_2d(&trim_loop) >= 0.0;
    let n = trim_loop.len();
    let center = average_polygon_point_2d(&trim_loop);
    push_parametric_vertex(&mut mesh, face, center);
    let center_id = 0usize;
    let mut previous_ring = vec![center_id; n];

    for level in 1..=density {
        let t = level as f64 / density as f64;
        let mut ring = Vec::with_capacity(n);
        for (idx, boundary_uv) in trim_loop.iter().enumerate() {
            let uv = [
                center[0] * (1.0 - t) + boundary_uv[0] * t,
                center[1] * (1.0 - t) + boundary_uv[1] * t,
            ];
            ring.push(mesh.vertices.len());
            if level == density {
                push_parametric_boundary_vertex(&mut mesh, face, uv, trim_samples[idx].point);
            } else {
                push_parametric_vertex(&mut mesh, face, uv);
            }
        }

        if level == 1 {
            for i in 0..n {
                let next = (i + 1) % n;
                push_parametric_triangle(&mut mesh, [center_id, ring[i], ring[next]], ccw);
            }
        } else {
            for i in 0..n {
                let next = (i + 1) % n;
                let inner_a = previous_ring[i];
                let inner_b = previous_ring[next];
                let outer_a = ring[i];
                let outer_b = ring[next];
                push_parametric_triangle(&mut mesh, [inner_a, outer_a, outer_b], ccw);
                push_parametric_triangle(&mut mesh, [inner_a, outer_b, inner_b], ccw);
            }
        }

        previous_ring = ring;
    }

    mesh
}

fn trim_loop_touches_parametric_singularity(trim_samples: &[TrimSample]) -> bool {
    let singular_tol = 1e-6;
    trim_samples.iter().any(|sample| {
        sample.uv[1].abs() <= singular_tol
            || (std::f64::consts::PI - sample.uv[1]).abs() <= singular_tol
    })
}

fn push_parametric_vertex(mesh: &mut TriMesh, face: &Face, uv: [f64; 2]) {
    let p = face.surface.evaluate(uv[0], uv[1]);
    push_parametric_boundary_vertex(mesh, face, uv, p);
}

fn push_parametric_boundary_vertex(mesh: &mut TriMesh, face: &Face, uv: [f64; 2], point: [f64; 3]) {
    let mut n = face.surface.normal_at(uv[0], uv[1]);
    if face.orientation_reversed {
        n = vec3::scale(n, -1.0);
    }
    mesh.vertices.push(point);
    mesh.normals.push(n);
}

fn push_parametric_vertex_with_boundary(
    mesh: &mut TriMesh,
    face: &Face,
    uv: [f64; 2],
    trim_samples: &[TrimSample],
) {
    if let Some(sample) = find_trim_sample(uv, trim_samples) {
        push_parametric_boundary_vertex(mesh, face, uv, sample.point);
    } else {
        push_parametric_vertex(mesh, face, uv);
    }
}

fn push_parametric_triangle(mesh: &mut TriMesh, triangle: [usize; 3], ccw: bool) {
    if ccw {
        mesh.triangles.push(triangle);
    } else {
        mesh.triangles.push([triangle[0], triangle[2], triangle[1]]);
    }
}

#[cfg(test)]
fn collect_trim_loop_samples(face: &Face, edges: &[Edge], density: usize) -> Vec<TrimSample> {
    try_collect_trim_loop_samples(face, edges, density)
        .expect("trim loop point must be inverse-projectable onto its face surface")
}

fn try_collect_trim_loop_samples(
    face: &Face,
    edges: &[Edge],
    density: usize,
) -> Result<Vec<TrimSample>, TrimmedParametricFailure> {
    let periods = surface_param_periods(&face.surface);
    let mut trim_loop = Vec::new();
    let mut previous_uv = None;

    for edge_ref in &face.loop_edges {
        let edge = &edges[edge_ref.edge_id];
        let mut samples = sample_edge_parametric(edge, &face.surface, density)?;
        if !edge_ref.forward {
            samples.reverse();
        }
        if !trim_loop.is_empty() && !samples.is_empty() {
            samples.remove(0);
        }
        for sample in samples {
            let raw_uv = normalize_singular_uv(&face.surface, sample.uv.into(), previous_uv);
            let uv = if let Some(prev) = previous_uv {
                unwrap_uv_near_reference(raw_uv, prev, periods)
            } else {
                [raw_uv.0, raw_uv.1]
            };
            if previous_uv.is_none_or(|prev| !same_point_2d(prev, uv, 1e-10)) {
                trim_loop.push(TrimSample {
                    uv,
                    point: sample.point,
                });
                previous_uv = Some(uv);
            }
        }
    }

    simplify_trim_samples(&mut trim_loop, 1e-8);

    if trim_loop.len() >= 2 {
        let first = trim_loop[0].uv;
        let last = trim_loop[trim_loop.len() - 1].uv;
        if same_point_2d(first, last, 1e-10) {
            trim_loop.pop();
        }
    }

    if trim_loop.len() < 3 {
        return Err(TrimmedParametricFailure::TrimLoopUnavailable);
    }

    Ok(trim_loop)
}

fn triangles_stay_inside_trim(face: &Face, mesh: &TriMesh, trim_samples: &[TrimSample]) -> bool {
    let trim_loop: Vec<[f64; 2]> = trim_samples.iter().map(|sample| sample.uv).collect();
    if trim_loop.len() < 3 {
        return false;
    }
    let reference = average_polygon_point_2d(&trim_loop);
    let periods = surface_param_periods(&face.surface);

    mesh.triangles.iter().all(|tri| {
        let mut uv = [[0.0, 0.0]; 3];
        for (slot, vertex_id) in uv.iter_mut().zip(tri) {
            let Some(raw) = face.surface.inverse_project(&mesh.vertices[*vertex_id]) else {
                return false;
            };
            *slot = unwrap_uv_near_reference(raw, reference, periods);
        }
        let centroid = [
            (uv[0][0] + uv[1][0] + uv[2][0]) / 3.0,
            (uv[0][1] + uv[1][1] + uv[2][1]) / 3.0,
        ];
        point_in_polygon_2d(centroid, &trim_loop)
    })
}

fn surface_param_periods(surface: &Surface) -> (Option<f64>, Option<f64>) {
    let tau = std::f64::consts::TAU;
    match surface {
        Surface::Cylinder { .. }
        | Surface::Cone { .. }
        | Surface::Sphere { .. }
        | Surface::Ellipsoid { .. } => (Some(tau), None),
        Surface::Torus { .. } => (Some(tau), Some(tau)),
        Surface::SurfaceOfRevolution { theta_range, .. } if *theta_range >= tau - 1e-12 => {
            (Some(tau), None)
        }
        _ => (None, None),
    }
}

fn normalize_singular_uv(
    surface: &Surface,
    uv: (f64, f64),
    previous_uv: Option<[f64; 2]>,
) -> (f64, f64) {
    let singular_tol = 1e-6;
    match surface {
        Surface::Sphere { .. } | Surface::Ellipsoid { .. }
            if uv.1.abs() <= singular_tol
                || (std::f64::consts::PI - uv.1).abs() <= singular_tol =>
        {
            (previous_uv.map_or(uv.0, |prev| prev[0]), uv.1)
        }
        _ => uv,
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

fn unwrap_periodic_component(value: f64, reference: f64, period: Option<f64>) -> f64 {
    match period {
        Some(period) if period > 0.0 => {
            let shift = ((reference - value) / period).round();
            value + shift * period
        }
        _ => value,
    }
}

fn bounds_2d(points: &[[f64; 2]]) -> (f64, f64, f64, f64) {
    let mut min_x = f64::INFINITY;
    let mut min_y = f64::INFINITY;
    let mut max_x = f64::NEG_INFINITY;
    let mut max_y = f64::NEG_INFINITY;

    for point in points {
        min_x = min_x.min(point[0]);
        min_y = min_y.min(point[1]);
        max_x = max_x.max(point[0]);
        max_y = max_y.max(point[1]);
    }

    (min_x, max_x, min_y, max_y)
}

fn same_point_2d(a: [f64; 2], b: [f64; 2], tol: f64) -> bool {
    let dx = a[0] - b[0];
    let dy = a[1] - b[1];
    dx * dx + dy * dy <= tol * tol
}

fn polygon_signed_area_2d(points: &[[f64; 2]]) -> f64 {
    let mut area = 0.0;
    for i in 0..points.len() {
        let p = points[i];
        let q = points[(i + 1) % points.len()];
        area += p[0] * q[1] - q[0] * p[1];
    }
    area * 0.5
}

fn average_polygon_point_2d(points: &[[f64; 2]]) -> [f64; 2] {
    let sum = points
        .iter()
        .copied()
        .fold([0.0, 0.0], |acc, p| [acc[0] + p[0], acc[1] + p[1]]);
    [sum[0] / points.len() as f64, sum[1] / points.len() as f64]
}

fn simplify_trim_samples(points: &mut Vec<TrimSample>, tol: f64) {
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
            if point_is_redundant_2d(prev.uv, curr.uv, next.uv, tol) {
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

/// Collect vertex positions from a Face's loop_edges.
fn collect_loop_points(face: &Face, edges: &[Edge], density: usize) -> Vec<[f64; 3]> {
    let mut pts = Vec::new();
    for eref in &face.loop_edges {
        let edge = &edges[eref.edge_id];
        let mut points = sample_curve_parametric_points(&edge.curve, density);
        if !eref.forward {
            points.reverse();
        }
        if !pts.is_empty() && !points.is_empty() {
            points.remove(0);
        }
        for point in points {
            if pts
                .last()
                .is_none_or(|prev| vec3::distance(*prev, point) > 1e-10)
            {
                pts.push(point);
            }
        }
    }
    pts
}

fn collect_loop_vertex_points(face: &Face, vertices: &[[f64; 3]], edges: &[Edge]) -> Vec<[f64; 3]> {
    let mut pts = Vec::with_capacity(face.loop_edges.len());
    for edge_ref in &face.loop_edges {
        let edge = &edges[edge_ref.edge_id];
        let point = if edge_ref.forward {
            vertices[edge.v_start]
        } else {
            vertices[edge.v_end]
        };
        if pts
            .last()
            .is_none_or(|prev| vec3::distance(*prev, point) > 1e-10)
        {
            pts.push(point);
        }
    }
    pts
}

fn sample_curve_parametric_points(curve: &Curve3D, density: usize) -> Vec<[f64; 3]> {
    let (t0, t1) = curve.param_range();
    let tolerance = curve_sampling_tolerance(curve, density);
    let mut points = vec![curve.evaluate(t0)];
    sample_curve_parametric_segment(curve, t0, t1, tolerance, &mut points);
    points
}

fn sample_curve_parametric_segment(
    curve: &Curve3D,
    t0: f64,
    t1: f64,
    tolerance: f64,
    points: &mut Vec<[f64; 3]>,
) {
    if (t1 - t0).abs() < 1e-10 {
        points.push(curve.evaluate(t1));
        return;
    }

    let mid_t = (t0 + t1) * 0.5;
    let p0 = curve.evaluate(t0);
    let p1 = curve.evaluate(t1);
    let mid_curve = curve.evaluate(mid_t);
    let mid_chord = vec3::scale(vec3::add(p0, p1), 0.5);
    let chord_height = vec3::length(vec3::sub(mid_curve, mid_chord));
    if chord_height > tolerance {
        sample_curve_parametric_segment(curve, t0, mid_t, tolerance, points);
        sample_curve_parametric_segment(curve, mid_t, t1, tolerance, points);
    } else {
        points.push(curve.evaluate(t1));
    }
}

fn curve_sampling_tolerance(curve: &Curve3D, density: usize) -> f64 {
    let (t0, t1) = curve.param_range();
    let p0 = curve.evaluate(t0);
    let p1 = curve.evaluate(t1);
    let pm = curve.evaluate((t0 + t1) * 0.5);
    let scale = vec3::distance(p0, p1)
        .max(vec3::distance(p0, pm))
        .max(vec3::distance(pm, p1))
        .max(1e-6);
    scale / density.max(4) as f64 * 0.25
}

fn sample_edge_parametric(
    edge: &Edge,
    surface: &Surface,
    density: usize,
) -> Result<Vec<TrimSample>, TrimmedParametricFailure> {
    let points = sample_curve_parametric_points(&edge.curve, density);
    let mut samples = Vec::with_capacity(points.len());
    for point in points {
        let Some((u, v)) = surface.inverse_project(&point) else {
            return Err(TrimmedParametricFailure::TrimLoopUnavailable);
        };
        samples.push(TrimSample { uv: [u, v], point });
    }

    Ok(samples)
}

fn find_trim_sample(uv: [f64; 2], trim_samples: &[TrimSample]) -> Option<TrimSample> {
    trim_samples
        .iter()
        .copied()
        .find(|sample| same_point_2d(sample.uv, uv, 1e-10))
}

/// 2D point-in-polygon test (ray casting).
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::brep::{Curve3D, EdgeRef, Surface};
    use crate::primitives::{shell_from_cylinder, shell_from_sphere};

    /// Build a unit cube (0,0,0)-(1,1,1) Shell.
    fn make_box_shell() -> Shell {
        let mut shell = Shell::new();

        // 8 vertices
        let v = [
            shell.add_vertex([0.0, 0.0, 0.0]), // 0: ---
            shell.add_vertex([1.0, 0.0, 0.0]), // 1: +--
            shell.add_vertex([1.0, 1.0, 0.0]), // 2: ++-
            shell.add_vertex([0.0, 1.0, 0.0]), // 3: -+-
            shell.add_vertex([0.0, 0.0, 1.0]), // 4: --+
            shell.add_vertex([1.0, 0.0, 1.0]), // 5: +-+
            shell.add_vertex([1.0, 1.0, 1.0]), // 6: +++
            shell.add_vertex([0.0, 1.0, 1.0]), // 7: -++
        ];

        // 12 line edges
        // Bottom (z=0): 0-1, 1-2, 2-3, 3-0
        let e_bot = [
            shell.add_edge(
                v[0],
                v[1],
                Curve3D::Line {
                    start: [0.0, 0.0, 0.0],
                    end: [1.0, 0.0, 0.0],
                },
            ),
            shell.add_edge(
                v[1],
                v[2],
                Curve3D::Line {
                    start: [1.0, 0.0, 0.0],
                    end: [1.0, 1.0, 0.0],
                },
            ),
            shell.add_edge(
                v[2],
                v[3],
                Curve3D::Line {
                    start: [1.0, 1.0, 0.0],
                    end: [0.0, 1.0, 0.0],
                },
            ),
            shell.add_edge(
                v[3],
                v[0],
                Curve3D::Line {
                    start: [0.0, 1.0, 0.0],
                    end: [0.0, 0.0, 0.0],
                },
            ),
        ];

        // Top (z=1): 4-5, 5-6, 6-7, 7-4
        let e_top = [
            shell.add_edge(
                v[4],
                v[5],
                Curve3D::Line {
                    start: [0.0, 0.0, 1.0],
                    end: [1.0, 0.0, 1.0],
                },
            ),
            shell.add_edge(
                v[5],
                v[6],
                Curve3D::Line {
                    start: [1.0, 0.0, 1.0],
                    end: [1.0, 1.0, 1.0],
                },
            ),
            shell.add_edge(
                v[6],
                v[7],
                Curve3D::Line {
                    start: [1.0, 1.0, 1.0],
                    end: [0.0, 1.0, 1.0],
                },
            ),
            shell.add_edge(
                v[7],
                v[4],
                Curve3D::Line {
                    start: [0.0, 1.0, 1.0],
                    end: [0.0, 0.0, 1.0],
                },
            ),
        ];

        // Vertical: 0-4, 1-5, 2-6, 3-7
        let e_vert = [
            shell.add_edge(
                v[0],
                v[4],
                Curve3D::Line {
                    start: [0.0, 0.0, 0.0],
                    end: [0.0, 0.0, 1.0],
                },
            ),
            shell.add_edge(
                v[1],
                v[5],
                Curve3D::Line {
                    start: [1.0, 0.0, 0.0],
                    end: [1.0, 0.0, 1.0],
                },
            ),
            shell.add_edge(
                v[2],
                v[6],
                Curve3D::Line {
                    start: [1.0, 1.0, 0.0],
                    end: [1.0, 1.0, 1.0],
                },
            ),
            shell.add_edge(
                v[3],
                v[7],
                Curve3D::Line {
                    start: [0.0, 1.0, 0.0],
                    end: [0.0, 1.0, 1.0],
                },
            ),
        ];

        // 6 faces
        // Bottom (z=0): normal -Z, CCW from outside -> 0-3-2-1 (reversed)
        shell.faces.push(Face {
            loop_edges: vec![
                EdgeRef {
                    edge_id: e_bot[3],
                    forward: false,
                }, // 0←3
                EdgeRef {
                    edge_id: e_bot[2],
                    forward: false,
                }, // 3←2
                EdgeRef {
                    edge_id: e_bot[1],
                    forward: false,
                }, // 2←1
                EdgeRef {
                    edge_id: e_bot[0],
                    forward: false,
                }, // 1←0
            ],
            surface: Surface::Plane {
                origin: [0.0, 0.0, 0.0],
                normal: [0.0, 0.0, -1.0],
            },
            orientation_reversed: false,
        });

        // Top (z=1): normal +Z, edges 4-5-6-7
        shell.faces.push(Face {
            loop_edges: vec![
                EdgeRef {
                    edge_id: e_top[0],
                    forward: true,
                },
                EdgeRef {
                    edge_id: e_top[1],
                    forward: true,
                },
                EdgeRef {
                    edge_id: e_top[2],
                    forward: true,
                },
                EdgeRef {
                    edge_id: e_top[3],
                    forward: true,
                },
            ],
            surface: Surface::Plane {
                origin: [0.0, 0.0, 1.0],
                normal: [0.0, 0.0, 1.0],
            },
            orientation_reversed: false,
        });

        // Front (y=0): normal -Y, edges 0-1-5-4
        shell.faces.push(Face {
            loop_edges: vec![
                EdgeRef {
                    edge_id: e_bot[0],
                    forward: true,
                }, // 0→1
                EdgeRef {
                    edge_id: e_vert[1],
                    forward: true,
                }, // 1→5
                EdgeRef {
                    edge_id: e_top[0],
                    forward: false,
                }, // 5←4
                EdgeRef {
                    edge_id: e_vert[0],
                    forward: false,
                }, // 4←0
            ],
            surface: Surface::Plane {
                origin: [0.0, 0.0, 0.0],
                normal: [0.0, -1.0, 0.0],
            },
            orientation_reversed: false,
        });

        // Right (x=1): normal +X, edges 1-2-6-5
        shell.faces.push(Face {
            loop_edges: vec![
                EdgeRef {
                    edge_id: e_bot[1],
                    forward: true,
                }, // 1→2
                EdgeRef {
                    edge_id: e_vert[2],
                    forward: true,
                }, // 2→6
                EdgeRef {
                    edge_id: e_top[1],
                    forward: false,
                }, // 6←5
                EdgeRef {
                    edge_id: e_vert[1],
                    forward: false,
                }, // 5←1
            ],
            surface: Surface::Plane {
                origin: [1.0, 0.0, 0.0],
                normal: [1.0, 0.0, 0.0],
            },
            orientation_reversed: false,
        });

        // Back (y=1): normal +Y, edges 2-3-7-6
        shell.faces.push(Face {
            loop_edges: vec![
                EdgeRef {
                    edge_id: e_bot[2],
                    forward: true,
                }, // 2→3
                EdgeRef {
                    edge_id: e_vert[3],
                    forward: true,
                }, // 3→7
                EdgeRef {
                    edge_id: e_top[2],
                    forward: false,
                }, // 7←6
                EdgeRef {
                    edge_id: e_vert[2],
                    forward: false,
                }, // 6←2
            ],
            surface: Surface::Plane {
                origin: [0.0, 1.0, 0.0],
                normal: [0.0, 1.0, 0.0],
            },
            orientation_reversed: false,
        });

        // Left (x=0): normal -X, edges 3-0-4-7
        shell.faces.push(Face {
            loop_edges: vec![
                EdgeRef {
                    edge_id: e_bot[3],
                    forward: true,
                }, // 3→0
                EdgeRef {
                    edge_id: e_vert[0],
                    forward: true,
                }, // 0→4
                EdgeRef {
                    edge_id: e_top[3],
                    forward: false,
                }, // 4←7
                EdgeRef {
                    edge_id: e_vert[3],
                    forward: false,
                }, // 7←3
            ],
            surface: Surface::Plane {
                origin: [0.0, 0.0, 0.0],
                normal: [-1.0, 0.0, 0.0],
            },
            orientation_reversed: false,
        });

        shell
    }

    #[test]
    fn manual_box_tessellation() {
        let shell = make_box_shell();
        let mesh = shell.tessellate(4).unwrap();

        // 6 faces x 4 vertices = 24 vertices (each face has independent vertices)
        assert!(!mesh.vertices.is_empty(), "vertices empty");
        assert!(
            mesh.triangles.len() >= 12,
            "too few triangles: {}",
            mesh.triangles.len()
        );

        let v = mesh.validate();
        assert!(v.is_watertight, "not watertight");
        assert!(v.is_consistently_oriented, "inconsistent orientation");
        assert!(v.has_no_degenerate_faces, "degenerate faces exist");
        assert_eq!(v.euler_number, 2, "euler number != 2: {}", v.euler_number);
        assert!(
            v.signed_volume > 0.0,
            "signed volume not positive: {}",
            v.signed_volume
        );
        assert!(
            (v.signed_volume - 1.0).abs() < 1e-6,
            "volume != 1.0: {}",
            v.signed_volume
        );
        assert_eq!(
            v.n_connected_components, 1,
            "connected components != 1: {}",
            v.n_connected_components
        );
    }

    #[test]
    fn parametric_sphere_tessellation() {
        let mut shell = Shell::new();
        shell.faces.push(Face {
            loop_edges: vec![],
            surface: Surface::Sphere {
                center: [0.0, 0.0, 0.0],
                radius: 1.0,
            },
            orientation_reversed: false,
        });
        let mesh = shell.tessellate(16).unwrap();
        assert!(!mesh.vertices.is_empty());
        assert!(!mesh.triangles.is_empty());
        // 16*16*2 = 512 triangles
        // Welding merges poles and u=0/u=2pi seam, so vertex count < 289
        assert_eq!(mesh.triangles.len(), 16 * 16 * 2);
        assert!(
            mesh.vertices.len() < 17 * 17,
            "poles and seam should be welded"
        );
        assert!(mesh.vertices.len() > 100, "too few vertices");
    }

    #[test]
    fn point_in_polygon_basic() {
        let polygon = vec![[0.0, 0.0], [1.0, 0.0], [1.0, 1.0], [0.0, 1.0]];
        assert!(point_in_polygon_2d([0.5, 0.5], &polygon));
        assert!(!point_in_polygon_2d([1.5, 0.5], &polygon));
        assert!(!point_in_polygon_2d([-0.5, 0.5], &polygon));
    }

    #[test]
    fn sample_curve_parametric_points_line_and_arc() {
        let line = Curve3D::Line {
            start: [0.0, 0.0, 0.0],
            end: [1.0, 0.0, 0.0],
        };
        let line_points = sample_curve_parametric_points(&line, 8);
        assert_eq!(line_points.len(), 2);
        assert!(vec3::distance(line_points[0], [0.0, 0.0, 0.0]) < 1e-12);
        assert!(vec3::distance(line_points[1], [1.0, 0.0, 0.0]) < 1e-12);

        let arc = Curve3D::Arc {
            center: [0.0, 0.0, 0.0],
            axis: [0.0, 0.0, 1.0],
            start: [1.0, 0.0, 0.0],
            end: [0.0, 1.0, 0.0],
            radius: 1.0,
        };
        let arc_points = sample_curve_parametric_points(&arc, 8);
        assert!(arc_points.len() > 2);
        assert!(vec3::distance(arc_points[0], [1.0, 0.0, 0.0]) < 1e-12);
        assert!(vec3::distance(*arc_points.last().unwrap(), [0.0, 1.0, 0.0]) < 1e-12);
    }

    #[test]
    fn collect_loop_points_uses_parametric_sampling_for_full_ellipse() {
        let ellipse = Curve3D::Ellipse {
            center: [0.0, 0.0, 0.0],
            axis_u: [1.0, 0.0, 0.0],
            axis_v: [0.0, 0.5, 0.0],
            t_start: 0.0,
            t_end: std::f64::consts::TAU,
        };
        let face = Face {
            loop_edges: vec![EdgeRef {
                edge_id: 0,
                forward: true,
            }],
            surface: Surface::Plane {
                origin: [0.0, 0.0, 0.0],
                normal: [0.0, 0.0, 1.0],
            },
            orientation_reversed: false,
        };
        let edges = vec![Edge {
            v_start: 0,
            v_end: 0,
            curve: ellipse,
        }];

        let points = collect_loop_points(&face, &edges, 12);
        assert!(
            points.len() > 8,
            "ellipse boundary should be adaptively sampled"
        );
        assert!(vec3::distance(points[0], [1.0, 0.0, 0.0]) < 1e-12);
    }

    fn average_uv(points: &[[f64; 2]]) -> [f64; 2] {
        let sum = points
            .iter()
            .copied()
            .fold([0.0, 0.0], |acc, p| [acc[0] + p[0], acc[1] + p[1]]);
        [sum[0] / points.len() as f64, sum[1] / points.len() as f64]
    }

    fn assert_triangles_stay_inside_trim(shell: &Shell, face_index: usize, density: usize) {
        let face = &shell.faces[face_index];
        let trim_samples = collect_trim_loop_samples(face, &shell.edges, density);
        let trim_loop: Vec<[f64; 2]> = trim_samples.iter().map(|sample| sample.uv).collect();
        let reference = average_uv(&trim_loop);
        let periods = surface_param_periods(&face.surface);
        let mesh = tessellate_face(face, &shell.vertices, &shell.edges, density).unwrap();

        assert!(
            !mesh.triangles.is_empty(),
            "trimmed face produced no triangles"
        );
        for tri in &mesh.triangles {
            let mut uv = [[0.0, 0.0]; 3];
            for (slot, vertex_id) in uv.iter_mut().zip(tri) {
                let raw = face
                    .surface
                    .inverse_project(&mesh.vertices[*vertex_id])
                    .unwrap();
                *slot = unwrap_uv_near_reference(raw, reference, periods);
            }
            let centroid = [
                (uv[0][0] + uv[1][0] + uv[2][0]) / 3.0,
                (uv[0][1] + uv[1][1] + uv[2][1]) / 3.0,
            ];
            assert!(
                point_in_polygon_2d(centroid, &trim_loop),
                "triangle centroid escaped trim loop: face_index={face_index} centroid={centroid:?} trim_loop={trim_loop:?}"
            );
        }
    }

    #[test]
    fn sphere_trim_crossing_seam_stays_inside_trim() {
        let shell = shell_from_sphere(1.0);
        assert_triangles_stay_inside_trim(&shell, 3, 12);
    }

    #[test]
    fn cylinder_trim_crossing_seam_stays_inside_trim() {
        let shell = shell_from_cylinder(1.0, None, 2.0);
        assert_triangles_stay_inside_trim(&shell, 3, 12);
    }
}
