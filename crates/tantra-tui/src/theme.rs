//! Visual design adapted from Trading-R1 (Tauric Research, Apache-2.0): https://github.com/TauricResearch/Trading-R1 — tui/src/theme, components/, ui/.
//!
//! Colour tokens plus small block helpers shared by the shell renderer.

use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, BorderType, Borders};

/// Brand primary.
pub const PRIMARY: Color = Color::Rgb(0x14, 0xC2, 0x90);
/// Accent.
pub const ACCENT: Color = Color::Rgb(0x3D, 0xD9, 0xA6);
/// Default text.
pub const TEXT: Color = Color::Rgb(0xF2, 0xF6, 0xF4);
/// Muted text.
pub const TEXT_MUTED: Color = Color::Rgb(0xA3, 0xB0, 0xAA);
/// Canvas background.
pub const BACKGROUND: Color = Color::Rgb(0x0D, 0x13, 0x11);
/// Panel background.
pub const BACKGROUND_PANEL: Color = Color::Rgb(0x12, 0x1A, 0x17);
/// Element background.
pub const BACKGROUND_ELEMENT: Color = Color::Rgb(0x18, 0x22, 0x1E);
/// Idle border.
pub const BORDER: Color = Color::Rgb(0x34, 0x44, 0x3B);
/// Focused border.
pub const BORDER_ACTIVE: Color = Color::Rgb(0x14, 0xC2, 0x90);
/// Reasoning bar.
pub const REASONING_BAR: Color = Color::Rgb(0x1F, 0x5F, 0x50);
/// Success.
pub const SUCCESS: Color = Color::Rgb(0x14, 0xC2, 0x90);
/// Error.
pub const ERROR: Color = Color::Rgb(0xE5, 0x67, 0x5B);
/// Warning.
pub const WARNING: Color = Color::Rgb(0xE0, 0xA3, 0x3E);
/// Info.
pub const INFO: Color = Color::Rgb(0x5A, 0xA7, 0xE0);
/// Placeholder text.
pub const PLACEHOLDER: Color = Color::Rgb(0x66, 0x66, 0x66);

/// Markdown h1.
pub const MD_H1: Color = Color::Rgb(0x88, 0xC0, 0xD0);
/// Markdown h2.
pub const MD_H2: Color = Color::Rgb(0x8F, 0xBC, 0xBB);
/// Markdown h3.
pub const MD_H3: Color = Color::Rgb(0xA3, 0xBE, 0x8C);
/// Markdown bold.
pub const MD_BOLD: Color = Color::Rgb(0xEC, 0xEF, 0xF4);
/// Markdown italic.
pub const MD_ITALIC: Color = Color::Rgb(0xD8, 0xDE, 0xE9);
/// Markdown inline code.
pub const MD_CODE: Color = Color::Rgb(0xEB, 0xCB, 0x8B);
/// Markdown link.
pub const MD_LINK: Color = Color::Rgb(0x81, 0xA1, 0xC1);
/// Markdown list marker.
pub const MD_LIST: Color = Color::Rgb(0x81, 0xA1, 0xC1);

/// Braille spinner frames.
pub const SPINNER: [&str; 10] = ["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"];

/// Spinner glyph for an animation tick.
pub fn spinner_frame(tick: u64) -> &'static str {
    SPINNER[((tick / 3) % 10) as usize]
}

/// Style with only a foreground colour.
pub fn fg(color: Color) -> Style {
    Style::default().fg(color)
}

/// Bold foreground style.
pub fn bold(color: Color) -> Style {
    Style::default().fg(color).add_modifier(Modifier::BOLD)
}

/// Rounded bordered panel with a Trading-R1 style `╭─Title───` title.
pub fn panel_block(title: &str, title_color: Color) -> Block<'static> {
    Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(fg(BORDER))
        .title(Line::from(vec![
            Span::styled("─", fg(BORDER)),
            Span::styled(title.to_string(), bold(title_color)),
        ]))
}

/// Rounded input field block; the border highlights when focused.
pub fn field_block(focused: bool) -> Block<'static> {
    Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(fg(if focused { BORDER_ACTIVE } else { BORDER }))
}

/// Darken a colour for the modal backdrop.
pub fn dim_color(color: Color) -> Color {
    if color == PRIMARY {
        Color::Rgb(0x07, 0x48, 0x36)
    } else {
        Color::Rgb(0x3D, 0x42, 0x3F)
    }
}
