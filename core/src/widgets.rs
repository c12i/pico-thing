//! Building blocks shared by the screens: bordered buttons that register their own
//! touch target, the screen header with its back button, and big block digits.

use ratatui::Frame;
use ratatui::buffer::Buffer;
use ratatui::layout::{Alignment, Constraint, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, BorderType, Paragraph};

use crate::app::{Action, State};
use crate::targets::Targets;
use crate::theme::Theme;

/// Header height in rows: a 3-row back button with the title beside it.
pub const HEADER: u16 = 3;

pub fn panel(th: &Theme) -> Block<'static> {
    Block::bordered().border_type(BorderType::Plain).border_style(Style::new().fg(th.muted))
}

/// A bordered button (needs 3 rows) whose whole area is a touch target.
pub fn button(f: &mut Frame, t: &mut Targets, th: &Theme, area: Rect, label: &str, action: Action, active: bool) {
    let (border, text, bg) = if active {
        (th.accent, Style::new().fg(th.accent).add_modifier(Modifier::BOLD), th.panel)
    } else {
        (th.muted, Style::new().fg(th.fg), th.bg)
    };
    let block = Block::bordered()
        .border_type(BorderType::Rounded)
        .border_style(Style::new().fg(border))
        .style(Style::new().bg(bg));
    let inner = block.inner(area);
    f.render_widget(block, area);
    let y = inner.y + inner.height.saturating_sub(1) / 2;
    f.render_widget(Line::styled(label, text).centered(), Rect::new(inner.x, y, inner.width, 1));
    t.add(area, action);
}

/// The top of every non-face screen: a back button and the title. Returns the rest.
pub fn header(f: &mut Frame, t: &mut Targets, st: &State, area: Rect, title: &str, back: Action) -> Rect {
    let th = &st.theme;
    let [head, body] = Layout::vertical([Constraint::Length(HEADER), Constraint::Fill(1)]).areas(area);
    let [b, title_area, clock_area] =
        Layout::horizontal([Constraint::Length(8), Constraint::Fill(1), Constraint::Length(8)]).areas(head);
    button(f, t, th, b, "<", back, false);
    let mid = Rect::new(title_area.x + 1, title_area.y + 1, title_area.width.saturating_sub(1), 1);
    f.render_widget(Span::styled(title, Style::new().fg(th.fg).add_modifier(Modifier::BOLD)), mid);
    if let Some(now) = st.now() {
        let clock = alloc::format!("{:02}:{:02}", now.hour, now.minute);
        f.render_widget(
            Paragraph::new(Span::styled(clock, Style::new().fg(th.dim))).alignment(Alignment::Right),
            Rect::new(clock_area.x, clock_area.y + 1, clock_area.width - 1, 1),
        );
    }
    body
}

// 3x5 pixel glyphs for big digits, one u16 per glyph, rows top to bottom, 3 bits each
const DIGITS: [u16; 11] = [
    0b111_101_101_101_111, // 0
    0b110_010_010_010_111, // 1
    0b111_001_111_100_111, // 2
    0b111_001_111_001_111, // 3
    0b101_101_111_001_001, // 4
    0b111_100_111_001_111, // 5
    0b111_100_111_101_111, // 6
    0b111_001_001_001_001, // 7
    0b111_101_111_101_111, // 8
    0b111_101_111_001_111, // 9
    0b000_000_111_000_000, // -
];
/// Each pixel is two cells wide (cells are 10x20px, so pixels come out square).
pub const DIGIT_W: u16 = 6;
pub const DIGIT_H: u16 = 5;

/// Width in cells of `text` drawn by `big_text` (digits, '-', ':' and ' ').
pub fn big_width(text: &str) -> u16 {
    let n = text.chars().count() as u16;
    text.chars().map(|c| if c == ':' { 2 } else { DIGIT_W }).sum::<u16>() + 2 * n.saturating_sub(1)
}

/// tty-clock style digits made of full blocks.
pub fn big_text(buf: &mut Buffer, x: u16, y: u16, text: &str, color: Color) {
    let mut cx = x;
    for ch in text.chars() {
        if ch == ':' {
            for r in [1, 3] {
                fill(buf, cx, y + r, 2, color);
            }
            cx += 4;
            continue;
        }
        let g = match ch {
            '0'..='9' => DIGITS[ch as usize - '0' as usize],
            '-' => DIGITS[10],
            _ => 0,
        };
        for r in 0..5u16 {
            let bits = (g >> (3 * (4 - r))) & 0b111;
            for c in 0..3u16 {
                if bits & (0b100 >> c) != 0 {
                    fill(buf, cx + c * 2, y + r, 2, color);
                }
            }
        }
        cx += DIGIT_W + 2;
    }
}

fn fill(buf: &mut Buffer, x: u16, y: u16, w: u16, color: Color) {
    for i in 0..w {
        if let Some(cell) = buf.cell_mut((x + i, y)) {
            cell.set_char('█').set_fg(color);
        }
    }
}

/// A one-row bar: `fraction` of it filled.
pub fn bar(buf: &mut Buffer, area: Rect, fraction: f32, fill_color: Color, empty: Color) {
    let n = ((area.width as f32) * fraction.clamp(0.0, 1.0) + 0.5) as u16;
    for i in 0..area.width {
        if let Some(cell) = buf.cell_mut((area.x + i, area.y)) {
            cell.set_char('━').set_fg(if i < n { fill_color } else { empty });
        }
    }
}
