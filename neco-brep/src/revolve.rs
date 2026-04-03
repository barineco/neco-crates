use neco_nurbs::{NurbsCurve2D, NurbsRegion, NurbsSurface3D};

use crate::brep::{Curve3D, EdgeRef, Face, Shell, Surface};
use crate::radians::Radians;
use crate::transform::elevate_bezier_2d;
use crate::types::Axis;
use crate::vec3;

/// Revolve a NurbsRegion around `axis` by `angle` to produce a Shell.
///
/// Profile Point2(x, y): x = radius, y = axial coordinate.
/// degree=1 -> analytic surfaces (Cylinder / Cone / Plane);
/// degree>=2 -> NurbsSurface per Bezier span.
pub fn shell_from_revolve(
    profile: &NurbsRegion,
    axis: Axis,
    angle: Radians,
) -> Result<Shell, String> {
    if profile.outer.iter().any(|c| c.degree >= 2) {
        return shell_from_revolve_nurbs(profile, axis, angle);
    }
    let (verts_2d, is_closed) = crate::transform::profile_vertices(profile)?;
    let n = verts_2d.len();
    let n_edges = if is_closed { n } else { n - 1 };
    let angle_rad = angle.0;
    let full_rotation = angle.is_full_rotation();

    let mut shell = Shell::new();

    let axis_vec = axis.direction();

    // Convert [f64; 2](radius, axis_coord) to 3D at angle theta
    let rotate_point = |p2: &[f64; 2], theta: f64| -> [f64; 3] {
        let cos_t = theta.cos();
        let sin_t = theta.sin();
        match axis {
            Axis::Y => [p2[0] * cos_t, p2[1], p2[0] * sin_t],
            Axis::X => [p2[1], p2[0] * cos_t, p2[0] * sin_t],
            Axis::Z => [p2[0] * cos_t, p2[0] * sin_t, p2[1]],
        }
    };

    let axis_point = |axis_coord: f64| -> [f64; 3] { vec3::scale(axis_vec, axis_coord) };

    let mut start_verts: Vec<usize> = Vec::with_capacity(n);
    for p2 in &verts_2d {
        start_verts.push(shell.add_vertex(rotate_point(p2, 0.0)));
    }

    let mut end_verts: Vec<usize> = Vec::with_capacity(n);
    if !full_rotation {
        for p2 in &verts_2d {
            end_verts.push(shell.add_vertex(rotate_point(p2, angle_rad)));
        }
    }

    let fwd = |eid: usize| EdgeRef {
        edge_id: eid,
        forward: true,
    };
    let rev = |eid: usize| EdgeRef {
        edge_id: eid,
        forward: false,
    };

    // One face per profile edge
    for ei in 0..n_edges {
        let ej = if is_closed { (ei + 1) % n } else { ei + 1 };
        let p_i = &verts_2d[ei];
        let p_j = &verts_2d[ej];

        let r_i = p_i[0];
        let r_j = p_j[0];
        let z_i = p_i[1];
        let z_j = p_j[1];

        // Both endpoints on axis -> zero-area face, skip
        if r_i.abs() < 1e-10 && r_j.abs() < 1e-10 {
            continue;
        }

        let vi_start = start_verts[ei];
        let vj_start = start_verts[ej];

        if full_rotation {
            let vi_end = vi_start;
            let vj_end = vj_start;

            let arc_i = if r_i.abs() < 1e-10 {
                None
            } else {
                let center_i = axis_point(z_i);
                let start_pt = shell.vertices[vi_start];
                let arc = Curve3D::Arc {
                    center: center_i,
                    axis: axis_vec,
                    start: start_pt,
                    end: start_pt,
                    radius: r_i,
                };
                let eid = shell.add_edge(vi_start, vi_end, arc);
                Some(eid)
            };

            let arc_j = if r_j.abs() < 1e-10 {
                None
            } else {
                let center_j = axis_point(z_j);
                let start_pt = shell.vertices[vj_start];
                let arc = Curve3D::Arc {
                    center: center_j,
                    axis: axis_vec,
                    start: start_pt,
                    end: start_pt,
                    radius: r_j,
                };
                let eid = shell.add_edge(vj_start, vj_end, arc);
                Some(eid)
            };

            let dr = (r_j - r_i).abs();
            let dz = (z_j - z_i).abs();

            let surface = if dr < 1e-10 {
                Surface::Cylinder {
                    origin: axis_point(z_i.min(z_j)),
                    axis: vec3::scale(axis_vec, dz),
                    radius: r_i,
                }
            } else if dz < 1e-10 {
                let normal = if r_j > r_i {
                    vec3::scale(axis_vec, -1.0)
                } else {
                    axis_vec
                };
                Surface::Plane {
                    origin: axis_point(z_i),
                    normal,
                }
            } else {
                let half_angle = (dr / dz).atan();
                let t_apex = r_i / (r_i - r_j);
                let z_apex = z_i + t_apex * (z_j - z_i);
                let far_z = if r_i > r_j { z_i } else { z_j };
                let cone_height = (far_z - z_apex).abs();
                let cone_axis = if far_z > z_apex {
                    vec3::scale(axis_vec, cone_height)
                } else {
                    vec3::scale(axis_vec, -cone_height)
                };
                Surface::Cone {
                    origin: axis_point(z_apex),
                    axis: cone_axis,
                    half_angle,
                }
            };

            match (arc_i, arc_j) {
                (Some(ei_arc), Some(ej_arc)) => {
                    if dr < 1e-10 {
                        if z_j > z_i {
                            shell.faces.push(Face {
                                loop_edges: vec![fwd(ej_arc), rev(ei_arc)],
                                surface,
                                orientation_reversed: false,
                            });
                        } else {
                            shell.faces.push(Face {
                                loop_edges: vec![fwd(ei_arc), rev(ej_arc)],
                                surface,
                                orientation_reversed: false,
                            });
                        }
                    } else if dz < 1e-10 {
                        if r_j > r_i {
                            shell.faces.push(Face {
                                loop_edges: vec![fwd(ej_arc), rev(ei_arc)],
                                surface,
                                orientation_reversed: false,
                            });
                        } else {
                            shell.faces.push(Face {
                                loop_edges: vec![fwd(ei_arc), rev(ej_arc)],
                                surface,
                                orientation_reversed: false,
                            });
                        }
                    } else {
                        shell.faces.push(Face {
                            loop_edges: vec![fwd(ej_arc), rev(ei_arc)],
                            surface,
                            orientation_reversed: false,
                        });
                    }
                }
                (Some(ei_arc), None) => {
                    shell.faces.push(Face {
                        loop_edges: vec![fwd(ei_arc)],
                        surface,
                        orientation_reversed: false,
                    });
                }
                (None, Some(ej_arc)) => {
                    shell.faces.push(Face {
                        loop_edges: vec![fwd(ej_arc)],
                        surface,
                        orientation_reversed: false,
                    });
                }
                (None, None) => {}
            }
        } else {
            // Partial rotation
            let vi_end = end_verts[ei];
            let vj_end = end_verts[ej];

            let line_start = {
                let vs = &shell.vertices;
                Curve3D::Line {
                    start: vs[vi_start],
                    end: vs[vj_start],
                }
            };
            let e_line_start = shell.add_edge(vi_start, vj_start, line_start);

            let line_end = {
                let vs = &shell.vertices;
                Curve3D::Line {
                    start: vs[vi_end],
                    end: vs[vj_end],
                }
            };
            let e_line_end = shell.add_edge(vi_end, vj_end, line_end);

            let arc_i_eid = if r_i.abs() < 1e-10 {
                None
            } else {
                let center_i = axis_point(z_i);
                let start_pt = shell.vertices[vi_start];
                let end_pt = shell.vertices[vi_end];
                let arc = Curve3D::Arc {
                    center: center_i,
                    axis: axis_vec,
                    start: start_pt,
                    end: end_pt,
                    radius: r_i,
                };
                Some(shell.add_edge(vi_start, vi_end, arc))
            };

            let arc_j_eid = if r_j.abs() < 1e-10 {
                None
            } else {
                let center_j = axis_point(z_j);
                let start_pt = shell.vertices[vj_start];
                let end_pt = shell.vertices[vj_end];
                let arc = Curve3D::Arc {
                    center: center_j,
                    axis: axis_vec,
                    start: start_pt,
                    end: end_pt,
                    radius: r_j,
                };
                Some(shell.add_edge(vj_start, vj_end, arc))
            };

            let dr = (r_j - r_i).abs();
            let dz = (z_j - z_i).abs();

            let surface = if dr < 1e-10 {
                Surface::Cylinder {
                    origin: axis_point(z_i.min(z_j)),
                    axis: vec3::scale(axis_vec, dz),
                    radius: r_i,
                }
            } else if dz < 1e-10 {
                let normal = if r_j > r_i {
                    vec3::scale(axis_vec, -1.0)
                } else {
                    axis_vec
                };
                Surface::Plane {
                    origin: axis_point(z_i),
                    normal,
                }
            } else {
                let half_angle = (dr / dz).atan();
                let t_apex = r_i / (r_i - r_j);
                let z_apex = z_i + t_apex * (z_j - z_i);
                let far_z = if r_i > r_j { z_i } else { z_j };
                let cone_height = (far_z - z_apex).abs();
                let cone_axis = if far_z > z_apex {
                    vec3::scale(axis_vec, cone_height)
                } else {
                    vec3::scale(axis_vec, -cone_height)
                };
                Surface::Cone {
                    origin: axis_point(z_apex),
                    axis: cone_axis,
                    half_angle,
                }
            };

            let mut loop_edges = Vec::new();
            loop_edges.push(fwd(e_line_start));
            if let Some(ej_arc) = arc_j_eid {
                loop_edges.push(fwd(ej_arc));
            }
            loop_edges.push(rev(e_line_end));
            if let Some(ei_arc) = arc_i_eid {
                loop_edges.push(rev(ei_arc));
            }

            shell.faces.push(Face {
                loop_edges,
                surface,
                orientation_reversed: false,
            });
        }
    }

    // Cap faces for partial rotation
    if !full_rotation {
        add_cap_face(&mut shell, &start_verts, false);
        add_cap_face(&mut shell, &end_verts, true);
    }

    Ok(shell)
}

/// Add a cap face from profile vertex loop.
///
/// `reverse` = false: start cap (normal opposes rotation direction).
/// `reverse` = true: end cap (normal follows rotation direction).
fn add_cap_face(shell: &mut Shell, vert_ids: &[usize], reverse: bool) {
    let n = vert_ids.len();
    if n < 3 {
        return;
    }

    // Remove duplicates (on-axis vertices may coincide)
    let mut unique_ids: Vec<usize> = Vec::with_capacity(n);
    for &vid in vert_ids {
        if unique_ids.last().copied() != Some(vid) {
            unique_ids.push(vid);
        }
    }
    if unique_ids.len() < 3 {
        return;
    }

    let order: Vec<usize> = if reverse {
        (0..unique_ids.len()).collect()
    } else {
        (0..unique_ids.len()).rev().collect()
    };

    let mut loop_edges = Vec::with_capacity(unique_ids.len());
    for i in 0..order.len() {
        let vi = unique_ids[order[i]];
        let vj = unique_ids[order[(i + 1) % order.len()]];
        let verts = &shell.vertices;
        let curve = Curve3D::Line {
            start: verts[vi],
            end: verts[vj],
        };
        let eid = shell.add_edge(vi, vj, curve);
        loop_edges.push(EdgeRef {
            edge_id: eid,
            forward: true,
        });
    }

    // Normal from first 3 vertices
    let verts = &shell.vertices;
    let p0 = verts[unique_ids[order[0]]];
    let p1 = verts[unique_ids[order[1]]];
    let p2 = verts[unique_ids[order[2]]];
    let edge1 = vec3::sub(p1, p0);
    let edge2 = vec3::sub(p2, p0);
    let normal = vec3::normalized(vec3::cross(edge1, edge2));
    let origin = vec3::scale(
        unique_ids
            .iter()
            .fold([0.0, 0.0, 0.0], |acc, &vi| vec3::add(acc, verts[vi])),
        1.0 / unique_ids.len() as f64,
    );

    shell.faces.push(Face {
        loop_edges,
        surface: Surface::Plane { origin, normal },
        orientation_reversed: false,
    });
}

/// Convert 2D profile point (radius, axis_coord) to 3D at rotation angle theta.
fn rotate_profile_point(r: f64, z: f64, theta: f64, axis: Axis) -> [f64; 3] {
    let cos_t = theta.cos();
    let sin_t = theta.sin();
    match axis {
        Axis::Y => [r * cos_t, z, r * sin_t],
        Axis::X => [z, r * cos_t, r * sin_t],
        Axis::Z => [r * cos_t, r * sin_t, z],
    }
}

/// Generate rational quadratic arc NURBS control points (3 points).
///
/// Mid-point weight = cos(angle/2), mid-point distance = r / cos(angle/2).
fn arc_control_points(
    r: f64,
    z: f64,
    theta_start: f64,
    angle: f64,
    axis: Axis,
) -> [([f64; 3], f64); 3] {
    let half = angle / 2.0;
    let w_mid = half.cos();
    let theta_end = theta_start + angle;
    let theta_mid = theta_start + half;

    let p_start = rotate_profile_point(r, z, theta_start, axis);
    let p_end = rotate_profile_point(r, z, theta_end, axis);
    let p_mid = rotate_profile_point(r / w_mid, z, theta_mid, axis);

    [(p_start, 1.0), (p_mid, w_mid), (p_end, 1.0)]
}

/// Build a NurbsSurface3D by revolving a 2D Bezier span around an axis.
///
/// u = profile curve direction (degree_u = span.degree),
/// v = rotation direction (degree_v = 2, rational quadratic segments).
fn nurbs_surface_from_revolve_span(
    span: &NurbsCurve2D,
    axis: Axis,
    angle: Radians,
) -> NurbsSurface3D {
    let angle_rad = angle.0;
    let n_u = span.control_points.len();
    let n_segments = (angle_rad / std::f64::consts::FRAC_PI_2).ceil() as usize;
    let theta_seg = angle_rad / n_segments as f64;
    let n_v = n_segments * 2 + 1;

    let mut control_points = Vec::with_capacity(n_u);
    let mut weights = Vec::with_capacity(n_u);

    for i in 0..n_u {
        let r = span.control_points[i][0];
        let z = span.control_points[i][1];
        let w_profile = span.weights[i];

        let mut row_pts = Vec::with_capacity(n_v);
        let mut row_wts = Vec::with_capacity(n_v);

        if r.abs() < 1e-14 {
            let pt = rotate_profile_point(0.0, z, 0.0, axis);
            for _j in 0..n_v {
                row_pts.push(pt);
                row_wts.push(w_profile);
            }
        } else {
            for seg in 0..n_segments {
                let theta_start = theta_seg * seg as f64;
                let arc = arc_control_points(r, z, theta_start, theta_seg, axis);

                if seg == 0 {
                    row_pts.push(arc[0].0);
                    row_wts.push(w_profile * arc[0].1);
                }
                row_pts.push(arc[1].0);
                row_wts.push(w_profile * arc[1].1);
                row_pts.push(arc[2].0);
                row_wts.push(w_profile * arc[2].1);
            }
        }

        control_points.push(row_pts);
        weights.push(row_wts);
    }

    // v-direction knot vector: uniform segments with double knots at boundaries
    let mut knots_v = vec![0.0; 3];
    for seg in 1..n_segments {
        let knot = seg as f64 / n_segments as f64;
        knots_v.push(knot);
        knots_v.push(knot);
    }
    knots_v.extend_from_slice(&[1.0, 1.0, 1.0]);

    NurbsSurface3D {
        degree_u: span.degree,
        degree_v: 2,
        control_points,
        weights,
        knots_u: span.knots.clone(),
        knots_v,
    }
}

/// Check if a Bezier span is a circular arc; returns (center_r, center_z) if so.
fn is_circular_arc_span(span: &NurbsCurve2D) -> Option<(f64, f64)> {
    if span.degree != 2 || span.control_points.len() != 3 {
        return None;
    }
    let p0 = &span.control_points[0];
    let p1 = &span.control_points[1];
    let p2 = &span.control_points[2];
    let w0 = span.weights[0];
    let w1 = span.weights[1];
    let w2 = span.weights[2];

    if (w0 - 1.0).abs() > 1e-10 || (w2 - 1.0).abs() > 1e-10 {
        return None;
    }
    if w1 <= 0.0 || w1 > 1.0 + 1e-10 {
        return None;
    }

    let d0x = p1[0] - p0[0];
    let d0y = p1[1] - p0[1];
    let d1x = p2[0] - p1[0];
    let d1y = p2[1] - p1[1];

    let a11 = -d0y;
    let a12 = d1y;
    let a21 = d0x;
    let a22 = -d1x;
    let b1 = p2[0] - p0[0];
    let b2 = p2[1] - p0[1];
    let det = a11 * a22 - a12 * a21;
    if det.abs() < 1e-14 {
        return None;
    }
    let s = (b1 * a22 - b2 * a12) / det;

    let center_r = p0[0] + s * (-d0y);
    let center_z = p0[1] + s * d0x;

    let radius = ((p0[0] - center_r).powi(2) + (p0[1] - center_z).powi(2)).sqrt();
    let r_check = ((p2[0] - center_r).powi(2) + (p2[1] - center_z).powi(2)).sqrt();

    if (radius - r_check).abs() > 1e-8 {
        return None;
    }

    Some((center_r, center_z))
}

/// Detect if the entire profile forms a single circle; returns (center_r, center_z, radius).
fn detect_circle_profile(spans: &[NurbsCurve2D]) -> Option<(f64, f64, f64)> {
    if spans.is_empty() {
        return None;
    }
    let mut center: Option<(f64, f64)> = None;
    let mut radius: Option<f64> = None;

    for span in spans {
        let (cr, cz) = is_circular_arc_span(span)?;
        let r = ((span.control_points[0][0] - cr).powi(2)
            + (span.control_points[0][1] - cz).powi(2))
        .sqrt();

        if let Some((prev_cr, prev_cz)) = center {
            if (cr - prev_cr).abs() > 1e-10 || (cz - prev_cz).abs() > 1e-10 {
                return None;
            }
        }
        if let Some(prev_r) = radius {
            if (r - prev_r).abs() > 1e-10 {
                return None;
            }
        }
        center = Some((cr, cz));
        radius = Some(r);
    }

    let (cr, cz) = center?;
    let r = radius?;
    Some((cr, cz, r))
}

/// Build edge loop for a single revolve span.
#[allow(clippy::too_many_arguments)]
fn build_revolve_loop_edges(
    shell: &mut Shell,
    axis_vec: &[f64; 3],
    start_vids: &[usize],
    end_vids: &[usize],
    i: usize,
    j: usize,
    span: &NurbsCurve2D,
    full_rotation: bool,
    fwd: &dyn Fn(usize) -> EdgeRef,
    rev: &dyn Fn(usize) -> EdgeRef,
) -> Vec<EdgeRef> {
    let r_i = span.control_points[0][0];
    let r_j = span.control_points[span.control_points.len() - 1][0];
    let mut loop_edges = Vec::new();

    if full_rotation {
        if r_i.abs() >= 1e-10 {
            let z_i = span.control_points[0][1];
            let center_i = vec3::scale(*axis_vec, z_i);
            let start_pt = shell.vertices[start_vids[i]];
            let arc_i = Curve3D::Arc {
                center: center_i,
                axis: *axis_vec,
                start: start_pt,
                end: start_pt,
                radius: r_i,
            };
            let eid = shell.add_edge(start_vids[i], start_vids[i], arc_i);
            loop_edges.push(fwd(eid));
        }
        if r_j.abs() >= 1e-10 {
            let z_j = span.control_points[span.control_points.len() - 1][1];
            let center_j = vec3::scale(*axis_vec, z_j);
            let start_pt = shell.vertices[start_vids[j]];
            let arc_j = Curve3D::Arc {
                center: center_j,
                axis: *axis_vec,
                start: start_pt,
                end: start_pt,
                radius: r_j,
            };
            let eid = shell.add_edge(start_vids[j], start_vids[j], arc_j);
            loop_edges.push(rev(eid));
        }
    } else {
        let curve_start = Curve3D::Line {
            start: shell.vertices[start_vids[i]],
            end: shell.vertices[start_vids[j]],
        };
        let eid_start = shell.add_edge(start_vids[i], start_vids[j], curve_start);
        loop_edges.push(fwd(eid_start));

        if r_j.abs() >= 1e-10 {
            let z_j = span.control_points[span.control_points.len() - 1][1];
            let center_j = vec3::scale(*axis_vec, z_j);
            let arc_j = Curve3D::Arc {
                center: center_j,
                axis: *axis_vec,
                start: shell.vertices[start_vids[j]],
                end: shell.vertices[end_vids[j]],
                radius: r_j,
            };
            let eid = shell.add_edge(start_vids[j], end_vids[j], arc_j);
            loop_edges.push(fwd(eid));
        }

        let curve_end = Curve3D::Line {
            start: shell.vertices[end_vids[i]],
            end: shell.vertices[end_vids[j]],
        };
        let eid_end = shell.add_edge(end_vids[i], end_vids[j], curve_end);
        loop_edges.push(rev(eid_end));

        if r_i.abs() >= 1e-10 {
            let z_i = span.control_points[0][1];
            let center_i = vec3::scale(*axis_vec, z_i);
            let arc_i = Curve3D::Arc {
                center: center_i,
                axis: *axis_vec,
                start: shell.vertices[start_vids[i]],
                end: shell.vertices[end_vids[i]],
                radius: r_i,
            };
            let eid = shell.add_edge(start_vids[i], end_vids[i], arc_i);
            loop_edges.push(rev(eid));
        }
    }
    loop_edges
}

/// Revolve Shell for degree>=2 profiles.
///
/// Circular profiles use NurbsSurface per span; non-circular profiles
/// merge contiguous non-degenerate spans into SurfaceOfRevolution faces.
/// Supports arbitrary partial rotation angles.
fn shell_from_revolve_nurbs(
    profile: &NurbsRegion,
    axis: Axis,
    angle: Radians,
) -> Result<Shell, String> {
    let angle_rad = angle.0;
    debug_assert!(
        angle_rad > 0.0 && angle_rad <= std::f64::consts::TAU,
        "angle_rad must be in (0, TAU]: {angle_rad}"
    );
    let full_rotation = angle.is_full_rotation();

    let spans: Vec<_> = profile
        .outer
        .iter()
        .flat_map(|c| c.to_bezier_spans())
        .collect();
    if spans.is_empty() {
        return Err("profile has no Bezier spans".to_string());
    }
    let is_circular = detect_circle_profile(&spans);

    let mut shell = Shell::new();
    let axis_vec = axis.direction();

    // Frame aligned with rotate_profile_point at theta=0
    let (frame_u, frame_v) = match axis {
        Axis::Y => ([1.0, 0.0, 0.0], [0.0, 0.0, 1.0]),
        Axis::X => ([0.0, 1.0, 0.0], [0.0, 0.0, 1.0]),
        Axis::Z => ([1.0, 0.0, 0.0], [0.0, 1.0, 0.0]),
    };
    let n = spans.len();

    let fwd = |eid: usize| EdgeRef {
        edge_id: eid,
        forward: true,
    };
    let rev = |eid: usize| EdgeRef {
        edge_id: eid,
        forward: false,
    };

    // Vertices at theta=0
    let mut start_vids = Vec::with_capacity(n);
    for span in &spans {
        let p2 = &span.control_points[0];
        let pt = rotate_profile_point(p2[0], p2[1], 0.0, axis);
        start_vids.push(shell.add_vertex(pt));
    }

    // Vertices at theta=angle
    let end_vids: Vec<usize> = if full_rotation {
        start_vids.clone()
    } else {
        spans
            .iter()
            .enumerate()
            .map(|(i, span)| {
                let r = span.control_points[0][0];
                if r.abs() < 1e-10 {
                    start_vids[i]
                } else {
                    let pt = rotate_profile_point(r, span.control_points[0][1], angle_rad, axis);
                    shell.add_vertex(pt)
                }
            })
            .collect()
    };

    if is_circular.is_some() {
        // Circular profile: NurbsSurface per span
        for (i, span) in spans.iter().enumerate() {
            let j = (i + 1) % n;
            let r_i = span.control_points[0][0];
            let r_j = span.control_points[span.control_points.len() - 1][0];
            if r_i.abs() < 1e-10 && r_j.abs() < 1e-10 {
                continue;
            }
            let nurbs = nurbs_surface_from_revolve_span(span, axis, angle);
            let surface = Surface::NurbsSurface {
                data: Box::new(nurbs),
            };
            let loop_edges = build_revolve_loop_edges(
                &mut shell,
                &axis_vec,
                &start_vids,
                &end_vids,
                i,
                j,
                span,
                full_rotation,
                &fwd,
                &rev,
            );
            shell.faces.push(Face {
                loop_edges,
                surface,
                orientation_reversed: false,
            });
        }
    } else {
        // Non-circular profile: merge contiguous non-degenerate spans
        let mut i = 0;
        while i < n {
            let span = &spans[i];
            let r_i = span.control_points[0][0];
            let r_j = span.control_points[span.control_points.len() - 1][0];

            if r_i.abs() < 1e-10 && r_j.abs() < 1e-10 {
                i += 1;
                continue;
            }

            let group_start = i;
            let mut group_end = i + 1;
            while group_end < n {
                let gs = &spans[group_end];
                let gr_i = gs.control_points[0][0];
                let gr_j = gs.control_points[gs.control_points.len() - 1][0];
                if gr_i.abs() < 1e-10 && gr_j.abs() < 1e-10 {
                    break;
                }
                group_end += 1;
            }
            let group_len = group_end - group_start;

            let max_degree = spans[group_start..group_end]
                .iter()
                .map(|s| s.degree)
                .max()
                .unwrap_or(2);

            let mut profile_cps: Vec<[f64; 2]> = Vec::new();
            let mut profile_weights: Vec<f64> = Vec::new();

            for gs in spans.iter().take(group_end).skip(group_start) {
                if gs.degree == max_degree {
                    for p in &gs.control_points {
                        profile_cps.push(*p);
                    }
                    profile_weights.extend_from_slice(&gs.weights);
                } else {
                    let elevated =
                        elevate_bezier_2d(&gs.control_points, &gs.weights, gs.degree, max_degree);
                    for (cp, w) in &elevated {
                        profile_cps.push(*cp);
                        profile_weights.push(*w);
                    }
                }
            }

            let surface = Surface::SurfaceOfRevolution {
                center: [0.0, 0.0, 0.0],
                axis: axis_vec,
                frame_u,
                frame_v,
                profile_control_points: profile_cps,
                profile_weights,
                profile_degree: u32::try_from(max_degree).expect("degree fits in u32"),
                n_profile_spans: u32::try_from(group_len).expect("span count fits in u32"),
                theta_start: 0.0,
                theta_range: angle_rad,
            };

            let first_span = &spans[group_start];
            let last_span = &spans[group_end - 1];
            let first_j = group_start;
            let last_j = group_end % n;
            let r_first = first_span.control_points[0][0];
            let r_last = last_span.control_points[last_span.control_points.len() - 1][0];

            let mut loop_edges = Vec::new();

            if full_rotation {
                if r_first.abs() >= 1e-10 {
                    let z = first_span.control_points[0][1];
                    let center_pt = vec3::scale(axis_vec, z);
                    let start_pt = shell.vertices[start_vids[first_j]];
                    let arc = Curve3D::Arc {
                        center: center_pt,
                        axis: axis_vec,
                        start: start_pt,
                        end: start_pt,
                        radius: r_first,
                    };
                    let eid = shell.add_edge(start_vids[first_j], start_vids[first_j], arc);
                    loop_edges.push(fwd(eid));
                }
                if r_last.abs() >= 1e-10 {
                    let z = last_span.control_points[last_span.control_points.len() - 1][1];
                    let center_pt = vec3::scale(axis_vec, z);
                    let start_pt = shell.vertices[start_vids[last_j]];
                    let arc = Curve3D::Arc {
                        center: center_pt,
                        axis: axis_vec,
                        start: start_pt,
                        end: start_pt,
                        radius: r_last,
                    };
                    let eid = shell.add_edge(start_vids[last_j], start_vids[last_j], arc);
                    loop_edges.push(rev(eid));
                }
            } else {
                // Profile edge at theta=0
                let curve_start = Curve3D::Line {
                    start: shell.vertices[start_vids[first_j]],
                    end: shell.vertices[start_vids[last_j]],
                };
                let eid_start =
                    shell.add_edge(start_vids[first_j], start_vids[last_j], curve_start);
                loop_edges.push(fwd(eid_start));

                if r_last.abs() >= 1e-10 {
                    let z = last_span.control_points[last_span.control_points.len() - 1][1];
                    let center_pt = vec3::scale(axis_vec, z);
                    let arc = Curve3D::Arc {
                        center: center_pt,
                        axis: axis_vec,
                        start: shell.vertices[start_vids[last_j]],
                        end: shell.vertices[end_vids[last_j]],
                        radius: r_last,
                    };
                    let eid = shell.add_edge(start_vids[last_j], end_vids[last_j], arc);
                    loop_edges.push(fwd(eid));
                }

                let curve_end = Curve3D::Line {
                    start: shell.vertices[end_vids[first_j]],
                    end: shell.vertices[end_vids[last_j]],
                };
                let eid_end = shell.add_edge(end_vids[first_j], end_vids[last_j], curve_end);
                loop_edges.push(rev(eid_end));

                if r_first.abs() >= 1e-10 {
                    let z = first_span.control_points[0][1];
                    let center_pt = vec3::scale(axis_vec, z);
                    let arc = Curve3D::Arc {
                        center: center_pt,
                        axis: axis_vec,
                        start: shell.vertices[start_vids[first_j]],
                        end: shell.vertices[end_vids[first_j]],
                        radius: r_first,
                    };
                    let eid = shell.add_edge(start_vids[first_j], end_vids[first_j], arc);
                    loop_edges.push(rev(eid));
                }
            }

            shell.faces.push(Face {
                loop_edges,
                surface,
                orientation_reversed: false,
            });
            i = group_end;
        }
    }

    if !full_rotation {
        add_cap_face(&mut shell, &start_vids, false);
        add_cap_face(&mut shell, &end_vids, true);
    }

    Ok(shell)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::brep::Surface;
    use neco_nurbs::NurbsCurve2D;

    /// Closed degree-2 revolve profile (3 spans)
    fn make_nurbs_revolve_profile() -> NurbsRegion {
        let outer = NurbsCurve2D::new(
            2,
            vec![
                [1.0, 0.0],
                [1.0, 1.0],
                [0.0, 1.0],
                [0.0, 0.5],
                [0.0, 0.0],
                [0.5, 0.0],
                [1.0, 0.0],
            ],
            vec![
                0.0,
                0.0,
                0.0,
                1.0 / 3.0,
                1.0 / 3.0,
                2.0 / 3.0,
                2.0 / 3.0,
                1.0,
                1.0,
                1.0,
            ],
        );
        NurbsRegion {
            outer: vec![outer],
            holes: vec![],
        }
    }

    #[test]
    fn revolve_degree2_nurbs_surface_faces() {
        let w = std::f64::consts::FRAC_1_SQRT_2;
        let cx = 2.0_f64;
        let cr = 0.5_f64;

        let circle = NurbsCurve2D::new_rational(
            2,
            vec![
                [cx + cr, 0.0],
                [cx + cr, cr],
                [cx, cr],
                [cx - cr, cr],
                [cx - cr, 0.0],
                [cx - cr, -cr],
                [cx, -cr],
                [cx + cr, -cr],
                [cx + cr, 0.0],
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

        let shell = shell_from_revolve(&profile, Axis::Y, Radians::from_degrees(360.0)).unwrap();

        let nurbs_count = shell
            .faces
            .iter()
            .filter(|f| matches!(f.surface, Surface::NurbsSurface { .. }))
            .count();
        assert_eq!(nurbs_count, 4, "expected 4 NurbsSurface faces");
    }

    #[test]
    fn shell_from_revolve_nurbs_90deg() {
        let profile = make_nurbs_revolve_profile();
        let shell =
            shell_from_revolve_nurbs(&profile, Axis::Y, Radians::from_degrees(90.0)).unwrap();
        assert!(shell.faces.len() >= 3, "face count: {}", shell.faces.len());
        let n_sor = shell
            .faces
            .iter()
            .filter(|f| matches!(f.surface, Surface::SurfaceOfRevolution { .. }))
            .count();
        assert!(n_sor > 0, "should have SurfaceOfRevolution faces");
    }

    #[test]
    fn shell_from_revolve_nurbs_120deg() {
        let profile = make_nurbs_revolve_profile();
        let shell =
            shell_from_revolve_nurbs(&profile, Axis::Y, Radians::from_degrees(120.0)).unwrap();
        assert!(shell.faces.len() >= 3, "face count: {}", shell.faces.len());
    }

    #[test]
    fn shell_from_revolve_nurbs_180deg() {
        let profile = make_nurbs_revolve_profile();
        let shell =
            shell_from_revolve_nurbs(&profile, Axis::Y, Radians::from_degrees(180.0)).unwrap();
        assert!(
            shell.faces.len() >= 3,
            "face count: {} (SurfaceOfRevolution + caps)",
            shell.faces.len()
        );
    }

    #[test]
    fn shell_from_revolve_nurbs_360deg_regression() {
        let profile = make_nurbs_revolve_profile();
        let shell =
            shell_from_revolve_nurbs(&profile, Axis::Y, Radians::from_degrees(360.0)).unwrap();
        let n_sor = shell
            .faces
            .iter()
            .filter(|f| matches!(f.surface, Surface::SurfaceOfRevolution { .. }))
            .count();
        assert!(
            n_sor > 0,
            "360 deg should still have SurfaceOfRevolution faces"
        );
    }

    #[test]
    fn nurbs_surface_revolve_span_180deg() {
        let span = NurbsCurve2D::new(
            2,
            vec![[1.0, 0.0], [1.0, 1.0], [0.0, 1.0]],
            vec![0.0, 0.0, 0.0, 1.0, 1.0, 1.0],
        );
        let spans = span.to_bezier_spans();
        let span = &spans[0];
        let angle = Radians::PI;
        let surf = nurbs_surface_from_revolve_span(span, Axis::Y, angle);
        assert_eq!(surf.control_points[0].len(), 5);
        assert_eq!(surf.knots_v.len(), 8);
    }

    #[test]
    fn nurbs_surface_revolve_span_360deg_regression() {
        let span = NurbsCurve2D::new(
            2,
            vec![[1.0, 0.0], [1.0, 1.0], [0.0, 1.0]],
            vec![0.0, 0.0, 0.0, 1.0, 1.0, 1.0],
        );
        let spans = span.to_bezier_spans();
        let span = &spans[0];
        let surf = nurbs_surface_from_revolve_span(span, Axis::Y, Radians::TAU);
        assert_eq!(surf.control_points[0].len(), 9);
        assert_eq!(surf.knots_v.len(), 12);
    }

    #[test]
    fn arc_control_points_90deg() {
        let arc = arc_control_points(1.0, 0.0, 0.0, std::f64::consts::FRAC_PI_2, Axis::Y);
        assert_eq!(arc.len(), 3);
        assert!((arc[1].1 - std::f64::consts::FRAC_1_SQRT_2).abs() < 1e-10);
        assert!((arc[0].0[0] - 1.0).abs() < 1e-10);
        assert!(arc[0].0[2].abs() < 1e-10);
        assert!(arc[2].0[0].abs() < 1e-10);
        assert!((arc[2].0[2] - 1.0).abs() < 1e-10);
    }

    #[test]
    fn arc_control_points_60deg() {
        let angle = std::f64::consts::PI / 3.0;
        let arc = arc_control_points(1.0, 0.0, 0.0, angle, Axis::Y);
        let expected_w = (angle / 2.0).cos();
        assert!((arc[1].1 - expected_w).abs() < 1e-10);
        let mid = arc[1].0;
        let r_mid = (mid[0] * mid[0] + mid[2] * mid[2]).sqrt();
        assert!((r_mid - 1.0 / expected_w).abs() < 1e-10);
    }

    #[test]
    fn arc_control_points_45deg() {
        let angle = std::f64::consts::PI / 4.0;
        let arc = arc_control_points(1.0, 0.0, 0.0, angle, Axis::Y);
        let expected_w = (angle / 2.0).cos();
        assert!((arc[1].1 - expected_w).abs() < 1e-10);
    }
}
