//! A ratatui backend that draws straight to the panel.
//!
//! Each glyph of the 10x20 font is rasterised once into 25 bytes of bits, and each
//! run of changed cells in a row goes out as one window, streamed a pixel row at a
//! time. (mousefood draws every cell pixel by pixel through generic iterators,
//! which made full redraws take ~400ms.)

use alloc::collections::BTreeMap;
use alloc::vec::Vec;
use core::convert::Infallible;
use embedded_graphics::mono_font::{MonoFont, MonoTextStyle};
use embedded_graphics::pixelcolor::BinaryColor;
use embedded_graphics::prelude::*;
use embedded_graphics::text::{Baseline, Text};
use ratatui::backend::{Backend, ClearType, WindowSize};
use ratatui::buffer::Cell;
use ratatui::layout::{Position, Size};
use ratatui::style::{Color, Modifier};

use core::future::Future;
use core::pin::pin;
use core::task::{Context, Waker};
use embassy_rp::spi::Async;

use crate::lcd::Lcd;

pub const CW: u16 = 10;
pub const CH: u16 = 20;
const COLS: u16 = 48;
const ROWS: u16 = 16;

/// One 10-bit mask per pixel row (bit 9 = leftmost pixel).
type Glyph = [u16; CH as usize];

pub struct LcdBackend {
    pub lcd: Lcd<Async>,
    font: MonoFont<'static>,
    ascii: [Glyph; 95],
    other: BTreeMap<char, Glyph>,
    run: Vec<(Glyph, u16, u16)>,
    run_at: (u16, u16),
    /// Pixel rows of the run, as big-endian RGB565 (byte-swapped u16s): one is sent
    /// by DMA while the next is built
    line: [u16; (COLS * CW) as usize],
    line2: [u16; (COLS * CW) as usize],
    /// Per frame: microseconds spent in draw(), and cells drawn
    pub stats: (u64, u32),
}

impl LcdBackend {
    pub fn new(lcd: Lcd<Async>) -> Self {
        let font = embedded_graphics_unicodefonts::mono_10x20_atlas();
        let mut b = Self {
            lcd,
            font,
            ascii: [[0; CH as usize]; 95],
            other: BTreeMap::new(),
            run: Vec::with_capacity(COLS as usize),
            run_at: (0, 0),
            line: [0; (COLS * CW) as usize],
            line2: [0; (COLS * CW) as usize],
            stats: (0, 0),
        };
        for i in 0..95u8 {
            b.ascii[i as usize] = rasterise(&b.font, (b' ' + i) as char);
        }
        b
    }

    pub fn glyphs_cached(&self) -> usize {
        95 + self.other.len()
    }

    fn glyph(&mut self, c: char) -> Glyph {
        if (' '..='~').contains(&c) {
            return self.ascii[c as usize - 32];
        }
        if let Some(g) = self.other.get(&c) {
            return *g;
        }
        let g = rasterise(&self.font, c);
        self.other.insert(c, g);
        g
    }

    fn flush_run(&mut self) {
        let n = self.run.len() as u16;
        if n == 0 {
            return;
        }
        let (x, y) = self.run_at;
        let n_px = n as usize * CW as usize;
        let Self { lcd, run, line, line2, .. } = self;
        let (mut cur, mut next) = (line, line2);
        lcd.begin(x * CW, y * CH, n * CW, CH);
        fill_row(run, 0, &mut cur[..n_px]);
        for r in 0..CH as usize {
            // u16s already hold big-endian bytes, so send them as they lie in memory
            let bytes = unsafe { core::slice::from_raw_parts(cur.as_ptr() as *const u8, n_px * 2) };
            let mut send = pin!(lcd.spi.write(bytes));
            // The first poll starts the DMA; build the next row while it runs
            let started = send.as_mut().poll(&mut Context::from_waker(Waker::noop()));
            if r + 1 < CH as usize {
                fill_row(run, r + 1, &mut next[..n_px]);
            }
            if started.is_pending() {
                let _ = embassy_futures::block_on(send);
            }
            core::mem::swap(&mut cur, &mut next);
        }
        lcd.end();
        run.clear();
    }
}

/// Pixel row `r` of a run of cells.
fn fill_row(run: &[(Glyph, u16, u16)], r: usize, out: &mut [u16]) {
    for (&(ref g, fg, bg), px) in run.iter().zip(out.chunks_exact_mut(CW as usize)) {
        let bits = g[r];
        for (c, p) in px.iter_mut().enumerate() {
            *p = if bits & (0x200 >> c) != 0 { fg } else { bg };
        }
    }
}

/// Draw one character into a 10x20 bitmap.
fn rasterise(font: &MonoFont<'static>, c: char) -> Glyph {
    struct Bits(Glyph);
    impl OriginDimensions for Bits {
        fn size(&self) -> Size2 {
            Size2::new(CW as u32, CH as u32)
        }
    }
    impl DrawTarget for Bits {
        type Color = BinaryColor;
        type Error = Infallible;
        fn draw_iter<I: IntoIterator<Item = Pixel<BinaryColor>>>(&mut self, pixels: I) -> Result<(), Infallible> {
            for Pixel(p, c) in pixels {
                if c.is_on() && (0..CW as i32).contains(&p.x) && (0..CH as i32).contains(&p.y) {
                    self.0[p.y as usize] |= 0x200 >> p.x;
                }
            }
            Ok(())
        }
    }
    type Size2 = embedded_graphics::geometry::Size;
    let mut bits = Bits([0; CH as usize]);
    let mut s = [0u8; 4];
    let _ = Text::with_baseline(c.encode_utf8(&mut s), Point::zero(), MonoTextStyle::new(font, BinaryColor::On), Baseline::Top)
        .draw(&mut bits);
    bits.0
}

/// A colour as RGB565, byte-swapped so it sits in memory big-endian (the panel's order).
fn rgb565(c: Color, default: u16) -> u16 {
    let v = match c {
        Color::Rgb(r, g, b) => ((r as u16 & 0xF8) << 8) | ((g as u16 & 0xFC) << 3) | (b as u16 >> 3),
        Color::Reset => default,
        Color::Black => 0x0000,
        Color::White | Color::Gray => 0xFFFF,
        Color::DarkGray => 0x7BEF,
        Color::Red | Color::LightRed => 0xF800,
        Color::Green | Color::LightGreen => 0x07E0,
        Color::Yellow | Color::LightYellow => 0xFFE0,
        Color::Blue | Color::LightBlue => 0x001F,
        Color::Magenta | Color::LightMagenta => 0xF81F,
        Color::Cyan | Color::LightCyan => 0x07FF,
        Color::Indexed(_) => default,
    };
    v.to_be()
}

impl Backend for LcdBackend {
    type Error = Infallible;

    fn draw<'a, I>(&mut self, content: I) -> Result<(), Infallible>
    where
        I: Iterator<Item = (u16, u16, &'a Cell)>,
    {
        let t0 = embassy_time::Instant::now();
        let mut cells = 0;
        for (x, y, cell) in content {
            cells += 1;
            if x >= COLS || y >= ROWS {
                continue;
            }
            let (rx, ry) = self.run_at;
            if self.run.is_empty() || ry != y || rx + self.run.len() as u16 != x {
                self.flush_run();
                self.run_at = (x, y);
            }
            let c = cell.symbol().chars().next().unwrap_or(' ');
            let g = self.glyph(c);
            let (mut fg, mut bg) = (rgb565(cell.fg, 0xFFFF), rgb565(cell.bg, 0x0000));
            if cell.modifier.contains(Modifier::REVERSED) {
                core::mem::swap(&mut fg, &mut bg);
            }
            self.run.push((g, fg, bg));
        }
        self.flush_run();
        self.stats = ((embassy_time::Instant::now() - t0).as_micros(), cells);
        Ok(())
    }

    fn hide_cursor(&mut self) -> Result<(), Infallible> {
        Ok(())
    }

    fn show_cursor(&mut self) -> Result<(), Infallible> {
        Ok(())
    }

    fn get_cursor_position(&mut self) -> Result<Position, Infallible> {
        Ok(Position::ORIGIN)
    }

    fn set_cursor_position<P: Into<Position>>(&mut self, _: P) -> Result<(), Infallible> {
        Ok(())
    }

    fn clear(&mut self) -> Result<(), Infallible> {
        self.lcd.begin(0, 0, COLS * CW, ROWS * CH);
        let zero = [0u8; 480];
        for _ in 0..(ROWS * CH * 2) {
            self.lcd.push(&zero);
        }
        self.lcd.end();
        Ok(())
    }

    fn clear_region(&mut self, _: ClearType) -> Result<(), Infallible> {
        self.clear()
    }

    fn size(&self) -> Result<Size, Infallible> {
        Ok(Size::new(COLS, ROWS))
    }

    fn window_size(&mut self) -> Result<WindowSize, Infallible> {
        Ok(WindowSize { columns_rows: Size::new(COLS, ROWS), pixels: Size::new(COLS * CW, ROWS * CH) })
    }

    fn flush(&mut self) -> Result<(), Infallible> {
        Ok(())
    }
}
