# `tantra-ui-core` — Design System View Models & Strategy Primitives

Portable (`wasm32` + native). Zero OS/Tokio deps; deps are `tantra-core`, `serde`,
`rust_decimal`, `chrono`, `toml`.

## Modules

| Module | Contents |
| :--- | :--- |
| `atoms.rs` | Presentational atoms: `Badge`/`BadgeTone`, `StatusPill`, `TrendPill`/`TrendDirection`, `Delta`, `MetricValue`, `EquityPoint`, `Accent`, `Spacing`, `TextAlign`, `Tag`. |
| `screener.rs` | `ScreenerColumn`/`ColumnAlign`, `ScreenerSort`/`ScreenerSortOrder`, `ScreenerFilterChip`, `ScreenerRowModel`, `ScreenerViewModel`. |
| `home.rs` | `MarketStatus`/`MarketStatusKind`, `WatchlistRow`, `AccountSummary`, `WatchlistViewModel`. |
| `backtest.rs` | `BacktestStatus`, `BacktestSummary`, `BacktestResultView`. |
| `portfolio.rs` | `PortfolioPositionRow`, `PortfolioHoldingRow`, `PortfolioTotals`, `PortfolioViewModel`. |
| `algos.rs` | `AlgoStatus`, `AlgoRow`, `AlgoManagerViewModel`. |
| `strategy.rs` | The strategy primitives kernel (see below). |

## Strategy primitives kernel (`strategy.rs`)

The heaviest module and the one wired into `tantra-cli`/`tantra-tui` and the `tantra-agent`
chat tools:

- `StrategyBridge` — public entry point with `run_backtest` (120 deterministic synthetic bars via
  `generate_market_bars`), `run_backtest_with_curve` (also returns a normalized 0..100
  sparkline), `tune_strategy` (grid sweep ≤100 trials + DSR + optional six-gate `ValidationBlock`),
  and `generate_trade_suggestion`.
- `BacktestReport { …, trades: Vec<TradeRow> }` — per-trade rows (`entry_ts`, `exit_ts`,
  `symbol`, `side`, `entry_price`, `exit_price`, `quantity`, `fees`, `pnl`), all `Decimal`,
  deterministic timestamps anchored to an explicit `base_ts` (closed gap 0002: jobs journals the
  row list, not just an aggregate count).
- `TradeRow`/`TradeBias`/`TradeSignal`/`TradeSuggestion`, `OptimizationResult`, `RiskSizing`,
  `StatutoryCharges`, `StrategyDescriptor`/`StrategyParameterSpec`/`StrategySourceKind`.
- Calculators: `calculate_sharpe`, `calculate_max_drawdown`, `calculate_deflated_sharpe`,
  `calculate_risk_sizing`, `calculate_statutory_charges`; `MarketBar` (line ~1438).
- The private `simulate_strategy` kernel emits the per-trade rows.

**Zero-floats seam (documented)**: public wire types are `Decimal`, but the statistical math
internals use `f64` in ~22 places — `norm_cdf`, variance `sqrt`, DSR, `generate_market_bars`
(sin/noise), and sparkline normalization. This matches the workspace's documented
indicator/optimizer seam class; the wire boundary stays exact.

## Tests

`tests/ui_core_primitives_test.rs`, `screener_primitives_test.rs`, `home_primitives_test.rs`,
`watchlist_primitives_test.rs`, `backtest_primitives_test.rs`, `portfolio_primitives_test.rs`,
`algos_primitives_test.rs`, `strategy_primitives_test.rs` (44 `#[test]` functions).
