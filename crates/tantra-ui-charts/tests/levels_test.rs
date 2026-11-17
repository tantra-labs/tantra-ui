//! Tests for level-line geometry (`tantra-ui-charts`).

use rust_decimal::Decimal;
use tantra_ui_charts::axis::ChartRange;
use tantra_ui_charts::candle::GeometryError;
use tantra_ui_charts::levels::{LevelLine, LevelLineStyle};

fn dec(s: &str) -> Decimal {
    s.parse().unwrap()
}

fn range(min: &str, max: &str) -> ChartRange {
    ChartRange::new(dec(min), dec(max))
}

#[test]
fn y_from_range_golden_vectors() {
    let r = range("0", "100");
    let line = LevelLine::new(dec("100"), 0, 100).unwrap();
    assert_eq!(line.y_from_range(&r, 101), Ok(0)); // max price → top row
    let line = LevelLine::new(dec("0"), 0, 100).unwrap();
    assert_eq!(line.y_from_range(&r, 101), Ok(100)); // min price → bottom row
    let line = LevelLine::new(dec("50"), 0, 100).unwrap();
    assert_eq!(line.y_from_range(&r, 101), Ok(50)); // midpoint
    let line = LevelLine::new(dec("25"), 0, 100).unwrap();
    assert_eq!(line.y_from_range(&r, 101), Ok(75));
    let line = LevelLine::new(dec("75"), 0, 100).unwrap();
    assert_eq!(line.y_from_range(&r, 101), Ok(25));
}

#[test]
fn y_from_range_midpoint_rounding_matches_crate() {
    // height_d = 49 → the midpoint lands on 24.5; rust_decimal rounds
    // half-to-even, so 24.5 → 24 (the same rule `y_for_price` uses).
    let r = range("0", "100");
    let line = LevelLine::new(dec("50"), 0, 100).unwrap();
    assert_eq!(line.y_from_range(&r, 50), Ok(24));
}

#[test]
fn y_from_range_clamps_out_of_range_prices() {
    // Range [50, 100], height 51 → rows 0..=50.
    let r = range("50", "100");
    // Above the range clamps to the top row.
    let line = LevelLine::new(dec("200"), 0, 100).unwrap();
    assert_eq!(line.y_from_range(&r, 51), Ok(0));
    // Below the range (but non-negative) clamps to the bottom row.
    let line = LevelLine::new(dec("0"), 0, 100).unwrap();
    assert_eq!(line.y_from_range(&r, 51), Ok(50));
}

#[test]
fn y_from_range_rejects_flat_and_zero_height() {
    let flat = range("50", "50");
    let line = LevelLine::new(dec("50"), 0, 100).unwrap();
    assert_eq!(
        line.y_from_range(&flat, 101),
        Err(GeometryError::FlatPriceRange)
    );
    let r = range("0", "100");
    assert_eq!(line.y_from_range(&r, 0), Err(GeometryError::ZeroViewport));
}

#[test]
fn new_rejects_inverted_x_span() {
    assert_eq!(
        LevelLine::new(dec("100"), 10, 5),
        Err(GeometryError::InvalidGeometry)
    );
    // Degenerate span x0 == x1 is allowed.
    assert!(LevelLine::new(dec("100"), 5, 5).is_ok());
}

#[test]
fn new_rejects_negative_price() {
    assert_eq!(
        LevelLine::new(dec("-1"), 0, 10),
        Err(GeometryError::InvalidGeometry)
    );
    // Zero is a valid (non-negative) price.
    assert!(LevelLine::new(Decimal::ZERO, 0, 10).is_ok());
}

#[test]
fn label_passthrough() {
    let line = LevelLine::new(dec("100"), 0, 100)
        .unwrap()
        .with_label("PDH");
    assert_eq!(line.label.as_deref(), Some("PDH"));
}

#[test]
fn default_style_is_solid_thin_first_color() {
    let line = LevelLine::new(dec("100"), 0, 100).unwrap();
    assert_eq!(
        line.style,
        LevelLineStyle {
            dashed: false,
            color_index: 0,
            line_width: 1,
        }
    );
}

#[test]
fn with_style_overrides_style() {
    let style = LevelLineStyle {
        dashed: true,
        color_index: 3,
        line_width: 2,
    };
    let line = LevelLine::new(dec("100"), 0, 100)
        .unwrap()
        .with_style(style);
    assert_eq!(line.style, style);
}
