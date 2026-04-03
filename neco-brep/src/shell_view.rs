//! Lightweight shell view API for downstream inspection and serialization prep.

use crate::boolean3d::classify3d::point_in_face_polygon;
use crate::boolean3d::intersect3d::face_polygon;
use crate::vec3;
use crate::{Face, Shell};

#[derive(Debug, Clone)]
pub struct FaceSample {
    pub point: [f64; 3],
    pub uv: Option<[f64; 2]>,
    pub normal: Option<[f64; 3]>,
}

#[derive(Debug, Clone)]
pub struct FaceView {
    pub face_index: usize,
    pub surface_kind: &'static str,
    pub orientation_reversed: bool,
    pub loop_edge_count: usize,
    pub polygon_3d: Vec<[f64; 3]>,
    pub polygon_uv: Vec<[f64; 2]>,
    pub sample: Option<FaceSample>,
}

#[derive(Debug, Clone)]
pub struct ShellView {
    pub face_count: usize,
    pub edge_count: usize,
    pub vertex_count: usize,
    pub faces: Vec<FaceView>,
}

pub fn shell_view(shell: &Shell) -> ShellView {
    let faces = shell
        .faces
        .iter()
        .enumerate()
        .map(|(face_index, face)| FaceView {
            face_index,
            surface_kind: surface_kind(face),
            orientation_reversed: face.orientation_reversed,
            loop_edge_count: face.loop_edges.len(),
            polygon_3d: face_polygon(face, shell),
            polygon_uv: face_polygon_uv(face, shell),
            sample: face_debug_sample(face, shell),
        })
        .collect();

    ShellView {
        face_count: shell.faces.len(),
        edge_count: shell.edges.len(),
        vertex_count: shell.vertices.len(),
        faces,
    }
}

fn surface_kind(face: &Face) -> &'static str {
    match &face.surface {
        crate::Surface::Plane { .. } => "Plane",
        crate::Surface::Cylinder { .. } => "Cylinder",
        crate::Surface::Cone { .. } => "Cone",
        crate::Surface::Sphere { .. } => "Sphere",
        crate::Surface::Ellipsoid { .. } => "Ellipsoid",
        crate::Surface::Torus { .. } => "Torus",
        crate::Surface::SurfaceOfRevolution { .. } => "SurfaceOfRevolution",
        crate::Surface::SurfaceOfSweep { .. } => "SurfaceOfSweep",
        crate::Surface::NurbsSurface { .. } => "NurbsSurface",
    }
}

fn face_polygon_uv(face: &Face, shell: &Shell) -> Vec<[f64; 2]> {
    face_polygon(face, shell)
        .into_iter()
        .filter_map(|point| face.surface.inverse_project(&point).map(|(u, v)| [u, v]))
        .collect()
}

fn face_debug_sample(face: &Face, shell: &Shell) -> Option<FaceSample> {
    let polygon = face_polygon(face, shell);
    let point = interior_point(face, shell, &polygon)?;
    let uv = face.surface.inverse_project(&point).map(|(u, v)| [u, v]);
    let normal = uv.map(|[u, v]| {
        let surf_n = face.surface.normal_at(u, v);
        orient_normal(face, &polygon, surf_n)
    });
    Some(FaceSample { point, uv, normal })
}

fn orient_normal(face: &Face, polygon: &[[f64; 3]], surf_n: [f64; 3]) -> [f64; 3] {
    if face.orientation_reversed {
        vec3::scale(surf_n, -1.0)
    } else {
        let poly_n = polygon_normal(polygon);
        if vec3::dot(poly_n, surf_n) < 0.0 {
            vec3::scale(surf_n, -1.0)
        } else {
            surf_n
        }
    }
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

fn project_point_to_face_surface(face: &Face, point: &[f64; 3]) -> [f64; 3] {
    match face.surface.inverse_project(point) {
        Some((u, v)) => face.surface.evaluate(u, v),
        None => *point,
    }
}

fn interior_point(face: &Face, shell: &Shell, poly: &[[f64; 3]]) -> Option<[f64; 3]> {
    if poly.len() < 3 {
        return None;
    }

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

    let root = poly[0];
    for i in 1..(poly.len() - 1) {
        let c = project_point_to_face_surface(face, &triangle_centroid(root, poly[i], poly[i + 1]));
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

    None
}
