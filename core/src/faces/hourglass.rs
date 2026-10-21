//! An hourglass: pick a time, tap the glass to turn it over, and watch the sand run.
//! While its own timer is idle it shows a running Pomodoro instead.
//!
//! The glass is drawn in half-block pixels (a cell is 10x20px, so `▀`/`▄` halves are
//! square): sand piles from the neck up in the top bulb and from the floor up in the
//! bottom one, with a trickle between them while it runs.

use alloc::format;
use ratatui::Frame;
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::Line;

use crate::app::{Action, State};
use crate::targets::Targets;
use crate::term::{button_row, mid_text, mid_width, pane};

/// Animation frame length while sand is falling.
pub const STEP_MS: u64 = 200;
const PRESETS: [(u32, &str); 4] = [(5 * 60, "5m"), (15 * 60, "15m"), (25 * 60, "25m"), (60 * 60, "60m")];

/// Half-widths of the top bulb's rows, top to bottom (the bottom bulb mirrors it).
const BULB: [i32; 10] = [7, 7, 7, 6, 5, 4, 3, 2, 1, 0];
const BULB_H: i32 = BULB.len() as i32;
/// The glass in pixels: a cap, two bulbs, a cap.
const GLASS_W: i32 = 2 * 7 + 5;
const GLASS_H: i32 = 2 * BULB_H + 4;

#[derive(Default)]
pub struct Glass {
    /// Seconds of sand
    secs: u32,
    /// When the top bulb was (or would have been) full
    start_ms: Option<u64>,
    /// The end has been announced
    rang: bool,
}

impl Glass {
    fn dur_ms(&self) -> u64 {
        self.secs.max(1) as u64 * 1000
    }

    fn elapsed(&self, now: u64) -> Option<u64> {
        Some((now - self.start_ms?).min(self.dur_ms()))
    }

    pub fn running(&self, now: u64) -> bool {
        self.elapsed(now).is_some_and(|e| e < self.dur_ms())
    }

    /// Fill the top bulb with this many seconds of sand and let it run.
    pub fn set(&mut self, secs: u32, now: u64) {
        self.secs = secs;
        self.start_ms = Some(now);
        self.rang = false;
    }

    /// Turn it over: whatever has run through starts running back.
    pub fn flip(&mut self, now: u64) {
        if self.secs == 0 {
            return self.set(PRESETS[2].0, now);
        }
        let back = match self.elapsed(now) {
            Some(e) => self.dur_ms() - e,
            None => 0,
        };
        self.start_ms = Some(now.saturating_sub(back));
        self.rang = false;
    }

    /// True once, when the sand runs out.
    pub fn tick(&mut self, now: u64) -> bool {
        if !self.rang && self.start_ms.is_some() && !self.running(now) {
            self.rang = true;
            return true;
        }
        false
    }
}

/// Paints half-block pixels into a buffer: (x, y) in pixels, two per cell row.
struct Pixels<'a> {
    buf: &'a mut Buffer,
    x0: u16,
    y0: u16,
    bg: Color,
}

impl Pixels<'_> {
    fn set(&mut self, x: i32, y: i32, c: Color) {
        if x < 0 || y < 0 {
            return;
        }
        let Some(cell) = self.buf.cell_mut((self.x0 + x as u16, self.y0 + (y / 2) as u16)) else { return };
        // Keep whichever half was painted already
        let (mut top, mut bottom) = match cell.symbol() {
            "▀" => (cell.fg, cell.bg),
            "▄" => (cell.bg, cell.fg),
            "█" => (cell.fg, cell.fg),
            _ => (self.bg, self.bg),
        };
        if y % 2 == 0 { top = c } else { bottom = c }
        cell.set_char('▀').set_fg(top).set_bg(bottom);
    }
}

/// Draw the glass with `left` (0..=1) of the sand still in the top bulb.
fn glass(st: &State, buf: &mut Buffer, x0: u16, y0: u16, left: f32, falling: bool) {
    let th = &st.theme;
    let mut px = Pixels { buf, x0, y0, bg: th.bg };
    let cx = GLASS_W / 2;
    let wood = th.orange;
    let sand = th.yellow;
    let glass = th.muted;
    for x in 0..GLASS_W {
        px.set(x, 0, wood);
        px.set(x, GLASS_H - 1, wood);
    }
    for x in 1..GLASS_W - 1 {
        px.set(x, 1, wood);
        px.set(x, GLASS_H - 2, wood);
    }
    let top_y = |i: i32| 2 + i;
    let bottom_y = |i: i32| 2 + BULB_H + (BULB_H - 1 - i);
    for i in 0..BULB_H {
        let w = BULB[i as usize] + 1;
        for y in [top_y(i), bottom_y(i)] {
            px.set(cx - w, y, glass);
            px.set(cx + w, y, glass);
        }
    }
    // Sand: the top bulb drains from its surface, the bottom one fills from its floor
    let cap: i32 = BULB.iter().map(|w| 2 * w + 1).sum();
    let fill = |amount: i32, rows: &mut dyn Iterator<Item = i32>, at: &dyn Fn(i32) -> i32, px: &mut Pixels| {
        let mut left = amount;
        for i in rows {
            let w = BULB[i as usize];
            let n = (2 * w + 1).min(left);
            left -= n;
            // Centre-out, so a part-filled row heaps in the middle
            for k in 0..n {
                let dx = if k % 2 == 0 { k / 2 } else { -(k / 2 + 1) };
                px.set(cx + dx, at(i), sand);
            }
            if left <= 0 {
                break;
            }
        }
    };
    let top = ((cap as f32) * left.clamp(0.0, 1.0) + 0.5) as i32;
    fill(top, &mut (0..BULB_H).rev(), &top_y, &mut px);
    // (bottom bulb: row 0 is the wide floor, filled first)
    fill(cap - top, &mut (0..BULB_H), &bottom_y, &mut px);
    if falling && top > 0 {
        // A dotted trickle from the neck down to the heap, moving each frame
        let phase = (st.uptime_ms / STEP_MS) as i32;
        let mut heap = cap - top;
        let mut heap_rows = 0;
        while heap > 0 && heap_rows < BULB_H {
            heap -= 2 * BULB[heap_rows as usize] + 1;
            heap_rows += 1;
        }
        // From just under the neck to just above the heap (or the floor)
        let end = if heap_rows == 0 { bottom_y(0) + 1 } else { bottom_y(heap_rows - 1) };
        for y in top_y(BULB_H - 1) + 1..end {
            if (y + phase) % 2 == 0 {
                px.set(cx, y, sand);
            }
        }
    }
}

pub fn draw(st: &mut State, t: &mut Targets, f: &mut Frame, area: Rect) {
    super::strip(st, f, area);
    let th = st.theme;
    let now = st.uptime_ms;
    let body = Rect::new(area.x, area.y + 2, area.width, area.height - 2);
    let gw = (GLASS_W as u16) + 4;
    let left_area = Rect::new(body.x, body.y, gw, body.height);
    let right_area = Rect::new(body.x + gw - 1, body.y, body.width - gw + 1, body.height);

    // Which sand to show: our own timer, or else a running Pomodoro
    let g = &st.faces.hourglass;
    let pomo = &st.pomodoro;
    let (remaining_ms, frac, falling, title) = if let Some(e) = g.elapsed(now) {
        let d = g.dur_ms();
        (d - e, 1.0 - e as f32 / d as f32, e < d, format!("{} min", g.secs / 60))
    } else if pomo.active() {
        let left = pomo.remaining(now) as u64 * 1000;
        (left, 1.0 - pomo.progress(now), pomo.state == crate::pomodoro::PomoState::Running, "pomodoro".into())
    } else {
        (0, 1.0, false, "turn me over".into())
    };

    let inner = pane(f, &th, left_area, "", false);
    let gx = inner.x + (inner.width - GLASS_W as u16) / 2;
    let gy = inner.y + (inner.height.saturating_sub((GLASS_H / 2) as u16)) / 2;
    glass(st, f.buffer_mut(), gx, gy, frac, falling);
    t.add(inner, Action::HourglassFlip);

    let d = pane(f, &th, right_area, &title, true);
    let s = (remaining_ms / 1000) as u32;
    let time = format!("{:02}:{:02}", s / 60, s % 60);
    let tx = d.x + (d.width - mid_width(&time)) / 2;
    mid_text(f.buffer_mut(), tx, d.y + 1, &time, if falling { th.fg } else { th.dim });
    let hint = if falling { "tap the glass to flip" } else if remaining_ms == 0 && g.start_ms.is_some() { "time's up" } else { "pick a time" };
    f.render_widget(
        Line::styled(hint, Style::new().fg(if hint == "time's up" { th.orange } else { th.dim }).add_modifier(Modifier::BOLD)).centered(),
        Rect::new(d.x, d.y + 5, d.width, 1),
    );
    // Four presets across the bottom
    let y = d.y + d.height - 3;
    let items = PRESETS.map(|(secs, label)| (label, Action::HourglassSet(secs), g.secs == secs && falling));
    button_row(f, t, &th, Rect::new(d.x + 1, y, d.width - 2, 3), &items);
}
