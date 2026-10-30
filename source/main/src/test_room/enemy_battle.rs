//! Ordinary source-bound enemies in the generated development arena.
//! Story flags, encounter scripts and unsupported variants stay outside this host.
use super::{
    Game, RoomWorld,
    enemy_battle_hits::ContactState,
    enemy_catalog::{Entry, Kind},
    enemy_combat::{AttackWindow, EnemyBinding, EnemyCombat, ShapePose},
    enemy_gallery::EnemyGallery,
};
use bevy::prelude::*;
use data::{motion::SelectedMotion, motion_eval, probe_source::ProbeSource};
use player::{
    World, actor_physics, combat, dusk,
    shadow::{self, ScriptRandom, Shadow},
    soldier,
};
use std::{collections::HashMap, sync::Arc};

pub enum Brain {
    /// The enemy's own object script, run by the translated interpreter.
    Script(Box<super::enemy_script::ScriptBrain>),
    Shadow(Box<Shadow>),
    Dusk(Box<dusk::Dusk>),
    Soldier(Box<soldier::Soldier>),
}
impl Brain {
    /// Moves the object by a push (FUN_14040ad70 adds it to obj+0xa48).
    pub fn nudge(&mut self, d: [f32; 3]) {
        match self {
            Self::Script(a) => {
                let mut p = a.position();
                (0..3).for_each(|k| p[k] += d[k]);
                a.set_position(p);
            }
            Self::Shadow(a) => (0..3).for_each(|k| a.position[k] += d[k]),
            Self::Dusk(a) => (0..3).for_each(|k| a.position[k] += d[k]),
            Self::Soldier(a) => (0..3).for_each(|k| a.position[k] += d[k]),
        }
    }
    pub fn position(&self) -> [f32; 3] {
        match self {
            Self::Script(a) => a.position(),
            Self::Shadow(a) => a.position,
            Self::Dusk(a) => a.position,
            Self::Soldier(a) => a.position,
        }
    }
    pub fn yaw(&self) -> f32 {
        match self {
            Self::Script(a) => a.yaw(),
            Self::Shadow(a) => a.yaw,
            Self::Dusk(a) => a.yaw,
            Self::Soldier(a) => a.yaw,
        }
    }
    pub fn hp(&self) -> i32 {
        match self {
            Self::Script(a) => a.hp(),
            Self::Shadow(a) => a.hp,
            Self::Dusk(a) => a.hp,
            Self::Soldier(a) => a.hp,
        }
    }
    pub fn alive(&self) -> bool {
        match self {
            Self::Script(a) => a.alive(),
            Self::Shadow(a) => a.alive(),
            Self::Dusk(a) => a.alive(),
            Self::Soldier(a) => a.alive(),
        }
    }
    pub fn targetable(&self) -> bool {
        match self {
            Self::Script(a) => a.targetable(),
            Self::Shadow(a) => a.targetable(),
            Self::Dusk(a) => a.targetable(),
            Self::Soldier(a) => a.targetable(),
        }
    }
    pub fn record_mut(&mut self) -> &mut combat::HitRecord {
        match self {
            Self::Script(a) => a.record_mut(),
            Self::Shadow(a) => &mut a.record,
            Self::Dusk(a) => a.record(),
            Self::Soldier(a) => a.record(),
        }
    }
    pub fn hit(
        &mut self,
        d: [f32; 3],
        high: bool,
        p: &combat::AttackParams,
        s: &combat::AttackerStats,
        difficulty: combat::Difficulty,
        record: HitRecordInfo,
    ) -> Option<i32> {
        match self {
            Self::Script(a) => a.hit(d, high, p, s, difficulty, record),
            Self::Shadow(a) => a.hit(d, high, p, s, difficulty),
            Self::Dusk(a) => a.hit(d, high, p, s, difficulty),
            Self::Soldier(a) => a.hit(d, high, p, s, difficulty),
        }
    }
    /// FUN_1403ba4c0's "active" test for the reaction search: alive and
    /// not dead-marked. The hand ports offer nothing.
    pub fn reaction_active(&self) -> bool {
        match self {
            Self::Script(a) => a.targetable(),
            _ => false,
        }
    }
    /// FUN_1403ab6f0's test of one enemy-class object for the battle state:
    /// a boss-class object (+0x6c8 bit 10) always counts; one of team 2
    /// (+0x4dc) counts within `range` of Sora horizontally, or anywhere when
    /// its character id is 0x417 to 0x423 (FUN_1403baa60). The hand ports
    /// count as team 2.
    pub fn keeps_battle(&self, character: u16, sora: [f32; 3], range: f32) -> bool {
        if !self.alive() {
            return false;
        }
        let team = match self {
            Self::Script(a) => {
                if a.boss() {
                    return true;
                }
                a.team()
            }
            _ => 2,
        };
        let p = self.position();
        team == 2
            && ((0x417..0x424).contains(&character)
                || ((p[0] - sora[0]).powi(2) + (p[2] - sora[2]).powi(2)).sqrt() < range)
    }
    /// FUN_1403ba550: whether collision group `group` is on (the hand ports
    /// keep the table's default groups).
    pub fn group_on(&self, group: u8, default_groups: u32) -> bool {
        match self {
            Self::Script(a) => a.group_on(i32::from(group), default_groups),
            _ => group < 32 && default_groups >> group & 1 != 0,
        }
    }
    /// FUN_1403ba600: whether a REACTION shape of `group` offers `command`.
    pub fn reaction_offered(&self, command: i32, group: i32, default_groups: u32) -> bool {
        match self {
            Self::Script(a) => a.reaction_offered(command, group, default_groups),
            _ => false,
        }
    }
    /// FUN_1403b5fa0: Sora executed reaction command `command` on this
    /// actor (its script's trigger 11).
    pub fn command(&mut self, command: u16, group: u8) {
        if let Self::Script(a) = self {
            a.command(command, group);
        }
    }
    /// Development aid: set a script actor's HP.
    pub fn set_hp(&mut self, hp: i32) {
        if let Self::Script(a) = self {
            a.set_hp(hp);
        }
    }
    /// The script engine's boss state for logs (null for the hand ports).
    pub fn boss_state(&self) -> serde_json::Value {
        match self {
            Self::Script(a) => a.boss_state(),
            _ => serde_json::Value::Null,
        }
    }
    pub fn reflect(&mut self, motion: usize, back: f32) -> bool {
        match self {
            Self::Script(a) => {
                a.reflect(motion, back);
                true
            }
            Self::Soldier(a) => {
                a.reflect(motion, back);
                true
            }
            Self::Shadow(a) => {
                a.reflect(motion, back);
                true
            }
            Self::Dusk(a) => {
                a.reflect(motion, back);
                true
            }
        }
    }
    pub fn motion(&self) -> usize {
        match self {
            Self::Script(a) => a.motion(),
            Self::Shadow(a) => a.motion,
            Self::Dusk(a) => a.motion,
            Self::Soldier(a) => a.motion,
        }
    }
    pub fn elapsed(&self) -> u32 {
        match self {
            Self::Script(a) => a.elapsed(),
            Self::Shadow(a) => a.motion_tick,
            Self::Dusk(a) => a.motion_tick,
            Self::Soldier(a) => a.motion_tick,
        }
    }
    pub fn serial(&self) -> u64 {
        match self {
            Self::Script(a) => a.serial(),
            Self::Shadow(a) => a.motion_serial as u64,
            Self::Dusk(a) => a.motion_serial,
            Self::Soldier(a) => a.motion_serial,
        }
    }
    /// The object's alpha (+0xa08) and whether the class hid it (`obj_hide`,
    /// `obj_model_hide`).
    pub fn shown(&self) -> (f32, bool) {
        match self {
            Self::Script(a) => a.shown(),
            _ => (1., false),
        }
    }
    pub fn scale_y(&self) -> f32 {
        match self {
            Self::Script(a) => a.scale_y(),
            Self::Shadow(a) => a.scale_y,
            Self::Dusk(a) => a.scale_y,
            Self::Soldier(_) => 1.,
        }
    }
    /// The new motion's share of the shown pose while its blend runs (the
    /// hand ports show the new motion's start frame).
    pub fn blend_weight(&self) -> Option<f32> {
        match self {
            Self::Script(a) => a.blend_weight(),
            _ => None,
        }
    }
    pub fn blend(&self) -> f32 {
        match self {
            Self::Script(a) => a.blend(),
            Self::Shadow(_) => 0.,
            Self::Dusk(a) => a.blend,
            Self::Soldier(a) => a.blend,
        }
    }
    pub fn gone(&self) -> bool {
        match self {
            Self::Script(a) => a.gone(),
            Self::Shadow(a) => a.gone,
            Self::Dusk(a) => a.gone,
            Self::Soldier(a) => a.gone,
        }
    }
    pub fn fault(&self) -> Option<String> {
        match self {
            Self::Script(a) => a.fault.clone(),
            Self::Shadow(_) => None,
            Self::Dusk(a) => a.fault.map(|f| format!("{f:?}")),
            Self::Soldier(a) => a.fault.map(|f| format!("{f:?}")),
        }
    }
    pub fn description(&self) -> String {
        match self {
            Self::Script(a) => a.description(),
            Self::Shadow(a) => format!("{:?}", a.current_act()),
            Self::Dusk(a) => format!("{:?}", a.current_act()),
            Self::Soldier(a) => format!("{:?}", a.current_act()),
        }
    }
    pub fn defeat(&mut self) {
        match self {
            Self::Script(a) => a.defeat(),
            Self::Shadow(a) => a.defeat(),
            Self::Dusk(a) => a.defeat(),
            Self::Soldier(a) => a.defeat(),
        }
    }
    pub fn respawn(&mut self) {
        match self {
            Self::Script(a) => a.respawn(),
            Self::Shadow(a) => a.respawn(),
            Self::Dusk(a) => a.respawn(),
            Self::Soldier(a) => a.respawn(),
        }
    }
    pub fn capture(&mut self, key: u64, centre: [f32; 3], radius: f32, spin: f32) -> bool {
        match self {
            Self::Script(_) => false,
            Self::Shadow(a) => {
                a.capture(key, centre, radius, spin);
                true
            }
            Self::Dusk(_) | Self::Soldier(_) => false,
        }
    }
    /// The attack objects a script brain placed (none for the other brains).
    pub fn script_attacks(&self) -> Vec<super::enemy_script::ScriptAttack> {
        match self {
            Self::Script(a) => a.attacks(),
            _ => Vec::new(),
        }
    }
    /// A script attack struck Sora.
    /// FUN_1403d1730's owner call (vt+0xe0) for a hit on Sora: the
    /// script's OBJ_ATTACK with the attack object (`slot`) or the motion
    /// window's attack `atkp`.
    pub fn attack_reached_player(&mut self, slot: Option<usize>, atkp: u16) {
        if let Self::Script(a) = self {
            a.attack_reached_player(slot, i32::from(atkp));
        }
    }
    pub fn mark_script_attack(&mut self, slot: usize) {
        if let Self::Script(a) = self {
            a.mark_attack_hit(slot);
        }
    }
    /// Sora's guard turned a script attack back.
    pub fn mark_script_attack_reflected(&mut self, slot: usize) {
        if let Self::Script(a) = self {
            a.mark_attack_reflected(slot);
        }
    }
    /// The attacks a script applied to Sora directly this frame
    /// (`attack_strike`): slot, parameter id and extra.
    pub fn take_script_strikes(&mut self) -> Vec<(usize, i32, i32)> {
        match self {
            Self::Script(a) => a.take_strikes(),
            _ => Vec::new(),
        }
    }
    /// The effects a script brain started this frame (none for other brains).
    pub fn take_script_effects(&mut self) -> Vec<super::enemy_script::ScriptEffect> {
        match self {
            Self::Script(a) => std::mem::take(&mut a.effects),
            _ => Vec::new(),
        }
    }
    /// What a script brain asked of the host this frame (none for other
    /// brains).
    pub fn take_script_reports(&mut self) -> super::enemy_script::ScriptReports {
        match self {
            Self::Script(a) => a.take_reports(),
            _ => Default::default(),
        }
    }
    pub fn follow_capture(&mut self, volumes: &[player::magic::Volume]) {
        if let Self::Shadow(a) = self
            && let Some(h) = a.held
        {
            if let Some(v) = volumes.iter().find(|v| v.key == h.key) {
                if let Some(held) = a.held.as_mut() {
                    held.centre = v.centre;
                }
            } else {
                a.release_held();
            }
        }
    }
}

pub(super) struct Clip {
    choice: usize,
    motion: Arc<SelectedMotion>,
    duration: u32,
    loop_top: u32,
    start: f32,
    fps: f32,
    roots: Vec<[f32; 3]>,
    windows: Vec<AttackWindow>,
    /// The clip's trigger tables as the enemy engine reads them.
    events: enemy::motion::MotionEvents,
}
fn native_clock(t: u32, duration: u32, loop_top: u32) -> u32 {
    if t < duration {
        t
    } else if loop_top < duration {
        loop_top + (t - duration) % (duration - loop_top)
    } else {
        duration
    }
}
fn native_frame_event(frame: u16, t: u32, duration: u32, loop_top: u32) -> bool {
    let frame = frame as u32;
    if t == 0 {
        return frame == 0;
    }
    let clock = native_clock(t, duration, loop_top);
    let step = if t == 2 || (loop_top == 0 && clock == 2) {
        2
    } else {
        1
    };
    let previous = native_clock(t.saturating_sub(step), duration, loop_top);
    if previous < clock {
        previous < frame && frame <= clock
    } else if previous > clock {
        (previous < frame && frame <= duration) || (loop_top < frame && frame <= clock)
    } else {
        false
    }
}
impl Clip {
    fn clock(&self, t: u32) -> u32 {
        native_clock(t, self.duration, self.loop_top)
    }
    fn key(&self, t: u32) -> f32 {
        self.clock(t) as f32 + self.start * 60. / self.fps
    }
    fn root(&self, t: u32) -> [f32; 3] {
        self.roots[self.clock(t) as usize]
    }
}
pub(super) struct Clips(HashMap<usize, Clip>);
impl Clips {
    /// The clip a brain's motion slot plays: its choice, motion and the
    /// source key at `elapsed` (the first key while blending), as
    /// [`BattleActor::tick`] poses it.
    /// The attack windows of slot `slot`'s clip (none while blending) and
    /// its event clock at `elapsed`.
    pub(super) fn windows_at(
        &self,
        slot: usize,
        elapsed: u32,
        blending: bool,
    ) -> Option<(&[AttackWindow], f32)> {
        let c = self.0.get(&slot)?;
        let windows: &[AttackWindow] = if blending { &[] } else { &c.windows };
        Some((windows, c.clock(elapsed) as f32))
    }

    pub(super) fn pose_key(
        &self,
        slot: usize,
        elapsed: u32,
        blending: bool,
    ) -> Option<(usize, Arc<SelectedMotion>, f32)> {
        let c = self.0.get(&slot)?;
        Some((
            c.choice,
            c.motion.clone(),
            c.key(if blending { 0 } else { elapsed }),
        ))
    }
}
impl shadow::Clips for Clips {
    fn advance(&self, _s: usize, t: u32) -> u32 {
        if t == 0 { 2 } else { t.saturating_add(1) }
    }
    fn root_step(&self, s: usize, from: u32, to: u32) -> [f32; 3] {
        let Some(c) = self.0.get(&s) else {
            return [0.; 3];
        };
        if to < from {
            [0, 1, 2].map(|i| {
                c.roots[c.duration as usize][i] - c.root(from)[i] + c.root(to)[i]
                    - c.roots[c.loop_top as usize][i]
            })
        } else {
            [0, 1, 2].map(|i| c.root(to)[i] - c.root(from)[i])
        }
    }
    fn length(&self, s: usize) -> u32 {
        self.0.get(&s).map_or(0, |c| c.duration)
    }
    fn loop_start(&self, s: usize) -> u32 {
        self.0.get(&s).map_or(0, |c| c.loop_top)
    }
    fn root(&self, s: usize, t: u32) -> [f32; 3] {
        self.0.get(&s).map_or([0.; 3], |c| c.root(t))
    }
}
impl enemy::env::Clips for Clips {
    fn length(&self, slot: usize) -> u32 {
        shadow::Clips::length(self, slot)
    }
    fn loop_start(&self, slot: usize) -> u32 {
        shadow::Clips::loop_start(self, slot)
    }
    fn root_delta(&self, slot: usize, tick: u32) -> [f32; 3] {
        dusk::Clips::root_delta(self, slot, tick)
    }
    fn advance(&self, slot: usize, tick: u32) -> u32 {
        dusk::Clips::advance(self, slot, tick)
    }
    fn events(&self, slot: usize) -> Option<&enemy::motion::MotionEvents> {
        self.0.get(&slot).map(|c| &c.events)
    }
    fn ended(&self, slot: usize, tick: u32) -> bool {
        dusk::Clips::ended(self, slot, tick)
    }
    fn in_loop(&self, slot: usize, tick: u32) -> bool {
        dusk::Clips::in_loop(self, slot, tick)
    }
}

impl dusk::Clips for Clips {
    fn takeoff(&self, s: usize, t: u32) -> Option<bool> {
        let c = self.0.get(&s)?;
        Some(
            c.motion
                .triggers
                .iter()
                .flat_map(|tr| &tr.records.frames)
                .any(|e| {
                    e.trigger == 0 && native_frame_event(e.frame as u16, t, c.duration, c.loop_top)
                }),
        )
    }
    fn gravity_frozen(&self, s: usize, t: u32) -> Option<bool> {
        soldier::Clips::no_gravity(self, s, t)
    }
    fn advance(&self, s: usize, t: u32) -> u32 {
        let at = self.0.get(&s).map_or(t, |c| c.clock(t));
        t.saturating_add(if at == 0 { 2 } else { 1 })
    }
    fn length(&self, s: usize) -> u32 {
        shadow::Clips::length(self, s)
    }
    fn loop_start(&self, s: usize) -> u32 {
        shadow::Clips::loop_start(self, s)
    }
    fn root(&self, s: usize, t: u32) -> [f32; 3] {
        shadow::Clips::root(self, s, t)
    }
    fn root_delta(&self, s: usize, t: u32) -> [f32; 3] {
        let Some(c) = self.0.get(&s) else {
            return [0.; 3];
        };
        let at = c.clock(t);
        let next = (if at == 0 { 2 } else { at + 1 }).min(c.duration);
        [0, 1, 2].map(|i| c.roots[next as usize][i] - c.roots[at as usize][i])
    }
    fn range(&self, s: usize, t: u32, r: u8) -> Option<bool> {
        let c = self.0.get(&s)?;
        let clock = c.clock(t);
        Some(
            c.motion
                .triggers
                .iter()
                .flat_map(|t| &t.records.ranges)
                .any(|e| {
                    e.trigger == 15 + r
                        && e.start_frame as u16 as u32 <= clock
                        && (e.end_frame as u16 == u16::MAX || clock < e.end_frame as u16 as u32)
                }),
        )
    }
    fn ended(&self, s: usize, t: u32) -> bool {
        self.0
            .get(&s)
            .is_some_and(|c| t > 0 && (c.clock(t - 1) + 1 >= c.duration))
    }
    fn in_loop(&self, s: usize, t: u32) -> bool {
        self.0.get(&s).is_some_and(|c| c.clock(t) >= c.loop_top)
    }
}

struct Arena<'a>(&'a RoomWorld);
impl soldier::Clips for Clips {
    fn no_gravity(&self, s: usize, t: u32) -> Option<bool> {
        let c = self.0.get(&s)?;
        let clock = c.clock(t);
        Some(
            c.motion
                .triggers
                .iter()
                .flat_map(|t| &t.records.ranges)
                .any(|r| {
                    matches!(r.trigger, 3 | 24 | 30)
                        && r.start_frame as u16 as u32 <= clock
                        && (r.end_frame as u16 == u16::MAX || clock < r.end_frame as u16 as u32)
                }),
        )
    }
}
impl shadow::Env for Arena<'_> {
    fn floor(&self, x: f32, z: f32, top: f32) -> Option<f32> {
        self.0.floor(x, z, top)
    }
}
impl dusk::Env for Arena<'_> {
    fn floor(&self, x: f32, z: f32, top: f32) -> Option<f32> {
        self.0.floor(x, z, top)
    }
    // The generated room has a flat floor and no step obstacles. Source
    // encounter step probes are not admitted by this development-room host.
    fn check_step(&self, _: [f32; 3], _: [f32; 3]) -> Option<u8> {
        Some(0)
    }
}

/// The parts of a hit's damage record the script-driven enemies read
/// (FUN_1403d23c0): the ATKP id and its RevengeDamage, the HP pool of the
/// struck shape, and who attacked.
#[derive(Clone, Copy, Debug, Default)]
pub struct HitRecordInfo {
    pub param_id: u16,
    pub revenge: u8,
    pub pool: u8,
    pub by_leader: bool,
    pub by_party: bool,
}

pub struct BattleActor {
    pub brain: Brain,
    pub assets: EnemyCombat,
    pub pose: ShapePose,
    pub motion: Arc<SelectedMotion>,
    pub frame: f32,
    pub serial: u64,
    pub windows: Vec<AttackWindow>,
    pub contacts: ContactState,
    /// The HP after each hit applied this tick, in order: the damage events
    /// the lock manager hands the Scan gauge (FUN_1403d2eb0 / FUN_1403d2c50).
    pub hp_events: Vec<i32>,
    clips: Arc<Clips>,
    pub paused: bool,
    pub error: Option<String>,
    /// A script brain's effect containers (see [`ScriptFx`]).
    script_fx: Option<ScriptFx>,
    /// The bones of this tick's pose, for effects bound to them.
    pub bones: super::enemy_combat::ActorBones,
}

/// A script brain's effects on the tracked effect path (`spell_fx`), as the
/// summon's: each call on the PAX of the object whose script made it
/// (FUN_1403cc650's object effects), loaded on first use.
struct ScriptFx {
    library: data::path_lookup::Library,
    models: HashMap<u32, String>,
    paxes: HashMap<u32, Option<Arc<data::object_effect::Pax>>>,
}

impl ScriptFx {
    fn open(source: &ProbeSource) -> Option<Self> {
        let mut library = data::path_lookup::Library::open(source).ok()?;
        let objects = data::objentry::read(&library.read("00objentry.bin").ok()?).ok()?;
        let models = objects
            .iter()
            .map(|o| {
                (
                    o.object_id,
                    String::from_utf8_lossy(&o.model_name).into_owned(),
                )
            })
            .collect();
        Some(Self {
            library,
            models,
            paxes: HashMap::new(),
        })
    }

    fn pax(&mut self, entry: u32) -> Option<Arc<data::object_effect::Pax>> {
        let Self {
            library,
            models,
            paxes,
        } = self;
        paxes
            .entry(entry)
            .or_insert_with(|| {
                let name = models.get(&(entry & 0x0fff_ffff))?;
                super::drive_room::object_pax(library, name).map(Arc::new)
            })
            .clone()
    }
}

/// The supported enemies' objentry ids and model names.
pub const SUPPORTED: [(u32, &str); 4] = [
    (301, "M_EX010"),
    (302, "M_EX020"),
    (318, "M_EX990"),
    (SEPHIROTH, "N_HB630"),
];
/// Sephiroth's objentry id (model N_HB630): the one boss run by the
/// object script alone.
pub const SEPHIROTH: u32 = 2230;

/// An object's YS::VOICE_EE: the line it was asked for (+0xc, -1 none), the
/// priority of the last accepted request (+0x10) and the line playing
/// (+0x14). [I] The priority's starting value is taken as 0.
#[derive(Clone, Copy, Debug, Default)]
pub struct VoiceSlot {
    pending: Option<u32>,
    priority: i32,
    playing: Option<u32>,
}

impl VoiceSlot {
    /// FUN_1403ce580: a request is taken when nothing waits or plays, or
    /// when its priority is at least the held one.
    pub fn request(&mut self, line: u32, priority: i32) -> bool {
        let taken = (self.pending.is_none() && self.playing.is_none()) || self.priority <= priority;
        if taken {
            self.pending = Some(line);
            self.priority = priority;
        }
        taken
    }

    /// FUN_1403ce260, the object's update: a waiting line stops the one
    /// playing (FUN_1401de500) and plays (FUN_1403ce560 -> FUN_1401de230);
    /// otherwise a line that finished is dropped. Returns (stop, play).
    pub fn update(&mut self, still_playing: impl Fn(u32) -> bool) -> (Option<u32>, Option<u32>) {
        if let Some(line) = self.pending.take() {
            let stop = self.playing.replace(line);
            return (stop, Some(line));
        }
        if self.playing.is_some_and(|l| !still_playing(l)) {
            self.playing = None;
        }
        (None, None)
    }
}
/// The ENMP row Sephiroth's script binds.
pub const SEPHIROTH_SHEET: u16 = 140;
/// The row `btlobj_set_sheet` falls back to (FUN_1403ebf30): the host's
/// stand-in until the object's own script binds its sheet.
const FALLBACK_SHEET: u16 = 0x45;
/// Objects that can fight in the room: Sephiroth, and every ordinary enemy
/// (objentry types 4 and 21, OpenKH Objentry.cs's normal enemies), each run
/// by its own object script.
pub fn supported(e: &Entry) -> bool {
    (e.object_id, e.model_name.as_str()) == (SEPHIROTH, "N_HB630")
        || matches!(e.object_type, 4 | 21)
}
/// The hand-written Soldier, Shadow and Dusk brains, kept for comparison
/// runs only (`KH2RW_ENEMY_AI=port`).
fn hand_port(e: &Entry) -> bool {
    std::env::var("KH2RW_ENEMY_AI").is_ok_and(|v| v == "port")
        && matches!(
            (e.object_id, e.model_name.as_str(), e.character),
            (301, "M_EX010", 1000) | (302, "M_EX020", 1001) | (318, "M_EX990", 3000)
        )
}
pub fn target_params(source: &ProbeSource) -> Result<player::targeting::Params, String> {
    source.verify_session().map_err(|e| e.to_string())?;
    let mut lib = data::path_lookup::Library::open(source).map_err(|e| e.to_string())?;
    let system = lib.read("03system.bin").map_err(|e| e.to_string())?;
    let pref = super::bar_entry(&system, b"pref").map_err(|e| e.to_string())?;
    let plyr = super::bar_entry(pref, b"plyr").map_err(|e| e.to_string())?;
    let offset = u32::from_le_bytes(
        plyr.get(8..12)
            .ok_or("Sora PLYR offset absent")?
            .try_into()
            .map_err(|_| "PLYR offset invalid")?,
    ) as usize;
    let params =
        player::targeting::Params::read(plyr.get(offset..).ok_or("PLYR record outside table")?)
            .ok_or("Sora PLYR record truncated")?;
    if [
        params.range,
        params.dy_min,
        params.dy_max,
        params.hold,
        params.height_weight,
        params.angle_weight,
    ]
    .iter()
    .any(|v| !v.is_finite())
    {
        return Err("PLYR target parameters nonfinite".into());
    }
    source.verify_session().map_err(|e| e.to_string())?;
    Ok(params)
}
pub fn lock_range(source: &ProbeSource) -> Result<f32, String> {
    let mut lib = data::path_lookup::Library::open(source).map_err(|e| e.to_string())?;
    let system = lib.read("03system.bin").map_err(|e| e.to_string())?;
    let pref = super::bar_entry(&system, b"pref").map_err(|e| e.to_string())?;
    let sstm = super::bar_entry(pref, b"sstm").map_err(|e| e.to_string())?;
    let offset = u32::from_le_bytes(
        sstm.get(4..8)
            .ok_or("SSTM record offset absent")?
            .try_into()
            .map_err(|_| "SSTM offset invalid")?,
    ) as usize;
    let at = offset
        .checked_add(0x4c)
        .ok_or("SSTM lock offset overflow")?;
    let range = f32::from_le_bytes(
        sstm.get(at..at + 4)
            .ok_or("SSTM lock range absent")?
            .try_into()
            .map_err(|_| "SSTM lock range invalid")?,
    );
    if !range.is_finite() || range <= 0. {
        return Err("SSTM lock range invalid".into());
    }
    Ok(range)
}
/// An enemy a script made (`sysobj_appear`, M_EX870's M_EX960): its model and
/// motions. It lives in its maker's script world; the gallery draws it. Each
/// entry is read once and kept for the session, so a borrowed environment can
/// hand its clips out. [I] Its motions' attack windows are not read.
pub(super) struct MadeKind {
    pub clips: Clips,
}

fn made_kinds() -> &'static std::sync::Mutex<HashMap<u32, &'static MadeKind>> {
    static KINDS: std::sync::OnceLock<std::sync::Mutex<HashMap<u32, &'static MadeKind>>> =
        std::sync::OnceLock::new();
    KINDS.get_or_init(Default::default)
}

/// The kind of a made enemy already read.
pub(super) fn made_kind(entry: u32) -> Option<&'static MadeKind> {
    made_kinds().lock().ok()?.get(&entry).copied()
}

/// Reads a made enemy's motions the first time its entry is made.
pub(super) fn load_made_kind(source: &ProbeSource, object: &data::objentry::ObjectEntry) {
    if made_kind(object.object_id).is_some() {
        return;
    }
    let Ok(entry) = super::enemy_catalog::plain_entry(source, object) else {
        return;
    };
    let Ok(kind) = super::enemy_catalog::load_entry_kind(source, &entry) else {
        return;
    };
    let Ok(clips) = build_clips(&kind, |_| Ok(Vec::new())) else {
        return;
    };
    if let Ok(mut kinds) = made_kinds().lock() {
        kinds.insert(object.object_id, Box::leak(Box::new(MadeKind { clips })));
    }
}

/// The clips of a kind's motion set: native clock, root motion, the attack
/// windows `windows` gives and the trigger tables the engine reads.
pub(super) fn build_clips(
    kind: &Kind,
    mut windows: impl FnMut(&SelectedMotion) -> Result<Vec<AttackWindow>, String>,
) -> Result<Clips, String> {
    let mut clips = HashMap::new();
    for (choice, m) in kind
        .motions
        .iter()
        .enumerate()
        .filter(|(_, m)| m.status.is_none())
    {
        let motion = kind.load_motion(choice).map_err(|e| e.to_string())?;
        // An absent type-16 entry means no events, including the ordinary
        // Shadow/Soldier idle clips. It is not a malformed event table.
        if motion.triggers.len() > 1
            || motion.triggers.iter().any(|t| {
                t.records.declared_frame_offset < 0
                    || t.records.declared_frame_offset as usize != t.records.sequential_frame_offset
            })
        {
            return Err("native event table offset differs from admitted decoder".into());
        }
        let (start, _, fps, repeat) = motion.motion.frame_window().map_err(|e| e.to_string())?;
        let duration = motion.motion.raw.as_ref().map_or_else(
            || {
                motion
                    .motion
                    .interpolated_header_words
                    .get(1)
                    .copied()
                    .unwrap_or(0)
            },
            |r| r.raw_header_words[4],
        );
        // Clock 0 is the motion's first key, which can be negative (Air
        // Pirate's slot 241 starts at -10): the evaluated sampler takes it.
        let loop_top = (repeat - start) * 60. / fps;
        if duration == 0
            || duration > 4096
            || !loop_top.is_finite()
            || loop_top < 0.
            || loop_top > duration as f32
            || loop_top.fract() != 0.
            || !start.is_finite()
            || !fps.is_finite()
            || fps <= 0.
        {
            return Err(format!(
                "slot {} native clock outside admitted profile (start {start}, fps {fps}, \
                 return {repeat}, length {duration})",
                m.slot
            ));
        }
        let mut roots = Vec::new();
        for t in 0..=duration {
            let key = t as f32 + start * 60. / fps;
            // A raw-matrix motion (No. 4, B_EX210, B_EX380) carries its root
            // in its own table; the pose is sampled the same way when shown.
            let root = match &motion.motion.raw {
                Some(raw) => raw.sample(key).map_err(|e| e.to_string())?.1,
                None => {
                    motion_eval::evaluate(&kind.model.skinning.bones, &motion.motion, key)
                        .map_err(|e| e.to_string())?
                        .root_translation
                }
            };
            if root.iter().any(|v| !v.is_finite()) {
                return Err("nonfinite root motion".into());
            }
            roots.push(root);
        }
        let windows = windows(&motion)?;
        let events = enemy::motion::MotionEvents {
            ranges: motion
                .triggers
                .iter()
                .flat_map(|t| &t.records.ranges)
                .map(|r| enemy::motion::RangeEvent {
                    start: r.start_frame as u16,
                    end: r.end_frame as u16,
                    kind: r.trigger,
                    params: r.parameters.iter().map(|&v| v as u16).collect(),
                })
                .collect(),
            frames: motion
                .triggers
                .iter()
                .flat_map(|t| &t.records.frames)
                .map(|f| enemy::motion::FrameEvent {
                    frame: f.frame as u16,
                    kind: f.trigger,
                    params: f.parameters.iter().map(|&v| v as u16).collect(),
                })
                .collect(),
        };
        clips.insert(
            m.slot,
            Clip {
                choice,
                motion,
                duration,
                loop_top: loop_top.round() as u32,
                start,
                fps,
                roots,
                windows,
                events,
            },
        );
    }
    Ok(Clips(clips))
}

impl BattleActor {
    #[allow(clippy::too_many_arguments)]
    pub fn load(
        source: &Arc<ProbeSource>,
        entry: &Entry,
        kind: &Kind,
        position: Vec3,
        yaw: f32,
        level: u16,
        difficulty: combat::Difficulty,
        spawn_serial: u16,
    ) -> Result<Self, String> {
        if !supported(entry) {
            return Err(
                "only ordinary enemies (objentry types 4 and 21) and Sephiroth fight".into(),
            );
        }
        if kind.variants {
            return Err("contextual motion variants are not qualified for battle".into());
        }
        let port = hand_port(entry);
        let binding = if entry.object_id == SEPHIROTH {
            // His OBJ_INIT binds ENMP row 140 (`btlobj_set_sheet(self, 140,
            // 0)`, FUN_140411350); the host builds the same sheet up front so
            // his attacks on Sora use its strength.
            EnemyBinding {
                enmp_id: SEPHIROTH_SHEET,
                prize_id: None,
            }
        } else if !port {
            // An ordinary enemy's init binds its own row (Soldier 33, Shadow
            // 34, Dusk 41); the assets are read again with that row once the
            // script has run.
            EnemyBinding {
                enmp_id: FALLBACK_SHEET,
                prize_id: None,
            }
        } else if entry.character == 3000 {
            EnemyBinding {
                enmp_id: 41,
                prize_id: Some(6),
            }
        } else if entry.character == 1000 {
            EnemyBinding {
                enmp_id: 33,
                prize_id: Some(1),
            }
        } else {
            EnemyBinding {
                enmp_id: 34,
                prize_id: Some(2),
            }
        };
        let mut assets = EnemyCombat::load(source, entry, kind, binding, level, difficulty)?;
        let clips = Arc::new(build_clips(kind, |m| assets.windows(m))?);
        let brain = if !port {
            let brain = super::enemy_script::ScriptBrain::load(
                source,
                &entry.model_name,
                position.to_array(),
                yaw,
                assets.stats,
                assets.sstm,
                clips.clone(),
                super::enemy_script::ScriptSetup {
                    object_type: assets.object_type,
                    entry_id: entry.object_id,
                    sheets: Some(assets.sheets.clone()),
                    patn: assets.patn.clone(),
                    spawn_serial,
                    character: entry.character,
                    entry_flags: entry.flags,
                },
                super::boss::spawner(source).map(|s| Box::new(s) as Box<dyn enemy::world::Spawner>),
            )?;
            // The row the script bound (0: it bound none, the stand-in
            // stays). Its EXP, prize and strength are the host's.
            if let Ok(sheet) = u16::try_from(brain.sheet())
                && sheet != 0
                && sheet != binding.enmp_id
            {
                let binding = EnemyBinding {
                    enmp_id: sheet,
                    prize_id: None,
                };
                assets = EnemyCombat::load(source, entry, kind, binding, level, difficulty)?;
            }
            Brain::Script(Box::new(brain))
        } else if entry.character == 1000 {
            Brain::Soldier(Box::new(soldier::Soldier::new(
                position.to_array(),
                yaw,
                assets.stats,
                assets.sstm,
            )))
        } else if entry.character == 3000 {
            Brain::Dusk(Box::new(dusk::Dusk::new(
                position.to_array(),
                yaw,
                assets.stats,
                assets.sstm,
            )))
        } else {
            Brain::Shadow(Box::new(Shadow::new(
                position.to_array(),
                yaw,
                assets.stats,
                assets.sstm,
            )))
        };
        let c = clips
            .0
            .get(&brain.motion())
            .ok_or("initial source motion absent")?;
        let frame = c.key(0);
        let (pose, bones) = assets.sample_shapes(kind, &c.motion, frame, position, yaw, false)?;
        let script_fx = matches!(brain, Brain::Script(_))
            .then(|| ScriptFx::open(source))
            .flatten();
        Ok(Self {
            brain,
            motion: c.motion.clone(),
            windows: c.windows.clone(),
            assets,
            pose,
            frame,
            serial: 0,
            contacts: ContactState::default(),
            hp_events: Vec::new(),
            clips,
            paused: false,
            error: None,
            script_fx,
            bones,
        })
    }
    pub fn tick(
        &mut self,
        world: &RoomWorld,
        player: [f32; 3],
        kind: &Kind,
        rng: &mut ScriptRandom,
    ) -> Result<(usize, f32), String> {
        if !self.paused && !self.brain.gone() {
            match &mut self.brain {
                Brain::Script(a) => {
                    a.wall_radius = self.assets.wall_radius;
                    a.set_map_body(self.assets.map_body);
                    a.tick(world, player, rng)
                }
                Brain::Shadow(a) => a.tick(rng, &*self.clips, &Arena(world), player, player),
                Brain::Dusk(a) => a.tick(
                    dusk::Input::arena(
                        player,
                        player,
                        actor_physics::ObjParams::defaults(self.assets.sstm.fall_max).turn,
                    ),
                    &*self.clips,
                    &Arena(world),
                    rng,
                ),
                Brain::Soldier(a) => a.tick(
                    dusk::Input::arena(
                        player,
                        player,
                        actor_physics::ObjParams::defaults(self.assets.sstm.fall_max).turn,
                    ),
                    &*self.clips,
                    &Arena(world),
                    rng,
                ),
            }
        }
        if let Some(f) = self.brain.fault() {
            return Err(f);
        }
        let c = self.clips.0.get(&self.brain.motion()).ok_or_else(|| {
            format!(
                "AI requested unsupported source motion {}",
                self.brain.motion()
            )
        })?;
        self.motion = c.motion.clone();
        self.serial = self.brain.serial();
        self.frame = c.clock(self.brain.elapsed()) as f32;
        self.windows = if self.brain.blend() > 0. {
            Vec::new()
        } else {
            c.windows.clone()
        };
        let key = c.key(if self.brain.blend() > 0. {
            0
        } else {
            self.brain.elapsed()
        });
        (self.pose, self.bones) = self.assets.sample_shapes(
            kind,
            &self.motion,
            key,
            Vec3::from_array(self.brain.position()),
            self.brain.yaw(),
            false,
        )?;
        Ok((c.choice, key))
    }
}

pub fn tick(mut game: ResMut<Game>, mut gallery: ResMut<EnemyGallery>) {
    let tick = game.tick.saturating_sub(1);
    let forward_kinematics_only = gallery.forward_kinematics_only;
    let Some(rt) = game.enemy.as_mut() else {
        return;
    };
    // Scripts consume one shared stream. Summoning a new actor must not
    // restart that stream or give every actor an identical private seed.
    let mut rng = std::mem::take(&mut rt.rng);
    for actor in &mut gallery.actors {
        let Some(battle) = actor.battle.as_mut() else {
            continue;
        };
        if battle.error.is_some() {
            continue;
        }
        battle.paused = actor.paused || game.cheats.enemy_frozen;
        battle
            .brain
            .follow_capture(game.magic.as_ref().map_or(&[], |m| m.volumes.as_slice()));
        if let Brain::Script(a) = &mut battle.brain {
            // Its effects on the tracked path: their state after the room's
            // last effect update, as the script's effect tests see it.
            if battle.script_fx.is_some() {
                let base = super::enemy_script::ACTOR_FX_KEY | (actor.id & 0xff_ffff) << 32;
                a.fx.drawn = true;
                a.fx.anchor = Some(player::magic::Anchor::Actor(actor.id));
                a.fx.key_base = base;
                a.fx.tick = tick;
                a.fx.live = game
                    .fx_live
                    .iter()
                    .filter(|(k, _)| **k >> 32 == base >> 32)
                    .map(|(&k, &l)| (k, l))
                    .collect();
            }
            a.player_yaw = game.sora.player.yaw;
            // A battle actor fights: the battle flag (DAT_142a11404), and
            // whether Sora cannot be damaged (`obj_is_star`).
            a.facts.battle = true;
            a.facts.player_centre = super::SORA_OBJ[2];
            a.facts.player_hp = game
                .enemy
                .as_ref()
                .map(|_| (game.sora.vitals.hp, game.sora.vitals.target.max_hp));
            a.facts.player_mp = Some((game.sora.player.mp.cur, game.sora.player.mp.max));
            a.facts.player_star = game.cheats.invincible
                || game.sora.player.range_invulnerable(&game.clips)
                || game
                    .enemy
                    .as_ref()
                    .is_some_and(|_| game.sora.vitals.invulnerable > 0.);
        }
        let prior = (battle.brain.motion(), battle.brain.serial());
        match battle.tick(
            &game.world,
            game.sora.player.position,
            &actor.kind,
            &mut rng,
        ) {
            Ok((choice, key)) => {
                // FUN_1403b9090 → FUN_14040ad70: Sora and the actor push
                // each other apart by weight. [I] Once a tick after the
                // actor moved; the pushed positions skip the wall sweep.
                if battle.brain.alive() && !battle.brain.gone() && !game.sora.player.dead() {
                    let mut sora = super::actor_push::sora(
                        &game.sora_obj_shapes,
                        game.sora_weight,
                        &game.sora.player,
                        &game.clips,
                    );
                    let mut other = player::body_push::Body {
                        shapes: super::actor_push::shapes(
                            &battle.assets.obj_shapes,
                            battle.brain.position(),
                            battle.brain.yaw(),
                        ),
                        weight: battle.assets.weight,
                        fixed: false,
                    };
                    let (moved, back) = player::body_push::resolve(&mut sora, &mut other, [0.; 3]);
                    battle.brain.nudge([moved[0], 0., moved[2]]);
                    let p = &mut game.sora.player.position;
                    p[0] -= back[0];
                    p[2] -= back[2];
                    if moved != [0.; 3] || back != [0.; 3] {
                        let record = serde_json::json!({"event":"body_push","tick":tick,
                            "actor":actor.id,"moved":moved,"back":back,"weight":battle.assets.weight,
                            "shapes":battle.assets.obj_shapes.len()});
                        game.event(record);
                    }
                }
                actor.position = Vec3::from_array(battle.brain.position());
                actor.yaw = battle.brain.yaw();
                match actor.battle_pose(choice, key, forward_kinematics_only) {
                    Err(e) => actor.error = Some(e),
                    Ok(Some(jump)) if game.autoplay.is_some() => {
                        let blend = actor.battle.as_ref().and_then(|b| b.brain.blend_weight());
                        game.event(serde_json::json!({"event":"enemy_pose_switch","tick":tick,
                            "actor":actor.id,"motion":actor.battle.as_ref().map(|b| b.brain.motion()),
                            "jump":jump,"blend_weight":blend}));
                    }
                    Ok(_) => {}
                }
                super::enemy_motion_fx::scan(&mut game, actor);
                if let Some(battle) = actor.battle.as_mut() {
                    let reports = battle.brain.take_script_reports();
                    for &(id, _) in &reports.sounds {
                        if let Ok(id) = u32::try_from(id) {
                            game.sound_queue.push((id, "enemy_script_se"));
                        }
                    }
                    // `obj_voice` (FUN_1403dad90 -> FUN_1403ce580): line `id`
                    // of the object's voice bank at priority `arg`, played by
                    // its YS::VOICE_EE on its update (play_sounds). [I] The
                    // level 0x1800 is not applied.
                    let bank = game
                        .enemy_voice_banks
                        .get(&battle.assets.object_id)
                        .copied();
                    for (id, arg) in reports.voices {
                        let taken = match (bank, u32::try_from(id)) {
                            (Some(bank), Ok(line)) => game
                                .enemy_voice_slots
                                .entry(actor.id)
                                .or_default()
                                .request(u32::from(bank) * 1000 + line, arg),
                            _ => false,
                        };
                        game.event(serde_json::json!({"event":"enemy_script_voice","tick":tick,"actor":actor.id,"id":id,"arg":arg,"bank":bank,"taken":taken}));
                    }
                    for name in reports.services {
                        game.event(serde_json::json!({"event":"enemy_script_service","tick":tick,"actor":actor.id,"service":name}));
                    }
                    for name in reports.unsupported {
                        game.event(serde_json::json!({"event":"enemy_script_unsupported","tick":tick,"actor":actor.id,"service":name}));
                    }
                    if let (Brain::Script(a), Some(sfx)) =
                        (&mut battle.brain, battle.script_fx.as_mut())
                    {
                        for (object, fx) in std::mem::take(&mut a.fx.calls) {
                            let Some(entry) = a.object_entry(object) else {
                                continue;
                            };
                            let pax = sfx.pax(entry);
                            game.fx_calls.push(super::spell_fx::Call { pax, fx });
                        }
                    }
                    let object = battle.assets.object_id;
                    for e in battle.brain.take_script_effects() {
                        game.effect_queue.push(match e.at {
                            Some(at) => super::EffectRequest::ActorElementAt {
                                object,
                                id: e.id,
                                at: Vec3::from_array(at),
                            },
                            None => super::EffectRequest::ActorElement {
                                object,
                                actor: actor.id,
                                id: e.id,
                            },
                        });
                    }
                }
            }
            Err(e) => {
                battle.error = Some(e.clone());
                actor.error = Some(e.clone());
                game.event(serde_json::json!({"event":"enemy_battle_fault","tick":tick,"actor":actor.id,"reason":e}));
            }
        }
        if let Some(battle) = &actor.battle
            && prior != (battle.brain.motion(), battle.brain.serial())
        {
            game.event(serde_json::json!({"event":"enemy_battle_action","tick":tick,"actor":actor.id,"motion":battle.brain.motion(),"serial":battle.brain.serial(),"action":battle.brain.description(),"hp":battle.brain.hp(),"position":battle.brain.position()}));
        }
    }
    game.enemy.as_mut().expect("combat session retained").rng = rng;
    super::enemy_battle_hits::tick(&mut game, &mut gallery.actors, tick);
    super::enemy_battle_hits::retire(&mut game, &mut gallery.actors, tick);
    super::boss::requests(&mut game, &mut gallery.actors);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn clock_wraps_at_duration_into_the_source_return_window() {
        assert_eq!(native_clock(63, 64, 8), 63);
        assert_eq!(native_clock(64, 64, 8), 8);
        assert_eq!(native_clock(65, 64, 8), 9);
        assert_eq!(native_clock(120, 64, 8), 8);
        assert_eq!(native_clock(65, 64, 64), 64);
    }

    #[test]
    fn frame_events_cover_the_initial_skip_and_loop_end_once() {
        assert!(native_frame_event(0, 0, 10, 4));
        assert!(native_frame_event(1, 2, 10, 4));
        assert!(native_frame_event(2, 2, 10, 4));
        assert!(!native_frame_event(1, 3, 10, 4));
        assert!(native_frame_event(10, 10, 10, 4));
        assert!(!native_frame_event(4, 10, 10, 4));
        assert!(native_frame_event(5, 11, 10, 4));
        assert!(!native_frame_event(10, 11, 10, 4));
    }

    #[test]
    fn voice_slot_takes_requests_by_priority_and_replaces_the_line() {
        let mut v = VoiceSlot::default();
        // Idle: anything is taken.
        assert!(v.request(10, 5));
        // A waiting line of priority 5 refuses a lower one, takes an equal one.
        assert!(!v.request(11, 4));
        assert!(v.request(12, 5));
        assert_eq!(v.update(|_| true), (None, Some(12)));
        // Playing at 5: a lower request is refused, a higher one replaces
        // the playing line on the next update.
        assert!(!v.request(13, 1));
        assert!(v.request(14, 9));
        assert_eq!(v.update(|_| true), (Some(12), Some(14)));
        // Once the line ends the slot is free again, whatever the priority.
        assert_eq!(v.update(|_| false), (None, None));
        assert!(v.request(15, 0));
    }
}
