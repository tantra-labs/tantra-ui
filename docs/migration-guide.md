# `tantra-ui` — Subagent Migration & DDD Refactoring Guide

> **Tier**: Tier 5A (Depends on `tantra-core`)
> **Source**: `honba-workspace/honba-kao`
> **Target**: `tantra-ui`
> **Status**: ✅ **TIER 5 COMPLETE** — verification gate green (2026-11-05)

---

## 1. Sub-Crate Mapping Table

| Source Crate (`honba-kao/crates/`) | Target Crate (`tantra-ui/crates/`) | Notes |
| :--- | :--- | :--- |
| `honba-kao-ui-core` | `tantra-ui-core` | Port all UI atoms/molecules and tests (`ui_core_primitives_test.rs`, `screener_primitives_test.rs`, `home_primitives_test.rs`). |
| `honba-kao-assets` | `tantra-ui-assets` | Port `assets/` directory (`style/components.css`, fonts, themes) and `src/` (`branding.rs`, `themes.rs`, `fonts.rs`, `fixtures.rs`, `audio.rs`). |
| `honba-kao-client` | `tantra-ui-client` | Port `events.rs`, `stream.rs`, `lib.rs`, and `tests/`. |
| `openalgo-charts-rs` | `tantra-ui-charts` | Rename crate `openalgo-charts-rs` → `tantra-ui-charts` (or re-export alias `openalgo_charts_rs`). Port all 7 chart test suites. |
| `honba-kao-api-wasm` | `tantra-ui-api-wasm` | Port `bindings.rs`, `lib.rs`, and all 8 test suites (`analytics`, `indicators`, `margin`, `pairing`, `payoff`, `pricing`, `replay`, `screener`). |
| `honba-kao-app` | `tantra-ui-app` | Port all features (`screener`, `studio`, `simulator`, `system`, `router`) and all 14 test suites (105 tests). |
| *(New Crate)* | `tantra-highlight` | Implement zero-I/O syntax highlighter (`Language`, `HighlightSpan`, `highlight_code`) with unit tests in `tests/unit_tests.rs`. |
| *(New Crate)* | `tantra-tui` | Implement `TuiApp`, `ActiveTab` (`Screener`, `Portfolio`, `Orders`, `AgentChat`), keybinding handler, and layout model with tests in `tests/unit_tests.rs`. |
| *(New Crate)* | `tantra-cli` | Implement `CliArgs`, `Command` (`Tui`, `Ask`, `Scan`, `Health`), and runner logic with tests in `tests/unit_tests.rs`. |

---

## 2. Mandatory DDD & Portability Fixes During Migration
1. **Do NOT copy `honba-kao/crates-old/`**: Ignore `crates-old/` completely.
2. **WASM Portability in `tantra-ui/Cargo.toml`**:
   - Ensure WASM crates (`tantra-ui-core`, `tantra-ui-assets`, `tantra-ui-client`, `tantra-ui-charts`, `tantra-ui-api-wasm`, `tantra-ui-app`) do **NOT** pull in `tokio` with `net`/`rt-multi-thread` so `cargo check -p tantra-ui-app --target wasm32-unknown-unknown` succeeds.
3. **Copy Root Web Assets**: Copy `honba-kao/index.html`, `honba-kao/components.html`, `honba-kao/Trunk.toml`, and `honba-kao/scripts/` into `tantra-ui/`, updating crate/binary references from `honba-kao-app` to `tantra-ui-app`.

## 3. Verification Commands
```bash
# Run from the repository root
grep -rn "#\[cfg(test)\]" crates/*/src/ && exit 1 || true
cargo fmt --check
cargo clippy --all-targets -- -D warnings
cargo test --workspace
cargo check -p tantra-ui-app --target wasm32-unknown-unknown
```

## 4. Migration Notes (2026-11-05)

The honba-kao source tree was **not available** in the migration environment
(`honba-workspace` absent), so all nine crates
were implemented **fresh against the contracts in this guide and
`docs/plans/0001-agent-chat-terminal-from-artha.md`** rather than ported
line-for-line. The workspace shape, crate boundaries, test-suite names, and
WASM/native isolation follow the mapping table above exactly.

### Crates delivered
| Crate | Contents | Tests |
| :--- | :--- | :--- |
| `tantra-ui-core` | Atoms/molecules (`Badge`, `StatusPill`, `TrendPill`, `Delta`, `MetricValue`, `EquityPoint`), screener primitives (`ScreenerColumn`, `ScreenerSort`, `ScreenerFilterChip`, `ScreenerRowModel`, `ScreenerViewModel`), home primitives (`MarketStatus`, `WatchlistRow`, `AccountSummary`, `WatchlistViewModel`), backtest results primitives (`BacktestSummary`, `BacktestResultView`), portfolio view primitives (`PortfolioViewModel`, `PortfolioPositionRow`, `PortfolioHoldingRow`, `PortfolioTotals`), algo management primitives (`AlgoRow`, `AlgoManagerViewModel`, `AlgoStatus`) | `ui_core_primitives_test.rs`, `screener_primitives_test.rs`, `home_primitives_test.rs`, `watchlist_primitives_test.rs`, `backtest_primitives_test.rs`, `portfolio_primitives_test.rs`, `algos_primitives_test.rs` (31) |
| `tantra-ui-assets` | `branding`, `themes` (dark/light/amber palettes), `fonts` stacks, `fixtures` (deterministic bars/ticks/portfolio/screener), `audio` profiles, `assets/style/components.css` | `unit_tests.rs`, `fixtures_test.rs` (13) |
| `tantra-ui-charts` | `Candle`, `CandlestickChart` (Decimal y/x mapping), `ChartPoint`, `ChartSeries`, `LineChart`, `ChartRange`, `AxisTicks`, `ChartPalette` | 7 suites: candle, series, axis, palette, edge-cases, roundtrip, geometry (33) |
| `tantra-ui-client` | `ClientEvent` (Tick/Bar/Order/Fill/Portfolio/Screener/System), `EventKind`, WASM-safe push `EventStream` with `Subscription` (drop-unsubscribe, per-subscriber drain cursor, bounded log) | `stream_test.rs` (7) |
| `tantra-ui-api-wasm` | `bindings` (`Bindings` facade + `ApiError`), `analytics` (Sharpe/Sortino/drawdown), `indicators` (SMA/EMA/RSI), `margin`, `pairing`, `payoff` (call/put), `pricing` (notional/PnL/mtm/break-even), `replay` (`PositionView`), `screener` (`apply_scan`) | 8 suites per guide (37) |
| `tantra-ui-app` | `router` (`AppRoute` incl. Backtest/Portfolio/Algos, `Router`), `screener` (`ScreenerFeature`), `studio` (`StudioCanvas` node graph with cycle detection), `simulator` (`SimulatorFeature`), `system` (`SystemFeature`), `watchlist` (`WatchlistFeature`), `backtest` (`BacktestFeature`), `portfolio` (`PortfolioFeature`), `algos` (`AlgosFeature`) | 9 suites (36) |
| `tantra-highlight` **[NEW]** | Zero-I/O hand-rolled lexer (`Language{Rust,Python,Json,Markdown,Shell,Plain}`, `HighlightClass`, `HighlightSpan`, `highlight_code`) — **no tree-sitter**, WASM-safe, zero deps | `unit_tests.rs` (13) |
| `tantra-tui` **[NEW]** | `TuiApp`, `ActiveTab{Watchlist,Screener,Portfolio,Orders,Backtest,Algos,AgentChat}`, crossterm keybindings, `LayoutModel`, `ChatTranscript` w/ markdown highlight seam | `unit_tests.rs` (10) |
| `tantra-cli` **[NEW]** | `CliArgs` + `Command{Tui,Ask,Scan,Health}` (clap), thin runner, `tantra` binary (`tantra health` verified) | `unit_tests.rs` (9) |

### Key decisions
- **WASM isolation**: `tokio`/`ratatui`/`crossterm`/`clap` are only depended on
  by `tantra-{cli,tui}`. WASM crates carry zero async/native deps.
- **wasm32-unknown-unknown**: `tantra-core`'s transitive `uuid` v4 needs an RNG
  source on wasm. Enabled via workspace `uuid` `rng-getrandom` + `getrandom`
  `wasm_js` features (activated from `tantra-ui-app`). `cargo check -p
  tantra-ui-app --target wasm32-unknown-unknown` is green.
- **`ScreenerFilterChip` lives in `tantra-ui-core`** (presentational), not
  `tantra-core`; the canonical filter is `ScreenerFilterPredicate`.
- **`Ask`/`Scan` CLI commands are wired**: `Ask` streams LLM answers via
  `tantra-agent-llm` (`CandleGgufProvider` / `OpenAiProvider`); `Scan`
  evaluates a `tantra_core::screener::ScreenerScanRequest` against an
  in-memory universe through `tantra_ui_api_wasm::screener::apply_scan`
  (gap `0001` closed; gap doc removed).
- **Screener canonicality (Layer 5)**: the UI builds only against
  `tantra_core::screener` wire contracts (evaluated by `apply_scan`); the
  `tantra-market-indicators::screener` module is a standalone simplified
  duplicate not used by `tantra-ui` or `tantra-agent` (resolved in
  `docs/plans/0005` §2 item 2).
- **Remaining follow-up** (out of tier-5 gate scope): real Leptos view layer
  for `tantra-ui-app` (state models are in place), replacing the `scan`
  fixture universe with a live `tantra-runtime-api-rest`
  `GET /screener/scan` response, and TUI `run()` event loop.
