use std::time::Instant;

use neco_cdt::Cdt;

fn lcg_next(state: &mut u64) -> f64 {
    *state = state.wrapping_mul(6364136223846793005).wrapping_add(1);
    let bits = (*state >> 11) as f64;
    bits / ((1u64 << 53) as f64)
}

fn main() {
    let n_points = 10_000usize;
    let mut state = 0x5eed_cafe_d00d_beefu64;
    let mut points = Vec::with_capacity(n_points);
    for _ in 0..n_points {
        points.push([lcg_next(&mut state), lcg_next(&mut state)]);
    }

    let start = Instant::now();
    let mut cdt = Cdt::new((0.0, 0.0, 1.0, 1.0));
    for point in &points {
        cdt.insert(point[0], point[1]);
    }
    let elapsed = start.elapsed();

    println!(
        "neco-cdt benchmark: {} points, {} triangles, elapsed={:.3?}",
        n_points,
        cdt.triangles().len(),
        elapsed
    );
}
