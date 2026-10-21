//! Pomodoro timer state. It lives in the app, not a screen, so it keeps running
//! and the faces can show it.

/// (focus, short break, long break) minutes
pub const PRESETS: [(u32, u32, u32); 3] = [(25, 5, 15), (50, 10, 20), (15, 3, 10)];
pub const LONG_EVERY: u32 = 4;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Phase {
    Focus,
    Short,
    Long,
}

impl Phase {
    pub fn label(self) -> &'static str {
        match self {
            Phase::Focus => "Focus",
            Phase::Short => "Short break",
            Phase::Long => "Long break",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PomoState {
    Idle,
    Running,
    Paused,
    /// A phase just finished and is waiting for you
    Done,
}

pub struct Pomodoro {
    pub preset: usize,
    pub phase: Phase,
    pub state: PomoState,
    /// The phase that just completed, while state is Done
    pub finished: Option<Phase>,
    /// Focus sessions completed towards the next long break
    pub cycle: u32,
    /// (day of month, focus sessions done that day)
    pub today: (u8, u32),
    end_ms: u64,
    left_s: u32,
}

impl Pomodoro {
    pub fn new(preset: usize, today: (u8, u32)) -> Self {
        let mut p = Self {
            preset: preset % PRESETS.len(),
            phase: Phase::Focus,
            state: PomoState::Idle,
            finished: None,
            cycle: 0,
            today,
            end_ms: 0,
            left_s: 0,
        };
        p.left_s = p.duration();
        p
    }

    pub fn active(&self) -> bool {
        self.state != PomoState::Idle
    }

    pub fn duration(&self) -> u32 {
        let (f, s, l) = PRESETS[self.preset];
        60 * match self.phase {
            Phase::Focus => f,
            Phase::Short => s,
            Phase::Long => l,
        }
    }

    pub fn remaining(&self, now_ms: u64) -> u32 {
        if self.state == PomoState::Running {
            (self.end_ms.saturating_sub(now_ms) / 1000) as u32
        } else {
            self.left_s
        }
    }

    pub fn progress(&self, now_ms: u64) -> f32 {
        1.0 - self.remaining(now_ms) as f32 / self.duration() as f32
    }

    pub fn start(&mut self, now_ms: u64) {
        if self.state != PomoState::Running {
            self.state = PomoState::Running;
            self.finished = None;
            self.end_ms = now_ms + self.left_s as u64 * 1000;
        }
    }

    pub fn pause(&mut self, now_ms: u64) {
        if self.state == PomoState::Running {
            self.left_s = self.remaining(now_ms);
            self.state = PomoState::Paused;
        }
    }

    pub fn reset(&mut self) {
        self.state = PomoState::Idle;
        self.finished = None;
        self.left_s = self.duration();
    }

    pub fn skip(&mut self) {
        self.advance();
        self.state = PomoState::Idle;
        self.finished = None;
    }

    pub fn next_preset(&mut self) {
        self.preset = (self.preset + 1) % PRESETS.len();
        if matches!(self.state, PomoState::Idle | PomoState::Done) {
            self.left_s = self.duration();
        }
    }

    /// Call about once a second with today's day of the month. True when a phase
    /// just ended.
    pub fn tick(&mut self, now_ms: u64, day: Option<u8>) -> bool {
        if let Some(d) = day {
            if d != self.today.0 {
                self.today = (d, 0);
            }
        }
        if self.state != PomoState::Running || self.remaining(now_ms) > 0 {
            return false;
        }
        self.finished = Some(self.phase);
        if self.phase == Phase::Focus {
            self.cycle += 1;
            self.today.1 += 1;
        }
        self.advance();
        self.state = PomoState::Done;
        true
    }

    fn advance(&mut self) {
        self.phase = match self.phase {
            Phase::Focus if self.cycle > 0 && self.cycle % LONG_EVERY == 0 => Phase::Long,
            Phase::Focus => Phase::Short,
            Phase::Long => {
                self.cycle = 0;
                Phase::Focus
            }
            Phase::Short => Phase::Focus,
        };
        self.left_s = self.duration();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cycle() {
        let mut p = Pomodoro::new(2, (1, 0)); // 15 / 3 / 10
        p.start(0);
        assert_eq!(p.remaining(60_000), 14 * 60);
        assert!(!p.tick(899_000, Some(1)));
        assert!(p.tick(900_000, Some(1)));
        assert_eq!((p.state, p.finished, p.phase, p.today), (PomoState::Done, Some(Phase::Focus), Phase::Short, (1, 1)));
        p.start(900_000);
        p.pause(960_000);
        assert_eq!(p.remaining(999_999), 2 * 60);
        for _ in 0..3 {
            p.skip(); // -> focus
            p.start(0);
            assert!(p.tick(900_000, Some(1)));
        }
        assert_eq!(p.phase, Phase::Long); // after the 4th focus
    }
}
