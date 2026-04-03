use crate::point3::Point3;

#[derive(Debug, Clone)]
pub struct Mesh3D {
    pub nodes: Vec<Point3>,
    pub tetrahedra: Vec<[usize; 4]>,
}

impl Mesh3D {
    pub fn n_nodes(&self) -> usize {
        self.nodes.len()
    }

    pub fn n_tetrahedra(&self) -> usize {
        self.tetrahedra.len()
    }

    pub fn tet_volume(&self, tet_idx: usize) -> f64 {
        let [i0, i1, i2, i3] = self.tetrahedra[tet_idx];
        let p0 = &self.nodes[i0];
        let p1 = &self.nodes[i1];
        let p2 = &self.nodes[i2];
        let p3 = &self.nodes[i3];

        let a = [p1.x - p0.x, p1.y - p0.y, p1.z - p0.z];
        let b = [p2.x - p0.x, p2.y - p0.y, p2.z - p0.z];
        let c = [p3.x - p0.x, p3.y - p0.y, p3.z - p0.z];

        let det = a[0] * (b[1] * c[2] - b[2] * c[1]) - a[1] * (b[0] * c[2] - b[2] * c[0])
            + a[2] * (b[0] * c[1] - b[1] * c[0]);
        det.abs() / 6.0
    }

    pub fn total_volume(&self) -> f64 {
        (0..self.n_tetrahedra()).map(|i| self.tet_volume(i)).sum()
    }
}

pub fn compact_mesh(mesh: Mesh3D, fill_fractions: Option<Vec<f64>>) -> (Mesh3D, Option<Vec<f64>>) {
    let n = mesh.nodes.len();
    let mut used = vec![false; n];
    for tet in &mesh.tetrahedra {
        for &vi in tet {
            used[vi] = true;
        }
    }

    let mut old_to_new = vec![0usize; n];
    let mut new_nodes = Vec::new();
    for (i, &u) in used.iter().enumerate() {
        if u {
            old_to_new[i] = new_nodes.len();
            new_nodes.push(mesh.nodes[i]);
        }
    }

    let new_tets: Vec<[usize; 4]> = mesh
        .tetrahedra
        .iter()
        .map(|tet| {
            [
                old_to_new[tet[0]],
                old_to_new[tet[1]],
                old_to_new[tet[2]],
                old_to_new[tet[3]],
            ]
        })
        .collect();

    (
        Mesh3D {
            nodes: new_nodes,
            tetrahedra: new_tets,
        },
        fill_fractions,
    )
}

#[cfg(test)]
pub fn generate_box_mesh(lx: f64, ly: f64, lz: f64, max_edge: f64) -> Mesh3D {
    let nx = (lx / max_edge).ceil() as usize + 1;
    let ny = (ly / max_edge).ceil() as usize + 1;
    let nz = (lz / max_edge).ceil() as usize + 1;
    let dx = lx / (nx - 1) as f64;
    let dy = ly / (ny - 1) as f64;
    let dz = lz / (nz - 1) as f64;

    let mut nodes = Vec::with_capacity(nx * ny * nz);
    for k in 0..nz {
        for j in 0..ny {
            for i in 0..nx {
                nodes.push(Point3::new(
                    i as f64 * dx - lx * 0.5,
                    j as f64 * dy - ly * 0.5,
                    k as f64 * dz - lz * 0.5,
                ));
            }
        }
    }

    let idx = |i: usize, j: usize, k: usize| -> usize { k * ny * nx + j * nx + i };

    let mut tetrahedra = Vec::new();
    for k in 0..(nz - 1) {
        for j in 0..(ny - 1) {
            for i in 0..(nx - 1) {
                let v0 = idx(i, j, k);
                let v1 = idx(i + 1, j, k);
                let v2 = idx(i + 1, j + 1, k);
                let v3 = idx(i, j + 1, k);
                let v4 = idx(i, j, k + 1);
                let v5 = idx(i + 1, j, k + 1);
                let v6 = idx(i + 1, j + 1, k + 1);
                let v7 = idx(i, j + 1, k + 1);

                tetrahedra.push([v0, v1, v2, v6]);
                tetrahedra.push([v0, v1, v6, v5]);
                tetrahedra.push([v0, v3, v6, v2]);
                tetrahedra.push([v0, v3, v7, v6]);
                tetrahedra.push([v0, v4, v5, v6]);
                tetrahedra.push([v0, v4, v6, v7]);
            }
        }
    }

    Mesh3D { nodes, tetrahedra }
}
