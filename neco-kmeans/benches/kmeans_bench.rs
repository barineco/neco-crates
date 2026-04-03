use criterion::{criterion_group, criterion_main, Criterion};
use neco_kmeans::kmeans;

fn deterministic_data(n: usize, d: usize) -> Vec<f64> {
    let total = n * d;
    let mut data = Vec::with_capacity(total);
    for idx in 0..total {
        let hash = (idx as u64).wrapping_mul(2654435761);
        data.push(hash as f64 / u64::MAX as f64);
    }
    data
}

fn bench_kmeans_d3_n50k_k10(c: &mut Criterion) {
    let data = deterministic_data(50_000, 3);
    c.bench_function("kmeans_d3_n50k_k10", |b| {
        b.iter(|| kmeans(&data, 3, 10, 20).expect("benchmark inputs must be valid"));
    });
}

fn bench_kmeans_d128_n5k_k20(c: &mut Criterion) {
    let data = deterministic_data(5_000, 128);
    c.bench_function("kmeans_d128_n5k_k20", |b| {
        b.iter(|| kmeans(&data, 128, 20, 20).expect("benchmark inputs must be valid"));
    });
}

criterion_group!(benches, bench_kmeans_d3_n50k_k10, bench_kmeans_d128_n5k_k20);
criterion_main!(benches);
