//! Unified 6-layer interactive graphical scene primitives (`OverlayScene`).
//!
//! Covers all six grouped, toggleable visual layers rendered identically across
//! Web Canvas, TUI `ratatui::widgets::Canvas`, and CLI JSON/SVG exports:
//! 1. `signals` (`SignalMarker` + `TradeConnectorRay`)
//! 2. `bands` (`ZoneBand` + `LevelLine`)
//! 3. `stops` (`StopLine`)
//! 4. `indicators` (`ChartSeries`)
//! 5. `hints` (`HintAnnotation`)
//! 6. `hedges` (`HedgeLevelOverlay`)

use rust_decimal::Decimal;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::axis::ChartRange;
use crate::candle::GeometryError;
use crate::levels::{LevelLine, price_to_row};
use crate::markers::{MarkerSide, SignalMarker};
use crate::series::ChartSeries;
use crate::stops::StopLine;
use crate::zones::ZoneBand;

/// Six top-level primitive layer groups in an [`OverlayScene`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum SceneLayerGroup {
    /// Buy/Sell/Sweep markers and entry-to-exit connector rays.
    Signals,
    /// Contextual zone boxes (`S1`/`S2`/`R1`/`R2`, `ORB`, `OB`) and level lines.
    Bands,
    /// Tight/Outer/Trailing/Breakeven stops and R-multiple target rays.
    Stops,
    /// Overlay and sub-pane indicator series.
    Indicators,
    /// Clickable/hoverable rationale pins explaining why a trade won/lost.
    Hints,
    /// F&O option strike lines, payoff breakeven, and hedge role overlays.
    Hedges,
}

/// Toggle mask for the six [`SceneLayerGroup`]s.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct SceneLayerMask {
    /// Layer 1: Buy/Sell/Sweep signals & trade connectors.
    pub signals: bool,
    /// Layer 2: Contextual zone bands & equilibrium/pivot levels.
    pub bands: bool,
    /// Layer 3: Stops & R-multiple targets.
    pub stops: bool,
    /// Layer 4: Overlay/sub-pane indicator series.
    pub indicators: bool,
    /// Layer 5: Interactive rationale hint annotations.
    pub hints: bool,
    /// Layer 6: Hedge strike levels & breakeven overlays.
    pub hedges: bool,
}

impl Default for SceneLayerMask {
    fn default() -> Self {
        Self::all()
    }
}

impl SceneLayerMask {
    /// Enable all six primitive layers.
    pub const fn all() -> Self {
        Self {
            signals: true,
            bands: true,
            stops: true,
            indicators: true,
            hints: true,
            hedges: true,
        }
    }

    /// Disable all six primitive layers.
    pub const fn none() -> Self {
        Self {
            signals: false,
            bands: false,
            stops: false,
            indicators: false,
            hints: false,
            hedges: false,
        }
    }

    /// Check visibility of a layer group.
    pub const fn is_visible(&self, group: SceneLayerGroup) -> bool {
        match group {
            SceneLayerGroup::Signals => self.signals,
            SceneLayerGroup::Bands => self.bands,
            SceneLayerGroup::Stops => self.stops,
            SceneLayerGroup::Indicators => self.indicators,
            SceneLayerGroup::Hints => self.hints,
            SceneLayerGroup::Hedges => self.hedges,
        }
    }

    /// Set visibility of a layer group.
    pub fn set_visible(&mut self, group: SceneLayerGroup, visible: bool) {
        match group {
            SceneLayerGroup::Signals => self.signals = visible,
            SceneLayerGroup::Bands => self.bands = visible,
            SceneLayerGroup::Stops => self.stops = visible,
            SceneLayerGroup::Indicators => self.indicators = visible,
            SceneLayerGroup::Hints => self.hints = visible,
            SceneLayerGroup::Hedges => self.hedges = visible,
        }
    }

    /// Flip visibility of a layer group and return the new state.
    pub fn toggle(&mut self, group: SceneLayerGroup) -> bool {
        let next = !self.is_visible(group);
        self.set_visible(group, next);
        next
    }
}

/// Pixel `(x, y)` anchor coordinate.
pub type AnchorPoint = (u16, u16);

/// Entry-to-exit `(start, end)` pixel anchor pair.
pub type ConnectorAnchors = (AnchorPoint, AnchorPoint);

/// Visual severity for an interactive [`HintAnnotation`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum HintSeverity {
    /// Neutral informational note.
    Info,
    /// Beneficial save / alpha edge (e.g. sweep survived by outer SL).
    Positive,
    /// Caution / tighter-stop vulnerability warning.
    Warning,
    /// Critical risk / split jump / trap alert.
    Critical,
}

/// Key-value metric attached to a [`HintAnnotation`] card.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct HintMetric {
    /// Display label (e.g. `"Penetration"`).
    pub key: String,
    /// Formatted value (e.g. `"0.18x ATR"`).
    pub value: String,
}

/// Clickable / hoverable rationale pin anchored to a candle and price.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct HintAnnotation {
    /// 0-based candle index anchoring x coordinate.
    pub bar_index: usize,
    /// Non-negative anchor price.
    pub price: Decimal,
    /// Visual severity badge.
    pub severity: HintSeverity,
    /// Short pin title (e.g. `"Sweep Saved (+2.4R)"`).
    pub title: String,
    /// Detailed human-readable rationale.
    pub detail: String,
    /// Structured metrics surfaced inside the inspector popover/status strip.
    #[serde(default)]
    pub metrics: Vec<HintMetric>,
}

impl HintAnnotation {
    /// Construct a validated hint annotation (`price >= 0`).
    pub fn new(
        bar_index: usize,
        price: Decimal,
        severity: HintSeverity,
        title: impl Into<String>,
        detail: impl Into<String>,
    ) -> Result<Self, GeometryError> {
        if price < Decimal::ZERO {
            return Err(GeometryError::InvalidGeometry);
        }
        Ok(Self {
            bar_index,
            price,
            severity,
            title: title.into(),
            detail: detail.into(),
            metrics: Vec::new(),
        })
    }

    /// Append a structured key-value metric pill.
    pub fn with_metric(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.metrics.push(HintMetric {
            key: key.into(),
            value: value.into(),
        });
        self
    }

    /// Pixel `(x, y)` anchor matching [`SignalMarker::anchor`].
    pub fn anchor(
        &self,
        range: &ChartRange,
        height: u16,
        candle_width: u16,
    ) -> Result<(u16, u16), GeometryError> {
        let x = u16::try_from(self.bar_index.saturating_mul(usize::from(candle_width)))
            .unwrap_or(u16::MAX);
        let y = price_to_row(self.price, range, height)?;
        Ok((x, y))
    }
}

/// Entry-to-exit connector ray showing realized outcome of a simulated trade.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct TradeConnectorRay {
    /// Entry bar index (`<= to_index`).
    pub from_index: usize,
    /// Entry price (`>= 0`).
    pub from_price: Decimal,
    /// Exit bar index (`>= from_index`).
    pub to_index: usize,
    /// Exit price (`>= 0`).
    pub to_price: Decimal,
    /// Direction of trade (`Buy` or `Sell`).
    pub side: MarkerSide,
    /// Realized outcome in `R` multiples.
    pub realized_r: Decimal,
    /// Whether the trade exited via stop-loss.
    pub stopped_out: bool,
    /// Optional short label (e.g. `"+3R Target"` or `"Stopped (-1R)"`).
    pub label: Option<String>,
}

impl TradeConnectorRay {
    /// Create a validated connector ray (`to_index >= from_index`, non-negative prices).
    pub fn new(
        from_index: usize,
        from_price: Decimal,
        to_index: usize,
        to_price: Decimal,
        side: MarkerSide,
        realized_r: Decimal,
        stopped_out: bool,
    ) -> Result<Self, GeometryError> {
        if to_index < from_index || from_price < Decimal::ZERO || to_price < Decimal::ZERO {
            return Err(GeometryError::InvalidGeometry);
        }
        Ok(Self {
            from_index,
            from_price,
            to_index,
            to_price,
            side,
            realized_r,
            stopped_out,
            label: None,
        })
    }

    /// Attach an outcome badge label.
    pub fn with_label(mut self, label: impl Into<String>) -> Self {
        self.label = Some(label.into());
        self
    }

    /// Pixel `((x0, y0), (x1, y1))` endpoint anchors for this connector ray.
    pub fn anchors(
        &self,
        range: &ChartRange,
        height: u16,
        candle_width: u16,
    ) -> Result<ConnectorAnchors, GeometryError> {
        let cw = usize::from(candle_width);
        let x0 = u16::try_from(self.from_index.saturating_mul(cw)).unwrap_or(u16::MAX);
        let x1 = u16::try_from(self.to_index.saturating_mul(cw)).unwrap_or(u16::MAX);
        let y0 = price_to_row(self.from_price, range, height)?;
        let y1 = price_to_row(self.to_price, range, height)?;
        Ok(((x0, y0), (x1, y1)))
    }
}

/// Leg action for an option/futures hedge overlay.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum HedgeLegSide {
    /// Long leg.
    Buy,
    /// Short leg.
    Sell,
}

/// Instrument type for a hedge leg strike overlay.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum HedgeOptionKind {
    /// Cash / Spot instrument.
    Spot,
    /// Futures contract.
    Future,
    /// Call option (`CE`).
    Call,
    /// Put option (`PE`).
    Put,
}

/// Strike & payoff ray rendered on the chart for synthesized F&O hedges.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct HedgeLevelOverlay {
    /// Human-readable role (e.g. `"Short OTM Put"`, `"Protective Wing PE"`).
    pub role: String,
    /// Buy or Sell leg.
    pub side: HedgeLegSide,
    /// Option/Future kind.
    pub option_kind: HedgeOptionKind,
    /// Positive strike or entry reference price.
    pub strike: Decimal,
    /// Left bar/pixel span (`<= x1`).
    pub x0: usize,
    /// Right bar/pixel span (`>= x0`).
    pub x1: usize,
    /// Optional breakeven price for right-axis shaded reference.
    pub breakeven: Option<Decimal>,
    /// Optional worst-case rupee/currency loss cap for summary badge.
    pub max_loss: Option<Decimal>,
    /// Optional summary tag rendered near the strike ray.
    pub summary: Option<String>,
}

impl HedgeLevelOverlay {
    /// Create a validated hedge level overlay (`strike > 0`, `x1 >= x0`).
    pub fn new(
        role: impl Into<String>,
        side: HedgeLegSide,
        option_kind: HedgeOptionKind,
        strike: Decimal,
        x0: usize,
        x1: usize,
    ) -> Result<Self, GeometryError> {
        if strike <= Decimal::ZERO || x1 < x0 {
            return Err(GeometryError::InvalidGeometry);
        }
        Ok(Self {
            role: role.into(),
            side,
            option_kind,
            strike,
            x0,
            x1,
            breakeven: None,
            max_loss: None,
            summary: None,
        })
    }

    /// Attach a breakeven reference price.
    pub fn with_breakeven(mut self, breakeven: Decimal) -> Self {
        self.breakeven = Some(breakeven);
        self
    }

    /// Attach max defined loss for the structure.
    pub fn with_max_loss(mut self, max_loss: Decimal) -> Self {
        self.max_loss = Some(max_loss);
        self
    }

    /// Attach a display summary tag.
    pub fn with_summary(mut self, summary: impl Into<String>) -> Self {
        self.summary = Some(summary.into());
        self
    }

    /// Pixel row for the strike within `[0, height)`.
    pub fn y_from_range(&self, range: &ChartRange, height: u16) -> Result<u16, GeometryError> {
        price_to_row(self.strike, range, height)
    }
}

/// Element counts per [`SceneLayerGroup`] for HUD badges and parity tests.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize, JsonSchema)]
pub struct SceneLayerCounts {
    /// Signals (`SignalMarker` count; connectors follow signals).
    pub signals: usize,
    /// Bands (`ZoneBand` count + `LevelLine` count).
    pub bands: usize,
    /// Stops (`StopLine` count).
    pub stops: usize,
    /// Indicators (`ChartSeries` count).
    pub indicators: usize,
    /// Hints (`HintAnnotation` count).
    pub hints: usize,
    /// Hedges (`HedgeLevelOverlay` count).
    pub hedges: usize,
}

/// Unified 6-layer interactive overlay scene shared across Web, TUI, and CLI.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct OverlayScene {
    /// Strategy identifier (e.g. `"elephant_edge_v3"`, `"orb_15"`).
    pub strategy_id: String,
    /// Instrument symbol (e.g. `"NSE:NIFTY50"`).
    pub symbol: String,
    /// Timeframe label (e.g. `"5m"`, `"1D"`).
    pub timeframe: String,
    /// Active visibility mask for the six layer groups.
    #[serde(default)]
    pub layers: SceneLayerMask,
    /// Layer 1a: Buy/Sell/Sweep/Target markers.
    #[serde(default)]
    pub signals: Vec<SignalMarker>,
    /// Layer 1b: Entry-to-exit connector rays.
    #[serde(default)]
    pub connectors: Vec<TradeConnectorRay>,
    /// Layer 2a: Contextual zone boxes.
    #[serde(default)]
    pub bands: Vec<ZoneBand>,
    /// Layer 2b: Dotted Equilibrium / Pivot / VWAP level lines.
    #[serde(default)]
    pub levels: Vec<LevelLine>,
    /// Layer 3: Initial, Outer-Buffer, Trailing, Breakeven stops & targets.
    #[serde(default)]
    pub stops: Vec<StopLine>,
    /// Layer 4: Indicator series (overlay + sub-pane).
    #[serde(default)]
    pub indicators: Vec<ChartSeries>,
    /// Layer 5: Clickable/hoverable hint rationale pins.
    #[serde(default)]
    pub hints: Vec<HintAnnotation>,
    /// Layer 6: F&O / Commodity hedge strike overlays.
    #[serde(default)]
    pub hedges: Vec<HedgeLevelOverlay>,
}

impl OverlayScene {
    /// Construct an empty scene with all six layers visible by default.
    pub fn new(
        strategy_id: impl Into<String>,
        symbol: impl Into<String>,
        timeframe: impl Into<String>,
    ) -> Self {
        Self {
            strategy_id: strategy_id.into(),
            symbol: symbol.into(),
            timeframe: timeframe.into(),
            layers: SceneLayerMask::all(),
            signals: Vec::new(),
            connectors: Vec::new(),
            bands: Vec::new(),
            levels: Vec::new(),
            stops: Vec::new(),
            indicators: Vec::new(),
            hints: Vec::new(),
            hedges: Vec::new(),
        }
    }

    /// Return counts per primitive group across populated vectors.
    pub fn layer_counts(&self) -> SceneLayerCounts {
        SceneLayerCounts {
            signals: self.signals.len(),
            bands: self.bands.len() + self.levels.len(),
            stops: self.stops.len(),
            indicators: self.indicators.len(),
            hints: self.hints.len(),
            hedges: self.hedges.len(),
        }
    }

    /// Return a shallow copy containing only layers enabled in [`Self::layers`].
    pub fn filtered_visible(&self) -> Self {
        Self {
            strategy_id: self.strategy_id.clone(),
            symbol: self.symbol.clone(),
            timeframe: self.timeframe.clone(),
            layers: self.layers,
            signals: if self.layers.signals {
                self.signals.clone()
            } else {
                Vec::new()
            },
            connectors: if self.layers.signals {
                self.connectors.clone()
            } else {
                Vec::new()
            },
            bands: if self.layers.bands {
                self.bands.clone()
            } else {
                Vec::new()
            },
            levels: if self.layers.bands {
                self.levels.clone()
            } else {
                Vec::new()
            },
            stops: if self.layers.stops {
                self.stops.clone()
            } else {
                Vec::new()
            },
            indicators: if self.layers.indicators {
                self.indicators.clone()
            } else {
                Vec::new()
            },
            hints: if self.layers.hints {
                self.hints.clone()
            } else {
                Vec::new()
            },
            hedges: if self.layers.hedges {
                self.hedges.clone()
            } else {
                Vec::new()
            },
        }
    }

    /// Compute the inclusive y-range spanning all currently populated scene primitives.
    pub fn y_range(&self) -> Option<ChartRange> {
        let mut min: Option<Decimal> = None;
        let mut max: Option<Decimal> = None;
        let mut include = |p: Decimal| {
            min = Some(min.map_or(p, |m| m.min(p)));
            max = Some(max.map_or(p, |m| m.max(p)));
        };

        for s in &self.signals {
            include(s.price);
        }
        for c in &self.connectors {
            include(c.from_price);
            include(c.to_price);
        }
        for b in &self.bands {
            include(b.low);
            include(b.high);
        }
        for l in &self.levels {
            include(l.price);
        }
        for st in &self.stops {
            include(st.price);
        }
        for series in &self.indicators {
            for pt in &series.points {
                include(pt.y);
            }
        }
        for h in &self.hints {
            include(h.price);
        }
        for hedge in &self.hedges {
            include(hedge.strike);
        }

        match (min, max) {
            (Some(lo), Some(hi)) => Some(ChartRange::new(lo, hi)),
            _ => None,
        }
    }
}
