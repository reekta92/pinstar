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
#[cfg(feature = "mermaid-preview")]
pub mod preview;
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
    use std::io::Write;
    let target = match std::fs::symlink_metadata(path) {
        Ok(metadata) if metadata.file_type().is_symlink() => path.canonicalize()?,
        Ok(_) => path.to_path_buf(),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => path.to_path_buf(),
        Err(error) => return Err(error.into()),
    };
    let path = target.as_path();
    let permissions = match std::fs::metadata(path) {
        Ok(metadata) => Some(metadata.permissions()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => None,
        Err(error) => return Err(error.into()),
    };
    let parent = path.parent().unwrap_or_else(|| Path::new("."));
    let tmp = parent.join(format!(
        ".{}.{}.tmp",
        path.file_name().and_then(|s| s.to_str()).unwrap_or("tmp"),
        uuid::Uuid::new_v4()
    ));
    let mut options = std::fs::OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    let mut file = options
        .open(&tmp)
        .with_context(|| format!("failed to create {}", tmp.display()))?;
    let result = (|| -> Result<()> {
        file.write_all(content.as_bytes())?;
        if let Some(permissions) = permissions {
            file.set_permissions(permissions)?;
        }
        file.sync_all().context("failed to sync temp file")?;
        drop(file);
        std::fs::rename(&tmp, path).with_context(|| {
            format!(
                "failed to rename temp file {} to {}",
                tmp.display(),
                path.display()
            )
        })?;
        Ok(())
    })();
    if result.is_err() {
        let _ = std::fs::remove_file(&tmp);
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn atomic_write_replaces_contents_and_cleans_up_on_failure() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("diagram.canvas");
        atomic_write(&path, "before").unwrap();
        atomic_write(&path, "after").unwrap();
        assert_eq!(std::fs::read_to_string(&path).unwrap(), "after");
        let directory = dir.path().join("directory");
        std::fs::create_dir(&directory).unwrap();
        assert!(atomic_write(&directory, "cannot replace directory").is_err());
        assert_eq!(std::fs::read_dir(dir.path()).unwrap().count(), 2);
    }

    #[cfg(unix)]
    #[test]
    fn atomic_write_preserves_private_permissions_and_symlinks() {
        use std::os::unix::fs::{PermissionsExt, symlink};
        let dir = tempfile::tempdir().unwrap();
        let target = dir.path().join("target.canvas");
        std::fs::write(&target, "before").unwrap();
        std::fs::set_permissions(&target, std::fs::Permissions::from_mode(0o600)).unwrap();
        let link = dir.path().join("link.canvas");
        symlink("target.canvas", &link).unwrap();
        atomic_write(&link, "after").unwrap();
        assert!(
            std::fs::symlink_metadata(&link)
                .unwrap()
                .file_type()
                .is_symlink()
        );
        assert_eq!(std::fs::read_to_string(&target).unwrap(), "after");
        assert_eq!(
            std::fs::metadata(&target).unwrap().permissions().mode() & 0o777,
            0o600
        );
    }
}
