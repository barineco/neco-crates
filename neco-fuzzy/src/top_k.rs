use core::cmp::Ordering;
use std::collections::BinaryHeap;

use super::Match;

pub(crate) fn compare_match(lhs: &Match<'_>, rhs: &Match<'_>) -> Ordering {
    rhs.score
        .cmp(&lhs.score)
        .then_with(|| lhs.candidate.len().cmp(&rhs.candidate.len()))
        .then_with(|| lhs.index.cmp(&rhs.index))
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct HeapEntry<'a> {
    item: Match<'a>,
}

impl PartialOrd for HeapEntry<'_> {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for HeapEntry<'_> {
    fn cmp(&self, other: &Self) -> Ordering {
        compare_match(&self.item, &other.item)
    }
}

pub(crate) fn collect_top_k<'a>(
    iter: impl Iterator<Item = Match<'a>>,
    limit: usize,
    out: &mut Vec<Match<'a>>,
) {
    out.clear();
    if limit == 0 {
        return;
    }

    let mut heap = BinaryHeap::with_capacity(limit);
    for item in iter {
        if heap.len() < limit {
            heap.push(HeapEntry { item });
            continue;
        }

        let should_replace = heap
            .peek()
            .is_some_and(|worst| compare_match(&item, &worst.item) == Ordering::Less);
        if should_replace {
            heap.pop();
            heap.push(HeapEntry { item });
        }
    }

    out.extend(heap.into_iter().map(|entry| entry.item));
    out.sort_by(compare_match);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Score;

    fn entry<'a>(
        candidate: &'a str,
        value: i64,
        start: usize,
        end: usize,
        index: usize,
    ) -> Match<'a> {
        Match {
            candidate,
            score: Score {
                value,
                energy: -(value as f32),
                confidence: 0.5,
                start,
                end,
                matched: 1,
            },
            index,
        }
    }

    #[test]
    fn collect_top_k_empty_iterator_yields_empty_output() {
        let mut out = vec![entry("stale", 1, 0, 1, 0)];
        collect_top_k(core::iter::empty(), 3, &mut out);
        assert!(out.is_empty());
    }

    #[test]
    fn collect_top_k_zero_limit_yields_empty_output() {
        let mut out = vec![entry("stale", 1, 0, 1, 0)];
        collect_top_k([entry("alpha", 10, 0, 1, 0)].into_iter(), 0, &mut out);
        assert!(out.is_empty());
    }

    #[test]
    fn collect_top_k_matches_linear_selection_order() {
        let all = vec![
            entry("alpha", 10, 0, 2, 0),
            entry("beta", 10, 0, 2, 1),
            entry("alphabet", 10, 0, 2, 2),
            entry("gamma", 8, 1, 3, 3),
            entry("delta", 11, 2, 4, 4),
            entry("epsilon", 11, 2, 4, 5),
            entry("zeta", 11, 1, 3, 6),
        ];

        let mut expected = Vec::new();
        for item in all.iter().copied() {
            if expected.len() < 3 {
                expected.push(item);
                continue;
            }

            let mut worst_index = 0usize;
            for index in 1..expected.len() {
                if compare_match(&expected[index], &expected[worst_index]) == Ordering::Greater {
                    worst_index = index;
                }
            }
            if compare_match(&item, &expected[worst_index]) == Ordering::Less {
                expected[worst_index] = item;
            }
        }
        expected.sort_by(compare_match);

        let mut out = Vec::new();
        collect_top_k(all.into_iter(), 3, &mut out);

        assert_eq!(out, expected);
    }
}
