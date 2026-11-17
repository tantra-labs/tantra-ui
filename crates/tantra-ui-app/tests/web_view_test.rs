//! View-layer tests for `tantra_ui_app::web` (gap 0004 / T10.2).
//!
//! These tests deliberately avoid leptos rendering: the `csr`-only feature
//! surface (see `crates/tantra-ui-app/Cargo.toml`) has no host-side
//! `render_to_string` — that would need the `ssr` feature we avoid to keep
//! the wasm32-unknown-unknown invariant minimal. Instead they assert on:
//!   * the pure `view_model` helpers (`route_title`, `watchlist_lines`,
//!     `portfolio_lines`, `screener_predicate_text`, ...),
//!   * the router wiring and full 9-view coverage of [`AppRoute::ALL`],
//!   * compile-level coverage — the host `cargo check -p tantra-ui-app`
//!     proves the `view!` components in `src/web.rs` themselves compile.

use rust_decimal::Decimal;
use tantra_core::screener::FilterOp;
use tantra_core::types::{Exchange, InstrumentId, Price};
use tantra_ui_app::web::{
    algos_lines, backtest_lines, filter_op_label, portfolio_lines, route_title,
    screener_predicate_text, simulator_lines, studio_kind_label, system_lines, watchlist_lines,
};
use tantra_ui_app::{
    AlgosFeature, AppRoute, BacktestFeature, NodeId, PortfolioFeature, Router, RouterError,
    ScreenerFeature, SimulatorFeature, StudioCanvas, StudioNode, StudioNodeKind, SystemFeature,
    WatchlistFeature,
};
use tantra_ui_core::atoms::Delta;
use tantra_ui_core::home::WatchlistRow;

/// Every route has a `label (path)` title containing both label and path.
#[test]
fn route_title_maps_every_route() {
    for route in AppRoute::ALL {
        let title = route_title(route);
        assert!(
            title.contains(route.label()),
            "title {title:?} lacks label {}",
            route.label()
        );
        assert!(
            title.contains(route.path()),
            "title {title:?} lacks path {}",
            route.path()
        );
    }
}

/// `AppRoute::ALL` covers exactly the nine views, all distinct.
#[test]
fn all_routes_are_distinct_and_cover_nine_views() {
    assert_eq!(AppRoute::ALL.len(), 9);
    let mut seen = Vec::new();
    for route in AppRoute::ALL {
        assert!(!seen.contains(&route), "duplicate route in ALL: {route:?}");
        seen.push(route);
    }
}

/// A `Router` can be driven through every view; back() and AlreadyActive work.
#[test]
fn router_drives_all_views() {
    let mut router = Router::new(AppRoute::Home);
    let mut visited = vec![router.active];
    for route in AppRoute::ALL {
        if route != AppRoute::Home {
            router.navigate(route).expect("navigate to a new route");
            visited.push(router.active);
        }
    }
    assert_eq!(visited, AppRoute::ALL.to_vec());

    // Navigating to the already-active route is an error.
    assert_eq!(
        router.navigate(router.active),
        Err(RouterError::AlreadyActive)
    );

    // Back returns to the route active before the last navigation.
    router.back();
    assert_eq!(router.active, AppRoute::Algos);
}

/// Screener comparison operators render as symbols.
#[test]
fn filter_op_labels_are_symbolic() {
    assert_eq!(filter_op_label(FilterOp::Eq), "=");
    assert_eq!(filter_op_label(FilterOp::Ne), "!=");
    assert_eq!(filter_op_label(FilterOp::Gt), ">");
    assert_eq!(filter_op_label(FilterOp::Gte), ">=");
    assert_eq!(filter_op_label(FilterOp::Lt), "<");
    assert_eq!(filter_op_label(FilterOp::Lte), "<=");
    assert_eq!(filter_op_label(FilterOp::Between), "between");
    assert_eq!(filter_op_label(FilterOp::In), "in");
    assert_eq!(filter_op_label(FilterOp::NotIn), "not in");
    assert_eq!(filter_op_label(FilterOp::Like), "like");
    assert_eq!(filter_op_label(FilterOp::Has), "has");
    assert_eq!(filter_op_label(FilterOp::CrossesAbove), "crosses above");
    assert_eq!(filter_op_label(FilterOp::CrossesBelow), "crosses below");
}

/// Drafted filter predicates render as `"metric op value"` text.
#[test]
fn screener_predicate_text_renders_filters() {
    let mut feature = ScreenerFeature::new();
    feature
        .add_filter("volume", FilterOp::Gt, Decimal::from(1000), None)
        .expect("valid metric");
    feature
        .add_filter(
            "rsi_14",
            FilterOp::Between,
            Decimal::from(30),
            Some(Decimal::from(70)),
        )
        .expect("valid metric");

    let texts: Vec<String> = feature
        .request
        .filters
        .predicates()
        .into_iter()
        .map(screener_predicate_text)
        .collect();

    assert_eq!(texts, vec!["volume > 1000", "rsi_14 between 30 and 70"]);
}

/// Watchlist rows render as `(instrument, last/delta/spark summary)` lines.
#[test]
fn watchlist_lines_render_rows() {
    let mut feature = WatchlistFeature::new();
    assert!(watchlist_lines(&feature).is_empty());

    let instrument = InstrumentId::new(Exchange::Nse, "RELIANCE");
    let row = WatchlistRow::new(
        instrument.clone(),
        "Reliance Industries",
        Price::new(Decimal::from(2500)),
        Delta::new(Decimal::from(50), Decimal::from(2)),
    );
    feature.upsert_row(row);

    let lines = watchlist_lines(&feature);
    assert_eq!(lines.len(), 1);
    assert_eq!(lines[0].0, instrument.to_string());
    assert!(lines[0].1.contains("last 2500"), "line: {}", lines[0].1);
    assert!(lines[0].1.contains("delta 50"), "line: {}", lines[0].1);
    assert!(lines[0].1.contains("spark 0"), "line: {}", lines[0].1);
}

/// Portfolio totals always render all five money rows, even when empty.
#[test]
fn portfolio_lines_always_include_totals() {
    let feature = PortfolioFeature::new();
    let lines = portfolio_lines(&feature);
    let keys: Vec<&str> = lines.iter().map(|(k, _)| k.as_str()).collect();
    assert_eq!(
        keys,
        vec![
            "cash",
            "margin_used",
            "margin_available",
            "total_pnl",
            "net_worth"
        ]
    );
    assert!(!feature.has_data());
    assert!(lines.iter().all(|(_, v)| !v.is_empty()));
}

/// An empty backtest feature renders an empty-results summary.
#[test]
fn backtest_lines_render_empty_state() {
    let feature = BacktestFeature::new();
    let lines = backtest_lines(&feature);
    assert_eq!(
        lines.iter().find(|(k, _)| k == "equity_points").unwrap().1,
        "0"
    );
    assert_eq!(
        lines
            .iter()
            .find(|(k, _)| k == "total_return_pct")
            .unwrap()
            .1,
        "n/a"
    );
    assert!(lines.iter().any(|(k, _)| k == "status"));
}

/// A fresh simulator renders its idle phase and zeroed metrics.
#[test]
fn simulator_lines_render_idle_state() {
    let feature = SimulatorFeature::new();
    let lines = simulator_lines(&feature);
    assert_eq!(lines.iter().find(|(k, _)| k == "phase").unwrap().1, "Idle");
    assert_eq!(
        lines.iter().find(|(k, _)| k == "equity_points").unwrap().1,
        "0"
    );
    assert_eq!(lines.iter().find(|(k, _)| k == "sharpe").unwrap().1, "0");
}

/// The default system feature renders its five healthy subsystems.
#[test]
fn system_lines_render_default_subsystems() {
    let system = SystemFeature::default_system();
    assert!(system.all_healthy());
    let lines = system_lines(&system);
    assert_eq!(lines.len(), 5);
    assert!(lines.iter().all(|(_, v)| v.contains("[ok]")));
}

/// An empty algo manager renders zero counts and no selection.
#[test]
fn algos_lines_render_empty_state() {
    let feature = AlgosFeature::new();
    let lines = algos_lines(&feature);
    assert_eq!(lines.iter().find(|(k, _)| k == "total").unwrap().1, "0");
    assert_eq!(lines.iter().find(|(k, _)| k == "live").unwrap().1, "0");
    assert_eq!(
        lines.iter().find(|(k, _)| k == "selected").unwrap().1,
        "none"
    );
}

/// Studio node kinds render descriptive labels, and a small canvas counts.
#[test]
fn studio_kind_labels_render() {
    let mut canvas = StudioCanvas::new();
    canvas.add_node(StudioNode::new(
        NodeId(1),
        StudioNodeKind::Source(InstrumentId::new(Exchange::Nse, "NIFTY50")),
        "NIFTY",
    ));
    canvas.add_node(StudioNode::new(
        NodeId(2),
        StudioNodeKind::Indicator("sma".to_string()),
        "SMA",
    ));
    canvas.connect(NodeId(1), NodeId(2)).expect("connect");
    assert_eq!(canvas.node_count(), 2);
    assert_eq!(canvas.connection_count(), 1);

    let source = studio_kind_label(&StudioNodeKind::Source(InstrumentId::new(
        Exchange::Nse,
        "NIFTY50",
    )));
    assert!(source.starts_with("source"), "label: {source}");
    assert_eq!(
        studio_kind_label(&StudioNodeKind::Indicator("sma".to_string())),
        "indicator sma"
    );
    assert_eq!(
        studio_kind_label(&StudioNodeKind::Constant(Decimal::from(10))),
        "constant 10"
    );
}
