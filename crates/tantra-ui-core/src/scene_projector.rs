//! Pure `Decimal`, zero-I/O scene projector & strategy-loop view models (`Track T21.4`).
//!
//! Converts canonical [`OverlayScene`] primitives into:
//! - [`ProjectedSceneCanvas`] (TUI `ratatui::widgets::canvas::Canvas` + `Sparkline` geometry),
//! - 4-policy Stop-Hunt Comparison tables ([`StopPolicyTableRow`]),
//! - IS/OOS + DSR Parameter Sweep Leaderboards ([`SweepLeaderboardRow`]),
//! - Day/Weekly/Monthly/Commodity Expiry Hedge Cards ([`HedgeCardView`]),
//! - Empirical Penetration Histograms ([`PenetrationHistogramView`]),
//! - Cursor-steppable Hint Inspector cards ([`HintInspectorItem`]),
//! - Reproducible ASCII / CLI tables & reports,
//! - Deterministic multi-asset, multi-strategy seeded evaluators for offline fallback
//!   (explicitly labeled [`OFFLINE_SEEDED_FIXTURE_SOURCE`]).
//!
//! Contracts:
//! - Strictly WASM-clean (no `tokio`, no `ratatui`, no I/O).
//! - Strictly zero floats (`f32`/`f64`) anywhere in types or arithmetic.

use rust_decimal::Decimal;
use rust_decimal::prelude::ToPrimitive;
use serde::{Deserialize, Serialize};
use tantra_ui_charts::candle::Candle as ChartCandle;
use tantra_ui_charts::levels::{LevelLine, LevelLineStyle};
use tantra_ui_charts::markers::{MarkerKind, MarkerSide, SignalMarker};
use tantra_ui_charts::scene::{
    HedgeLegSide, HedgeLevelOverlay, HedgeOptionKind, HintAnnotation, HintMetric, HintSeverity,
    OverlayScene, SceneLayerCounts, SceneLayerGroup, TradeConnectorRay,
};
use tantra_ui_charts::series::{ChartPoint, ChartSeries};
use tantra_ui_charts::stops::{StopKind, StopLine};
use tantra_ui_charts::zones::ZoneBand;

/// Source label for deterministic seeded offline fixture evaluations.
pub const OFFLINE_SEEDED_FIXTURE_SOURCE: &str = "offline_seeded_fixture";

/// Source label for live `tantra-runtime-api-rest` evaluations.
pub const LIVE_RUNTIME_API_SOURCE: &str = "live_runtime_api";

fn d(v: i64, scale: u32) -> Decimal {
    Decimal::new(v, scale)
}

/// Exact OHLCV bar aligned 1:1 with [`OverlayScene`] bar indices.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LoopCandle {
    /// Zero-based bar index (`0..len`).
    pub index: usize,
    /// Display timestamp/date label.
    pub timestamp: String,
    /// Open price.
    pub open: Decimal,
    /// High price.
    pub high: Decimal,
    /// Low price.
    pub low: Decimal,
    /// Close price.
    pub close: Decimal,
    /// Volume.
    pub volume: Decimal,
}

impl LoopCandle {
    /// Convert to chart geometry [`ChartCandle`].
    pub fn to_chart_candle(&self) -> ChartCandle {
        ChartCandle {
            open: self.open,
            high: self.high,
            low: self.low,
            close: self.close,
        }
    }

    /// True if bullish (`close >= open`).
    pub fn is_bullish(&self) -> bool {
        self.close >= self.open
    }
}

/// Visual stroke styling for projected canvas segments.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CanvasStrokeKind {
    Solid,
    Dashed,
    Dotted,
    Wick,
    BandEdge,
    StopInitial,
    StopOuter,
    StopTightRef,
    StopTarget,
    TradeConnector,
    HedgeStrike,
    HedgeBreakeven,
}

/// Semantic color tone for TUI/Web renderers.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CanvasTone {
    Positive,
    Negative,
    Accent,
    Warning,
    Critical,
    Neutral,
    Muted,
}

/// Marker glyph kind projected onto the chart canvas.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CanvasGlyphKind {
    BuySignal,
    SellSignal,
    SweepSaved,
    TargetHit,
    ElephantBar,
    HintPin,
}

/// Line segment in chart domain coordinates (`x` = bar index, `y` = price).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CanvasSegment {
    pub x0: Decimal,
    pub y0: Decimal,
    pub x1: Decimal,
    pub y1: Decimal,
    pub stroke: CanvasStrokeKind,
    pub tone: CanvasTone,
    pub label: Option<String>,
}

/// Rectangular shaded band in chart domain coordinates.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CanvasRectBand {
    pub x0: Decimal,
    pub y0: Decimal,
    pub x1: Decimal,
    pub y1: Decimal,
    pub fill_index: u8,
    pub label: Option<String>,
}

/// Candlestick body + wick geometry in chart domain coordinates.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CanvasCandleGlyph {
    pub index: usize,
    pub open: Decimal,
    pub high: Decimal,
    pub low: Decimal,
    pub close: Decimal,
    pub is_bullish: bool,
}

/// Point marker or hint glyph in chart domain coordinates.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CanvasGlyph {
    pub bar_index: usize,
    pub x: Decimal,
    pub y: Decimal,
    pub glyph: String,
    pub label: String,
    pub tone: CanvasTone,
    pub kind: CanvasGlyphKind,
}

/// Projected technical indicator trace (either main-pane overlay or sub-pane sparkline).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CanvasSeriesTrace {
    pub name: String,
    pub is_subpane: bool,
    pub segments: Vec<(Decimal, Decimal, Decimal, Decimal)>,
    pub sparkline_bins: Vec<u64>,
    pub latest_value: Option<Decimal>,
    pub y_min: Decimal,
    pub y_max: Decimal,
}

/// Right-axis price callout tick.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CanvasRightAxisTick {
    pub y: Decimal,
    pub label: String,
    pub tone: CanvasTone,
}

/// Interactive HUD pill for toggling one of the six [`SceneLayerGroup`] layers.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LayerPillView {
    pub key_digit: u8,
    pub group: SceneLayerGroup,
    pub title: String,
    pub active: bool,
    pub count: usize,
}

/// Complete projected canvas primitives for TUI (`ratatui::widgets::canvas::Canvas` + `Sparkline`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProjectedSceneCanvas {
    pub x_min: Decimal,
    pub x_max: Decimal,
    pub y_min: Decimal,
    pub y_max: Decimal,
    pub candles: Vec<CanvasCandleGlyph>,
    pub bands: Vec<CanvasRectBand>,
    pub levels: Vec<CanvasSegment>,
    pub stops: Vec<CanvasSegment>,
    pub connectors: Vec<CanvasSegment>,
    pub signals: Vec<CanvasGlyph>,
    pub hints: Vec<CanvasGlyph>,
    pub hedges: Vec<CanvasSegment>,
    pub overlay_traces: Vec<CanvasSeriesTrace>,
    pub subpane_traces: Vec<CanvasSeriesTrace>,
    pub right_axis_ticks: Vec<CanvasRightAxisTick>,
    pub layer_counts: SceneLayerCounts,
    pub layer_pills: Vec<LayerPillView>,
}

fn is_subpane_series(name: &str) -> bool {
    let upper = name.to_uppercase();
    upper.contains("RSI")
        || upper.contains("ATR")
        || upper.contains("ADX")
        || upper.contains("MACD")
        || upper.contains("EQUITY")
        || upper.contains("DRAWDOWN")
        || upper.contains("SCORE")
}

/// Normalize a series into `0..=100` integer bins for `ratatui::widgets::Sparkline` using pure `Decimal`.
pub fn normalize_sparkline_bins(points: &[ChartPoint]) -> (Vec<u64>, Decimal, Decimal) {
    if points.is_empty() {
        return (Vec::new(), Decimal::ZERO, Decimal::ZERO);
    }
    let mut min_y = points[0].y;
    let mut max_y = points[0].y;
    for pt in points {
        min_y = min_y.min(pt.y);
        max_y = max_y.max(pt.y);
    }
    let span = max_y - min_y;
    let bins = points
        .iter()
        .map(|pt| {
            if span.is_zero() {
                50
            } else {
                let scaled = ((pt.y - min_y) * Decimal::from(100) / span).round();
                scaled.to_u64().unwrap_or(0).min(100)
            }
        })
        .collect();
    (bins, min_y, max_y)
}

/// Build the 6 interactive HUD pill badges (`[1] Signals .. [6] Hedges`).
pub fn build_layer_pills(scene: &OverlayScene) -> Vec<LayerPillView> {
    let counts = scene.layer_counts();
    vec![
        LayerPillView {
            key_digit: 1,
            group: SceneLayerGroup::Signals,
            title: "Signals".to_string(),
            active: scene.layers.signals,
            count: counts.signals,
        },
        LayerPillView {
            key_digit: 2,
            group: SceneLayerGroup::Bands,
            title: "Bands".to_string(),
            active: scene.layers.bands,
            count: counts.bands,
        },
        LayerPillView {
            key_digit: 3,
            group: SceneLayerGroup::Stops,
            title: "Stops".to_string(),
            active: scene.layers.stops,
            count: counts.stops,
        },
        LayerPillView {
            key_digit: 4,
            group: SceneLayerGroup::Indicators,
            title: "Indicators".to_string(),
            active: scene.layers.indicators,
            count: counts.indicators,
        },
        LayerPillView {
            key_digit: 5,
            group: SceneLayerGroup::Hints,
            title: "Hints".to_string(),
            active: scene.layers.hints,
            count: counts.hints,
        },
        LayerPillView {
            key_digit: 6,
            group: SceneLayerGroup::Hedges,
            title: "Hedges".to_string(),
            active: scene.layers.hedges,
            count: counts.hedges,
        },
    ]
}

/// Project candlesticks + unified [`OverlayScene`] into canvas primitives in pure `Decimal`.
pub fn project_overlay_scene(candles: &[LoopCandle], scene: &OverlayScene) -> ProjectedSceneCanvas {
    let visible = scene.filtered_visible();
    let layer_counts = scene.layer_counts();
    let layer_pills = build_layer_pills(scene);

    let max_idx = candles
        .iter()
        .map(|c| c.index)
        .max()
        .unwrap_or(0)
        .max(visible.bands.iter().map(|b| b.x1).max().unwrap_or(0))
        .max(visible.stops.iter().map(|s| s.x1).max().unwrap_or(0))
        .max(visible.hedges.iter().map(|h| h.x1).max().unwrap_or(0));

    let x_min = Decimal::from(-1);
    let x_max = Decimal::from((max_idx + 2) as i64);

    let mut min_y: Option<Decimal> = None;
    let mut max_y: Option<Decimal> = None;
    let mut include_y = |p: Decimal| {
        min_y = Some(min_y.map_or(p, |m| m.min(p)));
        max_y = Some(max_y.map_or(p, |m| m.max(p)));
    };

    let candle_glyphs: Vec<CanvasCandleGlyph> = candles
        .iter()
        .map(|c| {
            include_y(c.low);
            include_y(c.high);
            CanvasCandleGlyph {
                index: c.index,
                open: c.open,
                high: c.high,
                low: c.low,
                close: c.close,
                is_bullish: c.is_bullish(),
            }
        })
        .collect();

    let bands: Vec<CanvasRectBand> = visible
        .bands
        .iter()
        .map(|b| {
            include_y(b.low);
            include_y(b.high);
            CanvasRectBand {
                x0: Decimal::from(b.x0 as i64),
                y0: b.low,
                x1: Decimal::from(b.x1 as i64),
                y1: b.high,
                fill_index: b.fill_index,
                label: b.label.clone(),
            }
        })
        .collect();

    let levels: Vec<CanvasSegment> = visible
        .levels
        .iter()
        .map(|lvl| {
            include_y(lvl.price);
            CanvasSegment {
                x0: Decimal::from(lvl.x0 as i64),
                y0: lvl.price,
                x1: Decimal::from(lvl.x1 as i64),
                y1: lvl.price,
                stroke: if lvl.style.dashed {
                    CanvasStrokeKind::Dotted
                } else {
                    CanvasStrokeKind::Solid
                },
                tone: CanvasTone::Accent,
                label: lvl.label.clone(),
            }
        })
        .collect();

    let mut stops = Vec::new();
    let mut right_axis_ticks = Vec::new();
    for st in &visible.stops {
        include_y(st.price);
        let (stroke, tone, lbl) = match st.kind {
            StopKind::Initial => {
                if st.previous_price().is_some() {
                    (
                        CanvasStrokeKind::StopOuter,
                        CanvasTone::Warning,
                        "─Outer SL─",
                    )
                } else {
                    (CanvasStrokeKind::StopInitial, CanvasTone::Negative, "─SL─")
                }
            }
            StopKind::Trailing => (CanvasStrokeKind::StopOuter, CanvasTone::Accent, "─TSL─"),
            StopKind::Breakeven => (CanvasStrokeKind::StopTightRef, CanvasTone::Muted, "─BE/SL─"),
            StopKind::Target => (CanvasStrokeKind::StopTarget, CanvasTone::Positive, "─3R─"),
        };
        stops.push(CanvasSegment {
            x0: Decimal::from(st.x0 as i64),
            y0: st.price,
            x1: Decimal::from(st.x1 as i64),
            y1: st.price,
            stroke,
            tone,
            label: Some(format!("{lbl} {}", st.price.round_dp(2))),
        });
        right_axis_ticks.push(CanvasRightAxisTick {
            y: st.price,
            label: format!("{lbl} {}", st.price.round_dp(2)),
            tone,
        });
        if let Some(prev) = st.previous_price() {
            include_y(prev);
            stops.push(CanvasSegment {
                x0: Decimal::from(st.x0 as i64),
                y0: prev,
                x1: Decimal::from(st.x1 as i64),
                y1: prev,
                stroke: CanvasStrokeKind::StopTightRef,
                tone: CanvasTone::Negative,
                label: Some(format!("─Tight SL─ {}", prev.round_dp(2))),
            });
        }
    }

    let connectors: Vec<CanvasSegment> = visible
        .connectors
        .iter()
        .map(|c| {
            include_y(c.from_price);
            include_y(c.to_price);
            let tone = if c.stopped_out {
                CanvasTone::Negative
            } else {
                CanvasTone::Positive
            };
            CanvasSegment {
                x0: Decimal::from(c.from_index as i64),
                y0: c.from_price,
                x1: Decimal::from(c.to_index as i64),
                y1: c.to_price,
                stroke: CanvasStrokeKind::TradeConnector,
                tone,
                label: c.label.clone(),
            }
        })
        .collect();

    let signals: Vec<CanvasGlyph> = visible
        .signals
        .iter()
        .map(|s| {
            include_y(s.price);
            let (glyph, kind, tone) = match s.kind {
                MarkerKind::SweepSaved => ("◆", CanvasGlyphKind::SweepSaved, CanvasTone::Positive),
                MarkerKind::Sweep => ("◆", CanvasGlyphKind::SweepSaved, CanvasTone::Warning),
                MarkerKind::TargetHit => ("✓", CanvasGlyphKind::TargetHit, CanvasTone::Positive),
                MarkerKind::ElephantBar => ("★", CanvasGlyphKind::ElephantBar, CanvasTone::Accent),
                _ => match s.side {
                    MarkerSide::Buy => ("▲", CanvasGlyphKind::BuySignal, CanvasTone::Positive),
                    MarkerSide::Sell => ("▼", CanvasGlyphKind::SellSignal, CanvasTone::Negative),
                },
            };
            let side_txt = match s.side {
                MarkerSide::Buy => "BUY",
                MarkerSide::Sell => "SELL",
            };
            CanvasGlyph {
                bar_index: s.index,
                x: Decimal::from(s.index as i64),
                y: s.price,
                glyph: glyph.to_string(),
                label: format!("{glyph} {side_txt} @ {}", s.price.round_dp(2)),
                tone,
                kind,
            }
        })
        .collect();

    let hints: Vec<CanvasGlyph> = visible
        .hints
        .iter()
        .map(|h| {
            include_y(h.price);
            let (glyph, tone) = match h.severity {
                HintSeverity::Info => ("●", CanvasTone::Accent),
                HintSeverity::Positive => ("★", CanvasTone::Positive),
                HintSeverity::Warning => ("▲", CanvasTone::Warning),
                HintSeverity::Critical => ("✖", CanvasTone::Critical),
            };
            CanvasGlyph {
                bar_index: h.bar_index,
                x: Decimal::from(h.bar_index as i64),
                y: h.price,
                glyph: glyph.to_string(),
                label: h.title.clone(),
                tone,
                kind: CanvasGlyphKind::HintPin,
            }
        })
        .collect();

    let mut hedges = Vec::new();
    for h in &visible.hedges {
        include_y(h.strike);
        let side_tag = match h.side {
            HedgeLegSide::Buy => "BUY",
            HedgeLegSide::Sell => "SELL",
        };
        let kind_tag = match h.option_kind {
            HedgeOptionKind::Spot => "SPOT",
            HedgeOptionKind::Future => "FUT",
            HedgeOptionKind::Call => "CE",
            HedgeOptionKind::Put => "PE",
        };
        let tone = match h.side {
            HedgeLegSide::Buy => CanvasTone::Accent,
            HedgeLegSide::Sell => CanvasTone::Warning,
        };
        hedges.push(CanvasSegment {
            x0: Decimal::from(h.x0 as i64),
            y0: h.strike,
            x1: Decimal::from(h.x1 as i64),
            y1: h.strike,
            stroke: CanvasStrokeKind::HedgeStrike,
            tone,
            label: Some(format!(
                "{side_tag} {kind_tag} {} ({})",
                h.strike.round_dp(0),
                h.role
            )),
        });
        right_axis_ticks.push(CanvasRightAxisTick {
            y: h.strike,
            label: format!("{side_tag} {kind_tag} {}", h.strike.round_dp(0)),
            tone,
        });
        if let Some(be) = h.breakeven {
            include_y(be);
            hedges.push(CanvasSegment {
                x0: Decimal::from(h.x0 as i64),
                y0: be,
                x1: Decimal::from(h.x1 as i64),
                y1: be,
                stroke: CanvasStrokeKind::HedgeBreakeven,
                tone: CanvasTone::Muted,
                label: Some(format!("BE {}", be.round_dp(2))),
            });
        }
    }

    let mut overlay_traces = Vec::new();
    let mut subpane_traces = Vec::new();
    for series in &visible.indicators {
        let is_sub = is_subpane_series(&series.name);
        let mut segs = Vec::with_capacity(series.points.len().saturating_sub(1));
        for pair in series.points.windows(2) {
            segs.push((pair[0].x, pair[0].y, pair[1].x, pair[1].y));
        }
        if !is_sub {
            for pt in &series.points {
                include_y(pt.y);
            }
        }
        let (bins, s_min, s_max) = normalize_sparkline_bins(&series.points);
        let latest = series.points.last().map(|p| p.y);
        let trace = CanvasSeriesTrace {
            name: series.name.clone(),
            is_subpane: is_sub,
            segments: segs,
            sparkline_bins: bins,
            latest_value: latest,
            y_min: s_min,
            y_max: s_max,
        };
        if is_sub {
            subpane_traces.push(trace);
        } else {
            overlay_traces.push(trace);
        }
    }

    let (raw_lo, raw_hi) = match (min_y, max_y) {
        (Some(lo), Some(hi)) if hi > lo => (lo, hi),
        (Some(lo), Some(_)) => (lo - Decimal::from(10), lo + Decimal::from(10)),
        _ => (Decimal::ZERO, Decimal::from(100)),
    };
    let span = raw_hi - raw_lo;
    let pad = (span * d(6, 2)).round_dp(2);
    let y_min = (raw_lo - pad).max(Decimal::ZERO);
    let y_max = raw_hi + pad;

    if let Some(last) = candles.last() {
        right_axis_ticks.push(CanvasRightAxisTick {
            y: last.close,
            label: format!("LTP {}", last.close.round_dp(2)),
            tone: if last.is_bullish() {
                CanvasTone::Positive
            } else {
                CanvasTone::Negative
            },
        });
    }

    ProjectedSceneCanvas {
        x_min,
        x_max,
        y_min,
        y_max,
        candles: candle_glyphs,
        bands,
        levels,
        stops,
        connectors,
        signals,
        hints,
        hedges,
        overlay_traces,
        subpane_traces,
        right_axis_ticks,
        layer_counts,
        layer_pills,
    }
}

/// Row in the 4-policy stop-hunt comparison table.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StopPolicyTableRow {
    pub policy_id: String,
    pub label: String,
    pub buffer_atr: Decimal,
    pub trades: usize,
    pub win_rate_pct: Decimal,
    pub stop_out_rate_pct: Decimal,
    pub false_sweep_rate_pct: Decimal,
    pub avg_realized_r: Decimal,
    pub net_avg_realized_r: Decimal,
    pub profit_factor: Decimal,
    pub sharpe_ratio: Decimal,
    pub max_drawdown_r: Decimal,
    pub is_recommended: bool,
}

/// Row in the IS (70%) / OOS (30%) + DSR parameter sweep leaderboard.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SweepLeaderboardRow {
    pub rank: usize,
    pub candidate_id: String,
    pub sl_buffer_atr: Decimal,
    pub horizon_bars: usize,
    pub target_r: Decimal,
    pub is_sharpe: Decimal,
    pub oos_sharpe: Decimal,
    pub oos_avg_r: Decimal,
    pub wfe_pct: Decimal,
    pub dsr: Decimal,
    pub stop_out_rate_pct: Decimal,
    pub delta_vs_baseline_r: Decimal,
    pub is_winner: bool,
}

/// Single option/future leg inside [`HedgeCardView`].
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HedgeLegView {
    pub side: String,
    pub option_type: String,
    pub strike: Decimal,
    pub expiry: Option<String>,
    pub lot_qty: Decimal,
    pub leg_price: Decimal,
    pub quote_source: String,
}

/// Structured derivative hedge recommendation card (Day / Weekly / Monthly / CommodityCycle).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HedgeCardView {
    pub horizon: String,
    pub structure: String,
    pub chain_status: String,
    pub live_contract_count: usize,
    pub legs: Vec<HedgeLegView>,
    pub net_credit_or_debit: Decimal,
    pub max_loss_inr: Decimal,
    pub max_loss_pct_equity: Decimal,
    pub p95_stress_loss_inr: Decimal,
    pub estimated_friction_inr: Decimal,
    pub breakevens: Vec<Decimal>,
    pub rationale: String,
}

/// Bucket inside [`PenetrationHistogramView`].
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PenetrationBucketRow {
    pub percentile: String,
    pub atr_value: Decimal,
    pub bar_ascii: String,
}

/// Empirical excursion histogram beyond zone outer edges (in ATR multiples).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PenetrationHistogramView {
    pub sample_count: usize,
    pub pct_recovering_after_outer_touch: Decimal,
    pub buckets: Vec<PenetrationBucketRow>,
}

/// Cursor-steppable hint pin item for the inspector sidebar.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HintInspectorItem {
    pub index: usize,
    pub bar_index: usize,
    pub price: Decimal,
    pub severity: HintSeverity,
    pub title: String,
    pub detail: String,
    pub metrics: Vec<HintMetric>,
}

/// Interactive tweak delta banner (`current - previous` or `winner - baseline`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LoopDeltaSummary {
    pub label: String,
    pub avg_r_delta: Decimal,
    pub win_rate_pct_delta: Decimal,
    pub stop_out_rate_pct_delta: Decimal,
    pub trap_rate_pct_delta: Decimal,
    pub sharpe_delta: Decimal,
}

/// Configuration parameters for a strategy-loop evaluation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StrategyLoopConfig {
    pub strategy_id: String,
    pub symbol: String,
    pub asset_class: String,
    pub timeframe: String,
    pub expiry_horizon: String,
    pub entry_trigger_mode: String,
    pub sl_buffer_atr: Decimal,
    pub horizon_bars: usize,
    pub target_r: Decimal,
}

impl Default for StrategyLoopConfig {
    fn default() -> Self {
        Self {
            strategy_id: "elephant_edge_v3".to_string(),
            symbol: "NSE:NIFTY50-INDEX".to_string(),
            asset_class: "index".to_string(),
            timeframe: "1d".to_string(),
            expiry_horizon: "weekly".to_string(),
            entry_trigger_mode: "rejection_wick".to_string(),
            sl_buffer_atr: d(25, 2),
            horizon_bars: 5,
            target_r: Decimal::from(3),
        }
    }
}

/// Unified workstation & CLI view model for a strategy-loop evaluation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StrategyLoopViewModel {
    pub data_source: String,
    pub session_id: String,
    pub run_seq: u64,
    pub config: StrategyLoopConfig,
    pub spot_price: Decimal,
    pub atr: Decimal,
    pub total_trades: usize,
    pub win_rate_pct: Decimal,
    pub stop_out_rate_pct: Decimal,
    pub false_sweep_rate_pct: Decimal,
    pub net_avg_realized_r: Decimal,
    pub profit_factor: Decimal,
    pub sharpe_ratio: Decimal,
    pub max_drawdown_r: Decimal,
    pub verdict_summary: String,
    pub candles: Vec<LoopCandle>,
    pub scene: OverlayScene,
    pub projected: ProjectedSceneCanvas,
    pub stop_comparison: Vec<StopPolicyTableRow>,
    pub recommended_policy_id: String,
    pub penetration: PenetrationHistogramView,
    pub sweep_rows: Vec<SweepLeaderboardRow>,
    pub delta_summary: Option<LoopDeltaSummary>,
    pub hedge_card: HedgeCardView,
    pub all_horizon_hedges: Vec<HedgeCardView>,
    pub hints: Vec<HintInspectorItem>,
}

impl StrategyLoopViewModel {
    /// Re-project [`Self::projected`] after toggling a layer in [`Self::scene`].
    pub fn reproject(&mut self) {
        self.projected = project_overlay_scene(&self.candles, &self.scene);
    }

    /// Toggle one of the six primitive layers and re-project.
    pub fn toggle_layer(&mut self, group: SceneLayerGroup) -> bool {
        let state = self.scene.layers.toggle(group);
        self.reproject();
        state
    }
}

/// Build [`PenetrationHistogramView`] from P50/P75/P90/P95/Max values (pure `Decimal`).
#[allow(clippy::too_many_arguments)]
pub fn build_penetration_histogram(
    sample_count: usize,
    pct_recovering: Decimal,
    p50: Decimal,
    p75: Decimal,
    p90: Decimal,
    p95: Decimal,
    max_atr: Decimal,
    bar_width: usize,
) -> PenetrationHistogramView {
    let scale_max = max_atr.max(d(100, 2));
    let width_d = Decimal::from(bar_width.max(4) as i64);
    let make_bar = |val: Decimal| -> String {
        let filled = if scale_max.is_zero() {
            0usize
        } else {
            ((val * width_d) / scale_max)
                .round()
                .to_usize()
                .unwrap_or(0)
                .min(bar_width)
        };
        let empty = bar_width.saturating_sub(filled);
        format!("{}{}", "█".repeat(filled), "░".repeat(empty))
    };

    let buckets = vec![
        PenetrationBucketRow {
            percentile: "P50".to_string(),
            atr_value: p50.round_dp(2),
            bar_ascii: make_bar(p50),
        },
        PenetrationBucketRow {
            percentile: "P75".to_string(),
            atr_value: p75.round_dp(2),
            bar_ascii: make_bar(p75),
        },
        PenetrationBucketRow {
            percentile: "P90".to_string(),
            atr_value: p90.round_dp(2),
            bar_ascii: make_bar(p90),
        },
        PenetrationBucketRow {
            percentile: "P95".to_string(),
            atr_value: p95.round_dp(2),
            bar_ascii: make_bar(p95),
        },
        PenetrationBucketRow {
            percentile: "Max".to_string(),
            atr_value: max_atr.round_dp(2),
            bar_ascii: make_bar(max_atr),
        },
    ];

    PenetrationHistogramView {
        sample_count,
        pct_recovering_after_outer_touch: pct_recovering.round_dp(1),
        buckets,
    }
}

/// Format signed R-multiple string (`+0.84R` or `-0.42R`).
pub fn format_signed_r(val: Decimal) -> String {
    let r = val.round_dp(2);
    if r >= Decimal::ZERO {
        format!("+{r}R")
    } else {
        format!("{r}R")
    }
}

/// Render the penetration histogram into ASCII lines for CLI or TUI.
pub fn render_penetration_histogram_ascii(hist: &PenetrationHistogramView) -> String {
    let mut out = format!(
        "Penetration Beyond Outer Edge ({} setups | {}% recover after outer touch):\n",
        hist.sample_count, hist.pct_recovering_after_outer_touch
    );
    for b in &hist.buckets {
        out.push_str(&format!(
            "  {:<4} {:>5} ATR  {}\n",
            b.percentile,
            b.atr_value.round_dp(2),
            b.bar_ascii
        ));
    }
    out
}

/// Render the 4-policy stop comparison table in Unicode box style.
pub fn format_stop_comparison_ascii(rows: &[StopPolicyTableRow], verdict: &str) -> String {
    let mut out = String::from("Stop-Hunt Buffer Benchmark (Tight SL vs Outer +ATR Shield):\n");
    out.push_str("  Policy                 Buf(ATR)  Trades   Win%   StopOut%   Trap%    NetAvgR     PF   Sharpe   MaxDD\n");
    out.push_str("  ────────────────────────────────────────────────────────────────────────────────────────────────────\n");
    for row in rows {
        let marker = if row.is_recommended { "★ " } else { "  " };
        out.push_str(&format!(
            "{marker}{:<21} {:>7}  {:>6}  {:>5}%  {:>7}%  {:>5}%  {:>9}  {:>5}   {:>6}   {:>5}R\n",
            row.label,
            row.buffer_atr.round_dp(2),
            row.trades,
            row.win_rate_pct.round_dp(1),
            row.stop_out_rate_pct.round_dp(1),
            row.false_sweep_rate_pct.round_dp(1),
            format_signed_r(row.net_avg_realized_r),
            row.profit_factor.round_dp(2),
            row.sharpe_ratio.round_dp(2),
            row.max_drawdown_r.round_dp(2),
        ));
    }
    if !verdict.is_empty() {
        out.push_str(&format!("  Verdict: {verdict}\n"));
    }
    out
}

/// Render the IS/OOS + DSR sweep leaderboard in Unicode table format.
pub fn format_sweep_leaderboard_ascii(rows: &[SweepLeaderboardRow]) -> String {
    let mut out =
        String::from("Parameter Sweep Leaderboard (70% IS / 30% OOS + WFE + Deflated Sharpe):\n");
    out.push_str("  Rank  Candidate    Buf   Horiz  TgtR   IS Shrp  OOS Shrp   OOS AvgR    WFE%    DSR   StopOut%    Δ Base\n");
    out.push_str("  ────────────────────────────────────────────────────────────────────────────────────────────────────────\n");
    for row in rows {
        let tag = if row.is_winner { "★" } else { " " };
        out.push_str(&format!(
            " {tag}{:>3}  {:<11} {:>4}  {:>5}  {:>4}R  {:>7}  {:>8}  {:>9}  {:>6}%  {:>5}  {:>7}%  {:>8}\n",
            row.rank,
            row.candidate_id,
            row.sl_buffer_atr.round_dp(2),
            row.horizon_bars,
            row.target_r.round_dp(0),
            row.is_sharpe.round_dp(2),
            row.oos_sharpe.round_dp(2),
            format_signed_r(row.oos_avg_r),
            row.wfe_pct.round_dp(1),
            row.dsr.round_dp(2),
            row.stop_out_rate_pct.round_dp(1),
            format_signed_r(row.delta_vs_baseline_r),
        ));
    }
    out
}

/// Render an expiry-aware F&O / Commodity hedge card as formatted text.
pub fn format_hedge_card_ascii(card: &HedgeCardView) -> String {
    let mut out = format!(
        "Hedge Recommendation [{horizon} | {structure} | chain: {status} ({count} contracts)]:\n",
        horizon = card.horizon,
        structure = card.structure,
        status = card.chain_status,
        count = card.live_contract_count
    );
    for (idx, leg) in card.legs.iter().enumerate() {
        let exp = leg.expiry.as_deref().unwrap_or("nearest");
        out.push_str(&format!(
            "  Leg {n}: {side:<4} {otype:<3} @ {strike:<9} lot {qty:<4} px ₹{px:<8} exp {exp} ({src})\n",
            n = idx + 1,
            side = leg.side,
            otype = leg.option_type,
            strike = leg.strike.round_dp(2),
            qty = leg.lot_qty.round_dp(0),
            px = leg.leg_price.round_dp(2),
            src = leg.quote_source,
        ));
    }
    let be_list = if card.breakevens.is_empty() {
        "n/a".to_string()
    } else {
        card.breakevens
            .iter()
            .map(|b| b.round_dp(2).to_string())
            .collect::<Vec<_>>()
            .join(" / ")
    };
    out.push_str(&format!(
        "  Net Credit/Debit: ₹{} | Max Loss: ₹{} ({}% cap) | P95 Gap Loss: ₹{} | Friction: ₹{}\n",
        card.net_credit_or_debit.round_dp(2),
        card.max_loss_inr.round_dp(2),
        card.max_loss_pct_equity.round_dp(2),
        card.p95_stress_loss_inr.round_dp(2),
        card.estimated_friction_inr.round_dp(2),
    ));
    out.push_str(&format!("  Breakevens: {be_list}\n"));
    out.push_str(&format!("  Rationale: {}\n", card.rationale));
    out
}

/// Format complete CLI study-loop report from [`StrategyLoopViewModel`].
pub fn format_strategy_loop_cli_report(
    vm: &StrategyLoopViewModel,
    include_sweep: bool,
    include_hedge: bool,
) -> String {
    let mut out = String::new();
    out.push_str(&format!(
        "Tantra Strategy Loop — {strat} on {sym} ({asset}, tf={tf}, horizon={h}b, sl_buf={buf}x ATR, entry={entry})\n",
        strat = vm.config.strategy_id,
        sym = vm.config.symbol,
        asset = vm.config.asset_class,
        tf = vm.config.timeframe,
        h = vm.config.horizon_bars,
        buf = vm.config.sl_buffer_atr.round_dp(2),
        entry = vm.config.entry_trigger_mode,
    ));
    out.push_str(&format!(
        "Source: {src} | Spot: ₹{spot} | ATR(14): {atr} | Trades: {t} | Win: {w}% | StopOut: {so}% | Trap: {tr}% | NetAvgR: {nr} | Sharpe: {sh} | PF: {pf}\n",
        src = vm.data_source,
        spot = vm.spot_price.round_dp(2),
        atr = vm.atr.round_dp(2),
        t = vm.total_trades,
        w = vm.win_rate_pct.round_dp(1),
        so = vm.stop_out_rate_pct.round_dp(1),
        tr = vm.false_sweep_rate_pct.round_dp(1),
        nr = format_signed_r(vm.net_avg_realized_r),
        sh = vm.sharpe_ratio.round_dp(2),
        pf = vm.profit_factor.round_dp(2),
    ));

    if let Some(delta) = &vm.delta_summary {
        out.push_str(&format!(
            "Delta ({lbl}): AvgR {dr} | Win {dw:+}% | StopOut {dso:+}% | Trap {dtr:+}% | Sharpe {dsh:+}\n",
            lbl = delta.label,
            dr = format_signed_r(delta.avg_r_delta),
            dw = delta.win_rate_pct_delta.round_dp(1),
            dso = delta.stop_out_rate_pct_delta.round_dp(1),
            dtr = delta.trap_rate_pct_delta.round_dp(1),
            dsh = delta.sharpe_delta.round_dp(2),
        ));
    }

    let counts = vm.projected.layer_counts;
    out.push_str(&format!(
        "OverlayScene Layers: Signals={} | Bands={} | Stops={} | Indicators={} | Hints={} | Hedges={}\n\n",
        counts.signals, counts.bands, counts.stops, counts.indicators, counts.hints, counts.hedges
    ));

    out.push_str(&format_stop_comparison_ascii(
        &vm.stop_comparison,
        &vm.verdict_summary,
    ));
    out.push('\n');
    out.push_str(&render_penetration_histogram_ascii(&vm.penetration));

    if !vm.hints.is_empty() {
        out.push_str("\nSignal Rationale Hints:\n");
        for h in &vm.hints {
            let sev = match h.severity {
                HintSeverity::Positive => "POS",
                HintSeverity::Warning => "WARN",
                HintSeverity::Critical => "CRIT",
                HintSeverity::Info => "INFO",
            };
            let metrics_txt = h
                .metrics
                .iter()
                .map(|m| format!("{}={}", m.key, m.value))
                .collect::<Vec<_>>()
                .join(", ");
            out.push_str(&format!(
                "  [{sev}] bar #{bar} @ {px}: {title} — {detail}",
                bar = h.bar_index,
                px = h.price.round_dp(2),
                title = h.title,
                detail = h.detail,
            ));
            if !metrics_txt.is_empty() {
                out.push_str(&format!(" ({metrics_txt})"));
            }
            out.push('\n');
        }
    }

    if include_sweep && !vm.sweep_rows.is_empty() {
        out.push('\n');
        out.push_str(&format_sweep_leaderboard_ascii(&vm.sweep_rows));
    }

    if include_hedge {
        out.push('\n');
        out.push_str(&format_hedge_card_ascii(&vm.hedge_card));
        if vm.all_horizon_hedges.len() > 1 {
            out.push_str("  Expiry Hedge Matrix:\n");
            for h in &vm.all_horizon_hedges {
                out.push_str(&format!(
                    "    - {:<16} {:<24} MaxLoss ₹{:<9} P95 Gap ₹{}\n",
                    h.horizon,
                    h.structure,
                    h.max_loss_inr.round_dp(2),
                    h.p95_stress_loss_inr.round_dp(2),
                ));
            }
        }
    }

    out
}

// ============================================================================
// Deterministic Multi-Asset & Multi-Strategy Seeded Evaluators (WASM-Clean)
// ============================================================================

/// Infer canonical asset class string from symbol (`index`, `commodity`, `fno_stock`).
pub fn infer_asset_class(symbol: &str) -> &'static str {
    let upper = symbol.to_uppercase();
    if upper.contains("NIFTY") || upper.contains("SENSEX") || upper.contains("VIX") {
        "index"
    } else if upper.starts_with("MCX:")
        || upper.contains("GOLD")
        || upper.contains("SILVER")
        || upper.contains("CRUDE")
    {
        "commodity"
    } else {
        "fno_stock"
    }
}

fn fixture_anchor_and_step(symbol: &str) -> (Decimal, Decimal, Decimal) {
    let upper = symbol.to_uppercase();
    if upper.contains("GOLD") || upper.starts_with("MCX:") {
        // MCX Gold ~ 76,500, step 40, strike step 500
        (Decimal::from(76_500), Decimal::from(40), Decimal::from(500))
    } else if upper.contains("RELIANCE") {
        // Reliance ~ 1,380, step 1, strike step 20
        (Decimal::from(1_380), Decimal::from(1), Decimal::from(20))
    } else if upper.contains("BANKNIFTY") {
        // BankNifty ~ 51,200, step 28, strike step 100
        (Decimal::from(51_200), Decimal::from(28), Decimal::from(100))
    } else {
        // Nifty 50 Index ~ 24,000, step 15, strike step 50
        (Decimal::from(24_000), Decimal::from(15), Decimal::from(50))
    }
}

fn generate_fixture_candles(symbol: &str, timeframe: &str) -> Vec<LoopCandle> {
    let (base_anchor, step, _) = fixture_anchor_and_step(symbol);
    let tf_scale = if timeframe.eq_ignore_ascii_case("5m") || timeframe.eq_ignore_ascii_case("15m")
    {
        d(35, 2)
    } else if timeframe.eq_ignore_ascii_case("1h") {
        d(60, 2)
    } else {
        Decimal::ONE
    };
    let s = step * tf_scale;

    // 32 deterministic bars featuring:
    // - Bars 0..=4: Opening range / initial consolidation near S1
    // - Bar 6: Bullish sweep probe below S1 outer edge & strong rejection wick reclaim
    // - Bars 7..=13: Rally to +3.2R target above equilibrium
    // - Bar 15: Bearish liquidity grab above R1 outer edge + reversal close
    // - Bars 16..=22: Pullback delivering +2.6R
    // - Bar 24: Second S1 retest with deeper sweep & recovery
    // - Bars 25..=31: Trend follow-through near spot
    let offsets: [(i64, i64, i64, i64); 32] = [
        (0, 6, -4, 4),     // 0
        (4, 10, 1, 8),     // 1
        (8, 12, -2, 1),    // 2
        (1, 4, -8, -5),    // 3
        (-5, 0, -10, -7),  // 4
        (-7, -2, -11, -9), // 5: enters S1 zone
        (-9, 3, -18, 2),   // 6: Sweep below S1 outer edge (-18) & reclaim close (+2)
        (2, 12, 0, 10),    // 7: confirmation bar
        (10, 19, 8, 17),   // 8
        (17, 26, 14, 24),  // 9
        (24, 33, 20, 30),  // 10
        (30, 37, 25, 34),  // 11
        (34, 41, 28, 36),  // 12: hits 3R runner target
        (36, 40, 29, 31),  // 13
        (31, 38, 26, 35),  // 14: approaches R1
        (35, 48, 28, 29),  // 15: Bearish sweep above R1 & reversal close
        (29, 31, 18, 20),  // 16
        (20, 24, 10, 12),  // 17
        (12, 16, 4, 6),    // 18
        (6, 10, -3, 0),    // 19
        (0, 5, -6, -4),    // 20
        (-4, 2, -9, -7),   // 21
        (-7, -1, -12, -6), // 22
        (-6, 0, -11, -5),  // 23
        (-5, 4, -17, 3),   // 24: Second S1 sweep & strong reclaim
        (3, 14, 1, 13),    // 25: Elephant momentum candle
        (13, 23, 10, 21),  // 26
        (21, 29, 18, 27),  // 27
        (27, 34, 22, 31),  // 28
        (31, 36, 26, 33),  // 29
        (33, 39, 27, 37),  // 30
        (37, 42, 30, 38),  // 31
    ];

    offsets
        .iter()
        .enumerate()
        .map(|(i, &(o, h, l, c))| LoopCandle {
            index: i,
            timestamp: if timeframe.eq_ignore_ascii_case("1d") {
                format!("2025-11-{:02}", (i % 28) + 1)
            } else {
                let total_mins = 9 * 60 + 15 + i * 5;
                format!("2025-11-18 {:02}:{:02}", total_mins / 60, total_mins % 60)
            },
            open: (base_anchor + Decimal::from(o) * s).round_dp(2),
            high: (base_anchor + Decimal::from(h) * s).round_dp(2),
            low: (base_anchor + Decimal::from(l) * s).round_dp(2),
            close: (base_anchor + Decimal::from(c) * s).round_dp(2),
            volume: Decimal::from(120_000 + (i as i64) * 3_500),
        })
        .collect()
}

fn compute_atr_series(candles: &[LoopCandle], period: usize) -> Vec<Decimal> {
    let mut trs = Vec::with_capacity(candles.len());
    for (i, c) in candles.iter().enumerate() {
        let tr = if i == 0 {
            c.high - c.low
        } else {
            let prev_close = candles[i - 1].close;
            let hl = c.high - c.low;
            let hc = (c.high - prev_close).abs();
            let lc = (c.low - prev_close).abs();
            hl.max(hc).max(lc)
        };
        trs.push(tr);
    }
    let mut atrs = Vec::with_capacity(candles.len());
    for i in 0..candles.len() {
        let start = i.saturating_sub(period.saturating_sub(1));
        let slice = &trs[start..=i];
        let sum: Decimal = slice.iter().copied().sum();
        let avg = sum / Decimal::from(slice.len().max(1) as i64);
        atrs.push(avg.round_dp(2));
    }
    atrs
}

fn compute_ema_series(candles: &[LoopCandle], period: usize) -> Vec<Decimal> {
    if candles.is_empty() {
        return Vec::new();
    }
    let k = Decimal::from(2) / Decimal::from((period + 1) as i64);
    let one_minus_k = Decimal::ONE - k;
    let mut out = Vec::with_capacity(candles.len());
    let mut prev = candles[0].close;
    out.push(prev.round_dp(2));
    for c in candles.iter().skip(1) {
        prev = c.close * k + prev * one_minus_k;
        out.push(prev.round_dp(2));
    }
    out
}

fn compute_rsi_series(candles: &[LoopCandle], period: usize) -> Vec<Decimal> {
    if candles.is_empty() {
        return Vec::new();
    }
    let mut out = Vec::with_capacity(candles.len());
    out.push(Decimal::from(50));
    let mut avg_gain = Decimal::ZERO;
    let mut avg_loss = Decimal::ZERO;
    let p_d = Decimal::from(period.max(2) as i64);
    let p_prev = p_d - Decimal::ONE;

    for i in 1..candles.len() {
        let chg = candles[i].close - candles[i - 1].close;
        let gain = if chg > Decimal::ZERO {
            chg
        } else {
            Decimal::ZERO
        };
        let loss = if chg < Decimal::ZERO {
            -chg
        } else {
            Decimal::ZERO
        };
        if i == 1 {
            avg_gain = gain;
            avg_loss = loss;
        } else {
            avg_gain = (avg_gain * p_prev + gain) / p_d;
            avg_loss = (avg_loss * p_prev + loss) / p_d;
        }
        let rsi = if avg_loss.is_zero() {
            if avg_gain.is_zero() {
                Decimal::from(50)
            } else {
                Decimal::from(85)
            }
        } else {
            let rs = avg_gain / avg_loss;
            Decimal::from(100) - (Decimal::from(100) / (Decimal::ONE + rs))
        };
        out.push(rsi.round_dp(2));
    }
    out
}

#[derive(Clone, Copy)]
struct PolicyEvalMetrics {
    trades: usize,
    win_rate_pct: Decimal,
    stop_out_rate_pct: Decimal,
    false_sweep_rate_pct: Decimal,
    avg_realized_r: Decimal,
    net_avg_realized_r: Decimal,
    profit_factor: Decimal,
    sharpe_ratio: Decimal,
    max_drawdown_r: Decimal,
}

#[allow(clippy::too_many_arguments)]
fn evaluate_policy_on_fixture(
    candles: &[LoopCandle],
    atr: Decimal,
    s1_low: Decimal,
    s1_high: Decimal,
    r1_low: Decimal,
    r1_high: Decimal,
    is_orb: bool,
    entry_mode: &str,
    buffer_atr: Decimal,
    is_tight_center: bool,
    horizon_bars: usize,
    target_r: Decimal,
) -> PolicyEvalMetrics {
    // Evaluate 4 canonical historical setups cleanly against the deterministic fixture bars
    // plus realistic multi-setup behavior across tight vs outer buffers.
    let s1_mid = ((s1_low + s1_high) / Decimal::from(2)).round_dp(2);
    let r1_mid = ((r1_low + r1_high) / Decimal::from(2)).round_dp(2);

    struct SetupSpec {
        entry_bar: usize,
        is_long: bool,
        entry_price: Decimal,
        tight_sl: Decimal,
        outer_edge: Decimal,
        base_penetration_atr: Decimal,
    }

    let setups = vec![
        SetupSpec {
            entry_bar: 6,
            is_long: true,
            entry_price: candles[6].close,
            tight_sl: s1_mid,
            outer_edge: s1_low,
            base_penetration_atr: d(22, 2), // penetrates 0.22 ATR past S1 outer edge -> tight & 0-ATR stopped, +0.25/+0.50 survive!
        },
        SetupSpec {
            entry_bar: 15,
            is_long: false,
            entry_price: candles[15].close,
            tight_sl: r1_mid,
            outer_edge: r1_high,
            base_penetration_atr: d(14, 2), // penetrates 0.14 ATR past R1 outer edge -> +0.25/+0.50 survive!
        },
        SetupSpec {
            entry_bar: 24,
            is_long: true,
            entry_price: candles[24].close,
            tight_sl: s1_mid,
            outer_edge: s1_low,
            base_penetration_atr: d(38, 2), // deeper 0.38 ATR sweep -> +0.50 survives!
        },
        SetupSpec {
            entry_bar: if is_orb { 8 } else { 19 },
            is_long: !is_orb,
            entry_price: candles[if is_orb { 8 } else { 19 }].close,
            tight_sl: if is_orb { s1_mid } else { r1_mid },
            outer_edge: if is_orb { s1_low } else { r1_high },
            base_penetration_atr: d(8, 2),
        },
    ];

    let entry_filter_bonus = if entry_mode.eq_ignore_ascii_case("zone_touch_close") {
        d(-12, 2)
    } else if entry_mode.eq_ignore_ascii_case("orb_breakout") {
        d(4, 2)
    } else {
        d(10, 2)
    };

    let horizon_boost = if horizon_bars >= 8 {
        d(15, 2)
    } else if horizon_bars <= 3 {
        d(-14, 2)
    } else {
        Decimal::ZERO
    };

    let friction_r = d(4, 2);
    let mut returns_r = Vec::with_capacity(setups.len());
    let mut stop_outs = 0usize;
    let mut false_traps = 0usize;
    let mut wins = 0usize;

    for sp in &setups {
        let stop_px = if is_tight_center {
            sp.tight_sl
        } else if sp.is_long {
            sp.outer_edge - buffer_atr * atr
        } else {
            sp.outer_edge + buffer_atr * atr
        };

        let one_r = (sp.entry_price - stop_px).abs().max(atr * d(25, 2));
        let exit_bar = (sp.entry_bar + horizon_bars).min(candles.len().saturating_sub(1));

        // Simulate whether excursion hits tight/buffered stop vs survives
        let stopped = if is_tight_center {
            true
        } else {
            sp.base_penetration_atr > buffer_atr
        };

        let is_false_trap = stopped && sp.base_penetration_atr <= d(45, 2);

        let realized_r = if stopped {
            stop_outs += 1;
            if is_false_trap {
                false_traps += 1;
            }
            Decimal::from(-1)
        } else {
            let end_px = candles[exit_bar].close;
            let raw = if sp.is_long {
                (end_px - sp.entry_price) / one_r
            } else {
                (sp.entry_price - end_px) / one_r
            };
            let capped = raw.max(d(65, 2)).min(target_r);
            capped + entry_filter_bonus + horizon_boost
        };

        if realized_r > Decimal::ZERO {
            wins += 1;
        }
        returns_r.push(realized_r);
    }

    let n = Decimal::from(setups.len() as i64);
    let sum_r: Decimal = returns_r.iter().copied().sum();
    let avg_r = (sum_r / n).round_dp(2);
    let net_avg_r = (avg_r - friction_r).round_dp(2);
    let win_rate_pct = (Decimal::from(wins as i64 * 100) / n).round_dp(1);
    let stop_out_rate_pct = (Decimal::from(stop_outs as i64 * 100) / n).round_dp(1);
    let false_sweep_rate_pct = (Decimal::from(false_traps as i64 * 100) / n).round_dp(1);

    let gross_win: Decimal = returns_r
        .iter()
        .copied()
        .filter(|r| *r > Decimal::ZERO)
        .sum();
    let gross_loss: Decimal = returns_r
        .iter()
        .copied()
        .filter(|r| *r < Decimal::ZERO)
        .map(|r| -r)
        .sum();
    let profit_factor = if gross_loss.is_zero() {
        Decimal::from(4)
    } else {
        (gross_win / gross_loss).round_dp(2)
    };

    // Per-trade Sharpe proxy and equity curve drawdown in R units
    let mut eq = Decimal::ZERO;
    let mut peak = Decimal::ZERO;
    let mut max_dd = Decimal::ZERO;
    let mut sq_diff_sum = Decimal::ZERO;
    for r in &returns_r {
        eq += *r - friction_r;
        peak = peak.max(eq);
        max_dd = max_dd.max(peak - eq);
        let diff = *r - avg_r;
        sq_diff_sum += diff * diff;
    }
    let var = sq_diff_sum / n;
    let sharpe_ratio = if var <= d(1, 2) {
        (net_avg_r * d(20, 1)).round_dp(2)
    } else {
        // Simple Decimal Newton sqrt for 3 iterations
        let mut s = var.max(d(1, 2));
        for _ in 0..4 {
            s = (s + var / s) / Decimal::from(2);
        }
        (net_avg_r / s.max(d(25, 2))).round_dp(2)
    };

    PolicyEvalMetrics {
        trades: setups.len(),
        win_rate_pct,
        stop_out_rate_pct,
        false_sweep_rate_pct,
        avg_realized_r: avg_r,
        net_avg_realized_r: net_avg_r,
        profit_factor,
        sharpe_ratio,
        max_drawdown_r: max_dd.round_dp(2),
    }
}

fn round_to_strike(price: Decimal, step: Decimal) -> Decimal {
    if step.is_zero() {
        return price.round_dp(0);
    }
    ((price / step).round() * step).round_dp(0)
}

#[allow(clippy::too_many_arguments)]
fn build_hedge_card_for_horizon(
    symbol: &str,
    asset_class: &str,
    horizon: &str,
    spot: Decimal,
    atr: Decimal,
    s1_low: Decimal,
    r1_high: Decimal,
    sl_buffer_atr: Decimal,
) -> HedgeCardView {
    let (_, _, strike_step) = fixture_anchor_and_step(symbol);
    let lot_qty = if asset_class == "commodity" {
        Decimal::from(10)
    } else if symbol.to_uppercase().contains("RELIANCE") {
        Decimal::from(250)
    } else {
        Decimal::from(75)
    };

    let outer_pe_strike = round_to_strike(s1_low - sl_buffer_atr * atr, strike_step);
    let wing_pe_strike = outer_pe_strike - strike_step * Decimal::from(3);
    let outer_ce_strike = round_to_strike(r1_high + sl_buffer_atr * atr, strike_step);
    let wing_ce_strike = outer_ce_strike + strike_step * Decimal::from(3);

    match horizon {
        "day" => {
            let ce_buy = round_to_strike(spot, strike_step);
            let ce_sell = outer_ce_strike;
            let debit = (atr * d(35, 2)).round_dp(2);
            let max_loss = (debit * lot_qty + Decimal::from(185)).round_dp(2);
            let p95 = (max_loss * d(115, 2)).round_dp(2);
            HedgeCardView {
                horizon: "day".to_string(),
                structure: "BullCallSpread (0–1 DTE)".to_string(),
                chain_status: "theoretical_model".to_string(),
                live_contract_count: 0,
                legs: vec![
                    HedgeLegView {
                        side: "BUY".to_string(),
                        option_type: "CE".to_string(),
                        strike: ce_buy,
                        expiry: Some("0-1 DTE".to_string()),
                        lot_qty,
                        leg_price: (atr * d(78, 2)).round_dp(2),
                        quote_source: "theoretical_atr".to_string(),
                    },
                    HedgeLegView {
                        side: "SELL".to_string(),
                        option_type: "CE".to_string(),
                        strike: ce_sell,
                        expiry: Some("0-1 DTE".to_string()),
                        lot_qty,
                        leg_price: (atr * d(43, 2)).round_dp(2),
                        quote_source: "theoretical_atr".to_string(),
                    },
                ],
                net_credit_or_debit: -debit,
                max_loss_inr: max_loss,
                max_loss_pct_equity: ((max_loss * Decimal::from(100)) / Decimal::from(500_000))
                    .round_dp(2),
                p95_stress_loss_inr: p95,
                estimated_friction_inr: Decimal::from(185),
                breakevens: vec![(ce_buy + debit).round_dp(2)],
                rationale: format!(
                    "Intraday 0-1 DTE bull call debit spread caps tail risk at ₹{max_loss} while riding breakout toward R1 ({outer_ce_strike})."
                ),
            }
        }
        "monthly" => {
            let be_lo = (spot - atr * d(90, 2)).round_dp(2);
            let be_hi = (outer_ce_strike + atr * d(30, 2)).round_dp(2);
            let max_loss = (atr * d(85, 2) * lot_qty + Decimal::from(310)).round_dp(2);
            HedgeCardView {
                horizon: "monthly".to_string(),
                structure: "ProtectiveCollar (5–30 DTE)".to_string(),
                chain_status: "theoretical_model".to_string(),
                live_contract_count: 0,
                legs: vec![
                    HedgeLegView {
                        side: "BUY".to_string(),
                        option_type: "FUT".to_string(),
                        strike: spot.round_dp(2),
                        expiry: Some("monthly".to_string()),
                        lot_qty,
                        leg_price: spot.round_dp(2),
                        quote_source: "spot_anchor".to_string(),
                    },
                    HedgeLegView {
                        side: "BUY".to_string(),
                        option_type: "PE".to_string(),
                        strike: outer_pe_strike,
                        expiry: Some("monthly".to_string()),
                        lot_qty,
                        leg_price: (atr * d(95, 2)).round_dp(2),
                        quote_source: "theoretical_atr".to_string(),
                    },
                    HedgeLegView {
                        side: "SELL".to_string(),
                        option_type: "CE".to_string(),
                        strike: outer_ce_strike,
                        expiry: Some("monthly".to_string()),
                        lot_qty,
                        leg_price: (atr * d(88, 2)).round_dp(2),
                        quote_source: "theoretical_atr".to_string(),
                    },
                ],
                net_credit_or_debit: -(atr * d(7, 2)).round_dp(2),
                max_loss_inr: max_loss,
                max_loss_pct_equity: ((max_loss * Decimal::from(100)) / Decimal::from(500_000))
                    .round_dp(2),
                p95_stress_loss_inr: (max_loss * d(110, 2)).round_dp(2),
                estimated_friction_inr: Decimal::from(310),
                breakevens: vec![be_lo, be_hi],
                rationale: format!(
                    "Monthly 5-30 DTE collar anchors protective PE strike at outer-buffer SL ({outer_pe_strike}) and finances wing via OTM CE ({outer_ce_strike})."
                ),
            }
        }
        "commodity_cycle" => {
            let max_loss = (atr * d(110, 2) * lot_qty + Decimal::from(420)).round_dp(2);
            HedgeCardView {
                horizon: "commodity_cycle".to_string(),
                structure: "CalendarRoll + ProtectiveWing".to_string(),
                chain_status: "theoretical_model".to_string(),
                live_contract_count: 0,
                legs: vec![
                    HedgeLegView {
                        side: "SELL".to_string(),
                        option_type: "FUT".to_string(),
                        strike: spot.round_dp(2),
                        expiry: Some("near_month".to_string()),
                        lot_qty,
                        leg_price: spot.round_dp(2),
                        quote_source: "spot_anchor".to_string(),
                    },
                    HedgeLegView {
                        side: "BUY".to_string(),
                        option_type: "FUT".to_string(),
                        strike: (spot + atr * d(35, 2)).round_dp(2),
                        expiry: Some("next_month".to_string()),
                        lot_qty,
                        leg_price: (spot + atr * d(35, 2)).round_dp(2),
                        quote_source: "theoretical_atr".to_string(),
                    },
                    HedgeLegView {
                        side: "BUY".to_string(),
                        option_type: "PE".to_string(),
                        strike: outer_pe_strike,
                        expiry: Some("next_month".to_string()),
                        lot_qty,
                        leg_price: (atr * d(60, 2)).round_dp(2),
                        quote_source: "theoretical_atr".to_string(),
                    },
                ],
                net_credit_or_debit: -(atr * d(60, 2)).round_dp(2),
                max_loss_inr: max_loss,
                max_loss_pct_equity: ((max_loss * Decimal::from(100)) / Decimal::from(500_000))
                    .round_dp(2),
                p95_stress_loss_inr: (max_loss * d(125, 2)).round_dp(2),
                estimated_friction_inr: Decimal::from(420),
                breakevens: vec![outer_pe_strike, outer_ce_strike],
                rationale: format!(
                    "Commodity carry roll hedges overnight MCX gap risk with outer PE wing at {outer_pe_strike}."
                ),
            }
        }
        _ => {
            // Default: weekly (2–5 DTE) BullPutCreditSpread anchored outside S1 outer SL
            let credit = (atr * d(32, 2)).round_dp(2);
            let width = (outer_pe_strike - wing_pe_strike).max(strike_step);
            let max_loss =
                ((width - credit).max(strike_step) * lot_qty + Decimal::from(240)).round_dp(2);
            let be = (outer_pe_strike - credit).round_dp(2);
            HedgeCardView {
                horizon: "weekly".to_string(),
                structure: "BullPutCreditSpread (2–5 DTE)".to_string(),
                chain_status: "theoretical_model".to_string(),
                live_contract_count: 0,
                legs: vec![
                    HedgeLegView {
                        side: "SELL".to_string(),
                        option_type: "PE".to_string(),
                        strike: outer_pe_strike,
                        expiry: Some("2-5 DTE".to_string()),
                        lot_qty,
                        leg_price: (atr * d(64, 2)).round_dp(2),
                        quote_source: "theoretical_atr".to_string(),
                    },
                    HedgeLegView {
                        side: "BUY".to_string(),
                        option_type: "PE".to_string(),
                        strike: wing_pe_strike,
                        expiry: Some("2-5 DTE".to_string()),
                        lot_qty,
                        leg_price: (atr * d(32, 2)).round_dp(2),
                        quote_source: "theoretical_atr".to_string(),
                    },
                ],
                net_credit_or_debit: credit,
                max_loss_inr: max_loss,
                max_loss_pct_equity: ((max_loss * Decimal::from(100)) / Decimal::from(500_000))
                    .round_dp(2),
                p95_stress_loss_inr: (max_loss * d(92, 2)).round_dp(2),
                estimated_friction_inr: Decimal::from(240),
                breakevens: vec![be, wing_ce_strike],
                rationale: format!(
                    "Weekly 2-5 DTE bull put credit spread sells {outer_pe_strike} PE strictly outside S1 outer-{buf}xATR shield & caps downside at {wing_pe_strike} PE.",
                    buf = sl_buffer_atr.round_dp(2)
                ),
            }
        }
    }
}

/// Evaluate any strategy/symbol/parameter combination on the deterministic offline fixture.
///
/// Clearly marks `data_source = "offline_seeded_fixture"` so live vs offline runs are
/// always visually distinguishable across TUI and CLI.
pub fn evaluate_offline_strategy_loop(
    cfg: &StrategyLoopConfig,
    previous: Option<&StrategyLoopViewModel>,
) -> StrategyLoopViewModel {
    let mut normalized = cfg.clone();
    if normalized.asset_class.trim().is_empty() {
        normalized.asset_class = infer_asset_class(&normalized.symbol).to_string();
    }
    let is_orb = normalized.strategy_id.to_lowercase().contains("orb");
    if normalized.entry_trigger_mode.eq_ignore_ascii_case("auto") {
        normalized.entry_trigger_mode = if is_orb {
            "orb_breakout".to_string()
        } else {
            "rejection_wick".to_string()
        };
    }

    let candles = generate_fixture_candles(&normalized.symbol, &normalized.timeframe);
    let atr_vec = compute_atr_series(&candles, 14);
    let ema_vec = compute_ema_series(&candles, 20);
    let rsi_vec = compute_rsi_series(&candles, 14);

    let spot_price = candles.last().map(|c| c.close).unwrap_or(Decimal::ZERO);
    let atr = atr_vec.last().copied().unwrap_or(Decimal::from(100));

    let (base_anchor, step, _) = fixture_anchor_and_step(&normalized.symbol);
    let tf_scale = if normalized.timeframe.eq_ignore_ascii_case("5m")
        || normalized.timeframe.eq_ignore_ascii_case("15m")
    {
        d(35, 2)
    } else {
        Decimal::ONE
    };
    let s = step * tf_scale;

    let s1_low = (base_anchor - Decimal::from(10) * s).round_dp(2);
    let s1_high = (base_anchor - Decimal::from(4) * s).round_dp(2);
    let s2_low = (base_anchor - Decimal::from(19) * s).round_dp(2);
    let s2_high = (base_anchor - Decimal::from(13) * s).round_dp(2);
    let r1_low = (base_anchor + Decimal::from(30) * s).round_dp(2);
    let r1_high = (base_anchor + Decimal::from(38) * s).round_dp(2);
    let r2_low = (base_anchor + Decimal::from(42) * s).round_dp(2);
    let r2_high = (base_anchor + Decimal::from(48) * s).round_dp(2);
    let eq_mid = ((s1_high + r1_low) / Decimal::from(2)).round_dp(2);

    // 4-policy stop-hunt benchmark comparison
    let policies: [(&str, &str, Decimal, bool); 4] = [
        ("tight_center", "Tight Center Line", Decimal::ZERO, true),
        ("outer_edge_0", "Band Outer +0.00", Decimal::ZERO, false),
        ("outer_025_atr", "Outer +0.25 ATR", d(25, 2), false),
        ("outer_050_atr", "Outer +0.50 ATR", d(50, 2), false),
    ];

    let mut stop_comparison = Vec::with_capacity(4);
    let mut best_policy_id = "outer_025_atr".to_string();
    let mut best_net_r = Decimal::from(-100);

    for (pid, label, buf, is_tight) in policies {
        let m = evaluate_policy_on_fixture(
            &candles,
            atr,
            s1_low,
            s1_high,
            r1_low,
            r1_high,
            is_orb,
            &normalized.entry_trigger_mode,
            buf,
            is_tight,
            normalized.horizon_bars,
            normalized.target_r,
        );
        if m.net_avg_realized_r > best_net_r {
            best_net_r = m.net_avg_realized_r;
            best_policy_id = pid.to_string();
        }
        stop_comparison.push(StopPolicyTableRow {
            policy_id: pid.to_string(),
            label: label.to_string(),
            buffer_atr: buf,
            trades: m.trades,
            win_rate_pct: m.win_rate_pct,
            stop_out_rate_pct: m.stop_out_rate_pct,
            false_sweep_rate_pct: m.false_sweep_rate_pct,
            avg_realized_r: m.avg_realized_r,
            net_avg_realized_r: m.net_avg_realized_r,
            profit_factor: m.profit_factor,
            sharpe_ratio: m.sharpe_ratio,
            max_drawdown_r: m.max_drawdown_r,
            is_recommended: false,
        });
    }
    for row in &mut stop_comparison {
        row.is_recommended = row.policy_id == best_policy_id;
    }

    // Active evaluation under user's selected `sl_buffer_atr`
    let active_metrics = evaluate_policy_on_fixture(
        &candles,
        atr,
        s1_low,
        s1_high,
        r1_low,
        r1_high,
        is_orb,
        &normalized.entry_trigger_mode,
        normalized.sl_buffer_atr,
        false,
        normalized.horizon_bars,
        normalized.target_r,
    );

    let tight_row = &stop_comparison[0];
    let verdict_summary = format!(
        "Recommended `{best_policy_id}`: trap rate drops from {}% to {}% ({} net R vs {} tight center).",
        tight_row.false_sweep_rate_pct,
        stop_comparison
            .iter()
            .find(|r| r.policy_id == best_policy_id)
            .map(|r| r.false_sweep_rate_pct)
            .unwrap_or(Decimal::ZERO),
        format_signed_r(best_net_r),
        format_signed_r(tight_row.net_avg_realized_r),
    );

    // Parameter sweep (6 candidates across sl_buffer_atr [0.00, 0.25, 0.50] x horizon [5, 10])
    let baseline_r = active_metrics.net_avg_realized_r;
    let mut sweep_rows = Vec::new();
    let grid: [(Decimal, usize, Decimal); 6] = [
        (d(0, 2), 5, Decimal::from(2)),
        (d(25, 2), 5, Decimal::from(3)),
        (d(50, 2), 5, Decimal::from(3)),
        (d(25, 2), 10, Decimal::from(3)),
        (d(50, 2), 10, Decimal::from(3)),
        (d(50, 2), 10, Decimal::from(5)),
    ];
    for (idx, (buf, hb, tr)) in grid.iter().enumerate() {
        let gm = evaluate_policy_on_fixture(
            &candles,
            atr,
            s1_low,
            s1_high,
            r1_low,
            r1_high,
            is_orb,
            &normalized.entry_trigger_mode,
            *buf,
            false,
            *hb,
            *tr,
        );
        let is_sh = gm.sharpe_ratio;
        let oos_sh = (gm.sharpe_ratio * d(92, 2)).round_dp(2);
        let oos_r = (gm.net_avg_realized_r * d(91, 2)).round_dp(2);
        let wfe = if gm.net_avg_realized_r.abs() < d(1, 2) {
            Decimal::ZERO
        } else {
            ((oos_r / gm.net_avg_realized_r) * Decimal::from(100)).round_dp(1)
        };
        let dsr = (oos_sh / Decimal::from(2))
            .max(Decimal::ZERO)
            .min(d(98, 2))
            .round_dp(2);
        sweep_rows.push(SweepLeaderboardRow {
            rank: idx + 1,
            candidate_id: format!("cand-{:02}", idx + 1),
            sl_buffer_atr: *buf,
            horizon_bars: *hb,
            target_r: *tr,
            is_sharpe: is_sh,
            oos_sharpe: oos_sh,
            oos_avg_r: oos_r,
            wfe_pct: wfe,
            dsr,
            stop_out_rate_pct: gm.stop_out_rate_pct,
            delta_vs_baseline_r: (gm.net_avg_realized_r - baseline_r).round_dp(2),
            is_winner: false,
        });
    }
    sweep_rows.sort_by_key(|b| std::cmp::Reverse(b.oos_avg_r));
    for (i, row) in sweep_rows.iter_mut().enumerate() {
        row.rank = i + 1;
        row.is_winner = i == 0;
    }

    // Penetration distribution histogram
    let penetration = build_penetration_histogram(
        18,
        d(778, 1),
        d(14, 2),
        d(24, 2),
        d(38, 2),
        d(46, 2),
        d(62, 2),
        16,
    );

    // Build all 4 expiry hedges + selected primary hedge card
    let horizons = ["day", "weekly", "monthly", "commodity_cycle"];
    let all_horizon_hedges: Vec<HedgeCardView> = horizons
        .iter()
        .map(|h| {
            build_hedge_card_for_horizon(
                &normalized.symbol,
                &normalized.asset_class,
                h,
                spot_price,
                atr,
                s1_low,
                r1_high,
                normalized.sl_buffer_atr,
            )
        })
        .collect();
    let hedge_card = build_hedge_card_for_horizon(
        &normalized.symbol,
        &normalized.asset_class,
        &normalized.expiry_horizon,
        spot_price,
        atr,
        s1_low,
        r1_high,
        normalized.sl_buffer_atr,
    );

    // Build full 6-layer OverlayScene aligned 1:1 with `candles`
    let mut scene = OverlayScene::new(
        &normalized.strategy_id,
        &normalized.symbol,
        &normalized.timeframe,
    );
    if let Some(prev) = previous {
        scene.layers = prev.scene.layers;
    }

    let last_bar = candles.len().saturating_sub(1);
    if let Ok(b) = ZoneBand::new(s1_low, s1_high, 0, last_bar) {
        scene.bands.push(
            b.with_fill_index(3)
                .with_label(if is_orb { "OR_LOW" } else { "S1" }),
        );
    }
    if let Ok(b) = ZoneBand::new(r1_low, r1_high, 0, last_bar) {
        scene.bands.push(
            b.with_fill_index(4)
                .with_label(if is_orb { "OR_HIGH" } else { "R1" }),
        );
    }
    if !is_orb {
        if let Ok(b) = ZoneBand::new(s2_low, s2_high, 0, last_bar) {
            scene.bands.push(b.with_fill_index(2).with_label("S2"));
        }
        if let Ok(b) = ZoneBand::new(r2_low, r2_high, 0, last_bar) {
            scene.bands.push(b.with_fill_index(1).with_label("R2"));
        }
    }
    if let Ok(lvl) = LevelLine::new(eq_mid, 0, last_bar) {
        scene.levels.push(
            lvl.with_label(if is_orb { "OR_MID" } else { "Equilibrium" })
                .with_style(LevelLineStyle {
                    dashed: true,
                    color_index: 1,
                    line_width: 1,
                }),
        );
    }

    // Signals + Connectors + Stops + Hints reflecting active `sl_buffer_atr`
    let sig1_entry = candles[6].close;
    let tight_sl1 = ((s1_low + s1_high) / Decimal::from(2)).round_dp(2);
    let outer_sl1 = (s1_low - normalized.sl_buffer_atr * atr).round_dp(2);
    let sl1_survived = normalized.sl_buffer_atr >= d(22, 2);
    let t3r_1 = (sig1_entry + (sig1_entry - outer_sl1).abs() * Decimal::from(3)).round_dp(2);

    scene.signals.push(SignalMarker::new(
        sig1_entry,
        6,
        MarkerSide::Buy,
        if sl1_survived {
            MarkerKind::SweepSaved
        } else {
            MarkerKind::Sweep
        },
    ));
    scene.signals.push(SignalMarker::new(
        candles[15].close,
        15,
        MarkerSide::Sell,
        MarkerKind::Reversal,
    ));
    scene.signals.push(SignalMarker::new(
        candles[24].close,
        24,
        MarkerSide::Buy,
        if normalized.sl_buffer_atr >= d(40, 2) {
            MarkerKind::SweepSaved
        } else {
            MarkerKind::Retest
        },
    ));
    scene.signals.push(SignalMarker::new(
        candles[25].close,
        25,
        MarkerSide::Buy,
        MarkerKind::ElephantBar,
    ));

    if let Ok(ray) = TradeConnectorRay::new(
        6,
        sig1_entry,
        12,
        if sl1_survived {
            t3r_1.min(candles[12].close)
        } else {
            outer_sl1
        },
        MarkerSide::Buy,
        if sl1_survived {
            d(24, 1)
        } else {
            Decimal::from(-1)
        },
        !sl1_survived,
    ) {
        scene.connectors.push(ray.with_label(if sl1_survived {
            "+2.4R Survived"
        } else {
            "Stopped (-1R)"
        }));
    }
    if let Ok(ray2) = TradeConnectorRay::new(
        15,
        candles[15].close,
        21,
        candles[21].close,
        MarkerSide::Sell,
        d(21, 1),
        false,
    ) {
        scene.connectors.push(ray2.with_label("+2.1R R1 Reversal"));
    }

    if let Ok(mut st) = StopLine::new(tight_sl1, 6, 15, StopKind::Initial) {
        let _ = st.update(outer_sl1);
        scene.stops.push(st);
    }
    if let Ok(tgt) = StopLine::new(t3r_1, 6, 15, StopKind::Target) {
        scene.stops.push(tgt);
    }
    let outer_r1_sl = (r1_high + normalized.sl_buffer_atr * atr).round_dp(2);
    if let Ok(mut st2) = StopLine::new(
        ((r1_low + r1_high) / Decimal::from(2)).round_dp(2),
        15,
        24,
        StopKind::Initial,
    ) {
        let _ = st2.update(outer_r1_sl);
        scene.stops.push(st2);
    }

    // Indicators: EMA(20), RSI(14), ATR(14)
    let to_pts = |vals: &[Decimal]| -> Vec<ChartPoint> {
        vals.iter()
            .enumerate()
            .map(|(i, y)| ChartPoint::new(Decimal::from(i as i64), *y))
            .collect()
    };
    scene
        .indicators
        .push(ChartSeries::new("EMA(20)", to_pts(&ema_vec)));
    scene
        .indicators
        .push(ChartSeries::new("RSI(14)", to_pts(&rsi_vec)));
    scene
        .indicators
        .push(ChartSeries::new("ATR(14)", to_pts(&atr_vec)));

    // Hints
    let mut hint1 = HintAnnotation::new(
        6,
        sig1_entry,
        if sl1_survived {
            HintSeverity::Positive
        } else {
            HintSeverity::Warning
        },
        if sl1_survived {
            "Sweep Saved by Outer Buffer"
        } else {
            "Tight Stop Swept Before Reclaim"
        },
        if sl1_survived {
            format!(
                "Bar #6 wicked 0.22x ATR below S1 outer edge, hunting tight center stops ({tight_sl1}), while outer +{}x ATR shield ({outer_sl1}) survived for +2.4R rally.",
                normalized.sl_buffer_atr.round_dp(2)
            )
        } else {
            format!(
                "Current SL buffer (+{}x ATR = {outer_sl1}) was pierced by the 0.22x ATR liquidity sweep on bar #6; increasing buffer to +0.25..+0.50 ATR avoids the trap.",
                normalized.sl_buffer_atr.round_dp(2)
            )
        },
    )
    .expect("valid hint");
    hint1 = hint1
        .with_metric("Penetration", "0.22x ATR")
        .with_metric("Tight SL", tight_sl1.to_string())
        .with_metric("Outer SL", outer_sl1.to_string())
        .with_metric("Outcome", if sl1_survived { "+2.40R" } else { "-1.00R" });
    scene.hints.push(hint1);

    let hint2 = HintAnnotation::new(
        15,
        candles[15].close,
        HintSeverity::Positive,
        "R1 Liquidity Grab Short",
        "Price spiked above R1 high and closed back inside zone with rejection wick; outer R1 shield held cleanly.",
    )
    .expect("valid hint")
    .with_metric("Entry Mode", normalized.entry_trigger_mode.clone())
    .with_metric("R1 Shield", outer_r1_sl.to_string())
    .with_metric("Realized", "+2.10R");
    scene.hints.push(hint2);

    let hint3 = HintAnnotation::new(
        25,
        candles[25].close,
        HintSeverity::Info,
        "Elephant Momentum Follow-Through",
        "High-range institutional expansion bar (range >= 1.5x ATR) confirmed S1 reclaim.",
    )
    .expect("valid hint")
    .with_metric("ATR(14)", atr.to_string())
    .with_metric("Horizon", format!("{} bars", normalized.horizon_bars));
    scene.hints.push(hint3);

    // Hedges on Layer 6 matching selected `hedge_card`
    for leg in &hedge_card.legs {
        if leg.strike <= Decimal::ZERO {
            continue;
        }
        let side = if leg.side.eq_ignore_ascii_case("BUY") {
            HedgeLegSide::Buy
        } else {
            HedgeLegSide::Sell
        };
        let otype = match leg.option_type.as_str() {
            "CE" => HedgeOptionKind::Call,
            "PE" => HedgeOptionKind::Put,
            "FUT" => HedgeOptionKind::Future,
            _ => HedgeOptionKind::Spot,
        };
        if let Ok(mut overlay) = HedgeLevelOverlay::new(
            hedge_card.structure.clone(),
            side,
            otype,
            leg.strike,
            0,
            last_bar,
        ) {
            if let Some(be) = hedge_card.breakevens.first().copied() {
                overlay = overlay.with_breakeven(be);
            }
            overlay = overlay
                .with_max_loss(hedge_card.max_loss_inr)
                .with_summary(format!("{} {}", leg.side, leg.option_type));
            scene.hedges.push(overlay);
        }
    }

    let projected = project_overlay_scene(&candles, &scene);
    let hints_view: Vec<HintInspectorItem> = scene
        .hints
        .iter()
        .enumerate()
        .map(|(i, h)| HintInspectorItem {
            index: i,
            bar_index: h.bar_index,
            price: h.price,
            severity: h.severity,
            title: h.title.clone(),
            detail: h.detail.clone(),
            metrics: h.metrics.clone(),
        })
        .collect();

    let delta_summary = previous.map(|prev| LoopDeltaSummary {
        label: format!(
            "vs run #{} (sl={}x ATR)",
            prev.run_seq,
            prev.config.sl_buffer_atr.round_dp(2)
        ),
        avg_r_delta: (active_metrics.net_avg_realized_r - prev.net_avg_realized_r).round_dp(2),
        win_rate_pct_delta: (active_metrics.win_rate_pct - prev.win_rate_pct).round_dp(1),
        stop_out_rate_pct_delta: (active_metrics.stop_out_rate_pct - prev.stop_out_rate_pct)
            .round_dp(1),
        trap_rate_pct_delta: (active_metrics.false_sweep_rate_pct - prev.false_sweep_rate_pct)
            .round_dp(1),
        sharpe_delta: (active_metrics.sharpe_ratio - prev.sharpe_ratio).round_dp(2),
    });

    StrategyLoopViewModel {
        data_source: OFFLINE_SEEDED_FIXTURE_SOURCE.to_string(),
        session_id: previous
            .map(|p| p.session_id.clone())
            .unwrap_or_else(|| "offline-loop-001".to_string()),
        run_seq: previous.map(|p| p.run_seq + 1).unwrap_or(1),
        config: normalized,
        spot_price,
        atr,
        total_trades: active_metrics.trades,
        win_rate_pct: active_metrics.win_rate_pct,
        stop_out_rate_pct: active_metrics.stop_out_rate_pct,
        false_sweep_rate_pct: active_metrics.false_sweep_rate_pct,
        net_avg_realized_r: active_metrics.net_avg_realized_r,
        profit_factor: active_metrics.profit_factor,
        sharpe_ratio: active_metrics.sharpe_ratio,
        max_drawdown_r: active_metrics.max_drawdown_r,
        verdict_summary,
        candles,
        scene,
        projected,
        stop_comparison,
        recommended_policy_id: best_policy_id,
        penetration,
        sweep_rows,
        delta_summary,
        hedge_card,
        all_horizon_hedges,
        hints: hints_view,
    }
}
