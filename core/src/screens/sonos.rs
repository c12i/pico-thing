//! Sonos: speakers on top (tap one to select it), the selected one's controls below,
//! and a Favorites list.

use alloc::format;
use alloc::string::String;
use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};

use crate::app::{Action, Screen, SpeakerEntry, State};
use crate::net::sonos::{SonosCmd, SpeakerState};
use crate::targets::Targets;
use crate::term::{SCROLLBAR_W, block_button, clip, pane, row, scrollbar, slots, spinner, text, thick_slider, top_bar};
use crate::theme::Theme;

fn state(sp: &SpeakerEntry) -> Option<&SpeakerState> {
    match &sp.state {
        Some(Ok(s)) => Some(s),
        _ => None,
    }
}

/// What's playing: (title, detail).
fn now_playing(sp: &SpeakerEntry) -> (String, String) {
    match &sp.state {
        None => ("…".into(), String::new()),
        Some(Err(_)) => ("offline".into(), String::new()),
        Some(Ok(s)) if s.transport == "STOPPED" && s.title.is_empty() => ("nothing playing".into(), String::new()),
        Some(Ok(s)) => {
            let title = if s.title.is_empty() { s.source.clone() } else { s.title.clone() };
            let mut sub = s.artist.clone();
            if !s.source.is_empty() && !s.title.is_empty() {
                sub = if sub.is_empty() { s.source.clone() } else { format!("{sub} · {}", s.source) };
            }
            (title, sub)
        }
    }
}

fn mark(st: &State, th: &Theme, sp: &SpeakerEntry) -> (String, Color) {
    if sp.busy {
        return (format!("{}", spinner(st)), th.accent);
    }
    match state(sp) {
        Some(s) if s.playing() => ("▶".into(), th.green),
        Some(_) => ("‖".into(), th.dim),
        None if matches!(sp.state, Some(Err(_))) => ("✗".into(), th.red),
        None => ("·".into(), th.muted),
    }
}

/// What holding speaker `i` does: leave its group if it's in one, or, if it's
/// not playing, join a speaker that is.
fn hold_action(st: &State, i: usize) -> Option<(&'static str, SonosCmd)> {
    let s = state(&st.speakers[i])?;
    if s.leader.is_some() {
        return Some(("hold to leave", SonosCmd::Leave));
    }
    if s.playing() {
        return None;
    }
    let leader = st
        .speakers
        .iter()
        .enumerate()
        .find(|(j, o)| *j != i && state(o).is_some_and(|o| o.playing() && o.leader.is_none()))?;
    Some(("hold to join", SonosCmd::Join(leader.1.ip)))
}

/// Width of the left-hand list pane on the Sonos and Lights screens.
pub const LIST_W: u16 = 17;

pub fn draw(st: &mut State, t: &mut Targets, f: &mut Frame, area: Rect) {
    let body = top_bar(f, t, st, area, "sonos", Action::Menu);
    let th = st.theme;
    if st.speakers.is_empty() {
        let inner = pane(f, &th, body, "sonos", true);
        text(f, inner.x + 1, inner.y + 1, inner.width, "no speakers in config.toml ([[sonos]])", Style::new().fg(th.dim));
        return;
    }
    // Speakers on the left, the player on the right
    let list = Rect::new(body.x, body.y, LIST_W, body.height);
    let player = Rect::new(body.x + LIST_W - 1, body.y, body.width - LIST_W + 1, body.height);
    let inner = pane(f, &th, list, "speakers", false);
    let sel = st.speaker_sel.min(st.speakers.len() - 1);
    let w = inner.width as usize;
    for (i, sp) in st.speakers.iter().enumerate() {
        let y = inner.y + 4 * i as u16;
        if y + 5 >= inner.y + inner.height {
            break;
        }
        let (m, c) = mark(st, &th, sp);
        let (title, _) = now_playing(sp);
        let line = Line::from(alloc::vec![
            Span::styled(format!(" {m} "), Style::new().fg(c)),
            Span::styled(clip(sp.name, w - 3), Style::new().fg(th.fg).add_modifier(Modifier::BOLD)),
        ]);
        let r = Rect::new(inner.x, y, inner.width, 3);
        row(f, t, &th, r, line, Action::SelectSpeaker(i), i == sel);
        text(f, inner.x + 3, y + 1, inner.width - 3, &clip(&title, w - 3), Style::new().fg(th.dim));
        // Holding a speaker groups it with the one playing, or takes it out of its group
        if let Some((hint, cmd)) = hold_action(st, i) {
            t.add_long(r, Action::SelectSpeaker(i), Action::Speaker(i, cmd));
            text(f, inner.x + 3, y + 2, inner.width - 3, hint, Style::new().fg(th.muted));
        }
    }
    let fav = Rect::new(inner.x, inner.y + inner.height - 2, inner.width, 2);
    block_button(f, t, &th, fav, "★ favorites", Action::Open(Screen::Favorites(st.speaker_sel.min(st.speakers.len() - 1))), false);

    // The player: panes stacked down the right, sharing borders
    let sp = &st.speakers[sel];
    let name = sp.name.to_lowercase();
    let Some(s) = state(sp).cloned() else {
        let d = pane(f, &th, player, &name, true);
        let msg = if matches!(sp.state, Some(Err(_))) { "offline" } else { "checking…" };
        text(f, d.x + 1, d.y + 1, d.width - 2, msg, Style::new().fg(th.dim));
        return;
    };
    // Now playing (4 rows), playback (3-row buttons), volume (3-row controls)
    let np = Rect::new(player.x, player.y, player.width, 6);
    let pb = Rect::new(player.x, np.y + np.height - 1, player.width, 5);
    let vol = Rect::new(player.x, pb.y + pb.height - 1, player.width, player.y + player.height - (pb.y + pb.height - 1));

    // Now playing: title, artist, album, and what it's playing from
    let d = pane(f, &th, np, &name, true);
    let (x, dw) = (d.x + 1, d.width - 2);
    let w = dw as usize;
    let (title, sub) = now_playing(sp);
    text(f, x, d.y, dw, &clip(&title, w), Style::new().fg(th.fg).add_modifier(Modifier::BOLD));
    text(f, x, d.y + 1, dw, &clip(&sub, w), Style::new().fg(th.dim));
    if !s.album.is_empty() {
        text(f, x, d.y + 2, dw, &clip(&s.album, w), Style::new().fg(th.dim));
    }
    let from = match &s.leader {
        Some(leader) => {
            let with = st.speakers.iter().find(|o| o.ip == *leader).map(|o| o.name).unwrap_or("another speaker");
            format!("grouped with {with}")
        }
        None if s.from.is_empty() => String::new(),
        None => format!("♫ {}", s.from),
    };
    text(f, x, d.y + 3, dw, &clip(&from, w), Style::new().fg(th.purple));

    // Playback: three big buttons of equal width, centred
    // (these two rows use the pane's full width, so the blocks divide it evenly)
    let d = pane(f, &th, pb, "playback", false);
    let (x, dw) = (d.x, d.width);
    let playing = s.playing();
    let skip = s.can_skip();
    let slot = slots(Rect::new(x, d.y, dw, 3), 3);
    if skip {
        block_button(f, t, &th, slot[0], "◀◀", Action::Speaker(sel, SonosCmd::Previous), false);
        block_button(f, t, &th, slot[2], "▶▶", Action::Speaker(sel, SonosCmd::Next), false);
    }
    if skip || !playing {
        let cmd = if playing { SonosCmd::Pause } else { SonosCmd::Play };
        block_button(f, t, &th, slot[1], if playing { "‖" } else { "▶" }, Action::Speaker(sel, cmd), true);
    } else {
        text(f, x, d.y + 1, dw, "tv sound: no pause or skip", Style::new().fg(th.muted));
    }

    // Volume: [−] [bar] [+] [♪×], the three buttons the same size
    let d = pane(f, &th, vol, &format!("vol {}%", s.volume), false);
    let (x, dw) = (d.x, d.width);
    let h = d.height.min(3);
    let bw = 5;
    let mute_x = x + dw - bw;
    let plus_x = mute_x - 1 - bw;
    let down = Action::Speaker(sel, SonosCmd::Volume(s.volume.saturating_sub(5)));
    let up = Action::Speaker(sel, SonosCmd::Volume((s.volume + 5).min(100)));
    block_button(f, t, &th, Rect::new(x, d.y, bw, h), "−", down, false);
    thick_slider(f, t, &th, Rect::new(x + bw + 1, d.y, plus_x - x - bw - 2, h), s.volume as f32 / 100.0, th.green, |p| {
        Action::Speaker(sel, SonosCmd::Volume(p))
    });
    block_button(f, t, &th, Rect::new(plus_x, d.y, bw, h), "+", up, false);
    block_button(f, t, &th, Rect::new(mute_x, d.y, bw, h), "♪×", Action::Speaker(sel, SonosCmd::Mute(!s.muted)), s.muted);
}

pub fn draw_favorites(st: &mut State, t: &mut Targets, f: &mut Frame, area: Rect, i: usize) {
    let body = top_bar(f, t, st, area, "sonos/favorites", Action::Open(Screen::Sonos));
    let th = st.theme;
    let name = st.speakers.get(i).map(|s| s.name.to_lowercase()).unwrap_or_default();
    // The list, and a scrollbar along the right edge of the screen
    let bar = Rect::new(body.x + body.width - SCROLLBAR_W, body.y, SCROLLBAR_W, body.height);
    let list = Rect::new(body.x, body.y, body.width - SCROLLBAR_W, body.height);
    let inner = pane(f, &th, list, &format!("favorites → {name}"), true);
    let favs = match &st.favorites {
        Some(Ok(v)) => v.clone(),
        Some(Err(_)) => {
            text(f, inner.x + 1, inner.y + 1, inner.width, "couldn't load favorites", Style::new().fg(th.red));
            return;
        }
        None => {
            text(f, inner.x + 1, inner.y + 1, inner.width, &format!("{} loading favorites…", spinner(st)), Style::new().fg(th.dim));
            return;
        }
    };
    // One line per favorite with a gap between, so each is easy to hit
    let visible = (inner.height as usize).div_ceil(2);
    scrollbar(f, t, st, bar, favs.len(), visible);
    let w = inner.width as usize;
    for (k, (_, title, mix)) in favs.iter().enumerate().skip(st.scroll).take(visible) {
        let y = inner.y + ((k - st.scroll) * 2) as u16;
        let kind = if *mix { "mix" } else { "song" };
        let line = Line::from(alloc::vec![
            Span::styled(if *mix { " ♫ " } else { " ♪ " }, Style::new().fg(if *mix { th.purple } else { th.green })),
            Span::styled(clip(title, w.saturating_sub(9)), Style::new().fg(th.fg)),
        ]);
        let r = Rect::new(inner.x, y, inner.width, 2);
        row(f, t, &th, r, line, Action::PlayFavorite(i, k), false);
        f.render_widget(Line::styled(format!("{kind} "), Style::new().fg(th.muted)).right_aligned(), Rect::new(r.x, r.y, r.width, 1));
    }
}
