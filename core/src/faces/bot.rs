//! The Productivity face's robot, as 11x6 cells of text art.

use ratatui::buffer::Buffer;
use ratatui::style::Color;

use crate::theme::Theme;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Mood {
    Happy,
    Overwhelmed,
    Excited,
    Busy,
    Sleeping,
    Chill,
    Confused,
    Bored,
}

pub const W: u16 = 11;
pub const H: u16 = 6;

/// Draw the robot at (x, y); `frame` advances once a second for the animations.
pub fn draw(buf: &mut Buffer, x: u16, y: u16, mood: Mood, frame: u32, th: &Theme) {
    let blink = mood == Mood::Happy && frame % 7 == 0;
    let (eyes, mouth) = match mood {
        Mood::Happy if blink => ("- -", "‿"),
        Mood::Happy => ("^ ^", "‿"),
        Mood::Overwhelmed => (if frame % 2 == 0 { "@ @" } else { "◎ ◎" }, "~"),
        Mood::Excited => ("★ ★", "▽"),
        Mood::Busy => ("▬ ▬", "─"),
        Mood::Sleeping => ("- -", "o"),
        Mood::Chill => ("■-■", "◡"),
        Mood::Confused => ("o O", "~"),
        Mood::Bored => ("_ _", "_"),
    };
    let ball = match mood {
        Mood::Busy => if frame % 2 == 0 { th.accent } else { th.orange },
        Mood::Excited => th.yellow,
        Mood::Overwhelmed => th.red,
        Mood::Sleeping => th.muted,
        _ => th.green,
    };
    // 11 columns: the head spans 2..=8, eyes sit at 4 and 6, the mouth at 5
    let body: [(&str, Color); 6] = [
        ("     ●     ", ball),
        ("  ╭──┴──╮  ", th.dim),
        ("  │     │  ", th.dim),
        ("  │     │  ", th.dim),
        ("  ╰──┬──╯  ", th.dim),
        ("  ╱▀▀▀▀▀╲  ", th.muted),
    ];
    for (r, (line, color)) in body.iter().enumerate() {
        put(buf, x, y + r as u16, line, *color);
    }
    // Eyes and mouth in the face colour
    let face = if mood == Mood::Sleeping { th.dim } else { th.accent };
    put(buf, x + 4, y + 2, eyes, face);
    put(buf, x + 5, y + 3, mouth, if mood == Mood::Excited { th.red } else { face });
    match mood {
        Mood::Sleeping => {
            let k = frame % 3;
            put(buf, x + 8, y + 1 - (k as u16 % 2), "z", th.dim);
            if k > 0 {
                put(buf, x + 9, y, "Z", th.dim);
            }
        }
        Mood::Excited => {
            let (a, b) = if frame % 2 == 0 { (x, x + 10) } else { (x + 10, x) };
            put(buf, a, y + 1, "✦", th.yellow);
            put(buf, b, y + 3, "✦", th.yellow);
        }
        Mood::Overwhelmed => put(buf, x + 9, y + 1 + (frame % 2) as u16, "•", th.blue),
        Mood::Confused => put(buf, x + 9, y, "?", th.orange),
        _ => {}
    }
}

fn put(buf: &mut Buffer, x: u16, y: u16, s: &str, color: Color) {
    for (i, ch) in s.chars().enumerate() {
        if ch == ' ' {
            continue;
        }
        if let Some(cell) = buf.cell_mut((x + i as u16, y)) {
            cell.set_char(ch).set_fg(color);
        }
    }
}
