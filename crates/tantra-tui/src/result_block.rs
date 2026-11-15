//! Ratatui widget for rendering MCP gateway result blocks (`DisplayBlock`)
//! inside the terminal workstation (plan 0003).

use ratatui::buffer::Buffer;
use ratatui::layout::{Constraint, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Cell, Paragraph, Row, Table, Widget};
use tantra_agent_core::result::DisplayBlock;

/// Renders a single gateway [`DisplayBlock`] into a [`Rect`].
pub struct ResultBlockWidget<'a> {
    /// The block to render.
    pub block: &'a DisplayBlock,
}

impl<'a> Widget for ResultBlockWidget<'a> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        match self.block {
            DisplayBlock::Table {
                title,
                columns,
                rows,
            } => {
                let width = content_width(columns, rows);
                let table = Table::new(
                    rows.iter().map(|row| {
                        Row::new(row.iter().map(|cell| Cell::new(Line::from(cell.as_str()))))
                    }),
                    columns.iter().map(|_| Constraint::Length(width)),
                )
                .header(Row::new(columns.iter().map(|column| {
                    Cell::new(Line::from(Span::styled(
                        column.as_str(),
                        Style::default().add_modifier(Modifier::BOLD),
                    )))
                })))
                .block(Block::default().borders(Borders::ALL).title(title.as_str()));
                Widget::render(table, area, buf);
            }
            DisplayBlock::MetricGrid { items } => {
                let lines = items
                    .iter()
                    .map(|item| Line::from(format!("{}: {}", item.label, item.value)))
                    .collect::<Vec<_>>();
                let paragraph = Paragraph::new(lines).block(Block::default().borders(Borders::ALL));
                Widget::render(paragraph, area, buf);
            }
            DisplayBlock::Alert { level, message } => {
                let upper = level.to_uppercase();
                let color = match upper.as_str() {
                    "INFO" => Color::Green,
                    "WARN" => Color::Yellow,
                    "CRITICAL" => Color::Red,
                    _ => Color::White,
                };
                let line = Line::from(vec![
                    Span::styled(format!("[{upper}]"), Style::default().fg(color)),
                    Span::raw(format!(" {message}")),
                ]);
                let paragraph = Paragraph::new(line).block(Block::default().borders(Borders::ALL));
                Widget::render(paragraph, area, buf);
            }
            DisplayBlock::Chart {
                title,
                chart_type,
                series_ref,
            } => {
                // Chart data lives in the gateway-side data store; render the
                // reference honestly (plan 0003 Phase 1).
                let line = Line::from(format!("{title} ({chart_type}) series {series_ref}"));
                let paragraph = Paragraph::new(line).block(Block::default().borders(Borders::ALL));
                Widget::render(paragraph, area, buf);
            }
        }
    }
}

/// Equal column width for tables: the widest header/cell, clamped to a sane
/// 4..=20 and padded by two for the cell gutters.
fn content_width(columns: &[String], rows: &[Vec<String>]) -> u16 {
    let widest = columns
        .iter()
        .map(|c| c.chars().count())
        .chain(rows.iter().flatten().map(|r| r.chars().count()))
        .max()
        .unwrap_or(4)
        .clamp(4, 20) as u16;
    widest + 2
}
