use crate::data::CanvasData;
use crate::formats::{self, SupportedFormat};
use anyhow::Result;
use ratatui_textarea::{TextArea, WrapMode};
use std::path::{Path, PathBuf};

#[derive(Clone)]
pub struct PinstarSnapshot {
    pub data: CanvasData,
    pub has_layout_override: bool,
}

pub struct PinstarState {
    pub path: PathBuf,
    pub format: SupportedFormat,
    pub data: CanvasData,
    pub viewport_x: f64,
    pub viewport_y: f64,
    pub zoom: f64,
    pub selected_node_id: Option<String>,
    pub selected_edge_id: Option<String>,
    pub floating_editor: Option<TextArea<'static>>,
    pub raw_editor: TextArea<'static>,
    pub editor_focus: bool,
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
    pub select_rect_start: Option<(f64, f64)>,
    pub select_rect_end: Option<(f64, f64)>,
    pub has_layout_override: bool,
    pub undo_stack: Vec<PinstarSnapshot>,
    pub redo_stack: Vec<PinstarSnapshot>,
}

#[derive(Debug, Clone, Copy, PartialEq)]
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

pub struct PinstarContextMenu {
    pub x: u16,
    pub y: u16,
    pub selected: usize,
    pub items: Vec<String>,
    pub menu_type: PinstarMenuType,
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
            locked,
            last_modified,
            viewport_x: 0.0,
            viewport_y: 0.0,
            zoom: 0.1,
            selected_node_id: None,
            selected_edge_id: None,
            floating_editor: None,
            raw_editor,
            editor_focus: false,
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
            select_rect_start: None,
            select_rect_end: None,
            has_layout_override,
            undo_stack: Vec::new(),
            redo_stack: Vec::new(),
        })
    }

    pub fn save(&mut self) -> Result<()> {
        let original = std::fs::read_to_string(&self.path).unwrap_or_default();
        let content = formats::save_to_format(&self.data, &original, self.format, self.has_layout_override)?;
        std::fs::write(&self.path, &content)?;

        self.raw_editor = TextArea::from(content.lines().map(String::from).collect::<Vec<_>>());
        self.raw_editor.set_cursor_line_style(ratatui::style::Style::default());
        self.raw_editor.set_wrap_mode(WrapMode::WordOrGlyph);

        if let Ok(metadata) = std::fs::metadata(&self.path) {
            if let Ok(modified) = metadata.modified() {
                self.last_modified = modified;
            }
        }
        Ok(())
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

    pub fn undo(&mut self) -> Result<()> {
        if let Some(snapshot) = self.undo_stack.pop() {
            let current = PinstarSnapshot {
                data: self.data.clone(),
                has_layout_override: self.has_layout_override,
            };
            self.redo_stack.push(current);

            self.data = snapshot.data;
            self.has_layout_override = snapshot.has_layout_override;

            // Clean up dangling selection references
            if let Some(sel_id) = &self.selected_node_id {
                if !self.data.nodes.iter().any(|n| n.id() == sel_id) {
                    self.selected_node_id = None;
                    self.drag_captured_nodes.clear();
                }
            }

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

            if let Some(sel_id) = &self.selected_node_id {
                if !self.data.nodes.iter().any(|n| n.id() == sel_id) {
                    self.selected_node_id = None;
                    self.drag_captured_nodes.clear();
                }
            }

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
        self.raw_editor.set_cursor_line_style(ratatui::style::Style::default());
        self.raw_editor.set_wrap_mode(WrapMode::WordOrGlyph);
        
        if let Some(sel_id) = &self.selected_node_id {
            if !self.data.nodes.iter().any(|n| n.id() == sel_id) {
                self.selected_node_id = None;
            }
        }
        if let Some(sel_id) = &self.selected_edge_id {
            if !self.data.edges.iter().any(|e| e.id == *sel_id) {
                self.selected_edge_id = None;
            }
        }

        if let Ok(metadata) = std::fs::metadata(&self.path) {
            if let Ok(modified) = metadata.modified() {
                self.last_modified = modified;
            }
        }
        Ok(())
    }

    pub fn sync_from_raw_editor(&mut self) -> Result<()> {
        let content = self.raw_editor.lines().join("\n");
        if let Ok(mut data) = formats::load_from_format(&self.path, &content, self.format) {
            self.record_undo_state();
            // Force re-layout if orientation changed
            if data.orientation != self.data.orientation && self.format != formats::SupportedFormat::Canvas {
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

    pub fn pan(&mut self, dx: f64, dy: f64) {
        self.viewport_x += dx / self.zoom;
        self.viewport_y += dy / self.zoom;
    }

    pub fn center_on_selected(&mut self) {
        if let Some(id) = &self.selected_node_id
            && let Some(node) = self.data.nodes.iter().find(|n| n.id() == id)
        {
            let (nx, ny) = node.pos();
            let (nw, nh) = node.size();
            self.viewport_x = nx + nw / 2.0;
            self.viewport_y = ny + nh / 2.0;
        }
    }

    pub fn zoom_in(&mut self) {
        self.zoom *= 1.1;
    }

    pub fn zoom_out(&mut self) {
        self.zoom /= 1.1;
    }

    pub fn fit_to_view(&mut self, area: ratatui::layout::Rect) {
        if self.data.nodes.is_empty() {
            return;
        }

        let min_x = self.data.nodes.iter().map(|n| n.pos().0).reduce(f64::min).unwrap_or(0.0);
        let min_y = self.data.nodes.iter().map(|n| n.pos().1).reduce(f64::min).unwrap_or(0.0);
        let max_x = self.data.nodes.iter().map(|n| n.pos().0 + n.size().0).reduce(f64::max).unwrap_or(0.0);
        let max_y = self.data.nodes.iter().map(|n| n.pos().1 + n.size().1).reduce(f64::max).unwrap_or(0.0);

        // Center of bounding box
        let cx = (min_x + max_x) / 2.0;
        let cy = (min_y + max_y) / 2.0;

        // Bounding box dimensions with padding
        let padding = 100.0;
        let bbox_w = (max_x - min_x) + padding * 2.0;
        let bbox_h = (max_y - min_y) + padding * 2.0;

        // Available canvas area (account for status bar)
        let avail_w = area.width as f64;
        let avail_h = (area.height.saturating_sub(1)) as f64;

        // Pick zoom that fits the bounding box
        let zoom_x = if bbox_w > 0.0 { avail_w / bbox_w } else { 1.0 };
        let zoom_y = if bbox_h > 0.0 { avail_h / bbox_h } else { 1.0 };
        let zoom = zoom_x.min(zoom_y);

        // Clamp zoom to reasonable range
        let zoom = zoom.clamp(0.01, 10.0);

        self.viewport_x = cx;
        self.viewport_y = cy;
        self.zoom = zoom;
    }

    pub fn screen_to_canvas(&self, sx: u16, sy: u16, area: ratatui::layout::Rect) -> (f64, f64) {
        let cx =
            (sx as f64 - (area.x as f64 + area.width as f64 / 2.0)) / self.zoom + self.viewport_x;
        let cy =
            (sy as f64 - (area.y as f64 + area.height as f64 / 2.0)) / self.zoom + self.viewport_y;
        (cx, cy)
    }

    pub fn node_at(&self, mx: u16, my: u16, area: ratatui::layout::Rect) -> Option<String> {
        let mut best_hit: Option<(String, f64, usize)> = None;
        let mx_i = mx as i32;
        let my_i = my as i32;

        for (idx, node) in self.data.nodes.iter().enumerate() {
            let (nx, ny) = node.pos();
            let (nw, nh) = node.size();

            // Compute exact screen coordinates identically to render.rs
            let sx = ((nx - self.viewport_x) * self.zoom)
                + (area.x as f64 + area.width as f64 / 2.0);
            let sy = ((ny - self.viewport_y) * self.zoom)
                + (area.y as f64 + area.height as f64 / 2.0);
            let sw = nw * self.zoom;
            let sh = nh * self.zoom;

            // Round to discrete screen grid coordinates
            let left = sx.round() as i32;
            let top = sy.round() as i32;
            let right = (sx + sw).round() as i32;
            let bottom = (sy + sh).round() as i32;

            let is_hit = if matches!(node, crate::data::CanvasNode::Group(_)) {
                // Groups are selectable by their title area (top line + titlebar background line)
                mx_i >= left && mx_i < right && my_i >= top && my_i <= top + 1
            } else {
                // Standard nodes are selectable in their entire bounding rectangle
                mx_i >= left && mx_i < right && my_i >= top && my_i < bottom
            };

            if is_hit {
                let area_size = nw * nh;
                let should_replace = match &best_hit {
                    None => true,
                    Some((_, best_area, _)) if area_size < *best_area => true,
                    Some((_, best_area, best_idx))
                        if (area_size - *best_area).abs() < 0.0001 && idx > *best_idx =>
                    {
                        true
                    }
                    _ => false,
                };
                if should_replace {
                    best_hit = Some((node.id().to_string(), area_size, idx));
                }
            }
        }

        best_hit.map(|(id, _, _)| id)
    }

    pub fn select_node_at(&mut self, mx: u16, my: u16, area: ratatui::layout::Rect) -> Option<String> {
        if let Some(id) = self.node_at(mx, my, area) {
            self.selected_node_id = Some(id.clone());
            self.selected_edge_id = None;
            Some(id)
        } else {
            self.selected_node_id = None;
            None
        }
    }

    pub fn select_nodes_in_rect(&mut self, x1: f64, y1: f64, x2: f64, y2: f64) {
        let (min_x, max_x) = if x1 < x2 { (x1, x2) } else { (x2, x1) };
        let (min_y, max_y) = if y1 < y2 { (y1, y2) } else { (y2, y1) };
        let mut selected = std::collections::HashSet::new();

        for node in &self.data.nodes {
            let (nx, ny) = node.pos();
            let (nw, nh) = node.size();
            let cx = nx + nw / 2.0;
            let cy = ny + nh / 2.0;
            if cx >= min_x && cx <= max_x && cy >= min_y && cy <= max_y {
                selected.insert(node.id().to_string());
            }
        }

        // Set first as primary, rest as captured
        let mut ids: Vec<String> = selected.into_iter().collect();
        ids.sort();
        if let Some(primary) = ids.first().cloned() {
            self.selected_node_id = Some(primary);
            self.drag_captured_nodes = ids.into_iter().skip(1).collect();
            self.selected_edge_id = None;
        } else {
            self.selected_node_id = None;
            self.drag_captured_nodes.clear();

            if self.format == crate::formats::SupportedFormat::Canvas || self.format == crate::formats::SupportedFormat::Mermaid {
                self.selected_edge_id = None;
                return;
            }

            // If no nodes inside the box, fallback to selecting intersecting connections
            let mut found_edge = None;
            let line_intersects_rect = |sx: f64, sy: f64, ex: f64, ey: f64, min_x: f64, min_y: f64, max_x: f64, max_y: f64| -> bool {
                let inside = |x: f64, y: f64| x >= min_x && x <= max_x && y >= min_y && y <= max_y;
                if inside(sx, sy) || inside(ex, ey) {
                    return true;
                }
                let intersect = |x1: f64, y1: f64, x2: f64, y2: f64, x3: f64, y3: f64, x4: f64, y4: f64| -> bool {
                    let denom = (y4 - y3) * (x2 - x1) - (x4 - x3) * (y2 - y1);
                    if denom.abs() < 0.0001 { return false; }
                    let ua = ((x4 - x3) * (y1 - y3) - (y4 - y3) * (x1 - x3)) / denom;
                    let ub = ((x2 - x1) * (y1 - y3) - (y2 - y1) * (x1 - x3)) / denom;
                    ua >= 0.0 && ua <= 1.0 && ub >= 0.0 && ub <= 1.0
                };
                intersect(sx, sy, ex, ey, min_x, min_y, max_x, min_y) ||
                intersect(sx, sy, ex, ey, min_x, max_y, max_x, max_y) ||
                intersect(sx, sy, ex, ey, min_x, min_y, min_x, max_y) ||
                intersect(sx, sy, ex, ey, max_x, min_y, max_x, max_y)
            };

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
    }

    pub fn select_node_in_direction(&mut self, dx: f64, dy: f64) {
        let current_node = if let Some(id) = &self.selected_node_id {
            self.data.nodes.iter().find(|n| n.id() == id)
        } else {
            None
        };

        let (cur_x, cur_y) = if let Some(n) = current_node {
            let (nx, ny) = n.pos();
            let (nw, nh) = n.size();
            (nx + nw / 2.0, ny + nh / 2.0)
        } else {
            (self.viewport_x, self.viewport_y)
        };

        let mut best_node = None;
        let mut min_dist = f64::MAX;

        for node in &self.data.nodes {
            if let Some(id) = &self.selected_node_id
                && node.id() == id
            {
                continue;
            }

            let (nx, ny) = node.pos();
            let (nw, nh) = node.size();
            let (tx, ty) = (nx + nw / 2.0, ny + nh / 2.0);

            let v_x = tx - cur_x;
            let v_y = ty - cur_y;

            let dot = v_x * dx + v_y * dy;
            if dot <= 0.0 {
                continue;
            }

            let dist_sq = v_x * v_x + v_y * v_y;
            let ortho_dist = (v_x * -dy + v_y * dx).abs();
            let score = dist_sq + ortho_dist * ortho_dist * 2.0;

            if score < min_dist {
                min_dist = score;
                best_node = Some(node.id().to_string());
            }
        }

        if let Some(id) = best_node {
            self.selected_node_id = Some(id);
        } else if self.selected_node_id.is_none() && !self.data.nodes.is_empty() {
            self.selected_node_id = Some(self.data.nodes[0].id().to_string());
        }
    }

    pub fn toggle_editor(&mut self) {
        if self.floating_editor.is_some() {
            if let Some(node_id) = &self.selected_node_id {
                let text = self.floating_editor.as_ref().unwrap().lines().join("\n");
                for node in &mut self.data.nodes {
                    if node.id() == node_id {
                        node.set_text(text);
                        break;
                    }
                }
                let _ = self.save();
            }
            self.floating_editor = None;
        } else if let Some(node_id) = &self.selected_node_id {
            let text_opt = self.data.nodes.iter()
                .find(|n| n.id() == node_id)
                .map(|n| n.text().to_string());
            if let Some(text) = text_opt {
                self.record_undo_state();
                let mut textarea = TextArea::from(text.lines().map(String::from).collect::<Vec<_>>());
                textarea.set_cursor_line_style(ratatui::style::Style::default());
                textarea.set_wrap_mode(WrapMode::WordOrGlyph);
                self.floating_editor = Some(textarea);
            }
        }
    }

    pub fn open_context_menu(&mut self, x: u16, y: u16, canvas_x: f64, canvas_y: f64) {
        let mut items = if self.selected_node_id.is_some() {
            vec![
                "Create Connection".to_string(),
                "Delete Connection".to_string(),
                "Rename Node".to_string(),
                "Resize Node".to_string(),
                "Set Shape...".to_string(),
                "Set Color...".to_string(),
                "Delete All Connections".to_string(),
                "Delete Node".to_string(),
            ]
        } else {
            vec!["Add Text Node".to_string(), "Add Group".to_string()]
        };

        if self.format != SupportedFormat::Canvas {
            items.retain(|item| item != "Add Group");
        }
        if self.format == SupportedFormat::Canvas {
            items.retain(|item| item != "Set Shape...");
        }
        if self.format == SupportedFormat::Mermaid || self.format == SupportedFormat::PlantUml {
            items.retain(|item| item != "Set Color...");
        }

        if self.format.is_flowchart() {
            items.push("Set Orientation...".to_string());
        }

        self.context_menu_pos = (canvas_x, canvas_y);
        self.context_menu = Some(PinstarContextMenu {
            x,
            y,
            selected: 0,
            items,
            menu_type: PinstarMenuType::Canvas,
        });
    }

    pub fn open_editor_context_menu(&mut self, x: u16, y: u16) {
        let items = vec![
            "Copy".to_string(),
            "Cut".to_string(),
            "Paste".to_string(),
            "Select All".to_string(),
        ];

        self.context_menu = Some(PinstarContextMenu {
            x,
            y,
            selected: 0,
            items,
            menu_type: PinstarMenuType::Editor,
        });
    }

    pub fn start_resize(&mut self) {
        let id_opt = self.selected_node_id.clone();
        if let Some(id) = id_opt {
            self.record_undo_state();
            self.resizing_node_id = Some(id);
            self.context_menu = None;
        }
    }

    pub fn start_delete_connection(&mut self) {
        if let Some(id) = &self.selected_node_id {
            self.deleting_connection_source_id = Some(id.clone());
            self.context_menu = None;
        }
    }

    pub fn rename_node(&mut self, new_id: String) {
        if let Some(old_id) = self.selected_node_id.take() {
            self.record_undo_state();
            if old_id == new_id {
                self.selected_node_id = Some(old_id);
                return;
            }
            let final_id = if new_id.is_empty() {
                uuid::Uuid::new_v4().to_string()
            } else {
                new_id
            };
            let new_id = final_id;

            for node in &mut self.data.nodes {
                match node {
                    crate::data::CanvasNode::Text(n) if n.id == old_id => {
                        n.id = new_id.clone()
                    }
                    crate::data::CanvasNode::File(n) if n.id == old_id => {
                        n.id = new_id.clone()
                    }
                    crate::data::CanvasNode::Link(n) if n.id == old_id => {
                        n.id = new_id.clone()
                    }
                    crate::data::CanvasNode::Group(n) if n.id == old_id => {
                        n.id = new_id.clone()
                    }
                    _ => {}
                }
            }

            for edge in &mut self.data.edges {
                if edge.from_node == old_id {
                    edge.from_node = new_id.clone();
                }
                if edge.to_node == old_id {
                    edge.to_node = new_id.clone();
                }
            }

            self.selected_node_id = Some(new_id);
            let _ = self.save();
        }
    }

    pub fn all_selected_node_ids(&self) -> std::collections::HashSet<String> {
        let mut ids = std::collections::HashSet::new();
        if let Some(id) = &self.selected_node_id {
            ids.insert(id.clone());
        }
        for id in &self.drag_captured_nodes {
            ids.insert(id.clone());
        }
        ids
    }

    pub fn delete_node_connections(&mut self) {
        let ids = self.all_selected_node_ids();
        if !ids.is_empty() {
            self.record_undo_state();
            self.data
                .edges
                .retain(|e| !ids.contains(&e.from_node) && !ids.contains(&e.to_node));
            let _ = self.save();
        }
    }

    pub fn set_node_color(&mut self, color: Option<String>) {
        let ids = self.all_selected_node_ids();
        if !ids.is_empty() {
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
    }

    pub fn set_node_shape(&mut self, shape: crate::data::NodeShape) {
        let ids = self.all_selected_node_ids();
        if !ids.is_empty() {
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
    }

    pub fn add_text_node(&mut self, x: f64, y: f64) {
        self.record_undo_state();
        let id = format!("node_{}", &uuid::Uuid::new_v4().to_string()[..8]);
        self.data.nodes.push(crate::data::CanvasNode::Text(
            crate::data::TextNode {
                id: id.clone(),
                x,
                y,
                width: 200.0,
                height: 100.0,
                text: "".to_string(),
                color: None,
                shape: Default::default(),
            },
        ));
        self.selected_node_id = Some(id.clone());
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
        self.selected_node_id = Some(id.clone());
        self.resizing_node_id = Some(id);
        let _ = self.save();
    }

    pub fn start_connection(&mut self) {
        if let Some(id) = &self.selected_node_id {
            self.connection_source_id = Some(id.clone());
            self.context_menu = None;
        }
    }

    pub fn finish_connection(&mut self, target_id: &str) {
        if let Some(source_id) = self.connection_source_id.take()
            && source_id != target_id
        {
            self.record_undo_state();
            let edge_id = format!("edge_{}_{}", source_id, target_id);
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
                    style: Default::default(),
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
            self.data
                .edges
                .retain(|e| !(e.from_node == source_id && e.to_node == target_id));
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

    pub fn capture_group_children(&mut self) {
        self.drag_group_children.clear();
        let mut group_bounds = Vec::new();
        
        if let Some(id) = &self.selected_node_id {
            if let Some(node) = self.data.nodes.iter().find(|n| n.id() == id) {
                if let crate::data::CanvasNode::Group(n) = node {
                    group_bounds.push((n.x, n.y, n.width, n.height));
                }
            }
        }
        for id in &self.drag_captured_nodes {
            if let Some(node) = self.data.nodes.iter().find(|n| n.id() == id) {
                if let crate::data::CanvasNode::Group(n) = node {
                    group_bounds.push((n.x, n.y, n.width, n.height));
                }
            }
        }

        let mut to_capture = Vec::new();
        for (gx, gy, gw, gh) in group_bounds {
            for node in &self.data.nodes {
                let nid = node.id();
                if self.selected_node_id.as_ref().map_or(true, |id| id != nid)
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
        if (dx.abs() > 0.001 || dy.abs() > 0.001) && (self.selected_node_id.is_some() || !self.drag_captured_nodes.is_empty()) {
            self.has_layout_override = true;
        }
        if self.selected_node_id.is_some() || !self.drag_captured_nodes.is_empty() {
            let mut to_move = std::collections::HashSet::new();
            if let Some(id) = &self.selected_node_id {
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

    pub fn get_edge_segments(&self, edge: &crate::data::CanvasEdge) -> Option<Vec<(f64, f64, f64, f64)>> {
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
        } else {
            if dy > 0.0 { (scx, fy + fh) } else { (scx, fy) }
        };

        let (bx, by) = if is_horiz {
            if dx > 0.0 { (tx, tcy) } else { (tx + tw, tcy) }
        } else {
            if dy > 0.0 { (tcx, ty) } else { (tcx, ty + th) }
        };

        let use_orthogonal = if self.format == SupportedFormat::Canvas {
            self.orthogonal_connections
        } else {
            true
        };

        let segments = if use_orthogonal {
            if is_horiz {
                let mid_x = (ax + bx) / 2.0;
                vec![(ax, ay, mid_x, ay), (mid_x, ay, mid_x, by), (mid_x, by, bx, by)]
            } else {
                let mid_y = (ay + by) / 2.0;
                vec![(ax, ay, ax, mid_y), (ax, mid_y, bx, mid_y), (bx, mid_y, bx, by)]
            }
        } else {
            vec![(ax, ay, bx, by)]
        };

        Some(segments)
    }

    pub fn select_edge_at(&mut self, x: f64, y: f64) -> Option<String> {
        if self.format == SupportedFormat::Canvas || self.format == SupportedFormat::Mermaid {
            return None;
        }
        let tolerance = 5.0;
        let mut best: Option<(String, f64)> = None;

        for edge in &self.data.edges {
            if let Some(segments) = self.get_edge_segments(edge) {
                for &(sx, sy, ex, ey) in &segments {
                    let seg_dx = ex - sx;
                    let seg_dy = ey - sy;
                    let len2 = seg_dx * seg_dx + seg_dy * seg_dy;
                    let dist = if len2 == 0.0 {
                        ((x - sx).powi(2) + (y - sy).powi(2)).sqrt()
                    } else {
                        let t = ((x - sx) * seg_dx + (y - sy) * seg_dy) / len2;
                        let t = t.clamp(0.0, 1.0);
                        let px = sx + t * seg_dx;
                        let py = sy + t * seg_dy;
                        ((x - px).powi(2) + (y - py).powi(2)).sqrt()
                    };

                    if dist < tolerance {
                        let should_replace = match &best {
                            None => true,
                            Some((_, best_dist)) if dist < *best_dist => true,
                            _ => false,
                        };
                        if should_replace {
                            best = Some((edge.id.clone(), dist));
                        }
                    }
                }
            }
        }

        if let Some((id, _)) = best {
            self.selected_edge_id = Some(id.clone());
            self.selected_node_id = None;
            Some(id)
        } else {
            self.selected_edge_id = None;
            None
        }
    }

    pub fn set_edge_color(&mut self, color: Option<String>) {
        let edge_id_opt = self.selected_edge_id.clone();
        if let Some(id) = edge_id_opt {
            self.record_undo_state();
            for edge in &mut self.data.edges {
                if edge.id == id {
                    edge.color = color.clone();
                    break;
                }
            }
            let _ = self.save();
        }
    }

    pub fn set_edge_style(&mut self, style: crate::data::EdgeStyle) {
        let edge_id_opt = self.selected_edge_id.clone();
        if let Some(id) = edge_id_opt {
            self.record_undo_state();
            for edge in &mut self.data.edges {
                if edge.id == id {
                    edge.style = style;
                    break;
                }
            }
            let _ = self.save();
        }
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
            crate::data::DiagramOrientation::LeftRight => crate::data::DiagramOrientation::RightLeft,
            crate::data::DiagramOrientation::RightLeft => crate::data::DiagramOrientation::DownTop,
            crate::data::DiagramOrientation::DownTop => crate::data::DiagramOrientation::TopDown,
        };
        formats::apply_hierarchical_layout(&mut self.data);
        self.has_layout_override = false;
        let _ = self.save();
    }

    pub fn open_edge_context_menu(&mut self, x: u16, y: u16) {
        let mut items = vec![
            "Set Color...".to_string(),
            "Set Style...".to_string(),
        ];
        if self.format == SupportedFormat::Mermaid {
            items.retain(|item| item != "Set Color..." && item != "Set Style...");
        }
        if self.format == SupportedFormat::PlantUml {
            items.retain(|item| item != "Set Color...");
        }
        if items.is_empty() {
            return;
        }
        self.context_menu = Some(PinstarContextMenu {
            x,
            y,
            selected: 0,
            items,
            menu_type: PinstarMenuType::EdgeMenu,
        });
    }
}
