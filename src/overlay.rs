//! Marquee drag state + translucent fill (ported from clin's `ui::canvas_overlay`).

use ratatui::{Frame, layout::Rect, style::Color};

pub struct MarqueeState {
    pub start: Option<(f64, f64)>,
    pub end: Option<(f64, f64)>,
    pub threshold_cells: u32,
}

impl MarqueeState {
    pub fn new(threshold_cells: u32) -> Self {
        Self {
            start: None,
            end: None,
            threshold_cells,
        }
    }

    pub fn on_down(&mut self, x: f64, y: f64) {
        self.start = Some((x, y));
        self.end = Some((x, y));
    }

    pub fn on_drag(&mut self, x: f64, y: f64) {
        self.end = Some((x, y));
    }

    pub fn is_dragging_screen(
        &self,
        sx_now: u16,
        sy_now: u16,
        sx_start: u16,
        sy_start: u16,
    ) -> bool {
        let dx = sx_now.abs_diff(sx_start);
        let dy = sy_now.abs_diff(sy_start);
        u32::from(dx) + u32::from(dy) > self.threshold_cells
    }

    pub fn commit_rect(&self) -> Option<(f64, f64, f64, f64)> {
        let (sx, sy) = self.start?;
        let (ex, ey) = self.end?;
        Some((sx.min(ex), sy.min(ey), sx.max(ex), sy.max(ey)))
    }

    pub fn clear(&mut self) {
        self.start = None;
        self.end = None;
    }
}

/// Marquee fill color shared by hosts.
pub fn muted_canvas_selection_fill(accent: Color, highlight_bg: Color) -> Color {
    match accent {
        Color::Cyan => Color::Rgb(0, 68, 68),
        Color::Green => Color::Rgb(0, 68, 34),
        Color::Yellow => Color::Rgb(68, 68, 0),
        Color::Magenta => Color::Rgb(68, 0, 68),
        Color::Red => Color::Rgb(68, 0, 0),
        Color::Blue => Color::Rgb(0, 0, 68),
        _ => highlight_bg,
    }
}

/// Translucent marquee fill preserving every underlying glyph and foreground.
pub fn draw_canvas_rect_filled(frame: &mut Frame, rect: Rect, fill: Color) {
    let buf = frame.buffer_mut();
    for row in rect.y..rect.y.saturating_add(rect.height) {
        for col in rect.x..rect.x.saturating_add(rect.width) {
            if let Some(cell) = buf.cell_mut((col, row)) {
                cell.set_bg(fill);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn on_down_sets_start_end() {
        let mut m = MarqueeState::new(3);
        m.on_down(1.0, 2.0);
        assert_eq!(m.start, Some((1.0, 2.0)));
        assert_eq!(m.end, Some((1.0, 2.0)));
    }

    #[test]
    fn on_drag_updates_end() {
        let mut m = MarqueeState::new(3);
        m.on_down(0.0, 0.0);
        m.on_drag(5.0, 7.0);
        assert_eq!(m.end, Some((5.0, 7.0)));
    }

    #[test]
    fn clear_nukes_both() {
        let mut m = MarqueeState::new(3);
        m.on_down(0.0, 0.0);
        m.clear();
        assert!(m.start.is_none());
        assert!(m.end.is_none());
    }

    #[test]
    fn commit_rect_normalizes_both_directions() {
        let mut m = MarqueeState::new(3);
        m.on_down(10.0, 20.0);
        m.on_drag(0.0, 40.0);
        assert_eq!(m.commit_rect(), Some((0.0, 20.0, 10.0, 40.0)));
    }

    #[test]
    fn is_dragging_screen_manhattan() {
        let m = MarqueeState::new(3);
        // Manhattan move of 3 -> not dragging (strict threshold).
        assert!(!m.is_dragging_screen(3, 0, 0, 0));
        assert!(!m.is_dragging_screen(2, 1, 0, 0));
        // Manhattan move of 4 -> dragging.
        assert!(m.is_dragging_screen(4, 0, 0, 0));
        assert!(m.is_dragging_screen(2, 2, 0, 0));
    }
}
