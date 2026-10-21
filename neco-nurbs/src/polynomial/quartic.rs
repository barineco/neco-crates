//! 実係数の二次・三次・四次多項式の実根を求めます。

const EPS: f64 = 1e-12;

/// `a*x² + b*x + c` の相異なる実根を昇順で返します。重根は一つだけ返します。
pub fn solve_quadratic(a: f64, b: f64, c: f64) -> Vec<f64> {
    if a.abs() < EPS {
        if b.abs() < EPS {
            return vec![];
        }
        return vec![-c / b];
    }
    let disc = b * b - 4.0 * a * c;
    if disc < -EPS {
        return vec![];
    }
    if disc.abs() <= EPS {
        return vec![-b / (2.0 * a)];
    }
    let sqrt_disc = disc.sqrt();
    let q = -0.5 * (b + b.signum() * sqrt_disc);
    let x1 = q / a;
    let x2 = c / q;
    if x1 < x2 {
        vec![x1, x2]
    } else {
        vec![x2, x1]
    }
}

/// 定数項から昇べき順の係数を受け取り、三次多項式の実根だけを相異なる値で返します。重根は一つにまとめます。
pub fn solve_cubic(coeffs: [f64; 4]) -> Vec<f64> {
    let [a0, a1, a2, a3] = coeffs;
    if a3.abs() < EPS {
        return solve_quadratic(a2, a1, a0);
    }
    let b = a2 / a3;
    let c = a1 / a3;
    let d = a0 / a3;

    let p = c - b * b / 3.0;
    let q = d - b * c / 3.0 + 2.0 * b * b * b / 27.0;
    let shift = -b / 3.0;

    let disc = -4.0 * p * p * p - 27.0 * q * q;

    if disc > EPS {
        let m = (-p / 3.0).sqrt();
        let theta = (-q / (2.0 * m * m * m)).clamp(-1.0, 1.0).acos() / 3.0;
        let two_m = 2.0 * m;
        vec![
            two_m * theta.cos() + shift,
            two_m * (theta - std::f64::consts::FRAC_PI_3 * 2.0).cos() + shift,
            two_m * (theta + std::f64::consts::FRAC_PI_3 * 2.0).cos() + shift,
        ]
    } else if disc.abs() <= EPS {
        if q.abs() < EPS {
            vec![shift]
        } else {
            let u = cbrt(-q / 2.0);
            let r1 = 2.0 * u + shift;
            let r2 = -u + shift;
            vec![r1, r2]
        }
    } else {
        let s = -q / 2.0;
        let t = (q * q / 4.0 + p * p * p / 27.0).sqrt();
        let u = cbrt(s + t);
        let v = cbrt(s - t);
        vec![u + v + shift]
    }
}

fn cbrt(x: f64) -> f64 {
    x.signum() * x.abs().cbrt()
}

/// 定数項から昇べき順の係数を受け取り、四次多項式の実根だけを相異なる値で返します。重根は一つにまとめます。
pub fn solve_quartic(coeffs: [f64; 5]) -> Vec<f64> {
    let [a0, a1, a2, a3, a4] = coeffs;
    if a4.abs() < EPS {
        return solve_cubic([a0, a1, a2, a3]);
    }
    let b = a3 / a4;
    let c = a2 / a4;
    let d = a1 / a4;
    let e = a0 / a4;

    let b2 = b * b;
    let b3 = b2 * b;
    let b4 = b2 * b2;
    let p = c - 3.0 * b2 / 8.0;
    let q = d - b * c / 2.0 + b3 / 8.0;
    let r = e - b * d / 4.0 + b2 * c / 16.0 - 3.0 * b4 / 256.0;
    let shift = -b / 4.0;

    if q.abs() < EPS {
        let u_roots = solve_quadratic(1.0, p, r);
        let mut roots = Vec::new();
        for u in u_roots {
            if u > EPS {
                let s = u.sqrt();
                roots.push(s + shift);
                roots.push(-s + shift);
            } else if u.abs() <= EPS {
                roots.push(shift);
            }
        }
        refine_and_return(coeffs, roots)
    } else {
        let cubic_coeffs = [p * r / 2.0 - q * q / 8.0, -r, -p / 2.0, 1.0];
        let cubic_roots = solve_cubic(cubic_coeffs);
        let z0 = cubic_roots.into_iter().fold(f64::NEG_INFINITY, f64::max);

        let s2 = 2.0 * z0 - p;
        if s2 < -EPS {
            return vec![];
        }
        let s = if s2 > EPS { s2.sqrt() } else { 0.0 };

        let mut roots = Vec::new();

        if s.abs() < EPS {
            let inner = z0 * z0 - r;
            if inner >= -EPS {
                let sqrt_inner = inner.max(0.0).sqrt();
                roots.extend(solve_quadratic(1.0, 0.0, z0 - sqrt_inner));
                roots.extend(solve_quadratic(1.0, 0.0, z0 + sqrt_inner));
            }
        } else {
            let half_q_over_s = q / (2.0 * s);
            roots.extend(solve_quadratic(1.0, s, z0 - half_q_over_s));
            roots.extend(solve_quadratic(1.0, -s, z0 + half_q_over_s));
        }

        for root in &mut roots {
            *root += shift;
        }
        refine_and_return(coeffs, roots)
    }
}

fn refine_and_return(coeffs: [f64; 5], mut roots: Vec<f64>) -> Vec<f64> {
    for root in &mut roots {
        *root = super::newton_refine(&coeffs, *root, 2);
    }
    roots.sort_by(|a, b| a.total_cmp(b));
    roots.dedup_by(|a, b| (*a - *b).abs() < 1e-10);
    roots
}

#[cfg(test)]
mod tests {
    use super::*;

    fn check_quartic_residual(coeffs: &[f64; 5], roots: &[f64], tol: f64) {
        for &x in roots {
            let val = coeffs[4] * x.powi(4)
                + coeffs[3] * x.powi(3)
                + coeffs[2] * x.powi(2)
                + coeffs[1] * x
                + coeffs[0];
            assert!(
                val.abs() < tol,
                "large residual: f({x}) = {val}, coeffs = {coeffs:?}"
            );
        }
    }

    fn check_cubic_residual(coeffs: &[f64; 4], roots: &[f64], tol: f64) {
        for &x in roots {
            let val = coeffs[3] * x.powi(3) + coeffs[2] * x.powi(2) + coeffs[1] * x + coeffs[0];
            assert!(
                val.abs() < tol,
                "large residual: f({x}) = {val}, coeffs = {coeffs:?}"
            );
        }
    }

    #[test]
    fn cubic_three_real_roots() {
        let coeffs = [-6.0, 11.0, -6.0, 1.0];
        let roots = solve_cubic(coeffs);
        assert_eq!(roots.len(), 3);
        check_cubic_residual(&coeffs, &roots, 1e-10);
    }

    #[test]
    fn cubic_one_real_root() {
        let coeffs = [1.0, 1.0, 0.0, 1.0];
        let roots = solve_cubic(coeffs);
        assert_eq!(roots.len(), 1);
        check_cubic_residual(&coeffs, &roots, 1e-10);
    }

    #[test]
    fn quartic_four_distinct_roots() {
        let coeffs = [24.0, -50.0, 35.0, -10.0, 1.0];
        let roots = solve_quartic(coeffs);
        assert_eq!(roots.len(), 4, "expected 4 roots: {roots:?}");
        check_quartic_residual(&coeffs, &roots, 1e-10);
        for (i, expected) in [1.0, 2.0, 3.0, 4.0].iter().enumerate() {
            assert!(
                (roots[i] - expected).abs() < 1e-8,
                "root[{i}] = {} != {expected}",
                roots[i]
            );
        }
    }

    #[test]
    fn quartic_double_roots() {
        let coeffs = [4.0, -12.0, 13.0, -6.0, 1.0];
        let roots = solve_quartic(coeffs);
        check_quartic_residual(&coeffs, &roots, 1e-10);
        assert!(
            roots.len() >= 2,
            "expected at least 2 distinct roots for double-root case: {roots:?}"
        );
    }

    #[test]
    fn quartic_two_real_two_complex() {
        let coeffs = [4.0, 0.0, -5.0, 0.0, 1.0];
        let roots = solve_quartic(coeffs);
        assert_eq!(roots.len(), 4, "expected 4 roots: {roots:?}");
        check_quartic_residual(&coeffs, &roots, 1e-10);
        let expected = [-2.0, -1.0, 1.0, 2.0];
        for (i, e) in expected.iter().enumerate() {
            assert!(
                (roots[i] - e).abs() < 1e-8,
                "root[{i}] = {} != {e}",
                roots[i]
            );
        }
    }

    #[test]
    fn quartic_ray_torus_intersection() {
        let coeffs = [8.1081, -23.28, 21.82, -8.0, 1.0];
        let roots = solve_quartic(coeffs);
        assert_eq!(
            roots.len(),
            4,
            "expected 4 roots for torus intersection: {roots:?}"
        );
        check_quartic_residual(&coeffs, &roots, 1e-8);
        let expected = [0.7, 1.3, 2.7, 3.3];
        for (i, e) in expected.iter().enumerate() {
            assert!(
                (roots[i] - e).abs() < 1e-4,
                "root[{i}] = {} != {e}",
                roots[i]
            );
        }
    }

    #[test]
    fn quartic_grazing_angle() {
        let coeffs = [1.0, 0.0, -2.0, 0.0, 1.0];
        let roots = solve_quartic(coeffs);
        check_quartic_residual(&coeffs, &roots, 1e-10);
        assert!(
            roots.len() >= 2,
            "expected at least 2 roots for grazing case: {roots:?}"
        );
    }

    #[test]
    fn quartic_extreme_ratio_small() {
        let big_r = 10.0_f64;
        let small_r = 0.05_f64;
        let k = big_r * big_r - small_r * small_r;
        let lin = 2.0 * k - 4.0 * big_r * big_r;
        let c4 = 1.0;
        let c3 = -48.0;
        let c2 = 864.0 + lin;
        let c1 = -6912.0 - 24.0 * lin;
        let c0 = 20736.0 + 144.0 * lin + k * k;
        let coeffs = [c0, c1, c2, c3, c4];
        let roots = solve_quartic(coeffs);
        check_quartic_residual(&coeffs, &roots, 1e-6);
        assert!(
            roots.len() == 4,
            "expected 4 roots for r/R < 0.01 case: {roots:?}"
        );
    }

    #[test]
    fn quartic_extreme_ratio_large() {
        let k = 0.0975_f64;
        let c4 = 1.0;
        let c3 = -8.0;
        let c2 = 20.0 + 2.0 * k;
        let c1 = -16.0 - 8.0 * k;
        let c0 = 8.0 * k + k * k;
        let coeffs = [c0, c1, c2, c3, c4];
        let roots = solve_quartic(coeffs);
        assert!(
            roots.len() == 4,
            "expected 4 roots for r/R > 0.9 case: {roots:?}"
        );
        check_quartic_residual(&coeffs, &roots, 1e-8);
    }
}
