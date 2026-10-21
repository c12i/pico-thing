//! Print screens as text, to check layouts without a Pico:
//!   cargo run --example dump [name ...]
//! Names are those in demo::SCREENS (e.g. `sonos face-pet`); no names prints them all.

mod demo;

use ratatui::Terminal;
use ratatui::backend::TestBackend;
use thing::{COLS, ROWS};

fn main() {
    let only: Vec<String> = std::env::args().skip(1).collect();
    let mut term = Terminal::new(TestBackend::new(COLS, ROWS)).unwrap();
    for (name, screen, face) in demo::SCREENS {
        if !only.is_empty() && !only.iter().any(|o| o == name) {
            continue;
        }
        let mut app = demo::app(*screen, *face);
        term.draw(|f| app.draw(f, (0, 0))).unwrap();
        println!("── {name} ──");
        let buf = term.backend().buffer();
        for y in 0..ROWS {
            let line: String = (0..COLS).map(|x| buf[(x, y)].symbol().to_string()).collect();
            println!("│{line}│");
        }
    }
}
