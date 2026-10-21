//! A Tamagotchi-style pet: four needs that wear down with real time, and the things
//! you can do about them. Saved with the settings, so it lives through restarts and
//! catches up on the time it was off (wall-clock minutes, once the clock is set).

use serde::{Deserialize, Serialize};

/// Most time a restart catches up on (it doesn't starve over a long holiday).
const MAX_CATCH_UP_MIN: i64 = 3 * 24 * 60;
/// Minutes between poops while it has food in it.
const DIGEST_MIN: u16 = 180;
const MAX_POOPS: u8 = 3;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Pet {
    /// Unix seconds it hatched; 0 while it's still an egg
    pub born: i64,
    /// Unix seconds the needs were last brought up to date (whole minutes)
    pub at: i64,
    /// 0-100, higher is better
    pub food: u8,
    pub fun: u8,
    pub energy: u8,
    pub clean: u8,
    pub asleep: bool,
    pub poops: u8,
    /// Minutes since it last pooped
    pub digest: u16,
}

impl Default for Pet {
    fn default() -> Self {
        Self { born: 0, at: 0, food: 80, fun: 80, energy: 100, clean: 100, asleep: false, poops: 0, digest: 0 }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PetCmd {
    Hatch,
    Feed,
    Play,
    Clean,
    /// Lights off (or on again)
    Sleep,
    /// A pat on the head
    Pat,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Mood {
    Egg,
    Asleep,
    /// Something is nearly empty
    Sad,
    Happy,
    Fine,
}

fn add(v: u8, d: i16) -> u8 {
    (v as i16 + d).clamp(0, 100) as u8
}

impl Pet {
    pub fn hatched(&self) -> bool {
        self.born != 0
    }

    /// Age in whole days.
    pub fn age_days(&self, now: i64) -> i64 {
        if self.hatched() { (now - self.born).max(0) / 86_400 } else { 0 }
    }

    pub fn mood(&self) -> Mood {
        if !self.hatched() {
            Mood::Egg
        } else if self.asleep {
            Mood::Asleep
        } else if self.food < 20 || self.fun < 20 || self.energy < 15 || self.clean < 25 {
            Mood::Sad
        } else if self.food >= 60 && self.fun >= 60 && self.clean >= 60 && self.poops == 0 {
            Mood::Happy
        } else {
            Mood::Fine
        }
    }

    /// Wear the needs down to `now` (unix seconds), a minute at a time. Which
    /// minutes count is fixed by the clock, so catching up after a restart lands
    /// on the same numbers as having been on the whole time.
    pub fn advance(&mut self, now: i64) {
        if !self.hatched() {
            return;
        }
        let to = now / 60;
        let from = (self.at / 60).max(to - MAX_CATCH_UP_MIN);
        for m in from + 1..=to {
            self.minute(m);
        }
        self.at = self.at.max(to * 60);
    }

    fn minute(&mut self, m: i64) {
        let every = |n: i64| m % n == 0;
        if self.asleep {
            if every(16) {
                self.food = add(self.food, -1);
            }
            if every(2) {
                self.energy = add(self.energy, 1);
            }
            if self.energy == 100 {
                self.asleep = false; // slept enough
            }
        } else {
            if every(8) {
                self.food = add(self.food, -1);
            }
            if every(6) {
                self.fun = add(self.fun, -1);
            }
            if every(7) {
                self.energy = add(self.energy, -1);
            }
            if self.energy == 0 {
                self.asleep = true; // dozes off wherever it is
            }
        }
        if every(10) && self.poops > 0 {
            self.clean = add(self.clean, -(self.poops as i16) * 2);
        }
        self.digest = self.digest.saturating_add(1);
        if self.digest >= DIGEST_MIN && self.food > 30 {
            self.digest = 0;
            self.poops = (self.poops + 1).min(MAX_POOPS);
        }
    }

    /// Do something with it; false if it isn't up for it (asleep, or an egg).
    pub fn apply(&mut self, cmd: PetCmd, now: i64) -> bool {
        if cmd == PetCmd::Hatch {
            if self.hatched() {
                return false;
            }
            *self = Pet { born: now, at: now / 60 * 60, ..Pet::default() };
            return true;
        }
        if !self.hatched() {
            return false;
        }
        self.advance(now);
        match cmd {
            PetCmd::Sleep => self.asleep = !self.asleep,
            PetCmd::Clean => {
                self.poops = 0;
                self.clean = 100;
            }
            _ if self.asleep => return false,
            PetCmd::Feed => {
                self.food = add(self.food, 25);
                self.energy = add(self.energy, 2);
            }
            PetCmd::Play => {
                self.fun = add(self.fun, 20);
                self.energy = add(self.energy, -8);
                self.food = add(self.food, -3);
            }
            PetCmd::Pat => self.fun = add(self.fun, 4),
            PetCmd::Hatch => {}
        }
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const T0: i64 = 1_800_000_000 / 60 * 60;

    #[test]
    fn needs_wear_down_and_catch_up_the_same() {
        let mut a = Pet::default();
        a.apply(PetCmd::Hatch, T0);
        let mut b = a.clone();
        // One pet kept up to date every minute, the other caught up in one go
        for m in 1..=600 {
            a.advance(T0 + m * 60);
        }
        b.advance(T0 + 600 * 60);
        assert_eq!(a, b);
        assert!(a.food < 80 && a.fun < 80 && a.energy < 100);
        assert!(a.poops >= 1, "it digests in ten hours");
    }

    #[test]
    fn sleeps_when_tired_and_wakes_rested() {
        let mut p = Pet { energy: 1, ..Pet::default() };
        p.apply(PetCmd::Hatch, T0);
        p.energy = 1;
        p.advance(T0 + 20 * 60);
        assert!(p.asleep);
        assert!(!p.apply(PetCmd::Feed, T0 + 21 * 60), "won't eat asleep");
        // Fully rested after ~200 minutes, then awake (and tiring again) since
        p.advance(T0 + 300 * 60);
        assert!(!p.asleep && p.energy > 80, "{p:?}");
    }

    #[test]
    fn eggs_do_nothing_until_hatched() {
        let mut p = Pet::default();
        assert!(!p.apply(PetCmd::Feed, T0));
        p.advance(T0 + 9999 * 60);
        assert_eq!(p, Pet::default());
        assert!(p.apply(PetCmd::Hatch, T0));
        assert_eq!(p.mood(), Mood::Happy);
    }
}
