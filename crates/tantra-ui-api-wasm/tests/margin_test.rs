//! Tests for margin bindings.

use rust_decimal::Decimal;
use tantra_core::types::{Exchange, InstrumentId, Price, Quantity};
use tantra_ui_api_wasm::margin::margin_required;

fn dec(s: &str) -> Decimal {
    s.parse().unwrap()
}

fn price(s: &str) -> Price {
    Price::try_new(dec(s)).unwrap()
}

#[test]
fn leverage_two_halves_margin() {
    let inst = InstrumentId::try_new(Exchange::Nfo, "NIFTY").unwrap();
    let m = margin_required(&inst, price("20000"), Quantity::from_u64(50), dec("2"));
    // notional = 1,000,000 / 2 = 500,000
    assert_eq!(m.amount(), dec("500000"));
}

#[test]
fn leverage_one_equals_notional() {
    let inst = InstrumentId::try_new(Exchange::Nse, "RELIANCE").unwrap();
    let m = margin_required(&inst, price("2500"), Quantity::from_u64(10), Decimal::ONE);
    assert_eq!(m.amount(), dec("25000"));
}

#[test]
fn zero_or_negative_leverage_defaults_to_one() {
    let inst = InstrumentId::try_new(Exchange::Nse, "TCS").unwrap();
    let m = margin_required(&inst, price("1000"), Quantity::from_u64(5), Decimal::ZERO);
    assert_eq!(m.amount(), dec("5000"));
}
