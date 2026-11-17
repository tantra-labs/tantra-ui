//! Tests for stop-line update determinism and geometry (`tantra-ui-charts`).

use rust_decimal::Decimal;
use tantra_ui_charts::axis::ChartRange;
use tantra_ui_charts::candle::GeometryError;
use tantra_ui_charts::stops::{StopKind, StopLine};

fn dec(s: &str) -> Decimal {
    s.parse().unwrap()
}

#[test]
fn update_sequence_is_deterministic() {
    let mut stop = StopLine::new(dec("100"), 0, 100, StopKind::Trailing).unwrap();
    // No previous price before the first update.
    assert_eq!(stop.previous_price(), None);

    assert_eq!(stop.update(dec("110")), Ok(()));
    assert_eq!(stop.price, dec("110"));
    assert_eq!(stop.previous_price(), Some(dec("100")));

    assert_eq!(stop.update(dec("115")), Ok(()));
    assert_eq!(stop.price, dec("115"));
    assert_eq!(stop.previous_price(), Some(dec("110")));

    // A trailing stop can ratchet back down; previous follows the sequence.
    assert_eq!(stop.update(dec("108")), Ok(()));
    assert_eq!(stop.price, dec("108"));
    assert_eq!(stop.previous_price(), Some(dec("115")));
}

#[test]
fn new_validates_non_positive_price() {
    assert_eq!(
        StopLine::new(Decimal::ZERO, 0, 100, StopKind::Initial),
        Err(GeometryError::InvalidGeometry)
    );
    assert_eq!(
        StopLine::new(dec("-5"), 0, 100, StopKind::Initial),
        Err(GeometryError::InvalidGeometry)
    );
}

#[test]
fn new_validates_x_span() {
    assert_eq!(
        StopLine::new(dec("100"), 10, 5, StopKind::Breakeven),
        Err(GeometryError::InvalidGeometry)
    );
    assert!(StopLine::new(dec("100"), 5, 5, StopKind::Breakeven).is_ok());
}

#[test]
fn update_rejects_non_positive_prices_and_leaves_stop_unchanged() {
    let mut stop = StopLine::new(dec("100"), 0, 100, StopKind::Trailing).unwrap();
    assert_eq!(
        stop.update(Decimal::ZERO),
        Err(GeometryError::InvalidGeometry)
    );
    assert_eq!(stop.update(dec("-1")), Err(GeometryError::InvalidGeometry));
    assert_eq!(stop.price, dec("100"));
    assert_eq!(stop.previous_price(), None);
}

#[test]
fn y_from_range_reflects_updates() {
    let r = ChartRange::new(dec("100"), dec("120"));
    let mut stop = StopLine::new(dec("100"), 0, 100, StopKind::Trailing).unwrap();
    // price 100 → bottom row of the range.
    assert_eq!(stop.y_from_range(&r, 101), Ok(100));
    assert_eq!(stop.update(dec("110")), Ok(()));
    // price 110 → midpoint row.
    assert_eq!(stop.y_from_range(&r, 101), Ok(50));
    assert_eq!(stop.update(dec("115")), Ok(()));
    // price 115 → row 25 (ratio 0.75 of 100).
    assert_eq!(stop.y_from_range(&r, 101), Ok(25));
    // The previous price still maps geometrically.
    assert_eq!(stop.previous_price(), Some(dec("110")));
}

#[test]
fn stop_kind_serde_round_trip() {
    let cases = [
        (StopKind::Initial, "initial"),
        (StopKind::Trailing, "trailing"),
        (StopKind::Breakeven, "breakeven"),
        (StopKind::Target, "target"),
    ];
    for (variant, text) in cases {
        let json = serde_json::to_string(&variant).unwrap();
        assert_eq!(json, format!("\"{text}\""));
        let back: StopKind = serde_json::from_str(&json).unwrap();
        assert_eq!(back, variant);
    }
}
