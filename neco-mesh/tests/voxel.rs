use std::f64::consts::PI;

use neco_mesh::{
    solid_occupancy, surface_occupancy, FillFractionGrid, GeometryConfig, RodGeometry,
    SolidOccupancyError, SpatialVoxelGrid, TriangleGeometry,
};

#[test]
fn test_module_paths_expose_grid_and_wire_types() {
    let grid = neco_mesh::voxel::grid::VoxelGrid {
        nx: 1,
        ny: 1,
        nz: 1,
        values: vec![0.25_f64],
    };
    assert!((grid.get(0, 0, 0) - 0.25).abs() < 1e-15);

    let rod = neco_mesh::voxel::wire::GeometryConfig::Rod {
        diameter: 0.01,
        centerline: vec![[0.0, 0.0, 0.0], [0.1, 0.0, 0.0]],
    }
    .to_rod_geometry();
    assert!((rod.diameter - 0.01).abs() < 1e-15);

    let occupancy = neco_mesh::voxel::grid::OccupancyGrid {
        nx: 1,
        ny: 1,
        nz: 1,
        values: vec![true],
    };
    assert!(occupancy.get(0, 0, 0));
}

fn total_volume(fill: &FillFractionGrid, dx: f64) -> f64 {
    fill.values.iter().sum::<f64>() * dx * dx * dx
}

fn unit_cube_surface() -> (Vec<[f64; 3]>, Vec<[usize; 3]>) {
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
    (nodes, triangles)
}

#[test]
fn test_surface_occupancy_generates_nonempty_binary_grid() {
    let (nodes, triangles) = unit_cube_surface();
    let occupancy = surface_occupancy(&nodes, &triangles, 0.5);

    assert!(occupancy.grid.values.iter().any(|&value| value));
    assert!(occupancy.grid.values.iter().any(|&value| !value));
}

#[test]
fn test_surface_occupancy_module_path_matches_top_level() {
    let (nodes, triangles) = unit_cube_surface();
    let top_level = surface_occupancy(&nodes, &triangles, 0.5);
    let module_level = neco_mesh::voxel::surface::surface_occupancy(&nodes, &triangles, 0.5);

    assert_eq!(top_level, module_level);
}

#[test]
fn test_solid_occupancy_module_path_matches_top_level() {
    let (nodes, triangles) = unit_cube_surface();
    let top_level = solid_occupancy(&nodes, &triangles, 0.5).expect("closed cube should pass");
    let module_level = neco_mesh::voxel::solid::solid_occupancy(&nodes, &triangles, 0.5)
        .expect("closed cube should pass");

    assert_eq!(top_level, module_level);
}

#[test]
fn test_solid_occupancy_matches_surface_layout_and_values() {
    let (nodes, triangles) = unit_cube_surface();
    let surface = surface_occupancy(&nodes, &triangles, 0.5);
    let solid = solid_occupancy(&nodes, &triangles, 0.5).expect("closed cube should pass");

    assert_eq!(solid, surface);
}

#[test]
fn test_solid_occupancy_rejects_open_surface_boundary_edge() {
    let nodes = vec![[0.0, 0.0, 0.0], [1.0, 0.0, 0.0], [0.0, 1.0, 0.0]];
    let triangles = vec![[0, 1, 2]];

    let error = solid_occupancy(&nodes, &triangles, 0.5).expect_err("open surface must fail");
    assert!(matches!(error, SolidOccupancyError::BoundaryEdge { .. }));
}

#[test]
fn test_solid_occupancy_rejects_empty_surface() {
    let error = solid_occupancy(&[], &[], 0.5).expect_err("empty surface must fail");
    assert_eq!(error, SolidOccupancyError::EmptySurface);
}

#[test]
fn test_solid_occupancy_rejects_out_of_bounds_index() {
    let nodes = vec![[0.0, 0.0, 0.0], [1.0, 0.0, 0.0], [0.0, 1.0, 0.0]];
    let triangles = vec![[0, 1, 3]];

    let error =
        solid_occupancy(&nodes, &triangles, 0.5).expect_err("invalid triangle index must fail");
    assert!(matches!(
        error,
        SolidOccupancyError::TriangleIndexOutOfBounds {
            triangle: 0,
            vertex: 3,
            node_count: 3,
        }
    ));
}

#[test]
fn test_solid_occupancy_rejects_non_manifold_edge() {
    let nodes = vec![
        [0.0, 0.0, 0.0],
        [1.0, 0.0, 0.0],
        [0.0, 1.0, 0.0],
        [0.0, 0.0, 1.0],
        [0.0, -1.0, 0.0],
    ];
    let triangles = vec![[0, 1, 2], [1, 0, 3], [0, 1, 4]];

    let error = solid_occupancy(&nodes, &triangles, 0.5).expect_err("non-manifold edge must fail");
    assert!(matches!(
        error,
        SolidOccupancyError::NonManifoldEdge {
            edge: [0, 1],
            count: 3,
        }
    ));
}

#[test]
fn test_surface_occupancy_marks_cube_center_inside_and_corner_outside() {
    let (nodes, triangles) = unit_cube_surface();
    let occupancy: SpatialVoxelGrid<bool> = surface_occupancy(&nodes, &triangles, 0.5);

    assert!(occupancy.grid.get(2, 2, 2));
    assert!(!occupancy.grid.get(0, 0, 0));
}

#[test]
fn test_surface_occupancy_exposes_layout_metadata() {
    let (nodes, triangles) = unit_cube_surface();
    let occupancy = surface_occupancy(&nodes, &triangles, 0.5);

    assert_eq!(occupancy.layout.origin, [-0.5, -0.5, -0.5]);
    assert_eq!(occupancy.layout.spacing, [0.5, 0.5, 0.5]);
    assert_eq!(occupancy.layout.point(2, 2, 2), [0.5, 0.5, 0.5]);
    assert_eq!(occupancy.grid.nx, occupancy.layout.nx);
    assert_eq!(occupancy.grid.ny, occupancy.layout.ny);
    assert_eq!(occupancy.grid.nz, occupancy.layout.nz);
}

#[test]
fn test_straight_rod_volume() {
    let rod = RodGeometry {
        diameter: 0.008,
        centerline: vec![[-0.04, 0.0, 0.0], [0.04, 0.0, 0.0]],
    };
    let dx = 0.002;
    let n = 60;
    let cx = n as f64 * dx / 2.0;
    let fill = rod.fill_fraction(dx, n, n, n, cx, cx, cx);

    let vol_numerical: f64 = fill.values.iter().sum::<f64>() * dx * dx * dx;
    let vol_theory = PI * 0.004_f64.powi(2) * 0.08;
    let rel_err = (vol_numerical - vol_theory).abs() / vol_theory;
    assert!(
        rel_err < 0.10,
        "fill fraction volume error: {rel_err:.3} (numerical={vol_numerical:.6e}, theory={vol_theory:.6e})"
    );
}

#[test]
fn test_fill_fraction_bounded() {
    let rod = RodGeometry {
        diameter: 0.008,
        centerline: vec![[0.0, 0.0, -0.02], [0.0, 0.0, 0.02]],
    };
    let dx = 0.004;
    let n = 20;
    let cx = n as f64 * dx / 2.0;
    let fill = rod.fill_fraction(dx, n, n, n, cx, cx, cx);

    assert!(fill.values.iter().all(|&f| (0.0..=1.0).contains(&f)));
    assert!((fill.get(0, 0, 0) - 0.0).abs() < 1e-15);
}

#[test]
fn test_triangle_to_rod_geometry() {
    let tri = TriangleGeometry {
        side_length: 0.20,
        rod_diameter: 0.008,
        corner_radius: 0.010,
        gap_width: 0.02,
    };
    let rod = tri.to_rod_geometry();

    assert!((rod.diameter - 0.008).abs() < 1e-15);
    assert!(
        rod.centerline.len() >= 10,
        "should have enough control points"
    );

    let start = rod.centerline.first().expect("start point");
    let end = rod.centerline.last().expect("end point");
    let cy = 0.20 * 3.0_f64.sqrt() / 2.0 / 3.0;
    assert!(
        (start[1] - (-cy)).abs() < 1e-10,
        "start should be on bottom edge"
    );
    assert!(
        (end[1] - (-cy)).abs() < 1e-10,
        "end should be on bottom edge"
    );

    for p in &rod.centerline {
        assert!(
            (p[2] - 0.0).abs() < 1e-12,
            "all points should be in z=0 plane, got z={}",
            p[2]
        );
    }
}

#[test]
fn test_triangle_gap() {
    let tri = TriangleGeometry {
        side_length: 0.20,
        rod_diameter: 0.008,
        corner_radius: 0.010,
        gap_width: 0.03,
    };
    let rod = tri.to_rod_geometry();

    let start = rod.centerline.first().expect("start point");
    let end = rod.centerline.last().expect("end point");

    assert!((start[0] - 0.015).abs() < 1e-10, "gap right at x=+half_gap");
    assert!((end[0] - (-0.015)).abs() < 1e-10, "gap left at x=-half_gap");
}

#[test]
fn test_geometry_config_triangle() {
    let cfg = GeometryConfig::Triangle {
        side_length: 0.20,
        rod_diameter: 0.008,
        corner_radius: 0.010,
        gap_width: 0.0,
    };
    let rod = cfg.to_rod_geometry();
    assert!((rod.diameter - 0.008).abs() < 1e-15);
    assert!(rod.centerline.len() > 5);
}

#[test]
fn test_geometry_config_rod() {
    let cfg = GeometryConfig::Rod {
        diameter: 0.01,
        centerline: vec![[0.0, 0.0, 0.0], [0.1, 0.0, 0.0]],
    };
    let rod = cfg.to_rod_geometry();
    assert!((rod.diameter - 0.01).abs() < 1e-15);
    assert_eq!(rod.centerline.len(), 2);
}

#[test]
fn test_fill_fraction_is_invariant_to_centerline_reversal() {
    let rod = RodGeometry {
        diameter: 0.008,
        centerline: vec![[-0.03, -0.01, 0.0], [0.0, 0.02, 0.0], [0.03, -0.01, 0.0]],
    };
    let reversed = RodGeometry {
        diameter: rod.diameter,
        centerline: rod.centerline.iter().rev().copied().collect(),
    };
    let dx = 0.002;
    let n = 48;
    let c = n as f64 * dx / 2.0;
    let fill = rod.fill_fraction(dx, n, n, n, c, c, c);
    let fill_reversed = reversed.fill_fraction(dx, n, n, n, c, c, c);

    for (index, (a, b)) in fill
        .values
        .iter()
        .zip(fill_reversed.values.iter())
        .enumerate()
    {
        assert!((a - b).abs() < 1e-12, "voxel {index}: a={a}, b={b}");
    }
}

#[test]
fn test_zero_length_segment_does_not_change_total_volume() {
    let base = RodGeometry {
        diameter: 0.008,
        centerline: vec![[-0.04, 0.0, 0.0], [0.0, 0.02, 0.0], [0.04, 0.0, 0.0]],
    };
    let with_zero = RodGeometry {
        diameter: base.diameter,
        centerline: vec![
            [-0.04, 0.0, 0.0],
            [0.0, 0.02, 0.0],
            [0.0, 0.02, 0.0],
            [0.04, 0.0, 0.0],
        ],
    };
    let dx = 0.002;
    let n = 60;
    let c = n as f64 * dx / 2.0;
    let base_fill = base.fill_fraction(dx, n, n, n, c, c, c);
    let zero_fill = with_zero.fill_fraction(dx, n, n, n, c, c, c);

    let base_volume = total_volume(&base_fill, dx);
    let zero_volume = total_volume(&zero_fill, dx);
    let rel_err = (base_volume - zero_volume).abs() / base_volume.max(1e-12);
    assert!(
        rel_err < 1e-12,
        "base={base_volume}, zero={zero_volume}, rel_err={rel_err}"
    );
}

#[test]
fn test_parallel_translation_preserves_total_fill_volume() {
    let dx = 0.002;
    let n = 64;
    let c = n as f64 * dx / 2.0;
    let base = RodGeometry {
        diameter: 0.008,
        centerline: vec![[-0.03, 0.0, 0.0], [0.03, 0.0, 0.0]],
    };
    let translated = RodGeometry {
        diameter: base.diameter,
        centerline: vec![[-0.02, 0.01, 0.0], [0.04, 0.01, 0.0]],
    };

    let base_fill = base.fill_fraction(dx, n, n, n, c, c, c);
    let translated_fill = translated.fill_fraction(dx, n, n, n, c, c, c);
    let base_volume = total_volume(&base_fill, dx);
    let translated_volume = total_volume(&translated_fill, dx);
    let rel_err = (base_volume - translated_volume).abs() / base_volume.max(1e-12);
    assert!(
        rel_err < 1e-3,
        "base={base_volume}, translated={translated_volume}, rel_err={rel_err}"
    );
}

#[test]
fn test_multisegment_fill_fraction_remains_bounded() {
    let rod = RodGeometry {
        diameter: 0.008,
        centerline: vec![
            [-0.03, -0.01, 0.0],
            [0.0, 0.02, 0.0],
            [0.03, -0.01, 0.0],
            [0.04, 0.01, 0.0],
        ],
    };
    let dx = 0.002;
    let n = 48;
    let c = n as f64 * dx / 2.0;
    let fill = rod.fill_fraction(dx, n, n, n, c, c, c);
    assert!(fill.values.iter().all(|&f| (0.0..=1.0).contains(&f)));
}

#[test]
fn test_far_translated_centerline_produces_empty_grid() {
    let rod = RodGeometry {
        diameter: 0.008,
        centerline: vec![[1.0e12, 0.0, 0.0], [1.0e12 + 0.02, 0.0, 0.0]],
    };
    let dx = 0.002;
    let n = 32;
    let c = n as f64 * dx / 2.0;
    let fill = rod.fill_fraction(dx, n, n, n, c, c, c);

    assert!(
        fill.values.iter().all(|&f| f.abs() < 1e-15),
        "far translated centerline should not contribute inside the grid"
    );
}
