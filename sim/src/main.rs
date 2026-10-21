//! pico-thing in your terminal: the Pico's 48x16 screen inside a bezel, with mouse
//! clicks as taps. Runs the same `thing` app as the firmware.
//!
//!   cargo run            (q or Esc quits)

mod net;

use std::io;
use std::sync::mpsc;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use crossterm::event::{self, DisableMouseCapture, EnableMouseCapture, Event as CEvent, KeyCode, MouseButton, MouseEventKind};
use crossterm::execute;
use ratatui::layout::Rect;
use ratatui::style::{Color, Style};
use ratatui::text::Line;
use ratatui::widgets::{Block, BorderType};
use thing::app::Status;
use thing::{App, COLS, Event, ROWS, Request, Settings};

const SETTINGS: &str = "settings.json";

fn main() -> io::Result<()> {
    if std::env::args().any(|a| a == "--check") {
        return check();
    }
    let settings: Settings = std::fs::read(SETTINGS)
        .ok()
        .and_then(|b| serde_json::from_slice(&b).ok())
        .unwrap_or_default();
    let seed = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().subsec_nanos();
    let app = &mut App::new(settings, seed);

    let mut terminal = ratatui::init();
    execute!(io::stdout(), EnableMouseCapture)?;
    let result = run(&mut terminal, app);
    execute!(io::stdout(), DisableMouseCapture)?;
    ratatui::restore();
    result
}

fn run(terminal: &mut ratatui::DefaultTerminal, app: &mut App) -> io::Result<()> {
    let start = Instant::now();
    app.handle(Event::Tick { uptime_ms: 0 });
    app.handle(Event::Status(Status { ip: net::local_ip(), heap_free_kb: 0, heap_total_kb: 0 }));
    app.handle(Event::Boot { key: "sim", text: "sim       terminal, 48x16 cells".into(), ok: Some(true) });
    app.handle(Event::Online(net::local_ip()));
    // Network jobs run on their own thread, like the Pico's network task
    let (job_tx, job_rx) = mpsc::channel();
    let (done_tx, done_rx) = mpsc::channel();
    std::thread::spawn(move || net::worker(job_rx, done_tx));
    let mut backlight = app.state.settings.brightness;
    let mut flash_until: Option<Instant> = None;
    let mut origin = (0, 0);
    let mut force = true;
    loop {
        app.handle(Event::Tick { uptime_ms: start.elapsed().as_millis() as u64 });
        while let Ok(done) = done_rx.try_recv() {
            app.handle(Event::Net(done));
        }
        for r in app.take_requests() {
            match r {
                Request::Brightness(pct) => backlight = pct,
                Request::FlashBacklight => flash_until = Some(Instant::now() + Duration::from_secs(1)),
                Request::SaveSettings(s) => {
                    let _ = std::fs::write(SETTINGS, serde_json::to_vec_pretty(&s).unwrap());
                }
                Request::Net(job) => {
                    let _ = job_tx.send(job);
                }
            }
        }
        if app.dirty() || force {
            force = false;
            terminal.draw(|f| {
                let a = f.area();
                // Centre the screen in a bezel, with a status line underneath
                let w = COLS + 2;
                let h = ROWS + 2;
                let bezel = Rect::new(a.x + a.width.saturating_sub(w) / 2, a.y + a.height.saturating_sub(h + 1) / 2, w, h)
                    .intersection(a);
                f.render_widget(
                    Block::bordered().border_type(BorderType::Thick).border_style(Style::new().fg(Color::DarkGray))
                        .title(" pico-thing "),
                    bezel,
                );
                origin = (bezel.x + 1, bezel.y + 1);
                app.draw(f, origin);
                let flashing = flash_until.is_some_and(|t| Instant::now() < t);
                let status = format!(
                    "click = tap · q quits · backlight {backlight}%{}",
                    if flashing { " · ✦ flashing ✦" } else { "" }
                );
                f.render_widget(
                    Line::styled(status, Style::new().fg(Color::DarkGray)).centered(),
                    Rect::new(a.x, (bezel.y + bezel.height).min(a.height.saturating_sub(1)), a.width, 1),
                );
            })?;
        }
        if event::poll(Duration::from_millis(15))? {
            match event::read()? {
                CEvent::Key(k) if matches!(k.code, KeyCode::Char('q') | KeyCode::Esc) => return Ok(()),
                CEvent::Mouse(m) if m.kind == MouseEventKind::Down(MouseButton::Left) => {
                    if app.wants_raw_touch() {
                        // No touch panel here: calibrate with pixel positions
                        let (x, y) = (m.column.saturating_sub(origin.0), m.row.saturating_sub(origin.1));
                        app.handle(Event::RawTouch { x: x * 10 + 5, y: y * 20 + 10 });
                    } else {
                        app.handle(Event::Tap { col: m.column, row: m.row });
                    }
                }
                CEvent::Mouse(m) if m.kind == MouseEventKind::Up(MouseButton::Left) => app.handle(Event::Release),
                CEvent::Mouse(m) if m.kind == MouseEventKind::Drag(MouseButton::Left) => {
                    app.handle(Event::Drag { col: m.column, row: m.row });
                }
                CEvent::Resize(..) => force = true,
                _ => {}
            }
        }
    }
}

/// `cargo run -- --check`: run each network job once against the real services.
fn check() -> io::Result<()> {
    use thing::net::services::{Job, run};
    let mut n = net::StdNet;
    let jobs = [
        Job::SyncTime,
        Job::Weather { place: None, fahrenheit: thing::config::FAHRENHEIT },
        Job::CompanionStatus,
        Job::Todos,
        Job::Task(0),
        // Read-only: nothing gets switched or played
        Job::Light { ip: "192.168.1.13".into(), transport: thing::net::yeelight::Transport::Yeelight, cmd: None, was_on: false },
        Job::Sonos { ip: "192.168.1.5".into(), cmd: None, expect: String::new() },
        Job::Sonos { ip: "192.168.1.7".into(), cmd: None, expect: String::new() },
        Job::Favorites { ip: "192.168.1.7".into() },
    ];
    for job in jobs {
        println!("{job:?}\n  -> {:?}", embassy_futures::block_on(run(&mut n, job.clone())));
    }
    Ok(())
}
