//! Mouse-driven text selection for canvas textareas (ported from clin's
//! `text_edit::MouseTextSelection`; the clipboard write is surfaced to hosts
//! via the returned text instead of performed here).

use ratatui_textarea::TextArea;

#[derive(Debug, Default, Clone)]
pub struct MouseTextSelection {
    pub active: bool,
    pub dragged: bool,
}

impl MouseTextSelection {
    pub fn begin(&mut self, textarea: &mut TextArea<'static>) {
        textarea.start_selection();
        self.active = true;
        self.dragged = false;
    }

    pub fn mark_drag(&mut self) {
        if self.active {
            self.dragged = true;
        }
    }

    /// Finish a selection. Returns `Some((notice, clipboard_text))` when a
    /// dragged selection was copied — hosts write `clipboard_text` to the
    /// system clipboard and show `notice`.
    pub fn finish(&mut self, textarea: &mut TextArea<'static>) -> Option<(&'static str, String)> {
        if !self.active {
            return None;
        }
        let dragged = self.dragged;
        self.active = false;
        self.dragged = false;
        if !dragged {
            textarea.cancel_selection();
            return None;
        }
        textarea.selection_range()?;
        textarea.copy();
        Some(("Copied to clipboard", textarea.yank_text()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn click_without_drag_cancels_selection() {
        let mut sel = MouseTextSelection::default();
        let mut ta = TextArea::from(["hello"]);
        sel.begin(&mut ta);
        let out = sel.finish(&mut ta);
        assert!(out.is_none());
        assert!(ta.selection_range().is_none());
    }
}
