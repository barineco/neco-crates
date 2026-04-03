use crate::{DampedModalSet, ModalSetError, SourceExcitation};

#[derive(Debug, Clone)]
pub struct OscillatorBank {
    rot_re: Vec<f64>,
    rot_im: Vec<f64>,
    state_re: Vec<f64>,
    state_im: Vec<f64>,
    source_amps_weighted: Vec<f64>,
    receiver_amps: Option<Vec<f64>>,
    mode_weights: Vec<f64>,
    mode_freqs: Vec<f64>,
    shapes_flat: Vec<f64>,
    n_modes: usize,
    n_nodes: usize,
}

impl OscillatorBank {
    pub fn new(
        modes: &DampedModalSet,
        source_amps: &[f64],
        sample_rate: f64,
        component: usize,
    ) -> Result<Self, ModalSetError> {
        if source_amps.len() != modes.len() {
            return Err(ModalSetError::SourceAmplitudeLengthMismatch {
                expected: modes.len(),
                actual: source_amps.len(),
            });
        }
        if !sample_rate.is_finite() || sample_rate <= 0.0 {
            return Err(ModalSetError::InvalidSampleRate { sample_rate });
        }

        let n_modes = modes.len();
        let n_nodes = modes.layout().n_nodes();
        let two_pi = 2.0 * std::f64::consts::PI;

        let mut rot_re = Vec::with_capacity(n_modes);
        let mut rot_im = Vec::with_capacity(n_modes);
        let mut weights = Vec::with_capacity(n_modes);
        let mut freqs = Vec::with_capacity(n_modes);
        let mut state_re = Vec::with_capacity(n_modes);
        let state_im = vec![0.0; n_modes];

        for (index, mode) in modes.iter().enumerate() {
            let omega = two_pi * mode.base().freq();
            let decay = (-mode.gamma() / sample_rate).exp();
            let phase = omega / sample_rate;
            rot_re.push(decay * phase.cos());
            rot_im.push(decay * phase.sin());
            weights.push(mode.base().weight());
            freqs.push(mode.base().freq());
            state_re.push(source_amps[index] * mode.base().weight());
        }

        let mut shapes_flat = Vec::with_capacity(n_modes * n_nodes);
        for mode_index in 0..n_modes {
            for node_index in 0..n_nodes {
                shapes_flat.push(modes.shape_value(mode_index, node_index, component)?);
            }
        }

        let source_amps_weighted = source_amps
            .iter()
            .zip(weights.iter())
            .map(|(amp, weight)| amp * weight)
            .collect();

        Ok(Self {
            rot_re,
            rot_im,
            state_re,
            state_im,
            source_amps_weighted,
            receiver_amps: None,
            mode_weights: weights,
            mode_freqs: freqs,
            shapes_flat,
            n_modes,
            n_nodes,
        })
    }

    pub fn new_multi(
        modes: &DampedModalSet,
        excitations: &[SourceExcitation],
        sample_rate: f64,
        component: usize,
    ) -> Result<Self, ModalSetError> {
        if !sample_rate.is_finite() || sample_rate <= 0.0 {
            return Err(ModalSetError::InvalidSampleRate { sample_rate });
        }
        let n_modes = modes.len();
        let n_nodes = modes.layout().n_nodes();
        let two_pi = 2.0 * std::f64::consts::PI;

        let mut rot_re = Vec::with_capacity(n_modes);
        let mut rot_im = Vec::with_capacity(n_modes);
        let mut weights = Vec::with_capacity(n_modes);
        let mut freqs = Vec::with_capacity(n_modes);
        let mut state_re = vec![0.0; n_modes];
        let mut state_im = vec![0.0; n_modes];

        for mode in modes.iter() {
            let omega = two_pi * mode.base().freq();
            let decay = (-mode.gamma() / sample_rate).exp();
            let phase = omega / sample_rate;
            rot_re.push(decay * phase.cos());
            rot_im.push(decay * phase.sin());
            weights.push(mode.base().weight());
            freqs.push(mode.base().freq());
        }

        let mut shapes_flat = Vec::with_capacity(n_modes * n_nodes);
        for mode_index in 0..n_modes {
            for node_index in 0..n_nodes {
                shapes_flat.push(modes.shape_value(mode_index, node_index, component)?);
            }
        }

        for mode_index in 0..n_modes {
            let omega = two_pi * freqs[mode_index];
            for excitation in excitations {
                let psi = modes.shape_value(mode_index, excitation.node(), component)?;
                let angle = excitation.phase() - omega * excitation.delay();
                let amplitude = excitation.gain() * weights[mode_index] * psi;
                state_re[mode_index] += amplitude * angle.cos();
                state_im[mode_index] += amplitude * angle.sin();
            }
        }

        Ok(Self {
            rot_re,
            rot_im,
            source_amps_weighted: state_re.clone(),
            receiver_amps: None,
            mode_weights: weights,
            mode_freqs: freqs,
            shapes_flat,
            n_modes,
            n_nodes,
            state_re,
            state_im,
        })
    }

    pub fn impulse(&mut self, source_amps: &[f64]) -> Result<(), ModalSetError> {
        if source_amps.len() != self.n_modes {
            return Err(ModalSetError::SourceAmplitudeLengthMismatch {
                expected: self.n_modes,
                actual: source_amps.len(),
            });
        }
        for (index, amp) in source_amps.iter().enumerate() {
            self.state_re[index] += amp * self.mode_weights[index];
        }
        Ok(())
    }

    pub fn impulse_multi(&mut self, excitations: &[SourceExcitation]) {
        let two_pi = 2.0 * std::f64::consts::PI;
        for mode_index in 0..self.n_modes {
            let omega = two_pi * self.mode_freqs[mode_index];
            for excitation in excitations {
                let psi = self.shapes_flat[mode_index * self.n_nodes + excitation.node()];
                let angle = excitation.phase() - omega * excitation.delay();
                let amplitude = excitation.gain() * self.mode_weights[mode_index] * psi;
                self.state_re[mode_index] += amplitude * angle.cos();
                self.state_im[mode_index] += amplitude * angle.sin();
            }
        }
    }

    pub fn set_receiver(&mut self, receiver_amps: &[f64]) -> Result<(), ModalSetError> {
        if receiver_amps.len() != self.n_modes {
            return Err(ModalSetError::SourceAmplitudeLengthMismatch {
                expected: self.n_modes,
                actual: receiver_amps.len(),
            });
        }
        self.receiver_amps = Some(
            receiver_amps
                .iter()
                .zip(self.mode_weights.iter())
                .map(|(amp, weight)| amp * weight)
                .collect(),
        );
        Ok(())
    }

    pub fn drive(&mut self, input: &[f32], output: &mut [f32]) {
        let len = input.len().min(output.len());
        if len == 0 || self.n_modes == 0 {
            return;
        }
        for sample_index in 0..len {
            let input_sample = input[sample_index] as f64;
            let mut sample = 0.0;
            for mode_index in 0..self.n_modes {
                let re = self.state_re[mode_index];
                let im = self.state_im[mode_index];
                let rot_re = self.rot_re[mode_index];
                let rot_im = self.rot_im[mode_index];
                let new_re = re * rot_re - im * rot_im;
                let new_im = re * rot_im + im * rot_re;
                self.state_re[mode_index] =
                    new_re + input_sample * self.source_amps_weighted[mode_index];
                self.state_im[mode_index] = new_im;
                sample += if let Some(receiver_amps) = &self.receiver_amps {
                    self.state_re[mode_index] * receiver_amps[mode_index]
                } else {
                    self.state_re[mode_index]
                };
            }
            output[sample_index] = sample as f32;
        }
    }

    pub fn drive_multi(
        &mut self,
        inputs: &[&[f32]],
        excitations: &[SourceExcitation],
        output: &mut [f32],
    ) {
        assert_eq!(
            inputs.len(),
            excitations.len(),
            "inputs and excitations must match"
        );
        let len = inputs
            .iter()
            .map(|input| input.len())
            .min()
            .unwrap_or(0)
            .min(output.len());
        if len == 0 || self.n_modes == 0 {
            output.fill(0.0);
            return;
        }
        let two_pi = 2.0 * std::f64::consts::PI;
        let mut inject_re = Vec::with_capacity(excitations.len());
        let mut inject_im = Vec::with_capacity(excitations.len());
        for excitation in excitations {
            let mut re_terms = Vec::with_capacity(self.n_modes);
            let mut im_terms = Vec::with_capacity(self.n_modes);
            for mode_index in 0..self.n_modes {
                let psi = self.shapes_flat[mode_index * self.n_nodes + excitation.node()];
                let omega = two_pi * self.mode_freqs[mode_index];
                let angle = excitation.phase() - omega * excitation.delay();
                let amplitude = excitation.gain() * self.mode_weights[mode_index] * psi;
                re_terms.push(amplitude * angle.cos());
                im_terms.push(amplitude * angle.sin());
            }
            inject_re.push(re_terms);
            inject_im.push(im_terms);
        }
        for sample_index in 0..len {
            let mut sample = 0.0;
            for mode_index in 0..self.n_modes {
                let re = self.state_re[mode_index];
                let im = self.state_im[mode_index];
                let rot_re = self.rot_re[mode_index];
                let rot_im = self.rot_im[mode_index];
                let mut new_re = re * rot_re - im * rot_im;
                let mut new_im = re * rot_im + im * rot_re;
                for input_index in 0..inputs.len() {
                    let x = inputs[input_index][sample_index] as f64;
                    new_re += x * inject_re[input_index][mode_index];
                    new_im += x * inject_im[input_index][mode_index];
                }
                self.state_re[mode_index] = new_re;
                self.state_im[mode_index] = new_im;
                sample += if let Some(receiver_amps) = &self.receiver_amps {
                    self.state_re[mode_index] * receiver_amps[mode_index]
                } else {
                    self.state_re[mode_index]
                };
            }
            output[sample_index] = sample as f32;
        }
    }

    pub fn process(&mut self, output: &mut [f32]) {
        if output.is_empty() || self.n_modes == 0 {
            return;
        }
        if !self.is_active() {
            output.fill(0.0);
            return;
        }
        let mut accum = vec![0.0; output.len()];
        for mode_index in 0..self.n_modes {
            let mut re = self.state_re[mode_index];
            let mut im = self.state_im[mode_index];
            let rot_re = self.rot_re[mode_index];
            let rot_im = self.rot_im[mode_index];
            for sample in &mut accum {
                *sample += re;
                let new_re = re * rot_re - im * rot_im;
                let new_im = re * rot_im + im * rot_re;
                re = new_re;
                im = new_im;
            }
            self.state_re[mode_index] = re;
            self.state_im[mode_index] = im;
        }
        for (dst, value) in output.iter_mut().zip(accum.into_iter()) {
            *dst = value as f32;
        }
    }

    pub fn field_weights(&self) -> Vec<f64> {
        self.state_re.clone()
    }

    pub fn compute_field(&self) -> Vec<f64> {
        let mut field = vec![0.0; self.n_nodes];
        for mode_index in 0..self.n_modes {
            let weight = self.state_re[mode_index];
            if weight.abs() < 1e-15 {
                continue;
            }
            let start = mode_index * self.n_nodes;
            let end = start + self.n_nodes;
            for (field_value, shape_value) in
                field.iter_mut().zip(self.shapes_flat[start..end].iter())
            {
                *field_value += weight * shape_value;
            }
        }
        field
    }

    pub fn is_active(&self) -> bool {
        self.state_re
            .iter()
            .zip(self.state_im.iter())
            .map(|(re, im)| re * re + im * im)
            .sum::<f64>()
            > 1e-20
    }

    pub fn n_modes(&self) -> usize {
        self.n_modes
    }

    pub fn n_nodes(&self) -> usize {
        self.n_nodes
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{ModalRecord, ShapeLayout};

    fn damped_modes(
        freqs: &[f64],
        gammas: &[f64],
        shapes: Vec<f64>,
        n_nodes: usize,
    ) -> DampedModalSet {
        let layout = ShapeLayout::new(1, n_nodes).unwrap();
        let modes = crate::ModalSet::new(
            freqs
                .iter()
                .enumerate()
                .map(|(index, freq)| {
                    let start = index * n_nodes;
                    let end = start + n_nodes;
                    ModalRecord::new(*freq, 1.0, shapes[start..end].to_vec(), None, None)
                })
                .collect(),
            layout,
        )
        .unwrap();
        modes.with_gammas(gammas).unwrap()
    }

    #[test]
    fn single_mode_frequency() {
        let modes = damped_modes(&[100.0], &[0.0], vec![1.0, 1.0], 2);
        let mut bank = OscillatorBank::new(&modes, &[1.0], 48_000.0, 0).unwrap();
        let mut output = vec![0.0; 480];
        bank.process(&mut output);
        assert!((output[0] - 1.0).abs() < 0.01);
        assert!((output[240] + 1.0).abs() < 0.01);
    }

    #[test]
    fn decays_over_time() {
        let modes = damped_modes(&[100.0], &[10.0], vec![1.0, 1.0], 2);
        let mut bank = OscillatorBank::new(&modes, &[1.0], 48_000.0, 0).unwrap();
        let mut output = vec![0.0; 48_000];
        bank.process(&mut output);
        assert!(output[0].abs() > 0.5);
        assert!(output[output.len() - 1].abs() < 0.001);
    }

    #[test]
    fn new_multi_matches_new_for_single_source() {
        let modes = damped_modes(&[100.0, 200.0], &[2.0, 3.0], vec![1.0, 0.5, 0.8, 0.3], 2);
        let mut linear = OscillatorBank::new(&modes, &[1.0, 0.8], 48_000.0, 0).unwrap();
        let excitation = SourceExcitation::new(0, 0.0, 1.0, 0.0).unwrap();
        let mut multi = OscillatorBank::new_multi(&modes, &[excitation], 48_000.0, 0).unwrap();
        let mut out_linear = vec![0.0; 480];
        let mut out_multi = vec![0.0; 480];
        linear.process(&mut out_linear);
        multi.process(&mut out_multi);
        for index in 0..out_linear.len() {
            assert!((out_linear[index] - out_multi[index]).abs() < 1e-6);
        }
    }

    #[test]
    fn drive_multi_matches_drive_for_single_source() {
        let modes = damped_modes(&[100.0, 200.0], &[2.0, 3.0], vec![1.0, 0.5, 0.8, 0.3], 2);
        let mut linear = OscillatorBank::new(&modes, &[1.0, 0.8], 48_000.0, 0).unwrap();
        let excitation = SourceExcitation::new(0, 0.0, 1.0, 0.0).unwrap();
        let mut multi =
            OscillatorBank::new_multi(&modes, std::slice::from_ref(&excitation), 48_000.0, 0)
                .unwrap();
        let input: Vec<f32> = (0..512)
            .map(|index| if index % 2 == 0 { 1.0 } else { -0.5 })
            .collect();
        let mut out_linear = vec![0.0; input.len()];
        let mut out_multi = vec![0.0; input.len()];
        linear.drive(&input, &mut out_linear);
        multi.drive_multi(&[&input], &[excitation], &mut out_multi);
        for index in 0..out_linear.len() {
            assert!((out_linear[index] - out_multi[index]).abs() < 1e-6);
        }
    }

    #[test]
    fn compute_field_reconstructs_shape() {
        let modes = damped_modes(&[100.0], &[0.0], vec![1.0, 0.5], 2);
        let bank = OscillatorBank::new(&modes, &[2.0], 48_000.0, 0).unwrap();
        let field = bank.compute_field();
        assert!((field[0] - 2.0).abs() < 1e-12);
        assert!((field[1] - 1.0).abs() < 1e-12);
    }
}
