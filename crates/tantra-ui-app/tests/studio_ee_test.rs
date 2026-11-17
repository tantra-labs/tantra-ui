//! View-model tests for the EE+ Strategy Studio feature (`studio_ee` / ui
//! plan 0006 Phases 2–3).
//!
//! Like `web_view_test.rs`, these avoid leptos rendering and assert on the
//! pure helpers: the seeded feature renders the level-map rows, regime badge,
//! signal/stop rows, and positioning table, and the honest-empty feature
//! (`unavailable = Some(...)`) renders the gap note and no rows.

use rust_decimal::Decimal;
use tantra_ui_app::studio_ee::{
    StudioEeFeature, StudioViewport, studio_ee_level_rows, studio_ee_position_rows,
    studio_ee_regime_text, studio_ee_signal_rows, studio_ee_unavailable,
};

fn dec(s: &str) -> Decimal {
    s.parse().unwrap()
}

fn viewport() -> StudioViewport {
    StudioViewport::new(dec("22500"), dec("25500"), 360, 240)
}

/// The seeded feature renders every EE+ section from the shared helpers.
#[test]
fn seeded_feature_renders_all_sections() {
    let feature = StudioEeFeature::new();
    assert_eq!(studio_ee_unavailable(&feature), None);
    let vp = viewport();

    let levels = studio_ee_level_rows(&feature, &vp);
    assert_eq!(levels.len(), 4);
    assert_eq!(levels[0], "S1 (support) 23930..23970 @ 23950 score 3");
    assert_eq!(levels[2], "R1 (resistance) 24130..24170 @ 24150 score 4");

    let regime = studio_ee_regime_text(&feature);
    assert!(regime.starts_with("long_gamma"), "regime: {regime}");

    let signals = studio_ee_signal_rows(&feature, &vp);
    assert_eq!(signals.len(), 2);
    assert!(
        signals[0].contains("SL 1R 23710.50"),
        "signal row: {}",
        signals[0]
    );
    assert!(
        signals[1].contains("T 5R 22942.50"),
        "signal row: {}",
        signals[1]
    );

    let positions = studio_ee_position_rows(&feature);
    assert_eq!(positions.len(), 2);
    assert_eq!(positions[0].0, "NSE:NIFTY50");
    assert_eq!(positions[0].1, "BUY 25");
    assert_eq!(positions[0].2, "₹5000");
    assert_eq!(positions[1].2, "₹12000");
}

/// The honest-empty feature renders the gap note and no rows.
#[test]
fn unavailable_feature_renders_honest_note() {
    let feature = StudioEeFeature::unavailable();
    let note = studio_ee_unavailable(&feature).expect("note present");
    assert!(
        note.contains("/level-map") && note.contains("/signals"),
        "note cites the upstream gap: {note}"
    );

    let vp = viewport();
    assert!(studio_ee_level_rows(&feature, &vp).is_empty());
    assert!(studio_ee_signal_rows(&feature, &vp).is_empty());
    assert!(studio_ee_position_rows(&feature).is_empty());
}

/// The regime chip reacts to the seeded spot price.
#[test]
fn regime_text_reflects_seeded_spot() {
    let mut feature = StudioEeFeature::new();
    let text = studio_ee_regime_text(&feature);
    assert!(text.starts_with("long_gamma"), "regime: {text}");

    // Spot below the gamma flip → short gamma.
    feature.last_price = dec("23900");
    let text = studio_ee_regime_text(&feature);
    assert!(text.starts_with("short_gamma"), "regime: {text}");
}
