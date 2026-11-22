//! Live-data fold state backing the Watchlist / Screener / Portfolio /
//! Orders tabs (gap 0005).
//!
//! [`LiveData`] is a pure in-memory projection of canonical
//! [`tantra_ui_client::ClientEvent`]s. Rendering stays pure: a feed consumer
//! (e.g. a tokio task draining an `EventStream`) pushes events through
//! `TuiApp::publish`, and the `draw_*` routines only ever read this state —
//! they never fetch, never touch the clock, and never do I/O.

use std::collections::{BTreeMap, VecDeque};

use rust_decimal::Decimal;

use tantra_core::{Bar, FillReport, OrderEvent, ScreenerRow, Tick};
use tantra_ui_client::ClientEvent;

/// One watchlist row, fed by LTP / Quote / Depth ticks and bars.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WatchlistRow {
    /// Stable instrument display string (e.g. `NSE:RELIANCE-EQ`).
    pub symbol: String,
    /// Last observed price (LTP, quote mid, or bar close).
    pub last: Decimal,
    /// Absolute change vs the previously observed last/close.
    pub change: Decimal,
    /// Percentage change vs the previously observed last/close.
    pub change_pct: Decimal,
    /// Traded volume from the last tick/bar that carried one.
    pub volume: Decimal,
}

impl WatchlistRow {
    fn new(symbol: String, last: Decimal, volume: Decimal) -> Self {
        Self {
            symbol,
            last,
            change: Decimal::ZERO,
            change_pct: Decimal::ZERO,
            volume,
        }
    }
}

/// One screener hit: instrument plus its metric map rendered as name → value.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScreenerHit {
    /// Instrument display string.
    pub instrument: String,
    /// Sorted metric cells rendered as `(name, value)` strings.
    pub metrics: Vec<(String, String)>,
}

/// One order row in the Orders tab.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OrderRow {
    /// Broker-assigned order id.
    pub order_id: String,
    /// Event kind that produced/updated this row
    /// (`acked` / `filled` / `rejected` / `cancelled`).
    pub kind: String,
    /// Human-readable order state.
    pub status: String,
}

/// Account-level summary derived from a [`tantra_core::PortfolioSnapshot`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PortfolioSummary {
    /// Free cash balance.
    pub cash: Decimal,
    /// Net worth (cash + MTM value of all holdings/positions).
    pub net_worth: Decimal,
    /// Total realized + unrealized PnL.
    pub total_pnl: Decimal,
    /// Number of open positions.
    pub positions: usize,
}

/// One paper working-order row (plan 0008 D3 paper panel).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PaperOrderRow {
    /// Broker-assigned order id.
    pub order_id: String,
    /// Instrument display string (empty until the first fill enriches it).
    pub instrument: String,
    /// `"Buy"` / `"Sell"` (empty until the first fill enriches it).
    pub side: String,
    /// Filled quantity (zero until the first fill enriches it).
    pub quantity: Decimal,
    /// Fill price (zero until the first fill enriches it).
    pub price: Decimal,
    /// Human-readable order state.
    pub status: String,
}

/// One paper fills-feed row (plan 0008 D3 paper panel).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PaperFillRow {
    /// Broker-assigned order id.
    pub order_id: String,
    /// Instrument display string.
    pub instrument: String,
    /// `"Buy"` / `"Sell"`.
    pub side: String,
    /// Filled quantity.
    pub quantity: Decimal,
    /// Exact fill price.
    pub price: Decimal,
}

/// Best bid/ask for one instrument in the paper depth ladder (plan 0008 D3).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PaperDepthRow {
    /// Instrument display string.
    pub instrument: String,
    /// Best bid, if any.
    pub bid: Option<Decimal>,
    /// Best ask, if any.
    pub ask: Option<Decimal>,
}

/// One paper position row (plan 0008 D3 paper panel).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PaperPositionRow {
    /// Instrument display string.
    pub instrument: String,
    /// Signed net quantity.
    pub quantity: Decimal,
}

/// Live-data state projected from canonical `ClientEvent`s.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LiveData {
    /// Watchlist rows (upserted by symbol).
    pub watchlist: Vec<WatchlistRow>,
    /// Latest screener scan results.
    pub screener: Vec<ScreenerHit>,
    /// Order rows (deduped by order id).
    pub orders: Vec<OrderRow>,
    /// Latest portfolio snapshot summary, if one has arrived.
    pub portfolio: Option<PortfolioSummary>,
    /// Whether the last `System` event reported a healthy feed.
    pub connected: bool,
    /// Paper working orders (deduped by order id; plan 0008 D3).
    pub paper_orders: Vec<PaperOrderRow>,
    /// Paper fills feed (append-only; plan 0008 D3).
    pub paper_fills: Vec<PaperFillRow>,
    /// Paper per-instrument depth ladder (plan 0008 D3).
    pub paper_depth: Vec<PaperDepthRow>,
    /// Paper positions (plan 0008 D3).
    pub paper_positions: Vec<PaperPositionRow>,
    /// Paper free cash balance (INR; plan 0008 D3).
    pub paper_cash: Decimal,
    /// Paper equity (cash + MTM; plan 0008 D3).
    pub paper_equity: Decimal,
    /// Rolling per-symbol price history (oldest first, capped at
    /// [`PRICE_HISTORY_CAP`]) feeding the analysis-page chart.
    pub price_history: BTreeMap<String, VecDeque<Decimal>>,
}

/// Maximum retained price points per symbol.
pub const PRICE_HISTORY_CAP: usize = 240;

impl LiveData {
    /// Create an honest, empty live-data state (no seeded fake rows).
    pub fn new() -> Self {
        Self {
            watchlist: Vec::new(),
            screener: Vec::new(),
            orders: Vec::new(),
            portfolio: None,
            connected: false,
            paper_orders: Vec::new(),
            paper_fills: Vec::new(),
            paper_depth: Vec::new(),
            paper_positions: Vec::new(),
            paper_cash: Decimal::ZERO,
            paper_equity: Decimal::ZERO,
            price_history: BTreeMap::new(),
        }
    }

    /// Seed constructor for tests.
    pub fn from_seed(
        watchlist: Vec<WatchlistRow>,
        screener: Vec<ScreenerHit>,
        orders: Vec<OrderRow>,
        portfolio: Option<PortfolioSummary>,
    ) -> Self {
        Self {
            watchlist,
            screener,
            orders,
            portfolio,
            connected: false,
            paper_orders: Vec::new(),
            paper_fills: Vec::new(),
            paper_depth: Vec::new(),
            paper_positions: Vec::new(),
            paper_cash: Decimal::ZERO,
            paper_equity: Decimal::ZERO,
            price_history: BTreeMap::new(),
        }
    }

    /// Price history for `symbol` (oldest first).
    pub fn history(&self, symbol: &str) -> Option<&VecDeque<Decimal>> {
        self.price_history.get(symbol)
    }

    /// Watchlist quote row for `symbol`.
    pub fn quote(&self, symbol: &str) -> Option<&WatchlistRow> {
        self.watchlist.iter().find(|r| r.symbol == symbol)
    }

    /// Paper depth row for `symbol`.
    pub fn depth(&self, symbol: &str) -> Option<&PaperDepthRow> {
        self.paper_depth.iter().find(|r| r.instrument == symbol)
    }

    /// Paper position row for `symbol`.
    pub fn position(&self, symbol: &str) -> Option<&PaperPositionRow> {
        self.paper_positions.iter().find(|r| r.instrument == symbol)
    }

    /// Replace `symbol`'s price history with `closes` (oldest first),
    /// keeping only the newest [`PRICE_HISTORY_CAP`] points.
    pub fn seed_history(&mut self, symbol: &str, closes: impl IntoIterator<Item = Decimal>) {
        let mut hist: VecDeque<Decimal> = closes.into_iter().collect();
        while hist.len() > PRICE_HISTORY_CAP {
            hist.pop_front();
        }
        self.price_history.insert(symbol.to_string(), hist);
    }

    fn record_price(&mut self, symbol: &str, price: Decimal) {
        let hist = self.price_history.entry(symbol.to_string()).or_default();
        hist.push_back(price);
        while hist.len() > PRICE_HISTORY_CAP {
            hist.pop_front();
        }
    }

    /// True when any live collection is populated — drives the empty-state
    /// message vs. the table inside the `draw_*` routines.
    pub fn has_live_data(&self) -> bool {
        !self.watchlist.is_empty()
            || !self.screener.is_empty()
            || !self.orders.is_empty()
            || self.portfolio.is_some()
            || self.has_paper_data()
    }

    /// True when any paper-specific live data has arrived — drives the
    /// `/positions` panel between the live render and the offline fixture
    /// fallback (plan 0008 D3).
    pub fn has_paper_data(&self) -> bool {
        !self.paper_orders.is_empty()
            || !self.paper_fills.is_empty()
            || !self.paper_depth.is_empty()
            || !self.paper_positions.is_empty()
            || self.paper_cash != Decimal::ZERO
            || self.paper_equity != Decimal::ZERO
    }

    /// Fold one canonical client event into the live-data state.
    ///
    /// Pure: no I/O, no clock reads, no fetches — deterministic given the
    /// same event sequence.
    pub fn apply_event(&mut self, ev: &ClientEvent) {
        match ev {
            ClientEvent::Tick(tick) => self.apply_tick(tick),
            ClientEvent::Bar(bar) => self.apply_bar(bar),
            ClientEvent::Screener(resp) => {
                self.screener = resp.rows.iter().map(row_to_hit).collect();
            }
            ClientEvent::Order(order) => {
                self.apply_order(order);
                self.apply_paper_order(order);
            }
            ClientEvent::Fill(fill) => {
                let row = OrderRow {
                    order_id: fill.order_id.clone(),
                    kind: "filled".to_string(),
                    status: "filled".to_string(),
                };
                self.upsert_order(row);
                self.apply_paper_fill(fill);
            }
            ClientEvent::Portfolio(snapshot) => {
                self.portfolio = Some(PortfolioSummary {
                    cash: snapshot.totals.cash.amount,
                    net_worth: snapshot.totals.net_worth.amount,
                    total_pnl: snapshot.totals.total_pnl.amount,
                    positions: snapshot.positions.len(),
                });
                self.paper_positions = snapshot
                    .positions
                    .iter()
                    .map(|p| PaperPositionRow {
                        instrument: format!("{}", p.instrument),
                        quantity: p.quantity.0,
                    })
                    .collect();
                self.paper_cash = snapshot.totals.cash.amount;
                self.paper_equity = snapshot.totals.net_worth.amount;
            }
            ClientEvent::System(status) => {
                self.connected = status.healthy;
            }
        }
    }

    /// Fold a paper order-lifecycle event into the paper working-orders list.
    fn apply_paper_order(&mut self, order: &OrderEvent) {
        let (order_id, status) = match order {
            OrderEvent::Acked(ack) => (ack.order_id.clone(), format!("{:?}", ack.status)),
            OrderEvent::Filled(fill) => (fill.order_id.clone(), "filled".to_string()),
            OrderEvent::Rejected { order_id, .. } => (order_id.clone(), "rejected".to_string()),
            OrderEvent::Cancelled { order_id } => (order_id.clone(), "cancelled".to_string()),
        };
        self.upsert_paper_order(PaperOrderRow {
            order_id,
            instrument: String::new(),
            side: String::new(),
            quantity: Decimal::ZERO,
            price: Decimal::ZERO,
            status,
        });
    }

    /// Fold a paper fill into the fills feed and enrich its order row.
    fn apply_paper_fill(&mut self, fill: &FillReport) {
        let side = format!("{:?}", fill.side);
        let quantity = fill.quantity.0;
        let price = fill.price.0;
        self.paper_fills.push(PaperFillRow {
            order_id: fill.order_id.clone(),
            instrument: format!("{}", fill.instrument),
            side: side.clone(),
            quantity,
            price,
        });
        self.upsert_paper_order(PaperOrderRow {
            order_id: fill.order_id.clone(),
            instrument: format!("{}", fill.instrument),
            side,
            quantity,
            price,
            status: "filled".to_string(),
        });
    }

    fn apply_tick(&mut self, tick: &Tick) {
        match tick {
            Tick::Ltp {
                instrument,
                price,
                volume,
                ..
            } => {
                self.record_price(&format!("{instrument}"), price.0);
                self.upsert_price(format!("{instrument}"), price.0, Some(volume.0));
            }
            Tick::Quote {
                instrument,
                bid,
                ask,
                last_price,
                ..
            } => {
                let last = last_price.unwrap_or(*bid).0;
                self.upsert_price(format!("{instrument}"), last, None);
                self.upsert_paper_depth(format!("{instrument}"), Some(bid.0), Some(ask.0));
            }
            Tick::Depth {
                instrument,
                bids,
                asks,
                last_price,
                ..
            } => {
                // Depth carries no single LTP in `last_price`; when it is
                // absent the row is left untouched.
                if let Some(price) = last_price {
                    self.upsert_price(format!("{instrument}"), price.0, None);
                }
                let best_bid = bids.first().map(|b| b.price.0);
                let best_ask = asks.first().map(|a| a.price.0);
                self.upsert_paper_depth(format!("{instrument}"), best_bid, best_ask);
            }
        }
    }

    fn apply_bar(&mut self, bar: &Bar) {
        let symbol = format!("{}", bar.instrument);
        let close = bar.close.0;
        self.record_price(&symbol, close);
        let volume = bar.volume.0;
        if let Some(row) = self.watchlist.iter_mut().find(|r| r.symbol == symbol) {
            let prev_close = row.last;
            let change = close - prev_close;
            let change_pct = if prev_close != Decimal::ZERO {
                (change / prev_close) * Decimal::new(100, 0)
            } else {
                Decimal::ZERO
            };
            row.last = close;
            row.change = change;
            row.change_pct = change_pct;
            row.volume = volume;
        } else {
            self.watchlist.push(WatchlistRow {
                symbol,
                last: close,
                change: Decimal::ZERO,
                change_pct: Decimal::ZERO,
                volume,
            });
        }
    }

    fn apply_order(&mut self, order: &OrderEvent) {
        let row = match order {
            OrderEvent::Acked(ack) => OrderRow {
                order_id: ack.order_id.clone(),
                kind: "acked".to_string(),
                status: format!("{:?}", ack.status),
            },
            OrderEvent::Filled(fill) => OrderRow {
                order_id: fill.order_id.clone(),
                kind: "filled".to_string(),
                status: "filled".to_string(),
            },
            OrderEvent::Rejected { order_id, .. } => OrderRow {
                order_id: order_id.clone(),
                kind: "rejected".to_string(),
                status: "rejected".to_string(),
            },
            OrderEvent::Cancelled { order_id } => OrderRow {
                order_id: order_id.clone(),
                kind: "cancelled".to_string(),
                status: "cancelled".to_string(),
            },
        };
        self.upsert_order(row);
    }

    fn upsert_price(&mut self, symbol: String, last: Decimal, new_volume: Option<Decimal>) {
        if let Some(row) = self.watchlist.iter_mut().find(|r| r.symbol == symbol) {
            let change = last - row.last;
            let change_pct = if row.last != Decimal::ZERO {
                (change / row.last) * Decimal::new(100, 0)
            } else {
                Decimal::ZERO
            };
            row.last = last;
            row.change = change;
            row.change_pct = change_pct;
            if let Some(volume) = new_volume {
                row.volume = volume;
            }
        } else {
            self.watchlist.push(WatchlistRow::new(
                symbol,
                last,
                new_volume.unwrap_or_default(),
            ));
        }
    }

    fn upsert_order(&mut self, row: OrderRow) {
        if let Some(existing) = self.orders.iter_mut().find(|o| o.order_id == row.order_id) {
            *existing = row;
        } else {
            self.orders.push(row);
        }
    }

    /// Upsert a paper working-order row by order id.
    fn upsert_paper_order(&mut self, row: PaperOrderRow) {
        if let Some(existing) = self
            .paper_orders
            .iter_mut()
            .find(|o| o.order_id == row.order_id)
        {
            // A later fill enriches the row's instrument/side/quantity/price;
            // an ack preserves a previously filled status otherwise.
            if !row.instrument.is_empty() {
                existing.instrument = row.instrument;
                existing.side = row.side;
                existing.quantity = row.quantity;
                existing.price = row.price;
            }
            existing.status = row.status;
        } else {
            self.paper_orders.push(row);
        }
    }

    /// Upsert a paper depth row by instrument.
    fn upsert_paper_depth(
        &mut self,
        instrument: String,
        bid: Option<Decimal>,
        ask: Option<Decimal>,
    ) {
        if let Some(row) = self
            .paper_depth
            .iter_mut()
            .find(|r| r.instrument == instrument)
        {
            if bid.is_some() {
                row.bid = bid;
            }
            if ask.is_some() {
                row.ask = ask;
            }
        } else {
            self.paper_depth.push(PaperDepthRow {
                instrument,
                bid,
                ask,
            });
        }
    }
}

impl Default for LiveData {
    fn default() -> Self {
        Self::new()
    }
}

/// The offline paper-panel fixture shown until `/api/v2/stream` paper events arrive
/// (plan 0008 D3 keeps the previous hardcoded fixture as the offline
/// fallback; the live render replaces it once the stream delivers data).
pub const PAPER_PANEL_FIXTURE: &str = "Paper Trading Positions:\n  • NIFTY 24500 CE  (100 qty)  LTP: ₹142.50  PnL: +₹3,200 (+2.3%)\n  • RELIANCE        (50 qty)   LTP: ₹2,980.10 PnL: -₹450 (-0.3%)\n  Total MTM: +₹2,750 (Pre-trade margin validated, SEBI risk limits OK)\n  Modal keys: Enter opens positions or order log.";

/// Render the paper panel from the live-data state: working orders, the fills
/// feed, the per-instrument depth ladder, and positions + cash/equity.
///
/// When no paper events have arrived yet (offline), the historical fixture is
/// returned unchanged so the panel never renders blank.
pub fn paper_panel_text(live: &LiveData) -> String {
    if !live.has_paper_data() {
        return PAPER_PANEL_FIXTURE.to_string();
    }

    let mut out = String::from("Paper Trading Positions (live /api/v2/stream):\n");
    if !live.paper_orders.is_empty() {
        out.push_str("Working orders:\n");
        for o in &live.paper_orders {
            out.push_str(&format!(
                "  • {} {} {} {} @ {} [{}]\n",
                o.order_id, o.instrument, o.side, o.quantity, o.price, o.status
            ));
        }
    }
    if !live.paper_fills.is_empty() {
        out.push_str("Fills:\n");
        for f in &live.paper_fills {
            out.push_str(&format!(
                "  • {} {} {} {} @ {}\n",
                f.order_id, f.instrument, f.side, f.quantity, f.price
            ));
        }
    }
    if !live.paper_depth.is_empty() {
        out.push_str("Depth:\n");
        for d in &live.paper_depth {
            let bid = d
                .bid
                .map(|b| b.to_string())
                .unwrap_or_else(|| "-".to_string());
            let ask = d
                .ask
                .map(|a| a.to_string())
                .unwrap_or_else(|| "-".to_string());
            out.push_str(&format!("  • {} bid {bid} / ask {ask}\n", d.instrument));
        }
    }
    out.push_str("Positions:\n");
    if live.paper_positions.is_empty() {
        out.push_str("  • (none)\n");
    } else {
        for p in &live.paper_positions {
            out.push_str(&format!("  • {} {}\n", p.instrument, p.quantity));
        }
    }
    out.push_str(&format!(
        "  Cash: {} | Equity: {}\n",
        live.paper_cash, live.paper_equity
    ));
    out.push_str("  Modal keys: Enter opens positions or order log.");
    out
}

/// Convert one screener row into a [`ScreenerHit`] with a sorted metric list.
fn row_to_hit(row: &ScreenerRow) -> ScreenerHit {
    let mut metrics: Vec<(String, String)> = row
        .metrics
        .iter()
        .map(|(name, value)| (name.clone(), value.to_string()))
        .collect();
    metrics.sort();
    ScreenerHit {
        instrument: format!("{}", row.instrument),
        metrics,
    }
}
