//! Trading-R1 style shell look tests.

use ratatui::Terminal;
use ratatui::backend::TestBackend;
use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyEventKind, KeyEventState, KeyModifiers};
use ratatui::style::Color;
use tantra_tui::{ActiveTab, AgentMessage, AgentRole, TuiApp, draw};

fn key(code: KeyCode, modifiers: KeyModifiers) -> KeyEvent {
    KeyEvent {
        code,
        modifiers,
        kind: KeyEventKind::Press,
        state: KeyEventState::NONE,
    }
}

fn render(app: &TuiApp, w: u16, h: u16) -> (Vec<String>, ratatui::buffer::Buffer) {
    let mut terminal = Terminal::new(TestBackend::new(w, h)).unwrap();
    terminal.draw(|f| draw(f, app)).unwrap();
    let buf = terminal.backend().buffer().clone();
    let rows = (0..h)
        .map(|y| (0..w).map(|x| buf[(x, y)].symbol()).collect::<String>())
        .collect();
    (rows, buf)
}

fn text(rows: &[String]) -> String {
    rows.join("\n")
}

#[test]
fn welcome_view_renders_logo_wordmark_and_input() {
    for (w, h) in [(120, 36), (80, 24)] {
        let (rows, _) = render(&TuiApp::new(), w, h);
        let t = text(&rows);
        assert!(t.contains("model:"), "{w}x{h}");
        assert!(t.contains("(tab to cycle)"));
        assert!(t.contains("|_   _|_ _ _ __"));
        assert!(t.contains('●'));
        assert!(t.contains("ask a finance question"));
    }
}

#[test]
fn footer_shows_hints_and_version() {
    let (rows, _) = render(&TuiApp::new(), 120, 36);
    let t = text(&rows);
    assert!(t.contains("^N new"));
    assert!(t.contains("^C quit"));
    assert!(t.contains(&format!("v{}", env!("CARGO_PKG_VERSION"))));
}

#[test]
fn research_view_shows_transcript_and_rail() {
    let mut app = TuiApp::new();
    app.chat
        .push(AgentMessage::new(AgentRole::User, "Hello agent"));
    app.chat
        .push(AgentMessage::new(AgentRole::Agent, "Hi there"));
    let (rows, _) = render(&app, 120, 36);
    let t = text(&rows);
    assert!(t.contains("› Hello agent"));
    assert!(rows.iter().any(|r| r.trim_start().starts_with("╭─Tantra")));
    assert!(t.contains("Market (0)"));
    assert!(t.contains("no market data yet"));
    assert!(t.contains("ask a follow-up"));
}

#[test]
fn palette_shows_commands_modal_and_groups() {
    let mut app = TuiApp::new();
    app.is_palette_open = true;
    let (rows, _) = render(&app, 120, 36);
    let t = text(&rows);
    assert!(t.contains("╭─Commands"));
    assert!(t.contains("Session"));
}

#[test]
fn ctrl_c_quits_and_ctrl_n_resets() {
    let mut app = TuiApp::new();
    app.submit_prompt("hi".to_string());
    assert_eq!(app.chat.len(), 2);
    app.on_key(key(KeyCode::Char('n'), KeyModifiers::CONTROL));
    assert!(app.chat.is_empty());
    assert!(app.pending_prompt.is_none());
    assert!(!app.should_quit);
    app.on_key(key(KeyCode::Char('c'), KeyModifiers::CONTROL));
    assert!(app.should_quit);
}

#[test]
fn tab_in_input_mode_cycles_view_and_drops_input() {
    let mut app = TuiApp::new();
    app.on_key(key(KeyCode::Tab, KeyModifiers::NONE));
    assert_eq!(app.active_tab, ActiveTab::StrategyLab);
    assert!(!app.is_input_mode);
}

#[test]
fn welcome_logo_cells_use_primary_colour() {
    let (rows, buf) = render(&TuiApp::new(), 120, 36);
    let mut found = false;
    for (y, row) in rows.iter().enumerate() {
        for (x, _) in row.chars().enumerate() {
            let c = &buf[(x as u16, y as u16)];
            if c.symbol() == "●" && c.fg == Color::Rgb(0x14, 0xC2, 0x90) {
                found = true;
            }
        }
    }
    assert!(found);
}
