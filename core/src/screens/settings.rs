//! Display settings on the left, network and device details on the right.

use alloc::format;
use alloc::string::String;
use ratatui::Frame;
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::Style;

use crate::app::{Action, Screen, State};
use crate::targets::Targets;
use crate::term::{block_button, pane, text, thick_slider, top_bar};

/// Never let the backlight go fully dark.
const MIN_BRIGHTNESS: u8 = 10;

pub fn draw(st: &mut State, t: &mut Targets, f: &mut Frame, area: Rect) {
    let body = top_bar(f, t, st, area, "settings", Action::Menu);
    let th = st.theme;
    let [left, right] = Layout::horizontal([Constraint::Length(23), Constraint::Fill(1)]).spacing(-1).areas(body);

    let d = pane(f, &th, left, "display", true);
    let dim = Style::new().fg(th.dim);
    let (x, w) = (d.x + 1, d.width - 2);
    let half = (w - 1) / 2;
    text(f, x, d.y, w, "theme", dim);
    block_button(f, t, &th, Rect::new(x, d.y + 1, half, 2), "dark", Action::SetDark(true), st.settings.dark);
    block_button(f, t, &th, Rect::new(x + half + 1, d.y + 1, w - half - 1, 2), "light", Action::SetDark(false), !st.settings.dark);
    text(f, x, d.y + 4, w, &format!("brightness {}%", st.settings.brightness), dim);
    thick_slider(f, t, &th, Rect::new(x, d.y + 5, w, 2), st.settings.brightness as f32 / 100.0, th.accent, |p| {
        Action::Brightness(p.max(MIN_BRIGHTNESS))
    });
    text(f, x, d.y + 8, w, "touch", dim);
    block_button(f, t, &th, Rect::new(x, d.y + 9, w, 2), "calibrate", Action::Open(Screen::Calibrate), false);

    let n = pane(f, &th, right, "network", false);
    let x = n.x + 1;
    let w = n.width - 2;
    let fg = Style::new().fg(th.fg);
    let ip = match st.status.ip {
        Some([a, b, c, e]) => format!("{a}.{b}.{c}.{e}"),
        None => String::from("offline"),
    };
    text(f, x, n.y, w, &format!("wifi     {ip}"), fg);
    let wx = match (&st.weather, st.weather_at) {
        (Some(wx), Some(at)) => format!("weather  {} {:02}:{:02}", wx.city, at.hour, at.minute),
        _ => String::from("weather  -"),
    };
    text(f, x, n.y + 1, w, &wx, fg);
    let time = if st.clock.is_some() { "time     synced" } else { "time     not set" };
    text(f, x, n.y + 2, w, time, fg);
    let s = &st.status;
    text(f, x, n.y + 3, w, &format!("memory   {}K/{}K free", s.heap_free_kb, s.heap_total_kb), Style::new().fg(th.dim));
    text(f, x, n.y + 4, w, &format!("uptime   {} min", st.uptime_ms / 60_000), Style::new().fg(th.dim));
    // (the top bar's spinner shows while these run)
    block_button(f, t, &th, Rect::new(x, n.y + 6, w, 2), "↻ refresh weather", Action::RefreshWeather, false);
    block_button(f, t, &th, Rect::new(x, n.y + 9, w, 2), "↻ sync time", Action::SyncTime, false);
}
