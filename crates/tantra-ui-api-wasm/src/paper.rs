//! # व्यापार・paper — in-browser paper trading session
//!
//! A thin, deterministic wrapper around the WASM-portable
//! [`tantra_exec_paper::PaperExchange`] matching engine (exec plan 0003 P0,
//! root plan 0007 T14 P3; plan 0008 D1). One browser tab = one isolated
//! paper account: the exchange is a single-user, zero-I/O, Decimal-only venue
//! that never reads the wall clock — every transition takes an explicit
//! `DateTime<Utc>`, so replaying an identical tape produces byte-identical
//! fills and events in-browser and in native tests.
//!
//! Mirrors `replay.rs`: plain structs + pure functions over `tantra_core`
//! types, no `wasm_bindgen` annotation (the JS bindgen shim lives outside
//! this crate), zero-I/O, `Decimal`-only, `wasm32`-targetable.

use std::collections::HashMap;

use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use tantra_core::{Bar, FillReport, InstrumentId, OrderRequest, Price, Tick};

pub use tantra_exec_paper::{
    ExchangeError, ExchangeEvent, NetworkLatency, OrderState, PaperExchange, ZeroFillCost,
};

/// A single-user paper trading session wrapping the owned matching engine.
///
/// Default fee model is [`ZeroFillCost`] (no statutory charges), keeping fill
/// accounting byte-identical to the golden parity tape.
pub struct PaperSession {
    exchange: PaperExchange,
}

impl PaperSession {
    /// Create a new session with `cash` initial INR balance.
    ///
    /// * `seed: Some(n)` — a seeded in-browser network-latency profile
    ///   (deterministic across runs with the same seed; orders become
    ///   matchable after their simulated latency via [`PaperSession::advance_to`]);
    /// * `seed: None` — fully deterministic zero-latency session (orders
    ///   match immediately at the transition timestamp).
    pub fn new(cash: Decimal, seed: Option<u64>) -> Self {
        let exchange = PaperExchange::new(cash).with_cost_fn(Box::new(ZeroFillCost));
        let exchange = match seed {
            Some(seed) => exchange.with_latency(NetworkLatency::new(250, 80, seed)),
            None => exchange,
        };
        Self { exchange }
    }

    /// Submit an order at explicit time `ts`; returns the broker order id.
    pub fn submit(
        &mut self,
        order: OrderRequest,
        ts: DateTime<Utc>,
    ) -> Result<String, ExchangeError> {
        self.exchange.submit_order(order, ts)
    }

    /// Cancel an active order at explicit time `ts`.
    pub fn cancel_order(&mut self, id: &str, ts: DateTime<Utc>) -> Result<(), ExchangeError> {
        self.exchange.cancel_order(id, ts)
    }

    /// Feed a bar into the session's market data.
    ///
    /// Note: the engine derives its matching timestamp from the bar itself
    /// (`bar.timestamp`); `_ts` is accepted for signature uniformity with the
    /// other session transitions.
    pub fn feed_bar(&mut self, bar: &Bar, _ts: DateTime<Utc>) {
        self.exchange.feed_bar(bar);
    }

    /// Feed a tick into the session's market data (matching time comes from
    /// the tick's receive timestamp).
    pub fn feed_tick(&mut self, tick: &Tick) {
        self.exchange.feed_tick(tick);
    }

    /// Advance the simulated clock to `ts`: cancels expired GTD orders and
    /// matches any order whose simulated network latency has elapsed.
    pub fn advance_to(&mut self, ts: DateTime<Utc>) {
        self.exchange.advance_to(ts);
    }

    /// Current best bid/ask for an instrument (feed-book depth), if any.
    pub fn depth(&self, instrument: &InstrumentId) -> Option<(Option<Price>, Option<Price>)> {
        self.exchange.depth(instrument)
    }

    /// Every fill produced so far, in order.
    pub fn fills(&self) -> &[FillReport] {
        self.exchange.fills()
    }

    /// Every exchange event produced so far, in order.
    pub fn events(&self) -> &[ExchangeEvent] {
        self.exchange.events()
    }

    /// Signed net positions per instrument.
    pub fn positions(&self) -> &HashMap<InstrumentId, Decimal> {
        self.exchange.positions()
    }

    /// Current free cash balance (INR).
    pub fn cash(&self) -> Decimal {
        self.exchange.cash()
    }

    /// Equity = cash + Σ(position qty × last price).
    pub fn equity(&self) -> Decimal {
        self.exchange.equity()
    }

    /// The lifecycle state of a tracked order, if known.
    pub fn order_state(&self, id: &str) -> Option<OrderState> {
        self.exchange.order(id).map(|o| o.state)
    }

    /// The average (VWAP) fill price of a tracked order, if known.
    pub fn order_avg_fill_price(&self, id: &str) -> Option<Price> {
        self.exchange.order(id).map(|o| o.avg_fill_price)
    }
}
