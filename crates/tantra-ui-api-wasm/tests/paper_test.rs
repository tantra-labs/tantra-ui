//! U1 — paper exchange WASM bindings (plan 0008 D1).
//!
//! A `PaperSession` is a deterministic single-user in-browser session over
//! the WASM-portable `tantra-exec-paper` engine. A limit buy must fill
//! *exactly* when a bar crosses its limit, with exact `rust_decimal::Decimal`
//! values for the fill price/quantity and the resulting cash/position/equity.

use chrono::{TimeZone, Utc};
use rust_decimal_macros::dec;
use tantra_core::{
    Bar, Exchange, InstrumentId, OrderRequest, OrderSide, OrderType, Price, Quantity,
};
use tantra_ui_api_wasm::paper::{OrderState, PaperSession};
use uuid::Uuid;

const TS0: i64 = 1_700_000_000;

fn reliance() -> InstrumentId {
    InstrumentId::new(Exchange::Nse, "RELIANCE")
}

/// A fresh session has the seeded cash, empty fills/positions/events, and
/// equity equal to cash (no positions to mark).
#[test]
fn new_session_has_empty_state() {
    let session = PaperSession::new(dec!(1_000_000), None);
    assert_eq!(session.cash(), dec!(1_000_000));
    assert_eq!(session.equity(), dec!(1_000_000));
    assert!(session.fills().is_empty());
    assert!(session.positions().is_empty());
    assert_eq!(session.events().len(), 0);
    assert_eq!(session.order_state("PAPER-0001"), None);
}

/// A buy limit with no depth rests; the order stays `Working` until a bar
/// crosses its limit, at which point it fills at the feed-book price.
#[test]
fn limit_buy_fills_when_bar_crosses_limit() {
    let ts0 = Utc.timestamp_opt(TS0, 0).unwrap();
    let inst = reliance();
    let mut session = PaperSession::new(dec!(1_000_000), None);

    let order = OrderRequest::with_id(
        Uuid::from_u128(1),
        inst.clone(),
        OrderSide::Buy,
        OrderType::Limit(Price::from_rupees("100").unwrap()),
        Quantity::from_u64(100),
        "ui_paper_test",
    );
    let id = session.submit(order, ts0).unwrap();
    assert_eq!(id, "PAPER-0001");

    // No depth yet: the limit buy rests, nothing fills.
    assert_eq!(session.order_state(&id), Some(OrderState::Working));
    assert!(session.fills().is_empty());

    // A bar whose close (90) crosses below the buy limit (100) makes the buy
    // marketable against the feed-book depth seeded at the close.
    let bar = Bar {
        instrument: inst.clone(),
        open: Price::from_rupees("95").unwrap(),
        high: Price::from_rupees("95").unwrap(),
        low: Price::from_rupees("85").unwrap(),
        close: Price::from_rupees("90").unwrap(),
        volume: Quantity::from_u64(1000),
        timestamp: ts0,
    };
    session.feed_bar(&bar, ts0);

    let fills = session.fills();
    assert_eq!(fills.len(), 1);
    assert_eq!(fills[0].side, OrderSide::Buy);
    assert_eq!(fills[0].quantity.0, dec!(100));
    assert_eq!(fills[0].price.0, dec!(90));

    assert_eq!(session.order_state(&id), Some(OrderState::Filled));
    assert_eq!(session.order_avg_fill_price(&id).unwrap().0, dec!(90));
    // Cash = 1_000_000 − 100 × 90; position +100; equity marks it back up.
    assert_eq!(session.cash(), dec!(991_000));
    assert_eq!(session.positions().get(&inst), Some(&dec!(100)));
    assert_eq!(session.equity(), dec!(1_000_000));
}

/// A market order with no contra liquidity is rejected with `NoMarketPrice`
/// (the in-browser session surfaces the engine error untouched).
#[test]
fn market_order_without_liquidity_is_rejected() {
    let ts0 = Utc.timestamp_opt(TS0, 0).unwrap();
    let inst = reliance();
    let mut session = PaperSession::new(dec!(1_000_000), None);

    let order = OrderRequest::with_id(
        Uuid::from_u128(2),
        inst,
        OrderSide::Sell,
        OrderType::Market,
        Quantity::from_u64(10),
        "ui_paper_test",
    );
    assert!(session.submit(order, ts0).is_err());
}
