# neco-modal

[日本語](README-ja.md)

Modal extraction, modal-set storage, and linear modal response utilities for time-domain vibration and resonance signals.

## Modal extraction, storage, and response

`extract_modes` picks dominant peaks from one uniformly sampled readout and estimates frequency, damping, amplitude, phase, and quality for each mode.

`ModalSet` stores validated modal shapes in a small crate-local model, and downstream code keeps solver-specific metadata in its own layer.

`DampedModalSet` extends the core modal model with externally supplied damping coefficients. Linear response helpers and `OscillatorBank` operate on this damped model instead of carrying solver-specific metadata directly.

## Usage

```rust
use neco_modal::{extract_modes, ModalRecord, ModalSet, ShapeLayout};

let dt = 1.0 / 8_000.0;
let samples = 4096;
let readout: Vec<f64> = (0..samples)
    .map(|n| (2.0 * std::f64::consts::PI * 440.0 * n as f64 * dt).sin())
    .collect();

let extracted = extract_modes(&readout, dt, 4, -40.0);
let layout = ShapeLayout::new(1, 8)?;
let modal_set = ModalSet::new(
    vec![ModalRecord::new(
        extracted[0].freq,
        extracted[0].amplitude,
        vec![1.0; 8],
        Some(extracted[0].damping_rate),
        Some(extracted[0].quality),
    )],
    layout,
)?;

assert_eq!(modal_set.len(), 1);
# Ok::<(), neco_modal::ModalSetError>(())
```

## API

| Item | Description |
|------|-------------|
| `extract_modes(readout, dt, n_modes_max, threshold_db)` | Detect dominant temporal modes from one readout signal using a Hann window, FFT spectrum, and peak picking |
| `extract_spatial_modes(snapshots, snapshot_times, mode_freqs, nx, ny)` | Reconstruct spatial mode magnitudes for the requested frequencies from snapshot series |
| `ModeInfo` | Per-mode extraction result with frequency, damping estimate, amplitude, phase, and quality |
| `SpatialMode` | Reconstructed spatial magnitude and estimated circumferential order |
| `ShapeLayout::new(dof_per_node, n_nodes)` | Validate the flattened shape layout metadata |
| `ModalRecord::new(freq, weight, shape, observed_damping, quality)` | Build one modal record |
| `ModalSet::new(modes, layout)` | Validate and store multiple modal records with a shared shape layout |
| `ModalSet::filter_freq_min(f_min)` | Keep only modes whose frequency is at least `f_min` |
| `ModalSet::subset(indices)` | Reorder or select a subset of modes by index |
| `ModalSet::sorted_by_freq()` | Return a frequency-sorted copy |
| `ModalSet::merge(other)` | Merge two modal sets with matching layouts and keep frequency order |
| `ModalSet::normalized_shapes_l2()` | Return a copy whose shapes are L2-normalized to 1 while keeping `weight` unchanged |
| `ModalSet::component(index)` | Extract one component per node from flattened vector shapes |
| `ModalSet::components(indices)` | Rebuild a modal set from the selected per-node components |
| `ModalSet::with_gammas(gammas)` | Attach external damping coefficients as a separate extension model |
| `DampedModalRecord` / `DampedModalSet` | Extension-layer wrappers that keep `gamma` outside the core modal model |
| `SourceExcitation::new(node, delay, gain, phase)` | Validate one excitation for multi-source modal response or driven oscillator updates |
| `compute_source_amps(modes, source_node, receiver_node, component)` | Build one per-mode source-receiver projection from modal shapes |
| `compute_source_amps_multi(modes, excitations, component)` | Build raw per-mode source projections for multiple excitation nodes |
| `generate_ir(modes, source_amps, duration, sample_rate)` | Generate a linear damped modal impulse response from precomputed source amplitudes |
| `generate_ir_multi(modes, excitations, receiver_node, duration, sample_rate, component)` | Generate a multi-source damped modal impulse response directly from excitation descriptors |
| `OscillatorBank::new(modes, source_amps, sample_rate, component)` | Build a linear modal oscillator bank from damped modes and one source projection |
| `OscillatorBank::new_multi(modes, excitations, sample_rate, component)` | Build a linear modal oscillator bank from multiple excitations |
| `OscillatorBank::set_receiver(receiver_amps)` | Configure receiver-side modal reconstruction weights |
| `OscillatorBank::drive(input, output)` / `drive_multi(inputs, excitations, output)` | Inject continuous input signals while advancing the oscillator bank |
| `OscillatorBank::process(output)` | Advance the oscillator bank without new input and write audio samples |
| `OscillatorBank::field_weights()` / `compute_field()` | Read modal weights or reconstruct the current spatial field |

### Preconditions

- `extract_modes` assumes a uniformly sampled forward time series with `dt > 0`.
- Very short signals (`len < 4`) and near-zero signals return an empty result.
- Peak detection is threshold-based and relative to the strongest spectral peak.
- Damping is estimated from spectral width, so it should be treated as an observed approximation rather than a calibrated physics model.
- `extract_spatial_modes` reconstructs shape magnitudes from the supplied snapshots and currently returns flattened `Vec<f64>` data.
- `ModalSet` validates shape length against `ShapeLayout` and rejects invalid frequencies, weights, and non-finite optional quantities.
- `ModalSet::normalized_shapes_l2()` normalizes shape values only. `weight` is left unchanged, and zero or non-finite L2 norms are rejected.
- `ModalSet::component()` and `ModalSet::components()` use `ShapeLayout` to interpret the flattened shape stride. Out-of-bounds component indices are rejected, and scalar layouts work with `component(0)`.
- The FFT implementation is hidden behind a private facade built on `neco-stft`; the public API exposes only local types, and the private spectrum helper stores `neco-complex::Complex`.
- `DampedModalSet` is the extension boundary for externally applied damping coefficients. The core `ModalRecord` keeps only core modal fields; `gamma` and consumer-specific metadata such as solver labels live on extension records.
- Linear response and oscillator helpers require a `DampedModalSet`; the core `ModalSet` alone is intentionally not enough because `gamma` remains an external extension.
- `SourceExcitation` validates only local numeric constraints. Node bounds are checked against the modal layout when response or oscillator helpers read shapes.
- `generate_ir_multi()` and `OscillatorBank::new_multi()` are the linear multi-source entry points; nonlinear table builders and nonlinear oscillator wrappers are provided by separate crates.

## License

MIT
