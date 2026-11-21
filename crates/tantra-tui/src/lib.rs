//! # व्यापार・face tantra-tui
//!
//! Ratatui terminal workstation for the Tantra platform — a multi-tab
//! trading terminal (migration-guide.md §1: *`TuiApp`, `ActiveTab{Watchlist,
//! Screener, Portfolio, Orders, Backtest, Algos, AgentChat, StrategyLab}`,
//! keybindings, layout*).
//!
//! Design (see `docs/plans/0001-agent-chat-terminal-from-artha.md` & `0009`):
//! - **Multi-tab, not chat-only**: `AgentChat` is one of eight tabs;
//!   `StrategyLab` renders the unified 6-layer `OverlayScene` on a
//!   `ratatui::widgets::canvas::Canvas` + `Sparkline` sub-pane.
//! - **Sync-render / async-core bridge**: the render loop is synchronous
//!   (Ratatui + crossterm); backend events arrive through the
//!   `tantra-ui-client` event stream and are drained each frame.
//! - **Highlight seam**: markdown/code rendering calls
//!   `tantra_highlight::highlight_code` and maps [`HighlightSpan`]s to
//!   styled Ratatui spans.
//! - **Tests in `tests/unit_tests.rs` & `tests/strategy_lab_test.rs`** using
//!   `ratatui::TestBackend` + scripted key fixtures (no live terminal).

pub mod agent_chat;
pub mod analysis;
pub mod analysis_view;
pub mod app;
pub mod keybindings;
pub mod layout;
pub mod live_data;
pub mod positioning;
pub mod result_block;
pub mod strategy_lab;
pub mod terminal;
pub mod theme;
pub mod ui;

pub use agent_chat::{
    AgentMessage, AgentRole, AgentStreamUpdate, ChatTranscript, ToolRow, ToolStatus,
};
pub use analysis::{AnalysisPage, ToolCard};
pub use app::{ActiveTab, StrategyLabDetailView, StrategyLabState, TuiApp};
pub use keybindings::{Action, KeyBinding, handle_key};
pub use layout::LayoutModel;
pub use positioning::{EE_POSITIONING_UNAVAILABLE, positioning_panel_text};
pub use terminal::run_terminal;
pub use ui::draw;
