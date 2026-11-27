//! Margin estimation for a prospective order.

use rust_decimal::Decimal;
use tantra_core::types::{InstrumentId, Money, Price, Quantity};

/// Estimate the margin blocked by an order.
///
/// Margin is `notional / leverage` when a leverage factor is given; for
/// fully-collateralized products (leverage of 1) this equals the notional.
/// Returns [`Money`] in the instrument venue's currency.
pub fn margin_required(
    instrument: &InstrumentId,
    price: Price,
    quantity: Quantity,
    leverage: Decimal,
) -> Money {
    let notional = Money::notional(price, quantity, instrument.exchange().currency());
    let safe_leverage = if leverage <= Decimal::ZERO {
        Decimal::ONE
    } else {
        leverage
    };
    let amount = notional.amount() / safe_leverage;
    Money::inr(amount)
}
