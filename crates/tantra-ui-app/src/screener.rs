//! Screener feature: reactive scan state over the screener view model.

use serde::{Deserialize, Serialize};
use tantra_core::screener::{
    FilterOp, MetricKeySpec, MetricPeriod, MetricRef, PredicateValue, ScreenerFilterGroup,
    ScreenerFilterItem, ScreenerFilterPredicate, ScreenerScanRequest, ScreenerScanResponse,
};
use tantra_ui_api_wasm::screener::apply_scan;
use tantra_ui_core::screener::{
    ScreenerColumn, ScreenerFilterChip, ScreenerSort, ScreenerViewModel,
};

/// Errors produced by the screener feature.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ScreenerFeatureError {
    /// A filter predicate could not be built (empty metric name).
    InvalidMetric,
}

/// The screener feature state: query drafting + result view model.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ScreenerFeature {
    /// The scan request being drafted.
    pub request: ScreenerScanRequest,
    /// The rendered result table.
    pub view: ScreenerViewModel,
    /// Whether a scan is in flight (host async renders a spinner).
    pub scanning: bool,
}

impl ScreenerFeature {
    /// Create a new screener feature.
    pub fn new() -> Self {
        Self {
            request: ScreenerScanRequest {
                universe: None,
                filters: ScreenerFilterGroup::empty(),
                sort: None,
                limit: Some(50),
            },
            view: ScreenerViewModel::empty(),
            scanning: false,
        }
    }

    /// Add a filter predicate to the query.
    ///
    /// # Errors
    /// [`ScreenerFeatureError::InvalidMetric`] when the metric name is empty.
    pub fn add_filter(
        &mut self,
        metric: impl Into<String>,
        op: FilterOp,
        value: rust_decimal::Decimal,
        value_upper: Option<rust_decimal::Decimal>,
    ) -> Result<(), ScreenerFeatureError> {
        let name = metric.into();
        if name.trim().is_empty() {
            return Err(ScreenerFeatureError::InvalidMetric);
        }
        let metric_ref = MetricRef(MetricKeySpec::new(name).with_period(MetricPeriod::D1));
        let predicate = if op == FilterOp::Between {
            let upper = value_upper.unwrap_or(value);
            ScreenerFilterPredicate::between(
                metric_ref,
                PredicateValue::Decimal(value),
                PredicateValue::Decimal(upper),
            )
        } else {
            ScreenerFilterPredicate::new(metric_ref, op, PredicateValue::Decimal(value))
        };
        self.request
            .filters
            .items
            .push(ScreenerFilterItem::Predicate(predicate));
        Ok(())
    }

    /// Clear all filter predicates.
    pub fn clear_filters(&mut self) {
        self.request.filters.items.clear();
        self.view.filters.clear();
    }

    /// Run the drafted scan against a row universe, producing the view model.
    ///
    /// This is a pure, synchronous computation over the in-memory universe —
    /// it is what the WASM host calls once rows are available.
    pub fn run_scan(&mut self, universe: &[tantra_core::screener::ScreenerRow]) {
        self.scanning = true;
        let response = apply_scan(universe, &self.request);
        self.scanning = false;
        if let Ok(resp) = response {
            self.apply_response(resp);
        }
    }

    fn apply_response(&mut self, response: ScreenerScanResponse) {
        let rows = response
            .rows
            .into_iter()
            .map(|r| tantra_ui_core::screener::ScreenerRowModel {
                instrument: r.instrument,
                metrics: r.metrics,
            })
            .collect();
        self.view.rows = rows;
        self.view.total_matched = response.total_matched;
        self.view.columns = Self::default_columns(&self.request);
        self.view.filters = self
            .request
            .filters
            .predicates()
            .into_iter()
            .filter_map(|p| {
                if let Some((lo, hi)) = p.between_bounds() {
                    let lo_dec = lo.as_decimal()?;
                    let hi_dec = hi.as_decimal()?;
                    Some(
                        ScreenerFilterChip::new(p.metric.0.key.clone(), p.op, lo_dec)
                            .with_upper(hi_dec),
                    )
                } else {
                    p.value
                        .as_decimal()
                        .map(|d| ScreenerFilterChip::new(p.metric.0.key.clone(), p.op, d))
                }
            })
            .collect();
        self.view.sort = self.request.sort.as_ref().map(|s| ScreenerSort {
            metric: s.metric.clone(),
            order: if s.descending {
                tantra_ui_core::screener::ScreenerSortOrder::Desc
            } else {
                tantra_ui_core::screener::ScreenerSortOrder::Asc
            },
        });
    }

    fn default_columns(request: &ScreenerScanRequest) -> Vec<ScreenerColumn> {
        let mut columns = vec![ScreenerColumn::new("instrument", "Instrument")];
        for pred in request.filters.predicates() {
            let key = pred.metric.0.key.clone();
            columns.push(ScreenerColumn::new(key.clone(), key).right_aligned());
        }
        columns
    }
}

impl Default for ScreenerFeature {
    fn default() -> Self {
        Self::new()
    }
}
