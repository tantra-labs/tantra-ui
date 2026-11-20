//! # व्यापार・face tantra-ui-core
//!
//! Presentational primitives for the Tantra workstation: pure layout and
//! rendering atoms/molecules, screener table primitives, home dashboard
//! primitives, and strategy optimization / trade suggestion models.
//!
//! Contracts (migration-guide.md §1 and `tantra-ui/.agents/AGENTS.md`):
//! - **Presentational purity**: atoms describe *layout/rendering* intent;
//!   reactive state signals live in the feature crates (`tantra-ui-app`).
//! - **WASM-safe**: zero-I/O, no `tokio`, compiles for `wasm32-unknown-unknown`.
//! - **Zero-floats**: prices, deltas, and ratios use `rust_decimal::Decimal`
//!   (re-exported through `tantra_core`).
//! - **Tests in `tests/`**: `ui_core_primitives_test.rs`,
//!   `screener_primitives_test.rs`, `home_primitives_test.rs`, `strategy_primitives_test.rs`.

pub mod algos;
pub mod atoms;
pub mod backtest;
pub mod ee;
pub mod home;
pub mod portfolio;
pub mod scene_projector;
pub mod screener;
pub mod strategy;

pub use algos::{AlgoManagerViewModel, AlgoRow, AlgoStatus};
pub use atoms::{
    Accent, Badge, BadgeTone, Delta, EquityPoint, MetricValue, Spacing, StatusPill, Tag, TextAlign,
    TrendDirection, TrendPill,
};
pub use backtest::{BacktestResultView, BacktestStatus, BacktestSummary};
pub use ee::{
    EE_STOP_LABELS, StudioViewport, ee_level_report_rows, ee_seed_last_price, ee_seed_level_map,
    ee_seed_positions, ee_seed_signals, ee_signal_report_rows, studio_ee_bands, studio_ee_lines,
    studio_ee_marks, studio_ee_positions, studio_ee_regime, studio_ee_stops,
};
pub use home::{AccountSummary, MarketStatus, MarketStatusKind, WatchlistRow, WatchlistViewModel};
pub use portfolio::{
    PortfolioHoldingRow, PortfolioPositionRow, PortfolioTotals, PortfolioViewModel,
};
pub use scene_projector::{
    CanvasCandleGlyph, CanvasGlyph, CanvasGlyphKind, CanvasRectBand, CanvasRightAxisTick,
    CanvasSegment, CanvasSeriesTrace, CanvasStrokeKind, CanvasTone, HedgeCardView, HedgeLegView,
    HintInspectorItem, LIVE_RUNTIME_API_SOURCE, LayerPillView, LoopCandle, LoopDeltaSummary,
    OFFLINE_SEEDED_FIXTURE_SOURCE, PenetrationBucketRow, PenetrationHistogramView,
    ProjectedSceneCanvas, StopPolicyTableRow, StrategyLoopConfig, StrategyLoopViewModel,
    SweepLeaderboardRow, build_layer_pills, build_penetration_histogram,
    evaluate_offline_strategy_loop, format_hedge_card_ascii, format_signed_r,
    format_stop_comparison_ascii, format_strategy_loop_cli_report, format_sweep_leaderboard_ascii,
    infer_asset_class, normalize_sparkline_bins, project_overlay_scene,
    render_penetration_histogram_ascii,
};
pub use screener::{
    ScreenerColumn, ScreenerFilterChip, ScreenerRowModel, ScreenerSort, ScreenerSortOrder,
    ScreenerViewModel,
};
pub use strategy::{
    BacktestReport, OptimizationResult, RiskSizing, StatutoryCharges, StrategyDescriptor,
    StrategyParameterSpec, StrategySourceKind, TradeBias, TradeRow, TradeSignal, TradeSuggestion,
    calculate_deflated_sharpe, calculate_max_drawdown, calculate_risk_sizing, calculate_sharpe,
    calculate_statutory_charges,
};
