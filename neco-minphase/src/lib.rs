use neco_complex::Complex;
use neco_stft::{DspFloat, FftError};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MinPhaseError {
    InvalidGainCurveLen { expected: usize, got: usize },
    Fft(FftError),
}

impl core::fmt::Display for MinPhaseError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::InvalidGainCurveLen { expected, got } => {
                write!(f, "wrong gain curve length: expected {expected}, got {got}")
            }
            Self::Fft(err) => err.fmt(f),
        }
    }
}

impl std::error::Error for MinPhaseError {}

impl From<FftError> for MinPhaseError {
    fn from(value: FftError) -> Self {
        Self::Fft(value)
    }
}

pub fn compute_min_phase_spectrum<T: DspFloat>(
    gain_curve: &[T],
    fft_size: usize,
) -> Result<Vec<Complex<T>>, MinPhaseError> {
    let num_bins = fft_size / 2 + 1;
    if gain_curve.len() != num_bins {
        return Err(MinPhaseError::InvalidGainCurveLen {
            expected: num_bins,
            got: gain_curve.len(),
        });
    }

    let epsilon = T::from_f64(1e-20);
    let two = T::from_f64(2.0);

    T::with_fft_planner(|planner| {
        let fft_fwd = planner.plan_fft_forward(fft_size);
        let fft_inv = planner.plan_fft_inverse(fft_size);
        let scale = T::one() / T::from_usize(fft_size);

        let mut log_spectrum: Vec<Complex<T>> = gain_curve
            .iter()
            .map(|&gain| Complex::new(gain.max(epsilon).ln(), T::zero()))
            .collect();

        let mut cepstrum = fft_inv.make_output_vec();
        fft_inv.process(&mut log_spectrum, &mut cepstrum)?;
        for value in &mut cepstrum {
            *value *= scale;
        }

        let mut cepstrum_min = vec![T::zero(); fft_size];
        cepstrum_min[0] = cepstrum[0];
        for i in 1..fft_size / 2 {
            cepstrum_min[i] = two * cepstrum[i];
        }
        cepstrum_min[fft_size / 2] = cepstrum[fft_size / 2];

        let mut min_log_spectrum = fft_fwd.make_output_vec();
        fft_fwd.process(&mut cepstrum_min, &mut min_log_spectrum)?;

        for bin in &mut min_log_spectrum {
            let amplitude = bin.re.exp();
            let phase = bin.im;
            *bin = Complex::new(amplitude * phase.cos(), amplitude * phase.sin());
        }

        Ok(min_log_spectrum)
    })
}

pub fn compute_min_phase_ir<T: DspFloat>(
    gain_curve: &[T],
    fft_size: usize,
) -> Result<Vec<T>, MinPhaseError> {
    let mut min_spectrum = compute_min_phase_spectrum(gain_curve, fft_size)?;
    T::with_fft_planner(|planner| {
        let fft_inv = planner.plan_fft_inverse(fft_size);
        let scale = T::one() / T::from_usize(fft_size);
        let mut ir = fft_inv.make_output_vec();
        fft_inv.process(&mut min_spectrum, &mut ir)?;
        for sample in &mut ir {
            *sample *= scale;
        }
        Ok(ir)
    })
}

pub fn convolve_ola<T: DspFloat>(input: &[T], ir: &[T]) -> Result<Vec<T>, MinPhaseError> {
    let n = input.len();
    let m = ir.len();
    if n == 0 || m == 0 {
        return Ok(vec![T::zero(); n]);
    }

    let block_size = m.next_power_of_two();
    let conv_size = (block_size + m - 1).next_power_of_two();

    T::with_fft_planner(|planner| {
        let fft_fwd = planner.plan_fft_forward(conv_size);
        let fft_inv = planner.plan_fft_inverse(conv_size);
        let scale = T::one() / T::from_usize(conv_size);

        let mut ir_padded = vec![T::zero(); conv_size];
        ir_padded[..m].copy_from_slice(ir);
        let mut ir_spectrum = fft_fwd.make_output_vec();
        fft_fwd.process(&mut ir_padded, &mut ir_spectrum)?;

        let mut output = vec![T::zero(); n];
        let mut pos = 0usize;
        let mut block = vec![T::zero(); conv_size];
        let mut block_spectrum = fft_fwd.make_output_vec();
        let mut result = fft_inv.make_output_vec();

        while pos < n {
            let end = (pos + block_size).min(n);
            block.fill(T::zero());
            block[..end - pos].copy_from_slice(&input[pos..end]);

            fft_fwd.process(&mut block, &mut block_spectrum)?;
            for (lhs, rhs) in block_spectrum.iter_mut().zip(ir_spectrum.iter()) {
                let re = lhs.re * rhs.re - lhs.im * rhs.im;
                let im = lhs.re * rhs.im + lhs.im * rhs.re;
                lhs.re = re;
                lhs.im = im;
            }

            fft_inv.process(&mut block_spectrum, &mut result)?;
            for i in 0..conv_size {
                if pos + i < n {
                    output[pos + i] += result[i] * scale;
                }
            }

            pos += block_size;
        }

        Ok(output)
    })
}

pub fn compute_blend_curve(
    transient_map: &[f64],
    lookahead_samples: usize,
    smooth_samples: usize,
    threshold: f64,
) -> Vec<f64> {
    let n = transient_map.len();
    let mut raw_blend = vec![0.0; n];

    for (i, &value) in transient_map.iter().enumerate() {
        if value > threshold {
            let start = i.saturating_sub(lookahead_samples);
            let end = (i + lookahead_samples / 2).min(n);
            for item in &mut raw_blend[start..end] {
                *item = 1.0;
            }
        }
    }

    if smooth_samples < 2 {
        return raw_blend;
    }

    let half = smooth_samples / 2;
    let mut smoothed = vec![0.0; n];
    let mut running_sum = 0.0;
    for value in &raw_blend[..half.min(n)] {
        running_sum += *value;
    }

    for (i, out) in smoothed.iter_mut().enumerate() {
        let right = i + half;
        if right < n {
            running_sum += raw_blend[right];
        }
        if i > half + 1 {
            let left = i - half - 1;
            running_sum -= raw_blend[left];
        }
        let actual_window = (i + half + 1).min(n) - i.saturating_sub(half);
        *out = (running_sum / actual_window as f64).clamp(0.0, 1.0);
    }

    smoothed
}

#[cfg(test)]
mod tests {
    use std::f64::consts::PI;

    use neco_complex::Complex;
    use neco_stft::{cast_vec, DspFloat};

    use super::*;

    fn forward_spectrum<T: DspFloat>(input: &[T], fft_size: usize) -> Vec<Complex<T>> {
        T::with_fft_planner(|planner| {
            let fft = planner.plan_fft_forward(fft_size);
            let mut buffer = input.to_vec();
            let mut spectrum = fft.make_output_vec();
            fft.process(&mut buffer, &mut spectrum)
                .expect("fft buffers from planner");
            spectrum
        })
    }

    #[test]
    fn min_phase_rejects_wrong_gain_curve_len() {
        let err = compute_min_phase_spectrum(&[1.0f64, 2.0], 8).expect_err("invalid len");
        assert_eq!(
            err,
            MinPhaseError::InvalidGainCurveLen {
                expected: 5,
                got: 2,
            }
        );
    }

    #[test]
    fn min_phase_ir_has_correct_magnitude() {
        let fft_size = 4096;
        let num_bins = fft_size / 2 + 1;
        let sample_rate = 48000.0;
        let bin_freq = sample_rate / fft_size as f64;

        let gain_curve: Vec<f64> = (0..num_bins)
            .map(|i| {
                let f = i as f64 * bin_freq;
                let a = 10.0f64.powf(6.0 / 20.0);
                let bw = 1000.0 / 2.0;
                let x = (f - 1000.0) / (bw / 2.0);
                1.0 + (a - 1.0) / (1.0 + x * x)
            })
            .collect();

        let ir = compute_min_phase_ir(&gain_curve, fft_size).expect("min phase ir");
        let spectrum = forward_spectrum(&ir, fft_size);

        let max_err_db = (1..num_bins - 1)
            .filter_map(|i| {
                let actual_mag =
                    (spectrum[i].re * spectrum[i].re + spectrum[i].im * spectrum[i].im).sqrt();
                let expected_mag = gain_curve[i];
                (expected_mag > 0.01).then(|| (20.0 * (actual_mag / expected_mag).log10()).abs())
            })
            .fold(0.0, f64::max);

        assert!(max_err_db < 0.01, "magnitude error: {max_err_db:.4}dB");
    }

    #[test]
    fn min_phase_ir_is_causal() {
        let fft_size = 4096;
        let num_bins = fft_size / 2 + 1;
        let sample_rate = 48000.0;
        let bin_freq = sample_rate / fft_size as f64;

        let gain_curve: Vec<f64> = (0..num_bins)
            .map(|i| {
                let f = i as f64 * bin_freq;
                let a = 10.0f64.powf(6.0 / 20.0);
                let bw = 1000.0 / 2.0;
                let x = (f - 1000.0) / (bw / 2.0);
                1.0 + (a - 1.0) / (1.0 + x * x)
            })
            .collect();

        let ir = compute_min_phase_ir(&gain_curve, fft_size).expect("min phase ir");
        let quarter = fft_size / 4;
        let energy_front: f64 = ir[..quarter].iter().map(|x| x * x).sum();
        let energy_back: f64 = ir[3 * quarter..].iter().map(|x| x * x).sum();
        assert!(energy_front > energy_back * 100.0);
    }

    #[test]
    fn convolve_ola_identity() {
        let n = 8192;
        let input: Vec<f64> = (0..n)
            .map(|i| (2.0 * PI * 440.0 * i as f64 / 48000.0).sin())
            .collect();

        let mut ir = vec![0.0; 256];
        ir[0] = 1.0;
        let output = convolve_ola(&input, &ir).expect("convolve");
        let max_err = output
            .iter()
            .zip(input.iter())
            .map(|(&o, &i)| (o - i).abs())
            .fold(0.0, f64::max);
        assert!(max_err < 1e-10, "identity error: {max_err:.2e}");
    }

    #[test]
    fn blend_curve_stays_in_range() {
        let transient_map = vec![0.0, 0.2, 0.9, 0.8, 0.1, 0.0];
        let blend = compute_blend_curve(&transient_map, 2, 4, 0.3);
        assert_eq!(blend.len(), transient_map.len());
        assert!(blend.iter().all(|&value| (0.0..=1.0).contains(&value)));
        assert!(blend[0] > 0.0);
        assert!(blend[2] >= blend[5]);
    }

    #[test]
    fn min_phase_ir_f32_has_reasonable_magnitude() {
        let fft_size = 4096;
        let num_bins = fft_size / 2 + 1;
        let sample_rate = 48000.0;
        let bin_freq = sample_rate / fft_size as f64;

        let gain_curve_f64: Vec<f64> = (0..num_bins)
            .map(|i| {
                let f = i as f64 * bin_freq;
                let a = 10.0f64.powf(6.0 / 20.0);
                let bw = 1000.0 / 2.0;
                let x = (f - 1000.0) / (bw / 2.0);
                1.0 + (a - 1.0) / (1.0 + x * x)
            })
            .collect();
        let gain_curve_f32: Vec<f32> = cast_vec(&gain_curve_f64);

        let ir = compute_min_phase_ir(&gain_curve_f32, fft_size).expect("min phase ir");
        let spectrum = forward_spectrum(&ir, fft_size);

        let max_err_db = (1..num_bins - 1)
            .filter_map(|i| {
                let actual_mag =
                    (spectrum[i].re * spectrum[i].re + spectrum[i].im * spectrum[i].im).sqrt();
                let expected_mag = gain_curve_f32[i];
                (expected_mag > 0.01).then(|| (20.0f32 * (actual_mag / expected_mag).log10()).abs())
            })
            .fold(0.0, f32::max);

        assert!(max_err_db < 0.05, "magnitude error: {max_err_db:.4}dB");
    }

    #[test]
    fn min_phase_ir_non_power_of_two_has_reasonable_magnitude() {
        let fft_size = 1535;
        let num_bins = fft_size / 2 + 1;
        let sample_rate = 48000.0;
        let bin_freq = sample_rate / fft_size as f64;

        let gain_curve: Vec<f64> = (0..num_bins)
            .map(|i| {
                let f = i as f64 * bin_freq;
                let peak = 10.0f64.powf(4.0 / 20.0);
                let width = 800.0 / 2.0;
                let x = (f - 1800.0) / (width / 2.0);
                1.0 + (peak - 1.0) / (1.0 + x * x)
            })
            .collect();

        let ir = compute_min_phase_ir(&gain_curve, fft_size).expect("min phase ir");
        let spectrum = forward_spectrum(&ir, fft_size);

        let max_err_db = (1..num_bins - 1)
            .filter_map(|i| {
                let actual_mag =
                    (spectrum[i].re * spectrum[i].re + spectrum[i].im * spectrum[i].im).sqrt();
                let expected_mag = gain_curve[i];
                (expected_mag > 0.01).then(|| (20.0 * (actual_mag / expected_mag).log10()).abs())
            })
            .fold(0.0, f64::max);

        assert!(max_err_db < 0.03, "magnitude error: {max_err_db:.4}dB");
    }
}
