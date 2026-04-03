# neco-stft

[日本語](README-ja.md)

Backend-agnostic real FFT facade, window functions, and STFT / ISTFT utilities, for audio and signal-processing pipelines.

This crate keeps FFT backend details behind a small public contract so signal-processing code can move between implementations without changing user-facing APIs. Complex spectrum values are exposed through `neco-complex::Complex` rather than a backend crate type.

## API

| Item | Description |
|------|-------------|
| `FftError` | Buffer-size mismatch error for FFT operations |
| `RealToComplex<T>` | Forward real FFT contract |
| `ComplexToReal<T>` | Inverse real FFT contract |
| `FftPlanner<T>` | Planner trait for cached forward / inverse transforms |
| `RustFftPlannerF32`, `RustFftPlannerF64` | current default planner implementations: crate-local radix-2 for power-of-two lengths, crate-local general-length FFT otherwise |
| `DspFloat` | Numeric trait for `f32` / `f64` DSP code with thread-local planners |
| `hann(n)` | Hann window |
| `kaiser_bessel_derived(n, alpha)` | KBD window |
| `StftProcessor` | WOLA-normalized STFT / ISTFT processor |
| `SpectrumFrame<T>` | Positive-frequency complex spectrum frame |

## Preconditions

- The public FFT facade covers real-to-complex and complex-to-real transforms only.
- Inverse transforms are unnormalized; callers divide by `N` when needed.
- `StftProcessor` performs weighted overlap-add normalization internally for fixed hop sizes.
- No public API exposes backend-specific concrete transform types.
- The planner traits are the stable contract. `RustFftPlannerF32` and `RustFftPlannerF64` are the current default implementations, not the long-term boundary.
- Power-of-two lengths use a crate-local radix-2 backend. Non power-of-two lengths use a crate-local general-length backend built on the same public facade.
- The public spectrum boundary uses `neco-complex::Complex`, while backend-specific complex buffers stay behind the facade.

## License

MIT
