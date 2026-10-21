//! The Mac's vitals, btop-style: CPU, memory, network and the machine itself in four
//! panes, with sparklines of the last minute and a half (one column per sample).
//! The companion on the Mac samples them; the app polls it while this face is up.

use alloc::format;
use alloc::string::String;
use ratatui::Frame;
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Color, Style};

use crate::app::State;
use crate::net::companion::{self, MacStats};
use crate::term::{pane, spinner, text};

const LEVELS: [char; 8] = ['▁', '▂', '▃', '▄', '▅', '▆', '▇', '█'];

/// A sparkline filling `area`: the newest sample at the right edge, each column
/// `area.height` cells tall at eight levels per cell.
fn spark(buf: &mut Buffer, area: Rect, values: &[u32], max: u32, color: Color, track: Color) {
    let w = area.width as usize;
    let h = area.height as u32;
    let start = values.len().saturating_sub(w);
    let shown = &values[start..];
    let pad = w - shown.len();
    for col in 0..w {
        let x = area.x + col as u16;
        let v = if col < pad { None } else { Some(shown[col - pad]) };
        // Eighths of a cell, at least one when there's any value at all
        let eighths = v.map(|v| {
            let e = (v.min(max) * h * 8 + max / 2) / max.max(1);
            if v > 0 { e.max(1) } else { 0 }
        });
        for row in 0..h {
            let Some(cell) = buf.cell_mut((x, area.y + area.height - 1 - row as u16)) else { continue };
            match eighths {
                Some(e) if e >= (row + 1) * 8 => {
                    cell.set_char('█').set_fg(color);
                }
                Some(e) if e > row * 8 => {
                    cell.set_char(LEVELS[(e - row * 8 - 1) as usize]).set_fg(color);
                }
                _ if row == 0 => {
                    cell.set_char('▁').set_fg(track);
                }
                _ => {}
            }
        }
    }
}

/// KB/s as "820K" or "1.4M".
fn rate(k: u32) -> String {
    if k >= 1000 { format!("{}.{}M", k / 1024, (k % 1024) * 10 / 1024) } else { format!("{k}K") }
}

fn uptime(secs: u64) -> String {
    let (d, h, m) = (secs / 86_400, secs / 3600 % 24, secs / 60 % 60);
    if d > 0 { format!("{d}d {h}h") } else { format!("{h}h {m}m") }
}

fn f1(v: f32) -> String {
    let t = (v * 10.0 + 0.5) as u32;
    format!("{}.{}", t / 10, t % 10)
}

pub fn draw(st: &mut State, f: &mut Frame, area: Rect) {
    super::strip(st, f, area);
    let th = st.theme;
    let body = Rect::new(area.x, area.y + 2, area.width, area.height - 2);
    let s: MacStats = match &st.mac_stats {
        Some(Ok(s)) if !s.cpu.is_empty() => s.clone(),
        other => {
            let inner = pane(f, &th, body, "mac", true);
            let msg = if !companion::configured() {
                String::from("set companion in config.toml to see your mac")
            } else if matches!(other, Some(Err(_))) {
                String::from("can't reach the companion on your mac")
            } else {
                format!("{} sampling your mac…", spinner(st))
            };
            text(f, inner.x + 1, inner.y + 1, inner.width - 2, &msg, Style::new().fg(th.dim));
            return;
        }
    };
    // Two by two, sharing borders
    let (w1, h1) = (body.width / 2, 7);
    let tl = Rect::new(body.x, body.y, w1 + 1, h1);
    let tr = Rect::new(body.x + w1, body.y, body.width - w1, h1);
    let bl = Rect::new(body.x, body.y + h1 - 1, w1 + 1, body.height - h1 + 1);
    let br = Rect::new(body.x + w1, body.y + h1 - 1, body.width - w1, body.height - h1 + 1);
    let dim = Style::new().fg(th.dim);
    let widen = |v: &[u8]| v.iter().map(|&x| x as u32).collect::<alloc::vec::Vec<u32>>();

    let cpu = s.cpu.last().copied().unwrap_or(0);
    let d = pane(f, &th, tl, &format!("cpu {cpu}% · {} cores", s.cores), true);
    spark(f.buffer_mut(), Rect::new(d.x, d.y, d.width, d.height - 1), &widen(&s.cpu), 100, th.accent, th.muted);
    let load = format!("load {} {} {}", f1(s.load[0]), f1(s.load[1]), f1(s.load[2]));
    text(f, d.x, d.y + d.height - 1, d.width, &load, dim);

    let mem = s.mem.last().copied().unwrap_or(0);
    let d = pane(f, &th, tr, &format!("mem {}/{}G", f1(s.mem_used), s.mem_total as u32), false);
    spark(f.buffer_mut(), Rect::new(d.x, d.y, d.width, d.height - 1), &widen(&s.mem), 100, th.purple, th.muted);
    text(f, d.x, d.y + d.height - 1, d.width, &format!("{mem}% · disk {}/{}G", s.disk_used, s.disk_total), dim);

    let (rx, tx) = (s.rx.last().copied().unwrap_or(0), s.tx.last().copied().unwrap_or(0));
    let d = pane(f, &th, bl, &format!("net ↓{} ↑{}", rate(rx), rate(tx)), false);
    // Down and up share a scale, so they compare at a glance
    let peak = s.rx.iter().chain(s.tx.iter()).copied().max().unwrap_or(0).max(16);
    let half = d.height / 2;
    spark(f.buffer_mut(), Rect::new(d.x, d.y, d.width, half), &s.rx, peak, th.green, th.muted);
    spark(f.buffer_mut(), Rect::new(d.x, d.y + half, d.width, d.height - half), &s.tx, peak, th.orange, th.muted);

    let d = pane(f, &th, br, &s.host.to_lowercase(), false);
    let fg = Style::new().fg(th.fg);
    text(f, d.x + 1, d.y, d.width - 2, &format!("up      {}", uptime(s.uptime)), fg);
    if let Some(b) = s.battery {
        let plug = if s.charging { " ⚡" } else { "" };
        text(f, d.x + 1, d.y + 1, d.width - 2, &format!("battery {b}%{plug}"), fg);
        let bar = Rect::new(d.x + 1, d.y + 2, d.width - 2, 1);
        let color = if b <= 20 && !s.charging { th.red } else { th.green };
        crate::widgets::bar(f.buffer_mut(), bar, b as f32 / 100.0, color, th.muted);
    }
    text(f, d.x + 1, d.y + 4, d.width - 2, &format!("peak    {}/s", rate(peak)), dim);
}
