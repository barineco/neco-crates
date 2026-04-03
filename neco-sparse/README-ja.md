# neco-sparse

[English](README.md)

疎行列の組み立てと行反復ベースの疎線形代数のための、軽量な COO / CSR 行列型です。

## 行列形式

neco-sparse は 2 つの形式に絞っています。

- `CooMat<T>`: `(row, col, value)` の三つ組を積む組み立て用形式
- `CsrMat<T>`: 行ごとに連続配置し、行反復を `O(nnz)` で回せる計算用形式

典型的には、まず COO でエントリを集め、重複を許したまま組み立て、最後に CSR へ変換して反復処理、対角抽出、部分行列構築、呼び出し側の SpMV に使います。

## 使い方

### CSR の直接構築

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

### COO から CSR への変換

```rust
use neco_sparse::{CooMat, CsrMat};

let mut coo = CooMat::new(3, 3);
coo.push(0, 0, 2.0);
coo.push(1, 2, 5.0);
coo.push(0, 0, 1.0);

let csr = CsrMat::from(&coo);
assert_eq!(csr.get(0, 0), Some(&3.0));
```

### 行反復と補助行列生成

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

### 対角および部分行列の取り出し

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

| 項目 | 説明 |
|------|-------------|
| `CooMat<T>` | 組み立て用の座標形式疎行列 |
| `CooMat::new(rows, cols)` | 空の COO 行列を作る |
| `CooMat::push(row, col, value)` | 三つ組エントリを 1 件追加する |
| `CsrMat<T>` | 計算用の行圧縮形式疎行列 |
| `CsrMat::try_from_csr_data(...)` | 生配列から検証付きで CSR を構築する |
| `From<&CooMat<T>> for CsrMat<T>` | COO を CSR に変換し、重複座標を加算する |
| `CsrMat::row(i)` / `row_iter()` | 単一行アクセスまたは行反復 |
| `CsrRow<'a, T>` | 列番号と値を返す借用行ビュー |
| `CsrMat::identity(n)` / `zeros(rows, cols)` | `f64` 用の補助コンストラクタ |
| `CsrMat::linear_combination(alpha, other, beta)` | CSR パターンが一致するときに `alpha * self + beta * other` を構築する |
| `CsrMat::diagonal()` | 対角成分を抽出し、必要な成分が欠けていれば `Err` を返す |
| `CsrMat::submatrix(rows, cols)` | 重複のない行/列インデックス列から CSR 部分行列を構築する |

## ライセンス

MIT
