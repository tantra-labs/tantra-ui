//! Ratatui widget for rendering MCP gateway result blocks (`DisplayBlock`)
//! inside the terminal workstation (plan 0003).

use ratatui::buffer::Buffer;
use ratatui::layout::{Constraint, Rect};
use ratatui::style::Style;
use ratatui::text::{Line, Span};
use ratatui::widgets::{Cell, Paragraph, Row, Table, Widget};
use tantra_agent_core::result::DisplayBlock;

use crate::theme;

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
                        theme::bold(theme::ACCENT),
                    )))
                })))
                .block(theme::panel_block(title.as_str(), theme::ACCENT));
                Widget::render(table, area, buf);
            }
            DisplayBlock::MetricGrid { items } => {
                let lines = items
                    .iter()
                    .map(|item| Line::from(format!("{}: {}", item.label, item.value)))
                    .collect::<Vec<_>>();
                let paragraph = Paragraph::new(lines).block(theme::field_block(false));
                Widget::render(paragraph, area, buf);
            }
            DisplayBlock::Alert { level, message } => {
                let upper = level.to_uppercase();
                let color = match upper.as_str() {
                    "INFO" => theme::INFO,
                    "WARN" => theme::WARNING,
                    "CRITICAL" => theme::ERROR,
                    _ => theme::TEXT,
                };
                let line = Line::from(vec![
                    Span::styled(format!("[{upper}]"), Style::default().fg(color)),
                    Span::raw(format!(" {message}")),
                ]);
                let paragraph = Paragraph::new(line).block(theme::field_block(false));
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
                let paragraph = Paragraph::new(line).block(theme::field_block(false));
                Widget::render(paragraph, area, buf);
            }
            DisplayBlock::OverlayScene {
                title,
                symbol,
                timeframe,
                scene,
                ..
            } => {
                // Agent-packaged chart scene (`chart_scene_annotate`): summarise its layers
                // here; the full scene renders in the Strategy Lab chart.
                let count = |k: &str| scene.get(k).and_then(|v| v.as_array()).map_or(0, Vec::len);
                let line = Line::from(format!(
                    "{symbol} {timeframe}: {} signals, {} bands, {} stops, {} indicators, {} hints, {} hedges",
                    count("signals"),
                    count("bands"),
                    count("stops"),
                    count("indicators"),
                    count("hints"),
                    count("hedges"),
                ));
                let paragraph =
                    Paragraph::new(line).block(theme::panel_block(title.as_str(), theme::ACCENT));
                Widget::render(paragraph, area, buf);
            }
            DisplayBlock::View {
                kind,
                title,
                summary,
                ..
            } => {
                // Minimal render; the real dynamic pane is built separately (agent plan 0020).
                let mut lines = vec![Line::from(format!("{kind:?}"))];
                lines.extend(summary.lines().map(|l| Line::from(l.to_string())));
                let paragraph = Paragraph::new(lines)
                    .wrap(ratatui::widgets::Wrap { trim: true })
                    .block(theme::panel_block(title.as_str(), theme::ACCENT));
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
