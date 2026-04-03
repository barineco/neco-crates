use crate::{DampedModalSet, ModalSet, ModalSetError};

#[derive(Debug, Clone, PartialEq)]
pub struct SourceExcitation {
    node: usize,
    delay: f64,
    gain: f64,
    phase: f64,
}

impl SourceExcitation {
    pub fn new(node: usize, delay: f64, gain: f64, phase: f64) -> Result<Self, ModalSetError> {
        if !delay.is_finite() || delay < 0.0 {
            return Err(ModalSetError::InvalidExcitationDelay { delay });
        }
        if !gain.is_finite() {
            return Err(ModalSetError::InvalidExcitationGain { gain });
        }
        if !phase.is_finite() {
            return Err(ModalSetError::InvalidExcitationPhase { phase });
        }
        Ok(Self {
            node,
            delay,
            gain,
            phase,
        })
    }

    pub fn node(&self) -> usize {
        self.node
    }

    pub fn delay(&self) -> f64 {
        self.delay
    }

    pub fn gain(&self) -> f64 {
        self.gain
    }

    pub fn phase(&self) -> f64 {
        self.phase
    }
}

fn validate_time_grid(duration: f64, sample_rate: f64) -> Result<usize, ModalSetError> {
    if !duration.is_finite() || duration <= 0.0 {
        return Err(ModalSetError::InvalidDuration { duration });
    }
    if !sample_rate.is_finite() || sample_rate <= 0.0 {
        return Err(ModalSetError::InvalidSampleRate { sample_rate });
    }
    Ok((duration * sample_rate).ceil() as usize)
}

pub fn compute_source_amps(
    modes: &ModalSet,
    source_node: usize,
    receiver_node: usize,
    component: usize,
) -> Result<Vec<f64>, ModalSetError> {
    let mut amps = Vec::with_capacity(modes.len());
    for index in 0..modes.len() {
        let source = modes.shape_value(index, source_node, component)?;
        let receiver = modes.shape_value(index, receiver_node, component)?;
        amps.push(source * receiver);
    }
    Ok(amps)
}

pub fn compute_source_amps_multi(
    modes: &ModalSet,
    excitations: &[SourceExcitation],
    component: usize,
) -> Result<Vec<Vec<f64>>, ModalSetError> {
    excitations
        .iter()
        .map(|excitation| {
            let mut amps = Vec::with_capacity(modes.len());
            for index in 0..modes.len() {
                amps.push(modes.shape_value(index, excitation.node(), component)?);
            }
            Ok(amps)
        })
        .collect()
}

pub fn generate_ir(
    modes: &DampedModalSet,
    source_amps: &[f64],
    duration: f64,
    sample_rate: f64,
) -> Result<Vec<f64>, ModalSetError> {
    if source_amps.len() != modes.len() {
        return Err(ModalSetError::SourceAmplitudeLengthMismatch {
            expected: modes.len(),
            actual: source_amps.len(),
        });
    }
    let n_samples = validate_time_grid(duration, sample_rate)?;
    let dt = 1.0 / sample_rate;
    let two_pi = 2.0 * std::f64::consts::PI;
    let mut response = vec![0.0; n_samples];

    for (mode_index, mode) in modes.iter().enumerate() {
        let omega = two_pi * mode.base().freq();
        let amplitude = mode.base().weight() * source_amps[mode_index];
        let gamma = mode.gamma();
        for (sample_index, value) in response.iter_mut().enumerate() {
            let t = sample_index as f64 * dt;
            *value += amplitude * (-gamma * t).exp() * (omega * t).cos();
        }
    }

    Ok(response)
}

pub fn generate_ir_multi(
    modes: &DampedModalSet,
    excitations: &[SourceExcitation],
    receiver_node: usize,
    duration: f64,
    sample_rate: f64,
    component: usize,
) -> Result<Vec<f64>, ModalSetError> {
    let n_samples = validate_time_grid(duration, sample_rate)?;
    let dt = 1.0 / sample_rate;
    let two_pi = 2.0 * std::f64::consts::PI;
    let n_modes = modes.len();
    let mut response = vec![0.0; n_samples];

    let mut receiver_shapes = Vec::with_capacity(n_modes);
    for mode_index in 0..n_modes {
        receiver_shapes.push(modes.shape_value(mode_index, receiver_node, component)?);
    }

    let mut source_shapes = Vec::with_capacity(excitations.len());
    for excitation in excitations {
        let mut per_mode = Vec::with_capacity(n_modes);
        for mode_index in 0..n_modes {
            per_mode.push(modes.shape_value(mode_index, excitation.node(), component)?);
        }
        source_shapes.push(per_mode);
    }

    for (sample_index, sample) in response.iter_mut().enumerate() {
        let t = sample_index as f64 * dt;
        let mut sum = 0.0;
        for (source_index, excitation) in excitations.iter().enumerate() {
            let t_effective = t - excitation.delay();
            if t_effective < 0.0 {
                continue;
            }
            for mode_index in 0..n_modes {
                let mode = &modes.modes()[mode_index];
                let omega = two_pi * mode.base().freq();
                let amplitude = excitation.gain()
                    * mode.base().weight()
                    * source_shapes[source_index][mode_index]
                    * receiver_shapes[mode_index];
                sum += amplitude
                    * (-mode.gamma() * t_effective).exp()
                    * (omega * t_effective + excitation.phase()).cos();
            }
        }
        *sample = sum;
    }

    Ok(response)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{DampedModalSet, ModalRecord, ShapeLayout};

    fn single_mode(freq: f64, gamma: f64) -> DampedModalSet {
        let layout = ShapeLayout::new(1, 2).unwrap();
        let modal = crate::ModalSet::new(
            vec![ModalRecord::new(freq, 1.0, vec![1.0, 1.0], None, None)],
            layout,
        )
        .unwrap();
        modal.with_gammas(&[gamma]).unwrap()
    }

    #[test]
    fn ir_decays() {
        let modes = single_mode(100.0, 5.0);
        let amps = vec![1.0];
        let ir = generate_ir(&modes, &amps, 1.0, 48_000.0).unwrap();
        assert!(ir[0].abs() > 0.1);
        assert!(ir[ir.len() - 1].abs() < 0.01);
    }

    #[test]
    fn ir_frequency_matches_cosine() {
        let modes = single_mode(100.0, 0.0);
        let ir = generate_ir(&modes, &[1.0], 0.02, 48_000.0).unwrap();
        assert!((ir[0] - 1.0).abs() < 0.01);
        let quarter = (0.0025 * 48_000.0) as usize;
        let half = (0.005 * 48_000.0) as usize;
        assert!(ir[quarter].abs() < 0.01);
        assert!((ir[half] + 1.0).abs() < 0.01);
    }

    #[test]
    fn multi_single_source_matches_linear_ir() {
        let modes = single_mode(100.0, 5.0);
        let linear = generate_ir(&modes, &[1.0], 0.1, 48_000.0).unwrap();
        let excitation = SourceExcitation::new(0, 0.0, 1.0, 0.0).unwrap();
        let multi = generate_ir_multi(&modes, &[excitation], 1, 0.1, 48_000.0, 0).unwrap();
        assert_eq!(linear.len(), multi.len());
        for index in 0..linear.len() {
            assert!((linear[index] - multi[index]).abs() < 1e-12);
        }
    }

    #[test]
    fn multi_same_phase_doubles() {
        let modes = single_mode(100.0, 0.0);
        let excitation = SourceExcitation::new(0, 0.0, 1.0, 0.0).unwrap();
        let one = generate_ir_multi(
            &modes,
            std::slice::from_ref(&excitation),
            1,
            0.01,
            48_000.0,
            0,
        )
        .unwrap();
        let two = generate_ir_multi(
            &modes,
            &[excitation.clone(), excitation],
            1,
            0.01,
            48_000.0,
            0,
        )
        .unwrap();
        for index in 0..one.len() {
            assert!((two[index] - 2.0 * one[index]).abs() < 1e-12);
        }
    }

    #[test]
    fn multi_opposite_phase_cancels() {
        let modes = single_mode(100.0, 0.0);
        let first = SourceExcitation::new(0, 0.0, 1.0, 0.0).unwrap();
        let second = SourceExcitation::new(0, 0.0, 1.0, std::f64::consts::PI).unwrap();
        let ir = generate_ir_multi(&modes, &[first, second], 1, 0.01, 48_000.0, 0).unwrap();
        for value in ir {
            assert!(value.abs() < 1e-12);
        }
    }

    #[test]
    fn multi_delay_shifts_signal() {
        let modes = single_mode(100.0, 0.0);
        let sr: f64 = 48_000.0;
        let delay: f64 = 0.001;
        let delay_samples = (delay * sr).round() as usize;
        let first = SourceExcitation::new(0, 0.0, 1.0, 0.0).unwrap();
        let delayed = SourceExcitation::new(0, delay, 1.0, 0.0).unwrap();
        let early = generate_ir_multi(&modes, &[first], 1, 0.02, sr, 0).unwrap();
        let late = generate_ir_multi(&modes, &[delayed], 1, 0.02, sr, 0).unwrap();
        for value in late.iter().take(delay_samples) {
            assert!(value.abs() < 1e-12);
        }
        for index in delay_samples..early.len() {
            assert!((late[index] - early[index - delay_samples]).abs() < 1e-10);
        }
    }
}
