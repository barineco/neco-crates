use criterion::{black_box, criterion_group, criterion_main, Criterion};

const GAMMA_LUT_SIZE: usize = 4096;

fn build_lut() -> [f32; GAMMA_LUT_SIZE] {
    let mut lut = [0.0; GAMMA_LUT_SIZE];
    for (i, slot) in lut.iter_mut().enumerate() {
        let c = i as f32 / (GAMMA_LUT_SIZE - 1) as f32;
        *slot = if c <= 0.04045 {
            c / 12.92
        } else {
            ((c + 0.055) / 1.055).powf(2.4)
        };
    }
    lut
}

fn gamma_lut_lookup_floor(lut: &[f32; GAMMA_LUT_SIZE], value: f32) -> f32 {
    let idx = (value * (GAMMA_LUT_SIZE - 1) as f32).clamp(0.0, (GAMMA_LUT_SIZE - 1) as f32);
    let lo = idx.floor() as usize;
    let hi = (lo + 1).min(GAMMA_LUT_SIZE - 1);
    let frac = idx - lo as f32;
    lut[lo] * (1.0 - frac) + lut[hi] * frac
}

fn gamma_lut_lookup_cast(lut: &[f32; GAMMA_LUT_SIZE], value: f32) -> f32 {
    let idx = (value * (GAMMA_LUT_SIZE - 1) as f32).clamp(0.0, (GAMMA_LUT_SIZE - 1) as f32);
    let lo = idx as usize;
    let hi = (lo + 1).min(GAMMA_LUT_SIZE - 1);
    let frac = idx - lo as f32;
    lut[lo] * (1.0 - frac) + lut[hi] * frac
}

fn input_values() -> Vec<f32> {
    (0..16_384)
        .map(|i| ((i * 97) % 16_384) as f32 / 16_383.0)
        .collect()
}

fn bench_gamma_lookup(c: &mut Criterion) {
    let lut = build_lut();
    let inputs = input_values();

    c.bench_function("gamma_lut_lookup_floor", |b| {
        b.iter(|| {
            let mut acc = 0.0f32;
            for &value in &inputs {
                acc += gamma_lut_lookup_floor(&lut, value);
            }
            black_box(acc)
        });
    });

    c.bench_function("gamma_lut_lookup_cast", |b| {
        b.iter(|| {
            let mut acc = 0.0f32;
            for &value in &inputs {
                acc += gamma_lut_lookup_cast(&lut, value);
            }
            black_box(acc)
        });
    });
}

criterion_group!(benches, bench_gamma_lookup);
criterion_main!(benches);
