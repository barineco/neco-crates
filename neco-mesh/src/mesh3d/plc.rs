use crate::internal_mesh3d::Mesh3D;
use crate::point3::Point3;
use std::collections::{HashMap, HashSet};

const DIHEDRAL_ANGLE_THRESHOLD: f64 = 30.0 * std::f64::consts::PI / 180.0;

/// 3D piecewise linear complex used as TetGen-style surface constraints.
#[derive(Debug, Clone)]
pub struct PLC {
    pub vertices: Vec<Point3>,
    pub segments: Vec<[usize; 2]>,
    pub polygons: Vec<[usize; 3]>,
}

fn edge_key(a: usize, b: usize) -> [usize; 2] {
    if a < b {
        [a, b]
    } else {
        [b, a]
    }
}

fn face_key(a: usize, b: usize, c: usize) -> [usize; 3] {
    let mut f = [a, b, c];
    f.sort();
    f
}

fn triangle_normal(p0: &Point3, p1: &Point3, p2: &Point3) -> Point3 {
    let e1 = p1.sub(p0);
    let e2 = p2.sub(p0);
    Point3::cross(&e1, &e2).normalized()
}

impl PLC {
    pub fn from_surface_mesh(nodes: &[Point3], triangles: &[[usize; 3]]) -> Self {
        let vertices = nodes.to_vec();
        let polygons = triangles.to_vec();

        let mut edge_set = HashSet::new();
        for tri in triangles {
            edge_set.insert(edge_key(tri[0], tri[1]));
            edge_set.insert(edge_key(tri[1], tri[2]));
            edge_set.insert(edge_key(tri[0], tri[2]));
        }
        let segments: Vec<[usize; 2]> = edge_set.into_iter().collect();

        PLC {
            vertices,
            segments,
            polygons,
        }
    }

    pub fn from_mesh3d(mesh: &Mesh3D) -> Self {
        let (vertices, surface_tris) = extract_surface_triangles(mesh);
        let polygons = surface_tris.clone();

        let mut edge_to_tris: HashMap<[usize; 2], Vec<usize>> = HashMap::new();
        for (ti, tri) in surface_tris.iter().enumerate() {
            let edges = [
                edge_key(tri[0], tri[1]),
                edge_key(tri[1], tri[2]),
                edge_key(tri[0], tri[2]),
            ];
            for e in edges {
                edge_to_tris.entry(e).or_default().push(ti);
            }
        }

        let mut segments = Vec::new();
        for (edge, tris) in &edge_to_tris {
            if tris.len() == 1 {
                segments.push(*edge);
            } else if tris.len() == 2 {
                let t0 = &surface_tris[tris[0]];
                let t1 = &surface_tris[tris[1]];
                let n0 = triangle_normal(&vertices[t0[0]], &vertices[t0[1]], &vertices[t0[2]]);
                let n1 = triangle_normal(&vertices[t1[0]], &vertices[t1[1]], &vertices[t1[2]]);
                let cos_angle = Point3::dot(&n0, &n1).clamp(-1.0, 1.0);
                let angle = cos_angle.acos();
                if angle > DIHEDRAL_ANGLE_THRESHOLD {
                    segments.push(*edge);
                }
            } else {
                segments.push(*edge);
            }
        }

        PLC {
            vertices,
            segments,
            polygons,
        }
    }
}

pub fn extract_surface_triangles(mesh: &Mesh3D) -> (Vec<Point3>, Vec<[usize; 3]>) {
    let mut face_info: HashMap<[usize; 3], Vec<(usize, usize)>> = HashMap::new();

    let face_indices: [[usize; 3]; 4] = [[1, 2, 3], [0, 2, 3], [0, 1, 3], [0, 1, 2]];

    for (tet_idx, tet) in mesh.tetrahedra.iter().enumerate() {
        for (fi, face_idx) in face_indices.iter().enumerate() {
            let key = face_key(tet[face_idx[0]], tet[face_idx[1]], tet[face_idx[2]]);
            face_info.entry(key).or_default().push((tet_idx, fi));
        }
    }

    let mut surface_tris = Vec::new();
    for (face, infos) in &face_info {
        if infos.len() == 1 {
            let (tet_idx, opposite_local) = infos[0];
            let tet = &mesh.tetrahedra[tet_idx];
            let fi = &face_indices[opposite_local];
            let v0 = tet[fi[0]];
            let v1 = tet[fi[1]];
            let v2 = tet[fi[2]];

            let v_opp = tet[opposite_local];

            let p0 = &mesh.nodes[v0];
            let p1 = &mesh.nodes[v1];
            let p2 = &mesh.nodes[v2];
            let p_opp = &mesh.nodes[v_opp];

            let normal = Point3::cross(&p1.sub(p0), &p2.sub(p0));
            let to_opp = p_opp.sub(p0);

            if Point3::dot(&normal, &to_opp) > 0.0 {
                surface_tris.push([v0, v2, v1]);
            } else {
                surface_tris.push([v0, v1, v2]);
            }
            let _ = face;
        }
    }

    (mesh.nodes.clone(), surface_tris)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::internal_mesh3d::generate_box_mesh;

    #[test]
    fn test_from_surface_mesh_dedup_edges() {
        let nodes = vec![
            Point3::new(0.0, 0.0, 0.0),
            Point3::new(1.0, 0.0, 0.0),
            Point3::new(0.0, 1.0, 0.0),
            Point3::new(1.0, 1.0, 0.0),
        ];
        let triangles = vec![[0, 1, 2], [1, 3, 2]];
        let plc = PLC::from_surface_mesh(&nodes, &triangles);

        assert_eq!(plc.vertices.len(), 4);
        assert_eq!(plc.polygons.len(), 2);
        assert_eq!(
            plc.segments.len(),
            5,
            "duplicate edges were not removed: {}",
            plc.segments.len()
        );
    }

    #[test]
    fn test_single_tet_plc() {
        let mesh = Mesh3D {
            nodes: vec![
                Point3::new(0.0, 0.0, 0.0),
                Point3::new(1.0, 0.0, 0.0),
                Point3::new(0.0, 1.0, 0.0),
                Point3::new(0.0, 0.0, 1.0),
            ],
            tetrahedra: vec![[0, 1, 2, 3]],
        };

        let plc = PLC::from_mesh3d(&mesh);

        assert_eq!(plc.vertices.len(), 4);
        assert_eq!(
            plc.polygons.len(),
            4,
            "surface triangle count: {}",
            plc.polygons.len()
        );
        assert_eq!(
            plc.segments.len(),
            6,
            "feature edge count: {}",
            plc.segments.len()
        );
    }

    #[test]
    fn test_box_mesh_plc() {
        let mesh = generate_box_mesh(1.0, 1.0, 1.0, 1.0);
        assert_eq!(mesh.n_nodes(), 8, "node count: {}", mesh.n_nodes());

        let plc = PLC::from_mesh3d(&mesh);

        assert_eq!(plc.vertices.len(), 8);

        assert_eq!(
            plc.polygons.len(),
            12,
            "surface triangle count: {}",
            plc.polygons.len()
        );

        let all_edges: HashSet<[usize; 2]> = plc
            .polygons
            .iter()
            .flat_map(|tri| {
                [
                    edge_key(tri[0], tri[1]),
                    edge_key(tri[1], tri[2]),
                    edge_key(tri[0], tri[2]),
                ]
            })
            .collect();
        assert!(
            all_edges.len() >= 18,
            "surface edge count: {}",
            all_edges.len()
        );

        assert_eq!(
            plc.segments.len(),
            12,
            "feature edge count: {}",
            plc.segments.len()
        );
    }

    #[test]
    fn test_extract_surface_triangles_outward_normal() {
        let mesh = Mesh3D {
            nodes: vec![
                Point3::new(0.0, 0.0, 0.0),
                Point3::new(1.0, 0.0, 0.0),
                Point3::new(0.0, 1.0, 0.0),
                Point3::new(0.0, 0.0, 1.0),
            ],
            tetrahedra: vec![[0, 1, 2, 3]],
        };

        let (verts, tris) = extract_surface_triangles(&mesh);

        let cx = 0.25;
        let cy = 0.25;
        let cz = 0.25;
        let centroid = Point3::new(cx, cy, cz);

        for tri in &tris {
            let p0 = &verts[tri[0]];
            let p1 = &verts[tri[1]];
            let p2 = &verts[tri[2]];
            let normal = Point3::cross(&p1.sub(p0), &p2.sub(p0));
            let tri_center = Point3::new(
                (p0.x + p1.x + p2.x) / 3.0,
                (p0.y + p1.y + p2.y) / 3.0,
                (p0.z + p1.z + p2.z) / 3.0,
            );
            let outward = tri_center.sub(&centroid);
            assert!(
                Point3::dot(&normal, &outward) > 0.0,
                "triangle {:?} has an inward normal",
                tri
            );
        }
    }

    #[test]
    fn test_box_surface_area_via_plc() {
        let mesh = generate_box_mesh(1.0, 1.0, 1.0, 1.0);
        let (verts, tris) = extract_surface_triangles(&mesh);

        let area: f64 = tris
            .iter()
            .map(|tri| {
                let p0 = &verts[tri[0]];
                let p1 = &verts[tri[1]];
                let p2 = &verts[tri[2]];
                let cross = Point3::cross(&p1.sub(p0), &p2.sub(p0));
                0.5 * cross.length()
            })
            .sum();

        assert!(
            (area - 6.0).abs() < 1e-10,
            "surface area = {} (expected 6.0)",
            area
        );
    }
}
