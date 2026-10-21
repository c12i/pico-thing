//! Lights: the list on top (tap a row to select it, its chip to switch it), the
//! selected light's controls below.

use alloc::format;
use alloc::string::String;
use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};

use crate::app::{Action, LightEntry, State};
use crate::net::yeelight::{LightCmd, Transport};
use crate::targets::Targets;
use ratatui::widgets::Block;

use super::sonos::LIST_W;
use alloc::vec::Vec;
use crate::term::{block_button, button_row, clip, pane, row, spinner, text, thick_slider, top_bar};
use crate::theme::Theme;

const CT_PRESETS: [(&str, u32); 4] = [("warm", 2700), ("soft", 3500), ("day", 4500), ("cool", 6500)];

/// (mark, colour, status text)
fn status(st: &State, th: &Theme, l: &LightEntry) -> (String, Color, String) {
    if l.busy {
        return (format!("{}", spinner(st)), th.accent, String::new());
    }
    match &l.state {
        _ if l.transport.is_none() => ("✗".into(), th.muted, "needs token".into()),
        Some(Ok(s)) if s.power => ("●".into(), th.yellow, format!("on {}%", s.bright)),
        Some(Ok(_)) => ("○".into(), th.dim, "off".into()),
        Some(Err(_)) => ("✗".into(), th.red, "offline".into()),
        None => ("·".into(), th.muted, String::new()),
    }
}

fn is_on(l: &LightEntry) -> bool {
    matches!(&l.state, Some(Ok(s)) if s.power)
}

pub fn draw(st: &mut State, t: &mut Targets, f: &mut Frame, area: Rect) {
    let body = top_bar(f, t, st, area, "lights", Action::Menu);
    let th = st.theme;
    if st.lights.is_empty() {
        let inner = pane(f, &th, body, "lights", true);
        text(f, inner.x + 1, inner.y + 1, inner.width, "no lights in config.toml ([lights])", Style::new().fg(th.dim));
        return;
    }
    // Lights on the left (with "all off" at the bottom), the selected light's
    // controls on the right
    let list = Rect::new(body.x, body.y, LIST_W, body.height);
    let ctl = Rect::new(body.x + LIST_W - 1, body.y, body.width - LIST_W + 1, body.height);
    let inner = pane(f, &th, list, "lights", false);
    let sel = st.light_sel.min(st.lights.len() - 1);
    let w = inner.width as usize;
    for (i, l) in st.lights.iter().enumerate() {
        let y = inner.y + 4 * i as u16;
        if y + 6 >= inner.y + inner.height {
            break;
        }
        let (mark, color, s) = status(st, &th, l);
        let line = Line::from(alloc::vec![
            Span::styled(format!(" {mark} "), Style::new().fg(color)),
            Span::styled(clip(l.name, w - 3), Style::new().fg(th.fg).add_modifier(Modifier::BOLD)),
        ]);
        row(f, t, &th, Rect::new(inner.x, y, inner.width, 3), line, Action::SelectLight(i), i == sel);
        text(f, inner.x + 3, y + 1, inner.width - 3, &s, Style::new().fg(th.dim));
    }
    block_button(f, t, &th, Rect::new(inner.x, inner.y + inner.height - 2, inner.width, 2), "all off", Action::AllLightsOff, false);

    let l = &st.lights[sel];
    let bright = match &l.state {
        Some(Ok(s)) if s.power => s.bright,
        _ => 0,
    };
    let miio = matches!(l.transport, Some(Transport::Miio(_)));
    let (_, _, status_text) = status(st, &th, l);
    let title = if status_text.is_empty() { l.name.to_lowercase() } else { format!("{} · {status_text}", l.name.to_lowercase()) };
    let d = pane(f, &th, ctl, &title, true);
    let x = d.x + 1;
    let dw = d.width - 2;
    let state = match &l.state {
        Some(Ok(s)) => Some(s.clone()),
        _ => None,
    };
    if l.transport.is_none() {
        text(f, x, d.y + 1, dw, "add its token to config.toml", Style::new().fg(th.dim));
        text(f, x, d.y + 2, dw, "([miio_tokens]) - see TODO.md", Style::new().fg(th.dim));
        return;
    }
    // Power: one big button
    let on = is_on(l);
    block_button(f, t, &th, Rect::new(x, d.y, dw, 2), if on { "● on - tap to turn off" } else { "○ off - tap to turn on" }, Action::Light(sel, LightCmd::Power(!on)), on);
    // Brightness: a thick bar
    thick_slider(f, t, &th, Rect::new(x, d.y + 3, dw, 2), bright as f32 / 100.0, th.yellow, |p| Action::Light(sel, LightCmd::Bright(p)));
    // Colour temperature, identify, colours
    let mut y = d.y + 6;
    if state.as_ref().is_some_and(|s| s.ct > 0) || miio {
        let items: Vec<_> = CT_PRESETS.iter().map(|(name, k)| (*name, Action::Light(sel, LightCmd::Ct(*k)), false)).collect();
        button_row(f, t, &th, Rect::new(x, y, dw, 2), &items);
        y += 3;
    }
    let h = (d.y + d.height).saturating_sub(y).min(3);
    if h >= 2 {
        let iw = if miio { 10 } else { dw };
        block_button(f, t, &th, Rect::new(x, y, iw, h), "identify", Action::Light(sel, LightCmd::Identify), false);
        if miio {
            // Colour swatches side by side; their colours tell them apart, so no gaps
            let swatches: [(u32, Color); 5] =
                [(0xFF0000, th.red), (0xFF7A00, th.orange), (0x00FF40, th.green), (0x0040FF, th.blue), (0x8A00FF, th.purple)];
            let sx = x + iw + 1;
            let sw = (x + dw - sx) / 5;
            for (k, (rgb, c)) in swatches.iter().enumerate() {
                let r = Rect::new(sx + k as u16 * sw, y, if k == 4 { x + dw - sx - 4 * sw } else { sw }, h);
                f.render_widget(Block::new().style(Style::new().bg(*c)), r);
                t.add(r, Action::Light(sel, LightCmd::Rgb(*rgb)));
            }
        }
    }
}
