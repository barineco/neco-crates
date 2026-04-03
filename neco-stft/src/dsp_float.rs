use std::cell::RefCell;

use crate::fft_backend::{FftPlanner, RustFftPlannerF32, RustFftPlannerF64};

pub trait DspFloat:
    Default
    + std::iter::Sum
    + std::ops::Add<Output = Self>
    + std::ops::AddAssign
    + std::ops::Sub<Output = Self>
    + std::ops::SubAssign
    + std::ops::Mul<Output = Self>
    + std::ops::MulAssign
    + std::ops::Div<Output = Self>
    + std::ops::DivAssign
    + std::ops::Neg<Output = Self>
    + Copy
    + PartialOrd
    + PartialEq
    + Send
    + Sync
    + std::fmt::Debug
    + 'static
{
    fn with_fft_planner<R>(f: impl FnOnce(&mut dyn FftPlanner<Self>) -> R) -> R;
    fn zero() -> Self;
    fn one() -> Self;
    fn from_f64(value: f64) -> Self;
    fn from_usize(value: usize) -> Self {
        Self::from_f64(value as f64)
    }
    fn pi() -> Self;
    fn sin(self) -> Self;
    fn cos(self) -> Self;
    fn exp(self) -> Self;
    fn ln(self) -> Self;
    fn sqrt(self) -> Self;
    fn abs(self) -> Self;
    fn max(self, other: Self) -> Self;
}

pub fn cast_vec<T: DspFloat>(src: &[f64]) -> Vec<T> {
    src.iter().map(|&value| T::from_f64(value)).collect()
}

thread_local! {
    static FFT_PLANNER_F64: RefCell<RustFftPlannerF64> = RefCell::new(RustFftPlannerF64::new());
    static FFT_PLANNER_F32: RefCell<RustFftPlannerF32> = RefCell::new(RustFftPlannerF32::new());
}

impl DspFloat for f64 {
    fn with_fft_planner<R>(f: impl FnOnce(&mut dyn FftPlanner<Self>) -> R) -> R {
        FFT_PLANNER_F64.with(|planner| f(&mut *planner.borrow_mut()))
    }

    fn zero() -> Self {
        0.0
    }

    fn one() -> Self {
        1.0
    }

    fn from_f64(value: f64) -> Self {
        value
    }

    fn pi() -> Self {
        std::f64::consts::PI
    }

    fn sin(self) -> Self {
        self.sin()
    }

    fn cos(self) -> Self {
        self.cos()
    }

    fn exp(self) -> Self {
        self.exp()
    }

    fn ln(self) -> Self {
        self.ln()
    }

    fn sqrt(self) -> Self {
        self.sqrt()
    }

    fn abs(self) -> Self {
        self.abs()
    }

    fn max(self, other: Self) -> Self {
        self.max(other)
    }
}

impl DspFloat for f32 {
    fn with_fft_planner<R>(f: impl FnOnce(&mut dyn FftPlanner<Self>) -> R) -> R {
        FFT_PLANNER_F32.with(|planner| f(&mut *planner.borrow_mut()))
    }

    fn zero() -> Self {
        0.0
    }

    fn one() -> Self {
        1.0
    }

    fn from_f64(value: f64) -> Self {
        value as f32
    }

    fn pi() -> Self {
        std::f32::consts::PI
    }

    fn sin(self) -> Self {
        self.sin()
    }

    fn cos(self) -> Self {
        self.cos()
    }

    fn exp(self) -> Self {
        self.exp()
    }

    fn ln(self) -> Self {
        self.ln()
    }

    fn sqrt(self) -> Self {
        self.sqrt()
    }

    fn abs(self) -> Self {
        self.abs()
    }

    fn max(self, other: Self) -> Self {
        self.max(other)
    }
}

pub fn with_planner<R>(f: impl FnOnce(&mut dyn FftPlanner<f64>) -> R) -> R {
    f64::with_fft_planner(f)
}
