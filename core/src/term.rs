//! The terminal look: tmux-style panes that share borders, a top bar with the path,
//! a back control, activity spinner and clock, block buttons, thick sliders, a
//! scrollbar, and mid-size digits.
//!
//! Everything tappable is finger-sized: a block button at least two rows tall, or a
//! whole row or pane. Each registers a touch target, and the app flashes whatever
//! was hit in reverse video before acting on it.

use alloc::format;
use alloc::string::String;
use ratatui::Frame;
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::symbols::merge::MergeStrategy;
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, BorderType};

use crate::app::{Action, State};
use crate::targets::Targets;
use crate::theme::Theme;

const SPINNER: [char; 10] = ['⠋', '⠙', '⠹', '⠸', '⠼', '⠴', '⠦', '⠧', '⠇', '⠏'];

/// A braille spinner frame (it turns every 100ms).
pub fn spinner(st: &State) -> char {
    SPINNER[(st.uptime_ms / 100) as usize % SPINNER.len()]
}

/// A pane: a plain border shared with its neighbours (lay panes out overlapping by
/// one cell) and a title set into the top border. Returns the inside.
pub fn pane(f: &mut Frame, th: &Theme, area: Rect, title: &str, focused: bool) -> Rect {
    let (border, title_style) = if focused {
        (th.accent, Style::new().fg(th.accent).add_modifier(Modifier::BOLD))
    } else {
        (th.muted, Style::new().fg(th.dim))
    };
    let block = Block::bordered()
        .border_type(BorderType::Plain)
        .border_style(Style::new().fg(border))
        .merge_borders(MergeStrategy::Exact);
    let block = if title.is_empty() { block } else { block.title(Span::styled(format!(" {title} "), title_style)) };
    let inner = block.inner(area);
    f.render_widget(block, area);
    inner
}

/// Rows taken by the top bar.
pub const BAR_H: u16 = 2;

/// The top bar, two rows tall: a big `‹ back` block on the left (tapping anywhere on
/// the bar goes back), the path, then a spinner while the network is busy, and the
/// clock. Returns the area below it.
pub fn top_bar(f: &mut Frame, t: &mut Targets, st: &State, area: Rect, path: &str, back: Action) -> Rect {
    let th = &st.theme;
    let bar = Rect::new(area.x, area.y, area.width, BAR_H);
    let back_w = 10;
    // Text on the top row; the second row is a half-height strip (`▀`) of the same
    // colours, so the text has some room under it
    f.render_widget(Block::new().style(Style::new().bg(th.panel)), Rect::new(bar.x, bar.y, bar.width, 1));
    f.render_widget(Block::new().style(Style::new().bg(th.accent)), Rect::new(bar.x, bar.y, back_w, 1));
    let buf = f.buffer_mut();
    for x in bar.x..bar.x + bar.width {
        if let Some(c) = buf.cell_mut((x, bar.y + 1)) {
            c.set_char('▀').set_fg(if x < bar.x + back_w { th.accent } else { th.panel }).set_bg(th.bg);
        }
    }
    let mid = bar.y;
    let style = Style::new().fg(th.bg).bg(th.accent).add_modifier(Modifier::BOLD);
    f.render_widget(Line::styled(" ‹ back", style), Rect::new(bar.x, mid, back_w, 1));
    let path_style = Style::new().fg(th.accent).bg(th.panel).add_modifier(Modifier::BOLD);
    f.render_widget(Line::styled(format!(" ~/{path}"), path_style), Rect::new(bar.x + back_w, mid, bar.width - back_w, 1));
    let mut right = String::new();
    if st.busy() {
        right.push(spinner(st));
        right.push(' ');
    }
    if let Some(n) = st.now() {
        right.push_str(&format!("{:02}:{:02} ", n.hour, n.minute));
    }
    // Styled as a span, so the line doesn't repaint the row (and the back block)
    let rest = Rect::new(bar.x + back_w, mid, bar.width - back_w, 1);
    f.render_widget(Line::from(Span::styled(right, Style::new().fg(th.dim).bg(th.panel))).right_aligned(), rest);
    // A generous back target: the whole bar and the border row under it
    // (the top edge is where a resistive panel is least accurate). Tabs and chips
    // drawn into that border row are added later, so they still win where they are.
    t.add(Rect::new(area.x, area.y, area.width, BAR_H + 1), back);
    Rect::new(area.x, area.y + BAR_H, area.width, area.height.saturating_sub(BAR_H))
}

/// A list row spanning the area's width: a tap target, highlighted when selected.
pub fn row(f: &mut Frame, t: &mut Targets, th: &Theme, area: Rect, line: Line, action: Action, selected: bool) {
    if selected {
        f.render_widget(Block::new().style(Style::new().bg(th.panel)), area);
    }
    f.render_widget(line, Rect::new(area.x, area.y, area.width, 1));
    t.add(area, action);
}

pub fn text(f: &mut Frame, x: u16, y: u16, w: u16, s: &str, style: Style) {
    f.render_widget(Line::styled(s, style), Rect::new(x, y, w, 1));
}

pub fn clip(s: &str, n: usize) -> String {
    if s.chars().count() <= n { s.into() } else { s.chars().take(n.saturating_sub(1)).chain(core::iter::once('…')).collect() }
}

/// Reverse-video a rectangle (tap feedback).
pub fn invert(buf: &mut Buffer, area: Rect) {
    let a = area.intersection(buf.area);
    for y in a.y..a.y + a.height {
        for x in a.x..a.x + a.width {
            if let Some(c) = buf.cell_mut((x, y)) {
                c.modifier.toggle(Modifier::REVERSED);
            }
        }
    }
}

// Mid-size digits: the 3x5 pixel font, drawn with half blocks so each pixel is one
// cell wide and half a cell tall (10x10px, square). A digit is 3x3 cells.
const DIGITS: [u16; 11] = [
    0b111_101_101_101_111,
    0b110_010_010_010_111,
    0b111_001_111_100_111,
    0b111_001_111_001_111,
    0b101_101_111_001_001,
    0b111_100_111_001_111,
    0b111_100_111_101_111,
    0b111_001_001_001_001,
    0b111_101_111_101_111,
    0b111_101_111_001_111,
    0b000_000_111_000_000,
];

pub fn mid_width(s: &str) -> u16 {
    s.chars().map(|c| if c == ':' { 1 } else { 3 }).sum::<u16>() + s.chars().count().saturating_sub(1) as u16
}

pub fn mid_text(buf: &mut Buffer, x: u16, y: u16, s: &str, color: Color) {
    let mut cx = x;
    for ch in s.chars() {
        let rows: [u8; 6] = if ch == ':' {
            [0, 1, 0, 1, 0, 0]
        } else {
            let g = match ch {
                '0'..='9' => DIGITS[ch as usize - '0' as usize],
                '-' => DIGITS[10],
                _ => 0,
            };
            let mut r = [0u8; 6];
            for (i, v) in r.iter_mut().take(5).enumerate() {
                *v = ((g >> (3 * (4 - i))) & 0b111) as u8;
            }
            r
        };
        let w = if ch == ':' { 1 } else { 3 };
        for cy in 0..3u16 {
            let (top, bottom) = (rows[2 * cy as usize], rows[2 * cy as usize + 1]);
            for c in 0..w {
                let bit = if ch == ':' { 1 } else { 0b100 >> c };
                let glyph = match (top & bit != 0, bottom & bit != 0) {
                    (true, true) => '█',
                    (true, false) => '▀',
                    (false, true) => '▄',
                    _ => continue,
                };
                if let Some(cell) = buf.cell_mut((cx + c, y + cy)) {
                    cell.set_char(glyph).set_fg(color);
                }
            }
        }
        cx += w + 1;
    }
}

/// Width of the scrollbar along the right edge of the screen (50px: thumb-sized).
pub const SCROLLBAR_W: u16 = 5;

/// Rows taken by each of the scrollbar's arrow buttons.
const ARROW_H: u16 = 3;

/// A scrollbar filling `area` (normally the rightmost SCROLLBAR_W columns): `▲` and
/// `▼` buttons at the ends step a page at a time, and the track between them can be
/// tapped or dragged. Records how far the list can scroll.
pub fn scrollbar(f: &mut Frame, t: &mut Targets, st: &mut State, area: Rect, total: usize, visible: usize) {
    let th = st.theme;
    st.scroll_max = total.saturating_sub(visible);
    st.scroll = st.scroll.min(st.scroll_max);
    let step = visible.saturating_sub(1).max(1) as i16;
    let up = Rect::new(area.x, area.y, area.width, ARROW_H);
    let down = Rect::new(area.x, area.y + area.height.saturating_sub(ARROW_H), area.width, ARROW_H);
    let track = Rect::new(area.x, area.y + ARROW_H, area.width, area.height.saturating_sub(2 * ARROW_H));
    let h = track.height as usize;
    let buf = f.buffer_mut();
    let (size, pos) = if st.scroll_max == 0 {
        (h, 0)
    } else {
        let size = (h * visible / total.max(1)).clamp(1, h);
        (size, (h - size) * st.scroll / st.scroll_max)
    };
    for r in 0..h {
        let on = r >= pos && r < pos + size;
        for c in 0..track.width {
            if let Some(cell) = buf.cell_mut((track.x + c, track.y + r as u16)) {
                if on {
                    cell.set_char('█').set_fg(if st.scroll_max == 0 { th.panel2 } else { th.accent });
                } else {
                    cell.set_char(if c == track.width / 2 { '│' } else { ' ' }).set_fg(th.muted).set_bg(th.bg);
                }
            }
        }
    }
    for r in 0..track.height {
        t.add(Rect::new(track.x, track.y + r, track.width, 1), Action::ScrollTo(r, track.height));
    }
    // The arrows go last so they win over the track rows next to them
    block_button(f, t, &th, up, "▲", Action::ScrollBy(-step), false);
    block_button(f, t, &th, down, "▼", Action::ScrollBy(step), false);
}

/// A solid block button (no border): a filled rectangle with a centred label.
pub fn block_button(f: &mut Frame, t: &mut Targets, th: &Theme, area: Rect, label: &str, action: Action, active: bool) {
    let (fg, bg) = if active { (th.bg, th.accent) } else { (th.fg, th.panel2) };
    f.render_widget(Block::new().style(Style::new().bg(bg)), area);
    let y = area.y + area.height.saturating_sub(1) / 2;
    f.render_widget(
        Line::styled(label, Style::new().fg(fg).bg(bg).add_modifier(Modifier::BOLD)).centered(),
        Rect::new(area.x, y, area.width, 1),
    );
    t.add(area, action);
}

/// `n` slots of equal width across `area`, a column apart; any spare columns go
/// into the margins so the row stays centred.
pub fn slots(area: Rect, n: u16) -> alloc::vec::Vec<Rect> {
    let n = n.max(1);
    let w = (area.width.saturating_sub(n - 1)) / n;
    let x0 = area.x + (area.width - (n * w + n - 1)) / 2;
    (0..n).map(|k| Rect::new(x0 + k * (w + 1), area.y, w, area.height)).collect()
}

/// A row of block buttons of equal width filling `area`: (label, action, active).
/// Returns where each went.
pub fn button_row(f: &mut Frame, t: &mut Targets, th: &Theme, area: Rect, items: &[(&str, Action, bool)]) -> alloc::vec::Vec<Rect> {
    let rects = slots(area, items.len() as u16);
    for (r, (label, action, active)) in rects.iter().zip(items) {
        block_button(f, t, th, *r, label, action.clone(), *active);
    }
    rects
}

/// A slider several rows thick, with ten tap zones the full height.
pub fn thick_slider(f: &mut Frame, t: &mut Targets, th: &Theme, area: Rect, fraction: f32, color: Color, pick: impl Fn(u8) -> Action) {
    let n = ((area.width as f32) * fraction.clamp(0.0, 1.0) + 0.5) as u16;
    let buf = f.buffer_mut();
    for y in area.y..area.y + area.height {
        for i in 0..area.width {
            if let Some(c) = buf.cell_mut((area.x + i, y)) {
                // The empty part is shaded, so it doesn't read as a button
                if i < n {
                    c.set_char('█').set_fg(color);
                } else {
                    c.set_char('░').set_fg(th.muted);
                }
            }
        }
    }
    for z in 0..10u16 {
        let x0 = area.x + area.width * z / 10;
        let x1 = area.x + area.width * (z + 1) / 10;
        t.add(Rect::new(x0, area.y, x1 - x0, area.height), pick(((z + 1) * 10) as u8));
    }
}
