# `tantra-ui-api-wasm` — Pure-Decimal Compute Facade

Portable / WASM. Deps: `tantra-core`, `tantra-ui-core`, `rust_decimal` (with `maths` feature),
`serde`, `thiserror`. All math is `rust_decimal::Decimal`. The WASM shim layer (JS `bindgen`)
is deliberately thin and lives outside this crate — `Bindings` is a plain unit-struct facade
over the pure functions.

## Modules

| Module | Contents |
| :--- | :--- |
| `analytics.rs` | `sharpe_ratio`, `sortino_ratio`, `drawdown`. |
| `indicators.rs` | `sma`, `ema`, `rsi`. |
| `margin.rs` | `margin_required`. |
| `pairing.rs` | `PairRow`, `pair_series`, `pair_series_truncate`. |
| `payoff.rs` | `OptionType`, `payoff`, `payoff_net`, `payoff_curve`. |
| `pricing.rs` | `notional`, `pnl`, `mtm`, `break_even`. |
| `replay.rs` | `replay_ticks` → `PositionView`. |
| `screener.rs` | The canonical screener engine: `apply_scan`, `filter_group_matches`, `row`, `ScanError` — evaluates `tantra_core::screener::ScreenerScanRequest` over `ScreenerRow`s. |
| `lib.rs` | `Bindings` facade, `ApiError`, `ApiResult`. |

## Design notes

- **Screener canonicality**: `apply_scan` is the evaluation engine `tantra-cli scan`,
  `tantra-ui-app::ScreenerFeature`, and the fixture-universe scans all route through.
- **Zero floats**: every numeric path is `Decimal`; the f64 mentions in doc comments are
  documentation only (zero actual `f64` usages).

## Tests

`tests/analytics_test.rs`, `indicators_test.rs`, `margin_test.rs`, `pairing_test.rs`,
`payoff_test.rs`, `pricing_test.rs`, `replay_test.rs`, `screener_test.rs` (37 `#[test]`
functions).
