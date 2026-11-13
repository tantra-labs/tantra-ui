# `tantra-ui` — DDD Architecture & Dual Workstation Bounded Context

## 1. Bounded Context & Responsibility

`tantra-ui` (Tier 5A, evolved from `honba-kao`; all nine crates written fresh because the
`honba-kao` source tree was unavailable — see `code-migration-map.md`) is the presentation layer
for the Tantra platform. It delivers three surfaces:

1. **Portable UI kernel** (`tantra-ui-core`, `tantra-ui-assets`, `tantra-ui-charts`,
   `tantra-ui-client`, `tantra-ui-api-wasm`) — design-system view models, embedded assets,
   Decimal chart geometry, in-memory event broadcast, and pure-Decimal compute bindings. All
   compile cleanly to `wasm32-unknown-unknown`.
2. **Browser workstation feature models** (`tantra-ui-app`) — WASM-safe feature-state models
   (router, screener, studio node graph, simulator, system, watchlist, backtest, portfolio,
   algos). **No Leptos/CSR view layer exists yet** (see `docs/gaps/0004`).
3. **Native terminal workstation** (`tantra-highlight`, `tantra-tui`, `tantra-cli`) — a
   zero-I/O syntax highlighter, a full Ratatui 7-tab TUI (`tantra tui`), and a 15-command `clap`
   binary (`tantra`) with one-shot ask, interactive chat, jobs, serve, verify, and strategy
   tooling.

WASM and native crates are strictly isolated: the six WASM crates carry zero `tokio` /
`reqwest` / `ratatui` / `crossterm` dependencies; the native crates (`tantra-{cli,tui}`) are the
only ones that pull the async stack.

## 2. Sub-Crate Partitioning (9 crates)

| Sub-Crate | Target | Responsibility |
| :--- | :--- | :--- |
| `tantra-ui-core` | Portable (`wasm32` + native) | Design-system atoms (`Badge`, `StatusPill`, `TrendPill`, `Delta`, `MetricValue`, `EquityPoint`, …), view models (screener, home/watchlist, portfolio, backtest, algos), and the strategy primitives kernel (`StrategyBridge`, `BacktestReport` + per-trade `TradeRow`, `OptimizationResult`, `simulate_strategy`, Sharpe/DSR/max-drawdown/risk-sizing/statutory-charges calculators). |
| `tantra-ui-assets` | Portable | `Branding`, `Theme`/`ThemeId` (dark/light/terminal), `FontFamily` stacks, deterministic `fixtures` (bars/ticks/portfolio/screener), descriptor-only `audio` profiles, embedded `COMPONENTS_CSS` + `FONTS_CSS` + font files. |
| `tantra-ui-charts` | Portable / WASM | Decimal **geometry** primitives (`Candle`, `CandlestickChart`, `ChartPoint`/`ChartSeries`/`LineChart`, `ChartRange`/`AxisTicks`, `ChartPalette`) — `y_for_price`/`x_for_candle` Decimal→pixel mapping. Rendering to canvas/ratatui happens in the consumer; there is no canvas engine and no depth module in this crate. |
| `tantra-ui-client` | Portable / WASM | In-memory, WASM-safe push broadcast: `EventStream` (subscribe/publish, bounded 4096-event log) + `Subscription` (drop-unsubscribe, per-subscriber drain cursor); `ClientEvent` = Tick/Bar/Order/Fill/Portfolio/Screener/System. **No REST/WebSocket transport** — it is a state bus, not an I/O client. |
| `tantra-ui-api-wasm` | Portable / WASM | Pure-`Decimal` compute facade: analytics (`sharpe_ratio`, `sortino_ratio`, `drawdown`), indicators (`sma`, `ema`, `rsi`), `margin_required`, `pair_series`, `payoff`/`payoff_net`/`payoff_curve`, pricing (`notional`, `pnl`, `mtm`, `break_even`), `replay_ticks` → `PositionView`, and the canonical `apply_scan` screener engine over `tantra_core::screener` wire contracts. |
| `tantra-ui-app` | Portable / WASM | Feature-state models: `Router`/`AppRoute` (8 routes), `ScreenerFeature`, `StudioCanvas` node graph (cycle detection), `SimulatorFeature`, `SystemFeature`, `WatchlistFeature`, `BacktestFeature`, `PortfolioFeature`, `AlgosFeature`. View layer pending (gap 0004). |
| `tantra-highlight` | Native / Portable | Zero-I/O hand-rolled lexer (`Language{Rust,Python,Json,Markdown,Shell,Plain}`, `HighlightClass`, `HighlightSpan`, `highlight_code`). **No tree-sitter**, zero runtime deps, WASM-safe. |
| `tantra-tui` | Native (`ratatui` + `crossterm`) | Full terminal workstation: `TuiApp`, 7-tab `ActiveTab{Watchlist,Screener,Portfolio,Orders,Backtest,Algos,AgentChat}`, 40-entry slash-command palette, keybinding dispatch, `ChatTranscript` w/ markdown highlight seam, `run_terminal` crossterm event loop wired to real LLM `chat_stream` providers. |
| `tantra-cli` | Native (`clap`, tokio) | `tantra` binary + lib: 15 `Command` variants (`Tui`, `Ask`, `Chat`, `Scan`, `Serve`, `Jobs`, `Verify`, `Memory`, `Plugin`, `Skills`, `Mcp`, `Doctor`, `Purge`, `Health`, `Strategy`), `run`/`run_with_provider` dispatch, `JobsRuntime` (add/list/run/tick/logs/approvals on `tantra-runtime`), `StrategyBridgeRunner` (agent tool-call journaling), natural-language `backtest_gen`. |

## 3. Layering & Portability Invariants

```
tantra-cli / tantra-tui        (native: tokio, ratatui, crossterm, clap, tantra-agent-*,
                                tantra-runtime-{jobs,api,api-rest} — binaries only)
        │
        └──────► tantra-highlight (zero-I/O, std only)
        │
        └──────► tantra-ui-core / tantra-ui-api-wasm / tantra-ui-charts
                 tantra-ui-assets / tantra-ui-client / tantra-ui-app
                 (portable: serde + rust_decimal + tantra-core only; wasm32-clean)
```

| Invariant | Where enforced |
| :--- | :--- |
| WASM crates carry no `tokio`/`reqwest`/`ratatui`/`crossterm` | `Cargo.toml` dependency split; CI `wasm` job runs `cargo check -p tantra-ui-app --target wasm32-unknown-unknown`. |
| Tests in `tests/` only | Zero `#[cfg(test)]` in any `crates/*/src/` (grep-verified in CI); 276 `#[test]` functions across all 9 crates. |
| Zero floats at the wire boundary | All public wire types are `rust_decimal::Decimal`. Known internal float seams: `tantra-ui-core::strategy` math internals (`norm_cdf`, variance `sqrt`, DSR, synthetic-bar generation, sparkline normalization) use `f64` inside — the same documented seam class the workspace allows for indicator/optimizer math. |
| Deterministic fixtures | `tantra-ui-assets::fixtures` and `simulate_strategy`'s `generate_market_bars` are anchored to explicit timestamps (no wall clock). |
| Canonical `prod` profile | `make prod` / `make check` / `make test` (`cargo build|check|test --workspace --profile prod`); `make clippy` runs `-D warnings`. |

## 4. Open Gaps & Plans (as of 2026-11-13)

- Open gaps: `docs/gaps/0003` (CLI `memory`/`plugin`/`skills`/`mcp`/`doctor`/`purge`/`health`
  return canned string reports), `0004` (`tantra-ui-app` has no Leptos/CSR view layer), `0005`
  (TUI Watchlist is hardcoded and Screener/Portfolio/Orders are placeholder panels), `0006`
  (CLI `scan` runs against an in-memory 4-scrip fixture universe, not a live
  `GET /screener/scan`).
- Active plans: `docs/plans/0002` (Quantcept web UX reference), `0003` (MCP gateway result-block
  rendering + client configs — ready for implementation).
- Implemented plans: `0001` (cli/tui/highlight build-out), `0004` (CLI command reference + TUI
  slash palette — see `docs/reference/cli-and-tui.md`), `0005` (FinceptTerminal screen inventory:
  Watchlist/Screener/Backtest/Algos/Portfolio features + 7-tab TUI).
- Closed gaps: 0001 (CLI `scan` wiring), 0002 (per-trade journal rows). Completed milestones and
  landings are recorded in `implementation-record.md` (the dismantled `docs/roadmap/`).

## 5. Per-Crate Design Docs

| Doc | Crate |
| :--- | :--- |
| `code-ui-core.md` | `tantra-ui-core` |
| `code-ui-assets.md` | `tantra-ui-assets` |
| `code-ui-charts.md` | `tantra-ui-charts` |
| `code-ui-client.md` | `tantra-ui-client` |
| `code-ui-api-wasm.md` | `tantra-ui-api-wasm` |
| `code-ui-app.md` | `tantra-ui-app` |
| `code-highlight.md` | `tantra-highlight` |
| `code-tui.md` | `tantra-tui` |
| `code-cli.md` | `tantra-cli` |
| `code-migration-map.md` | Historical honba-kao → tantra-ui migration record |
| `implementation-record.md` | Dismantled `docs/roadmap/` — completed milestones & landings |
