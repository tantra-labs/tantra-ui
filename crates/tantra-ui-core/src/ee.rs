//! EE+ (Elephant Edge) Strategy Studio pure view-model helpers (ui plan 0006,
//! Phases 2–4).
//!
//! This is the **single shared source** for the EE+ level-map / signals /
//! positioning geometry consumed by all three workstation surfaces:
//! - the Leptos CSR web view layer (`tantra-ui-app/src/studio_ee.rs`),
//! - the Ratatui TUI positioning panel (`tantra-tui/src/positioning.rs`),
//! - the `tantra elephant` CLI report (`tantra-cli/src/elephant.rs`).
//!
//! Everything here is WASM-clean, zero-I/O, and `Decimal`-exact: the helpers
//! build the same `tantra_ui_charts` geometry (`LevelLine`, `ZoneBand`,
//! `SignalMarker`, `StopLine`) the workstation canvas would draw, seeded from
//! the [`ee_seed_level_map`] / [`ee_seed_signals`] /
//! [`ee_seed_positions`] fixtures until the upstream `tantra-runtime-api-rest`
//! endpoints (`GET /level-map/{date}`, `GET /signals/{date}`) exist (plan
//! 0006 §2 — surfaces render an honest `unavailable` note meanwhile).
//!
//! ## Seeded entry / 1R convention
//!
//! A [`StrategySignal`] wire type carries no entry price; the seeded helpers
//! derive one deterministically so tests can assert exact Decimal strings:
//! the entry is the **referenced level price**, encoded in the seed's
//! [`LevelId`] (`entry = Decimal::from(ref_levels[0].0)`), and **1R is 1% of
//! the entry** (`entry × 0.01`). Stops are then `entry ∓ 1R` and targets
//! `entry ± 3R` / `entry ± 5R`, long vs short mirrored. This is a seeded
//! stand-in for the live stop/IV-derived 1R until plan 0006 §1.2 lands.

use rust_decimal::Decimal;
use tantra_core::levels::{LevelId, LevelKind, LevelMap};
use tantra_core::order::OrderSide;
use tantra_core::signals::{PositionIntent, SignalKind, StrategySignal};
use tantra_core::types::{Exchange, InstrumentId, Money, Quantity};
use tantra_ui_charts::ChartRange;
use tantra_ui_charts::levels::{LevelLine, LevelLineStyle};
use tantra_ui_charts::markers::{MarkerKind, MarkerSide, SignalMarker};
use tantra_ui_charts::stops::{StopKind, StopLine};
use tantra_ui_charts::zones::ZoneBand;

/// Labels for the three stop/target lines [`studio_ee_stops`] emits per
/// signal, in output order: `["SL 1R", "T 3R", "T 5R"]`.
pub const EE_STOP_LABELS: [&str; 3] = ["SL 1R", "T 3R", "T 5R"];

/// A chart viewport for EE geometry: the price range plus pixel dimensions.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StudioViewport {
    /// Inclusive price range spanned by the chart.
    pub range: ChartRange,
    /// Chart width in pixels (full-span lines run `0..=width - 1`).
    pub width: usize,
    /// Chart height in pixels (used for y anchoring).
    pub height: u16,
}

impl StudioViewport {
    /// Create a viewport from exact price bounds and pixel dimensions.
    pub fn new(min: Decimal, max: Decimal, width: usize, height: u16) -> Self {
        Self {
            range: ChartRange::new(min, max),
            width,
            height,
        }
    }
}

/// The elephant band order rendered by the Studio: S1, S2, R1, R2.
const EE_BAND_ORDER: [(&str, LevelKind); 4] = [
    ("S1", LevelKind::ElephantSupport1),
    ("S2", LevelKind::ElephantSupport2),
    ("R1", LevelKind::ElephantResistance1),
    ("R2", LevelKind::ElephantResistance2),
];

/// The two support (`S1`, `S2`) + two resistance (`R1`, `R2`) elephant bands
/// on the map as shaded [`ZoneBand`]s, labelled and re-coloured by confluence
/// score (`fill_index = score`, 0–4).
pub fn studio_ee_bands(map: &LevelMap, viewport: &StudioViewport) -> Vec<ZoneBand> {
    let x1 = viewport.width.saturating_sub(1);
    let mut bands = Vec::with_capacity(EE_BAND_ORDER.len());
    for (label, kind) in EE_BAND_ORDER {
        if let Some(level) = map.levels.iter().find(|l| l.kind == kind) {
            if let Ok(band) = ZoneBand::new(level.band.0, level.band.1, 0, x1) {
                bands.push(band.with_fill_index(level.score).with_label(label));
            }
        }
    }
    bands
}

/// The edge [`LevelLine`]s for the elephant bands: one per band at its
/// representative (mid) price, `color_index = score` (0–4) and dashed for the
/// resistance bands.
pub fn studio_ee_lines(map: &LevelMap, viewport: &StudioViewport) -> Vec<LevelLine> {
    let x1 = viewport.width.saturating_sub(1);
    let mut lines = Vec::with_capacity(EE_BAND_ORDER.len());
    for (label, kind) in EE_BAND_ORDER {
        if let Some(level) = map.levels.iter().find(|l| l.kind == kind) {
            let mid = (level.band.0 + level.band.1) / Decimal::from(2);
            let style = LevelLineStyle {
                dashed: matches!(
                    kind,
                    LevelKind::ElephantResistance1 | LevelKind::ElephantResistance2
                ),
                color_index: level.score,
                line_width: 1,
            };
            if let Ok(line) = LevelLine::new(mid, 0, x1) {
                lines.push(line.with_label(label).with_style(style));
            }
        }
    }
    lines
}

/// The deterministic entry price of a seeded signal: the referenced level's
/// price, encoded in its [`LevelId`] (see the module docs).
fn signal_entry(signal: &StrategySignal) -> Decimal {
    Decimal::from(signal.ref_levels.first().map(|id| id.0).unwrap_or(0))
}

/// The seeded 1R risk distance for a signal entry: 1% of the entry.
fn signal_risk_r(entry: Decimal) -> Decimal {
    entry * Decimal::new(1, 2)
}

/// One [`SignalMarker`] per signal: Buy→[`MarkerSide::Buy`],
/// Sell→[`MarkerSide::Sell`], `SignalKind` mapped onto [`MarkerKind`], at the
/// referenced level price. The candle `index` is a deterministic seed position
/// (`n × 5 + 2` for the `n`-th signal).
pub fn studio_ee_marks(
    signals: &[StrategySignal],
    _viewport: &StudioViewport,
) -> Vec<SignalMarker> {
    signals
        .iter()
        .enumerate()
        .map(|(n, s)| {
            SignalMarker::new(
                signal_entry(s),
                n * 5 + 2,
                match s.side {
                    OrderSide::Buy => MarkerSide::Buy,
                    OrderSide::Sell => MarkerSide::Sell,
                },
                match s.kind {
                    SignalKind::Breakout => MarkerKind::Breakout,
                    SignalKind::Reversal => MarkerKind::Reversal,
                    SignalKind::Retest => MarkerKind::Retest,
                },
            )
        })
        .collect()
}

/// Three [`StopLine`]s per signal, in `[SL 1R, T 3R, T 5R]` order (labels in
/// [`EE_STOP_LABELS`]): the initial 1R stop below (long) / above (short) the
/// entry plus the 3R/5R take-profit target lines. Long targets run above the
/// entry, short targets below it.
pub fn studio_ee_stops(signals: &[StrategySignal], viewport: &StudioViewport) -> Vec<StopLine> {
    let x1 = viewport.width.saturating_sub(1);
    let three = Decimal::new(3, 0);
    let five = Decimal::new(5, 0);
    let mut stops = Vec::with_capacity(signals.len() * 3);
    for signal in signals {
        let entry = signal_entry(signal);
        let r = signal_risk_r(entry);
        if entry <= Decimal::ZERO {
            continue;
        }
        let (sl, t3, t5) = match signal.side {
            OrderSide::Buy => (entry - r, entry + r * three, entry + r * five),
            OrderSide::Sell => (entry + r, entry - r * three, entry - r * five),
        };
        if let Ok(sl_line) = StopLine::new(sl, 0, x1, StopKind::Initial) {
            stops.push(sl_line);
        }
        if let Ok(t3_line) = StopLine::new(t3, 0, x1, StopKind::Target) {
            stops.push(t3_line);
        }
        if let Ok(t5_line) = StopLine::new(t5, 0, x1, StopKind::Target) {
            stops.push(t5_line);
        }
    }
    stops
}

/// The regime chip for the session: `"long_gamma"` (pin / mean-revert) when
/// spot sits at or above the gamma-flip level, `"short_gamma"` (trend /
/// breakout) below it, or honestly `"neutral"` when the map carries no gamma
/// flip level.
pub fn studio_ee_regime(map: &LevelMap, last_price: Decimal) -> (String, &'static str) {
    match map.levels.iter().find(|l| l.kind == LevelKind::GammaFlip) {
        Some(flip) => {
            let flip_price = (flip.band.0 + flip.band.1) / Decimal::from(2);
            if last_price >= flip_price {
                ("long_gamma".to_string(), "pin / mean-revert")
            } else {
                ("short_gamma".to_string(), "trend / breakout")
            }
        }
        None => ("neutral".to_string(), "no gamma flip level on the map"),
    }
}

/// Side label for an [`OrderSide`].
fn side_label(side: OrderSide) -> &'static str {
    match side {
        OrderSide::Buy => "BUY",
        OrderSide::Sell => "SELL",
    }
}

/// The positioning table rows: `(instrument, "<SIDE> <qty>", "₹<risk>")` with
/// exact Decimal strings (zero floats).
pub fn studio_ee_positions(intents: &[PositionIntent]) -> Vec<(String, String, String)> {
    intents
        .iter()
        .map(|p| {
            (
                p.instrument.to_string(),
                format!("{} {}", side_label(p.side), p.quantity.value()),
                format!("₹{}", p.risk_cap.amount()),
            )
        })
        .collect()
}

/// The side word (`support` / `resistance`) implied by an S/R label.
fn band_side_label(label: &str) -> &'static str {
    if label.starts_with('S') {
        "support"
    } else {
        "resistance"
    }
}

/// Level-map report rows, one per elephant band:
/// `"S1 (support) 23930..23970 @ 23950 score 3"`.
pub fn ee_level_report_rows(map: &LevelMap, viewport: &StudioViewport) -> Vec<String> {
    let bands = studio_ee_bands(map, viewport);
    let lines = studio_ee_lines(map, viewport);
    bands
        .into_iter()
        .zip(lines)
        .map(|(band, line)| {
            let label = band.label.clone().unwrap_or_default();
            format!(
                "{label} ({}) {}..{} @ {} score {}",
                band_side_label(&label),
                band.low,
                band.high,
                line.price,
                line.style.color_index
            )
        })
        .collect()
}

/// Signal kind label (`BREAKOUT` / `REVERSAL` / `RETEST`).
fn signal_kind_label(kind: SignalKind) -> &'static str {
    match kind {
        SignalKind::Breakout => "BREAKOUT",
        SignalKind::Reversal => "REVERSAL",
        SignalKind::Retest => "RETEST",
    }
}

/// Signal report rows, one per signal with its SL/3R/5R stop prices:
/// `"BUY RETEST @ 23950 (conf 75%): SL 1R 23710.50 | T 3R 24668.50 | T 5R 25147.50"`.
pub fn ee_signal_report_rows(signals: &[StrategySignal], viewport: &StudioViewport) -> Vec<String> {
    let stops = studio_ee_stops(signals, viewport);
    signals
        .iter()
        .enumerate()
        .map(|(n, s)| {
            let base = n * 3;
            let sl = stops.get(base).map(|l| l.price);
            let t3 = stops.get(base + 1).map(|l| l.price);
            let t5 = stops.get(base + 2).map(|l| l.price);
            format!(
                "{} {} @ {} (conf {}%): SL 1R {} | T 3R {} | T 5R {}",
                side_label(s.side),
                signal_kind_label(s.kind),
                signal_entry(s),
                s.confidence,
                sl.map(|d| d.to_string()).unwrap_or_else(|| "-".to_string()),
                t3.map(|d| d.to_string()).unwrap_or_else(|| "-".to_string()),
                t5.map(|d| d.to_string()).unwrap_or_else(|| "-".to_string()),
            )
        })
        .collect()
}

// ---------------------------------------------------------------------------
// Seeded fixtures — the same "seeded day" every surface renders (plan 0006
// §2 honest-degradation: live data replaces these once the REST endpoints
// exist).
// ---------------------------------------------------------------------------

/// The seeded spot price (NIFTY 24040) used for regime classification.
pub fn ee_seed_last_price() -> Decimal {
    Decimal::from(24_040)
}

/// The seeded intraday level map: S1/S2/R1/R2 elephant bands plus a gamma-flip
/// level at 24000. `LevelId` values encode each level's representative price
/// so the signal entry derivation stays coherent.
pub fn ee_seed_level_map() -> LevelMap {
    use tantra_core::levels::{Level, LevelSide};
    let level = |id: u64, kind, side, low: u64, high: u64, score| {
        Level::new(
            LevelId(id),
            kind,
            side,
            (Decimal::from(low), Decimal::from(high)),
            score,
        )
        .expect("valid seed level")
    };
    LevelMap::new(vec![
        level(
            23_950,
            LevelKind::ElephantSupport1,
            LevelSide::Support,
            23_930,
            23_970,
            3,
        ),
        level(
            23_850,
            LevelKind::ElephantSupport2,
            LevelSide::Support,
            23_820,
            23_880,
            2,
        ),
        level(
            24_150,
            LevelKind::ElephantResistance1,
            LevelSide::Resistance,
            24_130,
            24_170,
            4,
        ),
        level(
            24_250,
            LevelKind::ElephantResistance2,
            LevelSide::Resistance,
            24_220,
            24_280,
            1,
        ),
        level(
            24_000,
            LevelKind::GammaFlip,
            LevelSide::Support,
            24_000,
            24_000,
            0,
        ),
    ])
}

/// The seeded signals: a `BUY RETEST` anchored to S1 (entry 23950) and a
/// `SELL BREAKOUT` anchored to R1 (entry 24150).
pub fn ee_seed_signals() -> Vec<StrategySignal> {
    let instrument =
        InstrumentId::try_new(Exchange::Nse, "NIFTY50").expect("valid seed instrument");
    let ts = chrono::DateTime::from_timestamp(1_700_000_000, 0).expect("valid seed timestamp");
    vec![
        StrategySignal::new(
            OrderSide::Buy,
            SignalKind::Retest,
            vec![LevelId(23_950)],
            Decimal::from(75),
            instrument.clone(),
            ts,
        )
        .expect("valid seed signal"),
        StrategySignal::new(
            OrderSide::Sell,
            SignalKind::Breakout,
            vec![LevelId(24_150)],
            Decimal::from(82),
            instrument.clone(),
            ts,
        )
        .expect("valid seed signal"),
    ]
}

/// The seeded position intents: NIFTY long 25 (risk ₹5000) and RELIANCE short
/// 100 (risk ₹12000).
pub fn ee_seed_positions() -> Vec<PositionIntent> {
    vec![
        PositionIntent::new(
            InstrumentId::try_new(Exchange::Nse, "NIFTY50").expect("valid seed instrument"),
            OrderSide::Buy,
            Quantity::from_u64(25),
            Money::inr(Decimal::from(5_000)),
            Decimal::new(5, 2),
        )
        .expect("valid seed position"),
        PositionIntent::new(
            InstrumentId::try_new(Exchange::Nse, "RELIANCE").expect("valid seed instrument"),
            OrderSide::Sell,
            Quantity::from_u64(100),
            Money::inr(Decimal::from(12_000)),
            Decimal::new(2, 1),
        )
        .expect("valid seed position"),
    ]
}
