//! View theme tests: data views use the Trading-R1 rounded look and the composer grows.

use ratatui::Terminal;
use ratatui::backend::TestBackend;
use ratatui::buffer::Buffer;
use ratatui::style::Color;
use tantra_tui::app::{ActiveTab, TuiApp};
use tantra_tui::draw;
use tantra_tui::ui::composer_height;

fn render(app: &TuiApp, w: u16, h: u16) -> Buffer {
    let mut term = Terminal::new(TestBackend::new(w, h)).expect("terminal");
    term.draw(|f| draw(f, app)).expect("draw");
    term.backend().buffer().clone()
}

fn text_of(buf: &Buffer) -> String {
    buf.content().iter().map(|c| c.symbol()).collect()
}

fn row_text(buf: &Buffer, y: u16) -> String {
    (0..buf.area.width)
        .map(|x| buf[(x, y)].symbol().to_string())
        .collect()
}

#[test]
fn data_views_use_rounded_panels_and_theme_colours() {
    for tab in [
        ActiveTab::Watchlist,
        ActiveTab::Screener,
        ActiveTab::Portfolio,
        ActiveTab::Orders,
        ActiveTab::Backtest,
        ActiveTab::Algos,
        ActiveTab::StrategyLab,
    ] {
        let mut app = TuiApp::new();
        app.active_tab = tab;
        app.is_input_mode = false;
        let buf = render(&app, 140, 44);
        let text = text_of(&buf);
        assert!(text.contains("╭─"), "{tab:?}: no rounded title");
        assert!(!text.contains('┌'), "{tab:?}: square corner");
        for cell in buf.content() {
            assert!(
                ![Color::Cyan, Color::Yellow, Color::Magenta, Color::DarkGray].contains(&cell.fg),
                "{tab:?}: named colour {:?}",
                cell.fg
            );
        }
    }
}

#[test]
fn composer_height_wraps_and_clamps() {
    assert_eq!(
        composer_height("ask a finance question, or type / for commands", 44),
        4
    );
    assert_eq!(composer_height("hi", 44), 3);
    let long = vec!["wordwordw"; 40].join(" ");
    assert_eq!(composer_height(&long, 44), 8);
}

#[test]
fn welcome_placeholder_wraps_onto_two_rows() {
    let app = TuiApp::new();
    let mut app = app;
    app.active_tab = ActiveTab::AgentChat;
    let buf = render(&app, 120, 36);
    let rows: Vec<String> = (0..36).map(|y| row_text(&buf, y)).collect();
    let y = rows
        .iter()
        .position(|r| r.contains("ask a finance question"))
        .expect("placeholder row");
    assert!(rows[y + 1].contains("commands"));
}
