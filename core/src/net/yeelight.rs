//! Yeelight LAN control: JSON lines over TCP 55443 ("LAN Control" must be on in the
//! Yeelight app). miIO lights speak the same methods over a different transport.

use alloc::format;
use alloc::string::{String, ToString};
use alloc::vec::Vec;
use serde_json::Value;

use super::{Conn, Net, NetError, miio};

pub const PORT: u16 = 55443;

#[derive(Clone, Debug, Default, PartialEq)]
pub struct LightState {
    pub power: bool,
    pub bright: u8,
    /// Colour temperature in kelvin; 0 = the light doesn't do colour temperature
    pub ct: u32,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum LightCmd {
    Power(bool),
    Bright(u8),
    Ct(u32),
    Rgb(u32),
    /// Blink a few times so you can see which light this is
    Identify,
}

/// How to reach a light: Yeelight LAN, or miIO with its token.
#[derive(Clone, Debug, PartialEq)]
pub enum Transport {
    Yeelight,
    Miio([u8; 16]),
}

async fn call_yeelight<N: Net>(net: &mut N, ip: &str, method: &str, params: &str) -> Result<Value, NetError> {
    let mut conn = net.connect(ip, PORT).await?;
    let msg = format!("{{\"id\":1,\"method\":\"{method}\",\"params\":[{params}]}}\r\n");
    conn.write_all(msg.as_bytes()).await?;
    let mut buf: Vec<u8> = Vec::new();
    let mut chunk = [0u8; 256];
    loop {
        // Replies can be interleaved with unsolicited "props" notifications
        while let Some(i) = buf.iter().position(|&b| b == b'\n') {
            let line: Vec<u8> = buf.drain(..=i).collect();
            let Ok(v) = serde_json::from_slice::<Value>(&line) else { continue };
            if v.get("id").and_then(Value::as_u64) == Some(1) {
                if v.get("error").is_some() {
                    return Err(NetError::Protocol);
                }
                return Ok(v.get("result").cloned().unwrap_or(Value::Null));
            }
        }
        let n = conn.read(&mut chunk).await?;
        if n == 0 || buf.len() > 4096 {
            return Err(NetError::Io);
        }
        buf.extend_from_slice(&chunk[..n]);
    }
}

pub async fn call<N: Net>(net: &mut N, ip: &str, t: &Transport, method: &str, params: &str) -> Result<Value, NetError> {
    match t {
        Transport::Yeelight => call_yeelight(net, ip, method, params).await,
        Transport::Miio(token) => miio::call(net, ip, token, method, params).await,
    }
}

pub async fn state<N: Net>(net: &mut N, ip: &str, t: &Transport) -> Result<LightState, NetError> {
    let r = call(net, ip, t, "get_prop", "\"power\",\"bright\",\"ct\"").await?;
    let v: Vec<String> = r
        .as_array()
        .map(|a| a.iter().map(|x| x.as_str().map(String::from).unwrap_or_else(|| x.to_string())).collect())
        .unwrap_or_default();
    let num = |i: usize| v.get(i).and_then(|s| s.parse::<u32>().ok()).unwrap_or(0);
    Ok(LightState { power: v.first().is_some_and(|p| p == "on"), bright: num(1) as u8, ct: num(2) })
}

pub async fn command<N: Net>(net: &mut N, ip: &str, t: &Transport, cmd: LightCmd, was_on: bool) -> Result<(), NetError> {
    // Brightness and colour changes need the light on first
    if !was_on && !matches!(cmd, LightCmd::Power(_) | LightCmd::Identify) {
        call(net, ip, t, "set_power", "\"on\",\"smooth\",300").await?;
    }
    let _ = match cmd {
        LightCmd::Power(on) => call(net, ip, t, "set_power", if on { "\"on\",\"smooth\",300" } else { "\"off\",\"smooth\",300" }).await?,
        LightCmd::Bright(p) => call(net, ip, t, "set_bright", &format!("{},\"smooth\",300", p.clamp(1, 100))).await?,
        LightCmd::Ct(k) => call(net, ip, t, "set_ct_abx", &format!("{k},\"smooth\",300")).await?,
        LightCmd::Rgb(rgb) => call(net, ip, t, "set_rgb", &format!("{rgb},\"smooth\",300")).await?,
        LightCmd::Identify => {
            // Flow of (duration ms, mode 2 = colour temperature, kelvin, brightness);
            // action 0 = restore the previous state afterwards
            if !was_on {
                call(net, ip, t, "set_power", "\"on\"").await?;
            }
            call(net, ip, t, "start_cf", "6,0,\"350,2,4000,100,350,2,4000,1\"").await?
        }
    };
    Ok(())
}
