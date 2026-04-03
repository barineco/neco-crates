# neco-spectral

[日本語](README-ja.md)

Spectral clustering and recursive graph partitioning for weighted and unweighted graphs.

## Clustering and partitioning

For weighted graphs, the crate builds the unnormalized Laplacian

$$L = D - W,$$

extracts the smallest informative eigenvectors with `neco-eigensolve`, row-normalizes the embedding, and runs `neco-kmeans` on that embedding.

For unweighted adjacency lists, it also includes spectral bisection, Kernighan-Lin refinement, and recursive partitioning.

## Usage

### Cluster a symmetric graph

```rust
use neco_sparse::{CooMat, CsrMat};
use neco_spectral::spectral_cluster;

let mut coo = CooMat::new(100, 100);
for (i, j, w) in edges {
    coo.push(i, j, w);
    coo.push(j, i, w);
}
let adj = CsrMat::from(&coo);

let result = spectral_cluster(&adj, 3, 1e-6, 500, 100);
println!("clusters: {}", result.n_clusters);
println!("assignments: {}", result.assignments.len());
```

### Inspect the embedding

```rust
let result = spectral_cluster(&adj, 3, 1e-6, 500, 100);

// Row-normalized spectral embedding, one row per node
for row in &result.eigenvectors {
    println!("{row:?}");
}
```

## API

| Item | Description |
|------|-------------|
| `spectral_cluster(adjacency, n_clusters, tol, max_eigen_iter, max_kmeans_iter)` | Run the full spectral clustering pipeline |
| `spectral_bisect(graph)` | Split an unweighted graph with the second normalized-adjacency vector used by the current bisection routine |
| `kl_refine(graph, part_a, part_b)` | Improve a bisection with Kernighan-Lin swaps |
| `recursive_partition(graph, target_size)` | Recursively bisect until all parts are within the target size |
| `count_cut_edges(graph, part_a, part_b)` | Count edges that cross between two partitions |
| `SpectralResult` | Returns assignments, cluster count, embedding, and iteration counts |
| `SpectralResult::assignments` | Cluster ID for each node |
| `SpectralResult::eigenvectors` | Row-normalized embedding used by k-means |
| `SpectralResult::eigen_iterations` | LOBPCG iteration count |
| `SpectralResult::kmeans_iterations` | k-means iteration count |

## License

MIT
