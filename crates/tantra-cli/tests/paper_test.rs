//! U5 — `tantra paper` embedded session (plan 0008 D4).
//!
//! The embedded path runs the canonical golden demo tape through the
//! in-process `PaperExchange` and prints deterministic fill lines; two
//! identical runs (same seed) must produce byte-identical output.

use rust_decimal::Decimal;
use tantra_cli::paper::{run_embedded_report, run_server_instructions};

/// The embedded session prints the golden tape fills, sell VWAP, and the
/// zero-fee round-trip economics.
#[test]
fn embedded_session_prints_deterministic_fills() {
    let report = run_embedded_report(Decimal::new(1_000_000, 0), None);
    assert!(
        report.contains("Paper Trading Session"),
        "report:\n{report}"
    );
    assert!(report.contains("PAPER-0001"), "report:\n{report}");
    assert!(report.contains("PAPER-0002"), "report:\n{report}");
    assert!(report.contains("PAPER-0003"), "report:\n{report}");

    // Four fills in price-time-priority order.
    assert!(report.contains("Sell 100 @ 101"), "report:\n{report}");
    assert!(report.contains("Buy 100 @ 101"), "report:\n{report}");
    assert!(report.contains("Sell 50 @ 100"), "report:\n{report}");
    assert!(report.contains("Buy 50 @ 100"), "report:\n{report}");

    // Summary economics: sell VWAP 100.6667, net 0, cash back to initial.
    assert!(report.contains("Sell VWAP: 100.6667"), "report:\n{report}");
    assert!(report.contains("Net position: 0"), "report:\n{report}");
    assert!(report.contains("Cash: 1000000"), "report:\n{report}");
    assert!(report.contains("Equity: 1000000"), "report:\n{report}");
}

/// Two identical runs produce byte-identical output (with and without a
/// seeded latency profile).
#[test]
fn two_identical_runs_produce_identical_output() {
    let a = run_embedded_report(Decimal::new(1_000_000, 0), None);
    let b = run_embedded_report(Decimal::new(1_000_000, 0), None);
    assert_eq!(a, b);

    let c = run_embedded_report(Decimal::new(1_000_000, 0), Some(42));
    let d = run_embedded_report(Decimal::new(1_000_000, 0), Some(42));
    assert_eq!(c, d);

    // The report body (fills + economics) is seed-independent: only the
    // header line records the seed value.
    let body_a = a.split_once('\n').unwrap().1;
    let body_c = c.split_once('\n').unwrap().1;
    assert_eq!(body_a, body_c, "fill/equity content is seed-independent");
}

/// Server mode prints thin-client instructions pointing at the canonical
/// `/api/v2/orders` and `/api/v2/stream` endpoints (design 0006).
#[test]
fn server_mode_prints_instructions() {
    let msg = run_server_instructions(Some("http://127.0.0.1:8000"));
    assert!(msg.contains("/api/v2/orders"), "msg:\n{msg}");
    assert!(msg.contains("/api/v2/orders/{id}/cancel"), "msg:\n{msg}");
    assert!(msg.contains("/api/v2/stream"), "msg:\n{msg}");
    assert!(!msg.contains("/ws/live"), "msg:\n{msg}");
    assert!(msg.contains("Bearer"), "msg:\n{msg}");

    let default_msg = run_server_instructions(None);
    assert!(
        default_msg.contains("http://127.0.0.1:8000"),
        "msg:\n{default_msg}"
    );
}

/// `--mode embedded` with a custom cash amount is honoured end to end.
#[test]
fn embedded_session_honours_custom_cash() {
    let report = run_embedded_report(Decimal::new(2_000_000, 0), None);
    assert!(report.contains("Cash: 2000000"), "report:\n{report}");
    assert!(report.contains("Equity: 2000000"), "report:\n{report}");
}
