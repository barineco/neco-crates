use neco_array2::Array2;

/// Return the index of the cell nearest to `(tx, ty)` on a 2D grid.
pub fn find_nearest(x: &Array2<f64>, y: &Array2<f64>, tx: f64, ty: f64) -> (usize, usize) {
    let (nx, ny) = x.dim();
    let mut best = (0, 0);
    let mut best_distance = f64::INFINITY;
    for i in 0..nx {
        for j in 0..ny {
            let distance = (x[[i, j]] - tx).powi(2) + (y[[i, j]] - ty).powi(2);
            if distance < best_distance {
                best_distance = distance;
                best = (i, j);
            }
        }
    }
    best
}

/// Build a normalized cosine-taper spatial mask.
pub fn build_spatial_mask(
    x: &Array2<f64>,
    y: &Array2<f64>,
    hx: f64,
    hy: f64,
    width: f64,
    interior: Option<&Array2<bool>>,
) -> Array2<f64> {
    let (nx, ny) = x.dim();
    let mut mask = Array2::zeros((nx, ny));
    let mut sum = 0.0;
    for i in 0..nx {
        for j in 0..ny {
            if let Some(active) = interior {
                if !active[[i, j]] {
                    continue;
                }
            }
            let distance = ((x[[i, j]] - hx).powi(2) + (y[[i, j]] - hy).powi(2)).sqrt();
            if distance < width {
                let value = 0.5 * (1.0 + (std::f64::consts::PI * distance / width).cos());
                mask[[i, j]] = value;
                sum += value;
            }
        }
    }
    if sum > 0.0 {
        for value in mask.iter_mut() {
            *value /= sum;
        }
    }
    mask
}

/// Collect active interior cells that are at least `margin` cells from the border.
pub fn collect_interior(interior: &Array2<bool>, margin: usize) -> Vec<(usize, usize)> {
    let (nx, ny) = interior.dim();
    let mut points = Vec::new();
    for i in margin..nx.saturating_sub(margin) {
        for j in margin..ny.saturating_sub(margin) {
            if interior[[i, j]] {
                points.push((i, j));
            }
        }
    }
    points
}

/// Common Hertz contact update for beater-based excitation.
#[derive(Debug, Clone)]
pub struct HertzContact {
    pub beater_x: f64,
    pub beater_v: f64,
    beater_mass: f64,
    k_hertz: f64,
    alpha_hertz: f64,
    contact_ended: bool,
}

impl HertzContact {
    pub fn new(beater_mass: f64, k_hertz: f64, alpha_hertz: f64, v0: f64) -> Self {
        Self {
            beater_x: 0.0,
            beater_v: v0,
            beater_mass,
            k_hertz,
            alpha_hertz,
            contact_ended: false,
        }
    }

    /// Advance one step and return the contact force.
    pub fn step(&mut self, w_surface: f64, dt: f64) -> f64 {
        if self.contact_ended {
            return 0.0;
        }
        let delta = self.beater_x - w_surface;
        if delta > 0.0 {
            let force = self.k_hertz * delta.powf(self.alpha_hertz);
            let acceleration = -force / self.beater_mass;
            self.beater_v += acceleration * dt;
            self.beater_x += self.beater_v * dt;
            force
        } else {
            self.beater_x += self.beater_v * dt;
            if self.beater_x > 1e-12 && delta < -1e-10 {
                self.contact_ended = true;
            }
            0.0
        }
    }

    pub fn energy(&self) -> f64 {
        0.5 * self.beater_mass * self.beater_v * self.beater_v
    }

    pub fn contact_ended(&self) -> bool {
        self.contact_ended
    }

    pub fn set_contact_ended(&mut self, ended: bool) {
        self.contact_ended = ended;
    }
}
