/// Backend abstraction for CheFSI block operations.
pub trait ChefsiBackend {
    fn spmv_k(&self, x: &[f32], z: &mut [f32], n: usize, m: usize);
    fn diag_mul_m_inv(&self, z: &mut [f32], n: usize, m: usize);
    fn axpy(&self, alpha: f32, x: &[f32], y: &mut [f32]);
    fn scale(&self, alpha: f32, y: &mut [f32]);
    fn copy(&self, src: &[f32], dst: &mut [f32]);
}

/// Parameters for the low-pass Chebyshev filter.
pub struct FilterParams {
    pub lambda_cutoff: f64,
    pub lambda_min: f64,
    pub lambda_max: f64,
    pub degree: usize,
}

fn apply_a(backend: &dyn ChefsiBackend, v: &[f32], out: &mut [f32], n: usize, m: usize) {
    backend.spmv_k(v, out, n, m);
    backend.diag_mul_m_inv(out, n, m);
}

/// Apply the standard low-pass Chebyshev filter from Li, Xi, Saad (2016).
pub fn apply_chebyshev_filter(
    backend: &dyn ChefsiBackend,
    params: &FilterParams,
    y: &mut [f32],
    n: usize,
    m: usize,
) {
    assert_eq!(y.len(), n * m);
    assert!(params.degree >= 1, "CheFSI filter degree must be >= 1");

    let e = (params.lambda_max - params.lambda_cutoff) / 2.0;
    let c_center = (params.lambda_max + params.lambda_cutoff) / 2.0;
    if e <= 0.0 {
        return;
    }

    let sigma_1 = e / (params.lambda_min - c_center);
    let len = n * m;
    let mut y_prev = vec![0.0f32; len];
    let mut y_cur = vec![0.0f32; len];
    let mut av = vec![0.0f32; len];

    backend.copy(y, &mut y_prev);
    apply_a(backend, y, &mut av, n, m);
    backend.axpy(-(c_center as f32), &y_prev, &mut av);
    backend.copy(&av, &mut y_cur);
    backend.scale((sigma_1 / e) as f32, &mut y_cur);

    if params.degree == 1 {
        backend.copy(&y_cur, y);
        return;
    }

    let mut sigma_prev = sigma_1;
    for _ in 2..=params.degree {
        let sigma_j = 1.0 / (2.0 * c_center / (e * sigma_1) - sigma_prev);
        apply_a(backend, &y_cur, &mut av, n, m);

        let c1 = (2.0 * sigma_j / e) as f32;
        let c2 = (2.0 * sigma_j * c_center / e) as f32;
        let c3 = (sigma_j * sigma_prev) as f32;

        backend.copy(&av, y);
        backend.scale(c1, y);
        backend.axpy(-c2, &y_cur, y);
        backend.axpy(-c3, &y_prev, y);

        backend.copy(&y_cur, &mut y_prev);
        backend.copy(y, &mut y_cur);
        sigma_prev = sigma_j;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::chefsi::cpu_backend::CpuBackend;
    use neco_sparse::CsrMat;

    #[test]
    fn chebyshev_filter_amplifies_low_modes() {
        let n = 20;
        let offsets: Vec<usize> = (0..=n).collect();
        let indices: Vec<usize> = (0..n).collect();
        let k_values: Vec<f64> = (1..=n).map(|i| i as f64).collect();
        let k = CsrMat::try_from_csr_data(n, n, offsets, indices, k_values).unwrap();
        let m_diag = vec![1.0; n];

        let mut y = vec![1.0f32; n];
        let params = FilterParams {
            lambda_cutoff: 10.0,
            lambda_min: 0.5,
            lambda_max: 21.0,
            degree: 15,
        };
        let backend = CpuBackend::new(&k, &m_diag);
        apply_chebyshev_filter(&backend, &params, &mut y, n, 1);

        let wanted: f32 = y[0..10].iter().map(|value| value * value).sum();
        let unwanted: f32 = y[10..].iter().map(|value| value * value).sum();
        assert!(wanted > unwanted * 10.0);
    }
}
