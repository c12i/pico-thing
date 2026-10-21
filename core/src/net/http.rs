//! A small HTTP/1.0 client: one request per connection, body read to the end and
//! checked against Content-Length (a short read means a dropped connection).

use alloc::format;
use alloc::string::String;
use alloc::vec::Vec;

use super::{Conn, Net, NetError};

/// Responses bigger than this are refused (they'd crowd the heap).
const MAX_RESPONSE: usize = 48 * 1024;

pub struct Response {
    pub status: u16,
    pub body: Vec<u8>,
}

pub struct Url<'a> {
    pub host: &'a str,
    pub port: u16,
    pub path: &'a str,
}

/// Split "http://host[:port]/path".
pub fn parse_url(url: &str) -> Option<Url<'_>> {
    let rest = url.strip_prefix("http://")?;
    let (hostport, path) = match rest.find('/') {
        Some(i) => (&rest[..i], &rest[i..]),
        None => (rest, "/"),
    };
    let (host, port) = match hostport.split_once(':') {
        Some((h, p)) => (h, p.parse().ok()?),
        None => (hostport, 80),
    };
    Some(Url { host, port, path })
}

pub async fn request<N: Net>(
    net: &mut N,
    method: &str,
    url: &str,
    headers: &[(&str, &str)],
    body: &[u8],
) -> Result<Response, NetError> {
    let u = parse_url(url).ok_or(NetError::Protocol)?;
    let mut head = format!("{method} {} HTTP/1.0\r\nHost: {}\r\nConnection: close\r\n", u.path, u.host);
    for (k, v) in headers {
        head.push_str(&format!("{k}: {v}\r\n"));
    }
    if !body.is_empty() || method == "POST" {
        head.push_str(&format!("Content-Length: {}\r\n", body.len()));
    }
    head.push_str("\r\n");

    let mut conn = net.connect(u.host, u.port).await?;
    conn.write_all(head.as_bytes()).await?;
    if !body.is_empty() {
        conn.write_all(body).await?;
    }
    let mut data = Vec::with_capacity(4096);
    let mut chunk = [0u8; 512];
    loop {
        let n = conn.read(&mut chunk).await?;
        if n == 0 {
            break;
        }
        if data.len() + n > MAX_RESPONSE {
            return Err(NetError::Protocol);
        }
        data.extend_from_slice(&chunk[..n]);
    }
    parse_response(data)
}

pub async fn get<N: Net>(net: &mut N, url: &str, headers: &[(&str, &str)]) -> Result<Response, NetError> {
    request(net, "GET", url, headers, &[]).await
}

fn parse_response(mut data: Vec<u8>) -> Result<Response, NetError> {
    let end = data.windows(4).position(|w| w == b"\r\n\r\n").ok_or(NetError::Protocol)?;
    let head = core::str::from_utf8(&data[..end]).map_err(|_| NetError::Protocol)?;
    let mut lines = head.split("\r\n");
    let status: u16 = lines
        .next()
        .and_then(|l| l.split(' ').nth(1))
        .and_then(|s| s.parse().ok())
        .ok_or(NetError::Protocol)?;
    let mut length = None;
    for l in lines {
        if let Some((k, v)) = l.split_once(':') {
            if k.trim().eq_ignore_ascii_case("content-length") {
                length = v.trim().parse::<usize>().ok();
            }
        }
    }
    data.drain(..end + 4); // the body stays in place: no second copy
    let body = data;
    if let Some(n) = length {
        if body.len() < n {
            return Err(NetError::Io); // truncated
        }
    }
    Ok(Response { status, body })
}

impl Response {
    pub fn ok(self) -> Result<Self, NetError> {
        if (200..300).contains(&self.status) { Ok(self) } else { Err(NetError::Http(self.status)) }
    }

    /// The body as text (without copying it when it's valid UTF-8).
    pub fn into_text(self) -> String {
        String::from_utf8(self.body).unwrap_or_else(|e| String::from_utf8_lossy(e.as_bytes()).into_owned())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn urls() {
        let u = parse_url("http://192.168.1.8:8765/todos?x=1").unwrap();
        assert_eq!((u.host, u.port, u.path), ("192.168.1.8", 8765, "/todos?x=1"));
        let u = parse_url("http://api.open-meteo.com").unwrap();
        assert_eq!((u.host, u.port, u.path), ("api.open-meteo.com", 80, "/"));
    }

    #[test]
    fn responses() {
        let r = parse_response(b"HTTP/1.0 200 OK\r\nContent-Length: 2\r\n\r\nhi".to_vec()).ok().unwrap();
        assert_eq!((r.status, r.body.as_slice()), (200, &b"hi"[..]));
        assert!(parse_response(b"HTTP/1.0 200 OK\r\nContent-Length: 5\r\n\r\nhi".to_vec()).is_err());
    }
}
