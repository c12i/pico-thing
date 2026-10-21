//! Today's weather in detail, under the top strip.

use alloc::format;
use ratatui::Frame;
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};

use super::{art, deg, strip};
use crate::app::State;
use crate::widgets::{big_text, big_width, panel};

pub fn draw(st: &State, f: &mut Frame, area: Rect) {
    strip(st, f, area);
    let th = st.theme;
    let Some(w) = &st.weather else {
        let msg = if st.online() { "Fetching weather..." } else { "Weather unavailable (offline)" };
        f.render_widget(Line::styled(msg, Style::new().fg(th.dim)).centered(), Rect::new(area.x, area.y + 7, area.width, 1));
        return;
    };
    // Icon on the left, big temperature on the right
    art::draw(f.buffer_mut(), area.x + 3, area.y + 3, &art::weather(w.icon()), &th);
    let t = format!("{}", libm_round(w.temp));
    let tx = area.x + 20;
    big_text(f.buffer_mut(), tx, area.y + 3, &t, th.fg);
    let unit = if w.fahrenheit { "°F" } else { "°C" };
    f.render_widget(Span::styled(unit, Style::new().fg(th.dim)), Rect::new(tx + big_width(&t) + 1, area.y + 3, 2, 1));

    let line = Line::from(alloc::vec![
        Span::styled(w.label(), Style::new().fg(th.fg).add_modifier(Modifier::BOLD)),
        Span::styled(format!("   H {}  L {}   {}", deg(w.hi), deg(w.lo), w.city), Style::new().fg(th.dim)),
    ]);
    f.render_widget(line.centered(), Rect::new(area.x, area.y + 9, area.width, 1));

    // Details along the bottom
    let p = Rect::new(area.x + 1, area.y + area.height - 5, area.width - 2, 5);
    f.render_widget(panel(&th), p);
    let wind = if w.fahrenheit { "mph" } else { "km/h" };
    let cols = [
        ("FEELS", deg(w.feels)),
        ("HUMIDITY", format!("{}%", libm_round(w.humidity))),
        ("WIND", format!("{} {wind}", libm_round(w.wind))),
        ("RAIN", format!("{}%", libm_round(w.rain))),
    ];
    let inner = Rect::new(p.x + 1, p.y + 1, p.width - 2, 3);
    let cells = Layout::horizontal([Constraint::Fill(1); 4]).split(inner);
    for ((k, v), c) in cols.iter().zip(cells.iter()) {
        f.render_widget(Line::styled(*k, Style::new().fg(th.dim)).centered(), Rect::new(c.x, c.y, c.width, 1));
        f.render_widget(Line::styled(v.as_str(), Style::new().fg(th.fg)).centered(), Rect::new(c.x, c.y + 2, c.width, 1));
    }
}

fn libm_round(v: f32) -> i32 {
    super::round(v)
}
