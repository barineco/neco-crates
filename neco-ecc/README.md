# neco-ecc

[日本語](README-ja.md)

necosystems series Reed-Solomon error correction over GF(2^8).

Provides a `ReedSolomon` codec that supports systematic encoding, erasure correction (known positions), error correction (unknown positions via Berlekamp-Massey), and mixed correction. Built on `neco-gf256`.

## Usage

```rust
use neco_gf256::Gf256;
use neco_ecc::ReedSolomon;

// RS(15,11): 4 parity symbols, corrects up to 2 errors or 4 erasures
let rs = ReedSolomon::new(15, 11).unwrap();
let data: Vec<Gf256> = (1..=11).map(Gf256).collect();
let mut codeword = rs.encode(&data).unwrap();

// introduce an error
codeword[5] = codeword[5] + Gf256(0x42);

// correct it
let result = rs.correct_errors(&mut codeword).unwrap();
assert_eq!(result.errors_corrected, 1);
assert_eq!(&codeword[..11], &data[..]);
```

## API

- `ReedSolomon::new(n, k)`: create codec (n <= 255, k < n)
- `encode(&self, data)`: systematic encoding
- `correct_erasures(&self, received, positions)`: erasure correction
- `correct_errors(&self, received)`: error correction
- `correct(&self, received, erasure_positions)`: mixed correction

## License

MIT
