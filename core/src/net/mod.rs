//! Networking, written once for every platform. A platform implements `Net`
//! (TCP connections and UDP exchanges); the protocols on top live here.

pub mod companion;
pub mod http;
pub mod miio;
pub mod services;
pub mod sntp;
pub mod sonos;
pub mod weather;
pub mod yeelight;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NetError {
    Offline,
    Dns,
    Connect,
    Io,
    Timeout,
    /// The server answered, but not with something we could use
    Protocol,
    Http(u16),
}

/// A TCP connection.
#[allow(async_fn_in_trait)]
pub trait Conn {
    async fn write_all(&mut self, data: &[u8]) -> Result<(), NetError>;
    /// Read some bytes; Ok(0) means the peer closed the connection.
    async fn read(&mut self, buf: &mut [u8]) -> Result<usize, NetError>;
}

#[allow(async_fn_in_trait)]
pub trait Net {
    type Conn<'a>: Conn
    where
        Self: 'a;
    /// Connect to a host name or dotted IPv4 address.
    async fn connect(&mut self, host: &str, port: u16) -> Result<Self::Conn<'_>, NetError>;
    /// Send one datagram and wait for one reply; returns the reply's length.
    async fn udp_exchange(&mut self, host: &str, port: u16, send: &[u8], recv: &mut [u8]) -> Result<usize, NetError>;
}

/// "a.b.c.d" -> bytes, for hosts given as IPs.
pub fn parse_ipv4(s: &str) -> Option<[u8; 4]> {
    let mut out = [0u8; 4];
    let mut parts = s.split('.');
    for b in &mut out {
        *b = parts.next()?.parse().ok()?;
    }
    parts.next().is_none().then_some(out)
}
