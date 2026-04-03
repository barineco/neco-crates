/// Convert sRGB [0,1] to HSL (H in 0..1).
pub fn srgb_to_hsl(r: f32, g: f32, b: f32) -> (f32, f32, f32) {
    let max = r.max(g).max(b);
    let min = r.min(g).min(b);
    let l = (max + min) / 2.0;
    if (max - min).abs() < f32::EPSILON {
        return (0.0, 0.0, l);
    }
    let d = max - min;
    let s = if l > 0.5 {
        d / (2.0 - max - min)
    } else {
        d / (max + min)
    };
    let h = if (max - r).abs() < f32::EPSILON {
        ((g - b) / d + if g < b { 6.0 } else { 0.0 }) / 6.0
    } else if (max - g).abs() < f32::EPSILON {
        ((b - r) / d + 2.0) / 6.0
    } else {
        ((r - g) / d + 4.0) / 6.0
    };
    (h, s, l)
}

/// Convert HSL to sRGB [0,1].
pub fn hsl_to_srgb(h: f32, s: f32, l: f32) -> (f32, f32, f32) {
    if s.abs() < f32::EPSILON {
        return (l, l, l);
    }
    let q = if l < 0.5 {
        l * (1.0 + s)
    } else {
        l + s - l * s
    };
    let p = 2.0 * l - q;
    let r = hue_to_rgb(p, q, h + 1.0 / 3.0);
    let g = hue_to_rgb(p, q, h);
    let b = hue_to_rgb(p, q, h - 1.0 / 3.0);
    (r, g, b)
}

pub(crate) fn hue_to_rgb(p: f32, q: f32, mut t: f32) -> f32 {
    if t < 0.0 {
        t += 1.0;
    }
    if t > 1.0 {
        t -= 1.0;
    }
    if t < 1.0 / 6.0 {
        return p + (q - p) * 6.0 * t;
    }
    if t < 1.0 / 2.0 {
        return q;
    }
    if t < 2.0 / 3.0 {
        return p + (q - p) * (2.0 / 3.0 - t) * 6.0;
    }
    p
}

#[cfg(test)]
mod tests {
    use super::*;

    fn assert_rgb_approx(a: (f32, f32, f32), b: (f32, f32, f32), label: &str) {
        let eps = 1e-5;
        assert!(
            (a.0 - b.0).abs() < eps && (a.1 - b.1).abs() < eps && (a.2 - b.2).abs() < eps,
            "{label}: expected ({}, {}, {}), got ({}, {}, {})",
            b.0,
            b.1,
            b.2,
            a.0,
            a.1,
            a.2
        );
    }

    #[test]
    fn round_trip_primaries_and_secondaries() {
        let colors: &[(f32, f32, f32)] = &[
            (1.0, 0.0, 0.0), // R
            (0.0, 1.0, 0.0), // G
            (0.0, 0.0, 1.0), // B
            (0.0, 1.0, 1.0), // C
            (1.0, 0.0, 1.0), // M
            (1.0, 1.0, 0.0), // Y
            (1.0, 1.0, 1.0), // W
            (0.0, 0.0, 0.0), // K
        ];
        for &(r, g, b) in colors {
            let (h, s, l) = srgb_to_hsl(r, g, b);
            let rt = hsl_to_srgb(h, s, l);
            assert_rgb_approx(rt, (r, g, b), &format!("round-trip ({r},{g},{b})"));
        }
    }

    #[test]
    fn known_red() {
        let (h, s, l) = srgb_to_hsl(1.0, 0.0, 0.0);
        assert!((h - 0.0).abs() < 1e-5, "red H should be 0: got {h}");
        assert!((s - 1.0).abs() < 1e-5, "red S should be 1: got {s}");
        assert!((l - 0.5).abs() < 1e-5, "red L should be 0.5: got {l}");
    }

    #[test]
    fn known_green() {
        let (h, s, l) = srgb_to_hsl(0.0, 1.0, 0.0);
        assert!(
            (h - 1.0 / 3.0).abs() < 1e-5,
            "green H should be 1/3: got {h}"
        );
        assert!((s - 1.0).abs() < 1e-5, "green S should be 1: got {s}");
        assert!((l - 0.5).abs() < 1e-5, "green L should be 0.5: got {l}");
    }
}
