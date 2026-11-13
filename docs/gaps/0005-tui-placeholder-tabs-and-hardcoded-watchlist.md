# 0005 — TUI Watchlist renders hardcoded rows; Screener/Portfolio/Orders are placeholder panels

**Status**: open

## What's stubbed or hardcoded

`crates/tantra-tui/src/ui.rs` renders the seven-tab workstation, but four of the seven tabs have
no live data backing:

- **Watchlist tab**: renders a hardcoded set of static rows (no market-data feed or
  `tantra-runtime-api-rest` connection).
- **Screener / Portfolio / Orders tabs**: rendered as placeholder panels (no `apply_scan`
  results, no OMS/portfolio data, no order blotter).
- The functional tabs are Backtest (KPI quad + param-delta table + sparkline + suggestion card),
  Algos (table + parameter inspector), and AgentChat (live LLM streaming).

## Why

The TUI was built to the terminal-workstation contract of `docs/plans/0001` (crate build-out +
AgentChat). The market/portfolio/orders data surfaces were explicitly scoped as "driven by
`tantra-market`/`tantra-ledger`" follow-up work — those follow-ups were never wired into the
TUI because the TUI predates a live data source it could subscribe to.

## How to close it

- Wire the Watchlist tab to a live source: either `tantra_ui_client::EventStream` (the in-memory
  push bus, fed by a real market-data consumer) or a `tantra-runtime-api-rest` `GET /quotes`
  subscription.
- Render the Screener tab from `ScreenerScanRequest` + `tantra_ui_api_wasm::screener::apply_scan`
  results (the same path `tantra-cli scan` uses).
- Render Portfolio/Orders tabs from `GET /portfolio/*` and `GET /orders` responses (via a
  `tantra-runtime-api-rest` client) or from `tantra_ui_client::EventStream` `ClientEvent` folds.
- Keep rendering pure: present data passed into `draw`; do not fetch inside the render path.
