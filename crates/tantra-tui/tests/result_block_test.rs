//! Integration tests for the `ResultBlockWidget` ratatui widget (plan 0003).

use ratatui::Terminal;
use ratatui::backend::TestBackend;
use tantra_agent_core::result::{CodeDiffHunk, DisplayBlock};
use tantra_tui::result_block::ResultBlockWidget;

#[test]
fn result_block_widget_renders_table() {
    let block = DisplayBlock::Table {
        title: "Result Table".into(),
        columns: vec!["Symbol".into(), "Alpha".into()],
        rows: vec![
            vec!["RELIANCE".into(), "+1.25".into()],
            vec!["TCS".into(), "-0.50".into()],
        ],
    };
    let backend = TestBackend::new(40, 10);
    let mut terminal = Terminal::new(backend).expect("terminal");
    terminal
        .draw(|frame| {
            frame.render_widget(ResultBlockWidget { block: &block }, frame.area());
        })
        .expect("draw");
    let buffer = terminal.backend().buffer().clone();
    let text: String = buffer.content().iter().map(|cell| cell.symbol()).collect();
    assert!(text.contains("Symbol"), "missing header in: {text}");
    assert!(text.contains("RELIANCE"), "missing data cell in: {text}");
}

#[test]
fn result_block_widget_renders_alert() {
    let block = DisplayBlock::Alert {
        level: "WARN".into(),
        message: "Price band exceeded".into(),
    };
    let backend = TestBackend::new(40, 10);
    let mut terminal = Terminal::new(backend).expect("terminal");
    terminal
        .draw(|frame| {
            frame.render_widget(ResultBlockWidget { block: &block }, frame.area());
        })
        .expect("draw");
    let buffer = terminal.backend().buffer().clone();
    let text: String = buffer.content().iter().map(|cell| cell.symbol()).collect();
    assert!(text.contains("[WARN]"), "missing badge in: {text}");
    assert!(
        text.contains("Price band exceeded"),
        "missing message in: {text}"
    );
}

#[test]
fn result_block_widget_summarises_overlay_scene() {
    let block = DisplayBlock::OverlayScene {
        title: "EE+ on NIFTY".into(),
        symbol: "NSE:NIFTY50-INDEX".into(),
        timeframe: "1d".into(),
        scene: serde_json::json!({
            "signals": [{}, {}], "bands": [{}], "stops": [], "indicators": [], "hints": [{}], "hedges": []
        }),
        time_axis: vec![],
    };
    let backend = TestBackend::new(120, 6);
    let mut terminal = Terminal::new(backend).expect("terminal");
    terminal
        .draw(|frame| {
            frame.render_widget(ResultBlockWidget { block: &block }, frame.area());
        })
        .expect("draw");
    let text: String = terminal
        .backend()
        .buffer()
        .content()
        .iter()
        .map(|c| c.symbol())
        .collect();
    assert!(
        text.contains("2 signals, 1 bands"),
        "missing counts in: {text}"
    );
    assert!(text.contains("EE+ on NIFTY"), "missing title in: {text}");
}

#[test]
fn result_block_widget_renders_code_diff() {
    let block = DisplayBlock::CodeDiff {
        title: "EMA Crossover v2".into(),
        lang: "python".into(),
        code: String::new(),
        hunks: vec![CodeDiffHunk {
            id: "hunk_1".into(),
            header: "@@ -1,2 +1,2 @@".into(),
            lines: vec!["-fast = 12".into(), "+fast = 9".into()],
        }],
    };
    let backend = TestBackend::new(80, 10);
    let mut terminal = Terminal::new(backend).expect("terminal");
    terminal
        .draw(|frame| {
            frame.render_widget(ResultBlockWidget { block: &block }, frame.area());
        })
        .expect("draw");
    let text: String = terminal
        .backend()
        .buffer()
        .content()
        .iter()
        .map(|c| c.symbol())
        .collect();
    assert!(
        text.contains("EMA Crossover v2"),
        "missing title in: {text}"
    );
    assert!(text.contains("[python]"), "missing lang badge in: {text}");
    assert!(text.contains("+fast = 9"), "missing hunk line in: {text}");
}
