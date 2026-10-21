//! The boot log: a terminal pane printing each step as the system comes up, with
//! `[ ok ]`, `[fail]` or a spinner while it's still running. Tap to skip it.

use alloc::format;
use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};

use crate::app::{Action, State};
use crate::targets::Targets;
use crate::term::{pane, spinner, text};

pub fn draw(st: &mut State, t: &mut Targets, f: &mut Frame, area: Rect) {
    let th = st.theme;
    let inner = pane(f, &th, area, "pico-thing · boot", true);
    let x = inner.x + 1;
    let w = inner.width - 2;
    text(f, x, inner.y, w, "$ pico-thing --boot", Style::new().fg(th.dim));
    // Like a terminal: once the log fills the pane, the oldest lines scroll off
    let rows = inner.height.saturating_sub(2) as usize;
    let skip = st.boot.len().saturating_sub(rows);
    let mut y = inner.y + 1;
    for l in st.boot.iter().skip(skip) {
        let mark = match l.ok {
            Some(true) => Span::styled("[ ok ] ", Style::new().fg(th.green).add_modifier(Modifier::BOLD)),
            Some(false) => Span::styled("[fail] ", Style::new().fg(th.red).add_modifier(Modifier::BOLD)),
            None => Span::styled(format!("[ {}  ] ", spinner(st)), Style::new().fg(th.accent)),
        };
        let body = Style::new().fg(if l.ok.is_none() { th.fg } else { th.dim });
        f.render_widget(Line::from(alloc::vec![mark, Span::styled(l.text.as_str(), body)]), Rect::new(x, y, w, 1));
        y += 1;
    }
    // A blinking cursor under the last line
    if (st.uptime_ms / 500) % 2 == 0 {
        text(f, x, y, 1, "█", Style::new().fg(th.accent));
    }
    t.add(area, Action::Home);
}
