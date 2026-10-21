//! Wi-Fi (the CYW43439 on the Pico W) and the network task: joins the network, then
//! runs the app's network jobs one at a time and sends the results back.

use alloc::format;
use cyw43::{JoinOptions, PowerManagementMode};
use cyw43_pio::{DEFAULT_CLOCK_DIVIDER, PioSpi};
use embassy_executor::Spawner;
use embassy_net::dns::DnsQueryType;
use embassy_net::tcp::TcpSocket;
use embassy_net::udp::{PacketMetadata, UdpSocket};
use embassy_net::{IpAddress, Ipv4Address, Stack, StackResources};
use embassy_rp::gpio::{Level, Output};
use embassy_rp::peripherals::{DMA_CH0, PIN_23, PIN_24, PIN_25, PIN_29, PIO0};
use embassy_rp::pio::Pio;
use embassy_rp::{Peri, dma};
use embassy_sync::blocking_mutex::raw::CriticalSectionRawMutex;
use embassy_sync::channel::Channel;
use embassy_time::{Duration, Timer, with_timeout};
use static_cell::StaticCell;
use thing::app::Event;
use thing::config;
use thing::net::services::{self, Job};
use thing::net::{Conn, Net, NetError, parse_ipv4};

use crate::{Irqs, usb};

/// Jobs from the app, and events (results, connection status) back to it.
pub static JOBS: Channel<CriticalSectionRawMutex, Job, 8> = Channel::new();
pub static EVENTS: Channel<CriticalSectionRawMutex, Event, 8> = Channel::new();

/// Give up on a job after this long (the job's own sockets time out sooner).
const JOB_TIMEOUT: Duration = Duration::from_secs(20);

pub struct Pins {
    pub pio: Peri<'static, PIO0>,
    pub dma: Peri<'static, DMA_CH0>,
    pub pwr: Peri<'static, PIN_23>,
    pub dio: Peri<'static, PIN_24>,
    pub cs: Peri<'static, PIN_25>,
    pub clk: Peri<'static, PIN_29>,
}

type Spi = PioSpi<'static, PIO0, 0>;

/// Report a boot step to the app's boot log (it ignores them once booted).
async fn boot(key: &'static str, text: alloc::string::String, ok: Option<bool>) {
    EVENTS.send(Event::Boot { key, text, ok }).await;
}

#[embassy_executor::task]
async fn cyw43_task(runner: cyw43::Runner<'static, cyw43::SpiBus<Output<'static>, Spi>>) -> ! {
    runner.run().await
}

#[embassy_executor::task]
async fn stack_task(mut runner: embassy_net::Runner<'static, cyw43::NetDriver<'static>>) -> ! {
    runner.run().await
}

#[embassy_executor::task]
pub async fn task(spawner: Spawner, pins: Pins, seed: u64) -> ! {
    static FW: &cyw43::Aligned<cyw43::A4, [u8]> = &cyw43::Aligned(*cyw43_firmware::CYW43_43439A0);
    let clm = cyw43_firmware::CYW43_43439A0_CLM;
    let nvram = cyw43::aligned_bytes!("../nvram_43439.bin");

    boot("cyw43", "wifi chip loading firmware".into(), None).await;
    let pwr = Output::new(pins.pwr, Level::Low);
    let cs = Output::new(pins.cs, Level::High);
    let mut pio = Pio::new(pins.pio, Irqs);
    let dma = dma::Channel::new(pins.dma, Irqs);
    let spi = PioSpi::new(&mut pio.common, pio.sm0, DEFAULT_CLOCK_DIVIDER, pio.irq0, cs, pins.dio, pins.clk, dma);

    static STATE: StaticCell<cyw43::State> = StaticCell::new();
    let (device, mut control, runner) = cyw43::new(STATE.init(cyw43::State::new()), pwr, spi, FW, nvram).await;
    spawner.spawn(cyw43_task(runner).unwrap());
    control.init(clm).await;
    // No power saving: LAN devices (lights, speakers) answer much faster
    control.set_power_management(PowerManagementMode::None).await;
    boot("cyw43", "wifi chip cyw43439 up".into(), Some(true)).await;

    static RESOURCES: StaticCell<StackResources<6>> = StaticCell::new();
    let (stack, runner) = embassy_net::new(
        device,
        embassy_net::Config::dhcpv4(Default::default()),
        RESOURCES.init(StackResources::new()),
        seed,
    );
    spawner.spawn(stack_task(runner).unwrap());
    if thing::net::companion::configured() {
        spawner.spawn(poke_task(stack).unwrap());
    }

    let mut net = PicoNet { stack, rx: [0; 4096], tx: [0; 1024] };
    loop {
        // (Re)join, then wait for an address
        boot("wifi", format!("wifi      joining {}", config::WIFI_SSID), None).await;
        while let Err(e) = control.join(config::WIFI_SSID, JoinOptions::new(config::WIFI_PASSWORD.as_bytes())).await {
            let why = match e {
                cyw43::JoinError::NetworkNotFound => "network not found",
                cyw43::JoinError::AuthenticationFailure => "wrong password?",
                cyw43::JoinError::JoinFailure(_) => "join failure",
            };
            usb::log(&format!("wifi: join failed ({why})\n"));
            boot("wifi", format!("wifi      {why}, retrying"), None).await;
            Timer::after_secs(5).await;
        }
        boot("wifi", format!("wifi      joined {}", config::WIFI_SSID), Some(true)).await;
        boot("dhcp", "dhcp      waiting for an address".into(), None).await;
        stack.wait_config_up().await;
        let ip = stack.config_v4().map(|c| c.address.address().octets());
        usb::log(&format!("wifi: up, ip {ip:?}\n"));
        EVENTS.send(Event::Online(ip)).await;

        while stack.is_link_up() {
            let Ok(job) = with_timeout(Duration::from_secs(5), JOBS.receive()).await else { continue };
            let t0 = embassy_time::Instant::now();
            let done = match with_timeout(JOB_TIMEOUT, services::run(&mut net, job.clone())).await {
                Ok(d) => d,
                Err(_) => services::timed_out(&job),
            };
            usb::log(&format!("job {job:?}: {} ms, ok={}\n", t0.elapsed().as_millis(), done.ok()));
            if let services::Done::Favorites(_, Ok(v)) = &done {
                let tracks = v.iter().filter(|f| !f.2).count();
                usb::log(&format!("favorites: {} playable, {tracks} tracks\n", v.len()));
            }
            EVENTS.send(Event::Net(done)).await;
        }
        usb::log("wifi: link down\n");
        EVENTS.send(Event::Online(None)).await;
    }
}

struct PicoNet {
    stack: Stack<'static>,
    rx: [u8; 4096],
    tx: [u8; 1024],
}

async fn resolve(stack: Stack<'static>, host: &str) -> Result<Ipv4Address, NetError> {
    if let Some([a, b, c, d]) = parse_ipv4(host) {
        return Ok(Ipv4Address::new(a, b, c, d));
    }
    let addrs = stack.dns_query(host, DnsQueryType::A).await.map_err(|_| NetError::Dns)?;
    match addrs.first() {
        Some(IpAddress::Ipv4(a)) => Ok(*a),
        _ => Err(NetError::Dns),
    }
}

pub struct PicoConn<'a>(TcpSocket<'a>);

impl Conn for PicoConn<'_> {
    async fn write_all(&mut self, data: &[u8]) -> Result<(), NetError> {
        embedded_io_async::Write::write_all(&mut self.0, data).await.map_err(|_| NetError::Io)
    }

    async fn read(&mut self, buf: &mut [u8]) -> Result<usize, NetError> {
        self.0.read(buf).await.map_err(|_| NetError::Io)
    }
}

impl Drop for PicoConn<'_> {
    fn drop(&mut self) {
        self.0.abort();
    }
}

impl Net for PicoNet {
    type Conn<'a> = PicoConn<'a>;

    async fn connect(&mut self, host: &str, port: u16) -> Result<PicoConn<'_>, NetError> {
        let ip = resolve(self.stack, host).await?;
        let mut s = TcpSocket::new(self.stack, &mut self.rx, &mut self.tx);
        s.set_timeout(Some(Duration::from_secs(10)));
        s.connect((ip, port)).await.map_err(|_| NetError::Connect)?;
        Ok(PicoConn(s))
    }

    async fn udp_exchange(&mut self, host: &str, port: u16, send: &[u8], recv: &mut [u8]) -> Result<usize, NetError> {
        let ip = resolve(self.stack, host).await?;
        let mut rx_meta = [PacketMetadata::EMPTY; 2];
        let mut tx_meta = [PacketMetadata::EMPTY; 2];
        let mut rx_buf = [0u8; 1024];
        let mut tx_buf = [0u8; 512];
        let mut s = UdpSocket::new(self.stack, &mut rx_meta, &mut rx_buf, &mut tx_meta, &mut tx_buf);
        s.bind(0).map_err(|_| NetError::Io)?;
        s.send_to(send, (ip, port)).await.map_err(|_| NetError::Io)?;
        match with_timeout(Duration::from_secs(3), s.recv_from(recv)).await {
            Ok(Ok((n, _))) => Ok(n),
            Ok(Err(_)) => Err(NetError::Io),
            Err(_) => Err(NetError::Timeout),
        }
    }
}

/// The companion POSTs /poke here when something changes, so alerts show up at
/// once instead of on the next 30s poll. Anything without the token is refused.
#[embassy_executor::task]
async fn poke_task(stack: Stack<'static>) -> ! {
    const PORT: u16 = 8080;
    let mut rx = [0u8; 512];
    let mut tx = [0u8; 128];
    loop {
        let mut s = TcpSocket::new(stack, &mut rx, &mut tx);
        s.set_timeout(Some(Duration::from_secs(3)));
        if s.accept(PORT).await.is_err() {
            Timer::after_millis(500).await;
            continue;
        }
        let mut req = [0u8; 512];
        let mut n = 0;
        while n < req.len() {
            match s.read(&mut req[n..]).await {
                Ok(0) | Err(_) => break,
                Ok(k) => n += k,
            }
            if req[..n].windows(4).any(|w| w == b"\r\n\r\n") {
                break;
            }
        }
        let head = core::str::from_utf8(&req[..n]).unwrap_or("");
        let token_ok = head.lines().any(|l| {
            l.split_once(':').is_some_and(|(k, v)| k.trim().eq_ignore_ascii_case("x-token") && v.trim() == config::COMPANION_TOKEN)
        });
        let ok = head.starts_with("POST /poke") && token_ok;
        let reply: &[u8] = if ok { b"HTTP/1.0 204 No Content\r\n\r\n" } else { b"HTTP/1.0 403 Forbidden\r\n\r\n" };
        let _ = embedded_io_async::Write::write_all(&mut s, reply).await;
        let _ = embedded_io_async::Write::flush(&mut s).await;
        s.close();
        let _ = with_timeout(Duration::from_millis(500), s.flush()).await;
        if ok {
            EVENTS.send(Event::Poke).await;
        }
    }
}
