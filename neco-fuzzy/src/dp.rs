use core::cmp::Ordering;

use crate::boundary::CandidateChar;
use crate::config::ScoreConfig;

#[allow(clippy::too_many_arguments)]
pub(crate) fn dp_solve(
    query_chars: &[char],
    candidate_chars: &[CandidateChar],
    basename_start_char: usize,
    config: &ScoreConfig,
    matched_out: &mut Vec<usize>,
    dp_cost: &mut Vec<f32>,
    dp_prev: &mut Vec<usize>,
    dp_cost_swap: &mut Vec<f32>,
    dp_prev_swap: &mut Vec<usize>,
    corpus_idf: Option<&dyn Fn(char) -> f32>,
) -> Option<f32> {
    solve_impl(
        query_chars.len(),
        candidate_chars,
        basename_start_char,
        config,
        matched_out,
        dp_cost,
        dp_prev,
        dp_cost_swap,
        dp_prev_swap,
        corpus_idf,
        |query_index, candidate_index| {
            chars_equal_caseless(
                query_chars[query_index],
                candidate_chars[candidate_index].ch,
            )
        },
    )
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn dp_solve_case_sensitive(
    query_chars: &[char],
    candidate_chars: &[CandidateChar],
    basename_start_char: usize,
    config: &ScoreConfig,
    matched_out: &mut Vec<usize>,
    dp_cost: &mut Vec<f32>,
    dp_prev: &mut Vec<usize>,
    dp_cost_swap: &mut Vec<f32>,
    dp_prev_swap: &mut Vec<usize>,
    corpus_idf: Option<&dyn Fn(char) -> f32>,
) -> Option<f32> {
    solve_impl(
        query_chars.len(),
        candidate_chars,
        basename_start_char,
        config,
        matched_out,
        dp_cost,
        dp_prev,
        dp_cost_swap,
        dp_prev_swap,
        corpus_idf,
        |query_index, candidate_index| {
            query_chars[query_index] == candidate_chars[candidate_index].ch
        },
    )
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn dp_solve_ascii(
    query_bytes: &[u8],
    candidate_bytes: &[u8],
    candidate_chars: &[CandidateChar],
    basename_start_char: usize,
    config: &ScoreConfig,
    matched_out: &mut Vec<usize>,
    dp_cost: &mut Vec<f32>,
    dp_prev: &mut Vec<usize>,
    dp_cost_swap: &mut Vec<f32>,
    dp_prev_swap: &mut Vec<usize>,
    corpus_idf: Option<&dyn Fn(char) -> f32>,
) -> Option<f32> {
    solve_impl(
        query_bytes.len(),
        candidate_chars,
        basename_start_char,
        config,
        matched_out,
        dp_cost,
        dp_prev,
        dp_cost_swap,
        dp_prev_swap,
        corpus_idf,
        |query_index, candidate_index| query_bytes[query_index] == candidate_bytes[candidate_index],
    )
}

#[allow(clippy::too_many_arguments)]
fn solve_impl<MatchFn>(
    query_len: usize,
    candidate_chars: &[CandidateChar],
    basename_start_char: usize,
    config: &ScoreConfig,
    matched_out: &mut Vec<usize>,
    dp_cost: &mut Vec<f32>,
    dp_prev: &mut Vec<usize>,
    dp_cost_swap: &mut Vec<f32>,
    dp_prev_swap: &mut Vec<usize>,
    corpus_idf: Option<&dyn Fn(char) -> f32>,
    mut matches_at: MatchFn,
) -> Option<f32>
where
    MatchFn: FnMut(usize, usize) -> bool,
{
    matched_out.clear();

    if query_len == 0 {
        return Some(0.0);
    }

    let candidate_len = candidate_chars.len();
    if candidate_len == 0 || query_len > candidate_len {
        return None;
    }

    dp_cost.clear();
    dp_cost.resize(candidate_len, f32::INFINITY);
    dp_cost_swap.clear();
    dp_cost_swap.resize(candidate_len, f32::INFINITY);
    dp_prev.clear();
    dp_prev.resize(query_len * candidate_len, usize::MAX);
    dp_prev_swap.clear();
    dp_prev_swap.resize(candidate_len, usize::MAX);

    let candidate_len_f32 = candidate_len as f32;
    let sigma = config.sigma(candidate_len);
    let sigma_sq = sigma * sigma;
    let window = (3.0 * sigma).ceil() as usize;

    let unary_cost = |candidate_index: usize| {
        let candidate = candidate_chars[candidate_index];
        let position_cost = config.w_pos * (candidate_index as f32 / candidate_len_f32);
        let boundary_cost = config.w_bnd * (1.0 - candidate.boundary);
        let head_cost = if candidate_index == 0 {
            -config.w_head
        } else if basename_start_char > 0 && candidate_index == basename_start_char {
            -(config.w_head * 0.75)
        } else {
            0.0
        };
        let idf_cost = corpus_idf.map_or(0.0, |idf| -config.w_idf * idf(candidate.ch));
        position_cost + boundary_cost + head_cost + idf_cost
    };

    for (candidate_index, slot) in dp_cost.iter_mut().enumerate().take(candidate_len) {
        if matches_at(0, candidate_index) {
            *slot = unary_cost(candidate_index);
        }
    }

    if query_len == 1 {
        let (best_index, best_cost) = find_best(dp_cost)?;
        matched_out.push(best_index);
        return Some(best_cost);
    }

    let mut prev_cost = dp_cost;
    let mut curr_cost = dp_cost_swap;

    for query_index in 1..query_len {
        curr_cost.fill(f32::INFINITY);
        dp_prev_swap.fill(usize::MAX);

        let row_offset = query_index * candidate_len;
        let row_trace = &mut dp_prev[row_offset..row_offset + candidate_len];
        row_trace.fill(usize::MAX);

        let mut far_min = f32::INFINITY;
        let mut far_prev = usize::MAX;

        for candidate_index in 0..candidate_len {
            if matches_at(query_index, candidate_index) {
                let mut best_transition = f32::INFINITY;
                let mut best_prev = usize::MAX;

                let near_start = candidate_index.saturating_sub(window);
                for (previous_index, &prev_value) in prev_cost
                    .iter()
                    .enumerate()
                    .take(candidate_index)
                    .skip(near_start)
                {
                    if !prev_value.is_finite() {
                        continue;
                    }

                    let transition = prev_value
                        + pairwise_cost(
                            previous_index,
                            candidate_index,
                            candidate_len_f32,
                            config,
                            sigma_sq,
                        );
                    if transition.total_cmp(&best_transition) == Ordering::Less {
                        best_transition = transition;
                        best_prev = previous_index;
                    }
                }

                if far_min.is_finite() {
                    let transition = far_min + config.w_gap;
                    if transition.total_cmp(&best_transition) == Ordering::Less {
                        best_transition = transition;
                        best_prev = far_prev;
                    }
                }

                if best_transition.is_finite() {
                    curr_cost[candidate_index] = unary_cost(candidate_index) + best_transition;
                    dp_prev_swap[candidate_index] = best_prev;
                    row_trace[candidate_index] = best_prev;
                }
            }

            if let Some(left_index) = candidate_index.checked_sub(window) {
                let left_value = prev_cost[left_index];
                if left_value.is_finite() && left_value.total_cmp(&far_min) == Ordering::Less {
                    far_min = left_value;
                    far_prev = left_index;
                }
            }
        }

        core::mem::swap(&mut prev_cost, &mut curr_cost);
    }

    let (best_last_index, best_cost) = find_best(prev_cost)?;
    matched_out.resize(query_len, 0);
    matched_out[query_len - 1] = best_last_index;

    let mut current_index = best_last_index;
    for query_index in (1..query_len).rev() {
        let prev_index = dp_prev[query_index * candidate_len + current_index];
        if prev_index == usize::MAX {
            matched_out.clear();
            return None;
        }
        matched_out[query_index - 1] = prev_index;
        current_index = prev_index;
    }

    Some(best_cost)
}

fn find_best(row: &[f32]) -> Option<(usize, f32)> {
    let mut best_index = usize::MAX;
    let mut best_cost = f32::INFINITY;

    for (index, &cost) in row.iter().enumerate() {
        if !cost.is_finite() {
            continue;
        }
        if cost.total_cmp(&best_cost) == Ordering::Less {
            best_index = index;
            best_cost = cost;
        }
    }

    (best_index != usize::MAX).then_some((best_index, best_cost))
}

fn pairwise_cost(
    previous_index: usize,
    candidate_index: usize,
    candidate_len_f32: f32,
    config: &ScoreConfig,
    sigma_sq: f32,
) -> f32 {
    let gap = candidate_index - previous_index - 1;
    let gap_f32 = gap as f32;
    let gap_cost = if gap == 0 {
        0.0
    } else if sigma_sq <= f32::EPSILON {
        config.w_gap
    } else {
        config.w_gap * (1.0 - (-(gap_f32 * gap_f32) / (2.0 * sigma_sq)).exp())
    };
    let span_cost = config.w_span * gap_f32 / candidate_len_f32;
    gap_cost + span_cost
}

pub(crate) fn chars_equal_caseless(a: char, b: char) -> bool {
    if a.is_ascii() && b.is_ascii() {
        a.eq_ignore_ascii_case(&b)
    } else {
        a.to_lowercase().eq(b.to_lowercase())
    }
}

#[cfg(test)]
mod tests {
    use super::{chars_equal_caseless, dp_solve, dp_solve_ascii};
    use crate::boundary::build_prepared_parts;
    use crate::config::ScoreConfig;

    #[test]
    fn 空クエリはゼロエネルギーを返す() {
        let (candidate_chars, basename_start_char, _, _, _) = build_prepared_parts("foo");
        let config = ScoreConfig::default();
        let mut matched = Vec::new();
        let mut dp_cost = Vec::new();
        let mut dp_prev = Vec::new();
        let mut dp_cost_swap = Vec::new();
        let mut dp_prev_swap = Vec::new();

        let energy = dp_solve(
            &[],
            &candidate_chars,
            basename_start_char,
            &config,
            &mut matched,
            &mut dp_cost,
            &mut dp_prev,
            &mut dp_cost_swap,
            &mut dp_prev_swap,
            None,
        );

        assert_eq!(energy, Some(0.0));
        assert!(matched.is_empty());
    }

    #[test]
    fn 境界を優先して最適配置を選ぶ() {
        let (candidate_chars, basename_start_char, _, ascii_folded, _) =
            build_prepared_parts("StatusCommandBar");
        let config = ScoreConfig::default();
        let mut matched = Vec::new();
        let mut dp_cost = Vec::new();
        let mut dp_prev = Vec::new();
        let mut dp_cost_swap = Vec::new();
        let mut dp_prev_swap = Vec::new();

        let energy = dp_solve(
            &['s', 'c', 'b'],
            &candidate_chars,
            basename_start_char,
            &config,
            &mut matched,
            &mut dp_cost,
            &mut dp_prev,
            &mut dp_cost_swap,
            &mut dp_prev_swap,
            None,
        );

        assert!(energy.is_some());
        assert_eq!(matched, vec![0, 6, 13]);

        let mut ascii_matched = Vec::new();
        let ascii_energy = dp_solve_ascii(
            b"scb",
            ascii_folded
                .as_deref()
                .expect("ASCII folded candidate expected"),
            &candidate_chars,
            basename_start_char,
            &config,
            &mut ascii_matched,
            &mut dp_cost,
            &mut dp_prev,
            &mut dp_cost_swap,
            &mut dp_prev_swap,
            None,
        );

        assert_eq!(ascii_energy, energy);
        assert_eq!(ascii_matched, matched);
    }

    #[test]
    fn 区切り直後の文字を優先する() {
        let (candidate_chars, basename_start_char, _, ascii_folded, _) =
            build_prepared_parts("foo_bar");
        let config = ScoreConfig::default();
        let mut matched = Vec::new();
        let mut dp_cost = Vec::new();
        let mut dp_prev = Vec::new();
        let mut dp_cost_swap = Vec::new();
        let mut dp_prev_swap = Vec::new();

        let energy = dp_solve(
            &['f', 'b'],
            &candidate_chars,
            basename_start_char,
            &config,
            &mut matched,
            &mut dp_cost,
            &mut dp_prev,
            &mut dp_cost_swap,
            &mut dp_prev_swap,
            None,
        );

        assert!(energy.is_some());
        assert_eq!(matched, vec![0, 4]);

        let mut ascii_matched = Vec::new();
        let ascii_energy = dp_solve_ascii(
            b"fb",
            ascii_folded
                .as_deref()
                .expect("ASCII folded candidate expected"),
            &candidate_chars,
            basename_start_char,
            &config,
            &mut ascii_matched,
            &mut dp_cost,
            &mut dp_prev,
            &mut dp_cost_swap,
            &mut dp_prev_swap,
            None,
        );

        assert_eq!(ascii_energy, energy);
        assert_eq!(ascii_matched, matched);
    }

    #[test]
    fn マッチ不能ならなしを返す() {
        let (candidate_chars, basename_start_char, _, _, _) = build_prepared_parts("foo");
        let config = ScoreConfig::default();
        let mut matched = Vec::new();
        let mut dp_cost = Vec::new();
        let mut dp_prev = Vec::new();
        let mut dp_cost_swap = Vec::new();
        let mut dp_prev_swap = Vec::new();

        let energy = dp_solve(
            &['x'],
            &candidate_chars,
            basename_start_char,
            &config,
            &mut matched,
            &mut dp_cost,
            &mut dp_prev,
            &mut dp_cost_swap,
            &mut dp_prev_swap,
            None,
        );

        assert_eq!(energy, None);
        assert!(matched.is_empty());
    }

    #[test]
    fn 非asciiも大文字小文字を無視できる() {
        assert!(chars_equal_caseless('Ä', 'ä'));
    }
}
