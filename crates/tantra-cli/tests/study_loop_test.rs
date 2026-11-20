//! CLI & live-REST integration tests for `tantra study-loop` and upgraded
//! `tantra elephant` (`Track T21.4`) across Index, F&O Equity, and Commodity.

use chrono::{Datelike, NaiveDate};
use clap::Parser;
use tantra_cli::args::Cli;
use tantra_cli::runner::{CommandOutcome, run};
use tantra_cli::{CliArgs, Command, StudyLoopArgs};
use tantra_core::{Exchange, InstrumentId};
use tantra_runtime_api_rest::market_feed::wire::{
    BarAggregation, BarSpecification, PriceType, UnixNanos,
};
use tantra_runtime_api_rest::market_feed::{ColumnarSliceBuilder, Dataset, DatasetReader};
use tantra_runtime_api_rest::{AppState, api_router_with};
use tantra_ui_charts::scene::OverlayScene;
use tantra_ui_core::{LIVE_RUNTIME_API_SOURCE, OFFLINE_SEEDED_FIXTURE_SOURCE};

fn parse_args(argv: &[&str]) -> CliArgs {
    Cli::try_parse_from(argv).expect("parse cli").into()
}

#[test]
fn bare_elephant_preserves_legacy_report_and_flagged_elephant_runs_loop() {
    let bare = parse_args(&["tantra", "elephant"]);
    assert_eq!(bare.command, Command::Elephant(StudyLoopArgs::default()));
    match run(&bare).unwrap() {
        CommandOutcome::ElephantReport(txt) => {
            assert!(txt.contains("S1 (support) 23930..23970"));
        }
        other => panic!("expected ElephantReport, got {other:?}"),
    }

    let flagged = parse_args(&[
        "tantra",
        "elephant",
        "--strategy",
        "orb_15",
        "--symbol",
        "NSE:RELIANCE-EQ",
        "--tf",
        "5m",
        "--sweep",
        "--hedge",
    ]);
    match run(&flagged).unwrap() {
        CommandOutcome::StudyLoopReport(txt) => {
            assert!(txt.contains("orb_15 on NSE:RELIANCE-EQ"));
            assert!(txt.contains(OFFLINE_SEEDED_FIXTURE_SOURCE));
            assert!(txt.contains("Parameter Sweep Leaderboard"));
            assert!(txt.contains("Hedge Recommendation"));
        }
        other => panic!("expected StudyLoopReport, got {other:?}"),
    }
}

#[test]
fn study_loop_multi_asset_json_and_export_scene_are_deterministic() {
    let dir = tempfile::tempdir().unwrap();
    let scene_path = dir.path().join("nifty_scene.json");
    let scene_str = scene_path.to_string_lossy().to_string();

    // 1. Index (`NSE:NIFTY50-INDEX`) with `--export-scene` & `--json`
    let cli = parse_args(&[
        "tantra",
        "study-loop",
        "--strategy",
        "elephant_edge_v3",
        "--symbol",
        "NSE:NIFTY50-INDEX",
        "--tf",
        "1d",
        "--horizon",
        "5",
        "--sl-buffer",
        "0.25",
        "--sweep",
        "--hedge",
        "--json",
        "--export-scene",
        &scene_str,
    ]);
    let json_text = match run(&cli).unwrap() {
        CommandOutcome::StudyLoopReport(s) => s,
        other => panic!("expected StudyLoopReport, got {other:?}"),
    };
    let parsed_json: serde_json::Value = serde_json::from_str(&json_text).unwrap();
    assert_eq!(parsed_json["data_source"], OFFLINE_SEEDED_FIXTURE_SOURCE);
    assert_eq!(parsed_json["config"]["symbol"], "NSE:NIFTY50-INDEX");
    assert_eq!(parsed_json["stop_comparison"].as_array().unwrap().len(), 4);
    assert!(!parsed_json["sweep_rows"].as_array().unwrap().is_empty());

    // Exported scene JSON must deserialize back into `OverlayScene` cleanly
    let raw_scene = std::fs::read_to_string(&scene_path).unwrap();
    let scene: OverlayScene = serde_json::from_str(&raw_scene).unwrap();
    let counts = scene.layer_counts();
    assert!(counts.signals > 0);
    assert!(counts.bands > 0);
    assert!(counts.stops > 0);
    assert!(counts.indicators >= 3);
    assert!(counts.hints > 0);
    assert!(counts.hedges > 0);

    // 2. Commodity (`MCX:GOLD`) with `commodity_cycle` expiry hedge
    let gold = parse_args(&[
        "tantra",
        "study-loop",
        "--strategy",
        "elephant_edge_v3",
        "--symbol",
        "MCX:GOLD",
        "--expiry-horizon",
        "commodity_cycle",
        "--hedge",
    ]);
    let gold_txt = match run(&gold).unwrap() {
        CommandOutcome::StudyLoopReport(s) => s,
        other => panic!("expected StudyLoopReport, got {other:?}"),
    };
    assert!(gold_txt.contains("MCX:GOLD"));
    assert!(gold_txt.contains("CalendarRoll"));
    assert!(gold_txt.contains("Penetration Beyond Outer Edge"));
}

fn build_synthetic_daily_slice(
    exchange: Exchange,
    symbol: &str,
    days: usize,
) -> tantra_runtime_api_rest::market_feed::ColumnarSlice {
    let spec = BarSpecification::new(1, BarAggregation::Day, PriceType::Last);
    let mut builder = ColumnarSliceBuilder::new(InstrumentId::new(exchange, symbol), spec);
    let mut cur = NaiveDate::from_ymd_opt(2025, 6, 16).unwrap();
    let mut count = 0usize;
    let mut price = 1400.0_f64;
    while count < days {
        let wd = cur.weekday();
        if wd != chrono::Weekday::Sat && wd != chrono::Weekday::Sun {
            let (open_off, high_off, low_off, close_off) = match count % 10 {
                0 => (0.0, 22.0, -10.0, 14.0),
                1 => (14.0, 34.0, -6.0, 24.0),
                2 => (24.0, 32.0, -28.0, -18.0),
                3 => (-18.0, 8.0, -36.0, -26.0),
                4 => (-26.0, 15.0, -42.0, 10.0),
                5 => (10.0, 44.0, 2.0, 36.0),
                6 => (36.0, 52.0, 18.0, 46.0),
                7 => (46.0, 58.0, 12.0, 18.0),
                8 => (18.0, 28.0, -14.0, -10.0),
                _ => (-10.0, 12.0, -18.0, 6.0),
            };
            let base = price;
            let open = (base + open_off).max(100.0);
            let close = (base + close_off).max(100.0);
            let high = open.max(close).max(base + high_off) + 8.0;
            let low = open.min(close).min(base + low_off).max(50.0) - 8.0;
            let dt = cur.and_hms_opt(10, 0, 0).unwrap().and_utc();
            builder
                .push(
                    UnixNanos::from_datetime(dt),
                    open,
                    high,
                    low,
                    close,
                    250_000.0,
                )
                .unwrap();
            price = close;
            count += 1;
        }
        cur = cur.succ_opt().unwrap();
    }
    builder.finish().unwrap()
}

fn boot_runtime_server() -> std::net::SocketAddr {
    let dataset = Dataset::from_slices(vec![build_synthetic_daily_slice(
        Exchange::Nse,
        "RELIANCE",
        85,
    )])
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

#[test]
fn study_loop_live_server_hits_v2_endpoints_and_exports_scene() {
    let addr = boot_runtime_server();
    let server_url = format!("http://{addr}");
    let dir = tempfile::tempdir().unwrap();
    let scene_path = dir.path().join("live_reliance_scene.json");
    let scene_str = scene_path.to_string_lossy().to_string();

    let cli = parse_args(&[
        "tantra",
        "study-loop",
        "--server",
        &server_url,
        "--strategy",
        "elephant_edge_v3",
        "--symbol",
        "RELIANCE.NSE",
        "--tf",
        "1d",
        "--start-date",
        "2025-06-16",
        "--end-date",
        "2025-12-31",
        "--sweep",
        "--hedge",
        "--export-scene",
        &scene_str,
    ]);

    let report = match run(&cli).unwrap() {
        CommandOutcome::StudyLoopReport(s) => s,
        other => panic!("expected StudyLoopReport, got {other:?}"),
    };

    assert!(
        report.contains(LIVE_RUNTIME_API_SOURCE),
        "report:\n{report}"
    );
    assert!(report.contains("NSE:RELIANCE"), "report:\n{report}");
    assert!(
        report.contains("Stop-Hunt Buffer Benchmark"),
        "report:\n{report}"
    );
    assert!(
        report.contains("Parameter Sweep Leaderboard"),
        "report:\n{report}"
    );
    assert!(report.contains("Hedge Recommendation"), "report:\n{report}");

    let raw_scene = std::fs::read_to_string(&scene_path).unwrap();
    let scene: OverlayScene = serde_json::from_str(&raw_scene).unwrap();
    assert_eq!(scene.strategy_id, "elephant_edge_v3");
    assert!(scene.layer_counts().indicators >= 3);
}
