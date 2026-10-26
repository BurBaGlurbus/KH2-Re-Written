//! The duel's game messages over the LAN session: the joiner's buttons
//! (unreliable, with the last few ticks repeated so a lost datagram costs
//! nothing), the host's snapshots of both Soras (unreliable, newest wins),
//! and events that must arrive (reliable: hits to show, knock-outs).
//!
//! Only what drawing and the HUD need is sent; the host alone simulates.

use super::duel_rules::MatchRules;
use super::rival::{Command, RivalPad};
use net::wire::{Reader, Writer};
use player::Action;

const INPUT: u8 = 1;
const SNAPSHOT: u8 = 2;
const EVENTS: u8 = 3;
const RULES: u8 = 4;
/// The lobby (host to joiner), a joiner's lobby intent, and the host's
/// refusal of one (duel_lobby.rs).
pub(crate) const LOBBY: u8 = 5;
pub(crate) const INTENT: u8 = 6;
pub(crate) const REFUSED: u8 = 7;
/// A vote (host to joiner) and a joiner's ballot (duel_vote.rs).
pub(crate) const VOTE: u8 = 8;
pub(crate) const BALLOT: u8 = 9;
/// The match (host to joiner) and a joiner's rematch choice
/// (duel_match_room.rs).
pub(crate) const MATCH: u8 = 10;
pub(crate) const REMATCH: u8 = 11;

/// Ticks of buttons each input message repeats.
pub(crate) const INPUT_REDUNDANCY: usize = 6;

pub(crate) fn action_code(action: Action) -> (u8, u16) {
    let arg = |i: usize| u16::try_from(i).unwrap_or(u16::MAX);
    match action {
        Action::Idle => (0, 0),
        Action::Walk => (1, 0),
        Action::Run => (2, 0),
        Action::Jump => (3, 0),
        Action::Fall => (4, 0),
        Action::Land => (5, 0),
        Action::Attack(i) => (6, arg(i)),
        Action::AirAttack(i) => (7, arg(i)),
        Action::Cast(i) => (8, arg(i)),
        Action::AirCast(i) => (9, arg(i)),
        Action::Hurt(i) => (10, arg(i)),
        Action::Guard => (11, 0),
        Action::GuardRecoil => (12, 0),
        Action::WallRecoil => (13, 0),
        Action::Item => (14, 0),
        Action::Drive => (15, 0),
        Action::DriveIn => (16, 0),
        Action::HighJump => (17, 0),
        Action::Growth(m) => (18, m),
        Action::Scripted(m) => (19, m),
    }
}

pub(crate) fn action_from(code: u8, arg: u16) -> Option<Action> {
    let i = arg as usize;
    Some(match code {
        0 => Action::Idle,
        1 => Action::Walk,
        2 => Action::Run,
        3 => Action::Jump,
        4 => Action::Fall,
        5 => Action::Land,
        6 => Action::Attack(i),
        7 => Action::AirAttack(i),
        8 => Action::Cast(i),
        9 => Action::AirCast(i),
        10 => Action::Hurt(i),
        11 => Action::Guard,
        12 => Action::GuardRecoil,
        13 => Action::WallRecoil,
        14 => Action::Item,
        15 => Action::Drive,
        16 => Action::DriveIn,
        17 => Action::HighJump,
        18 => Action::Growth(arg),
        19 => Action::Scripted(arg),
        _ => return None,
    })
}

fn put_pad(w: &mut Writer, pad: &RivalPad) {
    w.f32(pad.direction[0]);
    w.f32(pad.direction[1]);
    let bits = u8::from(pad.jump)
        | u8::from(pad.jump_held) << 1
        | u8::from(pad.attack) << 2
        | u8::from(pad.guard) << 3
        | u8::from(pad.guard_held) << 4;
    w.u8(bits);
    let (tag, arg) = match pad.command {
        Command::None => (0, 0),
        Command::Cast(t) => (1, t),
        Command::Item(s) => (2, s),
        Command::Drive(f) => (3, f),
    };
    w.u8(tag);
    w.u8(arg);
}

fn get_pad(r: &mut Reader) -> Option<RivalPad> {
    let direction = [r.f32()?, r.f32()?];
    if !direction.iter().all(|v| v.is_finite()) {
        return None;
    }
    let bits = r.u8()?;
    let (tag, arg) = (r.u8()?, r.u8()?);
    let command = match tag {
        0 => Command::None,
        1 => Command::Cast(arg),
        2 => Command::Item(arg),
        3 => Command::Drive(arg),
        _ => return None,
    };
    Some(RivalPad {
        direction,
        jump: bits & 1 != 0,
        jump_held: bits & 2 != 0,
        attack: bits & 4 != 0,
        guard: bits & 8 != 0,
        guard_held: bits & 16 != 0,
        command,
    })
}

/// The joiner's buttons of its newest ticks, oldest first: (joiner tick,
/// pad).
pub(crate) fn encode_input(recent: &[(u32, RivalPad)]) -> Vec<u8> {
    let mut w = Writer::default();
    w.u8(INPUT);
    let recent = &recent[recent.len().saturating_sub(INPUT_REDUNDANCY)..];
    w.u8(recent.len() as u8);
    for (tick, pad) in recent {
        w.u32(*tick);
        put_pad(&mut w, pad);
    }
    w.finish()
}

pub(crate) fn decode_input(bytes: &[u8]) -> Option<Vec<(u32, RivalPad)>> {
    let mut r = Reader::new(bytes);
    if r.u8()? != INPUT {
        return None;
    }
    let n = r.u8()? as usize;
    if n > INPUT_REDUNDANCY {
        return None;
    }
    let mut out = Vec::with_capacity(n);
    for _ in 0..n {
        out.push((r.u32()?, get_pad(&mut r)?));
    }
    r.is_empty().then_some(out)
}

/// One Sora as the other instance draws him.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct FighterState {
    pub slot: u8,
    pub position: [f32; 3],
    pub yaw: f32,
    pub action: Action,
    pub frame: u32,
    pub armed: bool,
    pub blend: f32,
    /// The host tick the action started (a change restarts the blend).
    pub started: u64,
    pub hp: i32,
    pub max_hp: i32,
    pub mp: [i32; 2],
    pub drive: [u8; 4],
    pub invulnerable: f32,
    /// The Drive form his model shows (0 = none).
    pub form: u8,
    /// His item slots (0 = used up).
    pub items: Vec<u16>,
}

/// The most item slots a snapshot carries.
pub(crate) const MAX_ITEM_SLOTS: usize = 8;

/// The host's picture of one tick.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Snapshot {
    pub tick: u64,
    pub fighters: Vec<FighterState>,
}

pub(crate) fn encode_snapshot(s: &Snapshot) -> Vec<u8> {
    let mut w = Writer::default();
    w.u8(SNAPSHOT);
    w.u64(s.tick);
    w.u8(s.fighters.len() as u8);
    for f in &s.fighters {
        w.u8(f.slot);
        for v in f.position {
            w.f32(v);
        }
        w.f32(f.yaw);
        let (code, arg) = action_code(f.action);
        w.u8(code);
        w.u16(arg);
        w.u32(f.frame);
        w.u8(u8::from(f.armed));
        w.f32(f.blend);
        w.u64(f.started);
        w.i32(f.hp);
        w.i32(f.max_hp);
        w.i32(f.mp[0]);
        w.i32(f.mp[1]);
        w.bytes(&f.drive);
        w.f32(f.invulnerable);
        w.u8(f.form);
        let items = &f.items[..f.items.len().min(MAX_ITEM_SLOTS)];
        w.u8(items.len() as u8);
        for &id in items {
            w.u16(id);
        }
    }
    w.finish()
}

pub(crate) fn decode_snapshot(bytes: &[u8]) -> Option<Snapshot> {
    let mut r = Reader::new(bytes);
    if r.u8()? != SNAPSHOT {
        return None;
    }
    let tick = r.u64()?;
    let n = r.u8()? as usize;
    if n > 8 {
        return None;
    }
    let mut fighters = Vec::with_capacity(n);
    for _ in 0..n {
        let slot = r.u8()?;
        let position = [r.f32()?, r.f32()?, r.f32()?];
        let yaw = r.f32()?;
        let action = action_from(r.u8()?, r.u16()?)?;
        let frame = r.u32()?;
        let armed = r.u8()? != 0;
        let blend = r.f32()?;
        let started = r.u64()?;
        let hp = r.i32()?;
        let max_hp = r.i32()?;
        let mp = [r.i32()?, r.i32()?];
        let drive = r.array()?;
        let invulnerable = r.f32()?;
        let form = r.u8()?;
        let count = r.u8()? as usize;
        if count > MAX_ITEM_SLOTS {
            return None;
        }
        let items = (0..count).map(|_| r.u16()).collect::<Option<Vec<_>>>()?;
        if !position.iter().chain([&yaw, &blend]).all(|v| v.is_finite()) {
            return None;
        }
        fighters.push(FighterState {
            slot,
            position,
            yaw,
            action,
            frame,
            armed,
            blend,
            started,
            hp,
            max_hp,
            mp,
            drive,
            invulnerable,
            form,
            items,
        });
    }
    r.is_empty().then_some(Snapshot { tick, fighters })
}

/// The host's match rules (reliable, on connecting and on every change).
pub(crate) fn encode_rules(rules: &MatchRules) -> Vec<u8> {
    let mut w = Writer::default();
    w.u8(RULES);
    w.u8(rules.mode_code());
    w.f32(rules.damage);
    w.f32(rules.magic);
    w.u8(rules.hit_cap);
    w.u16(rules.time_limit);
    w.finish()
}

pub(crate) fn decode_rules(bytes: &[u8]) -> Option<MatchRules> {
    let mut r = Reader::new(bytes);
    if r.u8()? != RULES {
        return None;
    }
    let rules = MatchRules {
        mode: MatchRules::mode_from(r.u8()?)?,
        damage: r.f32()?,
        magic: r.f32()?,
        hit_cap: r.u8()?,
        time_limit: r.u16()?,
    };
    (r.is_empty() && rules.valid()).then_some(rules)
}

/// What the joiner shows of a host-side moment: sounds and sparks are
/// played and drawn locally from these, never replicated as such.
#[derive(Clone, Debug, PartialEq)]
pub(crate) enum DuelEvent {
    /// A sound by id (bank·1000 + sound).
    Sound(u32),
    /// A hit spark: effect, random count, contact point, direction, and
    /// whether it is the attacker's own (Sora's object effects).
    Spark {
        effect: u8,
        random: u8,
        contact: [f32; 3],
        direction: [f32; 3],
        own: bool,
    },
    /// A guard met a swing: the clash spark at this point.
    Guard([f32; 3]),
    /// A Sora was knocked out (slot).
    KnockOut(u8),
    /// A knocked-out Sora stood up again (slot).
    StoodUp(u8),
    /// A spell or item effect call (the spell scripts' `pax_start` and
    /// `effect_*`), on the effect container by name; a bound start names the
    /// Sora by slot (`Anchor::Rival`).
    Spell {
        pax: Option<String>,
        fx: player::magic::Fx,
    },
    /// A Drive or object effect element started on a Sora (by slot) or at a
    /// place.
    DriveFx {
        pax: String,
        id: u16,
        owner: u8,
        anchor: FxAnchor,
    },
}

/// Where a forwarded Drive effect starts.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) enum FxAnchor {
    Slot(u8),
    World { at: [f32; 3], pitch: f32, yaw: f32 },
}

fn put_vec(w: &mut Writer, v: [f32; 3]) {
    for x in v {
        w.f32(x);
    }
}

fn get_vec(r: &mut Reader) -> Option<[f32; 3]> {
    let v = [r.f32()?, r.f32()?, r.f32()?];
    v.iter().all(|x| x.is_finite()).then_some(v)
}

fn put_fx(w: &mut Writer, fx: &player::magic::Fx) {
    use player::magic::{Anchor, Fx};
    match fx {
        Fx::Start { key, id, bound, at } => {
            w.u8(0);
            w.u64(*key);
            w.u16(*id);
            w.u8(match bound {
                Some(Anchor::Rival(slot)) => *slot,
                _ => u8::MAX,
            });
            match at {
                Some(v) => {
                    w.u8(1);
                    put_vec(w, *v);
                }
                None => w.u8(0),
            }
        }
        Fx::Pos(key, v) => {
            w.u8(1);
            w.u64(*key);
            put_vec(w, *v);
        }
        Fx::Dir(key, v) => {
            w.u8(2);
            w.u64(*key);
            put_vec(w, *v);
        }
        Fx::Scale(key, s) => {
            w.u8(3);
            w.u64(*key);
            w.f32(*s);
        }
        Fx::Fade(key, f) => {
            w.u8(4);
            w.u64(*key);
            w.f32(*f);
        }
        Fx::Kill(key) => {
            w.u8(5);
            w.u64(*key);
        }
        Fx::LoopEnd(key) => {
            w.u8(6);
            w.u64(*key);
        }
    }
}

fn get_fx(r: &mut Reader) -> Option<player::magic::Fx> {
    use player::magic::{Anchor, Fx};
    let tag = r.u8()?;
    let key = r.u64()?;
    Some(match tag {
        0 => {
            let id = r.u16()?;
            let slot = r.u8()?;
            let at = match r.u8()? {
                0 => None,
                _ => Some(get_vec(r)?),
            };
            Fx::Start {
                key,
                id,
                bound: (slot != u8::MAX).then_some(Anchor::Rival(slot)),
                at,
            }
        }
        1 => Fx::Pos(key, get_vec(r)?),
        2 => Fx::Dir(key, get_vec(r)?),
        3 => Fx::Scale(key, r.f32()?),
        4 => Fx::Fade(key, r.f32()?),
        5 => Fx::Kill(key),
        6 => Fx::LoopEnd(key),
        _ => return None,
    })
}

pub(crate) fn encode_events(tick: u64, events: &[DuelEvent]) -> Vec<u8> {
    let mut w = Writer::default();
    w.u8(EVENTS);
    w.u64(tick);
    w.u8(events.len().min(255) as u8);
    for e in events.iter().take(255) {
        match e {
            DuelEvent::Sound(id) => {
                w.u8(0);
                w.u32(*id);
            }
            DuelEvent::Spark {
                effect,
                random,
                contact,
                direction,
                own,
            } => {
                w.u8(1);
                w.u8(*effect);
                w.u8(*random);
                for v in contact.iter().chain(direction) {
                    w.f32(*v);
                }
                w.u8(u8::from(*own));
            }
            DuelEvent::KnockOut(slot) => {
                w.u8(2);
                w.u8(*slot);
            }
            DuelEvent::StoodUp(slot) => {
                w.u8(3);
                w.u8(*slot);
            }
            DuelEvent::Guard(at) => {
                w.u8(4);
                for v in at {
                    w.f32(*v);
                }
            }
            DuelEvent::Spell { pax, fx } => {
                w.u8(5);
                w.text(pax.as_deref().unwrap_or(""), 255);
                put_fx(&mut w, fx);
            }
            DuelEvent::DriveFx {
                pax,
                id,
                owner,
                anchor,
            } => {
                w.u8(6);
                w.text(pax, 255);
                w.u16(*id);
                w.u8(*owner);
                match anchor {
                    FxAnchor::Slot(slot) => {
                        w.u8(0);
                        w.u8(*slot);
                    }
                    FxAnchor::World { at, pitch, yaw } => {
                        w.u8(1);
                        put_vec(&mut w, *at);
                        w.f32(*pitch);
                        w.f32(*yaw);
                    }
                }
            }
        }
    }
    w.finish()
}

pub(crate) fn decode_events(bytes: &[u8]) -> Option<(u64, Vec<DuelEvent>)> {
    let mut r = Reader::new(bytes);
    if r.u8()? != EVENTS {
        return None;
    }
    let tick = r.u64()?;
    let n = r.u8()? as usize;
    let mut out = Vec::with_capacity(n);
    for _ in 0..n {
        out.push(match r.u8()? {
            0 => DuelEvent::Sound(r.u32()?),
            1 => {
                let effect = r.u8()?;
                let random = r.u8()?;
                let contact = [r.f32()?, r.f32()?, r.f32()?];
                let direction = [r.f32()?, r.f32()?, r.f32()?];
                let own = r.u8()? != 0;
                if !contact.iter().chain(&direction).all(|v| v.is_finite()) {
                    return None;
                }
                DuelEvent::Spark {
                    effect,
                    random,
                    contact,
                    direction,
                    own,
                }
            }
            2 => DuelEvent::KnockOut(r.u8()?),
            3 => DuelEvent::StoodUp(r.u8()?),
            5 => {
                let pax = r.text(255)?;
                DuelEvent::Spell {
                    pax: (!pax.is_empty()).then_some(pax),
                    fx: get_fx(&mut r)?,
                }
            }
            6 => {
                let pax = r.text(255)?;
                let id = r.u16()?;
                let owner = r.u8()?;
                let anchor = match r.u8()? {
                    0 => FxAnchor::Slot(r.u8()?),
                    1 => {
                        let at = get_vec(&mut r)?;
                        let (pitch, yaw) = (r.f32()?, r.f32()?);
                        if !(pitch.is_finite() && yaw.is_finite()) {
                            return None;
                        }
                        FxAnchor::World { at, pitch, yaw }
                    }
                    _ => return None,
                };
                DuelEvent::DriveFx {
                    pax,
                    id,
                    owner,
                    anchor,
                }
            }
            4 => {
                let at = [r.f32()?, r.f32()?, r.f32()?];
                if !at.iter().all(|v| v.is_finite()) {
                    return None;
                }
                DuelEvent::Guard(at)
            }
            _ => return None,
        });
    }
    r.is_empty().then_some((tick, out))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pad(n: u8) -> RivalPad {
        RivalPad {
            direction: [0.25 * f32::from(n), -0.5],
            jump: n & 1 != 0,
            jump_held: n & 2 != 0,
            attack: n & 4 != 0,
            guard: n & 8 != 0,
            guard_held: n & 16 != 0,
            command: match n % 4 {
                0 => Command::None,
                1 => Command::Cast(n),
                2 => Command::Item(n),
                _ => Command::Drive(n % 7),
            },
        }
    }

    #[test]
    fn every_action_round_trips() {
        let actions = [
            Action::Idle,
            Action::Walk,
            Action::Run,
            Action::Jump,
            Action::Fall,
            Action::Land,
            Action::Attack(3),
            Action::AirAttack(70),
            Action::Cast(1),
            Action::AirCast(2),
            Action::Hurt(9),
            Action::Guard,
            Action::GuardRecoil,
            Action::WallRecoil,
            Action::Item,
            Action::Drive,
            Action::DriveIn,
            Action::HighJump,
            Action::Growth(205),
            Action::Scripted(0x2b),
        ];
        for a in actions {
            let (c, arg) = action_code(a);
            assert_eq!(action_from(c, arg), Some(a));
        }
        assert_eq!(action_from(200, 0), None);
    }

    #[test]
    fn inputs_keep_the_newest_ticks() {
        let recent: Vec<_> = (0..10u32).map(|t| (t, pad(t as u8))).collect();
        let decoded = decode_input(&encode_input(&recent)).unwrap();
        assert_eq!(decoded, recent[4..].to_vec());
        assert_eq!(decode_input(&encode_input(&[])).unwrap(), Vec::new());
        let bytes = encode_input(&recent);
        assert_eq!(decode_input(&bytes[..bytes.len() - 1]), None);
    }

    #[test]
    fn snapshots_round_trip() {
        let s = Snapshot {
            tick: 12_345,
            fighters: vec![
                FighterState {
                    slot: 0,
                    position: [1.5, -2., 300.25],
                    yaw: 3.1,
                    action: Action::Attack(4),
                    frame: 17,
                    armed: true,
                    blend: 8.,
                    started: 12_330,
                    hp: 120,
                    max_hp: 160,
                    mp: [80, 100],
                    drive: [0, 40, 3, 3],
                    invulnerable: 0.,
                    form: 1,
                    items: vec![1, 0, 5],
                },
                FighterState {
                    slot: 1,
                    position: [0.; 3],
                    yaw: 0.,
                    action: Action::Hurt(2),
                    frame: 0,
                    armed: false,
                    blend: 0.,
                    started: 1,
                    hp: 0,
                    max_hp: 160,
                    mp: [0, 100],
                    drive: [0; 4],
                    invulnerable: 80.,
                    form: 0,
                    items: Vec::new(),
                },
            ],
        };
        let bytes = encode_snapshot(&s);
        assert_eq!(decode_snapshot(&bytes), Some(s));
        assert_eq!(decode_snapshot(&bytes[..bytes.len() - 3]), None);
        let mut nan = bytes.clone();
        nan[11..15].copy_from_slice(&f32::NAN.to_le_bytes());
        assert_eq!(decode_snapshot(&nan), None);
    }

    #[test]
    fn rules_round_trip() {
        let rules = MatchRules {
            mode: super::super::duel_rules::Mode::Timed,
            damage: 3.5,
            magic: 1.5,
            hit_cap: 50,
            time_limit: 90,
        };
        assert_eq!(decode_rules(&encode_rules(&rules)), Some(rules));
        assert_eq!(decode_rules(&[RULES]), None);
        let mut nan = encode_rules(&rules);
        nan[2..6].copy_from_slice(&f32::NAN.to_le_bytes());
        assert_eq!(decode_rules(&nan), None);
    }

    #[test]
    fn events_round_trip() {
        let events = vec![
            DuelEvent::Sound(500_012),
            DuelEvent::Spark {
                effect: 3,
                random: 2,
                contact: [1., 2., 3.],
                direction: [0., 0., 1.],
                own: true,
            },
            DuelEvent::KnockOut(1),
            DuelEvent::StoodUp(0),
            DuelEvent::Guard([4., 5., 6.]),
            DuelEvent::Spell {
                pax: Some("magic/FIRE_3.mag".into()),
                fx: player::magic::Fx::Start {
                    key: 1 << 60 | 5,
                    id: 12,
                    bound: Some(player::magic::Anchor::Rival(1)),
                    at: Some([1., 2., 3.]),
                },
            },
            DuelEvent::Spell {
                pax: None,
                fx: player::magic::Fx::Dir(9, [0., 1., 0.]),
            },
            DuelEvent::Spell {
                pax: None,
                fx: player::magic::Fx::Fade(9, 15.),
            },
            DuelEvent::DriveFx {
                pax: "form:1".into(),
                id: 3,
                owner: 1,
                anchor: FxAnchor::Slot(0),
            },
            DuelEvent::DriveFx {
                pax: "system".into(),
                id: 11,
                owner: 0xff,
                anchor: FxAnchor::World {
                    at: [1., 0., 2.],
                    pitch: 0.5,
                    yaw: -1.,
                },
            },
        ];
        assert_eq!(
            decode_events(&encode_events(99, &events)),
            Some((99, events))
        );
        assert_eq!(decode_events(&[EVENTS]), None);
        assert_eq!(decode_events(&encode_input(&[])), None);
    }
}
