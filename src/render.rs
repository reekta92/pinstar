//! Canvas rendering. Canvas-format passes are clin's (titles in blocks,
//! image nodes, marquee, multi-select, edge-list overlay); flowchart-format
//! passes are the standalone renderer (shapes, braille borders, box-drawing
//! orthogonal edges, title-above-node). The bottom status row and rename
//! keybind hints are host-owned; this renderer reserves the bottom row.

use crate::data::{CanvasNode, EdgeStyle};
use crate::formats::SupportedFormat;
use crate::grid::{CanvasGridProjection, draw_canvas_grid};
use crate::menu::render_context_menu;
use crate::overlay::{draw_canvas_rect_filled, muted_canvas_selection_fill};
use crate::state::PinstarState;
use crate::theme::ThemeColors;
use crate::theme::{contains_cell, fill_cursor_line_bg, get_textarea_scroll, line_number_gutter};

use ratatui::{prelude::*, widgets::*};

pub fn draw_pinstar_view(
    frame: &mut Frame,
    state: &mut PinstarState,
    theme: &ThemeColors,
    area: Rect,
    mouse_pos: Option<(u16, u16)>,
) {
    let total_area = area;
    let mut area = area;
    if state.settings.show_hints {
        area.height = area.height.saturating_sub(1);
    }

    let canvas_mouse_pos = if state.context_menu.is_some() {
        None
    } else {
        mouse_pos
    };

    let (editor_area, canvas_area) = if state.show_editor_pane {
        let main_chunks = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Percentage(30), Constraint::Percentage(70)])
            .split(area);
        (Some(main_chunks[0]), main_chunks[1])
    } else {
        (None, area)
    };

    if let Some(editor_area) = editor_area {
        if state.format == SupportedFormat::Canvas {
            render_editor_pane_clin(frame, state, theme, editor_area);
        } else {
            render_editor_pane_standalone(frame, state, theme, editor_area);
        }
    }

    // Per-frame projection invariants (reused by grid, group, edge, node passes).
    let origin_x = canvas_area.x as f64 + canvas_area.width as f64 / 2.0;
    let origin_y = canvas_area.y as f64 + canvas_area.height as f64 / 2.0;
    let z = state.zoom;
    let vx = state.viewport_x;
    let vy = state.viewport_y;
    let view_left = canvas_area.left() as f64;
    let view_right = canvas_area.right() as f64;
    let view_top = canvas_area.top() as f64;
    let view_bottom = canvas_area.bottom() as f64;

    let canvas_border_color = if !state.editor_focus || !state.show_editor_pane {
        theme.accent
    } else {
        theme.muted
    };
    let canvas_block = Block::default()
        .borders(Borders::NONE)
        .border_style(Style::default().fg(canvas_border_color))
        .style(theme.bg_style());
    frame.render_widget(canvas_block, canvas_area);

    let (cx1, cy1) = state.screen_to_canvas(canvas_area.left(), canvas_area.top(), canvas_area);
    let (cx2, cy2) = state.screen_to_canvas(canvas_area.right(), canvas_area.bottom(), canvas_area);
    draw_canvas_grid(
        frame,
        canvas_area,
        state.show_grid,
        CanvasGridProjection {
            world_left: cx1.min(cx2),
            world_right: cx1.max(cx2),
            world_top: cy1.min(cy2),
            world_bottom: cy1.max(cy2),
            origin_col: origin_x - vx * z,
            origin_row: origin_y - vy * z,
            cols_per_world_x: z,
            rows_per_world_y: z,
        },
        theme.muted,
        state.zoom,
    );

    if state.format == SupportedFormat::Canvas {
        render_canvas_groups(
            frame,
            state,
            theme,
            canvas_area,
            canvas_mouse_pos,
            origin_x,
            origin_y,
            z,
            vx,
            vy,
            view_left,
            view_right,
            view_top,
            view_bottom,
        );
        render_canvas_edges(
            frame,
            state,
            theme,
            canvas_area,
            origin_x,
            origin_y,
            z,
            vx,
            vy,
            view_left,
            view_right,
            view_top,
            view_bottom,
        );
        render_canvas_nodes(
            frame,
            state,
            theme,
            canvas_mouse_pos,
            origin_x,
            origin_y,
            z,
            vx,
            vy,
            view_left,
            view_right,
            view_top,
            view_bottom,
        );
    } else {
        render_flowchart_groups(
            frame,
            state,
            theme,
            canvas_area,
            origin_x,
            origin_y,
            z,
            vx,
            vy,
        );
        render_flowchart_edges(frame, state, theme, canvas_area);
        render_flowchart_nodes(
            frame,
            state,
            theme,
            canvas_area,
            origin_x,
            origin_y,
            z,
            vx,
            vy,
        );
    }

    render_floating_editor(
        frame,
        state,
        theme,
        canvas_area,
        origin_x,
        origin_y,
        z,
        vx,
        vy,
    );

    if state.format == SupportedFormat::Canvas {
        // Marquee overlay: drawn AFTER all nodes/edges/editor.
        if let (Some(start), Some(curr)) = (state.marquee.start, state.marquee.end)
            && state.right_down_screen.is_some()
        {
            let (sx1, sy1) = ((start.0 - vx) * z + origin_x, (start.1 - vy) * z + origin_y);
            let (sx2, sy2) = ((curr.0 - vx) * z + origin_x, (curr.1 - vy) * z + origin_y);
            let (min_x, max_x) = if sx1 < sx2 { (sx1, sx2) } else { (sx2, sx1) };
            let (min_y, max_y) = if sy1 < sy2 { (sy1, sy2) } else { (sy2, sy1) };
            let left = (min_x
                .max(canvas_area.left() as f64)
                .min(canvas_area.right() as f64)) as u16;
            let top = (min_y
                .max(canvas_area.top() as f64)
                .min(canvas_area.bottom() as f64)) as u16;
            let width = ((max_x - min_x).max(1.0)) as u16;
            let height = ((max_y - min_y).max(1.0)) as u16;
            let fill = muted_canvas_selection_fill(
                theme.selection_indicator,
                theme.accent,
                theme.highlight_bg,
            );
            let screen_rect = Rect::new(left, top, width, height);
            draw_canvas_rect_filled(frame, screen_rect, fill);
        }

        render_edge_list_overlay(frame, state, theme, canvas_area, mouse_pos);
    } else if let (Some(start), Some(end)) = (state.select_rect_start, state.select_rect_end) {
        render_select_rect(frame, state, theme, canvas_area, start, end);
    }
    if state.settings.show_hints {
        render_hint_line(frame, state, theme, total_area);
    }
    render_menu(frame, state, theme, area, mouse_pos);
    render_rename_popup(frame, state, theme, area);
    render_help_overlay(frame, state, theme, area);
}

// ── editor panes ────────────────────────────────────────────────────────────

fn render_editor_pane_clin(
    frame: &mut Frame,
    state: &mut PinstarState,
    theme: &ThemeColors,
    editor_area: Rect,
) {
    let editor_border_color = if state.editor_focus {
        theme.accent
    } else {
        theme.muted
    };
    let editor_block = Block::default()
        .borders(Borders::RIGHT)
        .border_style(Style::default().fg(editor_border_color))
        .title(" Source (JSON) ")
        .style(theme.bg_style());

    state.raw_editor.set_block(editor_block);
    state.raw_editor.set_style(theme.bg_style());
    state.raw_editor.set_cursor_line_style(Style::default());
    frame.render_widget(&state.raw_editor, editor_area);
}

fn render_editor_pane_standalone(
    frame: &mut Frame,
    state: &mut PinstarState,
    theme: &ThemeColors,
    editor_area: Rect,
) {
    let editor_border_color = if state.editor_focus {
        theme.accent
    } else {
        theme.muted
    };
    let editor_block = Block::default()
        .borders(Borders::RIGHT)
        .border_style(Style::default().fg(editor_border_color))
        .title(" Source (JSON) ")
        .style(theme.preview_bg_style());

    let line_count = state.raw_editor.lines().len();
    let cursor_row = state.raw_editor.cursor().0;
    let scroll_row = get_textarea_scroll(&state.raw_editor).0;

    let content_area = editor_area;
    let digits = line_count.max(1).to_string().len() as u16;
    let gutter_width = digits + 1;
    let gutter_area = Rect::new(
        content_area.x,
        content_area.y,
        gutter_width.min(content_area.width),
        content_area.height,
    );
    let gutter = line_number_gutter(
        line_count,
        cursor_row,
        scroll_row,
        content_area.height,
        theme,
        1,
    );
    frame.render_widget(gutter, gutter_area);

    let editor_rect = Rect::new(
        content_area.x + gutter_area.width,
        content_area.y,
        content_area.width.saturating_sub(gutter_area.width),
        content_area.height,
    );

    state.raw_editor.set_block(editor_block);
    state.raw_editor.set_style(theme.preview_bg_style());
    state
        .raw_editor
        .set_cursor_line_style(if state.editor_focus {
            Style::default().bg(theme.preview_bg())
        } else {
            Style::default()
        });
    frame.render_widget(&state.raw_editor, editor_rect);

    if state.editor_focus {
        let cursor_bg = theme.preview_bg();
        fill_cursor_line_bg(frame, &state.raw_editor, editor_rect, cursor_bg);
    }
}

// ── canvas-format passes (clin) ─────────────────────────────────────────────

struct Proj {
    is_group: bool,
    sx: f64,
    sy: f64,
    sw: f64,
    sh: f64,
    on_screen: bool,
}

fn project(state: &PinstarState, canvas_area: Rect) -> Vec<Proj> {
    let origin_x = canvas_area.x as f64 + canvas_area.width as f64 / 2.0;
    let origin_y = canvas_area.y as f64 + canvas_area.height as f64 / 2.0;
    let z = state.zoom;
    let vx = state.viewport_x;
    let vy = state.viewport_y;
    let view_left = canvas_area.left() as f64;
    let view_right = canvas_area.right() as f64;
    let view_top = canvas_area.top() as f64;
    let view_bottom = canvas_area.bottom() as f64;
    state
        .data
        .nodes
        .iter()
        .map(|n| {
            let (nx, ny) = n.pos();
            let (nw, nh) = n.size();
            let sx = (nx - vx) * z + origin_x;
            let sy = (ny - vy) * z + origin_y;
            let sw = nw * z;
            let sh = nh * z;
            Proj {
                is_group: matches!(n, CanvasNode::Group(_)),
                sx,
                sy,
                sw,
                sh,
                on_screen: !(sx + sw < view_left
                    || sx > view_right
                    || sy + sh < view_top
                    || sy > view_bottom),
            }
        })
        .collect()
}

#[allow(clippy::too_many_arguments)]
fn render_canvas_groups(
    frame: &mut Frame,
    state: &mut PinstarState,
    theme: &ThemeColors,
    canvas_area: Rect,
    canvas_mouse_pos: Option<(u16, u16)>,
    origin_x: f64,
    origin_y: f64,
    z: f64,
    vx: f64,
    vy: f64,
    view_left: f64,
    view_right: f64,
    view_top: f64,
    view_bottom: f64,
) {
    let proj = project(state, canvas_area);
    let _ = (origin_x, origin_y, z, vx, vy);
    for (idx, p) in proj.iter().enumerate() {
        if !p.is_group || !p.on_screen {
            continue;
        }
        let node = &state.data.nodes[idx];
        let CanvasNode::Group(g) = node else {
            continue;
        };

        let sx = p.sx;
        let sy = p.sy;
        let sw = p.sw;
        let sh = p.sh;

        let left = sx.max(view_left);
        let top = sy.max(view_top);
        let right = (sx + sw).min(view_right);
        let bottom = (sy + sh).min(view_bottom);
        if right <= left || bottom <= top {
            continue;
        }
        let node_rect = Rect::new(
            left as u16,
            top as u16,
            (right - left) as u16,
            (bottom - top) as u16,
        );

        let is_primary = state.selection.primary.as_deref() == Some(g.id.as_str());
        let is_selected = is_primary || state.selection.extra.contains(g.id.as_str());
        let is_editing = is_primary && state.floating_editor.is_some();
        let base_color = crate::theme::get_node_color(g.color.as_deref(), theme);
        let border_color = if is_editing { theme.accent } else { base_color };

        let mut label = g.label.as_deref().unwrap_or("Group").to_string();
        if is_editing {
            label = format!("[EDITING] {label}");
        }

        let title_h = 2.min(node_rect.height).max(1);
        let title_rect = Rect::new(node_rect.x, node_rect.y, node_rect.width, title_h);

        let is_hovered = !is_selected
            && canvas_mouse_pos.is_some_and(|(col, row)| contains_cell(title_rect, col, row));

        let title_style = if is_hovered {
            theme.hover_style()
        } else {
            let mut s = Style::default().bg(base_color);
            s = s.fg(theme.bg);
            s
        };

        let mut block = Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(border_color))
            .title(ratatui::text::Line::from(format!(" {label} ")).style(title_style))
            .style(if is_hovered {
                theme.hover_style()
            } else {
                theme.bg_style()
            });

        if is_selected && !is_editing {
            block = block.border_set(ratatui::symbols::border::Set {
                top_left: "\u{250c}",
                top_right: "\u{2510}",
                bottom_left: "\u{2514}",
                bottom_right: "\u{2518}",
                vertical_left: "\u{2506}",
                vertical_right: "\u{2506}",
                horizontal_top: "\u{2504}",
                horizontal_bottom: "\u{2504}",
            });
        } else {
            block = block.border_type(if is_editing {
                BorderType::Rounded
            } else {
                BorderType::Double
            });
        }

        frame.render_widget(block, node_rect);

        render_selection_corners(frame, theme, node_rect, is_selected);

        if state.resizing_node_id.as_deref() == Some(g.id.as_str()) {
            render_resize_handle(frame, theme, sx, sy, sw, sh);
        }
    }
}

fn render_selection_corners(
    frame: &mut Frame,
    theme: &ThemeColors,
    node_rect: Rect,
    selected: bool,
) {
    if !selected {
        return;
    }
    let corner_style = Style::default()
        .fg(theme.accent)
        .add_modifier(Modifier::BOLD);
    if node_rect.width > 0 && node_rect.height > 0 {
        frame.render_widget(
            Paragraph::new("\u{21d8}").style(corner_style),
            Rect::new(node_rect.x, node_rect.y, 1, 1),
        );
        if node_rect.width > 1 {
            frame.render_widget(
                Paragraph::new("\u{21d9}").style(corner_style),
                Rect::new(node_rect.x + node_rect.width - 1, node_rect.y, 1, 1),
            );
        }
        if node_rect.height > 1 {
            frame.render_widget(
                Paragraph::new("\u{21d7}").style(corner_style),
                Rect::new(node_rect.x, node_rect.y + node_rect.height - 1, 1, 1),
            );
        }
        if node_rect.width > 1 && node_rect.height > 1 {
            frame.render_widget(
                Paragraph::new("\u{21d6}").style(corner_style),
                Rect::new(
                    node_rect.x + node_rect.width - 1,
                    node_rect.y + node_rect.height - 1,
                    1,
                    1,
                ),
            );
        }
    }
}

fn render_resize_handle(
    frame: &mut Frame,
    theme: &ThemeColors,
    sx: f64,
    sy: f64,
    sw: f64,
    sh: f64,
) {
    let handle_text = "[\u{2198}]";
    let handle_style = Style::default()
        .fg(theme.accent)
        .add_modifier(Modifier::BOLD);
    let handle_rect = Rect::new(
        (sx + sw - 3.0).max(0.0) as u16,
        (sy + sh - 1.0).max(0.0) as u16,
        3,
        1,
    );
    frame.render_widget(Paragraph::new(handle_text).style(handle_style), handle_rect);
}

#[allow(clippy::too_many_arguments)]
fn draw_braille_segment(
    buf: &mut ratatui::buffer::Buffer,
    x1: f64,
    y1: f64,
    x2: f64,
    y2: f64,
    view_left: f64,
    view_right: f64,
    view_top: f64,
    view_bottom: f64,
    style: EdgeStyle,
    color: Color,
) {
    let mut current_x = x1;
    let mut current_y = y1;
    let dist = ((x2 - x1).powi(2) + (y2 - y1).powi(2)).sqrt();
    let steps = (dist * 4.0) as usize;
    if steps == 0 {
        return;
    }
    let ddx = (x2 - x1) / steps as f64;
    let ddy = (y2 - y1) / steps as f64;
    for step in 0..=steps {
        let draw = match style {
            EdgeStyle::Solid | EdgeStyle::Thick => true,
            EdgeStyle::Dashed => step % 16 < 8,
            EdgeStyle::Dotted => step % 8 == 0,
        };
        if draw
            && current_x >= view_left
            && current_x < view_right
            && current_y >= view_top
            && current_y < view_bottom
        {
            let cell_x = current_x as u16;
            let cell_y = current_y as u16;
            let dot_x = ((current_x - cell_x as f64) * 2.0) as u16;
            let dot_y = ((current_y - cell_y as f64) * 4.0) as u16;
            set_braille_dot(buf, cell_x, cell_y, dot_x, dot_y, color);
        }
        current_x += ddx;
        current_y += ddy;
    }
}

fn set_braille_dot(
    buf: &mut ratatui::buffer::Buffer,
    cell_x: u16,
    cell_y: u16,
    dot_x: u16,
    dot_y: u16,
    color: Color,
) {
    if let Some(cell) = buf.cell_mut((cell_x, cell_y)) {
        let mut braille_char = cell.symbol().chars().next().unwrap_or('\u{2800}');
        if !('\u{2800}'..='\u{28FF}').contains(&braille_char) {
            braille_char = '\u{2800}';
        }
        let dot_bit = match (dot_x, dot_y) {
            (0, 0) => 0x01,
            (0, 1) => 0x02,
            (0, 2) => 0x04,
            (1, 0) => 0x08,
            (1, 1) => 0x10,
            (1, 2) => 0x20,
            (0, 3) => 0x40,
            (1, 3) => 0x80,
            _ => 0,
        };
        let new_code = (braille_char as u32 - 0x2800) | dot_bit;
        if let Some(c) = char::from_u32(0x2800 + new_code) {
            cell.set_char(c).set_fg(color);
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn render_canvas_edges(
    frame: &mut Frame,
    state: &mut PinstarState,
    theme: &ThemeColors,
    canvas_area: Rect,
    origin_x: f64,
    origin_y: f64,
    z: f64,
    vx: f64,
    vy: f64,
    view_left: f64,
    view_right: f64,
    view_top: f64,
    view_bottom: f64,
) {
    let _ = canvas_area;
    for edge in &state.data.edges {
        let Some(seg) = state.get_edge_segments(edge) else {
            continue;
        };
        let is_edge_selected = state.selected_edge_id.as_deref() == Some(edge.id.as_str());
        let edge_color =
            crate::theme::get_edge_color(edge.color.as_deref(), is_edge_selected, theme);
        for &(sx, sy, ex, ey) in &seg {
            let sfx = (sx - vx) * z + origin_x;
            let sfy = (sy - vy) * z + origin_y;
            let stx = (ex - vx) * z + origin_x;
            let sty = (ey - vy) * z + origin_y;
            // Cull per segment
            let min_x = sfx.min(stx);
            let max_x = sfx.max(stx);
            let min_y = sfy.min(sty);
            let max_y = sfy.max(sty);
            if max_x < view_left || min_x > view_right || max_y < view_top || min_y > view_bottom {
                continue;
            }
            draw_braille_segment(
                frame.buffer_mut(),
                sfx,
                sfy,
                stx,
                sty,
                view_left,
                view_right,
                view_top,
                view_bottom,
                edge.style,
                edge_color,
            );
        }
    }
}

#[cfg(feature = "images")]
fn is_image_ext(path: &str) -> bool {
    let ext = std::path::Path::new(path)
        .extension()
        .and_then(|e| e.to_str())
        .map(|e| e.to_lowercase())
        .unwrap_or_default();
    matches!(
        ext.as_str(),
        "png" | "jpg" | "jpeg" | "gif" | "webp" | "bmp"
    )
}

#[allow(clippy::too_many_arguments)]
fn render_canvas_nodes(
    frame: &mut Frame,
    state: &mut PinstarState,
    theme: &ThemeColors,
    canvas_mouse_pos: Option<(u16, u16)>,
    origin_x: f64,
    origin_y: f64,
    z: f64,
    vx: f64,
    vy: f64,
    view_left: f64,
    view_right: f64,
    view_top: f64,
    view_bottom: f64,
) {
    let view = ViewState {
        state,
        origin_x,
        origin_y,
        z,
        vx,
        vy,
        view_left,
        view_right,
        view_top,
        view_bottom,
    };
    let proj: Vec<Proj> = view.project_nodes().collect();

    for (idx, p) in proj.iter().enumerate() {
        if p.is_group || !p.on_screen {
            continue;
        }
        let node = &state.data.nodes[idx];
        let sx = p.sx;
        let sy = p.sy;
        let sw = p.sw;
        let sh = p.sh;

        let left = sx.max(view_left);
        let top = sy.max(view_top);
        let right = (sx + sw).min(view_right);
        let bottom = (sy + sh).min(view_bottom);
        if right <= left || bottom <= top {
            continue;
        }
        let node_rect = Rect::new(
            left as u16,
            top as u16,
            (right - left) as u16,
            (bottom - top) as u16,
        );

        frame.render_widget(Clear, node_rect);

        let is_primary = state.selection.primary.as_deref() == Some(node.id());
        let is_selected = is_primary || state.selection.extra.contains(node.id());
        let is_editing = is_primary && state.floating_editor.is_some();

        let node_color_attr = match node {
            CanvasNode::Text(n) => n.color.as_deref(),
            CanvasNode::File(n) => n.color.as_deref(),
            CanvasNode::Link(n) => n.color.as_deref(),
            _ => None,
        };

        let base_color = crate::theme::get_node_color(node_color_attr, theme);
        let border_color = if is_editing { theme.accent } else { base_color };

        let mut border_type = BorderType::Plain;
        if is_editing {
            border_type = BorderType::Double;
        }

        let mut node_title = match node.title() {
            Some(t) => t.to_string(),
            None => match node {
                CanvasNode::File(n) => std::path::Path::new(&n.file)
                    .file_name()
                    .and_then(|s| s.to_str())
                    .unwrap_or(&n.file)
                    .to_string(),
                CanvasNode::Link(n) => n.url.clone(),
                CanvasNode::Group(n) => n.label.clone().unwrap_or_default(),
                CanvasNode::Text(_) => "".to_string(),
            },
        };

        if is_editing {
            node_title = format!("[EDITING] {node_title}");
        }
        let is_hovered = !is_selected
            && canvas_mouse_pos.is_some_and(|(col, row)| contains_cell(node_rect, col, row));
        let bg_style = if is_hovered {
            theme.hover_style()
        } else {
            theme.bg_style()
        };
        let mut block = Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(border_color))
            .title(Span::styled(
                node_title,
                Style::default().fg(if is_editing { theme.accent } else { base_color }),
            ))
            .style(bg_style);

        if is_selected && !is_editing {
            block = block.border_set(ratatui::symbols::border::Set {
                top_left: "\u{250c}",
                top_right: "\u{2510}",
                bottom_left: "\u{2514}",
                bottom_right: "\u{2518}",
                vertical_left: "\u{2506}",
                vertical_right: "\u{2506}",
                horizontal_top: "\u{2504}",
                horizontal_bottom: "\u{2504}",
            });
        } else {
            block = block.border_type(border_type);
        }

        #[cfg(feature = "images")]
        let is_image_file = matches!(node, CanvasNode::File(n) if is_image_ext(&n.file))
            && state.image_picker.is_some();
        #[cfg(not(feature = "images"))]
        let is_image_file = false;

        // Short-circuit: during transforms, skip pixel decode and render plain text
        if state.is_view_transforming() {
            let text = Paragraph::new(node.text())
                .block(block)
                .style(Style::default().fg(theme.text))
                .wrap(Wrap { trim: false });
            frame.render_widget(text, node_rect);
            render_selection_corners(frame, theme, node_rect, is_selected);
            if state.resizing_node_id.as_deref() == Some(node.id()) {
                render_resize_handle(frame, theme, sx, sy, sw, sh);
            }
            continue;
        }

        // Render pixel image if available
        if is_image_file {
            #[cfg(feature = "images")]
            {
                let file_path = match node {
                    CanvasNode::File(n) => n.file.clone(),
                    _ => String::new(),
                };
                let key = std::path::PathBuf::from(&file_path);
                if let Some(tx) = &state.image_decode_tx {
                    state.image_cache.request(key.clone(), 2048, tx);
                }
                if let Some(proto) = state.image_cache.get_proto(&key)
                    && node_rect.width > 2
                    && node_rect.height > 2
                {
                    let inner_area = Rect::new(
                        node_rect.x + 1,
                        node_rect.y + 1,
                        node_rect.width.saturating_sub(2),
                        node_rect.height.saturating_sub(2),
                    );
                    frame.render_widget(block, node_rect);
                    frame.render_stateful_widget(
                        ratatui_image::StatefulImage::default()
                            .resize(ratatui_image::Resize::Fit(None)),
                        inner_area,
                        proto,
                    );
                } else {
                    let text = Paragraph::new(node.text())
                        .block(block)
                        .style(Style::default().fg(theme.text))
                        .wrap(Wrap { trim: false });
                    frame.render_widget(text, node_rect);
                }
            }
        } else {
            let text = Paragraph::new(node.text())
                .block(block)
                .style(Style::default().fg(theme.text))
                .wrap(Wrap { trim: false });
            frame.render_widget(text, node_rect);
        }

        render_selection_corners(frame, theme, node_rect, is_selected);

        if state.resizing_node_id.as_deref() == Some(node.id()) {
            render_resize_handle(frame, theme, sx, sy, sw, sh);
        }
    }
}

struct ViewState<'a> {
    state: &'a PinstarState,
    origin_x: f64,
    origin_y: f64,
    z: f64,
    vx: f64,
    vy: f64,
    view_left: f64,
    view_right: f64,
    view_top: f64,
    view_bottom: f64,
}

impl ViewState<'_> {
    fn project_nodes(&self) -> impl Iterator<Item = Proj> + '_ {
        self.state.data.nodes.iter().map(move |n| {
            let (nx, ny) = n.pos();
            let (nw, nh) = n.size();
            let sx = (nx - self.vx) * self.z + self.origin_x;
            let sy = (ny - self.vy) * self.z + self.origin_y;
            let sw = nw * self.z;
            let sh = nh * self.z;
            Proj {
                is_group: matches!(n, CanvasNode::Group(_)),
                sx,
                sy,
                sw,
                sh,
                on_screen: !(sx + sw < self.view_left
                    || sx > self.view_right
                    || sy + sh < self.view_top
                    || sy > self.view_bottom),
            }
        })
    }
}

// ── flowchart-format passes (standalone) ────────────────────────────────────

#[allow(clippy::too_many_arguments)]
fn render_flowchart_groups(
    frame: &mut Frame,
    state: &mut PinstarState,
    theme: &ThemeColors,
    canvas_area: Rect,
    origin_x: f64,
    origin_y: f64,
    z: f64,
    vx: f64,
    vy: f64,
) {
    let mut groups: Vec<&CanvasNode> = state
        .data
        .nodes
        .iter()
        .filter(|n| matches!(n, CanvasNode::Group(_)))
        .collect();

    // Sort descending by area so larger (parent) groups render first.
    groups.sort_by(|a, b| {
        let (wa, ha) = a.size();
        let (wb, hb) = b.size();
        (wb * hb)
            .partial_cmp(&(wa * ha))
            .unwrap_or(std::cmp::Ordering::Equal)
    });

    for node in groups {
        let CanvasNode::Group(g) = node else {
            continue;
        };
        let (nx, ny) = node.pos();
        let (nw, nh) = node.size();

        let sx = (nx - vx) * z + origin_x;
        let sy = (ny - vy) * z + origin_y;
        let sw = nw * z;
        let sh = nh * z;

        if sx + sw < canvas_area.left() as f64
            || sx > canvas_area.right() as f64
            || sy + sh < canvas_area.top() as f64
            || sy > canvas_area.bottom() as f64
        {
            continue;
        }

        let left = sx.max(canvas_area.left() as f64);
        let top = sy.max(canvas_area.top() as f64);
        let right = (sx + sw).min(canvas_area.right() as f64);
        let bottom = (sy + sh).min(canvas_area.bottom() as f64);

        if right <= left || bottom <= top {
            continue;
        }

        let node_rect = Rect::new(
            left.round() as u16,
            top.round() as u16,
            (right - left).round() as u16,
            (bottom - top).round() as u16,
        );

        let is_selected = state.selection.is_selected(&g.id.to_string());
        let is_editing = is_selected && state.floating_editor.is_some();
        let base_color = ThemeColors::parse_color(g.color.as_deref(), theme);

        let is_connected_to_selected = if let Some(sel_id) = &state.selection.primary {
            sel_id != &g.id
                && state.data.edges.iter().any(|e| {
                    (e.from_node == *sel_id && e.to_node == g.id)
                        || (e.to_node == *sel_id && e.from_node == g.id)
                })
        } else {
            false
        };

        let border_color = if is_editing {
            theme.accent
        } else if is_connected_to_selected {
            theme.success
        } else {
            base_color
        };

        let mut label = g.label.as_deref().unwrap_or("Group").to_string();
        if is_editing {
            label = format!("[EDITING] {label}");
        }

        let mut block = Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(border_color))
            .title(
                Line::from(Span::styled(
                    label,
                    Style::default().fg(if is_editing { theme.accent } else { base_color }),
                ))
                .alignment(Alignment::Center),
            )
            .style(theme.bg_style());

        if is_selected && !is_editing {
            block = block.border_set(ratatui::symbols::border::Set {
                top_left: "┌",
                top_right: "┐",
                bottom_left: "└",
                bottom_right: "┘",
                vertical_left: "┆",
                vertical_right: "┆",
                horizontal_top: "┄",
                horizontal_bottom: "┄",
            });
        } else {
            block = block.border_type(if is_editing {
                BorderType::Rounded
            } else {
                BorderType::Double
            });
        }

        frame.render_widget(block, node_rect);

        // Titlebar background — clickable area indicator for groups.
        if node_rect.height >= 3 {
            let tbar = Rect::new(
                node_rect.x + 1,
                node_rect.y + 1,
                node_rect.width.saturating_sub(2),
                1,
            );
            let tbar_color = if is_selected {
                theme.accent
            } else {
                theme.muted
            };
            frame.render_widget(
                Paragraph::new(" ".repeat(tbar.width as usize))
                    .style(Style::default().bg(tbar_color)),
                tbar,
            );
        }

        render_selection_corners(frame, theme, node_rect, is_selected);

        if state.resizing_node_id.as_ref() == Some(&g.id.to_string()) {
            render_resize_handle(frame, theme, sx, sy, sw, sh);
        }
    }
}

fn render_flowchart_edges(
    frame: &mut Frame,
    state: &mut PinstarState,
    theme: &ThemeColors,
    canvas_area: Rect,
) {
    for edge in &state.data.edges {
        let from_node = state.data.nodes.iter().find(|n| n.id() == edge.from_node);
        let to_node = state.data.nodes.iter().find(|n| n.id() == edge.to_node);

        if let (Some(f), Some(t)) = (from_node, to_node) {
            let effective_style = edge.style;
            let (fx, fy) = f.pos();
            let (fw, fh) = f.size();
            let (tx, ty) = t.pos();
            let (tw, th) = t.size();

            let scx = fx + fw / 2.0;
            let scy = fy + fh / 2.0;
            let tcx = tx + tw / 2.0;
            let tcy = ty + th / 2.0;

            let dx = tcx - scx;
            let dy = tcy - scy;

            let is_horizontal_exit = dx.abs() > dy.abs();

            let (ax, ay) = if is_horizontal_exit {
                if dx > 0.0 { (fx + fw, scy) } else { (fx, scy) }
            } else if dy > 0.0 {
                (scx, fy + fh)
            } else {
                (scx, fy)
            };

            let (bx, by) = if is_horizontal_exit {
                if dx > 0.0 { (tx, tcy) } else { (tx + tw, tcy) }
            } else if dy > 0.0 {
                (tcx, ty)
            } else {
                (tcx, ty + th)
            };

            let origin_x = canvas_area.x as f64 + canvas_area.width as f64 / 2.0;
            let origin_y = canvas_area.y as f64 + canvas_area.height as f64 / 2.0;
            let mut sfx = (ax - state.viewport_x) * state.zoom + origin_x;
            let mut sfy = (ay - state.viewport_y) * state.zoom + origin_y;
            let mut stx = (bx - state.viewport_x) * state.zoom + origin_x;
            let mut sty = (by - state.viewport_y) * state.zoom + origin_y;

            // Adjust coordinates for RIGHT and BOTTOM edges.
            if is_horizontal_exit {
                if dx > 0.0 {
                    sfx -= 1.0;
                } else {
                    stx -= 1.0;
                }
            } else if dy > 0.0 {
                sfy -= 1.0;
            } else {
                sty -= 1.0;
            }

            let edge_color = if state.selected_edge_id.as_ref() == Some(&edge.id) {
                theme.accent
            } else if edge.color.is_some() {
                ThemeColors::parse_color(edge.color.as_deref(), theme)
            } else {
                theme.muted
            };

            let buf = frame.buffer_mut();

            let draw_box_line = |buf: &mut ratatui::prelude::Buffer,
                                 x1: i32,
                                 y1: i32,
                                 x2: i32,
                                 y2: i32,
                                 horz_char: char,
                                 vert_char: char| {
                if y1 == y2 {
                    let (start, end) = if x1 < x2 { (x1, x2) } else { (x2, x1) };
                    for x in start..=end {
                        if x < canvas_area.left() as i32
                            || x >= canvas_area.right() as i32
                            || y1 < canvas_area.top() as i32
                            || y1 >= canvas_area.bottom() as i32
                        {
                            continue;
                        }
                        let ch = match effective_style {
                            EdgeStyle::Dotted => {
                                if (x - start) % 4 != 0 {
                                    continue;
                                }
                                horz_char
                            }
                            EdgeStyle::Dashed => {
                                if (x - start) % 8 >= 4 {
                                    continue;
                                }
                                horz_char
                            }
                            _ => horz_char,
                        };
                        if let Some(cell) = buf.cell_mut((x as u16, y1 as u16)) {
                            cell.set_char(ch).set_fg(edge_color);
                        }
                    }
                } else if x1 == x2 {
                    let (start, end) = if y1 < y2 { (y1, y2) } else { (y2, y1) };
                    for y in start..=end {
                        if x1 < canvas_area.left() as i32
                            || x1 >= canvas_area.right() as i32
                            || y < canvas_area.top() as i32
                            || y >= canvas_area.bottom() as i32
                        {
                            continue;
                        }
                        let ch = match effective_style {
                            EdgeStyle::Dotted => {
                                if (y - start) % 4 != 0 {
                                    continue;
                                }
                                vert_char
                            }
                            EdgeStyle::Dashed => {
                                if (y - start) % 8 >= 4 {
                                    continue;
                                }
                                vert_char
                            }
                            _ => vert_char,
                        };
                        if let Some(cell) = buf.cell_mut((x1 as u16, y as u16)) {
                            cell.set_char(ch).set_fg(edge_color);
                        }
                    }
                }
            };

            let draw_corner = |buf: &mut ratatui::prelude::Buffer, x: i32, y: i32, ch: char| {
                if x >= canvas_area.left() as i32
                    && x < canvas_area.right() as i32
                    && y >= canvas_area.top() as i32
                    && y < canvas_area.bottom() as i32
                    && let Some(cell) = buf.cell_mut((x as u16, y as u16))
                {
                    cell.set_char(ch).set_fg(edge_color);
                }
            };

            let draw_arrow = |buf: &mut ratatui::prelude::Buffer, ch: char, col: i32, row: i32| {
                if col >= canvas_area.left() as i32
                    && col < canvas_area.right() as i32
                    && row >= canvas_area.top() as i32
                    && row < canvas_area.bottom() as i32
                    && let Some(cell) = buf.cell_mut((col as u16, row as u16))
                {
                    cell.set_char(ch).set_fg(edge_color);
                }
            };

            // Flowchart formats always render orthogonally with box chars.
            let sx = sfx.round() as i32;
            let sy = sfy.round() as i32;
            let ex = stx.round() as i32;
            let ey = sty.round() as i32;

            if is_horizontal_exit {
                let mid_x = (sx + ex) / 2;
                draw_box_line(buf, sx, sy, mid_x, sy, '\u{2500}', '\u{2502}');
                draw_box_line(buf, mid_x, sy, mid_x, ey, '\u{2500}', '\u{2502}');
                draw_box_line(buf, mid_x, ey, ex, ey, '\u{2500}', '\u{2502}');

                if ex > sx {
                    if ey > sy {
                        draw_corner(buf, mid_x, sy, '\u{2510}');
                        draw_corner(buf, mid_x, ey, '\u{2514}');
                    } else if sy > ey {
                        draw_corner(buf, mid_x, sy, '\u{2518}');
                        draw_corner(buf, mid_x, ey, '\u{250C}');
                    }
                } else if ey > sy {
                    draw_corner(buf, mid_x, sy, '\u{250C}');
                    draw_corner(buf, mid_x, ey, '\u{2518}');
                } else if sy > ey {
                    draw_corner(buf, mid_x, sy, '\u{2514}');
                    draw_corner(buf, mid_x, ey, '\u{2510}');
                }

                let (arrow_c, arrow_col, arrow_row) = if ex > sx {
                    ('\u{25b6}', ex - 1, ey)
                } else {
                    ('\u{25c0}', ex + 1, ey)
                };
                draw_arrow(buf, arrow_c, arrow_col, arrow_row);
            } else {
                let mid_y = (sy + ey) / 2;
                draw_box_line(buf, sx, sy, sx, mid_y, '\u{2500}', '\u{2502}');
                draw_box_line(buf, sx, mid_y, ex, mid_y, '\u{2500}', '\u{2502}');
                draw_box_line(buf, ex, mid_y, ex, ey, '\u{2500}', '\u{2502}');

                if ey > sy {
                    if ex > sx {
                        draw_corner(buf, sx, mid_y, '\u{2514}');
                        draw_corner(buf, ex, mid_y, '\u{2510}');
                    } else if sx > ex {
                        draw_corner(buf, sx, mid_y, '\u{2518}');
                        draw_corner(buf, ex, mid_y, '\u{250C}');
                    }
                } else if ex > sx {
                    draw_corner(buf, sx, mid_y, '\u{250C}');
                    draw_corner(buf, ex, mid_y, '\u{2518}');
                } else if sx > ex {
                    draw_corner(buf, sx, mid_y, '\u{2510}');
                    draw_corner(buf, ex, mid_y, '\u{2514}');
                }

                let (arrow_c, arrow_col, arrow_row) = if ey > sy {
                    ('\u{25bc}', ex, ey - 1)
                } else {
                    ('\u{25b2}', ex, ey + 1)
                };
                draw_arrow(buf, arrow_c, arrow_col, arrow_row);
            }
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn render_flowchart_nodes(
    frame: &mut Frame,
    state: &mut PinstarState,
    theme: &ThemeColors,
    canvas_area: Rect,
    origin_x: f64,
    origin_y: f64,
    z: f64,
    vx: f64,
    vy: f64,
) {
    for node in &state.data.nodes {
        if matches!(node, CanvasNode::Group(_)) {
            continue;
        }

        let (nx, ny) = node.pos();
        let (nw, nh) = node.size();

        let sx = (nx - vx) * z + origin_x;
        let sy = (ny - vy) * z + origin_y;
        let sw = nw * z;
        let sh = nh * z;

        if sx + sw < canvas_area.left() as f64
            || sx > canvas_area.right() as f64
            || sy + sh < canvas_area.top() as f64
            || sy > canvas_area.bottom() as f64
        {
            continue;
        }

        let left = sx.max(canvas_area.left() as f64);
        let top = sy.max(canvas_area.top() as f64);
        let right = (sx + sw).min(canvas_area.right() as f64);
        let bottom = (sy + sh).min(canvas_area.bottom() as f64);

        if right <= left || bottom <= top {
            continue;
        }

        let node_rect = Rect::new(
            left.round() as u16,
            top.round() as u16,
            (right - left).round() as u16,
            (bottom - top).round() as u16,
        );

        frame.render_widget(Clear, node_rect);

        let is_selected = state.selection.is_selected(&node.id().to_string());
        let is_editing = is_selected && state.floating_editor.is_some();

        let node_color_attr = match node {
            CanvasNode::Text(n) => n.color.as_deref(),
            CanvasNode::File(n) => n.color.as_deref(),
            CanvasNode::Link(n) => n.color.as_deref(),
            _ => None,
        };

        let base_color = ThemeColors::parse_color(node_color_attr, theme);

        let is_connected_to_selected = if let Some(sel_id) = &state.selection.primary {
            sel_id != node.id()
                && state.data.edges.iter().any(|e| {
                    (e.from_node == *sel_id && e.to_node == node.id())
                        || (e.to_node == *sel_id && e.from_node == node.id())
                })
        } else {
            false
        };

        let border_color = if is_editing {
            theme.accent
        } else if is_connected_to_selected {
            theme.success
        } else {
            base_color
        };

        let mut border_type = BorderType::Plain;
        if is_editing {
            border_type = BorderType::Double;
        }

        let mut node_title = match node {
            CanvasNode::File(n) => std::path::Path::new(&n.file)
                .file_name()
                .and_then(|s| s.to_str())
                .unwrap_or(&n.file)
                .to_string(),
            CanvasNode::Link(n) => n.url.clone(),
            _ => {
                if let Some(t) = node.title() {
                    t.to_string()
                } else if is_generated_id(node.id()) {
                    "".to_string()
                } else {
                    node.id().to_string()
                }
            }
        };

        if is_editing {
            node_title = format!("[EDITING] {node_title}");
        }

        let use_braille_border = match node.shape() {
            crate::data::NodeShape::Rectangle => state.format.is_flowchart(),
            _ => true,
        };

        let mut block = Block::default().style(theme.bg_style());

        if !use_braille_border {
            block = block
                .borders(Borders::ALL)
                .border_style(Style::default().fg(border_color));

            if is_selected && !is_editing {
                block = block.border_set(ratatui::symbols::border::Set {
                    top_left: "┌",
                    top_right: "┐",
                    bottom_left: "└",
                    bottom_right: "┘",
                    vertical_left: "┆",
                    vertical_right: "┆",
                    horizontal_top: "┄",
                    horizontal_bottom: "┄",
                });
            } else {
                block = block.border_type(border_type);
            }
        }

        let get_text_with_divider =
            |original: &str, inner_w: usize, color: ratatui::style::Color| -> ratatui::text::Text {
                if original
                    .split('\n')
                    .any(|l| l.trim_end_matches('\r').trim() == "---")
                {
                    let divider = if inner_w > 0 {
                        "─".repeat(inner_w)
                    } else {
                        "---".to_string()
                    };
                    let mut lines = Vec::new();
                    for line in original.split('\n') {
                        let clean = line.trim_end_matches('\r');
                        if clean.trim() == "---" {
                            lines.push(ratatui::text::Line::from(ratatui::text::Span::styled(
                                divider.clone(),
                                Style::default().fg(color),
                            )));
                        } else {
                            lines.push(ratatui::text::Line::from(clean.to_string()));
                        }
                    }
                    ratatui::text::Text::from(lines)
                } else {
                    ratatui::text::Text::from(original.to_string())
                }
            };

        if !use_braille_border {
            let inner_w = node_rect.width.saturating_sub(2) as usize;
            let text_content = get_text_with_divider(node.text(), inner_w, border_color);

            let text = Paragraph::new(text_content)
                .block(block)
                .style(Style::default().fg(theme.text))
                .wrap(Wrap { trim: false });
            frame.render_widget(text, node_rect);
        } else {
            frame.render_widget(block, node_rect);

            let text_rect = node_rect.inner(ratatui::layout::Margin {
                horizontal: 2.min(node_rect.width.saturating_sub(1) / 2),
                vertical: 1.min(node_rect.height.saturating_sub(1) / 2),
            });

            let text_str = node.text();
            let mut est_lines = 0;
            for line in text_str.lines() {
                let char_count = line.chars().count();
                let needed = if text_rect.width > 0 {
                    ((char_count as f32) / (text_rect.width as f32)).ceil() as usize
                } else {
                    1
                };
                est_lines += needed.max(1);
            }
            let est_lines = est_lines.max(1);
            let available_h = text_rect.height as usize;
            let y_offset = if available_h > est_lines {
                (available_h - est_lines) / 2
            } else {
                0
            };

            let centered_rect = Rect::new(
                text_rect.x,
                text_rect.y + y_offset as u16,
                text_rect.width,
                text_rect.height.saturating_sub(y_offset as u16),
            );

            let text_content =
                get_text_with_divider(node.text(), text_rect.width as usize, border_color);

            let text = Paragraph::new(text_content)
                .alignment(ratatui::layout::Alignment::Center)
                .style(Style::default().fg(theme.text))
                .wrap(Wrap { trim: false });
            frame.render_widget(text, centered_rect);
        }

        if use_braille_border {
            trace_node_shape(
                frame,
                state,
                theme,
                node,
                node_rect,
                canvas_area,
                border_color,
            );
        }

        if !node_title.is_empty() && node_rect.y > canvas_area.top() {
            let title_rect = Rect::new(node_rect.x, node_rect.y - 1, node_rect.width, 1);
            frame.render_widget(Clear, title_rect);
            let title_p = Paragraph::new(node_title.clone())
                .alignment(Alignment::Center)
                .style(Style::default().fg(if is_editing { theme.accent } else { base_color }));
            frame.render_widget(title_p, title_rect);
        }

        render_selection_corners(frame, theme, node_rect, is_selected);

        if state.resizing_node_id.as_ref() == Some(&node.id().to_string()) {
            render_resize_handle(frame, theme, sx, sy, sw, sh);
        }
    }
}

fn trace_node_shape(
    frame: &mut Frame,
    state: &PinstarState,
    theme: &ThemeColors,
    node: &CanvasNode,
    node_rect: Rect,
    canvas_area: Rect,
    border_color: Color,
) {
    let _ = theme;
    let lx = node_rect.left() as f64;
    let ty = node_rect.top() as f64;
    let rx = node_rect.right() as f64;
    let by = node_rect.bottom() as f64;

    let put_pixel = |x: f64, y: f64, frame: &mut Frame| {
        if x >= canvas_area.left() as f64
            && x < canvas_area.right() as f64
            && y >= canvas_area.top() as f64
            && y < canvas_area.bottom() as f64
        {
            let cell_x = x as u16;
            let cell_y = y as u16;
            let dot_x = ((x - cell_x as f64) * 2.0) as u16;
            let dot_y = ((y - cell_y as f64) * 4.0) as u16;
            let buf = frame.buffer_mut();
            if let Some(cell) = buf.cell_mut((cell_x, cell_y)) {
                let mut braille_char = cell.symbol().chars().next().unwrap_or('\u{2800}');
                if !('\u{2800}'..='\u{28FF}').contains(&braille_char) {
                    braille_char = '\u{2800}';
                }
                let dot_bit = match (dot_x, dot_y) {
                    (0, 0) => 0x01,
                    (0, 1) => 0x02,
                    (0, 2) => 0x04,
                    (1, 0) => 0x08,
                    (1, 1) => 0x10,
                    (1, 2) => 0x20,
                    (0, 3) => 0x40,
                    (1, 3) => 0x80,
                    _ => 0,
                };
                let new_code = (braille_char as u32 - 0x2800) | dot_bit;
                if let Some(c) = char::from_u32(0x2800 + new_code) {
                    let mut style = Style::default().fg(border_color);
                    if state.format.is_flowchart() {
                        style = style.add_modifier(Modifier::BOLD);
                    }
                    cell.set_char(c).set_style(style);
                }
            }
        }
    };

    macro_rules! trace_line {
        ($x1:expr, $y1:expr, $x2:expr, $y2:expr) => {{
            let dist = (($x2 - $x1).powi(2) + ($y2 - $y1).powi(2)).sqrt();
            let steps = (dist * 4.0) as usize;
            if steps > 0 {
                let dx = ($x2 - $x1) / steps as f64;
                let dy = ($y2 - $y1) / steps as f64;
                let mut cx = $x1;
                let mut cy = $y1;
                for _ in 0..=steps {
                    put_pixel(cx, cy, frame);
                    cx += dx;
                    cy += dy;
                }
            }
        }};
    }

    macro_rules! trace_arc {
        ($cx:expr, $cy:expr, $rx:expr, $ry:expr, $start:expr, $end:expr) => {{
            let circumference = std::f64::consts::PI * ($rx + $ry);
            let steps = (circumference * 4.0) as usize;
            if steps > 0 {
                for i in 0..=steps {
                    let t = $start + ($end - $start) * (i as f64 / steps as f64);
                    put_pixel($cx + $rx * t.cos(), $cy + $ry * t.sin(), frame);
                }
            }
        }};
    }

    match node.shape() {
        crate::data::NodeShape::Diamond => {
            let mid_x = lx + (rx - lx) / 2.0;
            let mid_y = ty + (by - ty) / 2.0;
            trace_line!(mid_x, ty, rx, mid_y);
            trace_line!(rx, mid_y, mid_x, by);
            trace_line!(mid_x, by, lx, mid_y);
            trace_line!(lx, mid_y, mid_x, ty);
        }
        crate::data::NodeShape::Circle => {
            let cx = lx + (rx - lx) / 2.0;
            let cy = ty + (by - ty) / 2.0;
            let r_x = (rx - lx) / 2.0;
            let r_y = (by - ty) / 2.0;
            trace_arc!(cx, cy, r_x, r_y, 0.0, 2.0 * std::f64::consts::PI);
        }
        crate::data::NodeShape::Stadium => {
            let cap_r = (by - ty) / 2.0;
            let flat_left = lx + cap_r;
            let flat_right = rx - cap_r;
            if flat_right > flat_left {
                trace_line!(flat_left, ty, flat_right, ty);
                trace_line!(flat_left, by, flat_right, by);
                trace_arc!(
                    flat_right,
                    ty + cap_r,
                    cap_r,
                    cap_r,
                    -std::f64::consts::FRAC_PI_2,
                    std::f64::consts::FRAC_PI_2
                );
                trace_arc!(
                    flat_left,
                    ty + cap_r,
                    cap_r,
                    cap_r,
                    std::f64::consts::FRAC_PI_2,
                    3.0 * std::f64::consts::FRAC_PI_2
                );
            } else {
                let cx = lx + (rx - lx) / 2.0;
                let cy = ty + (by - ty) / 2.0;
                trace_arc!(
                    cx,
                    cy,
                    (rx - lx) / 2.0,
                    (by - ty) / 2.0,
                    0.0,
                    2.0 * std::f64::consts::PI
                );
            }
        }
        crate::data::NodeShape::Rectangle => {
            trace_line!(lx, ty, rx, ty);
            trace_line!(rx, ty, rx, by);
            trace_line!(rx, by, lx, by);
            trace_line!(lx, by, lx, ty);
        }
        crate::data::NodeShape::Cylinder => {
            let cy_h = 0.65;
            trace_line!(lx, ty + cy_h, lx, by - cy_h);
            trace_line!(rx, ty + cy_h, rx, by - cy_h);
            let cx = lx + (rx - lx) / 2.0;
            trace_arc!(
                cx,
                ty + cy_h,
                (rx - lx) / 2.0,
                cy_h,
                0.0,
                2.0 * std::f64::consts::PI
            );
            trace_arc!(
                cx,
                by - cy_h,
                (rx - lx) / 2.0,
                cy_h,
                0.0,
                std::f64::consts::PI
            );
        }
    }
}

// ── shared overlays ─────────────────────────────────────────────────────────

#[allow(clippy::too_many_arguments)]
fn render_floating_editor(
    frame: &mut Frame,
    state: &mut PinstarState,
    theme: &ThemeColors,
    canvas_area: Rect,
    origin_x: f64,
    origin_y: f64,
    z: f64,
    vx: f64,
    vy: f64,
) {
    state.floating_editor_rect = None;
    state.edge_overlay_rect = None;

    if let Some(editor) = &mut state.floating_editor
        && let Some(node_id) = &state.selection.primary
        && let Some(node) = state.data.nodes.iter().find(|n| n.id() == node_id)
    {
        let (nx, ny) = node.pos();
        let (nw, nh) = node.size();

        let sx = ((nx - vx) * z) + origin_x;
        let sy = ((ny - vy) * z) + origin_y;
        let sw = nw * z;
        let sh = nh * z;

        let left = sx.max(canvas_area.left() as f64);
        let top = sy.max(canvas_area.top() as f64);
        let right = (sx + sw).min(canvas_area.right() as f64);
        let bottom = (sy + sh).min(canvas_area.bottom() as f64);

        if right > left && bottom > top {
            let editor_rect = Rect::new(
                left as u16,
                top as u16,
                (right - left) as u16,
                (bottom - top) as u16,
            );

            editor.set_block(
                Block::default()
                    .borders(Borders::ALL)
                    .border_style(Style::default().fg(theme.accent))
                    .style(theme.bg_style()),
            );
            editor.set_style(theme.bg_style());
            state.floating_editor_rect =
                Some(Block::default().borders(Borders::ALL).inner(editor_rect));

            frame.render_widget(Clear, editor_rect);
            frame.render_widget(&*editor, editor_rect);
        }
    }
}

/// Color for an edge's text in the overlay: the edge's own color when set,
/// else the default text color.
fn edge_overlay_color(color: Option<&str>, theme: &ThemeColors) -> Color {
    color
        .and_then(crate::theme::parse_hex_color)
        .unwrap_or(theme.text)
}

/// Dimmed variant of a color, so "(no title)" text stays muted but still
/// hints at the edge's own color.
fn muted_edge_color(color: Color) -> Color {
    match color {
        Color::Rgb(r, g, b) => Color::Rgb(r / 2, g / 2, b / 2),
        _ => color,
    }
}

/// A resolved row for the edge-list overlay.
struct OverlayEdgeRow {
    index: usize,
    from_title: Option<String>,
    to_title: Option<String>,
    color: Option<String>,
}

fn render_edge_list_overlay(
    frame: &mut Frame,
    state: &mut PinstarState,
    theme: &ThemeColors,
    canvas_area: Rect,
    mouse_pos: Option<(u16, u16)>,
) {
    if state.selection.primary.is_none() {
        return;
    }
    let edges = state.selected_node_edges();
    if edges.is_empty() {
        return;
    }
    let no_title = "(no title)".to_string();
    let resolved: Vec<OverlayEdgeRow> = edges
        .iter()
        .enumerate()
        .map(|(i, e)| {
            let title_of = |id: &str| {
                state
                    .data
                    .nodes
                    .iter()
                    .find(|n| n.id() == id)
                    .and_then(|n| n.title())
                    .map(|t| t.to_string())
            };
            OverlayEdgeRow {
                index: i,
                from_title: title_of(&e.from_node),
                to_title: title_of(&e.to_node),
                color: e.color.clone(),
            }
        })
        .collect();

    let max_len = resolved
        .iter()
        .map(|r| {
            format!(
                "{} {} → {}",
                r.index + 1,
                r.from_title.as_deref().unwrap_or(no_title.as_str()),
                r.to_title.as_deref().unwrap_or(no_title.as_str())
            )
            .chars()
            .count()
        })
        .max()
        .unwrap_or(0);
    let overlay_width = (max_len + 4) as u16;
    let overlay_height = (edges.len() + 2) as u16;
    let overlay_rect = Rect::new(
        canvas_area.x + canvas_area.width.saturating_sub(overlay_width),
        canvas_area.y + canvas_area.height.saturating_sub(overlay_height),
        overlay_width,
        overlay_height,
    );
    let rows: Vec<ratatui::text::Line> = resolved
        .iter()
        .map(|r| {
            let edge_color = edge_overlay_color(r.color.as_deref(), theme);
            let mut spans = vec![Span::styled(
                format!("{} ", r.index + 1),
                Style::default()
                    .fg(theme.accent)
                    .add_modifier(Modifier::BOLD),
            )];
            match &r.from_title {
                Some(title) => {
                    spans.push(Span::styled(title.clone(), Style::default().fg(edge_color)))
                }
                None => spans.push(Span::styled(
                    no_title.clone(),
                    Style::default().fg(muted_edge_color(edge_color)),
                )),
            }
            spans.push(Span::styled(" → ", Style::default().fg(theme.muted)));
            match &r.to_title {
                Some(title) => {
                    spans.push(Span::styled(title.clone(), Style::default().fg(edge_color)))
                }
                None => spans.push(Span::styled(
                    no_title.clone(),
                    Style::default().fg(muted_edge_color(edge_color)),
                )),
            }
            ratatui::text::Line::from(spans)
        })
        .collect();
    let overlay = Paragraph::new(rows).block(
        Block::default()
            .borders(Borders::ALL)
            .title(" EDGES ")
            .border_style(Style::default().fg(theme.accent))
            .style(theme.bg_style()),
    );
    frame.render_widget(Clear, overlay_rect);
    frame.render_widget(overlay, overlay_rect);
    // Hover highlight on overlay rows.
    let hover_inner = Rect::new(
        overlay_rect.x + 1,
        overlay_rect.y + 1,
        overlay_rect.width.saturating_sub(2),
        overlay_rect.height.saturating_sub(2),
    );
    if let Some((col, row)) = mouse_pos
        && col > hover_inner.x
        && col < hover_inner.x + hover_inner.width
        && row > hover_inner.y
        && row < hover_inner.y + hover_inner.height
    {
        let buf = frame.buffer_mut();
        for c in hover_inner.left()..hover_inner.right() {
            if let Some(cell) = buf.cell_mut((c, row)) {
                cell.set_style(theme.hover_style());
            }
        }
    }
    state.edge_overlay_rect = Some(overlay_rect);
}

fn render_select_rect(
    frame: &mut Frame,
    state: &PinstarState,
    theme: &ThemeColors,
    canvas_area: Rect,
    start: (f64, f64),
    end: (f64, f64),
) {
    let origin_x = canvas_area.x as f64 + canvas_area.width as f64 / 2.0;
    let origin_y = canvas_area.y as f64 + canvas_area.height as f64 / 2.0;
    let sx = (start.0 - state.viewport_x) * state.zoom + origin_x;
    let sy = (start.1 - state.viewport_y) * state.zoom + origin_y;
    let ex = (end.0 - state.viewport_x) * state.zoom + origin_x;
    let ey = (end.1 - state.viewport_y) * state.zoom + origin_y;

    let (x1, x2) = if sx < ex { (sx, ex) } else { (ex, sx) };
    let (y1, y2) = if sy < ey { (sy, ey) } else { (ey, sy) };

    let buf = frame.buffer_mut();
    let mut dot = |x: f64, y: f64| {
        if x >= canvas_area.left() as f64
            && x < canvas_area.right() as f64
            && y >= canvas_area.top() as f64
            && y < canvas_area.bottom() as f64
            && let Some(cell) = buf.cell_mut((x as u16, y as u16))
        {
            cell.set_char('·').set_fg(theme.accent);
        }
    };

    let left = x1 as u16;
    let right = x2 as u16;
    let top = y1 as u16;
    let bot = y2 as u16;

    for x in left..=right {
        if (x - left) % 3 == 0 {
            dot(x as f64, y1);
            dot(x as f64, y2);
        }
    }
    for y in top..=bot {
        if (y - top) % 3 == 0 {
            dot(x1, y as f64);
            dot(x2, y as f64);
        }
    }
}

fn render_hint_line(
    frame: &mut Frame,
    state: &PinstarState,
    theme: &ThemeColors,
    total_area: Rect,
) {
    let mut hint_text = if state.editor_focus && state.show_editor_pane {
        "Editor Pane — Ctrl+S sync · Alt+Enter back to canvas".to_string()
    } else {
        "? help · a menu · i edit · Ctrl+S save · Ctrl+Z undo · Alt+Enter focus · Esc/q back"
            .to_string()
    };
    if state.connection_source_id.is_some() {
        hint_text = "CONNECTION MODE: Select target node with mouse or Enter".to_string();
    } else if state.deleting_connection_source_id.is_some() {
        hint_text = "DELETE CONNECTION MODE: Select target node to remove link".to_string();
    } else if state.resizing_node_id.is_some() {
        hint_text = "RESIZE MODE: Drag mouse to resize, Right-click to confirm".to_string();
    }

    let mut spans = Vec::new();
    let ext_label = if state.ext_editor_enabled {
        "ext:on"
    } else {
        "ext:off"
    };
    let ext_style = if state.ext_editor_enabled {
        Style::default()
            .fg(theme.success)
            .add_modifier(Modifier::BOLD)
    } else {
        Style::default().fg(theme.muted)
    };
    spans.push(Span::styled(format!(" {ext_label} "), ext_style));
    spans.push(Span::raw("  "));

    let lock_label = if state.locked { "lock:on" } else { "lock:off" };
    let lock_style = if state.locked {
        Style::default()
            .fg(theme.success)
            .add_modifier(Modifier::BOLD)
    } else {
        Style::default().fg(theme.muted)
    };
    spans.push(Span::styled(format!(" {lock_label} "), lock_style));
    spans.push(Span::raw("  "));

    if state.format == SupportedFormat::Canvas {
        let arrow_label = if state.orthogonal_connections {
            "arrow:on"
        } else {
            "arrow:off"
        };
        let arrow_style = if state.orthogonal_connections {
            Style::default()
                .fg(theme.success)
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(theme.muted)
        };
        spans.push(Span::styled(format!(" {arrow_label} "), arrow_style));
        spans.push(Span::raw("  "));
    }

    spans.push(Span::styled(hint_text, Style::default().fg(theme.muted)));

    let hint = Paragraph::new(Line::from(spans)).style(theme.hint_line_bg_style());

    let hint_area = Rect::new(
        total_area.x,
        total_area.bottom().saturating_sub(1),
        total_area.width,
        1,
    );
    frame.render_widget(hint, hint_area);
}

fn render_menu(
    frame: &mut Frame,
    state: &PinstarState,
    theme: &ThemeColors,
    area: Rect,
    mouse_pos: Option<(u16, u16)>,
) {
    if let Some(menu) = &state.context_menu {
        render_context_menu(frame, area, menu, theme, mouse_pos);
    }
}

fn render_rename_popup(
    frame: &mut Frame,
    state: &mut PinstarState,
    theme: &ThemeColors,
    area: Rect,
) {
    state.rename_popup_rect = None;
    if let Some(textarea) = &mut state.rename_popup {
        let popup_area = centered_rect(60, 20, area);
        frame.render_widget(Clear, popup_area);

        textarea.set_style(theme.bg_style());
        textarea.set_block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(Style::default().fg(theme.accent))
                .style(theme.bg_style())
                .title(Span::styled(
                    " Rename Node — Enter: confirm · Esc: cancel ",
                    Style::default().fg(theme.accent),
                )),
        );

        state.rename_popup_rect = Some(Block::default().borders(Borders::ALL).inner(popup_area));
        frame.render_widget(&*textarea, popup_area);
    }
}

fn render_help_overlay(
    frame: &mut Frame,
    state: &mut PinstarState,
    theme: &ThemeColors,
    area: Rect,
) {
    if !state.show_help {
        return;
    }
    let popup_area = centered_rect(80, 85, area);
    frame.render_widget(Clear, popup_area);

    let tab_bar_height = 1u16;
    let footer_height = 1u16;
    let border_height = 2u16;
    let content_height = popup_area
        .height
        .saturating_sub(tab_bar_height + footer_height + border_height);
    let content_width = popup_area.width.saturating_sub(2);

    let tab_titles: Vec<Line> = crate::state::PinstarHelpTab::ALL
        .iter()
        .map(|t| {
            let name = t.title();
            if *t == state.help_tab {
                Line::from(Span::styled(
                    format!(" {name} "),
                    Style::default()
                        .fg(theme.highlight_bg)
                        .bg(theme.accent)
                        .add_modifier(Modifier::BOLD),
                ))
            } else {
                Line::from(Span::styled(
                    format!(" {name} "),
                    Style::default().fg(theme.muted),
                ))
            }
        })
        .collect();

    let tab_bar_area = Rect::new(
        popup_area.x + 1,
        popup_area.y + 1,
        popup_area.width.saturating_sub(2),
        tab_bar_height,
    );

    let tab_spans: Vec<Span> = tab_titles
        .iter()
        .flat_map(|l| {
            let mut s: Vec<Span> = l.spans.to_vec();
            s.push(Span::raw(" "));
            s
        })
        .collect();

    let tab_bar = Paragraph::new(Line::from(tab_spans)).style(theme.bg_style());
    frame.render_widget(tab_bar, tab_bar_area);

    let sep_area = Rect::new(
        popup_area.x + 1,
        popup_area.y + 1 + tab_bar_height,
        popup_area.width.saturating_sub(2),
        1,
    );
    let sep = Paragraph::new(Line::from(Span::styled(
        "─".repeat(sep_area.width as usize),
        Style::default().fg(theme.muted),
    )))
    .style(theme.bg_style());
    frame.render_widget(sep, sep_area);

    let content_area = Rect::new(
        popup_area.x + 1,
        popup_area.y + 2 + tab_bar_height,
        content_width,
        content_height,
    );

    let content_lines = crate::help::help_content(state.help_tab, theme, content_width);
    let max_scroll =
        crate::help::help_content_height(state.help_tab).saturating_sub(content_height);
    state.help_scroll = state.help_scroll.min(max_scroll);

    let content_widget = Paragraph::new(content_lines)
        .scroll((state.help_scroll, 0))
        .style(theme.bg_style())
        .wrap(ratatui::widgets::Wrap { trim: false });
    frame.render_widget(content_widget, content_area);

    let footer_area = Rect::new(
        popup_area.x + 1,
        popup_area.bottom().saturating_sub(1 + footer_height),
        popup_area.width.saturating_sub(2),
        footer_height,
    );
    let footer = Paragraph::new(Line::from(Span::styled(
        " Tab: switch · j/k: scroll · Esc: close ",
        Style::default().fg(theme.muted),
    )))
    .style(theme.bg_style())
    .alignment(Alignment::Center);
    frame.render_widget(footer, footer_area);

    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(theme.accent))
        .style(theme.bg_style())
        .title(Span::styled(
            " Pinstar Help ",
            Style::default()
                .fg(theme.accent)
                .add_modifier(Modifier::BOLD),
        ));
    frame.render_widget(block, popup_area);
}

fn centered_rect(percent_x: u16, percent_y: u16, r: Rect) -> Rect {
    let popup_layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage((100 - percent_y) / 2),
            Constraint::Percentage(percent_y),
            Constraint::Percentage((100 - percent_y) / 2),
        ])
        .split(r);

    Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage((100 - percent_x) / 2),
            Constraint::Percentage(percent_x),
            Constraint::Percentage((100 - percent_x) / 2),
        ])
        .split(popup_layout[1])[1]
}

fn is_generated_id(id: &str) -> bool {
    if id.starts_with("node_") && id.len() <= 16 {
        return true;
    }
    if id.len() == 16 && id.chars().all(|c| c.is_ascii_hexdigit()) {
        return true;
    }
    if id.len() == 36 && id.chars().all(|c| c.is_ascii_hexdigit() || c == '-') {
        return true;
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::Terminal;
    use ratatui::backend::TestBackend;

    fn temp_canvas(content: &str) -> (tempfile::TempDir, std::path::PathBuf) {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("test.canvas");
        std::fs::write(&path, content).unwrap();
        (dir, path)
    }

    #[test]
    fn test_draw_pinstar_view_with_editor() {
        let (_dir, path) = temp_canvas(r#"{"nodes":[],"edges":[]}"#);
        let mut state = PinstarState::load(&path).unwrap();
        state.show_editor_pane = true;
        state.editor_focus = true;

        let backend = TestBackend::new(100, 30);
        let mut terminal = Terminal::new(backend).unwrap();
        let theme = ThemeColors::default();

        terminal
            .draw(|f| {
                let area = f.area();
                draw_pinstar_view(f, &mut state, &theme, area, None);
            })
            .unwrap();

        state.editor_focus = false;
        terminal
            .draw(|f| {
                let area = f.area();
                draw_pinstar_view(f, &mut state, &theme, area, None);
            })
            .unwrap();
    }

    #[test]
    fn test_draw_flowchart_view() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("test.md");
        std::fs::write(&path, "```mermaid\ngraph TD\nA-->B\n```\n").unwrap();
        let mut state = PinstarState::load(&path).unwrap();
        state.fit_to_view(Rect::new(0, 0, 80, 24));

        let backend = TestBackend::new(80, 24);
        let mut terminal = Terminal::new(backend).unwrap();
        let theme = ThemeColors::default();

        terminal
            .draw(|f| {
                let area = f.area();
                draw_pinstar_view(f, &mut state, &theme, area, None);
            })
            .unwrap();
        // Some nodes must be projected on screen after fit.
        assert!(!state.data.nodes.is_empty());
    }

    #[test]
    fn marquee_and_menu_render_without_panic() {
        let (_dir, path) = temp_canvas(r#"{"nodes":[],"edges":[]}"#);
        let mut state = PinstarState::load(&path).unwrap();
        state.marquee.on_down(0.0, 0.0);
        state.marquee.on_drag(50.0, 20.0);
        state.open_context_menu(5, 5, 0.0, 0.0);

        let backend = TestBackend::new(80, 24);
        let mut terminal = Terminal::new(backend).unwrap();
        let theme = ThemeColors::default();
        terminal
            .draw(|f| {
                draw_pinstar_view(f, &mut state, &theme, f.area(), Some((6, 6)));
            })
            .unwrap();
    }
}
