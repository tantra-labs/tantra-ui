//! Horizontal level-line geometry (PDH/PDL, gamma flip, max pain, strikes).

use rust_decimal::Decimal;
use rust_decimal::prelude::ToPrimitive;
use serde::{Deserialize, Serialize};

use crate::axis::ChartRange;
use crate::candle::GeometryError;

/// Map a price to a pixel row within `[0, height)` for a [`ChartRange`].
///
/// Higher prices map to smaller rows (the top of the chart), mirroring
/// `CandlestickChart::y_for_price`. All math stays in exact `Decimal`; the
/// final rounding to an integer row is exact and never passes through
/// `f32`/`f64`.
pub(crate) fn price_to_row(
    price: Decimal,
    range: &ChartRange,
    height: u16,
) -> Result<u16, GeometryError> {
    if height == 0 {
        return Err(GeometryError::ZeroViewport);
    }
    let span = range.max - range.min;
    if span.is_zero() {
        return Err(GeometryError::FlatPriceRange);
    }
    let height_d = Decimal::from(height.saturating_sub(1));
    // y grows downward: min price → bottom (height-1), max price → top (0).
    let ratio = (price - range.min) / span;
    let y = height_d - (ratio * height_d);
    let y = y.round();
    // Out-of-range prices clamp to the chart edge rows.
    Ok(y.to_u16().unwrap_or(0).min(height.saturating_sub(1)))
}

/// Drawing style for a [`LevelLine`] (geometry only — palettes are applied by
/// the renderer).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct LevelLineStyle {
    /// Whether the line is drawn with dashes.
    pub dashed: bool,
    /// Index into the renderer's palette / score color ramp.
    pub color_index: u8,
    /// Stroke width in pixels.
    pub line_width: u8,
}

impl Default for LevelLineStyle {
    fn default() -> Self {
        Self {
            dashed: false,
            color_index: 0,
            line_width: 1,
        }
    }
}

/// A horizontal level line spanning `x0..=x1` at an exact price.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LevelLine {
    /// Level price (non-negative).
    pub price: Decimal,
    /// Left edge (pixels).
    pub x0: usize,
    /// Right edge (pixels), `x1 >= x0`.
    pub x1: usize,
    /// Optional annotation label.
    pub label: Option<String>,
    /// Line style.
    pub style: LevelLineStyle,
}

impl LevelLine {
    /// Create a validated level line.
    ///
    /// # Errors
    /// Returns [`GeometryError::InvalidGeometry`] when `price` is negative or
    /// `x1 < x0`.
    pub fn new(price: Decimal, x0: usize, x1: usize) -> Result<Self, GeometryError> {
        if price < Decimal::ZERO {
            return Err(GeometryError::InvalidGeometry);
        }
        if x1 < x0 {
            return Err(GeometryError::InvalidGeometry);
        }
        Ok(Self {
            price,
            x0,
            x1,
            label: None,
            style: LevelLineStyle::default(),
        })
    }

    /// Attach an annotation label.
    pub fn with_label(mut self, label: impl Into<String>) -> Self {
        self.label = Some(label.into());
        self
    }

    /// Override the line style.
    pub fn with_style(mut self, style: LevelLineStyle) -> Self {
        self.style = style;
        self
    }

    /// The pixel row for this level's price within `[0, height)`.
    pub fn y_from_range(&self, range: &ChartRange, height: u16) -> Result<u16, GeometryError> {
        price_to_row(self.price, range, height)
    }
}
