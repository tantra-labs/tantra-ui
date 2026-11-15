//! Live `GET /screener/scan` path for `tantra scan --server` (T10.4, gap ui 0006).
//!
//! Boots a real `tantra-runtime-api-rest` router over an in-memory daily-bar
//! dataset on an ephemeral TCP port and drives the sync `live_scan` entry point
//! (`reqwest::blocking`) against it. The dataset/router construction mirrors
//! `tantra-runtime/crates/tantra-runtime-api-rest/tests/screener.rs`.
//!
//! The bar fixture is `f64` because `ColumnarSliceBuilder::push` takes f64 for
//! the wire format (same as the reference runtime test); the CLI's canonical
//! screener values (`ScreenerFilterPredicate::value`) remain `Decimal`, so no
//! float reaches the financial path in tantra-cli.

use tantra_cli::args::ScanArgs;
use tantra_cli::runner::CliError;
use tantra_cli::scan::{live_scan, parse_query, run_scan_args, to_server_filters};
use tantra_core::screener::ScreenerScanRequest;
use tantra_core::{Exchange, InstrumentId};
use tantra_runtime_api_rest::market_feed::wire::{
    BarAggregation, BarSpecification, PriceType, UnixNanos,
};
use tantra_runtime_api_rest::market_feed::{ColumnarSliceBuilder, Dataset, DatasetReader};
use tantra_runtime_api_rest::{AppState, api_router_with};

const DAY: u64 = 86_400_000_000_000;
/// 2024-01-01T00:00:00Z.
const T0: u64 = 1_704_067_200_000_000_000;

/// One 20-bar daily slice per symbol (deterministic, no wall clock).
fn daily(symbol: &str, closes: &[f64]) -> tantra_runtime_api_rest::market_feed::ColumnarSlice {
    let spec = BarSpecification::new(1, BarAggregation::Day, PriceType::Last);
    let mut builder = ColumnarSliceBuilder::new(InstrumentId::new(Exchange::Nse, symbol), spec);
    for (i, &c) in closes.iter().enumerate() {
        builder
            .push(
                UnixNanos::from_u64(T0 + i as u64 * DAY),
                c,
                c + 1.0,
                c - 1.0,
                c,
                1000.0,
            )
            .unwrap();
    }
    builder.finish().unwrap()
}

fn rising() -> Vec<f64> {
    (0..20).map(|i| 100.0 + f64::from(i)).collect()
}

fn falling() -> Vec<f64> {
    (0..20).map(|i| 200.0 - f64::from(i)).collect()
}

/// Boot the screener router on an ephemeral TCP port on a dedicated thread and
/// return its address. The server owns its own `current_thread` tokio runtime so
/// the test's runtime is never starved by `live_scan`'s blocking HTTP call. The
/// std listener is converted to a tokio listener *inside* the server thread so
/// it registers with the server runtime's reactor (converting on the test
/// runtime would register it with the wrong reactor and `accept` would never
/// wake).
fn boot_app() -> std::net::SocketAddr {
    let dataset = Dataset::from_slices(vec![
        daily("TCS", &rising()),
        daily("INFY", &falling()),
        daily("WIPRO", &rising()),
    ])
    .unwrap();
    let router = api_router_with(AppState::from_reader(DatasetReader::from_dataset(dataset)));
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = listener.local_addr().unwrap();
    listener.set_nonblocking(true).unwrap();
    std::thread::spawn(move || {
        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();
        let _enter = rt.enter();
        let listener = tokio::net::TcpListener::from_std(listener).unwrap();
        drop(_enter);
        rt.block_on(axum::serve(listener, router).into_future())
            .unwrap();
    });
    addr
}

#[tokio::test]
async fn live_scan_close_gt_filter_matches_only_falling_instrument() {
    let addr = boot_app();
    // TCS ends at 119, INFY ends at 181 -> only INFY satisfies close > 150.
    let request: ScreenerScanRequest = parse_query("close>150").unwrap();
    let rows = live_scan(
        &format!("http://{addr}"),
        &request,
        &["TCS.NSE".to_string(), "INFY.NSE".to_string()],
        None,
        None,
    )
    .unwrap();
    assert_eq!(rows, 1, "expected only INFY (close 181) above 150");
}

#[tokio::test]
async fn live_scan_unknown_instrument_returns_error() {
    let addr = boot_app();
    let request: ScreenerScanRequest = parse_query("close>1").unwrap();
    let err = live_scan(
        &format!("http://{addr}"),
        &request,
        &["TCS.NSE".to_string(), "NOPE.NSE".to_string()],
        None,
        None,
    )
    .unwrap_err();
    match err {
        CliError::Execution(msg) => {
            assert!(msg.contains("404"), "expected a 404 reason, got: {msg}");
        }
        other => panic!("expected Execution error, got {other:?}"),
    }
}

#[tokio::test]
async fn run_scan_args_live_path_reaches_remote_server() {
    let addr = boot_app();
    let args = ScanArgs {
        query: "close>150".into(),
        server: Some(format!("http://{addr}")),
        universe: Some(r#"["TCS.NSE","INFY.NSE"]"#.into()),
        tf: None,
        as_of: None,
    };
    match run_scan_args(&args).unwrap() {
        tantra_cli::runner::CommandOutcome::ScanResults { rows } => {
            assert_eq!(rows, 1, "live path must count the remote match");
        }
        other => panic!("expected ScanResults, got {other:?}"),
    }
}

#[test]
fn to_server_filters_maps_canonical_group_to_wire_shape() {
    // `between` maps to the server grammar `value: [low, high]` with numeric
    // operands (no `value_upper` field, no strings).
    let request: ScreenerScanRequest = parse_query("pe between 10 30").unwrap();
    let v = to_server_filters(&request);
    assert_eq!(v["operator"], serde_json::json!("AND"));
    assert_eq!(v["items"][0]["key"], serde_json::json!("pe"));
    assert_eq!(v["items"][0]["op"], serde_json::json!("between"));
    assert_eq!(v["items"][0]["value"], serde_json::json!([10, 30]));
    assert!(v["items"][0].get("value_upper").is_none());

    // Single comparisons send a plain JSON number operand.
    let request: ScreenerScanRequest = parse_query("close>150").unwrap();
    let v = to_server_filters(&request);
    assert_eq!(v["items"][0]["op"], serde_json::json!("gt"));
    assert_eq!(v["items"][0]["value"], serde_json::json!(150));
}
