//! Price card feed: path ids, base URL, body parsing, seeding, rendering, HTTP.

use std::str::FromStr;
use std::sync::Arc;

use ratatui::Terminal;
use ratatui::backend::TestBackend;
use rust_decimal::Decimal;
use tantra_agent_llm::StaticCatalog;
use tantra_tui::agent_link::AgentConnection;
use tantra_tui::live_data::LiveData;
use tantra_tui::market_feed::{
    PriceFeed, PriceFeedError, PriceStatus, bars_path_id, parse_bar_closes, runtime_base,
};
use tantra_tui::{TuiApp, draw};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;

fn d(s: &str) -> Decimal {
    Decimal::from_str(s).unwrap()
}

#[test]
fn path_ids() {
    assert_eq!(bars_path_id("NSE:NIFTY50-INDEX"), "NIFTY50-INDEX.NSE");
    assert_eq!(bars_path_id("NSE:RELIANCE-EQ"), "RELIANCE.NSE");
    assert_eq!(bars_path_id("TCS"), "TCS.NSE");
    assert_eq!(bars_path_id("RELIANCE.NSE"), "RELIANCE.NSE");
    assert_eq!(bars_path_id("BSE:SENSEX-INDEX"), "SENSEX-INDEX.BSE");
}

#[test]
fn bases() {
    let r = |u: &str| AgentConnection::Remote {
        url: u.into(),
        token: None,
    };
    assert_eq!(
        runtime_base(&r("ws://127.0.0.1:8000/api/v1/acp")),
        "http://127.0.0.1:8000"
    );
    assert_eq!(
        runtime_base(&r("wss://h.example/api/v1/acp")),
        "https://h.example"
    );
    let conn = AgentConnection::Embedded {
        catalog: Arc::new(StaticCatalog::new()),
        model: None,
    };
    assert_eq!(runtime_base(&conn), "http://127.0.0.1:8000");
}

#[test]
fn parses_bars() {
    let v: serde_json::Value =
        serde_json::from_str(r#"{"data":{"bars":[{"close":"101.5"},{"close":"102"}]}}"#).unwrap();
    assert_eq!(parse_bar_closes(&v).unwrap(), vec![d("101.5"), d("102")]);
    let v: serde_json::Value = serde_json::from_str(r#"{"bars":[{"close":103.25}]}"#).unwrap();
    assert_eq!(parse_bar_closes(&v).unwrap(), vec![d("103.25")]);
    let v: serde_json::Value = serde_json::from_str(r#"{"data":{"bars":[]}}"#).unwrap();
    assert_eq!(parse_bar_closes(&v), Err(PriceFeedError::NoBars));
    let v: serde_json::Value = serde_json::from_str(r#"{"x":1}"#).unwrap();
    assert!(matches!(
        parse_bar_closes(&v),
        Err(PriceFeedError::BadBody(_))
    ));
}

#[test]
fn seed_replaces_history() {
    let mut live = LiveData::new();
    live.seed_history("S", [d("1"), d("2")]);
    live.seed_history("S", [d("5"), d("6"), d("7")]);
    let h: Vec<Decimal> = live.history("S").unwrap().iter().copied().collect();
    assert_eq!(h, vec![d("5"), d("6"), d("7")]);
}

fn render(app: &TuiApp) -> String {
    let mut t = Terminal::new(TestBackend::new(120, 40)).unwrap();
    t.draw(|f| draw(f, app)).unwrap();
    let buf = t.backend().buffer().clone();
    buf.content().iter().map(|c| c.symbol()).collect::<String>()
}

#[test]
fn unreachable_state_renders() {
    let mut app = TuiApp::new();
    app.submit_prompt("analyze NSE:NIFTY50".to_string());
    let sym = app.analysis.primary().unwrap().to_string();
    app.price_status = PriceStatus::Failed {
        symbol: sym,
        error: PriceFeedError::Unreachable("refused".into()).to_string(),
    };
    let text = render(&app);
    assert!(text.contains("runtime not reachable"), "{text}");
    assert!(!text.contains("tantra serve <data_dir>"));
}

#[tokio::test]
async fn fetches_over_http() {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    let server = tokio::spawn(async move {
        let (mut s, _) = listener.accept().await.unwrap();
        let mut buf = vec![0u8; 4096];
        let n = s.read(&mut buf).await.unwrap();
        let req = String::from_utf8_lossy(&buf[..n]).to_string();
        let body = r#"{"data":{"bars":[{"close":"100.1"},{"close":"100.2"},{"close":"100.3"}]}}"#;
        let resp = format!(
            "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
            body.len()
        );
        s.write_all(resp.as_bytes()).await.unwrap();
        req
    });
    let feed = PriceFeed::new(format!("http://127.0.0.1:{port}"), None);
    let from = chrono::NaiveDate::from_ymd_opt(2026, 7, 1).unwrap();
    let closes = feed.daily_closes("NSE:NIFTY50-INDEX", from).await.unwrap();
    assert_eq!(closes, vec![d("100.1"), d("100.2"), d("100.3")]);
    let req = server.await.unwrap();
    let line = req.lines().next().unwrap();
    assert!(
        line.contains("/api/v2/bars/NIFTY50-INDEX.NSE?tf=1d&from=2026-07-01"),
        "{line}"
    );
}

#[tokio::test]
async fn closed_port_is_unreachable() {
    let port = {
        let l = TcpListener::bind("127.0.0.1:0").await.unwrap();
        l.local_addr().unwrap().port()
    };
    let feed = PriceFeed::new(format!("http://127.0.0.1:{port}"), None);
    let from = chrono::NaiveDate::from_ymd_opt(2026, 7, 1).unwrap();
    let err = feed.daily_closes("TCS", from).await.unwrap_err();
    assert!(matches!(err, PriceFeedError::Unreachable(_)), "{err:?}");
}
