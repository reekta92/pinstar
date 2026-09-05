//! Theme colors + color parsing + textarea helpers.
//!
//! `ThemeColors` is the standalone `PinstarTheme` (renamed, graf-style) with
//! the extra style methods clin's render used on `AppThemeColors`. Hosts
//! construct it from their own theme systems field-by-field.

use ratatui::prelude::*;
use ratatui::widgets::*;
use ratatui_textarea::{CursorMove, TextArea};

#[derive(Debug, Clone)]
pub struct ThemeColors {
    pub accent: Color,
    pub heading: Color,
    pub success: Color,
    pub warning: Color,
    pub destructive: Color,
    pub muted: Color,
    pub text: Color,
    pub fg: Color,
    pub bg: Color,
    pub border: Color,
    pub tag: Color,
    pub folder: Color,
    pub highlight_fg: Color,
    pub highlight_bg: Color,
}

impl Default for ThemeColors {
    fn default() -> Self {
        Self {
            accent: Color::Cyan,
            heading: Color::Yellow,
            success: Color::Green,
            warning: Color::Yellow,
            destructive: Color::Red,
            muted: Color::DarkGray,
            text: Color::Reset,
            fg: Color::White,
            bg: Color::Black,
            border: Color::DarkGray,
            tag: Color::LightMagenta,
            folder: Color::Blue,
            highlight_fg: Color::Black,
            highlight_bg: Color::Cyan,
        }
    }
}

impl ThemeColors {
    pub fn bg_style(&self) -> Style {
        Style::default().bg(self.bg)
    }

    pub fn hover_style(&self) -> Style {
        Style::default().bg(self.highlight_bg).fg(self.highlight_fg)
    }

    pub fn preview_bg(&self) -> Color {
        derive_color(self.bg, -15).unwrap_or(self.bg)
    }

    pub fn hint_line_bg(&self) -> Color {
        derive_color(self.bg, -8).unwrap_or(self.bg)
    }

    pub fn preview_bg_style(&self) -> Style {
        Style::default().bg(self.preview_bg())
    }

    pub fn hint_line_bg_style(&self) -> Style {
        Style::default().bg(self.hint_line_bg())
    }

    pub fn parse_color(color_code: Option<&str>, theme: &ThemeColors) -> Color {
        match color_code {
            Some(s) if s.starts_with('#') => parse_hex_color(s).unwrap_or(theme.accent),
            _ => get_node_color(color_code, theme),
        }
    }
}

fn derive_color(base: Color, delta: i16) -> Option<Color> {
    match base {
        Color::Rgb(r, g, b) => {
            let clamp = |v: i16| v.clamp(0, 255) as u8;
            Some(Color::Rgb(
                clamp(r as i16 + delta),
                clamp(g as i16 + delta),
                clamp(b as i16 + delta),
            ))
        }
        other => Some(other),
    }
}

pub fn parse_hex_color(s: &str) -> Option<Color> {
    let s = s.strip_prefix('#')?;
    if s.len() == 6 {
        let r = u8::from_str_radix(&s[0..2], 16).ok()?;
        let g = u8::from_str_radix(&s[2..4], 16).ok()?;
        let b = u8::from_str_radix(&s[4..6], 16).ok()?;
        return Some(Color::Rgb(r, g, b));
    }
    None
}

/// Resolve a node color attribute: `#rrggbb`, a 1-based palette index, a
/// palette name, or the theme accent as fallback.
pub fn get_node_color(color_code: Option<&str>, theme: &ThemeColors) -> Color {
    match color_code {
        Some(s) if s.starts_with('#') => parse_hex_color(s).unwrap_or(theme.accent),
        Some(s) => {
            if let Ok(idx) = s.parse::<usize>()
                && idx >= 1
                && idx <= crate::COLOR_PICKER_PALETTE.len()
            {
                return crate::COLOR_PICKER_PALETTE[idx - 1].2;
            }
            if let Some(entry) = crate::COLOR_PICKER_PALETTE
                .iter()
                .find(|e| e.0.eq_ignore_ascii_case(s))
            {
                entry.2
            } else {
                theme.accent
            }
        }
        None => theme.accent,
    }
}

pub fn get_edge_color(color: Option<&str>, selected: bool, theme: &ThemeColors) -> Color {
    if selected {
        return theme.accent;
    }
    color
        .and_then(|c| c.strip_prefix('#').map(|_| c).or(Some(c)))
        .and_then(parse_hex_color_full)
        .unwrap_or(theme.muted)
}

fn parse_hex_color_full(s: &str) -> Option<Color> {
    if s.starts_with('#') {
        parse_hex_color(s)
    } else {
        None
    }
}

// ── textarea helpers (ported from standalone helpers.rs / clin events) ──────

pub fn contains_cell(rect: Rect, col: u16, row: u16) -> bool {
    rect.width > 0
        && rect.height > 0
        && col >= rect.x
        && col < rect.x + rect.width
        && row >= rect.y
        && row < rect.y + rect.height
}

pub fn move_textarea_cursor_to_mouse(
    textarea: &mut TextArea,
    body_inner: Rect,
    mouse_col: u16,
    mouse_row: u16,
) {
    if textarea.lines().is_empty() || body_inner.width == 0 || body_inner.height == 0 {
        return;
    }

    let (scroll_row, scroll_col) = get_textarea_scroll(textarea);

    let row = mouse_row.saturating_sub(body_inner.y) as usize + scroll_row;
    let col = mouse_col.saturating_sub(body_inner.x) as usize + scroll_col;

    let max_row = textarea.lines().len().saturating_sub(1);
    let target_row = row.min(max_row);
    let max_col = textarea.lines()[target_row].chars().count();
    let target_col = col.min(max_col);

    textarea.move_cursor(CursorMove::Jump(target_row as u16, target_col as u16));
}

pub fn move_textarea_cursor_to_mouse_scrolled(
    textarea: &mut TextArea,
    body_inner: Rect,
    mouse_col: u16,
    mouse_row: u16,
    scroll_row: usize,
    scroll_col: usize,
) {
    if textarea.lines().is_empty() || body_inner.width == 0 || body_inner.height == 0 {
        return;
    }
    let row = mouse_row.saturating_sub(body_inner.y) as usize + scroll_row;
    let col = mouse_col.saturating_sub(body_inner.x) as usize + scroll_col;
    let max_row = textarea.lines().len().saturating_sub(1);
    let target_row = row.min(max_row);
    let max_col = textarea.lines()[target_row].chars().count();
    let target_col = col.min(max_col);
    textarea.move_cursor(CursorMove::Jump(target_row as u16, target_col as u16));
}

pub fn get_textarea_scroll(textarea: &TextArea) -> (usize, usize) {
    let mut scroll_row = 0;
    let mut scroll_col = 0;

    let debug_str = format!("{textarea:?}");
    if let Some(start) = debug_str.find("viewport: Viewport(") {
        let after_start = &debug_str[start + "viewport: Viewport(".len()..];
        if let Some(end) = after_start.find(')') {
            let number_str = &after_start[..end];
            if let Ok(number) = number_str.parse::<u64>() {
                scroll_row = ((number >> 16) & 0xFFFF) as usize;
                scroll_col = (number & 0xFFFF) as usize;
            }
        }
    }
    (scroll_row, scroll_col)
}

pub fn line_number_gutter(
    line_count: usize,
    cursor_row: usize,
    scroll_row: usize,
    height: u16,
    theme: &ThemeColors,
    top_padding: u16,
) -> Paragraph<'static> {
    let digits = line_count.max(1).to_string().len();
    let display_lines = height as usize;
    let mut gutter_lines: Vec<Line<'static>> = Vec::with_capacity(display_lines);
    for i in 0..display_lines.min(line_count.saturating_sub(scroll_row)) {
        let current_line_idx = i + scroll_row;
        let is_current = current_line_idx == cursor_row;
        let style = if is_current {
            Style::default()
                .fg(theme.accent)
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(theme.muted)
        };
        gutter_lines.push(Line::from(vec![Span::styled(
            format!("{:>width$} ", current_line_idx + 1, width = digits),
            style,
        )]));
    }
    for _ in gutter_lines.len()..display_lines {
        gutter_lines.push(Line::from(Span::raw(" ")));
    }
    Paragraph::new(gutter_lines)
        .style(theme.preview_bg_style())
        .block(
            Block::default()
                .padding(Padding::new(0, 0, top_padding, 0))
                .style(theme.preview_bg_style()),
        )
}

pub fn fill_cursor_line_bg(frame: &mut Frame, editor: &TextArea, area: Rect, bg: Color) {
    if editor.selection_range().is_some() {
        return;
    }
    let (scroll_row, _) = get_textarea_scroll(editor);
    let cursor_row = editor.cursor().0;
    let screen_row = cursor_row.saturating_sub(scroll_row) as u16;
    let inner_y = editor.block().map(|b| b.inner(area).y).unwrap_or(area.y);
    let y = inner_y + screen_row;
    if y < area.y || y >= area.bottom() {
        return;
    }
    let buf = frame.buffer_mut();
    for x in area.left()..area.right() {
        if let Some(cell) = buf.cell_mut((x, y)) {
            cell.set_bg(bg);
        }
    }
}
