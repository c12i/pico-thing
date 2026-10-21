//! The app: state, the current screen, and how events and taps change them.
//!
//! Platforms (the Pico firmware, the terminal simulator) drive it:
//! - `handle(Event)` for taps, time and network results
//! - `draw(frame)` whenever `dirty()` says something changed
//! - `take_requests()` for side effects to run (brightness, saving settings,
//!   network calls), whose results come back as events

use alloc::string::String;
use alloc::vec::Vec;
use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::widgets::Block;
use serde::{Deserialize, Serialize};

use crate::clock::DateTime;
use crate::config;
use crate::net::companion::{self, TaskDetail};
use crate::net::services::{Done, Job};
use crate::net::sonos::{Favorite, SonosCmd, SpeakerState};
use crate::net::yeelight::{LightCmd, LightState, Transport};
use crate::net::NetError;
use crate::net::weather::Weather;
use crate::faces::{self, FaceKind, Faces};
use crate::pomodoro::{PomoState, Pomodoro};
use crate::screens;
use crate::touch::{self, Calib};
use crate::targets::Targets;
use crate::theme::{DARK, LIGHT, Theme};
use crate::{COLS, ROWS};

/// Settings that survive a restart (the platform stores them).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Settings {
    pub face: FaceKind,
    pub dark: bool,
    pub brightness: u8,
    /// Affine touch calibration (see `touch`); None = `touch::DEFAULT`
    pub calib: Option<Calib>,
    pub pomo_preset: u8,
    /// (day of month, focus sessions done that day)
    pub pomo_today: (u8, u32),
    /// The pet face's pet
    pub pet: crate::pet::Pet,
}

impl Default for Settings {
    fn default() -> Self {
        Self { face: FaceKind::Digital, dark: true, brightness: 100, calib: None, pomo_preset: 0, pomo_today: (0, 0), pet: Default::default() }
    }
}

impl Settings {
    pub fn calib(&self) -> Calib {
        self.calib.unwrap_or(touch::DEFAULT)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Screen {
    /// The boot log, until the network jobs have reported in
    Boot,
    Face,
    Menu,
    FacePicker,
    Settings,
    Pomodoro,
    Calibrate,
    Command,
    Lights,
    Sonos,
    Favorites(usize),
    /// A saved agent task's output
    TaskResult(usize),
    /// A Claude Code session needs you (or finished)
    Attention,
}

/// Everything a tap can do.
#[derive(Clone, Debug, PartialEq)]
pub enum Action {
    Menu,
    Home,
    Open(Screen),
    SetFace(FaceKind),
    SetDark(bool),
    Brightness(u8),
    RefreshWeather,
    SyncTime,
    PomoMain,
    PomoSkip,
    PomoReset,
    PomoPreset,
    CommandTab(u8),
    /// Bring a Claude Code session's terminal to the front
    FocusSession(String),
    /// Run a saved task (or show its result if it has one)
    OpenTask(usize),
    RunTask(usize),
    MacAction(String),
    Dismiss,
    /// Scroll a long list to row `.0` of a `.1`-row scrollbar
    ScrollTo(u16, u16),
    /// Hourglass face: run for this many seconds (from full)
    HourglassSet(u32),
    /// Hourglass face: turn it over
    HourglassFlip,
    Pet(crate::pet::PetCmd),
    /// Scroll a long list by this many items (the scrollbar's arrows)
    ScrollBy(i16),
    Light(usize, LightCmd),
    SelectLight(usize),
    SelectSpeaker(usize),
    AllLightsOff,
    Speaker(usize, SonosCmd),
    /// Play favorite (speaker, favorite index in the list)
    PlayFavorite(usize, usize),
}

/// What the platform tells the app.
#[derive(Clone, Debug)]
pub enum Event {
    /// A finger came down, in the same frame coordinates `draw` was given
    Tap { col: u16, row: u16 },
    /// The finger lifted (targets that can be held wait for this)
    Release,
    /// The finger moved while held down (for dragging scrollbars)
    Drag { col: u16, row: u16 },
    /// A raw touch reading, sent instead of taps while `wants_raw_touch()`
    RawTouch { x: u16, y: u16 },
    /// Called every loop with milliseconds since boot
    Tick { uptime_ms: u64 },
    /// The wall clock, from SNTP (unix seconds, UTC)
    Clock { unix: i64 },
    /// Platform details for the settings screen
    Status(Status),
    /// A network job finished
    Net(Done),
    /// Wi-Fi came up (with our address) or went down
    Online(Option<[u8; 4]>),
    /// The Mac companion says something changed (POST /poke)
    Poke,
    /// A boot step: a new line, or an update to the line with the same key
    Boot { key: &'static str, text: String, ok: Option<bool> },
}

/// A line of the boot log; `ok` is None while the step is still running.
#[derive(Clone, Debug)]
pub struct BootLine {
    pub key: &'static str,
    pub text: String,
    pub ok: Option<bool>,
}

#[derive(Clone, Debug, Default)]
pub struct Status {
    pub ip: Option<[u8; 4]>,
    pub heap_free_kb: u32,
    pub heap_total_kb: u32,
}

/// Side effects for the platform to carry out.
#[derive(Clone, Debug, PartialEq)]
pub enum Request {
    Brightness(u8),
    /// Blink the backlight to get attention (a Pomodoro phase ended)
    FlashBacklight,
    SaveSettings(Settings),
    /// Run on the network task; the result comes back as `Event::Net`
    Net(Job),
}

/// State shared by every screen.
pub struct State {
    pub settings: Settings,
    pub theme: Theme,
    pub uptime_ms: u64,
    /// (unix seconds, uptime ms) at the last clock sync
    pub clock: Option<(i64, u64)>,
    /// Seconds east of UTC (from the weather service's location)
    pub utc_offset: i32,
    pub status: Status,
    pub weather: Option<Weather>,
    /// Local "HH:MM" of the last weather update
    pub weather_at: Option<DateTime>,
    pub faces: Faces,
    /// Open TODOs from the Mac companion (None until the first answer)
    pub todos: Option<Vec<String>>,
    pub todos_at_ms: u64,
    /// The robot celebrates until then (something got ticked off)
    pub excited_until_ms: u64,
    pub companion_offline: bool,
    pub pomodoro: Pomodoro,
    pub calib_samples: Vec<(u16, u16)>,
    pub calib_message: Option<&'static str>,
    pub companion: Option<companion::Status>,
    pub companion_error: Option<NetError>,
    pub command_tab: u8,
    pub task: Option<(usize, Result<TaskDetail, NetError>)>,
    /// How far the current list is scrolled (items or lines), and how far it can go
    /// (set by the screen while drawing)
    pub scroll: usize,
    pub scroll_max: usize,
    /// The session the attention screen is about, and since when
    pub attention: Option<(String, u64)>,
    pub lights: Vec<LightEntry>,
    pub speakers: Vec<SpeakerEntry>,
    /// Sonos Favorites (fetched from the first speaker that's asked)
    pub favorites: Option<Result<Vec<Favorite>, NetError>>,
    /// The Mac's vitals (for the system face)
    pub mac_stats: Option<Result<companion::MacStats, NetError>>,
    stats_pending: bool,
    pub light_sel: usize,
    pub speaker_sel: usize,
    /// Network jobs sent and not yet answered (the spinner turns while > 0)
    pub in_flight: u32,
    requests: Vec<Request>,
    /// The boot log, and whether it's still taking lines
    pub boot: Vec<BootLine>,
    pub booting: bool,
    companion_job: Timer,
    todos_job: Timer,
    todos_rev: Option<u32>,
    time_job: Timer,
    weather_job: Timer,
}

pub struct LightEntry {
    pub ip: &'static str,
    pub name: &'static str,
    /// None: a miIO light without a token in config
    pub transport: Option<Transport>,
    pub state: Option<Result<LightState, NetError>>,
    pub busy: bool,
}

/// Give up waiting on the network after this long and show the face anyway.
const BOOT_WAIT_MS: u64 = 30_000;
/// How long the finished boot log stays up.
const BOOT_HOLD_MS: u64 = 1_200;

pub struct SpeakerEntry {
    pub name: &'static str,
    pub ip: &'static str,
    pub state: Option<Result<SpeakerState, NetError>>,
    pub busy: bool,
}

/// Lights and speakers come from config (Yeelights by name, miIO lights by token).
fn lights_from_config() -> Vec<LightEntry> {
    let mut v: Vec<LightEntry> = config::LIGHT_NAMES
        .iter()
        .map(|(ip, name)| LightEntry {
            ip,
            name,
            transport: match config::MIIO_TOKENS.iter().find(|(i, _)| i == ip) {
                Some((_, tok)) => crate::net::miio::parse_token(tok).map(Transport::Miio),
                None => Some(Transport::Yeelight),
            },
            state: None,
            busy: false,
        })
        .collect();
    for (ip, tok) in config::MIIO_TOKENS {
        if !v.iter().any(|l| l.ip == *ip) {
            v.push(LightEntry { ip, name: ip, transport: crate::net::miio::parse_token(tok).map(Transport::Miio), state: None, busy: false });
        }
    }
    v
}

/// When a periodic network job is next due, and whether one is running.
#[derive(Default)]
struct Timer {
    due_ms: u64,
    running: bool,
}

impl Timer {
    fn take_if_due(&mut self, now: u64) -> bool {
        if self.running || now < self.due_ms {
            return false;
        }
        self.running = true;
        true
    }

    fn done(&mut self, now: u64, ok: bool, every_ms: u64, retry_ms: u64) {
        self.running = false;
        self.due_ms = now + if ok { every_ms } else { retry_ms };
    }
}

const TIME_EVERY: u64 = 6 * 3600 * 1000;
const TIME_RETRY: u64 = 30_000;
const WEATHER_EVERY: u64 = 30 * 60 * 1000;
const WEATHER_RETRY: u64 = 60_000;
const COMPANION_EVERY: u64 = 30_000;
const TODOS_EVERY: u64 = 5 * 60 * 1000;
const TODOS_RETRY: u64 = 60_000;
/// The robot celebrates this long after something gets ticked off
const EXCITED_MS: u64 = 60_000;
/// Attention alerts close themselves (and count as seen) after this long
pub const ATTENTION_MS: u64 = 15_000;

impl State {
    /// Local time, once the clock has been set.
    /// Add a line to the boot log, or update the one with this key.
    pub fn boot_step(&mut self, key: &'static str, text: String, ok: Option<bool>) {
        if !self.booting {
            return;
        }
        match self.boot.iter_mut().find(|l| l.key == key) {
            Some(l) => {
                l.text = text;
                l.ok = ok;
            }
            None => self.boot.push(BootLine { key, text, ok }),
        }
    }

    /// True once the time and weather (and the companion, if there is one) have
    /// answered, well or not.
    fn boot_finished(&self) -> bool {
        let done = |key| self.boot.iter().any(|l: &BootLine| l.key == key && l.ok.is_some());
        done("time") && done("weather") && (!companion::configured() || done("companion"))
    }

    /// Unix seconds, once the clock is set.
    pub fn unix(&self) -> Option<i64> {
        let (unix, at) = self.clock?;
        Some(unix + ((self.uptime_ms - at) / 1000) as i64)
    }

    pub fn now(&self) -> Option<DateTime> {
        let (unix, at) = self.clock?;
        let secs = unix + ((self.uptime_ms - at) / 1000) as i64 + self.utc_offset as i64;
        Some(DateTime::from_unix(secs))
    }

    pub fn request(&mut self, r: Request) {
        if matches!(r, Request::Net(_)) {
            if !self.online() {
                return; // nothing to send it over
            }
            self.in_flight += 1;
        }
        self.requests.push(r);
    }

    /// True while any network job is out.
    pub fn busy(&self) -> bool {
        self.in_flight > 0
    }

    pub fn online(&self) -> bool {
        self.status.ip.is_some()
    }

    /// Start periodic network jobs that are due.
    fn schedule_jobs(&mut self) {
        if !self.online() {
            return;
        }
        let now = self.uptime_ms;
        if self.time_job.take_if_due(now) {
            self.request(Request::Net(Job::SyncTime));
        }
        if self.weather_job.take_if_due(now) {
            let place = match (&self.weather, config::LATITUDE, config::LONGITUDE) {
                (_, Some(lat), Some(lon)) => Some((lat, lon, config::CITY.into())),
                (Some(w), ..) => Some((w.lat, w.lon, w.city.clone())),
                _ => None,
            };
            self.request(Request::Net(Job::Weather { place, fahrenheit: config::FAHRENHEIT }));
        }
        if companion::configured() {
            if self.companion_job.take_if_due(now) {
                self.request(Request::Net(Job::CompanionStatus));
            }
            if self.todos_job.take_if_due(now) {
                self.request(Request::Net(Job::Todos));
            }
        }
    }

    /// Ask the companion for its status now (after a poke, or opening Command).
    pub fn refresh_companion(&mut self) {
        if !self.companion_job.running {
            self.companion_job.due_ms = 0;
        }
    }

    pub fn companion_post(&mut self, path: String) {
        self.request(Request::Net(Job::CompanionPost(path)));
    }

    /// Ask for a job now (Settings' "Weather" and "Sync time" buttons).
    pub fn refresh(&mut self, weather: bool) {
        let t = if weather { &mut self.weather_job } else { &mut self.time_job };
        if !t.running {
            t.due_ms = 0;
        }
    }

    fn finished(&mut self, done: Done) {
        let now = self.uptime_ms;
        if self.booting {
            self.log_boot_result(&done);
        }
        match done {
            Done::Time(r) => {
                if let Ok(unix) = r {
                    self.clock = Some((unix, now));
                }
                self.time_job.done(now, r.is_ok(), TIME_EVERY, TIME_RETRY);
            }
            Done::Weather(r) => {
                let ok = r.is_ok();
                if let Ok(mut w) = r {
                    if !config::CITY.is_empty() {
                        w.city = config::CITY.into();
                    }
                    self.utc_offset = w.utc_offset;
                    self.weather = Some(w);
                    self.weather_at = self.now();
                }
                self.weather_job.done(now, ok, WEATHER_EVERY, WEATHER_RETRY);
            }
            Done::Companion(r) => {
                // (Posts come back this way too; their timer may not be running)
                self.companion_job.done(now, r.is_ok(), COMPANION_EVERY, COMPANION_EVERY);
                match r {
                    Ok(st) => {
                        if self.todos_rev.is_some_and(|rev| rev != st.todos_rev) && !self.todos_job.running {
                            self.todos_job.due_ms = 0; // the TODO file changed
                        }
                        self.todos_rev = Some(st.todos_rev);
                        self.companion = Some(st);
                        self.companion_error = None;
                        self.companion_offline = false;
                    }
                    Err(e) => {
                        self.companion_error = Some(e);
                        self.companion_offline = true;
                    }
                }
            }
            Done::Todos(r) => {
                self.todos_job.done(now, r.is_ok(), TODOS_EVERY, TODOS_RETRY);
                if let Ok(items) = r {
                    if self.todos.as_ref().is_some_and(|old| items.len() < old.len()) {
                        self.excited_until_ms = now + EXCITED_MS;
                    }
                    if self.todos.as_ref() != Some(&items) {
                        self.todos = Some(items);
                        self.todos_at_ms = now;
                    }
                }
            }
            Done::Task(n, r) => self.task = Some((n, r)),
            Done::Light(ip, r) => {
                if let Some(l) = self.lights.iter_mut().find(|l| l.ip == ip) {
                    l.state = Some(r);
                    l.busy = false;
                }
            }
            Done::Sonos(ip, r) => {
                if let Some(s) = self.speakers.iter_mut().find(|s| s.ip == ip) {
                    s.state = Some(r);
                    s.busy = false;
                }
            }
            Done::Favorites(_, r) => self.favorites = Some(r),
            Done::MacStats(r) => {
                self.stats_pending = false;
                self.mac_stats = Some(r);
            }
        }
    }

    fn log_boot_result(&mut self, done: &Done) {
        match done {
            Done::Time(r) => {
                let text = if r.is_ok() { "time      synced (sntp)" } else { "time      sntp failed, retrying later" };
                self.boot_step("time", text.into(), Some(r.is_ok()));
            }
            Done::Weather(r) => {
                let text = match r {
                    Ok(w) => {
                        let city = if config::CITY.is_empty() { w.city.as_str() } else { config::CITY };
                        alloc::format!("weather   {:.0}° {}, {city}", w.temp, w.label().to_lowercase())
                    }
                    Err(_) => "weather   unavailable, retrying later".into(),
                };
                self.boot_step("weather", text, Some(r.is_ok()));
            }
            Done::Companion(r) => {
                let text = if r.is_ok() { "companion mac reachable" } else { "companion mac not answering" };
                self.boot_step("companion", text.into(), Some(r.is_ok()));
            }
            _ => {}
        }
    }

    pub fn light_job(&mut self, i: usize, cmd: Option<LightCmd>) {
        if !self.online() {
            return;
        }
        let Some(l) = self.lights.get_mut(i) else { return };
        let Some(transport) = l.transport.clone() else { return };
        let was_on = matches!(&l.state, Some(Ok(s)) if s.power);
        l.busy = true;
        let job = Job::Light { ip: l.ip.into(), transport, cmd, was_on };
        self.request(Request::Net(job));
    }

    pub fn speaker_job(&mut self, i: usize, cmd: Option<SonosCmd>, expect: String) {
        if !self.online() {
            return;
        }
        let Some(s) = self.speakers.get_mut(i) else { return };
        s.busy = true;
        let job = Job::Sonos { ip: s.ip.into(), cmd, expect };
        self.request(Request::Net(job));
    }

    pub fn session(&self, id: &str) -> Option<&companion::Session> {
        self.companion.as_ref()?.sessions.iter().find(|s| s.id == id)
    }

    fn save(&mut self) {
        let p = &self.pomodoro;
        self.settings.pomo_preset = p.preset as u8;
        self.settings.pomo_today = p.today;
        let s = self.settings.clone();
        self.request(Request::SaveSettings(s));
    }
}

/// Screens other than faces fall back to the home face after this long untouched
/// (longer when the Pomodoro screen popped up to say a phase ended).
const IDLE_MS: u64 = 30_000;
const ALERT_IDLE_MS: u64 = 90_000;

pub struct App {
    pub state: State,
    pub screen: Screen,
    targets: Targets,
    dirty: bool,
    last_input_ms: u64,
    last_second: u64,
    alert: bool,
    /// The target being flashed after a tap: (area, action, when)
    pressed: Option<Press>,
    spin_step: u64,
    /// When the boot log finished (it stays up a moment before the face)
    boot_done_ms: Option<u64>,
}

/// How long a tapped target shows in reverse video before its action runs.
const PRESS_MS: u64 = 120;
/// Holding a target with a long-press action this long runs that instead.
const LONG_MS: u64 = 600;

/// A target being pressed: it shows in reverse video until its action runs.
struct Press {
    area: Rect,
    action: Action,
    long: Option<Action>,
    at: u64,
    /// The finger is still down (only tracked for targets with a long action)
    held: bool,
}

impl App {
    pub fn new(settings: Settings, seed: u32) -> Self {
        let theme = if settings.dark { DARK } else { LIGHT };
        let pomodoro = Pomodoro::new(settings.pomo_preset as usize, settings.pomo_today);
        Self {
            state: State {
                settings,
                theme,
                uptime_ms: 0,
                clock: None,
                utc_offset: 0,
                status: Status::default(),
                weather: None,
                weather_at: None,
                faces: Faces::new(seed),
                todos: None,
                todos_at_ms: 0,
                excited_until_ms: 0,
                companion_offline: false,
                companion: None,
                companion_error: None,
                command_tab: 0,
                task: None,
                scroll: 0,
                scroll_max: 0,
                attention: None,
                lights: lights_from_config(),
                speakers: config::SONOS
                    .iter()
                    .map(|(name, ip)| SpeakerEntry { name, ip, state: None, busy: false })
                    .collect(),
                favorites: None,
                mac_stats: None,
                stats_pending: false,
                light_sel: 0,
                speaker_sel: 0,
                in_flight: 0,
                companion_job: Timer::default(),
                todos_job: Timer::default(),
                todos_rev: None,
                pomodoro,
                calib_samples: Vec::new(),
                calib_message: None,
                requests: Vec::new(),
                boot: Vec::new(),
                booting: true,
                time_job: Timer::default(),
                weather_job: Timer::default(),
            },
            screen: Screen::Boot,
            targets: Targets::default(),
            dirty: true,
            last_input_ms: 0,
            last_second: u64::MAX,
            alert: false,
            pressed: None,
            spin_step: 0,
            boot_done_ms: None,
        }
    }

    /// Draw every screen once to `term` (an off-screen terminal) so ratatui's layout
    /// cache is warm: solving layouts is slow on a chip without floating point
    /// hardware, and this moves that cost to boot instead of each first visit.
    pub fn warm_up<B: ratatui::backend::Backend>(&mut self, term: &mut ratatui::Terminal<B>) {
        let (screen, face) = (self.screen, self.state.settings.face);
        let screens = alloc::vec![
            Screen::Menu, Screen::FacePicker, Screen::Settings, Screen::Pomodoro, Screen::Command,
            Screen::Lights, Screen::Sonos,
        ];
        for s in screens {
            self.screen = s;
            let _ = term.draw(|f| self.draw(f, (0, 0)));
        }
        self.screen = Screen::Face;
        for f in FaceKind::ALL {
            self.state.settings.face = f;
            let _ = term.draw(|fr| self.draw(fr, (0, 0)));
        }
        self.state.settings.face = face;
        self.screen = screen;
        self.dirty = true;
    }

    /// While true the platform should send `RawTouch` instead of `Tap`.
    pub fn wants_raw_touch(&self) -> bool {
        self.screen == Screen::Calibrate
    }

    /// Start touch calibration (e.g. a finger held on the screen at boot).
    pub fn calibrate(&mut self) {
        self.state.calib_samples.clear();
        self.state.calib_message = None;
        self.go(Screen::Calibrate);
    }

    pub fn dirty(&self) -> bool {
        self.dirty
    }

    pub fn take_requests(&mut self) -> Vec<Request> {
        core::mem::take(&mut self.state.requests)
    }

    pub fn handle(&mut self, event: Event) {
        let st = &mut self.state;
        match event {
            Event::Tap { col, row } => {
                self.last_input_ms = st.uptime_ms;
                if self.pressed.is_some() {
                    return; // still showing the last press
                }
                if let Some(hit) = self.targets.hit(col, row) {
                    if matches!(hit.action, Action::ScrollTo(..)) {
                        self.act(hit.action); // scrolling answers at once
                        return;
                    }
                    // Flash what was hit first; the action runs on a later tick (or
                    // on release, for targets that can be held)
                    let held = hit.long.is_some();
                    self.pressed = Some(Press { area: hit.area, action: hit.action, long: hit.long, at: st.uptime_ms, held });
                    self.dirty = true;
                }
            }
            Event::Release => {
                if let Some(p) = &mut self.pressed {
                    p.held = false;
                }
            }
            Event::Drag { col, row } => {
                self.last_input_ms = st.uptime_ms;
                if let Some(hit) = self.targets.hit(col, row).filter(|h| matches!(h.action, Action::ScrollTo(..))) {
                    self.act(hit.action);
                }
            }
            Event::RawTouch { x, y } => self.calibration_sample(x, y),
            Event::Tick { uptime_ms } => {
                st.uptime_ms = uptime_ms;
                let sec = uptime_ms / 1000;
                if sec != self.last_second {
                    self.last_second = sec;
                    self.dirty = true; // clocks
                    self.every_second(sec);
                }
                if let Some(p) = &self.pressed {
                    let held_ms = uptime_ms - p.at;
                    if p.held && held_ms >= LONG_MS {
                        let p = self.pressed.take().unwrap();
                        self.dirty = true;
                        self.act(p.long.unwrap());
                    } else if !p.held && held_ms >= PRESS_MS {
                        let p = self.pressed.take().unwrap();
                        self.act(p.action);
                    }
                }
                let st = &mut self.state;
                // The spinner turns while the network is busy
                let step = uptime_ms / 100;
                if (st.busy() || self.screen == Screen::Boot) && step != self.spin_step {
                    self.spin_step = step;
                    self.dirty = true;
                }
                st.schedule_jobs();
                let idle = if self.alert { ALERT_IDLE_MS } else { IDLE_MS };
                match self.screen {
                    Screen::Boot => self.boot_tick(uptime_ms),
                    Screen::Face => self.dirty |= faces::tick(st, uptime_ms),
                    Screen::Attention | Screen::Calibrate => {} // these never time out
                    _ if uptime_ms - self.last_input_ms > idle => self.go(Screen::Face),
                    _ => {}
                }
            }
            Event::Net(done) => {
                st.in_flight = st.in_flight.saturating_sub(1);
                st.finished(done);
                self.dirty = true;
                self.check_attention();
            }
            Event::Poke => st.refresh_companion(),
            Event::Online(ip) => {
                st.status.ip = ip;
                self.dirty = true;
                if let Some([a, b, c, d]) = ip {
                    st.boot_step("dhcp", alloc::format!("dhcp      {a}.{b}.{c}.{d}"), Some(true));
                    st.boot_step("time", "time      syncing (sntp)".into(), None);
                    st.boot_step("weather", "weather   fetching (open-meteo)".into(), None);
                    if companion::configured() {
                        st.boot_step("companion", "companion asking the mac".into(), None);
                    }
                }
            }
            Event::Boot { key, text, ok } => {
                st.boot_step(key, text, ok);
                self.dirty = true;
            }
            Event::Clock { unix } => {
                st.clock = Some((unix, st.uptime_ms));
                self.dirty = true;
            }
            Event::Status(s) => {
                st.status = Status { ip: st.status.ip, ..s };
                self.dirty |= self.screen == Screen::Settings;
            }
        }
    }

    /// Leave the boot log once the network has reported in (or it's taking too long).
    fn boot_tick(&mut self, uptime_ms: u64) {
        let st = &mut self.state;
        match self.boot_done_ms {
            None if st.boot_finished() || uptime_ms > BOOT_WAIT_MS => {
                let text = if st.online() { "ready" } else { "ready (offline, will keep trying)" };
                st.boot_step("ready", text.into(), Some(true));
                self.boot_done_ms = Some(uptime_ms);
                self.dirty = true;
            }
            Some(at) if uptime_ms - at >= BOOT_HOLD_MS => self.go(Screen::Face),
            _ => {}
        }
    }

    fn go(&mut self, screen: Screen) {
        if screen != Screen::Boot {
            self.state.booting = false;
            self.state.boot = Vec::new();
        }
        self.screen = screen;
        self.alert = false;
        self.dirty = true;
        let st = &mut self.state;
        match screen {
            Screen::Command => st.refresh_companion(),
            Screen::Menu => {
                for i in 0..st.lights.len() {
                    st.light_job(i, None);
                }
                for i in 0..st.speakers.len() {
                    st.speaker_job(i, None, String::new());
                }
            }
            Screen::TaskResult(n) => {
                st.scroll = 0;
                st.request(Request::Net(Job::Task(n)));
            }
            Screen::Lights => {
                for i in 0..st.lights.len() {
                    st.light_job(i, None);
                }
            }
            Screen::Sonos => {
                for i in 0..st.speakers.len() {
                    st.speaker_job(i, None, String::new());
                }
            }
            Screen::FacePicker => st.scroll = 0,
            Screen::Favorites(i) => {
                st.scroll = 0;
                if !matches!(st.favorites, Some(Ok(_))) {
                    st.favorites = None;
                    if let Some(sp) = st.speakers.get(i) {
                        let job = Job::Favorites { ip: sp.ip.into() };
                        st.request(Request::Net(job));
                    }
                }
            }
            _ => {}
        }
    }

    fn every_second(&mut self, sec: u64) {
        let st = &mut self.state;
        if let Some(now) = st.unix() {
            st.settings.pet.advance(now);
        }
        // The system face polls the Mac while it's showing
        let on_system = self.screen == Screen::Face && st.settings.face == FaceKind::System;
        if on_system && st.online() && companion::configured() && sec % 3 == 0 && !st.stats_pending {
            st.stats_pending = true;
            st.request(Request::Net(Job::MacStats));
        }
        let day = st.now().map(|n| n.day);
        if st.pomodoro.tick(st.uptime_ms, day) {
            self.pomodoro_finished();
        }
        match self.screen {
            Screen::Attention => self.attention_tick(),
            Screen::TaskResult(n) => {
                // Poll a running task every few seconds
                let st = &mut self.state;
                let running = matches!(&st.task, Some((i, Ok(d))) if *i == n && d.state == "running");
                if running && sec % 5 == 0 {
                    st.request(Request::Net(Job::Task(n)));
                }
            }
            _ => {}
        }
    }

    /// A session needs you: take over the home face.
    fn check_attention(&mut self) {
        let Some(id) = self.state.companion.as_ref().and_then(|c| c.attention.clone()) else { return };
        if self.screen == Screen::Face {
            self.state.attention = Some((id, self.state.uptime_ms));
            self.go(Screen::Attention);
        } else if self.screen == Screen::Attention {
            // A new event for the same alert gets the full time again
            if let Some((cur, at)) = &mut self.state.attention {
                if *cur != id {
                    *cur = id;
                    *at = self.state.uptime_ms;
                }
            }
        }
    }

    fn attention_tick(&mut self) {
        let st = &mut self.state;
        let Some((id, at)) = st.attention.clone() else { return self.go(Screen::Face) };
        let live = st.session(&id).is_some_and(|s| s.state == "waiting" || s.state == "done");
        if !live {
            return self.go(Screen::Face); // handled at the terminal already
        }
        if st.uptime_ms - at >= ATTENTION_MS {
            // Mark it seen so it doesn't pop up again
            st.companion_post(alloc::format!("/dismiss?id={}", companion::encode(&id)));
            self.go(Screen::Face);
        }
    }

    fn pomodoro_finished(&mut self) {
        self.state.request(Request::FlashBacklight);
        if companion::configured() {
            let msg = match self.state.pomodoro.finished {
                Some(crate::pomodoro::Phase::Focus) => "Focus done - take a break",
                _ => "Break over - back to it",
            };
            self.state.companion_post(alloc::format!("/notify?text={}", companion::encode(msg)));
        }
        self.state.save(); // today's count
        if self.screen != Screen::Calibrate {
            self.go(Screen::Pomodoro);
            self.alert = true;
            self.last_input_ms = self.state.uptime_ms;
        }
    }

    fn calibration_sample(&mut self, x: u16, y: u16) {
        use screens::calibrate::{CELLS, MAX_ERR, target_px};
        let st = &mut self.state;
        st.calib_samples.push((x, y));
        st.calib_message = None;
        self.dirty = true;
        if st.calib_samples.len() < CELLS.len() {
            return;
        }
        let targets: Vec<(i32, i32)> = (0..CELLS.len()).map(target_px).collect();
        let (cal, err) = touch::fit(&st.calib_samples, &targets);
        st.calib_samples.clear();
        if err > MAX_ERR {
            st.calib_message = Some("Missed a target - once more");
            return;
        }
        st.settings.calib = Some(cal);
        st.save();
        self.go(Screen::Face);
    }

    fn act(&mut self, action: Action) {
        let st = &mut self.state;
        self.dirty = true;
        match action {
            Action::Menu => self.go(Screen::Menu),
            Action::Home => self.go(Screen::Face),
            Action::Open(Screen::Calibrate) => self.calibrate(),
            Action::Open(s) => self.go(s),
            Action::HourglassSet(secs) => st.faces.hourglass.set(secs, st.uptime_ms),
            Action::HourglassFlip => st.faces.hourglass.flip(st.uptime_ms),
            Action::Pet(cmd) => {
                let Some(now) = st.unix() else { return }; // it lives on wall-clock time
                if st.settings.pet.apply(cmd, now) {
                    st.faces.pet_reaction = Some((cmd, st.uptime_ms));
                    st.save();
                }
            }
            Action::SetFace(f) => {
                st.settings.face = f;
                st.save();
                self.go(Screen::Face);
            }
            Action::SetDark(dark) => {
                st.settings.dark = dark;
                st.theme = if st.settings.dark { DARK } else { LIGHT };
                st.save();
            }
            Action::PomoMain => {
                let now = st.uptime_ms;
                let p = &mut st.pomodoro;
                if p.state == PomoState::Running { p.pause(now) } else { p.start(now) }
                self.alert = false;
            }
            Action::PomoSkip => st.pomodoro.skip(),
            Action::PomoReset => st.pomodoro.reset(),
            Action::PomoPreset => {
                if st.pomodoro.state != PomoState::Running {
                    st.pomodoro.next_preset();
                    st.save();
                }
            }
            Action::CommandTab(i) => st.command_tab = i,
            Action::FocusSession(id) => {
                st.companion_post(alloc::format!("/focus?id={}", companion::encode(&id)));
                if self.screen == Screen::Attention {
                    self.go(Screen::Face);
                }
            }
            Action::Dismiss => {
                if let Some((id, _)) = st.attention.take() {
                    st.companion_post(alloc::format!("/dismiss?id={}", companion::encode(&id)));
                }
                self.go(Screen::Face);
            }
            Action::OpenTask(n) => {
                let idle = st.companion.as_ref().and_then(|c| c.tasks.get(n)).is_none_or(|t| t.state == "idle");
                if idle {
                    st.companion_post(alloc::format!("/run?n={n}"));
                }
                self.go(Screen::TaskResult(n));
            }
            Action::RunTask(n) => {
                st.companion_post(alloc::format!("/run?n={n}"));
                st.task = None;
                st.scroll = 0;
                st.request(Request::Net(Job::Task(n)));
            }
            Action::MacAction(id) => st.companion_post(alloc::format!("/action?id={}", companion::encode(&id))),
            Action::ScrollTo(r, n) => {
                st.scroll = if n > 1 { (r as usize * st.scroll_max + (n as usize - 1) / 2) / (n as usize - 1) } else { 0 };
            }
            Action::ScrollBy(d) => {
                st.scroll = (st.scroll as isize + d as isize).clamp(0, st.scroll_max as isize) as usize;
            }
            Action::Light(i, cmd) => st.light_job(i, Some(cmd)),
            Action::SelectLight(i) => st.light_sel = i,
            Action::SelectSpeaker(i) => {
                st.speaker_sel = i;
                st.speaker_job(i, None, String::new());
            }
            Action::AllLightsOff => {
                for i in 0..st.lights.len() {
                    st.light_job(i, Some(LightCmd::Power(false)));
                }
            }
            Action::Speaker(i, cmd) => st.speaker_job(i, Some(cmd), String::new()),
            Action::PlayFavorite(i, k) => {
                let fav = match &st.favorites {
                    Some(Ok(f)) => f.get(k).cloned(),
                    _ => None,
                };
                if let Some((index, title, _)) = fav {
                    st.speaker_job(i, Some(SonosCmd::Favorite(index)), title);
                    st.speaker_sel = i;
                    self.go(Screen::Sonos);
                    return;
                }
            }
            Action::RefreshWeather => st.refresh(true),
            Action::SyncTime => st.refresh(false),
            Action::Brightness(pct) => {
                st.settings.brightness = pct;
                st.request(Request::Brightness(pct));
                st.save();
            }
        }
    }

    /// Draw the current screen into the 48x16 cells at `origin` (taps use the same
    /// frame coordinates).
    pub fn draw(&mut self, f: &mut Frame, origin: (u16, u16)) {
        self.dirty = false;
        self.targets.clear();
        let a = f.area();
        let area = Rect::new(origin.0, origin.1, COLS, ROWS).intersection(a);
        let st = &mut self.state;
        f.render_widget(Block::new().style(ratatui::style::Style::new().bg(st.theme.bg).fg(st.theme.fg)), area);
        let t = &mut self.targets;
        match self.screen {
            Screen::Boot => screens::boot::draw(st, t, f, area),
            Screen::Face => faces::draw(st, t, f, area),
            Screen::Menu => screens::menu::draw(st, t, f, area),
            Screen::FacePicker => screens::face_picker::draw(st, t, f, area),
            Screen::Settings => screens::settings::draw(st, t, f, area),
            Screen::Pomodoro => screens::pomodoro::draw(st, t, f, area),
            Screen::Calibrate => screens::calibrate::draw(st, f, area),
            Screen::Command => screens::command::draw(st, t, f, area),
            Screen::TaskResult(n) => screens::command::draw_task(st, t, f, area, n),
            Screen::Attention => screens::command::draw_attention(st, t, f, area),
            Screen::Lights => screens::lights::draw(st, t, f, area),
            Screen::Sonos => screens::sonos::draw(st, t, f, area),
            Screen::Favorites(i) => screens::sonos::draw_favorites(st, t, f, area, i),
        }
        // Tap feedback: what was hit shows in reverse video until its action runs
        if let Some(p) = &self.pressed {
            crate::term::invert(f.buffer_mut(), p.area);
        }
    }
}
