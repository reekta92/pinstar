//! Input handling: host-facing action API (`PinstarAction` + `apply_action`),
//! the merged mouse handler, and the standalone bin's default-keymap router.
//!
//! Routing model (graf precedent): hosts that own a keybind system resolve
//! keys to [`PinstarAction`] themselves and drive [`apply_action`]; the
//! standalone bin keeps [`handle_pinstar_event`] with its hardcoded keys.

use crate::data::EdgeStyle;
use crate::formats::SupportedFormat;
use crate::menu::PinstarMenuType;
use crate::state::{PinstarState, RenameMode};
use crate::theme::{contains_cell, get_textarea_scroll, move_textarea_cursor_to_mouse_scrolled};

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers, MouseButton, MouseEvent, MouseEventKind};
use ratatui::layout::Rect;
use ratatui_textarea::Input;

/// Host-level + stateful actions for the canvas engine. Clin maps its
/// `CanvasAction` 1:1 onto the shared subset; standalone-only variants are
/// driven by the default keymap.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PinstarAction {
    // host-level (returned via `ActionOutcome::host_action`)
    Quit,
    Save,
    Help,
    /// Host should run its image file dialog, then call
    /// `PinstarState::add_image_node_with(path, x, y)`.
    AddImageNode,
    /// Consumed by the engine (orth toggled); echoed so hosts can persist it.
    ToggleOrthogonal,
    // standalone-only
    PickShape,
    PickOrientation,
    ToggleLock,
    OpenExternalEditor,
    OpenWholeFileEditor,
    ShowHelp,
    // stateful (consumed by `apply_action`)
    Undo,
    Redo,
    ZoomIn,
    ZoomOut,
    ZoomFineIn,
    ZoomFineOut,
    MoveLeft,
    MoveRight,
    MoveUp,
    MoveDown,
    EditOrConnect,
    OpenContextMenu,
    MenuUp,
    MenuDown,
    MenuSelect,
    MenuClose,
    CreateConnection,
    DeleteConnection,
    RenameNode,
    ResizeMode,
    SetColor,
    DeleteNode,
    DeleteAllConnections,
    AddTextNode,
    AddGroup,
    ToggleGrid,
    ToggleEditorPane,
    CycleFocus,
    RenameConfirm,
    RenameCancel,
    ConfirmResize,
    CancelResize,
    EditorUnfocus,
    CloseEditor,
    CloseEditorAlt,
}

/// Per-call context for [`apply_action`].
pub struct ActionCtx {
    pub area: Rect,
    /// Repeat count from the host's count-prefix resolver (1-based).
    pub count: usize,
}

/// Result of applying one action.
#[derive(Default)]
pub struct ActionOutcome {
    /// Host-level request the engine cannot fulfill itself.
    pub host_action: Option<PinstarAction>,
    /// Transient status message the host should display.
    pub notice: Option<&'static str>,
}

impl ActionOutcome {
    fn none() -> Self {
        Self::default()
    }

    fn host(action: PinstarAction) -> Self {
        Self {
            host_action: Some(action),
            notice: None,
        }
    }

    fn notice(notice: &'static str) -> Self {
        Self {
            host_action: None,
            notice: Some(notice),
        }
    }
}

/// Apply one action to the canvas state. Stateful variants are consumed;
/// host-level ones are returned for dispatch.
pub fn apply_action(
    state: &mut PinstarState,
    action: PinstarAction,
    ctx: &ActionCtx,
) -> ActionOutcome {
    match action {
        PinstarAction::Quit => {
            if state.connection_source_id.is_some() {
                state.connection_source_id = None;
                ActionOutcome::none()
            } else {
                ActionOutcome::host(PinstarAction::Quit)
            }
        }
        action @ (PinstarAction::Save | PinstarAction::Help) => ActionOutcome::host(action),
        PinstarAction::AddImageNode => {
            if state.settings.enable_image_nodes && state.format == SupportedFormat::Canvas {
                ActionOutcome::host(PinstarAction::AddImageNode)
            } else {
                ActionOutcome::none()
            }
        }
        PinstarAction::Undo => {
            let _ = state.undo();
            ActionOutcome::none()
        }
        PinstarAction::Redo => {
            let _ = state.redo();
            ActionOutcome::none()
        }
        PinstarAction::ZoomIn | PinstarAction::ZoomFineIn => {
            state.zoom_in();
            ActionOutcome::none()
        }
        PinstarAction::ZoomOut | PinstarAction::ZoomFineOut => {
            state.zoom_out();
            ActionOutcome::none()
        }
        PinstarAction::MoveLeft => {
            for _ in 0..ctx.count {
                state.select_node_in_direction(-1.0, 0.0);
                state.center_on_selected();
            }
            ActionOutcome::none()
        }
        PinstarAction::MoveRight => {
            for _ in 0..ctx.count {
                state.select_node_in_direction(1.0, 0.0);
                state.center_on_selected();
            }
            ActionOutcome::none()
        }
        PinstarAction::MoveUp => {
            for _ in 0..ctx.count {
                state.select_node_in_direction(0.0, -1.0);
                state.center_on_selected();
            }
            ActionOutcome::none()
        }
        PinstarAction::MoveDown => {
            for _ in 0..ctx.count {
                state.select_node_in_direction(0.0, 1.0);
                state.center_on_selected();
            }
            ActionOutcome::none()
        }
        PinstarAction::EditOrConnect => {
            let target_id_opt = state.selection.primary.clone();
            if let Some(target_id) = target_id_opt {
                if state.connection_source_id.is_some() {
                    state.finish_connection(&target_id);
                } else {
                    state.toggle_editor();
                }
            }
            ActionOutcome::none()
        }
        PinstarAction::OpenContextMenu => {
            let menu_x = (ctx.area.width / 2).saturating_sub(12);
            let menu_y = ctx.area.height;

            let cx = state.viewport_x;
            let cy = state.viewport_y;

            if let Some(id) = &state.selection.primary {
                if state.data.nodes.iter().any(|n| n.id() == id) {
                    state.open_context_menu(menu_x, menu_y, cx, cy);
                }
            } else {
                state.open_context_menu(menu_x, menu_y, cx, cy);
            }
            ActionOutcome::none()
        }
        PinstarAction::ToggleGrid => {
            state.show_grid = !state.show_grid;
            ActionOutcome::none()
        }
        PinstarAction::ToggleOrthogonal => {
            if state.format == SupportedFormat::Canvas {
                state.orthogonal_connections = !state.orthogonal_connections;
                let notice = if state.orthogonal_connections {
                    "Orthogonal connections: on"
                } else {
                    "Orthogonal connections: off"
                };
                ActionOutcome {
                    host_action: Some(PinstarAction::ToggleOrthogonal),
                    notice: Some(notice),
                }
            } else {
                ActionOutcome::none()
            }
        }
        PinstarAction::ToggleEditorPane => {
            state.show_editor_pane = !state.show_editor_pane;
            if !state.show_editor_pane {
                state.editor_focus = false;
            }
            ActionOutcome::none()
        }
        PinstarAction::CycleFocus => {
            if state.show_editor_pane {
                state.editor_focus = true;
            }
            ActionOutcome::none()
        }
        PinstarAction::CreateConnection => {
            if state.selection.primary.is_some() {
                state.start_connection();
                ActionOutcome::none()
            } else {
                ActionOutcome::notice("Select a node first to create a connection")
            }
        }
        PinstarAction::DeleteConnection => {
            if state.selection.primary.is_some() {
                state.start_delete_connection();
                ActionOutcome::none()
            } else {
                ActionOutcome::notice("Select a node first to delete connections")
            }
        }
        PinstarAction::RenameNode => {
            if state.selection.primary.is_some() {
                let mode = if state.settings.rename_uses_id {
                    RenameMode::Id
                } else {
                    RenameMode::Title
                };
                state.open_rename_popup(mode);
                ActionOutcome::none()
            } else {
                ActionOutcome::notice("Select a node first to rename")
            }
        }
        PinstarAction::ResizeMode => {
            if state.selection.primary.is_some() {
                state.start_resize();
                ActionOutcome::none()
            } else {
                ActionOutcome::notice("Select a node first to resize")
            }
        }
        PinstarAction::SetColor => {
            if state.selection.is_empty() && state.selected_edge_id.is_none() {
                ActionOutcome::notice("Select a node or edge first to set color")
            } else {
                let menu_x = (ctx.area.width / 2).saturating_sub(12);
                let menu_y = ctx.area.height;
                state.open_color_menu(menu_x, menu_y, state.selected_edge_id.is_some());
                ActionOutcome::none()
            }
        }
        PinstarAction::DeleteNode => {
            if state.selection.all().is_empty() {
                ActionOutcome::notice("Select a node first to delete")
            } else {
                state.delete_selected_node();
                state.sync_to_raw_editor();
                ActionOutcome::none()
            }
        }
        PinstarAction::DeleteAllConnections => {
            if state.selection.all().is_empty() {
                ActionOutcome::notice("Select a node first to clear connections")
            } else {
                state.delete_node_connections();
                state.sync_to_raw_editor();
                ActionOutcome::none()
            }
        }
        PinstarAction::AddTextNode => {
            state.add_text_node(state.viewport_x, state.viewport_y);
            state.sync_to_raw_editor();
            ActionOutcome::none()
        }
        PinstarAction::AddGroup => {
            state.add_group(state.viewport_x, state.viewport_y);
            state.sync_to_raw_editor();
            ActionOutcome::none()
        }
        // Host-routed editor/menu keys never reach apply_action; harmless no-op.
        _ => ActionOutcome::none(),
    }
}

/// Execute a context-menu item by label. Returns a host request for
/// "Add Image Node".
pub fn execute_menu_action(
    state: &mut PinstarState,
    label: &str,
    menu_type: PinstarMenuType,
    menu_x: u16,
    menu_y: u16,
) -> ActionOutcome {
    if menu_type == PinstarMenuType::Editor {
        match label {
            "Copy" => {
                state.raw_editor.copy();
            }
            "Cut" => {
                state.raw_editor.cut();
                let _ = state.sync_from_raw_editor();
            }
            "Paste" => {
                state.raw_editor.paste();
                let _ = state.sync_from_raw_editor();
            }
            "Select All" => {
                state.raw_editor.select_all();
            }
            _ => {}
        }
        return ActionOutcome::none();
    }

    if menu_type == PinstarMenuType::ColorPicker || menu_type == PinstarMenuType::EdgeColorPicker {
        let color = if label == "Default" {
            None
        } else {
            crate::COLOR_PICKER_PALETTE
                .iter()
                .find(|e| e.0 == label)
                .map(|e| e.1.to_string())
        };
        if menu_type == PinstarMenuType::ColorPicker {
            state.set_node_color(color);
        } else {
            state.set_edge_color(color);
        }
        state.selection.primary = None;
        state.selected_edge_id = None;
        return ActionOutcome::none();
    }

    if menu_type == PinstarMenuType::ShapePicker {
        let shape = match label {
            "Rectangle" => crate::data::NodeShape::Rectangle,
            "Diamond" => crate::data::NodeShape::Diamond,
            "Circle" => crate::data::NodeShape::Circle,
            "Cylinder" => crate::data::NodeShape::Cylinder,
            "Stadium" => crate::data::NodeShape::Stadium,
            _ => crate::data::NodeShape::Rectangle,
        };
        state.set_node_shape(shape);
        state.selection.primary = None;
        state.selected_edge_id = None;
        return ActionOutcome::none();
    }

    if menu_type == PinstarMenuType::OrientationPicker {
        let orientation = match label {
            "Top-Down" => crate::data::DiagramOrientation::TopDown,
            "Left-Right" => crate::data::DiagramOrientation::LeftRight,
            "Right-Left" => crate::data::DiagramOrientation::RightLeft,
            "Bottom-Up" => crate::data::DiagramOrientation::DownTop,
            _ => crate::data::DiagramOrientation::TopDown,
        };
        state.set_orientation(orientation);
        state.selection.primary = None;
        state.selected_edge_id = None;
        return ActionOutcome::none();
    }

    if menu_type == PinstarMenuType::EdgeMenu {
        match label {
            "Set Color..." => state.open_color_menu(menu_x, menu_y, true),
            "Set Style..." => {
                let kind = PinstarMenuType::EdgeStylePicker;
                state.context_menu = Some(crate::menu::PinstarContextMenu::new(
                    menu_x,
                    menu_y,
                    crate::menu::menu_specs(
                        kind,
                        false,
                        state.format,
                        state.settings.enable_image_nodes,
                    ),
                    kind,
                ));
            }
            _ => {}
        }
        return ActionOutcome::none();
    }

    if menu_type == PinstarMenuType::EdgeStylePicker {
        let style = match label {
            "Solid" => EdgeStyle::Solid,
            "Dashed" => EdgeStyle::Dashed,
            "Dotted" => EdgeStyle::Dotted,
            _ => EdgeStyle::Solid,
        };
        state.set_edge_style(style);
        state.selected_edge_id = None;
        state.selection.primary = None;
        return ActionOutcome::none();
    }

    // Canvas menu
    let node_id = state.selection.primary.clone();

    if let Some(id) = node_id {
        match label {
            "Create Connection" => state.start_connection(),
            "Delete Connection" => state.start_delete_connection(),
            "Rename Node" => {
                let _ = id;
                let mode = if state.settings.rename_uses_id {
                    RenameMode::Id
                } else {
                    RenameMode::Title
                };
                state.open_rename_popup(mode);
            }
            "Resize Node" => state.start_resize(),
            "Set Shape..." => state.open_shape_menu(menu_x, menu_y),
            "Set Color..." => state.open_color_menu(menu_x, menu_y, false),
            "Set Orientation..." => state.open_orientation_menu(menu_x, menu_y),
            "Delete All Connections" => state.delete_node_connections(),
            "Delete Node" => state.delete_selected_node(),
            _ => {}
        }
    } else {
        match label {
            "Add Text Node" => state.add_text_node(state.context_menu_pos.0, state.context_menu_pos.1),
            "Add Group" => state.add_group(state.context_menu_pos.0, state.context_menu_pos.1),
            "Add Image Node" => {
                return ActionOutcome::host(PinstarAction::AddImageNode);
            }
            _ => {}
        }
    }
    state.sync_to_raw_editor();
    ActionOutcome::none()
}

/// Result of one mouse event.
#[derive(Default)]
pub struct MouseOutcome {
    pub consumed: bool,
    pub notice: Option<&'static str>,
    /// Text the host should write to the system clipboard.
    pub clipboard: Option<String>,
}

impl MouseOutcome {
    fn consumed() -> Self {
        Self {
            consumed: true,
            ..Self::default()
        }
    }
}

/// Merged mouse handler: clin's canvas flow (marquee, multi-select, image
/// resize handles, text selection) with standalone's flowchart right-drag
/// (select-rect) and editor context menu.
pub fn handle_pinstar_mouse(
    state: &mut PinstarState,
    mouse: MouseEvent,
    area: Rect,
) -> MouseOutcome {
    let mut area = area;
    area.height = area.height.saturating_sub(1);
    if state.rename_popup.is_some() {
        return MouseOutcome::consumed();
    }

    let (editor_area, canvas_area) = if state.show_editor_pane {
        let main_chunks = ratatui::layout::Layout::default()
            .direction(ratatui::layout::Direction::Horizontal)
            .constraints([
                ratatui::layout::Constraint::Percentage(30),
                ratatui::layout::Constraint::Percentage(70),
            ])
            .split(area);
        (Some(main_chunks[0]), main_chunks[1])
    } else {
        (None, area)
    };

    let (cx, cy) = state.screen_to_canvas(mouse.column, mouse.row, canvas_area);
    state.last_mouse_canvas_pos = Some((cx, cy));

    match mouse.kind {
        MouseEventKind::Down(MouseButton::Right) => {
            if state.resizing_node_id.is_some() {
                if state.format != SupportedFormat::Canvas {
                    state.resizing_node_id = None;
                    let _ = state.save();
                }
                return MouseOutcome::consumed();
            }

            if editor_area.is_some_and(|rect| contains_cell(rect, mouse.column, mouse.row))
                || state
                    .floating_editor_rect
                    .is_some_and(|rect| contains_cell(rect, mouse.column, mouse.row))
            {
                if state.format != SupportedFormat::Canvas
                    && let Some(rect) = editor_area
                    && contains_cell(rect, mouse.column, mouse.row)
                {
                    state.open_editor_context_menu(mouse.column, mouse.row);
                }
                return MouseOutcome::consumed();
            }

            if state.format == SupportedFormat::Canvas {
                state.right_down_screen = Some((mouse.column, mouse.row));
                state.marquee.on_down(cx, cy);
            } else {
                let hit_node = state.node_at(cx, cy);
                if hit_node.is_some() {
                    state.open_context_menu(mouse.column, mouse.row, cx, cy);
                } else if state.format.is_flowchart()
                    && state.format != SupportedFormat::Mermaid
                    && state.select_edge_at(cx, cy).is_some()
                {
                    state.open_edge_context_menu(mouse.column, mouse.row);
                } else {
                    // Right-click on empty space: start selection rectangle
                    state.select_rect_start = Some((cx, cy));
                    state.select_rect_end = Some((cx, cy));
                    state.last_mouse_pos = Some((mouse.column, mouse.row));
                }
            }
            MouseOutcome::consumed()
        }
        MouseEventKind::Drag(MouseButton::Right) => {
            if state.format == SupportedFormat::Canvas {
                if state.connection_source_id.is_none()
                    && state.deleting_connection_source_id.is_none()
                    && state.resizing_node_id.is_none()
                {
                    let dragging = if let Some((sx, sy)) = state.right_down_screen {
                        state
                            .marquee
                            .is_dragging_screen(mouse.column, mouse.row, sx, sy)
                    } else {
                        false
                    };
                    if dragging {
                        let (cx, cy) = state.screen_to_canvas(mouse.column, mouse.row, canvas_area);
                        state.marquee.on_drag(cx, cy);
                        if let Some(start) = state.marquee.start {
                            state.select_nodes_in_rect(start.0, start.1, cx, cy);
                        }
                    }
                }
            } else if state.select_rect_start.is_some() {
                let (cx, cy) = state.screen_to_canvas(mouse.column, mouse.row, canvas_area);
                state.select_rect_end = Some((cx, cy));
                state.last_mouse_pos = Some((mouse.column, mouse.row));
            }
            MouseOutcome::consumed()
        }
        MouseEventKind::Up(MouseButton::Right) => {
            if state.format == SupportedFormat::Canvas {
                if state.resizing_node_id.is_some() {
                    state.resizing_node_id = None;
                    state.is_dragging_resize_handle = false;
                    let _ = state.save();
                    state.sync_to_raw_editor();

                    state.right_down_screen = None;
                    state.marquee.clear();
                    state.drag_start_pos = None;
                    return MouseOutcome::consumed();
                }
                let dragging = if let Some((sx, sy)) = state.right_down_screen {
                    state
                        .marquee
                        .is_dragging_screen(mouse.column, mouse.row, sx, sy)
                } else {
                    false
                };
                if state.marquee.start.is_some() && dragging {
                    state.marquee.clear();
                    state.right_down_screen = None;
                    return MouseOutcome::consumed();
                }
                if !dragging {
                    let (cx, cy) = state.screen_to_canvas(mouse.column, mouse.row, canvas_area);
                    // Edge hit takes precedence over node hit
                    if state.select_edge_at(cx, cy).is_some() {
                        state.open_edge_context_menu(mouse.column, mouse.row);
                    } else {
                        state.select_node_at(cx, cy);
                        if state.selection.primary.is_none() {
                            state.selection.clear_set();
                            state.selected_edge_id = None;
                        }
                        state.open_context_menu(mouse.column, mouse.row, cx, cy);
                    }
                }
                state.right_down_screen = None;
                state.marquee.clear();
                state.drag_start_pos = None;
            } else if let (Some(start), Some(end)) = (state.select_rect_start, state.select_rect_end)
            {
                if (start.0 - end.0).abs() > 5.0 || (start.1 - end.1).abs() > 5.0 {
                    // Significant drag: select nodes in rectangle
                    state.select_nodes_in_rect(start.0, start.1, end.0, end.1);
                    if state.format.is_flowchart()
                        && state.format != SupportedFormat::Mermaid
                        && state.selected_edge_id.is_some()
                    {
                        state.open_edge_context_menu(mouse.column, mouse.row);
                    }
                } else {
                    // Just a click: show add-node menu
                    state.context_menu_pos = (start.0, start.1);
                    state.open_context_menu(mouse.column, mouse.row, start.0, start.1);
                }
                state.select_rect_start = None;
                state.select_rect_end = None;
                state.last_mouse_pos = None;
            }
            MouseOutcome::consumed()
        }
        MouseEventKind::Down(MouseButton::Middle) => {
            state.last_mouse_pos = Some((mouse.column, mouse.row));
            MouseOutcome::consumed()
        }
        MouseEventKind::Up(MouseButton::Middle) => {
            state.is_panning = false;
            state.last_mouse_pos = None;
            MouseOutcome::consumed()
        }
        MouseEventKind::Drag(MouseButton::Middle) => {
            state.is_panning = true;
            if let Some((lx, ly)) = state.last_mouse_pos {
                let dx = mouse.column as f64 - lx as f64;
                let dy = mouse.row as f64 - ly as f64;
                state.pan(-dx, -dy);
                state.last_mouse_pos = Some((mouse.column, mouse.row));
                MouseOutcome::consumed()
            } else {
                MouseOutcome::default()
            }
        }
        MouseEventKind::Down(MouseButton::Left) => {
            let mut menu_action: Option<(PinstarMenuType, String, u16, u16)> = None;
            let mut close_menu = false;

            if let Some(menu) = &state.context_menu {
                close_menu = true;
                let rect = menu.rect(canvas_area);
                if let Some(idx) = menu.row_at(rect, mouse.column, mouse.row) {
                    menu_action = menu
                        .label(idx)
                        .map(|l| (menu.menu_type, l.to_string(), menu.x, menu.y));
                }
            }

            if close_menu {
                state.context_menu = None;
            }

            if let Some((menu_type, label, mx, my)) = menu_action {
                execute_menu_action(state, &label, menu_type, mx, my);
                return MouseOutcome::consumed();
            }

            // Edge-list overlay: clicking a row selects that edge and opens
            // its context menu. Only when no menu is already open.
            if state.context_menu.is_none()
                && let Some(ov) = state.edge_overlay_rect
                && mouse.column > ov.x
                && mouse.column < ov.x + ov.width.saturating_sub(1)
                && mouse.row > ov.y
                && mouse.row < ov.y + ov.height.saturating_sub(1)
            {
                let row = mouse.row as usize - ov.y as usize - 1;
                if state.select_edge_of_selected_node(row + 1).is_some() {
                    state.open_edge_menu_centered(area);
                }
                return MouseOutcome::consumed();
            }
            if let Some(floating_area) = state.floating_editor_rect
                && let Some(editor) = &mut state.floating_editor
                && contains_cell(floating_area, mouse.column, mouse.row)
            {
                let (scroll_row, scroll_col) = get_textarea_scroll(editor);
                move_textarea_cursor_to_mouse_scrolled(
                    editor,
                    floating_area,
                    mouse.column,
                    mouse.row,
                    scroll_row,
                    scroll_col,
                );
                state.mouse_selection.begin(editor);
                state.text_selection_target = Some(crate::state::PinstarTextField::Floating);
                return MouseOutcome::consumed();
            }

            if let Some(editor_area) = editor_area {
                if contains_cell(editor_area, mouse.column, mouse.row) {
                    state.editor_focus = true;
                    let digits = state.raw_editor.lines().len().max(1).to_string().len() as u16;
                    let gutter_width = digits + 2;
                    let body_inner = ratatui::layout::Rect::new(
                        editor_area.x + gutter_width,
                        editor_area.y + 1,
                        editor_area.width.saturating_sub(gutter_width + 1),
                        editor_area.height.saturating_sub(1),
                    );
                    let (sr, sc) = get_textarea_scroll(&state.raw_editor);
                    move_textarea_cursor_to_mouse_scrolled(
                        &mut state.raw_editor,
                        body_inner,
                        mouse.column,
                        mouse.row,
                        sr,
                        sc,
                    );
                    state.mouse_selection.begin(&mut state.raw_editor);
                    state.text_selection_target = Some(crate::state::PinstarTextField::Raw);
                    return MouseOutcome::consumed();
                } else {
                    state.editor_focus = false;
                }
            }

            let (cx, cy) = state.screen_to_canvas(mouse.column, mouse.row, canvas_area);

            if state.connection_source_id.is_some() {
                if let Some(target_id) = state.select_node_at(cx, cy) {
                    state.finish_connection(&target_id);
                } else {
                    state.connection_source_id = None;
                }
                return MouseOutcome::consumed();
            }

            if state.deleting_connection_source_id.is_some() {
                if let Some(target_id) = state.select_node_at(cx, cy) {
                    state.finish_delete_connection(&target_id);
                } else {
                    state.deleting_connection_source_id = None;
                }
                return MouseOutcome::consumed();
            }

            if let Some(resizing_id) = &state.resizing_node_id
                && let Some(node) = state.data.nodes.iter().find(|n| n.id() == resizing_id)
            {
                let (nx, ny) = node.pos();
                let (nw, nh) = node.size();
                let handle_x = nx + nw;
                let handle_y = ny + nh;

                let tolerance = 10.0 / state.zoom;
                if cx >= handle_x - tolerance
                    && cx <= handle_x + tolerance
                    && cy >= handle_y - tolerance
                    && cy <= handle_y + tolerance
                {
                    state.is_dragging_resize_handle = true;
                    state.last_mouse_pos = Some((mouse.column, mouse.row));
                    return MouseOutcome::consumed();
                }
            }

            if state.floating_editor.is_some() {
                let prev_selected = state.selection.primary.clone();
                let hit_node = state.node_at(cx, cy);

                if hit_node != prev_selected {
                    state.toggle_editor();
                    state.sync_to_raw_editor();

                    if hit_node.is_none() {
                        state.selection.clear();
                        return MouseOutcome::consumed();
                    }
                } else {
                    return MouseOutcome::consumed();
                }
            }

            let is_double_click = if let Some((lx, ly, lt)) = state.last_click {
                lx == mouse.column && ly == mouse.row && lt.elapsed().as_millis() < 500
            } else {
                false
            };

            state.has_dragged = false;
            let hit_node = state.node_at(cx, cy);

            if is_double_click && let Some(id) = hit_node.clone() {
                state.selection.select_only(id);
                if state.ext_editor_enabled {
                    state.trigger_ext_editor = true;
                } else {
                    state.toggle_editor();
                }
                state.sync_to_raw_editor();
                state.last_click = None;
            } else if let Some(id) = hit_node {
                if !state.selection.is_selected(&id) {
                    state.selection.select_only(id.clone());
                } else {
                    state.selection.primary = Some(id.clone());
                    state.selection.extra.remove(&id);
                }
                if state.format != SupportedFormat::Canvas {
                    state.capture_group_children();
                }
                state.record_undo_state();
                state.drag_start_pos = Some((cx, cy));
                state.capture_drag_nodes();
                state.last_click = Some((mouse.column, mouse.row, std::time::Instant::now()));
            } else {
                state.selection.clear();
                state.selected_edge_id = None;
                state.last_click = Some((mouse.column, mouse.row, std::time::Instant::now()));
            }

            state.last_mouse_pos = Some((mouse.column, mouse.row));
            MouseOutcome::consumed()
        }
        MouseEventKind::Up(MouseButton::Left) => {
            state.is_panning = false;
            state.is_dragging_resize_handle = false;
            let mut outcome = MouseOutcome::consumed();
            if state.format != SupportedFormat::Canvas
                && state.mouse_selecting
                && !state.mouse_dragged
            {
                state.raw_editor.cancel_selection();
            }
            state.mouse_selecting = false;
            state.mouse_dragged = false;

            let notice = match state.text_selection_target.take() {
                Some(crate::state::PinstarTextField::Raw) => {
                    state.mouse_selection.finish(&mut state.raw_editor)
                }
                Some(crate::state::PinstarTextField::Floating) => state
                    .floating_editor
                    .as_mut()
                    .and_then(|editor| state.mouse_selection.finish(editor)),
                None => None,
            };
            if let Some((notice, clipboard)) = notice {
                outcome.notice = Some(notice);
                outcome.clipboard = Some(clipboard);
            }

            if state.drag_start_pos.is_some() {
                if !state.has_dragged
                    && let Some(id) = state.selection.primary.clone()
                {
                    state.selection.select_only(id);
                }
                state.drag_start_pos = None;
                state.drag_captured_nodes.clear();
                state.drag_group_children.clear();
                let _ = state.save();
                state.sync_to_raw_editor();
            }
            state.last_mouse_pos = None;
            outcome
        }
        MouseEventKind::Drag(MouseButton::Left) => {
            if state.mouse_selection.active {
                state.mouse_selection.mark_drag();
                match state.text_selection_target {
                    Some(crate::state::PinstarTextField::Raw) => {
                        if let Some(editor_area) = editor_area {
                            let digits =
                                state.raw_editor.lines().len().max(1).to_string().len() as u16;
                            let gutter_width = digits + 2;
                            let body_inner = ratatui::layout::Rect::new(
                                editor_area.x + gutter_width,
                                editor_area.y + 1,
                                editor_area.width.saturating_sub(gutter_width + 1),
                                editor_area.height.saturating_sub(1),
                            );
                            let (scroll_row, scroll_col) =
                                get_textarea_scroll(&state.raw_editor);
                            move_textarea_cursor_to_mouse_scrolled(
                                &mut state.raw_editor,
                                body_inner,
                                mouse.column,
                                mouse.row,
                                scroll_row,
                                scroll_col,
                            );
                        }
                    }
                    Some(crate::state::PinstarTextField::Floating) => {
                        if let (Some(editor_area), Some(editor)) =
                            (state.floating_editor_rect, state.floating_editor.as_mut())
                        {
                            let (scroll_row, scroll_col) = get_textarea_scroll(editor);
                            move_textarea_cursor_to_mouse_scrolled(
                                editor,
                                editor_area,
                                mouse.column,
                                mouse.row,
                                scroll_row,
                                scroll_col,
                            );
                        }
                    }
                    None => {}
                }
                return MouseOutcome::consumed();
            }

            if state.resizing_node_id.is_some()
                && !state.locked
                && let Some((lx, ly)) = state.last_mouse_pos
            {
                let dw = mouse.column as f64 - lx as f64;
                let dh = mouse.row as f64 - ly as f64;
                state.resize_selected_node(dw / state.zoom, dh / state.zoom);
                state.last_mouse_pos = Some((mouse.column, mouse.row));
                state.sync_to_raw_editor();
                return MouseOutcome::consumed();
            }

            if let Some(last_pos) = state.drag_start_pos {
                if state.locked {
                    return MouseOutcome::consumed();
                }
                let (cx, cy) = state.screen_to_canvas(mouse.column, mouse.row, canvas_area);
                let dx = cx - last_pos.0;
                let dy = cy - last_pos.1;
                state.move_selected_node(dx, dy);
                state.has_dragged = true;
                state.drag_start_pos = Some((cx, cy));
                if state.show_editor_pane {
                    state.sync_to_raw_editor();
                }
                MouseOutcome::consumed()
            } else if let Some((lx, ly)) = state.last_mouse_pos {
                state.is_panning = true;
                let dx = mouse.column as f64 - lx as f64;
                let dy = mouse.row as f64 - ly as f64;
                state.pan(-dx, -dy);
                state.last_mouse_pos = Some((mouse.column, mouse.row));
                MouseOutcome::consumed()
            } else {
                MouseOutcome::default()
            }
        }
        MouseEventKind::ScrollUp => {
            if state.show_editor_pane && mouse.column < canvas_area.x {
                state.raw_editor.scroll((-3, 0));
            } else {
                state.zoom_in();
            }
            MouseOutcome::consumed()
        }
        MouseEventKind::ScrollDown => {
            if state.show_editor_pane && mouse.column < canvas_area.x {
                state.raw_editor.scroll((3, 0));
            } else {
                state.zoom_out();
            }
            MouseOutcome::consumed()
        }
        _ => MouseOutcome::default(),
    }
}

// ── standalone bin: default keymap router ─────────────────────────────────

pub fn handle_pinstar_event(
    state: &mut PinstarState,
    key: KeyEvent,
    running: &mut bool,
    area: Rect,
) -> bool {
    if state.show_help {
        match key.code {
            KeyCode::Tab => {
                state.help_tab = state.help_tab.next();
                state.help_scroll = 0;
            }
            KeyCode::BackTab => {
                state.help_tab = state.help_tab.prev();
                state.help_scroll = 0;
            }
            KeyCode::Char('j') | KeyCode::Down => {
                state.help_scroll = state.help_scroll.saturating_add(1);
            }
            KeyCode::Char('k') | KeyCode::Up => {
                state.help_scroll = state.help_scroll.saturating_sub(1);
            }
            KeyCode::Char('G') | KeyCode::PageDown => {
                state.help_scroll = state.help_scroll.saturating_add(10);
            }
            KeyCode::Char('g') | KeyCode::PageUp => {
                state.help_scroll = state.help_scroll.saturating_sub(10);
            }
            _ => {
                state.show_help = false;
                state.help_scroll = 0;
            }
        }
        return true;
    }

    if let Some(textarea) = &mut state.rename_popup {
        match key.code {
            KeyCode::Esc => {
                state.rename_popup = None;
            }
            KeyCode::Enter => {
                let new_id = textarea.lines().join("");
                if state.settings.rename_uses_id {
                    state.rename_node_id(new_id);
                } else {
                    state.rename_node_title(new_id);
                }
                state.rename_popup = None;
            }
            _ => {
                textarea.input(Input::from(key));
            }
        }
        return true;
    }

    let mut menu_action: Option<(PinstarMenuType, String, u16, u16)> = None;
    let mut close_menu = false;

    if let Some(menu) = &mut state.context_menu {
        match key.code {
            KeyCode::Esc | KeyCode::Char('q') => {
                close_menu = true;
            }
            KeyCode::Up | KeyCode::Char('k') => {
                menu.move_up();
            }
            KeyCode::Down | KeyCode::Char('j') => {
                menu.move_down();
            }
            KeyCode::Enter => {
                menu_action = menu
                    .label(menu.selected)
                    .map(|l| (menu.menu_type, l.to_string(), menu.x, menu.y));
                close_menu = true;
            }
            KeyCode::Char(c) => {
                if let Some(index) = menu.find_shortcut(c) {
                    menu_action = menu
                        .label(index)
                        .map(|l| (menu.menu_type, l.to_string(), menu.x, menu.y));
                    close_menu = true;
                }
            }
            _ => {}
        }
    }

    if close_menu {
        state.context_menu = None;
    }

    if let Some((menu_type, label, mx, my)) = menu_action {
        execute_menu_action(state, &label, menu_type, mx, my);
        return true;
    } else if close_menu {
        return true;
    }

    if state.context_menu.is_some() {
        return true;
    }

    if let Some(editor) = &mut state.floating_editor {
        match key.code {
            KeyCode::Esc => {
                state.toggle_editor();
            }
            KeyCode::Enter if key.modifiers.contains(KeyModifiers::CONTROL) => {
                state.toggle_editor();
            }
            _ => {
                editor.input(Input::from(key));
                if let Some(node_id) = &state.selection.primary {
                    let text = editor.lines().join("\n");
                    for node in &mut state.data.nodes {
                        if node.id() == node_id {
                            node.set_text(text);
                            break;
                        }
                    }
                    let _ = state.save();
                }
            }
        }
        return true;
    }

    if state.resizing_node_id.is_some() {
        match key.code {
            KeyCode::Enter | KeyCode::Esc | KeyCode::Char('q') => {
                state.resizing_node_id = None;
                state.is_dragging_resize_handle = false;
                let _ = state.save();
                return true;
            }
            _ => {}
        }
    }

    if state.editor_focus {
        match key.code {
            KeyCode::Esc => {
                state.editor_focus = false;
            }
            KeyCode::Enter if key.modifiers.contains(KeyModifiers::ALT) => {
                state.editor_focus = false;
            }
            KeyCode::Char('s') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                let _ = state.sync_from_raw_editor();
            }
            _ => {
                state.raw_editor.input(Input::from(key));
            }
        }
        return true;
    }

    match key.code {
        KeyCode::Esc | KeyCode::Char('q') => {
            if state.connection_source_id.is_some() {
                state.connection_source_id = None;
            } else {
                *running = false;
            }
        }
        KeyCode::Enter if key.modifiers.contains(KeyModifiers::ALT) => {
            if state.show_editor_pane {
                state.editor_focus = true;
            }
        }
        KeyCode::Char('l') if key.modifiers.contains(KeyModifiers::CONTROL) => {
            state.locked = !state.locked;
        }
        KeyCode::Char('?') | KeyCode::Char('/') => {
            state.show_help = true;
        }
        KeyCode::Char('o') if key.modifiers.contains(KeyModifiers::CONTROL) => {
            if state.format == SupportedFormat::Canvas {
                state.orthogonal_connections = !state.orthogonal_connections;
            }
        }
        KeyCode::Char('z') | KeyCode::Char('Z')
            if key.modifiers.contains(KeyModifiers::CONTROL)
                && key.modifiers.contains(KeyModifiers::SHIFT) =>
        {
            let _ = state.redo();
        }
        KeyCode::Char('y') if key.modifiers.contains(KeyModifiers::CONTROL) => {
            let _ = state.redo();
        }
        KeyCode::Char('z') if key.modifiers.contains(KeyModifiers::CONTROL) => {
            let _ = state.undo();
        }
        KeyCode::Char('s') if key.modifiers.contains(KeyModifiers::CONTROL) => {
            let _ = state.save();
        }
        KeyCode::Char('r') if key.modifiers.contains(KeyModifiers::CONTROL) => {
            if state.format.is_flowchart() {
                state.cycle_orientation();
            } else {
                let _ = state.reload();
            }
        }
        KeyCode::Char('f') if key.modifiers.contains(KeyModifiers::CONTROL) => {
            let canvas_area = if state.show_editor_pane {
                let chunks = ratatui::layout::Layout::default()
                    .direction(ratatui::layout::Direction::Horizontal)
                    .constraints([
                        ratatui::layout::Constraint::Percentage(30),
                        ratatui::layout::Constraint::Percentage(70),
                    ])
                    .split(area);
                chunks[1]
            } else {
                area
            };
            state.fit_to_view(canvas_area);
        }
        KeyCode::Char('x') if key.modifiers.contains(KeyModifiers::CONTROL) => {
            state.ext_editor_enabled = !state.ext_editor_enabled;

            if state.ext_editor_enabled && state.show_editor_pane {
                state.show_editor_pane = false;
                state.editor_focus = false;
                state.trigger_whole_file_editor = true;
            }
        }
        KeyCode::Char('j') if key.modifiers.contains(KeyModifiers::CONTROL) => {
            state.zoom_in();
        }
        KeyCode::Char('k') if key.modifiers.contains(KeyModifiers::CONTROL) => {
            state.zoom_out();
        }
        KeyCode::Left | KeyCode::Char('h') => {
            state.select_node_in_direction(-1.0, 0.0);
            state.center_on_selected();
        }
        KeyCode::Right | KeyCode::Char('l') => {
            state.select_node_in_direction(1.0, 0.0);
            state.center_on_selected();
        }
        KeyCode::Up | KeyCode::Char('k') => {
            state.select_node_in_direction(0.0, -1.0);
            state.center_on_selected();
        }
        KeyCode::Down | KeyCode::Char('j') => {
            state.select_node_in_direction(0.0, 1.0);
            state.center_on_selected();
        }
        KeyCode::Char('+') | KeyCode::Char('=') => {
            state.zoom_in();
        }
        KeyCode::Char('-') | KeyCode::Char('_') => {
            state.zoom_out();
        }
        KeyCode::Char('c') if state.selection.primary.is_some() => {
            state.start_connection();
        }
        KeyCode::Char('d') if state.selection.primary.is_some() => {
            state.start_delete_connection();
        }
        KeyCode::Char('r') if state.selection.primary.is_some() => {
            state.open_rename_popup(RenameMode::Id);
        }
        KeyCode::Char('s') if state.selection.primary.is_some() => {
            state.start_resize();
        }
        KeyCode::Char('p')
            if state.selection.primary.is_some() && state.format != SupportedFormat::Canvas =>
        {
            let menu_x = (area.width / 2).saturating_sub(16);
            let menu_y = (area.height / 2).saturating_sub(3);
            state.open_shape_menu(menu_x, menu_y);
        }
        KeyCode::Char('o')
            if state.selection.primary.is_some()
                && state.format != SupportedFormat::Mermaid
                && state.format != SupportedFormat::PlantUml =>
        {
            let menu_x = (area.width / 2).saturating_sub(16);
            let menu_y = (area.height / 2).saturating_sub(6);
            state.open_color_menu(menu_x, menu_y, false);
        }
        KeyCode::Char('b') if state.selection.primary.is_some() => {
            state.delete_node_connections();
        }
        KeyCode::Char('x') if state.selection.primary.is_some() => {
            state.delete_selected_node();
        }
        KeyCode::Char('i') | KeyCode::Enter => {
            let target_id_opt = state.selection.primary.clone();
            if let Some(target_id) = target_id_opt {
                if state.connection_source_id.is_some() {
                    state.finish_connection(&target_id);
                } else if state.ext_editor_enabled {
                    state.trigger_ext_editor = true;
                } else {
                    state.toggle_editor();
                }
            }
        }
        KeyCode::Char('a') => {
            let menu_x = (area.width / 2).saturating_sub(12);
            let menu_y = area.height;

            let cx = state.viewport_x;
            let cy = state.viewport_y;

            if let Some(id) = &state.selection.primary {
                if state.data.nodes.iter().any(|n| n.id() == id) {
                    state.open_context_menu(menu_x, menu_y, cx, cy);
                }
            } else {
                state.open_context_menu(menu_x, menu_y, cx, cy);
            }
        }
        KeyCode::Char('g') if key.modifiers.contains(KeyModifiers::CONTROL) => {
            state.show_grid = !state.show_grid;
        }
        KeyCode::Char('e') if key.modifiers.contains(KeyModifiers::CONTROL) => {
            if state.ext_editor_enabled {
                state.trigger_whole_file_editor = true;
            } else {
                state.show_editor_pane = !state.show_editor_pane;
                if !state.show_editor_pane {
                    state.editor_focus = false;
                }
            }
        }

        _ => return false,
    }

    true
}
