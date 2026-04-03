use neco_gridfield::{Array2, BoundaryGeometry, FieldSet, Grid2D, GridError};

#[test]
fn grid_dimensions_match_spacing() {
    let grid = Grid2D::new(0.1, 0.1, 0.01).expect("valid grid must construct");
    assert_eq!(grid.nx(), 11);
    assert_eq!(grid.ny(), 11);
}

#[test]
fn coords_are_centered() {
    let grid = Grid2D::new(0.1, 0.1, 0.01).expect("valid grid must construct");
    let (x, y) = grid.coords();
    let center = grid.nx() / 2;
    assert!(x[[center, 0]].abs() < 1e-10);
    assert!(y[[0, center]].abs() < 1e-10);
}

#[test]
fn radius_map_is_symmetric_about_grid_center() {
    let grid = Grid2D::new(0.1, 0.1, 0.01).expect("valid grid must construct");
    let r = grid.radius_map();
    let last_x = grid.nx() - 1;
    let last_y = grid.ny() - 1;
    assert!((r[[0, 0]] - r[[last_x, last_y]]).abs() < 1e-12);
    assert!((r[[0, last_y]] - r[[last_x, 0]]).abs() < 1e-12);
}

#[test]
fn circular_mask_excludes_hole_and_outside() {
    let grid = Grid2D::new(0.1, 0.1, 0.01).expect("valid grid must construct");
    let geometry = BoundaryGeometry::Circular {
        r_outer: 0.045,
        r_hole: 0.005,
    };
    let mask = grid
        .interior_mask(&geometry)
        .expect("circular mask must match grid");
    let cx = grid.nx() / 2;
    let cy = grid.ny() / 2;
    assert!(!mask[[cx, cy]]);
    assert!(mask[[cx + 2, cy]]);
    assert!(!mask[[0, 0]]);
}

#[test]
fn rectangular_mask_on_tiny_grid_disables_every_cell() {
    let grid = Grid2D::new(0.02, 0.02, 0.01).expect("valid grid must construct");
    let mask = grid
        .interior_mask(&BoundaryGeometry::Rectangular)
        .expect("rectangular mask must match grid");
    assert!(mask.iter().all(|active| !*active));
}

#[test]
fn rectangular_mask_disables_two_cell_border() {
    let grid = Grid2D::new(0.1, 0.1, 0.01).expect("valid grid must construct");
    let mask = grid
        .interior_mask(&BoundaryGeometry::Rectangular)
        .expect("rectangular mask must match grid");
    assert!(!mask[[0, 0]]);
    assert!(!mask[[1, 1]]);
    assert!(mask[[3, 3]]);
}

#[test]
fn explicit_mask_is_forwarded() {
    let grid = Grid2D::new(0.02, 0.02, 0.01).expect("valid grid must construct");
    let explicit = Array2::from_shape_vec(
        (3, 3),
        vec![true, false, true, false, true, false, true, false, true],
    )
    .expect("explicit test mask shape must be valid");
    let mask = grid
        .interior_mask(&BoundaryGeometry::Mask(explicit.clone()))
        .expect("matching explicit mask must be accepted");
    assert_eq!(mask, explicit);
}

#[test]
fn grid_rejects_invalid_geometry_inputs() {
    assert_eq!(
        Grid2D::new(0.1, 0.1, 0.0).expect_err("zero spacing must be rejected"),
        GridError::InvalidSpacing { dx: 0.0 }
    );
    assert_eq!(
        Grid2D::new(f64::NAN, 0.1, 0.01).expect_err("non-finite extent must be rejected"),
        GridError::InvalidExtent {
            axis: "lx",
            value: f64::NAN,
        }
    );
    assert_eq!(
        Grid2D::new(-0.1, 0.1, 0.01).expect_err("negative extent must be rejected"),
        GridError::InvalidExtent {
            axis: "lx",
            value: -0.1,
        }
    );
}

#[test]
fn explicit_mask_must_match_grid_shape() {
    let grid = Grid2D::new(0.02, 0.02, 0.01).expect("valid grid must construct");
    let wrong = Array2::from_elem((2, 2), true);
    let error = grid
        .interior_mask(&BoundaryGeometry::Mask(wrong))
        .expect_err("mismatched mask shape must be rejected");
    assert_eq!(
        error,
        GridError::InvalidMaskShape {
            expected: (3, 3),
            actual: (2, 2),
        }
    );
}

#[test]
fn field_advance_rotates_next_into_current() {
    let mut fields = FieldSet::new(5, 5);
    {
        let buffers = fields.split_bufs();
        buffers.w_next.fill(1.0);
    }
    fields.advance();
    assert_eq!(fields.w()[[2, 2]], 1.0);

    {
        let buffers = fields.split_bufs();
        buffers.w_next.fill(0.5);
    }
    fields.advance();
    assert_eq!(fields.w()[[2, 2]], 0.5);
    assert_eq!(fields.w_prev()[[2, 2]], 1.0);
}

#[test]
fn checkpoint_roundtrip_restores_generation_and_data() {
    let mut fields = FieldSet::new(4, 3);
    fields.w_mut()[[1, 1]] = 2.0;
    fields.u_mut()[[2, 1]] = -1.5;
    fields.v_mut()[[0, 2]] = 0.25;
    {
        let buffers = fields.split_bufs();
        buffers.w_next[[3, 2]] = 9.0;
    }
    fields.advance();
    let checkpoint = fields.to_checkpoint();

    let mut restored = FieldSet::new(1, 1);
    restored
        .restore_checkpoint(&checkpoint)
        .expect("checkpoint roundtrip must restore");

    assert_eq!(restored.w().shape(), [4, 3]);
    assert_eq!(restored.w()[[3, 2]], 9.0);
    assert_eq!(restored.w_prev()[[1, 1]], 2.0);
    assert_eq!(restored.u_prev()[[2, 1]], -1.5);
    assert_eq!(restored.v_prev()[[0, 2]], 0.25);
}

#[test]
fn restore_checkpoint_rejects_shape_mismatch() {
    let mut restored = FieldSet::new(1, 1);
    let checkpoint = neco_gridfield::FieldSetCheckpoint {
        w: [vec![0.0; 3], vec![0.0; 4], vec![0.0; 4]],
        u: [vec![0.0; 4], vec![0.0; 4], vec![0.0; 4]],
        v: [vec![0.0; 4], vec![0.0; 4], vec![0.0; 4]],
        generation: 0,
        shape: (2, 2),
    };
    let error = restored
        .restore_checkpoint(&checkpoint)
        .expect_err("mismatched checkpoint must be rejected");
    assert_eq!(
        error.to_string(),
        "checkpoint w buffer must match its declared shape"
    );
}
