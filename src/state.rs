use crate::camera;
use crate::data::CanvasData;
use crate::formats::{self, SupportedFormat};
use crate::menu::{PinstarContextMenu, PinstarMenuType, menu_specs};
use crate::overlay::MarqueeState;
use crate::selection::Selection;
use crate::textsel::MouseTextSelection;

use anyhow::Result;
use ratatui_textarea::{TextArea, WrapMode};
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PinstarHelpTab {
    Keyboard,
    Mouse,
    Menus,
    Formats,
}

impl PinstarHelpTab {
    pub const ALL: [PinstarHelpTab; 4] = [
        PinstarHelpTab::Keyboard,
        PinstarHelpTab::Mouse,
        PinstarHelpTab::Menus,
        PinstarHelpTab::Formats,
    ];

    pub fn title(self) -> &'static str {
        match self {
            PinstarHelpTab::Keyboard => "Keyboard",
            PinstarHelpTab::Mouse => "Mouse",
            PinstarHelpTab::Menus => "Menus",
            PinstarHelpTab::Formats => "Formats",
        }
    }

    pub fn next(self) -> Self {
        match self {
            PinstarHelpTab::Keyboard => PinstarHelpTab::Mouse,
            PinstarHelpTab::Mouse => PinstarHelpTab::Menus,
            PinstarHelpTab::Menus => PinstarHelpTab::Formats,
            PinstarHelpTab::Formats => PinstarHelpTab::Keyboard,
        }
    }

    pub fn prev(self) -> Self {
        match self {
            PinstarHelpTab::Keyboard => PinstarHelpTab::Formats,
            PinstarHelpTab::Mouse => PinstarHelpTab::Keyboard,
            PinstarHelpTab::Menus => PinstarHelpTab::Mouse,
            PinstarHelpTab::Formats => PinstarHelpTab::Menus,
        }
    }
}

#[derive(Clone)]
pub struct PinstarSnapshot {
    pub data: CanvasData,
    pub has_layout_override: bool,
}

/// Which flavor of rename the popup performs: clin renames the node title,
/// the standalone bin renames the node id (rewiring edges).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RenameMode {
    Title,
    Id,
}

pub struct PinstarState {
    pub path: PathBuf,
    pub format: SupportedFormat,
    pub data: CanvasData,
    pub settings: crate::Settings,
    pub viewport_x: f64,
    pub viewport_y: f64,
    pub zoom: f64,
    pub selection: Selection<String>,
    pub selected_edge_id: Option<String>,
    pub floating_editor: Option<TextArea<'static>>,
    pub raw_editor: TextArea<'static>,
    pub editor_focus: bool,
    pub mouse_pos: Option<(u16, u16)>,
    pub last_mouse_pos: Option<(u16, u16)>,
    pub last_click: Option<(u16, u16, std::time::Instant)>,
    pub context_menu: Option<PinstarContextMenu>,
    pub context_menu_pos: (f64, f64),
    pub connection_source_id: Option<String>,
    pub resizing_node_id: Option<String>,
    pub is_dragging_resize_handle: bool,
    pub deleting_connection_source_id: Option<String>,
    pub trigger_ext_editor: bool,
    pub trigger_whole_file_editor: bool,
    pub trigger_image_picker: bool,
    pub show_editor_pane: bool,
    pub drag_start_pos: Option<(f64, f64)>,
    pub rename_popup: Option<TextArea<'static>>,
    pub ext_editor_enabled: bool,
    pub last_mouse_canvas_pos: Option<(f64, f64)>,
    pub drag_captured_nodes: std::collections::HashSet<String>,
    pub drag_group_children: std::collections::HashSet<String>,
    pub show_grid: bool,
    pub mouse_selecting: bool,
    pub mouse_dragged: bool,
    pub locked: bool,
    pub last_modified: std::time::SystemTime,
    pub orthogonal_connections: bool,
    pub show_help: bool,
    pub help_tab: PinstarHelpTab,
    pub help_scroll: u16,
    pub select_rect_start: Option<(f64, f64)>,
    pub select_rect_end: Option<(f64, f64)>,
    pub has_layout_override: bool,
    pub undo_stack: Vec<PinstarSnapshot>,
    pub redo_stack: Vec<PinstarSnapshot>,
    // clin-side interaction state
    pub(crate) mouse_selection: MouseTextSelection,
    pub(crate) text_selection_target: Option<PinstarTextField>,
    pub(crate) floating_editor_rect: Option<ratatui::layout::Rect>,
    pub rename_popup_rect: Option<ratatui::layout::Rect>,
    pub edge_overlay_rect: Option<ratatui::layout::Rect>,
    pub help_requested: bool,
    pub footer_hint: String,
    pub last_area: ratatui::layout::Rect,
    pub is_panning: bool,
    pub marquee: MarqueeState,
    pub right_down_screen: Option<(u16, u16)>,
    pub last_zoom_at: Option<std::time::Instant>,
    pub has_dragged: bool,
    #[cfg(feature = "images")]
    pub image_cache: crate::image::ImageCache,
    #[cfg(feature = "images")]
    pub image_picker: Option<ratatui_image::picker::Picker>,
    #[cfg(feature = "images")]
    pub image_decode_tx: Option<std::sync::mpsc::Sender<crate::image::ImageJob>>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum PinstarTextField {
    Raw,
    Floating,
}

impl PinstarState {
    pub fn load(path: &Path) -> Result<Self> {
        let format = formats::detect_format(path);
        let content = std::fs::read_to_string(path).unwrap_or_default();
        let data = formats::load_from_format(path, &content, format)?;
        let mut raw_editor = TextArea::from(content.lines().map(String::from).collect::<Vec<_>>());
        raw_editor.set_cursor_line_style(ratatui::style::Style::default());
        raw_editor.set_wrap_mode(WrapMode::WordOrGlyph);

        let locked = format != SupportedFormat::Canvas;
        let last_modified = std::fs::metadata(path)
            .and_then(|m| m.modified())
            .unwrap_or_else(|_| std::time::SystemTime::now());

        let has_layout_override = content.contains("pinstar_layout:");

        Ok(Self {
            path: path.to_path_buf(),
            format,
            data,
            settings: crate::Settings::default(),
            locked,
            last_modified,
            viewport_x: 0.0,
            viewport_y: 0.0,
            zoom: 0.1,
            selection: Selection::new(),
            selected_edge_id: None,
            floating_editor: None,
            raw_editor,
            editor_focus: false,
            mouse_pos: None,
            last_mouse_pos: None,
            last_click: None,
            context_menu: None,
            context_menu_pos: (0.0, 0.0),
            connection_source_id: None,
            resizing_node_id: None,
            is_dragging_resize_handle: false,
            deleting_connection_source_id: None,
            trigger_ext_editor: false,
            trigger_whole_file_editor: false,
            trigger_image_picker: false,
            show_editor_pane: false,
            drag_start_pos: None,
            rename_popup: None,
            ext_editor_enabled: false,
            last_mouse_canvas_pos: None,
            drag_captured_nodes: std::collections::HashSet::new(),
            drag_group_children: std::collections::HashSet::new(),
            show_grid: true,
            mouse_selecting: false,
            mouse_dragged: false,
            orthogonal_connections: false,
            show_help: false,
            help_tab: PinstarHelpTab::Keyboard,
            help_scroll: 0,
            select_rect_start: None,
            select_rect_end: None,
            has_layout_override,
            undo_stack: Vec::new(),
            redo_stack: Vec::new(),
            mouse_selection: MouseTextSelection::default(),
            text_selection_target: None,
            floating_editor_rect: None,
            rename_popup_rect: None,
            edge_overlay_rect: None,
            help_requested: false,
            footer_hint: String::new(),
            last_area: ratatui::layout::Rect::default(),
            is_panning: false,
            marquee: MarqueeState::new(3),
            right_down_screen: None,
            last_zoom_at: None,
            has_dragged: false,
            #[cfg(feature = "images")]
            image_cache: crate::image::ImageCache::new(32),
            #[cfg(feature = "images")]
            image_picker: None,
            #[cfg(feature = "images")]
            image_decode_tx: None,
        })
    }

    pub fn save(&mut self) -> Result<()> {
        let original = std::fs::read_to_string(&self.path).unwrap_or_default();
        let content =
            formats::save_to_format(&self.data, &original, self.format, self.has_layout_override)?;
        crate::atomic_write(&self.path, &content)?;

        self.raw_editor = TextArea::from(content.lines().map(String::from).collect::<Vec<_>>());
        self.raw_editor
            .set_cursor_line_style(ratatui::style::Style::default());
        self.raw_editor.set_wrap_mode(WrapMode::WordOrGlyph);

        if let Ok(metadata) = std::fs::metadata(&self.path) {
            if let Ok(modified) = metadata.modified() {
                self.last_modified = modified;
            }
        }
        Ok(())
    }

    /// Returns the header-bar status message for the active transient mode
    /// (connection / delete-connection / resize), or None when idle.
    pub fn active_mode_message(&self) -> Option<&'static str> {
        if self.connection_source_id.is_some() {
            Some("CONNECTION MODE: Select target node with mouse or Enter")
        } else if self.deleting_connection_source_id.is_some() {
            Some("DELETE CONNECTION MODE: Select target node to remove link")
        } else if self.resizing_node_id.is_some() {
            Some("RESIZE MODE: Drag mouse to resize, Right-click to confirm")
        } else {
            None
        }
    }

    pub fn record_undo_state(&mut self) {
        let snapshot = PinstarSnapshot {
            data: self.data.clone(),
            has_layout_override: self.has_layout_override,
        };
        self.undo_stack.push(snapshot);
        if self.undo_stack.len() > 100 {
            self.undo_stack.remove(0);
        }
        self.redo_stack.clear();
    }

    fn prune_selection_after_restore(&mut self) {
        if let Some(sel_id) = self.selection.primary.clone() {
            if !self.data.nodes.iter().any(|n| n.id() == sel_id) {
                self.selection.primary = None;
            }
        }
        self.selection
            .extra
            .retain(|id| self.data.nodes.iter().any(|n| n.id() == id));
        self.drag_captured_nodes.clear();
        if let Some(sel_id) = &self.selected_edge_id {
            if !self.data.edges.iter().any(|e| &e.id == sel_id) {
                self.selected_edge_id = None;
            }
        }
    }

    pub fn undo(&mut self) -> Result<()> {
        if let Some(snapshot) = self.undo_stack.pop() {
            let current = PinstarSnapshot {
                data: self.data.clone(),
                has_layout_override: self.has_layout_override,
            };
            self.redo_stack.push(current);

            self.data = snapshot.data;
            self.has_layout_override = snapshot.has_layout_override;
            self.prune_selection_after_restore();

            self.save()?;
        }
        Ok(())
    }

    pub fn redo(&mut self) -> Result<()> {
        if let Some(snapshot) = self.redo_stack.pop() {
            let current = PinstarSnapshot {
                data: self.data.clone(),
                has_layout_override: self.has_layout_override,
            };
            self.undo_stack.push(current);

            self.data = snapshot.data;
            self.has_layout_override = snapshot.has_layout_override;
            self.prune_selection_after_restore();

            self.save()?;
        }
        Ok(())
    }

    pub fn reload(&mut self) -> Result<()> {
        let content = std::fs::read_to_string(&self.path)?;
        let data = formats::load_from_format(&self.path, &content, self.format)?;
        self.data = data;
        self.has_layout_override = content.contains("pinstar_layout:");
        self.raw_editor = TextArea::from(content.lines().map(String::from).collect::<Vec<_>>());
        self.raw_editor
            .set_cursor_line_style(ratatui::style::Style::default());
        self.raw_editor.set_wrap_mode(WrapMode::WordOrGlyph);

        self.prune_selection_after_restore();

        if let Ok(metadata) = std::fs::metadata(&self.path) {
            if let Ok(modified) = metadata.modified() {
                self.last_modified = modified;
            }
        }
        Ok(())
    }

    // ── selection ──────────────────────────────────────────────────────────

    pub fn all_selected_node_ids(&self) -> std::collections::HashSet<String> {
        self.selection.all()
    }

    pub fn select_nodes_in_rect(&mut self, x1: f64, y1: f64, x2: f64, y2: f64) {
        let (min_x, max_x) = if x1 < x2 { (x1, x2) } else { (x2, x1) };
        let (min_y, max_y) = if y1 < y2 { (y1, y2) } else { (y2, y1) };
        let ids: std::collections::HashSet<String> = self
            .data
            .nodes
            .iter()
            .filter(|node| {
                let (nx, ny) = node.pos();
                let (nw, nh) = node.size();
                match node {
                    crate::data::CanvasNode::Group(_) => {
                        let title_height = (2.0 / self.zoom).min(nh);
                        nx + nw > min_x && nx < max_x && ny + title_height > min_y && ny < max_y
                    }
                    _ => nx + nw > min_x && nx < max_x && ny + nh > min_y && ny < max_y,
                }
            })
            .map(|n| n.id().to_string())
            .collect();

        if !ids.is_empty() {
            let primary = ids.iter().next().cloned();
            self.selection.replace_set(ids, primary);
            return;
        }

        self.selection.clear();

        if self.format == SupportedFormat::Canvas || self.format == SupportedFormat::Mermaid {
            self.selected_edge_id = None;
            return;
        }

        // No nodes inside the box: fall back to selecting intersecting
        // connections (flowchart formats).
        let line_intersects_rect = |sx: f64,
                                    sy: f64,
                                    ex: f64,
                                    ey: f64,
                                    min_x: f64,
                                    min_y: f64,
                                    max_x: f64,
                                    max_y: f64|
         -> bool {
            let inside = |x: f64, y: f64| x >= min_x && x <= max_x && y >= min_y && y <= max_y;
            if inside(sx, sy) || inside(ex, ey) {
                return true;
            }
            let intersect =
                |x1: f64, y1: f64, x2: f64, y2: f64, x3: f64, y3: f64, x4: f64, y4: f64| -> bool {
                    let denom = (y4 - y3) * (x2 - x1) - (x4 - x3) * (y2 - y1);
                    if denom.abs() < 0.0001 {
                        return false;
                    }
                    let ua = ((x4 - x3) * (y1 - y3) - (y4 - y3) * (x1 - x3)) / denom;
                    let ub = ((x2 - x1) * (y1 - y3) - (y2 - y1) * (x1 - x3)) / denom;
                    (0.0..=1.0).contains(&ua) && (0.0..=1.0).contains(&ub)
                };
            intersect(sx, sy, ex, ey, min_x, min_y, max_x, min_y)
                || intersect(sx, sy, ex, ey, min_x, max_y, max_x, max_y)
                || intersect(sx, sy, ex, ey, min_x, min_y, min_x, max_y)
                || intersect(sx, sy, ex, ey, max_x, min_y, max_x, max_y)
        };

        let mut found_edge = None;
        for edge in &self.data.edges {
            if let Some(segments) = self.get_edge_segments(edge) {
                let intersects = segments.iter().any(|&(sx, sy, ex, ey)| {
                    line_intersects_rect(sx, sy, ex, ey, min_x, min_y, max_x, max_y)
                });
                if intersects {
                    found_edge = Some(edge.id.clone());
                    break;
                }
            }
        }
        self.selected_edge_id = found_edge;
    }

    pub fn delete_selected_node(&mut self) {
        let ids = self.selection.all();
        if ids.is_empty() {
            return;
        }
        self.record_undo_state();
        self.data.nodes.retain(|n| !ids.contains(n.id()));
        self.data
            .edges
            .retain(|e| !ids.contains(&e.from_node) && !ids.contains(&e.to_node));
        self.selection.clear();
        let _ = self.save();
    }

    // ── edges ──────────────────────────────────────────────────────────────

    pub fn get_edge_segments(
        &self,
        edge: &crate::data::CanvasEdge,
    ) -> Option<Vec<(f64, f64, f64, f64)>> {
        let from_node = self.data.nodes.iter().find(|n| n.id() == edge.from_node)?;
        let to_node = self.data.nodes.iter().find(|n| n.id() == edge.to_node)?;

        let (fx, fy) = from_node.pos();
        let (fw, fh) = from_node.size();
        let (tx, ty) = to_node.pos();
        let (tw, th) = to_node.size();

        let scx = fx + fw / 2.0;
        let scy = fy + fh / 2.0;
        let tcx = tx + tw / 2.0;
        let tcy = ty + th / 2.0;

        let dx = tcx - scx;
        let dy = tcy - scy;
        let is_horiz = dx.abs() > dy.abs();

        let (ax, ay) = if is_horiz {
            if dx > 0.0 { (fx + fw, scy) } else { (fx, scy) }
        } else if dy > 0.0 {
            (scx, fy + fh)
        } else {
            (scx, fy)
        };

        let (bx, by) = if is_horiz {
            if dx > 0.0 { (tx, tcy) } else { (tx + tw, tcy) }
        } else if dy > 0.0 {
            (tcx, ty)
        } else {
            (tcx, ty + th)
        };

        let use_orthogonal = if self.format == SupportedFormat::Canvas {
            self.orthogonal_connections
        } else {
            true
        };

        let segments = if use_orthogonal {
            if is_horiz {
                let mid_x = (ax + bx) / 2.0;
                vec![
                    (ax, ay, mid_x, ay),
                    (mid_x, ay, mid_x, by),
                    (mid_x, by, bx, by),
                ]
            } else {
                let mid_y = (ay + by) / 2.0;
                vec![
                    (ax, ay, ax, mid_y),
                    (ax, mid_y, bx, mid_y),
                    (bx, mid_y, bx, by),
                ]
            }
        } else {
            vec![(ax, ay, bx, by)]
        };

        Some(segments)
    }

    pub fn select_edge_at(&mut self, cx: f64, cy: f64) -> Option<String> {
        let tolerance = 5.0 / self.zoom;
        let mut best: Option<(String, f64)> = None;
        for edge in &self.data.edges {
            let Some(seg) = self.get_edge_segments(edge) else {
                continue;
            };
            for &(sx, sy, ex, ey) in &seg {
                let dx = ex - sx;
                let dy = ey - sy;
                let len_sq = dx * dx + dy * dy;
                if len_sq == 0.0 {
                    let dist = ((cx - sx).powi(2) + (cy - sy).powi(2)).sqrt();
                    if dist < tolerance {
                        match &best {
                            Some((_, bd)) if dist >= *bd => {}
                            _ => best = Some((edge.id.clone(), dist)),
                        }
                    }
                    continue;
                }
                let t = ((cx - sx) * dx + (cy - sy) * dy) / len_sq;
                let t_clamped = t.clamp(0.0, 1.0);
                let px = sx + t_clamped * dx;
                let py = sy + t_clamped * dy;
                let dist = ((cx - px).powi(2) + (cy - py).powi(2)).sqrt();
                if dist < tolerance {
                    match &best {
                        Some((_, bd)) if dist >= *bd => {}
                        _ => best = Some((edge.id.clone(), dist)),
                    }
                }
            }
        }
        if let Some((id, _)) = best {
            self.selected_edge_id = Some(id.clone());
            self.selection.clear();
            Some(id)
        } else {
            self.selected_edge_id = None;
            None
        }
    }

    pub fn set_edge_color(&mut self, color: Option<String>) {
        if let Some(id) = &self.selected_edge_id {
            let id = id.clone();
            self.record_undo_state();
            for edge in &mut self.data.edges {
                if edge.id == id {
                    edge.color = color;
                    break;
                }
            }
            let _ = self.save();
            self.sync_to_raw_editor();
        }
    }

    pub fn set_edge_style(&mut self, style: crate::data::EdgeStyle) {
        if let Some(id) = &self.selected_edge_id {
            let id = id.clone();
            self.record_undo_state();
            for edge in &mut self.data.edges {
                if edge.id == id {
                    edge.style = style;
                    break;
                }
            }
            let _ = self.save();
            self.sync_to_raw_editor();
        }
    }

    pub fn open_edge_context_menu(&mut self, x: u16, y: u16) {
        let specs = menu_specs(
            PinstarMenuType::EdgeMenu,
            false,
            self.format,
            self.settings.enable_image_nodes,
        );
        if specs.is_empty() {
            return;
        }
        self.context_menu = Some(PinstarContextMenu::new(
            x,
            y,
            specs,
            PinstarMenuType::EdgeMenu,
        ));
    }

    /// Opens the edge context menu centered in the given view area.
    pub fn open_edge_menu_centered(&mut self, area: ratatui::layout::Rect) {
        let menu_x = (area.width / 2).saturating_sub(12);
        let menu_y = area.height;
        self.open_edge_context_menu(menu_x, menu_y);
    }

    /// Edges connected to the currently selected node, in stable storage
    /// order. Used by the edge-list overlay and number-key selection.
    pub fn selected_node_edges(&self) -> Vec<&crate::data::CanvasEdge> {
        let Some(node_id) = &self.selection.primary else {
            return Vec::new();
        };
        self.data
            .edges
            .iter()
            .filter(|e| e.from_node == *node_id || e.to_node == *node_id)
            .collect()
    }

    /// Selects the edge at the given 1-based index among the selected node's
    /// connected edges (deselecting the node). Returns its id, or None if out
    /// of range / no node selected.
    pub fn select_edge_of_selected_node(&mut self, index: usize) -> Option<String> {
        let edge_id = {
            let edges = self.selected_node_edges();
            if index >= 1 && index <= edges.len() {
                Some(edges[index - 1].id.clone())
            } else {
                None
            }
        };
        if let Some(edge_id) = edge_id {
            self.selected_edge_id = Some(edge_id.clone());
            self.selection.clear();
            Some(edge_id)
        } else {
            None
        }
    }

    // ── viewport ───────────────────────────────────────────────────────────

    /// Returns true while the view is undergoing continuous transforms
    /// (pan, zoom, node resize, connection drawing). During these states
    /// the pixel image render is suppressed to avoid churning the encode
    /// worker; cheap placeholder text is shown instead.
    pub fn is_view_transforming(&self) -> bool {
        self.resizing_node_id.is_some()
            || self.is_dragging_resize_handle
            || self.drag_start_pos.is_some()
            || self.is_panning
            || self.connection_source_id.is_some()
            || self.deleting_connection_source_id.is_some()
            || self
                .last_zoom_at
                .is_some_and(|t| t.elapsed() < crate::TRANSFORM_SETTLE)
    }

    pub fn sync_from_raw_editor(&mut self) -> Result<()> {
        let content = self.raw_editor.lines().join("\n");
        if let Ok(mut data) = formats::load_from_format(&self.path, &content, self.format) {
            self.record_undo_state();
            // Force re-layout if orientation changed
            if data.orientation != self.data.orientation && self.format != SupportedFormat::Canvas {
                formats::apply_hierarchical_layout(&mut data);
            }
            self.has_layout_override = content.contains("pinstar_layout:");
            self.data = data;
            let _ = self.save();
            Ok(())
        } else {
            anyhow::bail!("Invalid diagram syntax in editor")
        }
    }

    pub fn sync_to_raw_editor(&mut self) {
        if let Ok(content) = serde_json::to_string_pretty(&self.data) {
            self.raw_editor = TextArea::from(content.lines().map(String::from).collect::<Vec<_>>());
            self.raw_editor
                .set_cursor_line_style(ratatui::style::Style::default());
        }
    }

    pub fn pan(&mut self, dx: f64, dy: f64) {
        if let Some((nx, ny)) = camera::pan_centered(
            self.viewport_x,
            self.viewport_y,
            dx / self.zoom,
            dy / self.zoom,
        ) {
            self.viewport_x = nx;
            self.viewport_y = ny;
        }
    }

    pub fn zoom_in(&mut self) {
        if let Some(z) = camera::zoom_step(self.zoom, 1.1, camera::ZoomDir::In, 0.0) {
            self.zoom = z;
        }
        self.last_zoom_at = Some(std::time::Instant::now());
    }

    pub fn zoom_out(&mut self) {
        if let Some(z) = camera::zoom_step(
            self.zoom,
            1.1,
            camera::ZoomDir::Out,
            camera::CANVAS_ZOOM_MIN,
        ) {
            self.zoom = z;
        }
        self.last_zoom_at = Some(std::time::Instant::now());
    }

    pub fn fit_to_view(&mut self, area: ratatui::layout::Rect) {
        if self.data.nodes.is_empty() {
            return;
        }

        let min_x = self
            .data
            .nodes
            .iter()
            .map(|n| n.pos().0)
            .reduce(f64::min)
            .unwrap_or(0.0);
        let min_y = self
            .data
            .nodes
            .iter()
            .map(|n| n.pos().1)
            .reduce(f64::min)
            .unwrap_or(0.0);
        let max_x = self
            .data
            .nodes
            .iter()
            .map(|n| n.pos().0 + n.size().0)
            .reduce(f64::max)
            .unwrap_or(0.0);
        let max_y = self
            .data
            .nodes
            .iter()
            .map(|n| n.pos().1 + n.size().1)
            .reduce(f64::max)
            .unwrap_or(0.0);

        let cx = (min_x + max_x) / 2.0;
        let cy = (min_y + max_y) / 2.0;

        let padding = 100.0;
        let bbox_w = (max_x - min_x) + padding * 2.0;
        let bbox_h = (max_y - min_y) + padding * 2.0;

        let avail_w = area.width as f64;
        let avail_h = (area.height.saturating_sub(1)) as f64;

        let zoom_x = if bbox_w > 0.0 { avail_w / bbox_w } else { 1.0 };
        let zoom_y = if bbox_h > 0.0 { avail_h / bbox_h } else { 1.0 };
        let zoom = zoom_x.min(zoom_y).clamp(0.01, 10.0);

        self.viewport_x = cx;
        self.viewport_y = cy;
        self.zoom = zoom;
    }

    pub fn screen_to_canvas(&self, sx: u16, sy: u16, area: ratatui::layout::Rect) -> (f64, f64) {
        let cx = ((sx as f64 + 0.5) - (area.x as f64 + area.width as f64 / 2.0)) / self.zoom
            + self.viewport_x;
        let cy = ((sy as f64 + 0.5) - (area.y as f64 + area.height as f64 / 2.0)) / self.zoom
            + self.viewport_y;
        (camera::clamp_world(cx), camera::clamp_world(cy))
    }

    pub fn node_at(&self, x: f64, y: f64) -> Option<String> {
        let mut best_hit: Option<(String, f64, usize)> = None;

        for (idx, node) in self.data.nodes.iter().enumerate() {
            let (nx, ny) = node.pos();
            let (nw, nh) = node.size();
            let margin = 1.0 / self.zoom;
            let is_hit = match node {
                crate::data::CanvasNode::Group(_) => {
                    let title_height = (2.0 / self.zoom).min(nh);
                    x >= nx - margin
                        && x <= nx + nw + margin
                        && y >= ny - margin
                        && y <= ny + title_height
                }
                _ => {
                    x >= nx - margin
                        && x <= nx + nw + margin
                        && y >= ny - margin
                        && y <= ny + nh + margin
                }
            };
            if is_hit {
                let area = nw * nh;
                let should_replace = match &best_hit {
                    None => true,
                    Some((_, best_area, _)) if area < *best_area => true,
                    Some((_, best_area, best_idx))
                        if (area - *best_area).abs() < 0.0001 && idx > *best_idx =>
                    {
                        true
                    }
                    _ => false,
                };
                if should_replace {
                    best_hit = Some((node.id().to_string(), area, idx));
                }
            }
        }
        best_hit.map(|(id, _, _)| id)
    }

    pub fn select_node_at(&mut self, x: f64, y: f64) -> Option<String> {
        if let Some(id) = self.node_at(x, y) {
            self.selection.select_only(id.clone());
            self.selected_edge_id = None;
            Some(id)
        } else {
            self.selection.clear();
            self.selected_edge_id = None;
            None
        }
    }

    pub fn center_on_selected(&mut self) {
        if let Some(id) = &self.selection.primary
            && let Some(node) = self.data.nodes.iter().find(|n| n.id() == id)
        {
            let (nx, ny) = node.pos();
            let (nw, nh) = node.size();
            self.viewport_x = nx + nw / 2.0;
            self.viewport_y = ny + nh / 2.0;
        }
    }

    pub fn select_node_in_direction(&mut self, dx: f64, dy: f64) {
        let current_node = if let Some(id) = &self.selection.primary {
            self.data.nodes.iter().find(|n| n.id() == id)
        } else {
            None
        };

        let origin = if let Some(n) = current_node {
            let (nx, ny) = n.pos();
            let (nw, nh) = n.size();
            (nx + nw / 2.0, ny + nh / 2.0)
        } else {
            (self.viewport_x, self.viewport_y)
        };

        let mut ids: Vec<String> = Vec::new();
        let mut cands: Vec<(f64, f64)> = Vec::new();
        for node in &self.data.nodes {
            if let Some(id) = &self.selection.primary
                && node.id() == id
            {
                continue;
            }
            let (nx, ny) = node.pos();
            let (nw, nh) = node.size();
            ids.push(node.id().to_string());
            cands.push((nx + nw / 2.0, ny + nh / 2.0));
        }

        if let Some(i) =
            camera::nearest_in_dir(&cands, origin, (dx, dy), std::f64::consts::FRAC_PI_3)
        {
            self.selection.select_only(ids[i].clone());
        } else if self.selection.primary.is_none() && !self.data.nodes.is_empty() {
            self.selection
                .select_only(self.data.nodes[0].id().to_string());
        }
    }

    // ── editors & menus ────────────────────────────────────────────────────

    pub fn toggle_editor(&mut self) {
        let editor_text = if let Some(editor) = &self.floating_editor {
            Some(editor.lines().join("\n"))
        } else {
            None
        };
        if let Some(text) = editor_text {
            if self.selection.primary.is_some() {
                self.record_undo_state();
                let node_id = self
                    .selection
                    .primary
                    .as_ref()
                    .expect("checked is_some above")
                    .clone();
                for node in &mut self.data.nodes {
                    if node.id() == node_id {
                        node.set_text(text);
                        break;
                    }
                }
                let _ = self.save();
            }
            self.floating_editor = None;
        } else if let Some(node_id) = &self.selection.primary
            && let Some(node) = self.data.nodes.iter().find(|n| n.id() == node_id)
        {
            let mut textarea =
                TextArea::from(node.text().lines().map(String::from).collect::<Vec<_>>());
            textarea.set_cursor_line_style(ratatui::style::Style::default());
            textarea.set_wrap_mode(WrapMode::WordOrGlyph);
            self.floating_editor = Some(textarea);
        }
    }

    pub fn open_context_menu(&mut self, x: u16, y: u16, canvas_x: f64, canvas_y: f64) {
        let specs = menu_specs(
            PinstarMenuType::Canvas,
            self.selection.primary.is_some(),
            self.format,
            self.settings.enable_image_nodes,
        );
        self.context_menu_pos = (canvas_x, canvas_y);
        self.context_menu = Some(PinstarContextMenu::new(
            x,
            y,
            specs,
            PinstarMenuType::Canvas,
        ));
    }

    pub fn open_editor_context_menu(&mut self, x: u16, y: u16) {
        let specs = menu_specs(
            PinstarMenuType::Editor,
            false,
            self.format,
            self.settings.enable_image_nodes,
        );
        self.context_menu = Some(PinstarContextMenu::new(
            x,
            y,
            specs,
            PinstarMenuType::Editor,
        ));
    }

    pub fn open_color_menu(&mut self, x: u16, y: u16, edge: bool) {
        let kind = if edge {
            PinstarMenuType::EdgeColorPicker
        } else {
            PinstarMenuType::ColorPicker
        };
        let specs = menu_specs(kind, false, self.format, self.settings.enable_image_nodes);
        self.context_menu = Some(PinstarContextMenu::new(x, y, specs, kind));
    }

    pub fn open_shape_menu(&mut self, x: u16, y: u16) {
        let specs = menu_specs(
            PinstarMenuType::ShapePicker,
            false,
            self.format,
            self.settings.enable_image_nodes,
        );
        self.context_menu = Some(PinstarContextMenu::new(
            x,
            y,
            specs,
            PinstarMenuType::ShapePicker,
        ));
    }

    pub fn open_orientation_menu(&mut self, x: u16, y: u16) {
        let specs = menu_specs(
            PinstarMenuType::OrientationPicker,
            false,
            self.format,
            self.settings.enable_image_nodes,
        );
        self.context_menu = Some(PinstarContextMenu::new(
            x,
            y,
            specs,
            PinstarMenuType::OrientationPicker,
        ));
    }

    /// Open the rename popup seeded from the selected node.
    /// `Title` mode renames the node title (clin); `Id` mode renames the node
    /// id and rewires edges (standalone bin).
    pub fn open_rename_popup(&mut self, mode: RenameMode) {
        let Some(id) = self.selection.primary.clone() else {
            return;
        };
        let mut textarea = match mode {
            RenameMode::Title => {
                let current_title = self
                    .data
                    .nodes
                    .iter()
                    .find(|n| n.id() == id)
                    .and_then(|n| n.title())
                    .unwrap_or("")
                    .to_string();
                TextArea::from(vec![current_title])
            }
            RenameMode::Id => TextArea::from(vec![id]),
        };
        textarea.set_cursor_line_style(ratatui::style::Style::default());
        let title = match mode {
            RenameMode::Title => " Rename Node - Enter to confirm, Esc to cancel ",
            RenameMode::Id => " Rename Node (ID) - Enter to confirm, Esc to cancel ",
        };
        textarea.set_block(
            ratatui::widgets::Block::default()
                .borders(ratatui::widgets::Borders::ALL)
                .title(title),
        );
        self.rename_popup = Some(textarea);
    }

    pub fn rename_node_title(&mut self, new_title: String) {
        let node_id = self.selection.primary.clone();
        if let Some(node_id) = node_id {
            self.record_undo_state();
            let trimmed = new_title.trim().to_string();
            let title = if trimmed.is_empty() {
                None
            } else {
                Some(trimmed)
            };

            for node in &mut self.data.nodes {
                if node.id() == node_id {
                    node.set_title(title);
                    break;
                }
            }

            let _ = self.save();
        }
    }

    pub fn rename_node_id(&mut self, new_id: String) {
        let Some(old_id) = self.selection.primary.clone() else {
            return;
        };
        self.record_undo_state();
        if old_id == new_id {
            return;
        }
        let final_id = if new_id.is_empty() {
            uuid::Uuid::new_v4().to_string()
        } else {
            new_id
        };

        for node in &mut self.data.nodes {
            match node {
                crate::data::CanvasNode::Text(n) if n.id == old_id => n.id = final_id.clone(),
                crate::data::CanvasNode::File(n) if n.id == old_id => n.id = final_id.clone(),
                crate::data::CanvasNode::Link(n) if n.id == old_id => n.id = final_id.clone(),
                crate::data::CanvasNode::Group(n) if n.id == old_id => n.id = final_id.clone(),
                _ => {}
            }
        }

        for edge in &mut self.data.edges {
            if edge.from_node == old_id {
                edge.from_node = final_id.clone();
            }
            if edge.to_node == old_id {
                edge.to_node = final_id.clone();
            }
        }

        self.selection.select_only(final_id);
        let _ = self.save();
    }

    // ── node mutations ─────────────────────────────────────────────────────

    pub fn start_resize(&mut self) {
        let id = self.selection.primary.clone();
        if let Some(id) = id {
            self.record_undo_state();
            self.resizing_node_id = Some(id.clone());
            self.context_menu = None;
        }
    }

    pub fn start_delete_connection(&mut self) {
        if let Some(id) = &self.selection.primary {
            self.deleting_connection_source_id = Some(id.clone());
            self.context_menu = None;
        }
    }

    pub fn delete_node_connections(&mut self) {
        let ids = self.selection.all();
        if ids.is_empty() {
            return;
        }
        self.record_undo_state();
        self.data
            .edges
            .retain(|e| !ids.contains(&e.from_node) && !ids.contains(&e.to_node));
        let _ = self.save();
    }

    pub fn set_node_color(&mut self, color: Option<String>) {
        let ids = self.selection.all();
        if ids.is_empty() {
            return;
        }
        self.record_undo_state();
        for node in &mut self.data.nodes {
            if ids.contains(node.id()) {
                match node {
                    crate::data::CanvasNode::Text(n) => n.color = color.clone(),
                    crate::data::CanvasNode::File(n) => n.color = color.clone(),
                    crate::data::CanvasNode::Link(n) => n.color = color.clone(),
                    crate::data::CanvasNode::Group(n) => n.color = color.clone(),
                }
            }
        }
        let _ = self.save();
    }

    pub fn set_node_shape(&mut self, shape: crate::data::NodeShape) {
        let ids = self.selection.all();
        if ids.is_empty() {
            return;
        }
        self.record_undo_state();
        for node in &mut self.data.nodes {
            if ids.contains(node.id()) {
                if let crate::data::CanvasNode::Text(n) = node {
                    n.shape = shape;
                }
            }
        }
        let _ = self.save();
    }

    pub fn set_orientation(&mut self, orientation: crate::data::DiagramOrientation) {
        self.record_undo_state();
        self.data.orientation = orientation;
        let _ = self.save();
    }

    pub fn cycle_orientation(&mut self) {
        self.record_undo_state();
        self.data.orientation = match self.data.orientation {
            crate::data::DiagramOrientation::TopDown => crate::data::DiagramOrientation::LeftRight,
            crate::data::DiagramOrientation::LeftRight => {
                crate::data::DiagramOrientation::RightLeft
            }
            crate::data::DiagramOrientation::RightLeft => crate::data::DiagramOrientation::DownTop,
            crate::data::DiagramOrientation::DownTop => crate::data::DiagramOrientation::TopDown,
        };
        formats::apply_hierarchical_layout(&mut self.data);
        self.has_layout_override = false;
        let _ = self.save();
    }

    pub fn add_text_node(&mut self, x: f64, y: f64) {
        self.record_undo_state();
        let id = format!("node_{}", &uuid::Uuid::new_v4().to_string()[..8]);
        self.data
            .nodes
            .push(crate::data::CanvasNode::Text(crate::data::TextNode {
                id: id.clone(),
                x,
                y,
                width: 200.0,
                height: 100.0,
                text: "".to_string(),
                title: None,
                color: None,
                shape: Default::default(),
            }));
        self.selection.select_only(id.clone());
        self.resizing_node_id = Some(id);
        let _ = self.save();
    }

    pub fn add_group(&mut self, x: f64, y: f64) {
        self.record_undo_state();
        let id = format!("group_{}", &uuid::Uuid::new_v4().to_string()[..8]);
        self.data.nodes.insert(
            0,
            crate::data::CanvasNode::Group(crate::data::GroupNode {
                id: id.clone(),
                x,
                y,
                width: 400.0,
                height: 300.0,
                label: Some("New Group".to_string()),
                color: None,
            }),
        );
        self.selection.select_only(id.clone());
        self.resizing_node_id = Some(id);
        let _ = self.save();
    }

    /// Add an image file node at the given position. The host has already
    /// resolved the file path via its picker dialog.
    pub fn add_image_node_with(&mut self, path: PathBuf, x: f64, y: f64) {
        self.record_undo_state();
        let id = format!("node_{}", &uuid::Uuid::new_v4().to_string()[..8]);
        self.data
            .nodes
            .push(crate::data::CanvasNode::File(crate::data::FileNode {
                id: id.clone(),
                x,
                y,
                width: 300.0,
                height: 200.0,
                file: path.to_string_lossy().into_owned(),
                subpath: None,
                title: None,
                color: None,
            }));
        self.selection.select_only(id.clone());
        let _ = self.save();
    }

    pub fn start_connection(&mut self) {
        if let Some(id) = &self.selection.primary {
            self.connection_source_id = Some(id.clone());
            self.context_menu = None;
        }
    }

    pub fn finish_connection(&mut self, target_id: &str) {
        if let Some(source_id) = self.connection_source_id.take()
            && source_id != target_id
        {
            let edge_id = format!("edge_{source_id}_{target_id}");
            self.record_undo_state();
            if !self
                .data
                .edges
                .iter()
                .any(|e| e.from_node == source_id && e.to_node == target_id)
            {
                self.data.edges.push(crate::data::CanvasEdge {
                    id: edge_id,
                    from_node: source_id,
                    from_side: Some("right".to_string()),
                    to_node: target_id.to_string(),
                    to_side: Some("left".to_string()),
                    label: None,
                    color: None,
                    style: crate::data::EdgeStyle::Solid,
                });
                let _ = self.save();
            }
        }
    }

    pub fn finish_delete_connection(&mut self, target_id: &str) {
        if let Some(source_id) = self.deleting_connection_source_id.take()
            && source_id != target_id
        {
            self.record_undo_state();
            self.data.edges.retain(|e| {
                !((e.from_node == source_id && e.to_node == target_id)
                    || (e.from_node == target_id && e.to_node == source_id))
            });
            let _ = self.save();
        }
    }

    pub fn resize_selected_node(&mut self, dw: f64, dh: f64) {
        if (dw.abs() > 0.001 || dh.abs() > 0.001) && self.resizing_node_id.is_some() {
            self.has_layout_override = true;
        }
        if let Some(id) = &self.resizing_node_id {
            for node in &mut self.data.nodes {
                if node.id() == id {
                    match node {
                        crate::data::CanvasNode::Text(n) => {
                            n.width = (n.width + dw).max(10.0);
                            n.height = (n.height + dh).max(10.0);
                        }
                        crate::data::CanvasNode::File(n) => {
                            n.width = (n.width + dw).max(10.0);
                            n.height = (n.height + dh).max(10.0);
                        }
                        crate::data::CanvasNode::Link(n) => {
                            n.width = (n.width + dw).max(10.0);
                            n.height = (n.height + dh).max(10.0);
                        }
                        crate::data::CanvasNode::Group(n) => {
                            n.width = (n.width + dw).max(10.0);
                            n.height = (n.height + dh).max(10.0);
                        }
                    }
                    break;
                }
            }
        }
    }

    pub fn capture_drag_nodes(&mut self) {
        self.drag_captured_nodes.clear();
        if let Some(id) = &self.selection.primary {
            let mut group_bounds = None;
            for node in &self.data.nodes {
                if node.id() == id {
                    if let crate::data::CanvasNode::Group(n) = node {
                        group_bounds = Some((n.x, n.y, n.width, n.height));
                    }
                    break;
                }
            }

            if let Some((gx, gy, gw, gh)) = group_bounds {
                for node in &self.data.nodes {
                    let nid = node.id();
                    if nid != id {
                        let (nx, ny) = node.pos();
                        let (nw, nh) = node.size();
                        if nx >= gx && ny >= gy && (nx + nw) <= (gx + gw) && (ny + nh) <= (gy + gh)
                        {
                            self.drag_captured_nodes.insert(nid.to_string());
                        }
                    }
                }
            }
            // Also capture multi-selected nodes
            for nid in &self.selection.extra {
                self.drag_captured_nodes.insert(nid.clone());
            }
        } else {
            // When no primary node but multi-selected nodes exist, capture all of them
            for nid in &self.selection.extra {
                self.drag_captured_nodes.insert(nid.clone());
            }
        }
    }

    pub fn capture_group_children(&mut self) {
        self.drag_group_children.clear();
        let mut group_bounds = Vec::new();

        if let Some(id) = &self.selection.primary {
            if let Some(crate::data::CanvasNode::Group(n)) =
                self.data.nodes.iter().find(|n| n.id() == id)
            {
                group_bounds.push((n.x, n.y, n.width, n.height));
            }
        }
        for id in &self.drag_captured_nodes {
            if let Some(crate::data::CanvasNode::Group(n)) =
                self.data.nodes.iter().find(|n| n.id() == id)
            {
                group_bounds.push((n.x, n.y, n.width, n.height));
            }
        }

        let mut to_capture = Vec::new();
        for (gx, gy, gw, gh) in group_bounds {
            for node in &self.data.nodes {
                let nid = node.id();
                if self.selection.primary.as_ref().is_none_or(|id| id != nid)
                    && !self.drag_captured_nodes.contains(nid)
                {
                    let (nx, ny) = node.pos();
                    let (nw, nh) = node.size();
                    if nx >= gx && ny >= gy && (nx + nw) <= (gx + gw) && (ny + nh) <= (gy + gh) {
                        to_capture.push(nid.to_string());
                    }
                }
            }
        }

        for id in to_capture {
            self.drag_group_children.insert(id);
        }
    }

    pub fn move_selected_node(&mut self, dx: f64, dy: f64) {
        let has_selection =
            self.selection.primary.is_some() || !self.drag_captured_nodes.is_empty();
        if (dx.abs() > 0.001 || dy.abs() > 0.001) && has_selection {
            self.has_layout_override = true;
        }
        if has_selection {
            let mut to_move = std::collections::HashSet::new();
            if let Some(id) = &self.selection.primary {
                to_move.insert(id.clone());
            }
            for id in &self.drag_captured_nodes {
                to_move.insert(id.clone());
            }
            for id in &self.drag_group_children {
                to_move.insert(id.clone());
            }

            for node in &mut self.data.nodes {
                let nid = node.id();
                if to_move.contains(nid) {
                    match node {
                        crate::data::CanvasNode::Text(n) => {
                            n.x += dx;
                            n.y += dy;
                        }
                        crate::data::CanvasNode::File(n) => {
                            n.x += dx;
                            n.y += dy;
                        }
                        crate::data::CanvasNode::Link(n) => {
                            n.x += dx;
                            n.y += dy;
                        }
                        crate::data::CanvasNode::Group(n) => {
                            n.x += dx;
                            n.y += dy;
                        }
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn canvas_with(
        nodes: Vec<crate::data::CanvasNode>,
        edges: Vec<crate::data::CanvasEdge>,
    ) -> (tempfile::TempDir, PinstarState) {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("c.canvas");
        let data = CanvasData {
            nodes,
            edges,
            orientation: Default::default(),
        };
        std::fs::write(&path, serde_json::to_string(&data).unwrap()).unwrap();
        let state = PinstarState::load(&path).unwrap();
        (dir, state)
    }

    fn text_node(id: &str, x: f64, y: f64) -> crate::data::CanvasNode {
        crate::data::CanvasNode::Text(crate::data::TextNode {
            id: id.into(),
            x,
            y,
            width: 100.0,
            height: 50.0,
            text: "".into(),
            title: None,
            color: None,
            shape: Default::default(),
        })
    }

    #[test]
    fn canvas_context_menu_remains_available() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("canvas.json");
        std::fs::write(&path, r#"{"nodes":[],"edges":[]}"#).unwrap();
        let mut state = PinstarState::load(&path).unwrap();

        state.open_context_menu(4, 5, 0.0, 0.0);

        let menu = state.context_menu.expect("menu open");
        assert_eq!(menu.menu_type, PinstarMenuType::Canvas);
    }

    #[test]
    fn connection_flow_and_delete_both_ways() {
        let (_dir, mut s) = canvas_with(
            vec![text_node("a", 0.0, 0.0), text_node("b", 200.0, 0.0)],
            vec![],
        );
        s.selection.select_only("a".into());
        s.start_connection();
        s.select_node_in_direction(1.0, 0.0);
        assert_eq!(s.selection.primary.as_deref(), Some("b"));
        s.finish_connection("b");
        assert_eq!(s.data.edges.len(), 1);
        // delete both ways: source=b, target=a should remove a->b
        s.selection.select_only("b".into());
        s.start_delete_connection();
        s.finish_delete_connection("a");
        assert_eq!(s.data.edges.len(), 0);
    }

    #[test]
    fn edge_overlay_lists_and_selects_connected_edges() {
        let edges = vec![
            crate::data::CanvasEdge {
                id: "e1".into(),
                from_node: "a".into(),
                from_side: None,
                to_node: "b".into(),
                to_side: None,
                label: None,
                color: None,
                style: crate::data::EdgeStyle::Solid,
            },
            crate::data::CanvasEdge {
                id: "e2".into(),
                from_node: "a".into(),
                from_side: None,
                to_node: "c".into(),
                to_side: None,
                label: None,
                color: None,
                style: crate::data::EdgeStyle::Solid,
            },
        ];
        let (_dir, mut s) = canvas_with(
            vec![
                text_node("a", 0.0, 0.0),
                text_node("b", 200.0, 0.0),
                text_node("c", 0.0, 200.0),
            ],
            edges,
        );
        s.selection.select_only("a".into());
        let edges = s.selected_node_edges();
        assert_eq!(edges.len(), 2);
        assert_eq!(edges[0].id, "e1");
        assert_eq!(edges[1].id, "e2");
        assert_eq!(s.select_edge_of_selected_node(2).as_deref(), Some("e2"));
        assert_eq!(s.selected_edge_id.as_deref(), Some("e2"));
        assert_eq!(s.selection.primary, None);
        // out of range -> None
        s.selection.select_only("a".into());
        assert_eq!(s.select_edge_of_selected_node(3), None);
    }

    #[test]
    fn rename_title_and_id_modes() {
        let (_dir, mut s) = canvas_with(vec![text_node("a", 0.0, 0.0)], vec![]);
        s.selection.select_only("a".into());
        s.rename_node_title("My Title".into());
        assert_eq!(s.data.nodes[0].title(), Some("My Title"));
        assert_eq!(s.data.nodes[0].id(), "a");

        s.selection.select_only("a".into());
        s.rename_node_id("renamed".into());
        assert_eq!(s.data.nodes[0].id(), "renamed");
        assert_eq!(s.selection.primary.as_deref(), Some("renamed"));
    }

    #[test]
    fn undo_restores_and_prunes_selection() {
        let (_dir, mut s) = canvas_with(vec![text_node("a", 0.0, 0.0)], vec![]);
        s.selection.select_only("a".into());
        s.delete_selected_node();
        assert!(s.data.nodes.is_empty());
        assert!(s.selection.primary.is_none());
        s.undo().unwrap();
        assert_eq!(s.data.nodes.len(), 1);
    }

    #[test]
    fn screen_to_canvas_center_offset_and_clamp() {
        let (_dir, mut s) = canvas_with(vec![], vec![]);
        s.zoom = 1.0;
        let area = ratatui::layout::Rect::new(0, 0, 100, 50);
        let (cx, cy) = s.screen_to_canvas(50, 25, area);
        // Center of the middle cell maps to the viewport center.
        assert!((cx - 0.5).abs() < 1e-9);
        assert!((cy - 0.5).abs() < 1e-9);
    }
}
