//! Tests for `tantra elephant` (ui plan 0006 Phase 4): the seeded EE+
//! Strategy Studio report prints the S1/S2/R1/R2 level map with scores, the
//! signals with their SL/3R/5R stop lines, and the positioning table — all
//! deterministic and `Decimal`-exact.

use clap::Parser;
use tantra_cli::args::Cli;
use tantra_cli::elephant::run_elephant_report;
use tantra_cli::{CliArgs, Command};

/// The report lists every elephant band with its band prices and score.
#[test]
fn elephant_report_lists_s1_s2_r1_r2_with_scores() {
    let report = run_elephant_report();
    assert!(
        report.contains("S1 (support) 23930..23970 @ 23950 score 3"),
        "report:\n{report}"
    );
    assert!(
        report.contains("S2 (support) 23820..23880 @ 23850 score 2"),
        "report:\n{report}"
    );
    assert!(
        report.contains("R1 (resistance) 24130..24170 @ 24150 score 4"),
        "report:\n{report}"
    );
    assert!(
        report.contains("R2 (resistance) 24220..24280 @ 24250 score 1"),
        "report:\n{report}"
    );
}

/// The report prints the regime chip and the signal SL/3R/5R stop prices.
#[test]
fn elephant_report_prints_regime_and_signal_stop_lines() {
    let report = run_elephant_report();
    assert!(report.contains("Regime: long_gamma"), "report:\n{report}");

    assert!(
        report.contains(
            "BUY RETEST @ 23950 (conf 75%): SL 1R 23710.50 | T 3R 24668.50 | T 5R 25147.50"
        ),
        "report:\n{report}"
    );
    assert!(
        report.contains(
            "SELL BREAKOUT @ 24150 (conf 82%): SL 1R 24391.50 | T 3R 23425.50 | T 5R 22942.50"
        ),
        "report:\n{report}"
    );
}

/// The report prints the positioning table with exact side/qty/risk strings.
#[test]
fn elephant_report_prints_positioning_table() {
    let report = run_elephant_report();
    assert!(report.contains("NSE:NIFTY50"), "report:\n{report}");
    assert!(report.contains("BUY 25"), "report:\n{report}");
    assert!(report.contains("risk ₹5000"), "report:\n{report}");
    assert!(report.contains("NSE:RELIANCE"), "report:\n{report}");
    assert!(report.contains("SELL 100"), "report:\n{report}");
    assert!(report.contains("risk ₹12000"), "report:\n{report}");
}

/// Two identical runs produce byte-identical output.
#[test]
fn elephant_report_is_deterministic() {
    assert_eq!(run_elephant_report(), run_elephant_report());
}

/// `tantra elephant` parses into the domain `Command::Elephant`.
#[test]
fn elephant_subcommand_parses() {
    let parsed = Cli::try_parse_from(["tantra", "elephant"]).expect("parse elephant subcommand");
    let args: CliArgs = parsed.into();
    assert_eq!(args.command, Command::Elephant);
}
