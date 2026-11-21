//! Right-hand "Analysis" page of the split-screen Agent view: ticker strip,
//! price chart, spread/position, backtest summary and agent tool-result cards.

use ratatui::Frame;
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::Style;
use ratatui::symbols::Marker;
use ratatui::text::{Line, Span};
use ratatui::widgets::{Axis, Chart, Dataset, GraphType, Paragraph, Sparkline, Wrap};
use rust_decimal::Decimal;
use rust_decimal::prelude::ToPrimitive;
use tantra_agent_core::DisplayBlock;

use crate::analysis::base_symbol;
use crate::app::TuiApp;
use crate::result_block::ResultBlockWidget;
use crate::theme;

/// Convert an exact `Decimal` solely at the chart coordinate boundary.
fn coord_f64(value: Decimal) -> f64 {
    // render-only coordinate, not financial math
    value.to_f64().unwrap_or(0.0)
}

fn muted(s: impl Into<String>) -> Line<'static> {
    Line::from(Span::styled(s.into(), theme::fg(theme::TEXT_MUTED)))
}

fn signed_color(d: Decimal) -> ratatui::style::Color {
    if d >= Decimal::ZERO {
        theme::SUCCESS
    } else {
        theme::ERROR
    }
}

fn card_block(title: &str) -> ratatui::widgets::Block<'static> {
    // Trading-R1 docked inset: content sits one column in from the border.
    theme::panel_block(title, theme::ACCENT).padding(ratatui::widgets::Padding::horizontal(1))
}

/// Draw the full-height analysis pane into `area`.
pub fn draw_analysis(frame: &mut Frame, area: Rect, app: &TuiApp) {
    let title = match app.analysis.primary() {
        Some(p) => format!("Analysis · {}", base_symbol(p)),
        None => "Analysis".to_string(),
    };
    let block = theme::panel_block(&title, theme::PRIMARY);
    let inner = block.inner(area);
    frame.render_widget(block, area);
    let inner = Rect::new(
        inner.x.saturating_add(1),
        inner.y.saturating_add(1),
        inner.width.saturating_sub(2),
        inner.height.saturating_sub(1),
    );
    if inner.width == 0 || inner.height == 0 {
        return;
    }
    if app.analysis.is_empty() {
        let lines = vec![
            muted("analysis appears here as the agent works:"),
            muted("tickers you mention, prices, spreads,"),
            muted("positions, backtests and tool results."),
        ];
        frame.render_widget(Paragraph::new(lines).wrap(Wrap { trim: false }), inner);
        return;
    }

    let mut y = inner.y;
    let bottom = inner.y + inner.height;

    if !app.analysis.focus.is_empty() {
        frame.render_widget(
            Paragraph::new(ticker_strip(app)),
            Rect::new(inner.x, y, inner.width, 1),
        );
        y = (y + 2).min(bottom);
    }

    #[derive(Clone, Copy)]
    enum Item {
        Price,
        SpreadPos,
        Backtest,
        Tool(usize),
    }
    let mut items: Vec<Item> = Vec::new();
    if app.analysis.primary().is_some() {
        items.push(Item::Price);
        items.push(Item::SpreadPos);
    }
    if app.analysis.show_backtest && app.backtest_report.is_some() {
        items.push(Item::Backtest);
    }
    items.extend((0..app.analysis.cards.len()).map(Item::Tool));

    let stacked = inner.width < 60;
    let skip = app.analysis.scroll as usize;
    let visible: Vec<Item> = items.into_iter().skip(skip).collect();
    let total = visible.len();
    let mut drawn = 0usize;
    for (i, item) in visible.iter().enumerate() {
        let h = match item {
            Item::Price => 10,
            Item::SpreadPos => {
                if stacked {
                    10
                } else {
                    5
                }
            }
            Item::Backtest => 8,
            Item::Tool(idx) => app
                .analysis
                .cards
                .get(*idx)
                .map(|c| tool_height(&c.block))
                .unwrap_or(3),
        };
        let is_last = i + 1 == total;
        let reserve = if is_last { 0 } else { 1 };
        if y.saturating_add(h).saturating_add(reserve) > bottom {
            break;
        }
        let rect = Rect::new(inner.x, y, inner.width, h);
        match item {
            Item::Price => draw_price(frame, rect, app),
            Item::SpreadPos => draw_spread_position(frame, rect, app, stacked),
            Item::Backtest => draw_backtest(frame, rect, app),
            Item::Tool(idx) => {
                if let Some(card) = app.analysis.cards.get(*idx) {
                    draw_tool_card(frame, rect, &card.tool, &card.block);
                }
            }
        }
        y += h;
        drawn += 1;
    }
    let cut = total - drawn;
    if cut > 0 && y < bottom {
        frame.render_widget(
            Paragraph::new(muted(format!("+{cut} more · PgDn"))),
            Rect::new(inner.x, bottom - 1, inner.width, 1),
        );
    }
}

fn ticker_strip(app: &TuiApp) -> Line<'static> {
    let mut spans: Vec<Span<'static>> = Vec::new();
    for (i, sym) in app.analysis.focus.iter().enumerate() {
        if i > 0 {
            spans.push(Span::raw("   "));
        }
        let base = base_symbol(sym);
        match app.live.quote(sym) {
            Some(q) => {
                spans.push(Span::styled(base, theme::bold(theme::TEXT)));
                spans.push(Span::raw(" "));
                spans.push(Span::styled(
                    format!("{:.2}", q.last),
                    theme::fg(theme::TEXT),
                ));
                spans.push(Span::raw(" "));
                let sign = if q.change_pct >= Decimal::ZERO {
                    "+"
                } else {
                    ""
                };
                spans.push(Span::styled(
                    format!("{sign}{:.2}%", q.change_pct),
                    theme::fg(signed_color(q.change_pct)),
                ));
            }
            None => {
                spans.push(Span::styled(
                    format!("{base} —"),
                    theme::fg(theme::TEXT_MUTED),
                ));
            }
        }
    }
    Line::from(spans)
}

fn draw_price(frame: &mut Frame, area: Rect, app: &TuiApp) {
    let Some(sym) = app.analysis.primary() else {
        return;
    };
    let block = card_block(&format!("Price · {}", base_symbol(sym)));
    let inner = block.inner(area);
    frame.render_widget(block, area);
    let hist = app.live.history(sym);
    let Some(hist) = hist.filter(|h| h.len() >= 2) else {
        let line = if !app.live.connected {
            muted("no live data — run `tantra serve <data_dir>` to stream prices")
        } else {
            Line::from(Span::styled(
                format!("waiting for prices… {}", theme::spinner_frame(app.tick)),
                theme::fg(theme::ACCENT),
            ))
        };
        frame.render_widget(Paragraph::new(line).wrap(Wrap { trim: false }), inner);
        return;
    };
    let lo = hist.iter().copied().min().unwrap_or(Decimal::ZERO);
    let hi = hist.iter().copied().max().unwrap_or(Decimal::ZERO);
    let mut pad = (hi - lo) / Decimal::from(100);
    if pad.is_zero() {
        pad = if hi.is_zero() {
            Decimal::ONE
        } else {
            hi.abs() / Decimal::from(100)
        };
    }
    let data: Vec<(f64, f64)> = hist
        .iter()
        .enumerate()
        .map(|(i, p)| (i as f64, coord_f64(*p)))
        .collect();
    let ds = Dataset::default()
        .marker(Marker::Braille)
        .graph_type(GraphType::Line)
        .style(theme::fg(theme::PRIMARY))
        .data(&data);
    let label_style = theme::fg(theme::TEXT_MUTED);
    let chart = Chart::new(vec![ds])
        .x_axis(Axis::default().bounds([0.0, (hist.len() - 1) as f64]))
        .y_axis(
            Axis::default()
                .bounds([coord_f64(lo - pad), coord_f64(hi + pad)])
                .labels(vec![
                    Span::styled(format!("{:.2}", lo), label_style),
                    Span::styled(format!("{:.2}", hi), label_style),
                ]),
        );
    frame.render_widget(chart, inner);
}

fn draw_spread_position(frame: &mut Frame, area: Rect, app: &TuiApp, stacked: bool) {
    let Some(sym) = app.analysis.primary() else {
        return;
    };
    let base = base_symbol(sym);
    let (dir, cons) = if stacked {
        (
            Direction::Vertical,
            [Constraint::Length(5), Constraint::Length(5)],
        )
    } else {
        (
            Direction::Horizontal,
            [Constraint::Percentage(50), Constraint::Percentage(50)],
        )
    };
    let gap = u16::from(dir == Direction::Horizontal);
    let halves = Layout::default()
        .direction(dir)
        .spacing(gap)
        .constraints(cons)
        .split(area);

    let sblock = card_block(&format!("Spread · {base}"));
    let sinner = sblock.inner(halves[0]);
    frame.render_widget(sblock, halves[0]);
    let lines = match app.live.depth(sym).and_then(|d| Some((d.bid?, d.ask?))) {
        Some((bid, ask)) => {
            let spread = ask - bid;
            let mid = (ask + bid) / Decimal::from(2);
            let bps = if mid.is_zero() {
                Decimal::ZERO
            } else {
                (spread / mid * Decimal::from(10000)).round_dp(1)
            };
            vec![
                Line::from(vec![
                    Span::raw("bid  "),
                    Span::styled(bid.to_string(), theme::fg(theme::SUCCESS)),
                ]),
                Line::from(vec![
                    Span::raw("ask  "),
                    Span::styled(ask.to_string(), theme::fg(theme::ERROR)),
                ]),
                Line::from(format!("spread {spread} ({bps} bps)")),
            ]
        }
        None => vec![muted("no quote yet")],
    };
    frame.render_widget(Paragraph::new(lines), sinner);

    let pblock = card_block(&format!("Position · {base}"));
    let pinner = pblock.inner(halves[1]);
    frame.render_widget(pblock, halves[1]);
    let line = match app.live.position(sym) {
        Some(p) if !p.quantity.is_zero() => Line::from(vec![
            Span::raw("qty "),
            Span::styled(p.quantity.to_string(), theme::fg(signed_color(p.quantity))),
        ]),
        _ => muted("flat"),
    };
    frame.render_widget(Paragraph::new(line), pinner);
}

fn draw_backtest(frame: &mut Frame, area: Rect, app: &TuiApp) {
    let Some(r) = app.backtest_report.as_ref() else {
        return;
    };
    let block = card_block("Backtest");
    let inner = block.inner(area);
    frame.render_widget(block, area);
    let parts = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(2), Constraint::Min(0)])
        .split(inner);
    let line1 = Line::from(vec![
        Span::styled(
            format!("return {:.2}%", r.total_return_pct),
            theme::fg(signed_color(r.total_return_pct)),
        ),
        Span::raw(format!("  sharpe {:.2}  ", r.sharpe_ratio)),
        Span::styled(
            format!("max dd {:.2}%", r.max_drawdown_pct),
            theme::fg(theme::WARNING),
        ),
    ]);
    let line2 = Line::from(format!(
        "trades {}  win {:.2}%  pf {:.2}",
        r.total_trades, r.win_rate_pct, r.profit_factor
    ));
    frame.render_widget(Paragraph::new(vec![line1, line2]), parts[0]);
    frame.render_widget(
        Sparkline::default()
            .data(&app.backtest_equity_curve)
            .style(Style::default().fg(theme::PRIMARY)),
        parts[1],
    );
}

/// Card height (border included). `ResultBlockWidget` draws its own border, which
/// shares the card's outer frame (see `draw_tool_card`), so no extra rows are needed.
fn tool_height(block: &DisplayBlock) -> u16 {
    let h = match block {
        DisplayBlock::Table { rows, .. } => (rows.len() + 3).min(12),
        DisplayBlock::MetricGrid { items } => items.len().min(8) + 2,
        DisplayBlock::Alert { .. } => 3,
        DisplayBlock::OverlayScene { .. } => 4,
        DisplayBlock::Chart { .. } => 4,
        #[allow(unreachable_patterns)]
        _ => 4,
    };
    h as u16
}

fn block_label(block: &DisplayBlock) -> String {
    match block {
        DisplayBlock::Table { title, .. }
        | DisplayBlock::Chart { title, .. }
        | DisplayBlock::OverlayScene { title, .. } => title.clone(),
        DisplayBlock::MetricGrid { .. } => "metrics".to_string(),
        DisplayBlock::Alert { .. } => "alert".to_string(),
        #[allow(unreachable_patterns)]
        _ => "result".to_string(),
    }
}

fn draw_tool_card(frame: &mut Frame, area: Rect, tool: &str, block: &DisplayBlock) {
    let title = format!("{} · {}", tool.replace('_', " "), block_label(block));
    let frame_block = card_block(&title);
    let inner = frame_block.inner(area);
    // stand-in for tantra-agent/docs/gaps/0009-chart-series-ref-not-delivered-to-chat-clients.md
    if let DisplayBlock::Chart {
        chart_type,
        series_ref,
        ..
    } = block
    {
        let lines = vec![
            muted(format!(
                "{chart_type} chart · series not available in the TUI yet"
            )),
            muted(format!("ref {series_ref}")),
        ];
        frame.render_widget(frame_block, area);
        frame.render_widget(Paragraph::new(lines), inner);
    } else {
        // The widget draws its own rounded frame on `area`; the card frame is then drawn
        // over it (a Block only paints border cells), so the card title wins and there is
        // a single border, not a nested one.
        frame.render_widget(ResultBlockWidget { block }, area);
        frame.render_widget(frame_block, area);
    }
}
