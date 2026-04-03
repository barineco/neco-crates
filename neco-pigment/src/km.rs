use crate::illuminant::N_SPECTRAL;

/// K/S spectrum representation (164 bytes per color).
#[derive(Clone, Debug)]
pub struct KsSpectrum {
    pub ks: [f32; N_SPECTRAL],
}

// Manual serde impl: derive not available for [f32; N] where N > 32.
#[cfg(feature = "serde")]
impl serde::Serialize for KsSpectrum {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        self.ks.as_slice().serialize(serializer)
    }
}

#[cfg(feature = "serde")]
impl<'de> serde::Deserialize<'de> for KsSpectrum {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let v: Vec<f32> = Vec::deserialize(deserializer)?;
        let ks: [f32; N_SPECTRAL] = v
            .try_into()
            .map_err(|_| serde::de::Error::custom(format!("expected array length {N_SPECTRAL}")))?;
        Ok(KsSpectrum { ks })
    }
}

/// Upper clamp for K/S (corresponds to R ~ 0.001).
pub(crate) const KS_MAX: f32 = 1000.0;

/// Threshold for switching to Taylor expansion.
const KS_TAYLOR_THRESHOLD: f32 = 1e-6;

/// Reflectance to K/S conversion (per wavelength, KM infinite thickness).
pub fn reflectance_to_ks(refl: &[f32; N_SPECTRAL]) -> KsSpectrum {
    let mut ks = [0.0f32; N_SPECTRAL];
    for (i, ks_i) in ks.iter_mut().enumerate().take(N_SPECTRAL) {
        let r = refl[i].clamp(0.0, 1.0);
        if r < 1e-10 {
            *ks_i = KS_MAX;
        } else {
            let val = (1.0 - r) * (1.0 - r) / (2.0 * r);
            *ks_i = val.min(KS_MAX);
        }
    }
    KsSpectrum { ks }
}

/// K/S to reflectance (KM infinite thickness).
///
/// Uses Taylor expansion for K/S < 1e-6 to avoid catastrophic cancellation.
pub fn ks_to_reflectance(ks: &KsSpectrum) -> [f32; N_SPECTRAL] {
    let mut refl = [0.0f32; N_SPECTRAL];
    for (i, refl_i) in refl.iter_mut().enumerate().take(N_SPECTRAL) {
        let x = ks.ks[i].max(0.0);
        if x >= KS_MAX {
            *refl_i = 0.0;
        } else if x < KS_TAYLOR_THRESHOLD {
            // Taylor expansion to avoid cancellation
            *refl_i = (1.0 - (2.0 * x).sqrt()).max(0.0);
        } else {
            *refl_i = (1.0 + x - (x * x + 2.0 * x).sqrt()).clamp(0.0, 1.0);
        }
    }
    refl
}

/// Linear interpolation in K/S space. `t` is not clamped (caller's responsibility).
pub fn ks_mix(a: &KsSpectrum, b: &KsSpectrum, t: f32) -> KsSpectrum {
    let mut ks = [0.0f32; N_SPECTRAL];
    let s = 1.0 - t;
    for (i, ks_i) in ks.iter_mut().enumerate().take(N_SPECTRAL) {
        *ks_i = s * a.ks[i] + t * b.ks[i];
    }
    KsSpectrum { ks }
}

/// Weighted blend of multiple colors in K/S space. Weights are not normalized.
pub fn ks_mix_weighted(colors: &[(&KsSpectrum, f32)]) -> KsSpectrum {
    let mut ks = [0.0f32; N_SPECTRAL];
    for (spectrum, weight) in colors {
        for (i, ks_i) in ks.iter_mut().enumerate().take(N_SPECTRAL) {
            *ks_i += weight * spectrum.ks[i];
        }
    }
    KsSpectrum { ks }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn max_abs_diff(a: &[f32; N_SPECTRAL], b: &[f32; N_SPECTRAL]) -> f32 {
        let mut max_err = 0.0f32;
        for i in 0..N_SPECTRAL {
            max_err = max_err.max((a[i] - b[i]).abs());
        }
        max_err
    }

    fn normalize_weighted(ks: &KsSpectrum, weight_sum: f32) -> [f32; N_SPECTRAL] {
        let mut out = [0.0f32; N_SPECTRAL];
        let inv = 1.0 / weight_sum;
        for (i, out_i) in out.iter_mut().enumerate().take(N_SPECTRAL) {
            *out_i = ks.ks[i] * inv;
        }
        out
    }

    #[test]
    fn ks_reflectance_round_trip() {
        // Mid-range reflectance
        let refl_in: [f32; N_SPECTRAL] = [0.5; N_SPECTRAL];
        let ks = reflectance_to_ks(&refl_in);
        let refl_out = ks_to_reflectance(&ks);
        for (i, (&in_i, &out_i)) in refl_in.iter().zip(refl_out.iter()).enumerate() {
            assert!((in_i - out_i).abs() < 1e-5, "i={i}");
        }
    }

    #[test]
    fn ks_black_stability() {
        let refl_in: [f32; N_SPECTRAL] = [0.0; N_SPECTRAL];
        let ks = reflectance_to_ks(&refl_in);
        // K/S should be clamped
        for &value in ks.ks.iter().take(N_SPECTRAL) {
            assert!(value <= KS_MAX + 1e-6);
        }
        // Should not panic on reconstruction
        let _refl_out = ks_to_reflectance(&ks);
    }

    #[test]
    fn ks_white_stability() {
        let refl_in: [f32; N_SPECTRAL] = [1.0; N_SPECTRAL];
        let ks = reflectance_to_ks(&refl_in);
        let refl_out = ks_to_reflectance(&ks);
        for (i, (&in_i, &out_i)) in refl_in.iter().zip(refl_out.iter()).enumerate() {
            assert!((in_i - out_i).abs() < 1e-5, "i={i}");
        }
    }

    #[test]
    fn ks_mix_interpolation() {
        let a = KsSpectrum {
            ks: [1.0; N_SPECTRAL],
        };
        let b = KsSpectrum {
            ks: [3.0; N_SPECTRAL],
        };
        let mixed = ks_mix(&a, &b, 0.5);
        for &value in mixed.ks.iter().take(N_SPECTRAL) {
            assert!((value - 2.0).abs() < 1e-6);
        }
    }

    #[test]
    fn ks_mix_weighted_empty() {
        let result = ks_mix_weighted(&[]);
        for &value in result.ks.iter().take(N_SPECTRAL) {
            assert_eq!(value, 0.0);
        }
    }

    #[test]
    fn ks_reflectance_round_trip_nonuniform_spectrum() {
        let mut refl_in = [0.0f32; N_SPECTRAL];
        for (i, v) in refl_in.iter_mut().enumerate().take(N_SPECTRAL) {
            let t = i as f32 / (N_SPECTRAL as f32 - 1.0);
            *v = (0.05 + 0.9 * t).clamp(0.0, 1.0);
        }
        let ks = reflectance_to_ks(&refl_in);
        let refl_out = ks_to_reflectance(&ks);
        for (i, &value) in refl_out.iter().enumerate() {
            assert!((0.0..=1.0).contains(&value), "out of range i={i}: {value}");
        }
        assert!(max_abs_diff(&refl_in, &refl_out) < 1e-5);
    }

    #[test]
    fn ks_mix_is_symmetric() {
        let a = KsSpectrum {
            ks: [0.2; N_SPECTRAL],
        };
        let b = KsSpectrum {
            ks: [3.7; N_SPECTRAL],
        };
        let lhs = ks_mix(&a, &b, 0.37);
        let rhs = ks_mix(&b, &a, 0.63);
        assert!(max_abs_diff(&lhs.ks, &rhs.ks) < 1e-6);
    }

    #[test]
    fn ks_mix_weighted_is_permutation_invariant() {
        let a = KsSpectrum {
            ks: [0.5; N_SPECTRAL],
        };
        let b = KsSpectrum {
            ks: [2.0; N_SPECTRAL],
        };
        let c = KsSpectrum {
            ks: [4.5; N_SPECTRAL],
        };
        let left = ks_mix_weighted(&[(&a, 0.2), (&b, 0.3), (&c, 0.5)]);
        let right = ks_mix_weighted(&[(&c, 0.5), (&a, 0.2), (&b, 0.3)]);
        assert!(max_abs_diff(&left.ks, &right.ks) < 1e-6);
    }

    #[test]
    fn ks_mix_weighted_scaling_invariant_after_normalization() {
        let a = KsSpectrum {
            ks: [0.3; N_SPECTRAL],
        };
        let b = KsSpectrum {
            ks: [1.4; N_SPECTRAL],
        };
        let c = KsSpectrum {
            ks: [5.0; N_SPECTRAL],
        };
        let base_weights = [0.2f32, 0.3f32, 0.5f32];
        let scale = 7.0f32;
        let scaled_weights = [
            base_weights[0] * scale,
            base_weights[1] * scale,
            base_weights[2] * scale,
        ];

        let base = ks_mix_weighted(&[
            (&a, base_weights[0]),
            (&b, base_weights[1]),
            (&c, base_weights[2]),
        ]);
        let scaled = ks_mix_weighted(&[
            (&a, scaled_weights[0]),
            (&b, scaled_weights[1]),
            (&c, scaled_weights[2]),
        ]);

        let base_norm = normalize_weighted(&base, base_weights.iter().sum());
        let scaled_norm = normalize_weighted(&scaled, scaled_weights.iter().sum());
        assert!(max_abs_diff(&base_norm, &scaled_norm) < 1e-6);
    }
}
