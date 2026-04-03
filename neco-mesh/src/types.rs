/// 1D edge mesh.
#[derive(Debug, Clone)]
pub struct EdgeMesh {
    pub nodes: Vec<[f64; 2]>,
    pub edges: Vec<[usize; 2]>,
    pub lengths: Vec<f64>,
    pub arc_params: Vec<f64>,
}

/// 2D triangle mesh.
#[derive(Debug, Clone)]
pub struct TriMesh2D {
    pub nodes: Vec<[f64; 2]>,
    pub triangles: Vec<[usize; 3]>,
}

/// 3D tetrahedral mesh.
#[derive(Debug, Clone)]
pub struct TetMesh3D {
    pub nodes: Vec<[f64; 3]>,
    pub tetrahedra: Vec<[usize; 4]>,
}

impl TetMesh3D {
    pub fn n_nodes(&self) -> usize {
        self.nodes.len()
    }

    pub fn n_tetrahedra(&self) -> usize {
        self.tetrahedra.len()
    }

    pub fn tet_volume(&self, tet_idx: usize) -> f64 {
        let [i0, i1, i2, i3] = self.tetrahedra[tet_idx];
        tet_volume_from_nodes(&self.nodes, &[i0, i1, i2, i3])
    }

    pub fn total_volume(&self) -> f64 {
        (0..self.n_tetrahedra()).map(|i| self.tet_volume(i)).sum()
    }
}

/// Immersed boundary mesh with per-tetrahedron fill fractions.
#[derive(Debug, Clone)]
pub struct ImmersedMesh {
    pub mesh: TetMesh3D,
    pub fill_fractions: Vec<f64>,
}

fn tet_volume_from_nodes(nodes: &[[f64; 3]], tet: &[usize; 4]) -> f64 {
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
