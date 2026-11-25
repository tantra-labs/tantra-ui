//! Tests for the portfolio feature.

use rust_decimal::Decimal;
use tantra_core::AccountId;
use tantra_core::portfolio::{
    PortfolioAccount, PortfolioMode, PortfolioPosition, PortfolioSnapshot, PortfolioSource,
    PortfolioTotals,
};
use tantra_core::types::{Currency, Exchange, InstrumentId, Money, Price, Quantity};
use tantra_ui_app::portfolio::PortfolioFeature;

fn dec(s: &str) -> Decimal {
    s.parse().unwrap()
}

fn inr(amount: &str) -> Money {
    Money::new(dec(amount), Currency::Inr)
}

fn inst(symbol: &str) -> InstrumentId {
    InstrumentId::try_new(Exchange::Nse, symbol).unwrap()
}

fn snapshot() -> PortfolioSnapshot {
    PortfolioSnapshot {
        account: PortfolioAccount {
            account_id: AccountId::new("a1"),
            mode: PortfolioMode::Paper,
            source: PortfolioSource::Simulated,
        },
        positions: vec![PortfolioPosition {
            instrument: inst("RELIANCE"),
            side: tantra_core::instrument::PositionSide::Long,
            quantity: Quantity::from_u64(50),
            avg_price: Price::try_new(dec("2390.00")).unwrap(),
            unrealized_pnl: inr("587.50"),
            realized_pnl: inr("120.00"),
        }],
        holdings: Vec::new(),
        totals: PortfolioTotals {
            cash: inr("450000.00"),
            margin_used: inr("120000.00"),
            margin_available: inr("330000.00"),
            total_pnl: inr("707.50"),
            net_worth: inr("583862.50"),
        },
        greeks: None,
        timestamp: tantra_core::chrono::Utc::now(),
    }
}

#[test]
fn new_feature_is_empty() {
    let f = PortfolioFeature::new();
    assert!(!f.has_data());
    assert!(f.view.positions.is_empty());
}

#[test]
fn ingest_snapshot_projects_view() {
    let mut f = PortfolioFeature::new();
    f.ingest_snapshot(&snapshot());
    assert!(f.has_data());
    assert_eq!(f.view.positions.len(), 1);
    assert_eq!(f.totals().cash.amount, dec("450000.00"));
    assert_eq!(f.view.unrealized_pnl_total(), dec("587.50"));
}

#[test]
fn ingest_empty_snapshot_clears_data() {
    let mut f = PortfolioFeature::new();
    f.ingest_snapshot(&snapshot());
    let empty = PortfolioSnapshot {
        account: snapshot().account,
        positions: Vec::new(),
        holdings: Vec::new(),
        totals: snapshot().totals,
        greeks: None,
        timestamp: tantra_core::chrono::Utc::now(),
    };
    f.ingest_snapshot(&empty);
    assert!(!f.has_data());
}
