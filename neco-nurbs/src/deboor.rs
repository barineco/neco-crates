//! De Boor algorithm implementations shared by 2D/3D curves and surfaces.

pub(crate) const MAX_DEGREE: usize = 10;

/// Binary search for the knot span index.
///
/// Returns the largest `k` in `[degree, n-1]` such that `knots[k] <= t < knots[k+1]`.
/// Returns `n - 1` when `t >= knots[n]`.
pub(crate) fn find_knot_span(knots: &[f64], degree: usize, n: usize, t: f64) -> usize {
    if t >= knots[n] {
        return n - 1;
    }
    let mut lo = degree;
    let mut hi = n;
    while lo < hi {
        let mid = (lo + hi) / 2;
        if t < knots[mid] {
            hi = mid;
        } else {
            lo = mid + 1;
        }
    }
    lo - 1
}

/// Search for the knot span index using a previous span as a hint.
pub(crate) fn find_knot_span_hint(
    knots: &[f64],
    degree: usize,
    n: usize,
    t: f64,
    hint: usize,
) -> usize {
    if t >= knots[n] {
        return n - 1;
    }

    let hint = hint.clamp(degree, n - 1);
    if knots[hint] <= t && t < knots[hint + 1] {
        return hint;
    }

    if t < knots[hint] {
        let mut k = hint;
        while k > degree && t < knots[k] {
            k -= 1;
            if knots[k] <= t && t < knots[k + 1] {
                return k;
            }
        }
    } else {
        let mut k = hint;
        while k + 1 < n && t >= knots[k + 1] {
            k += 1;
            if knots[k] <= t && t < knots[k + 1] {
                return k;
            }
        }
    }

    find_knot_span(knots, degree, n, t)
}

#[inline]
fn clamp_parameter(knots: &[f64], degree: usize, n: usize, t: f64) -> f64 {
    t.clamp(knots[degree], knots[n])
}

#[inline]
fn blend_interleaved(dst: &mut [f64; 4], prev: [f64; 4], alpha: f64) {
    #[cfg(target_arch = "x86_64")]
    unsafe {
        use std::arch::x86_64::*;

        // Safety: all pointers are derived from valid `[f64; 4]` arrays and
        // `_mm_loadu_pd` / `_mm_storeu_pd` permit unaligned access.
        let one_minus = _mm_set1_pd(1.0 - alpha);
        let alpha_v = _mm_set1_pd(alpha);

        let prev_lo = _mm_loadu_pd(prev.as_ptr());
        let curr_lo = _mm_loadu_pd(dst.as_ptr());
        let out_lo = _mm_add_pd(_mm_mul_pd(one_minus, prev_lo), _mm_mul_pd(alpha_v, curr_lo));
        _mm_storeu_pd(dst.as_mut_ptr(), out_lo);

        let prev_hi = _mm_loadu_pd(prev.as_ptr().add(2));
        let curr_hi = _mm_loadu_pd(dst.as_ptr().add(2));
        let out_hi = _mm_add_pd(_mm_mul_pd(one_minus, prev_hi), _mm_mul_pd(alpha_v, curr_hi));
        _mm_storeu_pd(dst.as_mut_ptr().add(2), out_hi);
    }

    #[cfg(target_arch = "aarch64")]
    unsafe {
        use std::arch::aarch64::*;

        // Safety: all pointers are derived from valid `[f64; 4]` arrays and
        // NEON load/store intrinsics accept these contiguous addresses.
        let one_minus = vdupq_n_f64(1.0 - alpha);
        let alpha_v = vdupq_n_f64(alpha);

        let prev_lo = vld1q_f64(prev.as_ptr());
        let curr_lo = vld1q_f64(dst.as_ptr());
        let out_lo = vaddq_f64(vmulq_f64(one_minus, prev_lo), vmulq_f64(alpha_v, curr_lo));
        vst1q_f64(dst.as_mut_ptr(), out_lo);

        let prev_hi = vld1q_f64(prev.as_ptr().add(2));
        let curr_hi = vld1q_f64(dst.as_ptr().add(2));
        let out_hi = vaddq_f64(vmulq_f64(one_minus, prev_hi), vmulq_f64(alpha_v, curr_hi));
        vst1q_f64(dst.as_mut_ptr().add(2), out_hi);
    }

    #[cfg(not(any(target_arch = "x86_64", target_arch = "aarch64")))]
    {
        let one_minus = 1.0 - alpha;
        for axis in 0..4 {
            dst[axis] = one_minus * prev[axis] + alpha * dst[axis];
        }
    }
}

#[inline]
fn deboor_interleaved(
    degree: usize,
    knots: &[f64],
    t: f64,
    span: usize,
    mut load: impl FnMut(usize) -> [f64; 4],
) -> [f64; 4] {
    assert!(
        degree <= MAX_DEGREE,
        "degree {} exceeds MAX_DEGREE {}",
        degree,
        MAX_DEGREE
    );

    let mut buf = [[0.0; 4]; MAX_DEGREE + 1];
    for (j, slot) in buf.iter_mut().take(degree + 1).enumerate() {
        *slot = load(span - degree + j);
    }

    for r in 1..=degree {
        for j in (r..=degree).rev() {
            let left = span + j - degree;
            let right = span + 1 + j - r;
            let denom = knots[right] - knots[left];
            if denom.abs() < 1e-30 {
                continue;
            }
            let alpha = (t - knots[left]) / denom;
            let prev = buf[j - 1];
            blend_interleaved(&mut buf[j], prev, alpha);
        }
    }
    buf[degree]
}

/// 1D De Boor in homogeneous 3D coordinates (hx, hy, hz, hw).
pub(crate) fn deboor_1d_homogeneous_3d(
    degree: usize,
    knots: &[f64],
    hx: &[f64],
    hy: &[f64],
    hz: &[f64],
    hw: &[f64],
    t: f64,
) -> (f64, f64, f64, f64) {
    let n = hx.len();
    let p = degree;
    let t = clamp_parameter(knots, p, n, t);
    let span = find_knot_span(knots, p, n, t);
    let out = deboor_interleaved(p, knots, t, span, |idx| {
        [hx[idx], hy[idx], hz[idx], hw[idx]]
    });
    (out[0], out[1], out[2], out[3])
}

/// 1D De Boor in homogeneous 2D coordinates (hx, hy, hw).
#[allow(dead_code)]
pub(crate) fn deboor_1d_homogeneous_2d(
    degree: usize,
    knots: &[f64],
    hx: &[f64],
    hy: &[f64],
    hw: &[f64],
    t: f64,
) -> (f64, f64, f64) {
    let n = hx.len();
    let p = degree;
    let t = clamp_parameter(knots, p, n, t);
    let span = find_knot_span(knots, p, n, t);
    let out = deboor_interleaved(p, knots, t, span, |idx| [hx[idx], hy[idx], hw[idx], 0.0]);
    (out[0], out[1], out[2])
}

/// Evaluate a rational 2D curve directly from control points and weights.
pub(crate) fn deboor_1d_control_points_2d(
    degree: usize,
    knots: &[f64],
    control_points: &[[f64; 2]],
    weights: &[f64],
    t: f64,
    hint: Option<usize>,
) -> (f64, f64, f64, usize) {
    let n = control_points.len();
    let t = clamp_parameter(knots, degree, n, t);
    let span = hint
        .map(|k| find_knot_span_hint(knots, degree, n, t, k))
        .unwrap_or_else(|| find_knot_span(knots, degree, n, t));
    let out = deboor_interleaved(degree, knots, t, span, |idx| {
        let w = weights[idx];
        [
            control_points[idx][0] * w,
            control_points[idx][1] * w,
            w,
            0.0,
        ]
    });
    (out[0], out[1], out[2], span)
}

/// Evaluate a rational 3D curve directly from control points and weights.
pub(crate) fn deboor_1d_control_points_3d(
    degree: usize,
    knots: &[f64],
    control_points: &[[f64; 3]],
    weights: &[f64],
    t: f64,
    hint: Option<usize>,
) -> (f64, f64, f64, f64, usize) {
    let n = control_points.len();
    let t = clamp_parameter(knots, degree, n, t);
    let span = hint
        .map(|k| find_knot_span_hint(knots, degree, n, t, k))
        .unwrap_or_else(|| find_knot_span(knots, degree, n, t));
    let out = deboor_interleaved(degree, knots, t, span, |idx| {
        let w = weights[idx];
        [
            control_points[idx][0] * w,
            control_points[idx][1] * w,
            control_points[idx][2] * w,
            w,
        ]
    });
    (out[0], out[1], out[2], out[3], span)
}

/// 1D Boehm knot insertion in homogeneous 3D coordinates.
///
/// Inserts parameter `t` at span `k`, increasing the control point count by one.
pub(crate) fn knot_insert_1d_3d(
    degree: usize,
    knots: &[f64],
    hcoords: [&[f64]; 4],
    k: usize,
    t: f64,
) -> [Vec<f64>; 4] {
    let [hx, hy, hz, hw] = hcoords;
    let n = hx.len();
    let p = degree;

    let mut result = [
        vec![0.0; n + 1],
        vec![0.0; n + 1],
        vec![0.0; n + 1],
        vec![0.0; n + 1],
    ];
    let coords = [hx, hy, hz, hw];

    for i in 0..=n {
        if i <= k.saturating_sub(p) {
            for axis in 0..4 {
                result[axis][i] = coords[axis][i];
            }
        } else if i > k {
            for axis in 0..4 {
                result[axis][i] = coords[axis][i - 1];
            }
        } else {
            let denom = knots[i + p] - knots[i];
            let alpha = if denom.abs() < 1e-30 {
                0.0
            } else {
                (t - knots[i]) / denom
            };
            for axis in 0..4 {
                result[axis][i] = (1.0 - alpha) * coords[axis][i - 1] + alpha * coords[axis][i];
            }
        }
    }

    result
}
