//! 9-point touch calibration. The platform sends raw readings while this is up.

use alloc::format;
use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use ratatui::text::Line;

use crate::app::State;

/// Target cells; their centres in pixels are (col*10+5, row*20+10).
pub const CELLS: [(u16, u16); 9] = [(2, 1), (24, 1), (45, 1), (2, 8), (24, 8), (45, 8), (2, 14), (24, 14), (45, 14)];
/// A worse average fit than this (px) means a mis-tap: start over.
pub const MAX_ERR: f32 = 25.0;

pub fn target_px(i: usize) -> (i32, i32) {
    let (c, r) = CELLS[i];
    (c as i32 * 10 + 5, r as i32 * 20 + 10)
}

pub fn draw(st: &State, f: &mut Frame, area: Rect) {
    let th = st.theme;
    let n = st.calib_samples.len();
    f.render_widget(
        Line::styled("Touch calibration", Style::new().fg(th.fg).add_modifier(Modifier::BOLD)).centered(),
        Rect::new(area.x, area.y + 4, area.width, 1),
    );
    let (msg, color) = match st.calib_message {
        Some(m) => (m, th.accent2),
        None => ("Tap the centre of each cross", th.dim),
    };
    f.render_widget(Line::styled(msg, Style::new().fg(color)).centered(), Rect::new(area.x, area.y + 5, area.width, 1));
    f.render_widget(
        Line::styled(format!("{} / {}", n + 1, CELLS.len()), Style::new().fg(th.dim)).centered(),
        Rect::new(area.x, area.y + 10, area.width, 1),
    );
    if let Some(&(c, r)) = CELLS.get(n) {
        let buf = f.buffer_mut();
        let mut put = |x: u16, y: u16, ch: char, color| {
            if let Some(cell) = buf.cell_mut((area.x + x, area.y + y)) {
                cell.set_char(ch).set_fg(color);
            }
        };
        put(c, r, '┼', th.accent2);
        put(c.saturating_sub(1), r, '─', th.accent);
        put(c + 1, r, '─', th.accent);
        if r > 0 {
            put(c, r - 1, '│', th.accent);
        }
        put(c, r + 1, '│', th.accent);
    }
}
