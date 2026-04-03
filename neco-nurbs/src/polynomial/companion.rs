#![cfg(feature = "polynomial-highorder")]

use super::{eval_poly, newton_refine, poly_derivative, poly_square_free};

/// High-order polynomial solver via companion matrix + Schur decomposition.
pub(super) fn solve_companion_schur(coeffs: &[f64]) -> Vec<f64> {
    let n = coeffs.len() - 1;

    // Make monic
    let lead = coeffs[n];
    let monic: Vec<f64> = coeffs[..n].iter().map(|&c| c / lead).collect();

    // Coefficient balancing: alpha = |a0|^(1/n)
    let alpha = if monic[0].abs() > 1e-15 {
        monic[0].abs().powf(1.0 / n as f64)
    } else {
        1.0
    };

    // Scale coefficients via substitution x = alpha * t
    let scaled: Vec<f64> = monic
        .iter()
        .enumerate()
        .map(|(i, &c)| c / alpha.powi(i32::try_from(n - i).expect("degree fits in i32")))
        .collect();

    // Build companion matrix: sub-diagonal ones, last column = -scaled[i]
    let mut mat = nalgebra::DMatrix::<f64>::zeros(n, n);
    for i in 1..n {
        mat[(i, i - 1)] = 1.0;
    }
    for i in 0..n {
        mat[(i, n - 1)] = -scaled[i];
    }

    let schur = nalgebra::linalg::Schur::new(mat);
    let eigenvalues = schur.complex_eigenvalues();

    // Use a loose imaginary-part threshold to capture repeated-root candidates,
    // then filter via residual check after Newton refinement
    let im_tol = 1e-2;
    let mut roots: Vec<f64> = eigenvalues
        .iter()
        .filter(|ev| ev.im.abs() < im_tol * ev.re.abs().max(1.0))
        .map(|ev| alpha * ev.re)
        .collect();

    // Square-free factorization for better Newton convergence on repeated roots
    let deriv = poly_derivative(coeffs);
    let square_free = poly_square_free(coeffs, &deriv);
    let refine_target = if square_free.len() >= 2 {
        &square_free
    } else {
        coeffs
    };

    for root in &mut roots {
        *root = newton_refine(refine_target, *root, 20);
    }

    // Discard spurious roots via residual check on the original polynomial
    let max_coeff = coeffs.iter().map(|c| c.abs()).fold(0.0_f64, f64::max);
    let residual_tol = 1e-6 * max_coeff.max(1.0);
    roots.retain(|&x| eval_poly(coeffs, x).abs() < residual_tol);

    roots.sort_by(|a, b| a.total_cmp(b));
    roots.dedup_by(|a, b| (*a - *b).abs() < 1e-6);

    roots
}
