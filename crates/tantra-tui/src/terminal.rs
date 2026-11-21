//! Interactive terminal event loop; agent turns run over ACP.

use std::io::{self, Stdout};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use crossterm::event::{self, Event};
use crossterm::execute;
use crossterm::terminal::{
    EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode,
};
use ratatui::Terminal;
use ratatui::backend::CrosstermBackend;
use tantra_agent_acp::{AcpClient, ClientEvent};
use tokio::sync::mpsc;

use crate::agent_chat::{AgentMessage, AgentRole, AgentStreamUpdate};
use crate::agent_link::{AgentConnection, connect, map_client_event};
use crate::app::TuiApp;
use crate::ui;

/// Run the interactive Ratatui terminal workstation.
///
/// Connects to the agent (embedded or remote ACP) before entering raw mode,
/// then streams turn events into the AgentChat tab.
pub async fn run_terminal(mut app: TuiApp, conn: AgentConnection) -> Result<(), io::Error> {
    let approve = Arc::new(AtomicBool::new(app.auto_approve));
    let (client, ev_rx) = connect(&conn, Arc::clone(&approve))
        .await
        .map_err(|e| io::Error::other(format!("agent connection failed: {e}")))?;

    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let res = run_loop(&mut terminal, &mut app, conn, approve, client, ev_rx).await;

    // Clean teardown
    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
    terminal.show_cursor()?;

    res
}

fn sync_models(app: &mut TuiApp, client: &AcpClient) {
    app.model_name = client.model().to_string();
    app.base_model_name = app.model_name.clone();
    app.available_models = client
        .models()
        .iter()
        .map(|m| (m.id.clone(), m.name.clone()))
        .collect();
}

fn system_msg(app: &mut TuiApp, text: String) {
    app.chat.push(AgentMessage::new(AgentRole::System, text));
}

async fn run_loop(
    terminal: &mut Terminal<CrosstermBackend<Stdout>>,
    app: &mut TuiApp,
    conn: AgentConnection,
    approve: Arc<AtomicBool>,
    client: AcpClient,
    ev_rx: mpsc::Receiver<ClientEvent>,
) -> Result<(), io::Error> {
    let mut client = client;
    let mut ev_rx = ev_rx;
    let mut events_open = true;
    sync_models(app, &client);

    loop {
        app.tick = app.tick.wrapping_add(1);
        approve.store(app.auto_approve, Ordering::Relaxed);
        terminal.draw(|frame| ui::draw(frame, app))?;

        if app.should_quit {
            break;
        }

        // Fresh session requested (/new, Ctrl+N).
        if app.reset_session {
            app.reset_session = false;
            match connect(&conn, Arc::clone(&approve)).await {
                Ok((c, rx)) => {
                    client = c;
                    ev_rx = rx;
                    events_open = true;
                    sync_models(app, &client);
                }
                Err(e) => system_msg(app, format!("New session failed: {e}")),
            }
        }

        // Drain agent events.
        while events_open {
            match ev_rx.try_recv() {
                Ok(ev) => {
                    if let Some(update) = map_client_event(ev) {
                        app.apply_stream_update(update);
                    }
                }
                Err(mpsc::error::TryRecvError::Empty) => break,
                Err(mpsc::error::TryRecvError::Disconnected) => {
                    events_open = false;
                    app.apply_stream_update(AgentStreamUpdate::Error(
                        "agent connection closed".to_string(),
                    ));
                }
            }
        }

        if app.cancel_requested {
            app.cancel_requested = false;
            client.cancel().await;
        }

        if let Some(id) = app.pending_model.take() {
            match client.set_model(&id).await {
                Ok(()) => {
                    app.model_name = id.clone();
                    app.base_model_name = id.clone();
                    system_msg(app, format!("Model set to {id}"));
                }
                Err(e) => system_msg(app, format!("Model switch failed: {e}")),
            }
        }

        if let Some(prompt) = app.pending_prompt.take() {
            if let Err(e) = client.prompt(prompt).await {
                app.apply_stream_update(AgentStreamUpdate::Error(e.to_string()));
            }
        }

        // Poll for input with short timeout for responsive token rendering
        if event::poll(Duration::from_millis(30))? {
            if let Event::Key(key) = event::read()? {
                app.on_key(key);
            }
        }
    }

    Ok(())
}

/// Map a terminal turn outcome to the transcript update: a clean finish, or an
/// error line the user can see (a failed provider, a refusal, a round limit).
pub fn turn_outcome_update(outcome: &tantra_agent_chat::TurnOutcome) -> AgentStreamUpdate {
    use tantra_agent_chat::TurnOutcome;
    match outcome {
        TurnOutcome::Completed | TurnOutcome::MaxTokens => AgentStreamUpdate::Finished,
        TurnOutcome::Failed { error } => AgentStreamUpdate::Error(error.message.clone()),
        TurnOutcome::Refused { reason } => AgentStreamUpdate::Error(format!("refused: {reason}")),
        TurnOutcome::RoundLimit { limit } => {
            AgentStreamUpdate::Error(format!("stopped after {limit} tool rounds"))
        }
        TurnOutcome::Stopped | TurnOutcome::Cancelled => {
            AgentStreamUpdate::Error("cancelled".to_string())
        }
    }
}
