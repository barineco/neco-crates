use core::fmt;
use neco_eigensolve::{EigensolveConfig, EigensolveResult};
use neco_eigensolve_faer::{solve_symmetric_f64, EigensolveFaerError};
use neco_generalized_eigen::{ConvergenceStatus, GeneralizedEigenError, GeneralizedEigenProblem};
use neco_kmeans::KmeansError;
use neco_linear_types::LinearError;
use neco_sparse::{CooMatrix, CsrMatrix};

#[derive(Clone, Debug, PartialEq)]
pub enum SpectralError {
    InvalidAdjacency { reason: &'static str },
    InvalidClusterCount { requested: usize, nodes: usize },
    Linear(LinearError),
    GeneralizedEigen(GeneralizedEigenError),
    Eigensolve(EigensolveFaerError),
    Kmeans(KmeansError),
}

impl From<LinearError> for SpectralError {
    fn from(error: LinearError) -> Self {
        Self::Linear(error)
    }
}

impl From<GeneralizedEigenError> for SpectralError {
    fn from(error: GeneralizedEigenError) -> Self {
        Self::GeneralizedEigen(error)
    }
}

impl From<EigensolveFaerError> for SpectralError {
    fn from(error: EigensolveFaerError) -> Self {
        Self::Eigensolve(error)
    }
}

impl From<KmeansError> for SpectralError {
    fn from(error: KmeansError) -> Self {
        Self::Kmeans(error)
    }
}

impl fmt::Display for SpectralError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidAdjacency { reason } => {
                write!(formatter, "invalid adjacency matrix: {reason}")
            }
            Self::InvalidClusterCount { requested, nodes } => {
                write!(
                    formatter,
                    "cluster count {requested} is outside 1..={nodes}"
                )
            }
            Self::Linear(error) => error.fmt(formatter),
            Self::GeneralizedEigen(error) => error.fmt(formatter),
            Self::Eigensolve(error) => error.fmt(formatter),
            Self::Kmeans(error) => error.fmt(formatter),
        }
    }
}

impl std::error::Error for SpectralError {}

#[derive(Clone, Debug, PartialEq)]
pub struct SpectralResult {
    assignments: Vec<u32>,
    cluster_count: usize,
    embedding: Vec<Vec<f64>>,
    convergence: ConvergenceStatus,
    kmeans_iterations: usize,
}

impl SpectralResult {
    pub fn assignments(&self) -> &[u32] {
        &self.assignments
    }

    pub fn cluster_count(&self) -> usize {
        self.cluster_count
    }

    pub fn embedding(&self) -> &[Vec<f64>] {
        &self.embedding
    }

    pub fn convergence(&self) -> ConvergenceStatus {
        self.convergence
    }

    pub fn kmeans_iterations(&self) -> usize {
        self.kmeans_iterations
    }
}

pub fn spectral_cluster(
    adjacency: &CsrMatrix<f64>,
    cluster_count: usize,
    eigensolve_config: EigensolveConfig,
    max_kmeans_iterations: usize,
) -> Result<SpectralResult, SpectralError> {
    let (laplacian, mass) = build_laplacian(adjacency)?;
    let nodes = adjacency.shape().rows();
    if cluster_count == 0 || cluster_count > nodes {
        return Err(SpectralError::InvalidClusterCount {
            requested: cluster_count,
            nodes,
        });
    }
    if eigensolve_config.requested_modes() != cluster_count {
        return Err(SpectralError::InvalidAdjacency {
            reason: "the eigensolver mode count must equal the cluster count",
        });
    }

    let problem = GeneralizedEigenProblem::from_csr(&laplacian, &mass)?;
    let eigensolve_result = solve_symmetric_f64(&problem, eigensolve_config)?;
    let embedding = embedding_from_result(nodes, &eigensolve_result)?;
    let dimension = embedding.first().map_or(0, Vec::len);
    if dimension == 0 {
        return Err(SpectralError::InvalidAdjacency {
            reason: "the eigensolver returned no embedding dimensions",
        });
    }
    let mut values = Vec::with_capacity(nodes * dimension);
    for row in &embedding {
        values.extend_from_slice(row);
    }
    let kmeans = neco_kmeans::kmeans(&values, dimension, cluster_count, max_kmeans_iterations)?;

    Ok(SpectralResult {
        assignments: kmeans.assignments,
        cluster_count,
        embedding,
        convergence: eigensolve_result.convergence(),
        kmeans_iterations: kmeans.iterations,
    })
}

fn build_laplacian(
    adjacency: &CsrMatrix<f64>,
) -> Result<(CsrMatrix<f64>, CsrMatrix<f64>), SpectralError> {
    let shape = adjacency.shape();
    if shape.rows() != shape.columns() {
        return Err(SpectralError::InvalidAdjacency {
            reason: "matrix must be square",
        });
    }
    let nodes = shape.rows();
    if nodes == 0 {
        return Err(SpectralError::InvalidAdjacency {
            reason: "matrix must contain at least one node",
        });
    }

    let mut values = vec![0.0; nodes * nodes];
    for row in 0..nodes {
        for (column, value) in adjacency.row(shape.row_index(row)?)?.entries() {
            if !value.is_finite() {
                return Err(SpectralError::InvalidAdjacency {
                    reason: "matrix values must be finite",
                });
            }
            values[row * nodes + column] = *value;
        }
    }
    for row in 0..nodes {
        for column in 0..nodes {
            if values[row * nodes + column] != values[column * nodes + row] {
                return Err(SpectralError::InvalidAdjacency {
                    reason: "matrix must be symmetric",
                });
            }
        }
    }

    let mut degrees = vec![0.0; nodes];
    for row in 0..nodes {
        let degree: f64 = values[row * nodes..(row + 1) * nodes].iter().sum();
        if !degree.is_finite() {
            return Err(SpectralError::InvalidAdjacency {
                reason: "node degrees must be finite",
            });
        }
        degrees[row] = degree;
    }

    let mut laplacian = CooMatrix::new(shape);
    let mut mass = CooMatrix::new(shape);
    for row in 0..nodes {
        let row_index = shape.row_index(row)?;
        for column in 0..nodes {
            let value = if row == column {
                degrees[row] - values[row * nodes + column]
            } else {
                -values[row * nodes + column]
            };
            laplacian.push(row_index, shape.column_index(column)?, value)?;
        }
        let mass_value = if degrees[row] > 0.0 {
            degrees[row]
        } else {
            1.0
        };
        mass.push(row_index, shape.column_index(row)?, mass_value)?;
    }
    Ok((laplacian.to_csr()?, mass.to_csr()?))
}

fn embedding_from_result(
    nodes: usize,
    eigensolve_result: &EigensolveResult,
) -> Result<Vec<Vec<f64>>, SpectralError> {
    let dimensions: usize = eigensolve_result
        .eigenspaces()
        .iter()
        .map(|eigenspace| eigenspace.basis().len())
        .sum();
    let mut embedding = vec![Vec::with_capacity(dimensions); nodes];
    for eigenspace in eigensolve_result.eigenspaces() {
        for pair in eigenspace.basis() {
            for (row, value) in pair.eigenvector().values().iter().enumerate() {
                embedding[row].push(*value);
            }
        }
    }
    for row in &mut embedding {
        let norm = row.iter().map(|value| value * value).sum::<f64>().sqrt();
        if norm.is_finite() && norm > 0.0 {
            for value in row {
                *value /= norm;
            }
        }
    }
    Ok(embedding)
}

#[cfg(test)]
mod tests {
    use super::{spectral_cluster, SpectralError};
    use neco_eigensolve::EigensolveConfig;
    use neco_linear_types::Shape;
    use neco_sparse::{CooMatrix, CsrMatrix};

    fn config(cluster_count: usize) -> EigensolveConfig {
        EigensolveConfig::new(cluster_count, 1.0e-9, 1.0e-9, 64).expect("configuration")
    }

    fn adjacency(nodes: usize, edges: &[(usize, usize, f64)]) -> CsrMatrix<f64> {
        let shape = Shape::new(nodes, nodes);
        let mut matrix = CooMatrix::new(shape);
        for &(left, right, weight) in edges {
            matrix
                .push(
                    shape.row_index(left).expect("row"),
                    shape.column_index(right).expect("column"),
                    weight,
                )
                .expect("entry");
            matrix
                .push(
                    shape.row_index(right).expect("row"),
                    shape.column_index(left).expect("column"),
                    weight,
                )
                .expect("entry");
        }
        matrix.to_csr().expect("CSR")
    }

    #[test]
    fn two_weighted_groups_have_distinct_assignments() {
        let matrix = adjacency(
            6,
            &[
                (0, 1, 4.0),
                (1, 2, 4.0),
                (0, 2, 4.0),
                (3, 4, 4.0),
                (4, 5, 4.0),
                (3, 5, 4.0),
                (2, 3, 0.01),
            ],
        );
        let result = spectral_cluster(&matrix, 2, config(2), 32).expect("clustering");
        assert_eq!(result.assignments().len(), 6);
        assert_ne!(result.assignments()[0], result.assignments()[3]);
        assert!(result.assignments()[..3]
            .iter()
            .all(|assignment| *assignment == result.assignments()[0]));
        assert!(result.assignments()[3..]
            .iter()
            .all(|assignment| *assignment == result.assignments()[3]));
    }

    #[test]
    fn three_weighted_groups_have_distinct_assignments() {
        let matrix = adjacency(
            9,
            &[
                (0, 1, 4.0),
                (1, 2, 4.0),
                (0, 2, 4.0),
                (3, 4, 4.0),
                (4, 5, 4.0),
                (3, 5, 4.0),
                (6, 7, 4.0),
                (7, 8, 4.0),
                (6, 8, 4.0),
                (2, 3, 0.01),
                (5, 6, 0.01),
            ],
        );
        let result = spectral_cluster(&matrix, 3, config(3), 32).expect("clustering");
        assert_eq!(result.assignments().len(), 9);
        assert!(result.assignments()[..3]
            .iter()
            .all(|assignment| *assignment == result.assignments()[0]));
        assert!(result.assignments()[3..6]
            .iter()
            .all(|assignment| *assignment == result.assignments()[3]));
        assert!(result.assignments()[6..]
            .iter()
            .all(|assignment| *assignment == result.assignments()[6]));
        assert_ne!(result.assignments()[0], result.assignments()[3]);
        assert_ne!(result.assignments()[0], result.assignments()[6]);
        assert_ne!(result.assignments()[3], result.assignments()[6]);
    }

    fn scaled(edges: &[(usize, usize, f64)], factor: f64) -> Vec<(usize, usize, f64)> {
        edges
            .iter()
            .map(|(left, right, weight)| (*left, *right, weight * factor))
            .collect()
    }

    #[test]
    fn uniform_positive_weight_scale_preserves_partition() {
        let edges = [
            (0, 1, 4.0),
            (1, 2, 4.0),
            (0, 2, 4.0),
            (3, 4, 4.0),
            (4, 5, 4.0),
            (3, 5, 4.0),
            (2, 3, 0.01),
        ];
        let baseline =
            spectral_cluster(&adjacency(6, &edges), 2, config(2), 32).expect("baseline clustering");
        let scaled = spectral_cluster(&adjacency(6, &scaled(&edges, 7.0)), 2, config(2), 32)
            .expect("scaled clustering");
        for left in 0..6 {
            for right in 0..6 {
                assert_eq!(
                    baseline.assignments()[left] == baseline.assignments()[right],
                    scaled.assignments()[left] == scaled.assignments()[right]
                );
            }
        }
    }

    #[test]
    fn isolated_node_preserves_assignment_shape() {
        let matrix = adjacency(3, &[(0, 1, 2.0)]);
        let result = spectral_cluster(&matrix, 2, config(2), 32).expect("clustering");
        assert_eq!(result.assignments().len(), 3);
        assert_eq!(result.embedding().len(), 3);
        assert!(result
            .assignments()
            .iter()
            .all(|assignment| *assignment < 2));
    }

    #[test]
    fn node_relabeling_preserves_partition() {
        let edges = [
            (0, 1, 4.0),
            (1, 2, 4.0),
            (0, 2, 4.0),
            (3, 4, 4.0),
            (4, 5, 4.0),
            (3, 5, 4.0),
            (2, 3, 0.01),
        ];
        let permutation = [3, 4, 5, 0, 1, 2];
        let relabeled: Vec<_> = edges
            .iter()
            .map(|(left, right, weight)| (permutation[*left], permutation[*right], *weight))
            .collect();
        let baseline =
            spectral_cluster(&adjacency(6, &edges), 2, config(2), 32).expect("baseline clustering");
        let relabeled = spectral_cluster(&adjacency(6, &relabeled), 2, config(2), 32)
            .expect("relabeled clustering");
        for left in 0..6 {
            for right in 0..6 {
                assert_eq!(
                    baseline.assignments()[left] == baseline.assignments()[right],
                    relabeled.assignments()[permutation[left]]
                        == relabeled.assignments()[permutation[right]]
                );
            }
        }
    }

    #[test]
    fn invalid_configuration_returns_a_public_failure() {
        let matrix = adjacency(2, &[(0, 1, 1.0)]);
        assert!(matches!(
            spectral_cluster(&matrix, 0, config(1), 32),
            Err(SpectralError::InvalidClusterCount { .. })
        ));
        assert!(matches!(
            spectral_cluster(&matrix, 2, config(1), 32),
            Err(SpectralError::InvalidAdjacency { .. })
        ));

        let shape = Shape::new(2, 2);
        let mut nonsymmetric = CooMatrix::new(shape);
        nonsymmetric
            .push(
                shape.row_index(0).expect("row"),
                shape.column_index(1).expect("column"),
                1.0,
            )
            .expect("entry");
        assert!(matches!(
            spectral_cluster(&nonsymmetric.to_csr().expect("CSR"), 1, config(1), 32),
            Err(SpectralError::InvalidAdjacency {
                reason: "matrix must be symmetric"
            })
        ));
    }
}
