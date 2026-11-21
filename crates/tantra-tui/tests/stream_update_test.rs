//! Tests for `TuiApp::apply_stream_update` and analysis wiring.

use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use tantra_agent_core::DisplayBlock;
use tantra_tui::{AgentRole, AgentStreamUpdate, ToolStatus, TuiApp};

fn alert() -> DisplayBlock {
    DisplayBlock::Alert {
        level: "info".into(),
        message: "hi".into(),
    }
}

fn started(app: &mut TuiApp) {
    app.apply_stream_update(AgentStreamUpdate::ToolStarted {
        call_id: "1".into(),
        name: "run_backtest".into(),
        summary: "s".into(),
    });
}

#[test]
fn submit_sets_focus() {
    let mut app = TuiApp::new();
    app.submit_prompt("how is RELIANCE".into());
    assert_eq!(app.analysis.primary(), Some("NSE:RELIANCE"));
    assert!(app.is_loading);
}

#[test]
fn token_appends_and_cleans() {
    let mut app = TuiApp::new();
    app.submit_prompt("hello".into());
    app.apply_stream_update(AgentStreamUpdate::Token("a<|im_end|>".into()));
    app.apply_stream_update(AgentStreamUpdate::Token("b".into()));
    assert_eq!(app.chat.last().unwrap().text, "ab");
}

#[test]
fn tool_started_replaces_placeholder() {
    let mut app = TuiApp::new();
    app.submit_prompt("hello".into());
    let n = app.chat.len();
    started(&mut app);
    assert_eq!(app.chat.len(), n);
    let last = app.chat.last().unwrap();
    assert_eq!(last.role, AgentRole::Tool);
    assert_eq!(last.tool.as_ref().unwrap().status, ToolStatus::Running);
}

#[test]
fn token_after_tool_starts_new_message() {
    let mut app = TuiApp::new();
    app.submit_prompt("hello".into());
    started(&mut app);
    app.apply_stream_update(AgentStreamUpdate::Token("done".into()));
    assert_eq!(app.chat.last().unwrap().role, AgentRole::Agent);
    assert_eq!(app.chat.last().unwrap().text, "done");
}

#[test]
fn tool_finished_marks_ok_and_pushes_blocks() {
    let mut app = TuiApp::new();
    app.submit_prompt("hello".into());
    started(&mut app);
    app.apply_stream_update(AgentStreamUpdate::ToolFinished {
        call_id: "1".into(),
        name: "run_backtest".into(),
        ok: true,
        elapsed_ms: 42,
        blocks: vec![alert()],
    });
    let row = app
        .chat
        .messages
        .iter()
        .find_map(|m| m.tool.as_ref())
        .unwrap();
    assert_eq!(row.status, ToolStatus::Ok);
    assert_eq!(row.elapsed_ms, Some(42));
    assert_eq!(app.analysis.cards.len(), 1);
    assert_eq!(app.analysis.cards[0].tool, "run_backtest");
}

#[test]
fn finished_clears_loading_and_drops_trailing_empty() {
    let mut app = TuiApp::new();
    app.submit_prompt("hello".into());
    started(&mut app);
    app.apply_stream_update(AgentStreamUpdate::Token("  ".into()));
    app.apply_stream_update(AgentStreamUpdate::Finished);
    assert!(!app.is_loading);
    assert_eq!(app.chat.last().unwrap().role, AgentRole::Tool);
}

#[test]
fn finished_without_tool_keeps_placeholder() {
    let mut app = TuiApp::new();
    app.submit_prompt("hello".into());
    let n = app.chat.len();
    app.apply_stream_update(AgentStreamUpdate::Finished);
    assert_eq!(app.chat.len(), n);
    assert!(!app.is_loading);
}

#[test]
fn error_path() {
    let mut app = TuiApp::new();
    app.submit_prompt("hello".into());
    app.apply_stream_update(AgentStreamUpdate::Error("boom".into()));
    assert_eq!(app.chat.last().unwrap().text, "*Error: boom*");
    assert!(!app.is_loading);
}

#[test]
fn ctrl_n_clears_analysis() {
    let mut app = TuiApp::new();
    app.submit_prompt("how is RELIANCE".into());
    app.on_key(KeyEvent::new(KeyCode::Char('n'), KeyModifiers::CONTROL));
    assert!(app.analysis.is_empty());
}

#[test]
fn failed_turn_outcome_surfaces_as_a_visible_error() {
    use tantra_agent_chat::{ChatErrorView, TurnOutcome};
    use tantra_tui::terminal::turn_outcome_update;
    let err = ChatErrorView {
        kind: "transport".into(),
        message: "GGUF model file not found".into(),
    };
    let upd = turn_outcome_update(&TurnOutcome::Failed { error: err });
    let mut app = TuiApp::new();
    app.submit_prompt("What is NIFTY?".into());
    app.apply_stream_update(AgentStreamUpdate::Finished);
    app.apply_stream_update(upd);
    assert!(!app.is_loading);
    let last = app.chat.last().map(|m| m.text.clone()).unwrap_or_default();
    assert!(last.contains("GGUF model file not found"), "{last}");
    assert!(matches!(
        turn_outcome_update(&TurnOutcome::Completed),
        AgentStreamUpdate::Finished
    ));
}
