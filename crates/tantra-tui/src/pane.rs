//! Pure state for the dynamic right-hand pane (no I/O, no rendering).
//!
//! The Agent view keeps a small stack of panes (tabs); each pane shows what
//! fits the user's last action (help, command output, analysis, an
//! agent-composed `present_view`, ...).

use tantra_agent_core::{MetricItem, ViewKind};

use crate::analysis::ToolCard;

/// Maximum number of simultaneously open panes.
pub const PANE_CAP: usize = 8;

/// An agent-composed view (from the `present_view` tool).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AgentView {
    /// View kind.
    pub kind: ViewKind,
    /// View title.
    pub title: String,
    /// Symbols the view is about.
    pub symbols: Vec<String>,
    /// Short markdown-ish summary.
    pub summary: String,
    /// Headline metrics.
    pub highlights: Vec<MetricItem>,
    /// Tool names whose cards to include (empty = all of the turn's cards).
    pub include_tools: Vec<String>,
    /// Turn in which the view was presented.
    pub turn: u64,
}

/// What a pane shows.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PaneBody {
    /// Slash-command help, optionally for one command.
    Help { topic: Option<String> },
    /// The existing analysis page.
    Analysis,
    /// Agent-composed view.
    View(AgentView),
    /// Captured output of an informational slash command.
    Info { lines: Vec<String> },
    /// Strategy Lab renderer.
    StrategyLab,
    /// Backtest renderer.
    Backtest,
    /// Portfolio renderer.
    Portfolio,
}

/// One pane (tab).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Pane {
    /// Tab title.
    pub title: String,
    /// Body.
    pub body: PaneBody,
    /// Scroll offset (lines for text bodies, cards for views).
    pub scroll: u16,
}

impl Pane {
    /// Build a pane with zero scroll.
    pub fn new(title: impl Into<String>, body: PaneBody) -> Self {
        Self {
            title: title.into(),
            body,
            scroll: 0,
        }
    }

    fn same_identity(&self, other: &Pane) -> bool {
        match (&self.body, &other.body) {
            (PaneBody::Help { .. }, PaneBody::Help { .. })
            | (PaneBody::Analysis, PaneBody::Analysis)
            | (PaneBody::StrategyLab, PaneBody::StrategyLab)
            | (PaneBody::Backtest, PaneBody::Backtest)
            | (PaneBody::Portfolio, PaneBody::Portfolio) => true,
            (PaneBody::View(_), PaneBody::View(_))
            | (PaneBody::Info { .. }, PaneBody::Info { .. }) => self.title == other.title,
            _ => false,
        }
    }
}

/// Ordered stack of panes with one active.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct PaneStack {
    panes: Vec<Pane>,
    active: Option<usize>,
}

impl PaneStack {
    /// Open (or replace) a pane and activate it.
    pub fn open(&mut self, pane: Pane) {
        if let Some(i) = self.panes.iter().position(|p| p.same_identity(&pane)) {
            self.panes[i] = pane;
            self.active = Some(i);
            return;
        }
        self.panes.push(pane);
        let mut idx = self.panes.len() - 1;
        if self.panes.len() > PANE_CAP {
            // Evict the oldest non-active pane (the new one is active).
            self.panes.remove(0);
            idx -= 1;
        }
        self.active = Some(idx);
    }

    /// The active pane.
    pub fn active(&self) -> Option<&Pane> {
        self.active.and_then(|i| self.panes.get(i))
    }

    /// The active pane, mutably.
    pub fn active_mut(&mut self) -> Option<&mut Pane> {
        match self.active {
            Some(i) => self.panes.get_mut(i),
            None => None,
        }
    }

    /// Close the active pane, activating a neighbour.
    pub fn close_active(&mut self) {
        let Some(i) = self.active else { return };
        if i < self.panes.len() {
            self.panes.remove(i);
        }
        self.active = if self.panes.is_empty() {
            None
        } else {
            Some(i.min(self.panes.len() - 1))
        };
    }

    /// Activate the next pane (wrapping).
    pub fn next(&mut self) {
        if let Some(i) = self.active {
            self.active = Some((i + 1) % self.panes.len().max(1));
        }
    }

    /// Activate the previous pane (wrapping).
    pub fn prev(&mut self) {
        if let Some(i) = self.active {
            let n = self.panes.len().max(1);
            self.active = Some((i + n - 1) % n);
        }
    }

    /// Tab titles with an active flag.
    pub fn titles(&self) -> Vec<(String, bool)> {
        self.panes
            .iter()
            .enumerate()
            .map(|(i, p)| (p.title.clone(), self.active == Some(i)))
            .collect()
    }

    /// True when no pane is open.
    pub fn is_empty(&self) -> bool {
        self.panes.is_empty()
    }

    /// Close all panes.
    pub fn clear(&mut self) {
        self.panes.clear();
        self.active = None;
    }
}

/// Human label for a view kind.
pub fn view_kind_label(kind: ViewKind) -> &'static str {
    match kind {
        ViewKind::Analysis => "Analysis",
        ViewKind::Options => "Options",
        ViewKind::Backtest => "Backtest",
        ViewKind::StrategyLab => "Strategy Lab",
        ViewKind::Portfolio => "Portfolio",
        ViewKind::Compare => "Compare",
        ViewKind::Report => "Report",
    }
}

/// Cards belonging to a view: same turn, filtered by `include_tools` when set.
pub fn view_cards<'a>(view: &AgentView, cards: &'a [ToolCard]) -> Vec<&'a ToolCard> {
    cards
        .iter()
        .filter(|c| c.turn == view.turn)
        .filter(|c| view.include_tools.is_empty() || view.include_tools.contains(&c.tool))
        .collect()
}
