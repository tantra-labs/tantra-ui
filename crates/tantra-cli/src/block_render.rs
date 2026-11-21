//! Pure string rendering of MCP gateway result blocks into terminal text.
//!
//! [`render_blocks`] turns `tantra_agent_core::result::DisplayBlock` values
//! (tables, metric grids, charts, alerts) into ANSI/box-drawing text;
//! [`render_envelope`] renders a `TantraResultEnvelope` JSON value through the
//! same path when it carries blocks. No new dependencies: plain string
//! building, using `rust_decimal` (already a tantra-cli dependency) purely to
//! detect numeric columns for right-alignment.

use std::str::FromStr;

use rust_decimal::Decimal;
use serde_json::Value;
use tantra_agent_core::result::{DisplayBlock, MetricItem};

const BOLD: &str = "\x1b[1m";
const RESET: &str = "\x1b[0m";
const GREEN: &str = "\x1b[32m";
const RED: &str = "\x1b[31m";
const YELLOW: &str = "\x1b[33m";
const CYAN: &str = "\x1b[36m";

/// True when `cell` parses as a fixed-point decimal (used to right-align
/// numeric money/ratio columns in tables).
fn is_decimal(cell: &str) -> bool {
    Decimal::from_str(cell).is_ok()
}

/// Render all blocks, each followed by a blank line.
pub fn render_blocks(blocks: &[DisplayBlock]) -> String {
    let mut out = String::new();
    for block in blocks {
        match block {
            DisplayBlock::Table {
                title,
                columns,
                rows,
            } => out.push_str(&render_table(title, columns, rows)),
            DisplayBlock::MetricGrid { items } => out.push_str(&render_metric_grid(items)),
            DisplayBlock::Chart {
                title,
                chart_type,
                series_ref,
            } => out.push_str(&render_chart(title, chart_type, series_ref)),
            DisplayBlock::Alert { level, message } => out.push_str(&render_alert(level, message)),
            DisplayBlock::OverlayScene {
                title,
                symbol,
                timeframe,
                scene,
                ..
            } => out.push_str(&render_overlay_scene(title, symbol, timeframe, scene)),
            DisplayBlock::View {
                title,
                summary,
                highlights,
                ..
            } => {
                out.push_str(&format!("## {title}\n"));
                if !summary.is_empty() {
                    out.push_str(summary);
                    out.push('\n');
                }
                for h in highlights {
                    out.push_str(&format!("{}: {}\n", h.label, h.value));
                }
            }
        }
        out.push('\n');
    }
    out
}

/// Render a `TantraResultEnvelope` JSON value: the summary line plus any
/// display blocks; falls back to pretty JSON when the value carries no blocks
/// or the blocks cannot be decoded.
pub fn render_envelope(value: &Value) -> String {
    if let Some(blocks) = value
        .get("blocks")
        .and_then(|b| b.as_array())
        .filter(|blocks| !blocks.is_empty())
    {
        if let Ok(blocks) =
            serde_json::from_value::<Vec<DisplayBlock>>(serde_json::Value::Array(blocks.clone()))
        {
            let mut out = String::new();
            if let Some(summary) = value.get("summary").and_then(|s| s.as_str()) {
                out.push_str(summary);
                out.push('\n');
            }
            out.push_str(&render_blocks(&blocks));
            return out;
        }
    }
    serde_json::to_string_pretty(value).unwrap_or_else(|_| value.to_string())
}

fn render_table(title: &str, columns: &[String], rows: &[Vec<String>]) -> String {
    // Per-column width = max(header length, widest cell length).
    let widths = columns
        .iter()
        .enumerate()
        .map(|(i, column)| {
            let mut width = column.chars().count();
            for row in rows {
                if let Some(cell) = row.get(i) {
                    width = width.max(cell.chars().count());
                }
            }
            width
        })
        .collect::<Vec<_>>();
    // Right-align a column when every cell parses as a Decimal (money/ratio
    // columns from the gateway).
    let right_align = columns
        .iter()
        .enumerate()
        .map(|(i, _)| {
            rows.iter()
                .all(|row| row.get(i).is_none_or(|cell| is_decimal(cell)))
        })
        .collect::<Vec<_>>();

    let mut out = String::new();
    if !title.is_empty() {
        out.push_str(title);
        out.push('\n');
    }
    out.push_str(&border_line(&widths, '┌', '┬', '┐'));
    out.push('\n');
    out.push_str(&header_line(columns, &widths, &right_align));
    out.push('\n');
    out.push_str(&border_line(&widths, '├', '┼', '┤'));
    out.push('\n');
    for row in rows {
        let cells = (0..columns.len())
            .map(|i| row.get(i).map(String::as_str).unwrap_or(""))
            .collect::<Vec<_>>();
        out.push_str(&row_line(&cells, &widths, &right_align));
        out.push('\n');
    }
    out.push_str(&border_line(&widths, '└', '┴', '┘'));
    out
}

fn render_metric_grid(items: &[MetricItem]) -> String {
    let mut out = String::new();
    for item in items {
        let color = if item.value.starts_with('+') {
            GREEN
        } else if item.value.starts_with('-') {
            RED
        } else {
            CYAN
        };
        out.push_str(&format!("{}: {color}{}{RESET}\n", item.label, item.value));
    }
    out
}

fn render_chart(title: &str, chart_type: &str, series_ref: &str) -> String {
    // series refs are gateway-side data-store URIs (tantra-agent-mcp
    // DataReferenceStore); CLI renders the reference honestly — plan 0003
    // Phase 1.
    format!("📈 {title} ({chart_type}) — series {series_ref}\n")
}

/// Count of one scene layer array (`signals`, `bands`, ...); 0 when absent.
fn scene_layer_len(scene: &serde_json::Value, key: &str) -> usize {
    scene
        .get(key)
        .and_then(|v| v.as_array())
        .map_or(0, Vec::len)
}

/// An agent-packaged chart scene: the CLI has no chart surface, so it prints the
/// layer counts and points the user at `--export-scene` / the TUI Strategy Lab.
fn render_overlay_scene(
    title: &str,
    symbol: &str,
    timeframe: &str,
    scene: &serde_json::Value,
) -> String {
    let counts = ["signals", "bands", "stops", "indicators", "hints", "hedges"]
        .iter()
        .map(|k| format!("{k}={}", scene_layer_len(scene, k)))
        .collect::<Vec<_>>()
        .join(" ");
    format!(
        "{BOLD}Scene{RESET} {title} [{symbol} {timeframe}] {counts}\n  (render in the TUI Strategy Lab or the web chart)\n"
    )
}

fn render_alert(level: &str, message: &str) -> String {
    let upper = level.to_uppercase();
    let color = match upper.as_str() {
        "INFO" => GREEN,
        "WARN" => YELLOW,
        "CRITICAL" => RED,
        _ => RESET,
    };
    format!("{color}[{upper}]{RESET} {message}\n")
}

fn border_line(widths: &[usize], left: char, mid: char, right: char) -> String {
    let mut s = String::new();
    s.push(left);
    for (i, width) in widths.iter().enumerate() {
        if i > 0 {
            s.push(mid);
        }
        s.push_str(&"─".repeat(width + 2));
    }
    s.push(right);
    s
}

fn pad(text: &str, width: usize, right_align: bool) -> String {
    let padding = width.saturating_sub(text.chars().count());
    if right_align {
        format!("{}{}", " ".repeat(padding), text)
    } else {
        format!("{}{}", text, " ".repeat(padding))
    }
}

fn row_line(cells: &[&str], widths: &[usize], right_align: &[bool]) -> String {
    let mut s = String::new();
    s.push('│');
    for (i, cell) in cells.iter().enumerate() {
        s.push(' ');
        s.push_str(&pad(cell, widths[i], right_align[i]));
        s.push(' ');
        s.push('│');
    }
    s
}

fn header_line(columns: &[String], widths: &[usize], right_align: &[bool]) -> String {
    let mut s = String::new();
    s.push('│');
    for (i, column) in columns.iter().enumerate() {
        let padded = pad(column, widths[i], right_align[i]);
        s.push(' ');
        s.push_str(&format!("{BOLD}{padded}{RESET}"));
        s.push(' ');
        s.push('│');
    }
    s
}
