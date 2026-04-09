# neco crates

[日本語](README-ja.md)

`neco crates` is a set of Rust crates for geometry, numerical computation, visualization, and related math tools.

This repository is organized as a reusable set of crates rather than a single monolithic framework. Current crates cover computational geometry, spline / NURBS processing, sparse and eigenvalue routines, clustering, color and pigment models, STL / mesh processing, and 2D view utilities.

More crates may be added over time.

## Crates

External dependencies list always-on dependencies first, with optional ones grouped in parentheses.
`serde` here means an opt-in `Serialize` / `Deserialize` surface for downstream integration. JSON-specific parsing and encoding live in [`neco-json`](./neco-json).

### Geometry & Meshing

| Crate | Summary | Internal dependencies | Main external dependencies |
|---|---|---|---|
| [`neco-nurbs`](./neco-nurbs) | NURBS curves, surfaces, fitting, and polynomial routines | none | (`nalgebra`) |
| [`neco-brep`](./neco-brep) | boundary representation, solid construction, tessellation, and 3D boolean operations | `neco-nurbs`, `neco-cdt` | none |
| [`neco-mesh`](./neco-mesh) | 2D / 3D mesh generation and mesh utilities | `neco-cdt`, `neco-nurbs`, `neco-stl` | (`serde`) |
| [`neco-stl`](./neco-stl) | STL parsing and writing | none | none |
| [`neco-cdt`](./neco-cdt) | constrained Delaunay triangulation | none | none |
| [`neco-spline`](./neco-spline) | spline interpolation | none | (`serde`) |

### Linear Algebra & Solvers

| Crate | Summary | Internal dependencies | Main external dependencies |
|---|---|---|---|
| [`neco-array2`](./neco-array2) | lightweight row-major 2D array foundation for grid-oriented crates | none | (`serde`) |
| [`neco-complex`](./neco-complex) | lightweight complex-number foundation for FFT- and solver-adjacent crates | none | none |
| [`neco-gridfield`](./neco-gridfield) | uniform 2D grids and triple-buffered field state for time stepping | `neco-array2` | (`serde`) |
| [`neco-contact`](./neco-contact) | Hertz contact and spatial helper routines on uniform 2D fields | `neco-array2` | none |
| [`neco-sparse`](./neco-sparse) | sparse matrix data structures | none | none |
| [`neco-eigensolve`](./neco-eigensolve) | sparse eigenvalue solvers | `neco-sparse` | (`rayon`, `faer`) |
| [`neco-dop853`](./neco-dop853) | adaptive Dormand-Prince 8(5,3) ODE integration | none | none |
| [`neco-stencil`](./neco-stencil) | finite-difference stencil operators on uniform 2D grids | none | (`rayon`) |

### Signal Processing

| Crate | Summary | Internal dependencies | Main external dependencies |
|---|---|---|---|
| [`neco-stft`](./neco-stft) | backend-agnostic real FFT facade, windows, and STFT / ISTFT | `neco-complex` | none |
| [`neco-minphase`](./neco-minphase) | minimum-phase spectrum / impulse kernels and overlap-add convolution | `neco-stft`, `neco-complex` | none |

### Clustering

| Crate | Summary | Internal dependencies | Main external dependencies |
|---|---|---|---|
| [`neco-kmeans`](./neco-kmeans) | k-means clustering | none | (`rayon`) |
| [`neco-spectral`](./neco-spectral) | spectral clustering | `neco-sparse`, `neco-eigensolve`, `neco-kmeans` | none |

### Search & Ranking

| Crate | Summary | Internal dependencies | Main external dependencies |
|---|---|---|---|
| [`neco-fuzzy`](./neco-fuzzy) | minimal fuzzy score core for commands, paths, and short identifiers | none | none |

### Cryptography

| Crate | Summary | Internal dependencies | Main external dependencies |
|---|---|---|---|
| [`neco-rand`](./neco-rand) | deterministic non-cryptographic random generators and stable bucket assignment | none | none |
| [`neco-secp`](./neco-secp) | minimal secp256k1 and Nostr signing core | none | `k256`, `sha2` (`serde_json`, `bech32`, `aes`, `cbc`, `chacha20`, `hkdf`, `hmac`, `base64`) |
| [`neco-vault`](./neco-vault) | memory-only signing vault built on `neco-secp` | `neco-secp` | none (`aes`, `cbc`, `scrypt`, `getrandom`, `sha2`) |
| [`neco-nostr-wasm`](./neco-nostr-wasm) | WebAssembly bindings for `neco-secp` and `neco-vault` | `neco-secp`, `neco-vault` | `bech32`, `serde_json`, `wasm-bindgen` |

### Acoustics

| Crate | Summary | Internal dependencies | Main external dependencies |
|---|---|---|---|
| [`neco-radiation`](./neco-radiation) | acoustic radiation power estimators for vibrating surfaces and plate modes | none | (`serde`) |
| [`neco-modal`](./neco-modal) | modal extraction and modal-set utilities for vibration signals | `neco-stft` | none |

### Color Utilities

| Crate | Summary | Internal dependencies | Main external dependencies |
|---|---|---|---|
| [`neco-color`](./neco-color) | color space and colorimetric utilities | none | none |
| [`neco-pigment`](./neco-pigment) | pigment-oriented spectral and color mixing utilities | `neco-color` | (`serde`) |

### View & Bindings

| Crate | Summary | Internal dependencies | Main external dependencies |
|---|---|---|---|
| [`neco-view2d`](./neco-view2d) | 2D camera / viewport utilities | none | (`serde`) |
| [`neco-view2d-wasm`](./neco-view2d-wasm) | WebAssembly bindings for `neco-view2d` | `neco-view2d` | `wasm-bindgen` |

Most crates are intentionally independent so they can be published and consumed separately on crates.io. The repository is a monorepo for maintenance convenience, not a runtime-coupled framework.

This repository is still under active development, and some crates or code paths are more mature than others. Parts of the workspace are already usable, while other parts are still being hardened, expanded, or reshaped.

Updates may still change internal implementations relatively often. In particular, function inlining / internalization, algorithm swaps, and performance-oriented rewrites are more likely than long-term API stability across every crate.

## Status

- Workspace formatting, lint, and test gates are maintained at the repository level.
- GitHub Actions CI is configured in [`.github/workflows/ci.yml`](./.github/workflows/ci.yml).
- Individual crates may evolve at different speeds.
- Some conventions, especially older comment style inconsistencies, are still being regularized incrementally.

## Contribution

Issues and pull requests are welcome. In practice, focused requests with a clear target are easier to review and validate than broad or vague proposals.

See [CONTRIBUTING.md](./CONTRIBUTING.md) for development workflow and [SECURITY.md](./SECURITY.md) for security reporting.

## Support

If these crates or related apps are useful to you, you can support ongoing development here:

- OFUSE: <https://ofuse.me/barineco>
- Ko-fi: <https://ko-fi.com/barineco>

Support helps sustain maintenance, documentation, and ongoing development.

## License

Unless noted otherwise, this repository is licensed under the [MIT License](./LICENSE).
