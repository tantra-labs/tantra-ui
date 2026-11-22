//! Paper trading feature (plan 0008 D2): order ticket + working orders /
//! fills / depth / positions for the in-browser (embedded) and server-backed
//! paper exchange. Pure `view_model` helpers render from [`PaperFeature`]
//! state so tests stay unit-level and leptos rendering stays thin.
//!
//! Two modes:
//! - **Embedded**: drives a [`tantra_ui_api_wasm::paper::PaperSession`] — the
//!   real WASM-portable matching engine, fully offline and deterministic.
//! - **Server**: projects the same `/api/v2/stream` paper events the TUI
//!   consumes (fills / positions / working orders from a paper server).

use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use tantra_core::{
    Bar, Exchange, InstrumentId, OrderRequest, OrderSide, OrderType, Price, Quantity,
};
use tantra_ui_api_wasm::paper::PaperSession;
use uuid::Uuid;

/// Where the paper session runs.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PaperMode {
    /// In-browser `PaperSession` (offline, deterministic).
    Embedded,
    /// A `tantra-runtime-api-rest` paper server (`/api/v2/orders` + `/api/v2/stream`).
    Server,
}

/// Order ticket side.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum PaperSide {
    /// Buy order.
    Buy,
    /// Sell order.
    Sell,
}

/// Order ticket kind.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum PaperOrderKind {
    /// Limit order (fill at `price` or better).
    Limit,
    /// Market order (fill at the prevailing feed price).
    Market,
}

/// The drafted order ticket.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PaperOrderTicket {
    /// Instrument display string (e.g. `NSE:RELIANCE`).
    pub instrument: String,
    /// Order direction.
    pub side: PaperSide,
    /// Limit or market.
    pub kind: PaperOrderKind,
    /// Quantity to trade.
    pub quantity: Decimal,
    /// Limit price (`0` for market orders).
    pub price: Decimal,
    /// Time in force (`"Day"` or `"IOC"`).
    pub tif: String,
}

/// One working (active) order row.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PaperWorkingOrder {
    /// Broker-assigned order id.
    pub order_id: String,
    /// Instrument display string.
    pub instrument: String,
    /// Order direction.
    pub side: PaperSide,
    /// Remaining (unfilled) quantity.
    pub quantity: Decimal,
    /// Working limit price.
    pub price: Decimal,
    /// Human-readable lifecycle state.
    pub state: String,
}

/// One execution fill row.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PaperFill {
    /// Broker-assigned order id.
    pub order_id: String,
    /// Instrument display string.
    pub instrument: String,
    /// Fill direction.
    pub side: PaperSide,
    /// Filled quantity.
    pub quantity: Decimal,
    /// Exact fill price.
    pub price: Decimal,
}

/// Best bid/ask ladder row for one instrument.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PaperDepthRow {
    /// Instrument display string.
    pub instrument: String,
    /// Best bid, if any.
    pub bid: Option<Decimal>,
    /// Best ask, if any.
    pub ask: Option<Decimal>,
}

/// One open position row.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PaperPosition {
    /// Instrument display string.
    pub instrument: String,
    /// Signed net quantity (positive long, negative short).
    pub quantity: Decimal,
}

/// The paper trading feature state.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PaperFeature {
    /// Which session backend this feature drives.
    pub mode: PaperMode,
    /// The drafted order ticket.
    pub ticket: PaperOrderTicket,
    /// Active working orders.
    pub working_orders: Vec<PaperWorkingOrder>,
    /// The fills feed (newest last).
    pub fills: Vec<PaperFill>,
    /// Per-instrument depth ladder.
    pub depth: Vec<PaperDepthRow>,
    /// Open positions.
    pub positions: Vec<PaperPosition>,
    /// Free cash balance (INR).
    pub cash: Decimal,
    /// Equity = cash + Σ(position qty × last price).
    pub equity: Decimal,
}

impl Default for PaperFeature {
    fn default() -> Self {
        Self::new()
    }
}

impl PaperFeature {
    /// A fresh feature: embedded mode, a default RELIANCE limit-buy ticket,
    /// empty rows, seeded 1,000,000 INR cash.
    pub fn new() -> Self {
        Self {
            mode: PaperMode::Embedded,
            ticket: PaperOrderTicket {
                instrument: "NSE:RELIANCE".to_string(),
                side: PaperSide::Buy,
                kind: PaperOrderKind::Limit,
                quantity: Decimal::from(100u64),
                price: Decimal::from(95u64),
                tif: "Day".to_string(),
            },
            working_orders: Vec::new(),
            fills: Vec::new(),
            depth: Vec::new(),
            positions: Vec::new(),
            cash: Decimal::from(1_000_000u64),
            equity: Decimal::from(1_000_000u64),
        }
    }

    /// Run the deterministic embedded demo tape through the real in-browser
    /// `PaperSession` engine and project the result into this feature:
    ///
    /// 1. buy limit 100 @ 95;
    /// 2. a bar at 90 crosses below the limit → filled 100 @ 90;
    /// 3. buy limit 50 @ 85 rests below the market;
    /// 4. a bar at 88 leaves the book depth at 88.
    ///
    /// Byte-identical across runs (no latency, zero fees).
    pub fn run_embedded_demo(&mut self) {
        let inst = InstrumentId::new(Exchange::Nse, "RELIANCE");
        let ts0 = chrono::DateTime::from_timestamp(1_700_000_000, 0).expect("valid demo ts");
        let mut session = PaperSession::new(self.cash, None);

        let _id1 = session
            .submit(
                OrderRequest::with_id(
                    Uuid::from_u128(1),
                    inst.clone(),
                    OrderSide::Buy,
                    OrderType::Limit(Price::from_rupees("95").expect("price")),
                    Quantity::from_u64(100),
                    "ui_paper_demo",
                ),
                ts0,
            )
            .expect("buy limit accepted");

        // Bar crossing below the limit makes the buy marketable at the close.
        let bar90 = demo_bar(&inst, "95", "95", "85", "90", 1000, ts0);
        session.feed_bar(&bar90, ts0);

        let id2 = session
            .submit(
                OrderRequest::with_id(
                    Uuid::from_u128(2),
                    inst.clone(),
                    OrderSide::Buy,
                    OrderType::Limit(Price::from_rupees("85").expect("price")),
                    Quantity::from_u64(50),
                    "ui_paper_demo",
                ),
                ts0,
            )
            .expect("resting buy accepted");

        // A second bar keeps the resting order below the market and refreshes depth.
        let bar88 = demo_bar(&inst, "90", "90", "84", "88", 800, ts0);
        session.feed_bar(&bar88, ts0);

        // Project the engine state into the feature view model.
        let id2_state = session
            .order_state(&id2)
            .map(|s| format!("{s:?}"))
            .unwrap_or_else(|| "n/a".to_string());
        self.working_orders = vec![PaperWorkingOrder {
            order_id: id2,
            instrument: inst.to_string(),
            side: PaperSide::Buy,
            quantity: Decimal::from(50u64),
            price: Decimal::from(85u64),
            state: id2_state,
        }];
        self.fills = session
            .fills()
            .iter()
            .map(|f| PaperFill {
                order_id: f.order_id.clone(),
                instrument: f.instrument.to_string(),
                side: match f.side {
                    OrderSide::Buy => PaperSide::Buy,
                    OrderSide::Sell => PaperSide::Sell,
                },
                quantity: f.quantity.0,
                price: f.price.0,
            })
            .collect();
        self.depth = match session.depth(&inst) {
            Some((bid, ask)) => vec![PaperDepthRow {
                instrument: inst.to_string(),
                bid: bid.map(|p| p.0),
                ask: ask.map(|p| p.0),
            }],
            None => Vec::new(),
        };
        self.positions = session
            .positions()
            .iter()
            .map(|(inst, qty)| PaperPosition {
                instrument: inst.to_string(),
                quantity: *qty,
            })
            .collect();
        self.cash = session.cash();
        self.equity = session.equity();
    }
}

/// Build a demo bar (plan 0008 D2 embedded tape).
fn demo_bar(
    inst: &InstrumentId,
    open: &str,
    high: &str,
    low: &str,
    close: &str,
    volume: u64,
    ts: chrono::DateTime<chrono::Utc>,
) -> Bar {
    Bar {
        instrument: inst.clone(),
        open: Price::from_rupees(open).expect("price"),
        high: Price::from_rupees(high).expect("price"),
        low: Price::from_rupees(low).expect("price"),
        close: Price::from_rupees(close).expect("price"),
        volume: Quantity::from_u64(volume),
        timestamp: ts,
    }
}

/// Short side label (`BUY` / `SELL`).
fn side_label(side: PaperSide) -> &'static str {
    match side {
        PaperSide::Buy => "BUY",
        PaperSide::Sell => "SELL",
    }
}

// ---------------------------------------------------------------------------
// Pure `view_model` helpers — host-testable without leptos rendering.
// ---------------------------------------------------------------------------

/// Order ticket rows as `(label, value)`.
pub fn paper_order_ticket(state: &PaperFeature) -> Vec<(String, String)> {
    vec![
        ("instrument".to_string(), state.ticket.instrument.clone()),
        ("side".to_string(), format!("{:?}", state.ticket.side)),
        ("kind".to_string(), format!("{:?}", state.ticket.kind)),
        ("quantity".to_string(), state.ticket.quantity.to_string()),
        ("price".to_string(), state.ticket.price.to_string()),
        ("tif".to_string(), state.ticket.tif.clone()),
        ("mode".to_string(), format!("{:?}", state.mode)),
    ]
}

/// Working (active) order rows as text.
pub fn paper_working_orders(state: &PaperFeature) -> Vec<String> {
    state
        .working_orders
        .iter()
        .map(|o| {
            format!(
                "{} {} {} {} @ {} [{}]",
                o.order_id,
                o.instrument,
                side_label(o.side),
                o.quantity,
                o.price,
                o.state
            )
        })
        .collect()
}

/// Fills feed rows as text.
pub fn paper_fills(state: &PaperFeature) -> Vec<String> {
    state
        .fills
        .iter()
        .map(|f| {
            format!(
                "{} {} {} {} @ {}",
                f.order_id,
                f.instrument,
                side_label(f.side),
                f.quantity,
                f.price
            )
        })
        .collect()
}

/// Depth ladder rows as text.
pub fn paper_depth(state: &PaperFeature) -> Vec<String> {
    state
        .depth
        .iter()
        .map(|d| {
            let bid = d
                .bid
                .map(|b| b.to_string())
                .unwrap_or_else(|| "-".to_string());
            let ask = d
                .ask
                .map(|a| a.to_string())
                .unwrap_or_else(|| "-".to_string());
            format!("{} bid {bid} / ask {ask}", d.instrument)
        })
        .collect()
}

/// Positions + cash/equity rows as text.
pub fn paper_positions_equity(state: &PaperFeature) -> Vec<String> {
    let mut lines: Vec<String> = state
        .positions
        .iter()
        .map(|p| format!("{} {}", p.instrument, p.quantity))
        .collect();
    lines.push(format!("cash {}", state.cash));
    lines.push(format!("equity {}", state.equity));
    lines
}
