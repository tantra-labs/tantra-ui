# 0006 — CLI `scan` runs against an in-memory fixture universe instead of a live `GET /screener/scan`

**Status**: open

## What's stubbed or hardcoded

`crates/tantra-cli/src/scan.rs` (`parse_query`, `run_scan`, `scan_universe`) correctly parses
canonical `tantra_core::screener::ScreenerScanRequest` queries and evaluates them via
`tantra_ui_api_wasm::screener::apply_scan` — but the **universe** it scans is a hardcoded
in-memory fixture of four NSE scrips (RELIANCE, TCS, INFY, HDFCBANK) with sample `Decimal`
rows. There is no live `tantra-runtime-api-rest` `GET /screener/scan` round-trip, no
`tantra-market` instrument master, and no `tantra-data` lakehouse behind it.

## Why

The original CLI `scan` gap (0001) was closed by wiring the command to the *canonical screener
engine* (correct parse → `apply_scan` evaluation) with a fixture universe, because no live
screener endpoint or instrument data source was reachable from the CLI at the time. This gap is
the remaining half of that closure: real data.

## How to close it

- Add a live scan path to `run_scan` that calls `tantra-runtime-api-rest`'s `GET /screener/scan`
  when a server is reachable (the `tantra serve` command already boots that server), falling back
  to the fixture universe only when no endpoint is configured.
- Or wire the CLI to a `tantra-data`/`tantra-market` instrument universe reader once one is
  exposed to native consumers.
- Add an integration test in `crates/tantra-cli/tests/` covering the live-endpoint path against a
  booted test server.
