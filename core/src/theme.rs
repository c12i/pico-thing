//! Gruvbox, matching Ghostty's "Gruvbox Dark" / "Gruvbox Light" themes and the
//! `cursor-color = #d79921` in ~/.config/ghostty/config, which is the accent here.
//! Screens take colours from `Theme` so the light/dark switch re-themes everything.
//!
//! Ghostty palette slots: fg/bg, dim = 7, muted = selection background, red = 9,
//! green = 10, yellow = 11, purple = 13, green2 = 2, blue = 4. The panel shades and
//! orange are Gruvbox's own (Ghostty's palette doesn't have them).

use ratatui::style::Color;

#[derive(Clone, Copy)]
pub struct Theme {
    pub bg: Color,
    pub panel: Color,
    pub panel2: Color,
    pub muted: Color,
    pub dim: Color,
    pub fg: Color,
    pub white: Color,
    pub accent: Color,
    pub accent2: Color,
    pub red: Color,
    pub green: Color,
    pub green2: Color,
    pub blue: Color,
    pub yellow: Color,
    pub orange: Color,
    pub purple: Color,
}

const fn hex(v: u32) -> Color {
    Color::Rgb((v >> 16) as u8, (v >> 8) as u8, v as u8)
}

pub const DARK: Theme = Theme {
    bg: hex(0x282828),
    panel: hex(0x3c3836),
    panel2: hex(0x504945),
    muted: hex(0x665c54),
    dim: hex(0xa89984),
    fg: hex(0xebdbb2),
    white: hex(0xfbf1c7),
    accent: hex(0xd79921), // the cursor
    accent2: hex(0xfe8019),
    red: hex(0xfb4934),
    green: hex(0xb8bb26),
    green2: hex(0x98971a),
    blue: hex(0x458588),
    yellow: hex(0xfabd2f),
    orange: hex(0xd65d0e),
    purple: hex(0xd3869b),
};

pub const LIGHT: Theme = Theme {
    bg: hex(0xfbf1c7),
    panel: hex(0xebdbb2),
    panel2: hex(0xd5c4a1),
    muted: hex(0xbdae93), // (Ghostty's light selection is the foreground; too dark here)
    dim: hex(0x7c6f64),
    fg: hex(0x3c3836),
    white: hex(0x282828),
    accent: hex(0xb57614), // the cursor gold, darkened to read on a light background
    accent2: hex(0xaf3a03),
    red: hex(0x9d0006),
    green: hex(0x79740e),
    green2: hex(0x98971a),
    blue: hex(0x458588),
    yellow: hex(0xb57614),
    orange: hex(0xd65d0e),
    purple: hex(0x8f3f71),
};
