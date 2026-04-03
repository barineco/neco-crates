//! Recursive graph partitioning via spectral bisection and Kernighan-Lin refinement.

use std::collections::HashSet;

/// Split an unweighted graph into two parts using the Fiedler vector of the graph Laplacian.
pub fn spectral_bisect(graph: &[Vec<usize>]) -> (Vec<usize>, Vec<usize>) {
    let n = graph.len();
    if n <= 1 {
        return ((0..n).collect(), vec![]);
    }

    let components = connected_components(graph);
    if components.len() > 1 {
        return split_components_balanced(&components, n);
    }

    let fiedler = normalized_adjacency_second_vector(graph);
    let mut part_a = Vec::new();
    let mut part_b = Vec::new();
    let threshold = median(&fiedler);
    for (i, &value) in fiedler.iter().enumerate().take(n) {
        if value >= threshold {
            part_a.push(i);
        } else {
            part_b.push(i);
        }
    }

    if part_a.is_empty() || part_b.is_empty() {
        let mid = n / 2;
        return ((0..mid).collect(), (mid..n).collect());
    }

    (part_a, part_b)
}

fn connected_components(graph: &[Vec<usize>]) -> Vec<Vec<usize>> {
    let n = graph.len();
    let mut seen = vec![false; n];
    let mut components = Vec::new();

    for start in 0..n {
        if seen[start] {
            continue;
        }
        let mut stack = vec![start];
        let mut component = Vec::new();
        seen[start] = true;
        while let Some(node) = stack.pop() {
            component.push(node);
            for &adj in &graph[node] {
                if !seen[adj] {
                    seen[adj] = true;
                    stack.push(adj);
                }
            }
        }
        component.sort_unstable();
        components.push(component);
    }

    components
}

fn split_components_balanced(
    components: &[Vec<usize>],
    n_total: usize,
) -> (Vec<usize>, Vec<usize>) {
    let mut order: Vec<usize> = (0..components.len()).collect();
    order.sort_by_key(|&idx| std::cmp::Reverse(components[idx].len()));

    let mut part_a = Vec::new();
    let mut part_b = Vec::new();
    for idx in order {
        if part_a.len() <= part_b.len() {
            part_a.extend_from_slice(&components[idx]);
        } else {
            part_b.extend_from_slice(&components[idx]);
        }
    }
    if part_a.is_empty() || part_b.is_empty() {
        let mid = n_total / 2;
        return ((0..mid).collect(), (mid..n_total).collect());
    }
    part_a.sort_unstable();
    part_b.sort_unstable();
    (part_a, part_b)
}

fn normalized_adjacency_second_vector(graph: &[Vec<usize>]) -> Vec<f64> {
    let n = graph.len();
    let degrees: Vec<f64> = graph
        .iter()
        .map(|neighbors| {
            if neighbors.is_empty() {
                1.0
            } else {
                neighbors.len() as f64
            }
        })
        .collect();
    let base: Vec<f64> = degrees.iter().map(|d| d.sqrt()).collect();
    let base_norm = norm_sq(&base).sqrt().max(1e-15);
    let base_unit: Vec<f64> = base.iter().map(|v| v / base_norm).collect();

    let mut x: Vec<f64> = (0..n).map(|i| i as f64 - (n as f64 - 1.0) / 2.0).collect();
    project_out(&mut x, &base_unit);
    normalize(&mut x);

    for _ in 0..256 {
        let mut y = vec![0.0; n];
        for i in 0..n {
            let di_sqrt = degrees[i].sqrt();
            for &j in &graph[i] {
                y[i] += x[j] / (di_sqrt * degrees[j].sqrt());
            }
        }
        project_out(&mut y, &base_unit);
        normalize(&mut y);

        let diff = x
            .iter()
            .zip(&y)
            .map(|(a, b)| {
                let d = a - b;
                d * d
            })
            .sum::<f64>()
            .sqrt();
        let diff_neg = x
            .iter()
            .zip(&y)
            .map(|(a, b)| {
                let d = a + b;
                d * d
            })
            .sum::<f64>()
            .sqrt();
        x = y;
        if diff.min(diff_neg) < 1e-10 {
            break;
        }
    }

    x
}

fn norm_sq(v: &[f64]) -> f64 {
    v.iter().map(|x| x * x).sum()
}

fn project_out(v: &mut [f64], base_unit: &[f64]) {
    let dot = v
        .iter()
        .zip(base_unit.iter())
        .map(|(a, b)| a * b)
        .sum::<f64>();
    for (value, base) in v.iter_mut().zip(base_unit.iter()) {
        *value -= dot * base;
    }
}

fn normalize(v: &mut [f64]) {
    let norm = norm_sq(v).sqrt();
    if norm > 1e-15 {
        for value in v {
            *value /= norm;
        }
    }
}

fn median(values: &[f64]) -> f64 {
    let mut sorted = values.to_vec();
    sorted.sort_by(f64::total_cmp);
    sorted[sorted.len() / 2]
}

/// Count edges that cross between `part_a` and `part_b`.
pub fn count_cut_edges(graph: &[Vec<usize>], part_a: &[usize], part_b: &[usize]) -> usize {
    let mut side = vec![false; graph.len()];
    for &node in part_b {
        side[node] = true;
    }

    let mut cuts = 0;
    for &node in part_a {
        for &adj in &graph[node] {
            if side[adj] {
                cuts += 1;
            }
        }
    }
    cuts
}

/// Improve a bisection with Kernighan-Lin swaps.
pub fn kl_refine(
    graph: &[Vec<usize>],
    mut part_a: Vec<usize>,
    mut part_b: Vec<usize>,
) -> (Vec<usize>, Vec<usize>) {
    let n = graph.len();
    let max_passes = 10;

    for _ in 0..max_passes {
        let mut side = vec![0u8; n];
        for &node in &part_b {
            side[node] = 1;
        }

        let mut d = vec![0i64; n];
        for i in 0..n {
            let (mut ext, mut int) = (0i64, 0i64);
            for &j in &graph[i] {
                if side[j] != side[i] {
                    ext += 1;
                } else {
                    int += 1;
                }
            }
            d[i] = ext - int;
        }

        let mut locked = vec![false; n];
        let mut best_gain_sum = 0i64;
        let mut best_k = 0usize;
        let mut gain_sum = 0i64;
        let mut swaps = Vec::new();

        let pass_limit = part_a.len().min(part_b.len());
        for _ in 0..pass_limit {
            let mut best_pair = None;
            let mut best_gain = i64::MIN;
            for &a in &part_a {
                if locked[a] {
                    continue;
                }
                for &b in &part_b {
                    if locked[b] {
                        continue;
                    }
                    let edge_ab = if graph[a].contains(&b) { 1i64 } else { 0 };
                    let gain = d[a] + d[b] - 2 * edge_ab;
                    if gain > best_gain {
                        best_gain = gain;
                        best_pair = Some((a, b));
                    }
                }
            }

            let Some((a, b)) = best_pair else {
                break;
            };

            swaps.push((a, b));
            gain_sum += best_gain;
            if gain_sum > best_gain_sum {
                best_gain_sum = gain_sum;
                best_k = swaps.len();
            }

            locked[a] = true;
            locked[b] = true;
            side[a] = 1;
            side[b] = 0;

            for &node in graph[a].iter().chain(graph[b].iter()) {
                if locked[node] {
                    continue;
                }
                let (mut ext, mut int) = (0i64, 0i64);
                for &adj in &graph[node] {
                    if side[adj] != side[node] {
                        ext += 1;
                    } else {
                        int += 1;
                    }
                }
                d[node] = ext - int;
            }
        }

        if best_gain_sum <= 0 {
            break;
        }

        let mut set_a: HashSet<usize> = part_a.iter().copied().collect();
        let mut set_b: HashSet<usize> = part_b.iter().copied().collect();
        for &(a, b) in &swaps[..best_k] {
            set_a.remove(&a);
            set_a.insert(b);
            set_b.remove(&b);
            set_b.insert(a);
        }
        part_a = set_a.into_iter().collect();
        part_b = set_b.into_iter().collect();
        part_a.sort_unstable();
        part_b.sort_unstable();
    }

    (part_a, part_b)
}

fn build_subgraph(graph: &[Vec<usize>], nodes: &[usize]) -> Vec<Vec<usize>> {
    let mut old_to_new = vec![usize::MAX; graph.len()];
    for (new, &old) in nodes.iter().enumerate() {
        old_to_new[old] = new;
    }

    nodes
        .iter()
        .map(|&old| {
            graph[old]
                .iter()
                .filter(|&&j| old_to_new[j] != usize::MAX)
                .map(|&j| old_to_new[j])
                .collect()
        })
        .collect()
}

/// Recursively bisect a graph until every partition has at most `target_size` nodes.
pub fn recursive_partition(graph: &[Vec<usize>], target_size: usize) -> Vec<Vec<usize>> {
    if graph.len() <= target_size {
        return vec![(0..graph.len()).collect()];
    }

    let (part_a, part_b) = spectral_bisect(graph);
    let (part_a, part_b) = kl_refine(graph, part_a, part_b);

    let mut result = Vec::new();
    for part in [part_a, part_b] {
        if part.len() <= target_size {
            result.push(part);
        } else {
            let sub_graph = build_subgraph(graph, &part);
            for sub_part in recursive_partition(&sub_graph, target_size) {
                result.push(sub_part.iter().map(|&i| part[i]).collect());
            }
        }
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    fn path_graph(n: usize) -> Vec<Vec<usize>> {
        let mut graph = vec![Vec::new(); n];
        for i in 0..n.saturating_sub(1) {
            graph[i].push(i + 1);
            graph[i + 1].push(i);
        }
        graph
    }

    fn two_cliques_with_bridge() -> Vec<Vec<usize>> {
        vec![
            vec![1, 2],
            vec![0, 2],
            vec![0, 1, 3],
            vec![2, 4, 5],
            vec![3, 5],
            vec![3, 4],
        ]
    }

    #[test]
    fn connected_components_detects_multiple_parts() {
        let graph = vec![vec![1], vec![0], vec![3], vec![2]];
        let components = connected_components(&graph);
        assert_eq!(components, vec![vec![0, 1], vec![2, 3]]);
    }

    #[test]
    fn split_components_balanced_keeps_all_nodes() {
        let components = vec![vec![0, 1, 2], vec![3, 4], vec![5]];
        let (part_a, part_b) = split_components_balanced(&components, 6);
        let mut nodes = part_a.clone();
        nodes.extend_from_slice(&part_b);
        nodes.sort_unstable();
        assert_eq!(nodes, (0..6).collect::<Vec<_>>());
        assert!(!part_a.is_empty());
        assert!(!part_b.is_empty());
    }

    #[test]
    fn spectral_bisect_balances_path_graph() {
        let graph = path_graph(8);
        let (part_a, part_b) = spectral_bisect(&graph);

        assert_eq!(part_a.len() + part_b.len(), graph.len());
        let ratio = part_a.len() as f64 / graph.len() as f64;
        assert!(
            ratio > 0.3 && ratio < 0.7,
            "unbalanced split ratio={ratio:.2}"
        );
    }

    #[test]
    fn count_cut_edges_counts_crossings_once() {
        let graph = two_cliques_with_bridge();
        let cut = count_cut_edges(&graph, &[0, 1, 2], &[3, 4, 5]);
        assert_eq!(cut, 1);
    }

    #[test]
    fn kl_refine_does_not_increase_cut_edges() {
        let graph = two_cliques_with_bridge();
        let (part_a, part_b) = spectral_bisect(&graph);
        let cut_before = count_cut_edges(&graph, &part_a, &part_b);
        let (refined_a, refined_b) = kl_refine(&graph, part_a, part_b);
        let cut_after = count_cut_edges(&graph, &refined_a, &refined_b);

        assert!(
            cut_after <= cut_before,
            "KL increased cut edges: {cut_before} -> {cut_after}"
        );
    }

    #[test]
    fn spectral_bisect_splits_bridge_between_two_cliques() {
        let graph = two_cliques_with_bridge();
        let (part_a, part_b) = spectral_bisect(&graph);
        let cut = count_cut_edges(&graph, &part_a, &part_b);
        assert_eq!(cut, 1, "unexpected cut size: {cut}");
    }

    #[test]
    fn recursive_partition_respects_target_size_and_covers_all_nodes() {
        let graph = path_graph(12);
        let parts = recursive_partition(&graph, 3);

        assert!(parts.iter().all(|part| part.len() <= 3));
        let mut nodes: Vec<usize> = parts.iter().flat_map(|part| part.iter().copied()).collect();
        nodes.sort_unstable();
        assert_eq!(nodes, (0..graph.len()).collect::<Vec<_>>());
    }
}
