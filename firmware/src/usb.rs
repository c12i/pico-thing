//! USB serial on core 1: forwards text written to `LOG`, and takes commands from
//! the Mac, one per line:
//!   b            reboot into the USB bootloader (bootsel.sh), so flashing
//!                never needs the BOOTSEL button
//!   t COL ROW    a tap on that text cell (for testing without a finger)

use core::ptr::addr_of_mut;
use embassy_executor::Executor;
use embassy_rp::bind_interrupts;
use embassy_rp::multicore::{Stack, spawn_core1};
use embassy_rp::peripherals::{CORE1, USB};
use embassy_rp::usb::{self as rp_usb, Driver};
use embassy_rp::Peri;
use embassy_sync::blocking_mutex::raw::CriticalSectionRawMutex;
use embassy_sync::pipe::Pipe;
use embassy_usb::UsbDevice;
use embassy_usb::class::cdc_acm::{CdcAcmClass, State};
use static_cell::StaticCell;

/// Text written here shows up on the Mac's /dev/cu.usbmodem* port.
pub static LOG: Pipe<CriticalSectionRawMutex, 1024> = Pipe::new();
/// Taps sent over serial ("t COL ROW").
pub static TAPS: embassy_sync::channel::Channel<CriticalSectionRawMutex, (u16, u16), 8> =
    embassy_sync::channel::Channel::new();

bind_interrupts!(struct Irqs {
    USBCTRL_IRQ => rp_usb::InterruptHandler<USB>;
});

static mut CORE1_STACK: Stack<8192> = Stack::new();
static EXECUTOR1: StaticCell<Executor> = StaticCell::new();

/// Run USB on core 1, so it keeps answering while core 0 is busy drawing.
pub fn start(core1: Peri<'static, CORE1>, usb: Peri<'static, USB>, product: &'static str) {
    spawn_core1(core1, unsafe { &mut *addr_of_mut!(CORE1_STACK) }, move || {
        let driver = Driver::new(usb, Irqs);
        let mut config = embassy_usb::Config::new(0x2e8a, 0x000a);
        config.manufacturer = Some("pico-thing");
        config.product = Some(product);
        config.serial_number = Some(product);
        static CONFIG_DESC: StaticCell<[u8; 256]> = StaticCell::new();
        static BOS_DESC: StaticCell<[u8; 256]> = StaticCell::new();
        static CONTROL_BUF: StaticCell<[u8; 64]> = StaticCell::new();
        static STATE: StaticCell<State> = StaticCell::new();
        let mut builder = embassy_usb::Builder::new(
            driver,
            config,
            CONFIG_DESC.init([0; 256]),
            BOS_DESC.init([0; 256]),
            &mut [],
            CONTROL_BUF.init([0; 64]),
        );
        let class = CdcAcmClass::new(&mut builder, STATE.init(State::new()), 64);
        let usb = builder.build();
        let executor1 = EXECUTOR1.init(Executor::new());
        executor1.run(|spawner| {
            spawner.spawn(usb_task(usb).unwrap());
            spawner.spawn(serial_task(class).unwrap());
        })
    });
}

#[embassy_executor::task]
async fn usb_task(mut usb: UsbDevice<'static, Driver<'static, USB>>) -> ! {
    usb.run().await
}

#[embassy_executor::task]
async fn serial_task(mut class: CdcAcmClass<'static, Driver<'static, USB>>) -> ! {
    use embassy_futures::select::{Either, select};
    let mut buf = [0u8; 64];
    let mut rx = [0u8; 64];
    loop {
        class.wait_connection().await;
        loop {
            let n = match select(LOG.read(&mut buf), class.read_packet(&mut rx)).await {
                Either::First(n) => n,
                Either::Second(Ok(n)) => {
                    if command(&rx[..n]) {
                        reboot_to_bootloader(&class).await;
                    }
                    continue;
                }
                Either::Second(Err(_)) => break,
            };
            if class.write_packet(&buf[..n]).await.is_err() {
                break;
            }
        }
    }
}

/// Log a line without blocking (dropped if the Mac isn't reading).
pub fn log(s: &str) {
    let _ = LOG.try_write(s.as_bytes());
}

/// Reboot into the USB bootloader, but only once the Mac has closed the port (or
/// after a second and a half): vanishing while macOS still has the serial port open
/// can panic its USB serial driver (it took the whole Mac down once).
async fn reboot_to_bootloader(class: &CdcAcmClass<'static, Driver<'static, USB>>) -> ! {
    use embassy_time::Timer;
    for _ in 0..30 {
        if !class.dtr() {
            break;
        }
        Timer::after_millis(50).await;
    }
    Timer::after_millis(250).await;
    embassy_rp::rom_data::reset_to_usb_boot(0, 0);
    #[allow(clippy::empty_loop)]
    loop {}
}

/// Run a command from the Mac; true if it asked for the bootloader.
fn command(data: &[u8]) -> bool {
    let text = core::str::from_utf8(data).unwrap_or("");
    for line in text.split(['\n', '\r']) {
        let mut words = line.split_whitespace();
        match words.next() {
            Some("t") => {
                let col = words.next().and_then(|w| w.parse().ok());
                let row = words.next().and_then(|w| w.parse().ok());
                if let (Some(c), Some(r)) = (col, row) {
                    let _ = TAPS.try_send((c, r));
                }
            }
            Some("b") => return true,
            _ => {}
        }
    }
    false
}
