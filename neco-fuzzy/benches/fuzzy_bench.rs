use criterion::{criterion_group, criterion_main, Criterion};
use neco_fuzzy::{
    top_k, top_k_prepared, top_k_prepared_with_corpus, top_k_with_config, CorpusStats, Match,
    PreparedCandidate, PreparedQuery, ScoreConfig, Scratch,
};

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
    let config = ScoreConfig {
        w_idf: 1.5,
        ..ScoreConfig::default()
    };
    let stats = CorpusStats::from_candidates(&candidate_refs);
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

    c.bench_function("top_k/with_config", |b| {
        let mut out: Vec<Match<'_>> = Vec::new();
        b.iter(|| {
            top_k_with_config("scb", &candidate_refs, 20, &config, &mut out);
        });
    });

    c.bench_function("top_k/with_corpus", |b| {
        let mut out: Vec<Match<'_>> = Vec::new();
        b.iter(|| {
            top_k_prepared_with_corpus(
                &query,
                &prepared,
                20,
                &config,
                &mut out,
                &mut scratch,
                &stats,
            );
        });
    });
}

fn bench_score_paths(c: &mut Criterion) {
    let query = PreparedQuery::new("scb");
    let candidate = PreparedCandidate::new("src/lib/StatusCommandBar.ts");
    let unicode_query = PreparedQuery::new("とうき");
    let unicode_candidate = PreparedCandidate::new("src/東京/統計.ts");
    let mut scratch = Scratch::default();

    c.bench_function("score/ascii_dp", |b| {
        b.iter(|| {
            neco_fuzzy::score_prepared(&query, &candidate, &mut scratch);
        });
    });

    c.bench_function("score/unicode_dp", |b| {
        b.iter(|| {
            neco_fuzzy::score_prepared(&unicode_query, &unicode_candidate, &mut scratch);
        });
    });
}

criterion_group!(benches, bench_top_k, bench_score_paths);
criterion_main!(benches);
