//! Shell coordinate transforms and solid-to-Shell conversion.

use std::collections::HashMap;

use neco_nurbs::NurbsRegion;

use crate::brep::{Curve3D, Edge, EdgeId, EdgeRef, Face, Shell, Surface};
use crate::vec3;

// ── Helpers ──────────────────────────────────────────

/// Transform a point by a 4x4 affine matrix.
fn transform_point(m: &[[f64; 4]; 4], p: [f64; 3]) -> [f64; 3] {
    [
        m[0][0] * p[0] + m[0][1] * p[1] + m[0][2] * p[2] + m[0][3],
        m[1][0] * p[0] + m[1][1] * p[1] + m[1][2] * p[2] + m[1][3],
        m[2][0] * p[0] + m[2][1] * p[1] + m[2][2] * p[2] + m[2][3],
    ]
}

/// Transform a direction vector (rotation only, no translation).
fn transform_direction(m: &[[f64; 4]; 4], d: [f64; 3]) -> [f64; 3] {
    [
        m[0][0] * d[0] + m[0][1] * d[1] + m[0][2] * d[2],
        m[1][0] * d[0] + m[1][1] * d[1] + m[1][2] * d[2],
        m[2][0] * d[0] + m[2][1] * d[1] + m[2][2] * d[2],
    ]
}

/// Check whether the matrix is the identity.
fn is_identity(m: &[[f64; 4]; 4]) -> bool {
    const EPS: f64 = 1e-14;
    for (i, row) in m.iter().enumerate().take(4) {
        for (j, value) in row.iter().enumerate().take(4) {
            let expected = if i == j { 1.0 } else { 0.0 };
            if (*value - expected).abs() > EPS {
                return false;
            }
        }
    }
    true
}

/// Transform a Curve3D by a 4x4 matrix.
fn transform_curve3d(m: &[[f64; 4]; 4], curve: &Curve3D) -> Curve3D {
    match curve {
        Curve3D::Line { start, end } => Curve3D::Line {
            start: transform_point(m, *start),
            end: transform_point(m, *end),
        },
        Curve3D::Arc {
            center,
            axis,
            start,
            end,
            radius,
        } => Curve3D::Arc {
            center: transform_point(m, *center),
            axis: transform_direction(m, *axis),
            start: transform_point(m, *start),
            end: transform_point(m, *end),
            radius: *radius,
        },
        Curve3D::Ellipse {
            center,
            axis_u,
            axis_v,
            t_start,
            t_end,
        } => Curve3D::Ellipse {
            center: transform_point(m, *center),
            axis_u: transform_direction(m, *axis_u),
            axis_v: transform_direction(m, *axis_v),
            t_start: *t_start,
            t_end: *t_end,
        },
        Curve3D::NurbsCurve3D {
            degree,
            control_points,
            weights,
            knots,
        } => Curve3D::NurbsCurve3D {
            degree: *degree,
            control_points: control_points
                .iter()
                .map(|p| transform_point(m, *p))
                .collect(),
            weights: weights.clone(),
            knots: knots.clone(),
        },
    }
}

/// Transform a Surface by a 4x4 matrix.
fn transform_surface(m: &[[f64; 4]; 4], surface: &Surface) -> Surface {
    match surface {
        Surface::Plane { origin, normal } => Surface::Plane {
            origin: transform_point(m, *origin),
            normal: transform_direction(m, *normal),
        },
        Surface::Cylinder {
            origin,
            axis,
            radius,
        } => Surface::Cylinder {
            origin: transform_point(m, *origin),
            axis: transform_direction(m, *axis),
            radius: *radius,
        },
        Surface::Cone {
            origin,
            axis,
            half_angle,
        } => Surface::Cone {
            origin: transform_point(m, *origin),
            axis: transform_direction(m, *axis),
            half_angle: *half_angle,
        },
        Surface::Sphere { center, radius } => Surface::Sphere {
            center: transform_point(m, *center),
            radius: *radius,
        },
        Surface::Ellipsoid { center, rx, ry, rz } => Surface::Ellipsoid {
            center: transform_point(m, *center),
            rx: *rx,
            ry: *ry,
            rz: *rz,
        },
        Surface::Torus {
            center,
            axis,
            major_radius,
            minor_radius,
        } => Surface::Torus {
            center: transform_point(m, *center),
            axis: transform_direction(m, *axis),
            major_radius: *major_radius,
            minor_radius: *minor_radius,
        },
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
        } => Surface::SurfaceOfRevolution {
            center: transform_point(m, *center),
            axis: transform_direction(m, *axis),
            frame_u: transform_direction(m, *frame_u),
            frame_v: transform_direction(m, *frame_v),
            profile_control_points: profile_control_points.clone(),
            profile_weights: profile_weights.clone(),
            profile_degree: *profile_degree,
            n_profile_spans: *n_profile_spans,
            theta_start: *theta_start,
            theta_range: *theta_range,
        },
        Surface::SurfaceOfSweep {
            spine_control_points,
            spine_weights,
            spine_degree,
            profile_control_points,
            profile_weights,
            profile_degree,
            n_profile_spans,
            frames,
        } => {
            let new_spine: Vec<[f64; 3]> = spine_control_points
                .iter()
                .map(|p| transform_point(m, *p))
                .collect();
            let new_frames: Vec<[[f64; 3]; 3]> = frames
                .iter()
                .map(|f| {
                    [
                        transform_direction(m, f[0]),
                        transform_direction(m, f[1]),
                        transform_direction(m, f[2]),
                    ]
                })
                .collect();
            Surface::SurfaceOfSweep {
                spine_control_points: new_spine,
                spine_weights: spine_weights.clone(),
                spine_degree: *spine_degree,
                profile_control_points: profile_control_points.clone(),
                profile_weights: profile_weights.clone(),
                profile_degree: *profile_degree,
                n_profile_spans: *n_profile_spans,
                frames: new_frames,
            }
        }
        Surface::NurbsSurface { data } => {
            let mut new_data = data.as_ref().clone();
            for row in &mut new_data.control_points {
                for pt in row {
                    *pt = transform_point(m, *pt);
                }
            }
            Surface::NurbsSurface {
                data: Box::new(new_data),
            }
        }
    }
}

// ── Public API ───────────────────────────────────────

/// Apply a 4x4 affine transform to all vertices, edges, and faces.
///
/// Returns a clone if the matrix is the identity.
pub fn apply_transform(shell: &Shell, matrix: &[[f64; 4]; 4]) -> Shell {
    if is_identity(matrix) {
        return shell.clone();
    }
    let vertices: Vec<[f64; 3]> = shell
        .vertices
        .iter()
        .map(|v| transform_point(matrix, *v))
        .collect();
    let edges: Vec<Edge> = shell
        .edges
        .iter()
        .map(|e| Edge {
            v_start: e.v_start,
            v_end: e.v_end,
            curve: transform_curve3d(matrix, &e.curve),
        })
        .collect();
    let faces: Vec<Face> = shell
        .faces
        .iter()
        .map(|f| Face {
            loop_edges: f.loop_edges.clone(),
            surface: transform_surface(matrix, &f.surface),
            orientation_reversed: f.orientation_reversed,
        })
        .collect();
    Shell {
        vertices,
        edges,
        faces,
    }
}

/// Build a B-Rep Shell from indexed vertex/face data.
///
/// Each face is treated as planar; normal is derived from the first 3 vertices.
/// Warns if any vertex in a 4+ vertex face is non-coplanar.
pub fn solid_to_shell(
    vertices: &[[f64; 3]],
    faces: &[Vec<usize>],
) -> Result<(Shell, Vec<String>), String> {
    if vertices.is_empty() {
        return Err("solid_to_shell: empty vertex list".into());
    }
    if faces.is_empty() {
        return Err("solid_to_shell: empty face list".into());
    }

    let mut shell = Shell::new();
    let mut warnings: Vec<String> = Vec::new();

    // 1. Add vertices
    for v in vertices {
        shell.add_vertex(*v);
    }

    // 2. Edge map: (min_idx, max_idx) -> EdgeId (dedup)
    let mut edge_map: HashMap<(usize, usize), EdgeId> = HashMap::new();

    // 3. Process each face
    for face_indices in faces {
        if face_indices.len() < 3 {
            return Err(format!(
                "solid_to_shell: face has fewer than 3 vertices ({})",
                face_indices.len()
            ));
        }

        let mut loop_edges = Vec::with_capacity(face_indices.len());

        for i in 0..face_indices.len() {
            let a = face_indices[i];
            let b = face_indices[(i + 1) % face_indices.len()];

            let key = (a.min(b), a.max(b));
            let edge_id = *edge_map.entry(key).or_insert_with(|| {
                let (v_start, v_end) = (key.0, key.1);
                shell.add_edge(
                    v_start,
                    v_end,
                    Curve3D::Line {
                        start: vertices[v_start],
                        end: vertices[v_end],
                    },
                )
            });

            // forward = true when face traversal matches edge's v_start -> v_end
            let forward = a < b;
            loop_edges.push(EdgeRef { edge_id, forward });
        }

        // Normal: cross(v1 - v0, v2 - v0)
        let v0 = vertices[face_indices[0]];
        let v1 = vertices[face_indices[1]];
        let v2 = vertices[face_indices[2]];
        let raw_normal = vec3::cross(vec3::sub(v1, v0), vec3::sub(v2, v0));
        let normal = vec3::normalized(raw_normal);

        // Non-coplanarity check for 4+ vertex faces
        if face_indices.len() > 3 {
            let normal_len = vec3::length(raw_normal);
            if normal_len > 1e-15 {
                for &vi in &face_indices[3..] {
                    let d = vec3::dot(vec3::sub(vertices[vi], v0), normal).abs();
                    if d > 1e-6 {
                        warnings.push(format!(
                            "solid face: vertex {} is {:.6} off-plane (non-coplanar)",
                            vi, d
                        ));
                    }
                }
            }
        }

        // Use face centroid as origin
        let n = face_indices.len() as f64;
        let sum = face_indices
            .iter()
            .fold([0.0, 0.0, 0.0], |acc, &idx| vec3::add(acc, vertices[idx]));
        let origin = vec3::scale(sum, 1.0 / n);

        shell.faces.push(Face {
            loop_edges,
            surface: Surface::Plane { origin, normal },
            orientation_reversed: false,
        });
    }

    Ok((shell, warnings))
}

// ── Profile conversion helpers (shared by extrude / revolve) ──

/// Extract profile vertices from a degree-1 NurbsRegion.
///
/// Strips the closing duplicate point for closed curves.
/// Returns `(vertices, is_closed)`.
pub fn profile_vertices(profile: &NurbsRegion) -> Result<(Vec<[f64; 2]>, bool), String> {
    if profile.outer[0].degree != 1 {
        return Err(format!(
            "shell_from_extrude: degree {} not supported (degree-1 only)",
            profile.outer[0].degree
        ));
    }
    let pts = &profile.outer[0].control_points;
    let is_closed = profile.outer_is_closed(1e-10);
    if is_closed {
        if pts.len() < 4 {
            return Err("closed profile has too few vertices".to_string());
        }
        // Strip closing point (first == last)
        Ok((pts[..pts.len() - 1].to_vec(), true))
    } else {
        if pts.len() < 2 {
            return Err("open profile has too few vertices".to_string());
        }
        Ok((pts.to_vec(), false))
    }
}

/// Convert `[f64; 2]` to `[f64; 3]` on the XY plane (Z-up convention).
pub fn point2_to_point3(p: &[f64; 2]) -> [f64; 3] {
    [p[0], p[1], 0.0]
}

/// Degree elevation of a 2D rational Bezier curve.
///
/// Elevates control points and weights from `from_deg` to `to_deg`.
/// Returns `to_deg + 1` (control_point, weight) pairs.
pub fn elevate_bezier_2d(
    cps: &[[f64; 2]],
    weights: &[f64],
    from_deg: usize,
    to_deg: usize,
) -> Vec<([f64; 2], f64)> {
    if from_deg >= to_deg {
        return cps
            .iter()
            .zip(weights.iter())
            .map(|(c, w)| (*c, *w))
            .collect();
    }

    // Convert to homogeneous coordinates: (w*x, w*y, w)
    let mut hom: Vec<[f64; 3]> = cps
        .iter()
        .zip(weights.iter())
        .map(|(c, &w)| [c[0] * w, c[1] * w, w])
        .collect();

    // Elevate one degree at a time
    for deg in from_deg..to_deg {
        let n = deg + 1; // current control point count
        let mut new_hom = vec![[0.0; 3]; n + 1];
        new_hom[0] = hom[0];
        new_hom[n] = hom[n - 1];
        for i in 1..n {
            let alpha = i as f64 / (deg + 1) as f64;
            for k in 0..3 {
                new_hom[i][k] = alpha * hom[i - 1][k] + (1.0 - alpha) * hom[i][k];
            }
        }
        hom = new_hom;
    }

    // Convert back from homogeneous
    hom.iter()
        .map(|h| {
            let w = h[2];
            if w.abs() < 1e-30 {
                ([0.0, 0.0], 0.0)
            } else {
                ([h[0] / w, h[1] / w], w)
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn identity_returns_clone() {
        let shell = Shell::new();
        let identity = [
            [1.0, 0.0, 0.0, 0.0],
            [0.0, 1.0, 0.0, 0.0],
            [0.0, 0.0, 1.0, 0.0],
            [0.0, 0.0, 0.0, 1.0],
        ];
        let result = apply_transform(&shell, &identity);
        assert_eq!(result.vertices.len(), 0);
    }

    #[test]
    fn translation_shifts_vertices() {
        let mut shell = Shell::new();
        shell.add_vertex([1.0, 2.0, 3.0]);
        let translate = [
            [1.0, 0.0, 0.0, 10.0],
            [0.0, 1.0, 0.0, 20.0],
            [0.0, 0.0, 1.0, 30.0],
            [0.0, 0.0, 0.0, 1.0],
        ];
        let result = apply_transform(&shell, &translate);
        let v = result.vertices[0];
        assert!((v[0] - 11.0).abs() < 1e-12);
        assert!((v[1] - 22.0).abs() < 1e-12);
        assert!((v[2] - 33.0).abs() < 1e-12);
    }

    #[test]
    fn solid_to_shell_triangle() {
        let verts = vec![[0.0, 0.0, 0.0], [1.0, 0.0, 0.0], [0.0, 1.0, 0.0]];
        let faces = vec![vec![0, 1, 2]];
        let (shell, warnings) = solid_to_shell(&verts, &faces).unwrap();
        assert_eq!(shell.vertices.len(), 3);
        assert_eq!(shell.edges.len(), 3);
        assert_eq!(shell.faces.len(), 1);
        assert!(warnings.is_empty());
    }

    #[test]
    fn solid_to_shell_empty_vertices() {
        let result = solid_to_shell(&[], &[vec![0, 1, 2]]);
        assert!(result.is_err());
    }

    #[test]
    fn solid_to_shell_empty_faces() {
        let result = solid_to_shell(&[[0.0; 3]], &[]);
        assert!(result.is_err());
    }
}
