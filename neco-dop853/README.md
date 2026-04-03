# neco-dop853

[日本語](README-ja.md)

Adaptive Dormand-Prince 8(5,3) integration for ordinary differential equations on slice-based state vectors.

## Integration model

`integrate_dop853` advances an explicit ODE system on `&[f64]` state vectors and samples the solution at the requested `t_eval` points.

The solver uses adaptive step control internally, but the returned `Dop853Result` stays small: sampled times, sampled states, and a `success` flag.

## Usage

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

| Item | Description |
|------|-------------|
| `integrate_dop853(rhs, t_span, y0, t_eval, opts)` | Integrate an explicit ODE system and sample the solution at the requested times |
| `Dop853Options` | Relative tolerance, absolute tolerance, maximum step, and an `initial_step` hint (`0.0` means automatic choice) |
| `Dop853Options::default()` | Conservative defaults for high-accuracy forward integration |
| `Dop853Result` | Returned sample times, sampled states, success flag, accepted step count, and RHS evaluation count |
| `Dop853Result::success` | `true` when the solver reaches every requested `t_eval` point before hitting the internal stop conditions |

### Preconditions

- `rhs` must write one derivative value per state entry into `dydt`.
- `t_eval` is expected to be monotonically nondecreasing and to stay within `t_span`.
- The current implementation targets forward integration (`t_span.0 <= t_span.1`).
- Sample points exactly at `t_span.0` are returned as the initial state without advancing the solver.
- Intermediate `t_eval` points are reconstructed with Hermite cubic interpolation between accepted steps.
- If the solver cannot reach every requested output time, it returns the partial output it has accumulated and sets `success` to `false`.

### Failure semantics

This crate currently reports integration failure through `Dop853Result::success` instead of `Result`.

Typical `success = false` cases:

- some requested `t_eval` values lie beyond the reachable part of `t_span`
- the adaptive step size shrinks below the internal minimum threshold
- the solver hits the internal step cap before consuming all requested outputs

## License

MIT
