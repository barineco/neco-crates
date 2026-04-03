# neco-minphase

[日本語](README-ja.md)

Minimum-phase spectrum / impulse kernels and overlap-add convolution, for audio DSP such as FIR equalizer construction and impulse-response convolution.

This crate keeps the pure cepstral and convolution pieces separate from application-specific EQ assembly so minimum-phase processing can be reused without bringing in analyzer or preset logic. Complex spectra are exposed through the shared `neco-complex::Complex` foundation.

## API

| Item | Description |
|------|-------------|
| `compute_min_phase_spectrum(gain_curve, fft_size)` | Build a minimum-phase complex spectrum from a magnitude curve |
| `compute_min_phase_ir(gain_curve, fft_size)` | Build a minimum-phase impulse response |
| `convolve_ola(input, ir)` | Convolve with FFT overlap-add and truncate to input length |
| `compute_blend_curve(transient_map, lookahead, smooth, threshold)` | Build a transient-aware blend curve in `[0, 1]` |

## Preconditions

- `gain_curve` length must be `fft_size / 2 + 1`.
- `fft_size` may be power-of-two or any other positive length accepted by `neco-stft`'s public FFT facade.
- Minimum-phase kernels preserve magnitude while biasing energy toward the front of the impulse.
- `convolve_ola` returns an output truncated to the input length.
- This crate intentionally excludes application-specific hybrid EQ assembly.

## License

MIT
