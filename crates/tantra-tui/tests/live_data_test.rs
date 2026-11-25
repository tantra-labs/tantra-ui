//! Integration tests for the live-data fold state (gap 0005).
//!
//! Canonical `ClientEvent`s are folded through `LiveData::apply_event` and
//! the resulting rows/summaries are asserted. Every price / quantity / pnl
//! value uses exact `rust_decimal::Decimal` arithmetic — no floats.

use std::collections::BTreeMap;
use std::str::FromStr;

use rust_decimal::Decimal;
use tantra_core::chrono::{DateTime, Utc};
use tantra_core::uuid::Uuid;
use tantra_core::{
    AccountId, Bar, Exchange, InstrumentId, Money, OrderAck, OrderEvent, OrderStatus,
    PortfolioAccount, PortfolioMode, PortfolioPosition, PortfolioSnapshot, PortfolioSource,
    PortfolioTotals, PositionSide, Price, Quantity, ScreenerRow, ScreenerScanResponse, Tick,
};
use tantra_tui::live_data::{LiveData, PortfolioSummary, WatchlistRow};
use tantra_ui_client::{ClientEvent, SystemStatus};

fn sym(symbol: &str) -> InstrumentId {
    InstrumentId::try_new(Exchange::Nse, symbol).unwrap()
}

fn dec(s: &str) -> Decimal {
    Decimal::from_str(s).unwrap()
}

fn price(s: &str) -> Price {
    Price::try_new(dec(s)).unwrap()
}

fn qty(s: &str) -> Quantity {
    Quantity::try_new(dec(s)).unwrap()
}

fn now() -> DateTime<Utc> {
    Utc::now()
}

fn ltp_event(symbol: &str, last: &str, volume: &str) -> ClientEvent {
    ClientEvent::Tick(Tick::Ltp {
        instrument: sym(symbol),
        price: price(last),
        volume: qty(volume),
        exchange_ts: now(),
        receive_ts: now(),
    })
}

#[test]
fn empty_live_data_reports_no_data() {
    let live = LiveData::new();
    assert!(!live.has_live_data());
    assert!(live.watchlist.is_empty());
    assert!(live.screener.is_empty());
    assert!(live.orders.is_empty());
    assert!(live.portfolio.is_none());
    assert!(!live.connected);
}

#[test]
fn ltp_tick_upserts_watchlist_row() {
    let mut live = LiveData::new();
    live.apply_event(&ltp_event("RELIANCE", "2980.50", "100"));

    assert_eq!(live.watchlist.len(), 1);
    let row = &live.watchlist[0];
    assert_eq!(row.symbol, format!("{}", sym("RELIANCE")));
    assert_eq!(row.last, dec("2980.50"));
    // First observation has no prior last, so change is zero.
    assert_eq!(row.change, Decimal::ZERO);
    assert_eq!(row.change_pct, Decimal::ZERO);
    assert_eq!(row.volume, dec("100"));
    assert!(live.has_live_data());
}

#[test]
fn second_tick_updates_existing_row_not_append() {
    let mut live = LiveData::new();
    live.apply_event(&ltp_event("RELIANCE", "100", "10"));
    live.apply_event(&ltp_event("RELIANCE", "110", "20"));

    assert_eq!(
        live.watchlist.len(),
        1,
        "same symbol must upsert, not append"
    );
    let row = &live.watchlist[0];
    assert_eq!(row.last, dec("110"));
    assert_eq!(row.volume, dec("20"));
    assert_eq!(row.change, dec("10"));
    assert_eq!(row.change_pct, dec("10"));
}

#[test]
fn bar_computes_change_against_previous_close() {
    let mut live = LiveData::new();
    let bar = |close: &str| Bar {
        instrument: sym("NIFTY50"),
        open: price(close),
        high: price(close),
        low: price(close),
        close: price(close),
        volume: qty("100"),
        timestamp: now(),
    };

    live.apply_event(&ClientEvent::Bar(bar("100")));
    assert_eq!(live.watchlist.len(), 1);
    let first = &live.watchlist[0];
    assert_eq!(first.last, dec("100"));
    assert_eq!(first.change, Decimal::ZERO, "no previous close yet");

    live.apply_event(&ClientEvent::Bar(bar("105")));
    assert_eq!(
        live.watchlist.len(),
        1,
        "same symbol must upsert, not append"
    );
    let row = &live.watchlist[0];
    assert_eq!(row.last, dec("105"));
    assert_eq!(row.change, dec("5"));
    assert_eq!(row.change_pct, dec("5"));
    assert_eq!(row.volume, qty("100").value());
}

#[test]
fn screener_scan_replaces_hits() {
    let mut live = LiveData::new();
    let scan = ScreenerScanResponse {
        rows: vec![ScreenerRow {
            instrument: sym("RELIANCE"),
            metrics: BTreeMap::from([
                ("pe".to_string(), dec("23.5")),
                ("roce".to_string(), dec("14.2")),
            ]),
        }],
        total_matched: 1,
    };
    live.apply_event(&ClientEvent::Screener(scan));

    assert_eq!(live.screener.len(), 1);
    let hit = &live.screener[0];
    assert_eq!(hit.instrument, format!("{}", sym("RELIANCE")));
    assert_eq!(
        hit.metrics,
        vec![
            ("pe".to_string(), dec("23.5").to_string()),
            ("roce".to_string(), dec("14.2").to_string()),
        ]
    );

    // A second scan replaces the previous hits entirely.
    let empty = ScreenerScanResponse {
        rows: Vec::new(),
        total_matched: 0,
    };
    live.apply_event(&ClientEvent::Screener(empty));
    assert!(live.screener.is_empty());
}

#[test]
fn order_ack_dedupes_by_order_id() {
    let mut live = LiveData::new();
    let ack = |status: OrderStatus| {
        OrderEvent::Acked(OrderAck::new("ORD-1", Uuid::new_v4(), status, now()))
    };

    live.apply_event(&ClientEvent::Order(ack(OrderStatus::New)));
    assert_eq!(live.orders.len(), 1);
    assert_eq!(live.orders[0].order_id, "ORD-1");
    assert_eq!(live.orders[0].kind, "acked");
    assert_eq!(live.orders[0].status, "New");

    // Duplicate ack for the same order id replaces the row, not appends.
    live.apply_event(&ClientEvent::Order(ack(OrderStatus::PartiallyFilled)));
    assert_eq!(live.orders.len(), 1);
    assert_eq!(live.orders[0].status, "PartiallyFilled");

    // A different order id appends a second row.
    let other = OrderEvent::Acked(OrderAck::new(
        "ORD-2",
        Uuid::new_v4(),
        OrderStatus::New,
        now(),
    ));
    live.apply_event(&ClientEvent::Order(other));
    assert_eq!(live.orders.len(), 2);
}

#[test]
fn cancelled_order_appends_row() {
    let mut live = LiveData::new();
    live.apply_event(&ClientEvent::Order(OrderEvent::Cancelled {
        order_id: "ORD-3".to_string(),
    }));
    assert_eq!(live.orders.len(), 1);
    assert_eq!(live.orders[0].order_id, "ORD-3");
    assert_eq!(live.orders[0].kind, "cancelled");
    assert_eq!(live.orders[0].status, "cancelled");
}

fn portfolio_snapshot() -> PortfolioSnapshot {
    PortfolioSnapshot {
        account: PortfolioAccount {
            account_id: AccountId::new("acct-1"),
            mode: PortfolioMode::Paper,
            source: PortfolioSource::Simulated,
        },
        positions: vec![PortfolioPosition {
            instrument: sym("RELIANCE"),
            side: PositionSide::Long,
            quantity: qty("50"),
            avg_price: price("2900"),
            unrealized_pnl: Money::inr(dec("4025.50")),
            realized_pnl: Money::inr(dec("0")),
        }],
        holdings: Vec::new(),
        totals: PortfolioTotals {
            cash: Money::inr(dec("1000000")),
            margin_used: Money::inr(dec("250000")),
            margin_available: Money::inr(dec("750000")),
            total_pnl: Money::inr(dec("4025.50")),
            net_worth: Money::inr(dec("1004025.50")),
        },
        greeks: None,
        timestamp: now(),
    }
}

#[test]
fn portfolio_snapshot_sets_summary() {
    let mut live = LiveData::new();
    live.apply_event(&ClientEvent::Portfolio(portfolio_snapshot()));

    let p = live
        .portfolio
        .as_ref()
        .expect("portfolio summary should be set");
    assert_eq!(p.cash, dec("1000000"));
    assert_eq!(p.net_worth, dec("1004025.50"));
    assert_eq!(p.total_pnl, dec("4025.50"));
    assert_eq!(p.positions, 1);
    assert!(live.has_live_data());
}

#[test]
fn system_status_sets_connected() {
    let mut live = LiveData::new();
    assert!(!live.connected);

    live.apply_event(&ClientEvent::System(SystemStatus {
        healthy: true,
        message: "feed up".to_string(),
    }));
    assert!(live.connected);

    live.apply_event(&ClientEvent::System(SystemStatus {
        healthy: false,
        message: "feed down".to_string(),
    }));
    assert!(!live.connected);
}

#[test]
fn from_seed_roundtrips_state() {
    let seed = LiveData::from_seed(
        vec![WatchlistRow {
            symbol: format!("{}", sym("TCS")),
            last: dec("4000"),
            change: Decimal::ZERO,
            change_pct: Decimal::ZERO,
            volume: dec("0"),
        }],
        Vec::new(),
        Vec::new(),
        Some(PortfolioSummary {
            cash: dec("1"),
            net_worth: dec("2"),
            total_pnl: dec("3"),
            positions: 0,
        }),
    );
    assert_eq!(seed.watchlist.len(), 1);
    assert_eq!(seed.watchlist[0].last, dec("4000"));
    assert_eq!(seed.portfolio.as_ref().unwrap().total_pnl, dec("3"));
    assert!(seed.has_live_data());
}
