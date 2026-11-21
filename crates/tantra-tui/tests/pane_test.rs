//! Dynamic right-pane tests: pure stack/state, slash routing and rendering.

use ratatui::Terminal;
use ratatui::backend::TestBackend;
use tantra_agent_core::{DisplayBlock, MetricItem, ViewKind};
use tantra_tui::analysis::ToolCard;
use tantra_tui::pane::{
    AgentView, PANE_CAP, Pane, PaneBody, PaneStack, view_cards, view_kind_label,
};
use tantra_tui::{AgentRole, AgentStreamUpdate, TuiApp, draw};

fn info(title: &str) -> Pane {
    Pane::new(title, PaneBody::Info { lines: vec![] })
}

fn active_title(s: &PaneStack) -> String {
    s.active().map(|p| p.title.clone()).unwrap_or_default()
}

#[test]
fn open_dedups_and_activates() {
    let mut s = PaneStack::default();
    s.open(Pane::new("Help", PaneBody::Help { topic: None }));
    s.open(Pane::new("Analysis", PaneBody::Analysis));
    s.open(Pane::new(
        "Help · /x",
        PaneBody::Help {
            topic: Some("/x".into()),
        },
    ));
    assert_eq!(s.titles().len(), 2);
    assert_eq!(active_title(&s), "Help · /x");
    // Info dedups by title, not by variant alone.
    s.open(info("/mcp"));
    s.open(info("/memory"));
    s.open(info("/mcp"));
    assert_eq!(s.titles().len(), 4);
    assert_eq!(active_title(&s), "/mcp");
}

#[test]
fn cap_eviction_keeps_active() {
    let mut s = PaneStack::default();
    for i in 0..(PANE_CAP + 3) {
        s.open(info(&format!("i{i}")));
    }
    assert_eq!(s.titles().len(), PANE_CAP);
    assert_eq!(active_title(&s), format!("i{}", PANE_CAP + 2));
    assert!(!s.titles().iter().any(|(t, _)| t == "i0"));
}

#[test]
fn next_prev_wrap_and_close() {
    let mut s = PaneStack::default();
    s.open(info("a"));
    s.open(info("b"));
    s.open(info("c"));
    s.next();
    assert_eq!(active_title(&s), "a");
    s.prev();
    assert_eq!(active_title(&s), "c");
    s.close_active();
    assert_eq!(active_title(&s), "b");
    let flags: Vec<bool> = s.titles().iter().map(|(_, a)| *a).collect();
    assert_eq!(flags, vec![false, true]);
    s.close_active();
    s.close_active();
    assert!(s.is_empty());
    assert!(s.active().is_none());
    s.clear();
}

fn view(turn: u64, include: &[&str]) -> AgentView {
    AgentView {
        kind: ViewKind::Options,
        title: "t".into(),
        symbols: vec![],
        summary: String::new(),
        highlights: vec![],
        include_tools: include.iter().map(|s| s.to_string()).collect(),
        turn,
    }
}

fn card(tool: &str, turn: u64) -> ToolCard {
    ToolCard {
        tool: tool.into(),
        block: DisplayBlock::Alert {
            level: "info".into(),
            message: "m".into(),
        },
        turn,
    }
}

#[test]
fn view_cards_filter() {
    let cards = vec![card("a", 2), card("b", 2), card("a", 1)];
    assert_eq!(view_cards(&view(2, &[]), &cards).len(), 2);
    let f = view_cards(&view(2, &["b"]), &cards);
    assert_eq!(f.len(), 1);
    assert_eq!(f[0].tool, "b");
    assert_eq!(view_cards(&view(1, &[]), &cards).len(), 1);
    assert_eq!(view_kind_label(ViewKind::StrategyLab), "Strategy Lab");
}

#[test]
fn help_opens_pane_without_chat() {
    let mut app = TuiApp::new();
    app.execute_slash_command("/help");
    assert!(app.chat.is_empty());
    assert_eq!(app.panes.active().unwrap().title, "Help");
    app.execute_slash_command("/help backtest");
    assert!(app.chat.is_empty());
    match &app.panes.active().unwrap().body {
        PaneBody::Help { topic } => assert_eq!(topic.as_deref(), Some("/backtest")),
        other => panic!("unexpected {other:?}"),
    }
}

#[test]
fn info_command_moves_output_to_pane() {
    let mut app = TuiApp::new();
    app.execute_slash_command("/mcp");
    assert!(
        !app.chat
            .messages
            .iter()
            .any(|m| m.role == AgentRole::System)
    );
    if !app.panes.is_empty() {
        assert_eq!(app.panes.active().unwrap().title, "/mcp");
    }
}

#[test]
fn prompt_opens_analysis_and_view_opens_pane() {
    let mut app = TuiApp::new();
    app.submit_prompt("analyse RELIANCE".into());
    assert_eq!(app.turn, 1);
    assert!(
        app.panes.active().unwrap().title.starts_with("Analysis"),
        "{}",
        app.panes.active().unwrap().title
    );
    app.apply_stream_update(AgentStreamUpdate::ToolFinished {
        call_id: "1".into(),
        name: "t".into(),
        ok: true,
        elapsed_ms: 1,
        blocks: vec![
            DisplayBlock::Table {
                title: "T".into(),
                columns: vec!["a".into()],
                rows: vec![vec!["1".into()]],
            },
            DisplayBlock::View {
                kind: ViewKind::Options,
                title: "Chain".into(),
                symbols: vec!["NIFTY".into()],
                summary: "# Hi **there**".into(),
                highlights: vec![MetricItem {
                    label: "IV".into(),
                    value: "12".into(),
                }],
                include_tools: vec![],
            },
        ],
    });
    assert_eq!(app.panes.active().unwrap().title, "Options: Chain");
    assert_eq!(app.analysis.cards.len(), 1);
    assert_eq!(app.analysis.cards[0].turn, app.turn);

    let mut terminal = Terminal::new(TestBackend::new(120, 40)).unwrap();
    terminal.draw(|f| draw(f, &app)).unwrap();

    app.execute_slash_command("/pane close");
    assert!(app.panes.active().unwrap().title.starts_with("Analysis"));
    app.execute_slash_command("/new");
    assert!(app.panes.is_empty());
}

#[test]
fn help_renders_in_right_pane_not_welcome() {
    let mut app = TuiApp::new();
    app.execute_slash_command("/help");
    let mut terminal = Terminal::new(TestBackend::new(120, 40)).unwrap();
    terminal.draw(|f| draw(f, &app)).unwrap();
    let buf = terminal.backend().buffer().clone();
    let text: String = (0..40)
        .map(|y| (0..120).map(|x| buf[(x, y)].symbol()).collect::<String>())
        .collect::<Vec<_>>()
        .join("\n");
    assert!(text.contains("Help"));
    assert!(text.contains("/backtest"));
    assert!(!text.contains("|_   _|_ _ _ __"));
}

#[test]
fn new_command_drops_previous_session_cards() {
    let mut app = TuiApp::new();
    app.submit_prompt("look at NIFTY".to_string());
    app.apply_stream_update(AgentStreamUpdate::ToolFinished {
        call_id: "1".into(),
        name: "t".into(),
        ok: true,
        elapsed_ms: 1,
        blocks: vec![DisplayBlock::Alert {
            level: "info".into(),
            message: "old".into(),
        }],
    });
    assert!(!app.analysis.cards.is_empty());
    app.execute_slash_command("/new");
    assert!(app.analysis.cards.is_empty());
    assert!(app.panes.is_empty());
}
