//! Layout model: how the terminal splits into the main area and footer.

use ratatui::layout::{Constraint, Direction, Layout, Rect};

/// The vertical layout regions of the terminal.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LayoutModel {
    /// Main content area.
    pub main: Rect,
    /// Footer row (followed by one blank row).
    pub footer: Rect,
}

impl LayoutModel {
    /// Compute the layout for the whole terminal area.
    pub fn new(area: Rect) -> Self {
        let rows = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Min(0),
                Constraint::Length(1),
                Constraint::Length(1),
            ])
            .split(area);
        Self {
            main: rows[0],
            footer: rows[1],
        }
    }

    /// The main area split into a left panel and right panel.
    pub fn content_columns(&self) -> [Rect; 2] {
        let cols = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Percentage(40), Constraint::Percentage(60)])
            .split(self.main);
        [cols[0], cols[1]]
    }
}
