/// Polynomial solvers (direct formulas for degree 2-4, companion matrix for higher degrees).
pub mod quartic;

#[cfg(feature = "polynomial-highorder")]
mod companion;

use quartic::{solve_cubic, solve_quadratic, solve_quartic};

/// Error type for polynomial solvers.
#[derive(Debug)]
pub enum PolynomialError {
    /// High-order solver unavailable (enable the `polynomial-highorder` feature).
    HighOrderNotAvailable { degree: usize },
}

impl std::fmt::Display for PolynomialError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            PolynomialError::HighOrderNotAvailable { degree } => {
                write!(
                    f,
                    "degree-{} polynomial solver not available (enable polynomial-highorder feature)",
                    degree
                )
            }
        }
    }
}

impl std::error::Error for PolynomialError {}

/// Find all real roots of the polynomial coeffs[n]*x^n + ... + coeffs[1]*x + coeffs[0] = 0.
pub fn solve_polynomial(coeffs: &[f64]) -> Result<Vec<f64>, PolynomialError> {
    let effective_degree = match coeffs.iter().rposition(|&c| c.abs() > 1e-12) {
        Some(i) => i,
        None => return Ok(vec![]),
    };

    match effective_degree {
        0 => Ok(vec![]),
        1 => Ok(vec![-coeffs[0] / coeffs[1]]),
        2 => Ok(solve_quadratic(coeffs[2], coeffs[1], coeffs[0])),
        3 => Ok(solve_cubic([coeffs[0], coeffs[1], coeffs[2], coeffs[3]])),
        4 => Ok(solve_quartic([
            coeffs[0], coeffs[1], coeffs[2], coeffs[3], coeffs[4],
        ])),
        _ => {
            #[cfg(feature = "polynomial-highorder")]
            {
                Ok(companion::solve_companion_schur(
                    &coeffs[..=effective_degree],
                ))
            }
            #[cfg(not(feature = "polynomial-highorder"))]
            {
                Err(PolynomialError::HighOrderNotAvailable {
                    degree: effective_degree,
                })
            }
        }
    }
}

/// Newton's method root refinement for a polynomial. coeffs[i] is the coefficient of x^i.
pub fn newton_refine(coeffs: &[f64], x: f64, iterations: usize) -> f64 {
    let mut x = x;
    for _ in 0..iterations {
        let (f, df) = eval_poly_and_deriv(coeffs, x);
        if df.abs() < 1e-15 {
            break;
        }
        x -= f / df;
    }
    x
}

/// Evaluate a polynomial and its derivative simultaneously (Horner's method).
fn eval_poly_and_deriv(coeffs: &[f64], x: f64) -> (f64, f64) {
    let n = coeffs.len();
    if n == 0 {
        return (0.0, 0.0);
    }
    let mut f = coeffs[n - 1];
    let mut df = 0.0;
    for i in (0..n - 1).rev() {
        df = df * x + f;
        f = f * x + coeffs[i];
    }
    (f, df)
}

/// Evaluate a polynomial using Horner's method.
pub fn eval_poly(coeffs: &[f64], x: f64) -> f64 {
    let n = coeffs.len();
    if n == 0 {
        return 0.0;
    }
    let mut f = coeffs[n - 1];
    for i in (0..n - 1).rev() {
        f = f * x + coeffs[i];
    }
    f
}

/// Compute the derivative polynomial. coeffs[i] is the coefficient of x^i.
#[cfg(feature = "polynomial-highorder")]
pub(super) fn poly_derivative(coeffs: &[f64]) -> Vec<f64> {
    if coeffs.len() <= 1 {
        return vec![0.0];
    }
    coeffs
        .iter()
        .enumerate()
        .skip(1)
        .map(|(i, &c)| c * i as f64)
        .collect()
}

/// Approximate polynomial GCD via the Euclidean algorithm.
#[cfg(feature = "polynomial-highorder")]
fn poly_gcd(a: &[f64], b: &[f64]) -> Vec<f64> {
    const MAX_GCD_ITERATIONS: usize = 100;

    let mut r0 = trim_leading_zeros(a);
    let mut r1 = trim_leading_zeros(b);

    if r1.is_empty() {
        return r0;
    }
    if r0.is_empty() {
        return r1;
    }

    let mut iterations = 0;
    while r1.len() > 1 || (r1.len() == 1 && r1[0].abs() > 1e-8) {
        if iterations >= MAX_GCD_ITERATIONS {
            break;
        }
        let rem = poly_rem(&r0, &r1);
        r0 = r1;
        r1 = rem;
        iterations += 1;
    }
    r0
}

/// Polynomial remainder a mod b.
#[cfg(feature = "polynomial-highorder")]
fn poly_rem(a: &[f64], b: &[f64]) -> Vec<f64> {
    if a.len() < b.len() {
        return a.to_vec();
    }
    let mut rem = a.to_vec();
    let b_lead = *b.last().unwrap_or(&1.0);
    let b_deg = b.len() - 1;

    for i in (0..=(a.len() - b.len())).rev() {
        let coeff = rem[i + b_deg] / b_lead;
        for j in 0..b.len() {
            rem[i + j] -= coeff * b[j];
        }
    }
    rem.truncate(b_deg);
    trim_leading_zeros(&rem)
}

/// Strip trailing zero coefficients (highest degree).
#[cfg(feature = "polynomial-highorder")]
pub(super) fn trim_leading_zeros(p: &[f64]) -> Vec<f64> {
    let end = p
        .iter()
        .rposition(|&c| c.abs() > 1e-10)
        .map(|i| i + 1)
        .unwrap_or(0);
    p[..end].to_vec()
}

/// Compute the square-free part p / gcd(p, p').
#[cfg(feature = "polynomial-highorder")]
pub(super) fn poly_square_free(p: &[f64], dp: &[f64]) -> Vec<f64> {
    let g = poly_gcd(p, dp);
    if g.len() <= 1 {
        return p.to_vec();
    }
    poly_exact_div(p, &g)
}

/// Exact polynomial division a / b (assumes zero remainder).
#[cfg(feature = "polynomial-highorder")]
fn poly_exact_div(a: &[f64], b: &[f64]) -> Vec<f64> {
    if b.is_empty() || a.len() < b.len() {
        return a.to_vec();
    }
    let mut rem = a.to_vec();
    let b_lead = *b.last().unwrap_or(&1.0);
    let q_len = a.len() - b.len() + 1;
    let mut quotient = vec![0.0; q_len];

    for i in (0..q_len).rev() {
        let coeff = rem[i + b.len() - 1] / b_lead;
        quotient[i] = coeff;
        for j in 0..b.len() {
            rem[i + j] -= coeff * b[j];
        }
    }
    trim_leading_zeros(&quotient)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn quadratic_matches_direct_solver() {
        let coeffs = [-6.0, -1.0, 1.0];
        let poly_roots = solve_polynomial(&coeffs).unwrap();
        let direct_roots = solve_quadratic(1.0, -1.0, -6.0);
        assert_eq!(poly_roots.len(), direct_roots.len());
        for (a, b) in poly_roots.iter().zip(direct_roots.iter()) {
            assert!((a - b).abs() < 1e-12, "mismatch: poly={a}, direct={b}");
        }
    }

    #[test]
    fn cubic_matches_direct_solver() {
        let coeffs = [-6.0, 11.0, -6.0, 1.0];
        let poly_roots = solve_polynomial(&coeffs).unwrap();
        let direct_roots = solve_cubic([coeffs[0], coeffs[1], coeffs[2], coeffs[3]]);
        assert_eq!(poly_roots.len(), direct_roots.len());
        for (a, b) in poly_roots.iter().zip(direct_roots.iter()) {
            assert!((a - b).abs() < 1e-12, "mismatch: poly={a}, direct={b}");
        }
    }

    #[test]
    fn quartic_matches_direct_solver() {
        let coeffs = [24.0, -50.0, 35.0, -10.0, 1.0];
        let poly_roots = solve_polynomial(&coeffs).unwrap();
        let direct_roots = solve_quartic([coeffs[0], coeffs[1], coeffs[2], coeffs[3], coeffs[4]]);
        assert_eq!(poly_roots.len(), direct_roots.len());
        for (a, b) in poly_roots.iter().zip(direct_roots.iter()) {
            assert!((a - b).abs() < 1e-12, "mismatch: poly={a}, direct={b}");
        }
    }

    #[cfg(feature = "polynomial-highorder")]
    #[test]
    fn quintic_five_roots() {
        let coeffs = [-120.0, 274.0, -225.0, 85.0, -15.0, 1.0];
        let roots = solve_polynomial(&coeffs).unwrap();
        assert_eq!(roots.len(), 5, "expected 5 roots for degree 5: {roots:?}");
        let expected = [1.0, 2.0, 3.0, 4.0, 5.0];
        for (i, e) in expected.iter().enumerate() {
            assert!(
                (roots[i] - e).abs() < 1e-10,
                "root[{i}] = {} != {e}",
                roots[i]
            );
        }
        check_residual(&coeffs, &roots, 1e-10);
    }

    #[cfg(feature = "polynomial-highorder")]
    #[test]
    fn degree_8_torus_derived() {
        let p1 = [24.0, -50.0, 35.0, -10.0, 1.0];
        let p2 = [4.0, 0.0, -5.0, 0.0, 1.0];

        let mut product = vec![0.0; 9];
        for (i, &a) in p1.iter().enumerate() {
            for (j, &b) in p2.iter().enumerate() {
                product[i + j] += a * b;
            }
        }

        let roots = solve_polynomial(&product).unwrap();
        check_residual(&product, &roots, 1e-8);

        let expected = [-2.0, -1.0, 1.0, 2.0, 3.0, 4.0];
        assert_eq!(
            roots.len(),
            expected.len(),
            "expected 6 distinct roots for degree 8 (with repeated roots): {roots:?}"
        );
        for (i, e) in expected.iter().enumerate() {
            assert!(
                (roots[i] - e).abs() < 1e-8,
                "root[{i}] = {} != {e}",
                roots[i]
            );
        }
    }

    #[cfg(feature = "polynomial-highorder")]
    #[test]
    fn degree_16_all_roots() {
        let mut coeffs = vec![1.0];
        for k in 1..=16 {
            let mut new_coeffs = vec![0.0; coeffs.len() + 1];
            for (i, &c) in coeffs.iter().enumerate() {
                new_coeffs[i + 1] += c;
                new_coeffs[i] -= c * k as f64;
            }
            coeffs = new_coeffs;
        }

        let roots = solve_polynomial(&coeffs).unwrap();
        assert_eq!(
            roots.len(),
            16,
            "expected 16 roots for degree 16: {roots:?}"
        );
        for (i, expected) in (1..=16).enumerate() {
            assert!(
                (roots[i] - expected as f64).abs() < 1e-4,
                "root[{i}] = {} != {expected}",
                roots[i]
            );
        }
        let max_coeff = coeffs.iter().map(|c| c.abs()).fold(0.0_f64, f64::max);
        for &x in &roots {
            let val = eval_poly(&coeffs, x);
            let relative = val.abs() / max_coeff;
            assert!(
                relative < 1e-6,
                "large relative residual: f({x}) = {val}, relative = {relative}"
            );
        }
    }

    #[cfg(feature = "polynomial-highorder")]
    #[test]
    fn repeated_roots() {
        // (x-1)^4 * (x-2)^4
        let mut p1 = vec![1.0];
        for _ in 0..4 {
            let mut new = vec![0.0; p1.len() + 1];
            for (i, &c) in p1.iter().enumerate() {
                new[i + 1] += c;
                new[i] -= c;
            }
            p1 = new;
        }
        let mut p2 = vec![1.0];
        for _ in 0..4 {
            let mut new = vec![0.0; p2.len() + 1];
            for (i, &c) in p2.iter().enumerate() {
                new[i + 1] += c;
                new[i] -= 2.0 * c;
            }
            p2 = new;
        }

        let mut coeffs = vec![0.0; p1.len() + p2.len() - 1];
        for (i, &a) in p1.iter().enumerate() {
            for (j, &b) in p2.iter().enumerate() {
                coeffs[i + j] += a * b;
            }
        }

        let roots = solve_polynomial(&coeffs).unwrap();
        check_residual(&coeffs, &roots, 1e-6);

        assert_eq!(
            roots.len(),
            2,
            "expected 2 distinct roots for repeated-root case: {roots:?}"
        );
        assert!(
            (roots[0] - 1.0).abs() < 1e-3,
            "root[0] = {} != 1.0",
            roots[0]
        );
        assert!(
            (roots[1] - 2.0).abs() < 1e-3,
            "root[1] = {} != 2.0",
            roots[1]
        );
    }

    #[test]
    fn zero_polynomial() {
        assert_eq!(solve_polynomial(&[0.0]).unwrap(), Vec::<f64>::new());
    }

    #[test]
    fn constant_nonzero() {
        assert_eq!(solve_polynomial(&[5.0]).unwrap(), Vec::<f64>::new());
    }

    #[test]
    fn highorder_without_feature_returns_err() {
        let coeffs = [-120.0, 274.0, -225.0, 85.0, -15.0, 1.0];
        let result = solve_polynomial(&coeffs);
        if cfg!(feature = "polynomial-highorder") {
            assert!(result.is_ok());
        } else {
            assert!(result.is_err());
            let err = result.unwrap_err();
            match err {
                PolynomialError::HighOrderNotAvailable { degree } => {
                    assert_eq!(degree, 5);
                }
            }
        }
    }
}
