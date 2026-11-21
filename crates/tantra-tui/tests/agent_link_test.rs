//! ACP link: event mapping, embedded host, end-to-end embedded turn, `/model`.

use std::sync::Arc;
use std::sync::atomic::AtomicBool;

use agent_client_protocol::v1::schema::StopReason;
use tantra_agent_acp::{ClientEvent, StaticCatalog};
use tantra_agent_llm::ScriptedFakeLlm;
use tantra_tui::agent_link::{AgentConnection, connect, embedded_host, map_client_event};
use tantra_tui::pane::PaneBody;
use tantra_tui::{AgentStreamUpdate, TuiApp};

fn catalog(reply: &str) -> Arc<StaticCatalog> {
    let fake = ScriptedFakeLlm::new("m");
    fake.push_text_response(reply);
    Arc::new(StaticCatalog::new().with_model("m", "M", Arc::new(fake)))
}

#[test]
fn maps_every_client_event() {
    assert!(matches!(
        map_client_event(ClientEvent::Text("a".into())),
        Some(AgentStreamUpdate::Token(t)) if t == "a"
    ));
    assert!(map_client_event(ClientEvent::Thought("x".into())).is_none());
    assert!(matches!(
        map_client_event(ClientEvent::ToolStarted {
            call_id: "c".into(),
            tool: "t".into(),
            title: "s".into()
        }),
        Some(AgentStreamUpdate::ToolStarted { call_id, name, summary })
            if call_id == "c" && name == "t" && summary == "s"
    ));
    assert!(matches!(
        map_client_event(ClientEvent::ToolFinished {
            call_id: "c".into(),
            tool: "t".into(),
            ok: true,
            elapsed_ms: 5,
            blocks: Vec::new(),
            detail: String::new()
        }),
        Some(AgentStreamUpdate::ToolFinished {
            ok: true,
            elapsed_ms: 5,
            ..
        })
    ));
    assert!(matches!(
        map_client_event(ClientEvent::TurnEnded(StopReason::EndTurn)),
        Some(AgentStreamUpdate::Finished)
    ));
    assert!(matches!(
        map_client_event(ClientEvent::TurnEnded(StopReason::MaxTokens)),
        Some(AgentStreamUpdate::Finished)
    ));
    assert!(matches!(
        map_client_event(ClientEvent::TurnEnded(StopReason::Refusal)),
        Some(AgentStreamUpdate::Error(_))
    ));
    assert!(matches!(
        map_client_event(ClientEvent::TurnEnded(StopReason::Cancelled)),
        Some(AgentStreamUpdate::Error(_))
    ));
    assert!(matches!(
        map_client_event(ClientEvent::Error("boom".into())),
        Some(AgentStreamUpdate::Error(e)) if e == "boom"
    ));
}

#[test]
fn embedded_host_exposes_present_view() {
    let host = embedded_host(catalog("x"));
    assert!(host.tools.specs().iter().any(|s| s.name == "present_view"));
    assert!(host.system_prompt.contains("present_view"));
}

#[test]
fn remote_debug_redacts_token() {
    let c = AgentConnection::Remote {
        url: "ws://h/v1/acp".into(),
        token: Some("sekrit".into()),
    };
    assert!(!format!("{c:?}").contains("sekrit"));
}

#[tokio::test]
async fn embedded_turn_streams_scripted_reply() {
    let conn = AgentConnection::Embedded {
        catalog: catalog("hello there"),
        model: None,
    };
    let (client, mut rx) = connect(&conn, Arc::new(AtomicBool::new(false)))
        .await
        .expect("connect");
    client.prompt("hi".into()).await.expect("prompt");
    let mut text = String::new();
    while let Some(ev) = rx.recv().await {
        match map_client_event(ev) {
            Some(AgentStreamUpdate::Token(t)) => text.push_str(&t),
            Some(AgentStreamUpdate::Finished) => break,
            Some(AgentStreamUpdate::Error(e)) => panic!("error: {e}"),
            _ => {}
        }
    }
    assert_eq!(text, "hello there");
}

#[test]
fn model_command_lists_and_switches() {
    let mut app = TuiApp::new();
    app.available_models = vec![("a".into(), "A".into()), ("b".into(), "B".into())];
    app.model_name = "a".into();
    app.execute_slash_command("/model");
    let pane = app.panes.active().expect("info pane");
    assert_eq!(pane.title, "/model");
    match &pane.body {
        PaneBody::Info { lines } => {
            assert!(lines.iter().any(|l| l.starts_with('*') && l.contains(" a")));
            assert!(lines.iter().any(|l| l.contains(" b")));
        }
        _ => panic!("expected info pane"),
    }
    app.execute_slash_command("/model b");
    assert_eq!(app.pending_model.as_deref(), Some("b"));
    app.pending_model = None;
    app.execute_slash_command("/model zzz");
    assert!(app.pending_model.is_none());
}
