# neco-rand

決定論的な非暗号乱数生成と、安定したバケット割り当てを行う crate です。

シミュレーション、サンプリング、グループ分割に使います。暗号鍵、セキュアなノンス、その他のセキュリティ用途には使いません。

## 機能

- `SplitMix64` によるシード展開
- `Xoroshiro128Plus` による決定論的 PRNG
- `key + experiment + salt` からの安定なバケット割り当て

## 使い方

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

| 項目 | 説明 |
|------|------|
| `SplitMix64` | 決定論的な種子展開と1回混合器 |
| `Xoroshiro128Plus` | 非暗号用途向けの高速 PRNG |
| `bucket` | 安定したグループ割当て補助 |

## ライセンス

MIT
