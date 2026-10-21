//! The Pomodoro timer: countdown, progress, sessions and controls in one pane.

use alloc::format;
use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};

use crate::app::{Action, State};
use crate::pomodoro::{LONG_EVERY, PRESETS, Phase, PomoState};
use crate::targets::Targets;
use crate::term::{button_row, mid_text, mid_width, pane, top_bar};
use crate::theme::Theme;
use crate::widgets::bar;

pub fn phase_color(th: &Theme, phase: Phase) -> Color {
    if phase == Phase::Focus { th.accent } else { th.green }
}

pub fn draw(st: &mut State, t: &mut Targets, f: &mut Frame, area: Rect) {
    let body = top_bar(f, t, st, area, "pomodoro", Action::Menu);
    let th = st.theme;
    let p = &st.pomodoro;
    let now = st.uptime_ms;
    let color = phase_color(&th, p.phase);
    let title = match p.state {
        PomoState::Done if p.finished == Some(Phase::Focus) => "focus done - take a break",
        PomoState::Done => "break over - back to it",
        _ => p.phase.label(),
    };
    let inner = pane(f, &th, body, &title.to_lowercase(), true);

    let s = p.remaining(now);
    let text = format!("{:02}:{:02}", s / 60, s % 60);
    let w = mid_width(&text);
    let x = inner.x + (inner.width - w) / 2;
    let digits = if p.state == PomoState::Paused { th.dim } else { th.fg };
    mid_text(f.buffer_mut(), x, inner.y + 1, &text, digits);
    let progress = if p.active() { p.progress(now) } else { 0.0 };
    bar(f.buffer_mut(), Rect::new(inner.x + 4, inner.y + 5, inner.width - 8, 1), progress, color, th.panel2);

    // Status line: state, sessions towards the long break, today's count
    let state = match p.state {
        PomoState::Idle => "ready",
        PomoState::Running => "running",
        PomoState::Paused => "paused",
        PomoState::Done => format_next(p.phase),
    };
    let done = if p.cycle > 0 && p.cycle % LONG_EVERY == 0 && p.phase == Phase::Long { LONG_EVERY } else { p.cycle % LONG_EVERY };
    let mut spans = alloc::vec![Span::styled(format!("{state}   "), Style::new().fg(color).add_modifier(Modifier::BOLD))];
    for i in 0..LONG_EVERY {
        spans.push(Span::styled(if i < done { "●" } else { "○" }, Style::new().fg(if i < done { th.orange } else { th.muted })));
    }
    spans.push(Span::styled(format!("   today {}", p.today.1), Style::new().fg(th.dim)));
    f.render_widget(Line::from(spans).centered(), Rect::new(inner.x, inner.y + 7, inner.width, 1));

    let main = match p.state {
        PomoState::Idle => "▶ start",
        PomoState::Running => "‖ pause",
        PomoState::Paused => "▶ resume",
        PomoState::Done if p.phase == Phase::Focus => "▶ focus",
        PomoState::Done => "▶ break",
    };
    let (focus, short, _) = PRESETS[p.preset];
    let preset = format!("{focus}/{short} min");
    let items = [
        (main, Action::PomoMain, true),
        ("skip", Action::PomoSkip, false),
        ("reset", Action::PomoReset, false),
        (preset.as_str(), Action::PomoPreset, false),
    ];
    // Four big buttons across the bottom
    let y = inner.y + inner.height - 3;
    button_row(f, t, &th, Rect::new(inner.x + 1, y, inner.width - 2, 3), &items);
}

fn format_next(phase: Phase) -> &'static str {
    match phase {
        Phase::Focus => "next: focus",
        Phase::Short => "next: short break",
        Phase::Long => "next: long break",
    }
}
