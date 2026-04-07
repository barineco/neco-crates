# neco-galois

[English](README.md)

secp256k1 と P-256 の素体演算を提供する necosystems series クレートです。

256-bit 整数 (`U256`)、モンゴメリ乗算で実装された汎用素体 (`Fp<P>`)、secp256k1 と P-256 の素体・群位数定数、RFC 6979 に基づく決定論的ノンス生成 (HMAC-DRBG) を提供します。

## モジュール構成

- `bigint`: 4 limb (little-endian) で表現する `U256`
- `fp`: `Fp<P>`、`PrimeField` トレイト、モンゴメリ縮約 `redc`
- `secp256k1`: `Secp256k1Field`、`Secp256k1Order`、平方根用指数 `SQRT_EXP_*`
- `p256`: `P256Field`、`P256Order`、平方根用指数 `SQRT_EXP_*`
- `rfc6979`: 決定論的ノンス生成 `generate_k`

## 使い方

```rust
use neco_galois::{Fp, PrimeField, Secp256k1Field, U256};

// 大整数演算 (キャリー付き)
let a = U256::from_u64(0xdead_beef);
let b = U256::from_u64(0xcafe_babe);
let (sum, _carry) = U256::add(a, b);

// モンゴメリ形式の素体要素
let one = Fp::<Secp256k1Field>::one();
let two = Fp::<Secp256k1Field>::add(one, one);
let four = Fp::<Secp256k1Field>::mul(two, two);
# let _ = (sum, four);
```

## ライセンス

MIT
