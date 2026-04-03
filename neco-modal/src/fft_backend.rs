use neco_stft::{with_planner, Complex};

pub(super) struct Spectrum {
    bins: Vec<Complex<f64>>,
    power: Vec<f64>,
}

impl Spectrum {
    pub(super) fn power(&self) -> &[f64] {
        &self.power
    }

    pub(super) fn phase_at(&self, bin: usize) -> f64 {
        self.bins[bin].arg()
    }
}

pub(super) fn hann_window_spectrum(readout: &[f64]) -> Spectrum {
    let n = readout.len();
    let nfft = n.next_power_of_two();
    let mut windowed = vec![0.0; nfft];
    for (i, &sample) in readout.iter().enumerate().take(n) {
        let hann = 0.5 * (1.0 - (2.0 * std::f64::consts::PI * i as f64 / (n - 1) as f64).cos());
        windowed[i] = sample * hann;
    }
    let n_pos = nfft / 2;
    with_planner(|planner| {
        let fft = planner.plan_fft_forward(nfft);
        let mut spectrum = fft.make_output_vec();
        fft.process(&mut windowed, &mut spectrum).unwrap();

        let bins = spectrum[..n_pos].to_vec();
        let power = bins.iter().map(|c| c.norm_sqr()).collect();
        Spectrum { bins, power }
    })
}
