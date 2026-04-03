use criterion::{criterion_group, criterion_main, Criterion};
use neco_nurbs::{NurbsCurve2D, NurbsSurface3D};

fn bench_curve_samples(c: &mut Criterion) {
    let curve = NurbsCurve2D::new_rational(
        3,
        vec![
            [0.0, 0.0],
            [1.0, 2.0],
            [3.0, 3.0],
            [5.0, 1.0],
            [6.0, 0.0],
            [7.0, -1.0],
            [9.0, 1.0],
        ],
        vec![1.0, 0.8, 1.2, 0.9, 1.1, 0.95, 1.0],
        vec![0.0, 0.0, 0.0, 0.0, 0.25, 0.5, 0.75, 1.0, 1.0, 1.0, 1.0],
    );

    c.bench_function("nurbs_curve2d_sample_1000", |b| {
        b.iter(|| {
            let _ = curve.sample(1_000);
        });
    });
}

fn make_surface() -> NurbsSurface3D {
    NurbsSurface3D {
        degree_u: 3,
        degree_v: 3,
        control_points: vec![
            vec![
                [0.0, 0.0, 0.0],
                [1.0, 0.1, 0.2],
                [2.0, 0.2, 0.3],
                [3.0, 0.0, 0.1],
            ],
            vec![
                [0.0, 1.0, 0.4],
                [1.0, 1.2, 0.8],
                [2.0, 1.1, 0.7],
                [3.0, 1.0, 0.3],
            ],
            vec![
                [0.0, 2.0, 0.3],
                [1.0, 2.1, 1.0],
                [2.0, 2.0, 0.9],
                [3.0, 2.0, 0.4],
            ],
            vec![
                [0.0, 3.0, 0.0],
                [1.0, 3.2, 0.5],
                [2.0, 3.1, 0.4],
                [3.0, 3.0, 0.0],
            ],
        ],
        weights: vec![
            vec![1.0, 0.9, 1.1, 1.0],
            vec![0.95, 1.0, 1.05, 0.95],
            vec![1.0, 1.1, 0.9, 1.0],
            vec![1.0, 0.95, 1.0, 1.0],
        ],
        knots_u: vec![0.0, 0.0, 0.0, 0.0, 1.0, 1.0, 1.0, 1.0],
        knots_v: vec![0.0, 0.0, 0.0, 0.0, 1.0, 1.0, 1.0, 1.0],
    }
}

fn bench_surface_grid(c: &mut Criterion) {
    let surface = make_surface();
    c.bench_function("nurbs_surface3d_grid_100x100", |b| {
        b.iter(|| {
            for i in 0..100 {
                let u = i as f64 / 99.0;
                for j in 0..100 {
                    let v = j as f64 / 99.0;
                    let _ = surface.evaluate(u, v);
                }
            }
        });
    });
}

criterion_group!(benches, bench_curve_samples, bench_surface_grid);
criterion_main!(benches);
