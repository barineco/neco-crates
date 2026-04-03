//! Modal extraction utilities for time-domain vibration signals.

mod fft_backend;
mod oscillator;
mod response;

use std::error::Error;
use std::fmt;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ShapeLayout {
    dof_per_node: usize,
    n_nodes: usize,
}

impl ShapeLayout {
    pub fn new(dof_per_node: usize, n_nodes: usize) -> Result<Self, ModalSetError> {
        if dof_per_node == 0 {
            return Err(ModalSetError::InvalidDofPerNode { dof_per_node });
        }
        if n_nodes == 0 {
            return Err(ModalSetError::InvalidNodeCount { n_nodes });
        }
        Ok(Self {
            dof_per_node,
            n_nodes,
        })
    }

    pub fn dof_per_node(&self) -> usize {
        self.dof_per_node
    }

    pub fn n_nodes(&self) -> usize {
        self.n_nodes
    }

    pub fn shape_len(&self) -> usize {
        self.dof_per_node * self.n_nodes
    }
}

#[derive(Debug, Clone)]
pub struct ModalRecord {
    freq: f64,
    weight: f64,
    shape: Vec<f64>,
    observed_damping: Option<f64>,
    quality: Option<f64>,
}

impl ModalRecord {
    pub fn new(
        freq: f64,
        weight: f64,
        shape: Vec<f64>,
        observed_damping: Option<f64>,
        quality: Option<f64>,
    ) -> Self {
        Self {
            freq,
            weight,
            shape,
            observed_damping,
            quality,
        }
    }

    pub fn freq(&self) -> f64 {
        self.freq
    }

    pub fn weight(&self) -> f64 {
        self.weight
    }

    pub fn shape(&self) -> &[f64] {
        &self.shape
    }

    pub fn observed_damping(&self) -> Option<f64> {
        self.observed_damping
    }

    pub fn quality(&self) -> Option<f64> {
        self.quality
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum ModalSetError {
    InvalidDofPerNode {
        dof_per_node: usize,
    },
    InvalidNodeCount {
        n_nodes: usize,
    },
    InvalidShapeLength {
        index: usize,
        expected: usize,
        actual: usize,
    },
    InvalidFrequency {
        index: usize,
        freq: f64,
    },
    InvalidWeight {
        index: usize,
        weight: f64,
    },
    InvalidObservedDamping {
        index: usize,
        value: f64,
    },
    InvalidQuality {
        index: usize,
        value: f64,
    },
    LayoutMismatch {
        lhs: ShapeLayout,
        rhs: ShapeLayout,
    },
    IndexOutOfBounds {
        index: usize,
        len: usize,
    },
    InvalidShapeNorm {
        index: usize,
        norm: f64,
    },
    InvalidComponentIndex {
        index: usize,
        dof_per_node: usize,
    },
    GammaLengthMismatch {
        expected: usize,
        actual: usize,
    },
    InvalidGamma {
        index: usize,
        value: f64,
    },
    InvalidNodeIndex {
        index: usize,
        n_nodes: usize,
    },
    InvalidDuration {
        duration: f64,
    },
    InvalidSampleRate {
        sample_rate: f64,
    },
    InvalidExcitationDelay {
        delay: f64,
    },
    InvalidExcitationGain {
        gain: f64,
    },
    InvalidExcitationPhase {
        phase: f64,
    },
    SourceAmplitudeLengthMismatch {
        expected: usize,
        actual: usize,
    },
}

impl fmt::Display for ModalSetError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidDofPerNode { dof_per_node } => write!(f, "dof_per_node must be > 0, got {dof_per_node}"),
            Self::InvalidNodeCount { n_nodes } => write!(f, "n_nodes must be > 0, got {n_nodes}"),
            Self::InvalidShapeLength { index, expected, actual } => {
                write!(f, "mode {index} shape length mismatch: expected {expected}, got {actual}")
            }
            Self::InvalidFrequency { index, freq } => write!(f, "mode {index} has invalid frequency {freq}"),
            Self::InvalidWeight { index, weight } => write!(f, "mode {index} has invalid weight {weight}"),
            Self::InvalidObservedDamping { index, value } => {
                write!(f, "mode {index} has invalid observed damping {value}")
            }
            Self::InvalidQuality { index, value } => write!(f, "mode {index} has invalid quality {value}"),
            Self::LayoutMismatch { lhs, rhs } => write!(
                f,
                "shape layout mismatch: lhs(dof_per_node={}, n_nodes={}), rhs(dof_per_node={}, n_nodes={})",
                lhs.dof_per_node(),
                lhs.n_nodes(),
                rhs.dof_per_node(),
                rhs.n_nodes()
            ),
            Self::IndexOutOfBounds { index, len } => {
                write!(f, "index {index} out of bounds for modal set of length {len}")
            }
            Self::InvalidShapeNorm { index, norm } => {
                write!(f, "mode {index} has invalid L2 shape norm {norm}")
            }
            Self::InvalidComponentIndex {
                index,
                dof_per_node,
            } => write!(
                f,
                "component index {index} out of bounds for layout with dof_per_node={dof_per_node}"
            ),
            Self::GammaLengthMismatch { expected, actual } => {
                write!(f, "gamma length mismatch: expected {expected}, got {actual}")
            }
            Self::InvalidGamma { index, value } => {
                write!(f, "mode {index} has invalid gamma {value}")
            }
            Self::InvalidNodeIndex { index, n_nodes } => {
                write!(f, "node index {index} out of bounds for modal layout with n_nodes={n_nodes}")
            }
            Self::InvalidDuration { duration } => {
                write!(f, "duration must be finite and > 0, got {duration}")
            }
            Self::InvalidSampleRate { sample_rate } => {
                write!(f, "sample_rate must be finite and > 0, got {sample_rate}")
            }
            Self::InvalidExcitationDelay { delay } => {
                write!(f, "excitation delay must be finite and >= 0, got {delay}")
            }
            Self::InvalidExcitationGain { gain } => {
                write!(f, "excitation gain must be finite, got {gain}")
            }
            Self::InvalidExcitationPhase { phase } => {
                write!(f, "excitation phase must be finite, got {phase}")
            }
            Self::SourceAmplitudeLengthMismatch { expected, actual } => {
                write!(f, "source amplitude length mismatch: expected {expected}, got {actual}")
            }
        }
    }
}

impl Error for ModalSetError {}

#[derive(Debug, Clone)]
pub struct ModalSet {
    modes: Vec<ModalRecord>,
    layout: ShapeLayout,
}

impl ModalSet {
    pub fn new(modes: Vec<ModalRecord>, layout: ShapeLayout) -> Result<Self, ModalSetError> {
        for (index, mode) in modes.iter().enumerate() {
            if mode.shape.len() != layout.shape_len() {
                return Err(ModalSetError::InvalidShapeLength {
                    index,
                    expected: layout.shape_len(),
                    actual: mode.shape.len(),
                });
            }
            if !mode.freq.is_finite() || mode.freq < 0.0 {
                return Err(ModalSetError::InvalidFrequency {
                    index,
                    freq: mode.freq,
                });
            }
            if !mode.weight.is_finite() {
                return Err(ModalSetError::InvalidWeight {
                    index,
                    weight: mode.weight,
                });
            }
            if let Some(value) = mode.observed_damping {
                if !value.is_finite() {
                    return Err(ModalSetError::InvalidObservedDamping { index, value });
                }
            }
            if let Some(value) = mode.quality {
                if !value.is_finite() {
                    return Err(ModalSetError::InvalidQuality { index, value });
                }
            }
        }
        Ok(Self { modes, layout })
    }

    pub fn len(&self) -> usize {
        self.modes.len()
    }

    pub fn is_empty(&self) -> bool {
        self.modes.is_empty()
    }

    pub fn layout(&self) -> ShapeLayout {
        self.layout
    }

    pub fn modes(&self) -> &[ModalRecord] {
        &self.modes
    }

    pub fn iter(&self) -> impl Iterator<Item = &ModalRecord> {
        self.modes.iter()
    }

    pub fn freqs(&self) -> Vec<f64> {
        self.modes.iter().map(ModalRecord::freq).collect()
    }

    pub fn weights(&self) -> Vec<f64> {
        self.modes.iter().map(ModalRecord::weight).collect()
    }

    pub fn shape(&self, index: usize) -> Option<&[f64]> {
        self.modes.get(index).map(ModalRecord::shape)
    }

    pub fn shape_value(
        &self,
        mode_index: usize,
        node_index: usize,
        component_index: usize,
    ) -> Result<f64, ModalSetError> {
        if mode_index >= self.modes.len() {
            return Err(ModalSetError::IndexOutOfBounds {
                index: mode_index,
                len: self.modes.len(),
            });
        }
        if node_index >= self.layout.n_nodes() {
            return Err(ModalSetError::InvalidNodeIndex {
                index: node_index,
                n_nodes: self.layout.n_nodes(),
            });
        }
        if component_index >= self.layout.dof_per_node() {
            return Err(ModalSetError::InvalidComponentIndex {
                index: component_index,
                dof_per_node: self.layout.dof_per_node(),
            });
        }
        let offset = node_index * self.layout.dof_per_node() + component_index;
        Ok(self.modes[mode_index].shape[offset])
    }

    pub fn filter_freq_min(&self, f_min: f64) -> Result<Self, ModalSetError> {
        let modes = self
            .modes
            .iter()
            .filter(|mode| mode.freq() >= f_min)
            .cloned()
            .collect();
        Self::new(modes, self.layout)
    }

    pub fn subset(&self, indices: &[usize]) -> Result<Self, ModalSetError> {
        let mut modes = Vec::with_capacity(indices.len());
        for &index in indices {
            let mode = self
                .modes
                .get(index)
                .cloned()
                .ok_or(ModalSetError::IndexOutOfBounds {
                    index,
                    len: self.modes.len(),
                })?;
            modes.push(mode);
        }
        Self::new(modes, self.layout)
    }

    pub fn sorted_by_freq(&self) -> Result<Self, ModalSetError> {
        let mut modes = self.modes.clone();
        modes.sort_by(|a, b| a.freq().total_cmp(&b.freq()));
        Self::new(modes, self.layout)
    }

    pub fn merge(&self, other: &Self) -> Result<Self, ModalSetError> {
        if self.layout != other.layout {
            return Err(ModalSetError::LayoutMismatch {
                lhs: self.layout,
                rhs: other.layout,
            });
        }
        let mut modes = self.modes.clone();
        modes.extend(other.modes.iter().cloned());
        modes.sort_by(|a, b| a.freq().total_cmp(&b.freq()));
        Self::new(modes, self.layout)
    }

    pub fn normalized_shapes_l2(&self) -> Result<Self, ModalSetError> {
        let mut modes = Vec::with_capacity(self.modes.len());
        for (index, mode) in self.modes.iter().enumerate() {
            let norm = mode
                .shape
                .iter()
                .map(|value| value * value)
                .sum::<f64>()
                .sqrt();
            if !norm.is_finite() || norm <= 0.0 {
                return Err(ModalSetError::InvalidShapeNorm { index, norm });
            }
            let shape = mode.shape.iter().map(|value| value / norm).collect();
            modes.push(ModalRecord::new(
                mode.freq,
                mode.weight,
                shape,
                mode.observed_damping,
                mode.quality,
            ));
        }
        Self::new(modes, self.layout)
    }

    pub fn component(&self, index: usize) -> Result<Self, ModalSetError> {
        self.components(&[index])
    }

    pub fn components(&self, indices: &[usize]) -> Result<Self, ModalSetError> {
        for &index in indices {
            if index >= self.layout.dof_per_node() {
                return Err(ModalSetError::InvalidComponentIndex {
                    index,
                    dof_per_node: self.layout.dof_per_node(),
                });
            }
        }

        let layout = ShapeLayout::new(indices.len(), self.layout.n_nodes())?;
        let stride = self.layout.dof_per_node();
        let mut modes = Vec::with_capacity(self.modes.len());
        for mode in &self.modes {
            let mut shape = Vec::with_capacity(layout.shape_len());
            for node in 0..self.layout.n_nodes() {
                let base = node * stride;
                for &index in indices {
                    shape.push(mode.shape[base + index]);
                }
            }
            modes.push(ModalRecord::new(
                mode.freq,
                mode.weight,
                shape,
                mode.observed_damping,
                mode.quality,
            ));
        }
        Self::new(modes, layout)
    }

    pub fn with_gammas(&self, gammas: &[f64]) -> Result<DampedModalSet, ModalSetError> {
        if gammas.len() != self.modes.len() {
            return Err(ModalSetError::GammaLengthMismatch {
                expected: self.modes.len(),
                actual: gammas.len(),
            });
        }
        let modes = self
            .modes
            .iter()
            .cloned()
            .zip(gammas.iter().copied())
            .map(|(base, gamma)| DampedModalRecord { base, gamma })
            .collect();
        DampedModalSet::new(modes, self.layout)
    }
}

#[derive(Debug, Clone)]
pub struct DampedModalRecord {
    base: ModalRecord,
    gamma: f64,
}

impl DampedModalRecord {
    pub fn new(base: ModalRecord, gamma: f64) -> Self {
        Self { base, gamma }
    }

    pub fn base(&self) -> &ModalRecord {
        &self.base
    }

    pub fn gamma(&self) -> f64 {
        self.gamma
    }
}

#[derive(Debug, Clone)]
pub struct DampedModalSet {
    modes: Vec<DampedModalRecord>,
    layout: ShapeLayout,
}

impl DampedModalSet {
    pub fn new(modes: Vec<DampedModalRecord>, layout: ShapeLayout) -> Result<Self, ModalSetError> {
        let base_modes: Vec<ModalRecord> = modes.iter().map(|mode| mode.base.clone()).collect();
        let _ = ModalSet::new(base_modes, layout)?;
        for (index, mode) in modes.iter().enumerate() {
            if !mode.gamma.is_finite() {
                return Err(ModalSetError::InvalidGamma {
                    index,
                    value: mode.gamma,
                });
            }
        }
        Ok(Self { modes, layout })
    }

    pub fn len(&self) -> usize {
        self.modes.len()
    }

    pub fn is_empty(&self) -> bool {
        self.modes.is_empty()
    }

    pub fn layout(&self) -> ShapeLayout {
        self.layout
    }

    pub fn modes(&self) -> &[DampedModalRecord] {
        &self.modes
    }

    pub fn iter(&self) -> impl Iterator<Item = &DampedModalRecord> {
        self.modes.iter()
    }

    pub fn freqs(&self) -> Vec<f64> {
        self.modes.iter().map(|mode| mode.base().freq()).collect()
    }

    pub fn weights(&self) -> Vec<f64> {
        self.modes.iter().map(|mode| mode.base().weight()).collect()
    }

    pub fn gammas(&self) -> Vec<f64> {
        self.modes.iter().map(DampedModalRecord::gamma).collect()
    }

    pub fn shape(&self, index: usize) -> Option<&[f64]> {
        self.modes.get(index).map(|mode| mode.base().shape())
    }

    pub fn shape_value(
        &self,
        mode_index: usize,
        node_index: usize,
        component_index: usize,
    ) -> Result<f64, ModalSetError> {
        if mode_index >= self.modes.len() {
            return Err(ModalSetError::IndexOutOfBounds {
                index: mode_index,
                len: self.modes.len(),
            });
        }
        if node_index >= self.layout.n_nodes() {
            return Err(ModalSetError::InvalidNodeIndex {
                index: node_index,
                n_nodes: self.layout.n_nodes(),
            });
        }
        if component_index >= self.layout.dof_per_node() {
            return Err(ModalSetError::InvalidComponentIndex {
                index: component_index,
                dof_per_node: self.layout.dof_per_node(),
            });
        }
        let offset = node_index * self.layout.dof_per_node() + component_index;
        Ok(self.modes[mode_index].base().shape()[offset])
    }
}

pub use oscillator::OscillatorBank;
pub use response::{
    compute_source_amps, compute_source_amps_multi, generate_ir, generate_ir_multi,
    SourceExcitation,
};

#[derive(Debug, Clone)]
pub struct ModeInfo {
    pub freq: f64,
    pub damping_rate: f64,
    pub amplitude: f64,
    pub phase: f64,
    pub quality: f64,
}

pub fn extract_modes(
    readout: &[f64],
    dt: f64,
    n_modes_max: usize,
    threshold_db: f64,
) -> Vec<ModeInfo> {
    let n = readout.len();
    if n < 4 || !dt.is_finite() || dt <= 0.0 {
        return Vec::new();
    }

    let spectrum = fft_backend::hann_window_spectrum(readout);
    let power = spectrum.power();
    let nfft = n.next_power_of_two();
    let n_pos = nfft / 2;
    let df = 1.0 / (nfft as f64 * dt);
    let max_power = power.iter().cloned().fold(0.0_f64, f64::max);
    if max_power < 1e-30 {
        return Vec::new();
    }

    let threshold = max_power * 10.0_f64.powf(threshold_db / 10.0);
    let min_bin = 3;
    let mut peaks: Vec<(usize, f64)> = Vec::new();
    for i in min_bin..n_pos - 1 {
        if power[i] > power[i - 1] && power[i] > power[i + 1] && power[i] > threshold {
            peaks.push((i, power[i]));
        }
    }
    peaks.sort_by(|a, b| b.1.total_cmp(&a.1));
    peaks.truncate(n_modes_max);

    let mut modes = Vec::new();
    for (bin, peak_power) in &peaks {
        let bin = *bin;
        let alpha_val = power[bin - 1];
        let beta = power[bin];
        let gamma = power[bin + 1];
        let denom = alpha_val - 2.0 * beta + gamma;
        let delta_bin = if denom.abs() > 1e-30 {
            0.5 * (alpha_val - gamma) / denom
        } else {
            0.0
        };
        let freq_refined = (bin as f64 + delta_bin) * df;
        let phase = spectrum.phase_at(bin);
        let half_power = peak_power * 0.5;
        let mut left_bin = bin;
        while left_bin > min_bin && power[left_bin] > half_power {
            left_bin -= 1;
        }
        let mut right_bin = bin;
        while right_bin < n_pos - 2 && power[right_bin] > half_power {
            right_bin += 1;
        }
        let bandwidth = (right_bin as f64 - left_bin as f64) * df;
        let window_bw = 1.44 * df;
        let signal_bw = (bandwidth - window_bw).max(df * 0.1);
        let damping_rate = std::f64::consts::PI * signal_bw;
        let amplitude = peak_power.sqrt();
        let q = if damping_rate > 1e-10 {
            std::f64::consts::PI * freq_refined / damping_rate
        } else {
            f64::INFINITY
        };
        modes.push(ModeInfo {
            freq: freq_refined,
            damping_rate,
            amplitude,
            phase,
            quality: q,
        });
    }
    modes.sort_by(|a, b| a.freq.total_cmp(&b.freq));
    modes
}

pub fn extract_spatial_modes(
    snapshots: &[&[f64]],
    snapshot_times: &[f64],
    mode_freqs: &[f64],
    nx: usize,
    ny: usize,
) -> Vec<SpatialMode> {
    let n_snaps = snapshots.len();
    if n_snaps < 2 || snapshot_times.len() != n_snaps || mode_freqs.is_empty() || nx == 0 || ny == 0
    {
        return Vec::new();
    }
    let duration = snapshot_times[n_snaps - 1] - snapshot_times[0];
    if !duration.is_finite() || duration <= 0.0 {
        return Vec::new();
    }
    let n_cells = nx * ny;
    let mut modes = Vec::new();
    for &freq in mode_freqs {
        if !freq.is_finite() {
            continue;
        }
        let omega = 2.0 * std::f64::consts::PI * freq;
        let mut shape_real = vec![0.0_f64; n_cells];
        let mut shape_imag = vec![0.0_f64; n_cells];
        for (k, snap) in snapshots.iter().enumerate() {
            let t = snapshot_times[k] - snapshot_times[0];
            let hann = 0.5 * (1.0 - (2.0 * std::f64::consts::PI * t / duration).cos());
            let cos_val = (omega * t).cos() * hann;
            let sin_val = (omega * t).sin() * hann;
            for c in 0..n_cells.min(snap.len()) {
                shape_real[c] += snap[c] * cos_val;
                shape_imag[c] += snap[c] * sin_val;
            }
        }
        let shape: Vec<f64> = shape_real
            .iter()
            .zip(shape_imag.iter())
            .map(|(r, i)| (r * r + i * i).sqrt())
            .collect();
        let m_estimated = estimate_circumferential_order(&shape, nx, ny);
        modes.push(SpatialMode {
            freq,
            shape,
            m_estimated,
        });
    }
    modes
}

#[derive(Debug, Clone)]
pub struct SpatialMode {
    pub freq: f64,
    pub shape: Vec<f64>,
    pub m_estimated: i32,
}

fn estimate_circumferential_order(shape: &[f64], nx: usize, ny: usize) -> i32 {
    if nx == 0 || ny == 0 || shape.len() < nx.saturating_mul(ny) {
        return 0;
    }
    let cx = nx as f64 / 2.0;
    let cy = ny as f64 / 2.0;
    let r_sample = (cx.min(cy)) * 0.6;
    let n_samples = 64;
    let mut values = Vec::with_capacity(n_samples);
    for k in 0..n_samples {
        let theta = 2.0 * std::f64::consts::PI * k as f64 / n_samples as f64;
        let fi = cx + r_sample * theta.cos();
        let fj = cy + r_sample * theta.sin();
        let i = (fi.round() as usize).min(nx - 1);
        let j = (fj.round() as usize).min(ny - 1);
        values.push(shape[i * ny + j]);
    }
    let mean: f64 = values.iter().sum::<f64>() / values.len() as f64;
    let mut crossings = 0;
    for k in 0..values.len() {
        let a = values[k] - mean;
        let b = values[(k + 1) % values.len()] - mean;
        if a * b < 0.0 {
            crossings += 1;
        }
    }
    crossings / 2
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_layout() -> ShapeLayout {
        ShapeLayout::new(2, 3).unwrap()
    }

    fn sample_modes() -> Vec<ModalRecord> {
        vec![
            ModalRecord::new(
                220.0,
                0.8,
                vec![1.0, 0.0, 0.5, 0.1, 0.2, 0.3],
                Some(0.5),
                Some(8.0),
            ),
            ModalRecord::new(110.0, 1.0, vec![0.2, 0.1, 0.3, 0.4, 0.0, 0.5], None, None),
            ModalRecord::new(
                330.0,
                0.4,
                vec![0.0, 0.3, 0.4, 0.2, 0.1, 0.6],
                Some(0.2),
                Some(12.0),
            ),
        ]
    }

    #[test]
    fn modal_set_validates_shape_length() {
        let layout = ShapeLayout::new(2, 2).unwrap();
        let err = ModalSet::new(
            vec![ModalRecord::new(
                100.0,
                1.0,
                vec![1.0, 2.0, 3.0],
                None,
                None,
            )],
            layout,
        )
        .unwrap_err();
        assert_eq!(
            err,
            ModalSetError::InvalidShapeLength {
                index: 0,
                expected: 4,
                actual: 3
            }
        );
    }

    #[test]
    fn shape_layout_rejects_zero_dimensions() {
        assert_eq!(
            ShapeLayout::new(0, 3).unwrap_err(),
            ModalSetError::InvalidDofPerNode { dof_per_node: 0 }
        );
        assert_eq!(
            ShapeLayout::new(2, 0).unwrap_err(),
            ModalSetError::InvalidNodeCount { n_nodes: 0 }
        );
    }

    #[test]
    fn modal_set_rejects_invalid_frequency() {
        let err = ModalSet::new(
            vec![ModalRecord::new(-1.0, 1.0, vec![1.0; 6], None, None)],
            sample_layout(),
        )
        .unwrap_err();
        assert_eq!(
            err,
            ModalSetError::InvalidFrequency {
                index: 0,
                freq: -1.0
            }
        );
    }

    #[test]
    fn modal_set_rejects_invalid_weight() {
        let err = ModalSet::new(
            vec![ModalRecord::new(10.0, f64::NAN, vec![1.0; 6], None, None)],
            sample_layout(),
        )
        .unwrap_err();
        assert!(matches!(
            err,
            ModalSetError::InvalidWeight {
                index: 0,
                weight
            } if weight.is_nan()
        ));
    }

    #[test]
    fn modal_set_rejects_invalid_optional_quantities() {
        let damping_err = ModalSet::new(
            vec![ModalRecord::new(
                10.0,
                1.0,
                vec![1.0; 6],
                Some(f64::INFINITY),
                None,
            )],
            sample_layout(),
        )
        .unwrap_err();
        assert!(matches!(
            damping_err,
            ModalSetError::InvalidObservedDamping {
                index: 0,
                value
            } if value.is_infinite()
        ));

        let quality_err = ModalSet::new(
            vec![ModalRecord::new(
                10.0,
                1.0,
                vec![1.0; 6],
                None,
                Some(f64::NAN),
            )],
            sample_layout(),
        )
        .unwrap_err();
        assert!(matches!(
            quality_err,
            ModalSetError::InvalidQuality {
                index: 0,
                value
            } if value.is_nan()
        ));
    }

    #[test]
    fn modal_set_sorts_by_frequency() {
        let set = ModalSet::new(sample_modes(), sample_layout()).unwrap();
        let sorted = set.sorted_by_freq().unwrap();
        assert_eq!(sorted.freqs(), vec![110.0, 220.0, 330.0]);
    }

    #[test]
    fn modal_set_filters_and_subsets() {
        let set = ModalSet::new(sample_modes(), sample_layout()).unwrap();
        let filtered = set.filter_freq_min(200.0).unwrap();
        assert_eq!(filtered.freqs(), vec![220.0, 330.0]);
        let subset = set.subset(&[2, 0]).unwrap();
        assert_eq!(subset.freqs(), vec![330.0, 220.0]);
        assert_eq!(subset.shape(1).unwrap(), &[1.0, 0.0, 0.5, 0.1, 0.2, 0.3]);
    }

    #[test]
    fn modal_set_subset_rejects_out_of_bounds_index() {
        let set = ModalSet::new(sample_modes(), sample_layout()).unwrap();
        let err = set.subset(&[0, 3]).unwrap_err();
        assert_eq!(err, ModalSetError::IndexOutOfBounds { index: 3, len: 3 });
    }

    #[test]
    fn modal_set_merge_requires_matching_layout() {
        let lhs = ModalSet::new(sample_modes(), sample_layout()).unwrap();
        let rhs = ModalSet::new(
            vec![ModalRecord::new(
                440.0,
                0.2,
                vec![1.0, 2.0, 3.0],
                None,
                None,
            )],
            ShapeLayout::new(1, 3).unwrap(),
        )
        .unwrap();
        let err = lhs.merge(&rhs).unwrap_err();
        assert_eq!(
            err,
            ModalSetError::LayoutMismatch {
                lhs: sample_layout(),
                rhs: ShapeLayout::new(1, 3).unwrap()
            }
        );
    }

    #[test]
    fn modal_set_merge_keeps_frequency_order() {
        let lhs = ModalSet::new(sample_modes()[..2].to_vec(), sample_layout()).unwrap();
        let rhs = ModalSet::new(vec![sample_modes()[2].clone()], sample_layout()).unwrap();
        let merged = lhs.merge(&rhs).unwrap();
        assert_eq!(merged.freqs(), vec![110.0, 220.0, 330.0]);
        assert_eq!(merged.weights(), vec![1.0, 0.8, 0.4]);
    }

    #[test]
    fn modal_set_normalizes_shapes_without_changing_weight() {
        let set = ModalSet::new(sample_modes(), sample_layout()).unwrap();
        let normalized = set.normalized_shapes_l2().unwrap();

        assert_eq!(normalized.weights(), set.weights());
        for mode in normalized.modes() {
            let norm = mode
                .shape()
                .iter()
                .map(|value| value * value)
                .sum::<f64>()
                .sqrt();
            assert!((norm - 1.0).abs() < 1e-12, "norm={norm}");
        }
    }

    #[test]
    fn modal_set_normalization_rejects_zero_shape() {
        let set = ModalSet::new(
            vec![ModalRecord::new(10.0, 1.0, vec![0.0; 6], None, None)],
            sample_layout(),
        )
        .unwrap();

        assert_eq!(
            set.normalized_shapes_l2().unwrap_err(),
            ModalSetError::InvalidShapeNorm {
                index: 0,
                norm: 0.0,
            }
        );
    }

    #[test]
    fn modal_set_component_extracts_one_component_per_node() {
        let set = ModalSet::new(sample_modes(), sample_layout()).unwrap();
        let component = set.component(1).unwrap();

        assert_eq!(component.layout(), ShapeLayout::new(1, 3).unwrap());
        assert_eq!(component.shape(0).unwrap(), &[0.0, 0.1, 0.3]);
        assert_eq!(component.shape(1).unwrap(), &[0.1, 0.4, 0.5]);
    }

    #[test]
    fn modal_set_components_rebuilds_multi_component_shapes() {
        let set = ModalSet::new(sample_modes(), sample_layout()).unwrap();
        let components = set.components(&[1, 0]).unwrap();

        assert_eq!(components.layout(), sample_layout());
        assert_eq!(
            components.shape(0).unwrap(),
            &[0.0, 1.0, 0.1, 0.5, 0.3, 0.2]
        );
    }

    #[test]
    fn modal_set_component_rejects_out_of_bounds_index() {
        let set = ModalSet::new(sample_modes(), sample_layout()).unwrap();
        assert_eq!(
            set.component(2).unwrap_err(),
            ModalSetError::InvalidComponentIndex {
                index: 2,
                dof_per_node: 2,
            }
        );
    }

    #[test]
    fn modal_set_component_handles_scalar_layout() {
        let layout = ShapeLayout::new(1, 3).unwrap();
        let set = ModalSet::new(
            vec![ModalRecord::new(10.0, 1.0, vec![1.0, 2.0, 3.0], None, None)],
            layout,
        )
        .unwrap();

        let component = set.component(0).unwrap();
        assert_eq!(component.layout(), layout);
        assert_eq!(component.shape(0).unwrap(), &[1.0, 2.0, 3.0]);
    }

    #[test]
    fn modal_set_with_gammas_wraps_core_modes() {
        let set = ModalSet::new(sample_modes(), sample_layout()).unwrap();
        let damped = set.with_gammas(&[0.1, 0.2, 0.3]).unwrap();

        assert_eq!(damped.layout(), sample_layout());
        assert_eq!(damped.len(), 3);
        assert_eq!(damped.modes()[0].base().freq(), 220.0);
        assert_eq!(damped.modes()[2].gamma(), 0.3);
    }

    #[test]
    fn modal_set_with_gammas_rejects_length_mismatch_and_invalid_values() {
        let set = ModalSet::new(sample_modes(), sample_layout()).unwrap();
        assert_eq!(
            set.with_gammas(&[0.1, 0.2]).unwrap_err(),
            ModalSetError::GammaLengthMismatch {
                expected: 3,
                actual: 2,
            }
        );

        assert!(matches!(
            set.with_gammas(&[0.1, f64::NAN, 0.3]).unwrap_err(),
            ModalSetError::InvalidGamma {
                index: 1,
                value
            } if value.is_nan()
        ));
    }

    #[test]
    fn extract_modes_detects_a_single_tone() {
        let dt = 1.0 / 8_000.0;
        let freq = 440.0;
        let samples = 4096;
        let readout: Vec<f64> = (0..samples)
            .map(|n| (2.0 * std::f64::consts::PI * freq * n as f64 * dt).sin())
            .collect();
        let modes = extract_modes(&readout, dt, 4, -40.0);
        assert!(!modes.is_empty());
        assert!((modes[0].freq - freq).abs() < 2.0, "freq={}", modes[0].freq);
    }

    #[test]
    fn extract_modes_returns_empty_for_zero_signal() {
        let readout = vec![0.0; 1024];
        let modes = extract_modes(&readout, 1.0 / 44_100.0, 4, -40.0);
        assert!(modes.is_empty());
    }

    #[test]
    fn extract_modes_returns_empty_for_short_signal_or_zero_mode_limit() {
        let short = vec![0.0, 1.0, 0.0];
        assert!(extract_modes(&short, 0.01, 4, -40.0).is_empty());

        let dt = 1.0 / 8_000.0;
        let freq = 220.0;
        let readout: Vec<f64> = (0..1024)
            .map(|n| (2.0 * std::f64::consts::PI * freq * n as f64 * dt).sin())
            .collect();
        assert!(extract_modes(&readout, dt, 0, -40.0).is_empty());
    }

    #[test]
    fn extract_spatial_modes_returns_one_shape_per_frequency() {
        let nx = 4;
        let ny = 3;
        let snapshots = [
            vec![0.0, 1.0, 0.5, 0.0, 0.2, 0.4, 0.0, 0.1, 0.3, 0.0, 0.2, 0.1],
            vec![0.2, 0.8, 0.7, 0.1, 0.3, 0.3, 0.1, 0.2, 0.2, 0.0, 0.1, 0.2],
            vec![0.4, 0.5, 0.8, 0.2, 0.4, 0.1, 0.2, 0.3, 0.1, 0.1, 0.0, 0.3],
        ];
        let refs: Vec<&[f64]> = snapshots.iter().map(Vec::as_slice).collect();
        let times = [0.0, 0.01, 0.02];
        let modes = extract_spatial_modes(&refs, &times, &[120.0, 240.0], nx, ny);
        assert_eq!(modes.len(), 2);
        assert!(modes.iter().all(|mode| mode.shape.len() == nx * ny));
    }

    #[test]
    fn extract_spatial_modes_returns_empty_for_too_few_snapshots_or_no_frequencies() {
        let single = [vec![1.0, 0.0, 0.0, 1.0]];
        let single_refs: Vec<&[f64]> = single.iter().map(Vec::as_slice).collect();
        assert!(extract_spatial_modes(&single_refs, &[0.0], &[100.0], 2, 2).is_empty());

        let snapshots = [vec![1.0, 0.0, 0.0, 1.0], vec![0.5, 0.0, 0.0, 0.5]];
        let refs: Vec<&[f64]> = snapshots.iter().map(Vec::as_slice).collect();
        assert!(extract_spatial_modes(&refs, &[0.0, 0.1], &[], 2, 2).is_empty());
    }
}
