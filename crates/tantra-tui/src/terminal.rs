//! Interactive terminal event loop and LLM stream orchestration.

use std::collections::HashSet;
use std::io::{self, Stdout};
use std::sync::Arc;
use std::time::Duration;

use crossterm::event::{self, Event};
use crossterm::execute;
use crossterm::terminal::{
    EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode,
};
use ratatui::Terminal;
use ratatui::backend::CrosstermBackend;
use tokio::sync::{Mutex, mpsc};

use tantra_agent_chat::{
    CancellationToken, InMemorySessionStore, PermissionDecision, SessionEvent, ToolOutcome,
    ToolRegistry, TurnConfig, TurnId, TurnOutcome, run_turn,
};
use tantra_agent_core::{PersistedMessage, SessionStore};
use tantra_agent_llm::LlmProvider;

use crate::agent_chat::AgentStreamUpdate;
use crate::app::TuiApp;
use crate::ui;

/// Run the interactive Ratatui terminal workstation.
///
/// Handles terminal setup/teardown, event loop polling, and live token
/// streaming between the in-process LLM provider and the AgentChat tab.
pub async fn run_terminal(
    mut app: TuiApp,
    provider: Arc<dyn LlmProvider>,
) -> Result<(), io::Error> {
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let res = run_loop(&mut terminal, &mut app, provider).await;

    // Clean teardown
    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
    terminal.show_cursor()?;

    res
}

const SYSTEM_PROMPT: &str = "You are Tantra, an intelligent quantitative finance and algorithmic trading assistant for Indian equities (NSE/BSE).";

async fn run_loop(
    terminal: &mut Terminal<CrosstermBackend<Stdout>>,
    app: &mut TuiApp,
    provider: Arc<dyn LlmProvider>,
) -> Result<(), io::Error> {
    let (token_tx, mut token_rx) = mpsc::channel::<AgentStreamUpdate>(128);

    let mut registry = ToolRegistry::new();
    registry.register_all(tantra_agent_chat_tools::catalog::create_strategy_tools(
        None,
    ));
    let tools = Arc::new(registry.snapshot());
    let store: Arc<dyn SessionStore> = Arc::new(InMemorySessionStore::new());
    let history: Arc<Mutex<Vec<PersistedMessage>>> = Arc::new(Mutex::new(Vec::new()));
    let grants: Arc<Mutex<HashSet<String>>> = Arc::new(Mutex::new(HashSet::new()));
    let mut turn_counter: u64 = 0;

    loop {
        app.tick = app.tick.wrapping_add(1);
        terminal.draw(|frame| ui::draw(frame, app))?;

        if app.should_quit {
            break;
        }

        // Drain any incoming tokens from background streaming task
        while let Ok(update) = token_rx.try_recv() {
            app.apply_stream_update(update);
        }

        // Check if user submitted a prompt on the Agent tab
        if let Some(prompt) = app.pending_prompt.take() {
            turn_counter += 1;
            let turn = turn_counter;
            let auto_approve = app.auto_approve;
            let provider = Arc::clone(&provider);
            let tools = Arc::clone(&tools);
            let store = Arc::clone(&store);
            let history = Arc::clone(&history);
            let grants = Arc::clone(&grants);
            let tx = token_tx.clone();

            tokio::spawn(async move {
                let (ev_tx, mut ev_rx) = mpsc::channel::<SessionEvent>(256);
                let fwd_tx = tx.clone();
                let forwarder = tokio::spawn(async move {
                    while let Some(ev) = ev_rx.recv().await {
                        let update = match ev {
                            SessionEvent::TextDelta { text, .. } => {
                                Some(AgentStreamUpdate::Token(text))
                            }
                            SessionEvent::ToolStarted {
                                call_id,
                                name,
                                summary,
                                ..
                            } => Some(AgentStreamUpdate::ToolStarted {
                                call_id: format!("{call_id:?}"),
                                name,
                                summary,
                            }),
                            SessionEvent::ToolFinished {
                                call_id,
                                name,
                                outcome,
                                elapsed_ms,
                                blocks,
                                ..
                            } => Some(AgentStreamUpdate::ToolFinished {
                                call_id: format!("{call_id:?}"),
                                name,
                                ok: matches!(outcome, ToolOutcome::Success),
                                elapsed_ms,
                                blocks,
                            }),
                            SessionEvent::PermissionRequested(mut prompt) => {
                                if let Some(responder) = prompt.responder.take() {
                                    let decision = if auto_approve {
                                        PermissionDecision::AllowOnce
                                    } else {
                                        PermissionDecision::Deny {
                                            feedback: Some(
                                                "not approved — enable /auto to let the agent run this tool"
                                                    .into(),
                                            ),
                                        }
                                    };
                                    let _ = responder.respond(decision);
                                }
                                None
                            }
                            SessionEvent::TurnEnded { outcome, .. } => {
                                Some(turn_outcome_update(&outcome))
                            }
                            _ => None,
                        };
                        if let Some(u) = update {
                            if fwd_tx.send(u).await.is_err() {
                                break;
                            }
                        }
                    }
                });

                let hist = history.lock().await.clone();
                let grant_set = grants.lock().await.clone();
                let result = run_turn(
                    None,
                    TurnId(turn),
                    SYSTEM_PROMPT,
                    hist,
                    &prompt,
                    Vec::new(),
                    &tools,
                    provider,
                    store,
                    ev_tx,
                    &TurnConfig::default(),
                    CancellationToken::new(),
                    grant_set,
                )
                .await;
                let _ = forwarder.await;
                match result {
                    Ok((outcome, new_history, new_grants)) => {
                        *history.lock().await = new_history;
                        *grants.lock().await = new_grants;
                        // TurnEnded normally carries this already; resending is idempotent
                        // for Finished and covers a turn that ended without the event.
                        if matches!(turn_outcome_update(&outcome), AgentStreamUpdate::Finished) {
                            let _ = tx.send(AgentStreamUpdate::Finished).await;
                        }
                    }
                    Err(e) => {
                        let _ = tx.send(AgentStreamUpdate::Error(e.to_string())).await;
                    }
                }
            });
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
pub fn turn_outcome_update(outcome: &TurnOutcome) -> AgentStreamUpdate {
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
