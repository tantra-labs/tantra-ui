//! U2 — golden parity (plan 0008 D1 / root plan 0007 T14.14).
//!
//! Drive a `PaperSession` through the canonical golden tape encoded in
//! `golden/paper_golden.json` and assert the fills equal the golden values
//! (price / quantity / side / originating order) plus the summary economics
//! (sell average fill price, flat net position, zero-fee cash round trip,
//! event shape). The vector is the P4 oracle for root plan 0007 T14.14: the
//! file is byte-identical across `tantra-exec-paper` (Rust), the Python wheel
//! (`tantra-ffi`), and this WASM embedding.

use std::collections::HashMap;
use std::str::FromStr;

use chrono::{DateTime, Utc};
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
    market_price: String,
    ts0: String,
    orders: Vec<GoldenOrder>,
    expected: GoldenSummary,
}

#[derive(Debug, Deserialize)]
struct GoldenOrder {
    id: usize,
    side: String,
    order_type: String,
    quantity: String,
    limit_price: Option<String>,
}

#[derive(Debug, Deserialize)]
struct GoldenFill {
    side: String,
    price: String,
    quantity: String,
}

#[derive(Debug, Deserialize)]
struct GoldenSummary {
    fills: Vec<GoldenFill>,
    order_states: HashMap<String, String>,
    sell_filled_qty: String,
    sell_avg_fill_price: String,
    net_cash: String,
    event_count: usize,
}

fn decimal(s: &str) -> Decimal {
    Decimal::from_str(s).expect("valid decimal literal in golden tape")
}

fn side(s: &str) -> OrderSide {
    match s {
        "BUY" => OrderSide::Buy,
        "SELL" => OrderSide::Sell,
        other => panic!("unknown side {other:?} in golden tape"),
    }
}

/// Run the full golden tape through a fresh session and return it.
fn run_tape(tape: &GoldenTape) -> PaperSession {
    let inst = InstrumentId::try_new(Exchange::Nse, "RELIANCE").unwrap();
    let ts0 = DateTime::parse_from_rfc3339(&tape.ts0)
        .expect("valid rfc3339 ts0 in golden tape")
        .with_timezone(&Utc);
    let mut session = PaperSession::new(decimal(&tape.initial_cash), None);
    for order in &tape.orders {
        let order_type = match order.order_type.as_str() {
            "LIMIT" => OrderType::Limit(
                Price::try_new(decimal(order.limit_price.as_ref().expect("limit price"))).unwrap(),
            ),
            "MARKET" => OrderType::Market,
            other => panic!("unknown order type {other:?} in golden tape"),
        };
        let req = OrderRequest::with_id(
            Uuid::from_u128(order.id as u128),
            inst.clone(),
            side(&order.side),
            order_type,
            Quantity::try_new(decimal(&order.quantity)).unwrap(),
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
    assert_eq!(fills.len(), tape.expected.fills.len());

    let sell_id = tape
        .orders
        .iter()
        .find(|o| o.side == "SELL")
        .map(|o| o.id as u128)
        .expect("golden tape has one market sell");
    let buy_id_by_price: HashMap<&str, u128> = tape
        .orders
        .iter()
        .filter(|o| o.side == "BUY")
        .map(|o| {
            (
                o.limit_price.as_ref().expect("limit price").as_str(),
                o.id as u128,
            )
        })
        .collect();

    for (idx, expected) in tape.expected.fills.iter().enumerate() {
        let fill = &fills[idx];
        assert_eq!(
            fill.price.value(),
            decimal(&expected.price),
            "fill {idx} price"
        );
        assert_eq!(
            fill.quantity.value(),
            decimal(&expected.quantity),
            "fill {idx} quantity"
        );
        assert_eq!(fill.side, side(&expected.side), "fill {idx} side");
        let expected_id = if expected.side == "SELL" {
            sell_id
        } else {
            *buy_id_by_price
                .get(expected.price.as_str())
                .expect("buy limit for fill price")
        };
        assert_eq!(
            fill.client_order_id,
            Uuid::from_u128(expected_id),
            "fill {idx} order id"
        );
    }

    // Summary economics: event shape, sell average fill price, flat net
    // position, cash (zero-fee round trip back to the initial balance).
    assert_eq!(session.events().len(), tape.expected.event_count);
    let sell_fills: Vec<_> = fills
        .iter()
        .filter(|f| f.client_order_id == Uuid::from_u128(sell_id))
        .collect();
    let notional: Decimal = sell_fills
        .iter()
        .map(|f| f.price.value() * f.quantity.value())
        .sum();
    let quantity: Decimal = sell_fills.iter().map(|f| f.quantity.value()).sum();
    assert_eq!(
        (notional / quantity).round_dp(4),
        decimal(&tape.expected.sell_avg_fill_price)
    );

    let inst = InstrumentId::try_new(Exchange::Nse, "RELIANCE").unwrap();
    assert_eq!(session.positions().get(&inst), Some(&dec!(0)));
    assert_eq!(session.cash(), decimal(&tape.expected.net_cash));
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

/// The golden summary is internally consistent with the canonical tape.
#[test]
fn golden_tape_summary_is_consistent() {
    let tape: GoldenTape = serde_json::from_str(GOLDEN_JSON).expect("golden tape parses");
    assert_eq!(tape.expected.fills.len(), 4);
    assert_eq!(tape.expected.event_count, 7);
    assert_eq!(decimal(&tape.expected.sell_filled_qty), dec!(150));
    assert_eq!(decimal(&tape.expected.sell_avg_fill_price), dec!(100.6667));
    assert_eq!(decimal(&tape.expected.net_cash), dec!(1_000_000));
    assert_eq!(
        tape.expected.order_states.get("buy101").map(String::as_str),
        Some("FILLED")
    );
    assert_eq!(
        tape.expected.order_states.get("buy100").map(String::as_str),
        Some("PARTIALLY_FILLED")
    );
    assert_eq!(
        tape.expected.order_states.get("sell").map(String::as_str),
        Some("FILLED")
    );
    assert_eq!(tape.market_price, "100");
    assert_eq!(tape.ts0, "2023-11-14T22:13:20Z");
}
