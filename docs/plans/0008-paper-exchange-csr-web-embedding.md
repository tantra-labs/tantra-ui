# 0008 — Paper Exchange CSR / WASM + Web Embedding

**Status**: Implemented (T14 P3).
**Track**: T14 (P3) · **Phase**: 6 (UI / workstations)
**Owning repo**: `tantra-ui` · **Master plan**: root `docs/plans/0007-*.md`

## Why

`tantra-ui` already renders `PortfolioMode::Paper` fixtures (TUI "Paper Trading Positions"
panel, CLI `--mode paper`, app/test fixtures) and ships pure WASM-safe compute bindings
(`tantra-ui-api-wasm`: analytics, indicators, margin, replay, screener). But every UI paper
surface today is either a **static fixture** or a thin client of REST — there is no live
in-browser paper session, no order ticket against a real matching engine, and no
bidirectional order/fill/depth push channel. `tantra-runtime/docs/plans/0005` (D3) adds
`/ws/live`; this plan adds the UI half: a CSR WASM embedding of the *same*
`tantra-exec-paper` engine (single-user, in-browser) and the web/TUI surfaces that drive the
server-backed paper exchange.

## Deliverables (in `tantra-ui`)

### D1 — WASM bindings for the exchange core (`tantra-ui-api-wasm`)

Add `paper.rs` to `tantra-ui-api-wasm` (pure, zero-I/O, Decimal, `wasm32`-targetable —
mirrors the existing `replay.rs` pattern):

- `PaperExchange` type (thin `wasm_bindgen` shim over the pure matching core, or a
  re-export of `tantra-exec-paper` where the engine itself is WASM-portable);
- `place_order` / `cancel_order` / `feed_bar` / `feed_tick` / `depth` / `fills` /
  `positions` / `equity`;
- Deterministic single-user session: one browser tab = one isolated paper account;
  in-browser latency/slippage seeding keeps fills byte-identical to the server engine.

The WASM crate stays dependency-light: only the pure engine + `tantra-core` types, no `tokio`,
no `std::fs`, no threads (AGENTS WASM invariant).

### D2 — Leptos 0.8 CSR paper-trading panel (`tantra-ui-app`)

New `PaperPanel` route/view in `tantra-ui-app/src/web.rs`:

- Order ticket (side / quantity / limit or market / TIF), live working-orders table,
  fills feed, per-instrument depth ladder, positions + equity.
- Two modes:
  - **Embedded (CSR)**: the in-browser `PaperExchange` session above — zero server, works
    offline, demo/example mode.
  - **Server (web)**: connects to `tantra-runtime-api-rest` REST `/orders` +
    `/ws/live` (paper mode) for a shared/remote session.
- Feature-state driven like the existing views (pure `view_model` helpers), so tests stay
  unit-level.

### D3 — TUI paper panel parity (`tantra-tui`)

Upgrade the existing "Paper Trading Positions" panel from fixture data to the `/ws/live`
stream (orders/fills/depth/positions) when connected to a paper server; keep the fixture
fallback for offline use. Add working-orders + depth sub-panels.

### D4 — CLI paper session (`tantra-cli`)

`tantra paper --mode embedded|server`:
- `--embedded`: boots an in-browser/CLI embedded `PaperExchange` session, places/cancels
  orders, prints fills + equity (byte-identical to the server engine);
- `--server`: talks to `paper serve` REST + WS (no credentials).
Replaces the current `--mode paper` fixture path (`cli/src/args.rs`).

## TDD / test slices (all in `tests/`, parallel to `src/`)

| # | Red test (failing first) | Green deliverable |
|---|--------------------------|-------------------|
| U1 | `tantra-ui-api-wasm/tests/paper_test.rs`: place limit buy → feed bar → fill, exact Decimal | WASM `PaperExchange` binding |
| U2 | Same tape → WASM fills == Rust native fills == Python fills (golden vector) | Cross-embedding parity |
| U3 | `PaperPanel` view-model renders an order ticket + fills from a given state | CSR view layer |
| U4 | `tantra-tui` paper panel renders a `/ws/live`-shaped event batch | TUI stream integration |
| U5 | `tantra-cli paper --embedded` run prints deterministic fills | CLI session |

## Verification

```bash
cd tantra-ui && cargo check --workspace --profile prod
cargo check -p tantra-ui-api-wasm --target wasm32-unknown-unknown --profile prod
cargo test --workspace --profile prod
cargo clippy --all-targets -- -D warnings && cargo fmt --check
```

## Implemented (2026-11-16, T14 P3)

All five TDD slices (U1–U5) shipped and verified green on the full workspace
gate (`cargo check --workspace --profile prod`, wasm32 check,
`cargo test --workspace --profile prod`, `cargo clippy --all-targets -- -D
warnings`, `cargo fmt --check`, `make prod`).

### D1 — WASM bindings for the exchange core (`tantra-ui-api-wasm`) — U1/U2 ✅

- `src/paper.rs`: `PaperSession` — a thin, deterministic single-user wrapper
  over the owned `tantra_exec_paper::PaperExchange` (zero-I/O, Decimal-only,
  `wasm32`-targetable; mirrors `replay.rs`, no `wasm_bindgen` in-crate).
  `PaperSession::new(cash, seed)` (seed → seeded `NetworkLatency`, None →
  zero-latency) with `submit` / `cancel_order` / `feed_bar` / `feed_tick` /
  `advance_to` / `depth` / `fills` / `events` / `positions` / `cash` /
  `equity` / `order_state` / `order_avg_fill_price`; re-exports the engine
  error/event/state types.
- `Cargo.toml`: `tantra-exec-paper.workspace = true` (+ `chrono`), the wasm32
  `getrandom` target-deps block copied from `tantra-exec-paper`, and
  `serde_json` / `rust_decimal_macros` / `uuid` dev-deps. Workspace
  `Cargo.toml` gained `tantra-exec-paper` and `rust_decimal_macros`.
- `tests/paper_test.rs` (U1): limit buy fills exactly when a bar crosses its
  limit (fill 100 @ 90, cash 991 000, position +100, equity back to 1 000 000);
  empty-state + `NoMarketPrice` rejection.
- `tests/golden/paper_golden.json` + `tests/golden_parity_test.rs` (U2): the
  E9 golden tape (2 buy limits 100@101/100@100 + market sell 150) — 4 fills
  (sell 100@101, buy 100@101, sell 50@100, buy 50@100), sell VWAP 100.6667,
  net 0, cash 1 000 000, 7 events; P4 cross-validates Python ↔ WASM against
  this vector. Determinism asserted (two fresh sessions byte-identical).
- wasm32 check (`cargo check -p tantra-ui-api-wasm --target
  wasm32-unknown-unknown --profile prod`) green.

### D2 — Leptos 0.8 CSR paper-trading panel (`tantra-ui-app`) — U3 ✅

- `src/router.rs`: `AppRoute::Paper` (label "Paper", path "/paper"), `ALL` → 9.
- `src/paper.rs`: `PaperFeature` (embedded `PaperSession` demo + server mode
  flag), ticket/fills/depth/positions rows, and the pure `view_model`
  helpers `paper_order_ticket` / `paper_working_orders` / `paper_fills` /
  `paper_depth` / `paper_positions_equity`. `run_embedded_demo()` drives the
  real in-browser engine (buy 100@95, bar @90 → fill 100@90; resting buy
  50@85; depth bid 90 / ask 88).
- `src/web.rs`: `PaperView` (thin component over the helpers, "Run demo tape"
  button) + the `AppRoute::Paper` match arm; the nav button is rendered from
  `AppRoute::ALL` automatically.
- `tests/paper_view_test.rs` (U3) + updated `router_test.rs` /
  `web_view_test.rs` (9 routes, `back()` lands on `Algos`).

### D3 — TUI paper panel parity (`tantra-tui`) — U4 ✅

- `src/live_data.rs`: paper projection over `ClientEvent`s — working orders
  (`PaperOrderRow`, deduped + enriched by fills), fills feed
  (`PaperFillRow`), per-instrument depth ladder (`PaperDepthRow` from
  Quote/Depth ticks), positions + cash/equity from `PortfolioSnapshot`.
  `paper_panel_text(&LiveData)` renders the live panel, falling back to the
  previous hardcoded fixture (`PAPER_PANEL_FIXTURE`) when no events arrived.
- `src/app.rs`: `/positions` handler now renders `paper_panel_text(&self.live)`.
- `tests/paper_panel_test.rs` (U4): a `/ws/live`-shaped event batch renders
  working orders / fills / depth / positions; offline fallback + slash-command
  coverage.

### D4 — CLI paper session (`tantra-cli`) — U5 ✅

- `src/args.rs`: `CliCommand::Paper { mode, cash, seed, server }` +
  `PaperMode` clap `ValueEnum` (embedded | server) + `Command::Paper(PaperArgs)`
  + the `From<Cli>` conversion arm. `Ask --mode paper` untouched.
- `src/paper.rs`: `run_embedded_report(cash, seed)` (golden tape through the
  in-process `PaperExchange`, prints fills + sell VWAP + net position + cash +
  equity; seeded latency settles via `advance_to`) and
  `run_server_instructions(server)` (REST `/orders` + `/ws/live` thin client).
- `src/runner.rs` + `src/main.rs`: `Command::Paper` dispatch →
  `CommandOutcome::PaperSuccess`.
- `tests/paper_test.rs` (U5): deterministic fill lines; two identical runs
  byte-identical; custom cash; server instructions.

## Deviations recorded

- The in-browser binding ships as a plain wrapper (`PaperSession`) with no
  `wasm_bindgen` annotation in-crate — the JS bindgen shim stays outside the
  crate (the established `replay.rs` pattern), so the same code runs in native
  tests and WASM with zero host divergence.
- The D2 **server** mode is a feature-mode flag + the shared
  `ClientEvent` projection (same fold the TUI consumes in `tantra-tui`); the
  web component keeps network I/O out of the WASM crate, matching the
  workstation invariant that `/ws/live` consumers are native hosts.
- The golden-tape fill reading is the E9-verified **4 fills / net 0 /
  cash 1 000 000** (one `FillReport` per side per participant fill), not the
  older buy-side-only "3 fills / +150 / −15100" projection — see
  `tantra-runtime` plan 0005 D4 / `paper_parity_tests.rs`.
- `tantra-cli` `Ask --mode paper` and the TUI's other tabs are unchanged.

## Cross-repo dependencies

- **Requires** `tantra-exec/docs/plans/0003` (the pure matching core; WASM portability of
  `tantra-exec-paper` is a P0 acceptance item — E8). ✅ shipped (P0).
- **Requires** `tantra-runtime/docs/plans/0005` D3 (`/ws/live`) for server mode.
- The paper-trading user guide is root plan `0007` task T14.15 (coordinated with
  `tantra-docs` at execution time).
