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
/// Overlay text: size, line height, the top-right corner (below the duel's
/// HP gauge) and colours.
const SIZE: f32 = 10.;
const LINE: f32 = 12.;
const RIGHT: f32 = 590.;
const TOP: f32 = 46.;
const TEXT: [f32; 4] = [1.6, 1.6, 1.6, 1.];
const WARN: [f32; 4] = [1.8, 1.3, 0.5, 1.];
/// The vote's list: size, line height, and the cursor row's colour.
const LOBBY_SIZE: f32 = 12.;
const LOBBY_LINE: f32 = 15.;
const CURSOR: [f32; 4] = [1.9, 1.6, 0.7, 1.];
/// The vote's panel top.
const VOTE_TOP: f32 = 290.;
const BACKING: [f32; 4] = [0., 0., 0., 0.55];
/// The banner ("Waiting for a player", "Opponent left"): its size and row.
const BANNER_SIZE: f32 = 16.;
const BANNER_Y: f32 = 120.;
/// The middle of the 512-wide layout.
const BANNER_CENTRE: f32 = 256.;

/// Where the session stands, for the banner.
#[derive(Clone, Debug, PartialEq)]
pub(crate) enum Status {
    /// Host: waiting for a player on this port.
    Waiting(u16),
    /// Joiner: knocking at the host.
    Joining(SocketAddr),
    Playing,
    /// The session ended at this tick; the reason to show.
    Ended(String, u64),
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
    /// This tick's reliable messages, each taken by the system it is for
    /// (the rest are dropped at the end of the tick).
    pub inbox: Vec<Vec<u8>>,
    /// Frames since the link's numbers were last logged.
    stats_frames: u32,
    /// Joiner: the host's events by host tick, waiting for that tick's
    /// snapshot.
    pub events: VecDeque<(u64, Vec<duel_wire::DuelEvent>)>,
    /// Ticks the host had no new buttons for (late input) and ticks the
    /// joiner had no new snapshot for; reset each overlay refresh.
    pub starved: u32,
    pub stale: u32,
    /// Ticks in a row without them (the overlay warns at half a second).
    pub starve_run: u32,
    pub stale_run: u32,
    /// The link simulator's preset (`SIMULATED`), or None for the one
    /// `--net-sim` gave.
    pub preset: Option<usize>,
    /// Snapshots skipped to catch up, inputs dropped as too old.
    pub skipped: u64,
    /// Host: the rules last sent to the joiner.
    pub sent_rules: Option<duel_rules::MatchRules>,
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

/// The F3 menu's link simulator presets: name and conditions (what this
/// instance sends; choose the same on both for a round trip).
pub(crate) const SIMULATED: [(&str, Conditions); 4] = [
    (
        "Off",
        Conditions {
            latency_ms: 0,
            jitter_ms: 0,
            loss_percent: 0.,
        },
    ),
    (
        "Wi-Fi",
        Conditions {
            latency_ms: 15,
            jitter_ms: 5,
            loss_percent: 0.5,
        },
    ),
    (
        "Busy Wi-Fi",
        Conditions {
            latency_ms: 50,
            jitter_ms: 20,
            loss_percent: 3.,
        },
    ),
    (
        "Bad link",
        Conditions {
            latency_ms: 120,
            jitter_ms: 40,
            loss_percent: 10.,
        },
    ),
];

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
            starve_run: 0,
            stale_run: 0,
            preset: setup.conditions.is_clear().then_some(0),
            skipped: 0,
            sent_rules: None,
            events: VecDeque::new(),
            inbox: Vec::new(),
            stats_frames: 0,
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
/// Whether an address is this PC's own (loopback, or one of its
/// interfaces: binding to it works).
pub(crate) fn is_local(ip: std::net::IpAddr) -> bool {
    ip.is_loopback() || std::net::UdpSocket::bind((ip, 0)).is_ok()
}

/// `--input auto` (the default): two players on one PC each get their own
/// devices, the host the keyboard and mouse and the joiner a controller
/// [rewrite choice, T256]; otherwise each reads all of them.
fn assign_devices(controls: &mut visual::input::Controls, role: Role, peer: SocketAddr) -> bool {
    use visual::input::{Assign, Devices};
    if controls.wanted != Assign::Auto {
        return false;
    }
    let devices = match (is_local(peer.ip()), role) {
        (true, Role::Host) => Devices::KeyboardMouse,
        (true, Role::Join) => Devices::Pad(None),
        (false, _) => Devices::Both,
    };
    let changed = controls.devices != devices;
    controls.devices = devices;
    changed
}

pub(crate) fn begin(
    net: Option<ResMut<DuelNet>>,
    mut controls: ResMut<visual::input::Controls>,
    mut game: ResMut<Game>,
    mut loadout: ResMut<duel_loadout::DuelLoadout>,
) {
    let Some(mut net) = net else { return };
    net.session.update();
    net.inbox.clear();
    while let Some(m) = net.session.receive(net::RELIABLE) {
        net.inbox.push(m);
    }
    while let Some(event) = net.session.next_event() {
        let tick = game.tick;
        match event {
            NetEvent::Connected(peer) => {
                if assign_devices(&mut controls, net.role, peer.addr) {
                    let devices = controls.devices.label();
                    game.event(serde_json::json!({"event":"input_devices","tick":tick,
                        "devices":devices}));
                }
                net.peer = Some(peer.name.clone());
                net.status = Status::Playing;
                net.inputs.clear();
                net.used_input = None;
                net.snapshots.clear();
                net.shown_snapshot = None;
                net.events.clear();
                net.sent_rules = None;
                let slot = remote_slot(net.role);
                rival::spawn_remote(&mut game, slot);
                // The host's Drive rolls (Antiform) start from the clock, a
                // different sequence each session; each Sora draws his own.
                if net.role == Role::Host {
                    let clock = std::time::SystemTime::now()
                        .duration_since(std::time::UNIX_EPOCH)
                        .map_or(1, |d| d.subsec_nanos() ^ d.as_secs() as u32);
                    let g = &mut *game;
                    let drives = g
                        .drive
                        .as_mut()
                        .into_iter()
                        .chain(g.rivals.iter_mut().filter_map(|r| r.own.drive.as_mut()));
                    for (k, d) in drives.enumerate() {
                        d.seed = clock.wrapping_add(k as u32 * 0x9e37_79b9) | 1;
                    }
                }
                game.event(serde_json::json!({"event":"net_connected","tick":tick,
                    "peer":peer.name,"address":peer.addr.to_string()}));
            }
            NetEvent::Rejected(reason) => {
                game.event(serde_json::json!({"event":"net_rejected","tick":tick,"reason":reason}));
                net.status = Status::Ended(format!("The host refused: {reason}"), tick);
                leave_session(&mut game, &mut loadout);
            }
            NetEvent::Disconnected(reason) => {
                game.event(
                    serde_json::json!({"event":"net_disconnected","tick":tick,"reason":reason}),
                );
                game.rivals.clear();
                // The name stays for the result screen ("X left the duel").
                let text = match (net.role, net.peer.clone()) {
                    // The host keeps listening for the next player.
                    (Role::Host, who) => format!(
                        "{} left: {reason}. Waiting for a player",
                        who.as_deref().unwrap_or("The other player")
                    ),
                    (Role::Join, Some(_)) => format!("Opponent left: {reason}. Esc to quit"),
                    (Role::Join, None) => format!("Could not join: {reason}"),
                };
                net.status = Status::Ended(text, tick);
                if net.role == Role::Join {
                    leave_session(&mut game, &mut loadout);
                }
            }
        }
    }
    // A host whose player left goes back to waiting once the banner was read.
    if net.role == Role::Host
        && let Status::Ended(_, since) = net.status
        && game.tick.saturating_sub(since) >= BANNER_TICKS
    {
        let port = net.session.local_addr().map(|a| a.port()).unwrap_or(0);
        net.status = Status::Waiting(port);
        net.peer = None;
    }
}

/// How long the host shows why its player left before waiting again.
const BANNER_TICKS: u64 = 5 * 60;

/// The joiner's session ended: its Sora is its own again (the field
/// simulates him) and the room pauses as usual.
fn leave_session(game: &mut Game, loadout: &mut duel_loadout::DuelLoadout) {
    game.net_client = false;
    game.net_pad = rival::RivalPad::default();
    game.duel = false;
    game.rivals.clear();
    duel_loadout::restore(game, loadout);
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
pub(crate) fn sample(net: Option<ResMut<DuelNet>>, mut game: ResMut<Game>) {
    let Some(mut net) = net else { return };
    // The link's numbers in the log every 600 frames (the duel's
    // measurements).
    net.stats_frames += 1;
    if net.stats_frames >= 600 && net.session.is_connected() {
        net.stats_frames = 0;
        let s = net.session.stats();
        let tick = game.tick;
        game.event(
            serde_json::json!({"event":"net_stats","tick":tick,"rtt_ms":s.rtt_ms,
            "loss":s.packet_loss,"send_kbps":s.sent_bytes_per_sec / 1024.,
            "recv_kbps":s.received_bytes_per_sec / 1024.}),
        );
    }
    if net.overlay != game.net_overlay {
        net.overlay = game.net_overlay;
    }
    // F3 > Sora > Display > Simulated network.
    if let Some(preset) = game.net_simulated
        && net.preset != Some(preset)
        && let Some(&(name, conditions)) = SIMULATED.get(preset)
    {
        net.preset = Some(preset);
        net.session.set_conditions(conditions);
        net.warn(format!("Simulated network: {name}"));
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
                "Simulated +{} ms, jitter {} ms, {:.0}% lost, {} dropped",
                c.latency_ms, c.jitter_ms, c.loss_percent, s.simulated_drops
            ),
            true,
        ));
    }
    let warnings: Vec<String> = net.warnings.iter().cloned().collect();
    lines.extend(warnings.into_iter().map(|w| (w, true)));
    lines.push((
        format!(
            "Damage x{}  magic x{}  cap {}   tick {}",
            game.duel_rules.damage,
            game.duel_rules.magic,
            cap_text(game.duel_rules.hit_cap),
            game.tick
        ),
        false,
    ));
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
    faded_backing(white, rect, 1.)
}

/// A rectangle in one colour.
fn tinted(rect: [f32; 4], c: [f32; 4]) -> data::sprite_sequence::Quad {
    let [x0, y0, x1, y1] = rect;
    data::sprite_sequence::Quad {
        corners: [[x0, y0], [x1, y0], [x0, y1], [x1, y1]],
        uv: [0, 0, 1, 1],
        colors: [c; 4],
        blend: 0,
    }
}

/// Past every edge of the 2D layer (the flashes).
const SCREEN: [f32; 4] = [-512., -256., 1024., 768.];
/// The tie's coin: where, how large, its rim.
const COIN_CENTRE: [f32; 2] = [256., 150.];
const COIN_RADIUS: f32 = 26.;
const COIN_RIM: [f32; 4] = [1.1, 0.8, 0.3, 1.];

/// A disc `width` times as wide as it is tall, in horizontal strips.
fn disc(centre: [f32; 2], r: f32, width: f32, c: [f32; 4]) -> Vec<data::sprite_sequence::Quad> {
    const STRIPS: usize = 20;
    let half = |y: f32| r * width * (1. - (y / r).powi(2)).max(0.).sqrt();
    (0..STRIPS)
        .map(|i| {
            let y0 = -r + 2. * r * i as f32 / STRIPS as f32;
            let y1 = -r + 2. * r * (i + 1) as f32 / STRIPS as f32;
            let [cx, cy] = centre;
            data::sprite_sequence::Quad {
                corners: [
                    [cx - half(y0), cy + y0],
                    [cx + half(y0), cy + y0],
                    [cx - half(y1), cy + y1],
                    [cx + half(y1), cy + y1],
                ],
                uv: [0, 0, 1, 1],
                colors: [c; 4],
                blend: 0,
            }
        })
        .collect()
}

/// The backing at an opacity (the lobby's fade).
fn faded_backing(
    white: usize,
    rect: [f32; 4],
    alpha: f32,
) -> (usize, Vec<data::sprite_sequence::Quad>) {
    let [x0, y0, x1, y1] = rect;
    let c = [BACKING[0], BACKING[1], BACKING[2], BACKING[3] * alpha];
    (
        white,
        vec![data::sprite_sequence::Quad {
            corners: [[x0, y0], [x1, y0], [x0, y1], [x1, y1]],
            uv: [0, 0, 1, 1],
            colors: [c; 4],
            blend: 0,
        }],
    )
}

/// A hit cap as the lobby and the overlay show it.
pub(crate) fn cap_text(cap: u8) -> String {
    if cap >= 100 {
        "Off".into()
    } else {
        format!("{cap}%")
    }
}

/// The overlay at the top right and the session banner in the middle, on
/// the 2D layer.
#[allow(clippy::too_many_arguments)]
pub(crate) fn present(
    net: Option<ResMut<DuelNet>>,
    vote: Option<Res<duel_vote::VoteRoom>>,
    duel_match: Option<Res<duel_match_room::MatchRoom>>,
    camp: Option<Res<CampRes>>,
    game: Res<Game>,
    hud: Option<Res<Hud>>,
    fixed: Res<Time<Fixed>>,
    mut frame: ResMut<Ui2dFrame>,
    mut images: ResMut<Assets<Image>>,
) {
    let Some(mut net) = net else { return };
    // The part of a fixed tick since the last: the cosmetics below follow
    // the tick, smoothly between ticks.
    let frac = fixed.overstep_fraction().clamp(0., 1.);
    let Some(font) = hud.as_ref().and_then(|h| h.menu.as_ref()).map(|m| &m.font) else {
        return;
    };
    let (sheet, white) = sheets(&mut net, &mut frame, &mut images, font);
    let f = field_command_menu::Font {
        widths: &font.widths,
        pad: 1,
    };
    let banner = match &net.status {
        Status::Waiting(port) => Some(format!("Waiting for a player on UDP port {port}")),
        Status::Joining(host) => Some(format!("Joining {host}...")),
        Status::Playing => None,
        Status::Ended(reason, _) => Some(reason.clone()),
    };
    // A match's result says who left; the banner gives way to it.
    let result_up = duel_match.as_ref().is_some_and(|m| m.menu_up());
    if let Some(text) = banner.filter(|_| !result_up) {
        let encoded = pause_menu::encode_text(&text);
        let width = field_command_menu::text_width(&encoded, &f, BANNER_SIZE);
        let x = BANNER_CENTRE - width / 2.;
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
    // The match: countdown, clock, rounds and the result, centred (with the
    // pause menu's art the result's choices and a vote are in the menu).
    let in_camp = camp.is_some();
    // The match's text is over the field, and over the result's own top
    // level; any other menu covers it.
    let covered = camp
        .as_ref()
        .and_then(|c| c.open.as_ref())
        .is_some_and(|m| {
            let result = m
                .data
                .duel
                .as_ref()
                .is_some_and(|d| d.owner.0 == duel_camp::RESULT);
            !(result && m.state == pause_menu::State::Top)
        });
    if let Some(c) = duel_match
        .as_ref()
        .filter(|_| !covered)
        .and_then(|m| duel_match_room::flash(m, &net, &game, frac))
    {
        frame.draws.push((white, vec![tinted(SCREEN, c)]));
    }
    let labels = duel_match
        .filter(|_| !covered)
        .map(|m| duel_match_room::labels(&m, &net, &game, in_camp))
        .unwrap_or_default();
    for l in &labels {
        let width = field_command_menu::text_width(&l.text, &f, l.size);
        let x = l.centre[0] - width / 2.;
        let y = l.centre[1] - l.size / 2.;
        if l.backed {
            frame.draws.push(backing(
                white,
                [x - 8., y - 3., x + width + 8., y + l.size + 3.],
            ));
        }
        frame.draws.push((
            sheet,
            field_command_menu::text_quads_sized(&l.text, [x, y], l.colour, &f, l.size),
        ));
    }
    // A tie's coin, over the vote.
    if let Some((width, size)) = vote.as_ref().and_then(|v| duel_vote::coin(v, &game, frac)) {
        let r = COIN_RADIUS * size;
        frame
            .draws
            .push((white, disc(COIN_CENTRE, r, width, COIN_RIM)));
        frame
            .draws
            .push((white, disc(COIN_CENTRE, r * 0.82, width, CURSOR)));
    }
    // A vote without the pause menu's art: its own panel.
    let vote_lines: Vec<(Vec<u8>, bool)> = vote
        .filter(|_| !in_camp)
        .map(|v| duel_vote::lines(&v, &net, &game))
        .unwrap_or_default()
        .into_iter()
        .map(|(l, on)| (pause_menu::encode_text(&l), on))
        .collect();
    if !vote_lines.is_empty() {
        let width = vote_lines
            .iter()
            .map(|(l, _)| field_command_menu::text_width(l, &f, LOBBY_SIZE))
            .fold(0., f32::max);
        let x = BANNER_CENTRE - width / 2.;
        frame.draws.push(backing(
            white,
            [
                x - 10.,
                VOTE_TOP - 6.,
                x + width + 10.,
                VOTE_TOP + LOBBY_LINE * vote_lines.len() as f32 + 4.,
            ],
        ));
        let mut quads = Vec::new();
        for (n, (line, on)) in vote_lines.iter().enumerate() {
            quads.extend(field_command_menu::text_quads_sized(
                line,
                [x, VOTE_TOP + n as f32 * LOBBY_LINE],
                if *on { CURSOR } else { TEXT },
                &f,
                LOBBY_SIZE,
            ));
        }
        frame.draws.push((sheet, quads));
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
