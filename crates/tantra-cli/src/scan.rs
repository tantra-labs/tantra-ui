//! Headless `tantra scan` wiring (gap `0001` closed; gap doc removed).
//!
//! Parses the CLI query into a canonical [`tantra_core::screener::ScreenerScanRequest`].
//! Two evaluation paths:
//! - **Live** (`tantra scan --server <url>`): `GET /screener/scan` on a running
//!   `tantra-runtime-api-rest` server (T10.4, gap `ui 0006`).
//! - **Offline fixture fallback** (no `--server`): the in-memory fixture universe
//!   below, evaluated through [`tantra_ui_api_wasm::screener::apply_scan`].

use std::collections::BTreeMap;
use std::error::Error as _;
use std::str::FromStr;

use rust_decimal::Decimal;
use tantra_core::screener::{
    FilterOp, MetricKeySpec, MetricRef, ScreenerFilterGroup, ScreenerFilterPredicate,
    ScreenerScanRequest,
};
use tantra_core::types::{Exchange, InstrumentId};
use tantra_runtime_api::ScreenerQuery;
use tantra_ui_api_wasm::screener::apply_scan;

use crate::args::ScanArgs;
use crate::runner::{CliError, CommandOutcome};

/// Fixture universe of NSE instruments with scalar metrics.
///
/// Mirrors the shape of a `tantra_runtime_api_rest` screener response row
/// (`instrument` + metric name → `Decimal`). Kept `Decimal`-only: zero floats.
/// `pub` so integration tests can assert against a known universe.
/// // offline fixture universe — live path is `tantra scan --server`; gap ui 0006
pub fn scan_universe() -> Vec<tantra_core::screener::ScreenerRow> {
    let row = |symbol: &str, pe: u64, roe: u64, market_cr: u64| {
        let instrument = InstrumentId::new(Exchange::Nse, symbol);
        let mut metrics = BTreeMap::new();
        metrics.insert("pe".to_string(), Decimal::from(pe));
        metrics.insert("roe".to_string(), Decimal::from(roe));
        metrics.insert("market_cap".to_string(), Decimal::from(market_cr));
        tantra_core::screener::ScreenerRow {
            instrument,
            metrics,
        }
    };

    vec![
        row("RELIANCE", 25, 15, 200_000_000),
        row("TCS", 30, 20, 150_000_000),
        row("INFY", 28, 18, 80_000_000),
        row("HDFCBANK", 22, 12, 120_000_000),
    ]
}

/// Build a [`MetricRef`] for a metric name at the daily granularity.
fn metric_ref(name: &str) -> MetricRef {
    MetricRef(MetricKeySpec {
        metric: name.to_string(),
        period: tantra_core::signal::Timeframe::D1,
    })
}

/// Parse a screener query into a [`ScreenerScanRequest`].
///
/// Supported syntax:
/// - `metric<op>value` — single predicate, `op` in `{>, <, >=, <=, ==, !=}`
/// - `metric between <low> <high>` — inclusive range predicate
/// - `pred1 and pred2` — AND-combined group (`match_all = true`)
///
/// Metric values are [`Decimal`] — zero floats.
pub fn parse_query(query: &str) -> Result<ScreenerScanRequest, String> {
    let query = query.trim();
    if query.is_empty() {
        return Err("query cannot be empty".to_string());
    }

    let parts: Vec<&str> = query
        .split(" and ")
        .map(|p| p.trim())
        .filter(|p| !p.is_empty())
        .collect();
    if parts.is_empty() {
        return Err("query cannot be empty".to_string());
    }

    let mut predicates = Vec::with_capacity(parts.len());
    for part in parts {
        let part = part.trim();
        if let Some((metric, rest)) = part.split_once(" between ") {
            let metric = metric.trim();
            let mut bounds = rest.split_whitespace();
            let low = bounds
                .next()
                .ok_or_else(|| format!("missing lower bound for 'between' on '{metric}'"))?;
            let high = bounds
                .next()
                .ok_or_else(|| format!("missing upper bound for 'between' on '{metric}'"))?;
            if bounds.next().is_some() {
                return Err(format!("too many bounds for 'between' on '{metric}'"));
            }
            let value: Decimal = low
                .parse()
                .map_err(|_| format!("invalid lower bound '{low}' for '{metric}'"))?;
            let value_upper: Decimal = high
                .parse()
                .map_err(|_| format!("invalid upper bound '{high}' for '{metric}'"))?;
            predicates.push(ScreenerFilterPredicate {
                metric: metric_ref(metric),
                op: FilterOp::Between,
                value,
                value_upper: Some(value_upper),
            });
            continue;
        }

        // Two-char operators first so `>=`/`<=`/`==`/`!=` win over `>`/`<`.
        let (metric, value_str, op) = if let Some((m, v)) = part.split_once(">=") {
            (m.trim(), v.trim(), FilterOp::Gte)
        } else if let Some((m, v)) = part.split_once("<=") {
            (m.trim(), v.trim(), FilterOp::Lte)
        } else if let Some((m, v)) = part.split_once("==") {
            (m.trim(), v.trim(), FilterOp::Eq)
        } else if let Some((m, v)) = part.split_once("!=") {
            (m.trim(), v.trim(), FilterOp::Ne)
        } else if let Some((m, v)) = part.split_once(">") {
            (m.trim(), v.trim(), FilterOp::Gt)
        } else if let Some((m, v)) = part.split_once("<") {
            (m.trim(), v.trim(), FilterOp::Lt)
        } else {
            return Err(format!("no comparison operator in predicate '{part}'"));
        };
        let value: Decimal = value_str
            .parse()
            .map_err(|_| format!("invalid value '{value_str}' for '{metric}'"))?;
        predicates.push(ScreenerFilterPredicate {
            metric: metric_ref(metric),
            op,
            value,
            value_upper: None,
        });
    }

    Ok(ScreenerScanRequest {
        universe: None, // full fixture universe
        filters: ScreenerFilterGroup {
            predicates,
            match_all: true,
        },
        sort: None,
        limit: None,
    })
}

/// Run a `tantra scan` against the fixture universe (offline fallback).
pub fn run_scan(query: &str) -> Result<CommandOutcome, CliError> {
    let request = parse_query(query).map_err(CliError::Execution)?;
    let universe = scan_universe();
    let response = apply_scan(&universe, &request)
        .map_err(|e| CliError::Execution(format!("screener evaluation failed: {e:?}")))?;
    Ok(CommandOutcome::ScanResults {
        rows: response.rows.len(),
    })
}

/// Default live-path universe: the fixture scrips as `NSE:<SYM>` instrument ids.
pub fn default_live_universe() -> Vec<String> {
    scan_universe()
        .iter()
        .map(|row| format!("NSE:{}", row.instrument.symbol))
        .collect()
}

/// Serialize a [`Decimal`] as a JSON number (the server's wire grammar only
/// accepts `Value::Number` operands; `rust_decimal`'s default serde emits
/// strings). Thresholds from `parse_query` are short, exact decimals, so
/// `Number::from_str` round-trips them losslessly.
fn decimal_json(value: &Decimal) -> serde_json::Value {
    match serde_json::Number::from_str(&value.to_string()) {
        Ok(num) => serde_json::Value::Number(num),
        Err(_) => serde_json::Value::String(value.to_string()),
    }
}

/// Map a canonical [`ScreenerScanRequest`] filter group into the server's wire
/// shape: `{"operator": "AND"|"OR", "items": [{"key", "op", "value", ...}]}`.
///
/// `FilterOp` maps to the server op strings `gt/gte/lt/lte/eq/ne/between`;
/// operands are JSON numbers (the server rejects strings), and `between` is
/// encoded as `value: [low, high]` per the server grammar.
pub fn to_server_filters(request: &ScreenerScanRequest) -> serde_json::Value {
    let mut items = Vec::with_capacity(request.filters.predicates.len());
    for p in &request.filters.predicates {
        let op = match p.op {
            FilterOp::Gt => "gt",
            FilterOp::Gte => "gte",
            FilterOp::Lt => "lt",
            FilterOp::Lte => "lte",
            FilterOp::Eq => "eq",
            FilterOp::Ne => "ne",
            FilterOp::Between => "between",
        };
        let value = if p.op == FilterOp::Between {
            let upper = p.value_upper.unwrap_or(p.value);
            serde_json::json!([decimal_json(&p.value), decimal_json(&upper)])
        } else {
            decimal_json(&p.value)
        };
        items.push(serde_json::json!({
            "key": p.metric.0.metric,
            "op": op,
            "value": value,
        }));
    }
    serde_json::json!({
        "operator": if request.filters.match_all { "AND" } else { "OR" },
        "items": items,
    })
}

/// Render a `reqwest` error with its full source chain, so a transport failure
/// names the actual cause (connect refused, timeout, TLS, ...).
fn error_chain(err: &reqwest::Error) -> String {
    let mut out = String::new();
    let mut source = err.source();
    while let Some(s) = source {
        out.push_str(&format!(": {s}"));
        source = s.source();
    }
    out
}

/// Run a screener scan against a live `GET /screener/scan` endpoint and return
/// the number of matching rows. `universe` holds `SYM.EXCH` or `EXCH:SYM`
/// instrument ids; `tf`/`as_of` are optional wire overrides.
pub fn live_scan(
    server: &str,
    request: &ScreenerScanRequest,
    universe: &[String],
    tf: Option<&str>,
    as_of: Option<&str>,
) -> Result<usize, CliError> {
    let filters = to_server_filters(request).to_string();
    let universe_json = serde_json::to_string(universe)
        .map_err(|e| CliError::Execution(format!("cannot serialize universe: {e}")))?;
    let query = ScreenerQuery {
        filters: Some(filters),
        universe: Some(universe_json),
        tf: tf.map(String::from),
        as_of: as_of.map(String::from),
    };
    let base = server.trim_end_matches('/');
    let url = format!("{base}/screener/scan");
    let resp = reqwest::blocking::Client::new()
        .get(&url)
        .query(&query)
        .send()
        .map_err(|e| {
            CliError::Execution(format!(
                "cannot reach screener server at {base}: {e}{}",
                error_chain(&e)
            ))
        })?;
    let status = resp.status();
    let body: serde_json::Value = resp
        .json()
        .map_err(|e| CliError::Execution(format!("invalid screener response from {base}: {e}")))?;
    if !status.is_success() {
        let code = body["error"]["code"].as_str().unwrap_or("error");
        let reason = body["error"]["context"]["reason"]
            .as_str()
            .or_else(|| body["error"]["message"].as_str())
            .unwrap_or("unknown error");
        return Err(CliError::Execution(format!(
            "screener server at {base} returned HTTP {status} ({code}): {reason}"
        )));
    }
    let rows = body["data"]["rows"]
        .as_array()
        .map(|r| r.len())
        .ok_or_else(|| {
            CliError::Execution(format!(
                "malformed screener response from {base}: missing data.rows"
            ))
        })?;
    Ok(rows)
}

/// Convenience wrapper over [`ScanArgs`].
///
/// With `--server` the scan runs against a live `GET /screener/scan` endpoint
/// (no silent fallback on error); without it, the offline fixture universe is
/// used.
pub fn run_scan_args(args: &ScanArgs) -> Result<CommandOutcome, CliError> {
    if let Some(server) = args.server.as_deref() {
        if server.trim().is_empty() {
            return Err(CliError::Execution(
                "`--server` must be a non-empty base URL".to_string(),
            ));
        }
        let request = parse_query(args.query.trim()).map_err(CliError::Execution)?;
        let universe: Vec<String> = match &args.universe {
            Some(raw) => serde_json::from_str(raw).map_err(|e| {
                CliError::Execution(format!(
                    "`--universe` must be a JSON array of instrument ids: {e}"
                ))
            })?,
            None => default_live_universe(),
        };
        let rows = live_scan(
            server,
            &request,
            &universe,
            args.tf.as_deref(),
            args.as_of.as_deref(),
        )?;
        Ok(CommandOutcome::ScanResults { rows })
    } else {
        run_scan(args.query.trim())
    }
}
