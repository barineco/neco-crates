/// 64-bit complex number type — drop-in replacement for `faer::c64`.
///
/// Provides all arithmetic operations used by FEAST / GMRES solvers.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct C64 {
    pub re: f64,
    pub im: f64,
}

// ---------------------------------------------------------------------------
// Constructors
// ---------------------------------------------------------------------------

impl C64 {
    #[inline]
    pub fn new(re: f64, im: f64) -> Self {
        Self { re, im }
    }

    #[inline]
    pub fn zero() -> Self {
        Self { re: 0.0, im: 0.0 }
    }

    #[inline]
    pub fn one() -> Self {
        Self { re: 1.0, im: 0.0 }
    }

    #[inline]
    pub fn from_real(re: f64) -> Self {
        Self { re, im: 0.0 }
    }
}

// ---------------------------------------------------------------------------
// Methods
// ---------------------------------------------------------------------------

impl C64 {
    /// Complex conjugate.
    #[inline]
    pub fn conj(self) -> Self {
        Self {
            re: self.re,
            im: -self.im,
        }
    }

    /// Squared magnitude: `re² + im²`.
    #[inline]
    pub fn norm_sq(self) -> f64 {
        self.re * self.re + self.im * self.im
    }

    /// Magnitude (absolute value): `sqrt(re² + im²)`.
    #[inline]
    pub fn norm(self) -> f64 {
        self.re.hypot(self.im)
    }
}

// ---------------------------------------------------------------------------
// Unary: Neg
// ---------------------------------------------------------------------------

impl core::ops::Neg for C64 {
    type Output = Self;
    #[inline]
    fn neg(self) -> Self {
        Self {
            re: -self.re,
            im: -self.im,
        }
    }
}

// ---------------------------------------------------------------------------
// Binary: C64 ⊕ C64
// ---------------------------------------------------------------------------

impl core::ops::Add for C64 {
    type Output = Self;
    #[inline]
    fn add(self, rhs: Self) -> Self {
        Self {
            re: self.re + rhs.re,
            im: self.im + rhs.im,
        }
    }
}

impl core::ops::Sub for C64 {
    type Output = Self;
    #[inline]
    fn sub(self, rhs: Self) -> Self {
        Self {
            re: self.re - rhs.re,
            im: self.im - rhs.im,
        }
    }
}

impl core::ops::Mul for C64 {
    type Output = Self;
    #[inline]
    fn mul(self, rhs: Self) -> Self {
        Self {
            re: self.re * rhs.re - self.im * rhs.im,
            im: self.re * rhs.im + self.im * rhs.re,
        }
    }
}

impl core::ops::Div for C64 {
    type Output = Self;
    #[inline]
    fn div(self, rhs: Self) -> Self {
        let d = rhs.norm_sq();
        Self {
            re: (self.re * rhs.re + self.im * rhs.im) / d,
            im: (self.im * rhs.re - self.re * rhs.im) / d,
        }
    }
}

// ---------------------------------------------------------------------------
// Compound assignment: C64 ⊕= C64
// ---------------------------------------------------------------------------

impl core::ops::AddAssign for C64 {
    #[inline]
    fn add_assign(&mut self, rhs: Self) {
        self.re += rhs.re;
        self.im += rhs.im;
    }
}

impl core::ops::SubAssign for C64 {
    #[inline]
    fn sub_assign(&mut self, rhs: Self) {
        self.re -= rhs.re;
        self.im -= rhs.im;
    }
}

impl core::ops::MulAssign for C64 {
    #[inline]
    fn mul_assign(&mut self, rhs: Self) {
        *self = *self * rhs;
    }
}

impl core::ops::DivAssign for C64 {
    #[inline]
    fn div_assign(&mut self, rhs: Self) {
        *self = *self / rhs;
    }
}

// ---------------------------------------------------------------------------
// Mixed-type: C64 * f64, f64 * C64, C64 / f64
// ---------------------------------------------------------------------------

impl core::ops::Mul<f64> for C64 {
    type Output = Self;
    #[inline]
    fn mul(self, rhs: f64) -> Self {
        Self {
            re: self.re * rhs,
            im: self.im * rhs,
        }
    }
}

impl core::ops::Mul<C64> for f64 {
    type Output = C64;
    #[inline]
    fn mul(self, rhs: C64) -> C64 {
        C64 {
            re: self * rhs.re,
            im: self * rhs.im,
        }
    }
}

impl core::ops::Div<f64> for C64 {
    type Output = Self;
    #[inline]
    fn div(self, rhs: f64) -> Self {
        Self {
            re: self.re / rhs,
            im: self.im / rhs,
        }
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    const EPS: f64 = 1e-14;

    fn approx_eq(a: C64, b: C64) -> bool {
        (a.re - b.re).abs() < EPS && (a.im - b.im).abs() < EPS
    }

    #[test]
    fn constructors() {
        assert_eq!(C64::zero(), C64::new(0.0, 0.0));
        assert_eq!(C64::one(), C64::new(1.0, 0.0));
        assert_eq!(C64::from_real(3.5), C64::new(3.5, 0.0));
    }

    #[test]
    fn add_sub() {
        let a = C64::new(1.0, 2.0);
        let b = C64::new(3.0, -1.0);
        assert_eq!(a + b, C64::new(4.0, 1.0));
        assert_eq!(a - b, C64::new(-2.0, 3.0));
    }

    #[test]
    fn mul_complex() {
        // (1+2i)(3-1i) = 3 -1i +6i -2i² = 5 + 5i
        let a = C64::new(1.0, 2.0);
        let b = C64::new(3.0, -1.0);
        assert_eq!(a * b, C64::new(5.0, 5.0));
    }

    #[test]
    fn div_complex() {
        // (5+5i)/(3-1i) should give back (1+2i)
        let num = C64::new(5.0, 5.0);
        let den = C64::new(3.0, -1.0);
        let q = num / den;
        assert!(approx_eq(q, C64::new(1.0, 2.0)));
    }

    #[test]
    fn neg() {
        assert_eq!(-C64::new(1.0, -2.0), C64::new(-1.0, 2.0));
    }

    #[test]
    fn compound_assign() {
        let mut z = C64::new(1.0, 2.0);
        z += C64::new(3.0, 4.0);
        assert_eq!(z, C64::new(4.0, 6.0));
        z -= C64::new(1.0, 1.0);
        assert_eq!(z, C64::new(3.0, 5.0));
        z *= C64::new(2.0, 0.0);
        assert_eq!(z, C64::new(6.0, 10.0));
        z /= C64::new(2.0, 0.0);
        assert!(approx_eq(z, C64::new(3.0, 5.0)));
    }

    #[test]
    fn mixed_f64() {
        let z = C64::new(2.0, 3.0);
        assert_eq!(z * 2.0, C64::new(4.0, 6.0));
        assert_eq!(2.0 * z, C64::new(4.0, 6.0));
        assert!(approx_eq(z / 2.0, C64::new(1.0, 1.5)));
    }

    #[test]
    fn conj_norm() {
        let z = C64::new(3.0, 4.0);
        assert_eq!(z.conj(), C64::new(3.0, -4.0));
        assert!((z.norm_sq() - 25.0).abs() < EPS);
        assert!((z.norm() - 5.0).abs() < EPS);
    }

    #[test]
    fn mul_by_real_via_new() {
        // Pattern from FEAST: z * c64::new(mv, 0.0)
        let z = C64::new(1.0, 2.0);
        let mv = 3.0;
        let result = z * C64::new(mv, 0.0);
        assert_eq!(result, z * mv);
    }

    #[test]
    fn div_by_zero() {
        let z = C64::new(1.0, 2.0);
        let result = z / C64::zero();
        // IEEE 754: division by zero yields Inf/NaN
        assert!(result.re.is_infinite() || result.re.is_nan());
        assert!(result.im.is_infinite() || result.im.is_nan());

        // Real zero division
        let result2 = z / 0.0;
        assert!(result2.re.is_infinite());
        assert!(result2.im.is_infinite());
    }

    #[test]
    fn identity_properties() {
        let z = C64::new(2.5, -1.3);
        // z * 1 = z
        assert_eq!(z * C64::one(), z);
        // z + 0 = z
        assert_eq!(z + C64::zero(), z);
        // z * conj(z) = |z|²  (real)
        let zz = z * z.conj();
        assert!(zz.im.abs() < EPS);
        assert!((zz.re - z.norm_sq()).abs() < EPS);
    }
}
