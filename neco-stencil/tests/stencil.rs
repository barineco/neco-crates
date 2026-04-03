use neco_stencil as stencil;
use neco_stencil::StencilError;
use std::f64::consts::PI;

fn idx(ny: usize, i: usize, j: usize) -> usize {
    i * ny + j
}

#[test]
fn test_laplacian_quadratic() {
    let n = 11;
    let dx = 0.1;
    let mut f = vec![0.0; n * n];
    for i in 0..n {
        for j in 0..n {
            let x = (i as f64 - 5.0) * dx;
            let y = (j as f64 - 5.0) * dx;
            f[idx(n, i, j)] = x * x + y * y;
        }
    }
    let mut out = vec![0.0; n * n];
    stencil::laplacian(&f, n, n, dx, &mut out).expect("valid stencil inputs must succeed");
    assert!((out[idx(n, 5, 5)] - 4.0).abs() < 1e-10);
    assert!((out[idx(n, 3, 3)] - 4.0).abs() < 1e-10);
    assert_eq!(out[idx(n, 0, 0)], 0.0);
}

#[test]
fn test_gradient_x_linear() {
    let n = 11;
    let dx = 0.1;
    let mut f = vec![0.0; n * n];
    for i in 0..n {
        for j in 0..n {
            f[idx(n, i, j)] = 3.0 * i as f64 * dx;
        }
    }
    let mut out = vec![0.0; n * n];
    stencil::gradient_x(&f, n, n, dx, &mut out).expect("valid stencil inputs must succeed");
    assert!((out[idx(n, 5, 5)] - 3.0).abs() < 1e-10);
}

#[test]
fn test_gradient_y_linear() {
    let n = 11;
    let dx = 0.1;
    let mut f = vec![0.0; n * n];
    for i in 0..n {
        for j in 0..n {
            f[idx(n, i, j)] = -2.5 * j as f64 * dx;
        }
    }
    let mut out = vec![0.0; n * n];
    stencil::gradient_y(&f, n, n, dx, &mut out).expect("valid stencil inputs must succeed");
    assert!((out[idx(n, 5, 5)] + 2.5).abs() < 1e-10);
    assert_eq!(out[idx(n, 0, 0)], 0.0);
}

#[test]
fn test_second_derivatives_match_quadratic_and_mixed_polynomials() {
    let n = 11;
    let dx = 0.1;
    let mut xx = vec![0.0; n * n];
    let mut yy = vec![0.0; n * n];
    let mut xy = vec![0.0; n * n];
    for i in 0..n {
        for j in 0..n {
            let x = (i as f64 - 5.0) * dx;
            let y = (j as f64 - 5.0) * dx;
            xx[idx(n, i, j)] = x * x;
            yy[idx(n, i, j)] = y * y;
            xy[idx(n, i, j)] = x * y;
        }
    }

    let mut out_xx = vec![0.0; n * n];
    let mut out_yy = vec![0.0; n * n];
    let mut out_xy = vec![0.0; n * n];
    stencil::d2_dx2(&xx, n, n, dx, &mut out_xx).expect("valid stencil inputs must succeed");
    stencil::d2_dy2(&yy, n, n, dx, &mut out_yy).expect("valid stencil inputs must succeed");
    stencil::d2_dxdy(&xy, n, n, dx, &mut out_xy).expect("valid stencil inputs must succeed");

    let center = idx(n, 5, 5);
    assert!((out_xx[center] - 2.0).abs() < 1e-10);
    assert!((out_yy[center] - 2.0).abs() < 1e-10);
    assert!((out_xy[center] - 1.0).abs() < 1e-10);
}

#[test]
fn test_w_derivatives_matches_individual_operators() {
    let n = 13;
    let dx = 0.1;
    let mut w = vec![0.0; n * n];
    for i in 0..n {
        for j in 0..n {
            let x = (i as f64 - 6.0) * dx;
            let y = (j as f64 - 6.0) * dx;
            w[idx(n, i, j)] = x * x + 2.0 * x * y + 3.0 * y * y;
        }
    }

    let mut wx = vec![0.0; n * n];
    let mut wy = vec![0.0; n * n];
    let mut wxx = vec![0.0; n * n];
    let mut wyy = vec![0.0; n * n];
    let mut wxy = vec![0.0; n * n];
    stencil::w_derivatives(&w, n, n, dx, &mut wx, &mut wy, &mut wxx, &mut wyy, &mut wxy)
        .expect("valid stencil inputs must succeed");

    let mut grad_x = vec![0.0; n * n];
    let mut grad_y = vec![0.0; n * n];
    let mut dxx = vec![0.0; n * n];
    let mut dyy = vec![0.0; n * n];
    let mut dxy = vec![0.0; n * n];
    stencil::gradient_x(&w, n, n, dx, &mut grad_x).expect("valid stencil inputs must succeed");
    stencil::gradient_y(&w, n, n, dx, &mut grad_y).expect("valid stencil inputs must succeed");
    stencil::d2_dx2(&w, n, n, dx, &mut dxx).expect("valid stencil inputs must succeed");
    stencil::d2_dy2(&w, n, n, dx, &mut dyy).expect("valid stencil inputs must succeed");
    stencil::d2_dxdy(&w, n, n, dx, &mut dxy).expect("valid stencil inputs must succeed");

    for i in 1..n - 1 {
        for j in 1..n - 1 {
            let k = idx(n, i, j);
            assert!(
                (wx[k] - grad_x[k]).abs() < 1e-12,
                "wx mismatch at [{i},{j}]"
            );
            assert!(
                (wy[k] - grad_y[k]).abs() < 1e-12,
                "wy mismatch at [{i},{j}]"
            );
            assert!((wxx[k] - dxx[k]).abs() < 1e-12, "wxx mismatch at [{i},{j}]");
            assert!((wyy[k] - dyy[k]).abs() < 1e-12, "wyy mismatch at [{i},{j}]");
            assert!((wxy[k] - dxy[k]).abs() < 1e-12, "wxy mismatch at [{i},{j}]");
        }
    }
}

#[test]
fn test_uv_gradients_matches_individual_gradients() {
    let n = 11;
    let dx = 0.1;
    let mut u = vec![0.0; n * n];
    let mut v = vec![0.0; n * n];
    for i in 0..n {
        for j in 0..n {
            let x = (i as f64 - 5.0) * dx;
            let y = (j as f64 - 5.0) * dx;
            u[idx(n, i, j)] = 4.0 * x - y;
            v[idx(n, i, j)] = x + 2.0 * y;
        }
    }

    let mut ux = vec![0.0; n * n];
    let mut uy = vec![0.0; n * n];
    let mut vx = vec![0.0; n * n];
    let mut vy = vec![0.0; n * n];
    stencil::uv_gradients(&u, &v, n, n, dx, &mut ux, &mut uy, &mut vx, &mut vy)
        .expect("valid stencil inputs must succeed");

    let mut ux_ref = vec![0.0; n * n];
    let mut uy_ref = vec![0.0; n * n];
    let mut vx_ref = vec![0.0; n * n];
    let mut vy_ref = vec![0.0; n * n];
    stencil::gradient_x(&u, n, n, dx, &mut ux_ref).expect("valid stencil inputs must succeed");
    stencil::gradient_y(&u, n, n, dx, &mut uy_ref).expect("valid stencil inputs must succeed");
    stencil::gradient_x(&v, n, n, dx, &mut vx_ref).expect("valid stencil inputs must succeed");
    stencil::gradient_y(&v, n, n, dx, &mut vy_ref).expect("valid stencil inputs must succeed");

    for i in 1..n - 1 {
        for j in 1..n - 1 {
            let k = idx(n, i, j);
            assert!(
                (ux[k] - ux_ref[k]).abs() < 1e-12,
                "ux mismatch at [{i},{j}]"
            );
            assert!(
                (uy[k] - uy_ref[k]).abs() < 1e-12,
                "uy mismatch at [{i},{j}]"
            );
            assert!(
                (vx[k] - vx_ref[k]).abs() < 1e-12,
                "vx mismatch at [{i},{j}]"
            );
            assert!(
                (vy[k] - vy_ref[k]).abs() < 1e-12,
                "vy mismatch at [{i},{j}]"
            );
        }
    }
}

#[test]
fn test_biharmonic_constant_d() {
    let n = 21;
    let dx = 0.05;
    let d = 100.0;
    let mut f = vec![0.0; n * n];
    let d_grid = vec![d; n * n];
    for i in 0..n {
        for j in 0..n {
            let x = (i as f64 - 10.0) * dx;
            f[idx(n, i, j)] = x.powi(4);
        }
    }
    let mut lap = vec![0.0; n * n];
    let mut d_lap = vec![0.0; n * n];
    let mut bilap = vec![0.0; n * n];
    stencil::biharmonic(&f, &d_grid, n, n, dx, &mut lap, &mut d_lap, &mut bilap)
        .expect("valid stencil inputs must succeed");
    let val = bilap[idx(n, 10, 10)];
    assert!((val - 2400.0).abs() < 50.0, "got {val}");
}

#[test]
fn test_bilaplacian_uniform_quartic() {
    let n = 11;
    let dx = 0.1;
    let d = 1.0;
    let mut f = vec![0.0; n * n];
    for i in 0..n {
        for j in 0..n {
            let x = (i as f64 - 5.0) * dx;
            let y = (j as f64 - 5.0) * dx;
            f[idx(n, i, j)] = x.powi(4) + y.powi(4);
        }
    }
    let mut bilap = vec![0.0; n * n];
    stencil::bilaplacian_uniform(&f, n, n, d, dx, &mut bilap)
        .expect("valid stencil inputs must succeed");
    for i in 2..n - 2 {
        for j in 2..n - 2 {
            assert!(
                (bilap[idx(n, i, j)] - 48.0).abs() < 1e-6,
                "bilap[{i},{j}] = {} (expected 48.0)",
                bilap[idx(n, i, j)]
            );
        }
    }
}

#[test]
fn test_bilaplacian_uniform_matches_biharmonic() {
    let n = 21;
    let dx = 0.05;
    let d = 2.5;
    let mut w = vec![0.0; n * n];
    for i in 0..n {
        for j in 0..n {
            let x = i as f64 * dx;
            let y = j as f64 * dx;
            w[idx(n, i, j)] = (3.0 * x).sin() * (2.0 * y).cos() + (x * y).sin();
        }
    }
    let d_grid = vec![d; n * n];
    let mut lap = vec![0.0; n * n];
    let mut d_lap = vec![0.0; n * n];
    let mut bilap_bh = vec![0.0; n * n];
    stencil::biharmonic(&w, &d_grid, n, n, dx, &mut lap, &mut d_lap, &mut bilap_bh)
        .expect("valid stencil inputs must succeed");
    let mut bilap_uni = vec![0.0; n * n];
    stencil::bilaplacian_uniform(&w, n, n, d, dx, &mut bilap_uni)
        .expect("valid stencil inputs must succeed");
    for i in 2..n - 2 {
        for j in 2..n - 2 {
            let bh = bilap_bh[idx(n, i, j)];
            let uni = bilap_uni[idx(n, i, j)];
            let rel_err = if bh.abs() > 1e-12 {
                ((bh - uni) / bh).abs()
            } else {
                (bh - uni).abs()
            };
            assert!(
                rel_err < 1e-9,
                "mismatch at [{i},{j}]: biharmonic={bh}, uniform={uni}, rel_err={rel_err}"
            );
        }
    }
}

#[test]
fn test_biharmonic_pass1_fused_matches_separate() {
    let n = 21;
    let dx = 0.05;
    let mut w = vec![0.0; n * n];
    let mut d_grid = vec![0.0; n * n];
    for i in 0..n {
        for j in 0..n {
            let x = i as f64 * dx;
            let y = j as f64 * dx;
            w[idx(n, i, j)] = (3.0 * PI * x).sin() * (2.0 * PI * y).cos() + (x * y).sin();
            d_grid[idx(n, i, j)] = 1.0 + 0.5 * (PI * x).sin() * (PI * y).cos();
        }
    }
    let mut lap_sep = vec![0.0; n * n];
    let mut d_lap_sep = vec![0.0; n * n];
    let mut bilap_sep = vec![0.0; n * n];
    stencil::biharmonic(
        &w,
        &d_grid,
        n,
        n,
        dx,
        &mut lap_sep,
        &mut d_lap_sep,
        &mut bilap_sep,
    )
    .expect("valid stencil inputs must succeed");
    let mut wx_sep = vec![0.0; n * n];
    let mut wy_sep = vec![0.0; n * n];
    let mut wxx_sep = vec![0.0; n * n];
    let mut wyy_sep = vec![0.0; n * n];
    let mut wxy_sep = vec![0.0; n * n];
    stencil::w_derivatives(
        &w,
        n,
        n,
        dx,
        &mut wx_sep,
        &mut wy_sep,
        &mut wxx_sep,
        &mut wyy_sep,
        &mut wxy_sep,
    )
    .expect("valid stencil inputs must succeed");
    let mut d_lap_fused = vec![0.0; n * n];
    let mut wx_fused = vec![0.0; n * n];
    let mut wy_fused = vec![0.0; n * n];
    let mut wxx_fused = vec![0.0; n * n];
    let mut wyy_fused = vec![0.0; n * n];
    let mut wxy_fused = vec![0.0; n * n];
    stencil::biharmonic_pass1_fused(
        &w,
        &d_grid,
        n,
        n,
        dx,
        &mut d_lap_fused,
        &mut wx_fused,
        &mut wy_fused,
        &mut wxx_fused,
        &mut wyy_fused,
        &mut wxy_fused,
    )
    .expect("valid stencil inputs must succeed");
    let mut bilap_fused = vec![0.0; n * n];
    stencil::laplacian(&d_lap_fused, n, n, dx, &mut bilap_fused)
        .expect("valid stencil inputs must succeed");
    for i in 1..n - 1 {
        for j in 1..n - 1 {
            let k = idx(n, i, j);
            let check = |name: &str, a: f64, b: f64| {
                let rel = if a.abs() > 1e-12 {
                    ((a - b) / a).abs()
                } else {
                    (a - b).abs()
                };
                assert!(
                    rel < 1e-10,
                    "{name} mismatch at [{i},{j}]: sep={a}, fused={b}, rel_err={rel}"
                );
            };
            check("d_lap", d_lap_sep[k], d_lap_fused[k]);
            check("wx", wx_sep[k], wx_fused[k]);
            check("wy", wy_sep[k], wy_fused[k]);
            check("wxx", wxx_sep[k], wxx_fused[k]);
            check("wyy", wyy_sep[k], wyy_fused[k]);
            check("wxy", wxy_sep[k], wxy_fused[k]);
            check("bilap", bilap_sep[k], bilap_fused[k]);
        }
    }
}

#[test]
fn test_bilaplacian_ortho_uniform_matches_uniform_in_isotropic_case() {
    let n = 11;
    let dx = 0.1;
    let d = 1.5;
    let mut w = vec![0.0; n * n];
    for i in 0..n {
        for j in 0..n {
            let x = (i as f64 - 5.0) * dx;
            let y = (j as f64 - 5.0) * dx;
            w[idx(n, i, j)] = x.powi(4) + x * x * y * y + y.powi(4);
        }
    }

    let mut bilap_uniform = vec![0.0; n * n];
    stencil::bilaplacian_uniform(&w, n, n, d, dx, &mut bilap_uniform)
        .expect("valid stencil inputs must succeed");

    let mut bilap_ortho = vec![0.0; n * n];
    let cells: Vec<(usize, usize)> = (2..n - 2)
        .flat_map(|i| (2..n - 2).map(move |j| (i, j)))
        .collect();
    stencil::bilaplacian_ortho_uniform(&w, n, n, d, d, d, dx, &mut bilap_ortho, &cells)
        .expect("valid stencil inputs must succeed");

    for &(i, j) in &cells {
        let k = idx(n, i, j);
        assert!(
            (bilap_uniform[k] - bilap_ortho[k]).abs() < 1e-10,
            "ortho mismatch at [{i},{j}]: uniform={}, ortho={}",
            bilap_uniform[k],
            bilap_ortho[k]
        );
    }
}

#[cfg(feature = "rayon")]
#[test]
fn test_rayon_large_grid_contracts_match_reference_values() {
    let n = 128;
    let dx = 0.02;
    let d = 1.75;
    let mut w = vec![0.0; n * n];
    let d_grid = vec![d; n * n];
    for i in 0..n {
        for j in 0..n {
            let x = (i as f64 - 64.0) * dx;
            let y = (j as f64 - 64.0) * dx;
            w[idx(n, i, j)] = x.powi(4) + y.powi(4);
        }
    }
    let mut lap = vec![0.0; n * n];
    stencil::laplacian(&w, n, n, dx, &mut lap).expect("valid stencil inputs must succeed");
    let expected_center = 4.0 * dx * dx;
    assert!(
        (lap[idx(n, 64, 64)] - expected_center).abs() < 1e-10,
        "lap center={}, expected={expected_center}",
        lap[idx(n, 64, 64)]
    );
    let expected_off_center =
        12.0 * ((50.0 - 64.0) * dx).powi(2) + 12.0 * ((40.0 - 64.0) * dx).powi(2) + 4.0 * dx * dx;
    assert!(
        (lap[idx(n, 50, 40)] - expected_off_center).abs() < 1e-8,
        "lap off-center={}, expected={expected_off_center}",
        lap[idx(n, 50, 40)]
    );
    let mut lap_work = vec![0.0; n * n];
    let mut d_lap = vec![0.0; n * n];
    let mut bilap = vec![0.0; n * n];
    stencil::biharmonic(&w, &d_grid, n, n, dx, &mut lap_work, &mut d_lap, &mut bilap)
        .expect("valid stencil inputs must succeed");
    let expected = 48.0 * d;
    assert!(
        (bilap[idx(n, 64, 64)] - expected).abs() < 1e-6,
        "bilap center={}, expected={expected}",
        bilap[idx(n, 64, 64)]
    );
}

#[test]
fn mismatched_lengths_return_errors() {
    let u = vec![0.0; 8];
    let mut out = vec![0.0; 9];
    let error = stencil::laplacian(&u, 3, 3, 0.1, &mut out)
        .expect_err("mismatched input length must be rejected");
    assert_eq!(
        error,
        StencilError::InvalidLength {
            name: "u",
            expected: 9,
            actual: 8,
        }
    );

    let w = vec![0.0; 9];
    let mut wx = vec![0.0; 9];
    let mut wy = vec![0.0; 8];
    let mut wxx = vec![0.0; 9];
    let mut wyy = vec![0.0; 9];
    let mut wxy = vec![0.0; 9];
    let error = stencil::w_derivatives(
        &w, 3, 3, 0.1, &mut wx, &mut wy, &mut wxx, &mut wyy, &mut wxy,
    )
    .expect_err("mismatched output length must be rejected");
    assert_eq!(
        error,
        StencilError::InvalidLength {
            name: "wy",
            expected: 9,
            actual: 8,
        }
    );
}
