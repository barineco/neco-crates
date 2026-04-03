use std::collections::HashMap;
use std::fmt;
use std::sync::Arc;

use neco_complex::Complex;

use crate::dsp_float::DspFloat;
use crate::internal_fft;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FftError {
    InputBuffer(usize, usize),
    OutputBuffer(usize, usize),
}

impl fmt::Display for FftError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InputBuffer(expected, got) => {
                write!(
                    f,
                    "wrong input buffer length: expected {expected}, got {got}"
                )
            }
            Self::OutputBuffer(expected, got) => {
                write!(
                    f,
                    "wrong output buffer length: expected {expected}, got {got}"
                )
            }
        }
    }
}

pub trait RealToComplex<T>: Send + Sync {
    fn process(&self, input: &mut [T], output: &mut [Complex<T>]) -> Result<(), FftError>;
    fn make_input_vec(&self) -> Vec<T>;
    fn make_output_vec(&self) -> Vec<Complex<T>>;
    fn len(&self) -> usize;
    fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

pub trait ComplexToReal<T>: Send + Sync {
    fn process(&self, input: &mut [Complex<T>], output: &mut [T]) -> Result<(), FftError>;
    fn make_input_vec(&self) -> Vec<Complex<T>>;
    fn make_output_vec(&self) -> Vec<T>;
    fn len(&self) -> usize;
    fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

pub trait FftPlanner<T> {
    fn plan_fft_forward(&mut self, len: usize) -> Arc<dyn RealToComplex<T>>;
    fn plan_fft_inverse(&mut self, len: usize) -> Arc<dyn ComplexToReal<T>>;
}

struct InternalR2C<T> {
    len: usize,
    _marker: std::marker::PhantomData<T>,
}

impl<T> InternalR2C<T> {
    fn new(len: usize) -> Self {
        Self {
            len,
            _marker: std::marker::PhantomData,
        }
    }
}

impl<T> RealToComplex<T> for InternalR2C<T>
where
    T: DspFloat,
{
    fn process(&self, input: &mut [T], output: &mut [Complex<T>]) -> Result<(), FftError> {
        if input.len() != self.len {
            return Err(FftError::InputBuffer(self.len, input.len()));
        }
        let expected = self.len / 2 + 1;
        if output.len() != expected {
            return Err(FftError::OutputBuffer(expected, output.len()));
        }
        let spectrum = internal_fft::real_fft_forward(input);
        output.copy_from_slice(&spectrum);
        Ok(())
    }

    fn make_input_vec(&self) -> Vec<T> {
        vec![T::zero(); self.len]
    }

    fn make_output_vec(&self) -> Vec<Complex<T>> {
        vec![Complex::new(T::zero(), T::zero()); self.len / 2 + 1]
    }

    fn len(&self) -> usize {
        self.len
    }
}

struct InternalC2R<T> {
    len: usize,
    _marker: std::marker::PhantomData<T>,
}

impl<T> InternalC2R<T> {
    fn new(len: usize) -> Self {
        Self {
            len,
            _marker: std::marker::PhantomData,
        }
    }
}

impl<T> ComplexToReal<T> for InternalC2R<T>
where
    T: DspFloat,
{
    fn process(&self, input: &mut [Complex<T>], output: &mut [T]) -> Result<(), FftError> {
        let expected_in = self.len / 2 + 1;
        if input.len() != expected_in {
            return Err(FftError::InputBuffer(expected_in, input.len()));
        }
        if output.len() != self.len {
            return Err(FftError::OutputBuffer(self.len, output.len()));
        }
        internal_fft::real_fft_inverse(input, output);
        Ok(())
    }

    fn make_input_vec(&self) -> Vec<Complex<T>> {
        vec![Complex::new(T::zero(), T::zero()); self.len / 2 + 1]
    }

    fn make_output_vec(&self) -> Vec<T> {
        vec![T::zero(); self.len]
    }

    fn len(&self) -> usize {
        self.len
    }
}

pub struct RustFftPlannerF32 {
    r2c_cache: HashMap<usize, Arc<dyn RealToComplex<f32>>>,
    c2r_cache: HashMap<usize, Arc<dyn ComplexToReal<f32>>>,
}

impl RustFftPlannerF32 {
    pub fn new() -> Self {
        Self {
            r2c_cache: HashMap::new(),
            c2r_cache: HashMap::new(),
        }
    }
}

impl Default for RustFftPlannerF32 {
    fn default() -> Self {
        Self::new()
    }
}

impl FftPlanner<f32> for RustFftPlannerF32 {
    fn plan_fft_forward(&mut self, len: usize) -> Arc<dyn RealToComplex<f32>> {
        self.r2c_cache
            .entry(len)
            .or_insert_with(|| Arc::new(InternalR2C::<f32>::new(len)))
            .clone()
    }

    fn plan_fft_inverse(&mut self, len: usize) -> Arc<dyn ComplexToReal<f32>> {
        self.c2r_cache
            .entry(len)
            .or_insert_with(|| Arc::new(InternalC2R::<f32>::new(len)))
            .clone()
    }
}

pub struct RustFftPlannerF64 {
    r2c_cache: HashMap<usize, Arc<dyn RealToComplex<f64>>>,
    c2r_cache: HashMap<usize, Arc<dyn ComplexToReal<f64>>>,
}

impl RustFftPlannerF64 {
    pub fn new() -> Self {
        Self {
            r2c_cache: HashMap::new(),
            c2r_cache: HashMap::new(),
        }
    }
}

impl Default for RustFftPlannerF64 {
    fn default() -> Self {
        Self::new()
    }
}

impl FftPlanner<f64> for RustFftPlannerF64 {
    fn plan_fft_forward(&mut self, len: usize) -> Arc<dyn RealToComplex<f64>> {
        self.r2c_cache
            .entry(len)
            .or_insert_with(|| Arc::new(InternalR2C::<f64>::new(len)))
            .clone()
    }

    fn plan_fft_inverse(&mut self, len: usize) -> Arc<dyn ComplexToReal<f64>> {
        self.c2r_cache
            .entry(len)
            .or_insert_with(|| Arc::new(InternalC2R::<f64>::new(len)))
            .clone()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn planner_roundtrip_f64_power_of_two() {
        let mut planner = RustFftPlannerF64::new();
        let fft_fwd = planner.plan_fft_forward(1024);
        let fft_inv = planner.plan_fft_inverse(1024);

        let input: Vec<f64> = (0..1024)
            .map(|i| (2.0 * std::f64::consts::PI * 440.0 * i as f64 / 48000.0).sin())
            .collect();

        let mut buf = input.clone();
        let mut spectrum = fft_fwd.make_output_vec();
        fft_fwd.process(&mut buf, &mut spectrum).unwrap();

        let mut output = fft_inv.make_output_vec();
        fft_inv.process(&mut spectrum, &mut output).unwrap();

        let scale = 1.0 / 1024.0;
        let max_err = output
            .iter()
            .zip(input.iter())
            .map(|(&o, &i)| (o * scale - i).abs())
            .fold(0.0, f64::max);
        assert!(max_err < 1e-10, "roundtrip error: {max_err:.2e}");
    }

    #[test]
    fn planner_roundtrip_f64_non_power_of_two() {
        let len = 1001;
        let mut planner = RustFftPlannerF64::new();
        let fft_fwd = planner.plan_fft_forward(len);
        let fft_inv = planner.plan_fft_inverse(len);

        let input: Vec<f64> = (0..len)
            .map(|i| {
                let t = i as f64 / len as f64;
                (2.0 * std::f64::consts::PI * 7.0 * t).sin()
                    + 0.4 * (2.0 * std::f64::consts::PI * 19.0 * t).cos()
            })
            .collect();

        let mut buf = input.clone();
        let mut spectrum = fft_fwd.make_output_vec();
        fft_fwd.process(&mut buf, &mut spectrum).unwrap();

        let mut output = fft_inv.make_output_vec();
        fft_inv.process(&mut spectrum, &mut output).unwrap();

        let scale = 1.0 / len as f64;
        let max_err = output
            .iter()
            .zip(input.iter())
            .map(|(&o, &i)| (o * scale - i).abs())
            .fold(0.0, f64::max);
        assert!(max_err < 1e-9, "roundtrip error: {max_err:.2e}");
    }

    #[test]
    fn planner_roundtrip_f32_non_power_of_two() {
        let len = 777;
        let mut planner = RustFftPlannerF32::new();
        let fft_fwd = planner.plan_fft_forward(len);
        let fft_inv = planner.plan_fft_inverse(len);

        let input: Vec<f32> = (0..len)
            .map(|i| {
                let t = i as f32 / len as f32;
                (2.0f32 * std::f32::consts::PI * 5.0 * t).sin()
                    + 0.25 * (2.0f32 * std::f32::consts::PI * 11.0 * t).cos()
            })
            .collect();

        let mut buf = input.clone();
        let mut spectrum = fft_fwd.make_output_vec();
        fft_fwd.process(&mut buf, &mut spectrum).unwrap();

        let mut output = fft_inv.make_output_vec();
        fft_inv.process(&mut spectrum, &mut output).unwrap();

        let scale = 1.0f32 / len as f32;
        let max_err = output
            .iter()
            .zip(input.iter())
            .map(|(&o, &i)| (o * scale - i).abs())
            .fold(0.0, f32::max);
        assert!(max_err < 5e-4, "roundtrip error: {max_err:.2e}");
    }
}
