//! Holding a speaker that isn't playing joins it to one that is; tapping selects it.
use ratatui::backend::TestBackend;
use thing::app::{Request, Screen};
use thing::net::services::{Done, Job};
use thing::net::sonos::{SonosCmd, SpeakerState};
use thing::{App, COLS, Event, ROWS, Settings};

fn app_with_speakers() -> Option<(App, ratatui::Terminal<TestBackend>)> {
    let mut app = App::new(Settings::default(), 1);
    if app.state.speakers.len() < 2 {
        return None; // needs two speakers in config.toml
    }
    app.handle(Event::Online(Some([10, 0, 0, 2])));
    let playing = SpeakerState { transport: "PLAYING".into(), title: "Song".into(), ..Default::default() };
    let stopped = SpeakerState { transport: "STOPPED".into(), ..Default::default() };
    let (a, b) = (app.state.speakers[0].ip, app.state.speakers[1].ip);
    app.handle(Event::Net(Done::Sonos(a.into(), Ok(stopped))));
    app.handle(Event::Net(Done::Sonos(b.into(), Ok(playing))));
    app.screen = Screen::Sonos;
    let mut term = ratatui::Terminal::new(TestBackend::new(COLS, ROWS)).unwrap();
    term.draw(|f| app.draw(f, (0, 0))).unwrap();
    app.take_requests();
    Some((app, term))
}

fn sonos_jobs(app: &mut App) -> Vec<Option<SonosCmd>> {
    app.take_requests()
        .into_iter()
        .filter_map(|r| match r {
            Request::Net(Job::Sonos { cmd, .. }) => Some(cmd),
            _ => None,
        })
        .collect()
}

#[test]
fn hold_joins_tap_selects() {
    let Some((mut app, _term)) = app_with_speakers() else { return };
    let leader = app.state.speakers[1].ip;
    // The first speaker's row starts just under the top bar and pane border
    let (col, row) = (5, 4);

    // Hold: nothing until LONG_MS, then a join
    app.handle(Event::Tick { uptime_ms: 1000 });
    app.handle(Event::Tap { col, row });
    app.handle(Event::Tick { uptime_ms: 1300 });
    assert!(sonos_jobs(&mut app).is_empty());
    app.handle(Event::Tick { uptime_ms: 1700 });
    assert_eq!(sonos_jobs(&mut app), vec![Some(SonosCmd::Join(leader))]);
    app.handle(Event::Release);

    // Tap: selects (which refreshes the speaker), no join
    app.handle(Event::Tick { uptime_ms: 3000 });
    app.handle(Event::Tap { col, row });
    app.handle(Event::Release);
    app.handle(Event::Tick { uptime_ms: 3200 });
    assert_eq!(sonos_jobs(&mut app), vec![None]);
}
