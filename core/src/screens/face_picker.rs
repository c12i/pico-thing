//! Choose the home face.

use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};

use crate::app::{Action, State};
use crate::faces::FaceKind;
use crate::targets::Targets;
use crate::term::{SCROLLBAR_W, pane, row, scrollbar, top_bar};

fn blurb(f: FaceKind) -> &'static str {
    match f {
        FaceKind::Digital => "tty-clock and the weather",
        FaceKind::Analog => "a braille dial, date and weather",
        FaceKind::Weather => "today in detail",
        FaceKind::Productivity => "todos, a robot, quotes, the timer",
        FaceKind::Bonsai => "cbonsai --live --infinite",
        FaceKind::System => "your mac's cpu, memory, network",
        FaceKind::Hourglass => "turn it over, watch the sand",
        FaceKind::Pet => "a pet to feed, play with, clean",
    }
}

pub fn draw(st: &mut State, t: &mut Targets, f: &mut Frame, area: Rect) {
    let body = top_bar(f, t, st, area, "faces", Action::Menu);
    let th = st.theme;
    // The list, and a scrollbar along the right edge of the screen (as in favorites)
    let bar = Rect::new(body.x + body.width - SCROLLBAR_W, body.y, SCROLLBAR_W, body.height);
    let list = Rect::new(body.x, body.y, body.width - SCROLLBAR_W, body.height);
    let inner = pane(f, &th, list, "pick a face", true);
    let visible = (inner.height / 2) as usize;
    scrollbar(f, t, st, bar, FaceKind::ALL.len(), visible);
    for (i, face) in FaceKind::ALL.iter().enumerate().skip(st.scroll).take(visible) {
        let on = *face == st.settings.face;
        let r = Rect::new(inner.x, inner.y + (i - st.scroll) as u16 * 2, inner.width, 2);
        let name = Style::new().fg(th.fg).add_modifier(if on { Modifier::BOLD } else { Modifier::empty() });
        let line = Line::from(alloc::vec![
            Span::styled(if on { " ● " } else { "   " }, Style::new().fg(th.accent)),
            Span::styled(face.name(), name),
            Span::styled(alloc::format!("  {}", blurb(*face)), Style::new().fg(th.dim)),
        ]);
        row(f, t, &th, r, line, Action::SetFace(*face), on);
    }
}
