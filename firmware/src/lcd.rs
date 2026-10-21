//! Waveshare Pico-ResTouch-LCD-3.5 (ILI9488 behind shift registers, 480x320) plus its
//! XPT2046 touch controller, which shares the SPI bus.
//!
//! The board latches 16 bits per CS frame, so every command and parameter goes out as
//! its own word `[0x00, byte]`. Pixel data is RGB565 big-endian.

use core::convert::Infallible;
use embassy_rp::gpio::{Input, Output};
use embassy_rp::peripherals::SPI1;
use embassy_rp::spi::{Blocking, Mode, Spi};
use embassy_time::{Duration, block_for};
use embedded_graphics::pixelcolor::raw::RawU16;
use embedded_graphics::pixelcolor::Rgb565;
use embedded_graphics::prelude::*;
use embedded_graphics::primitives::Rectangle;

pub const WIDTH: u32 = 480;
pub const HEIGHT: u32 = 320;
pub const LCD_HZ: u32 = 31_250_000; // 62.5MHz is too fast for the shift registers
const TOUCH_HZ: u32 = 2_000_000;

/// `M` is the SPI mode: Blocking, or Async when the caller wants to stream pixels
/// with DMA (`lcd.spi.write(..)` between `begin` and `end`).
pub struct Lcd<M: Mode = Blocking> {
    pub spi: Spi<'static, SPI1, M>,
    pub dc: Output<'static>,
    pub cs: Output<'static>,
    pub rst: Output<'static>,
    pub tp_cs: Output<'static>,
    pub tp_irq: Input<'static>,
    buf: [u8; 512],
    // Last column/row ranges sent; text cells in one row share their row range, so
    // skipping repeats saves most of the per-cell command overhead
    cols: (u16, u16),
    rows: (u16, u16),
    /// Why the last touch reading was thrown away, with its (x, y, z1) - for tuning
    pub reject: Option<(&'static str, u16, u16, u16)>,
}

impl<M: Mode> Lcd<M> {
    pub fn new(
        spi: Spi<'static, SPI1, M>,
        dc: Output<'static>,
        cs: Output<'static>,
        rst: Output<'static>,
        tp_cs: Output<'static>,
        tp_irq: Input<'static>,
    ) -> Self {
        let mut lcd = Self { spi, dc, cs, rst, tp_cs, tp_irq, buf: [0; 512], cols: (u16::MAX, 0), rows: (u16::MAX, 0), reject: None };
        lcd.init();
        lcd
    }

    fn write16(&mut self, data: bool, value: u8) {
        if data { self.dc.set_high() } else { self.dc.set_low() }
        self.cs.set_low();
        let _ = self.spi.blocking_write(&[0x00, value]);
        self.cs.set_high();
    }

    fn cmd(&mut self, c: u8, params: &[u8]) {
        self.write16(false, c);
        for &p in params {
            self.write16(true, p);
        }
    }

    fn init(&mut self) {
        let ms = |n| block_for(Duration::from_millis(n));
        self.rst.set_high(); ms(5);
        self.rst.set_low(); ms(10);
        self.rst.set_high(); ms(5);
        self.cmd(0x21, &[]); // inversion on (the panel needs it)
        self.cmd(0xC2, &[0x33]);
        self.cmd(0xC5, &[0x00, 0x1E, 0x80]);
        self.cmd(0xB1, &[0xB0]);
        self.cmd(0xE0, &[0x00, 0x13, 0x18, 0x04, 0x0F, 0x06, 0x3A, 0x56, 0x4D, 0x03, 0x0A, 0x06, 0x30, 0x3E, 0x0F]);
        self.cmd(0xE1, &[0x00, 0x13, 0x18, 0x01, 0x11, 0x06, 0x38, 0x34, 0x4D, 0x06, 0x0D, 0x0B, 0x31, 0x37, 0x0F]);
        self.cmd(0x3A, &[0x55]); // 16 bits per pixel
        self.cmd(0x11, &[]); // sleep out
        ms(120);
        self.cmd(0x29, &[]); // display on
        self.cmd(0xB6, &[0x00, 0x62]);
        self.cmd(0x36, &[0x28]); // landscape, BGR
    }

    fn window(&mut self, x: u16, y: u16, w: u16, h: u16) {
        let (x1, y1) = (x + w - 1, y + h - 1);
        if self.cols != (x, x1) {
            self.cols = (x, x1);
            self.cmd(0x2A, &[(x >> 8) as u8, x as u8, (x1 >> 8) as u8, x1 as u8]);
        }
        if self.rows != (y, y1) {
            self.rows = (y, y1);
            self.cmd(0x2B, &[(y >> 8) as u8, y as u8, (y1 >> 8) as u8, y1 as u8]);
        }
        self.cmd(0x2C, &[]);
    }

    /// Stream pixels into the current window, 256 at a time.
    fn stream(&mut self, colors: impl Iterator<Item = Rgb565>) {
        self.dc.set_high();
        self.cs.set_low();
        let mut n = 0;
        for c in colors {
            let v = RawU16::from(c).into_inner();
            self.buf[n] = (v >> 8) as u8;
            self.buf[n + 1] = v as u8;
            n += 2;
            if n == self.buf.len() {
                let _ = self.spi.blocking_write(&self.buf);
                n = 0;
            }
        }
        if n > 0 {
            let _ = self.spi.blocking_write(&self.buf[..n]);
        }
        self.cs.set_high();
    }

    /// Start writing pixels to a rectangle: follow with `push` calls, then `end`.
    pub fn begin(&mut self, x: u16, y: u16, w: u16, h: u16) {
        self.window(x, y, w, h);
        self.dc.set_high();
        self.cs.set_low();
    }

    /// Big-endian RGB565 bytes for the rectangle opened by `begin`.
    pub fn push(&mut self, bytes: &[u8]) {
        let _ = self.spi.blocking_write(bytes);
    }

    pub fn end(&mut self) {
        self.cs.set_high();
    }

    /// Median raw (x, y) touch reading, or None when not pressed.
    pub fn touch_raw(&mut self) -> Option<(u16, u16)> {
        if self.tp_irq.is_high() {
            return None;
        }
        self.spi.set_frequency(TOUCH_HZ);
        let mut xs = [0u16; 7];
        let mut ys = [0u16; 7];
        for i in 0..7 {
            xs[i] = self.touch_sample(0xD0);
            ys[i] = self.touch_sample(0x90);
        }
        let z1 = self.touch_sample(0xB0);
        self.spi.set_frequency(LCD_HZ);
        if self.tp_irq.is_high() {
            return None;
        }
        xs.sort_unstable();
        ys.sort_unstable();
        let (x, y) = (xs[3], ys[3]);
        // Limits tuned from logged taps: real taps on this panel often read
        // z1 50-80 and spread 200-300, which the old limits (80, 200) dropped
        if z1 < 50 {
            self.reject = Some(("light press", x, y, z1));
            return None;
        }
        if xs[5] - xs[1] > 300 || ys[5] - ys[1] > 300 {
            self.reject = Some(("unsteady", x, y, z1));
            return None;
        }
        if !(150 < x && x < 3950 && 150 < y && y < 3950) {
            self.reject = Some(("out of range", x, y, z1));
            return None;
        }
        Some((x, y))
    }

    fn touch_sample(&mut self, cmd: u8) -> u16 {
        let mut rx = [0u8; 3];
        self.tp_cs.set_low();
        let _ = self.spi.blocking_transfer(&mut rx, &[cmd, 0, 0]);
        self.tp_cs.set_high();
        (((rx[1] as u16) << 8) | rx[2] as u16) >> 3
    }
}

impl<M: Mode> OriginDimensions for Lcd<M> {
    fn size(&self) -> Size {
        Size::new(WIDTH, HEIGHT)
    }
}

impl<M: Mode> DrawTarget for Lcd<M> {
    type Color = Rgb565;
    type Error = Infallible;

    fn draw_iter<I: IntoIterator<Item = Pixel<Rgb565>>>(&mut self, pixels: I) -> Result<(), Infallible> {
        let bounds = self.bounding_box();
        for Pixel(p, c) in pixels {
            if bounds.contains(p) {
                self.window(p.x as u16, p.y as u16, 1, 1);
                self.stream(core::iter::once(c));
            }
        }
        Ok(())
    }

    fn fill_contiguous<I: IntoIterator<Item = Rgb565>>(&mut self, area: &Rectangle, colors: I) -> Result<(), Infallible> {
        let clipped = area.intersection(&self.bounding_box());
        if clipped == *area && !area.is_zero_sized() {
            // The common case (a whole text cell): one window, one burst
            self.window(area.top_left.x as u16, area.top_left.y as u16, area.size.width as u16, area.size.height as u16);
            self.stream(colors.into_iter());
            Ok(())
        } else {
            self.draw_iter(area.points().zip(colors).map(|(p, c)| Pixel(p, c)))
        }
    }

    fn fill_solid(&mut self, area: &Rectangle, color: Rgb565) -> Result<(), Infallible> {
        let a = area.intersection(&self.bounding_box());
        if a.is_zero_sized() {
            return Ok(());
        }
        self.window(a.top_left.x as u16, a.top_left.y as u16, a.size.width as u16, a.size.height as u16);
        self.stream(core::iter::repeat_n(color, (a.size.width * a.size.height) as usize));
        Ok(())
    }
}
