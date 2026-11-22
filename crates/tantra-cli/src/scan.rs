//! Headless `tantra scan` wiring (gap `0001` closed; gap doc removed).
//!
//! Parses the CLI query into a canonical [`tantra_core::screener::ScreenerScanRequest`].
//! Two evaluation paths:
//! - **Live** (`tantra scan --server <url>`): `GET /api/v2/screener/scans` on a running
//!   `tantra-runtime-api-rest` server (T10.4, gap `ui 0006`).
//! - **Offline fixture fallback** (no `--server`): the in-memory fixture universe
//!   below, evaluated through [`tantra_ui_api_wasm::screener::apply_scan`].

use std::collections::BTreeMap;
use std::str::FromStr;

use rust_decimal::Decimal;
use tantra_core::screener::{
    FilterOp, MetricKeySpec, MetricPeriod, MetricRef, PredicateValue, ScreenerFilterGroup,
    ScreenerFilterItem, ScreenerFilterPredicate, ScreenerScanRequest,
};
use tantra_core::types::{Exchange, InstrumentId};
use tantra_runtime_api::ScreenerQuery;
use tantra_runtime_client::{BlockingRuntimeClient, ClientError};
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
    MetricRef(MetricKeySpec::new(name).with_period(MetricPeriod::D1))
}

/// Parse a screener query into a [`ScreenerScanRequest`].
///
/// Supported syntax:
/// - `metric<op>value` — single predicate, `op` in `{>, <, >=, <=, ==, !=}`
/// - `metric between <low> <high>` — inclusive range predicate
/// - `pred1 and pred2` — AND-combined group (`LogicalOp::And`)
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

    let mut items = Vec::with_capacity(parts.len());
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
            items.push(ScreenerFilterItem::Predicate(
                ScreenerFilterPredicate::between(
                    metric_ref(metric),
                    PredicateValue::Decimal(value),
                    PredicateValue::Decimal(value_upper),
                ),
            ));
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
        } else if let Some((m, v)) = part.split_once('>') {
            (m.trim(), v.trim(), FilterOp::Gt)
        } else if let Some((m, v)) = part.split_once('<') {
            (m.trim(), v.trim(), FilterOp::Lt)
        } else {
            return Err(format!("no comparison operator in predicate '{part}'"));
        };
        let value: Decimal = value_str
            .parse()
            .map_err(|_| format!("invalid value '{value_str}' for '{metric}'"))?;
        items.push(ScreenerFilterItem::Predicate(ScreenerFilterPredicate::new(
            metric_ref(metric),
            op,
            PredicateValue::Decimal(value),
        )));
    }

    Ok(ScreenerScanRequest {
        universe: None, // full fixture universe
        filters: ScreenerFilterGroup::and(items),
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

/// Serialize a [`Decimal`] as a JSON number when exact, or string otherwise.
fn decimal_json(value: &Decimal) -> serde_json::Value {
    match serde_json::Number::from_str(&value.to_string()) {
        Ok(num) => serde_json::Value::Number(num),
        Err(_) => serde_json::Value::String(value.to_string()),
    }
}

fn predicate_value_json(v: &PredicateValue) -> serde_json::Value {
    match v {
        PredicateValue::Decimal(d) => decimal_json(d),
        PredicateValue::Bool(b) => serde_json::Value::Bool(*b),
        PredicateValue::String(s) => serde_json::Value::String(s.clone()),
        PredicateValue::MetricRef(m) => serde_json::to_value(m).unwrap_or(serde_json::Value::Null),
        PredicateValue::List(items) => {
            serde_json::Value::Array(items.iter().map(predicate_value_json).collect())
        }
    }
}

fn filter_group_to_server_json(group: &ScreenerFilterGroup) -> serde_json::Value {
    let items: Vec<serde_json::Value> = group
        .items
        .iter()
        .map(|item| match item {
            ScreenerFilterItem::Group(nested) => filter_group_to_server_json(nested),
            ScreenerFilterItem::Predicate(p) => {
                let value = if let Some((lo, hi)) = p.between_bounds() {
                    serde_json::json!([predicate_value_json(lo), predicate_value_json(hi)])
                } else {
                    predicate_value_json(&p.value)
                };
                serde_json::json!({
                    "key": p.metric.0.key,
                    "op": p.op.as_str(),
                    "value": value,
                })
            }
        })
        .collect();
    serde_json::json!({
        "operator": group.operator.as_str(),
        "items": items,
    })
}

/// Map a canonical [`ScreenerScanRequest`] filter group into the server's wire
/// shape: `{"operator": "AND"|"OR", "items": [{"key", "op", "value", ...}]}`.
pub fn to_server_filters(request: &ScreenerScanRequest) -> serde_json::Value {
    filter_group_to_server_json(&request.filters)
}

/// Run a screener scan against a live `GET /api/v2/screener/scans` endpoint and return
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
    let client = BlockingRuntimeClient::new(base, None)
        .map_err(|e| CliError::Execution(format!("invalid screener server `{base}`: {e}")))?;
    let data = client.screener_scans(&query).map_err(|e| match e {
        ClientError::Unreachable(m) => {
            CliError::Execution(format!("cannot reach screener server at {base}: {m}"))
        }
        ClientError::Http {
            status,
            code,
            message,
            reason,
        } => {
            let code = code.map_or_else(
                || "error".to_string(),
                |c| {
                    serde_json::to_value(c)
                        .ok()
                        .and_then(|v| v.as_str().map(String::from))
                        .unwrap_or_else(|| "error".to_string())
                },
            );
            let reason = reason.unwrap_or(message);
            CliError::Execution(format!(
                "screener server at {base} returned HTTP {status} ({code}): {reason}"
            ))
        }
        ClientError::BadBody(m) => {
            CliError::Execution(format!("invalid screener response from {base}: {m}"))
        }
        ClientError::InvalidBase(m) => {
            CliError::Execution(format!("invalid screener server `{m}`"))
        }
    })?;
    let rows = data["rows"].as_array().map(|r| r.len()).ok_or_else(|| {
        CliError::Execution(format!(
            "malformed screener response from {base}: missing data.rows"
        ))
    })?;
    Ok(rows)
}

/// Convenience wrapper over [`ScanArgs`].
///
/// With `--server` the scan runs against a live `GET /api/v2/screener/scans` endpoint
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
