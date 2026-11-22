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
use crate::market_feed::{PriceFeed, PriceFeedError, PriceStatus, resolve_runtime_target};
use crate::ui;
use rust_decimal::Decimal;

/// Run the interactive Ratatui terminal workstation.
///
/// Connects to the agent (embedded or remote ACP) before entering raw mode,
/// then streams turn events into the AgentChat tab.
pub async fn run_terminal(mut app: TuiApp, conn: AgentConnection) -> Result<(), io::Error> {
    let approve = Arc::new(AtomicBool::new(app.auto_approve));
    let cfg_runtime = config_runtime();
    let (base, runtime_token) = resolve_runtime_target(
        &conn,
        cfg_runtime
            .as_ref()
            .map(|e| (e.url.as_str(), e.token.as_deref())),
    );
    app.runtime_base = base;
    let (client, ev_rx) = tokio::time::timeout(
        Duration::from_secs(15),
        connect(&conn, Arc::clone(&approve)),
    )
    .await
    .map_err(|_| io::Error::other("agent connection timed out"))?
    .map_err(|e| io::Error::other(connect_error_message(&conn, &e.to_string())))?;

    enable_raw_mode()?;
    let mut stdout = io::stdout();
    if let Err(e) = execute!(stdout, EnterAlternateScreen) {
        let _ = disable_raw_mode();
        return Err(e);
    }
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = match Terminal::new(backend) {
        Ok(t) => t,
        Err(e) => {
            let _ = disable_raw_mode();
            return Err(e);
        }
    };

    let res = run_loop(
        &mut terminal,
        &mut app,
        conn,
        approve,
        client,
        ev_rx,
        runtime_token,
    )
    .await;

    // Clean teardown
    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
    terminal.show_cursor()?;

    res
}

/// `runtime` block of `tantra.jsonc` (discovered from cwd/user dir). A load or resolve failure
/// is non-fatal: warn and fall back to defaults.
fn config_runtime() -> Option<tantra_agent_llm::RuntimeEndpoint> {
    let loaded = match tantra_agent_llm::load_config(None) {
        Ok(l) => l,
        Err(tantra_agent_llm::ConfigError::NotFound { .. }) => return None,
        Err(e) => {
            eprintln!("warning: tantra.jsonc unreadable ({e}); using default runtime endpoint");
            return None;
        }
    };
    match tantra_agent_llm::resolve_runtime(&loaded, &|k| std::env::var(k).ok()) {
        Ok(e) => e,
        Err(e) => {
            eprintln!("warning: tantra.jsonc runtime block unusable ({e}); using defaults");
            None
        }
    }
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
    runtime_token: Option<String>,
) -> Result<(), io::Error> {
    let mut client = client;
    let mut ev_rx = ev_rx;
    let mut events_open = true;
    sync_models(app, &client);

    let feed = PriceFeed::new(app.runtime_base.clone(), runtime_token);
    let (px_tx, mut px_rx) = mpsc::channel::<(String, Result<Vec<Decimal>, PriceFeedError>)>(8);
    let mut last_req: Option<(String, std::time::Instant)> = None;

    loop {
        app.tick = app.tick.wrapping_add(1);

        // Price card: fetch daily bars when the focus changes or goes stale.
        if let Some(sym) = app.analysis.primary().map(str::to_string) {
            let due = match &last_req {
                Some((s, at)) => *s != sym || at.elapsed() >= Duration::from_secs(300),
                None => true,
            };
            if due {
                last_req = Some((sym.clone(), std::time::Instant::now()));
                app.price_status = PriceStatus::Loading(sym.clone());
                let feed = feed.clone();
                let tx = px_tx.clone();
                let from = chrono::Local::now().date_naive() - chrono::Days::new(120);
                tokio::spawn(async move {
                    let res = feed.daily_closes(&sym, from).await;
                    let _ = tx.send((sym, res)).await;
                });
            }
        }
        while let Ok((sym, res)) = px_rx.try_recv() {
            match res {
                Ok(closes) if closes.len() >= 2 => {
                    app.live.seed_history(&sym, closes);
                    app.live.connected = true;
                    app.price_status = PriceStatus::Ready(sym);
                }
                Ok(_) => {
                    app.price_status = PriceStatus::Failed {
                        symbol: sym,
                        error: PriceFeedError::NoBars.to_string(),
                    }
                }
                Err(e) => {
                    app.price_status = PriceStatus::Failed {
                        symbol: sym,
                        error: e.to_string(),
                    }
                }
            }
        }

        approve.store(app.auto_approve, Ordering::Relaxed);
        terminal.draw(|frame| ui::draw(frame, app))?;

        if app.should_quit {
            break;
        }

        // Fresh session requested (/new, Ctrl+N).
        if app.reset_session {
            app.reset_session = false;
            match tokio::time::timeout(
                Duration::from_secs(10),
                connect(&conn, Arc::clone(&approve)),
            )
            .await
            {
                Ok(Ok((c, rx))) => {
                    client = c;
                    ev_rx = rx;
                    events_open = true;
                    sync_models(app, &client);
                }
                Ok(Err(e)) => system_msg(app, format!("New session failed: {e}")),
                Err(_) => system_msg(app, "reconnect timed out after 10s".to_string()),
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
            match tokio::time::timeout(Duration::from_secs(10), client.set_model(&id)).await {
                Ok(Ok(())) => {
                    app.model_name = id.clone();
                    app.base_model_name = id.clone();
                    system_msg(app, format!("Model set to {id}"));
                }
                Ok(Err(e)) => system_msg(app, format!("Model switch failed: {e}")),
                Err(_) => system_msg(app, "model switch timed out after 10s".to_string()),
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

/// Human-readable connection failure, with a hint for the common wrong-URL cases.
pub fn connect_error_message(conn: &AgentConnection, err: &str) -> String {
    let mut msg = format!("agent connection failed: {err}");
    if let AgentConnection::Remote { url, .. } = conn {
        if err.contains("HTTP 404") {
            msg.push_str(&format!(
                "\n  {url} has no ACP endpoint. Point --server at the tantra runtime \
                 (tantra-ui-web/scripts/start-backend.sh, port 8000), e.g. \
                 ws://127.0.0.1:8000{}; port 8080 is the LLM server, not the runtime.",
                crate::agent_link::RUNTIME_ACP_PATH
            ));
        } else if err.contains("onnection refused") || err.contains("unreachable") {
            msg.push_str(
                "\n  nothing is listening there — start the runtime: tantra-ui-web/scripts/start-backend.sh",
            );
        }
    }
    msg
}
