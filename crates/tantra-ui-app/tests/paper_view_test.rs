//! U3 — PaperPanel view-model tests (plan 0008 D2).
//!
//! The pure `view_model` helpers (`paper_order_ticket`, `paper_working_orders`,
//! `paper_fills`, `paper_depth`, `paper_positions_equity`) render an order
//! ticket + working orders + fills + depth + positions/equity from a given
//! `PaperFeature` state — host-testable without leptos rendering. The
//! embedded demo drives the real `PaperSession` engine.

use rust_decimal::Decimal;
use tantra_ui_app::paper::{
    PaperDepthRow, PaperFeature, PaperFill, PaperMode, PaperOrderKind, PaperOrderTicket,
    PaperPosition, PaperSide, PaperWorkingOrder, paper_depth, paper_fills, paper_order_ticket,
    paper_positions_equity, paper_working_orders,
};

/// The order ticket always renders its instrument / side / kind / quantity /
/// price / tif / mode rows.
#[test]
fn paper_order_ticket_renders_all_rows() {
    let feature = PaperFeature::new();
    let rows = paper_order_ticket(&feature);
    let keys: Vec<&str> = rows.iter().map(|(k, _)| k.as_str()).collect();
    assert_eq!(
        keys,
        vec![
            "instrument",
            "side",
            "kind",
            "quantity",
            "price",
            "tif",
            "mode"
        ]
    );
    assert_eq!(rows[0].1, "NSE:RELIANCE");
    assert!(rows.iter().all(|(_, v)| !v.is_empty()));
}

/// A populated state renders working orders, the fills feed, the depth
/// ladder, and positions + cash/equity.
#[test]
fn paper_view_model_renders_fills_depth_and_positions() {
    let mut feature = PaperFeature::new();
    feature.mode = PaperMode::Embedded;
    feature.ticket = PaperOrderTicket {
        instrument: "NSE:RELIANCE".to_string(),
        side: PaperSide::Buy,
        kind: PaperOrderKind::Limit,
        quantity: Decimal::from(100),
        price: Decimal::from(95),
        tif: "Day".to_string(),
    };
    feature.fills = vec![PaperFill {
        order_id: "PAPER-0001".to_string(),
        instrument: "NSE:RELIANCE".to_string(),
        side: PaperSide::Buy,
        quantity: Decimal::from(100),
        price: Decimal::from(90),
    }];
    feature.working_orders = vec![PaperWorkingOrder {
        order_id: "PAPER-0002".to_string(),
        instrument: "NSE:RELIANCE".to_string(),
        side: PaperSide::Buy,
        quantity: Decimal::from(50),
        price: Decimal::from(85),
        state: "Working".to_string(),
    }];
    feature.depth = vec![PaperDepthRow {
        instrument: "NSE:RELIANCE".to_string(),
        bid: Some(Decimal::from(88)),
        ask: Some(Decimal::from(88)),
    }];
    feature.positions = vec![PaperPosition {
        instrument: "NSE:RELIANCE".to_string(),
        quantity: Decimal::from(100),
    }];
    feature.cash = Decimal::new(991_000, 0);
    feature.equity = Decimal::new(999_800, 0);

    let fills = paper_fills(&feature);
    assert_eq!(fills.len(), 1);
    assert!(fills[0].contains("PAPER-0001"), "fill row: {}", fills[0]);
    assert!(fills[0].contains("BUY"), "fill row: {}", fills[0]);
    assert!(fills[0].contains("100 @ 90"), "fill row: {}", fills[0]);

    let orders = paper_working_orders(&feature);
    assert_eq!(orders.len(), 1);
    assert!(orders[0].contains("PAPER-0002"), "order row: {}", orders[0]);
    assert!(orders[0].contains("Working"), "order row: {}", orders[0]);

    let depth = paper_depth(&feature);
    assert_eq!(depth.len(), 1);
    assert!(
        depth[0].contains("bid 88 / ask 88"),
        "depth row: {}",
        depth[0]
    );

    let positions_equity = paper_positions_equity(&feature);
    assert!(
        positions_equity.iter().any(|l| l == "NSE:RELIANCE 100"),
        "positions: {positions_equity:?}"
    );
    assert!(
        positions_equity.iter().any(|l| l == "cash 991000"),
        "positions: {positions_equity:?}"
    );
    assert!(
        positions_equity.iter().any(|l| l == "equity 999800"),
        "positions: {positions_equity:?}"
    );
}

/// The embedded demo drives the real in-browser engine deterministically:
/// two identical runs produce byte-identical feature state, with one fill
/// (100 @ 90), one resting order, depth, and a +100 position.
#[test]
fn embedded_demo_is_deterministic_and_fills() {
    let mut a = PaperFeature::new();
    a.run_embedded_demo();
    let mut b = PaperFeature::new();
    b.run_embedded_demo();

    assert_eq!(a, b);
    assert_eq!(a.fills.len(), 1);
    assert_eq!(a.fills[0].quantity, Decimal::from(100));
    assert_eq!(a.fills[0].price, Decimal::from(90));
    assert_eq!(a.fills[0].side, PaperSide::Buy);
    assert_eq!(a.working_orders.len(), 1);
    assert_eq!(a.working_orders[0].state, "Working");
    assert_eq!(a.depth.len(), 1);
    // Synthetic depth accumulates: best bid stays at the first bar's close
    // (90), best ask moves down to the second bar's close (88).
    assert_eq!(a.depth[0].bid, Some(Decimal::from(90)));
    assert_eq!(a.depth[0].ask, Some(Decimal::from(88)));
    assert_eq!(a.positions.len(), 1);
    assert_eq!(a.positions[0].quantity, Decimal::from(100));
    assert_eq!(a.cash, Decimal::new(991_000, 0));
    assert_eq!(a.equity, Decimal::new(999_800, 0));
}

/// An empty feature renders honest empty rows (no seeded fixtures).
#[test]
fn empty_feature_renders_empty_panels() {
    let feature = PaperFeature::new();
    assert!(paper_fills(&feature).is_empty());
    assert!(paper_working_orders(&feature).is_empty());
    assert!(paper_depth(&feature).is_empty());
    let positions_equity = paper_positions_equity(&feature);
    assert!(positions_equity.iter().any(|l| l == "cash 1000000"));
    assert!(positions_equity.iter().any(|l| l == "equity 1000000"));
}
