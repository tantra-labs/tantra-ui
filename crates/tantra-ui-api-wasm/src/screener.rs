//! Apply a screener filter/sort/limit to rows (pure, no I/O).

use std::collections::BTreeMap;

use rust_decimal::Decimal;
use tantra_core::screener::{
    FilterOp, LogicalOp, PredicateValue, ScreenerFilterGroup, ScreenerFilterItem,
    ScreenerFilterPredicate, ScreenerRow, ScreenerScanRequest, ScreenerScanResponse,
};

/// Errors produced while scanning rows.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScanError {
    /// A filter references a metric no row in the universe provides.
    MissingMetric,
}

/// Apply a [`ScreenerScanRequest`] to a row universe in memory.
///
/// Rows that fail any filter predicate are excluded; remaining rows are
/// sorted (if a sort spec is present) and truncated to `limit`.
pub fn apply_scan(
    universe: &[ScreenerRow],
    request: &ScreenerScanRequest,
) -> Result<ScreenerScanResponse, ScanError> {
    let matches = universe
        .iter()
        .filter(|row| filter_group_matches(&request.filters, row))
        .cloned()
        .collect::<Vec<_>>();
    let total_matched = matches.len() as u32;

    let mut rows = matches;
    if let Some(sort) = &request.sort {
        let key = sort.metric.0.key.clone();
        rows.sort_by(|a, b| {
            let av = a.metrics.get(&key).copied().unwrap_or_default();
            let bv = b.metrics.get(&key).copied().unwrap_or_default();
            if sort.descending {
                bv.cmp(&av)
            } else {
                av.cmp(&bv)
            }
        });
    }
    if let Some(limit) = request.limit {
        rows.truncate(limit as usize);
    }
    Ok(ScreenerScanResponse {
        rows,
        total_matched,
    })
}

/// Whether a recursive filter group matches a row.
pub fn filter_group_matches(group: &ScreenerFilterGroup, row: &ScreenerRow) -> bool {
    if let Some(c) = &group.classification
        && !c.is_empty()
    {
        return false;
    }
    match group.operator {
        LogicalOp::And => group.items.iter().all(|item| item_matches(item, row)),
        LogicalOp::Or => {
            if group.items.is_empty() {
                true
            } else {
                group.items.iter().any(|item| item_matches(item, row))
            }
        }
    }
}

fn item_matches(item: &ScreenerFilterItem, row: &ScreenerRow) -> bool {
    match item {
        ScreenerFilterItem::Predicate(pred) => predicate_matches(pred, row),
        ScreenerFilterItem::Group(nested) => filter_group_matches(nested, row),
    }
}

fn operand_decimal(operand: &PredicateValue, row: &ScreenerRow) -> Option<Decimal> {
    match operand {
        PredicateValue::Decimal(d) => Some(*d),
        PredicateValue::MetricRef(m) => row.metrics.get(&m.0.key).copied(),
        _ => None,
    }
}

fn predicate_matches(pred: &ScreenerFilterPredicate, row: &ScreenerRow) -> bool {
    if pred.relative_to.is_some() || pred.relative_stat.is_some() {
        return false;
    }
    let Some(value) = row.metrics.get(&pred.metric.0.key) else {
        return false;
    };
    match pred.op {
        FilterOp::Eq
        | FilterOp::Ne
        | FilterOp::Gt
        | FilterOp::Gte
        | FilterOp::Lt
        | FilterOp::Lte => {
            let Some(target) = operand_decimal(&pred.value, row) else {
                return false;
            };
            match pred.op {
                FilterOp::Eq => *value == target,
                FilterOp::Ne => *value != target,
                FilterOp::Gt => *value > target,
                FilterOp::Gte => *value >= target,
                FilterOp::Lt => *value < target,
                FilterOp::Lte => *value <= target,
                _ => unreachable!(),
            }
        }
        FilterOp::Between => {
            let Some((lo, hi)) = pred.between_bounds() else {
                return false;
            };
            let (Some(low), Some(high)) = (operand_decimal(lo, row), operand_decimal(hi, row))
            else {
                return false;
            };
            *value >= low && *value <= high
        }
        FilterOp::In => pred.value.as_list().is_some_and(|items| {
            items
                .iter()
                .any(|item| operand_decimal(item, row) == Some(*value))
        }),
        FilterOp::NotIn => pred.value.as_list().is_some_and(|items| {
            !items
                .iter()
                .any(|item| operand_decimal(item, row) == Some(*value))
        }),
        FilterOp::Like | FilterOp::Has => {
            let needle = match &pred.value {
                PredicateValue::String(s) => s.to_lowercase(),
                PredicateValue::Decimal(d) => d.normalize().to_string().to_lowercase(),
                PredicateValue::Bool(b) => b.to_string().to_lowercase(),
                _ => return false,
            };
            value
                .normalize()
                .to_string()
                .to_lowercase()
                .contains(&needle)
        }
        FilterOp::CrossesAbove | FilterOp::CrossesBelow => false,
    }
}

/// Build a `ScreenerRow` helper for tests/consumers.
pub fn row(
    instrument: tantra_core::types::InstrumentId,
    metrics: BTreeMap<String, Decimal>,
) -> ScreenerRow {
    ScreenerRow {
        instrument,
        metrics,
    }
}
