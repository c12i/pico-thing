//! Big tty-clock style time with a seconds bar and the date.

use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};

use crate::app::State;
use super::deg;
use crate::widgets::{DIGIT_H, bar, big_text, big_width};

pub fn draw(st: &State, f: &mut Frame, area: Rect) {
    let th = st.theme;
    let now = st.now();
    let time = match now {
        Some(n) => alloc::format!("{:02}:{:02}", n.hour, n.minute),
        None => "--:--".into(),
    };
    let w = big_width(&time);
    let x = area.x + (area.width - w) / 2;
    big_text(f.buffer_mut(), x, area.y + 1, &time, th.fg);
    let sec = now.map(|n| n.second as f32 / 59.0).unwrap_or(0.0);
    bar(f.buffer_mut(), Rect::new(x, area.y + 1 + DIGIT_H + 1, w, 1), sec, th.accent, th.muted);
    if let Some(n) = now {
        let date = alloc::format!("{} {} {}", n.day_name(), n.day, n.month_name());
        f.render_widget(
            Line::styled(date, Style::new().fg(th.accent).add_modifier(Modifier::BOLD)).centered(),
            Rect::new(area.x, area.y + DIGIT_H + 4, area.width, 1),
        );
    }
    // Weather along the bottom: icon on the left, details beside it
    let wx = Rect::new(area.x, area.y + area.height - 6, area.width, 6);
    f.render_widget(
        ratatui::widgets::Block::new().borders(ratatui::widgets::Borders::TOP).border_style(Style::new().fg(th.muted)),
        wx,
    );
    let Some(w) = &st.weather else {
        let msg = if st.online() { "Fetching weather..." } else { "Weather unavailable" };
        f.render_widget(Line::styled(msg, Style::new().fg(th.dim)).centered(), Rect::new(wx.x, wx.y + 3, wx.width, 1));
        return;
    };
    super::art::draw(f.buffer_mut(), wx.x + 2, wx.y + 1, &super::art::weather(w.icon()), &th);
    let x = wx.x + 18;
    let tw = wx.width - 19;
    let rows = [
        Line::from(alloc::vec![
            Span::styled(deg(w.temp), Style::new().fg(th.fg).add_modifier(Modifier::BOLD)),
            Span::styled(alloc::format!("  {}", w.label()), Style::new().fg(th.fg)),
        ]),
        Line::styled(alloc::format!("H {}  L {}  feels {}", deg(w.hi), deg(w.lo), deg(w.feels)), Style::new().fg(th.dim)),
        Line::styled(alloc::format!("rain {}%  wind {}", super::round(w.rain), super::round(w.wind)), Style::new().fg(th.dim)),
        Line::styled(w.city.as_str(), Style::new().fg(th.dim)),
    ];
    for (i, l) in rows.into_iter().enumerate() {
        f.render_widget(l, Rect::new(x, wx.y + 1 + i as u16, tw, 1));
    }
}
