//! `tantra paper` — deterministic embedded paper exchange session (plan 0008 D4).
//!
//! `--mode embedded` boots an in-process [`PaperExchange`] (the same engine
//! that ships in the WASM workstation and on the server), runs the canonical
//! golden demo tape, and prints the fills + equity + net position. The output
//! is byte-identical across runs for the same `seed` — the exchange never
//! reads the wall clock; every transition takes an explicit `DateTime<Utc>`.
//!
//! `--mode server` is a thin client: it prints instructions pointing at a
//! `tantra paper serve` instance's REST `/orders` + `/ws/live` stream.

use chrono::{Duration, TimeZone, Utc};
use rust_decimal::Decimal;
use tantra_core::{Exchange, InstrumentId, OrderRequest, OrderSide, OrderType, Price, Quantity};
use tantra_exec_paper::{NetworkLatency, PaperExchange, ZeroFillCost};
use uuid::Uuid;

/// Run the canonical golden demo tape through the in-process paper exchange
/// and return the report text.
///
/// Golden tape (exec plan 0003 E9 / plan 0008 U2): two resting buy limits
/// (100@101, 100@100) then a 150-lot market sell on `NSE:RELIANCE` with
/// `cash` INR. Expected fills (price-time priority): sell 100@101, buy 100@101,
/// sell 50@100, buy 50@100; sell VWAP 100.6667; net position 0; cash returns
/// to the initial balance.
pub fn run_embedded_report(cash: Decimal, seed: Option<u64>) -> String {
    let inst = InstrumentId::new(Exchange::Nse, "RELIANCE");
    let ts0 = Utc.timestamp_opt(1_700_000_000, 0).unwrap();

    let mut ex = PaperExchange::new(cash).with_cost_fn(Box::new(ZeroFillCost));
    if let Some(seed) = seed {
        ex = ex.with_latency(NetworkLatency::new(250, 80, seed));
    }
    ex.set_market_price(inst.clone(), Price::from_rupees("100").expect("price"));

    let buy101 = OrderRequest::with_id(
        Uuid::from_u128(1),
        inst.clone(),
        OrderSide::Buy,
        OrderType::Limit(Price::from_rupees("101").expect("price")),
        Quantity::from_u64(100),
        "cli_paper",
    );
    let buy100 = OrderRequest::with_id(
        Uuid::from_u128(2),
        inst.clone(),
        OrderSide::Buy,
        OrderType::Limit(Price::from_rupees("100").expect("price")),
        Quantity::from_u64(100),
        "cli_paper",
    );
    let sell = OrderRequest::with_id(
        Uuid::from_u128(3),
        inst.clone(),
        OrderSide::Sell,
        OrderType::Market,
        Quantity::from_u64(150),
        "cli_paper",
    );

    let _id_buy101 = ex.submit_order(buy101, ts0).expect("buy101 accepted");
    let _id_buy100 = ex.submit_order(buy100, ts0).expect("buy100 accepted");
    let id_sell = ex.submit_order(sell, ts0).expect("sell accepted");

    if seed.is_some() {
        // Seeded latency: orders become matchable once their simulated
        // network latency has elapsed; advancing the clock settles the tape.
        ex.advance_to(ts0 + Duration::seconds(1));
    }

    let mut out = format!("Paper Trading Session (embedded engine, seed {seed:?})\n");
    out.push_str(&format!("Instrument: {inst}\n"));
    out.push_str("Fills:\n");
    for fill in ex.fills() {
        out.push_str(&format!(
            "  {} {} {:?} {} @ {}\n",
            fill.fill_id, fill.order_id, fill.side, fill.quantity.0, fill.price.0
        ));
    }
    let sell_vwap = ex
        .order(&id_sell)
        .map(|o| o.avg_fill_price.0)
        .unwrap_or(Decimal::ZERO);
    let net = ex.positions().get(&inst).copied().unwrap_or(Decimal::ZERO);
    out.push_str(&format!("Sell VWAP: {sell_vwap}\n"));
    out.push_str(&format!("Net position: {net}\n"));
    out.push_str(&format!("Cash: {}\n", ex.cash()));
    out.push_str(&format!("Equity: {}\n", ex.equity()));
    out
}

/// Print server-mode instructions pointing at a `tantra paper serve`
/// instance's REST `/orders` + `/ws/live` stream (thin client, no auth).
pub fn run_server_instructions(server: Option<&str>) -> String {
    let base = server.unwrap_or("http://127.0.0.1:8000");
    let ws_base = base
        .trim_start_matches("http://")
        .trim_start_matches("https://");
    format!(
        "Paper server mode — point the workstation at a paper server instance (thin client, no auth):\n\
         REST:    POST {base}/orders\n\
                  GET  {base}/orders\n\
                  DELETE {base}/orders/{{id}}\n\
         Stream:  ws://{ws_base}/ws/live  (fills, positions, working orders)\n\
         Start:   `tantra serve <data_dir> --addr 127.0.0.1:8000` in paper mode."
    )
}
