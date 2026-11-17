//! EE+ (Elephant Edge) Strategy Studio feature state (ui plan 0006, Phases
//! 2–3).
//!
//! [`StudioEeFeature`] holds the seeded `LevelMap` / signals / position
//! intents the Studio renders plus an honest `unavailable` note citing the
//! upstream data gaps until `tantra-runtime-api-rest` exposes
//! `GET /level-map/{date}` / `GET /signals/{date}` (plan 0006 §2).
//!
//! The pure geometry/view-model helpers (`studio_ee_lines`, `studio_ee_bands`,
//! `studio_ee_marks`, `studio_ee_stops`, `studio_ee_regime`,
//! `studio_ee_positions`, ...) are the single shared source in
//! [`tantra_ui_core::ee`] — the web view, the TUI positioning panel, and the
//! `tantra elephant` CLI report all consume the same helpers so the seeded day
//! renders identically everywhere. This module re-exports them and wraps them
//! in the feature gating (an `unavailable` feature renders the honest note and
//! no rows).

use rust_decimal::Decimal;
use tantra_core::levels::LevelMap;
use tantra_core::signals::{PositionIntent, StrategySignal};

pub use tantra_ui_core::ee::{
    EE_STOP_LABELS, StudioViewport, ee_level_report_rows, ee_seed_last_price, ee_seed_level_map,
    ee_seed_positions, ee_seed_signals, ee_signal_report_rows, studio_ee_bands, studio_ee_lines,
    studio_ee_marks, studio_ee_positions, studio_ee_regime, studio_ee_stops,
};

/// The EE+ Strategy Studio feature state.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StudioEeFeature {
    /// The session's intraday level map (seeded until the REST endpoint).
    pub level_map: LevelMap,
    /// The strategy's live signals (seeded until the REST endpoint).
    pub signals: Vec<StrategySignal>,
    /// The proposed position intents (seeded until the REST endpoint).
    pub positions: Vec<PositionIntent>,
    /// The latest spot price (seeded; drives the regime chip).
    pub last_price: Decimal,
    /// Honest note shown instead of the rows while the upstream data gap is
    /// open; `None` once the live endpoints are wired (plan 0006 §2).
    pub unavailable: Option<&'static str>,
}

impl Default for StudioEeFeature {
    fn default() -> Self {
        Self::new()
    }
}

impl StudioEeFeature {
    /// The upstream data-gap note rendered by every surface until the
    /// `tantra-runtime-api-rest` level-map / signals endpoints exist.
    pub const UNAVAILABLE_NOTE: &'static str = "EE+ live level-map and signals are unavailable: \
        the REST endpoints (`GET /level-map/{date}`, `GET /signals/{date}`) are not wired yet \
        (ui plan 0006 §2). Showing the seeded day's data once available.";

    /// A seeded feature: the canonical seeded day's level map, signals, and
    /// position intents with no `unavailable` note.
    pub fn new() -> Self {
        Self {
            level_map: ee_seed_level_map(),
            signals: ee_seed_signals(),
            positions: ee_seed_positions(),
            last_price: ee_seed_last_price(),
            unavailable: None,
        }
    }

    /// The honest empty state: same seeds but marked unavailable so every
    /// surface renders the gap note and no rows.
    pub fn unavailable() -> Self {
        Self {
            unavailable: Some(Self::UNAVAILABLE_NOTE),
            ..Self::new()
        }
    }
}

/// The `unavailable` note of a feature (`None` while seeded).
pub fn studio_ee_unavailable(feature: &StudioEeFeature) -> Option<&'static str> {
    feature.unavailable
}

/// Level-map report rows for the feature (empty while unavailable).
pub fn studio_ee_level_rows(feature: &StudioEeFeature, viewport: &StudioViewport) -> Vec<String> {
    if feature.unavailable.is_some() {
        Vec::new()
    } else {
        ee_level_report_rows(&feature.level_map, viewport)
    }
}

/// Regime badge text: `"long_gamma (pin / mean-revert)"` etc.
pub fn studio_ee_regime_text(feature: &StudioEeFeature) -> String {
    let (chip, desc) = studio_ee_regime(&feature.level_map, feature.last_price);
    format!("{chip} ({desc})")
}

/// Signal report rows for the feature (empty while unavailable).
pub fn studio_ee_signal_rows(feature: &StudioEeFeature, viewport: &StudioViewport) -> Vec<String> {
    if feature.unavailable.is_some() {
        Vec::new()
    } else {
        ee_signal_report_rows(&feature.signals, viewport)
    }
}

/// Positioning table rows `(instrument, "<SIDE> <qty>", "₹<risk>")` for the
/// feature (empty while unavailable).
pub fn studio_ee_position_rows(feature: &StudioEeFeature) -> Vec<(String, String, String)> {
    if feature.unavailable.is_some() {
        Vec::new()
    } else {
        studio_ee_positions(&feature.positions)
    }
}
