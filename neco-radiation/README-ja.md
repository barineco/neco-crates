# neco-radiation

[English](README.md)

`neco-radiation` は、振動面の標本点列や矩形板モードから音響放射パワーを見積もる crate です。

## 推定経路

`RadiationCalculator` は、有効点、法線速度、代表周波数 1 つから直接放射を見積もります。

`ModalRadiationCalculator` は、単純支持矩形板のモードを先に組み立て、有効セルの値を縮約基底へ射影して放射パワーを評価します。

## 使い方

### 点列起点の見積もり

```rust
use neco_radiation::RadiationCalculator;

let calc = RadiationCalculator::new();
let points = [[-0.05, 0.0], [0.05, 0.0]];
let velocities = [0.2, 0.2];
let power = calc.radiated_power(&points, &velocities, 0.01, 440.0);

assert!(power >= 0.0);
```

### 矩形板モードによる見積もり

```rust
use neco_radiation::{ModalRadiationCalculator, RadiationParams};

let params = RadiationParams {
    rho_air: 1.225,
    c_air: 343.0,
    max_modes: 4,
};
let active_cells = vec![(1, 1), (1, 2), (2, 1), (2, 2)];
let calc = ModalRadiationCalculator::new(&params, 5, 5, 0.1, &active_cells, 1.0, 1.0);
let power = calc.radiated_power(&[0.1, 0.2, 0.2, 0.1]);

assert!(power >= 0.0);
assert!(calc.num_modes() <= params.max_modes);
```

## API

| 項目 | 説明 |
| --- | --- |
| `RadiationCalculator::radiated_power` | 点列と値列から直接放射を推定する |
| `RadiationCalculator::modal_efficiency` | モード次数に対する簡易効率係数を返す |
| `ModalRadiationCalculator::new` | 単純支持矩形板のモード情報を前計算する |
| `ModalRadiationCalculator::radiated_power` | 有効セルの値からモード放射を推定する |
| `RadiationParams` | モード推定器の設定値 |

## オプション機能

| 項目 | 説明 |
| --- | --- |
| `serde` | `RadiationParams` に `serde::Deserialize` を付与する |

## ライセンス

MIT
