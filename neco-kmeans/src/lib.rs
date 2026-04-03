//! k-means++ clustering for arbitrary-dimensional f64 vectors.

mod kmeans;

pub use kmeans::{kmeans, KmeansError, KmeansResult};
