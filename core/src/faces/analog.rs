//! An analog clock drawn in Braille dots (2x4 per cell; with 10x20px cells the dots
//! come out square), with the date and weather on the right.

use alloc::format;
use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use ratatui::symbols::Marker;
use ratatui::text::Line;
use ratatui::widgets::canvas::{Canvas, Circle, Line as CLine, Points};

use super::{art, deg};
use crate::app::State;
use crate::widgets::{big_text, big_width};

/// sin/cos for the 60 minute positions, so the clock needs no float trig at runtime.
fn unit(i: u32) -> (f64, f64) {
    // sin(i * 6°) for i in 0..=15; the other quadrants by symmetry
    const S: [f64; 16] = [
        0.0, 0.104528, 0.207912, 0.309017, 0.406737, 0.5, 0.587785, 0.669131, 0.743145, 0.809017, 0.866025,
        0.913545, 0.951057, 0.978148, 0.994522, 1.0,
    ];
    let i = i % 60;
    let s = |k: u32| -> f64 {
        match k {
            0..=15 => S[k as usize],
            16..=30 => S[(30 - k) as usize],
            31..=45 => -S[(k - 30) as usize],
            _ => -S[(60 - k) as usize],
        }
    };
    (s(i), s((i + 15) % 60)) // (sin, cos)
}

pub fn draw(st: &State, f: &mut Frame, area: Rect) {
    let th = st.theme;
    let now = st.now();
    let dial = Rect::new(area.x, area.y, 32, area.height);
    let canvas = Canvas::default()
        .marker(Marker::Braille)
        .x_bounds([-1.0, 1.0])
        .y_bounds([-1.0, 1.0])
        .paint(|ctx| {
            ctx.draw(&Circle { x: 0.0, y: 0.0, radius: 0.97, color: th.muted });
            for i in (0..60).step_by(5) {
                let (s, c) = unit(i);
                let (r0, r1) = if i % 15 == 0 { (0.72, 0.9) } else { (0.8, 0.9) };
                let color = if i % 15 == 0 { th.fg } else { th.dim };
                ctx.draw(&CLine { x1: s * r0, y1: c * r0, x2: s * r1, y2: c * r1, color });
            }
            ctx.layer();
            if let Some(n) = now {
                let (m, h) = (n.minute as u32, n.hour as u32);
                let hand = |pos: u32, len: f64| {
                    let (s, c) = unit(pos);
                    CLine { x1: 0.0, y1: 0.0, x2: s * len, y2: c * len, color: th.fg }
                };
                ctx.draw(&hand((h % 12) * 5 + m / 12, 0.5));
                ctx.draw(&hand(m, 0.75));
                ctx.layer();
                let (s, c) = unit(n.second as u32);
                ctx.draw(&CLine { x1: -s * 0.15, y1: -c * 0.15, x2: s * 0.85, y2: c * 0.85, color: th.accent2 });
            }
            ctx.draw(&Points { coords: &[(0.0, 0.0)], color: th.accent2 });
        });
    f.render_widget(canvas, dial);

    // Date and weather in the right-hand column
    let x = area.x + 32;
    let w = area.width - 32;
    let center = |text: &str| Rect::new(x, 0, w, 1).x + (w.saturating_sub(text.chars().count() as u16)) / 2;
    let Some(n) = now else { return };
    let day = format!("{}", n.day);
    f.render_widget(Line::styled(n.day_name(), Style::new().fg(th.dim)).centered(), Rect::new(x, area.y, w, 1));
    big_text(f.buffer_mut(), x + (w - big_width(&day)) / 2, area.y + 1, &day, th.fg);
    f.render_widget(Line::styled(n.month_name(), Style::new().fg(th.dim)).centered(), Rect::new(x, area.y + 7, w, 1));
    if let Some(wx) = &st.weather {
        art::draw(f.buffer_mut(), center("             "), area.y + 9, &art::weather(wx.icon()), &th);
        f.render_widget(
            Line::styled(wx.label(), Style::new().fg(th.fg).add_modifier(Modifier::BOLD)).centered(),
            Rect::new(x, area.y + 14, w, 1),
        );
        let t = format!("{}  {}/{}", deg(wx.temp), deg(wx.hi), deg(wx.lo));
        f.render_widget(Line::styled(t, Style::new().fg(th.dim)).centered(), Rect::new(x, area.y + 15, w, 1));
    }
}
