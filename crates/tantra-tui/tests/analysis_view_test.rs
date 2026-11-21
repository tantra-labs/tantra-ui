//! Split-screen analysis page rendering tests.

use ratatui::Terminal;
use ratatui::backend::TestBackend;
use rust_decimal::Decimal;
use tantra_agent_core::{DisplayBlock, MetricItem};
use tantra_tui::live_data::{PaperDepthRow, PaperPositionRow, WatchlistRow};
use tantra_tui::{AgentStreamUpdate, TuiApp, draw};

const SYM: &str = "NSE:NIFTY50-INDEX";

fn rows(app: &TuiApp, w: u16, h: u16) -> Vec<String> {
    let mut terminal = Terminal::new(TestBackend::new(w, h)).unwrap();
    terminal.draw(|f| draw(f, app)).unwrap();
    let buf = terminal.backend().buffer().clone();
    (0..h)
        .map(|y| (0..w).map(|x| buf[(x, y)].symbol()).collect::<String>())
        .collect()
}

fn text(app: &TuiApp, w: u16, h: u16) -> String {
    rows(app, w, h).join("\n")
}

fn asked() -> TuiApp {
    let mut app = TuiApp::new();
    app.submit_prompt("What is NIFTY doing?".into());
    app
}

fn dec(n: i64) -> Decimal {
    Decimal::from(n)
}

#[test]
fn split_layout_after_first_prompt() {
    let app = asked();
    let r = rows(&app, 140, 40);
    let t = r.join("\n");
    assert!(t.contains("╭─Tantra"));
    assert!(t.contains("╭─Analysis · NIFTY50"));
    let line = r.iter().find(|l| l.contains("╭─Analysis")).unwrap();
    let col = line.chars().position(|c| c == '╭').unwrap_or(0);
    let col = line
        .chars()
        .take_while(|_| true)
        .enumerate()
        .find_map(|(i, _)| {
            line.chars()
                .skip(i)
                .collect::<String>()
                .starts_with("╭─Analysis")
                .then_some(i)
        })
        .unwrap_or(col);
    assert!(col >= 56, "analysis x={col}");
    for l in &r {
        if let Some(i) = l.find("ask a follow-up") {
            assert!(l[..i].chars().count() < 63);
        }
    }
    assert!(t.contains("ask a follow-up"));
    assert!(t.contains("no live data"));
}

#[test]
fn live_data_fills_strip_chart_spread_position() {
    let mut app = asked();
    app.live
        .price_history
        .insert(SYM.into(), (0..30).map(|i| dec(100 + i % 7)).collect());
    app.live.watchlist.push(WatchlistRow {
        symbol: SYM.into(),
        last: dec(105),
        change: dec(1),
        change_pct: Decimal::new(95, 2),
        volume: dec(10),
    });
    app.live.paper_depth.push(PaperDepthRow {
        instrument: SYM.into(),
        bid: Some(dec(100)),
        ask: Some(dec(101)),
    });
    app.live.paper_positions.push(PaperPositionRow {
        instrument: SYM.into(),
        quantity: dec(5),
    });
    let t = text(&app, 140, 40);
    assert!(t.contains("NIFTY50 105.00 +0.95%"));
    assert!(t.chars().any(|c| ('\u{2800}'..='\u{28FF}').contains(&c)));
    assert!(t.contains("spread 1"));
    assert!(t.contains("bps"));
    assert!(t.contains("qty 5"));
}

#[test]
fn tool_cards_and_rows() {
    let mut app = asked();
    app.apply_stream_update(AgentStreamUpdate::ToolStarted {
        call_id: "1".into(),
        name: "get_metrics".into(),
        summary: "sharpe".into(),
    });
    let t = text(&app, 140, 40);
    assert!(t.contains("get metrics"));
    assert!(theme_spinner_present(&t));
    app.apply_stream_update(AgentStreamUpdate::ToolFinished {
        call_id: "1".into(),
        name: "get_metrics".into(),
        ok: true,
        elapsed_ms: 250,
        blocks: vec![
            DisplayBlock::MetricGrid {
                items: vec![MetricItem {
                    label: "Sharpe".into(),
                    value: "1.44".into(),
                }],
            },
            DisplayBlock::Chart {
                title: "Equity".into(),
                chart_type: "line".into(),
                series_ref: "s1".into(),
            },
        ],
    });
    let t = text(&app, 140, 40);
    assert!(t.contains("✓"));
    assert!(t.contains("(0.2s)"));
    assert!(t.contains("metrics"));
    assert!(t.contains("Sharpe"), "{t}");
    assert!(t.contains("series not available"));
}

fn theme_spinner_present(t: &str) -> bool {
    t.chars().any(|c| ('\u{2800}'..='\u{28FF}').contains(&c))
}

#[test]
fn narrow_and_welcome_have_no_analysis() {
    let app = asked();
    assert!(!text(&app, 70, 40).contains("Analysis"));
    let welcome = text(&TuiApp::new(), 140, 40);
    assert!(!welcome.contains("Analysis"));
}

#[test]
fn tool_card_has_a_single_border_with_the_card_title() {
    let mut app = asked();
    app.apply_stream_update(AgentStreamUpdate::ToolFinished {
        call_id: "c1".into(),
        name: "backtest_metrics".into(),
        ok: true,
        elapsed_ms: 10,
        blocks: vec![DisplayBlock::MetricGrid {
            items: vec![MetricItem {
                label: "Sharpe".into(),
                value: "1.44".into(),
            }],
        }],
    });
    let r = rows(&app, 140, 40);
    let title_row = r
        .iter()
        .position(|l| l.contains("backtest metrics · metrics"))
        .expect("card title row");
    // the body sits directly under the card title: no nested frame row in between
    assert!(
        r[title_row + 1].contains("Sharpe: 1.44"),
        "{}",
        r.join("\n")
    );
    let card_x = r[title_row]
        .find("╭─backtest metrics")
        .expect("card corner");
    assert_eq!(
        r[title_row].matches('╭').count(),
        1,
        "exactly one card corner on the title row"
    );
    assert!(card_x > 0);
}
