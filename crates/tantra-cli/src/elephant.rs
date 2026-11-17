//! `tantra elephant` — the EE+ (Elephant Edge) Strategy Studio report
//! (ui plan 0006 Phase 4).
//!
//! Prints the same seeded day the web Studio and the TUI positioning panel
//! render, straight from the shared [`tantra_ui_core::ee`] helpers: the
//! S1/S2/R1/R2 level map with confluence scores, the signals with their
//! SL/3R/5R stop lines, and the positioning table. Zero floats, deterministic
//! output.

use rust_decimal::Decimal;
use tantra_ui_core::ee::{
    StudioViewport, ee_level_report_rows, ee_seed_last_price, ee_seed_level_map, ee_seed_positions,
    ee_seed_signals, ee_signal_report_rows, studio_ee_positions, studio_ee_regime,
};

/// A viewport covering every band/stop/target price on the seeded day.
fn report_viewport() -> StudioViewport {
    StudioViewport::new(Decimal::from(22_500), Decimal::from(25_500), 360, 240)
}

/// Build the full `tantra elephant` report text.
pub fn run_elephant_report() -> String {
    let map = ee_seed_level_map();
    let signals = ee_seed_signals();
    let positions = ee_seed_positions();
    let last_price = ee_seed_last_price();
    let viewport = report_viewport();

    let (chip, desc) = studio_ee_regime(&map, last_price);
    let mut out = String::from(
        "Elephant Edge (EE+) Strategy Studio report (seeded day — ui plan 0006 Phase 4)\n",
    );
    out.push_str(&format!("Regime: {chip} ({desc}) | spot {last_price}\n"));

    out.push_str("\nLevel map:\n");
    for row in ee_level_report_rows(&map, &viewport) {
        out.push_str(&format!("  {row}\n"));
    }

    out.push_str("\nSignals:\n");
    for row in ee_signal_report_rows(&signals, &viewport) {
        out.push_str(&format!("  {row}\n"));
    }

    out.push_str("\nPositioning:\n");
    for (instrument, side_qty, risk) in studio_ee_positions(&positions) {
        out.push_str(&format!("  {instrument:<15} {side_qty:<10} risk {risk}\n"));
    }
    out
}
