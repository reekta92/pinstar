//! pinstar — terminal canvas/diagram editor library.
//!
//! Host-agnostic engine for Obsidian-compatible `.canvas` files plus Mermaid,
//! DOT and PlantUML flowcharts. Hosts (the `pinstar` binary, clin-rs's
//! canvas view) own keybinds, status lines, dialogs and the system clipboard;
//! everything canvas-related lives here.

pub mod camera;
pub mod data;
pub mod formats;
pub mod grid;
pub mod help;
#[cfg(feature = "images")]
pub mod image;
pub mod input;
pub mod menu;
pub mod overlay;
pub mod render;
pub mod selection;
pub mod state;
pub mod textsel;
pub mod theme;

use anyhow::{Context, Result};
use ratatui::style::Color;
use std::path::Path;

pub use input::{
    ActionCtx, ActionOutcome, MouseOutcome, PinstarAction, apply_action, execute_menu_action,
    handle_pinstar_event, handle_pinstar_mouse,
};
pub use menu::{
    MenuItemSpec, PinstarContextMenu, PinstarMenuType, menu_item_shortcut_char, menu_specs,
};
pub use overlay::{MarqueeState, draw_canvas_rect_filled, muted_canvas_selection_fill};
pub use render::draw_pinstar_view;
pub use selection::Selection;
pub use state::{PinstarHelpTab, PinstarSnapshot, PinstarState, RenameMode};
pub use textsel::MouseTextSelection;
pub use theme::ThemeColors;

/// Host capability/behavior switches, stored on [`PinstarState::settings`].
#[derive(Debug, Clone)]
pub struct Settings {
    /// Offer image-node creation (host must supply a file dialog).
    pub enable_image_nodes: bool,
    /// LRU size for the decoded-image cache (images feature).
    pub image_cache_size: usize,
    /// Rename popup renames the node id (standalone) instead of its title.
    pub rename_uses_id: bool,
    /// Render internal hint bar at the bottom. Disable if host provides its own hints.
    pub show_hints: bool,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            enable_image_nodes: false,
            image_cache_size: 32,
            rename_uses_id: false,
            show_hints: true,
        }
    }
}

/// After this much idle time following a zoom/pan/resize, the view is
/// considered settled and real pixel images resume rendering.
pub const TRANSFORM_SETTLE: std::time::Duration = std::time::Duration::from_millis(150);

pub const COLOR_PICKER_PALETTE: &[(&str, &str, Color)] = &[
    ("Red", "#ff5252", Color::Rgb(255, 82, 82)),
    ("Orange", "#ff9800", Color::Rgb(255, 152, 0)),
    ("Yellow", "#ffeb3b", Color::Rgb(255, 235, 59)),
    ("Green", "#4caf50", Color::Rgb(76, 175, 80)),
    ("Cyan", "#00bcd4", Color::Rgb(0, 188, 212)),
    ("Purple", "#9c27b0", Color::Rgb(156, 39, 176)),
    ("Blue", "#2196f3", Color::Rgb(33, 150, 243)),
    ("Magenta", "#e91e63", Color::Rgb(233, 30, 99)),
    ("White", "#ffffff", Color::Rgb(255, 255, 255)),
];

/// Write `content` to `path` atomically (temp file in the same directory +
/// rename), so a crash mid-write never truncates the canvas file.
pub fn atomic_write(path: &Path, content: &str) -> Result<()> {
    let parent = path.parent().unwrap_or_else(|| Path::new("."));
    let tmp = parent.join(format!(
        ".{}.{}.tmp",
        path.file_name().and_then(|s| s.to_str()).unwrap_or("tmp"),
        uuid::Uuid::new_v4()
    ));
    std::fs::write(&tmp, content).with_context(|| format!("failed to write {}", tmp.display()))?;

    #[cfg(unix)]
    {
        let f = std::fs::File::open(&tmp).context("failed to open temp file for syncing")?;
        f.sync_all().context("failed to sync temp file")?;
        drop(f);
    }

    std::fs::rename(&tmp, path).with_context(|| {
        format!(
            "failed to rename temp file {} to {}",
            tmp.display(),
            path.display()
        )
    })?;
    Ok(())
}
