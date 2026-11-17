//! Signal-marker glyph anchoring (buy/sell arrows at price + candle index).

use rust_decimal::Decimal;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::axis::ChartRange;
use crate::candle::GeometryError;
use crate::levels::price_to_row;

/// Side of a signal marker.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum MarkerSide {
    /// Buy signal.
    Buy,
    /// Sell signal.
    Sell,
}

/// Signal kind drawn as a marker glyph.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum MarkerKind {
    /// Trend breakout.
    Breakout,
    /// Trend reversal.
    Reversal,
    /// Level retest.
    Retest,
    /// Stop-line touch.
    StopLine,
}

/// A signal marker anchored at a price and a candle index.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct SignalMarker {
    /// Marker price.
    pub price: Decimal,
    /// Candle index (0-based) anchoring the x coordinate.
    pub index: usize,
    /// Buy or sell.
    pub side: MarkerSide,
    /// Signal kind.
    pub kind: MarkerKind,
}

impl SignalMarker {
    /// Create a new signal marker.
    pub fn new(price: Decimal, index: usize, side: MarkerSide, kind: MarkerKind) -> Self {
        Self {
            price,
            index,
            side,
            kind,
        }
    }

    /// The `(x, y)` pixel anchor: `x = index × candle_width`, `y` the price row.
    ///
    /// An `index × candle_width` product that overflows `u16` clamps to
    /// `u16::MAX`; the y coordinate clamps to the chart edge rows.
    pub fn anchor(
        &self,
        range: &ChartRange,
        height: u16,
        candle_width: u16,
    ) -> Result<(u16, u16), GeometryError> {
        let x =
            u16::try_from(self.index.saturating_mul(usize::from(candle_width))).unwrap_or(u16::MAX);
        let y = price_to_row(self.price, range, height)?;
        Ok((x, y))
    }
}
