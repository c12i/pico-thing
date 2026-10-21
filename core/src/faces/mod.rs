//! Home screen faces. Tapping a face opens the menu.

mod analog;
pub mod art;
pub mod bonsai;
pub mod bonsai_tree;
pub mod bot;
mod digital;
pub mod hourglass;
mod pet;
pub mod productivity;
mod system;
mod weather;

use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use serde::{Deserialize, Serialize};

use crate::app::{Action, State};
use crate::targets::Targets;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum FaceKind {
    Digital,
    Analog,
    Weather,
    Productivity,
    Bonsai,
    System,
    Hourglass,
    Pet,
}

impl FaceKind {
    pub const ALL: [FaceKind; 8] = [
        FaceKind::Digital,
        FaceKind::Analog,
        FaceKind::Weather,
        FaceKind::Productivity,
        FaceKind::Bonsai,
        FaceKind::System,
        FaceKind::Hourglass,
        FaceKind::Pet,
    ];

    pub fn name(self) -> &'static str {
        match self {
            FaceKind::Digital => "Digital",
            FaceKind::Analog => "Analog",
            FaceKind::Weather => "Weather",
            FaceKind::Productivity => "Productivity",
            FaceKind::Bonsai => "Bonsai",
            FaceKind::System => "System",
            FaceKind::Hourglass => "Hourglass",
            FaceKind::Pet => "Pet",
        }
    }

    /// Faces with their own buttons: their top strip opens the menu (the rest of
    /// the screen is taken).
    fn interactive(self) -> bool {
        matches!(self, FaceKind::Hourglass | FaceKind::Pet)
    }
}

/// State the faces keep between frames.
pub struct Faces {
    pub bonsai: bonsai::Garden,
    pub hourglass: hourglass::Glass,
    /// The pet's last reaction: (what, when)
    pub pet_reaction: Option<(crate::pet::PetCmd, u64)>,
    marquee_step: u64,
    anim_step: u64,
}

impl Faces {
    pub fn new(seed: u32) -> Self {
        Self {
            bonsai: bonsai::Garden::new(seed),
            hourglass: hourglass::Glass::default(),
            pet_reaction: None,
            marquee_step: 0,
            anim_step: 0,
        }
    }

}

/// Advance the current face's animations; true if it needs redrawing.
pub fn tick(st: &mut State, uptime_ms: u64) -> bool {
    match st.settings.face {
        FaceKind::Bonsai => st.faces.bonsai.tick(uptime_ms),
        FaceKind::Hourglass => {
            if st.faces.hourglass.tick(uptime_ms) {
                st.request(crate::app::Request::FlashBacklight);
            }
            animate(st, uptime_ms, hourglass::STEP_MS, st.faces.hourglass.running(uptime_ms) || st.pomodoro.active())
        }
        FaceKind::Pet => animate(st, uptime_ms, pet::STEP_MS, true),
        FaceKind::Productivity if productivity::animating(st) => {
            let step = uptime_ms / productivity::STEP_MS;
            let changed = step != st.faces.marquee_step;
            st.faces.marquee_step = step;
            changed
        }
        _ => false,
    }
}

/// True every `step_ms` while `on`: time for the next animation frame.
fn animate(st: &mut State, uptime_ms: u64, step_ms: u64, on: bool) -> bool {
    let step = uptime_ms / step_ms;
    let changed = on && step != st.faces.anim_step;
    st.faces.anim_step = step;
    changed
}

pub fn draw(st: &mut State, t: &mut Targets, f: &mut Frame, area: Rect) {
    t.add(area, Action::Menu); // anywhere on a face opens the menu
    let face = st.settings.face;
    if face.interactive() {
        // Their buttons are added over this as they draw
    } else if matches!(face, FaceKind::Weather | FaceKind::Productivity | FaceKind::Bonsai | FaceKind::System) {
        // ...except the strip at the top, which opens the timer
        t.add(Rect::new(area.x, area.y, area.width, 2), Action::Open(crate::app::Screen::Pomodoro));
    }
    match st.settings.face {
        FaceKind::Digital => digital::draw(st, f, area),
        FaceKind::Analog => analog::draw(st, f, area),
        FaceKind::Weather => weather::draw(st, f, area),
        FaceKind::Productivity => productivity::draw(st, f, area),
        FaceKind::Bonsai => bonsai::draw(st, f, area),
        FaceKind::System => system::draw(st, f, area),
        FaceKind::Hourglass => hourglass::draw(st, t, f, area),
        FaceKind::Pet => pet::draw(st, t, f, area),
    }
}

/// Round half away from zero (no float runtime needed).
pub fn round(v: f32) -> i32 {
    if v < 0.0 { (v - 0.5) as i32 } else { (v + 0.5) as i32 }
}

/// A temperature like "29°".
pub fn deg(v: f32) -> alloc::string::String {
    alloc::format!("{}°", round(v))
}

/// The two-row strip at the top of the text faces: time and date (and the weather),
/// then a rule.
pub fn strip(st: &State, f: &mut Frame, area: Rect) {
    let th = &st.theme;
    let (time, date) = match st.now() {
        Some(n) => (
            alloc::format!("{:02}:{:02}", n.hour, n.minute),
            alloc::format!("{} {} {}", n.day_name(), n.day, n.month_name()),
        ),
        None => ("--:--".into(), alloc::string::String::new()),
    };
    let line = Line::from(alloc::vec![
        Span::raw(" "),
        Span::styled(time, Style::new().fg(th.fg).add_modifier(Modifier::BOLD)),
        Span::raw("  "),
        Span::styled(date, Style::new().fg(th.dim)),
    ]);
    f.render_widget(line, Rect::new(area.x, area.y, area.width, 1));
    let p = &st.pomodoro;
    let rule = Rect::new(area.x + 1, area.y + 1, area.width - 2, 1);
    if p.active() {
        // The timer replaces the weather while it's on
        use crate::pomodoro::{Phase, PomoState};
        let color = if p.state == PomoState::Done { th.orange } else { crate::screens::pomodoro::phase_color(th, p.phase) };
        let name = |ph: Option<Phase>| if ph == Some(Phase::Focus) { "FOCUS" } else { "BREAK" };
        let left = p.remaining(st.uptime_ms);
        let text = match p.state {
            PomoState::Done => alloc::format!("{} done ", name(p.finished)),
            _ => alloc::format!(
                "{} {:02}:{:02}{} ",
                name(Some(p.phase)),
                left / 60,
                left % 60,
                if p.state == PomoState::Paused { " II" } else { "" }
            ),
        };
        f.render_widget(
            Line::styled(text, Style::new().fg(color).add_modifier(Modifier::BOLD)).right_aligned(),
            Rect::new(area.x, area.y, area.width, 1),
        );
        crate::widgets::bar(f.buffer_mut(), rule, p.progress(st.uptime_ms), color, th.muted);
        return;
    }
    crate::widgets::bar(f.buffer_mut(), rule, 0.0, th.accent, th.muted);
    if let Some(w) = &st.weather {
        let right = Line::from(alloc::vec![
            Span::styled(w.label(), Style::new().fg(th.dim)),
            Span::styled(alloc::format!("  {} ", deg(w.temp)), Style::new().fg(th.fg)),
        ]);
        f.render_widget(right.right_aligned(), Rect::new(area.x, area.y, area.width, 1));
    }
}
