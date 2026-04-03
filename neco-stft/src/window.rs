use std::f64::consts::PI;

pub fn hann(n: usize) -> Vec<f64> {
    (0..n)
        .map(|i| 0.5 * (1.0 - (2.0 * PI * i as f64 / n as f64).cos()))
        .collect()
}

pub fn kaiser_bessel_derived(n: usize, alpha: f64) -> Vec<f64> {
    let half = n / 2;
    let kaiser = kaiser(half + 1, alpha);

    let mut cumsum = vec![0.0; half + 1];
    cumsum[0] = kaiser[0];
    for i in 1..=half {
        cumsum[i] = cumsum[i - 1] + kaiser[i];
    }
    let total = cumsum[half];

    let mut window = vec![0.0; n];
    for i in 0..half {
        window[i] = (cumsum[i] / total).sqrt();
    }
    for i in 0..half {
        window[n - 1 - i] = window[i];
    }

    window
}

fn kaiser(n: usize, alpha: f64) -> Vec<f64> {
    let m = (n - 1) as f64;
    let denom = bessel_i0(PI * alpha);
    (0..n)
        .map(|i| {
            let t = 2.0 * i as f64 / m - 1.0;
            bessel_i0(PI * alpha * (1.0 - t * t).sqrt()) / denom
        })
        .collect()
}

fn bessel_i0(x: f64) -> f64 {
    let mut sum = 1.0;
    let mut term = 1.0;
    let x_half = x / 2.0;
    for k in 1..=30 {
        term *= (x_half / k as f64) * (x_half / k as f64);
        sum += term;
        if term < 1e-16 * sum {
            break;
        }
    }
    sum
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hann_endpoints_are_near_zero() {
        let window = hann(1024);
        assert!(window[0].abs() < 1e-15);
        assert!(window[1023] < 1e-3);
    }

    #[test]
    fn hann_squared_cola_for_quarter_hop() {
        let n = 1024;
        let hop = n / 4;
        let window = hann(n);
        let test_len = n * 4;
        let mut sum = vec![0.0; test_len];
        let mut pos = 0;
        while pos + n <= test_len {
            for i in 0..n {
                sum[pos + i] += window[i] * window[i];
            }
            pos += hop;
        }
        let steady_start = n;
        let steady_end = test_len - n;
        let ref_val = sum[steady_start];
        for value in &sum[steady_start..steady_end] {
            assert!((*value - ref_val).abs() < 1e-10);
        }
    }

    #[test]
    fn kbd_is_symmetric() {
        let window = kaiser_bessel_derived(2048, 4.0);
        for i in 0..1024 {
            assert!((window[i] - window[2047 - i]).abs() < 1e-12);
        }
    }
}
