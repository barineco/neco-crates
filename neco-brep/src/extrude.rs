use neco_nurbs::NurbsRegion;

use crate::brep::{Curve3D, EdgeRef, Face, Shell, Surface};
use crate::vec3;

use crate::transform::{elevate_bezier_2d, point2_to_point3, profile_vertices};

/// Extrude a NurbsRegion along `direction` by `distance` to produce a Shell.
///
/// degree=1 profiles use Plane faces; degree>=2 profiles generate
/// NurbsSurface / Plane side faces per Bezier span.
pub fn shell_from_extrude(
    profile: &NurbsRegion,
    direction: [f64; 3],
    distance: f64,
) -> Result<Shell, String> {
    if profile.outer.iter().any(|c| c.degree >= 2) {
        return shell_from_extrude_nurbs(profile, direction, distance);
    }
    let (verts_2d, is_closed) = profile_vertices(profile)?;
    let n = verts_2d.len();
    let n_edges = if is_closed { n } else { n - 1 };

    let dir = [
        direction[0] * distance,
        direction[1] * distance,
        direction[2] * distance,
    ];
    let dir_norm = vec3::normalized([direction[0], direction[1], direction[2]]);

    let mut shell = Shell::new();

    let mut bottom_v = Vec::with_capacity(n);
    let mut top_v = Vec::with_capacity(n);
    for p2 in &verts_2d {
        let p3 = point2_to_point3(p2);
        bottom_v.push(shell.add_vertex(p3));
        top_v.push(shell.add_vertex(vec3::add(p3, dir)));
    }

    let verts = shell.vertices.clone();
    let line = |a: usize, b: usize| Curve3D::Line {
        start: verts[a],
        end: verts[b],
    };

    let mut bottom_e = Vec::with_capacity(n_edges);
    for i in 0..n_edges {
        let j = if is_closed { (i + 1) % n } else { i + 1 };
        bottom_e.push(shell.add_edge(bottom_v[i], bottom_v[j], line(bottom_v[i], bottom_v[j])));
    }
    let mut top_e = Vec::with_capacity(n_edges);
    for i in 0..n_edges {
        let j = if is_closed { (i + 1) % n } else { i + 1 };
        top_e.push(shell.add_edge(top_v[i], top_v[j], line(top_v[i], top_v[j])));
    }
    let mut vert_e = Vec::with_capacity(n);
    for i in 0..n {
        vert_e.push(shell.add_edge(bottom_v[i], top_v[i], line(bottom_v[i], top_v[i])));
    }

    let fwd = |eid: usize| EdgeRef {
        edge_id: eid,
        forward: true,
    };
    let rev = |eid: usize| EdgeRef {
        edge_id: eid,
        forward: false,
    };

    // Cap faces (closed profiles only)
    if is_closed {
        // Bottom face: normal = -dir_norm, loop reversed
        {
            let mut loop_edges = Vec::with_capacity(n_edges);
            for i in (0..n_edges).rev() {
                loop_edges.push(rev(bottom_e[i]));
            }
            let centroid = bottom_v
                .iter()
                .fold([0.0, 0.0, 0.0], |acc, &vi| vec3::add(acc, verts[vi]));
            let centroid = vec3::scale(centroid, 1.0 / n as f64);
            shell.faces.push(Face {
                loop_edges,
                surface: Surface::Plane {
                    origin: centroid,
                    normal: vec3::scale(dir_norm, -1.0),
                },
                orientation_reversed: false,
            });
        }

        // Top face: normal = +dir_norm
        {
            let loop_edges: Vec<EdgeRef> =
                top_e.iter().take(n_edges).map(|&eid| fwd(eid)).collect();
            let centroid = top_v
                .iter()
                .fold([0.0, 0.0, 0.0], |acc, &vi| vec3::add(acc, verts[vi]));
            let centroid = vec3::scale(centroid, 1.0 / n as f64);
            shell.faces.push(Face {
                loop_edges,
                surface: Surface::Plane {
                    origin: centroid,
                    normal: dir_norm,
                },
                orientation_reversed: false,
            });
        }
    }

    // --- Side faces ---
    for i in 0..n_edges {
        let j = if is_closed { (i + 1) % n } else { i + 1 };
        let loop_edges = vec![
            fwd(bottom_e[i]),
            fwd(vert_e[j]),
            rev(top_e[i]),
            rev(vert_e[i]),
        ];

        let edge_dir = vec3::sub(verts[bottom_v[j]], verts[bottom_v[i]]);
        let normal = vec3::normalized(vec3::cross(edge_dir, dir));

        let origin = vec3::scale(
            vec3::add(
                vec3::add(verts[bottom_v[i]], verts[bottom_v[j]]),
                vec3::add(verts[top_v[i]], verts[top_v[j]]),
            ),
            0.25,
        );

        shell.faces.push(Face {
            loop_edges,
            surface: Surface::Plane { origin, normal },
            orientation_reversed: false,
        });
    }

    Ok(shell)
}

/// Extrude Shell for degree>=2 profiles.
///
/// Splits into Bezier spans; degree=1 spans become Plane faces,
/// degree>=2 spans become SurfaceOfSweep faces.
fn shell_from_extrude_nurbs(
    profile: &NurbsRegion,
    direction: [f64; 3],
    distance: f64,
) -> Result<Shell, String> {
    let spans: Vec<_> = profile
        .outer
        .iter()
        .flat_map(|c| c.to_bezier_spans())
        .collect();
    if spans.is_empty() {
        return Err("profile has no Bezier spans".to_string());
    }

    let dir = [
        direction[0] * distance,
        direction[1] * distance,
        direction[2] * distance,
    ];
    let dir_norm = vec3::normalized([direction[0], direction[1], direction[2]]);

    let mut shell = Shell::new();

    // Collect span endpoints (span i start = control_points[0])
    // Last span end = first span start (closed curve)
    let mut bottom_pts: Vec<[f64; 3]> = Vec::new();
    for span in &spans {
        bottom_pts.push(point2_to_point3(&span.control_points[0]));
    }
    let n = bottom_pts.len();

    let mut bottom_v = Vec::with_capacity(n);
    let mut top_v = Vec::with_capacity(n);
    for bp in &bottom_pts {
        bottom_v.push(shell.add_vertex(*bp));
        top_v.push(shell.add_vertex(vec3::add(*bp, dir)));
    }

    let verts = shell.vertices.clone();
    let line_curve = |a: usize, b: usize| Curve3D::Line {
        start: verts[a],
        end: verts[b],
    };

    let fwd = |eid: usize| EdgeRef {
        edge_id: eid,
        forward: true,
    };
    let rev = |eid: usize| EdgeRef {
        edge_id: eid,
        forward: false,
    };

    let mut bottom_e = Vec::with_capacity(n);
    let mut top_e = Vec::with_capacity(n);
    let mut vert_e = Vec::with_capacity(n);

    for i in 0..n {
        let j = (i + 1) % n;
        let span = &spans[i];

        if span.degree == 1 {
            bottom_e.push(shell.add_edge(
                bottom_v[i],
                bottom_v[j],
                line_curve(bottom_v[i], bottom_v[j]),
            ));
            top_e.push(shell.add_edge(top_v[i], top_v[j], line_curve(top_v[i], top_v[j])));
        } else {
            let bottom_curve = Curve3D::NurbsCurve3D {
                degree: span.degree,
                control_points: span.control_points.iter().map(point2_to_point3).collect(),
                weights: span.weights.clone(),
                knots: span.knots.clone(),
            };
            bottom_e.push(shell.add_edge(bottom_v[i], bottom_v[j], bottom_curve));

            let top_curve = Curve3D::NurbsCurve3D {
                degree: span.degree,
                control_points: span
                    .control_points
                    .iter()
                    .map(|p2| vec3::add(point2_to_point3(p2), dir))
                    .collect(),
                weights: span.weights.clone(),
                knots: span.knots.clone(),
            };
            top_e.push(shell.add_edge(top_v[i], top_v[j], top_curve));
        }

        vert_e.push(shell.add_edge(bottom_v[i], top_v[i], line_curve(bottom_v[i], top_v[i])));
    }

    // --- Bottom face ---
    {
        let mut loop_edges = Vec::with_capacity(n);
        for i in (0..n).rev() {
            loop_edges.push(rev(bottom_e[i]));
        }
        let centroid = bottom_v
            .iter()
            .fold([0.0, 0.0, 0.0], |acc, &vi| vec3::add(acc, verts[vi]));
        let centroid = vec3::scale(centroid, 1.0 / n as f64);
        shell.faces.push(Face {
            loop_edges,
            surface: Surface::Plane {
                origin: centroid,
                normal: vec3::scale(dir_norm, -1.0),
            },
            orientation_reversed: false,
        });
    }

    // --- Top face ---
    {
        let loop_edges: Vec<EdgeRef> = (0..n).map(|i| fwd(top_e[i])).collect();
        let centroid = top_v
            .iter()
            .fold([0.0, 0.0, 0.0], |acc, &vi| vec3::add(acc, verts[vi]));
        let centroid = vec3::scale(centroid, 1.0 / n as f64);
        shell.faces.push(Face {
            loop_edges,
            surface: Surface::Plane {
                origin: centroid,
                normal: dir_norm,
            },
            orientation_reversed: false,
        });
    }

    // --- Side faces ---
    // Group consecutive curved spans into a single SurfaceOfSweep face;
    // degree=1 spans get individual Plane faces.
    {
        let mut i = 0;
        while i < n {
            if spans[i].degree >= 2 {
                // Find contiguous range [i, group_end) of curved spans
                let group_start = i;
                let mut group_end = i + 1;
                while group_end < n && spans[group_end].degree >= 2 {
                    group_end += 1;
                }
                let group_len = group_end - group_start;

                // Loop: bottom(fwd) -> right vertical(fwd) -> top(rev) -> left vertical(rev)
                let mut loop_edges = Vec::new();
                for &edge in bottom_e.iter().take(group_end).skip(group_start) {
                    loop_edges.push(fwd(edge));
                }
                loop_edges.push(fwd(vert_e[(group_end) % n]));
                for k in (group_start..group_end).rev() {
                    loop_edges.push(rev(top_e[k]));
                }
                loop_edges.push(rev(vert_e[group_start]));

                // Unify control points; apply degree elevation to match max degree
                let max_degree = spans[group_start..group_end]
                    .iter()
                    .map(|s| s.degree)
                    .max()
                    .unwrap_or(2);

                let mut profile_cps: Vec<[f64; 2]> = Vec::new();
                let mut profile_weights: Vec<f64> = Vec::new();

                for span in spans.iter().take(group_end).skip(group_start) {
                    if span.degree == max_degree {
                        for p in &span.control_points {
                            profile_cps.push(*p);
                        }
                        profile_weights.extend_from_slice(&span.weights);
                    } else {
                        let elevated = elevate_bezier_2d(
                            &span.control_points,
                            &span.weights,
                            span.degree,
                            max_degree,
                        );
                        for (cp, w) in &elevated {
                            profile_cps.push(*cp);
                            profile_weights.push(*w);
                        }
                    }
                }

                // Spine: straight line from origin along extrude direction
                let spine_start = [0.0, 0.0, 0.0];
                let spine_end = [
                    direction[0] * distance,
                    direction[1] * distance,
                    direction[2] * distance,
                ];
                // Frame: fixed XY profile coordinate system
                let frame: [[f64; 3]; 3] = [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], direction];
                shell.faces.push(Face {
                    loop_edges,
                    surface: Surface::SurfaceOfSweep {
                        spine_control_points: vec![spine_start, spine_end],
                        spine_weights: vec![1.0, 1.0],
                        spine_degree: 1,
                        profile_control_points: profile_cps,
                        profile_weights,
                        profile_degree: u32::try_from(max_degree).expect("degree fits in u32"),
                        n_profile_spans: u32::try_from(group_len).expect("span count fits in u32"),
                        frames: vec![frame, frame],
                    },
                    orientation_reversed: false,
                });

                i = group_end;
            } else {
                let j = (i + 1) % n;
                let loop_edges = vec![
                    fwd(bottom_e[i]),
                    fwd(vert_e[j]),
                    rev(top_e[i]),
                    rev(vert_e[i]),
                ];
                let edge_dir = vec3::sub(verts[bottom_v[j]], verts[bottom_v[i]]);
                let normal = vec3::normalized(vec3::cross(edge_dir, dir));
                let origin = vec3::scale(
                    vec3::add(
                        vec3::add(verts[bottom_v[i]], verts[bottom_v[j]]),
                        vec3::add(verts[top_v[i]], verts[top_v[j]]),
                    ),
                    0.25,
                );
                shell.faces.push(Face {
                    loop_edges,
                    surface: Surface::Plane { origin, normal },
                    orientation_reversed: false,
                });
                i += 1;
            }
        }
    }

    Ok(shell)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::brep::Surface;
    use neco_nurbs::{NurbsCurve2D, NurbsRegion};

    #[test]
    fn extrude_degree2_nurbs_surface_faces() {
        // Unit circle from 4 rational quadratic arc spans
        let w = std::f64::consts::FRAC_1_SQRT_2;
        let circle = NurbsCurve2D::new_rational(
            2,
            vec![
                [1.0, 0.0],
                [1.0, 1.0],
                [0.0, 1.0],
                [-1.0, 1.0],
                [-1.0, 0.0],
                [-1.0, -1.0],
                [0.0, -1.0],
                [1.0, -1.0],
                [1.0, 0.0],
            ],
            vec![1.0, w, 1.0, w, 1.0, w, 1.0, w, 1.0],
            vec![
                0.0, 0.0, 0.0, 0.25, 0.25, 0.5, 0.5, 0.75, 0.75, 1.0, 1.0, 1.0,
            ],
        );
        let profile = NurbsRegion {
            outer: vec![circle],
            holes: vec![],
        };

        let shell = shell_from_extrude(&profile, [0.0, 0.0, 1.0], 2.0).unwrap();

        // bottom(1) + top(1) + 1 merged side face = 3
        assert_eq!(shell.faces.len(), 3, "expected 3 faces (merged side)");

        let sweep_count = shell
            .faces
            .iter()
            .filter(|f| matches!(f.surface, Surface::SurfaceOfSweep { .. }))
            .count();
        assert_eq!(sweep_count, 1, "expected 1 merged SurfaceOfSweep");

        for face in &shell.faces {
            if let Surface::SurfaceOfSweep {
                n_profile_spans, ..
            } = &face.surface
            {
                assert_eq!(*n_profile_spans, 4, "n_profile_spans should be 4");
            }
        }

        let plane_count = shell
            .faces
            .iter()
            .filter(|f| matches!(f.surface, Surface::Plane { .. }))
            .count();
        assert_eq!(plane_count, 2, "expected 2 Plane faces (top + bottom)");
    }
}
