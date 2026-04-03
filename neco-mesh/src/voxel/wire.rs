use std::f64::consts::PI;

use super::grid::FillFractionGrid;

#[derive(Debug, Clone)]
struct VoxelAccum {
    nx: usize,
    ny: usize,
    nz: usize,
    values: Vec<f64>,
}

impl VoxelAccum {
    fn zeros(nx: usize, ny: usize, nz: usize) -> Self {
        let len = nx
            .checked_mul(ny)
            .and_then(|value| value.checked_mul(nz))
            .expect("voxel grid is too large");
        Self {
            nx,
            ny,
            nz,
            values: vec![0.0; len],
        }
    }

    #[inline]
    fn index(&self, i: usize, j: usize, k: usize) -> usize {
        debug_assert!(i < self.nx);
        debug_assert!(j < self.ny);
        debug_assert!(k < self.nz);
        (i * self.ny + j) * self.nz + k
    }

    #[inline]
    fn add(&mut self, i: usize, j: usize, k: usize, value: f64) {
        let index = self.index(i, j, k);
        self.values[index] += value;
    }

    fn into_fill_fraction_grid(mut self, max_value: f64) -> FillFractionGrid {
        for value in &mut self.values {
            *value = value.min(max_value);
        }
        FillFractionGrid {
            nx: self.nx,
            ny: self.ny,
            nz: self.nz,
            values: self.values,
        }
    }
}

/// A rod-like wire geometry described by a centerline and a finite diameter.
#[derive(Debug, Clone)]
pub struct RodGeometry {
    pub diameter: f64,
    pub centerline: Vec<[f64; 3]>,
}

impl RodGeometry {
    /// Rasterize the centerline into a fill-fraction grid.
    ///
    /// `cx`, `cy`, and `cz` denote the domain-center coordinates.
    #[allow(clippy::too_many_arguments)]
    pub fn fill_fraction(
        &self,
        dx: f64,
        nx: usize,
        ny: usize,
        nz: usize,
        cx: f64,
        cy: f64,
        cz: f64,
    ) -> FillFractionGrid {
        let mut fill = VoxelAccum::zeros(nx, ny, nz);
        let radius = self.diameter / 2.0;
        let rod_area = PI * radius * radius;

        for segment in self.centerline.windows(2) {
            let p0 = segment[0];
            let p1 = segment[1];
            let segment_len = vec_len(sub(p1, p0));
            if segment_len < 1e-15 {
                continue;
            }
            let dir = scale(sub(p1, p0), 1.0 / segment_len);

            let n_samples = (segment_len / (dx * 0.1)).ceil() as usize;
            let ds = segment_len / n_samples as f64;

            for sample_index in 0..n_samples {
                let t = (sample_index as f64 + 0.5) / n_samples as f64;
                let point = lerp(p0, p1, t);
                distribute_cross_section(
                    &mut fill, point, dir, radius, rod_area, ds, dx, nx, ny, nz, cx, cy, cz,
                );
            }
        }

        fill.into_fill_fraction_grid(1.0)
    }
}

#[allow(clippy::too_many_arguments)]
fn distribute_cross_section(
    fill: &mut VoxelAccum,
    center: [f64; 3],
    dir: [f64; 3],
    radius: f64,
    rod_area: f64,
    ds: f64,
    dx: f64,
    nx: usize,
    ny: usize,
    nz: usize,
    cx: f64,
    cy: f64,
    cz: f64,
) {
    let r_cells: i32 = ((radius / dx).ceil() as i64 + 1)
        .try_into()
        .expect("cell radius fits in i32");
    let dx3 = dx * dx * dx;
    let half_diag = dx * 0.5 * 2.0_f64.sqrt();
    let cutoff = radius + half_diag;

    let ci = floor_to_i32_clamped((center[0] + cx) / dx);
    let cj = floor_to_i32_clamped((center[1] + cy) / dx);
    let ck = floor_to_i32_clamped((center[2] + cz) / dx);
    let nx_i32 = i32::try_from(nx).expect("grid nx fits in i32");
    let ny_i32 = i32::try_from(ny).expect("grid ny fits in i32");
    let nz_i32 = i32::try_from(nz).expect("grid nz fits in i32");

    let cell_weight = |i: i32, j: i32, k: i32| -> f64 {
        let cell_cx = i as f64 * dx + dx * 0.5 - cx;
        let cell_cy = j as f64 * dx + dx * 0.5 - cy;
        let cell_cz = k as f64 * dx + dx * 0.5 - cz;
        let diff = [
            cell_cx - center[0],
            cell_cy - center[1],
            cell_cz - center[2],
        ];
        let d_along = diff[0] * dir[0] + diff[1] * dir[1] + diff[2] * dir[2];
        if d_along.abs() > dx {
            return 0.0;
        }
        let d_perp_sq =
            diff[0] * diff[0] + diff[1] * diff[1] + diff[2] * diff[2] - d_along * d_along;
        let d_perp = d_perp_sq.max(0.0).sqrt();
        if d_perp >= cutoff {
            return 0.0;
        }
        if d_perp <= radius - half_diag {
            1.0
        } else {
            1.0 - ((d_perp - (radius - half_diag).max(0.0))
                / (cutoff - (radius - half_diag).max(0.0)))
            .clamp(0.0, 1.0)
        }
    };

    let mut total_w = 0.0;
    for di in -r_cells..=r_cells {
        let i = ci.saturating_add(di);
        if i < 0 || i >= nx_i32 {
            continue;
        }
        for dj in -r_cells..=r_cells {
            let j = cj.saturating_add(dj);
            if j < 0 || j >= ny_i32 {
                continue;
            }
            for dk in -r_cells..=r_cells {
                let k = ck.saturating_add(dk);
                if k < 0 || k >= nz_i32 {
                    continue;
                }
                total_w += cell_weight(i, j, k);
            }
        }
    }

    if total_w < 1e-30 {
        return;
    }
    let scale_factor = rod_area * ds / (total_w * dx3);

    for di in -r_cells..=r_cells {
        let i = ci.saturating_add(di);
        if i < 0 || i >= nx_i32 {
            continue;
        }
        for dj in -r_cells..=r_cells {
            let j = cj.saturating_add(dj);
            if j < 0 || j >= ny_i32 {
                continue;
            }
            for dk in -r_cells..=r_cells {
                let k = ck.saturating_add(dk);
                if k < 0 || k >= nz_i32 {
                    continue;
                }
                let weight = cell_weight(i, j, k);
                if weight > 0.0 {
                    fill.add(i as usize, j as usize, k as usize, weight * scale_factor);
                }
            }
        }
    }
}

fn floor_to_i32_clamped(value: f64) -> i32 {
    let floored = value.floor();
    if !floored.is_finite() {
        return if floored.is_sign_negative() {
            i32::MIN
        } else {
            i32::MAX
        };
    }
    if floored <= i32::MIN as f64 {
        i32::MIN
    } else if floored >= i32::MAX as f64 {
        i32::MAX
    } else {
        let floored_i64 = floored as i64;
        i32::try_from(floored_i64).expect("floored cell index fits in i32")
    }
}

/// An equilateral triangular frame described in the `xy` plane.
#[derive(Debug, Clone)]
pub struct TriangleGeometry {
    pub side_length: f64,
    pub rod_diameter: f64,
    pub corner_radius: f64,
    pub gap_width: f64,
}

impl TriangleGeometry {
    /// Build an equilateral-triangle centerline in the `xy` plane.
    ///
    /// The centroid is placed at the origin and `gap_width` opens the bottom edge.
    pub fn to_rod_geometry(&self) -> RodGeometry {
        let l = self.side_length;
        let r = self.corner_radius;

        let h = l * 3.0_f64.sqrt() / 2.0;
        let cy = h / 3.0;

        let v_bl = [-l / 2.0, -cy, 0.0];
        let v_br = [l / 2.0, -cy, 0.0];
        let v_top = [0.0, h - cy, 0.0];

        let mut points = Vec::new();
        let half_gap = self.gap_width / 2.0;

        let gap_right = [half_gap, -cy, 0.0];
        points.push(gap_right);

        self.add_edge_and_corner(&mut points, gap_right, v_br, v_top, r);
        self.add_edge_and_corner(&mut points, v_br, v_top, v_bl, r);
        self.add_edge_and_corner(&mut points, v_top, v_bl, v_br, r);

        let gap_left = [-half_gap, -cy, 0.0];
        let dir = normalize(sub(gap_left, v_bl));
        let from_corner_end = add(v_bl, scale(dir, r.min(vec_len(sub(gap_left, v_bl)))));
        points.push(from_corner_end);
        points.push(gap_left);

        RodGeometry {
            diameter: self.rod_diameter,
            centerline: points,
        }
    }

    fn add_edge_and_corner(
        &self,
        points: &mut Vec<[f64; 3]>,
        from: [f64; 3],
        vertex: [f64; 3],
        next: [f64; 3],
        r: f64,
    ) {
        let dir_in = normalize(sub(vertex, from));
        let dir_out = normalize(sub(next, vertex));

        let edge_len_in = vec_len(sub(vertex, from));
        let edge_len_out = vec_len(sub(next, vertex));
        let r_eff = r.min(edge_len_in * 0.4).min(edge_len_out * 0.4);

        let arc_start = sub(vertex, scale(dir_in, r_eff));
        let arc_end = add(vertex, scale(dir_out, r_eff));

        points.push(arc_start);

        let n_arc = 8;
        let cos_angle = dot(dir_in, dir_out).clamp(-1.0, 1.0);
        let angle = cos_angle.acos();
        let half_angle = angle / 2.0;
        let max_bulge = r_eff * (1.0 - half_angle.cos());

        for i in 1..n_arc {
            let t = i as f64 / n_arc as f64;
            let p = lerp(arc_start, arc_end, t);
            let mid = lerp(arc_start, arc_end, 0.5);
            let bulge_dir = normalize(sub(vertex, mid));
            let bulge = max_bulge * (1.0 - (2.0 * t - 1.0).powi(2));
            let p = add(p, scale(bulge_dir, bulge));
            points.push(p);
        }

        points.push(arc_end);
    }
}

#[cfg_attr(feature = "serde", derive(serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(tag = "type", rename_all = "snake_case"))]
#[derive(Debug, Clone)]
pub enum GeometryConfig {
    Rod {
        diameter: f64,
        centerline: Vec<[f64; 3]>,
    },
    Triangle {
        side_length: f64,
        rod_diameter: f64,
        #[cfg_attr(feature = "serde", serde(default = "default_corner_radius"))]
        corner_radius: f64,
        #[cfg_attr(feature = "serde", serde(default))]
        gap_width: f64,
    },
}

#[cfg(feature = "serde")]
fn default_corner_radius() -> f64 {
    0.010
}

impl GeometryConfig {
    /// Convert a declarative geometry description into a rod centerline.
    pub fn to_rod_geometry(&self) -> RodGeometry {
        match self {
            GeometryConfig::Rod {
                diameter,
                centerline,
            } => RodGeometry {
                diameter: *diameter,
                centerline: centerline.clone(),
            },
            GeometryConfig::Triangle {
                side_length,
                rod_diameter,
                corner_radius,
                gap_width,
            } => {
                let triangle = TriangleGeometry {
                    side_length: *side_length,
                    rod_diameter: *rod_diameter,
                    corner_radius: *corner_radius,
                    gap_width: *gap_width,
                };
                triangle.to_rod_geometry()
            }
        }
    }
}

fn sub(a: [f64; 3], b: [f64; 3]) -> [f64; 3] {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}

fn add(a: [f64; 3], b: [f64; 3]) -> [f64; 3] {
    [a[0] + b[0], a[1] + b[1], a[2] + b[2]]
}

fn scale(a: [f64; 3], s: f64) -> [f64; 3] {
    [a[0] * s, a[1] * s, a[2] * s]
}

fn dot(a: [f64; 3], b: [f64; 3]) -> f64 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}

fn vec_len(a: [f64; 3]) -> f64 {
    dot(a, a).sqrt()
}

fn normalize(a: [f64; 3]) -> [f64; 3] {
    let l = vec_len(a);
    if l < 1e-15 {
        [0.0, 0.0, 0.0]
    } else {
        scale(a, 1.0 / l)
    }
}

fn lerp(a: [f64; 3], b: [f64; 3], t: f64) -> [f64; 3] {
    [
        a[0] + (b[0] - a[0]) * t,
        a[1] + (b[1] - a[1]) * t,
        a[2] + (b[2] - a[2]) * t,
    ]
}
