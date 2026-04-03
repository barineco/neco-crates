//! Acoustic radiation power estimators for vibrating surfaces.

#[derive(Debug, Clone)]
pub struct RadiationCalculator {
    rho_air: f64,
    c_air: f64,
}

impl RadiationCalculator {
    pub fn new() -> Self {
        Self {
            rho_air: 1.225,
            c_air: 343.0,
        }
    }

    pub fn with_params(rho_air: f64, c_air: f64) -> Self {
        Self { rho_air, c_air }
    }

    /// Compute radiated power from active sample points and their velocities.
    ///
    /// `points` and `values` must have identical length and corresponding order.
    /// `cell_area` is the area associated with each active sample.
    pub fn radiated_power(
        &self,
        points: &[[f64; 2]],
        values: &[f64],
        cell_area: f64,
        freq_dominant: f64,
    ) -> f64 {
        assert_eq!(points.len(), values.len());

        let omega = 2.0 * std::f64::consts::PI * freq_dominant;
        let k = omega / self.c_air;
        let prefactor = self.rho_air * omega * omega / (4.0 * std::f64::consts::PI * self.c_air);

        let mut power = 0.0;
        for a in 0..values.len() {
            let va = values[a];
            power += va * va * cell_area * cell_area;
            for b in a + 1..values.len() {
                let dx = points[a][0] - points[b][0];
                let dy = points[a][1] - points[b][1];
                let dist = (dx * dx + dy * dy).sqrt();
                let kr = k * dist;
                let sinc = if kr < 1e-10 { 1.0 } else { kr.sin() / kr };
                power += 2.0 * va * values[b] * sinc * cell_area * cell_area;
            }
        }

        prefactor * power
    }

    pub fn modal_efficiency(&self, radius: f64, m: i32, freq: f64) -> f64 {
        let k = 2.0 * std::f64::consts::PI * freq / self.c_air;
        let ka = k * radius;
        let sigma_base = match m.unsigned_abs() {
            0 => 1.0,
            1 => 0.5,
            _ => 0.25,
        };
        sigma_base * (ka * ka).min(1.0)
    }
}

impl Default for RadiationCalculator {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg_attr(feature = "serde", derive(serde::Deserialize))]
#[derive(Debug, Clone)]
pub struct RadiationParams {
    pub rho_air: f64,
    pub c_air: f64,
    pub max_modes: usize,
}

#[derive(Debug, Clone)]
struct ModeData {
    phi: Vec<f64>,
    r_factor: f64,
}

/// Modal radiation estimator for simply-supported rectangular plates.
#[derive(Debug, Clone)]
pub struct ModalRadiationCalculator {
    modes: Vec<ModeData>,
    da: f64,
    norm: f64,
    n_active: usize,
}

impl ModalRadiationCalculator {
    pub fn new(
        params: &RadiationParams,
        nx: usize,
        ny: usize,
        dx: f64,
        active_cells: &[(usize, usize)],
        d_val: f64,
        rho_h: f64,
    ) -> Self {
        use std::f64::consts::PI;

        let da = dx * dx;
        let lx_eff = (nx as f64 - 3.0) * dx;
        let ly_eff = (ny as f64 - 3.0) * dx;
        let norm = 4.0 / (lx_eff * ly_eff);

        let active_points: Vec<[f64; 2]> = active_cells
            .iter()
            .map(|&(i, j)| centered_point(i, j, nx, ny, dx))
            .collect();

        let n_cells = active_cells.len();
        let mut dist_table = vec![0.0_f64; n_cells * (n_cells + 1) / 2];
        let mut dist_idx = 0;
        for a in 0..n_cells {
            for b in a..n_cells {
                if a == b {
                    dist_table[dist_idx] = 0.0;
                } else {
                    let dx = active_points[a][0] - active_points[b][0];
                    let dy = active_points[a][1] - active_points[b][1];
                    dist_table[dist_idx] = (dx * dx + dy * dy).sqrt();
                }
                dist_idx += 1;
            }
        }

        let m_max = (nx - 3) / 2;
        let n_max = (ny - 3) / 2;
        let coeff = (d_val / rho_h).sqrt();

        let mut mode_list: Vec<(usize, usize, f64)> = Vec::new();
        for m in 1..=m_max {
            for n in 1..=n_max {
                let kx = m as f64 * PI / lx_eff;
                let ky = n as f64 * PI / ly_eff;
                let omega_mn = coeff * (kx * kx + ky * ky);
                mode_list.push((m, n, omega_mn / (2.0 * PI)));
            }
        }
        mode_list.sort_by(|a, b| a.2.total_cmp(&b.2));
        mode_list.truncate(params.max_modes);

        let mut modes = Vec::with_capacity(mode_list.len());
        for &(m, n, f_mn) in &mode_list {
            let phi: Vec<f64> = active_cells
                .iter()
                .map(|&(i, j)| {
                    (m as f64 * PI * (i as f64 - 1.0) / (nx as f64 - 3.0)).sin()
                        * (n as f64 * PI * (j as f64 - 1.0) / (ny as f64 - 3.0)).sin()
                })
                .collect();

            let omega = 2.0 * PI * f_mn;
            let k = omega / params.c_air;
            let prefactor = params.rho_air * omega * omega / (4.0 * PI * params.c_air);

            let mut sum = 0.0;
            let mut dist_idx = 0;
            for a in 0..n_cells {
                let phi_a = phi[a];
                for (b, &phi_b) in phi.iter().enumerate().take(n_cells).skip(a) {
                    let r = dist_table[dist_idx];
                    dist_idx += 1;
                    let kr = k * r;
                    let sinc = if kr < 1e-10 { 1.0 } else { kr.sin() / kr };
                    let contrib = phi_a * phi_b * sinc;
                    if a == b {
                        sum += contrib;
                    } else {
                        sum += 2.0 * contrib;
                    }
                }
            }

            modes.push(ModeData {
                phi,
                r_factor: prefactor * sum * da * da,
            });
        }

        Self {
            modes,
            da,
            norm,
            n_active: n_cells,
        }
    }

    pub fn radiated_power(&self, active_values: &[f64]) -> f64 {
        assert_eq!(active_values.len(), self.n_active);
        self.modes
            .iter()
            .map(|mode| {
                let mut a_mn = 0.0;
                for (value, phi) in active_values.iter().zip(mode.phi.iter()) {
                    a_mn += value * phi;
                }
                a_mn *= self.da * self.norm;
                mode.r_factor * a_mn * a_mn
            })
            .sum()
    }

    pub fn num_modes(&self) -> usize {
        self.modes.len()
    }
}

fn centered_point(i: usize, j: usize, nx: usize, ny: usize, dx: f64) -> [f64; 2] {
    let cx = (nx / 2) as f64 * dx;
    let cy = (ny / 2) as f64 * dx;
    [i as f64 * dx - cx, j as f64 * dx - cy]
}
