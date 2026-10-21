//! Network jobs the app asks for, run by the platform's network task. Each job
//! turns into one result event for the app.

use alloc::string::String;

use super::companion::{self, MacStats, Status, TaskDetail};
use super::sonos::{self, Favorite, SonosCmd, SpeakerState};
use super::yeelight::{self, LightCmd, LightState, Transport};
use crate::config;
use super::weather::{self, Weather};
use super::{Net, NetError, sntp};
use alloc::vec::Vec;

#[derive(Clone, Debug, PartialEq)]
pub enum Job {
    SyncTime,
    /// `place` is (lat, lon, city) once known; None looks it up from our IP
    Weather { place: Option<(f32, f32, String)>, fahrenheit: bool },
    CompanionStatus,
    Todos,
    /// POST to the companion (e.g. "/focus?id=..."), then fetch its status
    CompanionPost(String),
    /// Fetch a saved task's output
    Task(usize),
    /// Send a command to a light (if any), then read its state
    Light { ip: String, transport: Transport, cmd: Option<LightCmd>, was_on: bool },
    /// Send a command to a speaker (if any), then read its state. `expect` is the
    /// favorite's title for SonosCmd::Favorite.
    Sonos { ip: String, cmd: Option<SonosCmd>, expect: String },
    Favorites { ip: String },
    /// The Mac's vitals, for the system face
    MacStats,
}

#[derive(Clone, Debug)]
pub enum Done {
    Time(Result<i64, NetError>),
    Weather(Result<Weather, NetError>),
    Companion(Result<Status, NetError>),
    Todos(Result<Vec<String>, NetError>),
    /// (task index, detail)
    Task(usize, Result<TaskDetail, NetError>),
    Light(String, Result<LightState, NetError>),
    Sonos(String, Result<SpeakerState, NetError>),
    Favorites(String, Result<Vec<Favorite>, NetError>),
    MacStats(Result<MacStats, NetError>),
}

pub async fn run<N: Net>(net: &mut N, job: Job) -> Done {
    match job {
        Job::SyncTime => Done::Time(sntp::now(net).await),
        Job::Weather { place, fahrenheit } => Done::Weather(weather::fetch(net, place, fahrenheit).await),
        Job::CompanionStatus => Done::Companion(companion::status(net).await),
        Job::Todos => Done::Todos(companion::todos(net).await),
        Job::CompanionPost(path) => match companion::post(net, &path).await {
            Ok(()) => Done::Companion(companion::status(net).await),
            Err(e) => Done::Companion(Err(e)),
        },
        Job::Task(n) => Done::Task(n, companion::task(net, n).await),
        Job::Light { ip, transport, cmd, was_on } => {
            // A command the light refuses (e.g. colour temperature on a white-only
            // bulb) still ends with its real state rather than looking offline
            if let Some(c) = cmd {
                let _ = yeelight::command(net, &ip, &transport, c, was_on).await;
            }
            let r = yeelight::state(net, &ip, &transport).await;
            Done::Light(ip, r)
        }
        Job::Sonos { ip, cmd, expect } => {
            let others: Vec<&str> = config::SONOS.iter().map(|(_, ip)| *ip).collect();
            if let Some(c) = cmd {
                if let Err(e) = sonos::command(net, &ip, &others, c, &expect).await {
                    return Done::Sonos(ip, Err(e));
                }
            }
            let r = sonos::state(net, &ip, &others).await;
            Done::Sonos(ip, r)
        }
        Job::Favorites { ip } => {
            let r = sonos::favorites(net, &ip).await;
            Done::Favorites(ip, r)
        }
        Job::MacStats => Done::MacStats(companion::stats(net).await),
    }
}

/// The result for a job that took too long.
pub fn timed_out(job: &Job) -> Done {
    match job {
        Job::SyncTime => Done::Time(Err(NetError::Timeout)),
        Job::Weather { .. } => Done::Weather(Err(NetError::Timeout)),
        Job::CompanionStatus | Job::CompanionPost(_) => Done::Companion(Err(NetError::Timeout)),
        Job::Todos => Done::Todos(Err(NetError::Timeout)),
        Job::Task(n) => Done::Task(*n, Err(NetError::Timeout)),
        Job::Light { ip, .. } => Done::Light(ip.clone(), Err(NetError::Timeout)),
        Job::Sonos { ip, .. } => Done::Sonos(ip.clone(), Err(NetError::Timeout)),
        Job::Favorites { ip } => Done::Favorites(ip.clone(), Err(NetError::Timeout)),
        Job::MacStats => Done::MacStats(Err(NetError::Timeout)),
    }
}

impl Done {
    pub fn ok(&self) -> bool {
        match self {
            Done::Time(r) => r.is_ok(),
            Done::Weather(r) => r.is_ok(),
            Done::Companion(r) => r.is_ok(),
            Done::Todos(r) => r.is_ok(),
            Done::Task(_, r) => r.is_ok(),
            Done::Light(_, r) => r.is_ok(),
            Done::Sonos(_, r) => r.is_ok(),
            Done::Favorites(_, r) => r.is_ok(),
            Done::MacStats(r) => r.is_ok(),
        }
    }
}
