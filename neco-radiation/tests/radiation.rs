use neco_radiation::{ModalRadiationCalculator, RadiationCalculator, RadiationParams};

fn centered_point(i: usize, j: usize, nx: usize, ny: usize, dx: f64) -> [f64; 2] {
    let cx = (nx / 2) as f64 * dx;
    let cy = (ny / 2) as f64 * dx;
    [i as f64 * dx - cx, j as f64 * dx - cy]
}

fn circular_active_cells(nx: usize, ny: usize, dx: f64, r_outer: f64, r_hole: f64) -> Vec<usize> {
    let mut active = Vec::new();
    for i in 0..nx {
        for j in 0..ny {
            let p = centered_point(i, j, nx, ny, dx);
            let r = (p[0] * p[0] + p[1] * p[1]).sqrt();
            if r < r_outer && r > r_hole {
                active.push(i * ny + j);
            }
        }
    }
    active
}

fn rectangular_active_cells(nx: usize, ny: usize) -> Vec<(usize, usize)> {
    let mut active = Vec::new();
    for i in 2..nx - 2 {
        for j in 2..ny - 2 {
            active.push((i, j));
        }
    }
    active
}

fn assert_close(actual: f64, expected: f64, rel_tol: f64) {
    let scale = expected.abs().max(1.0);
    let err = (actual - expected).abs() / scale;
    assert!(
        err < rel_tol,
        "actual={actual}, expected={expected}, rel_err={err}, rel_tol={rel_tol}"
    );
}

#[test]
fn test_piston_mode_radiation() {
    let calc = RadiationCalculator::new();
    let dx = 0.005_f64;
    let nx = (0.1 / dx).round() as usize + 1;
    let ny = nx;
    let active = circular_active_cells(nx, ny, dx, 0.04, 0.0);
    let points: Vec<[f64; 2]> = active
        .iter()
        .map(|&idx| centered_point(idx / ny, idx % ny, nx, ny, dx))
        .collect();
    let values = vec![1.0; points.len()];

    let power = calc.radiated_power(&points, &values, dx * dx, 1000.0);
    assert!(power > 0.0);
}

#[test]
fn test_higher_mode_radiates_less() {
    let calc = RadiationCalculator::new();
    let dx = 0.005_f64;
    let nx = (0.1 / dx).round() as usize + 1;
    let ny = nx;
    let active = circular_active_cells(nx, ny, dx, 0.04, 0.0);

    let mut points = Vec::new();
    let mut piston = Vec::new();
    let mut dipole = Vec::new();
    for idx in active {
        let i = idx / ny;
        let j = idx % ny;
        let p = centered_point(i, j, nx, ny, dx);
        points.push(p);
        piston.push(1.0);
        let r = (p[0] * p[0] + p[1] * p[1]).sqrt();
        dipole.push(if r > 1e-10 { p[0] / r } else { 0.0 });
    }

    let power_piston = calc.radiated_power(&points, &piston, dx * dx, 500.0);
    let power_dipole = calc.radiated_power(&points, &dipole, dx * dx, 500.0);
    assert!(power_dipole < power_piston);
}

#[test]
fn test_modal_efficiency() {
    let calc = RadiationCalculator::new();
    assert!(calc.modal_efficiency(0.05, 0, 100.0) < 0.1);
    assert!((calc.modal_efficiency(0.05, 0, 10000.0) - 1.0).abs() < 0.01);
    assert!(calc.modal_efficiency(0.05, 2, 5000.0) < calc.modal_efficiency(0.05, 0, 5000.0));
}

#[test]
fn test_modal_single_mode_matches_direct() {
    let dx = 0.005_f64;
    let nx = (0.1 / dx).round() as usize + 1;
    let ny = (0.08 / dx).round() as usize + 1;
    let active = rectangular_active_cells(nx, ny);
    let points: Vec<[f64; 2]> = active
        .iter()
        .map(|&(i, j)| centered_point(i, j, nx, ny, dx))
        .collect();

    let lx_eff = (nx as f64 - 3.0) * dx;
    let ly_eff = (ny as f64 - 3.0) * dx;
    let e = 70e9_f64;
    let nu = 0.33;
    let rho = 2700.0;
    let h = 0.005;
    let d = e * h * h * h / (12.0 * (1.0 - nu * nu));
    let rho_h = rho * h;
    let pi = std::f64::consts::PI;
    let kx = pi / lx_eff;
    let ky = pi / ly_eff;
    let omega_11 = (d / rho_h).sqrt() * (kx * kx + ky * ky);
    let f_11 = omega_11 / (2.0 * pi);

    let values: Vec<f64> = active
        .iter()
        .map(|&(i, j)| {
            (pi * (i as f64 - 1.0) / (nx as f64 - 3.0)).sin()
                * (pi * (j as f64 - 1.0) / (ny as f64 - 3.0)).sin()
        })
        .collect();

    let params = RadiationParams {
        rho_air: 1.225,
        c_air: 343.0,
        max_modes: 64,
    };
    let modal = ModalRadiationCalculator::new(&params, nx, ny, dx, &active, d, rho_h);
    let p_modal = modal.radiated_power(&values);

    let calc = RadiationCalculator::with_params(1.225, 343.0);
    let p_direct = calc.radiated_power(&points, &values, dx * dx, f_11);
    let ratio = p_modal / p_direct;
    assert!((ratio - 1.0).abs() < 0.05);
}

#[test]
fn test_modal_multimode_higher_than_single_freq() {
    let dx = 0.005_f64;
    let nx = (0.1 / dx).round() as usize + 1;
    let ny = (0.08 / dx).round() as usize + 1;
    let active = rectangular_active_cells(nx, ny);
    let points: Vec<[f64; 2]> = active
        .iter()
        .map(|&(i, j)| centered_point(i, j, nx, ny, dx))
        .collect();

    let lx_eff = (nx as f64 - 3.0) * dx;
    let ly_eff = (ny as f64 - 3.0) * dx;
    let e = 70e9_f64;
    let nu = 0.33;
    let rho = 2700.0;
    let h = 0.005;
    let d = e * h * h * h / (12.0 * (1.0 - nu * nu));
    let rho_h = rho * h;
    let pi = std::f64::consts::PI;
    let kx1 = pi / lx_eff;
    let ky1 = pi / ly_eff;
    let omega_11 = (d / rho_h).sqrt() * (kx1 * kx1 + ky1 * ky1);
    let f_11 = omega_11 / (2.0 * pi);

    let values: Vec<f64> = active
        .iter()
        .map(|&(i, j)| {
            let xi = (i as f64 - 1.0) / (nx as f64 - 3.0);
            let yj = (j as f64 - 1.0) / (ny as f64 - 3.0);
            (pi * xi).sin() * (pi * yj).sin() + (2.0 * pi * xi).sin() * (2.0 * pi * yj).sin()
        })
        .collect();

    let params = RadiationParams {
        rho_air: 1.225,
        c_air: 343.0,
        max_modes: 64,
    };
    let modal = ModalRadiationCalculator::new(&params, nx, ny, dx, &active, d, rho_h);
    let p_modal = modal.radiated_power(&values);

    let calc = RadiationCalculator::with_params(1.225, 343.0);
    let p_direct = calc.radiated_power(&points, &values, dx * dx, f_11);
    assert!(p_modal > p_direct * 1.2);
}

#[test]
fn test_direct_radiation_is_permutation_invariant() {
    let calc = RadiationCalculator::new();
    let points = [[-0.1, 0.0], [0.05, 0.02], [0.2, -0.03]];
    let values = [0.5, -0.2, 0.8];
    let permuted_points = [points[2], points[0], points[1]];
    let permuted_values = [values[2], values[0], values[1]];

    let baseline = calc.radiated_power(&points, &values, 0.01, 880.0);
    let permuted = calc.radiated_power(&permuted_points, &permuted_values, 0.01, 880.0);
    assert_close(permuted, baseline, 1e-12);
}

#[test]
fn test_direct_radiation_scales_quadratically_with_velocity() {
    let calc = RadiationCalculator::new();
    let points = [[-0.1, 0.0], [0.1, 0.0]];
    let values = [0.3, -0.4];
    let scaled = [0.6, -0.8];

    let power = calc.radiated_power(&points, &values, 0.02, 440.0);
    let power_scaled = calc.radiated_power(&points, &scaled, 0.02, 440.0);
    assert_close(power_scaled, power * 4.0, 1e-12);
}

#[test]
fn test_single_point_matches_closed_form_expression() {
    let rho_air = 1.225;
    let c_air = 343.0;
    let calc = RadiationCalculator::with_params(rho_air, c_air);
    let cell_area = 0.02;
    let freq = 500.0;
    let velocity = 0.4;
    let omega = 2.0 * std::f64::consts::PI * freq;
    let prefactor = rho_air * omega * omega / (4.0 * std::f64::consts::PI * c_air);
    let expected = prefactor * velocity * velocity * cell_area * cell_area;

    let power = calc.radiated_power(&[[0.0, 0.0]], &[velocity], cell_area, freq);
    assert_close(power, expected, 1e-12);
}

#[test]
fn test_direct_radiation_returns_zero_for_empty_input() {
    let calc = RadiationCalculator::new();
    let power = calc.radiated_power(&[], &[], 0.01, 1000.0);
    assert_eq!(power, 0.0);
}

#[test]
#[should_panic]
fn test_direct_radiation_panics_on_mismatched_lengths() {
    let calc = RadiationCalculator::new();
    let _ = calc.radiated_power(&[[0.0, 0.0]], &[1.0, 2.0], 0.01, 1000.0);
}

#[test]
fn test_modal_constructor_respects_max_modes() {
    let params = RadiationParams {
        rho_air: 1.225,
        c_air: 343.0,
        max_modes: 3,
    };
    let active = rectangular_active_cells(8, 8);
    let calc = ModalRadiationCalculator::new(&params, 8, 8, 0.01, &active, 1.0, 1.0);
    assert!(calc.num_modes() <= params.max_modes);
}

#[test]
#[should_panic]
fn test_modal_radiation_panics_on_mismatched_active_value_length() {
    let params = RadiationParams {
        rho_air: 1.225,
        c_air: 343.0,
        max_modes: 4,
    };
    let active = rectangular_active_cells(6, 6);
    let calc = ModalRadiationCalculator::new(&params, 6, 6, 0.01, &active, 1.0, 1.0);
    let _ = calc.radiated_power(&vec![0.0; active.len() - 1]);
}
