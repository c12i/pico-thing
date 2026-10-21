//! cbonsai's tree growth (https://gitlab.com/jallbrit/cbonsai), as run with
//! `cbonsai --live --infinite --life=16 --time=0.1`. A port of cbonsai's C:
//! the recursion is an explicit stack so `step()` places one string per call.

use alloc::vec::Vec;

#[derive(Clone, Copy, PartialEq)]
enum Kind {
    Trunk,
    ShootLeft,
    ShootRight,
    Dying,
    Dead,
}

/// How the face should colour a placed string (cbonsai's colour pairs).
#[derive(Clone, Copy, PartialEq)]
pub enum Style {
    WoodBold,
    Wood,
    LeafBold,
    Leaf,
    LeafBright,
}

/// The pot (cbonsai's base type 1): rows of (text, style) runs; None = pot colour.
pub const BASE: [&[(&str, Option<Style>)]; 4] = [
    &[(":", None), ("___________", Some(Style::Leaf)), ("./~~~\\.", Some(Style::WoodBold)),
      ("___________", Some(Style::Leaf)), (":", None)],
    &[(" \\                           / ", None)],
    &[("  \\_________________________/ ", None)],
    &[("  (_)                     (_)", None)],
];
pub const BASE_W: i32 = 31;
pub const BASE_H: i32 = 4;

struct Frame {
    y: i32,
    x: i32,
    kind: Kind,
    life: i32,
    cooldown: i32,
    dx: i32,
    dy: i32,
    moved: bool, // false: before the child branch; true: still has to move and draw
}

pub struct Tree {
    rows: i32,
    life: i32,
    mult: i32,
    shoots: u32,
    rng: u32,
    stack: Vec<Frame>,
}

impl Tree {
    pub fn new(rows: i32, cols: i32, life: i32, seed: u32) -> Self {
        let mut t = Self { rows, life, mult: 5, shoots: 0, rng: seed | 1, stack: Vec::new() };
        t.shoots = t.r(1 << 16) as u32;
        t.stack.push(Frame { y: rows - 1, x: cols / 2, kind: Kind::Trunk, life, cooldown: 5,
                             dx: 0, dy: 0, moved: false });
        t
    }

    fn r(&mut self, n: i32) -> i32 {
        // xorshift32
        let mut x = self.rng;
        x ^= x << 13;
        x ^= x >> 17;
        x ^= x << 5;
        self.rng = x;
        ((x >> 8) % n as u32) as i32
    }

    /// Grow until the next string is placed: (y, x, text, style), or None when done.
    pub fn step(&mut self) -> Option<(i32, i32, &'static str, Style)> {
        let m = self.mult;
        while let Some(f) = self.stack.last_mut() {
            if f.moved {
                // Second half of an iteration, after any child branch has grown
                f.moved = false;
                f.cooldown -= 1;
                f.y += f.dy;
                f.x += f.dx;
                let (y, x, kind, life, dx, dy) = (f.y, f.x, f.kind, f.life, f.dx, f.dy);
                let text = self.string(kind, life, dx, dy);
                let style = self.style(kind);
                return Some((y, x, text, style));
            }
            if f.life <= 0 {
                self.stack.pop();
                continue;
            }
            f.life -= 1;
            let (y, x, kind, life, cooldown) = (f.y, f.x, f.kind, f.life, f.cooldown);
            let (dx, mut dy) = self.deltas(kind, life, self.life - life);
            if dy > 0 && y > self.rows - 2 {
                dy -= 1; // too close to the ground
            }
            let mut child = None;
            let mut new_cooldown = None;
            if life < 3 {
                child = Some((Kind::Dead, life)); // near-dead: lots of leaves
            } else if kind != Kind::Dying && kind != Kind::Dead && life < m + 2 {
                child = Some((Kind::Dying, life));
            } else if kind == Kind::Trunk && (self.r(3) == 0 || life % m == 0) {
                if self.r(8) == 0 && life > 7 {
                    new_cooldown = Some(m * 2);
                    child = Some((Kind::Trunk, life + self.r(5) - 2)); // a second trunk
                } else if cooldown <= 0 {
                    new_cooldown = Some(m * 2);
                    self.shoots += 1;
                    let k = if self.shoots % 2 == 0 { Kind::ShootLeft } else { Kind::ShootRight };
                    child = Some((k, life + m));
                }
            }
            let f = self.stack.last_mut().unwrap();
            f.dx = dx;
            f.dy = dy;
            f.moved = true;
            if let Some(c) = new_cooldown {
                f.cooldown = c;
            }
            if let Some((kind, life)) = child {
                self.stack.push(Frame { y, x, kind, life, cooldown: m, dx: 0, dy: 0, moved: false });
            }
        }
        None
    }

    fn pick(dice: i32, table: &[(i32, i32)]) -> i32 {
        table.iter().find(|(top, _)| dice <= *top).map(|(_, v)| *v).unwrap_or(0)
    }

    fn deltas(&mut self, kind: Kind, life: i32, age: i32) -> (i32, i32) {
        let m = self.mult;
        match kind {
            Kind::Trunk => {
                if age <= 2 || life < 4 {
                    (self.r(3) - 1, 0) // new or dead trunk
                } else if age < m * 3 {
                    // young trunk grows wide
                    let dy = if age % (m / 2) == 0 { -1 } else { 0 };
                    let d = self.r(10);
                    (Self::pick(d, &[(0, -2), (3, -1), (5, 0), (8, 1), (9, 2)]), dy)
                } else {
                    let dx = self.r(3) - 1;
                    (dx, if self.r(10) > 2 { -1 } else { 0 })
                }
            }
            Kind::ShootLeft | Kind::ShootRight => {
                let d = self.r(10);
                let dy = Self::pick(d, &[(1, -1), (7, 0), (9, 1)]);
                let d = self.r(10);
                let dx = Self::pick(d, &[(1, -2), (5, -1), (8, 0), (9, 1)]);
                (if kind == Kind::ShootLeft { dx } else { -dx }, dy)
            }
            Kind::Dying => {
                let d = self.r(10);
                let dy = Self::pick(d, &[(1, -1), (8, 0), (9, 1)]);
                let d = self.r(15);
                let dx = Self::pick(d, &[(0, -3), (2, -2), (5, -1), (8, 0), (11, 1), (13, 2), (14, 3)]);
                (dx, dy)
            }
            Kind::Dead => {
                let dx = self.r(3) - 1;
                let d = self.r(10);
                (dx, Self::pick(d, &[(2, -1), (6, 0), (9, 1)]))
            }
        }
    }

    fn string(&self, kind: Kind, life: i32, dx: i32, dy: i32) -> &'static str {
        if life < 4 || kind == Kind::Dying || kind == Kind::Dead {
            return "&";
        }
        match kind {
            Kind::Trunk if dy == 0 => "/~",
            Kind::Trunk if dx < 0 => "\\|",
            Kind::Trunk if dx == 0 => "/|\\",
            Kind::Trunk => "|/",
            Kind::ShootLeft if dy > 0 => "\\",
            Kind::ShootRight if dy > 0 => "/",
            Kind::ShootLeft if dy == 0 => "\\_",
            Kind::ShootRight if dy == 0 => "_/",
            _ if dx < 0 => "\\|",
            _ if dx == 0 => "/|",
            _ => "/",
        }
    }

    fn style(&mut self, kind: Kind) -> Style {
        match kind {
            Kind::Dying => if self.r(10) == 0 { Style::LeafBold } else { Style::Leaf },
            Kind::Dead => Style::LeafBright,
            _ => if self.r(2) == 0 { Style::WoodBold } else { Style::Wood },
        }
    }
}
