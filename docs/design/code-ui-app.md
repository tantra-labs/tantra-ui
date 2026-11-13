# `tantra-ui-app` — WASM-Safe Feature-State Models

Portable / WASM. Deps: `tantra-core`, `tantra-ui-core`, `tantra-ui-assets`, `tantra-ui-charts`,
`tantra-ui-client`, `tantra-ui-api-wasm`, `tantra-highlight`, `serde`, `thiserror`,
`rust_decimal`, `uuid` (`rng-getrandom`), `getrandom`. **Not a Leptos app** — there is no
`leptos` dependency anywhere in the workspace (confirmed in `Cargo.lock`); this crate is a pure
feature-state model layer. A CSR view layer is open gap 0004.

## Modules

| Module | Contents |
| :--- | :--- |
| `router.rs` | `AppRoute` (8 routes: Home, Screener, Studio, Simulator, System, Backtest, Portfolio, Algos) with `label()`/`path()`/`ALL`; `Router { active, previous }` with `navigate()` (errors `AlreadyActive`) and `back()`. |
| `screener.rs` | `ScreenerFeature` — drafts `ScreenerScanRequest`, `run_scan(&[ScreenerRow])` calls `tantra_ui_api_wasm::screener::apply_scan` and folds into `ScreenerViewModel`. |
| `studio.rs` | `StudioCanvas` node graph with real **cycle detection** (`would_cycle` DFS) and `NodeNotFound`/`Cycle`/`DuplicateConnection` errors; `StudioNodeKind` = Source(InstrumentId)/Indicator/Signal/Order/Constant(Decimal). |
| `simulator.rs` | `SimulatorFeature` phase machine (Idle/Running/Complete/Failed) + `EquityCurvePoint` (Decimal). |
| `system.rs` | `SystemFeature` with `SubsystemStatus` rows (market/risk/exec/ledger/agent), `set_subsystem`, `all_healthy`. |
| `watchlist.rs` | `WatchlistFeature` — folds `ClientEvent::Tick/Bar` into `WatchlistViewModel` (sparkline capped at 64 points). |
| `backtest.rs`, `portfolio.rs`, `algos.rs` | Thin feature-state wrappers over the `tantra-ui-core` view models. |

## Design notes

- Feature state models are complete and fully tested; the "Futuristic Neural AST Screener"
  (Matrix/Constellation/Tape/Heatmap) views from the old `architecture.md` do not exist in
  source — the screener is a plain filter/scan state model (see `docs/plans/0005` for the
  screen-inventory work that landed).
- `wasm32-unknown-unknown` builds cleanly (CI `wasm` job): `cargo check -p tantra-ui-app
  --target wasm32-unknown-unknown`.

## Tests

`tests/router_test.rs`, `screener_test.rs`, `studio_test.rs`, `simulator_test.rs`,
`system_test.rs`, `watchlist_test.rs`, `backtest_test.rs`, `portfolio_test.rs`, `algos_test.rs`
(47 `#[test]` functions).
