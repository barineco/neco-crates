use neco_complex::Complex;

use crate::dsp_float::DspFloat;

fn bit_reverse(mut value: usize, bits: u32) -> usize {
    let mut reversed = 0usize;
    for _ in 0..bits {
        reversed = (reversed << 1) | (value & 1);
        value >>= 1;
    }
    reversed
}

fn fft_radix2_in_place<T: DspFloat>(buffer: &mut [Complex<T>], inverse: bool) {
    let n = buffer.len();
    assert!(
        n.is_power_of_two(),
        "radix-2 FFT requires power-of-two length"
    );
    if n <= 1 {
        return;
    }

    let bits = n.trailing_zeros();
    for i in 0..n {
        let j = bit_reverse(i, bits);
        if j > i {
            buffer.swap(i, j);
        }
    }

    let two = T::from_f64(2.0);
    let sign = if inverse { T::one() } else { -T::one() };
    let mut len = 2usize;
    while len <= n {
        let half = len / 2;
        let angle = sign * two * T::pi() / T::from_usize(len);
        let w_len = Complex::new(angle.cos(), angle.sin());
        for start in (0..n).step_by(len) {
            let mut w = Complex::new(T::one(), T::zero());
            for offset in 0..half {
                let even = buffer[start + offset];
                let odd = buffer[start + offset + half] * w;
                buffer[start + offset] = even + odd;
                buffer[start + offset + half] = even - odd;
                w *= w_len;
            }
        }
        len <<= 1;
    }
}

fn chirp<T: DspFloat>(index: usize, len: usize, inverse: bool) -> Complex<T> {
    let sign = if inverse { T::one() } else { -T::one() };
    let angle = sign * T::pi() * T::from_usize(index * index) / T::from_usize(len);
    Complex::new(angle.cos(), angle.sin())
}

fn fft_bluestein_forward<T: DspFloat>(buffer: &mut [Complex<T>]) {
    let n = buffer.len();
    if n <= 1 {
        return;
    }

    let conv_len = (2 * n - 1).next_power_of_two();
    let scale = T::one() / T::from_usize(conv_len);
    let chirp_table: Vec<Complex<T>> = (0..n).map(|i| chirp(i, n, false)).collect();

    let mut lhs = vec![Complex::new(T::zero(), T::zero()); conv_len];
    let mut rhs = vec![Complex::new(T::zero(), T::zero()); conv_len];
    for i in 0..n {
        lhs[i] = buffer[i] * chirp_table[i];
        let chirp_conj = chirp_table[i].conj();
        rhs[i] = chirp_conj;
        if i != 0 {
            rhs[conv_len - i] = chirp_conj;
        }
    }

    fft_radix2_in_place(&mut lhs, false);
    fft_radix2_in_place(&mut rhs, false);
    for (lhs_value, rhs_value) in lhs.iter_mut().zip(rhs.iter()) {
        *lhs_value *= *rhs_value;
    }
    fft_radix2_in_place(&mut lhs, true);

    for i in 0..n {
        buffer[i] = lhs[i] * chirp_table[i] * scale;
    }
}

pub fn fft_in_place<T: DspFloat>(buffer: &mut [Complex<T>], inverse: bool) {
    let n = buffer.len();
    if n <= 1 {
        return;
    }
    if n.is_power_of_two() {
        fft_radix2_in_place(buffer, inverse);
        return;
    }
    if inverse {
        for value in buffer.iter_mut() {
            *value = value.conj();
        }
        fft_bluestein_forward(buffer);
        for value in buffer.iter_mut() {
            *value = value.conj();
        }
    } else {
        fft_bluestein_forward(buffer);
    }
}

pub fn real_fft_forward<T: DspFloat>(input: &[T]) -> Vec<Complex<T>> {
    let n = input.len();
    let mut buffer: Vec<Complex<T>> = input
        .iter()
        .copied()
        .map(|value| Complex::new(value, T::zero()))
        .collect();
    fft_in_place(&mut buffer, false);
    buffer[..(n / 2 + 1)].to_vec()
}

pub fn real_fft_inverse<T: DspFloat>(spectrum: &[Complex<T>], output: &mut [T]) {
    let n = output.len();
    assert_eq!(spectrum.len(), n / 2 + 1);
    if n == 0 {
        return;
    }
    if n == 1 {
        output[0] = spectrum[0].re;
        return;
    }

    let mut buffer = vec![Complex::new(T::zero(), T::zero()); n];
    buffer[0] = spectrum[0];
    if spectrum.len() > 1 {
        buffer[1..spectrum.len()].copy_from_slice(&spectrum[1..]);
    }
    let mirror_limit = if n % 2 == 0 {
        spectrum.len() - 1
    } else {
        spectrum.len()
    };
    for k in 1..mirror_limit {
        buffer[n - k] = spectrum[k].conj();
    }

    fft_in_place(&mut buffer, true);
    for (dst, src) in output.iter_mut().zip(buffer.iter()) {
        *dst = src.re;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn complex_fft_roundtrip_power_of_two_f64() {
        let mut buffer: Vec<Complex<f64>> = (0..16)
            .map(|i| Complex::new(i as f64 * 0.25, -(i as f64) * 0.125))
            .collect();
        let input = buffer.clone();
        fft_in_place(&mut buffer, false);
        fft_in_place(&mut buffer, true);
        let scale = 1.0 / input.len() as f64;
        for (actual, expected) in buffer.iter().zip(input.iter()) {
            assert!((actual.re * scale - expected.re).abs() < 1e-10);
            assert!((actual.im * scale - expected.im).abs() < 1e-10);
        }
    }

    #[test]
    fn complex_fft_roundtrip_non_power_of_two_f64() {
        let mut buffer: Vec<Complex<f64>> = (0..15)
            .map(|i| Complex::new(i as f64 * 0.2 - 1.0, (i as f64 * 0.15).cos()))
            .collect();
        let input = buffer.clone();
        fft_in_place(&mut buffer, false);
        fft_in_place(&mut buffer, true);
        let scale = 1.0 / input.len() as f64;
        for (actual, expected) in buffer.iter().zip(input.iter()) {
            assert!((actual.re * scale - expected.re).abs() < 1e-10);
            assert!((actual.im * scale - expected.im).abs() < 1e-10);
        }
    }

    #[test]
    fn real_fft_roundtrip_power_of_two_f32() {
        let input: Vec<f32> = (0..32)
            .map(|i| (2.0f32 * std::f32::consts::PI * i as f32 / 32.0).sin())
            .collect();
        let spectrum = real_fft_forward(&input);
        let mut output = vec![0.0f32; input.len()];
        real_fft_inverse(&spectrum, &mut output);
        let scale = 1.0f32 / input.len() as f32;
        let max_err = output
            .iter()
            .zip(input.iter())
            .map(|(&actual, &expected)| (actual * scale - expected).abs())
            .fold(0.0, f32::max);
        assert!(max_err < 1e-5, "roundtrip error: {max_err:.2e}");
    }

    #[test]
    fn real_fft_roundtrip_non_power_of_two_f32() {
        let input: Vec<f32> = (0..30)
            .map(|i| {
                let t = i as f32 / 30.0;
                (2.0f32 * std::f32::consts::PI * 3.0 * t).sin()
                    + 0.25 * (2.0f32 * std::f32::consts::PI * 5.0 * t).cos()
            })
            .collect();
        let spectrum = real_fft_forward(&input);
        let mut output = vec![0.0f32; input.len()];
        real_fft_inverse(&spectrum, &mut output);
        let scale = 1.0f32 / input.len() as f32;
        let max_err = output
            .iter()
            .zip(input.iter())
            .map(|(&actual, &expected)| (actual * scale - expected).abs())
            .fold(0.0, f32::max);
        assert!(max_err < 1e-4, "roundtrip error: {max_err:.2e}");
    }
}
