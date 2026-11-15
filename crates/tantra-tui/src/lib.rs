//! # व्यापार・face tantra-tui
//!
//! Ratatui terminal workstation for the Tantra platform — a multi-tab
//! trading terminal (migration-guide.md §1: *`TuiApp`, `ActiveTab{Watchlist,
//! Screener, Portfolio, Orders, Backtest, Algos, AgentChat}`, keybindings,
//! layout*).
//!
//! Design (see `docs/plans/0001-agent-chat-terminal-from-artha.md`):
//! - **Multi-tab, not chat-only**: `AgentChat` is one of seven tabs; the
//!   others (`Watchlist`, `Screener`, `Portfolio`, `Orders`, `Backtest`,
//!   `Algos`) are driven by `tantra-market` / `tantra-ledger` /
//!   `tantra-runtime-backtest` data (screen inventory per
//!   `docs/plans/0005-fincept-terminal-screen-inventory.md`).
//! - **Sync-render / async-core bridge**: the render loop is synchronous
//!   (Ratatui + crossterm); backend events arrive through the
//!   `tantra-ui-client` event stream and are drained each frame.
//! - **Highlight seam**: markdown/code rendering calls
//!   `tantra_highlight::highlight_code` and maps [`HighlightSpan`]s to
//!   styled Ratatui spans.
//! - **Tests in `tests/unit_tests.rs`** using `ratatui::TestBackend` +
//!   scripted key fixtures (no live terminal).

pub mod agent_chat;
pub mod app;
pub mod keybindings;
pub mod layout;
pub mod live_data;
pub mod terminal;
pub mod ui;

pub use agent_chat::{AgentMessage, AgentRole, ChatTranscript};
pub use app::{ActiveTab, TuiApp};
pub use keybindings::{Action, KeyBinding, handle_key};
pub use layout::LayoutModel;
pub use terminal::{AgentStreamUpdate, run_terminal};
pub use ui::draw;
