//! Context menu system: specs, state, hit-testing, rendering.
//!
//! Merges clin's `ui::canvas_menu` (spec-based items with colors/shortcuts,
//! rect/row_at hit-testing) with standalone pinstar's menu types (shape /
//! orientation pickers, editor menu).

use ratatui::text::{Line, Span};
use ratatui::{
    Frame,
    layout::Rect,
    style::{Color, Modifier, Style},
    widgets::{Block, Clear, List, ListItem},
};

use crate::formats::SupportedFormat;
use crate::theme::ThemeColors;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PinstarMenuType {
    Canvas,
    Editor,
    ColorPicker,
    ShapePicker,
    EdgeMenu,
    EdgeColorPicker,
    EdgeStylePicker,
    OrientationPicker,
}

pub struct MenuItemSpec {
    pub label: &'static str,
    pub shortcut: Option<char>,
    pub color_hint: Option<Color>,
}

impl MenuItemSpec {
    pub const fn new(label: &'static str) -> Self {
        Self {
            label,
            shortcut: None,
            color_hint: None,
        }
    }

    pub const fn shortcut(mut self, c: char) -> Self {
        self.shortcut = Some(c);
        self
    }

    pub const fn color(mut self, c: Color) -> Self {
        self.color_hint = Some(c);
        self
    }
}

/// Returns the single-letter keyboard shortcut for a context-menu item, if any.
/// Single source of truth shared by render (hint display) and input (key matching).
pub fn menu_item_shortcut_char(menu_type: PinstarMenuType, item: &str) -> Option<char> {
    match menu_type {
        PinstarMenuType::Canvas => match item {
            "Create Connection" => Some('c'),
            "Delete Connection" => Some('d'),
            "Rename Node" => Some('r'),
            "Resize Node" => Some('s'),
            "Set Shape..." => Some('p'),
            "Set Color..." => Some('o'),
            "Delete All Connections" => Some('b'),
            "Delete Node" => Some('x'),
            "Add Text Node" => Some('t'),
            "Add Group" => Some('g'),
            "Add Image Node" => Some('m'),
            _ => None,
        },
        PinstarMenuType::Editor => match item {
            "Copy" => Some('c'),
            "Cut" => Some('x'),
            "Paste" => Some('v'),
            "Select All" => Some('a'),
            _ => None,
        },
        PinstarMenuType::EdgeMenu => match item {
            "Set Color..." => Some('o'),
            "Set Style..." => Some('s'),
            _ => None,
        },
        PinstarMenuType::ShapePicker => match item {
            "Rectangle" => Some('r'),
            "Diamond" => Some('d'),
            "Circle" => Some('c'),
            "Cylinder" => Some('y'),
            "Stadium" => Some('s'),
            _ => None,
        },
        PinstarMenuType::ColorPicker | PinstarMenuType::EdgeColorPicker => match item {
            "Default" => Some('d'),
            "Red" => Some('r'),
            "Orange" => Some('o'),
            "Yellow" => Some('y'),
            "Green" => Some('g'),
            "Cyan" => Some('c'),
            "Purple" => Some('p'),
            "Blue" => Some('b'),
            "Magenta" => Some('m'),
            "White" => Some('w'),
            _ => None,
        },
        PinstarMenuType::EdgeStylePicker => match item {
            "Solid" => Some('s'),
            "Dashed" => Some('d'),
            "Dotted" => Some('t'),
            _ => None,
        },
        PinstarMenuType::OrientationPicker => match item {
            "Top-Down" => Some('t'),
            "Left-Right" => Some('l'),
            "Right-Left" => Some('r'),
            "Bottom-Up" => Some('b'),
            _ => None,
        },
    }
}

/// Build the item specs for a menu kind. Format + capability gating matches
/// the merged upstream behavior: clin canvas menus (title-rename, image
/// nodes) and standalone flowchart menus (shapes, orientations).
pub fn menu_specs(
    kind: PinstarMenuType,
    selected_node: bool,
    format: SupportedFormat,
    images_enabled: bool,
) -> Vec<MenuItemSpec> {
    let labels: Vec<&'static str> = match kind {
        PinstarMenuType::Canvas => {
            let mut items = if selected_node {
                vec![
                    "Create Connection",
                    "Delete Connection",
                    "Rename Node",
                    "Resize Node",
                    "Set Shape...",
                    "Set Color...",
                    "Delete All Connections",
                    "Delete Node",
                ]
            } else {
                let mut v = vec!["Add Text Node"];
                if format == SupportedFormat::Canvas {
                    v.push("Add Group");
                }
                if format == SupportedFormat::Canvas && images_enabled {
                    v.push("Add Image Node");
                }
                v
            };
            if format == SupportedFormat::Canvas {
                items.retain(|item| *item != "Set Shape...");
            } else {
                items.retain(|item| *item != "Add Group" && *item != "Add Image Node");
            }
            if format == SupportedFormat::Mermaid || format == SupportedFormat::PlantUml {
                items.retain(|item| *item != "Set Color...");
            }
            if format.is_flowchart() && selected_node {
                items.push("Set Orientation...");
            }
            items
        }
        PinstarMenuType::Editor => vec!["Copy", "Cut", "Paste", "Select All"],
        PinstarMenuType::EdgeMenu => {
            let mut items = vec!["Set Color...", "Set Style..."];
            if format == SupportedFormat::Mermaid {
                items.retain(|item| *item != "Set Color..." && *item != "Set Style...");
            }
            if format == SupportedFormat::PlantUml {
                items.retain(|item| *item != "Set Color...");
            }
            items
        }
        PinstarMenuType::EdgeStylePicker => vec!["Solid", "Dashed", "Dotted"],
        PinstarMenuType::ShapePicker => {
            let mut items = vec!["Rectangle", "Diamond", "Circle", "Cylinder", "Stadium"];
            if format == SupportedFormat::PlantUml {
                items.retain(|item| *item != "Diamond" && *item != "Stadium");
            }
            items
        }
        PinstarMenuType::OrientationPicker => {
            vec!["Top-Down", "Left-Right", "Right-Left", "Bottom-Up"]
        }
        PinstarMenuType::ColorPicker | PinstarMenuType::EdgeColorPicker => {
            // Built with colors below.
            let mut specs = vec![MenuItemSpec::new("Default").shortcut('d')];
            for (name, _, color) in crate::COLOR_PICKER_PALETTE {
                let mut spec = MenuItemSpec::new(name).color(*color);
                if let Some(c) = menu_item_shortcut_char(kind, name) {
                    spec = spec.shortcut(c);
                }
                specs.push(spec);
            }
            return specs;
        }
    };

    let mut specs = Vec::new();
    for label in labels {
        let mut spec = MenuItemSpec::new(label);
        if let Some(c) = menu_item_shortcut_char(kind, label) {
            spec = spec.shortcut(c);
        }
        specs.push(spec);
    }
    specs
}

pub struct PinstarContextMenu {
    pub x: u16,
    pub y: u16,
    pub selected: usize,
    pub items: Vec<MenuItemSpec>,
    pub menu_type: PinstarMenuType,
}

impl PinstarContextMenu {
    pub fn new(x: u16, y: u16, items: Vec<MenuItemSpec>, menu_type: PinstarMenuType) -> Self {
        Self {
            x,
            y,
            selected: 0,
            items,
            menu_type,
        }
    }

    pub fn move_up(&mut self) {
        self.selected = self.selected.saturating_sub(1);
    }

    pub fn move_down(&mut self) {
        if self.selected + 1 < self.items.len() {
            self.selected += 1;
        }
    }

    pub fn find_shortcut(&self, ch: char) -> Option<usize> {
        let cl = ch.to_ascii_lowercase();
        self.items
            .iter()
            .position(|i| i.shortcut.is_some_and(|s| s.to_ascii_lowercase() == cl))
    }

    pub fn label(&self, idx: usize) -> Option<&'static str> {
        self.items.get(idx).map(|i| i.label)
    }

    pub fn rect(&self, area: Rect) -> Rect {
        let max_content = self
            .items
            .iter()
            .map(|i| {
                let base = i.label.chars().count();
                let square = if i.color_hint.is_some() { 2 } else { 0 }; // "■ "
                let shortcut = i.shortcut.map_or(0, |_| 2); // "c "
                base + square + shortcut + 4 // 2 left + 2 right pad
            })
            .max()
            .unwrap_or(0);
        let width = max_content.max(8) as u16;
        let height = self.items.len() as u16;
        let x = self
            .x
            .min(area.x.saturating_add(area.width.saturating_sub(width)));
        let y = self
            .y
            .min(area.y.saturating_add(area.height.saturating_sub(height)));
        Rect::new(x, y, width, height)
    }

    pub fn row_at(&self, rect: Rect, col: u16, row: u16) -> Option<usize> {
        if col >= rect.x && col < rect.x + rect.width && row >= rect.y && row < rect.y + rect.height
        {
            let idx = (row - rect.y) as usize;
            (idx < self.items.len()).then_some(idx)
        } else {
            None
        }
    }
}

/// Render a context menu with hover highlight (clin visuals, self-contained).
pub fn render_context_menu(
    frame: &mut Frame,
    area: Rect,
    menu: &PinstarContextMenu,
    theme: &ThemeColors,
    mouse_pos: Option<(u16, u16)>,
) {
    let rect = menu.rect(area);
    frame.render_widget(Clear, rect);
    let items: Vec<ListItem> = menu
        .items
        .iter()
        .enumerate()
        .map(|(i, spec)| {
            let is_selected = i == menu.selected;
            let base = if is_selected {
                Style::default()
                    .fg(theme.highlight_fg)
                    .bg(theme.highlight_bg)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(theme.text)
            };
            let mut spans: Vec<Span> = Vec::new();
            spans.push(Span::styled("  ", base));
            if let Some(c) = spec.color_hint {
                spans.push(Span::styled("■ ", base.fg(c)));
            }
            let label = format!("{}  ", spec.label);
            spans.push(Span::styled(label, base));
            // dynamic padding so shortcut right-aligns.
            let content_len = spec.label.chars().count()
                + 4
                + usize::from(spec.color_hint.is_some())
                + spec.shortcut.map_or(0, |_| 2);
            let pad = (rect.width as usize).saturating_sub(content_len);
            if pad > 0 {
                spans.push(Span::styled(" ".repeat(pad), base));
            }
            if let Some(c) = spec.shortcut {
                spans.push(Span::styled(format!("{c} "), base.fg(theme.muted)));
            }
            ListItem::new(Line::from(spans))
        })
        .collect();
    let list = List::new(items).block(
        Block::default()
            .borders(ratatui::widgets::Borders::NONE)
            .style(theme.preview_bg_style()),
    );
    frame.render_widget(list, rect);

    // Hover highlight on the row under the mouse (excluding selected row).
    if let Some((col, row)) = mouse_pos
        && !rect.is_empty()
        && col >= rect.x
        && col < rect.x + rect.width
        && row >= rect.y
        && row < rect.y + rect.height
    {
        let idx = (row - rect.y) as usize;
        if idx < menu.items.len() && idx != menu.selected {
            let hover_rect = Rect::new(rect.x, row, rect.width, 1);
            let buf = frame.buffer_mut();
            for c in hover_rect.left()..hover_rect.right() {
                if let Some(cell) = buf.cell_mut((c, row)) {
                    cell.set_style(theme.hover_style());
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn canvas_menu_specs_gate_by_format() {
        let sel = menu_specs(
            PinstarMenuType::Canvas,
            true,
            SupportedFormat::Canvas,
            false,
        );
        assert!(sel.iter().all(|s| s.label != "Set Shape..."));
        let flow = menu_specs(PinstarMenuType::Canvas, true, SupportedFormat::Dot, false);
        assert!(flow.iter().any(|s| s.label == "Set Shape..."));
        assert!(flow.iter().any(|s| s.label == "Set Orientation..."));
    }

    #[test]
    fn image_node_item_only_when_enabled() {
        let with = menu_specs(
            PinstarMenuType::Canvas,
            false,
            SupportedFormat::Canvas,
            true,
        );
        assert!(with.iter().any(|s| s.label == "Add Image Node"));
        let without = menu_specs(
            PinstarMenuType::Canvas,
            false,
            SupportedFormat::Canvas,
            false,
        );
        assert!(without.iter().all(|s| s.label != "Add Image Node"));
    }

    #[test]
    fn color_picker_specs_carry_colors() {
        let specs = menu_specs(
            PinstarMenuType::ColorPicker,
            false,
            SupportedFormat::Canvas,
            false,
        );
        assert_eq!(specs.len(), crate::COLOR_PICKER_PALETTE.len() + 1);
        assert!(specs[1].color_hint.is_some());
    }

    #[test]
    fn find_shortcut_case_insensitive() {
        let menu = PinstarContextMenu::new(
            0,
            0,
            menu_specs(
                PinstarMenuType::Canvas,
                true,
                SupportedFormat::Canvas,
                false,
            ),
            PinstarMenuType::Canvas,
        );
        assert_eq!(menu.find_shortcut('C'), Some(0)); // Create Connection
        assert_eq!(menu.find_shortcut('x'), Some(6)); // Delete Node
    }

    #[test]
    fn rect_clamps_to_area() {
        let menu = PinstarContextMenu::new(
            200,
            200,
            menu_specs(
                PinstarMenuType::EdgeStylePicker,
                false,
                SupportedFormat::Canvas,
                false,
            ),
            PinstarMenuType::EdgeStylePicker,
        );
        let rect = menu.rect(Rect::new(0, 0, 40, 20));
        assert!(rect.right() <= 40);
        assert!(rect.bottom() <= 20);
    }
}
