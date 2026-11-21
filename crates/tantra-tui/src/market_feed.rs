//! Daily-bar price feed for the Analysis Price card (runtime `GET /api/v2/bars/{id}`).

use std::fmt;
use std::str::FromStr;
use std::time::Duration;

use rust_decimal::Decimal;

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

/// REST base (`scheme://host[:port]`) for the connection.
pub fn runtime_base(conn: &AgentConnection) -> String {
    match conn {
        AgentConnection::Remote { url, .. } => {
            let (scheme, rest) = if let Some(r) = url.strip_prefix("ws://") {
                ("http", r)
            } else if let Some(r) = url.strip_prefix("wss://") {
                ("https", r)
            } else if let Some(r) = url.strip_prefix("https://") {
                ("https", r)
            } else if let Some(r) = url.strip_prefix("http://") {
                ("http", r)
            } else {
                ("http", url.as_str())
            };
            let host = rest.split(['/', '?', '#']).next().unwrap_or(rest);
            format!("{scheme}://{host}")
        }
        _ => DEFAULT_RUNTIME_BASE.to_string(),
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

/// HTTP client for runtime daily bars.
#[derive(Clone)]
pub struct PriceFeed {
    base: String,
    token: Option<String>,
    http: reqwest::Client,
}

impl fmt::Debug for PriceFeed {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("PriceFeed")
            .field("base", &self.base)
            .field("token", &self.token.as_ref().map(|_| "[REDACTED]"))
            .finish()
    }
}

impl PriceFeed {
    pub fn new(base: String, token: Option<String>) -> Self {
        let http = reqwest::Client::builder()
            .timeout(Duration::from_secs(5))
            .build()
            .unwrap_or_default();
        Self { base, token, http }
    }

    pub async fn daily_closes(
        &self,
        instrument: &str,
        from: chrono::NaiveDate,
    ) -> Result<Vec<Decimal>, PriceFeedError> {
        let id = bars_path_id(instrument);
        let encoded: String = id
            .bytes()
            .map(|b| match b {
                b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                    (b as char).to_string()
                }
                _ => format!("%{b:02X}"),
            })
            .collect();
        let url = format!(
            "{}/api/v2/bars/{}?tf=1d&from={}",
            self.base.trim_end_matches('/'),
            encoded,
            from.format("%Y-%m-%d")
        );
        let mut req = self.http.get(url);
        if let Some(t) = &self.token {
            req = req.bearer_auth(t);
        }
        let resp = req
            .send()
            .await
            .map_err(|e| PriceFeedError::Unreachable(e.without_url().to_string()))?;
        let status = resp.status();
        if status == reqwest::StatusCode::NOT_FOUND {
            // The runtime answers 404 `not_found` when the instrument is not in its
            // instrument master / data dir: there are no bars to show.
            return Err(PriceFeedError::NoBars);
        }
        if !status.is_success() {
            return Err(PriceFeedError::Http(status.as_u16()));
        }
        let body: serde_json::Value = resp
            .json()
            .await
            .map_err(|e| PriceFeedError::BadBody(e.without_url().to_string()))?;
        parse_bar_closes(&body)
    }
}
