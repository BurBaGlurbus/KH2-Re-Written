//! The duel's LAN session in the room: hosting or joining (`--host`,
//! `--join`), the session's own tick at the start and end of every fixed
//! step, the other player arriving and leaving, and the network overlay
//! (F3 > Sora > Display, or `--net-overlay`).
//!
//! The host simulates both Soras; the joiner sends its buttons and draws
//! the host's snapshots (duel_sync.rs).

use super::*;
use net::link::Conditions;
use net::{Event as NetEvent, Role, Session, SessionConfig};
use std::collections::{BTreeMap, VecDeque};
use std::net::SocketAddr;
use std::time::Duration;

/// How long a silent peer is waited for.
const TIMEOUT: Duration = Duration::from_secs(5);
/// Overlay text: size, line height, the top-right corner and colours.
const SIZE: f32 = 10.;
const LINE: f32 = 12.;
const RIGHT: f32 = 590.;
const TOP: f32 = 34.;
const TEXT: [f32; 4] = [1.6, 1.6, 1.6, 1.];
const WARN: [f32; 4] = [1.8, 1.3, 0.5, 1.];
const BACKING: [f32; 4] = [0., 0., 0., 0.55];
/// The banner ("Waiting for a player", "Opponent left"): its size and row.
const BANNER_SIZE: f32 = 16.;
const BANNER_Y: f32 = 120.;

/// Where the session stands, for the banner.
#[derive(Clone, Debug, PartialEq)]
pub(crate) enum Status {
    /// Host: waiting for a player on this port.
    Waiting(u16),
    /// Joiner: knocking at the host.
    Joining(SocketAddr),
    Playing,
    /// The session ended; the reason to show.
    Ended(String),
}

/// What `--host` / `--join` asked for.
#[derive(Clone, Debug)]
pub(crate) struct Setup {
    pub role: Role,
    /// The host's address to listen on (every local one by default).
    pub bind: std::net::Ipv4Addr,
    pub port: u16,
    pub host: Option<SocketAddr>,
    pub name: String,
    pub conditions: Conditions,
    pub overlay: bool,
}

#[derive(Resource)]
pub(crate) struct DuelNet {
    pub session: Session,
    pub role: Role,
    pub name: String,
    pub peer: Option<String>,
    pub status: Status,
    pub overlay: bool,
    /// Joiner: its newest ticks of buttons (redundancy).
    pub recent: VecDeque<(u32, rival::RivalPad)>,
    /// Host: the joiner's buttons by joiner tick, not yet used.
    pub inputs: BTreeMap<u32, rival::RivalPad>,
    /// Host: the newest joiner tick used.
    pub used_input: Option<u32>,
    /// Joiner: snapshots by host tick, waiting to be shown.
    pub snapshots: BTreeMap<u64, duel_wire::Snapshot>,
    pub shown_snapshot: Option<u64>,
    /// Ticks the host had no new buttons for (late input) and ticks the
    /// joiner had no new snapshot for; reset each overlay refresh.
    pub starved: u32,
    pub stale: u32,
    /// Snapshots skipped to catch up, inputs dropped as too old.
    pub skipped: u64,
    pub warnings: VecDeque<String>,
    since_refresh: u32,
    lines: Vec<(String, bool)>,
    sheets: Option<(usize, usize)>,
}

/// SHA-256 of the running executable: two instances play together only on
/// the same build.
pub(crate) fn build_identity() -> Result<[u8; 32], String> {
    use sha2::{Digest, Sha256};
    let exe = std::env::current_exe().map_err(|e| e.to_string())?;
    let bytes = std::fs::read(&exe).map_err(|e| format!("{}: {e}", exe.display()))?;
    Ok(Sha256::digest(&bytes).into())
}

/// The Windows user name, the default player name.
pub(crate) fn default_name() -> String {
    std::env::var("USERNAME")
        .ok()
        .map(|n| n.trim().to_owned())
        .filter(|n| !n.is_empty())
        .unwrap_or_else(|| "Player".into())
}

/// `HOST[:PORT]` (the default port when none is given).
pub(crate) fn parse_address(text: &str) -> Result<SocketAddr, String> {
    use std::net::ToSocketAddrs;
    let text = text.trim();
    let with_port = if text.contains(':') {
        text.to_owned()
    } else {
        format!("{text}:{}", net::DEFAULT_PORT)
    };
    with_port
        .to_socket_addrs()
        .map_err(|e| format!("{text}: {e}"))?
        .find(SocketAddr::is_ipv4)
        .ok_or_else(|| format!("{text}: no IPv4 address"))
}

/// `LATENCY,JITTER,LOSS` (milliseconds, milliseconds, percent).
pub(crate) fn parse_conditions(text: &str) -> Result<Conditions, String> {
    let parts: Vec<&str> = text.split(',').map(str::trim).collect();
    let [latency, jitter, loss] = parts.as_slice() else {
        return Err("expected LATENCY,JITTER,LOSS (ms, ms, percent)".into());
    };
    let ms = |s: &str| s.parse::<u32>().map_err(|e| format!("{s}: {e}"));
    let loss = loss.parse::<f32>().map_err(|e| format!("{loss}: {e}"))?;
    if !(0. ..=100.).contains(&loss) {
        return Err("loss must be 0..100".into());
    }
    Ok(Conditions {
        latency_ms: ms(latency)?,
        jitter_ms: ms(jitter)?,
        loss_percent: loss,
    })
}

impl DuelNet {
    pub(crate) fn start(setup: &Setup) -> Result<DuelNet, String> {
        let build = build_identity()?;
        let config = SessionConfig {
            name: setup.name.clone(),
            build,
            timeout: TIMEOUT,
        };
        let (session, status) = match (setup.role, setup.host) {
            (Role::Host, _) => (
                Session::host(setup.bind, setup.port, config)
                    .map_err(|e| format!("could not host on UDP port {}: {e}", setup.port))?,
                Status::Waiting(setup.port),
            ),
            (Role::Join, Some(host)) => (
                Session::join(host, config).map_err(|e| format!("could not join {host}: {e}"))?,
                Status::Joining(host),
            ),
            (Role::Join, None) => return Err("join needs the host's address".into()),
        };
        let mut session = session;
        session.set_conditions(setup.conditions);
        Ok(DuelNet {
            session,
            role: setup.role,
            name: setup.name.clone(),
            peer: None,
            status,
            overlay: setup.overlay,
            recent: VecDeque::new(),
            inputs: BTreeMap::new(),
            used_input: None,
            snapshots: BTreeMap::new(),
            shown_snapshot: None,
            starved: 0,
            stale: 0,
            skipped: 0,
            warnings: VecDeque::new(),
            since_refresh: 0,
            lines: Vec::new(),
            sheets: None,
        })
    }

    pub(crate) fn playing(&self) -> bool {
        self.status == Status::Playing
    }

    fn warn(&mut self, text: String) {
        if self.warnings.len() >= 4 {
            self.warnings.pop_front();
        }
        self.warnings.push_back(text);
    }
}

/// The slot this instance's own Sora takes.
pub(crate) fn local_slot(role: Role) -> u8 {
    match role {
        Role::Host => fighter_slots::HOST,
        Role::Join => fighter_slots::JOIN,
    }
}

/// The slot the other player's Sora takes.
pub(crate) fn remote_slot(role: Role) -> u8 {
    match role {
        Role::Host => fighter_slots::JOIN,
        Role::Join => fighter_slots::HOST,
    }
}

pub(crate) mod fighter_slots {
    pub const HOST: u8 = 0;
    pub const JOIN: u8 = 1;
}

/// Start of every fixed step: what arrived, the other player coming and
/// going.
pub(crate) fn begin(net: Option<ResMut<DuelNet>>, mut game: ResMut<Game>) {
    let Some(mut net) = net else { return };
    net.session.update();
    while let Some(event) = net.session.next_event() {
        let tick = game.tick;
        match event {
            NetEvent::Connected(peer) => {
                net.peer = Some(peer.name.clone());
                net.status = Status::Playing;
                net.inputs.clear();
                net.used_input = None;
                net.snapshots.clear();
                net.shown_snapshot = None;
                let slot = remote_slot(net.role);
                rival::spawn_remote(&mut game, slot);
                game.event(serde_json::json!({"event":"net_connected","tick":tick,
                    "peer":peer.name,"address":peer.addr.to_string()}));
            }
            NetEvent::Rejected(reason) => {
                game.event(serde_json::json!({"event":"net_rejected","tick":tick,"reason":reason}));
                net.status = Status::Ended(format!("The host refused: {reason}"));
            }
            NetEvent::Disconnected(reason) => {
                game.event(
                    serde_json::json!({"event":"net_disconnected","tick":tick,"reason":reason}),
                );
                game.rivals.clear();
                let who = net.peer.take().unwrap_or_else(|| "The other player".into());
                net.status = Status::Ended(match net.role {
                    // The host keeps listening for the next player.
                    Role::Host => format!("{who} left ({reason}). Waiting for a player"),
                    Role::Join => format!("Opponent left: {reason}"),
                });
            }
        }
    }
    // A host whose player left goes back to waiting once the banner was read.
    if net.role == Role::Host
        && !net.session.is_connected()
        && matches!(net.status, Status::Ended(_))
        && game.tick.is_multiple_of(60 * 5)
    {
        let port = net.session.local_addr().map(|a| a.port()).unwrap_or(0);
        net.status = Status::Waiting(port);
    }
}

/// End of every fixed step: what this tick queued goes out.
pub(crate) fn end(net: Option<ResMut<DuelNet>>) {
    if let Some(mut net) = net {
        net.session.update();
    }
}

/// Leaves cleanly when the room closes (the other side hears Bye at once).
pub(crate) fn close_on_exit(mut exits: MessageReader<AppExit>, net: Option<ResMut<DuelNet>>) {
    if exits.read().next().is_some()
        && let Some(mut net) = net
    {
        net.session.close();
    }
}

/// Rebuilds the overlay lines a few times a second.
pub(crate) fn sample(net: Option<ResMut<DuelNet>>, game: Res<Game>) {
    let Some(mut net) = net else { return };
    if net.overlay != game.net_overlay {
        net.overlay = game.net_overlay;
    }
    net.since_refresh += 1;
    if net.since_refresh < 15 && !net.lines.is_empty() {
        return;
    }
    net.since_refresh = 0;
    let s = net.session.stats();
    let c = net.session.conditions();
    let role = match net.role {
        Role::Host => "Host",
        Role::Join => "Joined",
    };
    let mut lines = vec![(
        format!(
            "{role}: {} vs {}",
            net.name,
            net.peer.as_deref().unwrap_or("-")
        ),
        false,
    )];
    if net.session.is_connected() {
        lines.push((
            format!(
                "Ping {:.0} ms   loss {:.1}%",
                s.rtt_ms,
                s.packet_loss * 100.
            ),
            s.packet_loss > 0.05,
        ));
        lines.push((
            format!(
                "Send {:.1} KB/s   recv {:.1} KB/s",
                s.sent_bytes_per_sec / 1024.,
                s.received_bytes_per_sec / 1024.
            ),
            false,
        ));
        match net.role {
            Role::Host => lines.push((
                format!("Buttons queued {}   late {}", net.inputs.len(), net.starved),
                net.starved > 3,
            )),
            Role::Join => {
                let age = net
                    .shown_snapshot
                    .zip(net.snapshots.last_key_value().map(|(k, _)| *k))
                    .map_or(0, |(shown, newest)| newest.saturating_sub(shown));
                lines.push((
                    format!(
                        "Snapshots buffered {}   behind {age}   stale {}",
                        net.snapshots.len(),
                        net.stale
                    ),
                    net.stale > 3,
                ));
            }
        }
    }
    if !c.is_clear() {
        lines.push((
            format!(
                "Simulated: +{} ms ±{} ms, {:.0}% lost ({} dropped)",
                c.latency_ms, c.jitter_ms, c.loss_percent, s.simulated_drops
            ),
            true,
        ));
    }
    let warnings: Vec<String> = net.warnings.iter().cloned().collect();
    lines.extend(warnings.into_iter().map(|w| (w, true)));
    lines.push((format!("Tick {}", game.tick), false));
    net.starved = 0;
    net.stale = 0;
    net.lines = lines;
}

/// Adds a late or skipped-step warning to the overlay.
pub(crate) fn warn(net: &mut DuelNet, text: String) {
    net.warn(text);
}

fn sheets(
    net: &mut DuelNet,
    frame: &mut Ui2dFrame,
    images: &mut Assets<Image>,
    font: &SystemFont,
) -> (usize, usize) {
    *net.sheets.get_or_insert_with(|| {
        (
            frame.add_sheet(
                images,
                font.rgba.clone(),
                font.size,
                font.texel_scale as f32,
            ),
            frame.add_sheet(images, vec![255; 16], [2, 2], 1.),
        )
    })
}

fn backing(white: usize, rect: [f32; 4]) -> (usize, Vec<data::sprite_sequence::Quad>) {
    let [x0, y0, x1, y1] = rect;
    (
        white,
        vec![data::sprite_sequence::Quad {
            corners: [[x0, y0], [x1, y0], [x0, y1], [x1, y1]],
            uv: [0, 0, 1, 1],
            colors: [BACKING; 4],
            blend: 0,
        }],
    )
}

/// The overlay at the top right and the session banner in the middle, on
/// the 2D layer.
pub(crate) fn present(
    net: Option<ResMut<DuelNet>>,
    hud: Option<Res<Hud>>,
    mut frame: ResMut<Ui2dFrame>,
    mut images: ResMut<Assets<Image>>,
) {
    let Some(mut net) = net else { return };
    let Some(font) = hud.as_ref().and_then(|h| h.menu.as_ref()).map(|m| &m.font) else {
        return;
    };
    let (sheet, white) = sheets(&mut net, &mut frame, &mut images, font);
    let f = field_command_menu::Font {
        widths: &font.widths,
        pad: 1,
    };
    let banner = match &net.status {
        Status::Waiting(port) => Some(format!("Waiting for a player (UDP port {port})")),
        Status::Joining(host) => Some(format!("Joining {host}...")),
        Status::Playing => None,
        Status::Ended(reason) => Some(reason.clone()),
    };
    if let Some(text) = banner {
        let encoded = pause_menu::encode_text(&text);
        let width = field_command_menu::text_width(&encoded, &f, BANNER_SIZE);
        let x = -width / 2.;
        frame.draws.push(backing(
            white,
            [
                x - 8.,
                BANNER_Y - 4.,
                x + width + 8.,
                BANNER_Y + BANNER_SIZE + 6.,
            ],
        ));
        frame.draws.push((
            sheet,
            field_command_menu::text_quads_sized(&encoded, [x, BANNER_Y], TEXT, &f, BANNER_SIZE),
        ));
    }
    if !net.overlay || net.lines.is_empty() {
        return;
    }
    let encoded: Vec<(Vec<u8>, bool)> = net
        .lines
        .iter()
        .map(|(l, warn)| (pause_menu::encode_text(l), *warn))
        .collect();
    let width = encoded
        .iter()
        .map(|(l, _)| field_command_menu::text_width(l, &f, SIZE))
        .fold(0., f32::max);
    let x = RIGHT - width;
    frame.draws.push(backing(
        white,
        [
            x - 4.,
            TOP - 2.,
            RIGHT + 4.,
            TOP + LINE * encoded.len() as f32 + 2.,
        ],
    ));
    let mut quads = Vec::new();
    for (n, (line, warn)) in encoded.iter().enumerate() {
        let colour = if *warn { WARN } else { TEXT };
        quads.extend(field_command_menu::text_quads_sized(
            line,
            [x, TOP + n as f32 * LINE],
            colour,
            &f,
            SIZE,
        ));
    }
    frame.draws.push((sheet, quads));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn addresses_take_the_default_port() {
        assert_eq!(
            parse_address("127.0.0.1").unwrap(),
            SocketAddr::from(([127, 0, 0, 1], net::DEFAULT_PORT))
        );
        assert_eq!(
            parse_address(" 192.168.1.20:5000 ").unwrap(),
            SocketAddr::from(([192, 168, 1, 20], 5000))
        );
        assert!(parse_address("not an address at all:x").is_err());
    }

    #[test]
    fn link_conditions_parse() {
        assert_eq!(
            parse_conditions("80, 20, 5").unwrap(),
            Conditions {
                latency_ms: 80,
                jitter_ms: 20,
                loss_percent: 5.
            }
        );
        assert!(parse_conditions("80,20").is_err());
        assert!(parse_conditions("80,20,150").is_err());
    }
}
