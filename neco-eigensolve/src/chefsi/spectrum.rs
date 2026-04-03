use neco_sparse::CsrMat;

use crate::dense::symmetric_dense_kernel;

/// Estimated spectral window.
pub struct SpectralWindow {
    pub lambda_a: f64,
    pub lambda_b: f64,
    pub estimated_modes: usize,
}

#[inline]
fn csr_matvec_into(y: &mut [f64], a: &CsrMat<f64>, x: &[f64]) {
    let offsets = a.row_offsets();
    let cols = a.col_indices();
    let vals = a.values();
    for row in 0..a.nrows() {
        let start = offsets[row];
        let end = offsets[row + 1];
        let mut sum = 0.0;
        for pos in start..end {
            sum += vals[pos] * x[cols[pos]];
        }
        y[row] = sum;
    }
}

/// Estimate `(lambda_min, lambda_max)` for `A = M_L^-1 K` with Lanczos.
pub fn estimate_spectral_bounds(
    k_mat: &CsrMat<f64>,
    m_diag: &[f64],
    max_iter: usize,
) -> (f64, f64) {
    let n = k_mat.nrows();
    assert_eq!(n, m_diag.len());
    let max_iter = max_iter.min(n);

    let mut v: Vec<f64> = (0..n).map(|i| ((i * 7 + 13) % 97) as f64 - 48.0).collect();
    let ml_norm = |vec: &[f64]| {
        vec.iter()
            .zip(m_diag)
            .map(|(value, mass)| value * value * mass)
            .sum::<f64>()
            .sqrt()
    };

    let norm_v = ml_norm(&v);
    for value in &mut v {
        *value /= norm_v;
    }

    let mut alphas = Vec::with_capacity(max_iter);
    let mut betas = Vec::with_capacity(max_iter);
    let mut v_prev = vec![0.0; n];
    let mut w = vec![0.0; n];
    let mut beta_prev = 0.0;

    for _ in 0..max_iter {
        csr_matvec_into(&mut w, k_mat, &v);
        for i in 0..n {
            w[i] /= m_diag[i];
        }

        let alpha = v
            .iter()
            .zip(&w)
            .zip(m_diag)
            .map(|((vi, wi), mi)| vi * wi * mi)
            .sum::<f64>();
        alphas.push(alpha);

        for i in 0..n {
            w[i] -= alpha * v[i] + beta_prev * v_prev[i];
        }

        let beta = ml_norm(&w);
        if beta < 1e-14 {
            break;
        }
        betas.push(beta);

        v_prev.copy_from_slice(&v);
        for i in 0..n {
            v[i] = w[i] / beta;
        }
        beta_prev = beta;
    }

    if alphas.is_empty() {
        return (0.0, 1.0);
    }

    let eigen = symmetric_dense_kernel().symmetric_tridiagonal_eigen(&alphas, &betas);
    let lambda_min = eigen
        .eigenvalues
        .iter()
        .copied()
        .min_by(|a, b| a.total_cmp(b))
        .unwrap_or(0.0);
    let lambda_max = eigen
        .eigenvalues
        .iter()
        .copied()
        .max_by(|a, b| a.total_cmp(b))
        .unwrap_or(1.0);

    (lambda_min * 0.9, lambda_max * 1.1)
}

/// Plan overlapping equal-width windows.
pub fn plan_windows(
    lambda_min: f64,
    lambda_max: f64,
    target_modes_per_window: usize,
    _density_fn: Option<&dyn Fn(f64) -> f64>,
) -> Vec<SpectralWindow> {
    let total_range = lambda_max - lambda_min;
    if total_range <= 0.0 {
        return vec![SpectralWindow {
            lambda_a: lambda_min,
            lambda_b: lambda_max,
            estimated_modes: target_modes_per_window,
        }];
    }

    let n_windows = (total_range / target_modes_per_window as f64)
        .ceil()
        .max(1.0) as usize;
    let effective_width = total_range / n_windows as f64;
    let window_width = effective_width / 0.9;
    let overlap = window_width * 0.1;

    let mut windows = Vec::with_capacity(n_windows);
    for i in 0..n_windows {
        let lambda_a = if i == 0 {
            lambda_min
        } else {
            lambda_min + i as f64 * effective_width - overlap
        };
        let lambda_b = if i == n_windows - 1 {
            lambda_max
        } else {
            lambda_a + window_width
        };
        windows.push(SpectralWindow {
            lambda_a,
            lambda_b,
            estimated_modes: target_modes_per_window,
        });
    }
    windows
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plan_windows_covers_requested_range() {
        let windows = plan_windows(1.0, 1000.0, 100, None);
        assert!((windows.first().unwrap().lambda_a - 1.0).abs() < 1e-10);
        assert!((windows.last().unwrap().lambda_b - 1000.0).abs() < 1e-10);
        assert!(windows
            .windows(2)
            .all(|pair| pair[0].lambda_b >= pair[1].lambda_a));
    }
}
