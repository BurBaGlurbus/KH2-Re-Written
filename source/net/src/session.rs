//! One LAN session over one non-blocking UDP socket.
//!
//! The host binds the game port and takes one joiner: a Hello with the same
//! protocol and build gets a Welcome, anything else a Reject with its
//! reason. Once welcomed, renet's packets ride in Data datagrams. Either
//! side ends with Bye; a peer silent for longer than the timeout is gone.
//! The host also answers Discover with an Announce, which is how a joiner
//! finds hosts on the LAN.

use crate::link::{Conditions, Link};
use crate::wire::{Build, Datagram, PROTOCOL};
use crate::{RELIABLE, UNRELIABLE};
use renet::{ChannelConfig, ConnectionConfig, RenetClient, RenetServer, SendType};
use std::collections::VecDeque;
use std::io;
use std::net::{Ipv4Addr, SocketAddr, SocketAddrV4, UdpSocket};
use std::time::{Duration, Instant};

/// The one joiner's id inside renet's server.
const PEER: u64 = 1;
/// A Hello is repeated this often until the host answers.
const HELLO_EVERY: Duration = Duration::from_millis(250);
/// Something is sent at least this often so the other side knows we are here.
const KEEPALIVE: Duration = Duration::from_millis(200);
/// The largest datagram read.
const DATAGRAM: usize = 2048;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Role {
    Host,
    Join,
}

#[derive(Clone, Debug)]
pub struct SessionConfig {
    /// The name the other player sees.
    pub name: String,
    /// The executable's identity; a peer with another one is refused.
    pub build: Build,
    /// How long a silent peer (or a host that never answers) is waited for.
    pub timeout: Duration,
}

/// The other player.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Peer {
    pub name: String,
    pub addr: SocketAddr,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Event {
    /// The other player is in (host: a joiner was welcomed; joiner: the
    /// host welcomed us).
    Connected(Peer),
    /// The host refused us, with its reason.
    Rejected(String),
    /// The other player left, timed out, or never answered.
    Disconnected(String),
}

/// Link statistics for the overlay.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Stats {
    pub rtt_ms: f64,
    /// 0..1, renet's estimate from unacked packets.
    pub packet_loss: f64,
    pub sent_bytes_per_sec: f64,
    pub received_bytes_per_sec: f64,
    /// Datagrams the link simulator dropped.
    pub simulated_drops: u64,
    /// Milliseconds since the peer was last heard.
    pub silent_ms: f64,
}

enum Side {
    Host {
        server: Box<RenetServer>,
        peer: Option<(Peer, u64)>,
    },
    Join {
        client: Box<RenetClient>,
        host: SocketAddr,
        welcomed: Option<Peer>,
        last_hello: Option<Duration>,
    },
}

pub struct Session {
    socket: UdpSocket,
    config: SessionConfig,
    side: Side,
    nonce: u64,
    link: Link,
    start: Instant,
    last_update: Duration,
    last_heard: Duration,
    /// When a joiner's first Hello went (its wait for the host runs from it).
    first_hello: Option<Duration>,
    last_sent: Duration,
    events: VecDeque<Event>,
    closed: bool,
}

fn channels() -> Vec<ChannelConfig> {
    vec![
        ChannelConfig {
            channel_id: UNRELIABLE,
            max_memory_usage_bytes: 1 << 20,
            send_type: SendType::Unreliable,
        },
        ChannelConfig {
            channel_id: RELIABLE,
            max_memory_usage_bytes: 1 << 20,
            send_type: SendType::ReliableOrdered {
                resend_time: Duration::from_millis(100),
            },
        },
    ]
}

fn connection() -> ConnectionConfig {
    ConnectionConfig {
        available_bytes_per_tick: 60_000,
        server_channels_config: channels(),
        client_channels_config: channels(),
    }
}

/// A session nonce from the clock and the process, so a restarted joiner's
/// stale datagrams are not taken for the new session's.
fn fresh_nonce(port: u16) -> u64 {
    let t = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos() as u64)
        .unwrap_or(0);
    (t ^ u64::from(std::process::id()) << 32 ^ u64::from(port)) | 1
}

impl Session {
    /// Hosts on `port` of `ip` (every local address for a LAN game, the
    /// loopback one for a test on one PC).
    pub fn host(ip: Ipv4Addr, port: u16, config: SessionConfig) -> io::Result<Session> {
        let socket = UdpSocket::bind(SocketAddrV4::new(ip, port))?;
        Self::with_socket(
            socket,
            config,
            Side::Host {
                server: Box::new(RenetServer::new(connection())),
                peer: None,
            },
        )
    }

    /// Joins the host at `host` (a port of its own is picked).
    pub fn join(host: SocketAddr, config: SessionConfig) -> io::Result<Session> {
        // A host on this PC is reached over loopback only.
        let ip = if host.ip().is_loopback() {
            Ipv4Addr::LOCALHOST
        } else {
            Ipv4Addr::UNSPECIFIED
        };
        let socket = UdpSocket::bind(SocketAddrV4::new(ip, 0))?;
        let mut client = RenetClient::new(connection());
        client.set_connecting();
        Self::with_socket(
            socket,
            config,
            Side::Join {
                client: Box::new(client),
                host,
                welcomed: None,
                last_hello: None,
            },
        )
    }

    fn with_socket(socket: UdpSocket, config: SessionConfig, side: Side) -> io::Result<Session> {
        socket.set_nonblocking(true)?;
        let port = socket.local_addr()?.port();
        let nonce = fresh_nonce(port);
        Ok(Session {
            socket,
            config,
            side,
            nonce,
            link: Link::new(nonce),
            start: Instant::now(),
            last_update: Duration::ZERO,
            last_heard: Duration::ZERO,
            first_hello: None,
            last_sent: Duration::ZERO,
            events: VecDeque::new(),
            closed: false,
        })
    }

    pub fn role(&self) -> Role {
        match self.side {
            Side::Host { .. } => Role::Host,
            Side::Join { .. } => Role::Join,
        }
    }

    pub fn local_addr(&self) -> io::Result<SocketAddr> {
        self.socket.local_addr()
    }

    /// The other player once the session runs.
    pub fn peer(&self) -> Option<&Peer> {
        match &self.side {
            Side::Host { peer, .. } => peer.as_ref().map(|p| &p.0),
            Side::Join { welcomed, .. } => welcomed.as_ref(),
        }
    }

    pub fn is_connected(&self) -> bool {
        !self.closed && self.peer().is_some()
    }

    pub fn is_closed(&self) -> bool {
        self.closed
    }

    pub fn conditions(&self) -> Conditions {
        self.link.conditions
    }

    /// How long silence means the other player is gone, from now on.
    pub fn set_timeout(&mut self, timeout: Duration) {
        self.config.timeout = timeout;
    }

    pub fn set_conditions(&mut self, conditions: Conditions) {
        self.link.conditions = conditions;
    }

    pub fn next_event(&mut self) -> Option<Event> {
        self.events.pop_front()
    }

    fn now(&self) -> Duration {
        self.start.elapsed()
    }

    /// Reads what arrived, advances renet, handles timeouts and sends what
    /// is due. Call once per simulation tick.
    pub fn update(&mut self) {
        if self.closed {
            return;
        }
        let now = self.now();
        self.receive_all(now);
        let delta = now.saturating_sub(self.last_update);
        self.last_update = now;
        let timeout = self.config.timeout;
        let silent = now.saturating_sub(self.last_heard);
        let first_hello = &mut self.first_hello;
        match &mut self.side {
            Side::Host { server, peer } => {
                server.update(delta);
                if peer.is_some() && silent > timeout {
                    *peer = None;
                    server.remove_connection(PEER);
                    self.events
                        .push_back(Event::Disconnected("the other player timed out".into()));
                }
            }
            Side::Join {
                client,
                host,
                welcomed,
                last_hello,
            } => {
                client.update(delta);
                if welcomed.is_none() {
                    // The wait starts with the first Hello: a joiner's own
                    // load, before its first update, does not count.
                    let waited = now.saturating_sub(*first_hello.get_or_insert(now));
                    if waited > timeout {
                        self.closed = true;
                        self.events.push_back(Event::Disconnected(format!(
                            "no answer from {host}: check the address, that the host is waiting, and the firewall"
                        )));
                        return;
                    }
                    if last_hello.is_none_or(|t| now.saturating_sub(t) >= HELLO_EVERY) {
                        *last_hello = Some(now);
                        let hello = Datagram::Hello {
                            protocol: PROTOCOL,
                            build: self.config.build,
                            nonce: self.nonce,
                            name: self.config.name.clone(),
                        };
                        let to = *host;
                        self.transmit(now, to, hello.encode());
                    }
                } else if silent > timeout {
                    self.closed = true;
                    client.disconnect_due_to_transport();
                    self.events
                        .push_back(Event::Disconnected("the host timed out".into()));
                    return;
                }
            }
        }
        self.flush(now);
    }

    fn receive_all(&mut self, now: Duration) {
        let mut buffer = [0u8; DATAGRAM];
        loop {
            match self.socket.recv_from(&mut buffer) {
                Ok((n, from)) => {
                    if let Some(d) = Datagram::decode(&buffer[..n]) {
                        self.handle(now, from, d);
                    }
                    // A Bye or Reject ended the session: the rest is stale.
                    if self.closed {
                        break;
                    }
                }
                Err(e) if e.kind() == io::ErrorKind::WouldBlock => break,
                // Windows reports an earlier datagram's ICMP "port
                // unreachable" on the next read; it is not this socket's end.
                Err(e)
                    if matches!(
                        e.kind(),
                        io::ErrorKind::ConnectionReset | io::ErrorKind::ConnectionRefused
                    ) => {}
                Err(_) => break,
            }
        }
    }

    fn handle(&mut self, now: Duration, from: SocketAddr, d: Datagram) {
        let build = self.config.build;
        let name = self.config.name.clone();
        match (&mut self.side, d) {
            (Side::Host { .. }, Datagram::Discover { protocol }) if protocol == PROTOCOL => {
                let open = self.peer().is_none();
                let a = Datagram::Announce {
                    protocol: PROTOCOL,
                    build,
                    name,
                    open,
                };
                self.transmit(now, from, a.encode());
            }
            (
                Side::Host { server, peer },
                Datagram::Hello {
                    protocol,
                    build: theirs,
                    nonce,
                    name: their_name,
                },
            ) => {
                let reply = match peer {
                    // A repeated Hello of the session already welcomed.
                    Some((p, n)) if p.addr == from && *n == nonce => {
                        Datagram::Welcome { nonce, name }
                    }
                    Some(_) => Datagram::Reject {
                        nonce,
                        reason: "this match already has two players".into(),
                    },
                    None if protocol != PROTOCOL || theirs != build => Datagram::Reject {
                        nonce,
                        reason: "the host runs a different build: both players need the same launcher version".into(),
                    },
                    None => {
                        let p = Peer {
                            name: their_name,
                            addr: from,
                        };
                        *peer = Some((p.clone(), nonce));
                        server.add_connection(PEER);
                        self.last_heard = now;
                        self.events.push_back(Event::Connected(p));
                        Datagram::Welcome { nonce, name }
                    }
                };
                self.transmit(now, from, reply.encode());
            }
            (Side::Host { server, peer }, Datagram::Data { nonce, payload }) => {
                if let Some((p, n)) = peer
                    && p.addr == from
                    && *n == nonce
                {
                    self.last_heard = now;
                    if !payload.is_empty() {
                        let _ = server.process_packet_from(&payload, PEER);
                    }
                }
            }
            (Side::Host { server, peer }, Datagram::Bye { nonce }) => {
                if let Some((p, n)) = peer
                    && p.addr == from
                    && *n == nonce
                {
                    *peer = None;
                    server.remove_connection(PEER);
                    self.events
                        .push_back(Event::Disconnected("the other player left".into()));
                }
            }
            (
                Side::Join {
                    client,
                    host,
                    welcomed,
                    ..
                },
                Datagram::Welcome { nonce, name },
            ) if from == *host && nonce == self.nonce && welcomed.is_none() => {
                let p = Peer { name, addr: from };
                *welcomed = Some(p.clone());
                client.set_connected();
                self.last_heard = now;
                self.events.push_back(Event::Connected(p));
            }
            (Side::Join { host, .. }, Datagram::Reject { nonce, reason })
                if from == *host && nonce == self.nonce =>
            {
                self.closed = true;
                self.events.push_back(Event::Rejected(reason));
            }
            (
                Side::Join {
                    client,
                    host,
                    welcomed,
                    ..
                },
                Datagram::Data { nonce, payload },
            ) if from == *host && nonce == self.nonce && welcomed.is_some() => {
                self.last_heard = now;
                if !payload.is_empty() {
                    client.process_packet(&payload);
                }
            }
            (Side::Join { host, welcomed, .. }, Datagram::Bye { nonce })
                if from == *host && nonce == self.nonce && welcomed.is_some() =>
            {
                self.closed = true;
                self.events
                    .push_back(Event::Disconnected("the host left".into()));
            }
            _ => {}
        }
    }

    /// renet's packets, then a keep-alive if nothing went out for a while,
    /// then what the link simulator releases now.
    fn flush(&mut self, now: Duration) {
        let mut out: Vec<(SocketAddr, Datagram)> = Vec::new();
        let nonce_join = self.nonce;
        match &mut self.side {
            Side::Host {
                server,
                peer: Some((p, nonce)),
            } => {
                for payload in server.get_packets_to_send(PEER).unwrap_or_default() {
                    out.push((
                        p.addr,
                        Datagram::Data {
                            nonce: *nonce,
                            payload,
                        },
                    ));
                }
                if out.is_empty() && now.saturating_sub(self.last_sent) >= KEEPALIVE {
                    out.push((
                        p.addr,
                        Datagram::Data {
                            nonce: *nonce,
                            payload: Vec::new(),
                        },
                    ));
                }
            }
            Side::Join {
                client,
                host,
                welcomed: Some(_),
                ..
            } => {
                for payload in client.get_packets_to_send() {
                    out.push((
                        *host,
                        Datagram::Data {
                            nonce: nonce_join,
                            payload,
                        },
                    ));
                }
                if out.is_empty() && now.saturating_sub(self.last_sent) >= KEEPALIVE {
                    out.push((
                        *host,
                        Datagram::Data {
                            nonce: nonce_join,
                            payload: Vec::new(),
                        },
                    ));
                }
            }
            _ => {}
        }
        for (to, d) in out {
            self.transmit(now, to, d.encode());
        }
        for (to, bytes) in self.link.due(now) {
            let _ = self.socket.send_to(&bytes, to);
        }
    }

    fn transmit(&mut self, now: Duration, to: SocketAddr, bytes: Vec<u8>) {
        self.last_sent = now;
        if let Some((to, bytes)) = self.link.send(now, to, bytes) {
            let _ = self.socket.send_to(&bytes, to);
        }
    }

    /// Queues a message for the other player on `channel`.
    pub fn send(&mut self, channel: u8, message: Vec<u8>) {
        if !self.is_connected() {
            return;
        }
        match &mut self.side {
            Side::Host { server, .. } => server.send_message(PEER, channel, message),
            Side::Join { client, .. } => client.send_message(channel, message),
        }
    }

    /// The next message the other player sent on `channel`.
    pub fn receive(&mut self, channel: u8) -> Option<Vec<u8>> {
        if !self.is_connected() {
            return None;
        }
        match &mut self.side {
            Side::Host { server, .. } => server.receive_message(PEER, channel),
            Side::Join { client, .. } => client.receive_message(channel),
        }
        .map(|b| b.to_vec())
    }

    pub fn stats(&self) -> Stats {
        let info = match &self.side {
            Side::Host { server, peer } if peer.is_some() => server.network_info(PEER).ok(),
            Side::Join {
                client,
                welcomed: Some(_),
                ..
            } => Some(client.network_info()),
            _ => None,
        };
        let silent = self.now().saturating_sub(self.last_heard);
        Stats {
            rtt_ms: info.as_ref().map_or(0., |i| i.rtt * 1000.),
            packet_loss: info.as_ref().map_or(0., |i| i.packet_loss),
            sent_bytes_per_sec: info.as_ref().map_or(0., |i| i.bytes_sent_per_second),
            received_bytes_per_sec: info.as_ref().map_or(0., |i| i.bytes_received_per_second),
            simulated_drops: self.link.dropped,
            silent_ms: silent.as_secs_f64() * 1000.,
        }
    }

    /// Ends the session: Bye goes out (a few times, past a lossy link) and
    /// the socket closes when the session is dropped.
    pub fn close(&mut self) {
        if self.closed {
            return;
        }
        let bye = match &self.side {
            Side::Host {
                peer: Some((p, nonce)),
                ..
            } => Some((p.addr, *nonce)),
            Side::Join {
                host,
                welcomed: Some(_),
                ..
            } => Some((*host, self.nonce)),
            _ => None,
        };
        if let Some((to, nonce)) = bye {
            let bytes = Datagram::Bye { nonce }.encode();
            // Past the simulator: the other side should hear it at once.
            for _ in 0..3 {
                let _ = self.socket.send_to(&bytes, to);
            }
        }
        self.closed = true;
    }
}

impl Drop for Session {
    fn drop(&mut self) {
        self.close();
    }
}

/// A host found on the LAN.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Found {
    pub name: String,
    pub addr: SocketAddr,
    /// Same protocol and build as ours.
    pub compatible: bool,
    /// The host still waits for a player.
    pub open: bool,
    /// How long its answer to our last ask took (ms, smoothed).
    pub ping_ms: Option<u32>,
    last_seen: Duration,
}

/// LAN discovery for the join screen: Discover goes to the game port as a
/// broadcast (and to this PC, for two instances on one machine); hosts
/// answer with Announce.
pub struct Browser {
    socket: UdpSocket,
    port: u16,
    build: Build,
    start: Instant,
    last_probe: Option<Duration>,
    found: Vec<Found>,
}

/// How often the browser asks, and how long an unanswered host stays listed.
const PROBE_EVERY: Duration = Duration::from_millis(1000);
const FORGET_AFTER: Duration = Duration::from_millis(3500);

impl Browser {
    pub fn new(port: u16, build: Build) -> io::Result<Browser> {
        let socket = UdpSocket::bind(SocketAddrV4::new(Ipv4Addr::UNSPECIFIED, 0))?;
        socket.set_nonblocking(true)?;
        socket.set_broadcast(true)?;
        Ok(Browser {
            socket,
            port,
            build,
            start: Instant::now(),
            last_probe: None,
            found: Vec::new(),
        })
    }

    /// Asks again when due, reads the answers and forgets silent hosts.
    pub fn update(&mut self) {
        let now = self.start.elapsed();
        if self
            .last_probe
            .is_none_or(|t| now.saturating_sub(t) >= PROBE_EVERY)
        {
            self.last_probe = Some(now);
            let probe = Datagram::Discover { protocol: PROTOCOL }.encode();
            for ip in [Ipv4Addr::BROADCAST, Ipv4Addr::LOCALHOST] {
                let _ = self
                    .socket
                    .send_to(&probe, SocketAddrV4::new(ip, self.port));
            }
        }
        let mut buffer = [0u8; DATAGRAM];
        loop {
            match self.socket.recv_from(&mut buffer) {
                Ok((n, from)) => {
                    if let Some(Datagram::Announce {
                        protocol,
                        build,
                        name,
                        open,
                    }) = Datagram::decode(&buffer[..n])
                    {
                        // The answer's time since the last ask (asks go out a
                        // second apart, so a later answer is no measure).
                        let ping = self
                            .last_probe
                            .map(|t| now.saturating_sub(t))
                            .filter(|d| *d < PROBE_EVERY)
                            .map(|d| d.as_millis() as u32);
                        self.note(
                            now,
                            from,
                            name,
                            protocol == PROTOCOL && build == self.build,
                            open,
                            ping,
                        );
                    }
                }
                Err(e) if e.kind() == io::ErrorKind::WouldBlock => break,
                Err(e)
                    if matches!(
                        e.kind(),
                        io::ErrorKind::ConnectionReset | io::ErrorKind::ConnectionRefused
                    ) => {}
                Err(_) => break,
            }
        }
        self.found
            .retain(|f| now.saturating_sub(f.last_seen) < FORGET_AFTER);
    }

    fn note(
        &mut self,
        now: Duration,
        addr: SocketAddr,
        name: String,
        compatible: bool,
        open: bool,
        ping: Option<u32>,
    ) {
        // One host answers both the broadcast and the loopback probe: keep
        // one row per name and port, preferring the LAN address.
        let mut row = Found {
            name,
            addr,
            compatible,
            open,
            ping_ms: ping,
            last_seen: now,
        };
        match self
            .found
            .iter_mut()
            .find(|f| f.addr == addr || (f.name == row.name && f.addr.port() == addr.port()))
        {
            Some(f) => {
                // A steadier number: three parts the last, one the new.
                row.ping_ms = match (f.ping_ms, ping) {
                    (Some(old), Some(new)) => Some((old * 3 + new).div_ceil(4)),
                    (old, new) => new.or(old),
                };
                let keep_lan = f.addr.ip().is_loopback() && !addr.ip().is_loopback();
                let addr = if keep_lan || !f.addr.ip().is_loopback() {
                    if keep_lan { addr } else { f.addr }
                } else {
                    addr
                };
                *f = Found { addr, ..row };
            }
            None => self.found.push(row),
        }
        self.found
            .sort_by(|a, b| a.name.cmp(&b.name).then(a.addr.cmp(&b.addr)));
    }

    pub fn found(&self) -> &[Found] {
        &self.found
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn config(name: &str, build: u8) -> SessionConfig {
        SessionConfig {
            name: name.into(),
            build: [build; 32],
            timeout: Duration::from_secs(2),
        }
    }

    /// Steps both sessions until `done` or about two seconds pass.
    fn pump(
        a: &mut Session,
        b: &mut Session,
        mut done: impl FnMut(&mut Session, &mut Session) -> bool,
    ) -> bool {
        for _ in 0..400 {
            a.update();
            b.update();
            if done(a, b) {
                return true;
            }
            std::thread::sleep(Duration::from_millis(5));
        }
        false
    }

    fn loopback(port: u16) -> SocketAddr {
        SocketAddr::from((Ipv4Addr::LOCALHOST, port))
    }

    #[test]
    fn join_exchange_and_leave_over_loopback() {
        let mut host = Session::host(Ipv4Addr::LOCALHOST, 0, config("Host", 1)).unwrap();
        let port = host.local_addr().unwrap().port();
        let mut join = Session::join(loopback(port), config("Joiner", 1)).unwrap();
        assert!(pump(&mut host, &mut join, |h, j| h.is_connected()
            && j.is_connected()));
        assert_eq!(host.peer().map(|p| p.name.as_str()), Some("Joiner"));
        assert_eq!(join.peer().map(|p| p.name.as_str()), Some("Host"));
        assert!(matches!(host.next_event(), Some(Event::Connected(_))));
        assert!(matches!(join.next_event(), Some(Event::Connected(_))));

        join.send(RELIABLE, b"ready".to_vec());
        host.send(UNRELIABLE, b"state".to_vec());
        let mut got = (None, None);
        assert!(pump(&mut host, &mut join, |h, j| {
            if got.0.is_none() {
                got.0 = h.receive(RELIABLE);
            }
            if got.1.is_none() {
                got.1 = j.receive(UNRELIABLE);
            }
            got.0.is_some() && got.1.is_some()
        }));
        assert_eq!(got, (Some(b"ready".to_vec()), Some(b"state".to_vec())));

        join.close();
        assert!(pump(&mut host, &mut join, |h, _| !h.is_connected()));
        assert_eq!(
            host.next_event(),
            Some(Event::Disconnected("the other player left".into()))
        );
    }

    #[test]
    fn another_build_is_refused_with_a_reason() {
        let mut host = Session::host(Ipv4Addr::LOCALHOST, 0, config("Host", 1)).unwrap();
        let port = host.local_addr().unwrap().port();
        let mut join = Session::join(loopback(port), config("Joiner", 2)).unwrap();
        assert!(pump(&mut host, &mut join, |_, j| j.is_closed()));
        match join.next_event() {
            Some(Event::Rejected(reason)) => assert!(reason.contains("different build")),
            other => panic!("expected a rejection, got {other:?}"),
        }
        assert!(!host.is_connected());
    }

    #[test]
    fn a_third_player_is_refused() {
        let mut host = Session::host(Ipv4Addr::LOCALHOST, 0, config("Host", 1)).unwrap();
        let port = host.local_addr().unwrap().port();
        let mut first = Session::join(loopback(port), config("One", 1)).unwrap();
        assert!(pump(&mut host, &mut first, |h, j| h.is_connected()
            && j.is_connected()));
        let mut second = Session::join(loopback(port), config("Two", 1)).unwrap();
        assert!(pump(&mut host, &mut second, |_, j| j.is_closed()));
        assert!(
            matches!(second.next_event(), Some(Event::Rejected(r)) if r.contains("two players"))
        );
        first.update();
        assert!(first.is_connected());
    }

    #[test]
    fn a_leaving_host_is_heard_once() {
        let mut host = Session::host(Ipv4Addr::LOCALHOST, 0, config("Host", 1)).unwrap();
        let port = host.local_addr().unwrap().port();
        let mut cfg = config("Joiner", 1);
        cfg.timeout = Duration::from_millis(300);
        let mut join = Session::join(loopback(port), cfg).unwrap();
        assert!(pump(&mut host, &mut join, |h, j| h.is_connected()
            && j.is_connected()));
        drop(host);
        let deadline = Instant::now() + Duration::from_secs(1);
        let mut ended = Vec::new();
        while Instant::now() < deadline {
            join.update();
            while let Some(e) = join.next_event() {
                if let Event::Disconnected(r) = e {
                    ended.push(r);
                }
            }
            std::thread::sleep(Duration::from_millis(10));
        }
        // Dropping the host sends Bye (three times, past a lossy link); the
        // joiner hears it leave once.
        assert_eq!(ended, vec!["the host left".to_owned()]);
    }

    #[test]
    fn a_silent_host_times_out() {
        let mut host = Session::host(Ipv4Addr::LOCALHOST, 0, config("Host", 1)).unwrap();
        let port = host.local_addr().unwrap().port();
        let mut cfg = config("Joiner", 1);
        cfg.timeout = Duration::from_millis(300);
        let mut join = Session::join(loopback(port), cfg).unwrap();
        assert!(pump(&mut host, &mut join, |h, j| h.is_connected()
            && j.is_connected()));
        // The host stops answering without a Bye (it is no longer updated).
        let deadline = Instant::now() + Duration::from_secs(3);
        while Instant::now() < deadline && !join.is_closed() {
            join.update();
            std::thread::sleep(Duration::from_millis(10));
        }
        let events: Vec<_> = std::iter::from_fn(|| join.next_event()).collect();
        assert_eq!(
            events.last(),
            Some(&Event::Disconnected("the host timed out".into()))
        );
        drop(host);
    }

    #[test]
    fn nobody_answering_ends_the_join() {
        // A socket that never answers (kept bound so no other test takes
        // its port).
        let silent = UdpSocket::bind("127.0.0.1:0").unwrap();
        let free = silent.local_addr().unwrap();
        let mut cfg = config("Joiner", 1);
        cfg.timeout = Duration::from_millis(300);
        let mut join = Session::join(free, cfg).unwrap();
        let deadline = Instant::now() + Duration::from_secs(3);
        while Instant::now() < deadline && !join.is_closed() {
            join.update();
            std::thread::sleep(Duration::from_millis(10));
        }
        assert!(
            matches!(join.next_event(), Some(Event::Disconnected(r)) if r.contains("no answer"))
        );
    }

    #[test]
    fn a_slow_load_does_not_use_up_the_join_s_wait() {
        let silent = UdpSocket::bind("127.0.0.1:0").unwrap();
        let free = silent.local_addr().unwrap();
        let mut cfg = config("Joiner", 1);
        cfg.timeout = Duration::from_millis(100);
        let mut join = Session::join(free, cfg).unwrap();
        std::thread::sleep(Duration::from_millis(250));
        join.update();
        assert!(!join.is_closed());
        std::thread::sleep(Duration::from_millis(150));
        join.update();
        assert!(join.is_closed());
    }

    #[test]
    fn the_browser_finds_a_local_host() {
        let host = Session::host(Ipv4Addr::LOCALHOST, 0, config("Host", 1)).unwrap();
        let port = host.local_addr().unwrap().port();
        let mut host = host;
        let mut browser = Browser::new(port, [1; 32]).unwrap();
        let mut found = Vec::new();
        for _ in 0..200 {
            browser.update();
            host.update();
            browser.update();
            found = browser.found().to_vec();
            if !found.is_empty() {
                break;
            }
            std::thread::sleep(Duration::from_millis(10));
        }
        assert_eq!(found.len(), 1, "{found:?}");
        assert_eq!(found[0].name, "Host");
        assert!(found[0].compatible && found[0].open);
        // Answered within the ask's second: its round trip is known.
        assert!(found[0].ping_ms.is_some_and(|ms| ms < 1000), "{found:?}");
    }

    #[test]
    fn lossy_link_still_delivers_reliable_messages() {
        let mut host = Session::host(Ipv4Addr::LOCALHOST, 0, config("Host", 1)).unwrap();
        let port = host.local_addr().unwrap().port();
        let mut join = Session::join(loopback(port), config("Joiner", 1)).unwrap();
        assert!(pump(&mut host, &mut join, |h, j| h.is_connected()
            && j.is_connected()));
        let lossy = Conditions {
            latency_ms: 20,
            jitter_ms: 5,
            loss_percent: 30.,
        };
        host.set_conditions(lossy);
        join.set_conditions(lossy);
        for i in 0..20u8 {
            join.send(RELIABLE, vec![i]);
        }
        let mut got = Vec::new();
        assert!(pump(&mut host, &mut join, |h, _| {
            while let Some(m) = h.receive(RELIABLE) {
                got.push(m[0]);
            }
            got.len() == 20
        }));
        assert_eq!(got, (0..20).collect::<Vec<_>>());
    }
}
