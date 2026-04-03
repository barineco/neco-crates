//! Tet-plane clipping.
//!
//! Sequentially splits base mesh tets by planes and classifies inside/outside.

use crate::brep::{Shell, Surface};
use crate::vec3;
#[cfg(test)]
use crate::vec3::tet_volume;
use crate::vec3::{
    newton_refine_01, orthonormal_basis, solve_quadratic_01, tet_aspect_ratio, tet_signed_volume,
};

use super::tolerance::GEO_TOL;

type TetMeshClipResult = Result<(Vec<[f64; 3]>, Vec<[usize; 4]>), String>;

// ─────────────────────────────────────────────────────────────────────────────
// TetClipError
// ─────────────────────────────────────────────────────────────────────────────

#[derive(Debug)]
pub enum TetClipError {
    IntersectionDivergence { surface: String, edge: String },
    DegenerateTet { count: usize },
    NumericalInstability(String),
}

impl std::fmt::Display for TetClipError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            TetClipError::IntersectionDivergence { surface, edge } => {
                write!(f, "intersection diverged: surface={surface}, edge={edge}")
            }
            TetClipError::DegenerateTet { count } => {
                write!(f, "{count} degenerate tets")
            }
            TetClipError::NumericalInstability(msg) => {
                write!(f, "numerical instability: {msg}")
            }
        }
    }
}

impl std::error::Error for TetClipError {}

// ─────────────────────────────────────────────────────────────────────────────
// ClipSurface trait
// ─────────────────────────────────────────────────────────────────────────────

pub trait ClipSurface {
    /// Signed distance (positive=outside, negative=inside)
    fn signed_distance(&self, p: [f64; 3]) -> f64;

    /// Edge-surface intersection parameters t in [0, 1]
    fn intersect_edge_surface(&self, a: [f64; 3], b: [f64; 3]) -> Vec<f64>;

    /// Vertex classification (default: signed_distance + GEO_TOL threshold)
    fn classify_surface(&self, p: [f64; 3]) -> Side {
        let d = self.signed_distance(p);
        if d > GEO_TOL {
            Side::Positive
        } else if d < -GEO_TOL {
            Side::Negative
        } else {
            Side::OnPlane
        }
    }
}

pub struct TetClipWorkspace {
    pub nodes: Vec<[f64; 3]>,
    pub tets: Vec<[usize; 4]>,
    vertex_tol: f64,
}

pub struct ClipPlane {
    pub origin: [f64; 3],
    pub normal: [f64; 3],
    /// Triangle for orient3d
    pub tri: [[f64; 3]; 3],
}

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Side {
    Positive,
    Negative,
    OnPlane,
}

impl ClipPlane {
    pub fn from_origin_normal(origin: [f64; 3], normal: [f64; 3]) -> Self {
        // Build two vectors orthogonal to normal
        let arbitrary = if normal[0].abs() < 0.9 {
            [1.0, 0.0, 0.0]
        } else {
            [0.0, 1.0, 0.0]
        };
        let u = vec3::cross(normal, arbitrary);
        let u_len = vec3::length(u);
        let u = vec3::scale(u, 1.0 / u_len);
        let v = vec3::cross(normal, u);
        let tri = [origin, vec3::add(origin, u), vec3::add(origin, v)];
        Self {
            origin,
            normal,
            tri,
        }
    }

    pub fn classify(&self, p: [f64; 3]) -> Side {
        // Exact on-plane test via orient3d
        let o = neco_cdt::orient3d(self.tri[0], self.tri[1], self.tri[2], p);
        if o == 0.0 {
            return Side::OnPlane;
        }
        // Sign from dot product (independent of orient3d sign convention)
        let d = vec3::dot(vec3::sub(p, self.origin), self.normal);
        if d > 0.0 {
            Side::Positive
        } else {
            Side::Negative
        }
    }

    pub fn intersect_edge(&self, a: [f64; 3], b: [f64; 3]) -> [f64; 3] {
        let da = vec3::dot(vec3::sub(a, self.origin), self.normal);
        let db = vec3::dot(vec3::sub(b, self.origin), self.normal);
        let t = da / (da - db);
        [
            a[0] + t * (b[0] - a[0]),
            a[1] + t * (b[1] - a[1]),
            a[2] + t * (b[2] - a[2]),
        ]
    }
}

impl ClipSurface for ClipPlane {
    fn signed_distance(&self, p: [f64; 3]) -> f64 {
        vec3::dot(vec3::sub(p, self.origin), self.normal)
    }

    fn intersect_edge_surface(&self, a: [f64; 3], b: [f64; 3]) -> Vec<f64> {
        let da = self.signed_distance(a);
        let db = self.signed_distance(b);
        let denom = da - db;
        if denom.abs() < 1e-14 {
            return vec![];
        }
        let t = da / denom;
        if (0.0..=1.0).contains(&t) {
            vec![t]
        } else {
            vec![]
        }
    }

    /// Override: use orient3d exact predicate
    fn classify_surface(&self, p: [f64; 3]) -> Side {
        self.classify(p)
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// ClipSphere / ClipCylinder / ClipCone
// ─────────────────────────────────────────────────────────────────────────────

pub struct ClipSphere {
    center: [f64; 3],
    radius: f64,
}

impl ClipSphere {
    pub fn new(center: [f64; 3], radius: f64) -> Self {
        debug_assert!(radius > 0.0);
        Self { center, radius }
    }
}

impl ClipSurface for ClipSphere {
    fn signed_distance(&self, p: [f64; 3]) -> f64 {
        vec3::length(vec3::sub(p, self.center)) - self.radius
    }

    fn intersect_edge_surface(&self, a: [f64; 3], b: [f64; 3]) -> Vec<f64> {
        let d = vec3::sub(b, a);
        let f = vec3::sub(a, self.center);
        let a2 = vec3::dot(d, d);
        let b2 = 2.0 * vec3::dot(f, d);
        let c2 = vec3::dot(f, f) - self.radius * self.radius;
        solve_quadratic_01(a2, b2, c2)
    }
}

pub struct ClipEllipsoid {
    center: [f64; 3],
    rx: f64,
    ry: f64,
    rz: f64,
}

impl ClipEllipsoid {
    pub fn new(center: [f64; 3], rx: f64, ry: f64, rz: f64) -> Self {
        debug_assert!(rx > 0.0 && ry > 0.0 && rz > 0.0);
        Self { center, rx, ry, rz }
    }
}

impl ClipSurface for ClipEllipsoid {
    fn signed_distance(&self, p: [f64; 3]) -> f64 {
        let dx = (p[0] - self.center[0]) / self.rx;
        let dy = (p[1] - self.center[1]) / self.ry;
        let dz = (p[2] - self.center[2]) / self.rz;
        let f = (dx * dx + dy * dy + dz * dz).sqrt();
        let r_max = self.rx.max(self.ry).max(self.rz);
        (f - 1.0) * r_max
    }

    fn intersect_edge_surface(&self, a: [f64; 3], b: [f64; 3]) -> Vec<f64> {
        let sa = [
            (a[0] - self.center[0]) / self.rx,
            (a[1] - self.center[1]) / self.ry,
            (a[2] - self.center[2]) / self.rz,
        ];
        let sb = [
            (b[0] - self.center[0]) / self.rx,
            (b[1] - self.center[1]) / self.ry,
            (b[2] - self.center[2]) / self.rz,
        ];
        let d = vec3::sub(sb, sa);
        let a2 = vec3::dot(d, d);
        let b2 = 2.0 * vec3::dot(sa, d);
        let c2 = vec3::dot(sa, sa) - 1.0;
        solve_quadratic_01(a2, b2, c2)
    }
}

pub struct ClipCylinder {
    origin: [f64; 3],
    axis: [f64; 3],
    radius: f64,
}

impl ClipCylinder {
    pub fn new(origin: [f64; 3], axis: [f64; 3], radius: f64) -> Self {
        debug_assert!((vec3::length(axis) - 1.0).abs() < 1e-10);
        debug_assert!(radius > 0.0);
        Self {
            origin,
            axis,
            radius,
        }
    }
}

impl ClipSurface for ClipCylinder {
    fn signed_distance(&self, p: [f64; 3]) -> f64 {
        let v = vec3::sub(p, self.origin);
        let along = vec3::dot(v, self.axis);
        let perp = vec3::sub(v, vec3::scale(self.axis, along));
        vec3::length(perp) - self.radius
    }

    fn intersect_edge_surface(&self, a: [f64; 3], b: [f64; 3]) -> Vec<f64> {
        let d = vec3::sub(b, a);
        let f = vec3::sub(a, self.origin);
        let d_dot_ax = vec3::dot(d, self.axis);
        let f_dot_ax = vec3::dot(f, self.axis);
        let d_perp = vec3::sub(d, vec3::scale(self.axis, d_dot_ax));
        let f_perp = vec3::sub(f, vec3::scale(self.axis, f_dot_ax));
        let a2 = vec3::dot(d_perp, d_perp);
        let b2 = 2.0 * vec3::dot(f_perp, d_perp);
        let c2 = vec3::dot(f_perp, f_perp) - self.radius * self.radius;
        solve_quadratic_01(a2, b2, c2)
    }
}

pub struct ClipCone {
    origin: [f64; 3],
    axis: [f64; 3],
    half_angle: f64,
}

impl ClipCone {
    pub fn new(origin: [f64; 3], axis: [f64; 3], half_angle: f64) -> Self {
        debug_assert!((vec3::length(axis) - 1.0).abs() < 1e-10);
        debug_assert!(half_angle > 0.0 && half_angle < std::f64::consts::FRAC_PI_2);
        Self {
            origin,
            axis,
            half_angle,
        }
    }
}

impl ClipSurface for ClipCone {
    fn signed_distance(&self, p: [f64; 3]) -> f64 {
        let v = vec3::sub(p, self.origin);
        let along = vec3::dot(v, self.axis);
        let perp = vec3::length(vec3::sub(v, vec3::scale(self.axis, along)));
        perp - along * self.half_angle.tan()
    }

    fn intersect_edge_surface(&self, a: [f64; 3], b: [f64; 3]) -> Vec<f64> {
        let d = vec3::sub(b, a);
        let f = vec3::sub(a, self.origin);
        let tan2 = self.half_angle.tan().powi(2);
        let d_dot_ax = vec3::dot(d, self.axis);
        let f_dot_ax = vec3::dot(f, self.axis);
        let d_perp_sq = vec3::dot(d, d) - d_dot_ax * d_dot_ax;
        let f_perp_sq = vec3::dot(f, f) - f_dot_ax * f_dot_ax;
        let d_perp_f_perp = vec3::dot(d, f) - d_dot_ax * f_dot_ax;
        let a2 = d_perp_sq - tan2 * d_dot_ax * d_dot_ax;
        let b2 = 2.0 * (d_perp_f_perp - tan2 * d_dot_ax * f_dot_ax);
        let c2 = f_perp_sq - tan2 * f_dot_ax * f_dot_ax;
        // Filter: only keep roots where the point is on the apex-forward side
        solve_quadratic_01(a2, b2, c2)
            .into_iter()
            .filter(|&t| {
                let p = [a[0] + t * d[0], a[1] + t * d[1], a[2] + t * d[2]];
                vec3::dot(vec3::sub(p, self.origin), self.axis) >= 0.0
            })
            .collect()
    }
}

pub struct ClipTorus {
    center: [f64; 3],
    axis: [f64; 3], // normalized
    major_radius: f64,
    minor_radius: f64,
}

impl ClipTorus {
    pub fn new(center: [f64; 3], axis: [f64; 3], major_radius: f64, minor_radius: f64) -> Self {
        debug_assert!((vec3::length(axis) - 1.0).abs() < 1e-10);
        debug_assert!(major_radius > 0.0 && minor_radius > 0.0);
        Self {
            center,
            axis,
            major_radius,
            minor_radius,
        }
    }

    /// Build orthonormal frame (u, v, axis) and transform point to local coords
    /// where axis maps to Z.
    fn to_local(&self, p: [f64; 3]) -> [f64; 3] {
        let (u, v) = orthonormal_basis(self.axis);
        let rel = vec3::sub(p, self.center);
        [
            vec3::dot(rel, u),
            vec3::dot(rel, v),
            vec3::dot(rel, self.axis),
        ]
    }
}

impl ClipSurface for ClipTorus {
    fn signed_distance(&self, p: [f64; 3]) -> f64 {
        let v = vec3::sub(p, self.center);
        let h = vec3::dot(v, self.axis);
        let r_proj = vec3::length(vec3::sub(v, vec3::scale(self.axis, h)));
        let d_center = ((r_proj - self.major_radius).powi(2) + h * h).sqrt();
        d_center - self.minor_radius
    }

    fn intersect_edge_surface(&self, a: [f64; 3], b: [f64; 3]) -> Vec<f64> {
        // Transform to local coords (center=origin, axis=Z)
        let a_loc = self.to_local(a);
        let b_loc = self.to_local(b);
        let d_loc = vec3::sub(b_loc, a_loc);

        let big_r = self.major_radius;
        let small_r = self.minor_radius;

        // Torus implicit: (x^2+y^2+z^2+R^2-r^2)^2 - 4R^2(x^2+y^2) = 0
        // P(t) = a_loc + t * d_loc
        // f(t) and df(t) for Newton refinement
        let f = |t: f64| -> f64 {
            let px = a_loc[0] + t * d_loc[0];
            let py = a_loc[1] + t * d_loc[1];
            let pz = a_loc[2] + t * d_loc[2];
            let sum_sq = px * px + py * py + pz * pz;
            let s = sum_sq + big_r * big_r - small_r * small_r;
            s * s - 4.0 * big_r * big_r * (px * px + py * py)
        };

        let df = |t: f64| -> f64 {
            let px = a_loc[0] + t * d_loc[0];
            let py = a_loc[1] + t * d_loc[1];
            let pz = a_loc[2] + t * d_loc[2];
            let sum_sq = px * px + py * py + pz * pz;
            let s = sum_sq + big_r * big_r - small_r * small_r;
            let ds = 2.0 * (px * d_loc[0] + py * d_loc[1] + pz * d_loc[2]);
            let d_xy = 2.0 * (px * d_loc[0] + py * d_loc[1]);
            2.0 * s * ds - 4.0 * big_r * big_r * d_xy
        };

        // Subdivide [0, 1] into N intervals, find sign changes and near-zero sample points
        const N: usize = 64;
        let mut roots = Vec::new();
        let mut samples = Vec::with_capacity(N + 1);
        for i in 0..=N {
            let t = i as f64 / N as f64;
            samples.push((t, f(t)));
        }

        for i in 0..N {
            let (t0, f0) = samples[i];
            let (t1, f1) = samples[i + 1];

            // Exact (or near-exact) root at sample point
            if f0.abs() < 1e-8 && (0.0..=1.0).contains(&t0) {
                roots.push(t0);
                continue; // don't also bracket from this interval
            }

            if f0 * f1 < 0.0 {
                // Sign change — bracket root with Newton
                if let Some(root) = newton_refine_01(t0, t1, &f, &df) {
                    roots.push(root);
                }
            } else if f0 * f1 > 0.0 {
                // Check midpoint for tangent / narrow crossing cases
                let t_mid = (t0 + t1) * 0.5;
                let f_mid = f(t_mid);
                if f_mid.abs() < 1e-6 {
                    if let Some(root) = newton_refine_01(t0, t1, &f, &df) {
                        roots.push(root);
                    }
                } else if f0 * f_mid < 0.0 {
                    // Two sign changes within this interval
                    if let Some(r1) = newton_refine_01(t0, t_mid, &f, &df) {
                        roots.push(r1);
                    }
                    if let Some(r2) = newton_refine_01(t_mid, t1, &f, &df) {
                        roots.push(r2);
                    }
                }
            }
        }
        // Check last sample point
        let (t_last, f_last) = samples[N];
        if f_last.abs() < 1e-8 && t_last <= 1.0 {
            roots.push(t_last);
        }

        // Deduplicate roots that are too close
        roots.sort_by(|a, b| a.total_cmp(b));
        roots.dedup_by(|a, b| (*a - *b).abs() < 1e-10);
        roots
    }
}

impl TetClipWorkspace {
    pub fn new(nodes: Vec<[f64; 3]>, tets: Vec<[usize; 4]>) -> Self {
        Self {
            nodes,
            tets,
            vertex_tol: 1e-10,
        }
    }

    pub fn add_node(&mut self, p: [f64; 3]) -> usize {
        for (i, existing) in self.nodes.iter().enumerate() {
            let d = vec3::sub(p, *existing);
            if vec3::dot(d, d) < self.vertex_tol * self.vertex_tol {
                return i;
            }
        }
        let idx = self.nodes.len();
        self.nodes.push(p);
        idx
    }
}

/// Degenerate tet check (duplicate vertex indices).
#[inline]
fn is_degenerate(tet: &[usize; 4]) -> bool {
    tet[0] == tet[1]
        || tet[0] == tet[2]
        || tet[0] == tet[3]
        || tet[1] == tet[2]
        || tet[1] == tet[3]
        || tet[2] == tet[3]
}

/// Clip a tet by a plane -> (positive tets, negative tets).
///
/// Degenerate tets are excluded from results.
pub fn clip_tet(
    workspace: &mut TetClipWorkspace,
    tet: [usize; 4],
    plane: &ClipPlane,
) -> (Vec<[usize; 4]>, Vec<[usize; 4]>) {
    let classified: [(Side, usize); 4] =
        std::array::from_fn(|i| (plane.classify(workspace.nodes[tet[i]]), tet[i]));

    let n_pos = classified
        .iter()
        .filter(|(s, _)| *s == Side::Positive)
        .count();
    let n_neg = classified
        .iter()
        .filter(|(s, _)| *s == Side::Negative)
        .count();

    // All vertices on positive side
    if n_neg == 0 {
        return (vec![tet], vec![]);
    }
    // All vertices on negative side
    if n_pos == 0 {
        return (vec![], vec![tet]);
    }

    let (pos, neg) = if n_pos == 1 {
        clip_tet_1_3(workspace, &classified, plane, Side::Positive)
    } else if n_neg == 1 {
        clip_tet_1_3(workspace, &classified, plane, Side::Negative)
    } else {
        // 2+2 split
        clip_tet_2_2(workspace, &classified, plane)
    };

    // Remove degenerate tets
    let pos = pos.into_iter().filter(|t| !is_degenerate(t)).collect();
    let neg = neg.into_iter().filter(|t| !is_degenerate(t)).collect();
    (pos, neg)
}

/// 1+3 split: one vertex on isolated_side, three on opposite.
fn clip_tet_1_3(
    workspace: &mut TetClipWorkspace,
    classified: &[(Side, usize); 4],
    plane: &ClipPlane,
    isolated_side: Side,
) -> (Vec<[usize; 4]>, Vec<[usize; 4]>) {
    let iso_pos = classified
        .iter()
        .position(|(s, _)| *s == isolated_side)
        .unwrap();
    let v_iso = classified[iso_pos].1;
    let others: Vec<usize> = classified
        .iter()
        .enumerate()
        .filter(|&(i, _)| i != iso_pos)
        .map(|(_, (_, vi))| *vi)
        .collect();

    let cut_pts: [usize; 3] = std::array::from_fn(|j| {
        let other_pos = classified
            .iter()
            .position(|(_, vi)| *vi == others[j])
            .unwrap();
        if classified[other_pos].0 == Side::OnPlane {
            others[j]
        } else {
            let p = plane.intersect_edge(workspace.nodes[v_iso], workspace.nodes[others[j]]);
            workspace.add_node(p)
        }
    });

    // Isolated vertex side: 1 tet
    let iso_tet = [v_iso, cut_pts[0], cut_pts[1], cut_pts[2]];
    // Opposite side: prism -> 3 tets (Dompierre)
    let prism_tets = decompose_prism(cut_pts, [others[0], others[1], others[2]]);

    if isolated_side == Side::Positive {
        (vec![iso_tet], prism_tets)
    } else {
        (prism_tets, vec![iso_tet])
    }
}

/// Decompose prism (top, bot) into 3 tets.
/// Dompierre: diagonal chosen by minimum vertex index.
fn decompose_prism(top: [usize; 3], bot: [usize; 3]) -> Vec<[usize; 4]> {
    let all = [top[0], top[1], top[2], bot[0], bot[1], bot[2]];
    let min_v = *all.iter().min().unwrap();
    let min_pair = (0..3)
        .find(|&i| top[i] == min_v || bot[i] == min_v)
        .unwrap();

    match min_pair {
        0 => vec![
            [top[0], bot[0], bot[1], bot[2]],
            [top[0], top[1], bot[1], top[2]],
            [top[0], bot[1], bot[2], top[2]],
        ],
        1 => vec![
            [top[1], bot[1], bot[2], bot[0]],
            [top[1], top[2], bot[2], top[0]],
            [top[1], bot[2], bot[0], top[0]],
        ],
        _ => vec![
            [top[2], bot[2], bot[0], bot[1]],
            [top[2], top[0], bot[0], top[1]],
            [top[2], bot[0], bot[1], top[1]],
        ],
    }
}

impl TetClipWorkspace {
    /// Clip all tets by one plane, keeping the specified side.
    pub fn clip_by_plane(&mut self, plane: &ClipPlane, keep: Side) {
        let old_tets = std::mem::take(&mut self.tets);
        for tet in old_tets {
            let (pos_tets, neg_tets) = clip_tet(self, tet, plane);
            match keep {
                Side::Positive => self.tets.extend(pos_tets),
                Side::Negative => self.tets.extend(neg_tets),
                Side::OnPlane => {
                    self.tets.extend(pos_tets);
                    self.tets.extend(neg_tets);
                }
            }
        }
    }

    /// Remove unused nodes and compact indices.
    pub fn compact(self) -> (Vec<[f64; 3]>, Vec<[usize; 4]>) {
        let mut used = vec![false; self.nodes.len()];
        for tet in &self.tets {
            for &vi in tet {
                used[vi] = true;
            }
        }
        let mut new_idx = vec![0usize; self.nodes.len()];
        let mut new_nodes = Vec::new();
        for (i, &is_used) in used.iter().enumerate() {
            if is_used {
                new_idx[i] = new_nodes.len();
                new_nodes.push(self.nodes[i]);
            }
        }
        let new_tets: Vec<[usize; 4]> = self
            .tets
            .iter()
            .map(|tet| {
                [
                    new_idx[tet[0]],
                    new_idx[tet[1]],
                    new_idx[tet[2]],
                    new_idx[tet[3]],
                ]
            })
            .collect();
        (new_nodes, new_tets)
    }
}

/// Generate ClipPlanes from a box shell.
pub fn clip_planes_from_box_shell(shell: &Shell) -> Result<Vec<ClipPlane>, String> {
    shell
        .faces
        .iter()
        .map(|face| match &face.surface {
            Surface::Plane { origin, normal } => {
                Ok(ClipPlane::from_origin_normal(*origin, *normal))
            }
            other => Err(format!(
                "clip_planes_from_box_shell: non-Plane face found ({other:?})"
            )),
        })
        .collect()
}

/// Subtract box operand from mesh.
///
/// For each plane: positive tets (outside operand) are kept;
/// negative tets (inside candidate) proceed to next plane.
/// Tets surviving all planes are discarded (inside operand).
pub fn clip_mesh_subtract_box(
    nodes: Vec<[f64; 3]>,
    tets: Vec<[usize; 4]>,
    shell: &Shell,
) -> TetMeshClipResult {
    let planes = clip_planes_from_box_shell(shell)?;
    let mut ws = TetClipWorkspace::new(nodes, vec![]);
    // candidates: tets not yet classified
    let mut candidates = tets;
    // kept: confirmed outside tets
    let mut kept: Vec<[usize; 4]> = Vec::new();

    for plane in &planes {
        let mut next_candidates = Vec::new();
        for tet in candidates {
            let (pos_tets, neg_tets) = clip_tet(&mut ws, tet, plane);
            // Positive = outside operand -> keep
            kept.extend(pos_tets);
            // Negative = still inside candidate -> next plane
            next_candidates.extend(neg_tets);
        }
        candidates = next_candidates;
    }
    // Remaining candidates are inside operand -> discard

    ws.tets = kept;
    Ok(ws.compact())
}

/// Intersect with box operand: keep only interior.
pub fn clip_mesh_intersect_box(
    nodes: Vec<[f64; 3]>,
    tets: Vec<[usize; 4]>,
    shell: &Shell,
) -> TetMeshClipResult {
    let planes = clip_planes_from_box_shell(shell)?;
    let mut ws = TetClipWorkspace::new(nodes, tets);
    for plane in &planes {
        ws.clip_by_plane(plane, Side::Negative);
    }
    Ok(ws.compact())
}

// ─────────────────────────────────────────────────────────────────────────────
// Quality improvement: aspect ratio / adjacency / 2-3 flip / Laplacian smoothing
// ─────────────────────────────────────────────────────────────────────────────

fn sorted_tri(a: usize, b: usize, c: usize) -> (usize, usize, usize) {
    let mut v = [a, b, c];
    v.sort();
    (v[0], v[1], v[2])
}

fn tet_faces(tet: &[usize; 4]) -> [(usize, usize, usize); 4] {
    [
        sorted_tri(tet[0], tet[1], tet[2]),
        sorted_tri(tet[0], tet[1], tet[3]),
        sorted_tri(tet[0], tet[2], tet[3]),
        sorted_tri(tet[1], tet[2], tet[3]),
    ]
}

pub(crate) fn build_adjacency(tets: &[[usize; 4]]) -> Vec<[Option<usize>; 4]> {
    use std::collections::HashMap;
    let mut face_to_tet: HashMap<(usize, usize, usize), Vec<usize>> = HashMap::new();
    for (ti, tet) in tets.iter().enumerate() {
        for f in tet_faces(tet) {
            face_to_tet.entry(f).or_default().push(ti);
        }
    }
    let mut neighbors = vec![[None; 4]; tets.len()];
    for (ti, tet) in tets.iter().enumerate() {
        for (fi, f) in tet_faces(tet).iter().enumerate() {
            if let Some(adj_list) = face_to_tet.get(f) {
                for &adj in adj_list {
                    if adj != ti {
                        neighbors[ti][fi] = Some(adj);
                    }
                }
            }
        }
    }
    neighbors
}

/// Find tet containing point p (linear search).
pub(crate) fn find_containing_tet(
    nodes: &[[f64; 3]],
    tets: &[[usize; 4]],
    p: [f64; 3],
) -> Option<usize> {
    for (ti, tet) in tets.iter().enumerate() {
        let a = nodes[tet[0]];
        let b = nodes[tet[1]];
        let c = nodes[tet[2]];
        let d = nodes[tet[3]];

        let vol = neco_cdt::orient3d(a, b, c, d);
        if vol.abs() < 1e-30 {
            continue;
        } // degenerate tet

        // All 4 sub-tets formed by replacing one vertex with p must have the same sign as vol
        let sign = vol.signum();
        let o0 = neco_cdt::orient3d(p, b, c, d) * sign;
        let o1 = neco_cdt::orient3d(a, p, c, d) * sign;
        let o2 = neco_cdt::orient3d(a, b, p, d) * sign;
        let o3 = neco_cdt::orient3d(a, b, c, p) * sign;

        if o0 >= 0.0 && o1 >= 0.0 && o2 >= 0.0 && o3 >= 0.0 {
            return Some(ti);
        }
    }
    None
}

/// Bowyer-Watson cavity: BFS collect tets whose circumsphere contains point.
pub(crate) fn build_cavity(
    nodes: &[[f64; 3]],
    tets: &[[usize; 4]],
    adj: &[[Option<usize>; 4]],
    start_tet: usize,
    point: [f64; 3],
) -> Vec<usize> {
    use std::collections::{HashSet, VecDeque};
    let mut cavity = vec![start_tet];
    let mut visited = HashSet::new();
    visited.insert(start_tet);
    let mut queue = VecDeque::new();
    queue.push_back(start_tet);

    while let Some(ti) = queue.pop_front() {
        for &neighbor in adj[ti].iter().take(4) {
            if let Some(ni) = neighbor {
                if visited.contains(&ni) {
                    continue;
                }
                visited.insert(ni);

                let tet = &tets[ni];
                let a = nodes[tet[0]];
                let b = nodes[tet[1]];
                let c = nodes[tet[2]];
                let d = nodes[tet[3]];

                // Ensure positive orientation for insphere
                let orient = neco_cdt::orient3d(a, b, c, d);
                let in_sphere = if orient > 0.0 {
                    neco_cdt::insphere(a, b, c, d, point)
                } else if orient < 0.0 {
                    // Swap two vertices to flip orientation
                    neco_cdt::insphere(b, a, c, d, point)
                } else {
                    continue; // degenerate, skip
                };

                if in_sphere > 0.0 {
                    cavity.push(ni);
                    queue.push_back(ni);
                }
            }
        }
    }
    cavity
}

/// Extract cavity boundary faces, oriented to form positive-volume tets with new node.
fn extract_cavity_boundary(
    nodes: &[[f64; 3]],
    tets: &[[usize; 4]],
    adj: &[[Option<usize>; 4]],
    cavity: &[usize],
    new_point: [f64; 3],
) -> Vec<[usize; 3]> {
    let cavity_set: std::collections::HashSet<usize> = cavity.iter().copied().collect();
    let mut boundary = Vec::new();

    for &ti in cavity {
        let tet = tets[ti];
        // 4 faces matching tet_faces() ordering used by build_adjacency:
        //   fi=0: (tet[0], tet[1], tet[2])
        //   fi=1: (tet[0], tet[1], tet[3])
        //   fi=2: (tet[0], tet[2], tet[3])
        //   fi=3: (tet[1], tet[2], tet[3])
        let face_defs: [[usize; 3]; 4] = [
            [tet[0], tet[1], tet[2]],
            [tet[0], tet[1], tet[3]],
            [tet[0], tet[2], tet[3]],
            [tet[1], tet[2], tet[3]],
        ];

        for (fi, face) in face_defs.iter().enumerate() {
            let is_boundary = match adj[ti][fi] {
                Some(ni) => !cavity_set.contains(&ni),
                None => true,
            };
            if !is_boundary {
                continue;
            }

            // Orient face so tet [face, new_point] has positive signed volume.
            // Inline signed volume computation (same formula as tet_signed_volume).
            let sv = {
                let a = nodes[face[0]];
                let b = nodes[face[1]];
                let c = nodes[face[2]];
                let d = new_point;
                let ab = vec3::sub(b, a);
                let ac = vec3::sub(c, a);
                let ad = vec3::sub(d, a);
                vec3::dot(ab, vec3::cross(ac, ad))
            };

            if sv > 0.0 {
                boundary.push(*face);
            } else if sv < 0.0 {
                boundary.push([face[1], face[0], face[2]]); // flip
            }
            // sv == 0: degenerate (point on face plane), skip
        }
    }
    boundary
}

/// Insert Steiner point via Bowyer-Watson. Returns new node index or None.
pub fn insert_steiner_point(
    nodes: &mut Vec<[f64; 3]>,
    tets: &mut Vec<[usize; 4]>,
    point: [f64; 3],
) -> Option<usize> {
    let adj = build_adjacency(tets);
    let containing = find_containing_tet(nodes, tets, point)?;
    insert_steiner_point_core(nodes, tets, point, containing, &adj)
}

/// Bowyer-Watson insertion with known containing tet.
fn insert_steiner_point_core(
    nodes: &mut Vec<[f64; 3]>,
    tets: &mut Vec<[usize; 4]>,
    point: [f64; 3],
    containing: usize,
    adj: &[[Option<usize>; 4]],
) -> Option<usize> {
    let cavity = build_cavity(nodes, tets, adj, containing, point);
    if cavity.is_empty() {
        return None;
    }

    let boundary_faces = extract_cavity_boundary(nodes, tets, adj, &cavity, point);
    if boundary_faces.len() < 4 {
        return None;
    }

    let vi = nodes.len();
    nodes.push(point);

    let new_tets: Vec<[usize; 4]> = boundary_faces
        .iter()
        .map(|face| [face[0], face[1], face[2], vi])
        .collect();

    // Verify all new tets have positive volume; rollback if not
    for tet in &new_tets {
        if tet_signed_volume(nodes, tet) <= -1e-20 {
            nodes.pop(); // Rollback
            return None;
        }
    }

    let mut cavity_sorted = cavity.to_vec();
    cavity_sorted.sort_unstable_by(|a, b| b.cmp(a));
    for &ci in &cavity_sorted {
        if ci < tets.len() {
            tets.swap_remove(ci);
        }
    }

    tets.extend(new_tets);

    Some(vi)
}

fn find_shared_face(a: &[usize; 4], b: &[usize; 4]) -> Option<(usize, usize, usize)> {
    let fa = tet_faces(a);
    let fb = tet_faces(b);
    for f in &fa {
        if fb.contains(f) {
            return Some(*f);
        }
    }
    None
}

fn find_opposite_vertex(tet: &[usize; 4], face: &(usize, usize, usize)) -> Option<usize> {
    tet.iter()
        .find(|&&v| v != face.0 && v != face.1 && v != face.2)
        .copied()
}

fn try_flip_2_3(
    nodes: &[[f64; 3]],
    tet_a: &[usize; 4],
    tet_b: &[usize; 4],
) -> Option<[[usize; 4]; 3]> {
    let shared = find_shared_face(tet_a, tet_b)?;
    let apex_a = find_opposite_vertex(tet_a, &shared)?;
    let apex_b = find_opposite_vertex(tet_b, &shared)?;
    let new_tets = [
        [shared.0, shared.1, apex_a, apex_b],
        [shared.1, shared.2, apex_a, apex_b],
        [shared.0, shared.2, apex_a, apex_b],
    ];
    // Check positive volume
    for t in &new_tets {
        if tet_signed_volume(nodes, t).abs() < 1e-30 {
            return None;
        }
    }
    let old_worst = tet_aspect_ratio(nodes, tet_a).max(tet_aspect_ratio(nodes, tet_b));
    let new_worst = new_tets
        .iter()
        .map(|t| tet_aspect_ratio(nodes, t))
        .fold(0.0f64, f64::max);
    if new_worst < old_worst {
        Some(new_tets)
    } else {
        None
    }
}

pub const ASPECT_RATIO_THRESHOLD: f64 = 15.0;

/// 4-4 flip: rearrange 4 tets sharing an edge into a different diagonal configuration.
pub(crate) fn try_flip_4_4(
    nodes: &[[f64; 3]],
    tets: &[[usize; 4]],
    tet_indices: &[usize],
) -> Option<[[usize; 4]; 4]> {
    if tet_indices.len() != 4 {
        return None;
    }

    // Count vertex occurrences
    let mut counts: std::collections::HashMap<usize, u32> = std::collections::HashMap::new();
    for &ti in tet_indices {
        for &v in &tets[ti] {
            *counts.entry(v).or_insert(0) += 1;
        }
    }

    // Shared edge vertices: appear 4 times
    let shared: Vec<usize> = counts
        .iter()
        .filter(|(_, &c)| c == 4)
        .map(|(&v, _)| v)
        .collect();
    if shared.len() != 2 {
        return None;
    }

    // Ring vertices: appear less than 4 times
    let ring: Vec<usize> = counts
        .iter()
        .filter(|(_, &c)| c < 4)
        .map(|(&v, _)| v)
        .collect();
    if ring.len() != 4 {
        return None;
    }

    let (a, b) = (shared[0], shared[1]);

    find_best_4_4_config(nodes, a, b, &ring)
}

/// Find best 4-4 flip configuration.
///
/// Tries 3 diagonal splits of the 4-vertex ring, returns best quality.
fn find_best_4_4_config(
    nodes: &[[f64; 3]],
    a: usize,
    b: usize,
    ring: &[usize],
) -> Option<[[usize; 4]; 4]> {
    // Try 3 diagonal splits of ring vertices
    let perms = [
        ([ring[0], ring[1]], [ring[2], ring[3]]), // diagonal 0-1 vs 2-3
        ([ring[0], ring[2]], [ring[1], ring[3]]), // diagonal 0-2 vs 1-3
        ([ring[0], ring[3]], [ring[1], ring[2]]), // diagonal 0-3 vs 1-2
    ];

    let mut best: Option<[[usize; 4]; 4]> = None;
    let mut best_worst = f64::INFINITY;

    for (diag, anti) in &perms {
        let candidate = [
            [a, diag[0], anti[0], anti[1]],
            [b, diag[0], anti[0], anti[1]],
            [a, diag[1], anti[0], anti[1]],
            [b, diag[1], anti[0], anti[1]],
        ];

        // All tets must have positive volume
        let all_positive = candidate
            .iter()
            .all(|t| tet_signed_volume(nodes, t).abs() > 1e-30);
        if !all_positive {
            continue;
        }

        let worst = candidate
            .iter()
            .map(|t| tet_aspect_ratio(nodes, t))
            .fold(0.0f64, f64::max);

        if worst < best_worst {
            best_worst = worst;
            best = Some(candidate);
        }
    }

    best
}

/// Improve quality via 2-3, 3-2, and 4-4 flips.
pub fn improve_quality_flips(nodes: &[[f64; 3]], tets: &mut Vec<[usize; 4]>) {
    for _ in 0..30 {
        let adj = build_adjacency(tets);
        let edge_map = build_edge_to_tets(tets);
        let mut improved = false;

        // Find worst sliver
        let worst_idx = (0..tets.len())
            .filter(|&i| tet_aspect_ratio(nodes, &tets[i]) > ASPECT_RATIO_THRESHOLD)
            .max_by(|&a, &b| {
                tet_aspect_ratio(nodes, &tets[a])
                    .partial_cmp(&tet_aspect_ratio(nodes, &tets[b]))
                    .unwrap_or(std::cmp::Ordering::Equal)
            });

        let ti = match worst_idx {
            Some(i) => i,
            None => break,
        };

        // Try 2-3 flip on each face
        for &neighbor in adj[ti].iter().take(4) {
            if let Some(ni) = neighbor {
                if let Some(new3) = try_flip_2_3(nodes, &tets[ti], &tets[ni]) {
                    let (ra, rb) = if ti > ni { (ti, ni) } else { (ni, ti) };
                    tets.swap_remove(ra);
                    if rb < tets.len() {
                        tets.swap_remove(rb);
                    }
                    tets.extend(new3);
                    improved = true;
                    break;
                }
            }
        }
        if improved {
            continue;
        }

        // Try 3-2 / 4-4 flip on each edge
        let tet = tets[ti];
        'edge_loop: for i in 0..4 {
            for j in (i + 1)..4 {
                let edge = (tet[i].min(tet[j]), tet[i].max(tet[j]));
                if let Some(ring) = edge_map.get(&edge) {
                    if ring.len() == 3 {
                        if let Some(new2) = try_flip_3_2(nodes, tets, ring) {
                            let mut sorted = ring.clone();
                            sorted.sort_unstable_by(|a, b| b.cmp(a));
                            for &ci in &sorted {
                                if ci < tets.len() {
                                    tets.swap_remove(ci);
                                }
                            }
                            tets.extend(new2);
                            improved = true;
                            break 'edge_loop;
                        }
                    }
                    if ring.len() == 4 {
                        let current_worst = ring
                            .iter()
                            .map(|&i| tet_aspect_ratio(nodes, &tets[i]))
                            .fold(0.0f64, f64::max);

                        if let Some(new4) = try_flip_4_4(nodes, tets, ring) {
                            let new_worst = new4
                                .iter()
                                .map(|t| tet_aspect_ratio(nodes, t))
                                .fold(0.0f64, f64::max);

                            if new_worst < current_worst {
                                let mut sorted = ring.clone();
                                sorted.sort_unstable_by(|a, b| b.cmp(a));
                                for &ci in &sorted {
                                    if ci < tets.len() {
                                        tets.swap_remove(ci);
                                    }
                                }
                                tets.extend(new4);
                                improved = true;
                                break 'edge_loop;
                            }
                        }
                    }
                }
            }
        }

        if !improved {
            break;
        }
    }
}

/// Legacy quality improvement via 2-3 flip only.
#[deprecated(note = "use improve_quality_flips instead")]
pub fn improve_quality_flip(nodes: &[[f64; 3]], tets: &mut Vec<[usize; 4]>) {
    for _ in 0..10 {
        let adj = build_adjacency(tets);
        let mut improved = false;

        let bad_tets: Vec<usize> = (0..tets.len())
            .filter(|&i| tet_aspect_ratio(nodes, &tets[i]) > ASPECT_RATIO_THRESHOLD)
            .collect();
        if bad_tets.is_empty() {
            break;
        }

        for &ti in &bad_tets {
            if ti >= tets.len() {
                continue;
            }
            for &neighbor in adj[ti].iter().take(4) {
                if let Some(ni) = neighbor {
                    if let Some(new_tets_3) = try_flip_2_3(nodes, &tets[ti], &tets[ni]) {
                        let (remove_a, remove_b) = if ti > ni { (ti, ni) } else { (ni, ti) };
                        tets.swap_remove(remove_a);
                        if remove_b < tets.len() {
                            tets.swap_remove(remove_b);
                        }
                        tets.extend(new_tets_3);
                        improved = true;
                        break;
                    }
                }
            }
            if improved {
                break;
            }
        }
        if !improved {
            break;
        }
    }
}

/// Edge -> sharing tet indices reverse map.
pub(crate) fn build_edge_to_tets(
    tets: &[[usize; 4]],
) -> std::collections::HashMap<(usize, usize), Vec<usize>> {
    let mut map = std::collections::HashMap::new();
    for (ti, tet) in tets.iter().enumerate() {
        for i in 0..4 {
            for j in (i + 1)..4 {
                let edge = (tet[i].min(tet[j]), tet[i].max(tet[j]));
                map.entry(edge).or_insert_with(Vec::new).push(ti);
            }
        }
    }
    map
}

/// 3-2 flip: inverse of 2-3 flip. Removes shared edge, creates new shared face.
pub(crate) fn try_flip_3_2(
    nodes: &[[f64; 3]],
    tets: &[[usize; 4]],
    tet_indices: &[usize],
) -> Option<[[usize; 4]; 2]> {
    if tet_indices.len() != 3 {
        return None;
    }

    // Count vertex occurrences
    let mut counts: std::collections::HashMap<usize, u32> = std::collections::HashMap::new();
    for &ti in tet_indices {
        for &v in &tets[ti] {
            *counts.entry(v).or_insert(0) += 1;
        }
    }

    // Shared edge vertices appear 3 times, opposite vertices appear once each
    let shared: Vec<usize> = counts
        .iter()
        .filter(|(_, &c)| c == 3)
        .map(|(&v, _)| v)
        .collect();
    if shared.len() != 2 {
        return None;
    }

    let others: Vec<usize> = counts
        .iter()
        .filter(|(_, &c)| c < 3)
        .map(|(&v, _)| v)
        .collect();
    if others.len() != 3 {
        return None;
    }

    let (a, b) = (shared[0], shared[1]);
    let (c, d, e) = (others[0], others[1], others[2]);

    // New 2 tets: [c,d,e,a] and [c,d,e,b]
    let new_tets = [[c, d, e, a], [c, d, e, b]];

    // Positive volume check
    for t in &new_tets {
        if tet_signed_volume(nodes, t).abs() < 1e-30 {
            return None;
        }
    }

    // Quality improvement check
    let old_worst = tet_indices
        .iter()
        .map(|&i| tet_aspect_ratio(nodes, &tets[i]))
        .fold(0.0f64, f64::max);
    let new_worst = new_tets
        .iter()
        .map(|t| tet_aspect_ratio(nodes, t))
        .fold(0.0f64, f64::max);

    if new_worst < old_worst {
        Some(new_tets)
    } else {
        None
    }
}

/// Laplacian smoothing: move interior vertices to neighbor centroid.
pub fn smooth_vertices(
    nodes: &mut [[f64; 3]],
    tets: &[[usize; 4]],
    boundary_nodes: &std::collections::HashSet<usize>,
    iterations: usize,
) {
    let n = nodes.len();
    let mut neighbors: Vec<std::collections::HashSet<usize>> = vec![Default::default(); n];
    for tet in tets {
        for i in 0..4 {
            for j in (i + 1)..4 {
                neighbors[tet[i]].insert(tet[j]);
                neighbors[tet[j]].insert(tet[i]);
            }
        }
    }
    for _ in 0..iterations {
        let old_nodes = nodes.to_vec();
        for vi in 0..n {
            if boundary_nodes.contains(&vi) || neighbors[vi].is_empty() {
                continue;
            }
            let count = neighbors[vi].len() as f64;
            let mut avg = [0.0, 0.0, 0.0];
            for &ni in &neighbors[vi] {
                avg = vec3::add(avg, old_nodes[ni]);
            }
            nodes[vi] = vec3::scale(avg, 1.0 / count);
        }
    }
}

/// Detect boundary nodes (surface faces + clip planes).
pub fn detect_boundary_nodes(
    nodes: &[[f64; 3]],
    tets: &[[usize; 4]],
    clip_planes: &[ClipPlane],
) -> std::collections::HashSet<usize> {
    use std::collections::HashMap;
    // Vertices of exterior faces (appearing once)
    let mut face_counts: HashMap<(usize, usize, usize), u32> = HashMap::new();
    for tet in tets {
        for f in tet_faces(tet) {
            *face_counts.entry(f).or_insert(0) += 1;
        }
    }
    let mut boundary = std::collections::HashSet::new();
    for ((a, b, c), count) in &face_counts {
        if *count == 1 {
            boundary.insert(*a);
            boundary.insert(*b);
            boundary.insert(*c);
        }
    }
    // Vertices on clip planes are also immovable
    for (vi, node) in nodes.iter().enumerate() {
        for plane in clip_planes {
            let d = vec3::dot(vec3::sub(*node, plane.origin), plane.normal);
            if d.abs() < 1e-10 {
                boundary.insert(vi);
                break;
            }
        }
    }
    boundary
}

/// 2+2 split: two vertices on each side.
fn clip_tet_2_2(
    workspace: &mut TetClipWorkspace,
    classified: &[(Side, usize); 4],
    plane: &ClipPlane,
) -> (Vec<[usize; 4]>, Vec<[usize; 4]>) {
    let mut pos_verts = Vec::new();
    let mut neg_verts = Vec::new();
    for &(side, vi) in classified {
        match side {
            Side::Positive | Side::OnPlane => pos_verts.push(vi),
            Side::Negative => neg_verts.push(vi),
        }
    }

    // 4 intersection points: one per pos-neg edge pair
    let p00 = {
        let pt = plane.intersect_edge(workspace.nodes[pos_verts[0]], workspace.nodes[neg_verts[0]]);
        workspace.add_node(pt)
    };
    let p01 = {
        let pt = plane.intersect_edge(workspace.nodes[pos_verts[0]], workspace.nodes[neg_verts[1]]);
        workspace.add_node(pt)
    };
    let p10 = {
        let pt = plane.intersect_edge(workspace.nodes[pos_verts[1]], workspace.nodes[neg_verts[0]]);
        workspace.add_node(pt)
    };
    let p11 = {
        let pt = plane.intersect_edge(workspace.nodes[pos_verts[1]], workspace.nodes[neg_verts[1]]);
        workspace.add_node(pt)
    };

    // Each side forms a prism decomposed into 3 tets

    let pos_tets = decompose_prism([pos_verts[0], p00, p01], [pos_verts[1], p10, p11]);
    let neg_tets = decompose_prism([neg_verts[0], p00, p10], [neg_verts[1], p01, p11]);

    (pos_tets, neg_tets)
}

/// Insert Steiner points at worst sliver centroids.
/// Returns total number of inserted points.
pub fn insert_steiner_points_for_slivers(
    nodes: &mut Vec<[f64; 3]>,
    tets: &mut Vec<[usize; 4]>,
    threshold: f64,
    max_insertions: usize,
) -> usize {
    let mut total_inserted = 0;

    for _ in 0..max_insertions {
        let adj = build_adjacency(tets);

        let worst = (0..tets.len())
            .filter(|&i| {
                tet_signed_volume(nodes, &tets[i]).abs() > 1e-30
                    && tet_aspect_ratio(nodes, &tets[i]) > threshold
            })
            .max_by(|&a, &b| {
                tet_aspect_ratio(nodes, &tets[a])
                    .partial_cmp(&tet_aspect_ratio(nodes, &tets[b]))
                    .unwrap_or(std::cmp::Ordering::Equal)
            });

        let si = match worst {
            Some(i) => i,
            None => break,
        };

        let tet = tets[si];
        let centroid = [
            (nodes[tet[0]][0] + nodes[tet[1]][0] + nodes[tet[2]][0] + nodes[tet[3]][0]) / 4.0,
            (nodes[tet[0]][1] + nodes[tet[1]][1] + nodes[tet[2]][1] + nodes[tet[3]][1]) / 4.0,
            (nodes[tet[0]][2] + nodes[tet[1]][2] + nodes[tet[2]][2] + nodes[tet[3]][2]) / 4.0,
        ];

        if insert_steiner_point_core(nodes, tets, centroid, si, &adj).is_some() {
            total_inserted += 1;
        } else {
            break;
        }
    }

    total_inserted
}

/// Remove slivers by collapsing shortest edges.
///
/// Merges two vertices to midpoint; boundary nodes are preserved.
/// Returns number of slivers removed.
pub fn collapse_slivers(
    nodes: &mut [[f64; 3]],
    tets: &mut Vec<[usize; 4]>,
    boundary_nodes: &std::collections::HashSet<usize>,
    threshold: f64,
    max_collapses: usize,
) -> usize {
    let mut total_collapsed = 0;

    for _ in 0..max_collapses {
        // Find worst sliver
        let worst = (0..tets.len())
            .filter(|&i| {
                tet_signed_volume(nodes, &tets[i]).abs() > 1e-30
                    && tet_aspect_ratio(nodes, &tets[i]) > threshold
            })
            .max_by(|&a, &b| {
                tet_aspect_ratio(nodes, &tets[a])
                    .partial_cmp(&tet_aspect_ratio(nodes, &tets[b]))
                    .unwrap_or(std::cmp::Ordering::Equal)
            });

        let si = match worst {
            Some(i) => i,
            None => break,
        };

        let tet = tets[si];

        // Find shortest edge
        let edge_pairs = [
            (tet[0], tet[1]),
            (tet[0], tet[2]),
            (tet[0], tet[3]),
            (tet[1], tet[2]),
            (tet[1], tet[3]),
            (tet[2], tet[3]),
        ];
        let (mut v_keep, mut v_remove) = edge_pairs[0];
        let mut min_len = f64::INFINITY;
        for &(a, b) in &edge_pairs {
            let l = vec3::length(vec3::sub(nodes[a], nodes[b]));
            if l < min_len {
                min_len = l;
                // Prefer keeping boundary nodes
                if boundary_nodes.contains(&a) && !boundary_nodes.contains(&b) {
                    v_keep = a;
                    v_remove = b;
                } else {
                    v_keep = b;
                    v_remove = a;
                }
            }
        }

        // Both boundary: keep v_keep position; one boundary: keep boundary node
        if !boundary_nodes.contains(&v_keep) && !boundary_nodes.contains(&v_remove) {
            // Both interior: move to midpoint
            nodes[v_keep] = [
                (nodes[v_keep][0] + nodes[v_remove][0]) / 2.0,
                (nodes[v_keep][1] + nodes[v_remove][1]) / 2.0,
                (nodes[v_keep][2] + nodes[v_remove][2]) / 2.0,
            ];
        } else if boundary_nodes.contains(&v_keep) && boundary_nodes.contains(&v_remove) {
            // Both boundary: keep v_keep position
        }
        // v_keep is boundary, v_remove is interior -> keep v_keep position

        // Replace all references to v_remove with v_keep
        for tet_ref in tets.iter_mut() {
            for v in tet_ref.iter_mut() {
                if *v == v_remove {
                    *v = v_keep;
                }
            }
        }

        // Remove degenerate tets
        let before = tets.len();
        tets.retain(|t| !is_degenerate(t));
        let removed = before - tets.len();

        if removed > 0 {
            total_collapsed += 1;
        } else {
            break; // collapse didn't remove any tets
        }
    }

    total_collapsed
}

/// Iterative interleaved quality improvement pipeline.
///
/// Each round applies steiner -> flips -> collapse -> smooth.
/// Stops when sliver rate drops below target or max_rounds reached.
pub fn improve_mesh_quality(
    nodes: &mut Vec<[f64; 3]>,
    tets: &mut Vec<[usize; 4]>,
    clip_planes: &[ClipPlane],
    target_sliver_rate: f64,
    max_rounds: usize,
) {
    for _round in 0..max_rounds {
        let n_tets = tets.len();
        let n_slivers = (0..n_tets)
            .filter(|&i| tet_aspect_ratio(nodes, &tets[i]) > ASPECT_RATIO_THRESHOLD)
            .count();
        let rate = n_slivers as f64 / n_tets.max(1) as f64;

        if rate < target_sliver_rate || n_slivers == 0 {
            break;
        }

        // Steiner: inject interior nodes into worst slivers
        insert_steiner_points_for_slivers(nodes, tets, ASPECT_RATIO_THRESHOLD, 50);

        // Flips: try 2-3, 3-2, 4-4 to rearrange topology
        improve_quality_flips(nodes, tets);

        // Collapse: merge short edges of remaining slivers (4x steiner budget)
        let boundary = detect_boundary_nodes(nodes, tets, clip_planes);
        collapse_slivers(nodes, tets, &boundary, ASPECT_RATIO_THRESHOLD, 200);

        // Smooth: move interior nodes to improve shape
        let boundary = detect_boundary_nodes(nodes, tets, clip_planes);
        smooth_vertices(nodes, tets, &boundary, 5);
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// clip_tet_surface: generic tet clipping via ClipSurface trait
// ─────────────────────────────────────────────────────────────────────────────

/// Clip a tet by a generic ClipSurface -> (positive, negative).
///
/// Multi-intersection edges trigger recursive subdivision.
#[allow(clippy::type_complexity)]
pub fn clip_tet_surface(
    workspace: &mut TetClipWorkspace,
    tet: [usize; 4],
    surface: &dyn ClipSurface,
) -> Result<(Vec<[usize; 4]>, Vec<[usize; 4]>), TetClipError> {
    clip_tet_surface_recursive(workspace, tet, surface, 0)
}

const MAX_RECURSION_DEPTH: usize = 10;

#[allow(clippy::type_complexity)]
fn clip_tet_surface_recursive(
    workspace: &mut TetClipWorkspace,
    tet: [usize; 4],
    surface: &dyn ClipSurface,
    depth: usize,
) -> Result<(Vec<[usize; 4]>, Vec<[usize; 4]>), TetClipError> {
    if depth > MAX_RECURSION_DEPTH {
        return Err(TetClipError::NumericalInstability(
            "recursion depth limit reached".into(),
        ));
    }

    let classified: [(Side, usize); 4] =
        std::array::from_fn(|i| (surface.classify_surface(workspace.nodes[tet[i]]), tet[i]));

    let n_pos = classified
        .iter()
        .filter(|(s, _)| *s == Side::Positive)
        .count();
    let n_neg = classified
        .iter()
        .filter(|(s, _)| *s == Side::Negative)
        .count();

    // All vertices on same side
    if n_neg == 0 {
        return Ok((vec![tet], vec![]));
    }
    if n_pos == 0 {
        return Ok((vec![], vec![tet]));
    }

    // Collect intersection params for crossing edges (edges between opposite sides)
    let edges: [(usize, usize); 6] = [(0, 1), (0, 2), (0, 3), (1, 2), (1, 3), (2, 3)];

    let mut has_multi_intersection = false;
    let mut multi_edge: Option<(usize, usize, Vec<f64>)> = None;

    for &(ei, ej) in &edges {
        let si = classified[ei].0;
        let sj = classified[ej].0;
        // Only process crossing edges (opposite sides)
        let is_crossing = (si == Side::Positive && sj == Side::Negative)
            || (si == Side::Negative && sj == Side::Positive);
        if !is_crossing {
            continue;
        }
        let vi = classified[ei].1;
        let vj = classified[ej].1;
        let params = surface.intersect_edge_surface(workspace.nodes[vi], workspace.nodes[vj]);
        if params.len() >= 2 {
            has_multi_intersection = true;
            multi_edge = Some((ei, ej, params));
            break;
        }
    }

    if has_multi_intersection {
        // Recursive split at first intersection of multi-intersection edge
        let Some((ei, ej, params)) = multi_edge else {
            return Err(TetClipError::NumericalInstability(
                "multi-intersection edge flag set without edge parameters".into(),
            ));
        };
        let vi = classified[ei].1;
        let vj = classified[ej].1;
        let t = params[0];
        let a = workspace.nodes[vi];
        let b = workspace.nodes[vj];
        let split_pt = [
            a[0] + t * (b[0] - a[0]),
            a[1] + t * (b[1] - a[1]),
            a[2] + t * (b[2] - a[2]),
        ];
        let split_idx = workspace.add_node(split_pt);

        // Split tet: replace vertex at each end of the edge with split point
        let mut sub_a = tet;
        sub_a[ei] = split_idx;
        let mut sub_b = tet;
        sub_b[ej] = split_idx;

        let mut all_pos = Vec::new();
        let mut all_neg = Vec::new();

        for sub in [sub_a, sub_b] {
            if is_degenerate(&sub) {
                continue;
            }
            let (p, n) = clip_tet_surface_recursive(workspace, sub, surface, depth + 1)?;
            all_pos.extend(p);
            all_neg.extend(n);
        }

        return Ok((all_pos, all_neg));
    }

    // Simple case: all crossing edges have 0 or 1 intersections
    let (pos, neg) = if n_pos == 1 {
        clip_tet_surface_1_3(workspace, &classified, surface, Side::Positive)
    } else if n_neg == 1 {
        clip_tet_surface_1_3(workspace, &classified, surface, Side::Negative)
    } else {
        clip_tet_surface_2_2(workspace, &classified, surface)
    };

    let pos: Vec<_> = pos.into_iter().filter(|t| !is_degenerate(t)).collect();
    let neg: Vec<_> = neg.into_iter().filter(|t| !is_degenerate(t)).collect();
    Ok((pos, neg))
}

/// 1+3 split (ClipSurface version).
fn clip_tet_surface_1_3(
    workspace: &mut TetClipWorkspace,
    classified: &[(Side, usize); 4],
    surface: &dyn ClipSurface,
    isolated_side: Side,
) -> (Vec<[usize; 4]>, Vec<[usize; 4]>) {
    let iso_pos = classified
        .iter()
        .position(|(s, _)| *s == isolated_side)
        .unwrap();
    let v_iso = classified[iso_pos].1;
    let others: Vec<usize> = classified
        .iter()
        .enumerate()
        .filter(|&(i, _)| i != iso_pos)
        .map(|(_, (_, vi))| *vi)
        .collect();

    let cut_pts: [usize; 3] = std::array::from_fn(|j| {
        let other_pos = classified
            .iter()
            .position(|(_, vi)| *vi == others[j])
            .unwrap();
        if classified[other_pos].0 == Side::OnPlane {
            others[j]
        } else {
            let a = workspace.nodes[v_iso];
            let b = workspace.nodes[others[j]];
            let params = surface.intersect_edge_surface(a, b);
            if let Some(&t) = params.first() {
                let pt = [
                    a[0] + t * (b[0] - a[0]),
                    a[1] + t * (b[1] - a[1]),
                    a[2] + t * (b[2] - a[2]),
                ];
                workspace.add_node(pt)
            } else {
                // Fallback: midpoint (should not happen for valid crossing edges)
                let mid = vec3::scale(vec3::add(a, b), 0.5);
                workspace.add_node(mid)
            }
        }
    });

    let iso_tet = [v_iso, cut_pts[0], cut_pts[1], cut_pts[2]];
    let prism_tets = decompose_prism(cut_pts, [others[0], others[1], others[2]]);

    if isolated_side == Side::Positive {
        (vec![iso_tet], prism_tets)
    } else {
        (prism_tets, vec![iso_tet])
    }
}

/// 2+2 split (ClipSurface version).
fn clip_tet_surface_2_2(
    workspace: &mut TetClipWorkspace,
    classified: &[(Side, usize); 4],
    surface: &dyn ClipSurface,
) -> (Vec<[usize; 4]>, Vec<[usize; 4]>) {
    let mut pos_verts = Vec::new();
    let mut neg_verts = Vec::new();
    for &(side, vi) in classified {
        match side {
            Side::Positive | Side::OnPlane => pos_verts.push(vi),
            Side::Negative => neg_verts.push(vi),
        }
    }

    let compute_cut = |ws: &mut TetClipWorkspace, vi: usize, vj: usize| -> usize {
        let a = ws.nodes[vi];
        let b = ws.nodes[vj];
        let params = surface.intersect_edge_surface(a, b);
        if let Some(&t) = params.first() {
            let a = ws.nodes[vi];
            let b = ws.nodes[vj];
            let pt = [
                a[0] + t * (b[0] - a[0]),
                a[1] + t * (b[1] - a[1]),
                a[2] + t * (b[2] - a[2]),
            ];
            ws.add_node(pt)
        } else {
            let a = ws.nodes[vi];
            let b = ws.nodes[vj];
            let mid = vec3::scale(vec3::add(a, b), 0.5);
            ws.add_node(mid)
        }
    };

    let p00 = compute_cut(workspace, pos_verts[0], neg_verts[0]);
    let p01 = compute_cut(workspace, pos_verts[0], neg_verts[1]);
    let p10 = compute_cut(workspace, pos_verts[1], neg_verts[0]);
    let p11 = compute_cut(workspace, pos_verts[1], neg_verts[1]);

    let pos_tets = decompose_prism([pos_verts[0], p00, p01], [pos_verts[1], p10, p11]);
    let neg_tets = decompose_prism([neg_verts[0], p00, p10], [neg_verts[1], p01, p11]);

    (pos_tets, neg_tets)
}

/// Clip all tets by a surface, keeping the specified side.
pub fn clip_by_surface(
    workspace: &mut TetClipWorkspace,
    surface: &dyn ClipSurface,
    keep: Side,
) -> Result<(), TetClipError> {
    let old_tets = std::mem::take(&mut workspace.tets);
    for tet in old_tets {
        let (pos_tets, neg_tets) = clip_tet_surface(workspace, tet, surface)?;
        match keep {
            Side::Positive => workspace.tets.extend(pos_tets),
            Side::Negative => workspace.tets.extend(neg_tets),
            Side::OnPlane => {
                workspace.tets.extend(pos_tets);
                workspace.tets.extend(neg_tets);
            }
        }
    }
    Ok(())
}

/// Generic subtract: keep positive (outside) tets, pass negative to next surface.
pub fn clip_mesh_subtract_surfaces(
    workspace: &mut TetClipWorkspace,
    surfaces: &[Box<dyn ClipSurface>],
) -> Result<(), TetClipError> {
    let mut candidates = std::mem::take(&mut workspace.tets);
    let mut kept: Vec<[usize; 4]> = Vec::new();

    for surface in surfaces {
        let mut next_candidates = Vec::new();
        for tet in candidates {
            let (pos_tets, neg_tets) = clip_tet_surface(workspace, tet, surface.as_ref())?;
            kept.extend(pos_tets);
            next_candidates.extend(neg_tets);
        }
        candidates = next_candidates;
    }
    // Remaining candidates are inside operand -> discard

    workspace.tets = kept;
    Ok(())
}

/// Generic intersect: keep only negative (inside) tets.
pub fn clip_mesh_intersect_surfaces(
    workspace: &mut TetClipWorkspace,
    surfaces: &[Box<dyn ClipSurface>],
) -> Result<(), TetClipError> {
    for surface in surfaces {
        clip_by_surface(workspace, surface.as_ref(), Side::Negative)?;
    }
    Ok(())
}

/// Convert Surface enum to ClipSurface trait object.
pub fn surface_to_clip_surface(surface: &Surface) -> Option<Box<dyn ClipSurface>> {
    match surface {
        Surface::Plane { origin, normal } => {
            Some(Box::new(ClipPlane::from_origin_normal(*origin, *normal)))
        }
        Surface::Cylinder {
            origin,
            axis,
            radius,
        } => Some(Box::new(ClipCylinder::new(
            *origin,
            vec3::normalized(*axis),
            *radius,
        ))),
        Surface::Cone {
            origin,
            axis,
            half_angle,
        } => Some(Box::new(ClipCone::new(
            *origin,
            vec3::normalized(*axis),
            *half_angle,
        ))),
        Surface::Sphere { center, radius } => Some(Box::new(ClipSphere::new(*center, *radius))),
        Surface::Ellipsoid { center, rx, ry, rz } => {
            Some(Box::new(ClipEllipsoid::new(*center, *rx, *ry, *rz)))
        }
        Surface::Torus {
            center,
            axis,
            major_radius,
            minor_radius,
        } => Some(Box::new(ClipTorus::new(
            *center,
            *axis,
            *major_radius,
            *minor_radius,
        ))),
        _ => None,
    }
}

/// Generate summary string of surface types.
pub fn summarize_surfaces(operand: &Shell) -> String {
    let mut counts: std::collections::HashMap<&str, usize> = std::collections::HashMap::new();
    for face in &operand.faces {
        let name = match &face.surface {
            Surface::Plane { .. } => "Plane",
            Surface::Cylinder { .. } => "Cylinder",
            Surface::Cone { .. } => "Cone",
            Surface::Sphere { .. } => "Sphere",
            Surface::Ellipsoid { .. } => "Ellipsoid",
            Surface::Torus { .. } => "Torus",
            Surface::SurfaceOfRevolution { .. } => "Revolve",
            Surface::SurfaceOfSweep { .. } => "Sweep",
            Surface::NurbsSurface { .. } => "NURBS",
        };
        *counts.entry(name).or_insert(0) += 1;
    }
    let mut parts: Vec<String> = counts
        .into_iter()
        .map(|(name, count)| format!("{name}×{count}"))
        .collect();
    parts.sort();
    parts.join(", ")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn build_edge_to_tets_basic() {
        let tets = vec![[0, 1, 2, 3], [0, 1, 2, 4]];
        let map = build_edge_to_tets(&tets);
        // edge (0,1) is in both tets
        assert_eq!(map[&(0, 1)].len(), 2);
        // edge (0,2) is in both tets
        assert_eq!(map[&(0, 2)].len(), 2);
        // edge (2,3) is in tet 0 only
        assert_eq!(map[&(2, 3)].len(), 1);
        // edge (2,4) is in tet 1 only
        assert_eq!(map[&(2, 4)].len(), 1);
    }

    #[test]
    fn flip_3_2_basic() {
        // 3 tets sharing edge (0,1):
        //   [0,1,2,3], [0,1,3,4], [0,1,4,2]
        // opposite vertices are 2,3,4 -> can flip to [2,3,4,0] and [2,3,4,1]
        let nodes: Vec<[f64; 3]> = vec![
            [0.0, 0.0, 0.0],   // 0: shared
            [0.0, 0.0, 2.0],   // 1: shared
            [1.0, 0.0, 1.0],   // 2
            [-0.5, 1.0, 1.0],  // 3
            [-0.5, -1.0, 1.0], // 4
        ];
        let tets = vec![[0, 1, 2, 3], [0, 1, 3, 4], [0, 1, 4, 2]];
        let tet_indices = vec![0, 1, 2];

        // try_flip_3_2 should not panic
        let result = try_flip_3_2(&nodes, &tets, &tet_indices);
        // Returns Some only if quality improves
        // Verify volume conservation
        if let Some(new_tets) = result {
            assert_eq!(new_tets.len(), 2);
            let old_vol: f64 = tet_indices
                .iter()
                .map(|&i| tet_signed_volume(&nodes, &tets[i]).abs())
                .sum();
            let new_vol: f64 = new_tets
                .iter()
                .map(|t| tet_signed_volume(&nodes, t).abs())
                .sum();
            assert!(
                (old_vol - new_vol).abs() < 1e-12,
                "volume not conserved: old={old_vol}, new={new_vol}"
            );
        }
    }

    #[test]
    fn flip_3_2_wrong_count() {
        let nodes: Vec<[f64; 3]> = vec![
            [0.0, 0.0, 0.0],
            [1.0, 0.0, 0.0],
            [0.0, 1.0, 0.0],
            [0.0, 0.0, 1.0],
        ];
        let tets = vec![[0, 1, 2, 3]];
        // None if tet count != 3
        assert!(try_flip_3_2(&nodes, &tets, &[0]).is_none());
        assert!(try_flip_3_2(&nodes, &tets, &[]).is_none());
    }

    #[test]
    fn edge_to_tets_finds_ring() {
        // build_edge_to_tets should find 3 tets around edge (0,1)
        let tets = vec![[0, 1, 2, 3], [0, 1, 3, 4], [0, 1, 4, 2]];
        let map = build_edge_to_tets(&tets);
        let ring = &map[&(0, 1)];
        assert_eq!(ring.len(), 3);
        assert!(ring.contains(&0));
        assert!(ring.contains(&1));
        assert!(ring.contains(&2));
    }

    #[test]
    fn test_clip_plane_as_clip_surface() {
        let plane = ClipPlane::from_origin_normal([0.0, 0.0, 0.0], [0.0, 0.0, 1.0]);
        assert_eq!(plane.classify_surface([0.0, 0.0, 1.0]), Side::Positive);
        assert_eq!(plane.classify_surface([0.0, 0.0, -1.0]), Side::Negative);
        let ts = plane.intersect_edge_surface([0.0, 0.0, -1.0], [0.0, 0.0, 1.0]);
        assert_eq!(ts.len(), 1);
        assert!((ts[0] - 0.5).abs() < 1e-12);
    }

    #[test]
    fn test_clip_sphere_classify() {
        let sphere = ClipSphere::new([0.0, 0.0, 0.0], 1.0);
        assert_eq!(sphere.classify_surface([2.0, 0.0, 0.0]), Side::Positive);
        assert_eq!(sphere.classify_surface([0.5, 0.0, 0.0]), Side::Negative);
    }

    #[test]
    fn test_clip_sphere_intersect_edge() {
        let sphere = ClipSphere::new([0.0, 0.0, 0.0], 1.0);
        let ts = sphere.intersect_edge_surface([-2.0, 0.0, 0.0], [2.0, 0.0, 0.0]);
        assert_eq!(ts.len(), 2);
        assert!((ts[0] - 0.25).abs() < 1e-12);
        assert!((ts[1] - 0.75).abs() < 1e-12);
    }

    #[test]
    fn test_clip_cylinder_intersect_edge() {
        let cyl = ClipCylinder::new([0.0, 0.0, 0.0], [0.0, 0.0, 1.0], 1.0);
        let ts = cyl.intersect_edge_surface([-2.0, 0.0, 0.5], [2.0, 0.0, 0.5]);
        assert_eq!(ts.len(), 2);
    }

    #[test]
    fn test_clip_torus_signed_distance() {
        let torus = ClipTorus::new([0.0, 0.0, 0.0], [0.0, 0.0, 1.0], 2.0, 0.5);
        // Point on the tube center ring (2, 0, 0) → distance = -0.5 (inside)
        assert!((torus.signed_distance([2.0, 0.0, 0.0]) - (-0.5)).abs() < 1e-12);
        // Far point → positive
        assert!(torus.signed_distance([5.0, 0.0, 0.0]) > 0.0);
    }

    #[test]
    fn test_clip_torus_intersect_edge_through() {
        let torus = ClipTorus::new([0.0, 0.0, 0.0], [0.0, 0.0, 1.0], 2.0, 0.5);
        // X-axis edge through torus → should have 4 intersections
        // At x = -2.5, -1.5, 1.5, 2.5 → t = (x+3)/6
        let ts = torus.intersect_edge_surface([-3.0, 0.0, 0.0], [3.0, 0.0, 0.0]);
        assert_eq!(
            ts.len(),
            4,
            "expected 4 intersections, got {}: {:?}",
            ts.len(),
            ts
        );
    }

    #[test]
    fn test_clip_tet_with_sphere() {
        let sphere = ClipSphere::new([0.0, 0.0, 0.0], 1.0);
        let mut ws = TetClipWorkspace::new(
            vec![
                [0.0, 0.0, 0.0],
                [2.0, 0.0, 0.0],
                [0.0, 2.0, 0.0],
                [0.0, 0.0, 2.0],
            ],
            vec![[0, 1, 2, 3]],
        );
        let (pos, neg) = clip_tet_surface(&mut ws, [0, 1, 2, 3], &sphere).unwrap();
        assert!(!neg.is_empty(), "inside tets empty");
        assert!(!pos.is_empty(), "outside tets empty");
        // Volume conservation
        let vol_orig = tet_volume(&ws.nodes, &[0, 1, 2, 3]);
        let vol_pos: f64 = pos.iter().map(|t| tet_volume(&ws.nodes, t)).sum();
        let vol_neg: f64 = neg.iter().map(|t| tet_volume(&ws.nodes, t)).sum();
        assert!(
            (vol_pos + vol_neg - vol_orig).abs() / vol_orig < 0.01,
            "volume not conserved: orig={vol_orig}, pos={vol_pos}, neg={vol_neg}"
        );
    }

    #[test]
    fn test_clip_tet_surface_plane_matches_clip_tet() {
        // Use ClipPlane via clip_tet_surface to get same result as clip_tet
        let plane = ClipPlane::from_origin_normal([0.5, 0.0, 0.0], [1.0, 0.0, 0.0]);
        let mut ws = TetClipWorkspace::new(
            vec![
                [0.0, 0.0, 0.0],
                [1.0, 0.0, 0.0],
                [0.0, 1.0, 0.0],
                [0.0, 0.0, 1.0],
            ],
            vec![[0, 1, 2, 3]],
        );
        let (pos, neg) = clip_tet_surface(&mut ws, [0, 1, 2, 3], &plane).unwrap();
        assert!(!pos.is_empty());
        assert!(!neg.is_empty());
        let vol_orig = tet_volume(&ws.nodes, &[0, 1, 2, 3]);
        let vol_pos: f64 = pos.iter().map(|t| tet_volume(&ws.nodes, t)).sum();
        let vol_neg: f64 = neg.iter().map(|t| tet_volume(&ws.nodes, t)).sum();
        assert!(
            (vol_pos + vol_neg - vol_orig).abs() / vol_orig < 0.01,
            "Plane: volume not conserved"
        );
    }

    #[test]
    fn test_clip_by_surface_sphere() {
        let sphere = ClipSphere::new([0.0, 0.0, 0.0], 1.0);
        let mut ws = TetClipWorkspace::new(
            vec![
                [0.0, 0.0, 0.0],
                [2.0, 0.0, 0.0],
                [0.0, 2.0, 0.0],
                [0.0, 0.0, 2.0],
            ],
            vec![[0, 1, 2, 3]],
        );
        clip_by_surface(&mut ws, &sphere, Side::Negative).unwrap();
        assert!(!ws.tets.is_empty(), "clip_by_surface: inside tets empty");
    }

    #[test]
    fn test_summarize_surfaces() {
        use crate::brep::{Curve3D, Edge, EdgeRef, Face};
        let shell = Shell {
            vertices: vec![[0.0, 0.0, 0.0], [1.0, 0.0, 0.0]],
            edges: vec![Edge {
                v_start: 0,
                v_end: 1,
                curve: Curve3D::Line {
                    start: [0.0, 0.0, 0.0],
                    end: [1.0, 0.0, 0.0],
                },
            }],
            faces: vec![
                Face {
                    loop_edges: vec![EdgeRef {
                        edge_id: 0,
                        forward: true,
                    }],
                    surface: Surface::Plane {
                        origin: [0.0, 0.0, 0.0],
                        normal: [0.0, 0.0, 1.0],
                    },
                    orientation_reversed: false,
                },
                Face {
                    loop_edges: vec![EdgeRef {
                        edge_id: 0,
                        forward: true,
                    }],
                    surface: Surface::Plane {
                        origin: [0.0, 0.0, 1.0],
                        normal: [0.0, 0.0, -1.0],
                    },
                    orientation_reversed: false,
                },
                Face {
                    loop_edges: vec![EdgeRef {
                        edge_id: 0,
                        forward: true,
                    }],
                    surface: Surface::Sphere {
                        center: [0.0, 0.0, 0.0],
                        radius: 1.0,
                    },
                    orientation_reversed: false,
                },
            ],
        };
        let summary = summarize_surfaces(&shell);
        assert!(summary.contains("Plane×2"));
        assert!(summary.contains("Sphere×1"));
    }
}
