//! Tests for zone-band geometry (`tantra-ui-charts`).

use rust_decimal::Decimal;
use tantra_ui_charts::axis::ChartRange;
use tantra_ui_charts::candle::GeometryError;
use tantra_ui_charts::zones::ZoneBand;

fn dec(s: &str) -> Decimal {
    s.parse().unwrap()
}

fn zone(low: &str, high: &str) -> ZoneBand {
    ZoneBand::new(dec(low), dec(high), 0, 100).unwrap()
}

#[test]
fn overlaps_matrix() {
    let a = zone("10", "20");
    // Overlapping above.
    assert!(a.overlaps(&zone("15", "25")));
    // Fully contained.
    assert!(a.overlaps(&zone("12", "18")));
    // Fully containing.
    assert!(a.overlaps(&zone("5", "30")));
    // Symmetric.
    assert!(zone("15", "25").overlaps(&a));
    // Edge-touching does not overlap.
    assert!(!a.overlaps(&zone("20", "30")));
    assert!(!a.overlaps(&zone("5", "10")));
    // Disjoint.
    assert!(!a.overlaps(&zone("25", "35")));
    assert!(!a.overlaps(&zone("0", "5")));
    // Identity overlaps itself.
    assert!(a.overlaps(&a));
}

#[test]
fn y_rows_orders_top_to_bottom() {
    let r = ChartRange::new(dec("0"), dec("100"));
    let band = zone("25", "75");
    // top = row of 75 → 25, bottom = row of 25 → 75.
    assert_eq!(band.y_rows(&r, 101), Ok((25, 75)));
    let (top, bottom) = band.y_rows(&r, 101).unwrap();
    assert!(top <= bottom, "top must not exceed bottom");
}

#[test]
fn y_rows_full_extent_band() {
    let r = ChartRange::new(dec("0"), dec("100"));
    let band = zone("0", "100");
    assert_eq!(band.y_rows(&r, 101), Ok((0, 100)));
}

#[test]
fn y_rows_rejects_flat_range() {
    let flat = ChartRange::new(dec("50"), dec("50"));
    let band = zone("25", "75");
    assert_eq!(band.y_rows(&flat, 101), Err(GeometryError::FlatPriceRange));
}

#[test]
fn new_validates_band_orientation() {
    assert_eq!(
        ZoneBand::new(dec("20"), dec("10"), 0, 10),
        Err(GeometryError::InvalidGeometry)
    );
    // Degenerate band (low == high) is allowed.
    assert!(ZoneBand::new(dec("10"), dec("10"), 0, 10).is_ok());
}

#[test]
fn new_validates_x_span() {
    assert_eq!(
        ZoneBand::new(dec("10"), dec("20"), 10, 5),
        Err(GeometryError::InvalidGeometry)
    );
    assert!(ZoneBand::new(dec("10"), dec("20"), 5, 5).is_ok());
}

#[test]
fn fill_index_and_label() {
    let band = ZoneBand::new(dec("10"), dec("20"), 0, 100)
        .unwrap()
        .with_fill_index(3)
        .with_label("Elephant S1");
    assert_eq!(band.fill_index, 3);
    assert_eq!(band.label.as_deref(), Some("Elephant S1"));
    // Defaults on a plain construction.
    let plain = ZoneBand::new(dec("10"), dec("20"), 0, 100).unwrap();
    assert_eq!(plain.fill_index, 0);
    assert_eq!(plain.label, None);
}
