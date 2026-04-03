use std::ops::{Add, AddAssign, Div, DivAssign, Mul, MulAssign, Neg, Sub, SubAssign};

/// Lightweight complex number type shared by FFT- and solver-adjacent crates.
#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Complex<T> {
    pub re: T,
    pub im: T,
}

impl<T> Complex<T> {
    #[inline]
    pub fn new(re: T, im: T) -> Self {
        Self { re, im }
    }
}

impl Complex<f32> {
    #[inline]
    pub fn zero() -> Self {
        Self { re: 0.0, im: 0.0 }
    }

    #[inline]
    pub fn arg(self) -> f32 {
        self.im.atan2(self.re)
    }
}

impl Complex<f64> {
    #[inline]
    pub fn zero() -> Self {
        Self { re: 0.0, im: 0.0 }
    }

    #[inline]
    pub fn arg(self) -> f64 {
        self.im.atan2(self.re)
    }
}

impl<T> Complex<T>
where
    T: Copy + Neg<Output = T>,
{
    #[inline]
    pub fn conj(self) -> Self {
        Self {
            re: self.re,
            im: -self.im,
        }
    }
}

impl<T> Complex<T>
where
    T: Copy + Add<Output = T> + Mul<Output = T>,
{
    #[inline]
    pub fn norm_sqr(self) -> T {
        self.re * self.re + self.im * self.im
    }
}

impl<T> Neg for Complex<T>
where
    T: Neg<Output = T>,
{
    type Output = Self;

    fn neg(self) -> Self::Output {
        Self {
            re: -self.re,
            im: -self.im,
        }
    }
}

impl<T> Add for Complex<T>
where
    T: Add<Output = T>,
{
    type Output = Self;

    fn add(self, rhs: Self) -> Self::Output {
        Self {
            re: self.re + rhs.re,
            im: self.im + rhs.im,
        }
    }
}

impl<T> Sub for Complex<T>
where
    T: Sub<Output = T>,
{
    type Output = Self;

    fn sub(self, rhs: Self) -> Self::Output {
        Self {
            re: self.re - rhs.re,
            im: self.im - rhs.im,
        }
    }
}

impl<T> Mul for Complex<T>
where
    T: Copy + Add<Output = T> + Sub<Output = T> + Mul<Output = T>,
{
    type Output = Self;

    fn mul(self, rhs: Self) -> Self::Output {
        Self {
            re: self.re * rhs.re - self.im * rhs.im,
            im: self.re * rhs.im + self.im * rhs.re,
        }
    }
}

impl<T> Mul<T> for Complex<T>
where
    T: Copy + Mul<Output = T>,
{
    type Output = Self;

    fn mul(self, rhs: T) -> Self::Output {
        Self {
            re: self.re * rhs,
            im: self.im * rhs,
        }
    }
}

impl<T> Div<T> for Complex<T>
where
    T: Copy + Div<Output = T>,
{
    type Output = Self;

    fn div(self, rhs: T) -> Self::Output {
        Self {
            re: self.re / rhs,
            im: self.im / rhs,
        }
    }
}

impl<T> AddAssign for Complex<T>
where
    T: AddAssign,
{
    fn add_assign(&mut self, rhs: Self) {
        self.re += rhs.re;
        self.im += rhs.im;
    }
}

impl<T> SubAssign for Complex<T>
where
    T: SubAssign,
{
    fn sub_assign(&mut self, rhs: Self) {
        self.re -= rhs.re;
        self.im -= rhs.im;
    }
}

impl<T> MulAssign for Complex<T>
where
    T: Copy + Add<Output = T> + Sub<Output = T> + Mul<Output = T>,
{
    fn mul_assign(&mut self, rhs: Self) {
        *self = *self * rhs;
    }
}

impl<T> DivAssign<T> for Complex<T>
where
    T: Copy + Div<Output = T>,
{
    fn div_assign(&mut self, rhs: T) {
        *self = *self / rhs;
    }
}

#[cfg(test)]
mod tests {
    use super::Complex;

    #[test]
    fn arithmetic_matches_expected_values() {
        let a = Complex::new(1.0f64, 2.0);
        let b = Complex::new(3.0f64, -1.0);
        assert_eq!(a + b, Complex::new(4.0, 1.0));
        assert_eq!(a - b, Complex::new(-2.0, 3.0));
        assert_eq!(a * b, Complex::new(5.0, 5.0));
    }

    #[test]
    fn scalar_ops_and_conjugate_work() {
        let mut z = Complex::new(2.0f32, -4.0);
        z /= 2.0;
        assert_eq!(z, Complex::new(1.0, -2.0));
        assert_eq!(z * 2.0, Complex::new(2.0, -4.0));
        assert_eq!(z.conj(), Complex::new(1.0, 2.0));
    }

    #[test]
    fn norm_and_phase_match_reference() {
        let z = Complex::new(3.0f64, 4.0);
        assert!((z.norm_sqr() - 25.0).abs() < 1e-12);
        assert!((z.arg() - (4.0f64).atan2(3.0)).abs() < 1e-12);
    }

    #[test]
    fn zero_constructs_origin() {
        assert_eq!(Complex::<f64>::zero(), Complex::new(0.0, 0.0));
    }
}
