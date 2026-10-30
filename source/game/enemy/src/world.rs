//! The engine objects scripts act on, and the machinery that runs them.
//!
//! A [`World`] owns every script-driven object ([`Object`]) and the engine
//! state they share (random generator, frame time, the player). A trap runs
//! with a [`Ctx`]: the object whose script called it plus the world.

use crate::act;
use crate::env::Env;
use crate::script::{self, Callback, Exit, Fault, Host, Instance, Memory, Slot, Thread};
use crate::sysobj::Sysobj;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};

pub type ObjId = u32;

/// The scripts' shared random generator (DAT_1407535c0): an LCG with
/// multiplier 0x10dcd, initial value 1.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Lcg(pub u32);

impl Default for Lcg {
    fn default() -> Self {
        Lcg(1)
    }
}

impl Lcg {
    pub fn step(&mut self) -> u32 {
        self.0 = self.0.wrapping_mul(0x10dcd);
        self.0
    }
}

/// A script-driven object: engine body plus script.
pub struct Object {
    pub id: ObjId,
    pub sys: Sysobj,
    pub acts: act::Manager,
    pub script: ScriptSpace,
    /// Callbacks traps queued for the interpreter to run (see [`Host::take_callbacks`]).
    pub pending: Vec<Callback>,
    /// The kn::Friend controller of a summon (FUN_1401b06e0).
    pub friend: Option<Box<crate::friend::Friend>>,
    /// Detached threads of this object's program (see [`SideThread`]).
    pub threads: Vec<SideThread>,
    /// It belongs to another world's host and is in this one's list only for
    /// searches and handle lookups: nothing here runs it.
    pub lent: bool,
    /// FUN_1403b44d0 has not run yet: OBJ_INIT waits for the first update.
    pub init_pending: bool,
}

/// A detached thread of an object's own program that shares its work
/// area: `magic_start_thread` (FUN_1403c5ec0 links it into the list the
/// task FUN_1403c5520/FUN_1403c55f0 steps once per frame) or `obj_hook`
/// (FUN_1403d76d0 stores it in the hooked object's +0xd48, whose update
/// FUN_1403d7280 steps it). A thread whose code returns is unlinked.
pub struct SideThread {
    /// The handle the trap returned.
    pub handle: u32,
    pub thread: Thread,
    /// The object it is hooked to (`obj_hook`), which holds the handle in
    /// its [`crate::sysobj::Sysobj::hook`]; `None` for a magic thread.
    pub hooked: Option<ObjId>,
}

impl Object {
    /// An object with an empty script (tests and tools).
    pub fn bare(id: ObjId) -> Self {
        Self {
            id,
            sys: Sysobj::bare(),
            acts: act::Manager::default(),
            pending: Vec::new(),
            friend: None,
            threads: Vec::new(),
            lent: false,
            init_pending: false,
            script: ScriptSpace {
                instance: Instance::placeholder(),
                stack_bytes: 0,
                temp_bytes: 0,
                main: None,
                main_ended: false,
                act_thread: None,
                act_generation: 0,
                faults: Vec::new(),
            },
        }
    }
}

/// The object's script instance and its threads.
pub struct ScriptSpace {
    pub instance: Instance,
    pub stack_bytes: u32,
    pub temp_bytes: u32,
    /// The persistent main thread (OBJ_MAIN) and whether it has ended.
    pub main: Option<Thread>,
    pub main_ended: bool,
    /// The running act thread.
    pub act_thread: Option<Thread>,
    /// Counts act thread replacements, so a run can tell its thread was
    /// discarded while it executed.
    pub act_generation: u32,
    /// Faults the interpreter reported (`pc` of the thread, fault), newest last.
    pub faults: Vec<(i32, Fault)>,
}

/// Side effects the presentation layer or other systems must carry out.
#[derive(Clone, Debug, PartialEq)]
pub enum Event {
    EffectStart {
        object: ObjId,
        effect: i32,
        arguments: [i32; 2],
        handle: u32,
        /// Where the script placed it (`obj_effect_start_pos`); otherwise it
        /// follows the object.
        position: Option<[f32; 3]>,
        /// Bound to the object (`obj_effect_start_bind`).
        bound: bool,
        /// The object it is bound to when that is not the caller.
        other: Option<ObjId>,
    },
    /// A presentation request this build does not play (lights, screen fades,
    /// camera shots, command overrides): the trap's name and its operand words.
    Presentation {
        name: &'static str,
        words: [u32; 3],
    },
    /// A call into a subsystem this crate does not own (the camera, the
    /// screen, the mission HUD, sound, the party): the trap's name with its
    /// operands decoded as far as the executable does before it hands them
    /// on. Objects are resolved; `words` are the trap's argument words in
    /// order (floats as their bits); `vector` is the vector operand a trap
    /// takes, in the engine's +Y-up frame.
    Service {
        name: &'static str,
        object: Option<ObjId>,
        other: Option<ObjId>,
        words: [u32; 8],
        vector: Option<[f32; 4]>,
    },
    ScatterPrize {
        object: ObjId,
        count: i32,
    },
    /// A command on an effect handle: the first operand slots, and the vector
    /// the first one points at when it is readable.
    EffectCommand {
        /// The table 0 trap index (38 position, 41 direction, 51 kill ...).
        trap: u16,
        handle: u32,
        words: [u32; 2],
        vector: Option<[f32; 3]>,
    },
    /// A motion event whose engine routine another subsystem owns (sound,
    /// effects, camera, physics modes).
    MotionEvent {
        object: ObjId,
        range: bool,
        kind: u8,
        params: Vec<u16>,
    },
    /// Frame event 1: an effect or sound with this id started.
    MotionEffect {
        object: ObjId,
        id: u16,
        handle: u32,
    },
    EffectDeadBlock {
        object: ObjId,
        handle: u32,
    },
    /// `obj_hook` (FUN_14042ca20): an action thread of the program bound to
    /// an object, which the host runs (`magic_start_thread`'s threads are on
    /// [`World::magic`]).
    MagicStart {
        object: ObjId,
        id: i32,
        arguments: [i32; 4],
        handle: u32,
        /// Started by `obj_hook`: bound to the object, which holds the
        /// thread and stops it when it starts another or calls
        /// `obj_hook_stop`. Then `other` is the second object it names.
        hook: bool,
        other: Option<ObjId>,
    },
    /// A magic thread the object held ended (`obj_hook_stop`, or a new hook
    /// replaced it).
    MagicStop {
        handle: u32,
    },
    /// `attack_strike`: a script applied an attack to a target directly.
    AttackStrike {
        /// The attack's handle and its slot in [`World::attacks`].
        attack: u32,
        slot: usize,
        /// The object struck (Sora is [`PLAYER_OBJECT`]); `None` when the
        /// script's AI object named nothing.
        target: Option<ObjId>,
        arguments: [i32; 2],
        /// The attack as it struck (a script may free it in the same tick):
        /// its ATKP id and sub index, serial and owner.
        param: [i32; 2],
        serial: u64,
        owner: Option<ObjId>,
        /// Where the strike comes from when its maker names a point (a
        /// Stitch shot's view origin, FUN_1401b8250), for its direction.
        from: Option<[f32; 3]>,
    },
    /// `obj_dead`: the object finished dying and is removed.
    ObjectDead {
        object: ObjId,
    },
    /// `obj_play_se`: the object plays a sound effect where it is.
    Sound {
        object: ObjId,
        id: i32,
        position: [f32; 3],
    },
    /// `obj_motion_capture`: `victim` plays motion `motion` of `captor`'s
    /// model, blending in over `blend` ticks.
    MotionCapture {
        captor: ObjId,
        victim: ObjId,
        motion: i32,
        blend: f32,
    },
    /// `obj_voice`: the object plays a voice line.
    Voice {
        object: ObjId,
        id: i32,
        arg: i32,
    },
    /// FUN_1403aabe0: an event signal posted to the mission and map scripts
    /// (trigger 10 with `code` and `argument`) and the area's signal table.
    /// Code 4 is a boss's death with its entry id (FUN_1403f8c60).
    Signal {
        code: i32,
        argument: i32,
    },
    /// A service the executable provides that this build does not translate yet.
    Unsupported(&'static str),
}

/// A battle sheet (`btlobj_set_sheet`, FUN_140411350 -> FUN_1403c0280 ->
/// FUN_1403c2240): an ENMP row's HP pools and damage terms at the battle's
/// level.
#[derive(Clone, Debug, PartialEq)]
pub struct Sheet {
    pub stats: player::combat::TargetStats,
    /// Pools 1.. (pool 0 is `stats.max_hp`).
    pub pools: Vec<crate::sysobj::Pool>,
    pub attacker: player::combat::AttackerStats,
}

/// The host side of `btlobj_set_sheet`: it holds the ENMP and LVPM tables
/// and the area's battle level this crate does not.
pub trait SheetSource: Send + Sync {
    /// The sheet of ENMP row `id` (the executable falls back to row 69 when
    /// the id is absent, FUN_1403ebf30).
    fn sheet(&self, id: i32) -> Option<Sheet>;
}

/// A shout waiting for the frame's shout job (`act_shout`).
#[derive(Clone, Debug, PartialEq)]
pub struct Shout {
    pub name: Vec<u8>,
    pub shouter: ObjId,
    /// 0 or less: delivered at once; otherwise each receiver waits a random
    /// share of this many ticks.
    pub delay: f32,
}

/// DAT_142a23d50 holds 64 shouts.
pub const SHOUT_QUEUE: usize = 64;

/// The `teamwork_alloc` blocks (DAT_142ae75d0, FUN_1403f6df0): zeroed blocks by
/// key, the same for every object of the engine's list.
#[derive(Debug, Default)]
pub struct Teamwork {
    pub keys: HashMap<i32, u32>,
    pub words: Vec<u32>,
}

/// A light a script made (`light_create`, FUN_1403f6f80): a 0x28-byte block
/// of the executable's light pool. The host draws what these say.
#[derive(Clone, Debug, PartialEq)]
pub struct Light {
    /// +0: the light's kind (the argument of `light_create`).
    pub kind: i32,
    /// +8: the fade, starting at full (1.0).
    pub fade: crate::sysobj::Fade,
    /// +0x10: flag bits (`light_set_flag`).
    pub flags: Vec<u32>,
    /// +0x14..+0x17: r, g, b and a (0x80 after `light_set_color`).
    pub color: [u8; 4],
    /// +0x18..+0x24: fog near, far, min and max.
    pub fog: [f32; 4],
}

/// FUN_1403f7040's tint: each kind-0 light with flag bit 1 scales the
/// scene colour (0x80 = 1.0) toward its own by its fade, channel by
/// channel (`c * (byte((own - 0x80) * fade) + 0x80) >> 7`); the result goes
/// to FUN_1401626f0, which multiplies the map's light and fog colours and
/// the map effects by it. [U] A faded-out light is freed there
/// (FUN_1403f7580); here it only stops tinting.
pub fn scene_tint(lights: &[Option<Light>]) -> [u8; 3] {
    let mut c = [0x80u32; 3];
    for l in lights.iter().flatten() {
        let on = l.flags.first().is_some_and(|w| w & 2 != 0);
        if l.kind != 0 || !on || l.fade.value == 0. {
            continue;
        }
        for (ch, &own) in c.iter_mut().zip(&l.color) {
            let own = ((f32::from(own) - 128.) * l.fade.value) as i32 as u8;
            *ch = ((*ch & 0xff) * u32::from(own.wrapping_add(0x80))) >> 7;
        }
    }
    c.map(|v| (v & 0xff) as u8)
}

/// Light handles are 16 bytes apart from here.
pub const LIGHT_HANDLES: u32 = script::EXTERN_SEGMENT + 0x0800_0000;

/// A vacuum a script made (`vacuum_create`, FUN_1403f8f20): a pull toward a
/// point. The host applies the pull; scripts shape it.
#[derive(Clone, Debug, PartialEq)]
pub struct Vacuum {
    /// +0: the point it pulls toward.
    pub position: [f32; 4],
    /// +0x10: flag bits (`vacuum_set_flag`).
    pub flags: u32,
    /// +0x14: the effective range (`vacuum_set_effective_range`).
    pub effective_range: f32,
    /// +0x18..+0x20: three values the constructor fills with its argument
    /// and `vacuum_set` 2:81 replaces.
    pub at_18: [f32; 3],
    /// +0x24, +0x28 (100.0 at creation) and +0x2c (0.01 at creation), set by
    /// 2:82 to 2:84.
    pub at_24: f32,
    pub at_28: f32,
    pub at_2c: f32,
}

/// Vacuum handles are 16 bytes apart from here.
pub const VACUUM_HANDLES: u32 = script::EXTERN_SEGMENT + 0x0900_0000;

/// FUN_1403f9130 (summed into an object's movement by FUN_1403b89a0): the
/// pull of every vacuum on an object at `position` of objentry type
/// `object_type` (0 for the leader) and weight `weight`, for one tick of
/// `frame_time`. A vacuum whose flags name the type, or that is out of its
/// effective range (0: everywhere), skips it; the strength is the weight
/// over 100 times +0x18 clamped to [+0x1c, +0x20], scaled by the distance
/// past +0x28 times +0x2c clamped to [-1, 1]; +0x24 adds a sideways swirl.
/// [I] The swirl turns the direction a quarter turn about Y (FUN_1403b7680
/// with DAT_140623ab0); no installed fight checked uses it.
pub fn vacuum_pull(
    vacuums: &[Option<Vacuum>],
    position: [f32; 3],
    object_type: u32,
    weight: f32,
    frame_time: f32,
) -> [f32; 3] {
    let mut acc = [0f32; 3];
    for v in vacuums.iter().flatten() {
        if object_type < 32 && v.flags & 1 << object_type != 0 {
            continue;
        }
        let d = [v.position[0] - position[0], 0., v.position[2] - position[2]];
        let len = d[0].hypot(d[2]);
        if v.effective_range != 0. && len >= v.effective_range {
            continue;
        }
        let e = if len > 0. {
            [d[0] / len, 0., d[2] / len]
        } else {
            [0.; 3]
        };
        let [s, lo, hi] = v.at_18;
        let raw = weight / 100. * s;
        let k1 = if lo <= raw {
            if raw <= hi { raw } else { hi }
        } else {
            lo
        };
        let k2 = ((len - v.at_28) * v.at_2c).clamp(-1., 1.);
        for k in 0..3 {
            acc[k] += e[k] * k1 * k2;
        }
        if v.at_24 != 0. {
            let side = [e[2], 0., -e[0]];
            for k in 0..3 {
                acc[k] += side[k] * v.at_24;
            }
        }
    }
    acc.map(|c| c * frame_time)
}

/// Bits of the executable's global status word (DAT_142a11400) that scripts
/// set and read.
pub mod status {
    /// `status_secure_mode_start` / `_end`.
    pub const SECURE_MODE: u32 = 1 << 1;
    /// `status_close_pete_curtain` / `_open_pete_curtain`.
    pub const PETE_CURTAIN: u32 = 1 << 20;
    /// `status_no_leave`: enemies may not leave.
    pub const NO_LEAVE: u32 = 1 << 2;
    /// `status_prize_drain_start` / `_end`.
    pub const PRIZE_DRAIN: u32 = 1 << 3;
    /// `status_force_leave_start` / `_end`: every enemy leaves.
    pub const FORCE_LEAVE: u32 = 1 << 7;
    /// `status_peterpan_prize_drain_start` / `_end`.
    pub const PETERPAN_PRIZE_DRAIN: u32 = 1 << 11;
    /// `status_hide_shadow` / `status_show_shadow`.
    pub const HIDE_SHADOW: u32 = 1 << 12;
}

/// An object search a script made (kept while `World::trace` is on).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SearchRecord {
    /// `entry`, `serial`, `part` or `unit_arg`.
    pub kind: &'static str,
    pub value: i32,
    /// The unit argument index (`unit_arg` only).
    pub index: i32,
    pub found: bool,
}

/// A request to make an object of an entry appear (`sysobj_appear`,
/// FUN_1403df930).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct AppearRequest {
    /// The object entry id (the script's word, flag bits included).
    pub entry: u32,
    /// The object whose script asked.
    pub by: ObjId,
}

/// An object the host built for an [`AppearRequest`].
pub struct Appeared {
    /// The object script (BAR type 3 payload); `None` builds a bare object.
    pub payload: Option<Vec<u8>>,
    pub sys: Sysobj,
}

/// The host side of `sysobj_appear`: it owns the object table (entry ids,
/// models, scripts) this crate does not hold.
pub trait Spawner: Send + Sync {
    fn appear(&mut self, request: &AppearRequest) -> Option<Appeared>;
}

/// Everything shared between objects.
pub struct World {
    /// `DAT_140717480`: 1.0 at 60 fps and normal speed.
    pub frame_time: f32,
    pub rng: Lcg,
    pub objects: Vec<Option<Box<Object>>>,
    /// Static result cells traps hand scripts addresses of (the executable's
    /// per-trap `static` buffers); allocated on first use.
    statics: HashMap<(u16, u16, u16), u32>,
    static_words: Vec<u32>,
    /// Traps a script called that nothing implements yet.
    pub missing: Vec<(u16, u16)>,
    /// Entries `enemy_summon_entry` drew: objects of these the script makes
    /// are fighters of their own, which the host takes over.
    pub summoned: Vec<u32>,
    /// Sora's attackers (DAT_140ad6a30, at most 8, newest last), pushed by
    /// FUN_1401b56e0 mode 0 when he is hit and taken by the friend traps.
    pub player_attackers: Vec<ObjId>,
    /// Sora's attack notification (FUN_1401b56e0 mode 1).
    pub player_notify: crate::friend::PlayerNotify,
    /// Every trap called, newest last, when `trace` is on (development aid).
    pub trace: Option<Vec<(u16, u16)>>,
    /// The object searches scripts made, when `trace` is on.
    pub searches: Vec<SearchRecord>,
    /// The act names scripts started or queued, when tracing.
    pub act_log: Option<Vec<String>>,
    pub events: Vec<Event>,
    /// Keep running when a script calls a trap with no translation (the call
    /// returns zero and is listed in `missing`). Development aid.
    pub tolerate_missing: bool,
    next_effect: u32,
    /// Copies of target records (`trap_target_dup`), by slot.
    pub target_dups: Vec<Option<crate::sysobj::Target>>,
    /// `teamwork_alloc` blocks by key; a host running several worlds as one
    /// list gives them all the same store.
    pub teamwork: Arc<Mutex<Teamwork>>,
    next_magic: u32,
    /// The global control-off bits (DAT_142a171e8) and the hidden map parts.
    pub control_off: Vec<u32>,
    pub bg_hidden: u64,
    /// The stand-in traps reach when a script names Sora (see [`PLAYER_OBJECT`]).
    pub player_proxy: Option<Box<Object>>,
    /// Objects that are in the engine's list but held outside `objects`
    /// for now: the one the host runs itself while the others tick, and the
    /// object whose `sysobj_appear` is running. Searches and handle lookups
    /// see them.
    pub held: Vec<Box<Object>>,
    /// The attack objects scripts created (`attack_new`), by slot.
    pub attacks: Vec<Option<crate::attack::Attack>>,
    /// `attack_new_combo_group`'s counter (DAT_142a24afc).
    pub combo_group: u32,
    /// Attack objects created so far (`Attack::serial`).
    pub attack_serial: u64,
    /// `status_set_lockon_ratio`: the lock-on ratio scripts last set.
    pub lockon_ratio: f32,
    /// How `sysobj_appear` makes objects (none: the script gets a null object).
    pub spawner: Option<Box<dyn Spawner>>,
    appear_depth: u32,
    /// The lights scripts made, by handle slot.
    pub lights: Vec<Option<Light>>,
    /// The vacuums scripts made, by handle slot.
    pub vacuums: Vec<Option<Vacuum>>,
    /// Duck Flare's flare pool (`flare_init`, DAT_142b59670).
    pub flares: Option<crate::traps::flare::Pool>,
    /// Trinity Limit's shot pool (`trinity_shot_init`, DAT_142b596d0).
    pub shots: Option<crate::traps::trinity::Pool>,
    /// Static blocks of shot records a dropped pool left, to reuse.
    pub shots_freed: Vec<u32>,
    /// The saved flags (DAT_1409abcb0) and menu flags (DAT_1409abc40) scripts
    /// set: the room keeps them for the session.
    pub save_flags: Vec<u32>,
    pub menu_flags: Vec<u32>,
    /// The spawn serials `obj_history_mark` listed (DAT_142ae6690).
    pub history_marks: std::collections::BTreeSet<u16>,
    /// The shouts waiting for the frame's shout job.
    pub shouts: Vec<Shout>,
    /// Shouts of the other worlds of the host's list, for the same job.
    pub heard: Vec<Shout>,
    /// This world's shouts the job handed on, for the host to pass to the
    /// other worlds of its list.
    pub sent: Vec<Shout>,
    /// The host runs several worlds as one engine list: an object not lent to
    /// this one at the moment is elsewhere, not a bad handle.
    pub in_list: bool,
    /// What a handle to an object not here reaches (see [`World::in_list`]).
    pub absent: Option<Box<Object>>,
    /// Makes the id of each object `sysobj_appear` builds, for a host whose
    /// worlds share a list; `None` numbers them in this world.
    pub make_id: Option<fn() -> ObjId>,
    /// The engine's own random generator (DAT_140783ca0), which the shout
    /// delay draws from; the scripts' generator is [`World::rng`].
    pub engine_rng: Lcg,
    /// The hit mark's two vectors (DAT_14074c0d8 / DAT_14074c0e8), engine
    /// frame: where the hit spark goes, and a second point.
    pub hit_mark: [[f32; 4]; 2],
    /// DAT_142a11400 bits scripts set (see [`status`]).
    pub status: u32,
    /// The mission timer's pause bit (DAT_140abb854 bit 1).
    pub mission_timer_paused: bool,
    /// Enemy units (spawn groups) scripts disabled (`unit_disable`).
    pub units_disabled: std::collections::BTreeSet<u32>,
    /// Captures made so far (`obj_motion_capture` hands out the count).
    pub captures: i32,
    /// The C library's `rand()` state (the route start position and direction
    /// draw from it), seeded 1 as the C runtime does.
    pub crt_seed: u32,
    /// The event path walker (DAT_142b59840), see [`Eh22`].
    pub eh22: Eh22,
    /// The lap counter of the random map position search (DAT_142ae5d78).
    pub ring_counter: u32,
    /// The battle target table (DAT_142a16370).
    pub battle_targets:
        [crate::traps::target::BattleTarget; crate::traps::target::BATTLE_TARGET_SLOTS],
    /// The running limit (DAT_142a24cc0), when the host started one.
    pub limit: Option<crate::traps::limit::LimitState>,
    /// The host drives Sora's stand-in itself (a limit scene moves him):
    /// traps then read its own position and heading instead of the host's
    /// player.
    pub player_scripted: bool,
    /// The limit hover's spec (DAT_142b59470, `obj_limit_hover_set_spec`):
    /// walk, run, turn, steering inertia, fast and slow inertia, the speed
    /// between them, hover height and climb speed.
    pub hover_spec: [f32; 9],
    /// The `patn` table of 00battle.bin (DAT_142ae59d0): reaction overrides
    /// by state pattern bit (see [`crate::damage::patn_override`]).
    pub patn: Vec<[u8; 32]>,
    /// How `btlobj_set_sheet` builds a sheet (none: the number is kept and
    /// the stats stay as the host made them).
    pub sheets: Option<Box<dyn SheetSource>>,
}

/// The event path walker the `eh22_path_*` traps drive (DAT_142b59840):
/// where it is on the event's list of points, where it goes next, and
/// whether a move is running (1 forward, 2 backward). The host ends a move
/// by clearing `moving` once the camera or actor has arrived.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Eh22 {
    /// +0x110 and +0x118: the current point and the next one.
    pub current: i32,
    pub next: i32,
    /// +0x120: 0 idle, 1 moving forward, 2 moving backward.
    pub moving: i32,
    /// +0x114: the value of the point moved to.
    pub value: f32,
    /// +0x124 and +0x128: what `eh22_path_play` last set.
    pub play_value: u32,
    pub play: bool,
}

/// How many `sysobj_appear` calls may be inside each other's OBJ_INIT.
const MAX_APPEAR_DEPTH: u32 = 4;

impl Default for World {
    fn default() -> Self {
        Self {
            frame_time: 1.0,
            rng: Lcg::default(),
            objects: Vec::new(),
            statics: HashMap::new(),
            static_words: Vec::new(),
            missing: Vec::new(),
            summoned: Vec::new(),
            player_attackers: Vec::new(),
            player_notify: crate::friend::PlayerNotify::default(),
            trace: None,
            searches: Vec::new(),
            act_log: None,
            events: Vec::new(),
            tolerate_missing: false,
            next_effect: 0,
            target_dups: Vec::new(),
            teamwork: Default::default(),
            next_magic: 0,
            control_off: Vec::new(),
            bg_hidden: 0,
            player_proxy: Some(Box::new(Object::bare(PLAYER_OBJECT))),
            held: Vec::new(),
            attacks: Vec::new(),
            combo_group: 0,
            attack_serial: 0,
            lockon_ratio: 0.,
            spawner: None,
            appear_depth: 0,
            lights: Vec::new(),
            vacuums: Vec::new(),
            flares: None,
            shots: None,
            shots_freed: Vec::new(),
            save_flags: Vec::new(),
            menu_flags: Vec::new(),
            history_marks: Default::default(),
            shouts: Vec::new(),
            heard: Vec::new(),
            sent: Vec::new(),
            in_list: false,
            absent: None,
            make_id: None,
            engine_rng: Lcg::default(),
            hit_mark: [[0.; 4]; 2],
            status: 0,
            mission_timer_paused: false,
            units_disabled: Default::default(),
            ring_counter: 0,
            crt_seed: 1,
            captures: 0,
            eh22: Eh22::default(),
            battle_targets: Default::default(),
            limit: None,
            player_scripted: false,
            hover_spec: [0.; 9],
            patn: Vec::new(),
            sheets: None,
        }
    }
}

impl World {
    /// The C library's `rand()` (Microsoft's generator): 15 bits.
    pub fn crt_rand(&mut self) -> i32 {
        self.crt_seed = self.crt_seed.wrapping_mul(214_013).wrapping_add(2_531_011);
        (self.crt_seed >> 16 & 0x7fff) as i32
    }

    /// FUN_1403df930 as the host sees it: build the object the spawner
    /// describes, run its OBJ_INIT and put it in the object list. Returns its
    /// id; `me` is the object whose script asked (never reused).
    pub fn appear(&mut self, me: ObjId, entry: u32) -> Option<ObjId> {
        // An OBJ_INIT that makes objects of its own entry would recurse
        // without end; the executable's object table is finite.
        if self.appear_depth >= MAX_APPEAR_DEPTH {
            return None;
        }
        let mut spawner = self.spawner.take()?;
        let built = spawner.appear(&AppearRequest { entry, by: me });
        self.spawner = Some(spawner);
        let built = built?;
        self.appear_depth += 1;
        let made = self.appear_built(me, entry, built);
        self.appear_depth -= 1;
        made
    }

    fn appear_built(&mut self, me: ObjId, entry: u32, built: Appeared) -> Option<ObjId> {
        let id = match self.make_id {
            Some(make) => make(),
            None => (self.objects.len() as u32).max(me + 1).max(1),
        };
        let mut sys = built.sys;
        sys.entry_id = entry;
        // Its OBJ_INIT waits for its first update: the maker's script goes on
        // first, and may set what that OBJ_INIT reads (B_LK120's unit argument).
        let object = match built.payload {
            Some(payload) => Object::spawn_later(id, &payload, sys).ok()?,
            None => {
                let mut o = Object::bare(id);
                o.sys = sys;
                o
            }
        };
        if self.objects.len() <= id as usize {
            self.objects.resize_with(id as usize + 1, || None);
        }
        self.objects[id as usize] = Some(Box::new(object));
        Some(id)
    }

    /// Run the captured motions down: when the borrowed clip has played its
    /// length the object's motion ends (the end pulse) and the capture is
    /// over, so a script waiting on `obj_motion_capture_id` goes on. `me` is
    /// the object the host runs itself, outside the list; Sora's stand-in is
    /// stepped too.
    pub fn tick_captures(&mut self, me: Option<&mut Object>) {
        fn step(object: &mut Object) {
            let sys = &mut object.sys;
            let Some(capture) = sys.capture.as_mut() else {
                return;
            };
            if capture.remaining > 0 {
                capture.remaining -= 1;
            }
            if capture.remaining == 0 {
                // A looping clip plays on from its loop start; a queued
                // capture (FUN_1403c7140) follows the one that ended.
                if let Some(ls) = capture.loop_start.filter(|&l| l < capture.length) {
                    capture.remaining = capture.length - ls;
                    return;
                }
                if let Some((captor, motion, length, loop_start)) = capture.next.take() {
                    capture.captor = captor;
                    capture.motion = motion;
                    capture.remaining = length;
                    capture.length = length;
                    capture.loop_start = loop_start;
                    return;
                }
                sys.capture = None;
                sys.motion.flags.word &= !(1 << 14);
                sys.motion.flags.word |= crate::motion::flag::ENDED;
            }
        }
        for object in self.objects.iter_mut().flatten() {
            if !object.lent {
                step(object);
            }
        }
        if let Some(object) = me {
            step(object);
        }
        if let Some(proxy) = self.player_proxy.as_deref_mut() {
            step(proxy);
        }
    }

    /// Put every carried object on its holder (physics mode 3): at the
    /// holder's bone when the host knows it, else the holder's origin. `me` is
    /// the object the host runs itself, outside the list.
    pub fn follow_attachments(&mut self, env: &dyn Env, me: Option<&mut Object>) {
        let place = |holder: ObjId, bone: i32, world: &World, own: Option<&Object>| {
            if let Some(p) = env.attach_point(holder, bone) {
                return Some(p);
            }
            if holder == PLAYER_OBJECT {
                return Some(env.player_position());
            }
            world
                .objects
                .get(holder as usize)
                .and_then(|o| o.as_deref())
                .or(own.filter(|o| o.id == holder))
                .map(|o| o.sys.position)
        };
        // Carried objects of the list (the holder may be `me`).
        for id in 0..self.objects.len() {
            let Some(a) = self.objects[id]
                .as_ref()
                .filter(|o| !o.lent)
                .and_then(|o| o.sys.attached)
            else {
                continue;
            };
            if let Some(p) = place(a.to, a.bone, self, me.as_deref())
                && let Some(o) = self.objects[id].as_mut()
            {
                o.sys.position = p;
            }
        }
        if let Some(object) = me
            && let Some(a) = object.sys.attached
            && let Some(p) = place(a.to, a.bone, self, None)
        {
            object.sys.position = p;
        }
    }

    /// The frame's shout job (FUN_1403c9d40): every queued shout goes to every
    /// object of the list in turn, `me` (the object the host runs itself,
    /// outside the list) included. With a delay each receiver draws a random
    /// share of it from the engine's generator and waits; without one the
    /// receiver's act of that name calls its timeout routine now.
    pub fn deliver_shouts(&mut self, env: &dyn Env, mut me: Option<&mut Object>) {
        let own = std::mem::take(&mut self.shouts);
        if self.in_list {
            self.sent.extend(own.iter().cloned());
        }
        let heard = std::mem::take(&mut self.heard);
        for shout in own.into_iter().chain(heard) {
            let mut ids: Vec<ObjId> = self
                .objects
                .iter()
                .flatten()
                .filter(|o| !o.lent)
                .map(|o| o.id)
                .collect();
            ids.sort_unstable();
            if let Some(object) = me.as_deref_mut() {
                self.shout_to(env, object, &shout);
            }
            for id in ids {
                if let Some(mut object) = self.objects[id as usize].take() {
                    self.shout_to(env, &mut object, &shout);
                    self.objects[id as usize] = Some(object);
                }
            }
        }
    }

    fn shout_to(&mut self, env: &dyn Env, object: &mut Object, shout: &Shout) {
        if object.sys.flags & SEARCH_SKIP_FLAGS != 0 {
            return;
        }
        if shout.delay > 0. {
            // (float)rng * 2^-32 * delay (DAT_1405b4d30).
            let draw = self.engine_rng.step() as f32 * 2.328_306_4e-10 * shout.delay;
            object.acts.pending_shout = Some(crate::act::PendingShout {
                name: shout.name.clone(),
                shouter: object_handle(shout.shouter),
                timer: draw,
            });
        } else {
            object.deliver_shout(self, env, &shout.name, object_handle(shout.shouter));
        }
    }

    /// One frame for every object in the list except `skip` (the one the host
    /// is running itself).
    pub fn tick_objects(&mut self, env: &dyn Env, skip: Option<ObjId>) {
        for id in 0..self.objects.len() {
            if skip == Some(id as ObjId) || self.objects[id].as_ref().is_some_and(|o| o.lent) {
                continue;
            }
            if let Some(mut object) = self.objects[id].take() {
                object.tick(self, env);
                if self.objects.len() <= id {
                    self.objects.resize_with(id + 1, || None);
                }
                if !object.sys.gone {
                    self.objects[id] = Some(object);
                }
            }
        }
    }

    /// FUN_1403c6cc0: `victim` plays slot `motion` of `captor`'s motion set
    /// for as long as the captor's clip lasts (a looping clip on); returns
    /// the token `obj_motion_capture_id` reads back. The caller holds
    /// neither object outside the world, or passes the one it holds.
    pub fn capture_motion(
        &mut self,
        env: &dyn Env,
        captor: (ObjId, u32),
        victim: &mut Sysobj,
        motion: i32,
    ) -> i32 {
        self.captures += 1;
        let token = self.captures;
        let (length, loop_start) = Self::capture_clip(env, captor.1, motion);
        victim.capture = Some(crate::sysobj::Capture {
            captor: captor.0,
            motion,
            token,
            remaining: length,
            length,
            loop_start,
            next: None,
        });
        victim.motion.flags.word &= !crate::motion::flag::ENDED;
        victim.motion.flags.word |= 1 << 14;
        self.events.push(Event::MotionCapture {
            captor: captor.0,
            victim: PLAYER_OBJECT,
            motion,
            blend: 0.,
        });
        token
    }

    /// FUN_1403c7140: queue slot `motion` of the captor's set to play on
    /// `victim` once its current capture ends (at once when none plays).
    pub fn queue_capture(
        &mut self,
        env: &dyn Env,
        captor: (ObjId, u32),
        victim: &mut Sysobj,
        motion: i32,
    ) {
        let (length, loop_start) = Self::capture_clip(env, captor.1, motion);
        match victim.capture.as_mut() {
            Some(c) => c.next = Some((captor.0, motion, length, loop_start)),
            None => {
                self.capture_motion(env, captor, victim, motion);
            }
        }
    }

    /// A captured slot's length and loop start in the captor's clips.
    fn capture_clip(env: &dyn Env, model: u32, motion: i32) -> (u32, Option<u32>) {
        let Ok(slot) = usize::try_from(motion) else {
            return (0, None);
        };
        let clips = env.clips(model);
        let length = clips.length(slot);
        let loop_start = clips.loop_start(slot);
        (
            length,
            (loop_start > 0 && loop_start < length).then_some(loop_start),
        )
    }

    /// `trap_memory_alloc`: a zeroed block in the shared static area.
    pub fn alloc(&mut self, bytes: u32) -> u32 {
        self.take_words(bytes.div_ceil(4).max(1) as usize)
    }

    /// Zeroed words: in a list of several worlds from the shared store, as a
    /// handle one script hands another must mean the same to both.
    fn take_words(&mut self, words: usize) -> u32 {
        if self.in_list
            && let Ok(mut team) = self.teamwork.lock()
            && team.words.len() * 4 + words * 4 < TEAMWORK_SPAN as usize
        {
            let at = team.words.len() as u32 * 4;
            team.words.extend(std::iter::repeat_n(0, words));
            return TEAMWORK_HANDLES + at;
        }
        let at = self.static_words.len() as u32 * 4;
        self.static_words.extend(std::iter::repeat_n(0, words));
        script::EXTERN_SEGMENT + STATIC_BASE + at
    }

    /// A zeroed block of `bytes` in the teamwork store under `key`, new if the
    /// key has none; whether it was new.
    pub fn teamwork_block(&mut self, key: i32, bytes: u32) -> (u32, bool) {
        let Ok(mut team) = self.teamwork.lock() else {
            return (0, false);
        };
        if let Some(&h) = team.keys.get(&key) {
            return (h, false);
        }
        let at = team.words.len() as u32 * 4;
        team.words
            .extend(std::iter::repeat_n(0, bytes.div_ceil(4).max(1) as usize));
        let handle = TEAMWORK_HANDLES + at;
        team.keys.insert(key, handle);
        (handle, true)
    }

    /// An AI object reference for the engine object `handle` (the word at
    /// +4 names it), for arguments the host hands a script.
    pub fn ai_reference(&mut self, handle: u32) -> u32 {
        let at = self.alloc(8);
        self.static_write(at + 4, handle);
        at
    }

    /// The first effect handle not handed out yet: handles from here on were
    /// asked for after this call.
    pub fn effect_watermark(&self) -> u32 {
        EFFECT_HANDLES + (self.next_effect + 1) * 16
    }

    /// A fresh handle for an effect the host will create.
    pub fn next_effect_handle(&mut self) -> u32 {
        self.next_effect += 1;
        EFFECT_HANDLES + self.next_effect * 16
    }

    /// `func_obj_control_off` / `_on`: set or clear control bit `index`.
    pub fn set_control_bit(&mut self, index: u32, off: bool) {
        let w = (index >> 5) as usize;
        if self.control_off.len() <= w {
            self.control_off.resize(w + 1, 0);
        }
        let bit = 1u32 << (index & 31);
        if off {
            self.control_off[w] |= bit;
        } else {
            self.control_off[w] &= !bit;
        }
    }

    /// `trap_bg_hide` / `trap_bg_show`: whether map part `index` shows.
    pub fn set_bg_part(&mut self, index: u32, shown: bool) {
        let bit = 1u64 << (index & 63);
        if shown {
            self.bg_hidden &= !bit;
        } else {
            self.bg_hidden |= bit;
        }
    }

    /// One frame for every attack object (FUN_1404376b0) and every light
    /// (its fade runs). An attack bound to an effect follows it and goes when
    /// the effect ends; a timed shape grows and goes when its time is up.
    pub fn tick_attacks(&mut self, env: &dyn Env) {
        let frame_time = self.frame_time;
        // DAT_142a11400 bit 5 holds the timers.
        let held = self.status & (1 << 5) != 0;
        for slot in self.attacks.iter_mut() {
            let Some(a) = slot.as_mut() else {
                continue;
            };
            if a.effect != 0 {
                if !env.effect_active(a.effect) {
                    a.flags |= crate::attack::flag::FREED;
                    *slot = None;
                    continue;
                }
                if let Some(p) = env.effect_position(a.effect) {
                    a.follow(p, frame_time);
                }
            }
            if !held && a.advance(frame_time) {
                a.flags |= crate::attack::flag::FREED;
                *slot = None;
            }
        }
        for light in self.lights.iter_mut().flatten() {
            light.fade.step(frame_time);
        }
    }

    /// Put an attack object in the first free slot; the slot.
    pub fn add_attack(&mut self, a: crate::attack::Attack) -> usize {
        let i = match self.attacks.iter().position(Option::is_none) {
            Some(i) => i,
            None => {
                self.attacks.push(None);
                self.attacks.len() - 1
            }
        };
        self.attacks[i] = Some(a);
        i
    }

    /// Put a light in the first free slot; its handle.
    pub fn add_light(&mut self, light: Light) -> u32 {
        let slot = self.lights.iter().position(Option::is_none);
        let slot = slot.unwrap_or_else(|| {
            self.lights.push(None);
            self.lights.len() - 1
        });
        self.lights[slot] = Some(light);
        LIGHT_HANDLES + slot as u32 * 16
    }

    /// The vacuum slot a handle names.
    pub fn vacuum_slot(&self, handle: u32) -> Option<usize> {
        let o = handle.checked_sub(VACUUM_HANDLES)?;
        let i = (o / 16) as usize;
        (o % 16 == 0 && self.vacuums.get(i)?.is_some()).then_some(i)
    }

    /// The light slot a handle names.
    pub fn light_slot(&self, handle: u32) -> Option<usize> {
        let o = handle.checked_sub(LIGHT_HANDLES)?;
        let i = (o / 16) as usize;
        (o % 16 == 0 && self.lights.get(i)?.is_some()).then_some(i)
    }

    /// A fresh handle for a magic thread the host will create.
    pub fn next_magic_handle(&mut self) -> u32 {
        self.next_magic += 1;
        MAGIC_HANDLES + self.next_magic * 16
    }

    /// The static cell if some trap already allocated it.
    pub fn static_lookup(&self, table: u16, index: u16, slot: u16) -> Option<u32> {
        self.statics.get(&(table, index, slot)).copied()
    }

    /// The handle of a four-word static cell owned by `(table, index, slot)`;
    /// the first use allocates it zeroed.
    pub fn static_cell(&mut self, table: u16, index: u16, slot: u16) -> u32 {
        if let Some(&handle) = self.statics.get(&(table, index, slot)) {
            return handle;
        }
        let handle = self.take_words(4);
        self.statics.insert((table, index, slot), handle);
        handle
    }

    pub fn static_read(&self, handle: u32) -> Option<u32> {
        if let Some(at) = handle
            .checked_sub(TEAMWORK_HANDLES)
            .filter(|&a| a < TEAMWORK_SPAN)
        {
            let team = self.teamwork.lock().ok()?;
            return team.words.get((at / 4) as usize).copied();
        }
        let at = handle.checked_sub(script::EXTERN_SEGMENT + STATIC_BASE)?;
        self.static_words.get((at / 4) as usize).copied()
    }

    pub fn static_write(&mut self, handle: u32, value: u32) -> bool {
        if let Some(at) = handle
            .checked_sub(TEAMWORK_HANDLES)
            .filter(|&a| a < TEAMWORK_SPAN)
        {
            let Ok(mut team) = self.teamwork.lock() else {
                return false;
            };
            return match team.words.get_mut((at / 4) as usize) {
                Some(w) => {
                    *w = value;
                    true
                }
                None => false,
            };
        }
        let Some(at) = handle.checked_sub(script::EXTERN_SEGMENT + STATIC_BASE) else {
            return false;
        };
        match self.static_words.get_mut((at / 4) as usize) {
            Some(w) => {
                *w = value;
                true
            }
            None => false,
        }
    }
}

/// Engine handles: statics at 0x6000_0000, objects at 0x6100_0000 + id.
const STATIC_BASE: u32 = 0;
pub const OBJECT_HANDLES: u32 = script::EXTERN_SEGMENT + 0x0100_0000;

/// Teamwork blocks, which every world of a host's list reads the same.
const TEAMWORK_HANDLES: u32 = script::EXTERN_SEGMENT + 0x00e0_0000;
const TEAMWORK_SPAN: u32 = 0x0010_0000;

/// Sora's handle (the player is not one of the script-driven objects).
pub const PLAYER_HANDLE: u32 = script::EXTERN_SEGMENT + 0x00f0_0000;
/// Effect handles scripts keep, and copies of target records.
pub const EFFECT_HANDLES: u32 = script::EXTERN_SEGMENT + 0x0200_0000;
pub const ATTACK_HANDLES: u32 = script::EXTERN_SEGMENT + 0x0700_0000;
pub const MAGIC_HANDLES: u32 = script::EXTERN_SEGMENT + 0x0500_0000;
pub const TARGET_DUP_HANDLES: u32 = script::EXTERN_SEGMENT + 0x0300_0000;
/// Sora's lock-on target record (`trap_obj_lockon_target`: the record at +4 of
/// the player's lock-on block, FUN_14042b9c0).
pub const LOCKON_RECORD: u32 = TARGET_DUP_HANDLES + 0x00f0_0000;
/// The engine object's target record (sys+0xbf8) is this far into its handle range.
pub const TARGET_RECORD_OFFSET: u32 = 0x800;

/// FUN_1403baa80: objects with any of these +0x120 flag bits are invisible to
/// the engine's object searches.
pub const SEARCH_SKIP_FLAGS: u32 = 0x1008_0000;

/// The entry id of Sora's object (`trap_obj_search_by_entry` answers the
/// player for it without walking the list: FUN_1403b4110).
pub const PLAYER_ENTRY: u32 = 0x236;

/// The id the player proxy answers to. Sora is not a script-driven object, but
/// scripts call the same traps on her as on any object; those reach a stand-in
/// whose position and heading follow the host's player.
pub const PLAYER_OBJECT: ObjId = 0x00ff_ffff;

/// An object field's handle range holds this many object ids.
pub const OBJECT_IDS: ObjId = 0x1000;

pub fn object_handle(id: ObjId) -> u32 {
    if id == PLAYER_OBJECT {
        return PLAYER_HANDLE;
    }
    OBJECT_HANDLES.wrapping_add(id.wrapping_mul(0x1000))
}

/// A word inside an object's handle range: the object and its field offset.
pub fn object_field(address: u32) -> Option<(ObjId, u32)> {
    let o = address.checked_sub(OBJECT_HANDLES)?;
    (o < 0x0100_0000).then_some((o / 0x1000, o % 0x1000))
}

pub fn object_from_handle(handle: u32) -> Option<ObjId> {
    let o = handle.checked_sub(OBJECT_HANDLES)?;
    (o % 0x1000 == 0 && o < 0x0100_0000).then_some(o / 0x1000)
}

/// A word of a target record (sys+0xbf8): +0 and +8 hold the target's engine
/// handle (0 = nobody), +4 is zero and +0xc the search kind.
pub(crate) fn target_record_word(t: crate::sysobj::Target, offset: u32) -> u32 {
    match offset {
        0 | 8 => match t.id {
            0 => 0,
            crate::traps::target::PLAYER_TARGET => PLAYER_HANDLE,
            id if id & crate::traps::target::OBJECT_TARGET != 0 => {
                object_handle(id & !crate::traps::target::OBJECT_TARGET)
            }
            id => object_handle(id),
        },
        4 => t.group as u32,
        0xc => t.kind as u32,
        _ => 0,
    }
}

/// What a trap sees: the calling object and the shared world.
pub struct Ctx<'a> {
    pub world: &'a mut World,
    pub me: &'a mut Object,
    pub env: &'a dyn Env,
}

impl Ctx<'_> {
    /// The object with this id: the calling object or another in the world.
    pub fn object(&mut self, id: ObjId) -> Result<&mut Object, Fault> {
        if id == self.me.id {
            return Ok(self.me);
        }
        if id == PLAYER_OBJECT {
            let proxy = self
                .world
                .player_proxy
                .get_or_insert_with(|| Box::new(Object::bare(PLAYER_OBJECT)));
            if !self.world.player_scripted {
                proxy.sys.position = self.env.player_position();
                proxy.sys.yaw = self.env.player_yaw();
            }
            // Sora's sheet as scripts read it (`sheet_hp`, `sheet_hp_rate`).
            if let Some((hp, max)) = self.env.player_hp() {
                proxy.sys.hp = hp;
                proxy.sys.stats.max_hp = max;
            }
            return Ok(proxy);
        }
        if let Some(k) = self.world.held.iter().position(|o| o.id == id) {
            return Ok(&mut self.world.held[k]);
        }
        let listed = self
            .world
            .objects
            .get(id as usize)
            .is_some_and(Option::is_some);
        if !listed && self.world.in_list && id < OBJECT_IDS {
            let absent = self
                .world
                .absent
                .get_or_insert_with(|| Box::new(Object::bare(id)));
            absent.id = id;
            absent.sys.gone = true;
            self.world
                .events
                .push(Event::Unsupported("object_elsewhere"));
            let absent = self.world.absent.as_deref_mut();
            return absent.ok_or(Fault::Memory(object_handle(id)));
        }
        self.world
            .objects
            .get_mut(id as usize)
            .and_then(|o| o.as_deref_mut())
            .ok_or(Fault::Memory(object_handle(id)))
    }

    /// FUN_1403bf4f0's walk of the engine's object list with a test (the
    /// first object it accepts, in creation order). The walk skips objects
    /// whose +0x120 flags (FUN_1403baa80: `flags & 0x10080000`) are set. The
    /// calling object is part of the list; Sora is not (searches by entry id
    /// answer her separately).
    pub fn search(&self, mut test: impl FnMut(&Object) -> bool) -> Option<ObjId> {
        let mut best: Option<ObjId> = None;
        let mut consider = |o: &Object| {
            if o.sys.flags & SEARCH_SKIP_FLAGS == 0 && best.is_none_or(|b| o.id < b) && test(o) {
                best = Some(o.id);
            }
        };
        consider(self.me);
        for o in self.world.objects.iter().flatten() {
            if o.id != self.me.id {
                consider(o);
            }
        }
        for o in &self.world.held {
            if o.id != self.me.id {
                consider(o);
            }
        }
        best
    }

    /// The next object of the list after `after` (the first when `None`)
    /// that passes `test`, in creation order (FUN_1403bf4f0 steps; the walk
    /// skips the objects FUN_1403baa80 hides).
    pub fn next_object(
        &self,
        after: Option<ObjId>,
        mut test: impl FnMut(&Object) -> bool,
    ) -> Option<ObjId> {
        let mut best: Option<ObjId> = None;
        let mut consider = |o: &Object| {
            if o.sys.flags & SEARCH_SKIP_FLAGS == 0
                && after.is_none_or(|a| o.id > a)
                && best.is_none_or(|b| o.id < b)
                && test(o)
            {
                best = Some(o.id);
            }
        };
        consider(self.me);
        for o in self.world.objects.iter().flatten() {
            if o.id != self.me.id {
                consider(o);
            }
        }
        for o in &self.world.held {
            if o.id != self.me.id {
                consider(o);
            }
        }
        best
    }

    /// Bind the AI object `ai` to what a search found (FUN_1404ad240 of the
    /// engine object into +4; a miss stores 0).
    pub fn bind_ai(
        &mut self,
        memory: &mut Memory<'_>,
        ai: u32,
        found: Option<ObjId>,
    ) -> Result<(), Fault> {
        let handle = found.map_or(0, object_handle);
        if self.write_u32(memory, ai + 4, handle) {
            Ok(())
        } else {
            Err(Fault::Memory(ai))
        }
    }

    /// An engine-object argument (the trap received the handle itself).
    pub fn sys_of(&self, handle: u32) -> Result<ObjId, Fault> {
        if handle == PLAYER_HANDLE {
            return Ok(PLAYER_OBJECT);
        }
        object_from_handle(handle).ok_or(Fault::Memory(handle))
    }

    /// An AI-object argument that may name Sora: the word at +4 is either an
    /// engine object or the player.
    pub fn sys_ref_ai(&self, memory: &Memory<'_>, ai: u32) -> Result<SysRef, Fault> {
        let handle = self.read_u32(memory, ai + 4).ok_or(Fault::Memory(ai))?;
        if handle == PLAYER_HANDLE {
            return Ok(SysRef::Player);
        }
        self.sys_of(handle).map(SysRef::Object)
    }

    /// An AI-object argument: the engine object handle is the word at +4.
    pub fn sys_of_ai(&self, memory: &Memory<'_>, ai: u32) -> Result<ObjId, Fault> {
        let handle = self.read_u32(memory, ai + 4).ok_or(Fault::Memory(ai))?;
        self.sys_of(handle)
    }

    /// Read a 32-bit cell a script handle names: script memory first, then the
    /// static result cells.
    pub fn read_u32(&self, memory: &Memory<'_>, handle: u32) -> Option<u32> {
        memory
            .read_u32(handle)
            .or_else(|| self.world.static_read(handle))
            .or_else(|| self.self_handle_word(handle))
    }

    /// Word 1 of an engine handle used as an AI object names the engine
    /// object, which is the handle itself.
    fn self_handle_word(&self, address: u32) -> Option<u32> {
        let base = address.checked_sub(4)?;
        (base == PLAYER_HANDLE || object_from_handle(base).is_some()).then_some(base)
    }

    pub fn read_f32(&self, memory: &Memory<'_>, handle: u32) -> Option<f32> {
        self.read_u32(memory, handle).map(f32::from_bits)
    }

    pub fn write_u32(&mut self, memory: &mut Memory<'_>, handle: u32, value: u32) -> bool {
        memory.write_u32(handle, value) || self.world.static_write(handle, value)
    }

    pub fn write_f32(&mut self, memory: &mut Memory<'_>, handle: u32, value: f32) -> bool {
        self.write_u32(memory, handle, value.to_bits())
    }
}

/// What a script-held object reference names.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SysRef {
    Player,
    Object(ObjId),
}

pub type TrapFn = fn(&mut Ctx<'_>, &mut Memory<'_>, &mut [Slot; 16]) -> Result<(), Fault>;

/// The script [`Host`] for one object's thread: resolves traps through the
/// registry in [`crate::traps`].
pub struct Machine<'a> {
    pub ctx: Ctx<'a>,
    /// Keep going when a trap has no implementation (development).
    pub tolerate_missing: bool,
}

impl Host for Machine<'_> {
    fn frame_time(&self) -> f32 {
        self.ctx.world.frame_time
    }

    fn take_callbacks(&mut self) -> Vec<Callback> {
        std::mem::take(&mut self.ctx.me.pending)
    }

    fn patch_gate(&self) -> f32 {
        self.ctx
            .world
            .static_lookup(1, 0, 1)
            .and_then(|cell| self.ctx.world.static_read(cell + 12))
            .map_or(0.0, f32::from_bits)
    }

    fn peek(&self, address: u32) -> Option<u32> {
        if let Some(offset) = address.checked_sub(PLAYER_HANDLE).filter(|o| *o < 0x1000) {
            // An engine handle doubles as the AI object the scripts copy
            // around (`memcpy` of the 16-byte struct): word 1 names the engine
            // object, which here is the handle itself.
            if offset < 16 {
                return Some(if offset == 4 { PLAYER_HANDLE } else { 0 });
            }
            let sys = &self.ctx.world.player_proxy.as_ref()?.sys;
            if (TARGET_RECORD_OFFSET..TARGET_RECORD_OFFSET + 16).contains(&offset) {
                return Some(target_record_word(
                    sys.target,
                    offset - TARGET_RECORD_OFFSET,
                ));
            }
            return sys.peek_word(offset);
        }
        if let Some((id, offset)) = object_field(address) {
            if offset < 16 {
                return Some(if offset == 4 { object_handle(id) } else { 0 });
            }
            if id != self.ctx.me.id
                && self.ctx.world.in_list
                && self
                    .ctx
                    .world
                    .objects
                    .get(id as usize)
                    .is_none_or(Option::is_none)
            {
                return Some(0);
            }
            let record = offset.wrapping_sub(crate::damage::DAMAGE_RECORD_OFFSET);
            if record == 0x10 || record == 0x14 {
                // The damage record OBJ_DAMAGE receives (FUN_1403ce740): +0x10
                // the attacker, +0x14 the struck object, as engine handles.
                if record == 0x14 {
                    return Some(object_handle(id));
                }
                let sys = if id == self.ctx.me.id {
                    &self.ctx.me.sys
                } else {
                    &self.ctx.world.objects.get(id as usize)?.as_deref()?.sys
                };
                return Some(match sys.damage_attacker {
                    None => 0,
                    Some(crate::traps::target::PLAYER_TARGET) => PLAYER_HANDLE,
                    Some(a) => object_handle(a),
                });
            }
            if (TARGET_RECORD_OFFSET..TARGET_RECORD_OFFSET + 16).contains(&offset) {
                let sys = if id == self.ctx.me.id {
                    &self.ctx.me.sys
                } else {
                    &self.ctx.world.objects.get(id as usize)?.as_deref()?.sys
                };
                return Some(target_record_word(
                    sys.target,
                    offset - TARGET_RECORD_OFFSET,
                ));
            }
            if id == self.ctx.me.id {
                return self.ctx.me.sys.peek_word(offset);
            }
            return self
                .ctx
                .world
                .objects
                .get(id as usize)?
                .as_deref()?
                .sys
                .peek_word(offset);
        }
        // Trigger 16's third argument is the effect FUN_1402c6b30 made, which
        // B_EX390 copies as an AI object: the word at +4 names an object. [I]
        // Taken as the object running, which started it.
        if let Some(d) = address
            .checked_sub(EFFECT_HANDLES)
            .filter(|d| *d < 0x0100_0000)
        {
            return Some(if d % 16 == 4 {
                object_handle(self.ctx.me.id)
            } else {
                0
            });
        }
        if let Some(offset) = address.checked_sub(LOCKON_RECORD).filter(|o| *o < 16) {
            let t = crate::sysobj::Target {
                id: self.ctx.env.lockon_target().unwrap_or(0),
                kind: -1,
                group: 0,
            };
            return Some(target_record_word(t, offset));
        }
        if let Some(d) = address
            .checked_sub(TARGET_DUP_HANDLES)
            .filter(|d| *d < 0x0100_0000)
        {
            // A copy of a target record (`trap_target_dup`): a slot is 16
            // bytes. A slot nobody holds reads as an empty record.
            let t = self
                .ctx
                .world
                .target_dups
                .get((d / 16) as usize)
                .copied()
                .flatten()
                .unwrap_or_default();
            return Some(target_record_word(t, d % 16));
        }
        self.ctx.world.static_read(address)
    }

    fn poke(&mut self, address: u32, value: u32) -> bool {
        if let Some(d) = address
            .checked_sub(TARGET_DUP_HANDLES)
            .filter(|d| *d < 0x0100_0000)
        {
            // A script writing into its copy of a target record: the group
            // (+4) and the search kind (+0xc).
            let Some(Some(t)) = self.ctx.world.target_dups.get_mut((d / 16) as usize) else {
                return false;
            };
            match d % 16 {
                4 => t.group = value as i32,
                0xc => t.kind = value as i32,
                _ => {}
            }
            return true;
        }
        if let Some(offset) = address.checked_sub(PLAYER_HANDLE).filter(|o| *o < 0x1000) {
            return self
                .ctx
                .world
                .player_proxy
                .as_mut()
                .is_some_and(|p| p.sys.poke_word(offset, value));
        }
        if let Some((id, offset)) = object_field(address) {
            if id == self.ctx.me.id {
                return self.ctx.me.sys.poke_word(offset, value);
            }
            return match self.ctx.world.objects.get_mut(id as usize) {
                Some(Some(o)) => o.sys.poke_word(offset, value),
                _ => self.ctx.world.in_list,
            };
        }
        self.ctx.world.static_write(address, value)
    }

    fn trap_flags(&self, table: u16, index: u16) -> Option<u32> {
        crate::trap_table::lookup(table, index).map(|t| t.flags)
    }

    fn trap(
        &mut self,
        memory: &mut Memory<'_>,
        table: u16,
        index: u16,
        args: &mut [Slot; 16],
    ) -> Result<(), Fault> {
        if let Some(t) = self.ctx.world.trace.as_mut() {
            t.push((table, index));
        }
        match crate::traps::lookup(table, index) {
            Some(f) => f(&mut self.ctx, memory, args),
            None => {
                self.ctx.world.missing.push((table, index));
                if self.tolerate_missing {
                    // Arguments are consumed; a returned value reads as zero.
                    args[0] = Slot::int(0);
                    Ok(())
                } else {
                    Err(Fault::Trap { table, index })
                }
            }
        }
    }
}

/// Run one thread of `obj`'s script to its next yield.
pub fn run_thread(world: &mut World, env: &dyn Env, obj: &mut Object, thread: &mut Thread) -> Exit {
    let mut instance = std::mem::replace(&mut obj.script.instance, Instance::placeholder());
    let world_tolerates = world.tolerate_missing;
    let exit = {
        let mut machine = Machine {
            ctx: Ctx {
                world,
                me: obj,
                env,
            },
            tolerate_missing: world_tolerates,
        };
        script::tick_tree(&mut instance, thread, &mut machine, 1_000_000)
    };
    obj.script.instance = instance;
    if let Exit::Fault(f) = exit {
        obj.script.faults.push((thread.pc, f));
    }
    exit
}

#[cfg(test)]
mod vacuum_tests {
    use super::*;

    fn sephiroths(at: [f32; 4]) -> Vacuum {
        Vacuum {
            position: at,
            flags: 1 << 3,
            effective_range: 2000.,
            at_18: [6.; 3],
            at_24: 0.,
            at_28: 250.,
            at_2c: 0.01,
        }
    }

    #[test]
    fn the_pull_is_full_past_350_none_at_250_and_pushes_inside_it() {
        let v = [Some(sephiroths([0., 0., 1000., 1.]))];
        // Weight 100: k1 = 6 clamped to [6, 6].
        let far = vacuum_pull(&v, [0., 0., 0.], 0, 100., 1.);
        assert!((far[2] - 6.).abs() < 1e-4 && far[0] == 0.);
        let edge = vacuum_pull(&v, [0., 0., 750.], 0, 100., 1.);
        assert!(edge[2].abs() < 1e-4);
        let inside = vacuum_pull(&v, [0., 0., 900.], 0, 100., 1.);
        assert!((inside[2] + 6.).abs() < 1e-4, "pushed away at 100");
        // Out of range, or of an ignored type: nothing.
        assert_eq!(vacuum_pull(&v, [0., 0., -1500.], 0, 100., 1.), [0.; 3]);
        assert_eq!(vacuum_pull(&v, [0., 0., 0.], 3, 100., 1.), [0.; 3]);
    }
}

#[cfg(test)]
mod light_tests {
    use super::*;

    #[test]
    fn a_grey_light_darkens_the_scene_as_it_fades_in() {
        let mut l = Light {
            kind: 0,
            fade: crate::sysobj::Fade::default(),
            flags: vec![2],
            color: [48, 48, 48, 0x80],
            fog: [0.; 4],
        };
        l.fade.value = 1.;
        assert_eq!(scene_tint(&[Some(l.clone())]), [48; 3]);
        l.fade.value = 0.5;
        assert_eq!(scene_tint(&[Some(l.clone())]), [88; 3]);
        // Without flag bit 1 it does nothing.
        l.flags = vec![1];
        assert_eq!(scene_tint(&[Some(l)]), [0x80; 3]);
    }
}
