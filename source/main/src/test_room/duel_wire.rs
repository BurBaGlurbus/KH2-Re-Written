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
pub const LOBBY: u8 = 5;
pub const INTENT: u8 = 6;
pub const REFUSED: u8 = 7;
/// A vote (host to joiner) and a joiner's ballot (duel_vote.rs).
pub const VOTE: u8 = 8;
pub const BALLOT: u8 = 9;
/// The match (host to joiner) and a joiner's rematch choice
/// (duel_match_room.rs).
pub const MATCH: u8 = 10;
pub const REMATCH: u8 = 11;
/// The host's pick of the match's music (duel_music.rs).
pub const MUSIC: u8 = 12;
/// A joiner's forfeit (duel_match_room.rs).
pub const FORFEIT: u8 = 13;

/// Ticks of buttons each input message repeats.
pub const INPUT_REDUNDANCY: usize = 6;

pub fn action_code(action: Action) -> (u8, u16) {
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

pub fn action_from(code: u8, arg: u16) -> Option<Action> {
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

/// The stick's axes (-1..=1) in 16 bits each [T249].
fn axis_bits(v: f32) -> u16 {
    (v.clamp(-1., 1.) * 32767.).round() as i16 as u16
}

fn axis_of(bits: u16) -> f32 {
    f32::from(bits as i16) / 32767.
}

fn put_pad(w: &mut Writer, pad: &RivalPad) {
    w.u16(axis_bits(pad.direction[0]));
    w.u16(axis_bits(pad.direction[1]));
    let bits = u8::from(pad.jump)
        | u8::from(pad.jump_held) << 1
        | u8::from(pad.attack) << 2
        | u8::from(pad.guard) << 3
        | u8::from(pad.guard_held) << 4
        | u8::from(pad.shotlock_held) << 5
        | u8::from(pad.flow) << 6;
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
    let direction = [axis_of(r.u16()?), axis_of(r.u16()?)];
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
        shotlock_held: bits & 32 != 0,
        flow: bits & 64 != 0,
        command,
    })
}

/// The joiner's buttons of its newest ticks, oldest first: (joiner tick,
/// pad). The first tick is whole, each next one its step from the last (the
/// newest run whose steps fit a byte).
pub fn encode_input(recent: &[(u32, RivalPad)]) -> Vec<u8> {
    let mut w = Writer::default();
    w.u8(INPUT);
    let mut recent = &recent[recent.len().saturating_sub(INPUT_REDUNDANCY)..];
    while let Some(i) = recent
        .windows(2)
        .rposition(|p| p[1].0.wrapping_sub(p[0].0) > u32::from(u8::MAX))
    {
        recent = &recent[i + 1..];
    }
    w.u8(recent.len() as u8);
    let mut last = None;
    for (tick, pad) in recent {
        match last {
            None => w.u32(*tick),
            Some(t) => w.u8(tick.wrapping_sub(t) as u8),
        }
        last = Some(*tick);
        put_pad(&mut w, pad);
    }
    w.finish()
}

pub fn decode_input(bytes: &[u8]) -> Option<Vec<(u32, RivalPad)>> {
    let mut r = Reader::new(bytes);
    if r.u8()? != INPUT {
        return None;
    }
    let n = r.u8()? as usize;
    if n > INPUT_REDUNDANCY {
        return None;
    }
    let mut out: Vec<(u32, RivalPad)> = Vec::with_capacity(n);
    for _ in 0..n {
        let tick = match out.last() {
            None => r.u32()?,
            Some(&(t, _)) => t.wrapping_add(u32::from(r.u8()?)),
        };
        out.push((tick, get_pad(&mut r)?));
    }
    r.is_empty().then_some(out)
}

/// One Sora as the other instance draws him.
#[derive(Clone, Debug, PartialEq)]
pub struct FighterState {
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
    /// His Wisdom bullets (and KH3 shotlock bolts): key, position,
    /// direction.
    pub bullets: Vec<(u64, [f32; 3], [f32; 3])>,
    /// [KH3 ruleset] What his HUD and model need of the KH3 layer.
    pub kh3: Option<player::kh3::Shown>,
}

/// The most bullets a snapshot carries per fighter.
pub const MAX_BULLETS: usize = 32;

/// The most item slots a snapshot carries.
pub const MAX_ITEM_SLOTS: usize = 8;

/// Snapshot numbers in fewer bytes [duel choice, T249]: positions stay
/// f32 (they move the Soras); the yaw is a turn in 65536 steps, the blend
/// and invulnerability in 1/256 and 1/16 frame, HP and MP as 16 bits, the
/// frame as 16 bits, the action's start as ticks before the snapshot.
fn yaw_bits(yaw: f32) -> u16 {
    let turn = yaw.rem_euclid(std::f32::consts::TAU) / std::f32::consts::TAU;
    ((turn * 65536.).round() as u32 & 0xffff) as u16
}

fn yaw_of(bits: u16) -> f32 {
    let yaw = f32::from(bits) / 65536. * std::f32::consts::TAU;
    // (-pi, pi], as the field keeps it.
    if yaw > std::f32::consts::PI {
        yaw - std::f32::consts::TAU
    } else {
        yaw
    }
}

fn fixed_bits(v: f32, scale: f32) -> u16 {
    (v * scale).round().clamp(0., 65535.) as u16
}

/// An angle in (-pi, pi] in 16 bits [T279].
fn angle_bits(a: f32) -> u16 {
    ((a / std::f32::consts::PI).clamp(-1., 1.) * 32767.).round() as i16 as u16
}

fn angle_of(bits: u16) -> f32 {
    f32::from(bits as i16) / 32767. * std::f32::consts::PI
}

fn small(v: i32) -> u16 {
    v.clamp(i32::from(i16::MIN), i32::from(i16::MAX)) as i16 as u16
}

fn small_of(bits: u16) -> i32 {
    i32::from(bits as i16)
}

/// The host's picture of one tick.
#[derive(Clone, Debug, PartialEq)]
pub struct Snapshot {
    pub tick: u64,
    pub fighters: Vec<FighterState>,
}

pub fn encode_snapshot(s: &Snapshot) -> Vec<u8> {
    let mut w = Writer::default();
    w.u8(SNAPSHOT);
    w.u64(s.tick);
    w.u8(s.fighters.len() as u8);
    for f in &s.fighters {
        w.u8(f.slot);
        for v in f.position {
            w.f32(v);
        }
        w.u16(yaw_bits(f.yaw));
        let (code, arg) = action_code(f.action);
        w.u8(code);
        w.u16(arg);
        w.u16(f.frame.min(0xffff) as u16);
        w.u8(u8::from(f.armed));
        w.u16(fixed_bits(f.blend, 256.));
        w.u32(s.tick.saturating_sub(f.started).min(u64::from(u32::MAX)) as u32);
        w.u16(small(f.hp));
        w.u16(small(f.max_hp));
        w.u16(small(f.mp[0]));
        w.u16(small(f.mp[1]));
        w.bytes(&f.drive);
        w.u16(fixed_bits(f.invulnerable, 16.));
        w.u8(f.form);
        let items = &f.items[..f.items.len().min(MAX_ITEM_SLOTS)];
        w.u8(items.len() as u8);
        for &id in items {
            w.u16(id);
        }
        let bullets = &f.bullets[..f.bullets.len().min(MAX_BULLETS)];
        w.u8(bullets.len() as u8);
        for (key, pos, dir) in bullets {
            w.u64(*key);
            for v in pos.iter().chain(dir) {
                w.f32(*v);
            }
        }
        match &f.kh3 {
            None => w.u8(0),
            Some(k) => {
                w.u8(1);
                w.bytes(&[
                    k.focus,
                    k.style,
                    k.mode,
                    k.locks,
                    u8::from(k.aiming)
                        | u8::from(k.syphon) << 1
                        | u8::from(k.counter) << 2
                        | u8::from(k.chase) << 3
                        | u8::from(k.risk) << 4,
                    k.offer,
                ]);
                w.u16(angle_bits(k.lean[0]));
                w.u16(angle_bits(k.lean[1]));
                w.u16(k.row);
                w.u8(k.serial);
                w.u16(k.time);
                w.u16(k.axis as u16);
                w.u8(k.blend as u8);
                w.u8(u8::from(k.looped));
                w.u8(k.actors.len().min(64) as u8);
                for a in k.actors.iter().take(64) {
                    w.u16(a.serial);
                    w.u16(a.cmd);
                    w.u8(a.depth);
                    for v in a.pos.iter().chain(&a.dir) {
                        w.f32(*v);
                    }
                }
            }
        }
    }
    w.finish()
}

pub fn decode_snapshot(bytes: &[u8]) -> Option<Snapshot> {
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
        let yaw = yaw_of(r.u16()?);
        let action = action_from(r.u8()?, r.u16()?)?;
        let frame = u32::from(r.u16()?);
        let armed = r.u8()? != 0;
        let blend = f32::from(r.u16()?) / 256.;
        let started = tick.saturating_sub(u64::from(r.u32()?));
        let hp = small_of(r.u16()?);
        let max_hp = small_of(r.u16()?);
        let mp = [small_of(r.u16()?), small_of(r.u16()?)];
        let drive = r.array()?;
        let invulnerable = f32::from(r.u16()?) / 16.;
        let form = r.u8()?;
        let count = r.u8()? as usize;
        if count > MAX_ITEM_SLOTS {
            return None;
        }
        let items = (0..count).map(|_| r.u16()).collect::<Option<Vec<_>>>()?;
        let count = r.u8()? as usize;
        if count > MAX_BULLETS {
            return None;
        }
        let mut bullets = Vec::with_capacity(count);
        for _ in 0..count {
            let key = r.u64()?;
            let v = [r.f32()?, r.f32()?, r.f32()?, r.f32()?, r.f32()?, r.f32()?];
            if !v.iter().all(|x| x.is_finite()) {
                return None;
            }
            bullets.push((key, [v[0], v[1], v[2]], [v[3], v[4], v[5]]));
        }
        let kh3 = match r.u8()? {
            0 => None,
            1 => {
                let b: [u8; 6] = r.array()?;
                Some(player::kh3::Shown {
                    focus: b[0],
                    style: b[1],
                    mode: b[2],
                    locks: b[3],
                    aiming: b[4] & 1 != 0,
                    syphon: b[4] & 2 != 0,
                    counter: b[4] & 4 != 0,
                    chase: b[4] & 8 != 0,
                    risk: b[4] & 16 != 0,
                    offer: b[5],
                    lean: [angle_of(r.u16()?), angle_of(r.u16()?)],
                    row: r.u16()?,
                    serial: r.u8()?,
                    time: r.u16()?,
                    axis: r.u16()? as i16,
                    blend: r.u8()? as i8,
                    looped: r.u8()? != 0,
                    actors: {
                        let n = r.u8()? as usize;
                        if n > 64 {
                            return None;
                        }
                        let mut list = Vec::with_capacity(n);
                        for _ in 0..n {
                            let (serial, cmd, depth) = (r.u16()?, r.u16()?, r.u8()?);
                            let mut v = [0f32; 6];
                            for x in &mut v {
                                *x = r.f32()?;
                            }
                            list.push(player::kh3::ShownActor {
                                serial,
                                cmd,
                                depth,
                                pos: [v[0], v[1], v[2]],
                                dir: [v[3], v[4], v[5]],
                            });
                        }
                        list
                    },
                })
            }
            _ => return None,
        };
        if !position.iter().all(|v| v.is_finite()) {
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
            bullets,
            kh3,
        });
    }
    r.is_empty().then_some(Snapshot { tick, fighters })
}

/// The host's match rules (reliable, on connecting and on every change).
pub fn encode_rules(rules: &MatchRules) -> Vec<u8> {
    let mut w = Writer::default();
    w.u8(RULES);
    w.u8(rules.mode_code());
    w.f32(rules.damage);
    w.f32(rules.magic);
    w.u8(rules.hit_cap);
    w.u16(rules.time_limit);
    w.u8(rules.arena);
    w.u8(rules.ruleset_code());
    w.finish()
}

pub fn decode_rules(bytes: &[u8]) -> Option<MatchRules> {
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
        arena: r.u8()?,
        ruleset: MatchRules::ruleset_from(r.u8()?)?,
    };
    (r.is_empty() && rules.valid()).then_some(rules)
}

/// What the joiner shows of a host-side moment: sounds and sparks are
/// played and drawn locally from these, never replicated as such.
#[derive(Clone, Debug, PartialEq)]
pub enum DuelEvent {
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
    /// A guard met a swing: the system flash (11, or the clash's 13) at
    /// this point.
    Guard([f32; 3], u16),
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
    /// [KH3 ruleset] A composite look (`player::kh3::look`) by its id, on
    /// the Sora of a seat or at a place (and yaw), at a size in tenths:
    /// the joiner starts its layers from its own copy of the table.
    Look {
        look: u8,
        slot: u8,
        size: u8,
        at: Option<([f32; 3], f32)>,
    },
    /// [KH3 mode] An effect and a sound of KH3's by the book's names (the
    /// index; 0xffff none), at a place.
    Kh3Fx {
        effect: u16,
        sound: u16,
        at: [f32; 3],
        yaw: f32,
        attach: Option<Kh3Attach>,
    },
}

/// [KH3 mode] An effect on a Sora, `bone` by the book's names.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Kh3Attach {
    pub seat: u8,
    pub bone: u16,
    pub offset: [f32; 3],
    pub handle: u32,
    pub group: u32,
    /// 0 starts; 1 stops; 2 stops at once.
    pub stop: u8,
}

/// Where a forwarded Drive effect starts.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum FxAnchor {
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

pub fn encode_events(tick: u64, events: &[DuelEvent]) -> Vec<u8> {
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
            DuelEvent::Guard(at, flash) => {
                w.u8(4);
                for v in at {
                    w.f32(*v);
                }
                w.u16(*flash);
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
            DuelEvent::Look {
                look,
                slot,
                size,
                at,
            } => {
                w.u8(7);
                w.u8(*look);
                w.u8(*slot);
                w.u8(*size);
                match at {
                    None => w.u8(0),
                    Some((at, yaw)) => {
                        w.u8(1);
                        put_vec(&mut w, *at);
                        w.f32(*yaw);
                    }
                }
            }
            DuelEvent::Kh3Fx {
                effect,
                sound,
                at,
                yaw,
                attach,
            } => {
                w.u8(8);
                w.u16(*effect);
                w.u16(*sound);
                put_vec(&mut w, *at);
                w.f32(*yaw);
                match attach {
                    Some(a) => {
                        w.u8(1);
                        w.u8(a.seat);
                        w.u16(a.bone);
                        put_vec(&mut w, a.offset);
                        w.u32(a.handle);
                        w.u32(a.group);
                        w.u8(a.stop);
                    }
                    None => w.u8(0),
                }
            }
        }
    }
    w.finish()
}

pub fn decode_events(bytes: &[u8]) -> Option<(u64, Vec<DuelEvent>)> {
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
                DuelEvent::Guard(at, r.u16()?)
            }
            7 => {
                let (look, slot, size) = (r.u8()?, r.u8()?, r.u8()?);
                let at = match r.u8()? {
                    0 => None,
                    1 => {
                        let at = get_vec(&mut r)?;
                        let yaw = r.f32()?;
                        if !yaw.is_finite() {
                            return None;
                        }
                        Some((at, yaw))
                    }
                    _ => return None,
                };
                DuelEvent::Look {
                    look,
                    slot,
                    size,
                    at,
                }
            }
            8 => {
                let (effect, sound) = (r.u16()?, r.u16()?);
                let at = [r.f32()?, r.f32()?, r.f32()?];
                let yaw = r.f32()?;
                let attach = match r.u8()? {
                    0 => None,
                    _ => Some(Kh3Attach {
                        seat: r.u8()?,
                        bone: r.u16()?,
                        offset: [r.f32()?, r.f32()?, r.f32()?],
                        handle: r.u32()?,
                        group: r.u32()?,
                        stop: r.u8()?,
                    }),
                };
                DuelEvent::Kh3Fx {
                    effect,
                    sound,
                    at,
                    yaw,
                    attach,
                }
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
            // On the wire's steps, so they come back the same.
            direction: [0.1 * f32::from(n) - 0.5, -0.5].map(|v| axis_of(axis_bits(v))),
            jump: n & 1 != 0,
            jump_held: n & 2 != 0,
            attack: n & 4 != 0,
            guard: n & 8 != 0,
            guard_held: n & 16 != 0,
            shotlock_held: n & 32 != 0,
            flow: n & 64 != 0,
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
        // A step too long for a byte: the newest run alone.
        let gap = [(1, pad(1)), (2, pad(2)), (900, pad(3)), (901, pad(4))];
        assert_eq!(
            decode_input(&encode_input(&gap)).unwrap(),
            gap[2..].to_vec()
        );
        assert_eq!(axis_of(axis_bits(2.)), 1.);
    }

    #[test]
    fn snapshot_numbers_keep_their_precision() {
        for yaw in [-3.1, -1., 0., 0.25, 3.] {
            let back = yaw_of(yaw_bits(yaw));
            assert!((back - yaw).abs() < 1e-4, "{yaw} -> {back}");
        }
        assert!((yaw_of(yaw_bits(7.)) - (7. - std::f32::consts::TAU)).abs() < 1e-4);
        assert_eq!(f32::from(fixed_bits(2.5, 256.)) / 256., 2.5);
        assert_eq!(fixed_bits(-1., 16.), 0);
        assert_eq!(small_of(small(-35)), -35);
        assert_eq!(small_of(small(1 << 20)), i32::from(i16::MAX));
    }

    #[test]
    fn snapshots_round_trip() {
        let s = Snapshot {
            tick: 12_345,
            fighters: vec![
                FighterState {
                    slot: 0,
                    position: [1.5, -2., 300.25],
                    // Values the snapshot's steps hold exactly.
                    yaw: yaw_of(0xc000),
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
                    bullets: vec![(1 << 60 | 3, [1., 2., 3.], [0., 0., 1.])],
                    kh3: Some(player::kh3::Shown {
                        focus: 63,
                        style: 40,
                        mode: 12,
                        locks: 9,
                        aiming: true,
                        syphon: true,
                        counter: false,
                        chase: true,
                        risk: false,
                        offer: 1,
                        lean: [angle_of(angle_bits(-1.5)), 0.],
                        row: 7,
                        serial: 3,
                        time: 480,
                        axis: -12,
                        blend: -1,
                        looped: true,
                        actors: vec![player::kh3::ShownActor {
                            serial: 9,
                            cmd: 29,
                            depth: 1,
                            pos: [1., 2., 3.],
                            dir: [0., 0., 1.],
                        }],
                    }),
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
                    bullets: Vec::new(),
                    kh3: None,
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
            arena: 1,
            ruleset: super::super::duel_rules::Ruleset::Kh3,
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
            DuelEvent::Guard([4., 5., 6.], 13),
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
            DuelEvent::Look {
                look: 3,
                slot: 1,
                size: 10,
                at: None,
            },
            DuelEvent::Look {
                look: 30,
                slot: 0,
                size: 13,
                at: Some(([4., 90., -2.], 0.25)),
            },
        ];
        assert_eq!(
            decode_events(&encode_events(99, &events)),
            Some((99, events))
        );
        // A look on a Sora is five bytes: it replaces an element list.
        let one = |e: DuelEvent| encode_events(0, &[e]).len();
        assert_eq!(
            one(DuelEvent::Look {
                look: 0,
                slot: 0,
                size: 10,
                at: None
            }) - one(DuelEvent::KnockOut(0)),
            3
        );
        assert_eq!(decode_events(&[EVENTS]), None);
        assert_eq!(decode_events(&encode_input(&[])), None);
    }
}
