//! Rendering for the dynamic right-hand pane (tab strip + active body).

use ratatui::Frame;
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Paragraph, Wrap};
use tantra_agent_core::DisplayBlock;

use crate::analysis_view::{draw_analysis, draw_tool_card, tool_height};
use crate::app::{SLASH_COMMANDS, TuiApp};
use crate::pane::{AgentView, PaneBody, view_cards, view_kind_label};
use crate::result_block::ResultBlockWidget;
use crate::theme;

const TAB_HINT: &str = "Alt+←/→ · Alt+x";

fn muted(s: impl Into<String>) -> Line<'static> {
    Line::from(Span::styled(s.into(), theme::fg(theme::TEXT_MUTED)))
}

/// Draw the active pane (with its tab strip) into `area`.
pub fn draw_pane(frame: &mut Frame, area: Rect, app: &TuiApp) {
    if area.height < 2 || area.width == 0 {
        return;
    }
    let rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(1), Constraint::Min(0)])
        .split(area);
    draw_tab_strip(frame, rows[0], app);
    let body_area = rows[1];
    let Some(pane) = app.panes.active() else {
        return;
    };
    match &pane.body {
        PaneBody::Analysis => draw_analysis(frame, body_area, app),
        PaneBody::StrategyLab => crate::strategy_lab::draw_strategy_lab(frame, body_area, app),
        PaneBody::Backtest => crate::ui::draw_backtest(frame, body_area, app),
        PaneBody::Portfolio => crate::ui::draw_portfolio(frame, body_area, app),
        PaneBody::Help { topic } => {
            let lines = help_lines(app, topic.as_deref());
            draw_text(frame, body_area, "Help", lines, pane.scroll);
        }
        PaneBody::Info { lines } => {
            let lines: Vec<Line> = lines.iter().map(|l| Line::from(l.clone())).collect();
            draw_text(frame, body_area, &pane.title, lines, pane.scroll);
        }
        PaneBody::View(view) => draw_view(frame, body_area, app, view, pane.scroll),
    }
}

fn draw_tab_strip(frame: &mut Frame, area: Rect, app: &TuiApp) {
    let titles = app.panes.titles();
    let mut spans: Vec<Span> = Vec::new();
    for (i, (title, active)) in titles.iter().enumerate() {
        if i > 0 {
            spans.push(Span::styled(" │ ", theme::fg(theme::TEXT_MUTED)));
        }
        let style = if *active {
            theme::fg(theme::PRIMARY).add_modifier(Modifier::BOLD)
        } else {
            theme::fg(theme::TEXT_MUTED)
        };
        spans.push(Span::styled(title.clone(), style));
    }
    let hint_w = TAB_HINT.chars().count() as u16;
    let total: usize = spans.iter().map(|s| s.content.chars().count()).sum();
    let show_hint = total + usize::from(hint_w) + 2 <= usize::from(area.width);
    let tabs_w = if show_hint {
        area.width - hint_w - 1
    } else {
        area.width
    };
    // Truncate the strip to its width.
    let mut used = 0usize;
    let mut clipped: Vec<Span> = Vec::new();
    for span in spans {
        let n = span.content.chars().count();
        if used + n <= tabs_w as usize {
            used += n;
            clipped.push(span);
        } else {
            let keep = (tabs_w as usize).saturating_sub(used);
            let text: String = span.content.chars().take(keep).collect();
            clipped.push(Span::styled(text, span.style));
            break;
        }
    }
    frame.render_widget(
        Paragraph::new(Line::from(clipped)),
        Rect::new(area.x, area.y, tabs_w, 1),
    );
    if show_hint {
        frame.render_widget(
            Paragraph::new(muted(TAB_HINT)),
            Rect::new(area.x + area.width - hint_w, area.y, hint_w, 1),
        );
    }
}

fn draw_text(frame: &mut Frame, area: Rect, title: &str, lines: Vec<Line<'static>>, scroll: u16) {
    let block = theme::panel_block(title, theme::ACCENT);
    let inner = block.inner(area);
    frame.render_widget(block, area);
    let visible: Vec<Line> = lines.into_iter().skip(scroll as usize).collect();
    frame.render_widget(Paragraph::new(visible).wrap(Wrap { trim: false }), inner);
}

fn help_lines(app: &TuiApp, topic: Option<&str>) -> Vec<Line<'static>> {
    let mut out: Vec<Line<'static>> = Vec::new();
    if let Some(topic) = topic {
        match SLASH_COMMANDS.iter().find(|(c, _)| *c == topic) {
            Some((c, desc)) => {
                out.push(Line::from(Span::styled(
                    (*c).to_string(),
                    theme::fg(theme::ACCENT).add_modifier(Modifier::BOLD),
                )));
                out.push(Line::from(format!("  {desc}")));
            }
            None => {
                out.push(Line::from(Span::styled(
                    format!("Unknown command '{topic}'"),
                    theme::fg(theme::WARNING),
                )));
                out.push(muted("Type /help to view all slash commands."));
            }
        }
        return out;
    }
    for (group, rows) in app.palette_groups() {
        out.push(Line::from(Span::styled(
            group.to_string(),
            theme::fg(theme::ACCENT).add_modifier(Modifier::BOLD),
        )));
        for (cmd, desc) in rows {
            out.push(Line::from(vec![
                Span::styled(format!("  {cmd:<14}"), theme::fg(theme::PRIMARY)),
                Span::raw(desc.to_string()),
            ]));
        }
        out.push(Line::from(""));
    }
    out.push(Line::from(Span::styled(
        "Keys".to_string(),
        theme::fg(theme::ACCENT).add_modifier(Modifier::BOLD),
    )));
    for k in [
        "  Ctrl+P        command palette",
        "  Tab           autocomplete",
        "  Alt+←/→       switch pane",
        "  Alt+x         close pane",
        "  PgUp/PgDn     scroll pane",
    ] {
        out.push(Line::from(k));
    }
    out
}

fn clean_summary_line(l: &str) -> String {
    l.trim_start()
        .trim_start_matches('#')
        .trim_start()
        .replace("**", "")
}

fn draw_view(frame: &mut Frame, area: Rect, app: &TuiApp, view: &AgentView, scroll: u16) {
    let block = theme::panel_block(view_kind_label(view.kind), theme::ACCENT);
    let inner = block.inner(area);
    frame.render_widget(block, area);
    if inner.height == 0 {
        return;
    }

    let mut head: Vec<Line> = vec![Line::from(vec![
        Span::styled(
            view_kind_label(view.kind).to_string(),
            theme::fg(theme::ACCENT),
        ),
        Span::raw("  "),
        Span::styled(
            view.title.clone(),
            Style::default().add_modifier(Modifier::BOLD),
        ),
    ])];
    if !view.symbols.is_empty() {
        let mut chips: Vec<Span> = Vec::new();
        for s in &view.symbols {
            chips.push(Span::styled(
                format!(" {s} "),
                theme::fg(theme::PRIMARY).add_modifier(Modifier::REVERSED),
            ));
            chips.push(Span::raw(" "));
        }
        head.push(Line::from(chips));
    }
    for l in view.summary.lines() {
        head.push(Line::from(clean_summary_line(l)));
    }
    let wrap_w = inner.width.max(1) as usize;
    let head_rows: usize = head
        .iter()
        .map(|l| {
            let n: usize = l.spans.iter().map(|s| s.content.chars().count()).sum();
            n.div_ceil(wrap_w).max(1)
        })
        .sum();
    let head_h = (head_rows as u16).min(inner.height);

    let metrics = DisplayBlock::MetricGrid {
        items: view.highlights.clone(),
    };
    let metrics_h = if view.highlights.is_empty() {
        0
    } else {
        tool_height(&metrics).min(inner.height.saturating_sub(head_h))
    };

    let head_area = Rect::new(inner.x, inner.y, inner.width, head_h);
    frame.render_widget(Paragraph::new(head).wrap(Wrap { trim: false }), head_area);
    let mut y = inner.y + head_h;
    if metrics_h > 0 {
        let r = Rect::new(inner.x, y, inner.width, metrics_h);
        frame.render_widget(ResultBlockWidget { block: &metrics }, r);
        y += metrics_h;
    }

    let bottom = inner.y + inner.height;
    for card in view_cards(view, &app.analysis.cards)
        .into_iter()
        .skip(scroll as usize)
    {
        if y >= bottom {
            break;
        }
        let h = tool_height(&card.block).min(bottom - y);
        draw_tool_card(
            frame,
            Rect::new(inner.x, y, inner.width, h),
            &card.tool,
            &card.block,
        );
        y += h;
    }
}
