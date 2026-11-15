//! Live-data fold state backing the Watchlist / Screener / Portfolio /
//! Orders tabs (gap 0005).
//!
//! [`LiveData`] is a pure in-memory projection of canonical
//! [`tantra_ui_client::ClientEvent`]s. Rendering stays pure: a feed consumer
//! (e.g. a tokio task draining an `EventStream`) pushes events through
//! `TuiApp::publish`, and the `draw_*` routines only ever read this state —
//! they never fetch, never touch the clock, and never do I/O.

use rust_decimal::Decimal;

use tantra_core::{Bar, OrderEvent, ScreenerRow, Tick};
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
}

impl LiveData {
    /// Create an honest, empty live-data state (no seeded fake rows).
    pub fn new() -> Self {
        Self {
            watchlist: Vec::new(),
            screener: Vec::new(),
            orders: Vec::new(),
            portfolio: None,
            connected: false,
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
        }
    }

    /// True when any live collection is populated — drives the empty-state
    /// message vs. the table inside the `draw_*` routines.
    pub fn has_live_data(&self) -> bool {
        !self.watchlist.is_empty()
            || !self.screener.is_empty()
            || !self.orders.is_empty()
            || self.portfolio.is_some()
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
            ClientEvent::Order(order) => self.apply_order(order),
            ClientEvent::Fill(fill) => {
                let row = OrderRow {
                    order_id: fill.order_id.clone(),
                    kind: "filled".to_string(),
                    status: "filled".to_string(),
                };
                self.upsert_order(row);
            }
            ClientEvent::Portfolio(snapshot) => {
                self.portfolio = Some(PortfolioSummary {
                    cash: snapshot.totals.cash.amount,
                    net_worth: snapshot.totals.net_worth.amount,
                    total_pnl: snapshot.totals.total_pnl.amount,
                    positions: snapshot.positions.len(),
                });
            }
            ClientEvent::System(status) => {
                self.connected = status.healthy;
            }
        }
    }

    fn apply_tick(&mut self, tick: &Tick) {
        match tick {
            Tick::Ltp {
                instrument,
                price,
                volume,
                ..
            } => self.upsert_price(format!("{instrument}"), price.0, Some(volume.0)),
            Tick::Quote {
                instrument,
                bid,
                last_price,
                ..
            } => {
                let last = last_price.unwrap_or(*bid).0;
                self.upsert_price(format!("{instrument}"), last, None);
            }
            Tick::Depth {
                instrument,
                last_price,
                ..
            } => {
                // Depth carries no single LTP in `last_price`; when it is
                // absent the row is left untouched.
                if let Some(price) = last_price {
                    self.upsert_price(format!("{instrument}"), price.0, None);
                }
            }
        }
    }

    fn apply_bar(&mut self, bar: &Bar) {
        let symbol = format!("{}", bar.instrument);
        let close = bar.close.0;
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
}

impl Default for LiveData {
    fn default() -> Self {
        Self::new()
    }
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
