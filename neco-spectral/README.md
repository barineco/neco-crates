# neco-spectral

[日本語](README-ja.md)

Spectral clustering and recursive graph partitioning for weighted and unweighted graphs.

## Clustering and partitioning

For a weighted graph, the crate builds the unnormalized Laplacian:

$$L = D - W.$$

The degree matrix provides the mass matrix for a generalized eigenvalue problem. The resulting embedding is row-normalized before k-means assigns every node to a cluster. An isolated node remains in the result.

Unweighted adjacency lists can use spectral bisection, Kernighan-Lin refinement, and recursive partitioning.

Exact adjacency clustering uses these public values:

- `ExactSpectralRequest`: exact adjacency, input identity, input revision, and projection policy
- `ClusteringAdjacency`: projection purpose
- `spectral_cluster_exact`: one certified numerical projection and one clustering result
- `SpectralProjectionReference`: the retained input and projection correspondence

## Usage

### Cluster a symmetric graph

```rust
use neco_eigensolve::EigensolveConfig;
use neco_linear_types::Shape;
use neco_sparse::{CooMatrix, CsrMatrix};
use neco_spectral::spectral_cluster;

let shape = Shape::new(6, 6);
let mut adjacency = CooMatrix::new(shape);
for (left, right, weight) in [
    (0, 1, 4.0),
    (1, 2, 4.0),
    (0, 2, 4.0),
    (3, 4, 4.0),
    (4, 5, 4.0),
    (3, 5, 4.0),
    (2, 3, 0.01),
] {
    adjacency.push(shape.row_index(left)?, shape.column_index(right)?, weight)?;
    adjacency.push(shape.row_index(right)?, shape.column_index(left)?, weight)?;
}
let adjacency: CsrMatrix<f64> = adjacency.to_csr()?;
let config = EigensolveConfig::new(2, 1.0e-9, 1.0e-9, 64)?;
let result = spectral_cluster(&adjacency, 2, config, 32)?;

println!("clusters: {}", result.cluster_count());
println!("assignments: {}", result.assignments().len());
# Ok::<(), Box<dyn std::error::Error>>(())
```

### Inspect the embedding

```rust
# use neco_eigensolve::EigensolveConfig;
# use neco_linear_types::Shape;
# use neco_sparse::CooMatrix;
# use neco_spectral::spectral_cluster;
# let shape = Shape::new(2, 2);
# let mut adjacency = CooMatrix::new(shape);
# adjacency.push(shape.row_index(0)?, shape.column_index(1)?, 1.0)?;
# adjacency.push(shape.row_index(1)?, shape.column_index(0)?, 1.0)?;
# let adjacency = adjacency.to_csr()?;
# let config = EigensolveConfig::new(1, 1.0e-9, 1.0e-9, 64)?;
let result = spectral_cluster(&adjacency, 1, config, 32)?;

for row in result.embedding() {
    println!("{row:?}");
}
# Ok::<(), Box<dyn std::error::Error>>(())
```

## API

- Clustering:
  - `spectral_cluster`
  - `spectral_cluster_exact`
  - `ExactSpectralRequest`
  - `SpectralProjectionReference`
  - `SpectralResult::assignments()`
  - `SpectralResult::cluster_count()`
  - `SpectralResult::embedding()`
  - `SpectralResult::convergence()`
  - `SpectralResult::kmeans_iterations()`
  - `SpectralError`
- Partitioning:
  - `spectral_bisect(graph)`
  - `kl_refine(graph, part_a, part_b)`
  - `recursive_partition(graph, target_size)`
  - `count_cut_edges(graph, part_a, part_b)`

```text
spectral_cluster(adjacency, cluster_count, eigensolve_config, max_kmeans_iterations)
spectral_cluster_exact(exact_request)
```

## Preconditions

- Square, finite, symmetric adjacency matrix.
- Cluster count within `1..=node_count`.
- Requested eigensolver mode count equal to cluster count.

## License

MIT
