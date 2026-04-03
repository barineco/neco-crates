# neco-radiation

[日本語](README-ja.md)

Acoustic radiation power estimators for vibrating surfaces and rectangular plate modes, using point arrays and modal coefficients.

## Estimator paths

`RadiationCalculator` computes a direct estimate from active sample points, normal velocities, and one representative frequency.

`ModalRadiationCalculator` precomputes simply-supported rectangular plate modes and then evaluates radiation power from active-cell values on that reduced basis.

## Usage

### Direct point/value estimate

```rust
use neco_radiation::RadiationCalculator;

let calc = RadiationCalculator::new();
let points = [[-0.05, 0.0], [0.05, 0.0]];
let velocities = [0.2, 0.2];
let power = calc.radiated_power(&points, &velocities, 0.01, 440.0);

assert!(power >= 0.0);
```

### Reduced plate-mode estimate

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

| Item | Description |
| --- | --- |
| `RadiationCalculator::radiated_power` | Direct radiation estimate from sample points and values |
| `RadiationCalculator::modal_efficiency` | Simple mode-order efficiency heuristic |
| `ModalRadiationCalculator::new` | Precompute simply-supported rectangular plate modal data |
| `ModalRadiationCalculator::radiated_power` | Modal radiation estimate from active cell values |
| `RadiationParams` | Modal estimator parameters |

## Optional features

| Feature | Description |
| --- | --- |
| `serde` | Enables `serde::Deserialize` for `RadiationParams` |

## License

MIT
