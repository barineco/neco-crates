# neco-eigensolve

[English](README.md)

大規模疎行列から振動モード、共鳴周波数、そのほか必要な固有対だけを取り出すための一般化固有値ソルバーです。

詳細な数理背景は [MATH-ja.md](MATH-ja.md) を参照してください。

## ソルバー

neco-eigensolve には

$$K\mathbf{x} = \lambda M\mathbf{x}$$

を解く 2 種類の解法があります。

- `lobpcg`: 最小固有値側の固有対を求める
- `feast_solve_interval`: 指定区間に入る固有対をまとめて抽出する

LOBPCG は Rayleigh 商にもとづく前処理付き反復法、FEAST は輪郭積分で区間内成分を抽出する方法です。README では概要だけを示し、アルゴリズム詳細と制約は [MATH-ja.md](MATH-ja.md) に分離しています。

`feast_solve_interval` は FEAST の主入口です。既定では反復法ベースの解法を使い、設定が不正な場合、輪郭点ごとの線形方程式を解く準備に失敗した場合、`max_loops` までに収束しなかった場合は `Err` を返します。

オプションの直接 LU 分解経路は比較検証が主な用途です。現在のテストでは、対角問題、三重対角や帯行列、置換相似な問題、`K` と `M` の疎構造がずれる問題、境界付き行指向ピボッティングで解ける問題、複素シフトで対角優位が十分な問題を確認しています。

ただし守備範囲は既定経路より狭いままです。内部 LU 経路は境界付き行指向ピボッティングを使い、ピボット後に記号的パターンの再構成は行いません。そのため、探索窓内で安定ピボットを得られない場合や、極端に狭い区間で輪郭シフトの正則化が不足する場合は `Err` を返します。

加えて、Craig-Bampton の成分モード合成、IC(0) 前処理、CheFSI の多項式フィルタ中核部も公開しています。

## 使い方

### LOBPCG による低次モード抽出

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

### FEAST 区間抽出

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

### 進捗コールバック

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

| 項目 | 説明 |
|------|-------------|
| `lobpcg(K, M, n_modes, tol, max_iter, precond)` | 最小固有対を求めて `LobpcgResult` を返す |
| `lobpcg_with_progress(...)` | 進捗コールバック付きの LOBPCG |
| `lobpcg_configured(K, M, config, precond)` | `LobpcgConfig` を明示して解く入口 |
| `LobpcgConfig` | モード数、許容誤差、反復回数、DC 除去を設定する |
| `JacobiPreconditioner::new(&K)` | `K` から対角前処理を構築する |
| `Ic0Preconditioner::new(&K, m_diag)` | 入力行列を検証し、不完全コレスキー前処理を構築する。`m_diag` は `Option<&[f64]>` |
| `cms::craig_bampton_reduce(K, M, boundary_dofs, n_interior_modes)` | Craig-Bampton の成分モード合成で部分構造を縮退する |
| `cms::couple_cb_systems(a, b, interface_pairs)` | 共有界面を介して 2 つの Craig-Bampton 縮退系を結合する |
| `DenseMatrix` | 公開戻り値と前処理ブロックに使う軽量な列優先の密行列 |
| `chefsi::lump_mass(&M)` | CheFSI 系フィルタに使う集中質量対角を構築する |
| `chefsi::random_subspace_with_seed(n, m, seed)` | 乱数シードを明示して決定的な CheFSI 初期部分空間を構築する |
| `chefsi::filter::apply_chebyshev_filter(...)` | 抽象化した実装層上で低域 Chebyshev フィルタ中核部を適用する |
| `chefsi::rayleigh_ritz::rayleigh_ritz(...)` | フィルタ済み部分空間から Ritz 対を抽出する |
| `Preconditioner` | `DenseMatrix` 残差ブロックを受ける独自前処理 trait |
| `LobpcgResult` | 固有値、`DenseMatrix` 固有ベクトル、反復回数を返す |
| `feast_solve_interval(K, M, interval, config, on_progress)` | 既定の GMRES 実装で区間内固有対を抽出する |
| `FeastConfig` | 部分空間サイズ、求積点数、許容誤差、反復回数、乱数シードを設定する |
| `FeastInterval` | 区間 `[lambda_min, lambda_max]` を表す |
| `FeastIterationInfo` | FEAST の進捗コールバック情報 |
| `FeastIntervalResult` | 固有値、固有ベクトル、残差を返す |

### オプション機能

| 項目 | 説明 |
|---------|-------------|
| `parallel` | FEAST の輪郭点評価を rayon で並列化する |
| `faer-lu` | FEAST の比較・検証向けに直接 LU 分解を有効化する |

## ライセンス

MIT
