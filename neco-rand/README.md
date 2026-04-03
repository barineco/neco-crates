# neco-rand

Deterministic non-cryptographic random generators and stable bucket assignment.

Use this crate for simulation, sampling, and deterministic cohort splitting. Use cryptographic key handling, secure nonces, and security-sensitive random generation in dedicated cryptographic random crates.

## Features

- `SplitMix64` seed expansion
- `Xoroshiro128Plus` deterministic PRNG
- Stable bucket assignment from `key + experiment + salt`

## Usage

```rust
use neco_rand::{bucket, SplitMix64, Xoroshiro128Plus};

let mut seeder = SplitMix64::new(42);
let seed = seeder.next_u64();

let mut rng = Xoroshiro128Plus::new(seed);
let value = rng.next_f64();

let bucket_index = bucket::assign_bucket(b"user-1", b"exp-a", b"salt", 100);

assert!((0.0..1.0).contains(&value));
assert!(bucket_index < 100);
```

## API

| Item | Description |
|------|-------------|
| `SplitMix64` | Deterministic seed expansion and one-shot mixing |
| `Xoroshiro128Plus` | Fast deterministic PRNG for non-cryptographic use |
| `bucket` | Stable cohort assignment helpers |

## License

MIT
