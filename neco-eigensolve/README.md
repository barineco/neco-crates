# neco-eigensolve

[日本語](README-ja.md)

Sparse generalized eigenvalue solvers for vibration modes, resonances, and other selected eigenpairs in large sparse systems.

A Japanese mathematical note is available in [MATH-ja.md](MATH-ja.md).

## Solvers

neco-eigensolve has two solvers for

$$K\mathbf{x} = \lambda M\mathbf{x}$$

where `K` and `M` are sparse symmetric matrices.

- `lobpcg` computes the smallest eigenpairs and is suited to modal analysis or graph-like problems.
- `feast_solve_interval` extracts all eigenpairs inside a spectral interval.

LOBPCG uses a Rayleigh-quotient iteration with a preconditioned search space. FEAST builds a filtered subspace by contour integration. Algorithm details and limits are summarized in the Japanese note [MATH-ja.md](MATH-ja.md).

`feast_solve_interval` is the main FEAST entry point. By default it uses the iterative path and returns `Err` on invalid configuration, failed contour-point solves, or non-convergence within `max_loops`.

The optional direct LU path is still for comparison and validation. Current tests cover diagonal problems, tridiagonal and other banded systems, permutation-similar cases, mismatched `K` / `M` sparsity patterns, bounded row-pivot cases, and shift-regularized cases.

Its limits are narrower than the default path. The internal LU backend uses bounded row-only pivoting, keeps the symbolic pattern unchanged after pivoting, and returns `Err` when a stable pivot lies outside the pivot window or when the interval is too small to provide enough contour-shift regularization.

The crate also includes Craig-Bampton component-mode synthesis, IC(0) preconditioning, and CheFSI utilities for polynomial filtering.

## Usage

### LOBPCG for the smallest modes

```rust
use neco_eigensolve::{lobpcg, JacobiPreconditioner, LobpcgResult};
use neco_sparse::CsrMat;

let k_mat: CsrMat<f64> = /* ... */;
let m_mat: CsrMat<f64> = /* ... */;

let precond = JacobiPreconditioner::new(&k_mat);
let result: LobpcgResult = lobpcg(&k_mat, &m_mat, 3, 1e-8, 500, &precond);

println!("eigenvalues: {:?}", result.eigenvalues);
println!(
    "mode matrix shape: {}x{}",
    result.eigenvectors.nrows(),
    result.eigenvectors.ncols()
);
println!("iterations: {}", result.iterations);
```

### FEAST for an interval

```rust
use neco_eigensolve::{feast_solve_interval, FeastConfig, FeastInterval};
use neco_sparse::CsrMat;

let k_mat: CsrMat<f64> = /* ... */;
let m_mat: CsrMat<f64> = /* ... */;

let interval = FeastInterval {
    lambda_min: 0.0,
    lambda_max: 100.0,
};
let config = FeastConfig {
    m0: 30,
    ..Default::default()
};

let result = feast_solve_interval(&k_mat, &m_mat, &interval, &config, None).unwrap();
println!("found {} eigenvalues", result.eigenvalues.len());
```

### Progress callbacks

```rust
use neco_eigensolve::{feast_solve_interval, lobpcg_with_progress};

let _lobpcg = lobpcg_with_progress(
    &k_mat,
    &m_mat,
    3,
    1e-8,
    500,
    &precond,
    |iter, max_iter| eprintln!("lobpcg {iter}/{max_iter}"),
);

let mut on_progress = |info: &neco_eigensolve::FeastIterationInfo| {
    eprintln!(
        "loop {}: trace_change={:.2e}, converged={}",
        info.loop_idx, info.trace_change, info.converged
    );
};

let _feast = feast_solve_interval(
    &k_mat,
    &m_mat,
    &interval,
    &config,
    Some(&mut on_progress),
).unwrap();
```

## API

| Item | Description |
|------|-------------|
| `lobpcg(K, M, n_modes, tol, max_iter, precond)` | Solve for the smallest eigenpairs and return `LobpcgResult` |
| `lobpcg_with_progress(...)` | Same solver with an `FnMut(usize, usize)` progress callback |
| `lobpcg_configured(K, M, config, precond)` | LOBPCG entry point with explicit `LobpcgConfig` |
| `LobpcgConfig` | Controls mode count, tolerance, iteration count, and DC deflation |
| `JacobiPreconditioner::new(&K)` | Build a diagonal preconditioner from `K` |
| `Ic0Preconditioner::new(&K, m_diag)` | Validate the input pattern and build an incomplete Cholesky preconditioner, where `m_diag` is `Option<&[f64]>` |
| `cms::craig_bampton_reduce(K, M, boundary_dofs, n_interior_modes)` | Reduce a substructure with Craig-Bampton component-mode synthesis |
| `cms::couple_cb_systems(a, b, interface_pairs)` | Couple two reduced Craig-Bampton systems across a shared interface |
| `DenseMatrix` | Lightweight column-major dense matrix used in public results and preconditioner blocks |
| `chefsi::lump_mass(&M)` | Build a lumped mass diagonal for CheFSI-style polynomial filtering |
| `chefsi::random_subspace_with_seed(n, m, seed)` | Build a deterministic initial CheFSI subspace from an explicit seed |
| `chefsi::filter::apply_chebyshev_filter(...)` | Apply the low-pass Chebyshev filter on an abstract implementation |
| `chefsi::rayleigh_ritz::rayleigh_ritz(...)` | Extract Ritz pairs from a filtered subspace |
| `Preconditioner` | Trait for custom residual preconditioners on `DenseMatrix` residual blocks |
| `LobpcgResult` | Returned eigenvalues, `DenseMatrix` eigenvectors, and iteration count |
| `feast_solve_interval(K, M, interval, config, on_progress)` | Solve for eigenpairs inside an interval with the default GMRES implementation |
| `FeastConfig` | Controls subspace size, quadrature count, tolerance, loops, and seed |
| `FeastInterval` | Spectral interval `[lambda_min, lambda_max]` |
| `FeastIterationInfo` | Progress information passed to the FEAST callback |
| `FeastIntervalResult` | Returned eigenvalues, eigenvectors, and residuals |

### Optional features

| Feature | Description |
|---------|-------------|
| `parallel` | Enables rayon-based parallel evaluation of FEAST contour points |
| `faer-lu` | Enables optional direct LU support for FEAST comparison and validation |

## License

MIT
