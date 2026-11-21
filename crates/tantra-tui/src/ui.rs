//! Ratatui UI drawing routines for the Tantra workstation.

use ratatui::Frame;
use ratatui::layout::{Alignment, Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, BorderType, Borders, Clear, Paragraph, Row, Sparkline, Table, Wrap};
use rust_decimal::Decimal;
use tantra_ui_core::algos::AlgoStatus;

use tantra_highlight::HighlightClass;

use crate::agent_chat::{AgentMessage, AgentRole};
use crate::app::{ActiveTab, TuiApp};
use crate::layout::LayoutModel;
use crate::theme;

/// 8x8 dot-matrix Tantra mark (`#` = filled).
const MARK: [&str; 8] = [
    "#..##..#", "#..##..#", "#..##..#", "########", ".######.", "...##...", "...##...", "...##...",
];

/// Figlet wordmark.
const WORDMARK: [&str; 5] = [
    r" _____           _             ",
    r"|_   _|_ _ _ __ | |_ _ __ __ _ ",
    r"  | |/ _` | '_ \| __| '__/ _` |",
    r"  | | (_| | | | | |_| | | (_| |",
    r"  |_|\__,_|_| |_|\__|_|  \__,_|",
];

/// Map a syntax highlight lexical class to a Ratatui style (markdown palette).
pub fn class_to_style(class: HighlightClass) -> Style {
    match class {
        HighlightClass::Keyword => theme::bold(theme::MD_H1),
        HighlightClass::String => theme::fg(theme::MD_CODE),
        HighlightClass::Comment => theme::fg(theme::TEXT_MUTED).add_modifier(Modifier::ITALIC),
        HighlightClass::Number => theme::fg(theme::MD_CODE),
        HighlightClass::Function => theme::bold(theme::MD_H2),
        HighlightClass::Type => theme::fg(theme::MD_H3),
        HighlightClass::Punctuation => theme::fg(theme::TEXT_MUTED),
        HighlightClass::Plain => theme::fg(theme::TEXT),
    }
}

/// Split highlighted markdown into styled lines.
fn markdown_lines(msg: &AgentMessage) -> Vec<Vec<Span<'static>>> {
    let mut segs: Vec<(String, Style)> = Vec::new();
    let mut pos = 0usize;
    for span in msg.highlight() {
        if span.range.start > pos {
            if let Some(gap) = msg.text.get(pos..span.range.start) {
                segs.push((gap.to_string(), class_to_style(HighlightClass::Plain)));
            }
        }
        if let Some(t) = msg.text.get(span.range.clone()) {
            segs.push((t.to_string(), class_to_style(span.class)));
        }
        pos = pos.max(span.range.end);
    }
    if let Some(rest) = msg.text.get(pos..) {
        if !rest.is_empty() {
            segs.push((rest.to_string(), class_to_style(HighlightClass::Plain)));
        }
    }
    let mut lines: Vec<Vec<Span<'static>>> = vec![Vec::new()];
    for (text, style) in segs {
        for (i, part) in text.split('\n').enumerate() {
            if i > 0 {
                lines.push(Vec::new());
            }
            if !part.is_empty() {
                if let Some(cur) = lines.last_mut() {
                    cur.push(Span::styled(part.to_string(), style));
                }
            }
        }
    }
    lines
}

fn message_lines(msg: &AgentMessage, tick: u64, loading: bool) -> Vec<Line<'static>> {
    match msg.role {
        AgentRole::User => msg
            .text
            .split('\n')
            .enumerate()
            .map(|(i, l)| {
                let prefix = if i == 0 { "› " } else { "  " };
                Line::from(vec![
                    Span::styled(prefix, theme::fg(theme::PRIMARY)),
                    Span::styled(l.to_string(), theme::bold(theme::TEXT)),
                ])
            })
            .collect(),
        AgentRole::System => msg
            .text
            .split('\n')
            .map(|l| Line::from(Span::styled(l.to_string(), theme::fg(theme::TEXT_MUTED))))
            .collect(),
        AgentRole::Agent => {
            if msg.text.is_empty() {
                if loading {
                    vec![Line::from(Span::styled(
                        format!("{} thinking…", theme::spinner_frame(tick)),
                        theme::fg(theme::ACCENT),
                    ))]
                } else {
                    vec![Line::from("")]
                }
            } else if let Some(err) = msg.text.strip_prefix("*Error:") {
                let err = err.trim().trim_end_matches('*').trim();
                vec![Line::from(Span::styled(
                    format!("error: {err}"),
                    theme::fg(theme::ERROR),
                ))]
            } else {
                markdown_lines(msg).into_iter().map(Line::from).collect()
            }
        }
    }
}

/// Convert an AgentMessage with highlights into Ratatui Lines.
pub fn message_to_lines(msg: &AgentMessage) -> Vec<Line<'static>> {
    message_lines(msg, 0, true)
}

/// Clamp a rect built from raw offsets into `area`.
fn sub(area: Rect, x: u16, y: u16, w: u16, h: u16) -> Rect {
    Rect::new(area.x.saturating_add(x), area.y.saturating_add(y), w, h).intersection(area)
}

fn pad(area: Rect, left: u16, right: u16, top: u16) -> Rect {
    sub(
        area,
        left,
        top,
        area.width.saturating_sub(left + right),
        area.height.saturating_sub(top),
    )
}

fn truncate(s: &str, max: usize) -> String {
    if s.chars().count() <= max {
        s.to_string()
    } else if max == 0 {
        String::new()
    } else {
        let mut out: String = s.chars().take(max - 1).collect();
        out.push('…');
        out
    }
}

/// Draw the entire workstation UI onto a Ratatui Frame.
pub fn draw(frame: &mut Frame, app: &TuiApp) {
    let layout = LayoutModel::new(frame.area());
    if app.active_tab == ActiveTab::AgentChat && app.chat.is_empty() {
        draw_welcome(frame, layout.main, app);
    } else {
        draw_workspace(frame, layout.main, app);
    }
    draw_footer(frame, layout.footer);
    if app.is_palette_open {
        draw_palette(frame, frame.area(), app);
    }
}

fn draw_welcome(frame: &mut Frame, area: Rect, app: &TuiApp) {
    let area = pad(area, 2, 2, 0);
    let block_h: u16 = 8 + 3 + 1 + 3;
    let rem = area.height.saturating_sub(block_h);
    let top = rem * 7 / 15;
    let logo_area = sub(area, 0, top, area.width, 8);

    let wm_w = WORDMARK
        .iter()
        .map(|l| l.chars().count())
        .max()
        .unwrap_or(0) as u16;
    let logo_w = 16 + 3 + wm_w;
    let lx = area.width.saturating_sub(logo_w) / 2;
    let mark: Vec<Line> = MARK
        .iter()
        .map(|row| {
            let spans: Vec<Span> = row
                .chars()
                .map(|c| {
                    if c == '#' {
                        Span::styled("● ", theme::fg(theme::PRIMARY))
                    } else {
                        Span::styled("○ ", theme::fg(theme::TEXT_MUTED))
                    }
                })
                .collect();
            Line::from(spans)
        })
        .collect();
    frame.render_widget(Paragraph::new(mark), sub(logo_area, lx, 0, 16, 8));
    let word: Vec<Line> = WORDMARK
        .iter()
        .map(|l| Line::from(Span::styled(*l, theme::fg(theme::PRIMARY))))
        .collect();
    frame.render_widget(
        Paragraph::new(word),
        sub(logo_area, lx + 16 + 3, 1, wm_w, 5),
    );

    let bw = ((area.width as u32 * 40 / 100) as u16)
        .max(48)
        .min(area.width);
    let bx = area.width.saturating_sub(bw) / 2;
    let by = top + 8 + 3;
    draw_input_header(frame, sub(area, bx, by, bw, 1), app);
    draw_composer(
        frame,
        sub(area, bx, by + 1, bw, 3),
        app,
        "ask a finance question, or type / for commands",
    );
}

fn draw_workspace(frame: &mut Frame, area: Rect, app: &TuiApp) {
    let area = pad(area, 1, 1, 1);
    let rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Min(0),
            Constraint::Length(1),
            Constraint::Length(1),
            Constraint::Length(3),
        ])
        .split(area);
    if app.active_tab == ActiveTab::AgentChat {
        draw_chat_row(frame, rows[0], app);
    } else {
        draw_content(frame, rows[0], app);
    }
    draw_input_header(frame, rows[2], app);
    let placeholder = if app.active_tab == ActiveTab::AgentChat {
        "ask a follow-up, or / for commands"
    } else {
        "press i to ask, / for commands"
    };
    draw_composer(frame, rows[3], app, placeholder);
}

fn draw_chat_row(frame: &mut Frame, area: Rect, app: &TuiApp) {
    if area.width >= 60 {
        let rail_w = ((area.width as u32 * 20 / 100) as u16).max(24);
        let cols = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([
                Constraint::Min(0),
                Constraint::Length(1),
                Constraint::Length(rail_w),
            ])
            .split(area);
        draw_transcript(frame, cols[0], app);
        draw_rail(frame, cols[2], app);
    } else {
        draw_transcript(frame, area, app);
    }
}

fn draw_transcript(frame: &mut Frame, area: Rect, app: &TuiApp) {
    let block = theme::panel_block("Tantra", theme::PRIMARY);
    let inner = pad(block.inner(area), 1, 1, 1);
    frame.render_widget(block, area);

    let mut lines: Vec<Line<'static>> = Vec::new();
    let last = app.chat.len().saturating_sub(1);
    for (i, msg) in app.chat.messages.iter().enumerate() {
        if i > 0 {
            lines.push(Line::from(""));
        }
        let loading = app.is_loading && i == last;
        lines.extend(message_lines(msg, app.tick, loading));
    }
    let w = inner.width.max(1) as usize;
    let total: usize = lines.iter().map(|l| (l.width().max(1)).div_ceil(w)).sum();
    let visible = inner.height as usize;
    let scroll_y = total.saturating_sub(visible).min(u16::MAX as usize) as u16;
    frame.render_widget(
        Paragraph::new(lines)
            .scroll((scroll_y, 0))
            .wrap(Wrap { trim: false }),
        inner,
    );
}

fn draw_rail(frame: &mut Frame, area: Rect, app: &TuiApp) {
    let block = theme::panel_block(
        &format!("Market ({})", app.live.watchlist.len()),
        theme::ACCENT,
    )
    .style(Style::default().bg(theme::BACKGROUND_PANEL));
    let inner = pad(block.inner(area), 1, 1, 1);
    frame.render_widget(block, area);

    let foot_h: u16 = if inner.height > 6 { 4 } else { 0 };
    let body = sub(
        inner,
        0,
        0,
        inner.width,
        inner.height.saturating_sub(foot_h),
    );
    let mut lines: Vec<Line<'static>> = Vec::new();
    if app.live.watchlist.is_empty() {
        lines.push(Line::from(Span::styled(
            "no market data yet",
            theme::fg(theme::TEXT_MUTED),
        )));
    } else {
        for row in &app.live.watchlist {
            let color = if row.change >= Decimal::ZERO {
                theme::SUCCESS
            } else {
                theme::ERROR
            };
            lines.push(Line::from(Span::styled(
                row.symbol.clone(),
                theme::fg(theme::TEXT),
            )));
            lines.push(Line::from(vec![
                Span::styled(row.last.to_string(), theme::fg(theme::TEXT)),
                Span::raw("  "),
                Span::styled(
                    format!("{:+}%", row.change_pct.round_dp(2)),
                    theme::fg(color),
                ),
            ]));
            lines.push(Line::from(""));
        }
    }
    frame.render_widget(Paragraph::new(lines), body);

    if foot_h > 0 {
        let fy = inner.height - foot_h;
        let foot = vec![
            Line::from(Span::styled(
                "─".repeat(inner.width as usize),
                theme::fg(theme::BORDER),
            )),
            Line::from(Span::styled(
                format!("{} messages", app.chat.len()),
                theme::fg(theme::TEXT_MUTED),
            )),
            Line::from(Span::styled(
                format!(
                    "auto-approve {}",
                    if app.auto_approve { "on" } else { "off" }
                ),
                theme::fg(theme::TEXT_MUTED),
            )),
            Line::from(Span::styled(
                format!(
                    "backend {}",
                    if app.live.connected {
                        "connected"
                    } else {
                        "offline"
                    }
                ),
                theme::fg(theme::TEXT_MUTED),
            )),
        ];
        frame.render_widget(Paragraph::new(foot), sub(inner, 0, fy, inner.width, foot_h));
    }
}

fn draw_input_header(frame: &mut Frame, area: Rect, app: &TuiApp) {
    let area = pad(area, 1, 1, 0);
    let view_color = match app.active_tab {
        ActiveTab::AgentChat => theme::PRIMARY,
        ActiveTab::Watchlist | ActiveTab::Screener => theme::INFO,
        ActiveTab::Portfolio | ActiveTab::Orders => theme::WARNING,
        ActiveTab::Backtest | ActiveTab::Algos | ActiveTab::StrategyLab => theme::ACCENT,
    };
    let name = app.active_tab.title().to_lowercase();
    let left_len = name.chars().count() + " (tab to cycle)".len();
    frame.render_widget(
        Paragraph::new(Line::from(vec![
            Span::styled(name, theme::fg(view_color)),
            Span::styled(" (tab to cycle)", theme::fg(theme::TEXT_MUTED)),
        ])),
        area,
    );
    let avail = (area.width as usize).saturating_sub(left_len + 1 + "model: ".len());
    if avail > 0 {
        frame.render_widget(
            Paragraph::new(Line::from(vec![
                Span::styled("model: ", theme::fg(theme::TEXT_MUTED)),
                Span::styled(truncate(&app.model_name, avail), theme::fg(theme::TEXT)),
            ]))
            .alignment(Alignment::Right),
            area,
        );
    }
}

fn draw_composer(frame: &mut Frame, area: Rect, app: &TuiApp, placeholder: &str) {
    let block = theme::field_block(app.is_input_mode);
    let inner = pad(block.inner(area), 1, 1, 0);
    frame.render_widget(block, area);
    let line = if app.input_buffer.is_empty() {
        let ph = if app.is_input_mode {
            placeholder
        } else {
            "press i to ask, / for commands"
        };
        Line::from(Span::styled(ph.to_string(), theme::fg(theme::TEXT_MUTED)))
    } else {
        let mut spans = vec![Span::styled(
            app.input_buffer.clone(),
            theme::fg(theme::TEXT),
        )];
        if app.is_input_mode {
            spans.push(Span::styled("▋", theme::fg(theme::TEXT_MUTED)));
        }
        Line::from(spans)
    };
    frame.render_widget(Paragraph::new(line), inner);
}

fn draw_footer(frame: &mut Frame, area: Rect) {
    let area = pad(area, 2, 2, 0);
    let key = |k: &'static str| Span::styled(k, theme::fg(theme::ACCENT));
    let muted = |t: &'static str| Span::styled(t, theme::fg(theme::TEXT_MUTED));
    frame.render_widget(
        Paragraph::new(Line::from(vec![
            key("/"),
            muted(" commands"),
            Span::raw("  "),
            key("^N"),
            muted(" new"),
            Span::raw("  "),
            key("^C"),
            muted(" quit"),
        ])),
        area,
    );
    frame.render_widget(
        Paragraph::new(Line::from(muted_owned(format!(
            "v{}",
            env!("CARGO_PKG_VERSION")
        ))))
        .alignment(Alignment::Right),
        area,
    );
}

fn muted_owned(s: String) -> Span<'static> {
    Span::styled(s, theme::fg(theme::TEXT_MUTED))
}

fn draw_content(frame: &mut Frame, area: Rect, app: &TuiApp) {
    match app.active_tab {
        ActiveTab::Watchlist => draw_watchlist(frame, area, app),
        ActiveTab::Screener => draw_screener(frame, area, app),
        ActiveTab::Portfolio => draw_portfolio(frame, area, app),
        ActiveTab::Orders => draw_orders(frame, area, app),
        ActiveTab::Backtest => draw_backtest(frame, area, app),
        ActiveTab::Algos => draw_algos(frame, area, app),
        ActiveTab::AgentChat => draw_chat_row(frame, area, app),
        ActiveTab::StrategyLab => crate::strategy_lab::draw_strategy_lab(frame, area, app),
    }
}

fn draw_palette(frame: &mut Frame, area: Rect, app: &TuiApp) {
    let groups = app.palette_groups();
    let list_rows: usize = groups
        .iter()
        .map(|(_, r)| r.len() + 1)
        .sum::<usize>()
        .max(1);
    let content_h = (2 + 2 + 3 + 1 + list_rows + 1 + 1) as u16;
    let mw = 56.min(area.width.saturating_sub(4));
    let mh = content_h.min(area.height.saturating_sub(4));
    let modal = Rect::new(
        area.x + area.width.saturating_sub(mw) / 2,
        area.y + area.height.saturating_sub(mh) / 2,
        mw,
        mh,
    );

    // Dim the backdrop.
    {
        let buf = frame.buffer_mut();
        for y in area.top()..area.bottom() {
            for x in area.left()..area.right() {
                if x >= modal.left() && x < modal.right() && y >= modal.top() && y < modal.bottom()
                {
                    continue;
                }
                let cell = &mut buf[(x, y)];
                cell.bg = Color::Rgb(0, 0, 0);
                cell.fg = theme::dim_color(cell.fg);
            }
        }
    }

    frame.render_widget(Clear, modal);
    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(theme::fg(theme::PRIMARY))
        .title(Line::from(Span::styled(
            "─Commands",
            theme::bold(theme::PRIMARY),
        )))
        .style(Style::default().bg(theme::BACKGROUND_PANEL));
    let inner = pad(block.inner(modal), 2, 2, 1);
    frame.render_widget(block, modal);
    if inner.height < 6 {
        return;
    }

    // Field.
    let field_area = sub(inner, 0, 0, inner.width, 3);
    let fblock = theme::field_block(true).style(Style::default().bg(theme::BACKGROUND_PANEL));
    let finner = pad(fblock.inner(field_area), 1, 1, 0);
    frame.render_widget(fblock, field_area);
    let q = app.palette_query.as_str();
    let field_line = if q.is_empty() || q == "/" {
        Line::from(Span::styled(
            "type to filter…",
            Style::default().fg(theme::PLACEHOLDER),
        ))
    } else {
        Line::from(Span::styled(q.to_string(), theme::fg(theme::TEXT)))
    };
    frame.render_widget(Paragraph::new(field_line), finner);

    // List.
    let list_h = inner.height.saturating_sub(3 + 1 + 1 + 1) as usize;
    let w = inner.width as usize;
    let selected_cmd = app
        .filtered_palette()
        .get(app.palette_selected)
        .map(|(c, _)| *c);
    let mut lines: Vec<Line<'static>> = Vec::new();
    let mut sel_line = 0usize;
    if groups.is_empty() {
        lines.push(Line::from(Span::styled(
            "  no matching commands",
            theme::fg(theme::TEXT_MUTED),
        )));
    }
    for (group, rows) in &groups {
        let group_selected = rows.iter().any(|(c, _)| Some(*c) == selected_cmd);
        lines.push(Line::from(Span::styled(
            group.to_string(),
            if group_selected {
                theme::bold(theme::ACCENT)
            } else {
                theme::fg(theme::ACCENT)
            },
        )));
        for (cmd, desc) in rows {
            let is_sel = Some(*cmd) == selected_cmd;
            let left = format!("  {cmd}");
            let avail = w.saturating_sub(left.chars().count() + 1);
            let d = if avail > 1 {
                truncate(desc, avail)
            } else {
                String::new()
            };
            let gap = w.saturating_sub(left.chars().count() + d.chars().count());
            let text = format!("{left}{}{d}", " ".repeat(gap));
            if is_sel {
                sel_line = lines.len();
                lines.push(Line::from(Span::styled(
                    text,
                    Style::default().fg(theme::BACKGROUND).bg(theme::PRIMARY),
                )));
            } else {
                lines.push(Line::from(vec![
                    Span::styled(left.clone(), theme::fg(theme::TEXT)),
                    Span::styled(
                        format!("{}{d}", " ".repeat(gap)),
                        theme::fg(theme::TEXT_MUTED),
                    ),
                ]));
            }
        }
    }
    let offset = if list_h > 0 && sel_line >= list_h {
        sel_line + 1 - list_h
    } else {
        0
    };
    let visible: Vec<Line<'static>> = lines.into_iter().skip(offset).take(list_h).collect();
    frame.render_widget(
        Paragraph::new(visible),
        sub(inner, 0, 4, inner.width, list_h as u16),
    );
    let hy = 4 + list_h as u16 + 1;
    frame.render_widget(
        Paragraph::new(Line::from(Span::styled(
            "↑↓ move / enter run / tab complete / esc close",
            theme::fg(theme::TEXT_MUTED),
        ))),
        sub(inner, 0, hy, inner.width, 1),
    );
}

fn draw_watchlist(frame: &mut Frame, area: Rect, app: &TuiApp) {
    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::DarkGray))
        .title(" Watchlist — Indian Equities & Indices (NSE) ");

    if app.live.watchlist.is_empty() {
        render_empty_state(
            frame,
            area,
            block,
            "No live market data. Boot `tantra serve <data_dir>` and publish \
             `ClientEvent`s to an `EventStream`, or seed `TuiApp.live` \
             (gap 0005 wiring seam).",
        );
        return;
    }

    let header = Row::new(vec!["Symbol", "Instrument", "LTP (₹)", "Change", "Volume"]).style(
        Style::default()
            .fg(Color::Cyan)
            .add_modifier(Modifier::BOLD),
    );

    let rows: Vec<Row> = app
        .live
        .watchlist
        .iter()
        .map(|w| {
            let color = if w.change >= Decimal::ZERO {
                Color::Green
            } else {
                Color::Red
            };
            Row::new(vec![
                w.symbol.clone(),
                "NSE".to_string(),
                format!("₹{:.2}", w.last),
                format!("{:+.2} ({:+.2}%)", w.change, w.change_pct),
                format!("{}", w.volume),
            ])
            .style(Style::default().fg(color))
        })
        .collect();

    let widths = [
        Constraint::Length(14),
        Constraint::Length(14),
        Constraint::Length(14),
        Constraint::Length(22),
        Constraint::Length(12),
    ];

    let table = Table::new(rows, widths).header(header).block(block);
    frame.render_widget(table, area);
}

/// Render an honest empty-state panel for a tab that has no live data yet.
fn render_empty_state(frame: &mut Frame, area: Rect, block: Block<'static>, message: &str) {
    let text = vec![
        Line::from(""),
        Line::from(vec![Span::styled(
            "  No data yet.",
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        )]),
        Line::from(vec![Span::styled(
            format!("  {message}"),
            Style::default().fg(Color::White),
        )]),
        Line::from(vec![Span::styled(
            "  Live data arrives as canonical `ClientEvent`s published to \
             `TuiApp::publish` (gap 0005 wiring seam).",
            Style::default().fg(Color::DarkGray),
        )]),
    ];

    let p = Paragraph::new(text).block(block);
    frame.render_widget(p, area);
}

fn draw_screener(frame: &mut Frame, area: Rect, app: &TuiApp) {
    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::DarkGray))
        .title(" Neural AST Screener — Scan Results ");

    if app.live.screener.is_empty() {
        render_empty_state(
            frame,
            area,
            block,
            "No scan results yet. Publish a `ClientEvent::Screener` \
             (ScreenerScanResponse) to the live-data seam.",
        );
        return;
    }

    let header = Row::new(vec![
        "Instrument",
        "Metric 1",
        "Metric 2",
        "Metric 3",
        "Metric 4",
    ])
    .style(
        Style::default()
            .fg(Color::Cyan)
            .add_modifier(Modifier::BOLD),
    );

    let rows: Vec<Row> = app
        .live
        .screener
        .iter()
        .map(|hit| {
            let mut cells = vec![hit.instrument.clone()];
            for (name, value) in hit.metrics.iter().take(4) {
                cells.push(format!("{name}: {value}"));
            }
            while cells.len() < 5 {
                cells.push(String::new());
            }
            Row::new(cells).style(Style::default().fg(Color::White))
        })
        .collect();

    let widths = [
        Constraint::Percentage(25),
        Constraint::Percentage(25),
        Constraint::Percentage(25),
        Constraint::Percentage(25),
        Constraint::Percentage(25),
    ];

    let table = Table::new(rows, widths).header(header).block(block);
    frame.render_widget(table, area);
}

fn draw_portfolio(frame: &mut Frame, area: Rect, app: &TuiApp) {
    // The Portfolio tab is split: the live snapshot summary on top and the
    // EE+ positioning panel below (plan 0006 Phase 4 — same `studio_ee_positions`
    // rows the web Studio and `tantra elephant` render).
    let chunks =
        Layout::vertical([Constraint::Percentage(60), Constraint::Percentage(40)]).split(area);
    draw_portfolio_snapshot(frame, chunks[0], app);
    draw_ee_positioning(frame, chunks[1], app);
}

fn draw_portfolio_snapshot(frame: &mut Frame, area: Rect, app: &TuiApp) {
    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::DarkGray))
        .title(" Portfolio Snapshot ");

    let Some(portfolio) = &app.live.portfolio else {
        render_empty_state(
            frame,
            area,
            block,
            "No portfolio snapshot yet. Publish a `ClientEvent::Portfolio` \
             (PortfolioSnapshot) to the live-data seam.",
        );
        return;
    };

    let pnl_color = if portfolio.total_pnl >= Decimal::ZERO {
        Color::Green
    } else {
        Color::Red
    };
    let pnl_sign = if portfolio.total_pnl >= Decimal::ZERO {
        "+"
    } else {
        ""
    };

    let lines = vec![
        Line::from(""),
        Line::from(vec![
            Span::styled("  Cash:          ", Style::default().fg(Color::DarkGray)),
            Span::styled(
                format!("₹{:.2}", portfolio.cash),
                Style::default()
                    .fg(Color::White)
                    .add_modifier(Modifier::BOLD),
            ),
        ]),
        Line::from(vec![
            Span::styled("  Net Worth:     ", Style::default().fg(Color::DarkGray)),
            Span::styled(
                format!("₹{:.2}", portfolio.net_worth),
                Style::default()
                    .fg(Color::White)
                    .add_modifier(Modifier::BOLD),
            ),
        ]),
        Line::from(vec![
            Span::styled("  Total PnL:     ", Style::default().fg(Color::DarkGray)),
            Span::styled(
                format!("{pnl_sign}{:.2}", portfolio.total_pnl),
                Style::default().fg(pnl_color).add_modifier(Modifier::BOLD),
            ),
        ]),
        Line::from(vec![
            Span::styled("  Open Positions:", Style::default().fg(Color::DarkGray)),
            Span::styled(
                format!("{}", portfolio.positions),
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD),
            ),
        ]),
        Line::from(""),
        Line::from(Span::styled(
            "  FIFO tax-lots, unrealized STCG/LTCG, and real-time MTM valuation.",
            Style::default().fg(Color::DarkGray),
        )),
    ];

    let p = Paragraph::new(lines).block(block);
    frame.render_widget(p, area);
}

/// The EE+ positioning panel (plan 0006 Phase 4): renders the same
/// `studio_ee_positions` rows the web Studio and `tantra elephant` print.
fn draw_ee_positioning(frame: &mut Frame, area: Rect, app: &TuiApp) {
    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::DarkGray))
        .title(" EE+ Positioning ");
    let p = Paragraph::new(app.ee_positioning_text()).block(block);
    frame.render_widget(p, area);
}

fn draw_orders(frame: &mut Frame, area: Rect, app: &TuiApp) {
    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::DarkGray))
        .title(" Orders & OMS ");

    if app.live.orders.is_empty() {
        render_empty_state(
            frame,
            area,
            block,
            "No order activity yet. Publish `ClientEvent::Order` / \
             `ClientEvent::Fill` events to the live-data seam.",
        );
        return;
    }

    let header = Row::new(vec!["Order ID", "Kind", "Status"]).style(
        Style::default()
            .fg(Color::Cyan)
            .add_modifier(Modifier::BOLD),
    );

    let rows: Vec<Row> = app
        .live
        .orders
        .iter()
        .map(|o| {
            Row::new(vec![o.order_id.clone(), o.kind.clone(), o.status.clone()])
                .style(Style::default().fg(Color::White))
        })
        .collect();

    let widths = [
        Constraint::Percentage(40),
        Constraint::Percentage(20),
        Constraint::Percentage(40),
    ];

    let table = Table::new(rows, widths).header(header).block(block);
    frame.render_widget(table, area);
}

fn draw_algos(frame: &mut Frame, area: Rect, app: &TuiApp) {
    let vert_chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(3), Constraint::Length(1)])
        .split(area);

    let main_area = vert_chunks[0];
    let action_bar_area = vert_chunks[1];

    let horiz_chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(60), Constraint::Percentage(40)])
        .split(main_area);

    let left_area = horiz_chunks[0];
    let right_area = horiz_chunks[1];

    // Left Panel: Table of registered algorithms
    let header = Row::new(vec![
        "Algorithm",
        "Version",
        "Status",
        "Sharpe",
        "Instruments",
    ])
    .style(
        Style::default()
            .fg(Color::Cyan)
            .add_modifier(Modifier::BOLD),
    );

    let mut rows = Vec::new();
    for (idx, algo) in app.algos.iter().enumerate() {
        let is_selected = idx == app.selected_algo_index;
        let (status_str, _status_color) = match algo.status {
            AlgoStatus::Active => ("● Active", Color::Green),
            AlgoStatus::Paused => ("❚❚ Paused", Color::Yellow),
            AlgoStatus::Stopped => ("■ Stopped", Color::Red),
            AlgoStatus::Error => ("✖ Error", Color::LightRed),
        };

        let inst_str = algo.descriptor.target_instruments.join(", ");
        let name_display = if is_selected {
            format!("▶ {}", algo.descriptor.name)
        } else {
            format!("  {}", algo.descriptor.name)
        };

        let row_style = if is_selected {
            Style::default()
                .fg(Color::Black)
                .bg(Color::Cyan)
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(Color::White)
        };

        rows.push(
            Row::new(vec![
                name_display,
                algo.descriptor.version.clone(),
                status_str.to_string(),
                format!("{:.2}", algo.baseline_sharpe),
                inst_str,
            ])
            .style(row_style),
        );
    }

    let left_block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::DarkGray))
        .title(" Registered Algorithms (60%) ");

    let widths = [
        Constraint::Percentage(32),
        Constraint::Percentage(12),
        Constraint::Percentage(16),
        Constraint::Percentage(12),
        Constraint::Percentage(28),
    ];

    let table = Table::new(rows, widths).header(header).block(left_block);
    frame.render_widget(table, left_area);

    // Right Panel: Selected Strategy Details & Parameter Inspector
    let right_block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::DarkGray))
        .title(" Selected Strategy & Parameter Inspector (40%) ");

    let mut details_lines = Vec::new();
    if let Some(selected) = app.selected_algo() {
        details_lines.push(Line::from(vec![
            Span::styled(
                "Strategy: ",
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled(
                &selected.descriptor.name,
                Style::default()
                    .fg(Color::White)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled(
                format!(" ({})", selected.descriptor.id),
                Style::default().fg(Color::DarkGray),
            ),
        ]));
        details_lines.push(Line::from(vec![
            Span::styled("Version:  ", Style::default().fg(Color::Cyan)),
            Span::styled(
                &selected.descriptor.version,
                Style::default().fg(Color::White),
            ),
            Span::styled(" | Origin: ", Style::default().fg(Color::Cyan)),
            Span::styled(
                selected.descriptor.source.as_str(),
                Style::default().fg(Color::Yellow),
            ),
        ]));
        details_lines.push(Line::from(vec![
            Span::styled("Status:   ", Style::default().fg(Color::Cyan)),
            Span::styled(
                selected.status.as_str(),
                Style::default().fg(match selected.status {
                    AlgoStatus::Active => Color::Green,
                    AlgoStatus::Paused => Color::Yellow,
                    _ => Color::Red,
                }),
            ),
            Span::styled(
                format!(" | Baseline Sharpe: {:.2}", selected.baseline_sharpe),
                Style::default().fg(Color::White),
            ),
        ]));
        details_lines.push(Line::from(""));
        details_lines.push(Line::from(vec![
            Span::styled(
                "Description: ",
                Style::default()
                    .fg(Color::DarkGray)
                    .add_modifier(Modifier::ITALIC),
            ),
            Span::styled(
                &selected.descriptor.description,
                Style::default().fg(Color::White),
            ),
        ]));
        details_lines.push(Line::from(""));
        details_lines.push(Line::from(Span::styled(
            "─── Parameter Search Contract ──────────────────────────",
            Style::default().fg(Color::DarkGray),
        )));

        for p in &selected.descriptor.parameters {
            let range_str = if let (Some(min), Some(max), Some(step)) = (p.min, p.max, p.step) {
                format!("{min} .. {max} (step {step})")
            } else {
                "Fixed (no sweep)".to_string()
            };
            details_lines.push(Line::from(vec![
                Span::styled(
                    format!(" • {:<15} ", p.name),
                    Style::default()
                        .fg(Color::LightBlue)
                        .add_modifier(Modifier::BOLD),
                ),
                Span::styled(
                    format!("Default: {:<5} ", p.default),
                    Style::default().fg(Color::White),
                ),
                Span::styled(format!("[{range_str}]"), Style::default().fg(Color::Yellow)),
            ]));
            if !p.description.is_empty() {
                details_lines.push(Line::from(Span::styled(
                    format!("   ↳ {}", p.description),
                    Style::default().fg(Color::DarkGray),
                )));
            }
        }

        details_lines.push(Line::from(""));
        details_lines.push(Line::from(vec![
            Span::styled("Target Instruments: ", Style::default().fg(Color::DarkGray)),
            Span::styled(
                selected.descriptor.target_instruments.join(", "),
                Style::default().fg(Color::Green),
            ),
        ]));
    } else {
        details_lines.push(Line::from("  No strategy selected."));
    }

    let p = Paragraph::new(details_lines)
        .block(right_block)
        .wrap(Wrap { trim: true });
    frame.render_widget(p, right_area);

    // Action Bar
    let action_line = Line::from(vec![
        Span::styled(
            " [Enter] ",
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        ),
        Span::raw("Backtest | "),
        Span::styled(
            "[T] ",
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        ),
        Span::raw("Tune Params | "),
        Span::styled(
            "[S] ",
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        ),
        Span::raw("Suggest Trade | "),
        Span::styled(
            "[Space] ",
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        ),
        Span::raw("Toggle Status | "),
        Span::styled("[↑/↓ or j/k] ", Style::default().fg(Color::Cyan)),
        Span::raw("Navigate"),
    ]);
    let action_bar = Paragraph::new(action_line).alignment(Alignment::Center);
    frame.render_widget(action_bar, action_bar_area);
}

fn draw_backtest(frame: &mut Frame, area: Rect, app: &TuiApp) {
    let rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
        .split(area);

    let top_row = rows[0];
    let bottom_row = rows[1];

    let top_quads = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
        .split(top_row);

    let bottom_quads = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
        .split(bottom_row);

    let q1_area = top_quads[0];
    let q2_area = top_quads[1];
    let q3_area = bottom_quads[0];
    let q4_area = bottom_quads[1];

    // Q1: Top Left - KPI Cards
    let q1_block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::DarkGray))
        .title(" Key Performance Indicators (KPIs) ");

    let mut kpi_lines = Vec::new();
    if let Some(ref r) = app.backtest_report {
        let pnl_color = if r.net_pnl >= Decimal::ZERO {
            Color::Green
        } else {
            Color::Red
        };
        let pnl_sign = if r.net_pnl >= Decimal::ZERO { "+" } else { "" };
        let ret_sign = if r.total_return_pct >= Decimal::ZERO {
            "+"
        } else {
            ""
        };

        kpi_lines.push(Line::from(""));
        kpi_lines.push(Line::from(vec![
            Span::styled("  Initial Capital: ", Style::default().fg(Color::DarkGray)),
            Span::styled(
                format!("₹{:.2}", r.initial_capital),
                Style::default()
                    .fg(Color::White)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled("   Final Equity: ", Style::default().fg(Color::DarkGray)),
            Span::styled(
                format!("₹{:.2}", r.final_equity),
                Style::default()
                    .fg(Color::White)
                    .add_modifier(Modifier::BOLD),
            ),
        ]));
        kpi_lines.push(Line::from(""));
        kpi_lines.push(Line::from(vec![
            Span::styled("  Net Profit/Loss: ", Style::default().fg(Color::DarkGray)),
            Span::styled(
                format!("{}{:.2}", pnl_sign, r.net_pnl),
                Style::default().fg(pnl_color).add_modifier(Modifier::BOLD),
            ),
            Span::styled("   Total Return: ", Style::default().fg(Color::DarkGray)),
            Span::styled(
                format!("{}{:.2}%", ret_sign, r.total_return_pct),
                Style::default().fg(pnl_color).add_modifier(Modifier::BOLD),
            ),
        ]));
        kpi_lines.push(Line::from(""));
        kpi_lines.push(Line::from(vec![
            Span::styled("  Sharpe Ratio:    ", Style::default().fg(Color::DarkGray)),
            Span::styled(
                format!("{:.2}", r.sharpe_ratio),
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled("   Max Drawdown: ", Style::default().fg(Color::DarkGray)),
            Span::styled(
                format!("{:.2}%", r.max_drawdown_pct),
                Style::default()
                    .fg(Color::Yellow)
                    .add_modifier(Modifier::BOLD),
            ),
        ]));
        kpi_lines.push(Line::from(""));
        kpi_lines.push(Line::from(vec![
            Span::styled("  Total Trades:    ", Style::default().fg(Color::DarkGray)),
            Span::styled(
                format!("{}", r.total_trades),
                Style::default().fg(Color::White),
            ),
            Span::styled(
                "         Win Rate:     ",
                Style::default().fg(Color::DarkGray),
            ),
            Span::styled(
                format!("{:.2}%", r.win_rate_pct),
                Style::default().fg(Color::Green),
            ),
            Span::styled(" | Profit Factor: ", Style::default().fg(Color::DarkGray)),
            Span::styled(
                format!("{:.2}", r.profit_factor),
                Style::default().fg(Color::White),
            ),
        ]));
    } else {
        kpi_lines.push(Line::from(
            "  No backtest loaded. Press [Enter] on an algorithm or type /backtest.",
        ));
    }
    let p1 = Paragraph::new(kpi_lines).block(q1_block);
    frame.render_widget(p1, q1_area);

    // Q2: Top Right - Parameter Delta Comparison Table
    let q2_block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::DarkGray))
        .title(" Parameter Delta (Default vs Tuned) ");

    let header2 = Row::new(vec!["Parameter", "Default", "Tuned Value", "Status"]).style(
        Style::default()
            .fg(Color::Cyan)
            .add_modifier(Modifier::BOLD),
    );
    let mut rows2 = Vec::new();
    if let Some(ref opt) = app.optimization_result {
        for (k, v_tuned) in &opt.tuned_params {
            let v_def = opt.default_params.get(k).copied().unwrap_or(*v_tuned);
            let is_diff = v_tuned != &v_def;
            let val_style = if is_diff {
                Style::default()
                    .fg(Color::Green)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(Color::White)
            };
            rows2.push(
                Row::new(vec![
                    k.clone(),
                    format!("{v_def}"),
                    format!("{v_tuned}"),
                    if is_diff {
                        "Tuned (Optimal)".to_string()
                    } else {
                        "Unchanged".to_string()
                    },
                ])
                .style(val_style),
            );
        }
    }
    let widths2 = [
        Constraint::Percentage(35),
        Constraint::Percentage(20),
        Constraint::Percentage(25),
        Constraint::Percentage(20),
    ];
    let t2 = Table::new(rows2, widths2).header(header2).block(q2_block);
    frame.render_widget(t2, q2_area);

    // Q3: Bottom Left - ASCII / Braille Equity Curve & Drawdown Chart (Sparkline)
    let q3_block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::DarkGray))
        .title(" Analytical Equity Curve (Sparkline) ");

    let sparkline_data = &app.backtest_equity_curve;
    let sparkline = Sparkline::default()
        .block(q3_block)
        .style(Style::default().fg(Color::Green))
        .data(sparkline_data);
    frame.render_widget(sparkline, q3_area);

    // Q4: Bottom Right - Optimal Trade Suggestion Card
    let q4_block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Cyan))
        .title(" Optimal Trade Suggestion ");

    let mut sugg_lines = Vec::new();
    if let Some(ref s) = app.trade_suggestion {
        sugg_lines.push(Line::from(vec![
            Span::styled("Asset: ", Style::default().fg(Color::DarkGray)),
            Span::styled(
                &s.symbol,
                Style::default()
                    .fg(Color::White)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled(" | Bias: ", Style::default().fg(Color::DarkGray)),
            Span::styled(
                s.signal.bias.as_str(),
                Style::default()
                    .fg(Color::Green)
                    .add_modifier(Modifier::BOLD),
            ),
        ]));
        sugg_lines.push(Line::from(vec![
            Span::styled("Trigger: ", Style::default().fg(Color::DarkGray)),
            Span::styled(&s.signal.reason, Style::default().fg(Color::Yellow)),
        ]));
        sugg_lines.push(Line::from(vec![
            Span::styled("Entry: ", Style::default().fg(Color::DarkGray)),
            Span::styled(
                format!("₹{:.2}", s.signal.entry_price),
                Style::default()
                    .fg(Color::White)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled(" | Stop Loss: ", Style::default().fg(Color::DarkGray)),
            Span::styled(
                format!("₹{:.2}", s.signal.stop_loss),
                Style::default().fg(Color::Red),
            ),
        ]));
        sugg_lines.push(Line::from(vec![
            Span::styled("Target 1 (1:2): ", Style::default().fg(Color::DarkGray)),
            Span::styled(
                format!("₹{:.2}", s.signal.target_1),
                Style::default().fg(Color::Green),
            ),
            Span::styled(" | Target 2 (1:3): ", Style::default().fg(Color::DarkGray)),
            Span::styled(
                format!("₹{:.2}", s.signal.target_2),
                Style::default().fg(Color::Green),
            ),
        ]));
        sugg_lines.push(Line::from(vec![
            Span::styled("Sized Qty: ", Style::default().fg(Color::DarkGray)),
            Span::styled(
                format!("{} units", s.sizing.quantity),
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled(" | Margin: ", Style::default().fg(Color::DarkGray)),
            Span::styled(
                format!(
                    "₹{:.2} ({:.1}%)",
                    s.sizing.notional_value, s.sizing.margin_utilization_pct
                ),
                Style::default().fg(Color::White),
            ),
        ]));
        sugg_lines.push(Line::from(vec![
            Span::styled("Friction Fee: ", Style::default().fg(Color::DarkGray)),
            Span::styled(
                format!("₹{:.2}", s.statutory_charges.total_charges),
                Style::default().fg(Color::DarkGray),
            ),
            Span::styled(" | Net R:R: ", Style::default().fg(Color::DarkGray)),
            Span::styled(
                format!("1 : {:.2}", s.statutory_charges.net_risk_reward_ratio),
                Style::default()
                    .fg(Color::LightGreen)
                    .add_modifier(Modifier::BOLD),
            ),
        ]));
    } else {
        sugg_lines.push(Line::from(
            "  No trade suggestion computed. Press [S] on an algo or type /suggest.",
        ));
    }
    let p4 = Paragraph::new(sugg_lines)
        .block(q4_block)
        .wrap(Wrap { trim: true });
    frame.render_widget(p4, q4_area);
}
