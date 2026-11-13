# `tantra-ui` — Implementation Record

Dismantled from the former `docs/roadmap/` (`roadmap.md`). Completed milestones are recorded
here; remaining future work moved to [`docs/gaps/`](../gaps/README.md) and
[`docs/plans/`](../plans/README.md).

## Build-out: `honba-kao` → `tantra-ui` (Tier 5A)

The former `docs/roadmap/roadmap.md` listed three milestones, all of which are **complete**.
The `honba-kao` source tree was unavailable during migration, so all nine crates were written
fresh against the contracts in `docs/migration-guide.md` and `docs/plans/0001`.

| Milestone | Status |
|-----------|--------|
| Milestone 1: Portable UI core, assets, client, charts & API-WASM (`tantra-ui-core`, `tantra-ui-assets`, `tantra-ui-client`, `tantra-ui-charts`, `tantra-ui-api-wasm`) | Complete — all five crates build for `wasm32-unknown-unknown`; tests in `crates/*/tests/` |
| Milestone 2: WASM workstation (`tantra-ui-app`) | Complete (feature-state models) — `AppRoute` router, `ScreenerFeature`, `StudioCanvas`, `SimulatorFeature`, `SystemFeature`, `WatchlistFeature`, `BacktestFeature`, `PortfolioFeature`, `AlgosFeature`; `cargo check -p tantra-ui-app --target wasm32-unknown-unknown` green. A Leptos/CSR view layer is a separate open gap (`docs/gaps/0004`). |
| Milestone 3: Native terminal workstation (`tantra-highlight`, `tantra-tui`, `tantra-cli`) | Complete — zero-I/O highlighter, 7-tab Ratatui TUI with live LLM streaming, and the 15-command `tantra` CLI binary |
| Zero inline `#[cfg(test)]` in `crates/*/src/` (all tests in `tests/`) | Complete |
| `cargo fmt --check` / `cargo clippy --all-targets -- -D warnings` / `cargo test --workspace` | Clean / clean / passing (276 `#[test]` functions) |
| `cargo check -p tantra-ui-app --target wasm32-unknown-unknown` | Green |

The file-by-file mapping is in [`code-migration-map.md`](code-migration-map.md).

## Subsequent landings

| Landing | Status |
|---------|--------|
| Plan 0001 — Build `tantra-cli` / `tantra-tui` / `tantra-highlight` from the `artha` design work | Complete — crate build-out; residual `scan` wiring closed via gap 0001; `Ask`/`Chat`/`Tui` wired to real LLM providers (Candle GGUF in-process / OpenAI) |
| Gap 0001 — CLI `scan` was `NotWired` | Closed — wired to the canonical `tantra_core::screener` engine via `tantra-ui-api-wasm::screener::apply_scan` (`crates/tantra-cli/src/scan.rs`, `CommandOutcome::ScanResults`) |
| Gap 0002 — Job `trades.json` wrote aggregates only | Closed — `BacktestReport.trades: Vec<TradeRow>` + per-trade row emission through `simulate_strategy` and `jobs_runtime.rs` |
| Plan 0004 — Full CLI command reference & interactive TUI slash-command palette | Complete — definitive reference: `docs/reference/cli-and-tui.md` |
| Plan 0005 — FinceptTerminal's screen inventory vs. tantra-ui's roadmap | Complete — Watchlist, Screener, Backtest, Algos, Portfolio features in `tantra-ui-app` + 7-tab TUI workstation |
| `tantra serve` added to `tantra-cli` | Complete — `Command::Serve` boots `tantra-runtime-api-rest::bootstrap::run_server_blocking` (`ServeConfig`), smoke-tested over real HTTP (`GET /profile`) |
| Docs: former `docs/roadmap/` dismantled into `design/` / `gaps/` / `plans/` | Complete (this change) |

## Verification (docs batch)

```bash
# Run from the repository root
grep -rn "#\[cfg(test)\]" crates/*/src/   # zero inline tests in production source
cargo fmt --check
cargo clippy --all-targets -- -D warnings
cargo test --workspace --profile prod    # canonical prod profile (make test)
cargo check -p tantra-ui-app --target wasm32-unknown-unknown
```

## Open future work

All open items live in [`docs/gaps/`](../gaps/README.md) (currently 0003–0006: canned CLI
reports, no view layer, TUI placeholder tabs, `scan` fixture universe) and
[`docs/plans/`](../plans/README.md) (0003 MCP gateway result rendering + client configs).
