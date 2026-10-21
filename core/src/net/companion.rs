//! The Mac companion (companion/server.py): Claude Code sessions, saved agent tasks,
//! Mac actions and the TODO file. Every request carries the shared token.

use alloc::format;
use alloc::string::String;
use alloc::vec::Vec;
use serde::Deserialize;

use super::{Net, NetError, http};
use crate::config;

#[derive(Clone, Debug, Default, Deserialize)]
pub struct Status {
    #[serde(default)]
    pub sessions: Vec<Session>,
    /// A session that needs you (or just finished), by id
    #[serde(default)]
    pub attention: Option<String>,
    #[serde(default)]
    pub todos_rev: u32,
    #[serde(default)]
    pub tasks: Vec<TaskInfo>,
    #[serde(default)]
    pub actions: Vec<ActionInfo>,
}

#[derive(Clone, Debug, Default, Deserialize)]
pub struct Session {
    pub id: String,
    pub project: String,
    /// waiting | working | done | idle
    pub state: String,
    #[serde(default)]
    pub message: String,
}

#[derive(Clone, Debug, Default, Deserialize)]
pub struct TaskInfo {
    pub name: String,
    /// idle | running | done | failed
    pub state: String,
}

#[derive(Clone, Debug, Default, Deserialize)]
pub struct ActionInfo {
    pub id: String,
    pub label: String,
    /// For toggles (mic mute): whether it's on
    #[serde(default)]
    pub on: Option<bool>,
}

#[derive(Clone, Debug, Default, Deserialize)]
pub struct TaskDetail {
    pub name: String,
    pub state: String,
    #[serde(default)]
    pub output: String,
}

/// The Mac's vitals for the system face: histories (oldest first, one sample every
/// ~2s) and the latest readings.
#[derive(Clone, Debug, Default, Deserialize)]
#[serde(default)]
pub struct MacStats {
    /// CPU use, %
    pub cpu: Vec<u8>,
    /// Memory use, %
    pub mem: Vec<u8>,
    /// Network, KB/s down and up
    pub rx: Vec<u32>,
    pub tx: Vec<u32>,
    /// GB
    pub mem_used: f32,
    pub mem_total: f32,
    pub load: [f32; 3],
    pub battery: Option<u8>,
    pub charging: bool,
    /// GB
    pub disk_used: u32,
    pub disk_total: u32,
    /// Seconds
    pub uptime: u64,
    pub cores: u8,
    pub host: String,
}

#[derive(Deserialize)]
struct Todos {
    items: Vec<String>,
}

pub fn configured() -> bool {
    !config::COMPANION.is_empty()
}

async fn call<N: Net>(net: &mut N, method: &str, path: &str) -> Result<Vec<u8>, NetError> {
    if !configured() {
        return Err(NetError::Offline);
    }
    let url = format!("{}{}", config::COMPANION.trim_end_matches('/'), path);
    let r = http::request(net, method, &url, &[("X-Token", config::COMPANION_TOKEN)], &[]).await?;
    Ok(r.ok()?.body)
}

fn parse<'a, T: Deserialize<'a>>(body: &'a [u8]) -> Result<T, NetError> {
    serde_json::from_slice(body).map_err(|_| NetError::Protocol)
}

pub async fn status<N: Net>(net: &mut N) -> Result<Status, NetError> {
    parse(&call(net, "GET", "/status").await?)
}

pub async fn stats<N: Net>(net: &mut N) -> Result<MacStats, NetError> {
    parse(&call(net, "GET", "/stats").await?)
}

pub async fn todos<N: Net>(net: &mut N) -> Result<Vec<String>, NetError> {
    Ok(parse::<Todos>(&call(net, "GET", "/todos").await?)?.items)
}

pub async fn task<N: Net>(net: &mut N, n: usize) -> Result<TaskDetail, NetError> {
    parse(&call(net, "GET", &format!("/task?n={n}")).await?)
}

/// A POST like "/focus?id=..." or "/run?n=0" (no body, no meaningful reply).
pub async fn post<N: Net>(net: &mut N, path: &str) -> Result<(), NetError> {
    call(net, "POST", path).await.map(|_| ())
}

/// Percent-encode a query value.
pub fn encode(s: &str) -> String {
    let mut out = String::new();
    for b in s.bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => out.push(b as char),
            b' ' => out.push('+'),
            _ => out.push_str(&format!("%{b:02X}")),
        }
    }
    out
}
