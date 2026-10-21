//! `thing::net::Net` over the Mac's sockets. Blocking calls inside async fns are
//! fine here: jobs run one at a time on their own thread.

use std::io::{Read, Write};
use std::net::{TcpStream, ToSocketAddrs, UdpSocket};
use std::sync::mpsc::{Receiver, Sender};
use std::time::Duration;

use thing::net::services::{self, Done, Job};
use thing::net::{Conn, Net, NetError};

const TIMEOUT: Duration = Duration::from_secs(8);

pub struct StdNet;

pub struct StdConn(TcpStream);

impl Conn for StdConn {
    async fn write_all(&mut self, data: &[u8]) -> Result<(), NetError> {
        self.0.write_all(data).map_err(|_| NetError::Io)
    }

    async fn read(&mut self, buf: &mut [u8]) -> Result<usize, NetError> {
        self.0.read(buf).map_err(|e| match e.kind() {
            std::io::ErrorKind::WouldBlock | std::io::ErrorKind::TimedOut => NetError::Timeout,
            _ => NetError::Io,
        })
    }
}

impl Net for StdNet {
    type Conn<'a> = StdConn;

    async fn connect(&mut self, host: &str, port: u16) -> Result<StdConn, NetError> {
        let addr = (host, port).to_socket_addrs().map_err(|_| NetError::Dns)?.next().ok_or(NetError::Dns)?;
        let s = TcpStream::connect_timeout(&addr, TIMEOUT).map_err(|_| NetError::Connect)?;
        s.set_read_timeout(Some(TIMEOUT)).ok();
        Ok(StdConn(s))
    }

    async fn udp_exchange(&mut self, host: &str, port: u16, send: &[u8], recv: &mut [u8]) -> Result<usize, NetError> {
        let s = UdpSocket::bind("0.0.0.0:0").map_err(|_| NetError::Io)?;
        s.set_read_timeout(Some(Duration::from_secs(3))).ok();
        let addr = (host, port).to_socket_addrs().map_err(|_| NetError::Dns)?.next().ok_or(NetError::Dns)?;
        s.send_to(send, addr).map_err(|_| NetError::Io)?;
        s.recv(recv).map_err(|_| NetError::Timeout)
    }
}

/// Run jobs from the app until the app goes away.
pub fn worker(jobs: Receiver<Job>, done: Sender<Done>) {
    let mut net = StdNet;
    for job in jobs {
        let result = embassy_futures::block_on(services::run(&mut net, job));
        if done.send(result).is_err() {
            break;
        }
    }
}

/// This Mac's LAN address (connecting a UDP socket sends nothing).
pub fn local_ip() -> Option<[u8; 4]> {
    let s = UdpSocket::bind("0.0.0.0:0").ok()?;
    s.connect("8.8.8.8:80").ok()?;
    match s.local_addr().ok()?.ip() {
        std::net::IpAddr::V4(v4) => Some(v4.octets()),
        _ => None,
    }
}
