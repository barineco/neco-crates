//! Loft (connect multiple cross-section profiles) B-Rep generation.

use neco_nurbs::NurbsCurve2D;

use crate::brep::{Curve3D, EdgeRef, Face, Shell, Surface};
use crate::types::{LoftMode, LoftSection};
use crate::vec3;

/// Transform a point by a 4x4 matrix.
fn transform_point(m: &[[f64; 4]; 4], p: [f64; 3]) -> [f64; 3] {
    [
        m[0][0] * p[0] + m[0][1] * p[1] + m[0][2] * p[2] + m[0][3],
        m[1][0] * p[0] + m[1][1] * p[1] + m[1][2] * p[2] + m[1][3],
        m[2][0] * p[0] + m[2][1] * p[1] + m[2][2] * p[2] + m[2][3],
    ]
}

/// Loft sections into a Shell.
///
/// Straight mode: ruled surface (degree_u=1).
/// Smooth mode: Catmull-Rom to cubic Bezier (degree_u=3).
/// Generates faces from corresponding Bezier spans of each section pair,
/// plus cap faces (Plane) at first and last sections.
pub fn shell_from_loft(sections: &[LoftSection], mode: LoftMode) -> Result<Shell, String> {
    if sections.len() < 2 {
        return Err("loft: at least 2 sections required".into());
    }
    if mode == LoftMode::Smooth {
        return shell_from_loft_smooth(sections);
    }

    let all_spans: Vec<Vec<NurbsCurve2D>> = sections
        .iter()
        .map(|s| {
            s.profile
                .outer
                .iter()
                .flat_map(|c| c.to_bezier_spans())
                .collect::<Vec<_>>()
        })
        .collect();

    let n_spans = all_spans[0].len();
    if n_spans == 0 {
        return Err("loft: profile has no Bezier spans".into());
    }
    for (si, spans) in all_spans.iter().enumerate() {
        if spans.len() != n_spans {
            return Err(format!(
                "loft: section {} has {} spans, expected {} (from section 0)",
                si,
                spans.len(),
                n_spans
            ));
        }
    }

    let n_sections = sections.len();
    let mut shell = Shell::new();

    let to_world_3d = |sec: &LoftSection, p2: &[f64; 2]| -> [f64; 3] {
        transform_point(&sec.transform, [p2[0], p2[1], 0.0])
    };

    let mut vertex_ids: Vec<Vec<usize>> = Vec::with_capacity(n_sections);
    for (si, sec) in sections.iter().enumerate() {
        let mut sec_vids = Vec::with_capacity(n_spans);
        for span in &all_spans[si] {
            let p3 = to_world_3d(sec, &span.control_points[0]);
            sec_vids.push(shell.add_vertex(p3));
        }
        vertex_ids.push(sec_vids);
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

    // --- Side faces (ruled surfaces between adjacent sections) ---
    for layer in 0..n_sections - 1 {
        let sec0 = &sections[layer];
        let sec1 = &sections[layer + 1];
        let vids0 = &vertex_ids[layer];
        let vids1 = &vertex_ids[layer + 1];

        let mut profile_e0 = Vec::with_capacity(n_spans);
        let mut profile_e1 = Vec::with_capacity(n_spans);
        let mut vert_e = Vec::with_capacity(n_spans);

        for i in 0..n_spans {
            let j = (i + 1) % n_spans;
            let span0 = &all_spans[layer][i];
            let span1 = &all_spans[layer + 1][i];

            if span0.degree == 1 {
                profile_e0.push(shell.add_edge(vids0[i], vids0[j], line_curve(vids0[i], vids0[j])));
            } else {
                let curve = Curve3D::NurbsCurve3D {
                    degree: span0.degree,
                    control_points: span0
                        .control_points
                        .iter()
                        .map(|p2| to_world_3d(sec0, p2))
                        .collect(),
                    weights: span0.weights.clone(),
                    knots: span0.knots.clone(),
                };
                profile_e0.push(shell.add_edge(vids0[i], vids0[j], curve));
            }

            if span1.degree == 1 {
                profile_e1.push(shell.add_edge(vids1[i], vids1[j], line_curve(vids1[i], vids1[j])));
            } else {
                let curve = Curve3D::NurbsCurve3D {
                    degree: span1.degree,
                    control_points: span1
                        .control_points
                        .iter()
                        .map(|p2| to_world_3d(sec1, p2))
                        .collect(),
                    weights: span1.weights.clone(),
                    knots: span1.knots.clone(),
                };
                profile_e1.push(shell.add_edge(vids1[i], vids1[j], curve));
            }

            vert_e.push(shell.add_edge(vids0[i], vids1[i], line_curve(vids0[i], vids1[i])));
        }

        let nurbs =
            nurbs_surface_from_loft_unified(&all_spans[layer], &all_spans[layer + 1], sec0, sec1);

        let mut loop_edges = Vec::new();
        for &edge in profile_e0.iter().take(n_spans) {
            loop_edges.push(fwd(edge));
        }
        loop_edges.push(fwd(vert_e[0]));
        for i in (0..n_spans).rev() {
            loop_edges.push(rev(profile_e1[i]));
        }
        loop_edges.push(rev(vert_e[0]));

        shell.faces.push(Face {
            loop_edges,
            surface: Surface::NurbsSurface {
                data: Box::new(nurbs),
            },
            orientation_reversed: false,
        });
    }

    // --- Cap faces ---
    // Bottom (section 0)
    {
        let sec0 = &sections[0];
        let sec1 = &sections[1];
        let p0 = to_world_3d(sec0, &[0.0, 0.0]);
        let p1 = to_world_3d(sec1, &[0.0, 0.0]);
        let dir = vec3::sub(p1, p0);
        let cap_normal = vec3::neg(vec3::normalized(dir));
        let vids = &vertex_ids[0];

        let mut cap_edges = Vec::with_capacity(n_spans);
        for i in 0..n_spans {
            let j = (i + 1) % n_spans;
            let span = &all_spans[0][i];
            if span.degree == 1 {
                cap_edges.push(shell.add_edge(vids[i], vids[j], line_curve(vids[i], vids[j])));
            } else {
                let curve = Curve3D::NurbsCurve3D {
                    degree: span.degree,
                    control_points: span
                        .control_points
                        .iter()
                        .map(|p2| to_world_3d(sec0, p2))
                        .collect(),
                    weights: span.weights.clone(),
                    knots: span.knots.clone(),
                };
                cap_edges.push(shell.add_edge(vids[i], vids[j], curve));
            }
        }
        let loop_edges: Vec<EdgeRef> = (0..n_spans).rev().map(|i| rev(cap_edges[i])).collect();
        let centroid = vids
            .iter()
            .fold([0.0, 0.0, 0.0], |acc, &vi| vec3::add(acc, verts[vi]));
        let centroid = vec3::scale(centroid, 1.0 / n_spans as f64);
        shell.faces.push(Face {
            loop_edges,
            surface: Surface::Plane {
                origin: centroid,
                normal: cap_normal,
            },
            orientation_reversed: false,
        });
    }

    // Top (last section)
    {
        let last = n_sections - 1;
        let sec_last = &sections[last];
        let sec_prev = &sections[last - 1];
        let p_last = to_world_3d(sec_last, &[0.0, 0.0]);
        let p_prev = to_world_3d(sec_prev, &[0.0, 0.0]);
        let dir = vec3::sub(p_last, p_prev);
        let cap_normal = vec3::normalized(dir);
        let vids = &vertex_ids[last];

        let mut cap_edges = Vec::with_capacity(n_spans);
        for i in 0..n_spans {
            let j = (i + 1) % n_spans;
            let span = &all_spans[last][i];
            if span.degree == 1 {
                cap_edges.push(shell.add_edge(vids[i], vids[j], line_curve(vids[i], vids[j])));
            } else {
                let curve = Curve3D::NurbsCurve3D {
                    degree: span.degree,
                    control_points: span
                        .control_points
                        .iter()
                        .map(|p2| to_world_3d(sec_last, p2))
                        .collect(),
                    weights: span.weights.clone(),
                    knots: span.knots.clone(),
                };
                cap_edges.push(shell.add_edge(vids[i], vids[j], curve));
            }
        }
        let loop_edges: Vec<EdgeRef> = (0..n_spans).map(|i| fwd(cap_edges[i])).collect();
        let centroid = vids
            .iter()
            .fold([0.0, 0.0, 0.0], |acc, &vi| vec3::add(acc, verts[vi]));
        let centroid = vec3::scale(centroid, 1.0 / n_spans as f64);
        shell.faces.push(Face {
            loop_edges,
            surface: Surface::Plane {
                origin: centroid,
                normal: cap_normal,
            },
            orientation_reversed: false,
        });
    }

    Ok(shell)
}

/// Smooth mode Loft -> Shell.
///
/// Elevates all section Bezier spans to a unified degree, then converts
/// via Catmull-Rom -> cubic Bezier to build NurbsSurface { degree_u=3 }.
fn shell_from_loft_smooth(sections: &[LoftSection]) -> Result<Shell, String> {
    let n_sections = sections.len();

    let all_spans: Vec<Vec<NurbsCurve2D>> = sections
        .iter()
        .map(|s| {
            s.profile
                .outer
                .iter()
                .flat_map(|c| c.to_bezier_spans())
                .collect::<Vec<_>>()
        })
        .collect();

    let n_spans = all_spans[0].len();
    if n_spans == 0 {
        return Err("loft: profile has no Bezier spans".into());
    }
    for (si, spans) in all_spans.iter().enumerate() {
        if spans.len() != n_spans {
            return Err(format!(
                "loft: section {} has {} spans, expected {} (from section 0)",
                si,
                spans.len(),
                n_spans
            ));
        }
    }

    // Compute unified max degree across all sections and spans
    let max_deg = all_spans
        .iter()
        .flat_map(|spans| spans.iter().map(|s| s.degree))
        .max()
        .unwrap_or(1)
        .max(1);

    // Elevate each section's spans to max_deg and convert to 3D
    let to_world_3d = |sec: &LoftSection, p2: &[f64; 2]| -> [f64; 3] {
        transform_point(&sec.transform, [p2[0], p2[1], 0.0])
    };

    let mut section_cps: Vec<Vec<[f64; 3]>> = Vec::with_capacity(n_sections);
    let mut section_ws: Vec<Vec<f64>> = Vec::with_capacity(n_sections);

    for si in 0..n_sections {
        let mut cps_3d: Vec<[f64; 3]> = Vec::new();
        let mut ws: Vec<f64> = Vec::new();
        for (span_idx, span) in all_spans[si].iter().enumerate() {
            let (cps2d, wts) = elevate_span_to_degree(span, max_deg);
            let start = if span_idx == 0 { 0 } else { 1 };
            for j in start..cps2d.len() {
                cps_3d.push(to_world_3d(&sections[si], &cps2d[j]));
                ws.push(wts[j]);
            }
        }
        section_cps.push(cps_3d);
        section_ws.push(ws);
    }

    // C0 Bezier knot vector (v direction)
    let mut knots_v = vec![0.0; max_deg + 1];
    for i in 1..n_spans {
        let t = i as f64 / n_spans as f64;
        for _ in 0..max_deg {
            knots_v.push(t);
        }
    }
    knots_v.extend(vec![1.0; max_deg + 1]);

    let mut shell = Shell::new();

    let mut vertex_ids: Vec<Vec<usize>> = Vec::with_capacity(n_sections);
    for si in 0..n_sections {
        let mut sec_vids = Vec::with_capacity(n_spans);
        for span in all_spans[si].iter().take(n_spans) {
            let p3 = to_world_3d(&sections[si], &span.control_points[0]);
            sec_vids.push(shell.add_vertex(p3));
        }
        vertex_ids.push(sec_vids);
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

    // --- Side faces (Catmull-Rom -> Bezier segment per section pair) ---
    for seg in 0..n_sections - 1 {
        // Ghost points for Catmull-Rom at boundaries
        let p_prev: Vec<[f64; 3]> = if seg > 0 {
            section_cps[seg - 1].clone()
        } else {
            section_cps[0]
                .iter()
                .zip(section_cps[1].iter())
                .map(|(p0, p1)| vec3::sub(vec3::scale(*p0, 2.0), *p1))
                .collect()
        };
        let p_after: Vec<[f64; 3]> = if seg + 2 < n_sections {
            section_cps[seg + 2].clone()
        } else {
            let last = n_sections - 1;
            section_cps[last]
                .iter()
                .zip(section_cps[last - 1].iter())
                .map(|(pn, pn1)| vec3::sub(vec3::scale(*pn, 2.0), *pn1))
                .collect()
        };

        let rows =
            catmull_rom_to_bezier_rows(&p_prev, &section_cps[seg], &section_cps[seg + 1], &p_after);

        let weights_row = &section_ws[seg];

        let nurbs = nurbs_surface_from_smooth_loft(rows, weights_row, max_deg, knots_v.clone());

        let sec0 = &sections[seg];
        let sec1 = &sections[seg + 1];
        let vids0 = &vertex_ids[seg];
        let vids1 = &vertex_ids[seg + 1];

        let mut profile_e0 = Vec::with_capacity(n_spans);
        let mut profile_e1 = Vec::with_capacity(n_spans);

        for i in 0..n_spans {
            let j = (i + 1) % n_spans;
            let span0 = &all_spans[seg][i];
            let span1 = &all_spans[seg + 1][i];

            if span0.degree == 1 {
                profile_e0.push(shell.add_edge(vids0[i], vids0[j], line_curve(vids0[i], vids0[j])));
            } else {
                let curve = Curve3D::NurbsCurve3D {
                    degree: span0.degree,
                    control_points: span0
                        .control_points
                        .iter()
                        .map(|p2| to_world_3d(sec0, p2))
                        .collect(),
                    weights: span0.weights.clone(),
                    knots: span0.knots.clone(),
                };
                profile_e0.push(shell.add_edge(vids0[i], vids0[j], curve));
            }

            if span1.degree == 1 {
                profile_e1.push(shell.add_edge(vids1[i], vids1[j], line_curve(vids1[i], vids1[j])));
            } else {
                let curve = Curve3D::NurbsCurve3D {
                    degree: span1.degree,
                    control_points: span1
                        .control_points
                        .iter()
                        .map(|p2| to_world_3d(sec1, p2))
                        .collect(),
                    weights: span1.weights.clone(),
                    knots: span1.knots.clone(),
                };
                profile_e1.push(shell.add_edge(vids1[i], vids1[j], curve));
            }
        }

        // Vertical edge (cubic Bezier)
        let vert_b0 = nurbs.control_points[0][0];
        let vert_b1 = nurbs.control_points[1][0];
        let vert_b2 = nurbs.control_points[2][0];
        let vert_b3 = nurbs.control_points[3][0];
        let vert_curve = Curve3D::NurbsCurve3D {
            degree: 3,
            control_points: vec![vert_b0, vert_b1, vert_b2, vert_b3],
            weights: vec![1.0; 4],
            knots: vec![0.0, 0.0, 0.0, 0.0, 1.0, 1.0, 1.0, 1.0],
        };
        let vert_eid = shell.add_edge(vids0[0], vids1[0], vert_curve);

        let mut loop_edges = Vec::new();
        for &edge in profile_e0.iter().take(n_spans) {
            loop_edges.push(fwd(edge));
        }
        loop_edges.push(fwd(vert_eid));
        for i in (0..n_spans).rev() {
            loop_edges.push(rev(profile_e1[i]));
        }
        loop_edges.push(rev(vert_eid));

        shell.faces.push(Face {
            loop_edges,
            surface: Surface::NurbsSurface {
                data: Box::new(nurbs),
            },
            orientation_reversed: false,
        });
    }

    // --- Cap faces ---
    // Bottom (section 0)
    {
        let sec0 = &sections[0];
        let sec1 = &sections[1];
        let p0 = to_world_3d(sec0, &[0.0, 0.0]);
        let p1 = to_world_3d(sec1, &[0.0, 0.0]);
        let dir = vec3::sub(p1, p0);
        let cap_normal = vec3::neg(vec3::normalized(dir));
        let vids = &vertex_ids[0];

        let mut cap_edges = Vec::with_capacity(n_spans);
        for i in 0..n_spans {
            let j = (i + 1) % n_spans;
            let span = &all_spans[0][i];
            if span.degree == 1 {
                cap_edges.push(shell.add_edge(vids[i], vids[j], line_curve(vids[i], vids[j])));
            } else {
                let curve = Curve3D::NurbsCurve3D {
                    degree: span.degree,
                    control_points: span
                        .control_points
                        .iter()
                        .map(|p2| to_world_3d(sec0, p2))
                        .collect(),
                    weights: span.weights.clone(),
                    knots: span.knots.clone(),
                };
                cap_edges.push(shell.add_edge(vids[i], vids[j], curve));
            }
        }
        let loop_edges: Vec<EdgeRef> = (0..n_spans).rev().map(|i| rev(cap_edges[i])).collect();
        let centroid = vids
            .iter()
            .fold([0.0, 0.0, 0.0], |acc, &vi| vec3::add(acc, verts[vi]));
        let centroid = vec3::scale(centroid, 1.0 / n_spans as f64);
        shell.faces.push(Face {
            loop_edges,
            surface: Surface::Plane {
                origin: centroid,
                normal: cap_normal,
            },
            orientation_reversed: false,
        });
    }

    // Top (last section)
    {
        let last = n_sections - 1;
        let sec_last = &sections[last];
        let sec_prev = &sections[last - 1];
        let p_last = to_world_3d(sec_last, &[0.0, 0.0]);
        let p_prev = to_world_3d(sec_prev, &[0.0, 0.0]);
        let dir = vec3::sub(p_last, p_prev);
        let cap_normal = vec3::normalized(dir);
        let vids = &vertex_ids[last];

        let mut cap_edges = Vec::with_capacity(n_spans);
        for i in 0..n_spans {
            let j = (i + 1) % n_spans;
            let span = &all_spans[last][i];
            if span.degree == 1 {
                cap_edges.push(shell.add_edge(vids[i], vids[j], line_curve(vids[i], vids[j])));
            } else {
                let curve = Curve3D::NurbsCurve3D {
                    degree: span.degree,
                    control_points: span
                        .control_points
                        .iter()
                        .map(|p2| to_world_3d(sec_last, p2))
                        .collect(),
                    weights: span.weights.clone(),
                    knots: span.knots.clone(),
                };
                cap_edges.push(shell.add_edge(vids[i], vids[j], curve));
            }
        }
        let loop_edges: Vec<EdgeRef> = (0..n_spans).map(|i| fwd(cap_edges[i])).collect();
        let centroid = vids
            .iter()
            .fold([0.0, 0.0, 0.0], |acc, &vi| vec3::add(acc, verts[vi]));
        let centroid = vec3::scale(centroid, 1.0 / n_spans as f64);
        shell.faces.push(Face {
            loop_edges,
            surface: Surface::Plane {
                origin: centroid,
                normal: cap_normal,
            },
            orientation_reversed: false,
        });
    }

    Ok(shell)
}

/// Convert 4 Catmull-Rom section CP rows into 4 cubic Bezier CP rows.
fn catmull_rom_to_bezier_rows(
    p_prev: &[[f64; 3]],
    p_curr: &[[f64; 3]],
    p_next: &[[f64; 3]],
    p_after: &[[f64; 3]],
) -> [Vec<[f64; 3]>; 4] {
    let n = p_curr.len();
    let mut b0 = Vec::with_capacity(n);
    let mut b1 = Vec::with_capacity(n);
    let mut b2 = Vec::with_capacity(n);
    let mut b3 = Vec::with_capacity(n);

    for j in 0..n {
        b0.push(p_curr[j]);
        let tangent_curr = vec3::scale(vec3::sub(p_next[j], p_prev[j]), 1.0 / 6.0);
        b1.push(vec3::add(p_curr[j], tangent_curr));
        let tangent_next = vec3::scale(vec3::sub(p_after[j], p_curr[j]), 1.0 / 6.0);
        b2.push(vec3::sub(p_next[j], tangent_next));
        b3.push(p_next[j]);
    }

    [b0, b1, b2, b3]
}

/// Build NurbsSurface3D for smooth loft (degree_u=3, degree_v=max_deg).
fn nurbs_surface_from_smooth_loft(
    rows: [Vec<[f64; 3]>; 4],
    weights: &[f64],
    max_deg: usize,
    knots_v: Vec<f64>,
) -> neco_nurbs::NurbsSurface3D {
    let [r0, r1, r2, r3] = rows;
    neco_nurbs::NurbsSurface3D {
        degree_u: 3,
        degree_v: max_deg,
        control_points: vec![r0, r1, r2, r3],
        weights: vec![weights.to_vec(); 4],
        knots_u: vec![0.0, 0.0, 0.0, 0.0, 1.0, 1.0, 1.0, 1.0],
        knots_v,
    }
}

/// Build a ruled NurbsSurface3D from two section span sets.
///
/// Elevates each Bezier span to the max degree and concatenates in v direction.
/// S(u, v) = (1-u)*C0(v) + u*C1(v) with degree_u=1, degree_v=max_deg.
fn nurbs_surface_from_loft_unified(
    spans0: &[NurbsCurve2D],
    spans1: &[NurbsCurve2D],
    sec0: &LoftSection,
    sec1: &LoftSection,
) -> neco_nurbs::NurbsSurface3D {
    let n_spans = spans0.len();

    let max_deg = spans0
        .iter()
        .chain(spans1.iter())
        .map(|s| s.degree)
        .max()
        .unwrap_or(1)
        .max(1);

    let mut row0_cps: Vec<[f64; 3]> = Vec::new();
    let mut row1_cps: Vec<[f64; 3]> = Vec::new();
    let mut row0_ws: Vec<f64> = Vec::new();
    let mut row1_ws: Vec<f64> = Vec::new();

    for i in 0..n_spans {
        let (cps0, ws0) = elevate_span_to_degree(&spans0[i], max_deg);
        let (cps1, ws1) = elevate_span_to_degree(&spans1[i], max_deg);

        let n_cp = cps0.len();

        let start = if i == 0 { 0 } else { 1 };
        for j in start..n_cp {
            row0_cps.push(transform_point(
                &sec0.transform,
                [cps0[j][0], cps0[j][1], 0.0],
            ));
            row0_ws.push(ws0[j]);

            row1_cps.push(transform_point(
                &sec1.transform,
                [cps1[j][0], cps1[j][1], 0.0],
            ));
            row1_ws.push(ws1[j]);
        }
    }

    // v-direction knots: Bezier span concatenation
    let mut knots_v = vec![0.0; max_deg + 1];
    for i in 1..n_spans {
        let t = i as f64 / n_spans as f64;
        for _ in 0..max_deg {
            knots_v.push(t);
        }
    }
    knots_v.extend(vec![1.0; max_deg + 1]);

    let knots_u = vec![0.0, 0.0, 1.0, 1.0];

    neco_nurbs::NurbsSurface3D {
        degree_u: 1,
        degree_v: max_deg,
        control_points: vec![row0_cps, row1_cps],
        weights: vec![row0_ws, row1_ws],
        knots_u,
        knots_v,
    }
}

/// Degree-elevate a Bezier span to `target_deg`. Returns as-is if already at target.
fn elevate_span_to_degree(span: &NurbsCurve2D, target_deg: usize) -> (Vec<[f64; 2]>, Vec<f64>) {
    let mut cps = span.control_points.clone();
    let mut ws = span.weights.clone();
    let mut deg = span.degree;

    while deg < target_deg {
        let n = cps.len();
        let new_n = n + 1;
        let mut new_cps = Vec::with_capacity(new_n);
        let mut new_ws = Vec::with_capacity(new_n);

        // Rational degree elevation in homogeneous coordinates (w*x, w*y, w)
        for i in 0..new_n {
            let alpha = i as f64 / (deg + 1) as f64;
            if i == 0 {
                new_cps.push(cps[0]);
                new_ws.push(ws[0]);
            } else if i == new_n - 1 {
                new_cps.push(cps[n - 1]);
                new_ws.push(ws[n - 1]);
            } else {
                let w_prev = ws[i - 1];
                let w_curr = ws[i];
                let hw = alpha * w_prev + (1.0 - alpha) * w_curr;
                let hx = alpha * (w_prev * cps[i - 1][0]) + (1.0 - alpha) * (w_curr * cps[i][0]);
                let hy = alpha * (w_prev * cps[i - 1][1]) + (1.0 - alpha) * (w_curr * cps[i][1]);
                new_ws.push(hw);
                new_cps.push([hx / hw, hy / hw]);
            }
        }

        cps = new_cps;
        ws = new_ws;
        deg += 1;
    }

    (cps, ws)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::brep::Surface;
    use crate::types::identity_matrix;
    use neco_nurbs::{NurbsCurve2D, NurbsRegion};

    fn make_rect_nurbs_region(lx: f64, ly: f64) -> NurbsRegion {
        let hw = lx * 0.5;
        let hh = ly * 0.5;
        let verts = vec![[-hw, -hh], [hw, -hh], [hw, hh], [-hw, hh], [-hw, -hh]];
        let n = verts.len();
        let outer = NurbsCurve2D::new(1, verts, (0..=n).map(|i| i as f64).collect());
        NurbsRegion {
            outer: vec![outer],
            holes: vec![],
        }
    }

    fn translation_transform(dx: f64, dy: f64, dz: f64) -> [[f64; 4]; 4] {
        crate::types::translation_matrix(dx, dy, dz)
    }

    #[test]
    fn loft_straight_rect_plane_faces() {
        let sections = vec![
            LoftSection {
                profile: make_rect_nurbs_region(0.4, 0.4),
                transform: identity_matrix(),
            },
            LoftSection {
                profile: make_rect_nurbs_region(0.2, 0.2),
                transform: translation_transform(0.0, 0.0, 0.5),
            },
        ];
        let shell = shell_from_loft(&sections, LoftMode::Straight).unwrap();

        // 1 merged side + 2 caps = 3 faces
        assert_eq!(shell.faces.len(), 3, "face count: {}", shell.faces.len());

        let plane_count = shell
            .faces
            .iter()
            .filter(|f| matches!(f.surface, Surface::Plane { .. }))
            .count();
        assert_eq!(plane_count, 2, "cap Plane faces: {}", plane_count);

        let nurbs_count = shell
            .faces
            .iter()
            .filter(|f| matches!(f.surface, Surface::NurbsSurface { .. }))
            .count();
        assert_eq!(nurbs_count, 1, "merged NurbsSurface side: {}", nurbs_count);

        assert_eq!(
            shell.vertices.len(),
            8,
            "vertex count: {}",
            shell.vertices.len()
        );
    }

    #[test]
    fn loft_straight_3sections() {
        let sections = vec![
            LoftSection {
                profile: make_rect_nurbs_region(0.3, 0.3),
                transform: identity_matrix(),
            },
            LoftSection {
                profile: make_rect_nurbs_region(0.5, 0.5),
                transform: translation_transform(0.0, 0.0, 0.3),
            },
            LoftSection {
                profile: make_rect_nurbs_region(0.2, 0.2),
                transform: translation_transform(0.0, 0.0, 0.6),
            },
        ];
        let shell = shell_from_loft(&sections, LoftMode::Straight).unwrap();

        // 1 merged side x 2 layers + 2 caps = 4 faces
        assert_eq!(shell.faces.len(), 4, "face count: {}", shell.faces.len());
    }
}
