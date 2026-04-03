use neco_complex::Complex;

use crate::dsp_float::DspFloat;
use crate::FftError;

pub type SpectrumFrame<T = f64> = Vec<Complex<T>>;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StftError {
    InvalidFrameLen { expected: usize, got: usize },
    Fft(FftError),
}

impl core::fmt::Display for StftError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::InvalidFrameLen { expected, got } => {
                write!(
                    f,
                    "wrong spectrum frame length: expected {expected}, got {got}"
                )
            }
            Self::Fft(err) => err.fmt(f),
        }
    }
}

impl std::error::Error for StftError {}

impl From<FftError> for StftError {
    fn from(value: FftError) -> Self {
        Self::Fft(value)
    }
}

pub struct StftProcessor<T: DspFloat = f64> {
    fft_size: usize,
    hop_size: usize,
    analysis_window: Vec<T>,
    synthesis_window: Vec<T>,
    norm: Vec<T>,
}

impl<T: DspFloat> StftProcessor<T> {
    pub fn new(fft_size: usize, hop_size: usize, window: Vec<T>) -> Self {
        assert_eq!(window.len(), fft_size);
        assert!(hop_size > 0 && hop_size <= fft_size);

        let norm = compute_wola_norm(&window, &window, fft_size, hop_size);
        Self {
            fft_size,
            hop_size,
            analysis_window: window.clone(),
            synthesis_window: window,
            norm,
        }
    }

    pub fn fft_size(&self) -> usize {
        self.fft_size
    }

    pub fn hop_size(&self) -> usize {
        self.hop_size
    }

    pub fn num_bins(&self) -> usize {
        self.fft_size / 2 + 1
    }

    pub fn analyze(&self, input: &[T]) -> Result<Vec<SpectrumFrame<T>>, StftError> {
        T::with_fft_planner(|planner| {
            let fft = planner.plan_fft_forward(self.fft_size);
            let mut frames = Vec::new();
            let mut pos = 0usize;
            let mut windowed = vec![T::zero(); self.fft_size];
            let mut spectrum = fft.make_output_vec();

            while pos + self.fft_size <= input.len() {
                for i in 0..self.fft_size {
                    windowed[i] = input[pos + i] * self.analysis_window[i];
                }
                fft.process(&mut windowed, &mut spectrum)?;
                frames.push(spectrum.clone());
                pos += self.hop_size;
            }

            Ok(frames)
        })
    }

    pub fn synthesize(
        &self,
        frames: &[SpectrumFrame<T>],
        output_len: usize,
    ) -> Result<Vec<T>, StftError> {
        T::with_fft_planner(|planner| {
            let ifft = planner.plan_fft_inverse(self.fft_size);
            let mut output = vec![T::zero(); output_len];
            let mut pos = 0usize;
            let scale = T::one() / T::from_usize(self.fft_size);
            let mut spectrum = ifft.make_input_vec();
            let mut time_buf = ifft.make_output_vec();

            for frame in frames {
                if pos + self.fft_size > output_len {
                    break;
                }

                if frame.len() != spectrum.len() {
                    return Err(StftError::InvalidFrameLen {
                        expected: spectrum.len(),
                        got: frame.len(),
                    });
                }
                spectrum.copy_from_slice(frame);
                ifft.process(&mut spectrum, &mut time_buf)?;

                for i in 0..self.fft_size {
                    output[pos + i] += time_buf[i] * scale * self.synthesis_window[i];
                }
                pos += self.hop_size;
            }

            self.apply_norm(&mut output);
            Ok(output)
        })
    }

    fn apply_norm(&self, output: &mut [T]) {
        let epsilon = T::from_f64(1e-10);
        let norm_len = self.norm.len();
        for (index, sample) in output.iter_mut().enumerate() {
            let norm_value = self.norm[index % norm_len];
            if norm_value > epsilon {
                *sample /= norm_value;
            }
        }
    }
}

fn compute_wola_norm<T: DspFloat>(
    analysis_window: &[T],
    synthesis_window: &[T],
    fft_size: usize,
    hop_size: usize,
) -> Vec<T> {
    let num_frames = fft_size.div_ceil(hop_size) + 1;
    let total_len = num_frames * hop_size + fft_size;
    let mut norm = vec![T::zero(); total_len];
    let mut pos = 0usize;
    for _ in 0..num_frames + 1 {
        if pos + fft_size <= total_len {
            for i in 0..fft_size {
                norm[pos + i] += analysis_window[i] * synthesis_window[i];
            }
        }
        pos += hop_size;
    }
    let steady_start = fft_size;
    norm[steady_start..steady_start + hop_size].to_vec()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{cast_vec, hann};

    #[test]
    fn roundtrip_identity() {
        let fft_size = 1024;
        let hop_size = 256;
        let processor = StftProcessor::new(fft_size, hop_size, hann(fft_size));

        let n = 16384;
        let input: Vec<f64> = (0..n)
            .map(|i| {
                let t = i as f64 / 48000.0;
                (2.0 * std::f64::consts::PI * 440.0 * t).sin()
                    + 0.5 * (2.0 * std::f64::consts::PI * 1000.0 * t).sin()
            })
            .collect();

        let frames = processor.analyze(&input).expect("analyze");
        let output = processor.synthesize(&frames, n).expect("synthesize");

        let margin = fft_size * 2;
        let max_err = (margin..n - margin)
            .map(|i| (output[i] - input[i]).abs())
            .fold(0.0, f64::max);
        assert!(max_err < 1e-10, "roundtrip error: {max_err:.2e}");
    }

    #[test]
    fn roundtrip_high_overlap() {
        let fft_size = 4096;
        let hop_size = 128;
        let processor = StftProcessor::new(fft_size, hop_size, hann(fft_size));

        let n = 32768;
        let input: Vec<f64> = (0..n)
            .map(|i| {
                let t = i as f64 / 48000.0;
                (2.0 * std::f64::consts::PI * 100.0 * t).sin()
            })
            .collect();

        let frames = processor.analyze(&input).expect("analyze");
        let output = processor.synthesize(&frames, n).expect("synthesize");

        let margin = fft_size * 2;
        let max_err = (margin..n - margin)
            .map(|i| (output[i] - input[i]).abs())
            .fold(0.0, f64::max);
        assert!(max_err < 1e-10, "roundtrip error: {max_err:.2e}");
    }

    #[test]
    fn roundtrip_f32() {
        let fft_size = 1024;
        let hop_size = 256;
        let window = cast_vec(&hann(fft_size));
        let processor = StftProcessor::<f32>::new(fft_size, hop_size, window);

        let n = 16384;
        let input: Vec<f32> = (0..n)
            .map(|i| {
                let t = i as f32 / 48000.0;
                (2.0f32 * std::f32::consts::PI * 440.0 * t).sin()
                    + 0.5 * (2.0f32 * std::f32::consts::PI * 1000.0 * t).sin()
            })
            .collect();

        let frames = processor.analyze(&input).expect("analyze");
        let output = processor.synthesize(&frames, n).expect("synthesize");

        let margin = fft_size * 2;
        let max_err = (margin..n - margin)
            .map(|i| (output[i] - input[i]).abs())
            .fold(0.0, f32::max);
        assert!(max_err < 1e-5, "roundtrip error: {max_err:.2e}");
    }

    #[test]
    fn roundtrip_non_power_of_two() {
        let fft_size = 1001;
        let hop_size = 143;
        let processor = StftProcessor::new(fft_size, hop_size, hann(fft_size));

        let n = 24024;
        let input: Vec<f64> = (0..n)
            .map(|i| {
                let t = i as f64 / 48000.0;
                (2.0 * std::f64::consts::PI * 330.0 * t).sin()
                    + 0.35 * (2.0 * std::f64::consts::PI * 1234.0 * t).cos()
            })
            .collect();

        let frames = processor.analyze(&input).expect("analyze");
        let output = processor.synthesize(&frames, n).expect("synthesize");

        let margin = fft_size * 2;
        let max_err = (margin..n - margin)
            .map(|i| (output[i] - input[i]).abs())
            .fold(0.0, f64::max);
        assert!(max_err < 1e-9, "roundtrip error: {max_err:.2e}");
    }

    #[test]
    fn synthesize_rejects_wrong_frame_len() {
        let processor = StftProcessor::new(16, 8, hann(16));
        let err = processor
            .synthesize(&[vec![Complex::new(0.0, 0.0); 3]], 16)
            .expect_err("invalid frame len");
        assert_eq!(
            err,
            StftError::InvalidFrameLen {
                expected: processor.num_bins(),
                got: 3,
            }
        );
    }
}
