use criterion::{criterion_group, criterion_main, Criterion};
use neco_fuzzy::{top_k, top_k_prepared, Match, PreparedCandidate, PreparedQuery, Scratch};

fn bench_top_k(c: &mut Criterion) {
    let candidates: Vec<String> = (0..2000)
        .map(|index| format!("src/lib/module_{index}/StatusCommandBar.ts"))
        .collect();
    let candidate_refs: Vec<&str> = candidates.iter().map(String::as_str).collect();
    let prepared: Vec<PreparedCandidate<'_>> = candidate_refs
        .iter()
        .copied()
        .map(PreparedCandidate::new)
        .collect();
    let query = PreparedQuery::new("scb");
    let mut scratch = Scratch::default();

    c.bench_function("top_k/stable", |b| {
        let mut out: Vec<Match<'_>> = Vec::new();
        b.iter(|| {
            top_k("scb", &candidate_refs, 20, &mut out);
        });
    });

    c.bench_function("top_k/prepared", |b| {
        let mut out: Vec<Match<'_>> = Vec::new();
        b.iter(|| {
            top_k_prepared(&query, &prepared, 20, &mut out, &mut scratch);
        });
    });
}

criterion_group!(benches, bench_top_k);
criterion_main!(benches);
