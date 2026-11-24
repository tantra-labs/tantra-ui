//! Tests for screener-scan bindings.

use std::collections::BTreeMap;

use rust_decimal::Decimal;
use tantra_core::screener::{
    FilterOp, MetricKeySpec, MetricPeriod, MetricRef, PredicateValue, ScreenerFilterGroup,
    ScreenerFilterItem, ScreenerFilterPredicate, ScreenerRow, ScreenerScanRequest, SortSpec,
};
use tantra_core::types::{Exchange, InstrumentId};
use tantra_ui_api_wasm::screener::{apply_scan, filter_group_matches, row};

fn dec(s: &str) -> Decimal {
    s.parse().unwrap()
}

fn inst(symbol: &str) -> InstrumentId {
    InstrumentId::try_new(Exchange::Nse, symbol).unwrap()
}

fn metric(name: &str) -> MetricRef {
    MetricRef(MetricKeySpec::new(name).with_period(MetricPeriod::D1))
}

fn row_with(symbol: &str, metrics: &[(&str, &str)]) -> ScreenerRow {
    let mut m = BTreeMap::new();
    for (k, v) in metrics {
        m.insert((*k).to_string(), dec(v));
    }
    row(inst(symbol), m)
}

fn universe() -> Vec<ScreenerRow> {
    vec![
        row_with("RELIANCE", &[("pe", "14.5"), ("roe", "18")]),
        row_with("TCS", &[("pe", "25"), ("roe", "22")]),
        row_with("INFY", &[("pe", "9"), ("roe", "12")]),
    ]
}

#[test]
fn filter_gt_plus_between() {
    let u = universe();
    let request = ScreenerScanRequest {
        universe: None,
        filters: ScreenerFilterGroup::and(vec![
            ScreenerFilterItem::Predicate(ScreenerFilterPredicate::new(
                metric("roe"),
                FilterOp::Gt,
                PredicateValue::Decimal(dec("15")),
            )),
            ScreenerFilterItem::Predicate(ScreenerFilterPredicate::between(
                metric("pe"),
                PredicateValue::Decimal(dec("10")),
                PredicateValue::Decimal(dec("30")),
            )),
        ]),
        sort: None,
        limit: None,
    };
    let resp = apply_scan(&u, &request).unwrap();
    // RELIANCE (roe 18, pe 14.5) matches; TCS (roe 22, pe 25) matches;
    // INFY fails roe > 15.
    assert_eq!(resp.rows.len(), 2);
    assert_eq!(resp.total_matched, 2);
    let symbols = resp
        .rows
        .iter()
        .map(|r| r.instrument.symbol().to_string())
        .collect::<Vec<_>>();
    assert!(symbols.contains(&"RELIANCE".to_string()));
    assert!(symbols.contains(&"TCS".to_string()));
}

#[test]
fn match_any_group() {
    let u = universe();
    let request = ScreenerScanRequest {
        universe: None,
        filters: ScreenerFilterGroup::or(vec![
            ScreenerFilterItem::Predicate(ScreenerFilterPredicate::new(
                metric("pe"),
                FilterOp::Lt,
                PredicateValue::Decimal(dec("10")),
            )),
            ScreenerFilterItem::Predicate(ScreenerFilterPredicate::new(
                metric("roe"),
                FilterOp::Gt,
                PredicateValue::Decimal(dec("21")),
            )),
        ]),
        sort: None,
        limit: None,
    };
    let resp = apply_scan(&u, &request).unwrap();
    // INFY (pe 9) OR TCS (roe 22).
    assert_eq!(resp.rows.len(), 2);
}

#[test]
fn sort_desc_and_limit() {
    let u = universe();
    let request = ScreenerScanRequest {
        universe: None,
        filters: ScreenerFilterGroup::empty(),
        sort: Some(SortSpec {
            metric: metric("pe"),
            descending: true,
        }),
        limit: Some(2),
    };
    let resp = apply_scan(&u, &request).unwrap();
    assert_eq!(resp.rows.len(), 2);
    // Descending by pe: TCS (25), RELIANCE (14.5) are the top two.
    assert_eq!(resp.rows[0].instrument.symbol(), "TCS");
    assert_eq!(resp.rows[1].instrument.symbol(), "RELIANCE");
    // total_matched counts before limit.
    assert_eq!(resp.total_matched, 3);
}

#[test]
fn missing_metric_row_fails_filter() {
    let mut u = universe();
    u.push(row_with("HDFCBANK", &[("pe", "20")])); // no roe metric
    let group = ScreenerFilterGroup::and(vec![ScreenerFilterItem::Predicate(
        ScreenerFilterPredicate::new(
            metric("roe"),
            FilterOp::Gt,
            PredicateValue::Decimal(dec("0")),
        ),
    )]);
    assert!(!filter_group_matches(&group, u.last().unwrap()));
}
