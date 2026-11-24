//! U4 — TUI paper panel on the `/api/v2/stream` socket (plan 0008 D3, design 0006).
//!
//! Feed a stream-shaped batch of canonical `ClientEvent`s through the
//! pure `LiveData` fold and assert the rendered paper panel contains the
//! working orders / fills / depth rows and positions + cash/equity. The
//! offline fixture fallback renders only when no paper events have arrived.

use rust_decimal::Decimal;
use tantra_core::chrono::{DateTime, TimeZone, Utc};
use tantra_core::instrument::PositionSide;
use tantra_core::order::{FillReport, OrderAck, OrderEvent, OrderSide, OrderStatus};
use tantra_core::portfolio::{
    PortfolioAccount, PortfolioMode, PortfolioPosition, PortfolioSnapshot, PortfolioSource,
    PortfolioTotals,
};
use tantra_core::types::{Money, Price, Quantity};
use tantra_core::uuid::Uuid;
use tantra_core::{Exchange, InstrumentId, Tick};
use tantra_tui::live_data::{LiveData, PAPER_PANEL_FIXTURE, paper_panel_text};
use tantra_ui_client::ClientEvent;

const TS0: i64 = 1_700_000_000;

fn reliance() -> InstrumentId {
    InstrumentId::try_new(Exchange::Nse, "RELIANCE").unwrap()
}

fn ts() -> DateTime<Utc> {
    Utc.timestamp_opt(TS0, 0).unwrap()
}

/// A stream-shaped paper batch: two acks, one fill, a quote (depth), and
/// a portfolio snapshot (positions + cash/equity).
fn paper_event_batch() -> Vec<ClientEvent> {
    let inst = reliance();
    let now = ts();
    vec![
        ClientEvent::Order(OrderEvent::Acked(OrderAck::new(
            "PAPER-0001",
            Uuid::from_u128(1),
            OrderStatus::New,
            now,
        ))),
        ClientEvent::Order(OrderEvent::Acked(OrderAck::new(
            "PAPER-0002",
            Uuid::from_u128(2),
            OrderStatus::New,
            now,
        ))),
        ClientEvent::Fill(FillReport::new(
            "F000001",
            "PAPER-0001",
            Uuid::from_u128(1),
            inst.clone(),
            OrderSide::Buy,
            Price::from_rupees("90").unwrap(),
            Quantity::from_u64(100),
            "paper",
            now,
            "paper",
        )),
        ClientEvent::Tick(Tick::Quote {
            instrument: inst.clone(),
            bid: Price::from_rupees("90").unwrap(),
            ask: Price::from_rupees("92").unwrap(),
            bid_qty: Quantity::from_u64(100),
            ask_qty: Quantity::from_u64(150),
            last_price: Some(Price::from_rupees("91").unwrap()),
            band: None,
            exchange_ts: now,
            receive_ts: now,
        }),
        ClientEvent::Portfolio(PortfolioSnapshot {
            account: PortfolioAccount {
                account_id: "paper".to_string(),
                mode: PortfolioMode::Paper,
                source: PortfolioSource::Simulated,
            },
            positions: vec![PortfolioPosition {
                instrument: inst,
                side: PositionSide::Long,
                quantity: Quantity::from_u64(100),
                avg_price: Price::from_rupees("90").unwrap(),
                unrealized_pnl: Money::inr(Decimal::from(100)),
                realized_pnl: Money::inr(Decimal::ZERO),
            }],
            holdings: Vec::new(),
            totals: PortfolioTotals {
                cash: Money::inr(Decimal::new(991_000, 0)),
                margin_used: Money::inr(Decimal::ZERO),
                margin_available: Money::inr(Decimal::ZERO),
                total_pnl: Money::inr(Decimal::from(100)),
                net_worth: Money::inr(Decimal::new(999_800, 0)),
            },
            greeks: None,
            timestamp: now,
        }),
    ]
}

/// The rendered paper panel contains the working orders / fills / depth rows
/// and the positions + cash/equity projected from the event batch.
#[test]
fn paper_panel_renders_live_event_batch() {
    let mut live = LiveData::new();
    for ev in &paper_event_batch() {
        live.apply_event(ev);
    }
    assert!(live.has_paper_data());

    let text = paper_panel_text(&live);

    // Working orders (both acks; PAPER-0001 enriched by its fill).
    assert!(text.contains("PAPER-0001"), "panel:\n{text}");
    assert!(text.contains("PAPER-0002"), "panel:\n{text}");
    assert!(text.contains("[filled]"), "panel:\n{text}");

    // Fills feed.
    assert!(text.contains("Buy 100 @ 90"), "panel:\n{text}");
    assert!(text.contains("NSE:RELIANCE"), "panel:\n{text}");

    // Depth ladder.
    assert!(text.contains("bid 90 / ask 92"), "panel:\n{text}");

    // Positions + cash/equity.
    assert!(text.contains("Positions:"), "panel:\n{text}");
    assert!(text.contains("Cash: 991000"), "panel:\n{text}");
    assert!(text.contains("Equity: 999800"), "panel:\n{text}");

    // The live render replaces the offline fixture.
    assert!(!text.contains("NIFTY 24500 CE"), "panel:\n{text}");
}

/// With no paper events, the panel keeps the offline fixture fallback.
#[test]
fn paper_panel_renders_fixture_when_offline() {
    let live = LiveData::new();
    assert!(!live.has_paper_data());
    assert_eq!(paper_panel_text(&live), PAPER_PANEL_FIXTURE);
}

/// The `/positions` slash command routes through the same renderer.
#[test]
fn positions_slash_command_renders_panel() {
    let mut app = tantra_tui::TuiApp::new();
    app.execute_slash_command("/positions");
    let last = app.chat.last().expect("a message was pushed");
    // No events yet: the offline fixture renders.
    assert!(last.text.contains("Paper Trading Positions"), "{last:?}");
}
