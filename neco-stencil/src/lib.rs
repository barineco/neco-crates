//! Finite-difference stencil operators on uniform 2D grids.
use core::fmt;

#[cfg(feature = "rayon")]
const RAYON_THRESHOLD: usize = 10_000;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StencilError {
    GridTooLarge {
        nx: usize,
        ny: usize,
    },
    InvalidLength {
        name: &'static str,
        expected: usize,
        actual: usize,
    },
}

impl fmt::Display for StencilError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::GridTooLarge { nx, ny } => {
                write!(f, "grid dimensions nx={nx}, ny={ny} are too large")
            }
            Self::InvalidLength {
                name,
                expected,
                actual,
            } => write!(f, "{name} must have length {expected}, got {actual}"),
        }
    }
}

impl std::error::Error for StencilError {}

#[inline]
fn idx(ny: usize, i: usize, j: usize) -> usize {
    i * ny + j
}

#[inline]
fn grid_len(nx: usize, ny: usize) -> Result<usize, StencilError> {
    nx.checked_mul(ny)
        .ok_or(StencilError::GridTooLarge { nx, ny })
}

fn assert_len(name: &'static str, data: &[f64], nx: usize, ny: usize) -> Result<(), StencilError> {
    let expected = grid_len(nx, ny)?;
    if data.len() != expected {
        return Err(StencilError::InvalidLength {
            name,
            expected,
            actual: data.len(),
        });
    }
    Ok(())
}

fn assert_len_mut(
    name: &'static str,
    data: &mut [f64],
    nx: usize,
    ny: usize,
) -> Result<(), StencilError> {
    let expected = grid_len(nx, ny)?;
    if data.len() != expected {
        return Err(StencilError::InvalidLength {
            name,
            expected,
            actual: data.len(),
        });
    }
    Ok(())
}

#[inline]
pub fn laplacian(
    u: &[f64],
    nx: usize,
    ny: usize,
    dx: f64,
    out: &mut [f64],
) -> Result<(), StencilError> {
    assert_len("u", u, nx, ny)?;
    assert_len_mut("out", out, nx, ny)?;
    out.fill(0.0);
    if nx < 3 || ny < 3 {
        return Ok(());
    }
    let inv_dx2 = 1.0 / (dx * dx);
    #[cfg(feature = "rayon")]
    if nx * ny >= RAYON_THRESHOLD {
        use rayon::prelude::*;
        let u_base = u.as_ptr() as usize;
        let out_base = out.as_mut_ptr() as usize;
        (1..nx - 1).into_par_iter().for_each(|i| {
            let u_ptr = u_base as *const f64;
            let out_ptr = out_base as *mut f64;
            for j in 1..ny - 1 {
                unsafe {
                    let center = idx(ny, i, j);
                    *out_ptr.add(center) = (*u_ptr.add(idx(ny, i + 1, j))
                        + *u_ptr.add(idx(ny, i - 1, j))
                        + *u_ptr.add(idx(ny, i, j + 1))
                        + *u_ptr.add(idx(ny, i, j - 1))
                        - 4.0 * *u_ptr.add(center))
                        * inv_dx2;
                }
            }
        });
        return Ok(());
    }
    for i in 1..nx - 1 {
        for j in 1..ny - 1 {
            let center = idx(ny, i, j);
            out[center] = (u[idx(ny, i + 1, j)]
                + u[idx(ny, i - 1, j)]
                + u[idx(ny, i, j + 1)]
                + u[idx(ny, i, j - 1)]
                - 4.0 * u[center])
                * inv_dx2;
        }
    }
    Ok(())
}

#[inline]
pub fn gradient_x(
    u: &[f64],
    nx: usize,
    ny: usize,
    dx: f64,
    out: &mut [f64],
) -> Result<(), StencilError> {
    assert_len("u", u, nx, ny)?;
    assert_len_mut("out", out, nx, ny)?;
    out.fill(0.0);
    if nx < 3 {
        return Ok(());
    }
    let inv_2dx = 1.0 / (2.0 * dx);
    for i in 1..nx - 1 {
        for j in 0..ny {
            out[idx(ny, i, j)] = (u[idx(ny, i + 1, j)] - u[idx(ny, i - 1, j)]) * inv_2dx;
        }
    }
    Ok(())
}

#[inline]
pub fn gradient_y(
    u: &[f64],
    nx: usize,
    ny: usize,
    dx: f64,
    out: &mut [f64],
) -> Result<(), StencilError> {
    assert_len("u", u, nx, ny)?;
    assert_len_mut("out", out, nx, ny)?;
    out.fill(0.0);
    if ny < 3 {
        return Ok(());
    }
    let inv_2dx = 1.0 / (2.0 * dx);
    for i in 0..nx {
        for j in 1..ny - 1 {
            out[idx(ny, i, j)] = (u[idx(ny, i, j + 1)] - u[idx(ny, i, j - 1)]) * inv_2dx;
        }
    }
    Ok(())
}

#[inline]
pub fn d2_dx2(
    u: &[f64],
    nx: usize,
    ny: usize,
    dx: f64,
    out: &mut [f64],
) -> Result<(), StencilError> {
    assert_len("u", u, nx, ny)?;
    assert_len_mut("out", out, nx, ny)?;
    out.fill(0.0);
    if nx < 3 {
        return Ok(());
    }
    let inv_dx2 = 1.0 / (dx * dx);
    for i in 1..nx - 1 {
        for j in 0..ny {
            out[idx(ny, i, j)] =
                (u[idx(ny, i + 1, j)] - 2.0 * u[idx(ny, i, j)] + u[idx(ny, i - 1, j)]) * inv_dx2;
        }
    }
    Ok(())
}

#[inline]
pub fn d2_dy2(
    u: &[f64],
    nx: usize,
    ny: usize,
    dx: f64,
    out: &mut [f64],
) -> Result<(), StencilError> {
    assert_len("u", u, nx, ny)?;
    assert_len_mut("out", out, nx, ny)?;
    out.fill(0.0);
    if ny < 3 {
        return Ok(());
    }
    let inv_dx2 = 1.0 / (dx * dx);
    for i in 0..nx {
        for j in 1..ny - 1 {
            out[idx(ny, i, j)] =
                (u[idx(ny, i, j + 1)] - 2.0 * u[idx(ny, i, j)] + u[idx(ny, i, j - 1)]) * inv_dx2;
        }
    }
    Ok(())
}

#[inline]
pub fn d2_dxdy(
    u: &[f64],
    nx: usize,
    ny: usize,
    dx: f64,
    out: &mut [f64],
) -> Result<(), StencilError> {
    assert_len("u", u, nx, ny)?;
    assert_len_mut("out", out, nx, ny)?;
    out.fill(0.0);
    if nx < 3 || ny < 3 {
        return Ok(());
    }
    let inv_4dx2 = 1.0 / (4.0 * dx * dx);
    for i in 1..nx - 1 {
        for j in 1..ny - 1 {
            out[idx(ny, i, j)] =
                (u[idx(ny, i + 1, j + 1)] - u[idx(ny, i + 1, j - 1)] - u[idx(ny, i - 1, j + 1)]
                    + u[idx(ny, i - 1, j - 1)])
                    * inv_4dx2;
        }
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
pub fn w_derivatives(
    w: &[f64],
    nx: usize,
    ny: usize,
    dx: f64,
    wx: &mut [f64],
    wy: &mut [f64],
    wxx: &mut [f64],
    wyy: &mut [f64],
    wxy: &mut [f64],
) -> Result<(), StencilError> {
    assert_len("w", w, nx, ny)?;
    assert_len_mut("wx", wx, nx, ny)?;
    assert_len_mut("wy", wy, nx, ny)?;
    assert_len_mut("wxx", wxx, nx, ny)?;
    assert_len_mut("wyy", wyy, nx, ny)?;
    assert_len_mut("wxy", wxy, nx, ny)?;
    wx.fill(0.0);
    wy.fill(0.0);
    wxx.fill(0.0);
    wyy.fill(0.0);
    wxy.fill(0.0);
    if nx < 3 || ny < 3 {
        return Ok(());
    }
    let inv_2dx = 1.0 / (2.0 * dx);
    let inv_dx2 = 1.0 / (dx * dx);
    let inv_4dx2 = 1.0 / (4.0 * dx * dx);
    #[cfg(feature = "rayon")]
    if nx * ny >= RAYON_THRESHOLD {
        use rayon::prelude::*;
        let w_base = w.as_ptr() as usize;
        let wx_base = wx.as_mut_ptr() as usize;
        let wy_base = wy.as_mut_ptr() as usize;
        let wxx_base = wxx.as_mut_ptr() as usize;
        let wyy_base = wyy.as_mut_ptr() as usize;
        let wxy_base = wxy.as_mut_ptr() as usize;
        (1..nx - 1).into_par_iter().for_each(|i| {
            let w_ptr = w_base as *const f64;
            let wx_ptr = wx_base as *mut f64;
            let wy_ptr = wy_base as *mut f64;
            let wxx_ptr = wxx_base as *mut f64;
            let wyy_ptr = wyy_base as *mut f64;
            let wxy_ptr = wxy_base as *mut f64;
            for j in 1..ny - 1 {
                unsafe {
                    let center = idx(ny, i, j);
                    let wc = *w_ptr.add(center);
                    let wp = *w_ptr.add(idx(ny, i + 1, j));
                    let wm = *w_ptr.add(idx(ny, i - 1, j));
                    let wn = *w_ptr.add(idx(ny, i, j + 1));
                    let ws = *w_ptr.add(idx(ny, i, j - 1));
                    *wx_ptr.add(center) = (wp - wm) * inv_2dx;
                    *wy_ptr.add(center) = (wn - ws) * inv_2dx;
                    *wxx_ptr.add(center) = (wp - 2.0 * wc + wm) * inv_dx2;
                    *wyy_ptr.add(center) = (wn - 2.0 * wc + ws) * inv_dx2;
                    *wxy_ptr.add(center) = (*w_ptr.add(idx(ny, i + 1, j + 1))
                        - *w_ptr.add(idx(ny, i + 1, j - 1))
                        - *w_ptr.add(idx(ny, i - 1, j + 1))
                        + *w_ptr.add(idx(ny, i - 1, j - 1)))
                        * inv_4dx2;
                }
            }
        });
        return Ok(());
    }
    for i in 1..nx - 1 {
        for j in 1..ny - 1 {
            let center = idx(ny, i, j);
            let wc = w[center];
            let wp = w[idx(ny, i + 1, j)];
            let wm = w[idx(ny, i - 1, j)];
            let wn = w[idx(ny, i, j + 1)];
            let ws = w[idx(ny, i, j - 1)];
            wx[center] = (wp - wm) * inv_2dx;
            wy[center] = (wn - ws) * inv_2dx;
            wxx[center] = (wp - 2.0 * wc + wm) * inv_dx2;
            wyy[center] = (wn - 2.0 * wc + ws) * inv_dx2;
            wxy[center] =
                (w[idx(ny, i + 1, j + 1)] - w[idx(ny, i + 1, j - 1)] - w[idx(ny, i - 1, j + 1)]
                    + w[idx(ny, i - 1, j - 1)])
                    * inv_4dx2;
        }
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
pub fn uv_gradients(
    u: &[f64],
    v: &[f64],
    nx: usize,
    ny: usize,
    dx: f64,
    ux: &mut [f64],
    uy: &mut [f64],
    vx: &mut [f64],
    vy: &mut [f64],
) -> Result<(), StencilError> {
    assert_len("u", u, nx, ny)?;
    assert_len("v", v, nx, ny)?;
    assert_len_mut("ux", ux, nx, ny)?;
    assert_len_mut("uy", uy, nx, ny)?;
    assert_len_mut("vx", vx, nx, ny)?;
    assert_len_mut("vy", vy, nx, ny)?;
    ux.fill(0.0);
    uy.fill(0.0);
    vx.fill(0.0);
    vy.fill(0.0);
    if nx < 3 || ny < 3 {
        return Ok(());
    }
    let inv_2dx = 1.0 / (2.0 * dx);
    #[cfg(feature = "rayon")]
    if nx * ny >= RAYON_THRESHOLD {
        use rayon::prelude::*;
        let u_base = u.as_ptr() as usize;
        let v_base = v.as_ptr() as usize;
        let ux_base = ux.as_mut_ptr() as usize;
        let uy_base = uy.as_mut_ptr() as usize;
        let vx_base = vx.as_mut_ptr() as usize;
        let vy_base = vy.as_mut_ptr() as usize;
        (1..nx - 1).into_par_iter().for_each(|i| {
            let u_ptr = u_base as *const f64;
            let v_ptr = v_base as *const f64;
            let ux_ptr = ux_base as *mut f64;
            let uy_ptr = uy_base as *mut f64;
            let vx_ptr = vx_base as *mut f64;
            let vy_ptr = vy_base as *mut f64;
            for j in 1..ny - 1 {
                unsafe {
                    let center = idx(ny, i, j);
                    *ux_ptr.add(center) =
                        (*u_ptr.add(idx(ny, i + 1, j)) - *u_ptr.add(idx(ny, i - 1, j))) * inv_2dx;
                    *uy_ptr.add(center) =
                        (*u_ptr.add(idx(ny, i, j + 1)) - *u_ptr.add(idx(ny, i, j - 1))) * inv_2dx;
                    *vx_ptr.add(center) =
                        (*v_ptr.add(idx(ny, i + 1, j)) - *v_ptr.add(idx(ny, i - 1, j))) * inv_2dx;
                    *vy_ptr.add(center) =
                        (*v_ptr.add(idx(ny, i, j + 1)) - *v_ptr.add(idx(ny, i, j - 1))) * inv_2dx;
                }
            }
        });
        return Ok(());
    }
    for i in 1..nx - 1 {
        for j in 1..ny - 1 {
            let center = idx(ny, i, j);
            ux[center] = (u[idx(ny, i + 1, j)] - u[idx(ny, i - 1, j)]) * inv_2dx;
            uy[center] = (u[idx(ny, i, j + 1)] - u[idx(ny, i, j - 1)]) * inv_2dx;
            vx[center] = (v[idx(ny, i + 1, j)] - v[idx(ny, i - 1, j)]) * inv_2dx;
            vy[center] = (v[idx(ny, i, j + 1)] - v[idx(ny, i, j - 1)]) * inv_2dx;
        }
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
pub fn biharmonic_pass1_fused(
    w: &[f64],
    d_grid: &[f64],
    nx: usize,
    ny: usize,
    dx: f64,
    d_lap: &mut [f64],
    wx: &mut [f64],
    wy: &mut [f64],
    wxx: &mut [f64],
    wyy: &mut [f64],
    wxy: &mut [f64],
) -> Result<(), StencilError> {
    assert_len("w", w, nx, ny)?;
    assert_len("d_grid", d_grid, nx, ny)?;
    assert_len_mut("d_lap", d_lap, nx, ny)?;
    assert_len_mut("wx", wx, nx, ny)?;
    assert_len_mut("wy", wy, nx, ny)?;
    assert_len_mut("wxx", wxx, nx, ny)?;
    assert_len_mut("wyy", wyy, nx, ny)?;
    assert_len_mut("wxy", wxy, nx, ny)?;
    d_lap.fill(0.0);
    wx.fill(0.0);
    wy.fill(0.0);
    wxx.fill(0.0);
    wyy.fill(0.0);
    wxy.fill(0.0);
    if nx < 3 || ny < 3 {
        return Ok(());
    }
    let inv_2dx = 1.0 / (2.0 * dx);
    let inv_dx2 = 1.0 / (dx * dx);
    let inv_4dx2 = 1.0 / (4.0 * dx * dx);
    #[cfg(feature = "rayon")]
    if nx * ny >= RAYON_THRESHOLD {
        use rayon::prelude::*;
        let w_base = w.as_ptr() as usize;
        let d_base = d_grid.as_ptr() as usize;
        let d_lap_base = d_lap.as_mut_ptr() as usize;
        let wx_base = wx.as_mut_ptr() as usize;
        let wy_base = wy.as_mut_ptr() as usize;
        let wxx_base = wxx.as_mut_ptr() as usize;
        let wyy_base = wyy.as_mut_ptr() as usize;
        let wxy_base = wxy.as_mut_ptr() as usize;
        (1..nx - 1).into_par_iter().for_each(|i| {
            let w_ptr = w_base as *const f64;
            let d_ptr = d_base as *const f64;
            let d_lap_ptr = d_lap_base as *mut f64;
            let wx_ptr = wx_base as *mut f64;
            let wy_ptr = wy_base as *mut f64;
            let wxx_ptr = wxx_base as *mut f64;
            let wyy_ptr = wyy_base as *mut f64;
            let wxy_ptr = wxy_base as *mut f64;
            for j in 1..ny - 1 {
                unsafe {
                    let center = idx(ny, i, j);
                    let wc = *w_ptr.add(center);
                    let wp = *w_ptr.add(idx(ny, i + 1, j));
                    let wm = *w_ptr.add(idx(ny, i - 1, j));
                    let wn = *w_ptr.add(idx(ny, i, j + 1));
                    let ws = *w_ptr.add(idx(ny, i, j - 1));
                    let xx = (wp - 2.0 * wc + wm) * inv_dx2;
                    let yy = (wn - 2.0 * wc + ws) * inv_dx2;
                    *wxx_ptr.add(center) = xx;
                    *wyy_ptr.add(center) = yy;
                    *wx_ptr.add(center) = (wp - wm) * inv_2dx;
                    *wy_ptr.add(center) = (wn - ws) * inv_2dx;
                    *wxy_ptr.add(center) = (*w_ptr.add(idx(ny, i + 1, j + 1))
                        - *w_ptr.add(idx(ny, i + 1, j - 1))
                        - *w_ptr.add(idx(ny, i - 1, j + 1))
                        + *w_ptr.add(idx(ny, i - 1, j - 1)))
                        * inv_4dx2;
                    *d_lap_ptr.add(center) = *d_ptr.add(center) * (xx + yy);
                }
            }
        });
        return Ok(());
    }
    for i in 1..nx - 1 {
        for j in 1..ny - 1 {
            let center = idx(ny, i, j);
            let wc = w[center];
            let wp = w[idx(ny, i + 1, j)];
            let wm = w[idx(ny, i - 1, j)];
            let wn = w[idx(ny, i, j + 1)];
            let ws = w[idx(ny, i, j - 1)];
            let xx = (wp - 2.0 * wc + wm) * inv_dx2;
            let yy = (wn - 2.0 * wc + ws) * inv_dx2;
            wxx[center] = xx;
            wyy[center] = yy;
            wx[center] = (wp - wm) * inv_2dx;
            wy[center] = (wn - ws) * inv_2dx;
            wxy[center] =
                (w[idx(ny, i + 1, j + 1)] - w[idx(ny, i + 1, j - 1)] - w[idx(ny, i - 1, j + 1)]
                    + w[idx(ny, i - 1, j - 1)])
                    * inv_4dx2;
            d_lap[center] = d_grid[center] * (xx + yy);
        }
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
pub fn biharmonic(
    w: &[f64],
    d_grid: &[f64],
    nx: usize,
    ny: usize,
    dx: f64,
    lap: &mut [f64],
    d_lap: &mut [f64],
    bilap: &mut [f64],
) -> Result<(), StencilError> {
    assert_len("w", w, nx, ny)?;
    assert_len("d_grid", d_grid, nx, ny)?;
    assert_len_mut("lap", lap, nx, ny)?;
    assert_len_mut("d_lap", d_lap, nx, ny)?;
    assert_len_mut("bilap", bilap, nx, ny)?;
    lap.fill(0.0);
    d_lap.fill(0.0);
    bilap.fill(0.0);
    if nx < 3 || ny < 3 {
        return Ok(());
    }
    let inv_dx2 = 1.0 / (dx * dx);
    #[cfg(feature = "rayon")]
    let use_par = nx * ny >= RAYON_THRESHOLD;
    #[cfg(not(feature = "rayon"))]
    let use_par = false;
    if use_par {
        #[cfg(feature = "rayon")]
        {
            use rayon::prelude::*;
            let w_base = w.as_ptr() as usize;
            let d_base = d_grid.as_ptr() as usize;
            let lap_base = lap.as_mut_ptr() as usize;
            let d_lap_base = d_lap.as_mut_ptr() as usize;
            (1..nx - 1).into_par_iter().for_each(|i| {
                let w_ptr = w_base as *const f64;
                let d_ptr = d_base as *const f64;
                let lap_ptr = lap_base as *mut f64;
                let d_lap_ptr = d_lap_base as *mut f64;
                for j in 1..ny - 1 {
                    unsafe {
                        let center = idx(ny, i, j);
                        let l = (*w_ptr.add(idx(ny, i + 1, j))
                            + *w_ptr.add(idx(ny, i - 1, j))
                            + *w_ptr.add(idx(ny, i, j + 1))
                            + *w_ptr.add(idx(ny, i, j - 1))
                            - 4.0 * *w_ptr.add(center))
                            * inv_dx2;
                        *lap_ptr.add(center) = l;
                        *d_lap_ptr.add(center) = *d_ptr.add(center) * l;
                    }
                }
            });
        }
    } else {
        for i in 1..nx - 1 {
            for j in 1..ny - 1 {
                let center = idx(ny, i, j);
                let l = (w[idx(ny, i + 1, j)]
                    + w[idx(ny, i - 1, j)]
                    + w[idx(ny, i, j + 1)]
                    + w[idx(ny, i, j - 1)]
                    - 4.0 * w[center])
                    * inv_dx2;
                lap[center] = l;
                d_lap[center] = d_grid[center] * l;
            }
        }
    }

    if use_par {
        #[cfg(feature = "rayon")]
        {
            use rayon::prelude::*;
            let d_lap_base = d_lap.as_ptr() as usize;
            let bilap_base = bilap.as_mut_ptr() as usize;
            (1..nx - 1).into_par_iter().for_each(|i| {
                let d_lap_ptr = d_lap_base as *const f64;
                let bilap_ptr = bilap_base as *mut f64;
                for j in 1..ny - 1 {
                    unsafe {
                        let center = idx(ny, i, j);
                        *bilap_ptr.add(center) = (*d_lap_ptr.add(idx(ny, i + 1, j))
                            + *d_lap_ptr.add(idx(ny, i - 1, j))
                            + *d_lap_ptr.add(idx(ny, i, j + 1))
                            + *d_lap_ptr.add(idx(ny, i, j - 1))
                            - 4.0 * *d_lap_ptr.add(center))
                            * inv_dx2;
                    }
                }
            });
        }
    } else {
        for i in 1..nx - 1 {
            for j in 1..ny - 1 {
                let center = idx(ny, i, j);
                bilap[center] = (d_lap[idx(ny, i + 1, j)]
                    + d_lap[idx(ny, i - 1, j)]
                    + d_lap[idx(ny, i, j + 1)]
                    + d_lap[idx(ny, i, j - 1)]
                    - 4.0 * d_lap[center])
                    * inv_dx2;
            }
        }
    }
    Ok(())
}

#[inline]
pub fn bilaplacian_uniform(
    w: &[f64],
    nx: usize,
    ny: usize,
    d: f64,
    dx: f64,
    bilap: &mut [f64],
) -> Result<(), StencilError> {
    assert_len("w", w, nx, ny)?;
    assert_len_mut("bilap", bilap, nx, ny)?;
    bilap.fill(0.0);
    if nx < 5 || ny < 5 {
        return Ok(());
    }
    let coeff = d / (dx * dx * dx * dx);
    #[cfg(feature = "rayon")]
    if nx * ny >= RAYON_THRESHOLD {
        use rayon::prelude::*;
        let w_base = w.as_ptr() as usize;
        let bilap_base = bilap.as_mut_ptr() as usize;
        (2..nx - 2).into_par_iter().for_each(|i| {
            let w_ptr = w_base as *const f64;
            let bilap_ptr = bilap_base as *mut f64;
            for j in 2..ny - 2 {
                unsafe {
                    let center = idx(ny, i, j);
                    let val = *w_ptr.add(idx(ny, i + 2, j))
                        + *w_ptr.add(idx(ny, i - 2, j))
                        + *w_ptr.add(idx(ny, i, j + 2))
                        + *w_ptr.add(idx(ny, i, j - 2))
                        + 2.0
                            * (*w_ptr.add(idx(ny, i + 1, j + 1))
                                + *w_ptr.add(idx(ny, i + 1, j - 1))
                                + *w_ptr.add(idx(ny, i - 1, j + 1))
                                + *w_ptr.add(idx(ny, i - 1, j - 1)))
                        - 8.0
                            * (*w_ptr.add(idx(ny, i + 1, j))
                                + *w_ptr.add(idx(ny, i - 1, j))
                                + *w_ptr.add(idx(ny, i, j + 1))
                                + *w_ptr.add(idx(ny, i, j - 1)))
                        + 20.0 * *w_ptr.add(center);
                    *bilap_ptr.add(center) = val * coeff;
                }
            }
        });
        return Ok(());
    }
    for i in 2..nx - 2 {
        for j in 2..ny - 2 {
            let center = idx(ny, i, j);
            let val = w[idx(ny, i + 2, j)]
                + w[idx(ny, i - 2, j)]
                + w[idx(ny, i, j + 2)]
                + w[idx(ny, i, j - 2)]
                + 2.0
                    * (w[idx(ny, i + 1, j + 1)]
                        + w[idx(ny, i + 1, j - 1)]
                        + w[idx(ny, i - 1, j + 1)]
                        + w[idx(ny, i - 1, j - 1)])
                - 8.0
                    * (w[idx(ny, i + 1, j)]
                        + w[idx(ny, i - 1, j)]
                        + w[idx(ny, i, j + 1)]
                        + w[idx(ny, i, j - 1)])
                + 20.0 * w[center];
            bilap[center] = val * coeff;
        }
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
pub fn bilaplacian_ortho_uniform(
    w: &[f64],
    nx: usize,
    ny: usize,
    d_x: f64,
    d_y: f64,
    h_p: f64,
    dx: f64,
    bilap: &mut [f64],
    cells: &[(usize, usize)],
) -> Result<(), StencilError> {
    assert_len("w", w, nx, ny)?;
    assert_len_mut("bilap", bilap, nx, ny)?;
    let inv_dx4 = 1.0 / (dx * dx * dx * dx);
    let c_pm2_i = d_x * inv_dx4;
    let c_pm2_j = d_y * inv_dx4;
    let c_diag = 2.0 * h_p * inv_dx4;
    let c_pm1_i = (-4.0 * d_x - 4.0 * h_p) * inv_dx4;
    let c_pm1_j = (-4.0 * d_y - 4.0 * h_p) * inv_dx4;
    let c_center = (6.0 * d_x + 8.0 * h_p + 6.0 * d_y) * inv_dx4;
    for &(i, j) in cells {
        let center = idx(ny, i, j);
        bilap[center] = c_pm2_i * (w[idx(ny, i + 2, j)] + w[idx(ny, i - 2, j)])
            + c_pm2_j * (w[idx(ny, i, j + 2)] + w[idx(ny, i, j - 2)])
            + c_diag
                * (w[idx(ny, i + 1, j + 1)]
                    + w[idx(ny, i + 1, j - 1)]
                    + w[idx(ny, i - 1, j + 1)]
                    + w[idx(ny, i - 1, j - 1)])
            + c_pm1_i * (w[idx(ny, i + 1, j)] + w[idx(ny, i - 1, j)])
            + c_pm1_j * (w[idx(ny, i, j + 1)] + w[idx(ny, i, j - 1)])
            + c_center * w[center];
    }
    Ok(())
}
