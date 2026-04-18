# neco-gf256

[English](README.md)

ゼロ依存の GF(2^8) 有限体演算クレートです。

compile-time 生成の exp/log テーブルに基づく `Gf256` 型と、GF(2^8) 上の多項式型 `Poly` を提供します。既約多項式は AES/Rijndael 標準の x^8 + x^4 + x^3 + x + 1 (0x11B)、原始元は 0x03 です。

## 使い方

```rust
use neco_gf256::{Gf256, Poly};

// 体の演算
let a = Gf256(0x53);
let b = Gf256(0xCA);
let product = a * b;
let inverse = a.inv();
assert_eq!(a * inverse, Gf256::ONE);

// 多項式評価 (Horner 法)
let p = Poly::new(vec![Gf256(7), Gf256(3), Gf256(5)]); // 7 + 3x + 5x^2
let result = p.eval(Gf256(11));
```

## API

- `Gf256`: 体の元。`add`, `mul`, `inv`, `pow`, `exp`, `log`
- `Poly`: 多項式。`eval`, `mul`, `add`, `div_rem`, `scale`, `degree`
- `core::ops::{Add, Sub, Mul, Div}` を `Gf256` に実装

## ライセンス

MIT
