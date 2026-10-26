//! Sora's magic, translated from the PC executable: the command colour
//! FUN_140401d60 / FUN_1403c5940, the cast start FUN_1403c5170, the MP
//! rules FUN_1403d3cf0 / FUN_1403c0890 / FUN_1403c0de0, and the six spell
//! programs (the BDX scripts of `magic/*.mag`, trigger 21) written out as
//! state machines. Spells are driven by `magi` rows (03system `pref/magi`)
//! and ATKP ids; hits go through the normal attack pipeline.
//!
//! Positions are the simulation's Y-up space; the executable's Y-down
//! offsets are negated here.

use crate::Vec3;

/// Magic types (magc +0, cmd argument).
pub const FIRE: u8 = 0;
pub const BLIZZARD: u8 = 1;
pub const THUNDER: u8 = 2;
pub const CURE: u8 = 3;
pub const MAGNET: u8 = 4;
pub const REFLECT: u8 = 5;

/// `0x41f00000`-style constants of the MP routines.
/// SSTM +0x7c: MP gained during a charge shortens it by gain × 12.
pub const CHARGE_GAIN_FACTOR: f32 = 12.;
/// 03system `prty` +0x1c: the length of an MP Charge (3000 for Sora).
pub const CHARGE_LENGTH: f32 = 3000.;
/// SSTM +0x48: the air-cast hop height.
pub const AIR_HOP: f32 = 40.;

/// One `magc` record (0x38 bytes).
#[derive(Clone, Debug, PartialEq)]
pub struct Record {
    pub magic_type: u8,
    pub level: u8,
    /// 0 for any world.
    pub world: u8,
    pub file: String,
    /// The element item whose owned count is the level (0 = level 0).
    pub item: u16,
    pub command: u16,
    pub ground: (i16, i16),
    pub finisher: (i16, i16),
    pub air: (i16, i16),
    pub voice: i8,
    pub voice_finisher: i8,
    pub voice_self: i8,
}

impl Record {
    pub const SIZE: usize = 0x38;

    pub fn read(r: &[u8]) -> Option<Self> {
        let r = r.get(..Self::SIZE)?;
        let h = |at: usize| u16::from_le_bytes([r[at], r[at + 1]]);
        let end = r[4..0x24].iter().position(|&b| b == 0).unwrap_or(32);
        Some(Record {
            magic_type: r[0],
            level: r[1],
            world: r[2],
            file: String::from_utf8_lossy(&r[4..4 + end]).into_owned(),
            item: h(0x24),
            command: h(0x26),
            ground: (h(0x28) as i16, h(0x2a) as i16),
            finisher: (h(0x2c) as i16, h(0x2e) as i16),
            air: (h(0x30) as i16, h(0x32) as i16),
            voice: r[0x34] as i8,
            voice_finisher: r[0x35] as i8,
            voice_self: r[0x36] as i8,
        })
    }

    /// FUN_1403c5dc0: the record in force for `count` owned element items
    /// (the last of the type with level ≤ count, in a world-0 or matching
    /// world), or None when the slot is empty.
    pub fn current(records: &[Record], magic_type: u8, count: u8, world: u8) -> Option<&Record> {
        let mut of_type = records.iter().filter(|r| r.magic_type == magic_type);
        let first = of_type.next()?;
        if first.item != 0 && count == 0 {
            return None;
        }
        let mut best = first;
        for r in records.iter().filter(|r| r.magic_type == magic_type) {
            if r.level <= count && (r.world == 0 || r.world == world) {
                best = r;
            }
        }
        Some(best)
    }
}

/// A `magi` row: 31 dwords (f25 and f28 are integers).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Row(pub [u32; 31]);

impl Row {
    pub fn read(bytes: &[u8]) -> Option<Self> {
        let b = bytes.get(..124)?;
        let mut w = [0u32; 31];
        for (i, v) in w.iter_mut().enumerate() {
            *v = u32::from_le_bytes(b[i * 4..i * 4 + 4].try_into().unwrap());
        }
        Some(Row(w))
    }
    pub fn f(&self, i: usize) -> f32 {
        f32::from_bits(self.0[i])
    }
    pub fn i(&self, i: usize) -> i32 {
        self.0[i] as i32
    }
}

/// The row index of a cast (FUN_1403c5170): 2·level − 2, plus 1 for a
/// ground finisher, plus 10 / 20 / 30 for the Wisdom / Master / Final
/// forms of the player character.
pub fn variant_index(level: u8, finisher: bool, form: u8) -> usize {
    let mut idx = 2 * level.max(1) as usize - 2 + finisher as usize;
    match form {
        2 => idx += 10,
        4 => idx += 20,
        5 => idx += 30,
        _ => {}
    }
    idx
}

/// MP and MP Charge (stats +0x180 / +0x184 / +0x1bc / +0x1c0).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Mp {
    pub cur: i32,
    pub max: i32,
    /// Charge remaining and total (total > 0 while charging).
    pub remaining: f32,
    pub total: f32,
}

impl Mp {
    pub fn new(max: i32) -> Self {
        Mp {
            cur: max,
            max,
            remaining: 0.,
            total: 0.,
        }
    }

    pub fn charging(&self) -> bool {
        self.total > 0.
    }

    /// FUN_1403d3cf0: a change that would take MP below 0 starts a charge
    /// instead (MP 0, total = remaining = `length`).
    pub fn change(&mut self, delta: i32, length: f32) -> i32 {
        if self.cur + delta < 0 {
            self.total = length;
            self.remaining = length;
            self.cur = 0;
            return 0;
        }
        self.add(delta)
    }

    /// FUN_1403c20a0 (`obj_start_mpdrive`): MP drops to 0 and a charge of
    /// `length` starts at once.
    pub fn start_charge(&mut self, length: f32) {
        self.total = length;
        self.remaining = length;
        self.cur = 0;
    }

    /// FUN_1403c0890: gains shorten a running charge by gain × 12 and
    /// refill MP when it ends; otherwise MP clamps to [0, max].
    pub fn add(&mut self, d: i32) -> i32 {
        if self.charging() {
            if d > 0 {
                self.remaining =
                    (self.remaining - d as f32 * CHARGE_GAIN_FACTOR).clamp(0., self.total);
                if self.remaining == 0. {
                    self.remaining = 0.;
                    self.total = 0.;
                    self.cur = self.max;
                }
            }
            return self.cur;
        }
        self.cur = (self.cur + d).clamp(0, self.max);
        self.cur
    }

    /// FUN_1403c0de0, once per tick: the charge runs down at `dt × rate`
    /// (1 + MP Haste terms) and ends with a full MP bar.
    pub fn tick(&mut self, dt: f32, rate: f32) {
        if self.charging() {
            self.remaining = (self.remaining - dt * rate).clamp(0., self.total);
            if self.remaining == 0. {
                self.total = 0.;
                self.cur = self.max;
            }
        }
    }
}

/// FUN_140401d60 case Execute 2: the menu colour of a spell (≥ 4 cannot be
/// selected, 2 = orange "will start MP Charge"). `cost` is cmd +0x1c (0xff
/// for Cure); `dont_charge` is the camp's per-type toggle.
pub fn colour(cost: u8, mp: &Mp, dont_charge: bool, forbidden_area: bool) -> usize {
    let mut c = 0;
    if mp.charging() {
        c = 5;
    } else if mp.cur < cost as i32 {
        // FUN_1403c5940: Cure always, others unless "Don't MP Charge".
        c = if cost == 0xff || !dont_charge { 2 } else { 5 };
    }
    if forbidden_area {
        c = 5;
    }
    c
}

/// What FUN_1403c5170 decides when a spell command is accepted.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Start {
    pub motion: i16,
    pub blend: f32,
    pub voice: i8,
    pub row: usize,
    pub airborne: bool,
    pub finisher: bool,
}

/// FUN_1403c5170 (without the script object): the cast motion, blend,
/// voice and `magi` row. `finisher` must already be false for the Lion
/// form and for spells without a finisher motion it falls back to ground.
pub fn start(
    rec: &Record,
    finisher: bool,
    airborne: bool,
    form: u8,
    target_is_self: bool,
) -> Start {
    let mut voice = rec.voice;
    let mut idx = 2 * rec.level.max(1) as usize - 2;
    let (motion, blend, fin, air);
    if !airborne {
        if finisher && rec.finisher.0 >= 0 {
            motion = rec.finisher.0;
            blend = rec.finisher.1;
            voice = rec.voice_finisher;
            idx += 1;
            fin = true;
        } else {
            motion = rec.ground.0;
            blend = rec.ground.1;
            fin = false;
        }
        air = false;
    } else {
        motion = rec.air.0;
        blend = rec.air.1;
        fin = false;
        air = true;
    }
    if target_is_self && rec.voice_self >= 0 {
        voice = rec.voice_self;
    }
    match form {
        2 => idx += 10,
        4 => idx += 20,
        5 => idx += 30,
        _ => {}
    }
    Start {
        motion,
        blend: blend as f32,
        voice,
        row: idx,
        airborne: air,
        finisher: fin,
    }
}

/// An ATTACK_MAGIC hit volume this tick.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Volume {
    /// Stable per attack object (spell serial and slot).
    pub key: u64,
    pub atkp: u16,
    /// The ATKP sub-table (the variant index).
    pub sub: u8,
    pub centre: Vec3,
    pub radius: f32,
    pub height: f32,
    /// Last tick's centre, for the sweep.
    pub previous: Option<Vec3>,
    /// Movement direction (unit, XZ-and-Y) for the hit direction; None for
    /// a static volume.
    pub moving: Option<Vec3>,
    pub role: Role,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Role {
    /// An ordinary attack (ATKP Type 1).
    Damage,
    /// A guard (ATKP Type 3 barrier): clashes, never tests bodies.
    Guard,
}

/// A direct heal (`attack_strike`): ATKP id, sub-table and whom.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Strike {
    pub atkp: u16,
    pub sub: u8,
    /// Party slot (0 = Sora).
    pub party: usize,
}

/// The object an effect element is bound to (`pax_start_bind`'s `obj`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Anchor {
    /// The caster (Sora).
    Caster,
    /// A party member by slot (0 = Sora).
    Party(usize),
    /// The summon that is out.
    Summon,
    /// A battle enemy by gallery actor id (a script brain's own effects).
    Actor(u64),
    /// The duel's other Sora, by slot (his own spells and items).
    Rival(u8),
}

/// A script's effect call on the spell's PAX (the `.mag` file's effect
/// container): `pax_start` / `pax_start_bind` and the `effect_*` traps
/// 0x14041df80.. (`FUN_1402c6b30`, `FUN_14041e000`..`FUN_14041e270`).
/// Positions and directions are the simulation's Y-up space; `key` names
/// the effect handle the script keeps.
#[derive(Clone, Debug, PartialEq)]
pub enum Fx {
    /// Element `id`; `bound` is `pax_start_bind`'s object (None for
    /// `pax_start`), `at` an `effect_set_pos` made right after.
    Start {
        key: u64,
        id: u16,
        bound: Option<Anchor>,
        at: Option<Vec3>,
    },
    /// `effect_set_pos`.
    Pos(u64, Vec3),
    /// `effect_set_dir` (the facing as an Euler pair, FUN_1403b7710).
    Dir(u64, Vec3),
    /// `effect_set_scale` (uniform).
    Scale(u64, f32),
    /// `effect_fadeout(e, frames, 1)`.
    Fade(u64, f32),
    /// `effect_kill`.
    Kill(u64),
    /// `effect_loop_end`.
    LoopEnd(u64),
}

/// What the effect system knows of a script's effect handle.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct FxLive {
    /// `effect_is_alive`.
    pub alive: bool,
    /// `effect_pos`, in the simulation's space.
    pub pos: Vec3,
    /// False when no effect runs for the handle (no PAX, no element): the
    /// scripts then keep their own timers.
    pub tracked: bool,
}

/// One tick's output of a spell.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Output {
    pub volumes: Vec<Volume>,
    pub strikes: Vec<Strike>,
    /// Effect calls of this tick, in script order.
    pub fx: Vec<Fx>,
    /// Reflect's shared block after this tick (T[4] and T[8]).
    pub barrier: Option<Option<u64>>,
    pub barrier_count: Option<u32>,
    /// Ticks of invulnerability to give the caster (Leaf Bracer).
    pub invulnerable: Option<f32>,
    /// Knock-slide of the caster away from its facing (Blizzard's
    /// recoil, `obj_hitback`): strength, applied once.
    pub recoil: Option<f32>,
}

/// What a spell reads each tick.
pub struct Context<'a> {
    pub dt: f32,
    /// The caster's origin (feet) and facing unit vector in XZ.
    pub caster: Vec3,
    pub facing: [f32; 2],
    /// The caster's TARGET shape centre (blizzard start).
    pub caster_centre: Vec3,
    /// The caster's body centre (bone 0x4001), where Cure's heal effect sits.
    pub body_centre: Vec3,
    pub airborne: bool,
    /// The caster is still in the cast motion (spells wait on it).
    pub casting: bool,
    /// The locked target's TARGET centre, if any.
    pub target: Option<Vec3>,
    /// Party members' origins (Sora first).
    pub party: &'a [Vec3],
    /// Ground below a point within `max` (BG ray), if any.
    pub ground: &'a dyn Fn(Vec3, f32) -> Option<Vec3>,
    /// A volume path ended on the background.
    pub bg_hit: &'a dyn Fn(Vec3, f32) -> bool,
    /// Reflect's shared block (`teamwork_alloc(0x10002)`): the handle of the
    /// barrier in force (T[4]) and the clashes carried by replaced
    /// barriers (T[8]).
    pub barrier: Option<u64>,
    pub barrier_count: u32,
}

/// Feedback from the previous tick's collisions.
#[derive(Clone, Debug, Default)]
pub struct Feedback {
    /// Volume keys that touched a body.
    pub hit: Vec<u64>,
    /// Clash counts of guard volumes by key (obj+0xb0).
    pub reflected: Vec<(u64, u32)>,
    /// The effect handles' state after the last effect update.
    pub fx: Vec<(u64, FxLive)>,
}

impl Feedback {
    fn live(&self, key: u64) -> Option<FxLive> {
        self.fx.iter().find(|f| f.0 == key).map(|f| f.1)
    }
    /// `effect_is_alive` of a handle, None when no effect runs for it.
    fn alive(&self, key: u64) -> Option<bool> {
        self.live(key).filter(|l| l.tracked).map(|l| l.alive)
    }
    /// `effect_pos` of a handle, None when no effect runs for it.
    fn pos(&self, key: u64) -> Option<Vec3> {
        self.live(key).filter(|l| l.tracked).map(|l| l.pos)
    }
    fn was_hit(&self, key: u64) -> bool {
        self.hit.contains(&key)
    }
    fn reflects(&self, key: u64) -> u32 {
        self.reflected
            .iter()
            .find(|r| r.0 == key)
            .map_or(0, |r| r.1)
    }
}

fn len2(v: [f32; 2]) -> f32 {
    v[0].hypot(v[1])
}

/// `vector_homing` (FUN_14041d3d0 / FUN_1403b7200): turns `dir` toward
/// `want` about their cross product by the angle between them, at most
/// `rate` radians [the scale DAT_140717480 is taken as 1]. Opposite
/// vectors (no axis) stay put.
pub fn vector_homing(dir: Vec3, want: Vec3, rate: f32) -> Vec3 {
    let (d, w) = (norm(dir), norm(want));
    let dot = (d[0] * w[0] + d[1] * w[1] + d[2] * w[2]).clamp(-1., 1.);
    let angle = dot.acos().min(rate);
    let axis = [
        d[1] * w[2] - d[2] * w[1],
        d[2] * w[0] - d[0] * w[2],
        d[0] * w[1] - d[1] * w[0],
    ];
    let len = (axis[0] * axis[0] + axis[1] * axis[1] + axis[2] * axis[2]).sqrt();
    if len < 1e-6 || angle < 1e-6 {
        return d;
    }
    let k = axis.map(|c| c / len);
    let (s, c) = angle.sin_cos();
    let kd = k[0] * d[0] + k[1] * d[1] + k[2] * d[2];
    let kxd = [
        k[1] * d[2] - k[2] * d[1],
        k[2] * d[0] - k[0] * d[2],
        k[0] * d[1] - k[1] * d[0],
    ];
    norm([0, 1, 2].map(|i| d[i] * c + kxd[i] * s + k[i] * kd * (1. - c)))
}

fn norm(v: Vec3) -> Vec3 {
    let l = (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt();
    if l > 0. {
        [v[0] / l, v[1] / l, v[2] / l]
    } else {
        v
    }
}

fn add(a: Vec3, b: Vec3) -> Vec3 {
    [a[0] + b[0], a[1] + b[1], a[2] + b[2]]
}

fn scale(a: Vec3, k: f32) -> Vec3 {
    [a[0] * k, a[1] * k, a[2] * k]
}

fn rot_y(v: Vec3, a: f32) -> Vec3 {
    let (s, c) = a.sin_cos();
    [v[0] * c + v[2] * s, v[1], -v[0] * s + v[2] * c]
}

/// Fire's ring centre above the caster's origin by level (the PAX element
/// 2 / 5 offsets, Y-up): ground 92.88 / 86.33 / 72.0, air 116.20 / 115.89 /
/// 95.74.
fn fire_lift(level: u8, air: bool) -> f32 {
    let t = if air {
        [116.20, 115.89, 95.74]
    } else {
        [92.88, 86.33, 72.0]
    };
    t[(level.clamp(1, 3) - 1) as usize]
}

/// A running spell.
#[derive(Clone, Debug, PartialEq)]
pub struct Spell {
    serial: u64,
    magic_type: u8,
    level: u8,
    idx: usize,
    row: Row,
    form: u8,
    age: f32,
    airborne: bool,
    state: State,
    finished: bool,
    /// The cast effect's handle (S[28]) and this release's index within the
    /// cast (Blizzard's counter S[44]).
    cast_key: u64,
    volley: u32,
}

#[derive(Clone, Debug, PartialEq)]
enum State {
    Fire {
        left: f32,
        rings: u8,
        ring: u64,
    },
    Blizzard {
        shards: Vec<Shard>,
        released: bool,
        /// The release effects the shards start from (`a`, `b`).
        from: [Option<u64>; 2],
    },
    Thunder {
        bolts: Vec<Bolt>,
        queue: Vec<(f32, Vec3, f32)>,
        started: u64,
    },
    Cure {
        done: bool,
        leaf: f32,
        leaf_centre: Vec3,
        /// The chosen target's party slot (S[44]; 0 = Sora).
        target: usize,
    },
    Magnet {
        centre: Vec3,
        life: f32,
        placed: bool,
        /// The sphere effect (S[36]) and the ticks since it started.
        sphere: Option<(u64, f32)>,
        ended: Option<f32>,
    },
    Reflect {
        barrier: f32,
        centre_offset: Vec3,
        count: u32,
        seen: u32,
        counter: Option<Counter>,
        key: u64,
        flash: Option<u64>,
        flashes: u64,
        started: bool,
    },
}

#[derive(Clone, Debug, PartialEq)]
struct Shard {
    key: u64,
    /// The shard effect's handle (p[64]).
    fx: u64,
    pos: Vec3,
    dir: Vec3,
    life: f32,
    fade: f32,
    fading: bool,
    pierce: bool,
    rate: f32,
    radius: f32,
    speed: f32,
    previous: Option<Vec3>,
    dead: bool,
}

#[derive(Clone, Debug, PartialEq)]
struct Bolt {
    key: u64,
    pos: Vec3,
    column: Vec3,
    atkp: u16,
    wait: f32,
    live: f32,
}

#[derive(Clone, Debug, PartialEq)]
struct Counter {
    stage: u8,
    left: f32,
    tier: u8,
    centre: Vec3,
}

/// `bg_get_random_pos_air` (FUN_1403f4080 → FUN_1403f44a0), without the
/// background test: a radius drawn between the limits, an angle in one of
/// eight π/4 sectors taken in the order 0, 2, 4, 6, 1, 3, 5, 7 plus a draw
/// inside the sector, and a height draw. The draws are the script RNG's
/// multipliers (`DAT_140783ca0 · 0x10dcd`, 2^-32 scaled); the stream is the
/// spell's own, not the shared one, and the sector counter starts at 0 for
/// each spell (the exe's counter is global) [I].
#[derive(Clone, Debug, PartialEq)]
pub struct RandomAir {
    state: u32,
    sector: u32,
}

impl RandomAir {
    pub fn new(seed: u64) -> Self {
        RandomAir {
            state: (seed as u32).wrapping_mul(0x9e37_79b1) | 1,
            sector: 0,
        }
    }

    pub fn next(&mut self, centre: Vec3, r0: f32, r1: f32, y0: f32, y1: f32) -> Vec3 {
        const UNIT: f32 = 2.328_306_4e-10;
        let u1 = self.state.wrapping_mul(0x10dcd);
        self.state = self.state.wrapping_mul(0x1c58_7629);
        let radius = u1 as f32 * UNIT * (r1 - r0) + r0;
        let k = self.sector;
        let v = (k >> 2 & 1) | k.wrapping_mul(2);
        let angle = v as f32 * std::f32::consts::FRAC_PI_4
            + self.state as f32 * UNIT * std::f32::consts::FRAC_PI_4;
        self.sector = (k + 1) & 7;
        self.state = self.state.wrapping_mul(0x10dcd);
        let y = self.state as f32 * UNIT * (y1 - y0) + y0;
        [
            centre[0] + angle.sin() * radius,
            centre[1] + y,
            centre[2] + angle.cos() * radius,
        ]
    }
}

/// The slot of an effect handle among a spell's keys (the volume keys use
/// slots below 0x80).
fn fx_key(serial: u64, slot: u8) -> u64 {
    serial << 8 | 0x80 | (slot & 0x7f) as u64
}

/// The cast effect's handle (`S[28]`) of the cast with `serial`.
pub fn cast_key(serial: u64) -> u64 {
    serial << 8 | 0xf0
}

/// Trigger 20 of the spell programs: the effect started with the cast
/// motion (`pax_start_bind(pax, castPax, S, 1, 0)`). `idx` is the variant
/// index (form × 10 + level and finisher terms).
pub fn cast_effects(magic_type: u8, idx: usize, key: u64) -> Vec<Fx> {
    let form = idx / 10;
    let id = match magic_type {
        // Wisdom and Final have no cast effect.
        FIRE if form == 1 || form == 3 => None,
        FIRE | BLIZZARD | THUNDER | CURE | MAGNET => Some(0),
        _ => None,
    };
    id.map(|id| Fx::Start {
        key,
        id,
        bound: Some(Anchor::Caster),
        at: None,
    })
    .into_iter()
    .collect()
}

/// TR20's variant index: Master has no finisher row, Reflect never does.
fn script_index(magic_type: u8, idx: usize) -> usize {
    match magic_type {
        FIRE | BLIZZARD | THUNDER if idx / 10 == 2 => idx / 2 * 2,
        REFLECT => idx / 2 * 2,
        _ => idx,
    }
}

impl Spell {
    /// Releases a spell: trigger 21 of the program with the `magi` row.
    /// `ctx` is the state at the release tick.
    pub fn new(
        serial: u64,
        magic_type: u8,
        level: u8,
        idx: usize,
        row: Row,
        form: u8,
        ctx: &Context,
    ) -> Spell {
        let idx = script_index(magic_type, idx);
        let (airborne, wisdom, fin) = (ctx.airborne, idx / 10 == 1, idx / 10 == 3);
        let state = match magic_type {
            FIRE => {
                // Duration f2; the HD patch turns 44 into 42 at 60 fps.
                let mut left = row.f(2);
                if left == 44. && ctx.dt < 2. {
                    left = 42.;
                }
                State::Fire {
                    left,
                    rings: if fin { 2 } else { 1 },
                    ring: fx_key(serial, 2),
                }
            }
            BLIZZARD => State::Blizzard {
                shards: Vec::new(),
                released: false,
                from: [None; 2],
            },
            THUNDER => State::Thunder {
                bolts: Vec::new(),
                queue: Vec::new(),
                started: 0,
            },
            CURE => State::Cure {
                done: false,
                leaf: 0.,
                leaf_centre: ctx.caster_centre,
                target: 0,
            },
            MAGNET => State::Magnet {
                centre: ctx.caster,
                life: 16. + row.f(16) + row.f(17),
                placed: false,
                sphere: None,
                ended: None,
            },
            _ => {
                // Barrier centre offsets (Y-up), rotated by the facing at
                // release: ground (−22, 85, −18.2), air (0, 108.9, 0),
                // Wisdom/Final (0, 90, −12.4).
                let off = if airborne {
                    [0., 108.9, 0.]
                } else if wisdom || fin {
                    [0., 90., -12.4]
                } else {
                    [-22., 85., -18.2]
                };
                State::Reflect {
                    barrier: BARRIER_LIFE,
                    centre_offset: off,
                    count: 0,
                    seen: 0,
                    counter: None,
                    key: fx_key(serial, 1),
                    flash: None,
                    flashes: 0,
                    started: false,
                }
            }
        };
        Spell {
            serial,
            magic_type,
            level,
            idx,
            row,
            form,
            age: 0.,
            airborne,
            state,
            finished: false,
            cast_key: cast_key(serial),
            volley: 0,
        }
    }

    /// The cast this release belongs to: its effect handle and the index
    /// of the release among the cast's releases (Blizzard's S[44]).
    pub fn with_cast(mut self, cast_key: u64, volley: u32) -> Self {
        self.cast_key = cast_key;
        self.volley = volley;
        self
    }

    /// Cure's chosen target (the party slot picked in the target menu;
    /// Sora, slot 0, from a shortcut).
    pub fn with_target(mut self, slot: usize) -> Self {
        if let State::Cure { target, .. } = &mut self.state {
            *target = slot;
        }
        self
    }

    /// Leaf Bracer (TR20 of Cure on oneself, ability 114): 60 ticks of
    /// invulnerability and an ATKP 1247 super guard (r 100) at the caster's
    /// TARGET centre, from the cast's start.
    pub fn leaf_bracer(serial: u64, ctx: &Context) -> Spell {
        Spell {
            serial,
            magic_type: CURE,
            level: 1,
            idx: 0,
            row: Row::default(),
            form: 0,
            age: 0.,
            airborne: false,
            state: State::Cure {
                done: true,
                leaf: 60.,
                leaf_centre: ctx.caster_centre,
                target: 0,
            },
            finished: false,
            cast_key: cast_key(serial),
            volley: 0,
        }
    }

    pub fn finished(&self) -> bool {
        self.finished
    }

    pub fn magic_type(&self) -> u8 {
        self.magic_type
    }

    fn key(&self, slot: u8) -> u64 {
        self.serial << 8 | slot as u64
    }

    /// One tick of the spell (the script's `halt` interval).
    pub fn tick(&mut self, ctx: &Context, fb: &Feedback) -> Output {
        let mut out = Output::default();
        let dt = ctx.dt;
        let key0 = self.key(0);
        let idx = self.idx;
        let row = self.row;
        let sub = idx as u8;
        let serial = self.serial;
        let cast_fx = self.cast_key;
        let volley = self.volley;
        let airborne_now = self.airborne;
        let level = self.level;
        let facing = [ctx.facing[0], 0., ctx.facing[1]];
        let first = self.age == 0.;
        let fk = |slot: u8| fx_key(serial, slot);
        let (wis, master, fin, odd) = (idx / 10 == 1, idx / 10 == 2, idx / 10 == 3, idx & 1 == 1);
        // `rel(i)`: end the cast effect's loop, start element `i` bound.
        let release_effect = |out: &mut Output, key: u64, id: u16| {
            out.fx.push(Fx::LoopEnd(cast_fx));
            out.fx.push(Fx::Start {
                key,
                id,
                bound: Some(Anchor::Caster),
                at: None,
            });
        };
        match &mut self.state {
            State::Fire { left, rings, ring } => {
                if first {
                    // TR21: the release flash, the ring (standing / in the
                    // air), the extra ball and Final's own effects, then
                    // `halt`.
                    if !fin {
                        release_effect(&mut out, fk(1), 1);
                    }
                    out.fx.push(Fx::Start {
                        key: *ring,
                        id: if airborne_now { 5 } else { 2 },
                        bound: Some(Anchor::Caster),
                        at: None,
                    });
                    if (wis && odd) || master || (fin && odd) {
                        out.fx.push(Fx::Start {
                            key: fk(3),
                            id: if airborne_now { 7 } else { 6 },
                            bound: Some(Anchor::Caster),
                            at: None,
                        });
                    }
                    if fin {
                        let more: &[u16] = if odd { &[11, 12] } else { &[8, 9] };
                        for (n, &id) in [10u16].iter().chain(more).enumerate() {
                            out.fx.push(Fx::Start {
                                key: fk(4 + n as u8),
                                id,
                                bound: Some(Anchor::Caster),
                                at: None,
                            });
                        }
                    }
                } else {
                    let lift = fire_lift(level, airborne_now);
                    let own = [ctx.caster[0], ctx.caster[1] + lift, ctx.caster[2]];
                    let centre = fb.pos(*ring).unwrap_or(own);
                    let (r, h) = (row.f(0), row.f(1));
                    if *left > 0. {
                        out.volumes.push(Volume {
                            key: key0,
                            atkp: 502,
                            sub,
                            centre,
                            radius: if idx % 10 == 1 && idx / 10 == 3 {
                                450.
                            } else {
                                r
                            },
                            height: h,
                            previous: None,
                            moving: None,
                            role: Role::Damage,
                        });
                        if *rings > 1 {
                            out.volumes.push(Volume {
                                key: key0 + 1,
                                atkp: 502,
                                sub,
                                centre,
                                radius: r - 30.,
                                height: h - 30.,
                                previous: None,
                                moving: None,
                                role: Role::Damage,
                            });
                        }
                        *left -= dt;
                    } else {
                        self.finished = true;
                    }
                }
            }
            State::Blizzard {
                shards,
                released,
                from,
            } => {
                if first {
                    // TR21: mode 0 releases element 1, mode 1 element 7 and
                    // mode 2 both; Final's finisher alternates by volley.
                    let mode = if fin {
                        if odd {
                            match volley {
                                1 | 3 => 1,
                                4 => 2,
                                _ => 0,
                            }
                        } else {
                            2
                        }
                    } else {
                        0
                    };
                    match mode {
                        0 => {
                            release_effect(&mut out, fk(1), 1);
                            from[0] = Some(fk(1));
                        }
                        1 => {
                            release_effect(&mut out, fk(1), 7);
                            from[0] = Some(fk(1));
                        }
                        _ => {
                            release_effect(&mut out, fk(1), 1);
                            release_effect(&mut out, fk(2), 7);
                            *from = [Some(fk(1)), Some(fk(2))];
                        }
                    }
                }
                // TR21 releases the cast effect, `halt`s once, then recoils
                // the caster and starts the shards (section 4.2).
                if self.age >= 1. && !*released {
                    *released = true;
                    out.recoil = Some(row.f(7));
                    let d = norm(facing);
                    // (direction, pierces, homing factor, release effect)
                    let mut dirs: Vec<(Vec3, bool, f32, usize)> = Vec::new();
                    if wis && odd {
                        dirs.push((d, false, 0.5, 0));
                        for (a, k) in [(0.610865, 1.0), (1.22173, 2.0)] {
                            dirs.push((rot_y(d, a), false, k, 0));
                            dirs.push((rot_y(d, -a), false, k, 0));
                        }
                    } else if master {
                        dirs.push((d, false, 1.0, 0));
                        for y in [-0.96, 0.96] {
                            dirs.push(([d[0], y, d[2]], false, 1.0, 0));
                        }
                        for y in [0.4, -0.4] {
                            let v = [d[0], y, d[2]];
                            dirs.push((rot_y(v, std::f32::consts::FRAC_PI_4), false, 1.0, 0));
                            dirs.push((rot_y(v, -std::f32::consts::FRAC_PI_4), false, 1.0, 0));
                        }
                    } else {
                        dirs.push((d, true, 1.0, 0));
                        // Final: the second release effect starts its own
                        // shard.
                        if from[1].is_some() {
                            dirs.push((d, true, 1.0, 1));
                        }
                    }
                    // The shards start at the release effect's position
                    // (PAX element 1, bone 0x4003, the Keyblade's top).
                    for (i, (dir, pierce, k, source)) in dirs.into_iter().enumerate() {
                        let from_fx = from[source].and_then(|s| fb.pos(s));
                        let pos = from_fx.unwrap_or(ctx.caster_centre);
                        let sfx = fk(0x10 + i as u8);
                        out.fx.push(Fx::Start {
                            key: sfx,
                            id: 2,
                            bound: None,
                            at: Some(pos),
                        });
                        shards.push(Shard {
                            key: key0 + 1 + i as u64,
                            fx: sfx,
                            pos,
                            dir: norm(dir),
                            life: row.f(4) - 12.,
                            fade: 12.,
                            fading: false,
                            pierce,
                            rate: row.f(3) * k,
                            radius: row.f(6),
                            speed: row.f(5),
                            previous: None,
                            dead: false,
                        });
                    }
                }
                for s in shards.iter_mut() {
                    if s.dead {
                        continue;
                    }
                    // The loop runs while the effect lives; hit / reflect /
                    // background end the shard before it moves again.
                    let gone = fb.alive(s.fx) == Some(false) && !first;
                    if gone
                        || fb.was_hit(s.key) && !s.pierce
                        || fb.reflects(s.key) > 0
                        || (ctx.bg_hit)(s.pos, s.radius)
                    {
                        s.dead = true;
                        out.fx.push(Fx::Kill(s.fx));
                        continue;
                    }
                    out.fx.push(Fx::Dir(s.fx, s.dir));
                    out.fx.push(Fx::Pos(s.fx, s.pos));
                    out.volumes.push(Volume {
                        key: s.key,
                        atkp: 248,
                        sub,
                        centre: s.pos,
                        radius: s.radius,
                        height: s.radius,
                        previous: s.previous,
                        moving: if s.previous.is_some() {
                            Some(s.dir)
                        } else {
                            None
                        },
                        role: Role::Damage,
                    });
                    s.previous = Some(s.pos);
                    // vector_homing toward the target, then advance.
                    if let Some(t) = ctx.target
                        && s.rate > 0.
                    {
                        s.dir = vector_homing(
                            s.dir,
                            [t[0] - s.pos[0], t[1] - s.pos[1], t[2] - s.pos[2]],
                            s.rate * dt,
                        );
                    }
                    s.pos = add(s.pos, scale(s.dir, s.speed * dt));
                    if s.life > 0. {
                        s.life -= dt;
                        if s.life <= 0. {
                            out.fx.push(Fx::Fade(s.fx, 12.));
                            s.fading = true;
                        }
                    } else if fb.alive(s.fx).is_none() {
                        // No effect runs: the fade is a plain timer.
                        s.fade -= dt;
                        if s.fade <= 0. {
                            s.dead = true;
                        }
                    }
                }
                shards.retain(|s| !s.dead);
                if shards.is_empty() && *released {
                    self.finished = true;
                }
            }
            State::Thunder {
                bolts,
                queue,
                started,
            } => {
                if first {
                    release_effect(&mut out, fk(1), 1);
                    let origin = ctx.caster;
                    let dir = norm(facing);
                    let (p, raise) = match ctx.target {
                        Some(t) => (t, 100.),
                        None => {
                            let d = if master {
                                scale(rot_y(dir, std::f32::consts::FRAC_PI_4), 800.)
                            } else {
                                scale(dir, 300.)
                            };
                            let mut p = add(origin, d);
                            p[1] += 80.;
                            (p, 330.)
                        }
                    };
                    let mut at = 0.;
                    if master {
                        for t in [0., 0.47, 0.78] {
                            let q = [
                                origin[0] + (p[0] - origin[0]) * t,
                                origin[1] + (p[1] - origin[1]) * t,
                                origin[2] + (p[2] - origin[2]) * t,
                            ];
                            queue.push((at, q, raise));
                            at += 16.;
                        }
                    }
                    queue.push((at, p, raise));
                    if fin {
                        let r = 2.5 * row.f(9);
                        let n = if odd { 8 } else { 3 };
                        // bg_get_random_pos_air(P, 0.4R, R, −0.1R, 0.1R) per
                        // bolt: the draws of FUN_1403f4080 on a generator
                        // of the script RNG's kind, seeded by the cast.
                        let mut rng = RandomAir::new(serial);
                        for k in 1..n {
                            let q = rng.next(p, 0.4 * r, r, -0.1 * r, 0.1 * r);
                            queue.push((at + 8. * k as f32, q, 330.));
                        }
                    }
                    if wis && odd {
                        let mut v =
                            rot_y(scale(dir, row.f(9) * 1.5), 2. * std::f32::consts::FRAC_PI_3);
                        for k in 1..=3 {
                            queue.push((at + 8. * k as f32, add(p, v), 330.));
                            v = rot_y(v, 2. * std::f32::consts::FRAC_PI_3);
                        }
                    }
                }
                // Start bolts whose time has come.
                let age = self.age;
                let mut begun = Vec::new();
                queue.retain(|&(t, pos, raise)| {
                    if t <= age {
                        begun.push((pos, raise));
                        false
                    } else {
                        true
                    }
                });
                for (pos, raise) in begun {
                    // The strike point: ground within f8 below, else air.
                    let from = [pos[0], pos[1] + raise, pos[2]];
                    let mut at = pos;
                    let mut atkp = 251;
                    if let Some(g) = (ctx.ground)(from, 10000.)
                        && pos[1] - g[1] < row.f(8)
                    {
                        at = g;
                        atkp = 250;
                    }
                    // pax_start(pax, ground ? 2 : 3) at the strike point.
                    out.fx.push(Fx::Start {
                        key: fk(0x10 + (*started as u8 & 0x3f)),
                        id: if atkp == 250 { 2 } else { 3 },
                        bound: None,
                        at: Some(at),
                    });
                    *started += 1;
                    let n = bolts.len() as u64;
                    bolts.push(Bolt {
                        key: serial << 8 | (1 + n * 2),
                        pos: at,
                        column: [at[0], at[1] + row.f(10), at[2]],
                        atkp,
                        wait: row.f(12),
                        live: row.f(13),
                    });
                }
                for b in bolts.iter_mut() {
                    if b.wait > 0. {
                        b.wait -= dt;
                        continue;
                    }
                    if b.live > 0. {
                        out.volumes.push(Volume {
                            key: b.key,
                            atkp: b.atkp,
                            sub,
                            centre: b.pos,
                            radius: row.f(9),
                            height: row.f(9),
                            previous: None,
                            moving: None,
                            role: Role::Damage,
                        });
                        out.volumes.push(Volume {
                            key: serial << 8 | ((b.key & 0xff) + 1),
                            atkp: 249,
                            sub,
                            centre: b.column,
                            radius: row.f(11),
                            height: row.f(10),
                            previous: None,
                            moving: None,
                            role: Role::Damage,
                        });
                        b.live -= dt;
                    }
                }
                bolts.retain(|b| b.wait > 0. || b.live > 0.);
                if bolts.is_empty() && queue.is_empty() {
                    self.finished = true;
                }
            }
            State::Cure {
                done,
                leaf,
                leaf_centre,
                target,
            } => {
                if !*done {
                    *done = true;
                    release_effect(&mut out, fk(1), 1);
                    // The heal centre is the chosen target's body centre:
                    // `obj_apply_bone_matrix(S[44], 0x4001, origin)`.
                    let chosen = ctx.party.get(*target).copied().unwrap_or(ctx.caster);
                    let lift = ctx.body_centre[1] - ctx.caster[1];
                    let heal = [chosen[0], chosen[1] + lift, chosen[2]];
                    out.fx.push(Fx::Start {
                        key: fk(2),
                        id: 2,
                        bound: None,
                        at: Some(heal),
                    });
                    for (i, p) in ctx.party.iter().enumerate() {
                        let d = [p[0] - heal[0], p[2] - heal[2]];
                        if len2(d) >= row.f(14) {
                            continue;
                        }
                        // The chosen target (708) and the others (709).
                        let own = i == *target;
                        out.fx.push(Fx::Start {
                            key: fk(3 + i as u8),
                            id: if own { 3 } else { 4 },
                            bound: Some(Anchor::Party(i)),
                            at: None,
                        });
                        out.strikes.push(Strike {
                            atkp: if own { 708 } else { 709 },
                            sub,
                            party: i,
                        });
                    }
                }
                if *leaf > 0. {
                    if first {
                        out.invulnerable = Some(60.);
                    }
                    // ATKP 1247 super guard, r 100 at the caster's TARGET
                    // centre for 60 ticks.
                    *leaf_centre = ctx.caster_centre;
                    out.volumes.push(Volume {
                        key: key0,
                        atkp: 1247,
                        sub: 0,
                        centre: *leaf_centre,
                        radius: 100.,
                        height: 100.,
                        previous: None,
                        moving: None,
                        role: Role::Guard,
                    });
                    *leaf -= dt;
                }
                if *leaf <= 0. && *done {
                    self.finished = true;
                }
            }
            State::Magnet {
                centre,
                life,
                placed,
                sphere,
                ended,
            } => {
                if !*placed {
                    *placed = true;
                    release_effect(&mut out, fk(1), 1);
                    // P: the target, else the release effect's position;
                    // lowered to f15 above the ground below it.
                    let mut p = ctx.target.unwrap_or(ctx.caster);
                    let f15 = row.f(15);
                    let q = [p[0], p[1] + f15, p[2]];
                    if let Some(g) = (ctx.ground)(q, 2. * f15) {
                        p = [g[0], g[1] + f15, g[2]];
                    }
                    *centre = p;
                    // e2 = pax_start(pax, 2) at P.
                    out.fx.push(Fx::Start {
                        key: fk(2),
                        id: 2,
                        bound: None,
                        at: Some(p),
                    });
                }
                let f16 = row.f(16);
                let shrink = row.f(18);
                // After `wait 16`: the sphere (S[36]) at P, 1 above, grown
                // by sin((t / f16 − 0.5) π) over f16 ticks.
                if sphere.is_none() && self.age >= 16. {
                    let at = [centre[0], centre[1] + 1., centre[2]];
                    out.fx.push(Fx::Start {
                        key: fk(3),
                        id: 3,
                        bound: None,
                        at: Some(at),
                    });
                    *sphere = Some((fk(3), 0.));
                }
                if let Some((sk, t)) = sphere.as_mut()
                    && ended.is_none()
                {
                    let s = if *t < f16 {
                        (((*t / f16 - 0.5) * std::f32::consts::PI).sin() + 1.) / 2.
                    } else {
                        1.
                    };
                    out.fx.push(Fx::Scale(*sk, s));
                    *t += dt;
                }
                if *life > 0. {
                    out.volumes.push(Volume {
                        key: key0,
                        atkp: 658,
                        sub,
                        centre: *centre,
                        radius: row.f(19),
                        height: row.f(19),
                        previous: None,
                        moving: None,
                        role: Role::Damage,
                    });
                    if self.form != 0 && (idx / 10 >= 1) {
                        out.volumes.push(Volume {
                            key: key0 + 1,
                            atkp: 976,
                            sub,
                            centre: *centre,
                            radius: row.f(15) * 0.5,
                            height: row.f(15) * 0.5,
                            previous: None,
                            moving: None,
                            role: Role::Damage,
                        });
                    }
                    *life -= dt;
                    if *life <= 0. {
                        // The hold ends: e4 at P and the sphere shrinks.
                        out.fx.push(Fx::Start {
                            key: fk(4),
                            id: 4,
                            bound: None,
                            at: Some(*centre),
                        });
                        *ended = Some(shrink);
                    }
                } else if let Some(left) = ended.as_mut() {
                    if let Some((sk, _)) = sphere {
                        let s = (((*left / shrink.max(1.)) - 0.5) * std::f32::consts::PI).sin();
                        out.fx.push(Fx::Scale(*sk, (s + 1.) / 2.));
                        if *left <= 0. {
                            out.fx.push(Fx::Kill(*sk));
                        }
                    }
                    *left -= dt;
                    if *left < 0. {
                        self.finished = true;
                    }
                } else {
                    self.finished = true;
                }
            }
            State::Reflect {
                barrier,
                centre_offset,
                count,
                seen,
                counter,
                key,
                flash,
                flashes,
                started,
            } => {
                let base: u16 = if airborne_now {
                    2
                } else if wis || fin {
                    12
                } else {
                    0
                };
                if first {
                    // TR21: a new barrier replaces the last one.
                    if let Some(old) = ctx.barrier {
                        out.fx.push(Fx::Kill(old));
                    }
                    out.fx.push(Fx::Start {
                        key: *key,
                        id: base,
                        bound: Some(Anchor::Caster),
                        at: None,
                    });
                    out.barrier = Some(Some(*key));
                    *started = true;
                } else if let Some(c) = counter {
                    // Counter phases A then B (§4.6).
                    let (a, b) = match c.tier {
                        1 => (
                            (574u16, row.f(23), row.f(24)),
                            (667u16, row.f(23) + 20., row.f(24) + 20.),
                        ),
                        2 => (
                            (576, row.f(26), row.f(27)),
                            (668, row.f(26) + 20., row.f(27) + 20.),
                        ),
                        _ => (
                            (578, row.f(29), row.f(30)),
                            (669, row.f(29) + 20., row.f(30) + 20.),
                        ),
                    };
                    let (atkp, r, h) = if c.stage == 0 { a } else { b };
                    out.volumes.push(Volume {
                        key: key0 + 10 + c.stage as u64,
                        atkp,
                        sub,
                        centre: c.centre,
                        radius: r,
                        height: h,
                        previous: None,
                        moving: None,
                        role: Role::Damage,
                    });
                    c.left -= dt;
                    if c.left <= 0. {
                        if c.stage == 0 {
                            c.stage = 1;
                            c.left = row.f(22);
                        } else {
                            self.finished = true;
                        }
                    }
                } else {
                    let own = {
                        let o = *centre_offset;
                        let f = ctx.facing;
                        // The offset rotates with the caster's facing.
                        [
                            ctx.caster[0] + o[0] * f[1] + o[2] * f[0],
                            ctx.caster[1] + o[1],
                            ctx.caster[2] - o[0] * f[0] + o[2] * f[1],
                        ]
                    };
                    // The guard follows the barrier effect.
                    let centre = fb.pos(*key).unwrap_or(own);
                    let volume_key = key0;
                    let hits = fb.reflects(volume_key);
                    if hits > *seen {
                        // A new clash: the flash plays unless one still does.
                        let flash_alive = flash.is_some_and(|f| fb.alive(f).unwrap_or(false));
                        if !flash_alive {
                            let fl = fk(0x20 + (*flashes as u8 & 0x1f));
                            *flashes += 1;
                            out.fx.push(Fx::Start {
                                key: fl,
                                id: base + 1,
                                bound: Some(Anchor::Caster),
                                at: None,
                            });
                            *flash = Some(fl);
                        }
                        *seen = hits;
                    }
                    *count = *seen;
                    // The barrier lasts as long as its effect.
                    let alive = match fb.alive(*key) {
                        Some(a) => a,
                        None => *barrier > 0.,
                    };
                    if alive {
                        out.volumes.push(Volume {
                            key: volume_key,
                            atkp: 573,
                            sub,
                            centre,
                            radius: row.f(20),
                            height: row.f(20),
                            previous: None,
                            moving: None,
                            role: Role::Guard,
                        });
                        *barrier -= dt;
                    } else {
                        // T[8] += reflect_count; the last barrier of a
                        // chain turns the total into a counter attack.
                        let n = ctx.barrier_count + *count;
                        let last = ctx.barrier == Some(*key) || ctx.barrier.is_none();
                        out.barrier_count = Some(n);
                        if !last || n == 0 {
                            self.finished = true;
                        } else {
                            out.barrier = Some(None);
                            out.barrier_count = Some(0);
                            let tier = if n as i32 >= row.i(28) {
                                3
                            } else if n as i32 >= row.i(25) {
                                2
                            } else {
                                1
                            };
                            let f: u16 = if wis || master || fin { 10 } else { 0 };
                            let id = match tier {
                                1 => 4,
                                2 => 5,
                                _ => 6,
                            } + f;
                            out.fx.push(Fx::Start {
                                key: fk(0x40),
                                id,
                                bound: Some(Anchor::Caster),
                                at: None,
                            });
                            *counter = Some(Counter {
                                stage: 0,
                                left: row.f(21),
                                tier,
                                centre,
                            });
                        }
                    }
                    let _ = started;
                }
            }
        }
        self.age += dt;
        out
    }
}

/// [I] The barrier's PAX lifetime was not decoded: element 0's PDATA ends
/// by VM frame 27 (≈ 54 ticks at the 30 Hz VM).
pub const BARRIER_LIFE: f32 = 54.;

#[cfg(test)]
mod tests {
    use super::*;

    fn rec(t: u8, level: u8) -> Record {
        Record {
            magic_type: t,
            level,
            world: 0,
            file: String::new(),
            item: 0x15 + t as u16,
            command: 0x31 + level as u16,
            ground: (56, 8),
            finisher: (57, 8),
            air: (58, 8),
            voice: 7,
            voice_finisher: 11,
            voice_self: -1,
        }
    }

    fn row(vals: &[(usize, f32)]) -> Row {
        let mut w = [0u32; 31];
        for &(i, v) in vals {
            w[i] = v.to_bits();
        }
        Row(w)
    }

    fn flat_ctx<'a>(
        party: &'a [Vec3],
        ground: &'a dyn Fn(Vec3, f32) -> Option<Vec3>,
        bg: &'a dyn Fn(Vec3, f32) -> bool,
    ) -> Context<'a> {
        Context {
            dt: 1.,
            caster: [0., 0., 0.],
            facing: [0., 1.],
            caster_centre: [0., 70., 0.],
            body_centre: [0., 85., 0.],
            airborne: false,
            casting: true,
            target: None,
            party,
            ground,
            bg_hit: bg,
            barrier: None,
            barrier_count: 0,
        }
    }

    fn floor(from: Vec3, max: f32) -> Option<Vec3> {
        (from[1] >= 0. && from[1] <= max + 1.).then_some([from[0], 0., from[2]])
    }

    #[test]
    fn variant_index_and_start_follow_the_cast_rules() {
        let r = rec(FIRE, 2);
        assert_eq!(variant_index(2, false, 0), 2);
        assert_eq!(variant_index(2, true, 0), 3);
        assert_eq!(variant_index(1, true, 2), 11);
        assert_eq!(variant_index(3, false, 5), 34);
        let s = start(&r, true, false, 0, false);
        assert_eq!((s.motion, s.voice, s.row, s.finisher), (57, 11, 3, true));
        // Air casts never use the finisher motion or the odd row.
        let a = start(&r, true, true, 0, false);
        assert_eq!(
            (a.motion, a.row, a.finisher, a.airborne),
            (58, 2, false, true)
        );
        // A spell without a finisher motion falls back to the ground one.
        let mut cure = rec(CURE, 1);
        cure.finisher = (-1, 0);
        let c = start(&cure, true, false, 0, false);
        assert_eq!((c.motion, c.row), (56, 0));
    }

    #[test]
    fn current_record_follows_the_owned_count() {
        let records: Vec<Record> = (1..=3).map(|l| rec(FIRE, l)).collect();
        assert_eq!(Record::current(&records, FIRE, 0, 0), None);
        assert_eq!(Record::current(&records, FIRE, 2, 0).unwrap().level, 2);
        assert_eq!(Record::current(&records, FIRE, 9, 0).unwrap().level, 3);
        assert!(Record::current(&records, THUNDER, 1, 0).is_none());
    }

    #[test]
    fn mp_charge_starts_only_below_zero_and_ends_with_a_full_bar() {
        let mut mp = Mp::new(100);
        assert_eq!(mp.change(-12, CHARGE_LENGTH), 88);
        assert!(!mp.charging());
        mp.cur = 10;
        assert_eq!(mp.change(-12, CHARGE_LENGTH), 0);
        assert!(mp.charging());
        assert_eq!((mp.remaining, mp.total), (3000., 3000.));
        // Gains shorten the charge by 12 per point.
        mp.add(10);
        assert_eq!(mp.remaining, 3000. - 120.);
        // Haste doubles the run-down.
        mp.tick(1., 2.);
        assert_eq!(mp.remaining, 3000. - 120. - 2.);
        mp.tick(5000., 1.);
        assert!(!mp.charging());
        assert_eq!(mp.cur, 100);
    }

    #[test]
    fn colour_marks_charging_and_orange_casts() {
        let mut mp = Mp::new(100);
        assert_eq!(colour(12, &mp, false, false), 0);
        mp.cur = 5;
        assert_eq!(colour(12, &mp, false, false), 2);
        assert_eq!(colour(12, &mp, true, false), 5);
        // Cure always starts a charge, whatever the toggle says.
        assert_eq!(colour(0xff, &mp, true, false), 2);
        mp.change(-12, CHARGE_LENGTH);
        assert_eq!(colour(12, &mp, false, false), 5);
        assert_eq!(colour(12, &Mp::new(100), false, true), 5);
    }

    #[test]
    fn fire_ring_follows_the_caster_for_the_patched_duration() {
        let party = [[0., 0., 0.]];
        let r = row(&[(0, 200.), (1, 80.), (2, 44.)]);
        let ground = floor;
        let bg = |_: Vec3, _: f32| false;
        let mut ctx = flat_ctx(&party, &ground, &bg);
        let mut s = Spell::new(1, FIRE, 1, 0, r, 0, &ctx);
        let mut ticks = 0;
        let mut volumes = 0;
        while !s.finished() {
            ctx.caster = [ticks as f32, 0., 0.];
            let o = s.tick(&ctx, &Feedback::default());
            if let Some(v) = o.volumes.first() {
                volumes += 1;
                assert_eq!(v.atkp, 502);
                assert_eq!(v.centre, [ticks as f32, 92.88, 0.]);
                assert_eq!((v.radius, v.height), (200., 80.));
            }
            ticks += 1;
            assert!(ticks < 100);
        }
        // One `halt`, then 42 ticks of volume (the 60 fps patch).
        assert_eq!(volumes, 42);
        assert_eq!(ticks, 44);
    }

    #[test]
    fn blizzard_shard_flies_straight_pierces_and_fades() {
        let party = [[0., 0., 0.]];
        let r = row(&[(4, 30.), (5, 40.), (6, 80.), (7, 50.)]);
        let ground = floor;
        let bg = |_: Vec3, _: f32| false;
        let ctx = flat_ctx(&party, &ground, &bg);
        let mut s = Spell::new(2, BLIZZARD, 1, 0, r, 0, &ctx);
        let mut fb = Feedback::default();
        let mut last = None;
        let mut n = 0;
        while !s.finished() {
            let o = s.tick(&ctx, &fb);
            if let Some(v) = o.volumes.first() {
                last = Some(*v);
                fb.hit = vec![v.key];
            }
            n += 1;
            assert!(n < 100);
        }
        let v = last.unwrap();
        // Pierces: still alive after hits; moved 40 per tick along +Z.
        assert_eq!(v.atkp, 248);
        assert!(v.centre[2] > 40. * 10.);
        assert_eq!(v.centre[1], 70.);
        // Life 30 − 12 then 12 of fade: 30 ticks in all.
        assert!((29..=31).contains(&n));
    }

    #[test]
    fn thunder_strikes_the_ground_under_the_target_after_the_delay() {
        let party = [[0., 0., 0.]];
        let r = row(&[
            (8, 150.),
            (9, 170.),
            (10, 340.),
            (11, 30.),
            (12, 6.),
            (13, 10.),
        ]);
        let ground = floor;
        let bg = |_: Vec3, _: f32| false;
        let mut ctx = flat_ctx(&party, &ground, &bg);
        ctx.target = Some([100., 50., 200.]);
        let mut s = Spell::new(3, THUNDER, 1, 0, r, 0, &ctx);
        let fb = Feedback::default();
        let mut first_hit = None;
        let mut count = 0;
        for t in 0..40 {
            let o = s.tick(&ctx, &fb);
            if !o.volumes.is_empty() {
                first_hit.get_or_insert(t);
                count += 1;
                let strike = o.volumes.iter().find(|v| v.atkp == 250).unwrap();
                assert_eq!(strike.centre, [100., 0., 200.]);
                let col = o.volumes.iter().find(|v| v.atkp == 249).unwrap();
                assert_eq!((col.radius, col.height, col.centre[1]), (30., 340., 340.));
            }
            if s.finished() {
                break;
            }
        }
        assert_eq!(first_hit, Some(6));
        assert_eq!(count, 10);
        // Without a target: 300 ahead, raised 80, so the ground is within
        // 150 below and the strike is the ground one at y 0.
        let mut ctx2 = flat_ctx(&party, &ground, &bg);
        ctx2.target = None;
        let mut s = Spell::new(4, THUNDER, 1, 0, r, 0, &ctx2);
        let mut seen = None;
        for _ in 0..20 {
            let o = s.tick(&ctx2, &fb);
            if let Some(v) = o.volumes.first() {
                seen = Some(*v);
                break;
            }
        }
        let v = seen.unwrap();
        assert_eq!((v.atkp, v.centre), (250, [0., 0., 300.]));
    }

    #[test]
    fn cure_heals_the_party_in_range_and_leaf_bracer_guards() {
        let party = [[0., 0., 0.], [100., 0., 0.], [900., 0., 0.]];
        let r = row(&[(14, 280.)]);
        let ground = floor;
        let bg = |_: Vec3, _: f32| false;
        let ctx = flat_ctx(&party, &ground, &bg);
        let mut s = Spell::new(5, CURE, 1, 0, r, 0, &ctx);
        let o = s.tick(&ctx, &Feedback::default());
        assert_eq!(
            o.strikes,
            vec![
                Strike {
                    atkp: 708,
                    sub: 0,
                    party: 0
                },
                Strike {
                    atkp: 709,
                    sub: 0,
                    party: 1
                },
            ]
        );
        assert!(o.volumes.is_empty() && s.finished());
        // Leaf Bracer is its own thread from the cast's start.
        let mut leaf = Spell::leaf_bracer(6, &ctx);
        let o = leaf.tick(&ctx, &Feedback::default());
        assert_eq!(o.invulnerable, Some(60.));
        assert_eq!((o.volumes[0].atkp, o.volumes[0].role), (1247, Role::Guard));
        let mut n = 1;
        while !leaf.finished() {
            leaf.tick(&ctx, &Feedback::default());
            n += 1;
        }
        assert_eq!(n, 60);
    }

    #[test]
    fn magnet_sits_above_the_ground_and_lives_167_ticks() {
        let party = [[0., 0., 0.]];
        let r = row(&[(15, 180.), (16, 16.), (17, 135.), (19, 250.)]);
        let ground = floor;
        let bg = |_: Vec3, _: f32| false;
        let mut ctx = flat_ctx(&party, &ground, &bg);
        ctx.target = Some([40., 30., 60.]);
        let mut s = Spell::new(6, MAGNET, 1, 0, r, 0, &ctx);
        let first = s.tick(&ctx, &Feedback::default());
        assert_eq!(first.volumes[0].centre, [40., 180., 60.]);
        assert_eq!(first.volumes[0].radius, 250.);
        let mut n = 1;
        while !s.finished() {
            s.tick(&ctx, &Feedback::default());
            n += 1;
        }
        assert_eq!(n, 168);
    }

    #[test]
    fn reflect_barrier_counts_clashes_and_counters_by_tier() {
        let party = [[0., 0., 0.]];
        let mut w = [0u32; 31];
        let set = |w: &mut [u32; 31], i: usize, v: f32| w[i] = v.to_bits();
        set(&mut w, 20, 110.);
        set(&mut w, 21, 28.);
        set(&mut w, 22, 6.);
        set(&mut w, 23, 240.);
        set(&mut w, 24, 150.);
        w[25] = 5;
        set(&mut w, 26, 300.);
        set(&mut w, 27, 180.);
        w[28] = 10;
        set(&mut w, 29, 380.);
        set(&mut w, 30, 220.);
        let r = Row(w);
        let ground = floor;
        let bg = |_: Vec3, _: f32| false;
        let ctx = flat_ctx(&party, &ground, &bg);
        let mut s = Spell::new(7, REFLECT, 1, 0, r, 0, &ctx);
        let key = 7u64 << 8;
        let mut fb = Feedback::default();
        let mut guard_seen = false;
        let mut counters = Vec::new();
        for t in 0..200 {
            if t == 10 {
                fb.reflected = vec![(key, 6)];
            }
            let o = s.tick(&ctx, &fb);
            for v in &o.volumes {
                if v.atkp == 573 {
                    guard_seen = true;
                    assert_eq!(v.role, Role::Guard);
                    assert_eq!(v.centre, [-22. * 1. + 0., 85., -18.2]);
                } else {
                    counters.push((v.atkp, v.radius));
                }
            }
            if s.finished() {
                break;
            }
        }
        assert!(guard_seen);
        // Six reflects: tier 2 (≥ 5): ATKP 576 for 28 ticks, then 668.
        assert_eq!(counters.iter().filter(|c| c.0 == 576).count(), 28);
        assert_eq!(counters.iter().filter(|c| c.0 == 668).count(), 6);
        assert_eq!(counters[0].1, 300.);
        assert!(s.finished());
    }

    fn starts(out: &Output) -> Vec<(u16, bool)> {
        out.fx
            .iter()
            .filter_map(|f| match f {
                Fx::Start { id, bound, .. } => Some((*id, bound.is_some())),
                _ => None,
            })
            .collect()
    }

    #[test]
    fn the_cast_starts_the_elements_cast_effect_except_in_wisdom_and_final_fire() {
        let key = cast_key(9);
        let id = |t: u8, idx: usize| match cast_effects(t, idx, key).as_slice() {
            [
                Fx::Start {
                    id, bound, key: k, ..
                },
            ] => {
                assert_eq!((*bound, *k), (Some(Anchor::Caster), key));
                Some(*id)
            }
            [] => None,
            other => panic!("{other:?}"),
        };
        assert_eq!(id(FIRE, 4), Some(0));
        assert_eq!(id(FIRE, 11), None);
        assert_eq!(id(FIRE, 34), None);
        assert_eq!(id(FIRE, 24), Some(0));
        for t in [BLIZZARD, THUNDER, CURE, MAGNET] {
            assert_eq!(id(t, 2), Some(0));
        }
        assert_eq!(id(REFLECT, 2), None);
    }

    #[test]
    fn fire_releases_its_flash_and_ring_then_waits_a_tick() {
        let party = [[0., 0., 0.]];
        let ground = floor;
        let bg = |_: Vec3, _: f32| false;
        let mut ctx = flat_ctx(&party, &ground, &bg);
        let r = row(&[(0, 200.), (1, 80.), (2, 44.)]);
        let mut s = Spell::new(1, FIRE, 1, 0, r, 0, &ctx).with_cast(cast_key(1), 0);
        let first = s.tick(&ctx, &Feedback::default());
        // Standing: cast loop ends, flash (1), ring (2); no volume yet.
        assert!(first.fx.contains(&Fx::LoopEnd(cast_key(1))));
        assert_eq!(starts(&first), vec![(1, true), (2, true)]);
        assert!(first.volumes.is_empty());
        // In the air the ring is element 5.
        ctx.airborne = true;
        let mut a = Spell::new(2, FIRE, 1, 0, r, 0, &ctx);
        assert_eq!(
            starts(&a.tick(&ctx, &Feedback::default())),
            vec![(1, true), (5, true)]
        );
        // The Wisdom finisher (11) adds the extra ball, 6 standing.
        ctx.airborne = false;
        let mut w = Spell::new(3, FIRE, 1, 11, r, 2, &ctx);
        assert_eq!(
            starts(&w.tick(&ctx, &Feedback::default())),
            vec![(1, true), (2, true), (6, true)]
        );
        // Final has no release flash; its non-finisher adds 10, 8 and 9.
        let mut f = Spell::new(4, FIRE, 1, 30, r, 5, &ctx);
        assert_eq!(
            starts(&f.tick(&ctx, &Feedback::default())),
            vec![(2, true), (10, true), (8, true), (9, true)]
        );
    }

    #[test]
    fn the_fire_ring_follows_its_effects_position() {
        let party = [[0., 0., 0.]];
        let ground = floor;
        let bg = |_: Vec3, _: f32| false;
        let ctx = flat_ctx(&party, &ground, &bg);
        let r = row(&[(0, 200.), (1, 80.), (2, 44.)]);
        let mut s = Spell::new(5, FIRE, 1, 0, r, 0, &ctx);
        s.tick(&ctx, &Feedback::default());
        let ring = fx_key(5, 2);
        let mut fb = Feedback::default();
        fb.fx.push((
            ring,
            FxLive {
                alive: true,
                pos: [3., 90., 4.],
                tracked: true,
            },
        ));
        assert_eq!(s.tick(&ctx, &fb).volumes[0].centre, [3., 90., 4.]);
    }

    #[test]
    fn blizzard_shards_start_at_the_release_effect_and_end_with_their_effect() {
        let party = [[0., 0., 0.]];
        let ground = floor;
        let bg = |_: Vec3, _: f32| false;
        let ctx = flat_ctx(&party, &ground, &bg);
        let r = row(&[(4, 30.), (5, 40.), (6, 80.), (7, 50.)]);
        let mut s = Spell::new(6, BLIZZARD, 1, 0, r, 0, &ctx);
        let first = s.tick(&ctx, &Feedback::default());
        assert_eq!(starts(&first), vec![(1, true)]);
        // The release effect sits at the Keyblade's top.
        let mut fb = Feedback::default();
        fb.fx.push((
            fx_key(6, 1),
            FxLive {
                alive: true,
                pos: [5., 55., 20.],
                tracked: true,
            },
        ));
        let second = s.tick(&ctx, &fb);
        // Shard effect: element 2, unbound, at the release effect.
        let shard = second
            .fx
            .iter()
            .find_map(|f| match f {
                Fx::Start {
                    id: 2,
                    bound: None,
                    at,
                    key,
                } => Some((*key, *at)),
                _ => None,
            })
            .unwrap();
        assert_eq!(shard.1, Some([5., 55., 20.]));
        assert_eq!(second.volumes[0].centre, [5., 55., 20.]);
        // The shard lasts while its effect lives: it ends when it is dead.
        fb.fx.push((
            shard.0,
            FxLive {
                alive: false,
                pos: [5., 55., 60.],
                tracked: true,
            },
        ));
        let third = s.tick(&ctx, &fb);
        assert!(third.volumes.is_empty() && third.fx.contains(&Fx::Kill(shard.0)));
        assert!(s.finished());
    }

    #[test]
    fn final_form_blizzard_alternates_its_release_effects_by_volley() {
        let party = [[0., 0., 0.]];
        let ground = floor;
        let bg = |_: Vec3, _: f32| false;
        let ctx = flat_ctx(&party, &ground, &bg);
        let r = row(&[(4, 30.), (5, 40.), (6, 80.), (7, 50.)]);
        let ids = |idx: usize, volley: u32| {
            let mut s = Spell::new(7, BLIZZARD, 1, idx, r, 5, &ctx).with_cast(cast_key(7), volley);
            starts(&s.tick(&ctx, &Feedback::default()))
                .into_iter()
                .map(|s| s.0)
                .collect::<Vec<_>>()
        };
        // Finisher (odd): volleys 1 and 3 use element 7, 4 both, others 1.
        assert_eq!(ids(31, 0), vec![1]);
        assert_eq!(ids(31, 1), vec![7]);
        assert_eq!(ids(31, 4), vec![1, 7]);
        // Not a finisher: both at once.
        assert_eq!(ids(30, 0), vec![1, 7]);
    }

    #[test]
    fn thunder_starts_its_bolt_effect_at_the_strike_point() {
        let party = [[0., 0., 0.]];
        let r = row(&[
            (8, 150.),
            (9, 170.),
            (10, 340.),
            (11, 30.),
            (12, 6.),
            (13, 10.),
        ]);
        let ground = floor;
        let bg = |_: Vec3, _: f32| false;
        let mut ctx = flat_ctx(&party, &ground, &bg);
        ctx.target = Some([100., 50., 200.]);
        let mut s = Spell::new(8, THUNDER, 1, 0, r, 0, &ctx);
        let o = s.tick(&ctx, &Feedback::default());
        // Release effect (1), then the ground-strike element (2) at the
        // floor under the target.
        let bolt =
            o.fx.iter()
                .find_map(|f| match f {
                    Fx::Start {
                        id,
                        bound: None,
                        at,
                        ..
                    } => Some((*id, *at)),
                    _ => None,
                })
                .unwrap();
        assert_eq!(bolt, (2, Some([100., 0., 200.])));
        assert!(starts(&o).contains(&(1, true)));
    }

    #[test]
    fn a_new_reflect_replaces_the_barrier_and_carries_the_clashes_of_the_old() {
        let party = [[0., 0., 0.]];
        let mut w = [0u32; 31];
        let set = |w: &mut [u32; 31], i: usize, v: f32| w[i] = v.to_bits();
        set(&mut w, 20, 110.);
        set(&mut w, 21, 28.);
        set(&mut w, 22, 6.);
        set(&mut w, 23, 240.);
        set(&mut w, 24, 150.);
        w[25] = 5;
        set(&mut w, 26, 300.);
        set(&mut w, 27, 180.);
        w[28] = 10;
        let r = Row(w);
        let ground = floor;
        let bg = |_: Vec3, _: f32| false;
        let mut ctx = flat_ctx(&party, &ground, &bg);
        // A barrier is already up: the new cast kills it and registers its
        // own handle.
        ctx.barrier = Some(77);
        let mut s = Spell::new(9, REFLECT, 1, 0, r, 0, &ctx);
        let first = s.tick(&ctx, &Feedback::default());
        assert!(first.fx.contains(&Fx::Kill(77)));
        assert_eq!(starts(&first), vec![(0, true)]);
        assert_eq!(first.barrier, Some(Some(fx_key(9, 1))));
        // The barrier follows its effect and ends with it; the clashes are
        // carried on when a newer barrier replaced this one.
        ctx.barrier = Some(fx_key(10, 1));
        let key = fx_key(9, 1);
        let mut fb = Feedback {
            reflected: vec![(9 << 8, 2)],
            ..Default::default()
        };
        fb.fx.push((
            key,
            FxLive {
                alive: true,
                pos: [1., 2., 3.],
                tracked: true,
            },
        ));
        let o = s.tick(&ctx, &fb);
        assert_eq!(o.volumes[0].centre, [1., 2., 3.]);
        // The first clash starts the flash element (base + 1).
        assert_eq!(starts(&o), vec![(1, true)]);
        fb.fx[0].1.alive = false;
        let o = s.tick(&ctx, &fb);
        assert_eq!(o.barrier_count, Some(2));
        assert!(s.finished());
    }

    #[test]
    fn random_air_positions_stay_in_the_ring_and_cover_the_sectors() {
        let mut rng = RandomAir::new(3);
        let c = [100., 50., -20.];
        let mut sectors = [false; 8];
        for _ in 0..8 {
            let p = rng.next(c, 100., 250., -25., 25.);
            let (dx, dz) = (p[0] - c[0], p[2] - c[2]);
            let d = dx.hypot(dz);
            assert!((100. ..=250.).contains(&d), "{d}");
            assert!((-25. ..=25.).contains(&(p[1] - c[1])));
            let a = dx.atan2(dz).rem_euclid(std::f32::consts::TAU);
            sectors[(a / std::f32::consts::FRAC_PI_4) as usize % 8] = true;
        }
        // Eight draws visit eight different sectors.
        assert!(sectors.iter().all(|&s| s));
    }

    #[test]
    fn homing_turns_about_the_cross_axis_by_at_most_the_rate() {
        // A quarter turn wanted, 0.1 allowed: the heading turns 0.1 radians.
        let v = vector_homing([1., 0., 0.], [0., 0., 1.], 0.1);
        assert!((v[0] - 0.1f32.cos()).abs() < 1e-5 && (v[2] - 0.1f32.sin()).abs() < 1e-5);
        // Less than the rate left: it lands on the target direction.
        let v = vector_homing([1., 0., 0.], [0.99, 0., 0.01], 0.5);
        assert!((v[2] / v[0] - 0.01 / 0.99).abs() < 1e-4);
        // Nearly opposite: it still comes around.
        let mut v = [1., 0., 0.];
        for _ in 0..40 {
            v = vector_homing(v, [-1., 0., 0.01], 0.1745);
        }
        assert!(v[0] < -0.99);
    }
}
