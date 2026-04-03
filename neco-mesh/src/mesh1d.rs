//! 1D edge meshing for line segments and NURBS curves.

use neco_nurbs::NurbsCurve2D;

use crate::types::EdgeMesh;

/// Mesh a straight line segment into uniform edges.
pub fn mesh_line(origin: [f64; 2], length: f64, direction: [f64; 2], max_edge: f64) -> EdgeMesh {
    let n_segments = (length / max_edge).ceil().max(1.0) as usize;
    let n_nodes = n_segments + 1;
    let seg_len = length / n_segments as f64;

    let nodes: Vec<[f64; 2]> = (0..n_nodes)
        .map(|i| {
            let t = seg_len * i as f64;
            [origin[0] + direction[0] * t, origin[1] + direction[1] * t]
        })
        .collect();

    let edges: Vec<[usize; 2]> = (0..n_segments).map(|i| [i, i + 1]).collect();
    let lengths = vec![seg_len; n_segments];
    let arc_params: Vec<f64> = (0..n_nodes).map(|i| seg_len * i as f64).collect();

    EdgeMesh {
        nodes,
        edges,
        lengths,
        arc_params,
    }
}

/// Mesh a NURBS curve into adaptive edges.
pub fn mesh_curve(curve: &NurbsCurve2D, max_edge: f64) -> EdgeMesh {
    let nodes = curve.adaptive_sample(max_edge);
    let n = nodes.len();
    if n < 2 {
        return EdgeMesh {
            nodes,
            edges: vec![],
            lengths: vec![],
            arc_params: vec![0.0],
        };
    }

    let mut arc_params = vec![0.0_f64; n];
    let mut lengths = Vec::with_capacity(n - 1);
    let edges: Vec<[usize; 2]> = (0..n - 1).map(|i| [i, i + 1]).collect();
    for i in 1..n {
        let dx = nodes[i][0] - nodes[i - 1][0];
        let dy = nodes[i][1] - nodes[i - 1][1];
        let d = (dx * dx + dy * dy).sqrt();
        lengths.push(d);
        arc_params[i] = arc_params[i - 1] + d;
    }

    EdgeMesh {
        nodes,
        edges,
        lengths,
        arc_params,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mesh_line_node_count() {
        let mesh = mesh_line([0.0, 0.0], 1.0, [1.0, 0.0], 0.1);
        assert_eq!(mesh.nodes.len(), 11);
        assert_eq!(mesh.edges.len(), 10);
    }

    #[test]
    fn mesh_line_arc_params() {
        let mesh = mesh_line([0.0, 0.0], 0.5, [1.0, 0.0], 0.1);
        assert!((mesh.arc_params[0] - 0.0).abs() < 1e-12);
        let last = *mesh.arc_params.last().unwrap();
        assert!(
            (last - 0.5).abs() < 1e-12,
            "last arc length={last}, expected=0.5"
        );
    }

    #[test]
    fn mesh_line_uniform_lengths() {
        let mesh = mesh_line([0.0, 0.0], 1.0, [1.0, 0.0], 0.25);
        assert_eq!(mesh.lengths.len(), 4);
        for &length in &mesh.lengths {
            assert!((length - 0.25).abs() < 1e-12);
        }
    }
}
