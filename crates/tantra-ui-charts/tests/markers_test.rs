//! Tests for signal-marker anchoring and serde (`tantra-ui-charts`).

use rust_decimal::Decimal;
use schemars::schema_for;
use tantra_ui_charts::axis::ChartRange;
use tantra_ui_charts::markers::{MarkerKind, MarkerSide, SignalMarker};

fn dec(s: &str) -> Decimal {
    s.parse().unwrap()
}

fn range(min: &str, max: &str) -> ChartRange {
    ChartRange::new(dec(min), dec(max))
}

#[test]
fn anchor_golden_vectors() {
    let r = range("0", "100");
    // index × candle_width, price mapped into the same inverted rows.
    let m = SignalMarker::new(dec("50"), 3, MarkerSide::Buy, MarkerKind::Breakout);
    assert_eq!(m.anchor(&r, 101, 10), Ok((30, 50)));
    let m = SignalMarker::new(dec("100"), 0, MarkerSide::Sell, MarkerKind::Reversal);
    assert_eq!(m.anchor(&r, 101, 10), Ok((0, 0)));
    let m = SignalMarker::new(dec("0"), 4, MarkerSide::Buy, MarkerKind::Retest);
    assert_eq!(m.anchor(&r, 101, 20), Ok((80, 100)));
    // Non-degenerate width.
    let m = SignalMarker::new(dec("50"), 1, MarkerSide::Sell, MarkerKind::StopLine);
    assert_eq!(m.anchor(&r, 101, 1), Ok((1, 50)));
}

#[test]
fn anchor_clamps_overflowing_index_product() {
    let r = range("0", "100");
    let m = SignalMarker::new(dec("50"), usize::MAX, MarkerSide::Buy, MarkerKind::Breakout);
    assert_eq!(m.anchor(&r, 101, 2), Ok((u16::MAX, 50)));
    // Product 80000 exceeds u16::MAX → clamped.
    let m = SignalMarker::new(dec("50"), 40_000, MarkerSide::Sell, MarkerKind::Reversal);
    assert_eq!(m.anchor(&r, 101, 2), Ok((u16::MAX, 50)));
    // Product 60000 fits within u16.
    let m = SignalMarker::new(dec("50"), 3_000, MarkerSide::Buy, MarkerKind::Retest);
    assert_eq!(m.anchor(&r, 101, 20), Ok((60_000, 50)));
}

#[test]
fn marker_side_serde_round_trip() {
    for (variant, text) in [(MarkerSide::Buy, "buy"), (MarkerSide::Sell, "sell")] {
        let json = serde_json::to_string(&variant).unwrap();
        assert_eq!(json, format!("\"{text}\""));
        let back: MarkerSide = serde_json::from_str(&json).unwrap();
        assert_eq!(back, variant);
    }
    // CamelCase spelling is rejected.
    assert!(serde_json::from_str::<MarkerSide>("\"Buy\"").is_err());
}

#[test]
fn marker_kind_serde_round_trip() {
    let cases = [
        (MarkerKind::Breakout, "breakout"),
        (MarkerKind::Reversal, "reversal"),
        (MarkerKind::Retest, "retest"),
        (MarkerKind::StopLine, "stop_line"),
    ];
    for (variant, text) in cases {
        let json = serde_json::to_string(&variant).unwrap();
        assert_eq!(json, format!("\"{text}\""));
        let back: MarkerKind = serde_json::from_str(&json).unwrap();
        assert_eq!(back, variant);
    }
}

#[test]
fn signal_marker_serde_round_trip() {
    let m = SignalMarker::new(dec("104.5"), 7, MarkerSide::Buy, MarkerKind::StopLine);
    let json = serde_json::to_string(&m).unwrap();
    let back: SignalMarker = serde_json::from_str(&json).unwrap();
    assert_eq!(back, m);
}

#[test]
fn marker_schema_uses_snake_case_variants() {
    let kind_json = serde_json::to_string(&schema_for!(MarkerKind)).unwrap();
    for variant in ["breakout", "reversal", "retest", "stop_line"] {
        assert!(
            kind_json.contains(&format!("\"{variant}\"")),
            "schema missing snake_case variant {variant}: {kind_json}"
        );
    }
    assert!(!kind_json.contains("\"Breakout\""));

    let side_json = serde_json::to_string(&schema_for!(MarkerSide)).unwrap();
    for variant in ["buy", "sell"] {
        assert!(side_json.contains(&format!("\"{variant}\"")));
    }
}
