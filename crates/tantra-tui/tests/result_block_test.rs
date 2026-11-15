//! Integration tests for the `ResultBlockWidget` ratatui widget (plan 0003).

use ratatui::Terminal;
use ratatui::backend::TestBackend;
use tantra_agent_core::result::DisplayBlock;
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
