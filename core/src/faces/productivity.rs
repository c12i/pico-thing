//! TODOs from the Mac, a robot that reacts to them, and a rotating quote.

use alloc::format;
use alloc::string::String;
use alloc::vec::Vec;
use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::Style;
use ratatui::text::{Line, Span};

use super::bot::{self, Mood};
use super::strip;
use crate::app::State;
use crate::config;
use crate::term::{pane, text};

pub const DEFAULT_QUOTES: &[&str] = &[
    "Lock in.",
    "Avoid distractions. Do the work.",
    "Keep learning.",
    "Have you learnt something today?",
    "Slow improvement is still improvement.",
    "Keep going.",
    "1% better every day.",
    "Deep work beats busy work.",
    "Small steps, every single day.",
    "Discipline over motivation.",
    "Done is better than perfect.",
    "One thing at a time.",
    "Ship it, then improve it.",
    "Consistency compounds.",
    "Progress, not perfection.",
    "Stay curious.",
    "Read the docs. Then the code.",
    "Rest is part of the work.",
    "Future you will thank you.",
    "Hard things are learnable.",
];

const QUOTE_SECS: u64 = 300;
const ROWS: usize = 8;
const ITEM_W: usize = 29;
const QUOTE_W: usize = 44;
const OVERWHELMED: usize = 6;
const NIGHT: (u8, u8) = (23, 6);
/// Marquee: hold, scroll a character every STEP_MS to the end, hold, restart
const HOLD_MS: u64 = 2000;
pub const STEP_MS: u64 = 300;

fn quotes() -> &'static [&'static str] {
    if config::QUOTES.is_empty() { DEFAULT_QUOTES } else { config::QUOTES }
}

/// What the face shows of a text that may not fit: `width` chars, scrolled.
fn marquee(text: &str, width: usize, elapsed_ms: u64) -> String {
    let n = text.chars().count();
    if n <= width {
        return text.into();
    }
    let span = (n - width) as u64;
    let period = 2 * HOLD_MS + span * STEP_MS;
    let t = elapsed_ms % period;
    let off = if t < HOLD_MS { 0 } else { ((t - HOLD_MS) / STEP_MS).min(span) } as usize;
    text.chars().skip(off).take(width).collect()
}

/// True while something on the face is scrolling (it then redraws every STEP_MS).
pub fn animating(st: &State) -> bool {
    let long_item = st.todos.as_ref().is_some_and(|t| t.iter().take(ROWS).any(|i| i.chars().count() > ITEM_W));
    long_item || quote(st).chars().count() > 2 * QUOTE_W
}

fn quote(st: &State) -> &'static str {
    let q = quotes();
    let secs = st.now().map(|n| n.hour as u64 * 3600 + n.minute as u64 * 60).unwrap_or(st.uptime_ms / 1000);
    q[(secs / QUOTE_SECS) as usize % q.len()]
}

pub fn mood(st: &State) -> Mood {
    use crate::pomodoro::{Phase, PomoState};
    let p = &st.pomodoro;
    match p.state {
        PomoState::Running => return if p.phase == Phase::Focus { Mood::Busy } else { Mood::Sleeping },
        PomoState::Paused => return Mood::Bored,
        PomoState::Done if p.finished == Some(Phase::Focus) => return Mood::Excited,
        _ => {}
    }
    if st.uptime_ms < st.excited_until_ms {
        return Mood::Excited;
    }
    let Some(todos) = &st.todos else { return Mood::Confused };
    if st.companion_offline {
        return Mood::Confused;
    }
    if let Some(n) = st.now() {
        if n.hour >= NIGHT.0 || n.hour < NIGHT.1 {
            return Mood::Sleeping;
        }
    }
    match todos.len() {
        0 => Mood::Chill,
        n if n >= OVERWHELMED => Mood::Overwhelmed,
        _ => Mood::Happy,
    }
}

pub fn draw(st: &State, f: &mut Frame, area: Rect) {
    strip(st, f, area);
    let th = st.theme;
    let elapsed = st.uptime_ms.saturating_sub(st.todos_at_ms);
    // Panes: todos and the robot side by side, the quote along the bottom
    let top = Rect::new(area.x, area.y + 2, area.width, 10);
    let todo_area = Rect::new(top.x, top.y, area.width - bot::W - 3, top.height);
    let bot_area = Rect::new(todo_area.x + todo_area.width - 1, top.y, area.width - todo_area.width + 1, top.height);
    let title = match &st.todos {
        Some(items) => format!("todo · {} open", items.len()),
        None => String::from("todo"),
    };
    let list = pane(f, &th, todo_area, &title, false);
    match &st.todos {
        None => text(f, list.x + 1, list.y + 1, list.width, "waiting for your mac…", Style::new().fg(th.dim)),
        Some(items) => {
            if items.is_empty() {
                text(f, list.x + 1, list.y + 1, list.width, "all clear. nice.", Style::new().fg(th.dim));
            }
            let rows = list.height as usize;
            let shown = if items.len() <= rows { items.len() } else { rows - 1 };
            let w = list.width as usize - 3;
            for (i, item) in items.iter().take(shown).enumerate() {
                let line = Line::from(alloc::vec![
                    Span::styled(" ☐ ", Style::new().fg(th.dim)),
                    Span::styled(marquee(item, w.min(ITEM_W), elapsed), Style::new().fg(th.fg)),
                ]);
                f.render_widget(line, Rect::new(list.x, list.y + i as u16, list.width, 1));
            }
            if items.len() > shown {
                text(f, list.x + 3, list.y + shown as u16, list.width, &format!("+{} more", items.len() - shown), Style::new().fg(th.dim));
            }
        }
    }
    let b = pane(f, &th, bot_area, "", false);
    bot::draw(f.buffer_mut(), b.x + (b.width - bot::W) / 2, b.y + (b.height - bot::H) / 2, mood(st), (st.uptime_ms / 1000) as u32, &th);

    // The quote, on two lines or scrolling on one
    let q_area = Rect::new(area.x, top.y + top.height - 1, area.width, area.height - top.height - 1);
    let inner = pane(f, &th, q_area, "", false);
    let q = quote(st);
    let lines = wrap(q, QUOTE_W);
    let y0 = inner.y + inner.height.saturating_sub(lines.len().min(2) as u16) / 2;
    if lines.len() <= 2 {
        for (i, l) in lines.iter().enumerate() {
            f.render_widget(Line::styled(l.as_str(), Style::new().fg(th.fg)).centered(), Rect::new(inner.x, y0 + i as u16, inner.width, 1));
        }
    } else {
        f.render_widget(
            Line::styled(marquee(q, QUOTE_W, st.uptime_ms), Style::new().fg(th.fg)).centered(),
            Rect::new(inner.x, y0, inner.width, 1),
        );
    }
}

/// Word-wrap to lines of at most `width` characters.
pub fn wrap(text: &str, width: usize) -> Vec<String> {
    let mut lines = Vec::new();
    let mut line = String::new();
    for word in text.split_whitespace() {
        if !line.is_empty() && line.chars().count() + 1 + word.chars().count() > width {
            lines.push(core::mem::take(&mut line));
        }
        if !line.is_empty() {
            line.push(' ');
        }
        line.push_str(word);
    }
    if !line.is_empty() {
        lines.push(line);
    }
    lines
}
