//! # व्यापार・face tantra-ui-app
//!
//! The Tantra workstation application crate: router, screener, strategy
//! studio, simulator, and system features.
//!
//! Contracts (migration-guide.md §1 / `tantra-ui/.agents/AGENTS.md`):
//! - **WASM-portable**: this crate must compile cleanly to
//!   `wasm32-unknown-unknown` (`cargo check -p tantra-ui-app --target
//!   wasm32-unknown-unknown`). No `tokio`, no OS threads, no blocking I/O.
//! - **Zero-floats**: every financial value is `rust_decimal::Decimal`.
//! - **Feature/state separation**: each feature module owns its *reactive
//!   state* and transitions; presentational primitives come from
//!   `tantra-ui-core`. The Leptos 0.8 CSR view layer (`web`) renders the
//!   eight `AppRoute` views from those feature-state models.
//! - **Tests in `tests/`** (parallel test directory rule).

pub mod algos;
pub mod backtest;
pub mod paper;
pub mod portfolio;
pub mod router;
pub mod screener;
pub mod simulator;
pub mod studio;
pub mod studio_ee;
pub mod system;
pub mod watchlist;
pub mod web;

pub use algos::AlgosFeature;
pub use backtest::BacktestFeature;
pub use paper::{PaperFeature, PaperMode, PaperOrderKind, PaperSide};
pub use portfolio::PortfolioFeature;
pub use router::{AppRoute, Router, RouterError};
pub use screener::{ScreenerFeature, ScreenerFeatureError};
pub use simulator::{SimulatorFeature, SimulatorPhase};
pub use studio::{Connection, NodeId, StudioCanvas, StudioNode, StudioNodeKind};
pub use studio_ee::StudioEeFeature;
pub use system::{SubsystemState, SystemFeature};
pub use watchlist::WatchlistFeature;
