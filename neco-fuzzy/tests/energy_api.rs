use neco_fuzzy::{
    match_indices, score, score_with_config, score_with_corpus, top_k_with_config, CorpusStats,
    Match, ScoreConfig,
};

#[test]
fn default_config_matches_legacy_score_api() {
    let config = ScoreConfig::default();
    assert_eq!(
        score("cmd", "command"),
        score_with_config("cmd", "command", &config)
    );
}

#[test]
fn score_exposes_energy_and_confidence() {
    let score = score("cmd", "command").expect("must match");
    assert!(score.energy.is_finite());
    assert!(score.confidence.is_finite());
    assert!(score.value.abs() < i64::MAX);
    assert!(score.confidence > 0.0);
    assert!(score.confidence < 1.0);
}

#[test]
fn corpus_weight_changes_score() {
    let stats = CorpusStats::from_candidates(&["foo_bar", "foobar", "quxbuzz"]);
    let config = ScoreConfig {
        w_idf: 2.0,
        ..ScoreConfig::default()
    };
    let plain = score_with_config("fb", "foo_bar", &config).expect("plain score");
    let weighted =
        score_with_corpus("fb", "foo_bar", &config, &stats).expect("corpus-backed score");

    assert_ne!(plain.value, weighted.value);
    assert_ne!(plain.energy, weighted.energy);
}

#[test]
fn dp_traceback_prefers_boundary_chain() {
    let mut indices = Vec::new();
    assert!(match_indices("scb", "StatusCommandBar", &mut indices));
    assert_eq!(indices, vec![0, 6, 13]);
}

#[test]
fn top_k_with_config_keeps_regression_order() {
    let config = ScoreConfig::default();
    let candidates = ["foo_bar", "foobar"];
    let mut out: Vec<Match<'_>> = Vec::new();
    top_k_with_config("fb", &candidates, 2, &config, &mut out);
    assert_eq!(
        out.into_iter()
            .map(|entry| entry.candidate)
            .collect::<Vec<_>>(),
        vec!["foo_bar", "foobar"]
    );
}
