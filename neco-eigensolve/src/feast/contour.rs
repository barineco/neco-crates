use crate::c64::C64;

/// FEAST search interval.
#[derive(Debug, Clone)]
pub struct FeastInterval {
    pub lambda_min: f64,
    pub lambda_max: f64,
}

/// Quadrature point on the contour.
#[derive(Debug, Clone)]
pub struct ContourPoint {
    pub z: C64,
    pub weight: f64,
    pub theta: f64,
}

pub(crate) fn gauss_legendre_nodes_weights(n: usize) -> Result<(Vec<f64>, Vec<f64>), String> {
    let half: &[(f64, f64)] = match n {
        4 => &[
            (0.339_981_043_584_856, 0.652_145_154_862_546),
            (0.861_136_311_594_052_6, 0.347_854_845_137_454),
        ],
        8 => &[
            (0.183_434_642_495_649_8, 0.362_683_783_378_362),
            (0.525_532_409_916_328_9, 0.313_706_645_877_887_3),
            (0.796_666_477_413_626_7, 0.222_381_034_453_374_47),
            (0.960_289_856_497_536_2, 0.101_228_536_290_376_26),
        ],
        16 => &[
            (0.095_012_509_837_637_44, 0.189_450_610_455_068_5),
            (0.281_603_550_779_258_9, 0.182_603_415_044_923_59),
            (0.458_016_777_657_227_4, 0.169_156_519_395_002_8),
            (0.617_876_244_402_643_7, 0.149_595_988_816_576_73),
            (0.755_404_408_355_003, 0.124_628_971_255_533_87),
            (0.865_631_202_387_831_7, 0.095_158_511_682_492_78),
            (0.944_575_023_073_233, 0.062_253_523_938_647_89),
            (0.989_400_934_991_649_9, 0.027_152_459_411_754_095),
        ],
        _ => {
            return Err(format!("n_quadrature={n} is not supported (only 4, 8, 16)"));
        }
    };

    let mut nodes = Vec::with_capacity(n);
    let mut weights = Vec::with_capacity(n);

    for &(x, w) in half {
        nodes.push(-x);
        weights.push(w);
        nodes.push(x);
        weights.push(w);
    }

    Ok((nodes, weights))
}

pub fn contour_points(interval: &FeastInterval, n_e: usize) -> Result<Vec<ContourPoint>, String> {
    let (nodes, weights) = gauss_legendre_nodes_weights(n_e)?;
    let center = (interval.lambda_max + interval.lambda_min) / 2.0;
    let r = (interval.lambda_max - interval.lambda_min) / 2.0;

    Ok(nodes
        .iter()
        .zip(weights.iter())
        .map(|(&x_e, &w_e)| {
            let theta = -(std::f64::consts::PI / 2.0) * (x_e - 1.0);
            let z = C64::new(center + r * theta.cos(), r * theta.sin());
            ContourPoint {
                z,
                weight: w_e,
                theta,
            }
        })
        .collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_gauss_legendre_8_points() {
        let (nodes, weights) = gauss_legendre_nodes_weights(8).unwrap();
        assert_eq!(nodes.len(), 8);
        assert_eq!(weights.len(), 8);
        let sum: f64 = weights.iter().sum();
        assert!(
            (sum - 2.0).abs() < 1e-12,
            "weights sum = {sum}, expected 2.0"
        );
        for &x in &nodes {
            assert!(x > -1.0 && x < 1.0, "node {x} outside (-1, 1)");
        }
    }

    #[test]
    fn test_contour_points_on_circle() {
        let interval = FeastInterval {
            lambda_min: 10.0,
            lambda_max: 50.0,
        };
        let points = contour_points(&interval, 8).unwrap();
        assert_eq!(points.len(), 8);
        let center = 30.0;
        let r = 20.0;
        for (i, cp) in points.iter().enumerate() {
            let dx = cp.z.re - center;
            let dy = cp.z.im;
            let dist = (dx * dx + dy * dy).sqrt();
            assert!(
                (dist - r).abs() < 1e-10,
                "point {i}: dist={dist}, expected {r}"
            );
            assert!(
                cp.z.im >= -1e-10,
                "point {i} should be on upper half circle"
            );
        }
    }

    #[test]
    fn test_contour_points_rejects_unsupported_quadrature() {
        let interval = FeastInterval {
            lambda_min: 10.0,
            lambda_max: 50.0,
        };
        let err = contour_points(&interval, 6).unwrap_err();
        assert!(
            err.contains("n_quadrature=6"),
            "unexpected error message: {err}"
        );
    }
}
