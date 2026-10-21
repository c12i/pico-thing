//! Touch targets. Screens register a rectangle and an action for everything tappable
//! while they draw; a tap runs the target under it, or the nearest one within a cell
//! (fingers are bigger than cells).

use alloc::vec::Vec;
use ratatui::layout::Rect;

use crate::app::Action;

/// Taps this many cells outside every target still snap to the nearest one.
const SNAP: u16 = 1;

/// What a touch landed on.
pub struct Hit {
    pub area: Rect,
    pub action: Action,
    /// What holding it does, if anything
    pub long: Option<Action>,
}

#[derive(Default)]
pub struct Targets {
    list: Vec<(Rect, Action, Option<Action>)>,
}

impl Targets {
    pub fn clear(&mut self) {
        self.list.clear();
    }

    pub fn add(&mut self, area: Rect, action: Action) {
        self.list.push((area, action, None));
    }

    /// A target that does something else when held (see `LONG_MS` in app.rs).
    pub fn add_long(&mut self, area: Rect, action: Action, long: Action) {
        self.list.push((area, action, Some(long)));
    }

    /// The action for a tap at (col, row). Later targets win, so things drawn on top
    /// (popups) take precedence.
    pub fn hit(&self, col: u16, row: u16) -> Option<Hit> {
        let mut best: Option<(u16, &Rect, &Action, &Option<Action>)> = None;
        for (r, a, long) in self.list.iter().rev() {
            let dx = gap(col, r.x, r.x + r.width);
            let dy = gap(row, r.y, r.y + r.height);
            let d = dx.max(dy);
            if d == 0 {
                return Some(Hit { area: *r, action: a.clone(), long: long.clone() });
            }
            if d <= SNAP && best.is_none_or(|(bd, ..)| d < bd) {
                best = Some((d, r, a, long));
            }
        }
        best.map(|(_, r, a, long)| Hit { area: *r, action: a.clone(), long: long.clone() })
    }
}

/// Distance from v to the half-open range [lo, hi), 0 inside it.
fn gap(v: u16, lo: u16, hi: u16) -> u16 {
    if v < lo {
        lo - v
    } else if v >= hi {
        v + 1 - hi
    } else {
        0
    }
}
