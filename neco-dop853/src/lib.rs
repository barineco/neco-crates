//! Dormand-Prince 8(5,3) adaptive Runge-Kutta integrator.
//!
//! Butcher tableau coefficients follow the Hairer and Wanner DOP853 scheme.
#[allow(clippy::excessive_precision, clippy::unreadable_literal)]
mod coefficients {
    pub const C: [f64; 16] = [
        0.0,
        0.526001519587677318785587544488e-01,
        0.789002279381515978178381316732e-01,
        0.118350341907227396726757197510e+00,
        0.281649658092772603273242802490e+00,
        0.333333333333333333333333333333e+00,
        0.25e+00,
        0.307692307692307692307692307692e+00,
        0.651282051282051282051282051282e+00,
        0.6e+00,
        0.857142857142857142857142857142e+00,
        1.0,
        0.0,
        0.1e+00,
        0.2e+00,
        0.777777777777777777777777777778e+00,
    ];
    pub const A21: f64 = 5.26001519587677318785587544488e-2;
    pub const A31: f64 = 1.97250569845378994544595329183e-2;
    pub const A32: f64 = 5.91751709536136983633785987549e-2;
    pub const A41: f64 = 2.95875854768068491816892993775e-2;
    pub const A43: f64 = 8.87627564304205475450678981324e-2;
    pub const A51: f64 = 2.41365134159266685502369798665e-1;
    pub const A53: f64 = -8.84549479328286085344864962717e-1;
    pub const A54: f64 = 9.24834003261792003115737966543e-1;
    pub const A61: f64 = 3.7037037037037037037037037037e-2;
    pub const A64: f64 = 1.70828608729473871279604482173e-1;
    pub const A65: f64 = 1.25467687566822425016691814123e-1;
    pub const A71: f64 = 3.7109375e-2;
    pub const A74: f64 = 1.70252211019544039314978060272e-1;
    pub const A75: f64 = 6.02165389804559606850219397283e-2;
    pub const A76: f64 = -1.7578125e-2;
    pub const A81: f64 = 3.70920001185047927108779319836e-2;
    pub const A84: f64 = 1.70383925712239993810214054705e-1;
    pub const A85: f64 = 1.07262030446373284651809199168e-1;
    pub const A86: f64 = -1.53194377486244017527936158236e-2;
    pub const A87: f64 = 8.27378916381402288758473766002e-3;
    pub const A91: f64 = 6.24110958716075717114429577812e-1;
    pub const A94: f64 = -3.36089262944694129406857109825e+0;
    pub const A95: f64 = -8.68219346841726006818189891453e-1;
    pub const A96: f64 = 2.75920996994467083049415600797e+1;
    pub const A97: f64 = 2.01540675504778934086186788979e+1;
    pub const A98: f64 = -4.34898841810699588477366255144e+1;
    pub const A101: f64 = 4.77662536438264365890433908527e-1;
    pub const A104: f64 = -2.48811461997166764192642586468e+0;
    pub const A105: f64 = -5.90290826836842996371446475743e-1;
    pub const A106: f64 = 2.12300514481811942347288949897e+1;
    pub const A107: f64 = 1.52792336328824235832596922938e+1;
    pub const A108: f64 = -3.32882109689848629194453265587e+1;
    pub const A109: f64 = -2.03312017085086261358222928593e-2;
    pub const A111: f64 = -9.3714243008598732571704021658e-1;
    pub const A114: f64 = 5.18637242884406370830023853209e+0;
    pub const A115: f64 = 1.09143734899672957818500254654e+0;
    pub const A116: f64 = -8.14978701074692612513997267357e+0;
    pub const A117: f64 = -1.85200656599969598641566180701e+1;
    pub const A118: f64 = 2.27394870993505042818970056734e+1;
    pub const A119: f64 = 2.49360555267965238987089396762e+0;
    pub const A1110: f64 = -3.0467644718982195003823669022e+0;
    pub const A121: f64 = 2.27331014751653820792359768449e+0;
    pub const A124: f64 = -1.05344954667372501984066689879e+1;
    pub const A125: f64 = -2.00087205822486249909675718444e+0;
    pub const A126: f64 = -1.79589318631187989172765950534e+1;
    pub const A127: f64 = 2.79488845294199600508499808837e+1;
    pub const A128: f64 = -2.85899827713502369474065508674e+0;
    pub const A129: f64 = -8.87285693353062954433549289258e+0;
    pub const A1210: f64 = 1.23605671757943030647266201528e+1;
    pub const A1211: f64 = 6.43392746015763530355970484046e-1;
    pub const B: [f64; 12] = [
        5.42937341165687622380535766363e-2,
        0.0,
        0.0,
        0.0,
        0.0,
        4.45031289275240888144113950566e+0,
        1.89151789931450038304281599044e+0,
        -5.8012039600105847814672114227e+0,
        3.1116436695781989440891606237e-1,
        -1.52160949662516078556178806805e-1,
        2.01365400804030348374776537501e-1,
        4.47106157277725905176885569043e-2,
    ];
    pub const ER: [f64; 12] = [
        0.1312004499419488073250102996e-1,
        0.0,
        0.0,
        0.0,
        0.0,
        -0.1225156446376204440720569753e+1,
        -0.4957589496572501915214079952e+0,
        0.1664377182454986536961530415e+1,
        -0.3503288487499736816886487290e+0,
        0.3341791187130174790297318841e+0,
        0.8192320648511571246570742613e-1,
        -0.2235530786388629525884427845e-1,
    ];
    pub const BHH: [f64; 3] = [
        0.244094488188976377952755905512e+0,
        0.733846688281611857341361741547e+0,
        0.220588235294117647058823529412e-1,
    ];
}

use coefficients::*;

#[derive(Debug, Clone)]
pub struct Dop853Options {
    pub rtol: f64,
    pub atol: f64,
    pub max_step: f64,
    pub initial_step: f64,
}

impl Default for Dop853Options {
    fn default() -> Self {
        Self {
            rtol: 1e-10,
            atol: 1e-13,
            max_step: 5e-4,
            initial_step: 0.0,
        }
    }
}

#[derive(Debug, Clone)]
pub struct Dop853Result {
    pub t: Vec<f64>,
    pub y: Vec<Vec<f64>>,
    pub success: bool,
    pub n_steps: usize,
    pub n_evals: usize,
}

pub fn integrate_dop853<F>(
    rhs: F,
    t_span: (f64, f64),
    y0: &[f64],
    t_eval: &[f64],
    opts: &Dop853Options,
) -> Dop853Result
where
    F: Fn(f64, &[f64], &mut [f64]),
{
    let n = y0.len();
    let t0 = t_span.0;
    let t_end = t_span.1;
    let mut t = t0;
    let mut y = y0.to_vec();
    let mut h = if opts.initial_step > 0.0 {
        opts.initial_step.min(opts.max_step)
    } else {
        initial_step_size(&rhs, t0, &y, opts)
    };

    let mut result_t: Vec<f64> = Vec::with_capacity(t_eval.len());
    let mut result_y: Vec<Vec<f64>> = Vec::with_capacity(t_eval.len());
    let mut eval_idx = 0;
    let mut n_steps = 0usize;
    let mut n_evals = 0usize;
    let mut step_rejected = false;

    while eval_idx < t_eval.len() && t_eval[eval_idx] <= t0 + 1e-15 * t0.abs().max(1.0) {
        result_t.push(t_eval[eval_idx]);
        result_y.push(y.clone());
        eval_idx += 1;
    }

    let mut k = vec![vec![0.0; n]; 13];
    rhs(t, &y, &mut k[0]);
    n_evals += 1;
    let max_steps = 10_000_000;

    while t < t_end && n_steps < max_steps {
        if t + h > t_end {
            h = t_end - t;
        }
        if h < 1e-15 * t.abs().max(1.0) {
            break;
        }

        let mut ys = vec![0.0; n];
        for i in 0..n {
            ys[i] = y[i] + h * A21 * k[0][i];
        }
        rhs(t + C[1] * h, &ys, &mut k[1]);
        for i in 0..n {
            ys[i] = y[i] + h * (A31 * k[0][i] + A32 * k[1][i]);
        }
        rhs(t + C[2] * h, &ys, &mut k[2]);
        for i in 0..n {
            ys[i] = y[i] + h * (A41 * k[0][i] + A43 * k[2][i]);
        }
        rhs(t + C[3] * h, &ys, &mut k[3]);
        for i in 0..n {
            ys[i] = y[i] + h * (A51 * k[0][i] + A53 * k[2][i] + A54 * k[3][i]);
        }
        rhs(t + C[4] * h, &ys, &mut k[4]);
        for i in 0..n {
            ys[i] = y[i] + h * (A61 * k[0][i] + A64 * k[3][i] + A65 * k[4][i]);
        }
        rhs(t + C[5] * h, &ys, &mut k[5]);
        for i in 0..n {
            ys[i] = y[i] + h * (A71 * k[0][i] + A74 * k[3][i] + A75 * k[4][i] + A76 * k[5][i]);
        }
        rhs(t + C[6] * h, &ys, &mut k[6]);
        for i in 0..n {
            ys[i] = y[i]
                + h * (A81 * k[0][i]
                    + A84 * k[3][i]
                    + A85 * k[4][i]
                    + A86 * k[5][i]
                    + A87 * k[6][i]);
        }
        rhs(t + C[7] * h, &ys, &mut k[7]);
        for i in 0..n {
            ys[i] = y[i]
                + h * (A91 * k[0][i]
                    + A94 * k[3][i]
                    + A95 * k[4][i]
                    + A96 * k[5][i]
                    + A97 * k[6][i]
                    + A98 * k[7][i]);
        }
        rhs(t + C[8] * h, &ys, &mut k[8]);
        for i in 0..n {
            ys[i] = y[i]
                + h * (A101 * k[0][i]
                    + A104 * k[3][i]
                    + A105 * k[4][i]
                    + A106 * k[5][i]
                    + A107 * k[6][i]
                    + A108 * k[7][i]
                    + A109 * k[8][i]);
        }
        rhs(t + C[9] * h, &ys, &mut k[9]);
        for i in 0..n {
            ys[i] = y[i]
                + h * (A111 * k[0][i]
                    + A114 * k[3][i]
                    + A115 * k[4][i]
                    + A116 * k[5][i]
                    + A117 * k[6][i]
                    + A118 * k[7][i]
                    + A119 * k[8][i]
                    + A1110 * k[9][i]);
        }
        rhs(t + C[10] * h, &ys, &mut k[10]);
        for i in 0..n {
            ys[i] = y[i]
                + h * (A121 * k[0][i]
                    + A124 * k[3][i]
                    + A125 * k[4][i]
                    + A126 * k[5][i]
                    + A127 * k[6][i]
                    + A128 * k[7][i]
                    + A129 * k[8][i]
                    + A1210 * k[9][i]
                    + A1211 * k[10][i]);
        }
        rhs(t + h, &ys, &mut k[11]);
        n_evals += 11;

        let mut y_new = vec![0.0; n];
        for i in 0..n {
            y_new[i] = y[i]
                + h * (B[0] * k[0][i]
                    + B[5] * k[5][i]
                    + B[6] * k[6][i]
                    + B[7] * k[7][i]
                    + B[8] * k[8][i]
                    + B[9] * k[9][i]
                    + B[10] * k[10][i]
                    + B[11] * k[11][i]);
        }

        rhs(t + h, &y_new, &mut k[12]);
        n_evals += 1;

        let mut err_sq = 0.0;
        let mut err2_sq = 0.0;
        for i in 0..n {
            let sk = opts.atol + opts.rtol * y[i].abs().max(y_new[i].abs());
            let er_i = ER[0] * k[0][i]
                + ER[5] * k[5][i]
                + ER[6] * k[6][i]
                + ER[7] * k[7][i]
                + ER[8] * k[8][i]
                + ER[9] * k[9][i]
                + ER[10] * k[10][i]
                + ER[11] * k[11][i];
            let slope_i = (y_new[i] - y[i]) / h;
            let bhh_i = slope_i - BHH[0] * k[0][i] - BHH[1] * k[8][i] - BHH[2] * k[11][i];
            err_sq += (er_i / sk) * (er_i / sk);
            err2_sq += (bhh_i / sk) * (bhh_i / sk);
        }

        let denom = err_sq + 0.01 * err2_sq;
        let err = if denom > 0.0 {
            h.abs() * err_sq / (denom * n as f64).sqrt()
        } else {
            0.0
        };

        if err <= 1.0 {
            let t_new = t + h;
            n_steps += 1;
            while eval_idx < t_eval.len()
                && t_eval[eval_idx] <= t_new + 1e-15 * t_new.abs().max(1.0)
            {
                let te = t_eval[eval_idx];
                if (te - t_new).abs() < 1e-15 * t_new.abs().max(1.0) {
                    result_t.push(te);
                    result_y.push(y_new.clone());
                } else {
                    let theta = (te - t) / h;
                    let mut y_interp = vec![0.0; n];
                    for i in 0..n {
                        y_interp[i] = y[i] * (1.0 - theta)
                            + y_new[i] * theta
                            + theta
                                * (theta - 1.0)
                                * ((1.0 - 2.0 * theta) * (y_new[i] - y[i])
                                    + (theta - 1.0) * h * k[0][i]
                                    + theta * h * k[12][i]);
                    }
                    result_t.push(te);
                    result_y.push(y_interp);
                }
                eval_idx += 1;
            }

            t = t_new;
            y = y_new;
            k[0] = k[12].clone();
            let fac = if err > 0.0 {
                0.9 * err.powf(-1.0 / 8.0)
            } else {
                5.0
            };
            let fac = if step_rejected {
                fac.min(1.0)
            } else {
                fac.clamp(0.2, 10.0)
            };
            h *= fac;
            h = h.min(opts.max_step);
            step_rejected = false;
        } else {
            let fac = 0.9 * err.powf(-1.0 / 8.0);
            h *= fac.max(0.2);
            step_rejected = true;
        }
    }

    Dop853Result {
        t: result_t,
        y: result_y,
        success: n_steps < max_steps && eval_idx == t_eval.len(),
        n_steps,
        n_evals,
    }
}

fn initial_step_size<F>(rhs: &F, t0: f64, y0: &[f64], opts: &Dop853Options) -> f64
where
    F: Fn(f64, &[f64], &mut [f64]),
{
    let n = y0.len();
    let mut f0 = vec![0.0; n];
    rhs(t0, y0, &mut f0);
    let mut d0 = 0.0;
    let mut d1 = 0.0;
    for i in 0..n {
        let sk = opts.atol + opts.rtol * y0[i].abs();
        d0 += (y0[i] / sk) * (y0[i] / sk);
        d1 += (f0[i] / sk) * (f0[i] / sk);
    }
    d0 = (d0 / n as f64).sqrt();
    d1 = (d1 / n as f64).sqrt();
    let h0 = if d0 < 1e-5 || d1 < 1e-5 {
        1e-6
    } else {
        0.01 * d0 / d1
    };
    let h0 = h0.min(opts.max_step);

    let mut y1 = vec![0.0; n];
    for i in 0..n {
        y1[i] = y0[i] + h0 * f0[i];
    }
    let mut f1 = vec![0.0; n];
    rhs(t0 + h0, &y1, &mut f1);
    let mut d2 = 0.0;
    for i in 0..n {
        let sk = opts.atol + opts.rtol * y0[i].abs();
        d2 += ((f1[i] - f0[i]) / sk) * ((f1[i] - f0[i]) / sk);
    }
    d2 = (d2 / n as f64).sqrt() / h0;

    let h1 = if d1.max(d2) <= 1e-15 {
        (h0 * 1e-3).max(1e-6)
    } else {
        (0.01 / d1.max(d2)).powf(1.0 / 8.0)
    };
    h0.min(100.0 * h1).min(opts.max_step)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_harmonic_oscillator() {
        let rhs = |_t: f64, y: &[f64], dydt: &mut [f64]| {
            dydt[0] = y[1];
            dydt[1] = -y[0];
        };
        let y0 = [1.0, 0.0];
        let t_end = 10.0;
        let n_eval = 1000;
        let t_eval: Vec<f64> = (0..=n_eval)
            .map(|i| i as f64 * t_end / n_eval as f64)
            .collect();
        let result = integrate_dop853(
            rhs,
            (0.0, t_end),
            &y0,
            &t_eval,
            &Dop853Options {
                rtol: 1e-10,
                atol: 1e-13,
                max_step: 0.05,
                initial_step: 0.0,
            },
        );
        assert!(result.success, "DOP853 failed: steps={}", result.n_steps);
        assert_eq!(result.t.len(), t_eval.len());
        for (i, &ti) in result.t.iter().enumerate() {
            let exact_y = ti.cos();
            let err_y = (result.y[i][0] - exact_y).abs();
            assert!(err_y < 1e-7, "t={ti:.4}: y err = {err_y:.2e}");
        }
        let final_y = result.y.last().unwrap();
        let exact_final = t_end.cos();
        let err = (final_y[0] - exact_final).abs();
        assert!(err < 1e-9, "final err = {err:.2e}");
    }

    #[test]
    fn test_exponential_decay() {
        let rhs = |_t: f64, y: &[f64], dydt: &mut [f64]| {
            dydt[0] = -y[0];
        };
        let y0 = [1.0];
        let t_end = 5.0;
        let t_eval: Vec<f64> = (0..=50).map(|i| i as f64 * t_end / 50.0).collect();
        let result = integrate_dop853(rhs, (0.0, t_end), &y0, &t_eval, &Dop853Options::default());
        assert!(result.success, "DOP853 failed: steps={}", result.n_steps);
        for (i, &ti) in result.t.iter().enumerate() {
            let exact = (-ti).exp();
            let err = (result.y[i][0] - exact).abs();
            assert!(err < 1e-10, "t={ti:.2}: err = {err:.2e}");
        }
    }

    #[test]
    fn test_energy_conservation() {
        let rhs = |_t: f64, y: &[f64], dydt: &mut [f64]| {
            dydt[0] = -y[1];
            dydt[1] = y[0];
        };
        let y0 = [1.0, 0.0];
        let t_end = 100.0;
        let t_eval: Vec<f64> = (0..=1000).map(|i| i as f64 * t_end / 1000.0).collect();
        let result = integrate_dop853(
            rhs,
            (0.0, t_end),
            &y0,
            &t_eval,
            &Dop853Options {
                rtol: 1e-12,
                atol: 1e-14,
                max_step: 0.01,
                initial_step: 0.0,
            },
        );
        assert!(result.success, "DOP853 failed: steps={}", result.n_steps);
        let e0 = y0[0] * y0[0] + y0[1] * y0[1];
        for y in &result.y {
            let e = y[0] * y[0] + y[1] * y[1];
            let err = (e - e0).abs();
            assert!(err < 1e-10, "energy err = {err:.2e}");
        }
    }

    #[test]
    fn test_logistic_growth_regression() {
        let rhs = |_t: f64, y: &[f64], dydt: &mut [f64]| {
            let r = 1.5;
            let k = 2.0;
            dydt[0] = r * y[0] * (1.0 - y[0] / k);
        };
        let y0 = [0.25];
        let t_end = 3.0;
        let t_eval: Vec<f64> = (0..=30).map(|i| i as f64 * t_end / 30.0).collect();
        let result = integrate_dop853(
            rhs,
            (0.0, t_end),
            &y0,
            &t_eval,
            &Dop853Options {
                rtol: 1e-11,
                atol: 1e-13,
                max_step: 0.05,
                initial_step: 0.0,
            },
        );
        assert!(result.success, "DOP853 failed: steps={}", result.n_steps);
        assert_eq!(result.t, t_eval);
        for (i, &ti) in result.t.iter().enumerate() {
            let exact = 2.0 / (1.0 + 7.0 * (-1.5 * ti).exp());
            let err = (result.y[i][0] - exact).abs();
            assert!(err < 1e-8, "t={ti:.3}: err = {err:.2e}");
        }
    }

    #[test]
    fn test_includes_initial_state_when_t_eval_starts_at_t0() {
        let rhs = |_t: f64, y: &[f64], dydt: &mut [f64]| {
            dydt[0] = -2.0 * y[0];
        };
        let t_eval = [0.0, 0.25, 0.5];
        let y0 = [3.0];
        let result = integrate_dop853(rhs, (0.0, 0.5), &y0, &t_eval, &Dop853Options::default());
        assert!(result.success, "DOP853 failed: steps={}", result.n_steps);
        assert_eq!(result.t, t_eval);
        assert_eq!(result.y[0], y0);
    }

    #[test]
    fn test_reports_partial_output_when_t_eval_exceeds_t_span() {
        let rhs = |_t: f64, y: &[f64], dydt: &mut [f64]| {
            dydt[0] = -y[0];
        };
        let t_eval = [0.0, 0.5, 1.0, 1.5];
        let result = integrate_dop853(rhs, (0.0, 1.0), &[1.0], &t_eval, &Dop853Options::default());
        assert!(!result.success, "expected partial output semantics");
        assert_eq!(result.t, vec![0.0, 0.5, 1.0]);
        assert_eq!(result.y.len(), result.t.len());
    }
}
