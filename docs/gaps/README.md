# Gaps — `tantra-ui`

Every gap filed in this repo, newest last. See
`.agents/rules/gap-tracking.md` for the binding rule.

## Open

| # | Title | Area |
| :--- | :--- | :--- |
| [0003](0003-cli-canned-command-reports.md) | CLI `memory`/`plugin`/`skills`/`mcp`/`doctor`/`purge`/`health` commands return canned string reports | `tantra-cli` runner |
| [0004](0004-ui-app-has-no-view-layer.md) | `tantra-ui-app` has feature-state models but no Leptos/CSR view layer | `tantra-ui-app` |
| [0005](0005-tui-placeholder-tabs-and-hardcoded-watchlist.md) | TUI Watchlist tab renders hardcoded rows; Screener/Portfolio/Orders are placeholder panels | `tantra-tui` ui |
| [0006](0006-cli-scan-fixture-universe.md) | CLI `scan` runs against an in-memory 4-scrip fixture universe instead of a live `GET /screener/scan` | `tantra-cli` scan |

## Closed

| # | Title | Area | Resolution |
| :--- | :--- | :--- | :--- |
| 0001 | CLI `scan` was `NotWired`; wired to canonical screener | `tantra-cli` runner | Implemented via `crates/tantra-cli/src/scan.rs` + `runner.rs` (`CommandOutcome::ScanResults`); tests in `crates/tantra-cli/tests/unit_tests.rs`. Gap doc removed (implemented). |
| 0002 | Job `trades.json` wrote aggregates only; no per-trade rows | `tantra-ui-core` strategy | `BacktestReport.trades: Vec<TradeRow>` + `simulate_strategy` row emission; `jobs_runtime.rs` journals the row list; tests in `crates/tantra-ui-core/tests/strategy_primitives_test.rs`. Gap doc removed (implemented). |
