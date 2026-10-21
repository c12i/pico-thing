//! pico-thing's app, independent of hardware: a 48x16 ratatui UI driven by events.
//! The Pico firmware (`firmware/`) and the terminal simulator (`sim/`) both
//! run it.
#![no_std]

extern crate alloc;

pub mod app;
pub mod clock;
pub mod faces;
pub mod net;
pub mod pet;
pub mod pomodoro;
pub mod screens;
pub mod targets;
pub mod term;
pub mod theme;
pub mod touch;
pub mod widgets;

/// Build-time settings from config.toml (see build.rs).
pub mod config {
    include!(concat!(env!("OUT_DIR"), "/config.rs"));
}

pub use app::{App, Event, Request, Settings};

/// The screen as a text grid: 480x320 pixels in 10x20 cells.
pub const COLS: u16 = 48;
pub const ROWS: u16 = 16;
