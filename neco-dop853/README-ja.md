# neco-dop853

[English](README.md)

`neco-dop853` は、スライスベースの状態ベクトルで常微分方程式を積分する軽量な Dormand-Prince 8(5,3) 適応刻み解法です。

## 積分モデル

`integrate_dop853` は、陽的 ODE 系を内部では適応刻みで進め、必要な `t_eval` の時刻だけを `Dop853Result` として返します。

返り値は標本化した時刻列、状態列、`success` フラグに絞ってあり、周辺の大きな実装へ依存しません。

## 使い方

```rust
use neco_dop853::{integrate_dop853, Dop853Options};

let rhs = |_t: f64, y: &[f64], dydt: &mut [f64]| {
    dydt[0] = -y[0];
};

let t_eval = [0.0, 0.5, 1.0, 2.0];
let result = integrate_dop853(
    rhs,
    (0.0, 2.0),
    &[1.0],
    &t_eval,
    &Dop853Options::default(),
);

assert!(result.success);
assert_eq!(result.t, t_eval);
assert!((result.y[3][0] - (-2.0f64).exp()).abs() < 1e-10);
```

## API

| 項目 | 説明 |
|------|------|
| `integrate_dop853(rhs, t_span, y0, t_eval, opts)` | 陽的 ODE 系を積分し、要求した時刻で解を返す |
| `Dop853Options` | 相対許容誤差、絶対許容誤差、最大刻み幅、初期刻み幅のヒント（`0.0` は自動選択） |
| `Dop853Options::default()` | 高精度な前進積分を想定した保守的な既定値 |
| `Dop853Result` | 返却時刻列、状態列、成功フラグ、採択ステップ数、RHS 評価回数 |
| `Dop853Result::success` | 内部停止条件に当たらず、要求した `t_eval` を最後まで消化できたとき `true` |

### 前提条件

- `rhs` は、状態ベクトルの各成分に対応する微分値を `dydt` へ必ず書き込む必要がある
- `t_eval` は単調非減少で、`t_span` の範囲内にあることを前提とする
- 現在の実装は前進積分（`t_span.0 <= t_span.1`）を対象とする
- `t_span.0` と一致する評価時刻は、積分を進めずに初期状態として返す
- ステップ間の中間時刻は、受理された 2 点の間を Hermite 3 次補間して返す
- すべての出力時刻へ到達できなかった場合、そこまでに蓄積した部分結果を返し、`success` は `false` になる

### 失敗時の扱い

現時点では `Result` ではなく `Dop853Result::success` で積分失敗を返します。

典型的な `success = false` 条件:

- 要求した `t_eval` に、到達可能な `t_span` の外側が含まれている
- 適応刻み幅が内部しきい値より小さくなり、これ以上前進できない
- 内部ステップ上限に達し、要求した出力時刻を最後まで処理できない

## ライセンス

MIT
