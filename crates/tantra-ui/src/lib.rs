//! Facade of the `tantra-ui` bounded context (Face).
//!
//! Re-exports the member crates under their short names so consumers import the
//! whole workstation context from a single crate (facade-crate convention, root
//! plan 0003 Phase 4 / gap 0007).
//!
//! Note: this facade pulls in the native `tantra-tui`/`tantra-cli` crates and is
//! therefore **not** WASM-targetable; the WASM members keep compiling to
//! `wasm32-unknown-unknown` individually.

pub use tantra_cli as cli;
pub use tantra_highlight as highlight;
pub use tantra_tui as tui;
pub use tantra_ui_api_wasm as api_wasm;
pub use tantra_ui_app as app;
pub use tantra_ui_assets as assets;
pub use tantra_ui_charts as charts;
pub use tantra_ui_client as client;
pub use tantra_ui_core as core;
