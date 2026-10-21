//! Sample data for the examples (`dump`, `screenshots`): a morning in Nairobi with a
//! lamp on, music playing, two Claude Code sessions and a pet. Lights and speakers
//! come from config.toml, so their states are filled in by position.

use thing::app::{Screen, Status};
use thing::faces::FaceKind;
use thing::net::companion::MacStats;
use thing::net::services::Done;
use thing::net::sonos::SpeakerState;
use thing::net::weather::Weather;
use thing::net::yeelight::LightState;
use thing::pet::{Pet, PetCmd};
use thing::{App, Event, Settings};

/// 09:41 in Nairobi (UTC+3) on Wednesday 21 October 2026.
pub const UNIX: i64 = 1_792_564_860;
/// How long the demo app has been up when the screens are drawn.
pub const UPTIME_MS: u64 = 3_600_000;

/// Everything worth seeing, by file name: (name, screen, face for Screen::Face).
pub const SCREENS: &[(&str, Screen, Option<FaceKind>)] = &[
    ("boot", Screen::Boot, None),
    ("face-digital", Screen::Face, Some(FaceKind::Digital)),
    ("face-analog", Screen::Face, Some(FaceKind::Analog)),
    ("face-weather", Screen::Face, Some(FaceKind::Weather)),
    ("face-productivity", Screen::Face, Some(FaceKind::Productivity)),
    ("face-bonsai", Screen::Face, Some(FaceKind::Bonsai)),
    ("face-system", Screen::Face, Some(FaceKind::System)),
    ("face-hourglass", Screen::Face, Some(FaceKind::Hourglass)),
    ("face-pet", Screen::Face, Some(FaceKind::Pet)),
    ("menu", Screen::Menu, None),
    ("faces", Screen::FacePicker, None),
    ("lights", Screen::Lights, None),
    ("sonos", Screen::Sonos, None),
    ("favorites", Screen::Favorites(0), None),
    ("command", Screen::Command, None),
    ("pomodoro", Screen::Pomodoro, None),
    ("settings", Screen::Settings, None),
];

/// An app ready to show `screen` (and `face`), filled with the sample data.
pub fn app(screen: Screen, face: Option<FaceKind>) -> App {
    let mut app = App::new(Settings { face: face.unwrap_or(FaceKind::Digital), ..Settings::default() }, 7);
    if screen == Screen::Boot {
        boot(&mut app);
        return app;
    }
    app.handle(Event::Tick { uptime_ms: 0 });
    app.handle(Event::Online(Some([192, 168, 1, 15])));
    app.handle(Event::Status(Status { ip: None, heap_free_kb: 95, heap_total_kb: 160 }));
    app.handle(Event::Clock { unix: UNIX - (UPTIME_MS / 1000) as i64 });
    app.handle(Event::Net(Done::Weather(Ok(Weather {
        lat: -1.28, lon: 36.82, city: "Nairobi".into(), utc_offset: 10_800, temp: 22.0, feels: 21.4,
        hi: 26.0, lo: 14.0, humidity: 61.0, wind: 9.0, rain: 10.0, code: 2, day: true,
        sunrise: "06:13".into(), sunset: "18:21".into(), fahrenheit: false,
    }))));
    let status: thing::net::companion::Status = serde_json::from_str(
        r#"{"sessions": [
              {"id": "a", "project": "pico-thing", "state": "working", "message": ""},
              {"id": "b", "project": "website", "state": "waiting", "message": "Allow Bash: npm test?"}],
            "attention": null, "todos_rev": 1,
            "tasks": [{"name": "Project status", "state": "done"}, {"name": "What's next", "state": "idle"}],
            "actions": [{"id": "mic", "label": "Mute mic", "on": false}, {"id": "focus", "label": "Focus 25m"},
                        {"id": "lock", "label": "Lock screen"}, {"id": "slack", "label": "Open Slack"}]}"#,
    )
    .unwrap();
    app.handle(Event::Net(Done::Companion(Ok(status))));
    app.handle(Event::Net(Done::Todos(Ok(vec![
        "Water the plants".into(),
        "Review the open pull requests".into(),
        "Read the Rust book".into(),
    ]))));
    // The first light on, any others off
    let lights: Vec<&str> = app.state.lights.iter().map(|l| l.ip).collect();
    for (i, ip) in lights.into_iter().enumerate() {
        let state = LightState { power: i == 0, bright: 70, ct: 3500 };
        app.handle(Event::Net(Done::Light(ip.into(), Ok(state))));
    }
    // The first speaker playing, the next paused
    let speakers: Vec<&str> = app.state.speakers.iter().map(|s| s.ip).collect();
    for (i, ip) in speakers.into_iter().enumerate() {
        let state = if i == 0 {
            SpeakerState {
                transport: "PLAYING".into(), title: "Clair de Lune".into(), artist: "Claude Debussy".into(),
                album: "Suite bergamasque".into(), from: "Morning Classical".into(), volume: 32, ..Default::default()
            }
        } else {
            SpeakerState { transport: "PAUSED_PLAYBACK".into(), title: "So What".into(), artist: "Miles Davis".into(), volume: 20, ..Default::default() }
        };
        app.handle(Event::Net(Done::Sonos(ip.into(), Ok(state))));
    }
    let favorites = [
        ("Morning Classical", true), ("Clair de Lune", false), ("Kind of Blue", true), ("Gymnopédie No. 1", false),
        ("Lo-fi Beats", true), ("Weightless", false), ("Ambient Focus", true), ("River Flows in You", false),
        ("Jazz Classics", true), ("Nocturne Op. 9 No. 2", false), ("Deep Focus", true), ("Experience", false),
    ];
    let favorites = favorites.iter().enumerate().map(|(i, (t, mix))| (i, (*t).into(), *mix)).collect();
    app.handle(Event::Net(Done::Favorites(String::new(), Ok(favorites))));
    app.state.mac_stats = Some(Ok(mac_stats()));
    // A three-day-old pet that's just been patted
    let mut pet = Pet::default();
    pet.apply(PetCmd::Hatch, UNIX - 3 * 86_400 - 3600);
    app.state.settings.pet = Pet { food: 72, fun: 64, energy: 81, clean: 58, poops: 1, at: UNIX, ..pet };

    // Run the clock forward (this grows the bonsai, a step every 100ms)
    app.screen = screen;
    let mut t = 0;
    while t < UPTIME_MS {
        t += 100;
        app.handle(Event::Tick { uptime_ms: t });
        app.screen = screen; // (other screens would time out back to the face)
    }
    // Show the bonsai fully grown (it starts over every so often)
    let garden = &mut app.state.faces.bonsai;
    let mut t = UPTIME_MS;
    *garden = thing::faces::bonsai::Garden::new(7);
    while !garden.grown() {
        t += 100;
        garden.tick(t);
    }
    // The demo's own network requests never come back; don't show them as busy
    app.take_requests();
    app.state.in_flight = 0;
    // Set last, as they refer back from the time the screens are drawn
    app.state.faces.pet_reaction = Some((PetCmd::Pat, UPTIME_MS - 300));
    // An hourglass a third of the way through 25 minutes
    app.state.faces.hourglass.set(25 * 60, UPTIME_MS - 25 * 60 * 1000 / 3);
    app
}

/// The boot log part way through: the network's still answering.
fn boot(app: &mut App) {
    app.handle(Event::Tick { uptime_ms: 1_200 });
    for (key, text) in [
        ("lcd", "lcd       ili9488 480x320, spi 31 MHz"),
        ("touch", "touch     xpt2046, calibrated"),
        ("flash", "settings  loaded from flash"),
        ("heap", "heap      160 KB"),
        ("layout", "ratatui   layouts warmed in 349 ms"),
        ("cyw43", "wifi chip cyw43439 up"),
        ("wifi", "wifi      joined home"),
    ] {
        app.handle(Event::Boot { key, text: text.into(), ok: Some(true) });
    }
    app.handle(Event::Online(Some([192, 168, 1, 15])));
    app.handle(Event::Net(Done::Time(Ok(UNIX))));
}

/// A minute and a half of a Mac doing some work.
fn mac_stats() -> MacStats {
    let wave = |i: usize, period: usize, lo: u32, hi: u32| {
        let k = (i % period) as u32;
        let up = if k < period as u32 / 2 { k } else { period as u32 - k };
        lo + (hi - lo) * up / (period as u32 / 2)
    };
    MacStats {
        cpu: (0..44).map(|i| (wave(i, 16, 8, 46) + (i as u32 * 7) % 9) as u8).collect(),
        mem: (0..44).map(|i| (58 + (i / 11) as u32 + (i as u32 % 3)) as u8).collect(),
        rx: (0..44).map(|i| if (28..38).contains(&i) { 900 + (i as u32 * 131) % 700 } else { (i as u32 * 37) % 120 }).collect(),
        tx: (0..44).map(|i| 20 + (i as u32 * 23) % 90).collect(),
        mem_used: 9.6,
        mem_total: 16.0,
        load: [2.1, 2.6, 2.4],
        battery: Some(84),
        charging: true,
        disk_used: 312,
        disk_total: 494,
        uptime: 3 * 86_400 + 5 * 3600,
        cores: 8,
        host: "MacBook-Pro".into(),
    }
}
