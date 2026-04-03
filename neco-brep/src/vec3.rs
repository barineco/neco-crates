//! `[f64; 3]` vector helpers and geometry utilities.

#[inline]
pub fn add(a: [f64; 3], b: [f64; 3]) -> [f64; 3] {
    [a[0] + b[0], a[1] + b[1], a[2] + b[2]]
}

#[inline]
pub fn sub(a: [f64; 3], b: [f64; 3]) -> [f64; 3] {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}

#[inline]
pub fn scale(a: [f64; 3], s: f64) -> [f64; 3] {
    [a[0] * s, a[1] * s, a[2] * s]
}

#[inline]
pub fn cross(a: [f64; 3], b: [f64; 3]) -> [f64; 3] {
    [
        a[1] * b[2] - a[2] * b[1],
        a[2] * b[0] - a[0] * b[2],
        a[0] * b[1] - a[1] * b[0],
    ]
}

#[inline]
pub fn dot(a: [f64; 3], b: [f64; 3]) -> f64 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}

#[inline]
pub fn length(a: [f64; 3]) -> f64 {
    dot(a, a).sqrt()
}

#[inline]
pub fn distance(a: [f64; 3], b: [f64; 3]) -> f64 {
    length(sub(a, b))
}

/// Returns zero vector unchanged.
#[inline]
pub fn normalized(a: [f64; 3]) -> [f64; 3] {
    let len = length(a);
    if len == 0.0 {
        return a;
    }
    scale(a, 1.0 / len)
}

#[inline]
pub fn distance_2d(a: [f64; 2], b: [f64; 2]) -> f64 {
    let dx = a[0] - b[0];
    let dy = a[1] - b[1];
    (dx * dx + dy * dy).sqrt()
}

#[inline]
pub fn lerp(a: [f64; 3], b: [f64; 3], t: f64) -> [f64; 3] {
    add(scale(a, 1.0 - t), scale(b, t))
}

#[inline]
pub fn neg(a: [f64; 3]) -> [f64; 3] {
    [-a[0], -a[1], -a[2]]
}

// ─────────────────────────────────────────────────────────────────────────────
// Basis construction
// ─────────────────────────────────────────────────────────────────────────────

/// Returns an orthonormal basis (e1, e2) perpendicular to `axis`.
pub fn orthonormal_basis(axis: [f64; 3]) -> ([f64; 3], [f64; 3]) {
    let candidate = if axis[0].abs() < 0.9 {
        [1.0, 0.0, 0.0]
    } else {
        [0.0, 1.0, 0.0]
    };
    let e1 = normalized(cross(axis, candidate));
    let e2 = normalized(cross(axis, e1));
    (e1, e2)
}

// ─────────────────────────────────────────────────────────────────────────────
// Quadratic solver
// ─────────────────────────────────────────────────────────────────────────────

/// Real roots of at^2 + bt + c = 0 within [0,1], sorted.
pub fn solve_quadratic_01(a: f64, b: f64, c: f64) -> Vec<f64> {
    let disc = b * b - 4.0 * a * c;
    if disc < 0.0 {
        return vec![];
    }
    let sq = disc.sqrt();
    let mut roots = Vec::new();
    if a.abs() < 1e-14 {
        if b.abs() > 1e-14 {
            let t = -c / b;
            if (-1e-12..=1.0 + 1e-12).contains(&t) {
                roots.push(t.clamp(0.0, 1.0));
            }
        }
        return roots;
    }
    for t in [(-b - sq) / (2.0 * a), (-b + sq) / (2.0 * a)] {
        if (-1e-12..=1.0 + 1e-12).contains(&t) {
            roots.push(t.clamp(0.0, 1.0));
        }
    }
    roots.sort_by(|a, b| a.total_cmp(b));
    roots
}

// ─────────────────────────────────────────────────────────────────────────────
// Newton's method
// ─────────────────────────────────────────────────────────────────────────────

/// Find a root within [t_lo, t_hi] using Newton's method.
pub fn newton_root(
    t_lo: f64,
    t_hi: f64,
    f: &dyn Fn(f64) -> f64,
    df: &dyn Fn(f64) -> f64,
) -> Option<f64> {
    let mut t = (t_lo + t_hi) * 0.5;
    for _ in 0..50 {
        let fv = f(t);
        if fv.abs() < 1e-12 {
            return Some(t);
        }
        let dfv = df(t);
        if dfv.abs() < 1e-15 {
            break;
        }
        let dt = fv / dfv;
        t -= dt;
        t = t.clamp(t_lo - 0.1 * (t_hi - t_lo), t_hi + 0.1 * (t_hi - t_lo));
    }
    if f(t).abs() < 1e-6 {
        Some(t)
    } else {
        None
    }
}

/// Newton refinement with bisection fallback, clamped to [0, 1]
pub fn newton_refine_01(
    t_lo: f64,
    t_hi: f64,
    f: &dyn Fn(f64) -> f64,
    df: &dyn Fn(f64) -> f64,
) -> Option<f64> {
    let mut t = (t_lo + t_hi) * 0.5;
    for _ in 0..50 {
        let fv = f(t);
        if fv.abs() < 1e-12 {
            break;
        }
        let dfv = df(t);
        if dfv.abs() < 1e-15 {
            if f(t) * f(t_lo) < 0.0 {
                t = (t + t_lo) * 0.5;
            } else {
                t = (t + t_hi) * 0.5;
            }
            continue;
        }
        let dt = fv / dfv;
        t -= dt;
        t = t.clamp(t_lo - 0.05 * (t_hi - t_lo), t_hi + 0.05 * (t_hi - t_lo));
    }
    let t = t.clamp(0.0, 1.0);
    if f(t).abs() < 1e-6 {
        Some(t)
    } else {
        None
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Tetrahedron
// ─────────────────────────────────────────────────────────────────────────────

/// Signed volume of a tetrahedron.
pub fn tet_signed_volume(nodes: &[[f64; 3]], tet: &[usize; 4]) -> f64 {
    let a = nodes[tet[0]];
    let b = nodes[tet[1]];
    let c = nodes[tet[2]];
    let d = nodes[tet[3]];
    let ab = sub(b, a);
    let ac = sub(c, a);
    let ad = sub(d, a);
    dot(ab, cross(ac, ad)) / 6.0
}

/// Absolute volume of a tetrahedron.
pub fn tet_volume(nodes: &[[f64; 3]], tet: &[usize; 4]) -> f64 {
    tet_signed_volume(nodes, tet).abs()
}

/// Aspect ratio of a tetrahedron: max_edge / inradius.
pub fn tet_aspect_ratio(nodes: &[[f64; 3]], tet: &[usize; 4]) -> f64 {
    let [a, b, c, d] = [nodes[tet[0]], nodes[tet[1]], nodes[tet[2]], nodes[tet[3]]];
    let edges = [
        length(sub(a, b)),
        length(sub(a, c)),
        length(sub(a, d)),
        length(sub(b, c)),
        length(sub(b, d)),
        length(sub(c, d)),
    ];
    let max_edge = edges.iter().cloned().fold(0.0f64, f64::max);
    let vol = tet_signed_volume(nodes, tet).abs();
    if vol < 1e-30 {
        return f64::INFINITY;
    }
    let areas = [
        tri_area(a, b, c),
        tri_area(a, b, d),
        tri_area(a, c, d),
        tri_area(b, c, d),
    ];
    let sum_area: f64 = areas.iter().sum();
    let r_in = 3.0 * vol / sum_area;
    max_edge / r_in
}

// ─────────────────────────────────────────────────────────────────────────────
// Triangle
// ─────────────────────────────────────────────────────────────────────────────

/// Area of a triangle.
pub fn tri_area(a: [f64; 3], b: [f64; 3], c: [f64; 3]) -> f64 {
    length(cross(sub(b, a), sub(c, a))) * 0.5
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cross() {
        let x = [1.0, 0.0, 0.0];
        let y = [0.0, 1.0, 0.0];
        let z = cross(x, y);
        assert!((z[0]).abs() < 1e-12);
        assert!((z[1]).abs() < 1e-12);
        assert!((z[2] - 1.0).abs() < 1e-12);
    }

    #[test]
    fn test_dot() {
        let a = [1.0, 2.0, 3.0];
        let b = [4.0, 5.0, 6.0];
        assert!((dot(a, b) - 32.0).abs() < 1e-12);
    }

    #[test]
    fn test_normalized() {
        let v = [3.0, 4.0, 0.0];
        let n = normalized(v);
        assert!((length(n) - 1.0).abs() < 1e-12);
        assert!((n[0] - 0.6).abs() < 1e-12);
        assert!((n[1] - 0.8).abs() < 1e-12);

        let zero = [0.0, 0.0, 0.0];
        let nz = normalized(zero);
        assert!(length(nz).abs() < 1e-12);
    }

    #[test]
    fn test_orthonormal_basis() {
        let axis = normalized([1.0, 1.0, 1.0]);
        let (e1, e2) = orthonormal_basis(axis);
        assert!((dot(axis, e1)).abs() < 1e-10);
        assert!((dot(axis, e2)).abs() < 1e-10);
        assert!((dot(e1, e2)).abs() < 1e-10);
        assert!((length(e1) - 1.0).abs() < 1e-10);
        assert!((length(e2) - 1.0).abs() < 1e-10);
    }

    #[test]
    fn test_tet_volume() {
        let nodes = [
            [0.0, 0.0, 0.0],
            [1.0, 0.0, 0.0],
            [0.0, 1.0, 0.0],
            [0.0, 0.0, 1.0],
        ];
        let tet = [0, 1, 2, 3];
        let vol = tet_volume(&nodes, &tet);
        assert!((vol - 1.0 / 6.0).abs() < 1e-12);
    }

    #[test]
    fn test_tri_area() {
        let a = [0.0, 0.0, 0.0];
        let b = [1.0, 0.0, 0.0];
        let c = [0.0, 1.0, 0.0];
        assert!((tri_area(a, b, c) - 0.5).abs() < 1e-12);
    }

    #[test]
    fn test_solve_quadratic_01() {
        // t^2 - t = 0 => t = 0, 1
        let roots = solve_quadratic_01(1.0, -1.0, 0.0);
        assert_eq!(roots.len(), 2);
        assert!((roots[0]).abs() < 1e-10);
        assert!((roots[1] - 1.0).abs() < 1e-10);
    }
}
