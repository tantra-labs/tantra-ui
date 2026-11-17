//! Shaded price-band zone geometry (Elephant support/resistance, order blocks).

use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};

use crate::axis::ChartRange;
use crate::candle::GeometryError;
use crate::levels::price_to_row;

/// A shaded price band spanning `x0..=x1`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ZoneBand {
    /// Lower band price.
    pub low: Decimal,
    /// Upper band price, `high >= low`.
    pub high: Decimal,
    /// Left edge (pixels).
    pub x0: usize,
    /// Right edge (pixels), `x1 >= x0`.
    pub x1: usize,
    /// Index into the renderer's fill / score palette.
    pub fill_index: u8,
    /// Optional annotation label.
    pub label: Option<String>,
}

impl ZoneBand {
    /// Create a validated zone band.
    ///
    /// # Errors
    /// Returns [`GeometryError::InvalidGeometry`] when `high < low` or
    /// `x1 < x0`.
    pub fn new(low: Decimal, high: Decimal, x0: usize, x1: usize) -> Result<Self, GeometryError> {
        if high < low {
            return Err(GeometryError::InvalidGeometry);
        }
        if x1 < x0 {
            return Err(GeometryError::InvalidGeometry);
        }
        Ok(Self {
            low,
            high,
            x0,
            x1,
            fill_index: 0,
            label: None,
        })
    }

    /// Set the fill palette index.
    pub fn with_fill_index(mut self, fill_index: u8) -> Self {
        self.fill_index = fill_index;
        self
    }

    /// Attach an annotation label.
    pub fn with_label(mut self, label: impl Into<String>) -> Self {
        self.label = Some(label.into());
        self
    }

    /// Whether two bands overlap in price (`low < other.high && other.low < high`).
    ///
    /// Bands that only touch at an edge price do not overlap. The x span is
    /// ignored.
    pub fn overlaps(&self, other: &ZoneBand) -> bool {
        self.low < other.high && other.low < self.high
    }

    /// The `(top, bottom)` pixel rows bounding this band within `[0, height)`.
    ///
    /// `top` is the row of `high` (higher price, smaller row) and `bottom` the
    /// row of `low`, so `top <= bottom`.
    pub fn y_rows(&self, range: &ChartRange, height: u16) -> Result<(u16, u16), GeometryError> {
        let top = price_to_row(self.high, range, height)?;
        let bottom = price_to_row(self.low, range, height)?;
        Ok((top, bottom))
    }
}
