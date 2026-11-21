//! Link between the TUI and the agent over ACP (embedded host or runtime `/api/v1/acp`).

use std::fmt;
use std::sync::Arc;
use std::sync::atomic::AtomicBool;

use agent_client_protocol::v1::schema::StopReason;
use tantra_agent_acp::{
    AcpClient, AcpError, AgentHost, ClientEvent, ClientOptions, ModelCatalog, connect_embedded,
    connect_ws,
};
use tantra_agent_chat::{InMemorySessionStore, ToolRegistry, TurnConfig};
use tokio::sync::mpsc;

use crate::agent_chat::AgentStreamUpdate;

/// System prompt for the embedded agent.
pub const SYSTEM_PROMPT: &str = "You are Tantra, an intelligent quantitative finance and algorithmic trading assistant for Indian equities (NSE/BSE).";

/// Where the TUI's agent lives.
#[derive(Clone)]
pub enum AgentConnection {
    /// In-process agent host using the local model catalog.
    Embedded {
        /// Model catalog (from `tantra.jsonc`).
        catalog: Arc<dyn ModelCatalog>,
        /// Initial model id override.
        model: Option<String>,
    },
    /// Runtime ACP websocket endpoint (full runtime tool set).
    Remote {
        /// `ws://host:port/api/v1/acp` (a bare `ws://host:port` gets that path).
        url: String,
        /// Bearer token (never logged).
        token: Option<String>,
    },
}

impl fmt::Debug for AgentConnection {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Embedded { model, .. } => f
                .debug_struct("Embedded")
                .field("model", model)
                .finish_non_exhaustive(),
            Self::Remote { url, token } => f
                .debug_struct("Remote")
                .field("url", url)
                .field("token", &token.as_ref().map(|_| "[REDACTED]"))
                .finish(),
        }
    }
}

/// Build the embedded agent host: strategy and presentation tools.
pub fn embedded_host(catalog: Arc<dyn ModelCatalog>) -> Arc<AgentHost> {
    let mut registry = ToolRegistry::new();
    registry.register_all(tantra_agent_chat_tools::catalog::create_strategy_tools(
        None,
    ));
    registry.register_all(tantra_agent_chat_tools::catalog::create_presentation_tools());
    Arc::new(AgentHost {
        system_prompt: format!(
            "{SYSTEM_PROMPT}\n\n{}",
            tantra_agent_chat_tools::PRESENT_VIEW_GUIDANCE
        ),
        tools: Arc::new(registry.snapshot()),
        store: Arc::new(InMemorySessionStore::new()),
        catalog,
        turn_config: TurnConfig::default(),
        max_sessions: 4,
    })
}

/// Open a fresh ACP session.
pub async fn connect(
    conn: &AgentConnection,
    approve: Arc<AtomicBool>,
) -> Result<(AcpClient, mpsc::Receiver<ClientEvent>), AcpError> {
    match conn {
        AgentConnection::Embedded { catalog, model } => {
            let opts = ClientOptions {
                approve: Some(approve),
                model: model.clone(),
                ..Default::default()
            };
            connect_embedded(embedded_host(Arc::clone(catalog)), opts).await
        }
        AgentConnection::Remote { url, token } => {
            let opts = ClientOptions {
                approve: Some(approve),
                ..Default::default()
            };
            connect_ws(&acp_url(url), token.as_deref(), opts).await
        }
    }
}

/// Map an ACP client event to a transcript update (`None` = not shown).
pub fn map_client_event(ev: ClientEvent) -> Option<AgentStreamUpdate> {
    match ev {
        ClientEvent::Text(t) => Some(AgentStreamUpdate::Token(t)),
        ClientEvent::Thought(_) => None,
        ClientEvent::ToolStarted {
            call_id,
            tool,
            title,
        } => Some(AgentStreamUpdate::ToolStarted {
            call_id,
            name: tool,
            summary: title,
        }),
        ClientEvent::ToolFinished {
            call_id,
            tool,
            ok,
            elapsed_ms,
            blocks,
            ..
        } => Some(AgentStreamUpdate::ToolFinished {
            call_id,
            name: tool,
            ok,
            elapsed_ms,
            blocks,
        }),
        ClientEvent::TurnEnded(
            StopReason::EndTurn | StopReason::MaxTokens | StopReason::Cancelled,
        ) => Some(AgentStreamUpdate::Finished),
        ClientEvent::TurnEnded(other) => {
            Some(AgentStreamUpdate::Error(format!("turn ended: {other:?}")))
        }
        ClientEvent::Error(e) => Some(AgentStreamUpdate::Error(e)),
    }
}

/// Default ACP path on the runtime (`/api/v1` router, `acp_ws_handler`).
pub const RUNTIME_ACP_PATH: &str = "/api/v1/acp";

/// Complete a `--server` URL: a bare `ws://host:port` (no path) gets [`RUNTIME_ACP_PATH`].
pub fn acp_url(url: &str) -> String {
    let trimmed = url.trim().trim_end_matches('/');
    let after_scheme = trimmed.split_once("://").map_or(trimmed, |(_, rest)| rest);
    if after_scheme.contains('/') {
        trimmed.to_string()
    } else {
        format!("{trimmed}{RUNTIME_ACP_PATH}")
    }
}
