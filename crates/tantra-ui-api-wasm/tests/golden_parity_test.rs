//! U2 — golden parity (plan 0008 D1).
//!
//! Drive a `PaperSession` through the golden tape encoded in
//! `golden/paper_golden.json` and assert the fills equal the golden values
//! (price / quantity / side / order-index) plus the summary economics (sell
//! VWAP, net position, cash, event shape). The same vector is the P4 oracle
//! for cross-validating the Python engine against Rust/WASM.

use std::str::FromStr;

use chrono::{TimeZone, Utc};
use rust_decimal::Decimal;
use rust_decimal_macros::dec;
use serde::Deserialize;
use tantra_core::{Exchange, InstrumentId, OrderRequest, OrderSide, OrderType, Price, Quantity};
use tantra_ui_api_wasm::paper::PaperSession;
use uuid::Uuid;

const GOLDEN_JSON: &str = include_str!("golden/paper_golden.json");

#[derive(Debug, Deserialize)]
struct GoldenTape {
    instrument: String,
    initial_cash: String,
    timestamp: i64,
    orders: Vec<GoldenOrder>,
    #[serde(rename = "expected_fills")]
    expected_fills: Vec<GoldenFill>,
    expected: GoldenSummary,
}

#[derive(Debug, Deserialize)]
struct GoldenOrder {
    index: usize,
    side: String,
    #[serde(rename = "order_type")]
    order_type: String,
    quantity: String,
    price: Option<String>,
}

#[derive(Debug, Deserialize)]
struct GoldenFill {
    price: String,
    quantity: String,
    side: String,
    #[serde(rename = "order_index")]
    order_index: usize,
}

#[derive(Debug, Deserialize)]
struct GoldenSummary {
    #[serde(rename = "fill_count")]
    fill_count: usize,
    #[serde(rename = "event_count")]
    event_count: usize,
    #[serde(rename = "sell_vwap")]
    sell_vwap: String,
    #[serde(rename = "net_position")]
    net_position: String,
    cash: String,
}

fn decimal(s: &str) -> Decimal {
    Decimal::from_str(s).expect("valid decimal literal in golden tape")
}

fn side(s: &str) -> OrderSide {
    match s {
        "Buy" => OrderSide::Buy,
        "Sell" => OrderSide::Sell,
        other => panic!("unknown side {other:?} in golden tape"),
    }
}

/// Run the full golden tape through a fresh session and return it.
fn run_tape(tape: &GoldenTape) -> PaperSession {
    let inst = InstrumentId::new(Exchange::Nse, "RELIANCE");
    let ts0 = Utc.timestamp_opt(tape.timestamp, 0).unwrap();
    let mut session = PaperSession::new(decimal(&tape.initial_cash), None);
    for order in &tape.orders {
        let order_type = match order.order_type.as_str() {
            "Limit" => OrderType::Limit(Price::new(decimal(
                order.price.as_ref().expect("limit price"),
            ))),
            "Market" => OrderType::Market,
            other => panic!("unknown order type {other:?} in golden tape"),
        };
        let req = OrderRequest::with_id(
            Uuid::from_u128(order.index as u128),
            inst.clone(),
            side(&order.side),
            order_type,
            Quantity::new(decimal(&order.quantity)),
            "paper_golden",
        );
        session.submit(req, ts0).unwrap();
    }
    session
}

/// The session's fills match the golden vector in order:
/// price, quantity, side, and the originating order (by client id).
#[test]
fn golden_tape_fills_match_golden_values() {
    let tape: GoldenTape = serde_json::from_str(GOLDEN_JSON).expect("golden tape parses");
    assert_eq!(tape.instrument, "NSE:RELIANCE");
    let session = run_tape(&tape);

    let fills = session.fills();
    assert_eq!(fills.len(), tape.expected.fill_count);

    for (idx, expected) in tape.expected_fills.iter().enumerate() {
        let fill = &fills[idx];
        assert_eq!(fill.price.0, decimal(&expected.price), "fill {idx} price");
        assert_eq!(
            fill.quantity.0,
            decimal(&expected.quantity),
            "fill {idx} quantity"
        );
        assert_eq!(fill.side, side(&expected.side), "fill {idx} side");
        assert_eq!(
            fill.client_order_id,
            Uuid::from_u128(expected.order_index as u128),
            "fill {idx} order index"
        );
    }

    // Summary economics: event shape, sell VWAP, net position, cash.
    assert_eq!(session.events().len(), tape.expected.event_count);
    let sell_fills: Vec<_> = fills
        .iter()
        .filter(|f| f.client_order_id == Uuid::from_u128(3))
        .collect();
    let notional: Decimal = sell_fills.iter().map(|f| f.price.0 * f.quantity.0).sum();
    let quantity: Decimal = sell_fills.iter().map(|f| f.quantity.0).sum();
    assert_eq!(
        (notional / quantity).round_dp(4),
        decimal(&tape.expected.sell_vwap)
    );

    let inst = InstrumentId::new(Exchange::Nse, "RELIANCE");
    assert_eq!(
        session.positions().get(&inst),
        Some(&decimal(&tape.expected.net_position))
    );
    assert_eq!(session.cash(), decimal(&tape.expected.cash));
    // Zero-fee round trip: equity returns to the initial cash.
    assert_eq!(session.equity(), decimal(&tape.initial_cash));
}

/// Determinism: replaying the same tape through two fresh sessions produces
/// byte-identical fills, events, and cash.
#[test]
fn golden_tape_is_deterministic() {
    let tape: GoldenTape = serde_json::from_str(GOLDEN_JSON).expect("golden tape parses");
    let a = run_tape(&tape);
    let b = run_tape(&tape);
    assert_eq!(a.fills(), b.fills());
    assert_eq!(a.events(), b.events());
    assert_eq!(a.cash(), b.cash());
    assert_eq!(a.equity(), b.equity());
}

/// The golden summary totals are internally consistent with the tape.
#[test]
fn golden_tape_summary_is_consistent() {
    let tape: GoldenTape = serde_json::from_str(GOLDEN_JSON).expect("golden tape parses");
    assert_eq!(tape.expected.fill_count, 4);
    assert_eq!(tape.expected.event_count, 7);
    assert_eq!(decimal(&tape.expected.sell_vwap), dec!(100.6667));
}
