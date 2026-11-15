# Gaps — `tantra-ui`

Every gap filed in this repo, newest last. See
`.agents/rules/gap-tracking.md` for the binding rule.

## Open

| # | Title | Area |
| :--- | :--- | :--- |
| [0007](0007-chart-level-and-annotation-primitives.md) | Charts lack level-line / zone-band / marker / entry-SL-TSL line primitives (EE+ Strategy Studio) | `tantra-ui-charts` |

## Closed

| # | Title | Area | Resolution |
| :--- | :--- | :--- | :--- |
| 0001 | CLI `scan` was `NotWired`; wired to canonical screener | `tantra-cli` runner | Implemented via `crates/tantra-cli/src/scan.rs` + `runner.rs` (`CommandOutcome::ScanResults`); tests in `crates/tantra-cli/tests/unit_tests.rs`. Gap doc removed (implemented). |
| 0002 | Job `trades.json` wrote aggregates only; no per-trade rows | `tantra-ui-core` strategy | `BacktestReport.trades: Vec<TradeRow>` + `simulate_strategy` row emission; `jobs_runtime.rs` journals the row list; tests in `crates/tantra-ui-core/tests/strategy_primitives_test.rs`. Gap doc removed (implemented). |
| 0003 | CLI `memory`/`plugin`/`skills`/`mcp`/`doctor`/`purge`/`health` commands return canned string reports | `tantra-cli` runner | Implemented via real on-disk stores (`memory_store.rs` JSONL journal, `~/.tantra/{memory,plugins,skills,mcp}` dirs), real doctor env checks, purge real-path listing, factual health report (no stale "scan pending universe"); hermetic `*_with_home` variants. Tests in `tests/unit_tests.rs`. Gap doc removed (implemented, `19b8acd`). |
| 0004 | `tantra-ui-app` has feature-state models but no Leptos/CSR view layer | `tantra-ui-app` | Implemented via `src/web.rs` Leptos 0.8 CSR view layer (8 `AppRoute` components + pure `view_model` helpers + `mount_app`); WASM check green (pre-existing `tantra-ui-core` wasm `std::fs` break fixed via cfg-gated host helper); tests in `tests/web_view_test.rs`. Gap doc removed (implemented, `19b8acd`). |
| 0005 | TUI Watchlist tab renders hardcoded rows; Screener/Portfolio/Orders are placeholder panels | `tantra-tui` ui | Implemented via `src/live_data.rs` `LiveData` + pure `ClientEvent` fold, real `draw_watchlist`/`draw_screener`/`draw_portfolio`/`draw_orders` over `TuiApp.live` with honest empty-states and the `TuiApp::publish` feed seam; tests in `tests/live_data_test.rs`. Gap doc removed (implemented, `19b8acd`). |
| 0006 | CLI `scan` runs against an in-memory 4-scrip fixture universe instead of a live `GET /screener/scan` | `tantra-cli` scan | Implemented via `scan.rs` `--server` live path (`GET /screener/scan`, `ScreenerQuery`/`ScreenerResponse` wire types, `reqwest`, canonical filter serialization); fixture kept as offline fallback only; integration test boots an in-memory `tantra-runtime-api-rest` router on an ephemeral port in `tests/live_scan_test.rs`. Gap doc removed (implemented, `19b8acd`). |
