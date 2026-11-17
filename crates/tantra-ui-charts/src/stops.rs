//! Stop-line geometry with an update/re-render path (SL / TSL / breakeven).

use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};

use crate::axis::ChartRange;
use crate::candle::GeometryError;
use crate::levels::price_to_row;

/// Kind of a stop line.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum StopKind {
    /// Initial stop-loss.
    Initial,
    /// Trailing stop-loss (moves with price).
    Trailing,
    /// Breakeven stop.
    Breakeven,
    /// A take-profit target line (3R/5R runner target).
    Target,
}

/// A movable stop line spanning `x0..=x1`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StopLine {
    /// Current stop price (`> 0`).
    pub price: Decimal,
    /// Left edge (pixels).
    pub x0: usize,
    /// Right edge (pixels), `x1 >= x0`.
    pub x1: usize,
    /// Stop kind.
    pub kind: StopKind,
    /// The price before the most recent [`StopLine::update`], if any.
    previous: Option<Decimal>,
}

impl StopLine {
    /// Create a validated stop line.
    ///
    /// # Errors
    /// Returns [`GeometryError::InvalidGeometry`] when `price <= 0` or
    /// `x1 < x0`.
    pub fn new(
        price: Decimal,
        x0: usize,
        x1: usize,
        kind: StopKind,
    ) -> Result<Self, GeometryError> {
        if price <= Decimal::ZERO {
            return Err(GeometryError::InvalidGeometry);
        }
        if x1 < x0 {
            return Err(GeometryError::InvalidGeometry);
        }
        Ok(Self {
            price,
            x0,
            x1,
            kind,
            previous: None,
        })
    }

    /// The price before the most recent [`StopLine::update`], or `None`
    /// before the first update.
    pub fn previous_price(&self) -> Option<Decimal> {
        self.previous
    }

    /// Move the stop to `new_price` (must be `> 0`).
    ///
    /// Stores the current price as [`Self::previous_price`]; the caller
    /// re-renders after a successful update. The stop is left unchanged on
    /// error.
    pub fn update(&mut self, new_price: Decimal) -> Result<(), GeometryError> {
        if new_price <= Decimal::ZERO {
            return Err(GeometryError::InvalidGeometry);
        }
        self.previous = Some(self.price);
        self.price = new_price;
        Ok(())
    }

    /// The pixel row for this stop's price within `[0, height)`.
    pub fn y_from_range(&self, range: &ChartRange, height: u16) -> Result<u16, GeometryError> {
        price_to_row(self.price, range, height)
    }
}
