# neco-gf256

[日本語](README-ja.md)

zero dependency GF(2^8) finite field arithmetic.

Provides a compact `Gf256` element type backed by compile-time exp/log tables and a `Poly` polynomial type over GF(2^8). The irreducible polynomial is the AES/Rijndael standard x^8 + x^4 + x^3 + x + 1 (0x11B) with primitive element 0x03.

## Usage

```rust
use neco_gf256::{Gf256, Poly};

// field arithmetic
let a = Gf256(0x53);
let b = Gf256(0xCA);
let product = a * b;
let inverse = a.inv();
assert_eq!(a * inverse, Gf256::ONE);

// polynomial evaluation (Horner's method)
let p = Poly::new(vec![Gf256(7), Gf256(3), Gf256(5)]); // 7 + 3x + 5x^2
let result = p.eval(Gf256(11));
```

## API

- `Gf256`: field element with `add`, `mul`, `inv`, `pow`, `exp`, `log`
- `Poly`: polynomial with `eval`, `mul`, `add`, `div_rem`, `scale`, `degree`
- `core::ops::{Add, Sub, Mul, Div}` implemented for `Gf256`

## License

MIT
