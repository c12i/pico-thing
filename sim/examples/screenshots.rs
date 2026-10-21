//! Render every screen to a PNG in docs/screenshots, pixel for pixel as the Pico's
//! panel shows it: the firmware's 10x20 font and colours rounded to RGB565.
//!   cargo run --example screenshots

mod demo;

use std::convert::Infallible;
use std::path::Path;

use embedded_graphics::mono_font::{MonoFont, MonoTextStyle};
use embedded_graphics::pixelcolor::BinaryColor;
use embedded_graphics::prelude::*;
use embedded_graphics::text::{Baseline, Text};
use ratatui::Terminal;
use ratatui::backend::TestBackend;
use ratatui::buffer::Buffer;
use ratatui::style::{Color, Modifier};
use thing::{COLS, ROWS};

const CW: usize = 10;
const CH: usize = 20;
/// Each panel pixel becomes SCALE x SCALE, so the images stay crisp on big screens.
const SCALE: usize = 2;

/// One character's pixels, as the firmware's backend rasterises it.
fn glyph(font: &MonoFont<'static>, c: char) -> [[bool; CW]; CH] {
    struct Bits([[bool; CW]; CH]);
    impl OriginDimensions for Bits {
        fn size(&self) -> Size {
            Size::new(CW as u32, CH as u32)
        }
    }
    impl DrawTarget for Bits {
        type Color = BinaryColor;
        type Error = Infallible;
        fn draw_iter<I: IntoIterator<Item = Pixel<BinaryColor>>>(&mut self, pixels: I) -> Result<(), Infallible> {
            for Pixel(p, c) in pixels {
                if c.is_on() && (0..CW as i32).contains(&p.x) && (0..CH as i32).contains(&p.y) {
                    self.0[p.y as usize][p.x as usize] = true;
                }
            }
            Ok(())
        }
    }
    let mut bits = Bits([[false; CW]; CH]);
    let mut s = [0u8; 4];
    let _ = Text::with_baseline(c.encode_utf8(&mut s), Point::zero(), MonoTextStyle::new(font, BinaryColor::On), Baseline::Top)
        .draw(&mut bits);
    bits.0
}

/// A ratatui colour as the panel shows it (through RGB565).
fn rgb(c: Color, default: [u8; 3]) -> [u8; 3] {
    let [r, g, b] = match c {
        Color::Rgb(r, g, b) => [r, g, b],
        _ => return default,
    };
    let (r5, g6, b5) = (r >> 3, g >> 2, b >> 3);
    [(r5 << 3) | (r5 >> 2), (g6 << 2) | (g6 >> 4), (b5 << 3) | (b5 >> 2)]
}

fn render(buf: &Buffer, font: &MonoFont<'static>) -> Vec<u8> {
    let (w, h) = (COLS as usize * CW * SCALE, ROWS as usize * CH * SCALE);
    let mut px = vec![0u8; w * h * 3];
    for row in 0..ROWS {
        for col in 0..COLS {
            let cell = &buf[(col, row)];
            let g = glyph(font, cell.symbol().chars().next().unwrap_or(' '));
            let (mut fg, mut bg) = (rgb(cell.fg, [255; 3]), rgb(cell.bg, [0; 3]));
            if cell.modifier.contains(Modifier::REVERSED) {
                std::mem::swap(&mut fg, &mut bg);
            }
            for (y, line) in g.iter().enumerate() {
                for (x, on) in line.iter().enumerate() {
                    let color = if *on { fg } else { bg };
                    for dy in 0..SCALE {
                        for dx in 0..SCALE {
                            let px_x = (col as usize * CW + x) * SCALE + dx;
                            let px_y = (row as usize * CH + y) * SCALE + dy;
                            let i = (px_y * w + px_x) * 3;
                            px[i..i + 3].copy_from_slice(&color);
                        }
                    }
                }
            }
        }
    }
    px
}

fn main() {
    let font = embedded_graphics_unicodefonts::mono_10x20_atlas();
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../docs/screenshots");
    std::fs::create_dir_all(&dir).unwrap();
    let mut term = Terminal::new(TestBackend::new(COLS, ROWS)).unwrap();
    for (name, screen, face) in demo::SCREENS {
        let mut app = demo::app(*screen, *face);
        term.draw(|f| app.draw(f, (0, 0))).unwrap();
        let pixels = render(term.backend().buffer(), &font);
        let path = dir.join(format!("{name}.png"));
        let file = std::io::BufWriter::new(std::fs::File::create(&path).unwrap());
        let mut png = png::Encoder::new(file, (COLS as usize * CW * SCALE) as u32, (ROWS as usize * CH * SCALE) as u32);
        png.set_color(png::ColorType::Rgb);
        png.set_depth(png::BitDepth::Eight);
        png.write_header().unwrap().write_image_data(&pixels).unwrap();
        println!("{}", path.display());
    }
}
