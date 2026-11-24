//! Ratatui widget for rendering MCP gateway result blocks (`DisplayBlock`)
//! inside the terminal workstation (plan 0003).

use std::str::FromStr;

use ratatui::buffer::Buffer;
use ratatui::layout::{Constraint, Rect};
use ratatui::style::Style;
use ratatui::text::{Line, Span};
use ratatui::widgets::{Cell, Paragraph, Row, Table, Widget};
use rust_decimal::Decimal;
use rust_decimal::prelude::ToPrimitive;
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
                series,
            } => {
                let text = if series.is_empty() {
                    format!("{title} ({chart_type}) series {series_ref}")
                } else {
                    let spark = unicode_sparkline(series);
                    format!("{title} ({chart_type}, {} pts) {spark}", series.len())
                };
                let paragraph = Paragraph::new(Line::from(text)).block(theme::field_block(false));
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
            DisplayBlock::CodeDiff {
                title,
                lang,
                code,
                hunks,
            } => {
                let mut lines = Vec::new();
                if !lang.is_empty() {
                    lines.push(Line::from(Span::styled(
                        format!("[{lang}]"),
                        theme::bold(theme::ACCENT),
                    )));
                }
                if hunks.is_empty() {
                    lines.extend(code.lines().map(|l| Line::from(l.to_string())));
                } else {
                    for hunk in hunks {
                        lines.push(Line::from(Span::styled(
                            hunk.header.clone(),
                            Style::default().fg(theme::INFO),
                        )));
                        for line in &hunk.lines {
                            let color = if line.starts_with('+') {
                                theme::SUCCESS
                            } else if line.starts_with('-') {
                                theme::ERROR
                            } else {
                                theme::TEXT
                            };
                            lines.push(Line::from(Span::styled(
                                line.clone(),
                                Style::default().fg(color),
                            )));
                        }
                    }
                }
                let paragraph = Paragraph::new(lines)
                    .wrap(ratatui::widgets::Wrap { trim: false })
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

fn unicode_sparkline(series: &[(i64, String)]) -> String {
    const BARS: [char; 8] = ['▁', '▂', '▃', '▄', '▅', '▆', '▇', '█'];
    let vals: Vec<Decimal> = series
        .iter()
        .filter_map(|(_, raw)| Decimal::from_str(raw.trim()).ok())
        .collect();
    if vals.is_empty() {
        return String::new();
    }
    let lo = vals.iter().copied().min().unwrap_or(Decimal::ZERO);
    let hi = vals.iter().copied().max().unwrap_or(Decimal::ZERO);
    let span = hi - lo;
    vals.iter()
        .map(|v| {
            let idx = if span.is_zero() {
                3
            } else {
                ((*v - lo) / span * Decimal::from(7))
                    .round()
                    .to_usize()
                    .unwrap_or(0)
                    .min(7)
            };
            BARS[idx]
        })
        .collect()
}
