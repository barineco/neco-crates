# neco-ecc

[English](README.md)

necosystems series の GF(2^8) 上 Reed-Solomon 誤り訂正符号クレートです。

systematic encoding、消失訂正 (位置既知)、誤り訂正 (位置未知、Berlekamp-Massey)、混合訂正を提供します。`neco-gf256` 上に構築されています。

## 使い方

```rust
use neco_gf256::Gf256;
use neco_ecc::ReedSolomon;

// RS(15,11): パリティ 4 シンボル、最大 2 誤りまたは 4 消失を訂正
let rs = ReedSolomon::new(15, 11).unwrap();
let data: Vec<Gf256> = (1..=11).map(Gf256).collect();
let mut codeword = rs.encode(&data).unwrap();

// 誤りを挿入
codeword[5] = codeword[5] + Gf256(0x42);

// 訂正
let result = rs.correct_errors(&mut codeword).unwrap();
assert_eq!(result.errors_corrected, 1);
assert_eq!(&codeword[..11], &data[..]);
```

## API

- `ReedSolomon::new(n, k)`: コーデック生成 (n <= 255, k < n)
- `encode(&self, data)`: systematic encoding
- `correct_erasures(&self, received, positions)`: 消失訂正
- `correct_errors(&self, received)`: 誤り訂正
- `correct(&self, received, erasure_positions)`: 混合訂正

## ライセンス

MIT
