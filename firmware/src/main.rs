//! pico-thing on the Pico W: runs the `thing` app (core/) on the
//! Pico-ResTouch-LCD-3.5, feeding it taps and time and carrying out its requests.
//!
//! Core 0 runs the app; core 1 runs USB serial (usb.rs), which carries logs
//! and reboots into the bootloader when the Mac sends a byte (bootsel.sh).
#![no_std]
#![no_main]

extern crate alloc;

mod backend;
mod storage;
mod lcd;
mod usb;
mod wifi;

use alloc::format;
use core::mem::MaybeUninit;
use core::ptr::addr_of_mut;

use embassy_executor::Executor;
use embassy_rp::clocks::RoscRng;
use embassy_rp::gpio::{Input, Level, Output, Pull};
use embassy_rp::pwm::{self, Pwm};
use embassy_rp::peripherals::{DMA_CH0, DMA_CH1, DMA_CH2, PIO0};
use embassy_rp::spi::{self, Async, Spi};
use embassy_rp::{bind_interrupts, dma, pio};
use embassy_time::{Duration, Instant, Timer};
use embedded_alloc::LlffHeap as Heap;
use ratatui::Terminal;
use static_cell::StaticCell;
use thing::app::Status;
use thing::{App, Event, Request};

use backend::{CH, CW, LcdBackend};

/// A panic restarts the Pico; the message survives the reset (in RAM the startup
/// code doesn't clear) and is printed over USB on the next boot.
#[unsafe(link_section = ".uninit.PANIC")]
static mut PANIC_MSG: [u8; 200] = [0; 200];
#[unsafe(link_section = ".uninit.PANIC")]
static mut PANIC_MAGIC: u32 = 0;
const MAGIC: u32 = 0xDEAD_C0DE;

#[panic_handler]
fn panic(info: &core::panic::PanicInfo) -> ! {
    struct Buf(usize);
    impl core::fmt::Write for Buf {
        fn write_str(&mut self, s: &str) -> core::fmt::Result {
            for &b in s.as_bytes() {
                if self.0 < 200 {
                    unsafe { (*addr_of_mut!(PANIC_MSG))[self.0] = b };
                    self.0 += 1;
                }
            }
            Ok(())
        }
    }
    let mut b = Buf(0);
    let _ = core::fmt::write(&mut b, format_args!("{info}"));
    unsafe {
        if b.0 < 200 {
            (*addr_of_mut!(PANIC_MSG))[b.0] = 0;
        }
        *addr_of_mut!(PANIC_MAGIC) = MAGIC;
    }
    cortex_m::peripheral::SCB::sys_reset()
}

/// The last panic's message, if the previous run ended in one.
fn last_panic() -> Option<alloc::string::String> {
    unsafe {
        if *addr_of_mut!(PANIC_MAGIC) != MAGIC {
            return None;
        }
        *addr_of_mut!(PANIC_MAGIC) = 0;
        let m = &*addr_of_mut!(PANIC_MSG);
        let n = m.iter().position(|&b| b == 0).unwrap_or(m.len());
        Some(alloc::string::String::from_utf8_lossy(&m[..n]).into_owned())
    }
}

#[global_allocator]
static HEAP: Heap = Heap::empty();
const HEAP_SIZE: usize = 160 * 1024;

static EXECUTOR0: StaticCell<Executor> = StaticCell::new();

bind_interrupts!(pub struct Irqs {
    PIO0_IRQ_0 => pio::InterruptHandler<PIO0>;
    DMA_IRQ_0 => dma::InterruptHandler<DMA_CH0>, dma::InterruptHandler<DMA_CH1>, dma::InterruptHandler<DMA_CH2>;
});

/// The backlight on GP13, dimmed with PWM.
struct Backlight(Pwm<'static>, pwm::Config);

impl Backlight {
    fn set(&mut self, pct: u8) {
        self.1.compare_b = (self.1.top as u32 * pct.min(100) as u32 / 100) as u16;
        self.0.set_config(&self.1);
    }
}

#[cortex_m_rt::entry]
fn main() -> ! {
    {
        static mut HEAP_MEM: [MaybeUninit<u8>; HEAP_SIZE] = [MaybeUninit::uninit(); HEAP_SIZE];
        unsafe { HEAP.init(addr_of_mut!(HEAP_MEM) as usize, HEAP_SIZE) }
    }
    let p = embassy_rp::init(Default::default());
    usb::start(p.CORE1, p.USB, "pico-thing");

    let mut cfg = spi::Config::default();
    cfg.frequency = lcd::LCD_HZ;
    // DMA streams pixels to the panel (channels 1 and 2; 0 is the Wi-Fi chip's)
    let spi = Spi::new(p.SPI1, p.PIN_10, p.PIN_11, p.PIN_12, p.DMA_CH1, p.DMA_CH2, Irqs, cfg);
    let mut pwm_cfg = pwm::Config::default();
    pwm_cfg.top = 0xFFFF;
    pwm_cfg.compare_b = 0xFFFF;
    pwm_cfg.divider = fixed::FixedU16::from_num(4); // ~480Hz at 125MHz: no audible whine, no flicker
    let backlight = Backlight(Pwm::new_output_b(p.PWM_SLICE6, p.PIN_13, pwm_cfg.clone()), pwm_cfg);
    let display = lcd::Lcd::new(
        spi,
        Output::new(p.PIN_8, Level::High),
        Output::new(p.PIN_9, Level::High),
        Output::new(p.PIN_15, Level::High),
        Output::new(p.PIN_16, Level::High),
        Input::new(p.PIN_17, Pull::Up),
    );

    let store = storage::Store::new(p.FLASH);
    let wifi_pins = wifi::Pins { pio: p.PIO0, dma: p.DMA_CH0, pwr: p.PIN_23, dio: p.PIN_24, cs: p.PIN_25, clk: p.PIN_29 };
    let executor0 = EXECUTOR0.init(Executor::new());
    executor0.run(|spawner| {
        spawner.spawn(ui_task(display, backlight, store).unwrap());
        spawner.spawn(wifi::task(spawner, wifi_pins, random_u32() as u64 | (random_u32() as u64) << 32).unwrap());
    })
}

fn random_u32() -> u32 {
    let mut b = [0u8; 4];
    for x in &mut b {
        *x = RoscRng::next_u8();
    }
    u32::from_le_bytes(b)
}

/// Log a boot step and show it straight away (the main loop isn't running yet).
fn boot_step(app: &mut App, terminal: &mut Terminal<LcdBackend>, key: &'static str, text: alloc::string::String, ok: Option<bool>) {
    usb::log(&format!("boot: {text}\n"));
    app.handle(Event::Boot { key, text, ok });
    terminal.draw(|f| app.draw(f, (0, 0))).unwrap();
}

/// Loops (10ms apart, plus drawing) without a touch before the finger counts as lifted.
const RELEASE_LOOPS: u8 = 4;

#[embassy_executor::task]
async fn ui_task(display: lcd::Lcd<Async>, mut backlight: Backlight, mut store: storage::Store) -> ! {
    let mut terminal = Terminal::new(LcdBackend::new(display)).unwrap();
    let loaded = store.load().await;
    let from_flash = loaded.is_some();
    let settings = loaded.unwrap_or_default();
    let calibrated = settings.calib.is_some();
    backlight.set(settings.brightness);
    let mut app = App::new(settings, random_u32());
    // A finger held on the screen while booting forces recalibration
    if terminal.backend().lcd.tp_irq.is_low() {
        app.calibrate();
    }

    // The boot log (see screens/boot.rs); the Wi-Fi task and the app add the rest
    let mhz = lcd::LCD_HZ / 1_000_000;
    boot_step(&mut app, &mut terminal, "lcd", format!("lcd       ili9488 480x320, spi {mhz} MHz"), Some(true));
    let touch = if calibrated { "touch     xpt2046, calibrated" } else { "touch     xpt2046, default calibration" };
    boot_step(&mut app, &mut terminal, "touch", touch.into(), Some(true));
    let flash = if from_flash { "settings  loaded from flash" } else { "settings  defaults (none saved yet)" };
    boot_step(&mut app, &mut terminal, "flash", flash.into(), Some(true));
    boot_step(&mut app, &mut terminal, "heap", format!("heap      {} KB", HEAP_SIZE / 1024), Some(true));
    boot_step(&mut app, &mut terminal, "layout", "ratatui   warming layouts".into(), None);
    {
        // Solve every screen's layout once now, off-screen (see App::warm_up)
        let t0 = Instant::now();
        let mut offscreen = Terminal::new(ratatui::backend::TestBackend::new(thing::COLS, thing::ROWS)).unwrap();
        app.warm_up(&mut offscreen);
        let ms = t0.elapsed().as_millis();
        usb::log(&format!("layouts warmed in {ms} ms\n"));
        boot_step(&mut app, &mut terminal, "layout", format!("ratatui   layouts warmed in {ms} ms"), Some(true));
    }
    let panicked = last_panic();
    let boot = Instant::now();
    let mut next_status = Instant::now();
    let mut was_down = false;
    // Loops in a row with no touch reading; the finger counts as lifted after a few
    // (a single dropped reading mid-hold shouldn't end a long press)
    let mut up_loops = 0u8;
    let mut drag_cell = (u16::MAX, u16::MAX);
    let (mut frames, mut last_ms, mut max_ms) = (0u32, 0u64, 0u64);
    loop {
        let now = Instant::now();
        let raw = terminal.backend_mut().lcd.touch_raw();
        if let Some((why, x, y, z)) = terminal.backend_mut().lcd.reject.take() {
            usb::log(&format!("touch rejected ({why}): raw {x},{y} z1 {z}\n"));
        }
        // While a finger stays down, report where it moves (dragging scrollbars)
        if let (Some(r), true) = (raw, was_down) {
            let (x, y) = thing::touch::map(&app.state.settings.calib(), r);
            let cell = (x as u16 / CW, y as u16 / CH);
            if cell != drag_cell {
                drag_cell = cell;
                app.handle(Event::Drag { col: cell.0, row: cell.1 });
            }
        }
        if let (Some(r), false) = (raw, was_down) {
            if app.wants_raw_touch() {
                app.handle(Event::RawTouch { x: r.0, y: r.1 });
                usb::log(&format!("raw touch {r:?}\n"));
            } else {
                let (x, y) = thing::touch::map(&app.state.settings.calib(), r);
                let (col, row) = (x as u16 / CW, y as u16 / CH);
                drag_cell = (col, row);
                app.handle(Event::Tap { col, row });
                usb::log(&format!("tap ({x},{y}) -> cell ({col},{row})\n"));
            }
        }
        if raw.is_some() {
            was_down = true;
            up_loops = 0;
        } else if was_down {
            up_loops += 1;
            if up_loops >= RELEASE_LOOPS {
                was_down = false;
                app.handle(Event::Release);
            }
        }
        while let Ok((col, row)) = usb::TAPS.try_receive() {
            app.handle(Event::Tap { col, row });
            app.handle(Event::Release);
            usb::log(&format!("serial tap ({col},{row})\n"));
        }

        app.handle(Event::Tick { uptime_ms: (now - boot).as_millis() });
        while let Ok(ev) = wifi::EVENTS.try_receive() {
            app.handle(ev);
        }
        for r in app.take_requests() {
            match r {
                Request::Brightness(pct) => backlight.set(pct),
                Request::SaveSettings(s) => {
                    let ok = store.save(&s).await;
                    usb::log(&format!("settings saved: {ok}\n"));
                }
                Request::FlashBacklight => {
                    for _ in 0..3 {
                        backlight.set(0);
                        Timer::after_millis(150).await;
                        backlight.set(app.state.settings.brightness);
                        Timer::after_millis(150).await;
                    }
                }
                Request::Net(job) => {
                    if wifi::JOBS.try_send(job).is_err() {
                        usb::log("job queue full\n");
                    }
                }
            }
        }
        if app.dirty() {
            let t0 = Instant::now();
            terminal.draw(|f| app.draw(f, (0, 0))).unwrap();
            last_ms = (Instant::now() - t0).as_millis();
            max_ms = max_ms.max(last_ms);
            frames += 1;
            if last_ms > 50 {
                let (us, cells) = terminal.backend().stats;
                usb::log(&format!("slow frame {last_ms} ms: backend {} ms for {cells} cells\n", us / 1000));
            }
        }
        if let Some(msg) = &panicked {
            // Repeat it for a while, so a Mac that connects late still sees it
            if (now - boot).as_secs() < 60 && (now - boot).as_millis() % 2000 < 15 {
                usb::log(&format!("last run panicked: {msg}\n"));
            }
        }
        if now >= next_status {
            next_status = now + Duration::from_secs(2);
            let (used, free) = (HEAP.used(), HEAP.free());
            app.handle(Event::Status(Status {
                ip: None,
                heap_free_kb: (free / 1024) as u32,
                heap_total_kb: ((used + free) / 1024) as u32,
            }));
            usb::log(&format!(
                "heap {}/{} KB  frame {last_ms} ms (max {max_ms})  frames {frames}  glyphs {}\n",
                used / 1024,
                HEAP_SIZE / 1024,
                terminal.backend().glyphs_cached()
            ));
        }
        Timer::after_millis(10).await;
    }
}
