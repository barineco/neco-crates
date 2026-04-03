# neco-kmeans

[日本語](README-ja.md)

Deterministic k-means clustering for partitioning points into `k` groups without random initialization noise.

## Clustering behavior

The implementation uses a deterministic max-distance variant of k-means++ for initialization, then refines assignments with Lloyd iteration. Identical inputs therefore produce identical outputs, which helps in reproducible analysis pipelines.

## Usage

```rust
use neco_kmeans::kmeans;

let data = [
    0.0, 0.0,
    1.0, 0.0,
    10.0, 10.0,
    11.0, 10.0,
];

let result = kmeans(&data, 2, 2, 100).expect("valid clustering input");

println!("assignments: {:?}", result.assignments);
println!("centroids: {:?}", result.centroids);
println!("iterations: {}", result.iterations);
```

## API

| Item | Description |
|------|-------------|
| `kmeans(data, dim, k, max_iter)` | Run deterministic k-means and return `Result<KmeansResult, KmeansError>` |
| `KmeansResult` | Stores assignments, centroids, dimensionality, and iteration count |
| `KmeansResult::centroid(i)` | Borrow the `i`-th centroid as a slice |
| `KmeansResult::k()` | Return the number of clusters |

### Optional features

| Feature | Description |
|---------|-------------|
| `parallel` | Enables rayon-based parallel assignment updates |

## License

MIT
