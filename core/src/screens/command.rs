//! Command center: Claude Code sessions, saved agent tasks and Mac actions through
//! the companion server on the Mac. Also task results and the "Claude needs you"
//! alert.

use alloc::format;
use alloc::string::String;
use alloc::vec::Vec;
use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::Block;

use crate::app::{ATTENTION_MS, Action, Screen, State};
use crate::faces::productivity::wrap;
use crate::targets::Targets;
use crate::term::{SCROLLBAR_W, block_button, button_row, clip, pane, row, scrollbar, spinner, text, top_bar};
use crate::theme::Theme;

const TABS: [&str; 3] = ["agents", "tasks", "mac"];

fn state_style(th: &Theme, state: &str) -> (&'static str, Color) {
    match state {
        "waiting" => ("needs you", th.orange),
        "working" => ("working", th.accent),
        "done" => ("finished", th.green),
        "running" => ("running", th.accent),
        "failed" => ("failed", th.red),
        "idle" => ("idle", th.muted),
        _ => ("", th.muted),
    }
}

/// Tabs as a row of big blocks splitting `area`'s width; the current one is filled.
fn tabs(f: &mut Frame, t: &mut Targets, th: &Theme, area: Rect, current: u8) {
    let items: Vec<_> = TABS.iter().enumerate().map(|(i, name)| (*name, Action::CommandTab(i as u8), current == i as u8)).collect();
    button_row(f, t, th, area, &items);
}

pub fn draw(st: &mut State, t: &mut Targets, f: &mut Frame, area: Rect) {
    let body = top_bar(f, t, st, area, "command", Action::Menu);
    let th = st.theme;
    // A row's gap under the top bar (it's part of the back zone), then the tabs
    tabs(f, t, &th, Rect::new(body.x + 1, body.y + 1, body.width - 2, 2), st.command_tab);
    let list = Rect::new(body.x, body.y + 3, body.width, body.height - 3);
    let inner = pane(f, &th, list, TABS[st.command_tab as usize % TABS.len()], true);
    let w = inner.width as usize;
    let Some(c) = &st.companion else {
        let msg = if st.companion_error.is_some() { "can't reach your mac" } else { "connecting…" };
        text(f, inner.x + 1, inner.y + 2, inner.width, msg, Style::new().fg(th.dim));
        return;
    };
    // Two-line items with a gap between, each a tap target
    let slot = |i: usize| Rect::new(inner.x, inner.y + i as u16 * 3, inner.width, 2);
    let fits = (inner.height as usize + 1) / 3;
    match st.command_tab {
        0 | 1 => {
            let items: Vec<(String, String, String, Action)> = if st.command_tab == 0 {
                c.sessions
                    .iter()
                    .map(|s| {
                        let detail = if s.message.is_empty() { String::from("tap to open its terminal") } else { s.message.clone() };
                        (s.project.clone(), s.state.clone(), detail, Action::FocusSession(s.id.clone()))
                    })
                    .collect()
            } else {
                c.tasks
                    .iter()
                    .enumerate()
                    .map(|(i, task)| {
                        let hint = match task.state.as_str() {
                            "idle" => "tap to run",
                            "running" => "tap to watch",
                            "done" => "tap to read the result",
                            _ => "tap to see what went wrong",
                        };
                        (task.name.clone(), task.state.clone(), String::from(hint), Action::OpenTask(i))
                    })
                    .collect()
            };
            if items.is_empty() {
                let msg = if st.command_tab == 0 { "no claude code sessions yet" } else { "no tasks in companion/config.json" };
                text(f, inner.x + 1, inner.y + 2, inner.width, msg, Style::new().fg(th.dim));
            }
            for (i, (title, state, detail, action)) in items.into_iter().take(fits).enumerate() {
                let r = slot(i);
                let (label, color) = state_style(&th, &state);
                let line = Line::from(alloc::vec![
                    Span::styled("▌", Style::new().fg(color)),
                    Span::styled(clip(&title, w.saturating_sub(label.len() + 3)), Style::new().fg(th.fg).add_modifier(Modifier::BOLD)),
                ]);
                row(f, t, &th, r, line, action, false);
                f.render_widget(Line::styled(format!("{label} "), Style::new().fg(color)).right_aligned(), Rect::new(r.x, r.y, r.width, 1));
                f.render_widget(
                    Line::from(alloc::vec![Span::styled("▌", Style::new().fg(color)), Span::styled(clip(&detail, w - 2), Style::new().fg(th.dim))]),
                    Rect::new(r.x, r.y + 1, r.width, 1),
                );
            }
        }
        _ => {
            if c.actions.is_empty() {
                text(f, inner.x + 1, inner.y + 2, inner.width, "no actions configured", Style::new().fg(th.dim));
            }
            // A grid of big blocks filling the pane
            let n = c.actions.len().max(1) as u16;
            let cols = if n > 4 { 3 } else { 2 };
            let rows = n.div_ceil(cols);
            let h = ((inner.height + 1) / rows).saturating_sub(1).clamp(2, 4);
            let bw = (inner.width - 2 - (cols - 1)) / cols;
            for (k, a) in c.actions.iter().enumerate() {
                let (col, r) = (k as u16 % cols, k as u16 / cols);
                let area = Rect::new(inner.x + 1 + col * (bw + 1), inner.y + r * (h + 1), bw, h);
                if area.y + h > inner.y + inner.height {
                    break;
                }
                block_button(f, t, &th, area, &a.label.to_lowercase(), Action::MacAction(a.id.clone()), a.on == Some(true));
            }
        }
    }
}

pub fn draw_task(st: &mut State, t: &mut Targets, f: &mut Frame, area: Rect, n: usize) {
    let body = top_bar(f, t, st, area, "command/task", Action::Open(Screen::Command));
    let th = st.theme;
    let name = st.companion.as_ref().and_then(|c| c.tasks.get(n)).map(|t| t.name.to_lowercase()).unwrap_or_default();
    // The output, and a scrollbar along the right edge of the screen
    let bar = Rect::new(body.x + body.width - SCROLLBAR_W, body.y, SCROLLBAR_W, body.height);
    let inner = pane(f, &th, Rect::new(body.x, body.y, body.width - SCROLLBAR_W, body.height), &name, true);
    let w = inner.width as usize - 2;
    // The last two rows are the "run again" button
    let bottom = inner.y + inner.height - 2;
    let task = st.task.clone();
    match &task {
        Some((i, Ok(d))) if *i == n => {
            if d.state == "running" {
                text(f, inner.x + 1, inner.y + 4, inner.width, &format!("{} claude is working on it…", spinner(st)), Style::new().fg(th.dim));
            } else {
                let lines: Vec<String> = d
                    .output
                    .lines()
                    .flat_map(|l| {
                        let w = wrap(l, w);
                        if w.is_empty() { alloc::vec![String::new()] } else { w }
                    })
                    .collect();
                let visible = inner.height as usize - 3;
                scrollbar(f, t, st, bar, lines.len(), visible);
                for (k, l) in lines.iter().skip(st.scroll).take(visible).enumerate() {
                    text(f, inner.x + 1, inner.y + k as u16, inner.width - 2, l, Style::new().fg(th.fg));
                }
            }
            let (label, color) = state_style(&th, &d.state);
            f.render_widget(Line::styled(format!(" {label} "), Style::new().fg(color)).right_aligned(), Rect::new(body.x, body.y, body.width - 1, 1));
        }
        Some((i, Err(_))) if *i == n => text(f, inner.x + 1, inner.y + 4, inner.width, "couldn't load the result", Style::new().fg(th.red)),
        _ => text(f, inner.x + 1, inner.y + 4, inner.width, &format!("{} loading…", spinner(st)), Style::new().fg(th.dim)),
    }
    block_button(f, t, &th, Rect::new(inner.x + 1, bottom, inner.width - 2, 2), "▶ run again", Action::RunTask(n), false);
}

pub fn draw_attention(st: &mut State, t: &mut Targets, f: &mut Frame, area: Rect) {
    let th = st.theme;
    let Some((id, at)) = st.attention.clone() else { return };
    let s = st.session(&id).cloned().unwrap_or_default();
    let waiting = s.state == "waiting";
    let color = if waiting { th.orange } else { th.green };
    let banner = Rect::new(area.x, area.y, area.width, 1);
    f.render_widget(Block::new().style(Style::new().bg(color)), banner);
    let title = if waiting { " claude needs you" } else { " claude finished" };
    text(f, area.x, area.y, area.width, title, Style::new().fg(th.bg).bg(color).add_modifier(Modifier::BOLD));
    let left = ATTENTION_MS.saturating_sub(st.uptime_ms - at) / 1000;
    f.render_widget(Line::styled(format!("closing in {left}s "), Style::new().fg(th.bg).bg(color)).right_aligned(), banner);
    let body = Rect::new(area.x, area.y + 1, area.width, area.height - 1);
    let inner = pane(f, &th, body, &s.project, true);
    for (k, l) in wrap(&s.message, inner.width as usize - 2).iter().take(7).enumerate() {
        text(f, inner.x + 1, inner.y + 1 + k as u16, inner.width - 2, l, Style::new().fg(th.fg));
    }
    let y = inner.y + inner.height - 3;
    let half = (inner.width - 3) / 2;
    block_button(f, t, &th, Rect::new(inner.x + 1, y, half, 3), "focus terminal", Action::FocusSession(id), true);
    block_button(f, t, &th, Rect::new(inner.x + 2 + half, y, inner.width - 3 - half, 3), "dismiss", Action::Dismiss, false);
}
