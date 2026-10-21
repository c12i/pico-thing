//! The pet face: a little creature pottering about its room, its needs beside it and
//! four big buttons below. Tap the room to give it a pat (or to hatch the egg).
//! The pet itself (needs, ageing, saving) is in `crate::pet`.

use alloc::format;
use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::Line;

use crate::app::{Action, State};
use crate::pet::{Mood, Pet, PetCmd};
use crate::targets::Targets;
use crate::term::{block_button, button_row, pane, text};
use crate::theme::Theme;

/// Animation frame length (it takes a step, blinks, bobs).
pub const STEP_MS: u64 = 400;
/// How long it shows a reaction (eating, playing, hearts…).
const REACT_MS: u64 = 2400;
const BODY_W: u16 = 9;

fn need_color(th: &Theme, v: u8) -> Color {
    if v < 25 { th.red } else if v < 50 { th.yellow } else { th.green }
}

/// What it most wants, in a word.
fn feeling(p: &Pet) -> &'static str {
    match p.mood() {
        Mood::Egg => "an egg",
        Mood::Asleep => "asleep",
        _ if p.poops > 0 && p.clean < 50 => "needs a clean",
        _ if p.food < 30 => "hungry",
        _ if p.energy < 25 => "sleepy",
        _ if p.fun < 30 => "bored",
        Mood::Happy => "happy",
        _ => "content",
    }
}

pub fn draw(st: &mut State, t: &mut Targets, f: &mut Frame, area: Rect) {
    super::strip(st, f, area);
    let th = st.theme;
    let now = st.uptime_ms;
    let step = now / STEP_MS;
    let p = st.settings.pet.clone();
    let reaction = st.faces.pet_reaction.filter(|(_, at)| now - at < REACT_MS).map(|(c, _)| c);

    let body = Rect::new(area.x, area.y + 2, area.width, area.height - 2);
    let buttons_h = 3;
    let top = Rect::new(body.x, body.y, body.width, body.height - buttons_h);
    let room_w = 31;
    let room = Rect::new(top.x, top.y, room_w, top.height);
    let side = Rect::new(top.x + room_w - 1, top.y, top.width - room_w + 1, top.height);

    let Some(unix) = st.unix() else {
        let inner = pane(f, &th, top, "pet", true);
        text(f, inner.x + 1, inner.y + 1, inner.width - 2, "waiting for the clock (it lives in real time)", Style::new().fg(th.dim));
        return;
    };

    // The room
    let title = if p.hatched() { format!("pet · day {}", p.age_days(unix) + 1) } else { "pet".into() };
    let r = pane(f, &th, room, &title, true);
    let floor = r.y + r.height - 1;
    for x in r.x..r.x + r.width {
        if let Some(c) = f.buffer_mut().cell_mut((x, floor)) {
            c.set_char('▔').set_fg(th.muted);
        }
    }
    // Poops sit on the right of the floor
    for k in 0..p.poops as u16 {
        text(f, r.x + r.width - 2 - k * 3, floor - 1, 1, "♨", Style::new().fg(th.orange));
    }
    // It walks back and forth (not while asleep, eating or being an egg)
    let lane = r.width.saturating_sub(BODY_W + 2 + 3 * p.poops as u16).max(1);
    let still = !p.hatched() || p.asleep || reaction.is_some();
    let pos = if still { lane / 2 } else {
        let k = (step % (2 * lane as u64)) as u16;
        if k < lane { k } else { 2 * lane - k }
    };
    let x = r.x + 1 + pos.min(lane);
    let y = floor - 4;
    let ink = Style::new().fg(if p.asleep { th.dim } else { th.fg });
    if !p.hatched() {
        // An egg that wobbles now and then
        let wob = if step % 6 == 0 { 1 } else { 0 };
        let ex = x + 1 + wob;
        let shell = Style::new().fg(th.fg);
        text(f, ex, y, 7, "  ╭─╮", shell);
        text(f, ex, y + 1, 7, " ╱ · ╲", shell);
        text(f, ex, y + 2, 7, "│ · · │", shell);
        text(f, ex, y + 3, 7, " ╲___╱", shell);
    } else {
        let (eyes, mouth) = match (reaction, p.mood()) {
            (_, Mood::Asleep) => ("˘ ˘", "ᴗ"),
            (Some(PetCmd::Feed), _) => ("◕ ◕", if step % 2 == 0 { "▽" } else { "o" }),
            (Some(PetCmd::Play | PetCmd::Pat), _) => ("^ ^", "▽"),
            (Some(PetCmd::Clean), _) => ("◕ ◕", "‿"),
            (_, Mood::Sad) => ("╥ ╥", "⌒"),
            (_, Mood::Happy) => if step % 9 == 0 { ("- -", "‿") } else { ("◕ ◕", "‿") },
            _ => if step % 9 == 0 { ("- -", "ᴗ") } else { ("• •", "ᴗ") },
        };
        let feet = if still || step % 2 == 0 { "╰┬───┬╯" } else { "╰┴───┴╯" };
        text(f, x + 1, y, 7, "╭─────╮", ink);
        text(f, x + 1, y + 1, 7, &format!("│ {eyes} │"), ink);
        text(f, x + 1, y + 2, 7, &format!("│  {mouth}  │"), ink);
        text(f, x + 1, y + 3, 7, feet, ink);
        // Reactions around it
        let side_x = x + BODY_W;
        match reaction {
            Some(PetCmd::Feed) => text(f, side_x, y + 2, 2, if step % 2 == 0 { "●" } else { "◒" }, Style::new().fg(th.orange)),
            Some(PetCmd::Play) => {
                let h = [3u16, 1, 0, 1][(step % 4) as usize];
                text(f, side_x, y + h, 1, "◦", Style::new().fg(th.accent).add_modifier(Modifier::BOLD));
            }
            Some(PetCmd::Pat) => {
                let rise = (step % 3) as u16;
                text(f, x + 2, y.saturating_sub(1 + rise), 1, "♥", Style::new().fg(th.red));
                text(f, x + 6, y.saturating_sub(2 + (rise + 1) % 3), 1, "♥", Style::new().fg(th.red));
            }
            Some(PetCmd::Clean) => {
                for (k, dx) in [0u16, 4, 8].iter().enumerate() {
                    if (step + k as u64) % 2 == 0 {
                        text(f, x + dx, y.saturating_sub(1), 1, "✧", Style::new().fg(th.yellow));
                    }
                }
            }
            _ => {}
        }
        if p.asleep {
            let z = ["z", "zZ", "zZz"][(step % 3) as usize];
            text(f, side_x, y.saturating_sub(1), 4, z, Style::new().fg(th.dim));
        }
    }
    let pat = if p.hatched() { PetCmd::Pat } else { PetCmd::Hatch };
    t.add(r, Action::Pet(pat));

    // Its needs
    let d = pane(f, &th, side, feeling(&p), false);
    if p.hatched() {
        let needs = [("food", p.food), ("fun", p.fun), ("energy", p.energy), ("clean", p.clean)];
        for (k, (name, v)) in needs.iter().enumerate() {
            let y = d.y + k as u16 * 2;
            text(f, d.x + 1, y, 7, name, Style::new().fg(th.dim));
            let bar = Rect::new(d.x + 8, y, d.width.saturating_sub(9), 1);
            crate::widgets::bar(f.buffer_mut(), bar, *v as f32 / 100.0, need_color(&th, *v), th.muted);
        }
    } else {
        text(f, d.x + 1, d.y + 1, d.width - 2, "tap it to hatch", Style::new().fg(th.dim));
    }

    // The buttons
    let by = body.y + body.height - buttons_h;
    if !p.hatched() {
        block_button(f, t, &th, Rect::new(body.x, by, body.width, buttons_h), "hatch", Action::Pet(PetCmd::Hatch), true);
        return;
    }
    let sleep = if p.asleep { "☀ wake" } else { "☾ sleep" };
    let items = [("● feed", PetCmd::Feed), ("◦ play", PetCmd::Play), ("✧ clean", PetCmd::Clean), (sleep, PetCmd::Sleep)];
    let buttons = items.map(|(label, cmd)| (label, Action::Pet(cmd), cmd == PetCmd::Sleep && p.asleep));
    let rects = button_row(f, t, &th, Rect::new(body.x, by, body.width, buttons_h), &buttons);
    // Things it won't do asleep look switched off
    for (r, (label, cmd)) in rects.iter().zip(items) {
        if p.asleep && !matches!(cmd, PetCmd::Sleep | PetCmd::Clean) {
            f.render_widget(Line::styled(label, Style::new().fg(th.muted)).centered(), Rect::new(r.x, r.y + 1, r.width, 1));
        }
    }
}
