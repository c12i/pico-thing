//! The launcher: six live mini terminal panes. Each shows a glimpse of its screen;
//! tap one to open it.

use alloc::format;
use alloc::string::String;
use alloc::vec::Vec;
use ratatui::Frame;
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};

use crate::app::{Action, Screen, State};
use crate::faces::FaceKind;
use crate::pomodoro::{Phase, PomoState};
use crate::targets::Targets;
use crate::term::{clip, pane, top_bar};

type Lines = Vec<Line<'static>>;

fn l(text: String, color: Color) -> Line<'static> {
    Line::styled(text, Style::new().fg(color))
}

fn two(a: String, ac: Color, b: String, bc: Color) -> Line<'static> {
    Line::from(alloc::vec![Span::styled(a, Style::new().fg(ac)), Span::styled(b, Style::new().fg(bc))])
}

fn faces(st: &State, w: usize) -> Lines {
    let th = st.theme;
    // The current face first, so it shows however many fit
    let cur = st.settings.face;
    core::iter::once(&cur)
        .chain(FaceKind::ALL.iter().filter(|f| **f != cur))
        .map(|f| {
            let on = *f == st.settings.face;
            two(if on { "● ".into() } else { "  ".into() }, th.accent, clip(&f.name().to_lowercase(), w - 2), if on { th.fg } else { th.dim })
        })
        .collect()
}

fn lights(st: &State, w: usize) -> Lines {
    let th = st.theme;
    let mut v: Lines = st
        .lights
        .iter()
        .map(|l| {
            let (dot, c, s) = match &l.state {
                _ if l.transport.is_none() => ("✗ ", th.muted, String::from("token")),
                Some(Ok(s)) if s.power => ("● ", th.yellow, format!("{}%", s.bright)),
                Some(Ok(_)) => ("○ ", th.dim, String::from("off")),
                Some(Err(_)) => ("✗ ", th.red, String::from("offline")),
                None => ("· ", th.muted, String::new()),
            };
            // Long names lose all but their first word ("corner lamp" -> "corner")
            let full = l.name.to_lowercase();
            let room = w.saturating_sub(s.len() + 3);
            let name = if full.chars().count() <= room { full } else { clip(full.split_whitespace().next().unwrap_or(""), room) };
            let pad = w.saturating_sub(2 + name.chars().count() + s.len());
            Line::from(alloc::vec![
                Span::styled(dot, Style::new().fg(c)),
                Span::styled(name, Style::new().fg(th.fg)),
                Span::raw(" ".repeat(pad)),
                Span::styled(s, Style::new().fg(th.dim)),
            ])
        })
        .collect();
    if v.is_empty() {
        v.push(l("no lights".into(), th.dim));
    }
    v
}

fn sonos(st: &State, w: usize) -> Lines {
    let th = st.theme;
    let mut v = Lines::new();
    for sp in &st.speakers {
        let (mark, c, what) = match &sp.state {
            Some(Ok(s)) if s.playing() => ("▶ ", th.green, if s.title.is_empty() { s.source.clone() } else { s.title.clone() }),
            Some(Ok(s)) => ("‖ ", th.dim, if s.title.is_empty() { String::from("idle") } else { s.title.clone() }),
            Some(Err(_)) => ("✗ ", th.red, String::from("offline")),
            None => ("· ", th.muted, String::new()),
        };
        v.push(two(mark.into(), c, clip(&sp.name.to_lowercase(), w - 2), th.fg));
        v.push(l(format!("  {}", clip(&what, w - 2)), th.dim));
    }
    v
}

fn command(st: &State, w: usize) -> Lines {
    let th = st.theme;
    let Some(c) = &st.companion else {
        return alloc::vec![l(if st.companion_error.is_some() { "mac offline".into() } else { "connecting…".into() }, th.dim)];
    };
    let mut v: Lines = c
        .sessions
        .iter()
        .take(4)
        .map(|s| {
            let c = match s.state.as_str() {
                "waiting" => th.orange,
                "working" => th.accent,
                "done" => th.green,
                _ => th.muted,
            };
            two("▌".into(), c, clip(&s.project, w - 1), th.fg)
        })
        .collect();
    if v.is_empty() {
        v.push(l("no sessions".into(), th.dim));
    }
    v
}

fn settings(st: &State, _w: usize) -> Lines {
    let th = st.theme;
    let ip = match st.status.ip {
        Some([.., c, d]) => format!("wifi  .{c}.{d}"),
        None => String::from("wifi  offline"),
    };
    alloc::vec![
        l(ip, th.dim),
        l(format!("theme {}", if st.settings.dark { "dark" } else { "light" }), th.dim),
        l(format!("light {}%", st.settings.brightness), th.dim),
        l(format!("mem   {}K free", st.status.heap_free_kb), th.dim),
    ]
}

fn pomodoro(st: &State, _w: usize) -> Lines {
    let th = st.theme;
    let p = &st.pomodoro;
    let s = p.remaining(st.uptime_ms);
    let color = if p.phase == Phase::Focus { th.accent } else { th.green };
    let state = match p.state {
        PomoState::Idle => "idle",
        PomoState::Running => "running",
        PomoState::Paused => "paused",
        PomoState::Done => "done!",
    };
    alloc::vec![
        Line::styled(format!("{:02}:{:02}", s / 60, s % 60), Style::new().fg(th.fg).add_modifier(Modifier::BOLD)),
        l(format!("{} · {state}", p.phase.label().to_lowercase()), color),
        l(format!("today {}", p.today.1), th.dim),
    ]
}

pub fn draw(st: &mut State, t: &mut Targets, f: &mut Frame, area: Rect) {
    let body = top_bar(f, t, st, area, "", Action::Home);
    let th = st.theme;
    let panes: [(&str, Screen, fn(&State, usize) -> Lines); 6] = [
        ("◷ faces", Screen::FacePicker, faces),
        ("☼ lights", Screen::Lights, lights),
        ("♫ sonos", Screen::Sonos, sonos),
        (">_ command", Screen::Command, command),
        ("⚙ settings", Screen::Settings, settings),
        ("◔ pomodoro", Screen::Pomodoro, pomodoro),
    ];
    // Panes overlap by a cell so neighbours share their borders
    let rows = Layout::vertical([Constraint::Fill(1); 2]).spacing(-1).split(body);
    for (r, row) in rows.iter().enumerate() {
        let cols = Layout::horizontal([Constraint::Fill(1); 3]).spacing(-1).split(*row);
        for (c, cell) in cols.iter().enumerate() {
            let (title, screen, content) = &panes[r * 3 + c];
            let inner = pane(f, &th, *cell, title, false);
            for (i, line) in content(st, inner.width as usize).into_iter().take(inner.height as usize).enumerate() {
                f.render_widget(line, Rect::new(inner.x, inner.y + i as u16, inner.width, 1));
            }
            t.add(*cell, Action::Open(*screen));
        }
    }
}
