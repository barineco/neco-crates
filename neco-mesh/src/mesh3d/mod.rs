//! Quality tetrahedral meshing pipeline built from Delaunay insertion,
//! boundary recovery, refinement, and improvement.

pub mod improvement;
pub mod insertion;
pub mod plc;
pub mod quality;
pub mod recovery;
pub mod refinement;
pub mod tet_mesh;

use crate::point3::Point3;
use crate::types::TetMesh3D;

/// Generate a quality tetrahedral mesh from a closed triangle surface mesh.
///
/// Pipeline: Delaunay tetrahedralization, boundary recovery, refinement,
/// and quality improvement.
pub fn generate_quality_mesh(
    surface_nodes: &[[f64; 3]],
    surface_triangles: &[[usize; 3]],
    params: Option<refinement::RefinementParams>,
) -> Result<TetMesh3D, String> {
    if surface_nodes.is_empty() || surface_triangles.is_empty() {
        return Err("surface mesh must not be empty".to_string());
    }
    let surface_nodes: Vec<Point3> = surface_nodes.iter().copied().map(Into::into).collect();

    let mut plc = plc::PLC::from_surface_mesh(&surface_nodes, surface_triangles);

    // 2. Delaunay tetrahedralization
    let mut mesh = insertion::build_delaunay(&surface_nodes)?;

    // 3. Boundary recovery
    let edge_stats = recovery::recover_edges(&mut mesh, &plc, 3);
    if edge_stats.failed_edges > 0 {
        return Err(format!(
            "edge recovery failed: {} edges remain unrecovered after adding {} Steiner points",
            edge_stats.failed_edges, edge_stats.steiner_points
        ));
    }
    let _face_stats = recovery::recover_faces(&mesh, &plc);

    // 4. Refinement
    let refine_params = params.unwrap_or_default();
    let _refine_stats = refinement::refine_mesh(&mut mesh, &mut plc, &refine_params);

    // 5. Improvement
    let improve_params = improvement::ImprovementParams::default();
    let _improve_stats = improvement::improve_mesh(&mut mesh, &improve_params);

    mesh.compact_tombstones();
    let mesh = mesh.to_mesh3d();
    Ok(TetMesh3D {
        nodes: mesh.nodes.into_iter().map(Into::into).collect(),
        tetrahedra: mesh.tetrahedra,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn box_surface_mesh() -> (Vec<[f64; 3]>, Vec<[usize; 3]>) {
        let nodes = vec![
            [0.0, 0.0, 0.0],
            [1.0, 0.0, 0.0],
            [1.0, 1.0, 0.0],
            [0.0, 1.0, 0.0],
            [0.0, 0.0, 1.0],
            [1.0, 0.0, 1.0],
            [1.0, 1.0, 1.0],
            [0.0, 1.0, 1.0],
        ];
        let triangles = vec![
            // bottom (z=0)
            [0, 2, 1],
            [0, 3, 2],
            // top (z=1)
            [4, 5, 6],
            [4, 6, 7],
            // front (y=0)
            [0, 1, 5],
            [0, 5, 4],
            // back (y=1)
            [2, 3, 7],
            [2, 7, 6],
            // left (x=0)
            [0, 4, 7],
            [0, 7, 3],
            // right (x=1)
            [1, 2, 6],
            [1, 6, 5],
        ];

        (nodes, triangles)
    }

    #[test]
    fn generate_quality_mesh_box() {
        let (nodes, triangles) = box_surface_mesh();

        let result = generate_quality_mesh(&nodes, &triangles, None);
        assert!(
            result.is_ok(),
            "generate_quality_mesh failed: {:?}",
            result.err()
        );

        let mesh = result.unwrap();
        assert!(
            mesh.n_nodes() >= 8,
            "node count is too small: {}",
            mesh.n_nodes()
        );
        assert!(mesh.n_tetrahedra() > 0, "tetrahedron count is zero");

        let vol = mesh.total_volume();
        assert!((vol - 1.0).abs() < 0.1, "volume deviates from 1.0: {}", vol);
    }

    #[test]
    fn generate_quality_mesh_box_translation_invariant() {
        let (nodes, triangles) = box_surface_mesh();
        let translated_nodes: Vec<[f64; 3]> = nodes
            .iter()
            .map(|node| [node[0] + 12.5, node[1] - 7.25, node[2] + 3.0])
            .collect();

        let original = generate_quality_mesh(&nodes, &triangles, None)
            .expect("generate_quality_mesh failed on original box");
        let translated = generate_quality_mesh(&translated_nodes, &triangles, None)
            .expect("generate_quality_mesh failed on translated box");

        assert!(
            original.n_tetrahedra() > 0,
            "original tetrahedron count is zero"
        );
        assert!(
            translated.n_tetrahedra() > 0,
            "translated tetrahedron count is zero"
        );

        let original_volume = original.total_volume();
        let translated_volume = translated.total_volume();
        assert!(
            (original_volume - translated_volume).abs() < 1e-9,
            "volume changed after translation: original={}, translated={}",
            original_volume,
            translated_volume
        );
    }

    #[test]
    fn generate_quality_mesh_box_triangle_permutation_invariant() {
        let (nodes, triangles) = box_surface_mesh();
        let mut permuted_triangles = triangles.clone();
        permuted_triangles.reverse();

        let mesh = generate_quality_mesh(&nodes, &permuted_triangles, None)
            .expect("generate_quality_mesh failed on permuted triangle order");

        assert!(mesh.n_tetrahedra() > 0, "tetrahedron count is zero");

        let vol = mesh.total_volume();
        assert!((vol - 1.0).abs() < 0.1, "volume deviates from 1.0: {}", vol);

        let mut rotated_triangles = triangles.clone();
        rotated_triangles.rotate_left(3);
        let rotated_mesh = generate_quality_mesh(&nodes, &rotated_triangles, None)
            .expect("generate_quality_mesh failed on rotated triangle order");
        assert!(
            rotated_mesh.n_tetrahedra() > 0,
            "rotated tetrahedron count is zero"
        );
        let rotated_vol = rotated_mesh.total_volume();
        assert!(
            (rotated_vol - 1.0).abs() < 0.1,
            "rotated volume deviates from 1.0: {}",
            rotated_vol
        );
    }

    #[test]
    fn generate_quality_mesh_empty_input() {
        let result = generate_quality_mesh(&[], &[], None);
        assert!(result.is_err());
    }

    #[test]
    fn bench_quality_comparison() {
        use crate::internal_mesh3d::generate_box_mesh;

        fn print_stats(label: &str, mesh: &TetMesh3D) {
            let nodes: Vec<Point3> = mesh.nodes.iter().copied().map(Into::into).collect();
            let stats = quality::mesh_quality_stats(&nodes, &mesh.tetrahedra, 2.0);
            eprintln!("  [{label}]");
            eprintln!(
                "    nodes={}, tets={}, vol={:.4}",
                mesh.n_nodes(),
                mesh.n_tetrahedra(),
                mesh.total_volume()
            );
            eprintln!(
                "    radius-edge: min={:.3} max={:.3} mean={:.3}",
                stats.min_radius_edge, stats.max_radius_edge, stats.mean_radius_edge
            );
            eprintln!(
                "    dihedral: min={:.1}° max={:.1}°",
                stats.min_dihedral.to_degrees(),
                stats.max_dihedral.to_degrees()
            );
            eprintln!("    skinny(ρ>2): {}/{}", stats.num_slivers, stats.num_tets);
        }

        eprintln!("\n========== Mesh Quality Benchmark ==========");

        let old = generate_box_mesh(1.0, 1.0, 1.0, 0.3);
        let old = TetMesh3D {
            nodes: old.nodes.iter().copied().map(Into::into).collect(),
            tetrahedra: old.tetrahedra,
        };
        print_stats("old: Kuhn max_edge=0.3", &old);

        let old_fine = generate_box_mesh(1.0, 1.0, 1.0, 0.15);
        let old_fine = TetMesh3D {
            nodes: old_fine.nodes.iter().copied().map(Into::into).collect(),
            tetrahedra: old_fine.tetrahedra,
        };
        print_stats("old: Kuhn max_edge=0.15", &old_fine);

        let nodes = vec![
            [0.0, 0.0, 0.0],
            [1.0, 0.0, 0.0],
            [1.0, 1.0, 0.0],
            [0.0, 1.0, 0.0],
            [0.0, 0.0, 1.0],
            [1.0, 0.0, 1.0],
            [1.0, 1.0, 1.0],
            [0.0, 1.0, 1.0],
        ];
        let tris = vec![
            [0, 2, 1],
            [0, 3, 2],
            [4, 5, 6],
            [4, 6, 7],
            [0, 1, 5],
            [0, 5, 4],
            [2, 3, 7],
            [2, 7, 6],
            [0, 4, 7],
            [0, 7, 3],
            [1, 2, 6],
            [1, 6, 5],
        ];
        let params = refinement::RefinementParams {
            max_iterations: 500,
            max_steiner_points: 5000,
            ..Default::default()
        };
        let new_mesh = generate_quality_mesh(&nodes, &tris, Some(params)).unwrap();
        print_stats("new: Delaunay+refinement B=2", &new_mesh);

        let old_for_pts = generate_box_mesh(1.0, 1.0, 1.0, 0.3);
        let plc_from_old = plc::PLC::from_mesh3d(&old_for_pts);
        let plc_vertices: Vec<[f64; 3]> = plc_from_old
            .vertices
            .iter()
            .copied()
            .map(Into::into)
            .collect();
        let params2 = refinement::RefinementParams {
            max_iterations: 2000,
            max_steiner_points: 10000,
            min_dihedral_angle: 20.0_f64.to_radians(),
            ..Default::default()
        };
        let new_dense =
            generate_quality_mesh(&plc_vertices, &plc_from_old.polygons, Some(params2)).unwrap();
        print_stats("new: Delaunay+refinement (Kuhn-sampled input)", &new_dense);

        eprintln!("================================================\n");
    }
}
