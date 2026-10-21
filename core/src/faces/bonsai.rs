//! A bonsai that keeps growing (cbonsai --live --infinite --life=16 --time=0.1)
//! under the top strip.

use alloc::vec;
use alloc::vec::Vec;
use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::Color;

use super::bonsai_tree::{BASE, BASE_H, BASE_W, Style as Part, Tree};
use crate::app::State;
use crate::theme::Theme;

const COLS: i32 = crate::COLS as i32;
const ROWS: i32 = crate::ROWS as i32 - 2; // under the strip
const STEP_MS: u64 = 100; // --time=0.1
const WAIT_MS: u64 = 4000; // cbonsai's default --wait

/// What fills a cell, so colours follow the theme.
#[derive(Clone, Copy, PartialEq)]
enum Ink {
    Pot,
    Part(Part),
}

pub struct Garden {
    cells: Vec<(u8, Ink)>,
    tree: Tree,
    seed: u32,
    next_ms: u64,
    done_at: Option<u64>,
}

impl Garden {
    pub fn new(seed: u32) -> Self {
        let mut g = Self {
            cells: vec![(b' ', Ink::Pot); (COLS * ROWS) as usize],
            tree: Tree::new(ROWS - BASE_H, COLS, 16, seed),
            seed,
            next_ms: 0,
            done_at: None,
        };
        let top = ROWS - BASE_H;
        let bx = COLS / 2 - BASE_W / 2;
        for (r, runs) in BASE.iter().enumerate() {
            let mut x = bx;
            for (text, part) in runs.iter() {
                g.put(top + r as i32, x, text, part.map(Ink::Part).unwrap_or(Ink::Pot));
                x += text.len() as i32;
            }
        }
        g
    }

    fn put(&mut self, row: i32, col: i32, text: &str, ink: Ink) {
        for (i, b) in text.bytes().enumerate() {
            let x = col + i as i32;
            if (0..ROWS).contains(&row) && (0..COLS).contains(&x) {
                self.cells[(row * COLS + x) as usize] = (b, ink);
            }
        }
    }

    /// One --time step: grow a string, or wait and plant a new tree. True if changed.
    /// The tree has finished growing (it starts over a few seconds later).
    pub fn grown(&self) -> bool {
        self.done_at.is_some()
    }

    pub fn tick(&mut self, now_ms: u64) -> bool {
        if now_ms < self.next_ms {
            return false;
        }
        self.next_ms = now_ms + STEP_MS;
        if let Some(t) = self.done_at {
            if now_ms - t < WAIT_MS {
                return false;
            }
            let tree_rows = ROWS - BASE_H;
            for c in &mut self.cells[..(tree_rows * COLS) as usize] {
                *c = (b' ', Ink::Pot);
            }
            self.seed = self.seed.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
            self.tree = Tree::new(tree_rows, COLS, 16, self.seed);
            self.done_at = None;
            return true;
        }
        match self.tree.step() {
            Some((y, x, text, part)) => {
                self.put(y, x, text, Ink::Part(part));
                true
            }
            None => {
                self.done_at = Some(now_ms);
                false
            }
        }
    }
}

fn color(th: &Theme, ink: Ink) -> Color {
    match ink {
        Ink::Pot => th.dim,
        Ink::Part(Part::WoodBold) => th.yellow,
        Ink::Part(Part::Wood) => th.orange,
        Ink::Part(Part::Leaf) => th.green2,
        Ink::Part(Part::LeafBold | Part::LeafBright) => th.green,
    }
}

pub fn draw(st: &State, f: &mut Frame, area: Rect) {
    super::strip(st, f, area);
    let g = &st.faces.bonsai;
    let buf = f.buffer_mut();
    let top = area.y + area.height - ROWS as u16;
    for r in 0..ROWS {
        for c in 0..COLS {
            let (ch, ink) = g.cells[(r * COLS + c) as usize];
            if ch != b' ' {
                if let Some(cell) = buf.cell_mut((area.x + c as u16, top + r as u16)) {
                    cell.set_char(ch as char).set_fg(color(&st.theme, ink));
                }
            }
        }
    }
}
