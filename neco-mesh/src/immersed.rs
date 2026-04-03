//! Immersed-boundary meshing on a structured grid with per-tetrahedron fill fractions.

use crate::internal_mesh3d::Mesh3D;
use crate::point3::Point3;
use crate::types::{ImmersedMesh, TetMesh3D};
use neco_stl::TriSurface;

/// Generate an immersed tetrahedral mesh from a triangle surface mesh.
pub fn generate_immersed_mesh(
    surface_nodes: &[[f64; 3]],
    surface_triangles: &[[usize; 3]],
    max_edge: f64,
) -> ImmersedMesh {
    let surface = TriSurface {
        nodes: surface_nodes.to_vec(),
        triangles: surface_triangles.to_vec(),
    };

    let pad = max_edge;
    let (mut min, mut max) = bounding_box(&surface.nodes);
    min.x -= pad;
    min.y -= pad;
    min.z -= pad;
    max.x += pad;
    max.y += pad;
    max.z += pad;

    let lx = max.x - min.x;
    let ly = max.y - min.y;
    let lz = max.z - min.z;

    let nx = (lx / max_edge).ceil() as usize + 1;
    let ny = (ly / max_edge).ceil() as usize + 1;
    let nz = (lz / max_edge).ceil() as usize + 1;
    let dx = lx / (nx - 1).max(1) as f64;
    let dy = ly / (ny - 1).max(1) as f64;
    let dz = lz / (nz - 1).max(1) as f64;

    let mut nodes = Vec::with_capacity(nx * ny * nz);
    for k in 0..nz {
        for j in 0..ny {
            for i in 0..nx {
                nodes.push(Point3::new(
                    min.x + i as f64 * dx,
                    min.y + j as f64 * dy,
                    min.z + k as f64 * dz,
                ));
            }
        }
    }

    let inside: Vec<bool> = nodes
        .iter()
        .map(|p| is_inside_raycast(p, &surface))
        .collect();

    let idx = |i: usize, j: usize, k: usize| -> usize { k * ny * nx + j * nx + i };

    let mut tetrahedra = Vec::new();
    let mut fill_fractions = Vec::new();

    for k in 0..(nz - 1) {
        for j in 0..(ny - 1) {
            for i in 0..(nx - 1) {
                let v = [
                    idx(i, j, k),
                    idx(i + 1, j, k),
                    idx(i + 1, j + 1, k),
                    idx(i, j + 1, k),
                    idx(i, j, k + 1),
                    idx(i + 1, j, k + 1),
                    idx(i + 1, j + 1, k + 1),
                    idx(i, j + 1, k + 1),
                ];

                let n_inside: usize = v.iter().filter(|&&vi| inside[vi]).count();
                if n_inside == 0 {
                    continue;
                }
                let hex_fill = n_inside as f64 / 8.0;

                let tets = [
                    [v[0], v[1], v[2], v[6]],
                    [v[0], v[1], v[6], v[5]],
                    [v[0], v[3], v[6], v[2]],
                    [v[0], v[3], v[7], v[6]],
                    [v[0], v[4], v[5], v[6]],
                    [v[0], v[4], v[6], v[7]],
                ];

                for tet in &tets {
                    let tet_inside: usize = tet.iter().filter(|&&vi| inside[vi]).count();
                    let f = if tet_inside == 4 {
                        1.0
                    } else if tet_inside == 0 {
                        hex_fill * 0.1
                    } else {
                        tet_inside as f64 / 4.0
                    };

                    if f > 1e-6 {
                        tetrahedra.push(*tet);
                        fill_fractions.push(f);
                    }
                }
            }
        }
    }

    let raw_mesh = Mesh3D { nodes, tetrahedra };
    let (mesh, ff) = crate::internal_mesh3d::compact_mesh(raw_mesh, Some(fill_fractions));

    ImmersedMesh {
        mesh: TetMesh3D {
            nodes: mesh.nodes.into_iter().map(Into::into).collect(),
            tetrahedra: mesh.tetrahedra,
        },
        // compact_mesh receives fill fractions above, so it must preserve them here.
        fill_fractions: ff.expect("compact_mesh should preserve fill fractions when provided"),
    }
}

fn bounding_box(nodes: &[[f64; 3]]) -> (Point3, Point3) {
    let mut min = Point3::new(f64::INFINITY, f64::INFINITY, f64::INFINITY);
    let mut max = Point3::new(f64::NEG_INFINITY, f64::NEG_INFINITY, f64::NEG_INFINITY);
    for p in nodes {
        min.x = min.x.min(p[0]);
        min.y = min.y.min(p[1]);
        min.z = min.z.min(p[2]);
        max.x = max.x.max(p[0]);
        max.y = max.y.max(p[1]);
        max.z = max.z.max(p[2]);
    }
    (min, max)
}

pub(crate) fn is_inside_raycast(point: &Point3, surface: &TriSurface) -> bool {
    let mut votes = 0u32;
    if raycast_axis(point, surface, 2) {
        votes += 1;
    }
    if raycast_axis(point, surface, 1) {
        votes += 1;
    }
    if raycast_axis(point, surface, 0) {
        votes += 1;
    }
    votes >= 2
}

fn raycast_axis(point: &Point3, surface: &TriSurface, axis: usize) -> bool {
    let p = [point.x, point.y, point.z];
    let u_axis = (axis + 1) % 3;
    let v_axis = (axis + 2) % 3;

    let mut crossings = 0u32;

    for tri in &surface.triangles {
        let a = &surface.nodes[tri[0]];
        let b = &surface.nodes[tri[1]];
        let c = &surface.nodes[tri[2]];
        let va = *a;
        let vb = *b;
        let vc = *c;

        let d1u = vb[u_axis] - va[u_axis];
        let d1v = vb[v_axis] - va[v_axis];
        let d2u = vc[u_axis] - va[u_axis];
        let d2v = vc[v_axis] - va[v_axis];
        let det = d1u * d2v - d2u * d1v;
        if det.abs() < 1e-20 {
            continue;
        }

        let dpu = p[u_axis] - va[u_axis];
        let dpv = p[v_axis] - va[v_axis];
        let u_num = dpu * d2v - d2u * dpv;
        let v_num = d1u * dpv - dpu * d1v;

        if det > 0.0 {
            if u_num < 0.0 || v_num <= 0.0 || u_num + v_num >= det {
                continue;
            }
        } else if u_num > 0.0 || v_num >= 0.0 || u_num + v_num <= det {
            continue;
        }

        let u = u_num / det;
        let v = v_num / det;
        let hit = va[axis] + u * (vb[axis] - va[axis]) + v * (vc[axis] - va[axis]);
        if hit > p[axis] {
            crossings += 1;
        }
    }

    crossings % 2 == 1
}

#[cfg(test)]
mod tests {
    use super::*;
    use neco_stl::parse_stl;

    fn tet_volume(nodes: &[[f64; 3]], tet: &[usize; 4]) -> f64 {
        let p0 = nodes[tet[0]];
        let p1 = nodes[tet[1]];
        let p2 = nodes[tet[2]];
        let p3 = nodes[tet[3]];
        let a = [p1[0] - p0[0], p1[1] - p0[1], p1[2] - p0[2]];
        let b = [p2[0] - p0[0], p2[1] - p0[1], p2[2] - p0[2]];
        let c = [p3[0] - p0[0], p3[1] - p0[1], p3[2] - p0[2]];
        let det = a[0] * (b[1] * c[2] - b[2] * c[1]) - a[1] * (b[0] * c[2] - b[2] * c[0])
            + a[2] * (b[0] * c[1] - b[1] * c[0]);
        det.abs() / 6.0
    }

    fn unit_cube_stl() -> TriSurface {
        let ascii = b"solid cube
facet normal 0 0 -1
  outer loop
    vertex 0 0 0
    vertex 1 0 0
    vertex 1 1 0
  endloop
endfacet
facet normal 0 0 -1
  outer loop
    vertex 0 0 0
    vertex 1 1 0
    vertex 0 1 0
  endloop
endfacet
facet normal 0 0 1
  outer loop
    vertex 0 0 1
    vertex 1 1 1
    vertex 1 0 1
  endloop
endfacet
facet normal 0 0 1
  outer loop
    vertex 0 0 1
    vertex 0 1 1
    vertex 1 1 1
  endloop
endfacet
facet normal 0 -1 0
  outer loop
    vertex 0 0 0
    vertex 1 0 1
    vertex 1 0 0
  endloop
endfacet
facet normal 0 -1 0
  outer loop
    vertex 0 0 0
    vertex 0 0 1
    vertex 1 0 1
  endloop
endfacet
facet normal 0 1 0
  outer loop
    vertex 0 1 0
    vertex 1 1 0
    vertex 1 1 1
  endloop
endfacet
facet normal 0 1 0
  outer loop
    vertex 0 1 0
    vertex 1 1 1
    vertex 0 1 1
  endloop
endfacet
facet normal -1 0 0
  outer loop
    vertex 0 0 0
    vertex 0 1 0
    vertex 0 1 1
  endloop
endfacet
facet normal -1 0 0
  outer loop
    vertex 0 0 0
    vertex 0 1 1
    vertex 0 0 1
  endloop
endfacet
facet normal 1 0 0
  outer loop
    vertex 1 0 0
    vertex 1 0 1
    vertex 1 1 1
  endloop
endfacet
facet normal 1 0 0
  outer loop
    vertex 1 0 0
    vertex 1 1 1
    vertex 1 1 0
  endloop
endfacet
endsolid cube";
        parse_stl(ascii).unwrap()
    }

    #[test]
    fn raycast_inside_cube() {
        let surface = unit_cube_stl();
        assert!(is_inside_raycast(&Point3::new(0.5, 0.5, 0.5), &surface));
        assert!(!is_inside_raycast(&Point3::new(2.0, 0.5, 0.5), &surface));
        assert!(!is_inside_raycast(&Point3::new(0.5, 0.5, -1.0), &surface));
    }

    #[test]
    fn immersed_mesh_unit_cube() {
        let surface = unit_cube_stl();
        let result = generate_immersed_mesh(&surface.nodes, &surface.triangles, 0.5);

        assert!(result.mesh.n_nodes() > 0);
        assert!(!result.mesh.tetrahedra.is_empty());
        assert_eq!(result.fill_fractions.len(), result.mesh.tetrahedra.len());

        for &f in &result.fill_fractions {
            assert!((0.0..=1.0).contains(&f), "fill fraction out of range: {f}");
        }

        let vol: f64 = result
            .mesh
            .tetrahedra
            .iter()
            .enumerate()
            .map(|(i, tet)| result.fill_fractions[i] * tet_volume(&result.mesh.nodes, tet))
            .sum();
        assert!(
            vol > 0.5 && vol < 2.0,
            "immersed volume = {vol}, expected ≈ 1.0 (tolerance for boundary cells)"
        );
    }
}
