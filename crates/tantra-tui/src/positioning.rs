//! EE+ (Elephant Edge) positioning panel (ui plan 0006 Phase 4).
//!
//! Renders the shared [`tantra_ui_core::ee::studio_ee_positions`] rows from a
//! `PositionIntent` slice — the same positioning table the web Studio and the
//! `tantra elephant` CLI report print. Honest empty state: when the slice is
//! empty the panel cites the upstream gap (plan 0006 §2) instead of faking
//! rows.

use tantra_core::signals::PositionIntent;
use tantra_ui_core::ee::studio_ee_positions;

/// The note rendered while no position intents are available.
pub const EE_POSITIONING_UNAVAILABLE: &str = "EE+ positioning unavailable: no PositionIntent rows yet — upstream \
     `GET /level-map/{date}` / `GET /signals/{date}` endpoints pending \
     (ui plan 0006 §2).";

/// Render the positioning panel text from an intent slice: one `•` bullet per
/// `(instrument, "<SIDE> <qty>", "₹<risk>")` row, or the honest empty note.
pub fn positioning_panel_text(intents: &[PositionIntent]) -> String {
    let rows = studio_ee_positions(intents);
    let mut out = String::from("EE+ Positioning (seeded day, plan 0006):\n");
    if rows.is_empty() {
        out.push_str(&format!("  • {EE_POSITIONING_UNAVAILABLE}\n"));
    } else {
        for (instrument, side_qty, risk) in rows {
            out.push_str(&format!("  • {instrument}  {side_qty}  risk {risk}\n"));
        }
    }
    out
}
