//! ASCII art: wttr.in-style weather icons and the Productivity face's robot.
//!
//! Each picture is lines of text plus a same-shaped colour mask:
//! y = yellow, c = cloud (fg), d = dim, b = blue, w = white, a = accent,
//! r = red, o = orange, space = fg.

use ratatui::buffer::Buffer;
use ratatui::style::Color;

use crate::net::weather::Icon;
use crate::theme::Theme;

pub type Art = [(&'static str, &'static str); 5];
pub const ART_W: u16 = 13;

pub fn weather(icon: Icon) -> Art {
    match icon {
        Icon::Sun => [
            ("    \\   /    ", "    y   y    "),
            ("     .-.     ", "     yyy     "),
            ("  ― (   ) ―  ", "  y yyyyy y  "),
            ("     `-'     ", "     yyy     "),
            ("    /   \\    ", "    y   y    "),
        ],
        Icon::Moon => [
            ("             ", "             "),
            ("     .--.    ", "     wwww    "),
            ("    (  (     ", "    ww w     "),
            ("     `--'    ", "     wwww    "),
            ("             ", "             "),
        ],
        Icon::Partly | Icon::PartlyNight => [
            ("   \\  /      ", "   y  y      "),
            (" _ /\"\".-.    ", " y yyyccc    "),
            ("   \\_(   ).  ", "   yycccccc  "),
            ("   /(___(__) ", "   ycccccccc "),
            ("             ", "             "),
        ],
        Icon::Cloud => [
            ("             ", "             "),
            ("     .--.    ", "     cccc    "),
            ("  .-(    ).  ", "  ccccccccc  "),
            (" (___.__)__) ", " ccccccccccc "),
            ("             ", "             "),
        ],
        Icon::Fog => [
            ("             ", "             "),
            (" _ - _ - _ - ", " d d d d d d "),
            ("  _ - _ - _  ", "  d d d d d  "),
            (" _ - _ - _ - ", " d d d d d d "),
            ("             ", "             "),
        ],
        Icon::Drizzle => [
            ("     .-.     ", "     ccc     "),
            ("    (   ).   ", "    cccccc   "),
            ("   (___(__)  ", "   cccccccc  "),
            ("    ' ' ' '  ", "    b b b b  "),
            ("             ", "             "),
        ],
        Icon::Rain => [
            ("     .-.     ", "     ccc     "),
            ("    (   ).   ", "    cccccc   "),
            ("   (___(__)  ", "   cccccccc  "),
            ("  ‚'‚'‚'‚'   ", "  bbbbbbbb   "),
            ("  ‚'‚'‚'‚'   ", "  bbbbbbbb   "),
        ],
        Icon::Snow => [
            ("     .-.     ", "     ccc     "),
            ("    (   ).   ", "    cccccc   "),
            ("   (___(__)  ", "   cccccccc  "),
            ("    *  *  *  ", "    w  w  w  "),
            ("   *  *  *   ", "   w  w  w   "),
        ],
        Icon::Storm => [
            ("     .-.     ", "     ccc     "),
            ("    (   ).   ", "    cccccc   "),
            ("   (___(__)  ", "   cccccccc  "),
            ("  ‚'⚡‚'⚡‚'  ", "  bbybbybb   "),
            ("  ‚'‚'‚'‚'   ", "  bbbbbbbb   "),
        ],
    }
}

fn ink(th: &Theme, m: char) -> Color {
    match m {
        'y' => th.yellow,
        'c' => th.fg,
        'd' => th.dim,
        'b' => th.blue,
        'w' => th.white,
        'a' => th.accent,
        'r' => th.red,
        'o' => th.orange,
        _ => th.fg,
    }
}

/// Draw art with its top-left corner at (x, y); spaces leave the background alone.
pub fn draw(buf: &mut Buffer, x: u16, y: u16, art: &[(&str, &str)], th: &Theme) {
    for (r, (text, mask)) in art.iter().enumerate() {
        for ((c, ch), m) in text.chars().enumerate().zip(mask.chars().chain(core::iter::repeat(' '))) {
            if ch == ' ' {
                continue;
            }
            if let Some(cell) = buf.cell_mut((x + c as u16, y + r as u16)) {
                cell.set_char(ch).set_fg(ink(th, m));
            }
        }
    }
}
