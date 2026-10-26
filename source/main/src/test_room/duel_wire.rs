//! The duel's game messages over the LAN session: the joiner's buttons
//! (unreliable, with the last few ticks repeated so a lost datagram costs
//! nothing), the host's snapshots of both Soras (unreliable, newest wins),
//! and events that must arrive (reliable: hits to show, knock-outs).
//!
//! Only what drawing and the HUD need is sent; the host alone simulates.

use super::rival::RivalPad;
use net::wire::{Reader, Writer};
use player::Action;

const INPUT: u8 = 1;
const SNAPSHOT: u8 = 2;
const EVENTS: u8 = 3;

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
}

fn get_pad(r: &mut Reader) -> Option<RivalPad> {
    let direction = [r.f32()?, r.f32()?];
    if !direction.iter().all(|v| v.is_finite()) {
        return None;
    }
    let bits = r.u8()?;
    Some(RivalPad {
        direction,
        jump: bits & 1 != 0,
        jump_held: bits & 2 != 0,
        attack: bits & 4 != 0,
        guard: bits & 8 != 0,
        guard_held: bits & 16 != 0,
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
#[derive(Clone, Copy, Debug, PartialEq)]
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
}

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
        });
    }
    r.is_empty().then_some(Snapshot { tick, fighters })
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
        ];
        assert_eq!(
            decode_events(&encode_events(99, &events)),
            Some((99, events))
        );
        assert_eq!(decode_events(&[EVENTS]), None);
        assert_eq!(decode_events(&encode_input(&[])), None);
    }
}
