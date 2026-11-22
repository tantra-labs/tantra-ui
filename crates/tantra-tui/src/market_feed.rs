//! Daily-bar price feed for the Analysis Price card (runtime `GET /api/v2/bars/{id}`).

use std::str::FromStr;
use std::time::Duration;

use rust_decimal::Decimal;
use tantra_runtime_client::{ClientError, RuntimeClient};

use crate::agent_link::AgentConnection;

/// Local `tantra serve` default.
pub const DEFAULT_RUNTIME_BASE: &str = "http://127.0.0.1:8000";

/// Fetch status of the Price card for the focused symbol.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum PriceStatus {
    #[default]
    Idle,
    Loading(String),
    Ready(String),
    Failed {
        symbol: String,
        error: String,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum PriceFeedError {
    #[error("unreachable: {0}")]
    Unreachable(String),
    #[error("HTTP {0}")]
    Http(u16),
    #[error("bad body: {0}")]
    BadBody(String),
    #[error("no bars")]
    NoBars,
}

impl PriceFeedError {
    /// `Display` prefix of [`PriceFeedError::Unreachable`].
    pub const UNREACHABLE_PREFIX: &'static str = "unreachable";
    /// `Display` text of [`PriceFeedError::NoBars`].
    pub const NO_BARS_TEXT: &'static str = "no bars";
}

/// Map an instrument id to the runtime bars path id (port of
/// `toRuntimeInstrumentPathId`).
pub fn bars_path_id(instrument: &str) -> String {
    let trimmed = instrument.trim();
    let colon = trimmed.find(':');
    if colon.is_none() && trimmed.contains('.') {
        return trimmed.to_string();
    }
    let (exchange, rest) = match colon {
        Some(c) => (&trimmed[..c], &trimmed[c + 1..]),
        None => ("NSE", trimmed),
    };
    let symbol = ["-EQ", "-BE", "-BZ", "-SM"]
        .iter()
        .find_map(|suf| rest.strip_suffix(suf))
        .unwrap_or(rest);
    format!("{symbol}.{exchange}")
}

/// Normalise any `ws(s)://` / `http(s)://` / bare URL to `scheme://host[:port]`.
pub fn base_of(url: &str) -> String {
    let (scheme, rest) = if let Some(r) = url.strip_prefix("ws://") {
        ("http", r)
    } else if let Some(r) = url.strip_prefix("wss://") {
        ("https", r)
    } else if let Some(r) = url.strip_prefix("https://") {
        ("https", r)
    } else if let Some(r) = url.strip_prefix("http://") {
        ("http", r)
    } else {
        ("http", url)
    };
    let host = rest.split(['/', '?', '#']).next().unwrap_or(rest);
    format!("{scheme}://{host}")
}

/// REST base (`scheme://host[:port]`) for the connection.
pub fn runtime_base(conn: &AgentConnection) -> String {
    match conn {
        AgentConnection::Remote { url, .. } => base_of(url),
        _ => DEFAULT_RUNTIME_BASE.to_string(),
    }
}

/// Resolve the REST base and bearer token. Precedence: explicit flags (`--server`,
/// `--token-file`, carried by a `Remote` connection), then the `tantra.jsonc` `runtime`
/// block `(url, token)`, then the default base with no token. A config token is only sent to
/// the config's own host, never to a different `--server`.
pub fn resolve_runtime_target(
    conn: &AgentConnection,
    cfg: Option<(&str, Option<&str>)>,
) -> (String, Option<String>) {
    match conn {
        AgentConnection::Remote { url, token } => {
            let base = base_of(url);
            let token = token.clone().or_else(|| {
                cfg.and_then(|(cu, ct)| if base_of(cu) == base { ct } else { None })
                    .map(str::to_string)
            });
            (base, token)
        }
        _ => match cfg {
            Some((url, token)) => (base_of(url), token.map(str::to_string)),
            None => (DEFAULT_RUNTIME_BASE.to_string(), None),
        },
    }
}

/// Extract closes (oldest first) from an `/api/v2/bars` response body.
pub fn parse_bar_closes(body: &serde_json::Value) -> Result<Vec<Decimal>, PriceFeedError> {
    let root = body.get("data").unwrap_or(body);
    let bars = root
        .get("bars")
        .and_then(|b| b.as_array())
        .ok_or_else(|| PriceFeedError::BadBody("missing bars array".into()))?;
    if bars.is_empty() {
        return Err(PriceFeedError::NoBars);
    }
    bars.iter()
        .map(|bar| {
            let close = bar
                .get("close")
                .ok_or_else(|| PriceFeedError::BadBody("bar without close".into()))?;
            let text = match close {
                serde_json::Value::String(s) => s.clone(),
                serde_json::Value::Number(n) => n.to_string(),
                _ => return Err(PriceFeedError::BadBody("close is not a number".into())),
            };
            Decimal::from_str(&text)
                .or_else(|_| Decimal::from_scientific(&text))
                .map_err(|e| PriceFeedError::BadBody(format!("close {text:?}: {e}")))
        })
        .collect()
}

/// HTTP client for runtime daily bars (over [`RuntimeClient`]).
#[derive(Clone, Debug)]
pub struct PriceFeed {
    client: Result<RuntimeClient, ClientError>,
}

impl PriceFeed {
    pub fn new(base: String, token: Option<String>) -> Self {
        Self {
            client: RuntimeClient::with_timeout(&base, token, Duration::from_secs(5)),
        }
    }

    pub async fn daily_closes(
        &self,
        instrument: &str,
        from: chrono::NaiveDate,
    ) -> Result<Vec<Decimal>, PriceFeedError> {
        let client = self
            .client
            .as_ref()
            .map_err(|e| PriceFeedError::Unreachable(e.to_string()))?;
        let id = bars_path_id(instrument);
        let from = from.format("%Y-%m-%d").to_string();
        match client.bars(&id, "1d", Some(&from), None).await {
            Ok(body) => parse_bar_closes(&body),
            // The runtime answers 404 `not_found` when the instrument is not in its
            // instrument master / data dir: there are no bars to show.
            Err(ClientError::Http { status: 404, .. }) => Err(PriceFeedError::NoBars),
            Err(ClientError::Http { status, .. }) => Err(PriceFeedError::Http(status)),
            Err(ClientError::BadBody(m)) => Err(PriceFeedError::BadBody(m)),
            Err(e) => Err(PriceFeedError::Unreachable(e.to_string())),
        }
    }
}
