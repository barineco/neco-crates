use std::fmt;
use std::ops::{Add, Div, Mul, Neg, Sub};

/// Newtype wrapper for angles in radians.
#[derive(Clone, Copy, PartialEq, PartialOrd, Debug)]
pub struct Radians(pub f64);

fn parse_coeff(s: &str) -> Result<f64, String> {
    if s.is_empty() {
        Ok(1.0)
    } else if s == "-" {
        Ok(-1.0)
    } else if s == "+" {
        Ok(1.0)
    } else {
        s.parse::<f64>()
            .map_err(|e| format!("cannot parse '{s}' as number: {e}"))
    }
}

impl Radians {
    pub const PI: Radians = Radians(std::f64::consts::PI);
    pub const TAU: Radians = Radians(std::f64::consts::TAU);

    pub fn from_degrees(deg: f64) -> Radians {
        Radians(deg.to_radians())
    }

    /// Parse angle expression like `"pi"`, `"0.5pi"`, `"0.25tau"`. Lowercase only.
    pub fn from_expr(s: &str) -> Result<Radians, String> {
        let s = s.trim();
        if s.is_empty() {
            return Err("empty string is not a valid angle expression".to_string());
        }

        if let Some(coeff_str) = s.strip_suffix("tau") {
            let coeff = parse_coeff(coeff_str)?;
            Ok(Radians(coeff * std::f64::consts::TAU))
        } else if let Some(coeff_str) = s.strip_suffix("pi") {
            let coeff = parse_coeff(coeff_str)?;
            Ok(Radians(coeff * std::f64::consts::PI))
        } else {
            Err(format!(
                "'{s}' is not a valid angle expression (must end with 'pi' or 'tau')"
            ))
        }
    }

    /// Returns true if the angle is a full rotation (2pi).
    pub fn is_full_rotation(&self) -> bool {
        (self.0 - std::f64::consts::TAU).abs() < 1e-10
    }

    /// Convert to degrees.
    pub fn to_degrees(&self) -> f64 {
        self.0.to_degrees()
    }
}

impl fmt::Display for Radians {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:.6} rad", self.0)
    }
}

impl Add for Radians {
    type Output = Radians;
    fn add(self, rhs: Radians) -> Radians {
        Radians(self.0 + rhs.0)
    }
}

impl Sub for Radians {
    type Output = Radians;
    fn sub(self, rhs: Radians) -> Radians {
        Radians(self.0 - rhs.0)
    }
}

impl Mul<f64> for Radians {
    type Output = Radians;
    fn mul(self, rhs: f64) -> Radians {
        Radians(self.0 * rhs)
    }
}

impl Mul<Radians> for f64 {
    type Output = Radians;
    fn mul(self, rhs: Radians) -> Radians {
        Radians(self * rhs.0)
    }
}

impl Div<f64> for Radians {
    type Output = Radians;
    fn div(self, rhs: f64) -> Radians {
        Radians(self.0 / rhs)
    }
}

impl Neg for Radians {
    type Output = Radians;
    fn neg(self) -> Radians {
        Radians(-self.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::f64::consts::{PI, TAU};

    #[test]
    fn from_expr_pi() {
        let r = Radians::from_expr("pi").unwrap();
        assert!((r.0 - PI).abs() < 1e-15);
    }

    #[test]
    fn from_expr_tau() {
        let r = Radians::from_expr("tau").unwrap();
        assert!((r.0 - TAU).abs() < 1e-15);
    }

    #[test]
    fn from_expr_half_pi() {
        let r = Radians::from_expr("0.5pi").unwrap();
        assert!((r.0 - 0.5 * PI).abs() < 1e-15);
    }

    #[test]
    fn from_expr_neg_half_pi() {
        let r = Radians::from_expr("-0.5pi").unwrap();
        assert!((r.0 - (-0.5 * PI)).abs() < 1e-15);
    }

    #[test]
    fn from_expr_invalid() {
        assert!(Radians::from_expr("hello").is_err());
        assert!(Radians::from_expr("").is_err());
        assert!(Radians::from_expr("3.14").is_err());
    }

    #[test]
    fn is_full_rotation_tau() {
        assert!(Radians::TAU.is_full_rotation());
        assert!(Radians::from_degrees(360.0).is_full_rotation());
        assert!(!Radians::from_degrees(180.0).is_full_rotation());
    }

    #[test]
    fn arithmetic() {
        let a = Radians::PI;
        let b = Radians::PI;
        assert!((a + b).0 - TAU < 1e-15);
        assert!(((a + b) - a).0 - PI < 1e-15);
        assert!((a * 2.0).0 - TAU < 1e-15);
        assert!((2.0 * a).0 - TAU < 1e-15);
        assert!((Radians::TAU / 2.0).0 - PI < 1e-15);
        assert!((-a).0 - (-PI) < 1e-15);
    }
}
