# Gaps — `tantra-ui`

Every gap filed in this repo, newest last. See
`.agents/rules/gap-tracking.md` for the binding rule.

## Open

| # | Title | Area |
| :--- | :--- | :--- |
| — | — | — |

## Closed

| # | Title | Area | Resolution |
| :--- | :--- | :--- | :--- |
| 0001 | CLI `scan` was `NotWired`; wired to canonical screener | `tantra-cli` runner | Implemented via `crates/tantra-cli/src/scan.rs` + `runner.rs` (`CommandOutcome::ScanResults`); tests in `crates/tantra-cli/tests/unit_tests.rs`. Gap doc removed (implemented). |
| 0002 | Job `trades.json` wrote aggregates only; no per-trade rows | `tantra-ui-core` strategy | `BacktestReport.trades: Vec<TradeRow>` + `simulate_strategy` row emission; `jobs_runtime.rs` journals the row list; tests in `crates/tantra-ui-core/tests/strategy_primitives_test.rs`. Gap doc removed (implemented). |
