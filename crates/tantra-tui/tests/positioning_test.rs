//! U6 — EE+ positioning panel (ui plan 0006 Phase 4).
//!
//! The panel renders the shared `studio_ee_positions` rows from a
//! `PositionIntent` slice: the seeded day prints its two rows, the honest
//! empty state prints the upstream-gap note, and the TUI app seeds the panel
//! from the same fixtures.

use tantra_core::signals::PositionIntent;
use tantra_tui::TuiApp;
use tantra_tui::positioning::{EE_POSITIONING_UNAVAILABLE, positioning_panel_text};
use tantra_ui_core::ee::ee_seed_positions;

/// The seeded intent slice renders both positioning rows with exact strings.
#[test]
fn positioning_panel_renders_seeded_rows() {
    let intents = ee_seed_positions();
    let text = positioning_panel_text(&intents);

    assert!(text.contains("NSE:NIFTY50"), "panel:\n{text}");
    assert!(text.contains("BUY 25"), "panel:\n{text}");
    assert!(text.contains("risk ₹5000"), "panel:\n{text}");
    assert!(text.contains("NSE:RELIANCE"), "panel:\n{text}");
    assert!(text.contains("SELL 100"), "panel:\n{text}");
    assert!(text.contains("risk ₹12000"), "panel:\n{text}");
}

/// An empty intent slice renders the honest upstream-gap note — no fake rows.
#[test]
fn positioning_panel_renders_honest_empty_state() {
    let intents: Vec<PositionIntent> = Vec::new();
    let text = positioning_panel_text(&intents);
    assert!(text.contains(EE_POSITIONING_UNAVAILABLE), "panel:\n{text}");
    assert!(!text.contains("NSE:NIFTY50"), "panel:\n{text}");
}

/// The TUI app seeds its positioning panel from the shared fixtures.
#[test]
fn tui_app_seeds_the_positioning_panel() {
    let app = TuiApp::new();
    assert_eq!(app.ee_positions.len(), 2);
    let text = app.ee_positioning_text();
    assert!(text.contains("BUY 25"), "panel:\n{text}");
    assert!(text.contains("₹12000"), "panel:\n{text}");
}
