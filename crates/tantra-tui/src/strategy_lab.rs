//! Interactive `Strategy Lab / Chart` renderer (`Track T21.4`).
//!
//! Renders the 6-layer unified [`OverlayScene`] in `ratatui::widgets::canvas::Canvas`
//! and sub-pane [`Sparkline`]s alongside the cursor-steppable Hint Inspector,
//! Penetration histogram, Stop-Hunt Benchmark, IS/OOS + DSR Sweep Leaderboard,
//! and F&O / Commodity Expiry Hedge Card.

use ratatui::Frame;
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::symbols::Marker;
use ratatui::text::{Line, Span};
use ratatui::widgets::canvas::{Canvas, Line as CanvasLine, Rectangle};
use ratatui::widgets::{Paragraph, Row, Sparkline, Table, Wrap};
use rust_decimal::Decimal;
use rust_decimal::prelude::ToPrimitive;
use tantra_ui_charts::scene::HintSeverity;
use tantra_ui_core::{CanvasStrokeKind, CanvasTone, StrategyLoopViewModel, format_signed_r};

use crate::app::{StrategyLabDetailView, TuiApp};
use crate::theme;

/// Convert an exact `Decimal` solely at the final `ratatui::widgets::canvas::Canvas`
/// coordinate boundary. All domain state remains pure `Decimal`.
fn coord_f64(value: Decimal) -> f64 {
    value.to_f64().unwrap_or(0.0)
}

fn tone_to_color(tone: CanvasTone) -> Color {
    match tone {
        CanvasTone::Positive => theme::SUCCESS,
        CanvasTone::Negative => theme::ERROR,
        CanvasTone::Accent => theme::ACCENT,
        CanvasTone::Warning => theme::WARNING,
        CanvasTone::Critical => theme::ERROR,
        CanvasTone::Neutral => theme::TEXT,
        CanvasTone::Muted => theme::TEXT_MUTED,
    }
}

/// Draw the full Strategy Lab / Chart workstation view.
pub fn draw_strategy_lab(frame: &mut Frame, area: Rect, app: &TuiApp) {
    let lab = &app.strategy_lab;
    let vm = &lab.vm;

    let outer = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(2), // Strategy parameter HUD + [1..6] Layer Pill Bar
            Constraint::Min(10),   // Main Canvas + Sub-pane + Hint/Penetration Sidebar
            Constraint::Length(9), // Benchmark / Sweep / Hedges Detail Panel
            Constraint::Length(1), // Interactive controls legend
        ])
        .split(area);

    draw_hud_and_layers(frame, outer[0], vm, lab.detail_view);

    let middle = Layout::default()
        .direction(Direction::Horizontal)
        .spacing(1)
        .constraints([Constraint::Percentage(68), Constraint::Percentage(32)])
        .split(outer[1]);

    draw_chart_and_subpane(frame, middle[0], vm, lab.selected_hint_index);
    draw_inspector_sidebar(frame, middle[1], vm, lab.selected_hint_index);
    draw_detail_panel(frame, outer[2], vm, lab.detail_view);
    draw_controls_legend(frame, outer[3], lab.detail_view);
}

fn draw_hud_and_layers(
    frame: &mut Frame,
    area: Rect,
    vm: &StrategyLoopViewModel,
    detail_view: StrategyLabDetailView,
) {
    let view_tag = match detail_view {
        StrategyLabDetailView::Benchmark => "Benchmark",
        StrategyLabDetailView::Sweep => "Sweep(IS/OOS+DSR)",
        StrategyLabDetailView::Hedges => "HedgeCard",
    };

    let mut header_spans = vec![
        Span::styled(
            format!(" {} ", vm.config.strategy_id),
            Style::default()
                .fg(theme::BACKGROUND)
                .bg(theme::PRIMARY)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled(
            format!(" {} ({}) ", vm.config.symbol, vm.config.timeframe),
            Style::default()
                .fg(theme::TEXT)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled(
            format!(
                "Spot ₹{} | SL +{}xATR | Entry:{} | Exp:{} | Net {} | Trap {}% | [{}] ",
                vm.spot_price.round_dp(2),
                vm.config.sl_buffer_atr.round_dp(2),
                vm.config.entry_trigger_mode,
                vm.config.expiry_horizon,
                format_signed_r(vm.net_avg_realized_r),
                vm.false_sweep_rate_pct.round_dp(1),
                view_tag,
            ),
            Style::default().fg(theme::WARNING),
        ),
    ];

    if let Some(ref d) = vm.delta_summary {
        let delta_color = if d.avg_r_delta >= Decimal::ZERO {
            theme::SUCCESS
        } else {
            theme::ERROR
        };
        header_spans.push(Span::styled(
            format!("ΔR {} ", format_signed_r(d.avg_r_delta)),
            Style::default()
                .fg(delta_color)
                .add_modifier(Modifier::BOLD),
        ));
    }

    header_spans.push(Span::styled(
        format!("[{}]", vm.data_source),
        Style::default().fg(theme::TEXT_MUTED),
    ));

    let mut layer_spans = vec![Span::styled(
        " Layers: ",
        Style::default()
            .fg(theme::TEXT_MUTED)
            .add_modifier(Modifier::BOLD),
    )];
    for pill in &vm.projected.layer_pills {
        let state_dot = if pill.active { "●" } else { "○" };
        let style = if pill.active {
            Style::default()
                .fg(theme::BACKGROUND)
                .bg(theme::SUCCESS)
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(theme::TEXT_MUTED)
        };
        layer_spans.push(Span::styled(
            format!(
                "[{}:{}({}){state_dot}]",
                pill.key_digit, pill.title, pill.count
            ),
            style,
        ));
        layer_spans.push(Span::raw(" "));
    }

    let p = Paragraph::new(vec![Line::from(header_spans), Line::from(layer_spans)]);
    frame.render_widget(p, area);
}

fn draw_chart_and_subpane(
    frame: &mut Frame,
    area: Rect,
    vm: &StrategyLoopViewModel,
    selected_hint_index: usize,
) {
    let split = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Percentage(75), Constraint::Percentage(25)])
        .split(area);

    draw_price_canvas(frame, split[0], vm, selected_hint_index);
    draw_subpane_indicators(frame, split[1], vm);
}

fn draw_price_canvas(
    frame: &mut Frame,
    area: Rect,
    vm: &StrategyLoopViewModel,
    selected_hint_index: usize,
) {
    let proj = &vm.projected;
    let block = theme::panel_block(
        &format!(
            "Canvas: {} ({}) | y: {:.0}..{:.0}",
            vm.config.symbol,
            vm.config.timeframe,
            coord_f64(proj.y_min),
            coord_f64(proj.y_max),
        ),
        theme::PRIMARY,
    );

    let x_min = coord_f64(proj.x_min);
    let x_max = coord_f64(proj.x_max);
    let y_min = coord_f64(proj.y_min);
    let y_max = coord_f64(proj.y_max);
    let active_hint_bar = vm.hints.get(selected_hint_index).map(|h| h.bar_index);

    let canvas = Canvas::default()
        .block(block)
        .marker(Marker::Braille)
        .x_bounds([x_min, x_max])
        .y_bounds([y_min, y_max])
        .paint(|ctx| {
            // Layer 2: Contextual zone bands & equilibrium levels
            for band in &proj.bands {
                let bx0 = coord_f64(band.x0);
                let bx1 = coord_f64(band.x1);
                let by0 = coord_f64(band.y0);
                let by1 = coord_f64(band.y1);
                let color = if band.label.as_deref().unwrap_or("").starts_with('R')
                    || band.label.as_deref() == Some("OR_HIGH")
                {
                    theme::ERROR
                } else {
                    theme::SUCCESS
                };
                ctx.draw(&Rectangle {
                    x: bx0,
                    y: by0,
                    width: (bx1 - bx0).max(0.5),
                    height: (by1 - by0).max(0.01),
                    color,
                });
                if let Some(ref lbl) = band.label {
                    ctx.print(
                        bx0 + 0.3,
                        (by0 + by1) / 2.0,
                        Span::styled(lbl.clone(), Style::default().fg(color)),
                    );
                }
            }

            for lvl in &proj.levels {
                let color = tone_to_color(lvl.tone);
                ctx.draw(&CanvasLine {
                    x1: coord_f64(lvl.x0),
                    y1: coord_f64(lvl.y0),
                    x2: coord_f64(lvl.x1),
                    y2: coord_f64(lvl.y1),
                    color,
                });
                if let Some(ref lbl) = lvl.label {
                    ctx.print(
                        coord_f64(lvl.x0) + 0.5,
                        coord_f64(lvl.y0),
                        Span::styled(lbl.clone(), Style::default().fg(color)),
                    );
                }
            }

            // Layer 4 (Main-pane overlay): EMA / VWAP
            for trace in &proj.overlay_traces {
                for &(x0, y0, x1, y1) in &trace.segments {
                    ctx.draw(&CanvasLine {
                        x1: coord_f64(x0),
                        y1: coord_f64(y0),
                        x2: coord_f64(x1),
                        y2: coord_f64(y1),
                        color: theme::PRIMARY,
                    });
                }
            }

            ctx.layer();

            // Candlesticks (wicks + bodies)
            for c in &proj.candles {
                let x = c.index as f64;
                let color = if c.is_bullish {
                    theme::SUCCESS
                } else {
                    theme::ERROR
                };
                ctx.draw(&CanvasLine {
                    x1: x,
                    y1: coord_f64(c.low),
                    x2: x,
                    y2: coord_f64(c.high),
                    color,
                });
                let body_lo = coord_f64(c.open.min(c.close));
                let body_hi = coord_f64(c.open.max(c.close));
                ctx.draw(&Rectangle {
                    x: x - 0.28,
                    y: body_lo,
                    width: 0.56,
                    height: (body_hi - body_lo).max((y_max - y_min) * 0.008),
                    color,
                });
            }

            // Layer 3: Stops & Targets (`─SL─`, `─Outer SL─`, `─3R─`)
            for st in &proj.stops {
                let color = tone_to_color(st.tone);
                let x0 = coord_f64(st.x0);
                let x1 = coord_f64(st.x1);
                let y = coord_f64(st.y0);
                if st.stroke == CanvasStrokeKind::StopTightRef {
                    // Dotted/segmented tight reference line
                    let mut cur = x0;
                    while cur < x1 {
                        let next = (cur + 0.6).min(x1);
                        ctx.draw(&CanvasLine {
                            x1: cur,
                            y1: y,
                            x2: next,
                            y2: y,
                            color,
                        });
                        cur += 1.2;
                    }
                } else {
                    ctx.draw(&CanvasLine {
                        x1: x0,
                        y1: y,
                        x2: x1,
                        y2: y,
                        color,
                    });
                }
                if let Some(ref lbl) = st.label {
                    ctx.print(
                        (x1 - 2.0).max(x0),
                        y,
                        Span::styled(lbl.clone(), Style::default().fg(color)),
                    );
                }
            }

            // Layer 6: Hedge strike levels & breakevens
            for h in &proj.hedges {
                let color = tone_to_color(h.tone);
                ctx.draw(&CanvasLine {
                    x1: coord_f64(h.x0),
                    y1: coord_f64(h.y0),
                    x2: coord_f64(h.x1),
                    y2: coord_f64(h.y1),
                    color,
                });
                if let Some(ref lbl) = h.label {
                    ctx.print(
                        coord_f64(h.x0) + 1.0,
                        coord_f64(h.y0),
                        Span::styled(lbl.clone(), Style::default().fg(color)),
                    );
                }
            }

            // Layer 1b: Trade entry-to-exit connector rays
            for ray in &proj.connectors {
                ctx.draw(&CanvasLine {
                    x1: coord_f64(ray.x0),
                    y1: coord_f64(ray.y0),
                    x2: coord_f64(ray.x1),
                    y2: coord_f64(ray.y1),
                    color: tone_to_color(ray.tone),
                });
            }

            // Layer 1a: Signals (`▲`, `▼`, `◆`, `✓`, `★`)
            for sig in &proj.signals {
                let color = tone_to_color(sig.tone);
                ctx.print(
                    coord_f64(sig.x),
                    coord_f64(sig.y),
                    Span::styled(
                        sig.glyph.clone(),
                        Style::default().fg(color).add_modifier(Modifier::BOLD),
                    ),
                );
            }

            // Layer 5: Hint pins (`●`, `★`, `▲`, `✖`) with active cursor marker `▶`
            for h in &proj.hints {
                let is_selected = active_hint_bar == Some(h.bar_index);
                let txt = if is_selected {
                    format!("▶{}", h.glyph)
                } else {
                    h.glyph.clone()
                };
                let style = if is_selected {
                    Style::default()
                        .fg(theme::BACKGROUND)
                        .bg(theme::WARNING)
                        .add_modifier(Modifier::BOLD)
                } else {
                    Style::default().fg(tone_to_color(h.tone))
                };
                ctx.print(
                    coord_f64(h.x),
                    coord_f64(h.y) + (y_max - y_min) * 0.03,
                    Span::styled(txt, style),
                );
            }
        });

    frame.render_widget(canvas, area);
}

fn draw_subpane_indicators(frame: &mut Frame, area: Rect, vm: &StrategyLoopViewModel) {
    let traces = &vm.projected.subpane_traces;
    if !vm.scene.layers.indicators || traces.is_empty() {
        let msg = if !vm.scene.layers.indicators {
            "Indicators layer toggled OFF — press [4] to enable EMA / RSI / ATR sub-pane."
        } else {
            "No sub-pane indicator series populated."
        };
        let block = theme::panel_block("Sub-Pane Indicators (Sparkline)", theme::ACCENT);
        frame.render_widget(
            Paragraph::new(msg)
                .style(Style::default().fg(theme::TEXT_MUTED))
                .block(block),
            area,
        );
        return;
    }

    let pane_count = traces.len().min(2);
    let constraints = if pane_count == 2 {
        vec![Constraint::Percentage(50), Constraint::Percentage(50)]
    } else {
        vec![Constraint::Percentage(100)]
    };
    let panes = Layout::default()
        .direction(Direction::Horizontal)
        .constraints(constraints)
        .split(area);

    for (idx, tr) in traces.iter().take(2).enumerate() {
        let latest_str = tr
            .latest_value
            .map(|v| v.round_dp(2).to_string())
            .unwrap_or_else(|| "-".to_string());
        let color = if tr.name.to_uppercase().contains("RSI") {
            theme::INFO
        } else {
            theme::PRIMARY
        };
        let spark = Sparkline::default()
            .block(theme::panel_block(
                &format!("{}: {}", tr.name, latest_str),
                theme::ACCENT,
            ))
            .style(Style::default().fg(color))
            .max(100)
            .data(&tr.sparkline_bins);
        frame.render_widget(spark, panes[idx]);
    }
}

fn draw_inspector_sidebar(
    frame: &mut Frame,
    area: Rect,
    vm: &StrategyLoopViewModel,
    selected_hint_index: usize,
) {
    let split = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Percentage(58), Constraint::Percentage(42)])
        .split(area);

    // Top: Cursor-Steppable Hint Inspector
    let total_hints = vm.hints.len();
    let title = if total_hints == 0 {
        "Hint Inspector (0/0)".to_string()
    } else {
        format!(
            "Hint Inspector [←/→ p/n] ({}/{total_hints})",
            selected_hint_index.min(total_hints - 1) + 1
        )
    };
    let hint_block = theme::panel_block(&title, theme::ACCENT);

    let mut lines = Vec::new();
    if !vm.scene.layers.hints {
        lines.push(Line::from(Span::styled(
            "Hints layer hidden — press [5] to show pins.",
            Style::default().fg(theme::TEXT_MUTED),
        )));
    } else if let Some(h) = vm.hints.get(selected_hint_index) {
        let (sev_tag, sev_color) = match h.severity {
            HintSeverity::Positive => ("[POSITIVE]", theme::SUCCESS),
            HintSeverity::Warning => ("[WARNING]", theme::WARNING),
            HintSeverity::Critical => ("[CRITICAL]", theme::ERROR),
            HintSeverity::Info => ("[INFO]", theme::PRIMARY),
        };
        lines.push(Line::from(vec![
            Span::styled(
                format!("{sev_tag} "),
                Style::default().fg(sev_color).add_modifier(Modifier::BOLD),
            ),
            Span::styled(
                format!("Bar #{} @ ₹{}", h.bar_index, h.price.round_dp(2)),
                Style::default()
                    .fg(theme::TEXT)
                    .add_modifier(Modifier::BOLD),
            ),
        ]));
        lines.push(Line::from(Span::styled(
            h.title.clone(),
            Style::default().fg(sev_color).add_modifier(Modifier::BOLD),
        )));
        lines.push(Line::from(Span::raw(h.detail.clone())));
        if !h.metrics.is_empty() {
            lines.push(Line::from(""));
            for m in &h.metrics {
                lines.push(Line::from(vec![
                    Span::styled(
                        format!(" • {}: ", m.key),
                        Style::default().fg(theme::TEXT_MUTED),
                    ),
                    Span::styled(m.value.clone(), Style::default().fg(theme::PRIMARY)),
                ]));
            }
        }
    } else {
        lines.push(Line::from("No rationale hint annotations in scene."));
    }

    frame.render_widget(
        Paragraph::new(lines)
            .block(hint_block)
            .wrap(Wrap { trim: true }),
        split[0],
    );

    // Bottom: Penetration Beyond Outer Edge Histogram
    let pen = &vm.penetration;
    let pen_block = theme::panel_block(
        &format!(
            "Excursion >Outer ({} setups | {}% rec)",
            pen.sample_count, pen.pct_recovering_after_outer_touch
        ),
        theme::ACCENT,
    );
    let mut pen_lines = Vec::new();
    for b in &pen.buckets {
        pen_lines.push(Line::from(vec![
            Span::styled(
                format!(" {:<4}", b.percentile),
                Style::default().fg(theme::PRIMARY),
            ),
            Span::styled(
                format!("{:>4}x ", b.atr_value.round_dp(2)),
                Style::default().fg(theme::TEXT),
            ),
            Span::styled(b.bar_ascii.clone(), Style::default().fg(theme::WARNING)),
        ]));
    }
    frame.render_widget(Paragraph::new(pen_lines).block(pen_block), split[1]);
}

fn draw_detail_panel(
    frame: &mut Frame,
    area: Rect,
    vm: &StrategyLoopViewModel,
    detail_view: StrategyLabDetailView,
) {
    match detail_view {
        StrategyLabDetailView::Benchmark => {
            let block = theme::panel_block(
                &format!("4-Policy Stop Benchmark [m: Next] — {}", vm.verdict_summary),
                theme::ACCENT,
            );
            let header = Row::new(vec![
                "Policy", "Buf", "Trades", "Win%", "StopOut%", "Trap%", "NetAvgR", "PF", "Sharpe",
                "MaxDD",
            ])
            .style(
                Style::default()
                    .fg(theme::PRIMARY)
                    .add_modifier(Modifier::BOLD),
            );
            let rows: Vec<Row> = vm
                .stop_comparison
                .iter()
                .map(|r| {
                    let prefix = if r.is_recommended { "★ " } else { "  " };
                    let style = if r.is_recommended {
                        Style::default()
                            .fg(theme::SUCCESS)
                            .add_modifier(Modifier::BOLD)
                    } else {
                        Style::default().fg(theme::TEXT)
                    };
                    Row::new(vec![
                        format!("{prefix}{}", r.label),
                        format!("{}x", r.buffer_atr.round_dp(2)),
                        r.trades.to_string(),
                        format!("{}%", r.win_rate_pct.round_dp(1)),
                        format!("{}%", r.stop_out_rate_pct.round_dp(1)),
                        format!("{}%", r.false_sweep_rate_pct.round_dp(1)),
                        format_signed_r(r.net_avg_realized_r),
                        r.profit_factor.round_dp(2).to_string(),
                        r.sharpe_ratio.round_dp(2).to_string(),
                        format!("{}R", r.max_drawdown_r.round_dp(2)),
                    ])
                    .style(style)
                })
                .collect();
            let widths = [
                Constraint::Percentage(22),
                Constraint::Percentage(8),
                Constraint::Percentage(7),
                Constraint::Percentage(8),
                Constraint::Percentage(10),
                Constraint::Percentage(8),
                Constraint::Percentage(11),
                Constraint::Percentage(8),
                Constraint::Percentage(9),
                Constraint::Percentage(9),
            ];
            let table = Table::new(rows, widths).header(header).block(block);
            frame.render_widget(table, area);
        }
        StrategyLabDetailView::Sweep => {
            let block = theme::panel_block(
                "Parameter Sweep Leaderboard (IS 70% / OOS 30% + WFE + DSR) [m: Next]",
                theme::ACCENT,
            );
            let header = Row::new(vec![
                "Rank", "Cand", "Buf", "Horiz", "TgtR", "IS Shrp", "OOS Shrp", "OOS AvgR", "WFE%",
                "DSR", "Δ Base",
            ])
            .style(
                Style::default()
                    .fg(theme::PRIMARY)
                    .add_modifier(Modifier::BOLD),
            );
            let rows: Vec<Row> = vm
                .sweep_rows
                .iter()
                .map(|r| {
                    let style = if r.is_winner {
                        Style::default()
                            .fg(theme::SUCCESS)
                            .add_modifier(Modifier::BOLD)
                    } else {
                        Style::default().fg(theme::TEXT)
                    };
                    Row::new(vec![
                        if r.is_winner {
                            format!("★{}", r.rank)
                        } else {
                            format!(" {}", r.rank)
                        },
                        r.candidate_id.clone(),
                        format!("{}x", r.sl_buffer_atr.round_dp(2)),
                        format!("{}b", r.horizon_bars),
                        format!("{}R", r.target_r.round_dp(0)),
                        r.is_sharpe.round_dp(2).to_string(),
                        r.oos_sharpe.round_dp(2).to_string(),
                        format_signed_r(r.oos_avg_r),
                        format!("{}%", r.wfe_pct.round_dp(1)),
                        r.dsr.round_dp(2).to_string(),
                        format_signed_r(r.delta_vs_baseline_r),
                    ])
                    .style(style)
                })
                .collect();
            let widths = [
                Constraint::Percentage(6),
                Constraint::Percentage(11),
                Constraint::Percentage(8),
                Constraint::Percentage(8),
                Constraint::Percentage(8),
                Constraint::Percentage(10),
                Constraint::Percentage(10),
                Constraint::Percentage(11),
                Constraint::Percentage(9),
                Constraint::Percentage(9),
                Constraint::Percentage(10),
            ];
            let table = Table::new(rows, widths).header(header).block(block);
            frame.render_widget(table, area);
        }
        StrategyLabDetailView::Hedges => {
            let hc = &vm.hedge_card;
            let block = theme::panel_block(
                &format!(
                    "Expiry Hedge Card [x: Cycle Horizon | m: Next] — {} | {} ({})",
                    hc.horizon, hc.structure, hc.chain_status
                ),
                theme::ACCENT,
            );
            let mut lines = Vec::new();
            for leg in &hc.legs {
                lines.push(Line::from(vec![
                    Span::styled(
                        format!("  {:<4} {:<3} ", leg.side, leg.option_type),
                        Style::default()
                            .fg(if leg.side == "BUY" {
                                theme::PRIMARY
                            } else {
                                theme::WARNING
                            })
                            .add_modifier(Modifier::BOLD),
                    ),
                    Span::styled(
                        format!("@ ₹{:<9} ", leg.strike.round_dp(2)),
                        Style::default().fg(theme::TEXT),
                    ),
                    Span::styled(
                        format!(
                            "lot {} | px ₹{} | exp {} ({})",
                            leg.lot_qty.round_dp(0),
                            leg.leg_price.round_dp(2),
                            leg.expiry.as_deref().unwrap_or("nearest"),
                            leg.quote_source
                        ),
                        Style::default().fg(theme::TEXT_MUTED),
                    ),
                ]));
            }
            lines.push(Line::from(vec![
                Span::styled("  Risk: ", Style::default().fg(theme::TEXT_MUTED)),
                Span::styled(
                    format!(
                        "MaxLoss ₹{} ({}% cap) | P95 Stress ₹{} | Net Cr/Dr ₹{} | Friction ₹{}",
                        hc.max_loss_inr.round_dp(2),
                        hc.max_loss_pct_equity.round_dp(2),
                        hc.p95_stress_loss_inr.round_dp(2),
                        hc.net_credit_or_debit.round_dp(2),
                        hc.estimated_friction_inr.round_dp(2),
                    ),
                    Style::default().fg(theme::WARNING),
                ),
            ]));
            lines.push(Line::from(vec![
                Span::styled("  Why:  ", Style::default().fg(theme::TEXT_MUTED)),
                Span::styled(hc.rationale.clone(), Style::default().fg(theme::SUCCESS)),
            ]));
            frame.render_widget(
                Paragraph::new(lines).block(block).wrap(Wrap { trim: true }),
                area,
            );
        }
    }
}

fn draw_controls_legend(frame: &mut Frame, area: Rect, detail_view: StrategyLabDetailView) {
    let view_label = match detail_view {
        StrategyLabDetailView::Benchmark => "Benchmark→Sweep",
        StrategyLabDetailView::Sweep => "Sweep→Hedge",
        StrategyLabDetailView::Hedges => "Hedge→Benchmark",
    };
    let line = Line::from(vec![
        Span::styled(
            " [1-6] ",
            Style::default()
                .fg(theme::SUCCESS)
                .add_modifier(Modifier::BOLD),
        ),
        Span::raw("Layers | "),
        Span::styled(
            "[+/-] ",
            Style::default()
                .fg(theme::WARNING)
                .add_modifier(Modifier::BOLD),
        ),
        Span::raw("SL Buf | "),
        Span::styled(
            "[s] ",
            Style::default()
                .fg(theme::WARNING)
                .add_modifier(Modifier::BOLD),
        ),
        Span::raw("Strat | "),
        Span::styled(
            "[a] ",
            Style::default()
                .fg(theme::WARNING)
                .add_modifier(Modifier::BOLD),
        ),
        Span::raw("Asset | "),
        Span::styled(
            "[t] ",
            Style::default()
                .fg(theme::WARNING)
                .add_modifier(Modifier::BOLD),
        ),
        Span::raw("TF | "),
        Span::styled(
            "[e] ",
            Style::default()
                .fg(theme::WARNING)
                .add_modifier(Modifier::BOLD),
        ),
        Span::raw("Entry | "),
        Span::styled(
            "[x] ",
            Style::default()
                .fg(theme::WARNING)
                .add_modifier(Modifier::BOLD),
        ),
        Span::raw("Expiry | "),
        Span::styled(
            "[m] ",
            Style::default()
                .fg(theme::PRIMARY)
                .add_modifier(Modifier::BOLD),
        ),
        Span::raw(format!("{view_label} | ")),
        Span::styled(
            "[←/→ p/n] ",
            Style::default()
                .fg(theme::PRIMARY)
                .add_modifier(Modifier::BOLD),
        ),
        Span::raw("Hint"),
    ]);
    frame.render_widget(Paragraph::new(line), area);
}
