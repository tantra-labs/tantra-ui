//! Tests for the shared EE+ Strategy Studio view-model helpers (ui plan 0006
//! Phases 2–4). These assert exact `Decimal` geometry strings so the seeded
//! web/TUI/CLI surfaces cannot drift: line/band prices, scores, marker
//! anchors, SL/3R/5R stop lines, regime classification, and the positioning
//! table.

use rust_decimal::Decimal;
use tantra_core::levels::LevelMap;
use tantra_core::signals::PositionIntent;
use tantra_ui_charts::markers::{MarkerKind, MarkerSide};
use tantra_ui_charts::stops::StopKind;
use tantra_ui_core::ee::{
    EE_STOP_LABELS, StudioViewport, ee_level_report_rows, ee_seed_last_price, ee_seed_level_map,
    ee_seed_positions, ee_seed_signals, ee_signal_report_rows, studio_ee_bands, studio_ee_lines,
    studio_ee_marks, studio_ee_positions, studio_ee_regime, studio_ee_stops,
};

fn dec(s: &str) -> Decimal {
    s.parse().unwrap()
}

/// A wide viewport covering every stop/target price in the seeded day.
fn viewport() -> StudioViewport {
    StudioViewport::new(dec("22500"), dec("25500"), 360, 240)
}

#[test]
fn ee_lines_render_s1_s2_r1_r2_bands_and_edge_lines() {
    let map = ee_seed_level_map();
    let vp = viewport();

    let bands = studio_ee_bands(&map, &vp);
    assert_eq!(bands.len(), 4, "two supports + two resistances");
    let labels: Vec<Option<String>> = bands.iter().map(|b| b.label.clone()).collect();
    assert_eq!(
        labels,
        vec![
            Some("S1".to_string()),
            Some("S2".to_string()),
            Some("R1".to_string()),
            Some("R2".to_string()),
        ]
    );

    // Band geometry: exact band prices and the confluence-score fill index.
    assert_eq!(bands[0].low, dec("23930"));
    assert_eq!(bands[0].high, dec("23970"));
    assert_eq!(bands[0].fill_index, 3);
    assert_eq!(bands[1].low, dec("23820"));
    assert_eq!(bands[1].high, dec("23880"));
    assert_eq!(bands[1].fill_index, 2);
    assert_eq!(bands[2].low, dec("24130"));
    assert_eq!(bands[2].high, dec("24170"));
    assert_eq!(bands[2].fill_index, 4);
    assert_eq!(bands[3].low, dec("24220"));
    assert_eq!(bands[3].high, dec("24280"));
    assert_eq!(bands[3].fill_index, 1);

    // Edge lines at the band mid prices, colour-indexed by score; the
    // resistance lines are dashed.
    let lines = studio_ee_lines(&map, &vp);
    assert_eq!(lines.len(), 4);
    assert_eq!(lines[0].price, dec("23950"));
    assert_eq!(lines[0].style.color_index, 3);
    assert!(!lines[0].style.dashed, "support S1 is solid");
    assert_eq!(lines[1].price, dec("23850"));
    assert_eq!(lines[1].style.color_index, 2);
    assert_eq!(lines[2].price, dec("24150"));
    assert_eq!(lines[2].style.color_index, 4);
    assert!(lines[2].style.dashed, "resistance R1 is dashed");
    assert_eq!(lines[3].price, dec("24250"));
    assert_eq!(lines[3].style.color_index, 1);
    assert!(lines[3].style.dashed, "resistance R2 is dashed");
}

#[test]
fn ee_lines_y_anchor_within_the_viewport() {
    let map = ee_seed_level_map();
    let vp = viewport();
    let lines = studio_ee_lines(&map, &vp);
    for line in &lines {
        let y = line.y_from_range(&vp.range, vp.height).expect("y in range");
        assert!(y < vp.height, "y {y} within height {}", vp.height);
    }
}

#[test]
fn ee_marks_map_sides_and_kinds_at_seeded_positions() {
    let signals = ee_seed_signals();
    let marks = studio_ee_marks(&signals, &viewport());
    assert_eq!(marks.len(), 2);
    // First signal: Buy / Retest anchored at the S1 reference price.
    assert_eq!(marks[0].side, MarkerSide::Buy);
    assert_eq!(marks[0].kind, MarkerKind::Retest);
    assert_eq!(marks[0].price, dec("23950"));
    assert_eq!(marks[0].index, 2, "deterministic seed index");
    // Second signal: Sell / Breakout anchored at the R1 reference price.
    assert_eq!(marks[1].side, MarkerSide::Sell);
    assert_eq!(marks[1].kind, MarkerKind::Breakout);
    assert_eq!(marks[1].price, dec("24150"));
    assert_eq!(marks[1].index, 7, "deterministic seed index");
}

#[test]
fn ee_stops_compute_exact_sl_3r_5r_prices() {
    let signals = ee_seed_signals();
    let stops = studio_ee_stops(&signals, &viewport());
    // 3 lines per signal: [SL 1R, T 3R, T 5R].
    assert_eq!(stops.len(), 6);
    assert_eq!(EE_STOP_LABELS, ["SL 1R", "T 3R", "T 5R"]);

    // Buy 23950: 1R = 239.50; SL below, targets above.
    assert_eq!(stops[0].kind, StopKind::Initial);
    assert_eq!(stops[0].price.to_string(), "23710.50");
    assert_eq!(stops[1].kind, StopKind::Target);
    assert_eq!(stops[1].price.to_string(), "24668.50");
    assert_eq!(stops[2].kind, StopKind::Target);
    assert_eq!(stops[2].price.to_string(), "25147.50");

    // Sell 24150: SL above, targets below.
    assert_eq!(stops[3].kind, StopKind::Initial);
    assert_eq!(stops[3].price.to_string(), "24391.50");
    assert_eq!(stops[4].kind, StopKind::Target);
    assert_eq!(stops[4].price.to_string(), "23425.50");
    assert_eq!(stops[5].kind, StopKind::Target);
    assert_eq!(stops[5].price.to_string(), "22942.50");
}

#[test]
fn ee_regime_classifies_from_gamma_flip_vs_spot() {
    let map = ee_seed_level_map();
    // Spot 24040 sits above the 24000 gamma flip → long gamma.
    let (chip, desc) = studio_ee_regime(&map, dec("24040"));
    assert_eq!(chip, "long_gamma");
    assert_eq!(desc, "pin / mean-revert");
    // Spot below the flip → short gamma.
    let (chip, desc) = studio_ee_regime(&map, dec("23900"));
    assert_eq!(chip, "short_gamma");
    assert_eq!(desc, "trend / breakout");
    // Exactly at the flip counts as long gamma.
    let (chip, _) = studio_ee_regime(&map, dec("24000"));
    assert_eq!(chip, "long_gamma");
    // No gamma-flip level on the map → honest neutral.
    let (chip, desc) = studio_ee_regime(&LevelMap::new(Vec::new()), Decimal::ZERO);
    assert_eq!(chip, "neutral");
    assert_eq!(desc, "no gamma flip level on the map");
}

#[test]
fn ee_positions_rows_render_exact_strings() {
    let rows = studio_ee_positions(&ee_seed_positions());
    assert_eq!(rows.len(), 2);
    assert_eq!(
        rows[0],
        (
            "NSE:NIFTY50".to_string(),
            "BUY 25".to_string(),
            "₹5000".to_string()
        )
    );
    assert_eq!(
        rows[1],
        (
            "NSE:RELIANCE".to_string(),
            "SELL 100".to_string(),
            "₹12000".to_string()
        )
    );
    // An empty intent slice renders an empty table (honest — no fake rows).
    let empty: Vec<PositionIntent> = Vec::new();
    assert!(studio_ee_positions(&empty).is_empty());
}

#[test]
fn ee_report_rows_include_scores_signals_and_stops() {
    let map = ee_seed_level_map();
    let signals = ee_seed_signals();
    let vp = viewport();

    let levels = ee_level_report_rows(&map, &vp);
    assert_eq!(levels.len(), 4);
    assert_eq!(levels[0], "S1 (support) 23930..23970 @ 23950 score 3");
    assert_eq!(levels[1], "S2 (support) 23820..23880 @ 23850 score 2");
    assert_eq!(levels[2], "R1 (resistance) 24130..24170 @ 24150 score 4");
    assert_eq!(levels[3], "R2 (resistance) 24220..24280 @ 24250 score 1");

    let sigs = ee_signal_report_rows(&signals, &vp);
    assert_eq!(sigs.len(), 2);
    assert_eq!(
        sigs[0],
        "BUY RETEST @ 23950 (conf 75%): SL 1R 23710.50 | T 3R 24668.50 | T 5R 25147.50"
    );
    assert_eq!(
        sigs[1],
        "SELL BREAKOUT @ 24150 (conf 82%): SL 1R 24391.50 | T 3R 23425.50 | T 5R 22942.50"
    );
}

#[test]
fn ee_seed_last_price_is_built_from_exact_decimal() {
    assert_eq!(ee_seed_last_price(), dec("24040"));
}
