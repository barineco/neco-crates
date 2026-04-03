use std::time::Instant;

use neco_secp::{mine_pow_best, mine_vanity_npub_candidates};

fn main() {
    let vanity_attempts = 20_000u64;
    let pow_attempts = 20_000u64;

    let vanity_start = Instant::now();
    let vanity_candidates =
        mine_vanity_npub_candidates("q", vanity_attempts, 5).expect("vanity candidates");
    let vanity_elapsed = vanity_start.elapsed();

    let pow_start = Instant::now();
    let (pow_bundle, pow_difficulty) = mine_pow_best(1, pow_attempts).expect("pow result");
    let pow_elapsed = pow_start.elapsed();

    println!(
        "neco-secp vanity bench: attempts={}, candidates={}, best_match={}, elapsed={:.3?}",
        vanity_attempts,
        vanity_candidates.len(),
        vanity_candidates
            .first()
            .map(|candidate| candidate.matched_len())
            .unwrap_or(0),
        vanity_elapsed
    );
    println!(
        "neco-secp pow bench: attempts={}, difficulty={}, pubkey={}, elapsed={:.3?}",
        pow_attempts,
        pow_difficulty,
        pow_bundle.xonly_public_key().to_hex(),
        pow_elapsed
    );
}
