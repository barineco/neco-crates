# neco-sparse

[日本語](README-ja.md)

Lightweight COO and CSR sparse matrix types for assembling sparse operators and iterating rows efficiently in caller-side sparse linear algebra code.

## Matrix formats

neco-sparse focuses on two storage formats:

- `CooMat<T>` for assembly, where entries are pushed as `(row, col, value)` triplets
- `CsrMat<T>` for computation, where rows are contiguous and row-wise traversal runs in `O(nnz)`

The common workflow is to accumulate entries in COO, allow duplicates during assembly, and convert once to CSR for iteration, diagonal extraction, submatrix construction, or caller-side SpMV.

## Usage

### Build CSR directly

```rust
use neco_sparse::CsrMat;

let csr = CsrMat::try_from_csr_data(
    2,
    3,
    vec![0, 2, 3],
    vec![0, 2, 1],
    vec![1.0, 4.0, 3.0],
).unwrap();

assert_eq!(csr.nnz(), 3);
assert_eq!(csr.get(0, 2), Some(&4.0));
```

### Assemble in COO, then convert

```rust
use neco_sparse::{CooMat, CsrMat};

let mut coo = CooMat::new(3, 3);
coo.push(0, 0, 2.0);
coo.push(1, 2, 5.0);
coo.push(0, 0, 1.0);

let csr = CsrMat::from(&coo);
assert_eq!(csr.get(0, 0), Some(&3.0));
```

### Iterate rows and build auxiliary matrices

```rust
use neco_sparse::CsrMat;

let eye = CsrMat::identity(4);
let zeros = CsrMat::zeros(3, 5);

for row in eye.row_iter() {
    for (&col, &val) in row.col_indices().iter().zip(row.values()) {
        println!("col={col}, val={val}");
    }
}

assert_eq!(zeros.nnz(), 0);
```

### Extract diagonals and submatrices

```rust
use neco_sparse::{CooMat, CsrMat};

let mut coo = CooMat::new(3, 3);
coo.push(0, 0, 4.0);
coo.push(0, 2, 1.0);
coo.push(1, 1, 5.0);
coo.push(2, 0, 2.0);
coo.push(2, 2, 6.0);

let csr = CsrMat::from(&coo);
let diag = csr.diagonal().unwrap();
let sub = csr.submatrix(&[2, 0], &[2, 0]).unwrap();

assert_eq!(diag, vec![4.0, 5.0, 6.0]);
assert_eq!(sub.row(0).values(), &[6.0, 2.0]);
```

## API

| Item | Description |
|------|-------------|
| `CooMat<T>` | Coordinate-format sparse matrix for assembly |
| `CooMat::new(rows, cols)` | Create an empty COO matrix |
| `CooMat::push(row, col, value)` | Add one triplet entry |
| `CsrMat<T>` | Compressed sparse row matrix for computation |
| `CsrMat::try_from_csr_data(...)` | Construct CSR directly from raw arrays with validation |
| `From<&CooMat<T>> for CsrMat<T>` | Convert COO to CSR and sum duplicate coordinates |
| `CsrMat::row(i)` / `row_iter()` | Access one row or iterate rows |
| `CsrRow<'a, T>` | Borrowed row view with column indices and values |
| `CsrMat::identity(n)` / `zeros(rows, cols)` | Convenience constructors for `f64` matrices |
| `CsrMat::linear_combination(alpha, other, beta)` | Build `alpha * self + beta * other` when both CSR patterns match |
| `CsrMat::diagonal()` | Extract the diagonal entries and return `Err` when a required entry is missing |
| `CsrMat::submatrix(rows, cols)` | Build a CSR submatrix from unique row/column index lists |

## License

MIT
