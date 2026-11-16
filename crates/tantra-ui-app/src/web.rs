//! # व्यापार・face tantra-ui-app web
//!
//! The Leptos 0.8 **client-side-rendered (CSR)** view layer for the
//! workstation (gap 0004 / T10.2). Renders the eight [`AppRoute`] views from
//! the in-memory feature-state models (`ScreenerFeature`, `StudioCanvas`,
//! `SimulatorFeature`, `SystemFeature`, `BacktestFeature`,
//! `PortfolioFeature`, `AlgosFeature`, `WatchlistFeature`).
//!
//! WASM-safe by construction: components are thin presentational wrappers —
//! no I/O, no network, no clock, no `tokio`, no OS threads. Every financial
//! value stays `rust_decimal::Decimal`. The `csr`-only feature surface (see
//! `Cargo.toml`) keeps the crate compilable to `wasm32-unknown-unknown`
//! (deliberately no `ssr`/`hydrate`).
//!
//! The pure `view_model` helpers (`route_title`, `watchlist_lines`,
//! `portfolio_lines`, `screener_predicate_text`, ...) are host-testable
//! without leptos rendering; the components stay thin over them.

#[cfg(target_arch = "wasm32")]
use leptos::mount::mount_to_body;
use leptos::prelude::*;
use tantra_core::screener::{FilterOp, ScreenerFilterPredicate};
use tantra_core::types::Money;
use tantra_ui_core::backtest::BacktestSummary;

use crate::algos::AlgosFeature;
use crate::backtest::BacktestFeature;
use crate::paper::{
    PaperFeature, paper_depth, paper_fills, paper_order_ticket, paper_positions_equity,
    paper_working_orders,
};
use crate::portfolio::PortfolioFeature;
use crate::router::{AppRoute, Router};
use crate::screener::ScreenerFeature;
use crate::simulator::SimulatorFeature;
use crate::studio::{StudioCanvas, StudioNodeKind};
use crate::system::{SubsystemState, SystemFeature};
use crate::watchlist::WatchlistFeature;

/// Mount the whole app into the document body.
///
/// WASM-host entry point only: on non-wasm targets the host (tests, native
/// shell) never mounts the DOM, so the binding stays behind a target gate.
#[cfg(target_arch = "wasm32")]
pub fn mount_app() {
    mount_to_body(App);
}

/// The application shell: a navigation bar over [`AppRoute::ALL`] plus the
/// per-route view below.
#[component]
pub fn App() -> impl IntoView {
    let router = RwSignal::new(Router::default_router());
    provide_context(router);

    let active = move || router.get().active;

    view! {
        <div class="tantra-app">
            <nav class="tantra-nav">
                {AppRoute::ALL
                    .into_iter()
                    .map(|route| {
                        view! {
                            <button
                                class="tantra-nav-btn"
                                on:click=move |_| {
                                    router.update(|r| {
                                        let _ = r.navigate(route);
                                    });
                                }
                            >
                                {route.label()}
                            </button>
                        }
                    })
                    .map(|v| v.into_any())
                    .collect::<Vec<_>>()}
            </nav>
            <main class="tantra-main">
                {move || match active() {
                    AppRoute::Home => view! { <HomeView/> }.into_any(),
                    AppRoute::Screener => view! { <ScreenerView/> }.into_any(),
                    AppRoute::Studio => view! { <StudioView/> }.into_any(),
                    AppRoute::Simulator => view! { <SimulatorView/> }.into_any(),
                    AppRoute::System => view! { <SystemView/> }.into_any(),
                    AppRoute::Backtest => view! { <BacktestView/> }.into_any(),
                    AppRoute::Portfolio => view! { <PortfolioView/> }.into_any(),
                    AppRoute::Algos => view! { <AlgosView/> }.into_any(),
                    AppRoute::Paper => view! { <PaperView/> }.into_any(),
                }}
            </main>
        </div>
    }
}

/// Home dashboard: active route label plus short system & watchlist status.
#[component]
fn HomeView() -> impl IntoView {
    let router = expect_context::<RwSignal<Router>>();
    let system_lines = system_lines(&SystemFeature::default_system());
    let watchlist_lines = watchlist_lines(&WatchlistFeature::new());

    view! {
        <section class="tantra-panel">
            <h1>{move || format!("{} / {}", router.get().active.label(), router.get().active.path())}</h1>
            <h2>"System"</h2>
            <ul>
                {system_lines
                    .into_iter()
                    .map(|(k, v)| view! { <li><strong>{k}</strong>": " {v}</li> })
                    .map(|v| v.into_any())
                    .collect::<Vec<_>>()}
            </ul>
            <h2>"Watchlist"</h2>
            <ul>
                {watchlist_lines
                    .into_iter()
                    .map(|(k, v)| view! { <li><strong>{k}</strong>": " {v}</li> })
                    .map(|v| v.into_any())
                    .collect::<Vec<_>>()}
            </ul>
        </section>
    }
}

/// Neural AST screener: drafted filter predicates + scan results.
#[component]
fn ScreenerView() -> impl IntoView {
    let feature = RwSignal::new(ScreenerFeature::new());

    view! {
        <section class="tantra-panel">
            <h1>{route_title(AppRoute::Screener)}</h1>
            <button
                on:click=move |_| {
                    feature.update(|f| f.run_scan(&[]));
                }
            >
                "Run scan"
            </button>
            <p>{move || if feature.get().scanning { "scanning…" } else { "idle" }}</p>
            <h2>"Filters"</h2>
            <ul>
                {move || {
                    feature
                        .get()
                        .request
                        .filters
                        .predicates
                        .iter()
                        .map(screener_predicate_text)
                        .map(|text| view! { <li>{text}</li> })
                        .map(|v| v.into_any())
                        .collect::<Vec<_>>()
                }}
            </ul>
            <h2>"Results"</h2>
            <p>{move || format!("{} matched", feature.get().view.total_matched)}</p>
            <ul>
                {move || {
                    feature
                        .get()
                        .view
                        .rows
                        .iter()
                        .map(|r| r.instrument.to_string())
                        .map(|text| view! { <li>{text}</li> })
                        .map(|v| v.into_any())
                        .collect::<Vec<_>>()
                }}
            </ul>
        </section>
    }
}

/// Strategy Studio: node count, node list, and connection count.
#[component]
fn StudioView() -> impl IntoView {
    let canvas = RwSignal::new(StudioCanvas::new());

    view! {
        <section class="tantra-panel">
            <h1>{route_title(AppRoute::Studio)}</h1>
            <p>{move || format!("{} nodes, {} connections", canvas.get().node_count(), canvas.get().connection_count())}</p>
            <ul>
                {move || {
                    let canvas = canvas.get();
                    canvas
                        .nodes
                        .values()
                        .map(|n| {
                            let kind = studio_kind_label(&n.kind);
                            view! {
                                <li>
                                    {n.label.clone()}
                                    " [" {kind} "] @ (" {n.x} ", " {n.y} ")"
                                </li>
                            }
                        })
                        .map(|v| v.into_any())
                        .collect::<Vec<_>>()
                }}
            </ul>
        </section>
    }
}

/// Simulator workbench: run phase + summary metrics.
#[component]
fn SimulatorView() -> impl IntoView {
    let simulator = RwSignal::new(SimulatorFeature::new());

    view! {
        <section class="tantra-panel">
            <h1>{route_title(AppRoute::Simulator)}</h1>
            {move || {
                simulator_lines(&simulator.get())
                    .into_iter()
                    .map(|(k, v)| view! { <p><strong>{k}</strong>": " {v}</p> })
                    .map(|v| v.into_any())
                    .collect::<Vec<_>>()
            }}
        </section>
    }
}

/// System health panel: per-subsystem status rows.
#[component]
fn SystemView() -> impl IntoView {
    let system = RwSignal::new(SystemFeature::default_system());

    view! {
        <section class="tantra-panel">
            <h1>{route_title(AppRoute::System)}</h1>
            <p>{move || if system.get().all_healthy() { "all subsystems healthy" } else { "degraded" }}</p>
            <ul>
                {move || {
                    let system = system.get();
                    system
                        .subsystems
                        .into_iter()
                        .map(|s| {
                            let state = format!("{:?}", s.state);
                            view! { <li>{s.name} ": " {s.detail} " [" {state} "]"</li> }
                        })
                        .map(|v| v.into_any())
                        .collect::<Vec<_>>()
                }}
            </ul>
        </section>
    }
}

/// Backtest results viewer: run status and summary lines.
#[component]
fn BacktestView() -> impl IntoView {
    let backtest = RwSignal::new(BacktestFeature::new());

    view! {
        <section class="tantra-panel">
            <h1>{route_title(AppRoute::Backtest)}</h1>
            {move || {
                backtest_lines(&backtest.get())
                    .into_iter()
                    .map(|(k, v)| view! { <p><strong>{k}</strong>": " {v}</p> })
                    .map(|v| v.into_any())
                    .collect::<Vec<_>>()
            }}
        </section>
    }
}

/// Portfolio / positions view: totals and open-position summary.
#[component]
fn PortfolioView() -> impl IntoView {
    let portfolio = RwSignal::new(PortfolioFeature::new());

    view! {
        <section class="tantra-panel">
            <h1>{route_title(AppRoute::Portfolio)}</h1>
            <p>{move || if portfolio.get().has_data() { "positions open" } else { "no positions" }}</p>
            {move || {
                portfolio_lines(&portfolio.get())
                    .into_iter()
                    .map(|(k, v)| view! { <p><strong>{k}</strong>": " {v}</p> })
                    .map(|v| v.into_any())
                    .collect::<Vec<_>>()
            }}
        </section>
    }
}

/// Algo management: list counts and selection summary.
#[component]
fn AlgosView() -> impl IntoView {
    let algos = RwSignal::new(AlgosFeature::new());

    view! {
        <section class="tantra-panel">
            <h1>{route_title(AppRoute::Algos)}</h1>
            {move || {
                algos_lines(&algos.get())
                    .into_iter()
                    .map(|(k, v)| view! { <p><strong>{k}</strong>": " {v}</p> })
                    .map(|v| v.into_any())
                    .collect::<Vec<_>>()
            }}
        </section>
    }
}

/// Paper trading panel: order ticket + working orders / fills / depth /
/// positions over the in-browser `PaperSession` (embedded mode) or a
/// `/ws/live` server (plan 0008 D2). "Run demo tape" drives the real engine
/// deterministically.
#[component]
fn PaperView() -> impl IntoView {
    let feature = RwSignal::new(PaperFeature::new());

    view! {
        <section class="tantra-panel">
            <h1>{route_title(AppRoute::Paper)}</h1>
            <button on:click=move |_| { feature.update(|f| f.run_embedded_demo()); }>
                "Run demo tape"
            </button>
            <h2>"Order ticket"</h2>
            <ul>
                {move || {
                    paper_order_ticket(&feature.get())
                        .into_iter()
                        .map(|(k, v)| view! { <li><strong>{k}</strong>": " {v}</li> })
                        .map(|v| v.into_any())
                        .collect::<Vec<_>>()
                }}
            </ul>
            <h2>"Working orders"</h2>
            <ul>
                {move || {
                    paper_working_orders(&feature.get())
                        .into_iter()
                        .map(|text| view! { <li>{text}</li> })
                        .map(|v| v.into_any())
                        .collect::<Vec<_>>()
                }}
            </ul>
            <h2>"Fills"</h2>
            <ul>
                {move || {
                    paper_fills(&feature.get())
                        .into_iter()
                        .map(|text| view! { <li>{text}</li> })
                        .map(|v| v.into_any())
                        .collect::<Vec<_>>()
                }}
            </ul>
            <h2>"Depth"</h2>
            <ul>
                {move || {
                    paper_depth(&feature.get())
                        .into_iter()
                        .map(|text| view! { <li>{text}</li> })
                        .map(|v| v.into_any())
                        .collect::<Vec<_>>()
                }}
            </ul>
            <h2>"Positions / equity"</h2>
            <ul>
                {move || {
                    paper_positions_equity(&feature.get())
                        .into_iter()
                        .map(|text| view! { <li>{text}</li> })
                        .map(|v| v.into_any())
                        .collect::<Vec<_>>()
                }}
            </ul>
        </section>
    }
}

// ---------------------------------------------------------------------------
// Pure `view_model` helpers — host-testable without leptos rendering.
// ---------------------------------------------------------------------------

/// Human-readable title for a route: `"label (path)"`.
pub fn route_title(route: AppRoute) -> String {
    format!("{} ({})", route.label(), route.path())
}

/// Symbolic label for a screener comparison operator.
pub fn filter_op_label(op: FilterOp) -> &'static str {
    match op {
        FilterOp::Eq => "=",
        FilterOp::Ne => "!=",
        FilterOp::Gt => ">",
        FilterOp::Gte => ">=",
        FilterOp::Lt => "<",
        FilterOp::Lte => "<=",
        FilterOp::Between => "between",
    }
}

/// Render one drafted filter predicate as text, e.g. `"volume > 1000"`.
pub fn screener_predicate_text(p: &ScreenerFilterPredicate) -> String {
    let metric = &p.metric.0.metric;
    if p.op == FilterOp::Between {
        format!(
            "{} between {} and {}",
            metric,
            p.value,
            p.value_upper.unwrap_or_default()
        )
    } else {
        format!("{} {} {}", metric, filter_op_label(p.op), p.value)
    }
}

/// Label for a studio node kind, e.g. `"source NSE:RELIANCE-EQ"`.
pub fn studio_kind_label(kind: &StudioNodeKind) -> String {
    match kind {
        StudioNodeKind::Source(id) => format!("source {}", id),
        StudioNodeKind::Indicator(name) => format!("indicator {}", name),
        StudioNodeKind::Signal(name) => format!("signal {}", name),
        StudioNodeKind::Order(name) => format!("order {}", name),
        StudioNodeKind::Constant(value) => format!("constant {}", value),
    }
}

/// Watchlist rows as `(instrument, last/delta/spark summary)` lines.
pub fn watchlist_lines(w: &WatchlistFeature) -> Vec<(String, String)> {
    w.view
        .rows
        .iter()
        .map(|r| {
            (
                r.instrument.to_string(),
                format!(
                    "last {} delta {} ({}%) spark {}",
                    r.last_price.value(),
                    r.delta.absolute,
                    r.delta.pct,
                    r.spark.len()
                ),
            )
        })
        .collect()
}

/// Label for a subsystem state.
fn state_label(state: SubsystemState) -> &'static str {
    match state {
        SubsystemState::Healthy => "ok",
        SubsystemState::Degraded => "degraded",
        SubsystemState::Down => "down",
    }
}

/// System feature rows as `(subsystem, detail [state])` lines.
pub fn system_lines(s: &SystemFeature) -> Vec<(String, String)> {
    s.subsystems
        .iter()
        .map(|sub| {
            (
                sub.name.clone(),
                format!("{} [{}]", sub.detail, state_label(sub.state)),
            )
        })
        .collect()
}

/// Format a `Money` value as `"<amount> <currency>"`.
fn money_text(m: &Money) -> String {
    format!("{} {:?}", m.amount, m.currency)
}

/// Portfolio totals as `(label, money text)` lines.
pub fn portfolio_lines(f: &PortfolioFeature) -> Vec<(String, String)> {
    let totals = &f.view.totals;
    vec![
        ("cash".to_string(), money_text(&totals.cash)),
        ("margin_used".to_string(), money_text(&totals.margin_used)),
        (
            "margin_available".to_string(),
            money_text(&totals.margin_available),
        ),
        ("total_pnl".to_string(), money_text(&totals.total_pnl)),
        ("net_worth".to_string(), money_text(&totals.net_worth)),
    ]
}

/// Simulator feature state as `(label, value)` lines.
pub fn simulator_lines(s: &SimulatorFeature) -> Vec<(String, String)> {
    vec![
        ("phase".to_string(), format!("{:?}", s.phase)),
        (
            "equity_points".to_string(),
            s.equity_curve.len().to_string(),
        ),
        ("total_return".to_string(), s.total_return.to_string()),
        ("max_drawdown".to_string(), s.max_drawdown.to_string()),
        ("sharpe".to_string(), s.sharpe.to_string()),
    ]
}

/// Backtest results as `(label, value)` lines.
pub fn backtest_lines(b: &BacktestFeature) -> Vec<(String, String)> {
    let total_return_pct = b
        .view
        .summary
        .as_ref()
        .map(BacktestSummary::total_return_pct)
        .map(|d| d.to_string())
        .unwrap_or_else(|| "n/a".to_string());
    vec![
        ("status".to_string(), format!("{:?}", b.view.status)),
        (
            "equity_points".to_string(),
            b.view.equity_curve.len().to_string(),
        ),
        ("total_return_pct".to_string(), total_return_pct),
    ]
}

/// Algo manager state as `(label, value)` lines.
pub fn algos_lines(a: &AlgosFeature) -> Vec<(String, String)> {
    vec![
        ("total".to_string(), a.view.algos.len().to_string()),
        ("live".to_string(), a.live_count().to_string()),
        (
            "selected".to_string(),
            a.selected
                .map(|i| i.to_string())
                .unwrap_or_else(|| "none".to_string()),
        ),
    ]
}
