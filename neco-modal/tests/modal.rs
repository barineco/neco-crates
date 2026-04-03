use neco_modal::{extract_modes, extract_spatial_modes};

#[test]
fn extract_modes_detects_a_single_tone() {
    let dt = 1.0 / 8_000.0;
    let freq = 440.0;
    let samples = 4096;
    let readout: Vec<f64> = (0..samples)
        .map(|n| (2.0 * std::f64::consts::PI * freq * n as f64 * dt).sin())
        .collect();

    let modes = extract_modes(&readout, dt, 4, -40.0);
    assert!(!modes.is_empty());
    assert!((modes[0].freq - freq).abs() < 2.0, "freq={}", modes[0].freq);
}

#[test]
fn extract_modes_returns_empty_for_zero_signal() {
    let readout = vec![0.0; 1024];
    let modes = extract_modes(&readout, 1.0 / 44_100.0, 4, -40.0);
    assert!(modes.is_empty());
}

#[test]
fn extract_modes_returns_empty_for_non_positive_dt() {
    let readout = vec![1.0, 0.0, -1.0, 0.0, 1.0, 0.0, -1.0, 0.0];
    assert!(extract_modes(&readout, 0.0, 4, -40.0).is_empty());
    assert!(extract_modes(&readout, -0.01, 4, -40.0).is_empty());
}

#[test]
fn extract_spatial_modes_returns_one_shape_per_frequency() {
    let nx = 4;
    let ny = 3;
    let snapshots = [
        vec![0.0, 1.0, 0.5, 0.0, 0.2, 0.4, 0.0, 0.1, 0.3, 0.0, 0.2, 0.1],
        vec![0.2, 0.8, 0.7, 0.1, 0.3, 0.3, 0.1, 0.2, 0.2, 0.0, 0.1, 0.2],
        vec![0.4, 0.5, 0.8, 0.2, 0.4, 0.1, 0.2, 0.3, 0.1, 0.1, 0.0, 0.3],
    ];
    let refs: Vec<&[f64]> = snapshots.iter().map(Vec::as_slice).collect();
    let times = [0.0, 0.01, 0.02];

    let modes = extract_spatial_modes(&refs, &times, &[120.0, 240.0], nx, ny);
    assert_eq!(modes.len(), 2);
    assert!(modes.iter().all(|mode| mode.shape.len() == nx * ny));
}

#[test]
fn extract_spatial_modes_returns_empty_for_invalid_sampling_or_grid() {
    let snapshots = [vec![1.0, 0.0, 0.0, 1.0], vec![0.5, 0.0, 0.0, 0.5]];
    let refs: Vec<&[f64]> = snapshots.iter().map(Vec::as_slice).collect();

    assert!(extract_spatial_modes(&refs, &[0.0], &[100.0], 2, 2).is_empty());
    assert!(extract_spatial_modes(&refs, &[0.0, 0.0], &[100.0], 2, 2).is_empty());
    assert!(extract_spatial_modes(&refs, &[0.0, 0.1], &[100.0], 0, 2).is_empty());
    assert!(extract_spatial_modes(&refs, &[0.0, 0.1], &[100.0], 2, 0).is_empty());
}
