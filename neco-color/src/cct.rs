/// Compute CIE xy chromaticity from correlated color temperature T [K] (Kim et al. 1999).
pub fn cct_to_xy(t: f64) -> (f64, f64) {
    let t2 = t * t;
    let t3 = t2 * t;
    let x = if t <= 4000.0 {
        -0.2661239e9 / t3 - 0.2343589e6 / t2 + 0.8776956e3 / t + 0.179910
    } else {
        -3.0258469e9 / t3 + 2.1070379e6 / t2 + 0.2226347e3 / t + 0.240390
    };
    let x2 = x * x;
    let x3 = x2 * x;
    let y = if t <= 2222.0 {
        -1.1063814 * x3 - 1.34811020 * x2 + 2.18555832 * x - 0.20219683
    } else if t <= 4000.0 {
        -0.9549476 * x3 - 1.37418593 * x2 + 2.09137015 * x - 0.16748867
    } else {
        3.0817580 * x3 - 5.87338670 * x2 + 3.75112997 * x - 0.37001483
    };
    (x, y)
}

/// Build a 3x3 row-major white balance correction matrix.
pub fn build_wb_matrix(temperature: f64, tint: f64) -> [f64; 9] {
    let (tx, ty) = cct_to_xy(temperature);
    let t_big_x = tx / ty;
    let t_big_z = (1.0 - tx - ty) / ty;
    let (rx, ry) = cct_to_xy(6500.0);
    let r_big_x = rx / ry;
    let r_big_z = (1.0 - rx - ry) / ry;
    let scale_x = r_big_x / t_big_x;
    let scale_z = r_big_z / t_big_z;

    let xyz_to_rgb: [[f64; 3]; 3] = [
        [3.2404542, -1.5371385, -0.4985314],
        [-0.9692660, 1.8760108, 0.0415560],
        [0.0556434, -0.2040259, 1.0572252],
    ];
    let rgb_to_xyz: [[f64; 3]; 3] = [
        [0.4124564, 0.3575761, 0.1804375],
        [0.2126729, 0.7151522, 0.0721750],
        [0.0193339, 0.1191920, 0.9503041],
    ];

    let tint_scale = 1.0 - tint * 0.01;
    let mut diag_xyz = [[0.0f64; 3]; 3];
    for i in 0..3 {
        for j in 0..3 {
            diag_xyz[i][j] = rgb_to_xyz[i][j];
        }
    }
    for item in &mut diag_xyz[0] {
        *item *= scale_x;
    }
    for item in &mut diag_xyz[1] {
        *item *= tint_scale;
    }
    for item in &mut diag_xyz[2] {
        *item *= scale_z;
    }
    let mut result = [0.0f64; 9];
    for i in 0..3 {
        for j in 0..3 {
            let mut sum = 0.0;
            for k in 0..3 {
                sum += xyz_to_rgb[i][k] * diag_xyz[k][j];
            }
            result[i * 3 + j] = sum;
        }
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn d65_xy_coordinates() {
        let (x, y) = cct_to_xy(6500.0);
        assert!(
            (x - 0.3127).abs() < 0.01,
            "D65 x should be near 0.3127: got {x}"
        );
        assert!(
            (y - 0.3290).abs() < 0.01,
            "D65 y should be near 0.3290: got {y}"
        );
    }

    #[test]
    fn wb_matrix_identity_at_d65() {
        let m = build_wb_matrix(6500.0, 0.0);
        assert!(
            (m[0] - 1.0).abs() < 0.01,
            "m[0,0] should be near 1.0: got {}",
            m[0]
        );
        assert!(
            (m[4] - 1.0).abs() < 0.01,
            "m[1,1] should be near 1.0: got {}",
            m[4]
        );
        assert!(
            (m[8] - 1.0).abs() < 0.01,
            "m[2,2] should be near 1.0: got {}",
            m[8]
        );
        for (idx, &v) in m.iter().enumerate() {
            if idx == 0 || idx == 4 || idx == 8 {
                continue;
            }
            assert!(
                v.abs() < 0.01,
                "off-diagonal m[{idx}] should be near 0: got {v}"
            );
        }
    }
}
