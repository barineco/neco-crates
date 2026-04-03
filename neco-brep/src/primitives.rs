//! Primitive shape Shell generation.

use crate::brep::{Curve3D, EdgeRef, Face, Shell, Surface};
use crate::vec3;

fn torus_face_sample_normal(
    torus_surface: &Surface,
    centroid: [f64; 3],
    poly: &[[f64; 3]],
    fallback: [f64; 3],
) -> [f64; 3] {
    let sample = torus_surface
        .inverse_project(&centroid)
        .map(|(u, v)| torus_surface.evaluate(u, v))
        .or_else(|| {
            poly.iter()
                .copied()
                .find(|point| torus_surface.inverse_project(point).is_some())
        })
        .unwrap_or(centroid);

    torus_surface
        .inverse_project(&sample)
        .map(|(u, v)| torus_surface.normal_at(u, v))
        .unwrap_or(fallback)
}

/// Box Shell centered at origin with 6 Plane faces.
pub fn shell_from_box(lx: f64, ly: f64, lz: f64) -> Shell {
    let mut shell = Shell::new();
    let hlx = lx * 0.5;
    let hly = ly * 0.5;
    let hlz = lz * 0.5;

    let v0 = shell.add_vertex([-hlx, -hly, -hlz]);
    let v1 = shell.add_vertex([hlx, -hly, -hlz]);
    let v2 = shell.add_vertex([hlx, hly, -hlz]);
    let v3 = shell.add_vertex([-hlx, hly, -hlz]);
    let v4 = shell.add_vertex([-hlx, -hly, hlz]);
    let v5 = shell.add_vertex([hlx, -hly, hlz]);
    let v6 = shell.add_vertex([hlx, hly, hlz]);
    let v7 = shell.add_vertex([-hlx, hly, hlz]);

    let verts = shell.vertices.clone();
    let line = |a: usize, b: usize| Curve3D::Line {
        start: verts[a],
        end: verts[b],
    };

    let e0 = shell.add_edge(v0, v1, line(v0, v1));
    let e1 = shell.add_edge(v1, v2, line(v1, v2));
    let e2 = shell.add_edge(v2, v3, line(v2, v3));
    let e3 = shell.add_edge(v3, v0, line(v3, v0));
    let e4 = shell.add_edge(v4, v5, line(v4, v5));
    let e5 = shell.add_edge(v5, v6, line(v5, v6));
    let e6 = shell.add_edge(v6, v7, line(v6, v7));
    let e7 = shell.add_edge(v7, v4, line(v7, v4));
    let e8 = shell.add_edge(v0, v4, line(v0, v4));
    let e9 = shell.add_edge(v1, v5, line(v1, v5));
    let e10 = shell.add_edge(v2, v6, line(v2, v6));
    let e11 = shell.add_edge(v3, v7, line(v3, v7));

    let fwd = |eid: usize| EdgeRef {
        edge_id: eid,
        forward: true,
    };
    let rev = |eid: usize| EdgeRef {
        edge_id: eid,
        forward: false,
    };

    // bottom (z=-hlz, normal -Z)
    shell.faces.push(Face {
        loop_edges: vec![rev(e3), rev(e2), rev(e1), rev(e0)],
        surface: Surface::Plane {
            origin: [0.0, 0.0, -hlz],
            normal: [0.0, 0.0, -1.0],
        },
        orientation_reversed: false,
    });
    // top (z=+hlz, normal +Z)
    shell.faces.push(Face {
        loop_edges: vec![fwd(e4), fwd(e5), fwd(e6), fwd(e7)],
        surface: Surface::Plane {
            origin: [0.0, 0.0, hlz],
            normal: [0.0, 0.0, 1.0],
        },
        orientation_reversed: false,
    });
    // front (y=-hly, normal -Y)
    shell.faces.push(Face {
        loop_edges: vec![fwd(e0), fwd(e9), rev(e4), rev(e8)],
        surface: Surface::Plane {
            origin: [0.0, -hly, 0.0],
            normal: [0.0, -1.0, 0.0],
        },
        orientation_reversed: false,
    });
    // back (y=+hly, normal +Y)
    shell.faces.push(Face {
        loop_edges: vec![fwd(e2), fwd(e11), rev(e6), rev(e10)],
        surface: Surface::Plane {
            origin: [0.0, hly, 0.0],
            normal: [0.0, 1.0, 0.0],
        },
        orientation_reversed: false,
    });
    // left (x=-hlx, normal -X)
    shell.faces.push(Face {
        loop_edges: vec![fwd(e8), rev(e7), rev(e11), fwd(e3)],
        surface: Surface::Plane {
            origin: [-hlx, 0.0, 0.0],
            normal: [-1.0, 0.0, 0.0],
        },
        orientation_reversed: false,
    });
    // right (x=+hlx, normal +X)
    shell.faces.push(Face {
        loop_edges: vec![fwd(e1), fwd(e10), rev(e5), rev(e9)],
        surface: Surface::Plane {
            origin: [hlx, 0.0, 0.0],
            normal: [1.0, 0.0, 0.0],
        },
        orientation_reversed: false,
    });

    shell
}

/// Sphere Shell centered at origin, Y-axis up, 8 spherical triangle faces.
pub fn shell_from_sphere(radius: f64) -> Shell {
    let center = [0.0, 0.0, 0.0];
    let mut shell = Shell::new();

    let v_north = shell.add_vertex([0.0, radius, 0.0]);
    let v_south = shell.add_vertex([0.0, -radius, 0.0]);
    let v_eq0 = shell.add_vertex([radius, 0.0, 0.0]);
    let v_eq1 = shell.add_vertex([0.0, 0.0, radius]);
    let v_eq2 = shell.add_vertex([-radius, 0.0, 0.0]);
    let v_eq3 = shell.add_vertex([0.0, 0.0, -radius]);

    let axis = [0.0, 1.0, 0.0];
    let eq_verts = [v_eq0, v_eq1, v_eq2, v_eq3];

    // Equatorial arc edges
    let mut eq_edges = Vec::with_capacity(4);
    for i in 0..4 {
        let j = (i + 1) % 4;
        let arc = Curve3D::Arc {
            center,
            axis,
            start: shell.vertices[eq_verts[i]],
            end: shell.vertices[eq_verts[j]],
            radius,
        };
        eq_edges.push(shell.add_edge(eq_verts[i], eq_verts[j], arc));
    }

    // Northern meridian arc edges
    let mut north_edges = Vec::with_capacity(4);
    for &eq_vert in &eq_verts {
        let start_pt = shell.vertices[eq_vert];
        let north_pt = shell.vertices[v_north];
        let radial = vec3::normalized(start_pt);
        let meridian_axis = vec3::normalized(vec3::cross(radial, axis));
        let arc = Curve3D::Arc {
            center,
            axis: meridian_axis,
            start: start_pt,
            end: north_pt,
            radius,
        };
        north_edges.push(shell.add_edge(eq_vert, v_north, arc));
    }

    // Southern meridian arc edges
    let mut south_edges = Vec::with_capacity(4);
    for &eq_vert in &eq_verts {
        let start_pt = shell.vertices[eq_vert];
        let south_pt = shell.vertices[v_south];
        let radial = vec3::normalized(start_pt);
        let meridian_axis = vec3::normalized(vec3::cross(radial, axis));
        let arc = Curve3D::Arc {
            center,
            axis: meridian_axis,
            start: start_pt,
            end: south_pt,
            radius,
        };
        south_edges.push(shell.add_edge(eq_vert, v_south, arc));
    }

    let fwd = |eid: usize| EdgeRef {
        edge_id: eid,
        forward: true,
    };
    let rev = |eid: usize| EdgeRef {
        edge_id: eid,
        forward: false,
    };
    let sphere_surface = Surface::Sphere { center, radius };

    // Northern hemisphere: 4 spherical triangles
    for i in 0..4 {
        let j = (i + 1) % 4;
        shell.faces.push(Face {
            loop_edges: vec![fwd(eq_edges[i]), fwd(north_edges[j]), rev(north_edges[i])],
            surface: sphere_surface.clone(),
            orientation_reversed: false,
        });
    }

    // Southern hemisphere: 4 spherical triangles
    for i in 0..4 {
        let j = (i + 1) % 4;
        shell.faces.push(Face {
            loop_edges: vec![rev(eq_edges[i]), fwd(south_edges[i]), rev(south_edges[j])],
            surface: sphere_surface.clone(),
            orientation_reversed: false,
        });
    }

    shell
}

/// Ellipsoid Shell centered at origin. Delegates to sphere if radii are equal.
pub fn shell_from_ellipsoid(rx: f64, ry: f64, rz: f64) -> Shell {
    if (rx - ry).abs() < 1e-12 && (ry - rz).abs() < 1e-12 {
        return shell_from_sphere(rx);
    }

    let mut shell = shell_from_sphere(1.0);

    for v in &mut shell.vertices {
        v[0] *= rx;
        v[1] *= ry;
        v[2] *= rz;
    }

    let r_max = rx.max(ry).max(rz);
    for edge in &mut shell.edges {
        let sv = shell.vertices[edge.v_start];
        let ev = shell.vertices[edge.v_end];
        if let Curve3D::Arc {
            center,
            start,
            end,
            radius,
            ..
        } = &mut edge.curve
        {
            *start = sv;
            *end = ev;
            *radius = r_max;
            *center = [0.0, 0.0, 0.0];
        }
    }

    let ellipsoid_surface = Surface::Ellipsoid {
        center: [0.0, 0.0, 0.0],
        rx,
        ry,
        rz,
    };
    for face in &mut shell.faces {
        face.surface = ellipsoid_surface.clone();
    }

    shell
}

/// Torus Shell centered at origin, Z-axis, 4x4 grid = 16 faces.
pub fn shell_from_torus(major_radius: f64, minor_radius: f64) -> Shell {
    let center = [0.0, 0.0, 0.0];
    let axis_n = [0.0, 0.0, 1.0];
    let mut shell = Shell::new();

    let u_dir = [1.0, 0.0, 0.0];
    let v_dir = [0.0, 1.0, 0.0];

    // 4 points on the major circle
    let major_pts: Vec<[f64; 3]> = (0..4)
        .map(|i| {
            let theta = std::f64::consts::FRAC_PI_2 * i as f64;
            vec3::add(
                vec3::scale(u_dir, major_radius * theta.cos()),
                vec3::scale(v_dir, major_radius * theta.sin()),
            )
        })
        .collect();

    // 4 tube cross-section points at each major circle point
    let mut grid_verts = [[0usize; 4]; 4];
    for mi in 0..4 {
        let radial = vec3::normalized(major_pts[mi]);
        for (ni, grid_vert) in grid_verts[mi].iter_mut().enumerate() {
            let phi = std::f64::consts::FRAC_PI_2 * ni as f64;
            let pt = vec3::add(
                major_pts[mi],
                vec3::add(
                    vec3::scale(radial, minor_radius * phi.cos()),
                    vec3::scale(axis_n, minor_radius * phi.sin()),
                ),
            );
            *grid_vert = shell.add_vertex(pt);
        }
    }

    // Major-circle arc edges
    let mut major_edges = [[0usize; 4]; 4];
    for mi in 0..4 {
        let mj = (mi + 1) % 4;
        for ni in 0..4 {
            let start_pt = shell.vertices[grid_verts[mi][ni]];
            let end_pt = shell.vertices[grid_verts[mj][ni]];
            let phi = std::f64::consts::FRAC_PI_2 * ni as f64;
            let arc_radius = major_radius + minor_radius * phi.cos();
            let arc_center = vec3::scale(axis_n, minor_radius * phi.sin());
            let arc = Curve3D::Arc {
                center: arc_center,
                axis: axis_n,
                start: start_pt,
                end: end_pt,
                radius: arc_radius,
            };
            major_edges[mi][ni] = shell.add_edge(grid_verts[mi][ni], grid_verts[mj][ni], arc);
        }
    }

    // Minor-circle arc edges
    let mut minor_edges = [[0usize; 4]; 4];
    for mi in 0..4 {
        let radial = vec3::normalized(major_pts[mi]);
        for ni in 0..4 {
            let nj = (ni + 1) % 4;
            let start_pt = shell.vertices[grid_verts[mi][ni]];
            let end_pt = shell.vertices[grid_verts[mi][nj]];
            let minor_arc_axis = vec3::normalized(vec3::cross(axis_n, radial));
            let arc = Curve3D::Arc {
                center: major_pts[mi],
                axis: minor_arc_axis,
                start: start_pt,
                end: end_pt,
                radius: minor_radius,
            };
            minor_edges[mi][ni] = shell.add_edge(grid_verts[mi][ni], grid_verts[mi][nj], arc);
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
    let reverse_loop = |loop_edges: &[EdgeRef]| -> Vec<EdgeRef> {
        loop_edges
            .iter()
            .rev()
            .map(|eref| EdgeRef {
                edge_id: eref.edge_id,
                forward: !eref.forward,
            })
            .collect()
    };
    let torus_surface = Surface::Torus {
        center,
        axis: axis_n,
        major_radius,
        minor_radius,
    };

    for mi in 0..4 {
        let mj = (mi + 1) % 4;
        for ni in 0..4 {
            let nj = (ni + 1) % 4;
            let mut loop_edges = vec![
                fwd(major_edges[mi][nj]),
                rev(minor_edges[mj][ni]),
                rev(major_edges[mi][ni]),
                fwd(minor_edges[mi][ni]),
            ];
            let poly: Vec<[f64; 3]> = loop_edges
                .iter()
                .map(|eref| {
                    let edge = &shell.edges[eref.edge_id];
                    let vid = if eref.forward {
                        edge.v_start
                    } else {
                        edge.v_end
                    };
                    shell.vertices[vid]
                })
                .collect();
            let centroid = vec3::scale(
                poly.iter().copied().fold([0.0, 0.0, 0.0], vec3::add),
                1.0 / poly.len() as f64,
            );
            let mut poly_n = [0.0, 0.0, 0.0];
            for i in 0..poly.len() {
                let a = poly[i];
                let b = poly[(i + 1) % poly.len()];
                poly_n[0] += (a[1] - b[1]) * (a[2] + b[2]);
                poly_n[1] += (a[2] - b[2]) * (a[0] + b[0]);
                poly_n[2] += (a[0] - b[0]) * (a[1] + b[1]);
            }
            let poly_n = vec3::normalized(poly_n);
            let surf_n = torus_face_sample_normal(&torus_surface, centroid, &poly, poly_n);
            if vec3::dot(poly_n, surf_n) < 0.0 {
                loop_edges = reverse_loop(&loop_edges);
            }
            shell.faces.push(Face {
                loop_edges,
                surface: torus_surface.clone(),
                orientation_reversed: false,
            });
        }
    }

    shell
}

/// Cylinder Shell centered at origin, Z-axis, with caps and lateral faces.
pub fn shell_from_cylinder(outer_r: f64, inner_r: Option<f64>, length: f64) -> Shell {
    let mut shell = Shell::new();
    let hz = length * 0.5;

    // Outer surface vertices: 4 at bottom, 4 at top
    let mut bot_verts = Vec::with_capacity(4);
    let mut top_verts = Vec::with_capacity(4);
    for i in 0..4 {
        let theta = std::f64::consts::FRAC_PI_2 * i as f64;
        let x = outer_r * theta.cos();
        let y = outer_r * theta.sin();
        bot_verts.push(shell.add_vertex([x, y, -hz]));
        top_verts.push(shell.add_vertex([x, y, hz]));
    }

    let fwd = |eid: usize| EdgeRef {
        edge_id: eid,
        forward: true,
    };
    let rev = |eid: usize| EdgeRef {
        edge_id: eid,
        forward: false,
    };

    // Bottom and top arc edges
    let mut bot_edges = Vec::with_capacity(4);
    let mut top_edges = Vec::with_capacity(4);
    for i in 0..4 {
        let j = (i + 1) % 4;
        let bot_arc = Curve3D::Arc {
            center: [0.0, 0.0, -hz],
            axis: [0.0, 0.0, 1.0],
            start: shell.vertices[bot_verts[i]],
            end: shell.vertices[bot_verts[j]],
            radius: outer_r,
        };
        bot_edges.push(shell.add_edge(bot_verts[i], bot_verts[j], bot_arc));

        let top_arc = Curve3D::Arc {
            center: [0.0, 0.0, hz],
            axis: [0.0, 0.0, 1.0],
            start: shell.vertices[top_verts[i]],
            end: shell.vertices[top_verts[j]],
            radius: outer_r,
        };
        top_edges.push(shell.add_edge(top_verts[i], top_verts[j], top_arc));
    }

    // Vertical line edges
    let mut vert_edges = Vec::with_capacity(4);
    for i in 0..4 {
        let line = Curve3D::Line {
            start: shell.vertices[bot_verts[i]],
            end: shell.vertices[top_verts[i]],
        };
        vert_edges.push(shell.add_edge(bot_verts[i], top_verts[i], line));
    }

    // Lateral faces: 4 cylinder quads
    let cyl_surface = Surface::Cylinder {
        origin: [0.0, 0.0, -hz],
        axis: [0.0, 0.0, length],
        radius: outer_r,
    };
    for i in 0..4 {
        let j = (i + 1) % 4;
        shell.faces.push(Face {
            loop_edges: vec![
                fwd(bot_edges[i]),
                fwd(vert_edges[j]),
                rev(top_edges[i]),
                rev(vert_edges[i]),
            ],
            surface: cyl_surface.clone(),
            orientation_reversed: false,
        });
    }

    if inner_r.is_none() || inner_r == Some(0.0) {
        // Caps: bottom and top planes
        shell.faces.push(Face {
            loop_edges: (0..4).rev().map(|i| rev(bot_edges[i])).collect(),
            surface: Surface::Plane {
                origin: [0.0, 0.0, -hz],
                normal: [0.0, 0.0, -1.0],
            },
            orientation_reversed: false,
        });
        shell.faces.push(Face {
            loop_edges: (0..4).map(|i| fwd(top_edges[i])).collect(),
            surface: Surface::Plane {
                origin: [0.0, 0.0, hz],
                normal: [0.0, 0.0, 1.0],
            },
            orientation_reversed: false,
        });
    }
    // inner_r support is a future extension

    shell
}

/// Frustum (truncated cone) Shell centered at origin, Z-axis.
///
/// `r_bottom=0` for a pointed cone, `r_top=0` for an inverted cone.
pub fn shell_from_cone(r_bottom: f64, r_top: f64, length: f64) -> Shell {
    let mut shell = Shell::new();
    let hz = length * 0.5;

    let make_ring = |shell: &mut Shell, r: f64, z: f64| -> Vec<usize> {
        if r < 1e-15 {
            // Degenerate: single apex vertex
            let v = shell.add_vertex([0.0, 0.0, z]);
            vec![v; 4]
        } else {
            (0..4)
                .map(|i| {
                    let theta = std::f64::consts::FRAC_PI_2 * i as f64;
                    shell.add_vertex([r * theta.cos(), r * theta.sin(), z])
                })
                .collect()
        }
    };

    let bot_verts = make_ring(&mut shell, r_bottom, -hz);
    let top_verts = make_ring(&mut shell, r_top, hz);

    let fwd = |eid: usize| EdgeRef {
        edge_id: eid,
        forward: true,
    };
    let rev = |eid: usize| EdgeRef {
        edge_id: eid,
        forward: false,
    };

    let bot_degenerate = r_bottom < 1e-15;
    let top_degenerate = r_top < 1e-15;

    // Bottom and top arc edges (if not degenerate)
    let mut bot_edges = Vec::with_capacity(4);
    let mut top_edges = Vec::with_capacity(4);

    if !bot_degenerate {
        for i in 0..4 {
            let j = (i + 1) % 4;
            let arc = Curve3D::Arc {
                center: [0.0, 0.0, -hz],
                axis: [0.0, 0.0, 1.0],
                start: shell.vertices[bot_verts[i]],
                end: shell.vertices[bot_verts[j]],
                radius: r_bottom,
            };
            bot_edges.push(shell.add_edge(bot_verts[i], bot_verts[j], arc));
        }
    }

    if !top_degenerate {
        for i in 0..4 {
            let j = (i + 1) % 4;
            let arc = Curve3D::Arc {
                center: [0.0, 0.0, hz],
                axis: [0.0, 0.0, 1.0],
                start: shell.vertices[top_verts[i]],
                end: shell.vertices[top_verts[j]],
                radius: r_top,
            };
            top_edges.push(shell.add_edge(top_verts[i], top_verts[j], arc));
        }
    }

    // Vertical line edges
    let mut vert_edges = Vec::with_capacity(4);
    for i in 0..4 {
        let line = Curve3D::Line {
            start: shell.vertices[bot_verts[i]],
            end: shell.vertices[top_verts[i]],
        };
        vert_edges.push(shell.add_edge(bot_verts[i], top_verts[i], line));
    }

    // Half-angle computation
    let half_angle = if (r_bottom - r_top).abs() < 1e-15 {
        0.0 // pure cylinder
    } else {
        ((r_top - r_bottom).abs() / length).atan()
    };

    let cone_surface = Surface::Cone {
        origin: [0.0, 0.0, -hz],
        axis: [0.0, 0.0, length],
        half_angle,
    };

    // Lateral faces
    for i in 0..4 {
        let j = (i + 1) % 4;
        if bot_degenerate {
            // Triangle face (degenerate bottom)
            shell.faces.push(Face {
                loop_edges: vec![fwd(vert_edges[j]), rev(top_edges[i]), rev(vert_edges[i])],
                surface: cone_surface.clone(),
                orientation_reversed: false,
            });
        } else if top_degenerate {
            // Triangle face (degenerate top)
            shell.faces.push(Face {
                loop_edges: vec![fwd(bot_edges[i]), fwd(vert_edges[j]), rev(vert_edges[i])],
                surface: cone_surface.clone(),
                orientation_reversed: false,
            });
        } else {
            // Quad face
            shell.faces.push(Face {
                loop_edges: vec![
                    fwd(bot_edges[i]),
                    fwd(vert_edges[j]),
                    rev(top_edges[i]),
                    rev(vert_edges[i]),
                ],
                surface: cone_surface.clone(),
                orientation_reversed: false,
            });
        }
    }

    // Caps
    if !bot_degenerate {
        shell.faces.push(Face {
            loop_edges: (0..4).rev().map(|i| rev(bot_edges[i])).collect(),
            surface: Surface::Plane {
                origin: [0.0, 0.0, -hz],
                normal: [0.0, 0.0, -1.0],
            },
            orientation_reversed: false,
        });
    }
    if !top_degenerate {
        shell.faces.push(Face {
            loop_edges: (0..4).map(|i| fwd(top_edges[i])).collect(),
            surface: Surface::Plane {
                origin: [0.0, 0.0, hz],
                normal: [0.0, 0.0, 1.0],
            },
            orientation_reversed: false,
        });
    }

    shell
}

#[cfg(test)]
mod tests {
    use super::*;

    fn validate_shell(shell: &Shell, expected_faces: usize, label: &str) {
        assert_eq!(shell.faces.len(), expected_faces, "{label}: face count");
        let mesh = shell.tessellate(8).unwrap();
        let v = mesh.validate();
        assert!(v.is_watertight, "{label}: not watertight");
        assert_eq!(v.euler_number, 2, "{label}: euler number");
        assert!(
            v.signed_volume > 0.0,
            "{label}: volume not positive (got {})",
            v.signed_volume
        );
    }

    #[test]
    fn box_shell() {
        let shell = shell_from_box(2.0, 3.0, 4.0);
        validate_shell(&shell, 6, "Box");
        let mesh = shell.tessellate(4).unwrap();
        let vol = mesh.validate().signed_volume;
        assert!((vol - 24.0).abs() < 1.0, "Box volume ~ 24.0, got {vol}");
    }

    #[test]
    fn sphere_shell() {
        let shell = shell_from_sphere(1.0);
        assert_eq!(shell.vertices.len(), 6);
        assert_eq!(shell.faces.len(), 8);
        // Trim-aware tessellation now handles seam-crossing sphere faces, but the
        // full sphere shell is not yet watertight enough to promote to validate_shell().
    }

    #[test]
    fn ellipsoid_shell() {
        let shell = shell_from_ellipsoid(2.0, 1.0, 1.5);
        assert_eq!(shell.faces.len(), 8);
    }

    #[test]
    fn torus_shell() {
        let shell = shell_from_torus(1.0, 0.3);
        assert_eq!(shell.vertices.len(), 16);
        assert_eq!(shell.faces.len(), 16);
    }

    #[test]
    fn torus_normals_point_outward() {
        let shell = shell_from_torus(1.0, 0.3);
        for face in &shell.faces {
            let poly: Vec<[f64; 3]> = face
                .loop_edges
                .iter()
                .map(|eref| {
                    let edge = &shell.edges[eref.edge_id];
                    let vid = if eref.forward {
                        edge.v_start
                    } else {
                        edge.v_end
                    };
                    shell.vertices[vid]
                })
                .collect();
            let centroid = crate::vec3::scale(
                poly.iter().copied().fold([0.0, 0.0, 0.0], crate::vec3::add),
                1.0 / poly.len() as f64,
            );
            let sample = face
                .surface
                .inverse_project(&centroid)
                .map(|(u, v)| face.surface.evaluate(u, v))
                .unwrap_or(centroid);
            let surf_n =
                crate::boolean3d::combine3d::surface_normal_at(&face.surface, &sample).unwrap();
            let mut poly_n = [0.0, 0.0, 0.0];
            for i in 0..poly.len() {
                let a = poly[i];
                let b = poly[(i + 1) % poly.len()];
                poly_n[0] += (a[1] - b[1]) * (a[2] + b[2]);
                poly_n[1] += (a[2] - b[2]) * (a[0] + b[0]);
                poly_n[2] += (a[0] - b[0]) * (a[1] + b[1]);
            }
            poly_n = crate::vec3::normalized(poly_n);
            assert!(
                crate::vec3::dot(poly_n, surf_n) > 0.0,
                "torus face winding should align with outward surface normal: poly_n={poly_n:?} surf_n={surf_n:?} poly={poly:?}"
            );
        }
    }

    #[test]
    fn torus_shell_with_zero_major_radius_does_not_panic() {
        let shell = shell_from_torus(0.0, 1.0);
        assert_eq!(shell.faces.len(), 16);
        assert!(!shell.vertices.is_empty());
    }

    #[test]
    fn cylinder_shell() {
        let shell = shell_from_cylinder(1.0, None, 2.0);
        // 4 side + 2 cap = 6
        assert_eq!(shell.faces.len(), 6);
        // The lateral trim is now respected, but plane caps still tessellate arc edges
        // as straight chords, so full-shell watertight validation is not fixed here.
    }

    #[test]
    fn cone_shell() {
        let shell = shell_from_cone(1.0, 0.0, 2.0);
        // 4 side (triangles) + 1 bottom cap = 5
        assert_eq!(shell.faces.len(), 5);
    }

    #[test]
    fn cone_frustum_shell() {
        let shell = shell_from_cone(1.0, 0.5, 2.0);
        // 4 side (quads) + 2 caps = 6
        assert_eq!(shell.faces.len(), 6);
    }
}
