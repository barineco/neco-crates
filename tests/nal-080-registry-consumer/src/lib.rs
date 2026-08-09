#[cfg(test)]
mod tests {
    use neco_eigensolve::EigensolveConfig;
    use neco_linear_types::Shape;
    use neco_minphase::compute_min_phase_spectrum;
    use neco_modal::extract_modes;
    use neco_sparse::CooMatrix;
    use neco_spectral::spectral_cluster;
    use neco_stft::{with_planner, Complex, FftPlanner};

    #[test]
    fn registry_packages_execute_public_apis() -> Result<(), Box<dyn std::error::Error>> {
        let shape = Shape::new(4, 4);
        let mut adjacency = CooMatrix::new(shape);
        for (left, right, weight) in [(0, 1, 4.0), (2, 3, 4.0), (1, 2, 0.01)] {
            adjacency.push(shape.row_index(left)?, shape.column_index(right)?, weight)?;
            adjacency.push(shape.row_index(right)?, shape.column_index(left)?, weight)?;
        }
        let adjacency = adjacency.to_csr()?;
        let config = EigensolveConfig::new(2, 1.0e-9, 1.0e-9, 64)?;
        let result = spectral_cluster(&adjacency, 2, config, 32)?;
        assert_eq!(result.assignments().len(), 4);

        let fft_output = with_planner(|planner: &mut dyn FftPlanner<f64>| {
            let fft = planner.plan_fft_forward(4);
            let mut fft_input = fft.make_input_vec();
            fft_input[0] = 1.0;
            let mut fft_output = fft.make_output_vec();
            fft.process(&mut fft_input, &mut fft_output)
                .map_err(|error| std::io::Error::other(error.to_string()))?;
            Ok::<_, std::io::Error>(fft_output)
        })?;
        assert_eq!(fft_output.len(), 3);

        let spectrum = compute_min_phase_spectrum(&[1.0_f64, 1.0, 1.0], 4)?;
        assert_eq!(spectrum.len(), 3);
        let scalar = Complex::new(3.0_f64, 4.0);
        assert_eq!(scalar.norm_squared(), 25.0);

        let readout: Vec<f64> = (0..128)
            .map(|sample| (2.0 * std::f64::consts::PI * sample as f64 / 16.0).sin())
            .collect();
        assert!(!extract_modes(&readout, 1.0 / 8_000.0, 4, -40.0).is_empty());
        Ok(())
    }
}
