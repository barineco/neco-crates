use neco_array2::Array2;
use neco_contact::{build_spatial_mask, collect_interior, find_nearest, HertzContact};

#[test]
fn find_nearest_hits_center() {
    let n = 5;
    let mut x = Array2::zeros((n, n));
    let mut y = Array2::zeros((n, n));
    for i in 0..n {
        for j in 0..n {
            x[[i, j]] = -1.0 + 2.0 * i as f64 / (n - 1) as f64;
            y[[i, j]] = -1.0 + 2.0 * j as f64 / (n - 1) as f64;
        }
    }
    assert_eq!(find_nearest(&x, &y, 0.0, 0.0), (2, 2));
}

#[test]
fn find_nearest_breaks_ties_by_first_seen_cell() {
    let x = Array2::from_shape_vec((1, 2), vec![-1.0, 1.0]).expect("shape");
    let y = Array2::from_shape_vec((1, 2), vec![0.0, 0.0]).expect("shape");
    assert_eq!(find_nearest(&x, &y, 0.0, 0.0), (0, 0));
}

#[test]
fn spatial_mask_is_normalized() {
    let n = 21;
    let mut x = Array2::zeros((n, n));
    let mut y = Array2::zeros((n, n));
    for i in 0..n {
        for j in 0..n {
            x[[i, j]] = -1.0 + 0.1 * i as f64;
            y[[i, j]] = -1.0 + 0.1 * j as f64;
        }
    }
    let mask = build_spatial_mask(&x, &y, 0.0, 0.0, 0.3, None);
    let sum: f64 = mask.iter().sum();
    assert!((sum - 1.0).abs() < 1e-12, "sum={sum}");
}

#[test]
fn spatial_mask_zero_width_returns_all_zeros() {
    let x = Array2::from_shape_vec((2, 2), vec![0.0, 1.0, 0.0, 1.0]).expect("shape");
    let y = Array2::from_shape_vec((2, 2), vec![0.0, 0.0, 1.0, 1.0]).expect("shape");
    let mask = build_spatial_mask(&x, &y, 0.0, 0.0, 0.0, None);
    assert!(mask.iter().all(|value| *value == 0.0));
}

#[test]
fn spatial_mask_respects_interior_filter() {
    let n = 5;
    let mut x = Array2::zeros((n, n));
    let mut y = Array2::zeros((n, n));
    let mut interior = Array2::from_elem((n, n), true);
    for i in 0..n {
        for j in 0..n {
            x[[i, j]] = i as f64;
            y[[i, j]] = j as f64;
        }
    }
    interior[[0, 0]] = false;
    interior[[1, 0]] = false;
    let mask = build_spatial_mask(&x, &y, 0.0, 0.0, 5.0, Some(&interior));
    assert_eq!(mask[[0, 0]], 0.0);
    assert_eq!(mask[[1, 0]], 0.0);
}

#[test]
fn collect_interior_respects_margin_and_false_cells() {
    let mut interior = Array2::from_elem((5, 5), true);
    interior[[2, 2]] = false;
    let points = collect_interior(&interior, 1);
    assert_eq!(points.len(), 8);
    assert!(!points.contains(&(0, 0)));
    assert!(!points.contains(&(2, 2)));
    assert!(points.contains(&(1, 1)));
}

#[test]
fn collect_interior_large_margin_returns_empty() {
    let interior = Array2::from_elem((5, 5), true);
    let points = collect_interior(&interior, 3);
    assert!(points.is_empty());
}

#[test]
fn hertz_force_becomes_positive_during_contact() {
    let mut contact = HertzContact::new(0.01, 1e9, 1.5, 1.0);
    let _ = contact.step(0.0, 1e-5);
    let force = contact.step(0.0, 1e-5);
    assert!(force > 0.0);
}

#[test]
fn contact_override_forces_zero_output() {
    let mut contact = HertzContact::new(0.01, 1e9, 1.5, 1.0);
    contact.set_contact_ended(true);
    assert_eq!(contact.step(0.0, 1e-5), 0.0);
    assert!(contact.contact_ended());
}

#[test]
fn hertz_energy_stays_nonnegative() {
    let mut contact = HertzContact::new(0.01, 1e9, 1.5, 1.0);
    for _ in 0..10 {
        let _ = contact.step(0.0, 1e-5);
        assert!(contact.energy() >= 0.0);
    }
}

#[test]
fn hertz_contact_eventually_ends() {
    let mut contact = HertzContact::new(0.01, 1e9, 1.5, 1.0);
    let dt = 1e-7;
    let surface_mass = 0.1;
    let mut w_surface = 0.0;
    let mut v_surface = 0.0;
    for _ in 0..10_000_000 {
        let force = contact.step(w_surface, dt);
        v_surface += force / surface_mass * dt;
        w_surface += v_surface * dt;
        if contact.contact_ended() {
            break;
        }
    }
    assert!(contact.contact_ended());
}
