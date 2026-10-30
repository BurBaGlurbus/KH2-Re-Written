//! Script-driven enemy brain: the enemy's own object script, loaded from the
//! installed game and executed by the translated interpreter in the `enemy`
//! crate, replaces the hand-written decision ports. The motion data, collision
//! world and Sora's position come from the surrounding host.
use super::{RoomWorld, enemy_battle::Clips};
use data::{bar, path_lookup::Library, probe_source::ProbeSource};
use enemy::{
    env::{Clips as EnemyClips, Env},
    sysobj::Sysobj,
    world::{Object, PLAYER_OBJECT, World as EnemyWorld},
};
use player::{World, actor_physics, combat, shadow::ScriptRandom};
use std::sync::Arc;

type Vec3 = [f32; 3];

/// Sora's team number (`sysobj+0x4dc`): attacks with her team in their mask
/// can hit her.
pub(super) const SORA_TEAM: u8 = 1;

/// The host view the engine reads each tick.
struct BrainEnv<'a> {
    room: Option<&'a RoomWorld>,
    clips: ClipView<'a>,
    player: Vec3,
    player_yaw: f32,
    facts: HostFacts,
    /// Each command's (+0x22 chain, +0x1f decider, +0xc flags).
    commands: &'a CommandWindows,
    fx: &'a BrainFx,
}

/// Effect keys of a summon's script effects (above the limits' and the
/// party's).
pub const SUMMON_FX_KEY: u64 = 1 << 56;
/// Effect keys of a battle actor's script effects: this, the actor id in
/// bits 32..56, the handle below.
pub const ACTOR_FX_KEY: u64 = 2 << 56;

/// The script's effect handles as the room plays them. A host that draws
/// them (the summon's, from each object's own PAX) turns `drawn` on; for
/// the others `effect_is_active` answers no, as the engine's default does.
#[derive(Default)]
pub struct BrainFx {
    pub drawn: bool,
    /// What the brain's own object's bound effects follow, and the key range
    /// of its effects in the room's effect state.
    pub anchor: Option<player::magic::Anchor>,
    pub key_base: u64,
    /// Handle -> (effect key, tick asked for).
    handles: std::collections::HashMap<u32, (u64, u64)>,
    /// This brain's running effects after the room's last effect update.
    pub live: std::collections::HashMap<u64, player::magic::FxLive>,
    pub tick: u64,
    /// Handles from here on were asked for during this tick.
    fresh: u32,
    /// The effect calls for the host, each with the object whose script
    /// made it.
    pub calls: Vec<(u32, player::magic::Fx)>,
}

impl BrainFx {
    /// As the limit host's (`party_limit::RoomEnv`): the room's effect
    /// while it runs; one asked for this tick, or one the effect update has
    /// not taken yet, counts as running.
    fn active(&self, handle: u32) -> bool {
        if !self.drawn {
            return false;
        }
        let Some(&(key, started)) = self.handles.get(&handle) else {
            return handle >= self.fresh;
        };
        match self.live.get(&key) {
            Some(l) if l.tracked => l.alive,
            Some(_) => false,
            None => self.tick <= started + 1,
        }
    }
    fn position(&self, handle: u32) -> Option<Vec3> {
        let &(key, _) = self.handles.get(&handle)?;
        self.live.get(&key).filter(|l| l.tracked).map(|l| l.pos)
    }
}

/// The command records the motion-window test reads (FUN_1403b6950).
pub type CommandWindows = std::collections::HashMap<u16, (u16, u8, u32)>;

/// The object's clips, and the lengths of the motions of its set its
/// script captures for Sora (`obj_motion_capture`), which only his kit
/// holds.
struct ClipView<'a> {
    base: &'a Clips,
    captured: &'a std::collections::HashMap<usize, u32>,
}

impl EnemyClips for ClipView<'_> {
    fn length(&self, slot: usize) -> u32 {
        match EnemyClips::length(self.base, slot) {
            0 => self.captured.get(&slot).copied().unwrap_or(0),
            n => n,
        }
    }
    fn loop_start(&self, slot: usize) -> u32 {
        EnemyClips::loop_start(self.base, slot)
    }
    fn root_delta(&self, slot: usize, tick: u32) -> Vec3 {
        EnemyClips::root_delta(self.base, slot, tick)
    }
    fn advance(&self, slot: usize, tick: u32) -> u32 {
        EnemyClips::advance(self.base, slot, tick)
    }
    fn events(&self, slot: usize) -> Option<&enemy::motion::MotionEvents> {
        EnemyClips::events(self.base, slot)
    }
}

/// The clips of an object with no motion set.
struct NoClips;
impl EnemyClips for NoClips {
    fn length(&self, _: usize) -> u32 {
        0
    }
    fn loop_start(&self, _: usize) -> u32 {
        0
    }
    fn root_delta(&self, _: usize, _: u32) -> Vec3 {
        [0.; 3]
    }
    fn events(&self, _: usize) -> Option<&enemy::motion::MotionEvents> {
        None
    }
}

/// What the host tells a script about the game around it each tick.
#[derive(Clone, Copy, Debug, Default)]
pub struct HostFacts {
    /// The battle flag (DAT_142a11404).
    pub battle: bool,
    /// Sora's form or summon time (sheet +0x1b4).
    pub drive_time: f32,
    /// Peter Pan is out with Tinker Bell (FUN_1403de320).
    pub tink: bool,
    /// Sora's HP and maximum HP, and his velocity (+0xa38).
    pub player_hp: Option<(i32, i32)>,
    /// Sora's MP and maximum MP.
    pub player_mp: Option<(i32, i32)>,
    pub player_velocity: Vec3,
    /// The stick's movement request (`player_get_movement`).
    pub stick: Vec3,
    /// The camera's eye and look-at (this frame's coordinates) and the
    /// projection's scale.
    pub camera: Option<(Vec3, Vec3, f32)>,
    /// PRTY record 0x3e, the movement block of Peter Pan's limit flight
    /// (`obj_limit_peterpan_idle`).
    pub flight_spec: Option<player::actor_physics::ObjParams>,
    /// The TARGET radius of the room's enemy (its stand-in, [`ENEMY`]).
    pub enemy_radius: f32,
    /// Sora cannot be damaged now (`obj_is_star`).
    pub player_star: bool,
    /// How far above his feet the centre of Sora's OBJ shape is (his
    /// bone 0x4001).
    pub player_centre: f32,
}

/// The id of the room's enemy as a stand-in in a script's world.
pub const ENEMY: u32 = 1;

impl Env for BrainEnv<'_> {
    fn battle(&self) -> bool {
        self.facts.battle
    }
    fn effect_active(&self, handle: u32) -> bool {
        self.fx.active(handle)
    }
    fn effect_position(&self, handle: u32) -> Option<Vec3> {
        self.fx.position(handle)
    }
    fn drive_time(&self) -> f32 {
        self.facts.drive_time
    }
    fn summon_with_tink(&self) -> bool {
        self.facts.tink
    }
    fn player_hp(&self) -> Option<(i32, i32)> {
        self.facts.player_hp
    }
    fn player_mp(&self) -> Option<(i32, i32)> {
        self.facts.player_mp
    }
    fn player_move_input(&self) -> Vec3 {
        self.facts.stick
    }
    fn camera_view(&self, p: Vec3) -> Option<[f32; 4]> {
        let (eye, look, scale) = self.facts.camera?;
        let cam = bevy::prelude::Transform::from_translation(eye.into())
            .looking_at(look.into(), bevy::prelude::Vec3::Y);
        let v = cam.compute_affine().inverse().transform_point3(p.into());
        (v.z < 0.).then_some([v.x, -v.y, -v.z, scale])
    }
    fn movement_spec(&self, index: i32) -> Option<player::actor_physics::ObjParams> {
        (index == 0x3e).then_some(self.facts.flight_spec).flatten()
    }
    // [I] Sora's own lockout and windows are the room's; through a
    // script he can always take a command (the menu shows it).
    fn can_decide_command(&self, object: u32, _command: u16) -> bool {
        object == PLAYER_OBJECT
    }
    fn command_chain(&self, command: u16) -> u16 {
        self.commands.get(&command).map_or(0, |c| c.0)
    }
    fn command_decider(&self, command: u16) -> u8 {
        self.commands.get(&command).map_or(0, |c| c.1)
    }
    fn command_flags(&self, command: u16) -> u32 {
        self.commands.get(&command).map_or(0, |c| c.2)
    }
    fn player_velocity(&self) -> Vec3 {
        self.facts.player_velocity
    }
    fn floor(&self, x: f32, z: f32, top: f32) -> Option<f32> {
        self.room?.floor(x, z, top)
    }
    /// A game map's COCT through the translated solver; the generated
    /// arena has none.
    fn collide(
        &self,
        body: &enemy::env::MapBody,
        from: Vec3,
        displacement: Vec3,
        airborne: bool,
        normal: Vec3,
    ) -> Option<enemy::env::MapMove> {
        let super::RoomWorld::Map(map) = self.room? else {
            return None;
        };
        let body = ::collision::Body {
            radius: body.radius,
            height: body.height,
            offset: body.offset,
            mode: body.mode,
            class: body.class,
        };
        let (position, no_floor, normal) =
            map.body_move(&body, from, displacement, airborne, normal);
        Some(enemy::env::MapMove {
            position,
            no_floor,
            normal,
        })
    }
    // Objects the script made (`sysobj_appear`) carry their entry as the
    // model; only made enemies have motions of their own here. The word's
    // top bits name a hook table (`Sysobj::set_motion_hook`).
    fn clips(&self, model: u32) -> &dyn EnemyClips {
        let entry = enemy::sysobj::model_entry(model);
        let hook = (model >> enemy::sysobj::MODEL_ENTRY_BITS) as i32 - 1;
        if entry == 0 {
            return match self.clips.base.hook(hook) {
                Some(table) => table,
                None => &self.clips,
            };
        }
        match super::enemy_battle::made_kind(entry) {
            Some(made) => match made.clips.hook(hook) {
                Some(table) => table,
                None => &*made.clips,
            },
            None => &NoClips,
        }
    }
    /// FUN_1403c7c10: the set's slot `hook` becomes the hook table (+0x160).
    /// [I] The object's own set, whichever object is named.
    fn motion_hook(&self, _object: u32, hook: i32) -> bool {
        self.clips.base.hook(hook).is_some()
    }
    fn player_position(&self) -> Vec3 {
        self.player
    }
    fn player_yaw(&self) -> f32 {
        self.player_yaw
    }
    /// Vertical segments only: the floor where it crosses the segment.
    fn line_hit(&self, from: Vec3, to: Vec3) -> Option<Vec3> {
        if from[0] != to[0] || from[2] != to[2] {
            return None;
        }
        let top = from[1].max(to[1]);
        let floor = self.room?.floor(from[0], from[2], top)?;
        (from[1].min(to[1]) <= floor && floor <= top).then_some([from[0], floor, from[2]])
    }
    /// A game map answers any segment for the given class; the generated
    /// arena keeps the vertical floor test.
    fn line_hit_polygon(&self, from: Vec3, to: Vec3, class: u32) -> Option<(Vec3, i32)> {
        match self.room? {
            // FUN_14040c2b0: mask bit 16 takes the attack ray (FUN_140171090).
            // [U] Its polygon is not reported.
            super::RoomWorld::Map(map) if class >> 16 & 1 != 0 => {
                map.attack_hit(from, to).map(|h| (h.point, -1))
            }
            super::RoomWorld::Map(map) => map
                .line(from, to, class)
                .map(|(p, poly)| (p, i32::from(poly))),
            super::RoomWorld::Triangles(_) => self.line_hit(from, to).map(|p| (p, -1)),
        }
    }
    fn map_normal(&self, polygon: i32) -> Option<Vec3> {
        match self.room? {
            super::RoomWorld::Map(map) => map.polygon_normal(u16::try_from(polygon).ok()?),
            super::RoomWorld::Triangles(_) => None,
        }
    }
    fn map_surface(&self, polygon: i32) -> Option<u32> {
        match self.room? {
            super::RoomWorld::Map(map) => map.polygon_surface(u16::try_from(polygon).ok()?),
            super::RoomWorld::Triangles(_) => None,
        }
    }
    fn target_position(&self, _id: u32) -> Option<Vec3> {
        None
    }
    fn target_radius(&self, id: u32) -> Option<f32> {
        (id == ENEMY).then_some(self.facts.enemy_radius)
    }
    // The generated development arena is flat and has no step obstacles.
    fn check_step(&self, _: Vec3, _: Vec3, _: f32, _: f32, _: f32) -> Option<u8> {
        Some(0)
    }
    // The arena keeps enemies in the room.
    fn no_leave(&self) -> bool {
        true
    }
    fn player_invulnerable(&self) -> bool {
        self.facts.player_star
    }
    /// FUN_1403b50f0 for Sora: 0x4000 his origin, 0x4001 the centre of his
    /// first enabled OBJ shape, both with his rotation, in the script's
    /// (engine) frame: the scene's (x, -y, -z) and the engine heading
    /// pi - yaw. [U] His skeleton's own bones are not handed over.
    fn bone_matrix(&self, object: u32, bone: u32) -> Option<[f32; 16]> {
        let lift = match (object, bone) {
            (PLAYER_OBJECT, 0x4000) => 0.,
            (PLAYER_OBJECT, 0x4001) => self.facts.player_centre,
            _ => return None,
        };
        let (s, c) = (std::f32::consts::PI - self.player_yaw).sin_cos();
        let p = self.player;
        Some([
            c,
            0.,
            -s,
            0.,
            0.,
            1.,
            0.,
            0.,
            s,
            0.,
            c,
            0.,
            p[0],
            -(p[1] + lift),
            -p[2],
            1.,
        ])
    }
    fn area_world(&self) -> i32 {
        2
    }
}

/// An attack object a script placed, as the host sees it.
#[derive(Clone, Copy, Debug)]
pub struct ScriptAttack {
    pub slot: usize,
    pub serial: u64,
    pub id: i32,
    pub extra: i32,
    pub position: Vec3,
    pub previous: Option<Vec3>,
    pub radius: f32,
    /// Sora's team (1) is in the attack's team mask.
    pub hits_sora: bool,
    /// A reaction command the attack carries (`attack_set_rc`,
    /// FUN_140437b70: +0x11c the command, +0x120 / +0x124 the sphere's
    /// radius and height, +0x128 the offset from its position).
    pub rc: Option<ScriptRc>,
}

/// An attack-borne reaction command (FUN_140407b40's view of an attack).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ScriptRc {
    pub command: u16,
    pub radius: f32,
    pub height: f32,
    pub offset: Vec3,
    /// +0x118: the object the command belongs to (the attack's owner, or
    /// `attack_set_rc_owner`'s object) is the script's own.
    pub own: bool,
}

/// An effect a script started: element `id` of the enemy's own container.
#[derive(Clone, Copy, Debug)]
pub struct ScriptEffect {
    pub id: u16,
    /// Where it was placed (`obj_effect_start_pos`); `None` follows the enemy.
    pub at: Option<Vec3>,
}

/// See [`ScriptBrain::take_reports`].
#[derive(Debug, Default)]
pub struct ScriptReports {
    pub sounds: Vec<(i32, Vec3)>,
    pub voices: Vec<(i32, i32)>,
    pub services: Vec<&'static str>,
    pub unsupported: Vec<&'static str>,
}

/// The attack objects a world's scripts have placed.
pub fn script_attacks(world: &EnemyWorld) -> Vec<ScriptAttack> {
    use enemy::attack::flag;
    world
        .attacks
        .iter()
        .enumerate()
        .filter_map(|(slot, a)| {
            let a = a.as_ref()?;
            if a.flags & flag::HAS_POSITION == 0 && a.flags & flag::LINE == 0 {
                return None;
            }
            let v = |p: [f32; 4]| [p[0], p[1], p[2]];
            Some(ScriptAttack {
                slot,
                serial: a.serial,
                id: a.param_id,
                extra: a.param_extra,
                position: v(a.position),
                previous: Some(v(a.previous)),
                radius: a.current_radius(),
                hits_sora: a.can_hit_team(SORA_TEAM),
                rc: (a.rc[0] > 0).then(|| ScriptRc {
                    command: a.rc[0] as u16,
                    radius: a.rc[1] as f32,
                    height: a.rc[2] as f32,
                    offset: v(a.reaction_offset),
                    own: a.rc_owner.is_none_or(|o| Some(o) == a.owner),
                }),
            })
        })
        .collect()
}

/// How the engine builds the object around its script (FUN_1403df930):
/// the class from the objentry type, the entry id, and the tables its traps
/// read.
#[derive(Clone, Debug, Default)]
pub struct ScriptSetup {
    pub object_type: u8,
    pub entry_id: u32,
    pub sheets: Option<super::enemy_combat::EnmpSheets>,
    pub patn: Vec<[u8; 32]>,
    /// The spawn record's serial (+0x9f0 -> +0x1e) when an area placed the
    /// object; 0 otherwise.
    pub spawn_serial: u16,
    /// The objentry character id (+0x4c).
    pub character: u16,
    /// The objentry's +0x48 word (bit 3: it flies, `obj_is_entry_fly`).
    pub entry_flags: u16,
    /// Its id in the engine's list ([`next_fighter_id`]); 0 for an object
    /// alone in its world.
    pub object_id: u32,
}

/// Fighters' own objects take ids from 1 and what their scripts make from
/// here, so a handle names one object across their worlds.
const MADE_IDS: u32 = 0x200;

/// The id of a new fighter's object.
pub fn next_fighter_id() -> u32 {
    static NEXT: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);
    NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed) % (MADE_IDS - 1) + 1
}

fn next_made_id() -> u32 {
    static NEXT: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);
    NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed) % (enemy::world::OBJECT_IDS - MADE_IDS)
        + MADE_IDS
}

/// The teamwork blocks all fighters share (DAT_142ae75d0).
pub fn fighters_teamwork() -> Arc<std::sync::Mutex<enemy::world::Teamwork>> {
    static STORE: std::sync::OnceLock<Arc<std::sync::Mutex<enemy::world::Teamwork>>> =
        std::sync::OnceLock::new();
    STORE.get_or_init(Default::default).clone()
}

/// An enemy a script made, as the gallery draws it.
#[derive(Clone, Copy, Debug)]
pub struct MadeFighter {
    pub id: u32,
    pub entry: u32,
    pub position: Vec3,
    pub yaw: f32,
    pub slot: usize,
    pub tick: u32,
    pub blending: bool,
    pub alpha: f32,
    pub hidden: bool,
}

pub struct ScriptBrain {
    setup: ScriptSetup,
    world: EnemyWorld,
    object: Box<Object>,
    /// What stands in `object` while the object is lent to another fighter's
    /// world, and the made objects lent with it.
    stub: Option<Box<Object>>,
    lent_made: Vec<u32>,
    clips: Arc<Clips>,
    /// Lengths of the captured motions (see [`ClipView`]).
    captured: std::collections::HashMap<usize, u32>,
    payload: Vec<u8>,
    spawn: Vec3,
    player: Vec3,
    /// Sora's heading, set by the host before each tick.
    pub player_yaw: f32,
    /// The game state the host passes in before each tick.
    pub facts: HostFacts,
    /// The object's OBJ radius for the wall test (EnemyCombat::wall_radius).
    pub wall_radius: f32,
    /// Faults the interpreter or a missing trap reported, for the host log.
    pub fault: Option<String>,
    /// Effects the script started since the host last took them.
    pub effects: Vec<ScriptEffect>,
    /// Voice lines the script asked for (id, argument), for the host log.
    pub voices: Vec<(i32, i32)>,
    /// Sound effects the script asked for (`obj_play_se`): id and place.
    pub sounds: Vec<(i32, Vec3)>,
    /// Prize rows the script scattered (`obj_scatter_prize`) and where the
    /// scattering object stood.
    pub prizes: Vec<(i32, Vec3)>,
    /// Names of the engine services the script called that are not translated.
    pub unsupported: std::collections::BTreeSet<&'static str>,
    /// Names of the requests the script made of subsystems this host does not
    /// have (camera, screen, mission HUD, party, prizes).
    pub services: std::collections::BTreeSet<&'static str>,
    /// The motion events (range or frame, kind) the engine passed on and
    /// nothing here carries out, for the unattended logs.
    pub motion_events: std::collections::BTreeSet<(bool, u8)>,
    /// What the host has already reported, so each name is reported once.
    reported: std::collections::BTreeSet<&'static str>,
    /// Attack slots the script applied to Sora directly (`attack_strike`),
    /// for the host's hit resolution to take.
    pub strikes: Vec<usize>,
    /// Attack slots the script's objects applied to the enemy's stand-in
    /// directly (Chicken Little's fireworks), with their ATKP ids and serials.
    pub enemy_strikes: Vec<(usize, [i32; 2], u64, Option<Vec3>)>,
    /// Stitch's MP specials not yet carried out (FUN_1403c2040 on Sora).
    pub mp_refills: u32,
    /// Genie's new form number (FUN_1403ddfb0) for the host to keep.
    pub genie_form: Option<u8>,
    /// The summon's limit while it runs (its own script as the program).
    limit: Option<enemy::limit_scene::Scene>,
    /// The command records for the motion-window test.
    commands: CommandWindows,
    /// The commands the script offers this tick (`command_override`'s
    /// records: 0 Attack's row, 1 the reaction, 2 the Limit row) and each
    /// record's target.
    pub offers: [Option<u16>; 8],
    pub offer_targets: [Option<u32>; 8],
    /// The script's effects as the room plays them.
    pub fx: BrainFx,
    /// Event signals the script's objects posted (FUN_1403aabe0: code and
    /// argument), for the host's mission and area scripts.
    pub signals: Vec<(i32, i32)>,
    /// What the script did to Sora: the motions of its set it captured him
    /// into (slot, blend), and the motions it queued behind them.
    pub sora_captures: Vec<(i32, f32)>,
    pub sora_pushed: Vec<(usize, f32)>,
    /// Falls a hook thread started on Sora (`obj_fly_to_jump`), with the
    /// fall and land motions it named; whether his stand-in was falling.
    pub sora_falls: Vec<(i32, i32)>,
    proxy_falling: bool,
    /// HP changes (`obj_add_hp`: delta) and MP charges
    /// (`obj_start_mpdrive`: the factor) the script made on Sora.
    pub sora_hp: Vec<i32>,
    pub sora_mpdrive: Vec<f32>,
    /// Presentation requests for the host: the trap's name, the object it
    /// names and its argument words.
    pub presentations: Vec<(&'static str, Option<u32>, [u32; 8])>,
}

pub(super) fn script_payload(source: &ProbeSource, model: &str) -> Result<Vec<u8>, String> {
    let mut library = Library::open(source).map_err(|e| e.to_string())?;
    let bytes = library
        .read(&format!("obj/{model}.mdlx"))
        .map_err(|e| e.to_string())?;
    let table = bar::read_bar(&bytes).map_err(|e| e.to_string())?;
    let entry = table
        .entries
        .iter()
        .find(|e| e.entry_type == 3 && e.offset >= 0 && e.size != 0)
        .ok_or("the model has no object script")?;
    let start = entry.offset as usize;
    bytes
        .get(start..start + entry.size as usize)
        .map(<[u8]>::to_vec)
        .ok_or_else(|| "object script outside the archive".into())
}

/// The HIT spheres (kind 2, group 0) of a model's collision table (BAR
/// type 23): model-space offset raised by the entry's height offset, and
/// radius. [I] Bone-bound entries are taken at the model origin: the made
/// objects that use them (B_EX360) have no skeleton loaded here.
pub(super) fn hit_spheres(source: &ProbeSource, model: &str) -> Vec<([f32; 3], f32)> {
    let read = || -> Option<Vec<([f32; 3], f32)>> {
        let mut library = Library::open(source).ok()?;
        let bytes = library.read(&format!("obj/{model}.mdlx")).ok()?;
        let table = bar::read_bar(&bytes).ok()?;
        let entry = table
            .entries
            .iter()
            .find(|e| e.entry_type == 23 && e.offset >= 0 && e.size != 0)?;
        let start = entry.offset as usize;
        let collision =
            data::object_collision::read(bytes.get(start..start + entry.size as usize)?).ok()?;
        Some(
            collision
                .iter()
                .filter(|e| e.kind == data::object_collision::HIT && e.group == 0)
                .map(|e| {
                    let [x, y, z] = e.position.map(f32::from);
                    (
                        [x, y + f32::from(e.position_height), z],
                        f32::from(e.radius),
                    )
                })
                .collect(),
        )
    };
    read().unwrap_or_default()
}

/// An object a script made that Sora's attacks can hit: its id and its HIT
/// spheres in the scene.
#[derive(Clone, Debug)]
pub struct MadeTarget {
    pub id: u32,
    pub spheres: Vec<(bevy::math::Vec3, f32)>,
}

impl ScriptBrain {
    #[allow(clippy::too_many_arguments)]
    pub fn load(
        source: &ProbeSource,
        model: &str,
        position: Vec3,
        yaw: f32,
        stats: combat::TargetStats,
        sstm: actor_physics::Sstm,
        clips: Arc<Clips>,
        setup: ScriptSetup,
        spawner: Option<Box<dyn enemy::world::Spawner>>,
        adopted: Option<Box<Object>>,
    ) -> Result<Self, String> {
        let payload = script_payload(source, model)?;
        if setup.object_id != 0 {
            super::enemy_battle::register_fighter_kind(setup.entry_id, &clips);
        }
        let id = setup.object_id;
        let mut brain = Self {
            setup,
            world: EnemyWorld::default(),
            object: Box::new(Object::bare(id)),
            stub: Some(Box::new(Object::bare(id))),
            lent_made: Vec::new(),
            clips,
            payload,
            spawn: position,
            player: position,
            player_yaw: 0.,
            facts: HostFacts::default(),
            wall_radius: 0.,
            fault: None,
            effects: Vec::new(),
            voices: Vec::new(),
            sounds: Vec::new(),
            prizes: Vec::new(),
            unsupported: Default::default(),
            services: Default::default(),
            motion_events: Default::default(),
            reported: Default::default(),
            captured: Default::default(),
            strikes: Vec::new(),
            enemy_strikes: Vec::new(),
            mp_refills: 0,
            genie_form: None,
            limit: None,
            commands: CommandWindows::new(),
            offers: [None; 8],
            offer_targets: [None; 8],
            fx: BrainFx::default(),
            signals: Vec::new(),
            sora_captures: Vec::new(),
            sora_pushed: Vec::new(),
            sora_falls: Vec::new(),
            proxy_falling: false,
            sora_hp: Vec::new(),
            sora_mpdrive: Vec::new(),
            presentations: Vec::new(),
        };
        // The spawner is there for OBJ_INIT's own `sysobj_appear` (M_EX870's).
        brain.world.spawner = spawner;
        match adopted {
            Some(object) => brain.adopt(object, stats, sstm),
            None => brain.build(position, yaw, stats, sstm, &mut ScriptRandom::default())?,
        }
        Ok(brain)
    }

    fn build(
        &mut self,
        position: Vec3,
        yaw: f32,
        stats: combat::TargetStats,
        sstm: actor_physics::Sstm,
        rng: &mut ScriptRandom,
    ) -> Result<(), String> {
        let mut sys = Sysobj::new(0, position, yaw, stats, sstm);
        sys.appear_way = 2;
        sys.set_class(self.setup.object_type);
        self.entry_fields(&mut sys);
        self.fresh_world();
        self.world.rng.0 = rng.0;
        let env = BrainEnv {
            room: None,
            clips: ClipView {
                base: &self.clips,
                captured: &self.captured,
            },
            player: self.player,
            player_yaw: self.player_yaw,
            facts: self.facts,
            commands: &self.commands,
            fx: &self.fx,
        };
        let id = self.setup.object_id;
        self.object = Object::spawn(&mut self.world, &env, id, &self.payload, sys)
            .map(Box::new)
            .map_err(|e| format!("object script rejected: {e:?}"))?;
        rng.0 = self.world.rng.0;
        Ok(())
    }

    /// What the engine's constructor takes from the objentry and the spawn.
    fn entry_fields(&self, sys: &mut Sysobj) {
        sys.turn_rate = actor_physics::ObjParams::defaults(sys.sstm.fall_max).turn;
        sys.entry_id = self.setup.entry_id;
        sys.spawn_serial = self.setup.spawn_serial;
        sys.entry_fly = self.setup.entry_flags >> 3 & 1 != 0;
        // FUN_140411700, which every battle object's constructor starts
        // with (types 3, 4, 12, 13, 21): team 2 (+0x4dc) unless
        // FUN_1403baa60 (character 0x417 to 0x423).
        if matches!(self.setup.object_type, 3 | 4 | 12 | 13 | 21)
            && !(0x417..0x424).contains(&self.setup.character)
        {
            sys.team = 2;
        }
    }

    fn fresh_world(&mut self) {
        let spawner = self.world.spawner.take();
        self.world = EnemyWorld::default();
        self.world.spawner = spawner;
        self.world.tolerate_missing = true;
        if self.setup.object_id != 0 {
            self.world.in_list = true;
            self.world.make_id = Some(next_made_id);
            self.world.teamwork = fighters_teamwork();
        }
        self.world.patn = self.setup.patn.clone();
        self.world.sheets = self
            .setup
            .sheets
            .clone()
            .map(|s| Box::new(s) as Box<dyn enemy::world::SheetSource>);
    }

    /// An object another fighter's script made becomes a fighter of its own
    /// (its script already started, in the maker's world). The engine has one
    /// list; the maker's handle to it still reaches it through
    /// [`Self::lend_objects`].
    fn adopt(
        &mut self,
        mut object: Box<Object>,
        stats: combat::TargetStats,
        sstm: actor_physics::Sstm,
    ) {
        self.fresh_world();
        let sys = &mut object.sys;
        sys.model &= !enemy::sysobj::model_entry(sys.model);
        sys.sstm = sstm;
        // Its script's `btlobj_set_sheet` set these if it ran.
        if sys.sheet == 0 {
            sys.hp = stats.max_hp;
            sys.stats = stats;
        }
        self.entry_fields(&mut object.sys);
        self.object = object;
    }

    /// A made object leaves this script's world.
    pub fn take_made(&mut self, id: u32) -> Option<Box<Object>> {
        self.world.objects.get_mut(id as usize)?.take()
    }

    pub fn tick(&mut self, room: &RoomWorld, player: Vec3, rng: &mut ScriptRandom) {
        self.player = player;
        self.world.rng.0 = rng.0;
        self.offers = [None; 8];
        self.offer_targets = [None; 8];
        self.fx.fresh = self.world.effect_watermark();
        let env = BrainEnv {
            room: Some(room),
            clips: ClipView {
                base: &self.clips,
                captured: &self.captured,
            },
            player,
            player_yaw: self.player_yaw,
            facts: self.facts,
            commands: &self.commands,
            fx: &self.fx,
        };
        if let Some(mut scene) = self.limit.take() {
            // The limit's tick: Sora and the summon take part, the summon's
            // script is the program (its world and object lent to it).
            scene.swap_parts(&mut self.world, &mut self.object);
            let status = scene.tick(&env);
            scene.swap_parts(&mut self.world, &mut self.object);
            if status == enemy::limit_scene::Status::Over {
                enemy::limit_scene::Scene::release(&mut self.world, &mut self.object);
            } else {
                self.limit = Some(scene);
            }
            Self::tick_made(&mut self.world, &mut self.object, &env);
            self.hit_the_map(room);
            self.drain_events();
            // `player_set_exec_rc` (player +0xdd8): the reaction command Sora
            // runs next is the reaction offer.
            if let Some(l) = self.world.limit.as_ref()
                && l.exec_rc != 0
            {
                self.offers[1] = Some(l.exec_rc);
            }
            rng.0 = self.world.rng.0;
            self.record_faults();
            return;
        }
        self.object.tick(&mut self.world, &env);
        Self::keep_out_of_walls(&mut self.object, self.wall_radius, room);
        Self::tick_made(&mut self.world, &mut self.object, &env);
        self.world.tick_attacks(&env);
        self.hit_the_map(room);
        self.drain_events();
        self.take_player_queue();
        if self.object.sys.gone {
            self.world.attacks.clear();
        }
        rng.0 = self.world.rng.0;
        self.record_faults();
    }

    /// The engine has one object list; each fighter runs in a world of its
    /// own. While `to` runs, this fighter's objects stand in its list, which
    /// its searches and handles reach and which nothing there runs. The
    /// object's model is its entry there, so `to`'s environment finds its
    /// motions.
    pub fn lend_objects(&mut self, to: &mut Self) {
        fn put(list: &mut Vec<Option<Box<Object>>>, mut o: Box<Object>) {
            o.lent = true;
            let at = o.id as usize;
            if list.len() <= at {
                list.resize_with(at + 1, || None);
            }
            list[at] = Some(o);
        }
        let Some(stub) = self.stub.take() else {
            return;
        };
        let mut own = std::mem::replace(&mut self.object, stub);
        own.sys.model |= own.sys.entry_id;
        put(&mut to.world.objects, own);
        for slot in &mut self.world.objects {
            if let Some(o) = slot.take() {
                self.lent_made.push(o.id);
                put(&mut to.world.objects, o);
            }
        }
    }

    /// Back from `from`, after its tick.
    pub fn take_back_objects(&mut self, from: &mut Self) {
        let id = self.object.id as usize;
        if let Some(mut own) = from.world.objects.get_mut(id).and_then(Option::take) {
            own.lent = false;
            own.sys.model &= !enemy::sysobj::model_entry(own.sys.model);
            self.stub = Some(std::mem::replace(&mut self.object, own));
        }
        for id in self.lent_made.drain(..) {
            if let Some(mut o) = from
                .world
                .objects
                .get_mut(id as usize)
                .and_then(Option::take)
            {
                o.lent = false;
                self.world.objects[id as usize] = Some(o);
            }
        }
        while from.world.objects.last().is_some_and(Option::is_none) {
            from.world.objects.pop();
        }
    }

    /// Shouts this fighter's objects made, once its own were told.
    pub fn take_sent_shouts(&mut self) -> Vec<enemy::world::Shout> {
        std::mem::take(&mut self.world.sent)
    }

    /// Shouts of the other fighters: its next shout job tells its objects.
    pub fn hear_shouts(&mut self, shouts: &[enemy::world::Shout]) {
        self.world.heard.extend_from_slice(shouts);
    }

    pub fn motion_table(&self) -> Option<i32> {
        self.object.sys.motion.table
    }

    /// The attack objects the script has placed, for the host's hit tests.
    pub fn attacks(&self) -> Vec<ScriptAttack> {
        script_attacks(&self.world)
    }

    /// An attack of the script struck Sora: it records the strike and the
    /// script's hit callback runs (FUN_1403d1730).
    pub fn mark_attack_hit(&mut self, slot: usize) {
        let env = BrainEnv {
            room: None,
            clips: ClipView {
                base: &self.clips,
                captured: &self.captured,
            },
            player: self.player,
            player_yaw: self.player_yaw,
            facts: self.facts,
            commands: &self.commands,
            fx: &self.fx,
        };
        self.object
            .attack_struck(&mut self.world, &env, slot, PLAYER_OBJECT);
        self.record_faults();
    }

    /// The attacks the script's objects applied to the enemy directly.
    pub fn take_enemy_strikes(&mut self) -> Vec<(usize, [i32; 2], u64, Option<Vec3>)> {
        std::mem::take(&mut self.enemy_strikes)
    }

    /// The attacks the script applied to Sora directly, with their parameter
    /// ids (an attack freed since is dropped).
    pub fn take_strikes(&mut self) -> Vec<(usize, i32, i32)> {
        std::mem::take(&mut self.strikes)
            .into_iter()
            .filter_map(|slot| {
                let a = self.world.attacks.get(slot)?.as_ref()?;
                Some((slot, a.param_id, a.param_extra))
            })
            .collect()
    }

    /// Sora's guard turned an attack of the script back: it is marked
    /// reflected and the script's reflect callback runs (FUN_1403d2140).
    pub fn mark_attack_reflected(&mut self, slot: usize) {
        let env = BrainEnv {
            room: None,
            clips: ClipView {
                base: &self.clips,
                captured: &self.captured,
            },
            player: self.player,
            player_yaw: self.player_yaw,
            facts: self.facts,
            commands: &self.commands,
            fx: &self.fx,
        };
        self.object
            .attack_reflected(&mut self.world, &env, slot, PLAYER_OBJECT);
        self.record_faults();
    }

    /// The attacks that touch the map this frame get the wall or floor flag
    /// and the point they touched, the way a script reads them
    /// (`attack_is_hit_wall`, `attack_is_hit_floor`, `attack_get_hit_bg_pos`).
    /// A sphere is tested against the room's floor under it and the walls
    /// around it. [U] The executable's own map test for attack objects was
    /// not located; this uses the room's collision directly.
    fn hit_the_map(&mut self, room: &RoomWorld) {
        use enemy::attack::flag;
        for a in self.world.attacks.iter_mut().flatten() {
            a.flags &= !(flag::HIT_WALL | flag::HIT_FLOOR);
            if a.flags & flag::HAS_POSITION == 0 || a.flags & flag::LINE != 0 {
                continue;
            }
            let r = a.current_radius();
            let [x, y, z, _] = a.position;
            if let Some(floor) = room.floor(x, z, y + r)
                && y - r <= floor
            {
                a.flags |= flag::HIT_FLOOR;
                a.hit_position = [x, floor, z, 1.];
            }
            let pushed = room.resolve_walls([x, z], r, y - r, y + r);
            let (dx, dz) = (pushed[0] - x, pushed[1] - z);
            let len = dx.hypot(dz);
            if len > 1.0e-3 {
                // The wall is a radius away from where the sphere was pushed
                // to, back along the push.
                a.flags |= flag::HIT_WALL;
                a.hit_position = [pushed[0] - dx / len * r, y, pushed[1] - dz / len * r, 1.];
            }
        }
    }

    /// The object's map collision body, from its model (the solver then
    /// keeps it out of the walls).
    pub fn set_map_body(&mut self, body: Option<enemy::env::MapBody>) {
        self.object.sys.map_body = body;
    }

    /// The object's body against the map after its move: its OBJ circle is
    /// pushed out of the room's walls over its body's height. [I] For an
    /// object without a map body only: one with a body goes through the
    /// exe's solver (FUN_1403b9090). Without this a blow or a warp could
    /// carry a bodiless object off the map, where no floor catches it.
    /// Objects it made keep their own moves.
    fn keep_out_of_walls(object: &mut Object, r: f32, room: &RoomWorld) {
        if r <= 0. || object.sys.gone || object.sys.map_body.is_some() {
            return;
        }
        let [x, y, z] = object.sys.position;
        let [px, pz] = room.resolve_walls([x, z], r, y, y + 2. * r);
        object.sys.position = [px, y, pz];
    }

    /// The objects the script made (and the enemy's stand-in). The host's
    /// own object stays reachable to them meanwhile (the made objects of a
    /// summon read and drive their maker).
    fn tick_made(world: &mut EnemyWorld, object: &mut Object, env: &BrainEnv<'_>) {
        let id = object.id;
        let me = std::mem::replace(object, enemy::world::Object::bare(id));
        world.held.push(Box::new(me));
        world.tick_objects(env, None);
        if let Some(k) = world.held.iter().rposition(|o| o.id == id) {
            *object = *world.held.remove(k);
        }
        world.deliver_shouts(env, Some(&mut *object));
        world.follow_attachments(env, Some(&mut *object));
        world.tick_captures(Some(object));
    }

    /// FUN_1403d8b40 -> FUN_1403d7fa0 for the summon's limit `command`:
    /// the limit object with Sora (as `player` stands now) and the summon,
    /// and its script's trigger 22. `time` is the gauge (command +0x20).
    pub fn begin_limit(
        &mut self,
        command: u16,
        time: f32,
        player: enemy::sysobj::Sysobj,
    ) -> Result<(), String> {
        let id = self.object.id;
        let world = std::mem::take(&mut self.world);
        let object = *std::mem::replace(&mut self.object, Box::new(Object::bare(id)));
        match enemy::limit_scene::Scene::summon(world, object, command, time, player) {
            Ok(mut scene) => {
                scene.swap_parts(&mut self.world, &mut self.object);
                self.limit = Some(scene);
                Ok(())
            }
            Err(failed) => {
                let (e, world, object) = *failed;
                self.world = world;
                *self.object = object;
                Err(e)
            }
        }
    }

    /// The lengths of the motions of the object's set its script captures
    /// for Sora.
    pub fn set_captured(&mut self, lengths: std::collections::HashMap<usize, u32>) {
        self.captured = lengths;
    }

    /// The command records the scripts' window tests read.
    pub fn set_commands(&mut self, commands: CommandWindows) {
        self.commands = commands;
    }

    /// Hits that landed this tick count toward the limit's combo (+0xc0).
    pub fn count_limit_hits(&mut self, n: i32) {
        if self.limit.is_some()
            && let Some(l) = self.world.limit.as_mut()
        {
            l.hits += n;
        }
    }

    /// The limit runs.
    pub fn in_limit(&self) -> bool {
        self.limit.is_some()
    }

    /// The limit's state (its gauge time, hits, ending), while it runs.
    pub fn limit_state(&self) -> Option<&enemy::traps::limit::LimitState> {
        self.limit.as_ref().and(self.world.limit.as_ref())
    }

    /// Sora as the limit has him (the scripted stand-in).
    pub fn limit_player(&self) -> Option<&enemy::sysobj::Sysobj> {
        self.limit.as_ref()?;
        self.world.player_proxy.as_deref().map(|p| &p.sys)
    }

    /// The motion the script has Sora capture now (outside a limit too).
    pub fn player_capture(&self) -> Option<enemy::sysobj::Capture> {
        self.world.player_proxy.as_deref()?.sys.capture
    }

    /// FUN_1403b6420 on Sora's stand-in during the limit: his heading.
    pub fn turn_limit_player(&mut self, yaw: f32) {
        if self.limit.is_some()
            && let Some(p) = self.world.player_proxy.as_deref_mut()
        {
            p.sys.yaw = yaw;
        }
    }

    /// A follow-up command Sora took (trigger 23 with the command).
    pub fn limit_follow_up(&mut self, command: u16) {
        let Some(mut scene) = self.limit.take() else {
            return;
        };
        let env = BrainEnv {
            room: None,
            clips: ClipView {
                base: &self.clips,
                captured: &self.captured,
            },
            player: self.player,
            player_yaw: self.player_yaw,
            facts: self.facts,
            commands: &self.commands,
            fx: &self.fx,
        };
        scene.swap_parts(&mut self.world, &mut self.object);
        if let Some(l) = scene.world.limit.as_mut()
            && l.exec_rc == command
        {
            l.exec_rc = 0;
        }
        scene.follow_up(&env, command);
        scene.swap_parts(&mut self.world, &mut self.object);
        self.limit = Some(scene);
        self.drain_events();
        self.record_faults();
    }

    /// Sort the engine's events into what the host plays and what it only
    /// reports; the list never grows past one frame.
    fn drain_events(&mut self) {
        use enemy::world::Event;
        for event in std::mem::take(&mut self.world.events) {
            match event {
                Event::EffectStart {
                    object,
                    effect,
                    position,
                    handle,
                    other,
                    ..
                } => {
                    if let Ok(id) = u16::try_from(effect) {
                        if !self.fx.drawn {
                            self.effects.push(ScriptEffect { id, at: position });
                        } else {
                            let key = self.fx.key_base | u64::from(handle);
                            self.fx.handles.insert(handle, (key, self.fx.tick));
                            // [I] Bound to the brain's own root when it or its
                            // own object starts it; another object's effect
                            // starts where that object stands.
                            let by = other.unwrap_or(object);
                            let (bound, at) = if by == self.object.id {
                                (self.fx.anchor, position)
                            } else {
                                (None, position.or_else(|| self.object_position(by)))
                            };
                            self.fx
                                .calls
                                .push((object, player::magic::Fx::Start { key, id, bound, at }));
                        }
                    }
                }
                Event::EffectCommand {
                    trap,
                    handle,
                    vector,
                    ..
                } if self.fx.drawn => {
                    let Some(&(key, _)) = self.fx.handles.get(&handle) else {
                        continue;
                    };
                    // The script's (engine) frame to the room's: (x, -y, -z).
                    let v = vector.map(|v| [v[0], -v[1], -v[2]]);
                    let fx = match (trap, v) {
                        (38, Some(v)) => player::magic::Fx::Pos(key, v),
                        (41, Some(v)) => player::magic::Fx::Dir(key, v),
                        (44, _) => player::magic::Fx::LoopEnd(key),
                        (51 | 85, _) => player::magic::Fx::Kill(key),
                        _ => continue,
                    };
                    self.fx.calls.push((self.object.id, fx));
                }
                Event::Voice { id, arg, .. } => self.voices.push((id, arg)),
                // FUN_1403b6170: the prize table row the script scatters
                // (Soldier's death act passes the row its init stored).
                Event::ScatterPrize { object, count } => {
                    if let Some(at) = self.object_position(object) {
                        self.prizes.push((count, at));
                    }
                }
                // Range 10 / 0x21 (attack windows) and frame event 8 (sound)
                // are read from the clip by the host itself.
                Event::MotionEvent { range, kind, .. }
                    if !matches!((range, kind), (true, 10 | 0x21) | (false, 8)) =>
                {
                    self.motion_events.insert((range, kind));
                }
                Event::MotionEvent { .. } => {}
                Event::Unsupported(name) => {
                    self.unsupported.insert(name);
                }
                Event::Presentation { name, .. } => {
                    self.unsupported.insert(name);
                }
                Event::Service {
                    name: "stitch_mp_refill",
                    ..
                } => self.mp_refills += 1,
                Event::Service {
                    name: "genie_form",
                    words,
                    ..
                } => self.genie_form = Some(words[0] as u8),
                // 6:1 FUN_14053dba0 -> FUN_1403dc170: the prize row's drops at a
                // place the script names (M_EX590 scatters as it is struck).
                Event::Service {
                    name: "prize_appear",
                    words,
                    vector: Some(v),
                    ..
                } => self.prizes.push((words[1] as i32, [v[0], v[1], v[2]])),
                // FUN_1403b2850: the script offers a command on a record.
                Event::Service {
                    name: "command_override",
                    words,
                    other,
                    ..
                } => {
                    let r = words[0] as usize;
                    if r < self.offers.len() {
                        self.offers[r] = Some(words[1] as u16);
                        self.offer_targets[r] = other;
                    }
                }
                Event::Service {
                    name: "obj_add_hp",
                    object: Some(PLAYER_OBJECT),
                    words,
                    ..
                } => self.sora_hp.push(words[0] as i32),
                Event::Service {
                    name: "obj_start_mpdrive",
                    object: Some(PLAYER_OBJECT),
                    words,
                    ..
                } => self.sora_mpdrive.push(f32::from_bits(words[0])),
                Event::Service {
                    name: name @ ("camera_watch" | "func_screen_whiteout" | "func_screen_whitein"),
                    object,
                    words,
                    ..
                } => self.presentations.push((name, object, words)),
                Event::Service { name, .. } => {
                    self.services.insert(name);
                }
                Event::Sound { id, position, .. } => self.sounds.push((id, position)),
                // Sora plays what a running limit captures for him (the
                // summon host reads his stand-in's capture); another
                // victim's is not drawn.
                Event::MotionCapture { victim, .. }
                    if victim == PLAYER_OBJECT && self.limit.is_some() => {}
                // A boss captures Sora into a motion of its own set.
                Event::MotionCapture {
                    victim,
                    motion,
                    blend,
                    ..
                } if victim == PLAYER_OBJECT => self.sora_captures.push((motion, blend)),
                Event::MotionCapture { .. } => {
                    self.unsupported.insert("obj_motion_capture");
                }
                Event::AttackStrike {
                    slot,
                    target: Some(PLAYER_OBJECT),
                    ..
                } => self.strikes.push(slot),
                Event::AttackStrike {
                    slot,
                    target: Some(ENEMY),
                    param,
                    serial,
                    from,
                    ..
                } => self.enemy_strikes.push((slot, param, serial, from)),
                // Only a limit scene runs the action threads `obj_hook`
                // binds to objects.
                Event::MagicStart { .. } => {
                    self.unsupported.insert("obj_hook");
                }
                Event::Signal { code, argument } => self.signals.push((code, argument)),
                _ => {}
            }
        }
    }

    /// What the script asked of the host since the last call: the sound
    /// effects to play, the voice lines, and the names of the requests that
    /// nothing here plays (each reported once).
    pub fn take_reports(&mut self) -> ScriptReports {
        let new = |set: &std::collections::BTreeSet<&'static str>,
                   reported: &mut std::collections::BTreeSet<&'static str>| {
            set.iter()
                .copied()
                .filter(|name| reported.insert(name))
                .collect::<Vec<_>>()
        };
        ScriptReports {
            sounds: std::mem::take(&mut self.sounds),
            voices: std::mem::take(&mut self.voices),
            services: new(&self.services, &mut self.reported),
            unsupported: new(&self.unsupported, &mut self.reported),
        }
    }

    fn record_faults(&mut self) {
        if self.fault.is_none() {
            if let Some((word, fault)) = self.object.script.faults.first() {
                self.fault = Some(format!("script fault at word {word}: {fault:?}"));
            } else if let Some((entry, (word, fault))) = self
                .world
                .objects
                .iter()
                .flatten()
                .find_map(|o| o.script.faults.first().map(|f| (o.sys.entry_id, f)))
            {
                self.fault = Some(format!(
                    "made object {entry:#x}: script fault at word {word}: {fault:?}"
                ));
            } else if let Some((table, index)) = self.world.missing.first() {
                self.fault = Some(format!("trap {table}:{index} has no translation"));
            }
        }
    }

    pub fn position(&self) -> Vec3 {
        self.object.sys.position
    }
    /// Where an object of the script's world stands (its own, or one its
    /// script made).
    fn object_position(&self, id: u32) -> Option<Vec3> {
        if id == self.object.id {
            return Some(self.object.sys.position);
        }
        let o = self.world.objects.get(id as usize)?.as_deref()?;
        Some(o.sys.position)
    }
    /// The model of an object the script made (its object entry's).
    pub fn object_entry(&self, id: u32) -> Option<u32> {
        if id == self.object.id {
            return Some(self.object.sys.entry_id);
        }
        Some(
            self.world
                .objects
                .get(id as usize)?
                .as_deref()?
                .sys
                .entry_id,
        )
    }
    /// Where the host put an object it places itself (Stitch's camera
    /// space).
    pub fn set_position(&mut self, p: Vec3) {
        self.object.sys.position = p;
    }
    /// A summon's limit deck while it is up: Chicken Little's (mode 5) or
    /// Stitch's (mode 4), (command, selector) rows.
    pub fn special_deck(&self) -> Option<Vec<(u16, u16)>> {
        let f = self.object.friend.as_deref()?;
        if f.limit {
            return Some(enemy::friend::CHICKEN_LITTLE_DECK.to_vec());
        }
        let s = f.stitch.as_deref()?;
        s.limit.then(|| enemy::friend::STITCH_DECK.to_vec())
    }
    /// The deck's sheet mode (FUN_1403db140's argument).
    pub fn special_mode(&self) -> Option<u8> {
        let f = self.object.friend.as_deref()?;
        if f.limit {
            return Some(5);
        }
        f.stitch.as_deref()?.limit.then_some(4)
    }
    /// Chicken Little's limit mode (+0x28 bit 0): his first-person view.
    pub fn chicken_limit(&self) -> bool {
        self.object.friend.as_deref().is_some_and(|f| f.limit)
    }
    /// Stitch's limit: Sora rides in camera space at this screen point.
    pub fn stitch_sora_point(&self) -> Option<[f32; 4]> {
        let s = self.object.friend.as_deref()?.stitch.as_deref()?;
        s.limit.then_some(enemy::friend::stitch::SORA_POINT)
    }

    /// Chicken Little's aim point (personality +0x40).
    pub fn aim(&self) -> Option<Vec3> {
        Some(self.object.friend.as_deref()?.aim)
    }

    /// Chicken Little's aim point (personality +0x40), as FUN_1401cb830
    /// sets it in his limit mode.
    pub fn set_aim(&mut self, aim: Vec3) {
        if let Some(f) = self.object.friend.as_deref_mut() {
            f.aim = aim;
        }
    }

    /// Stitch's screen position (x right, y down, depth, heading), once
    /// it is set.
    pub fn stitch_screen(&self) -> Option<[f32; 4]> {
        let s = self.object.friend.as_deref()?.stitch.as_deref()?;
        s.valid.then_some(s.screen)
    }
    /// The enemies the script made that the gallery draws for it.
    pub fn made_fighters(&self) -> Vec<MadeFighter> {
        self.world
            .objects
            .iter()
            .flatten()
            .filter(|o| {
                let entry = enemy::sysobj::model_entry(o.sys.model);
                !o.sys.gone && super::enemy_battle::made_kind(entry).is_some()
            })
            .map(|o| MadeFighter {
                id: o.id,
                entry: enemy::sysobj::model_entry(o.sys.model),
                position: o.sys.position,
                yaw: o.sys.yaw,
                slot: o.sys.motion.slot,
                tick: o.sys.motion.tick,
                blending: o.sys.motion.blend_weight().is_some(),
                alpha: o.sys.fade.value,
                hidden: o.sys.hidden || o.sys.model_hidden,
            })
            .collect()
    }
    /// Objects the script made of an entry `enemy_summon_entry` drew: each
    /// leaves this script's world (their entry, place and heading) to fight
    /// as an actor of its own.
    pub fn take_summoned(&mut self) -> Vec<(u32, Vec3, f32)> {
        let summoned = std::mem::take(&mut self.world.summoned);
        let mut out = Vec::new();
        for o in self.world.objects.iter_mut().flatten() {
            if !o.sys.gone && summoned.contains(&o.sys.model) {
                o.sys.gone = true;
                out.push((o.sys.model, o.sys.position, o.sys.yaw));
            }
        }
        out
    }
    /// See [`super::enemy_battle::Brain::shown`].
    pub fn shown(&self) -> (f32, bool) {
        let s = &self.object.sys;
        (s.fade.value, s.hidden || s.model_hidden)
    }
    /// The object's fade (+0xa08): its alpha and whether it is falling.
    pub fn fade(&self) -> (f32, bool) {
        let f = &self.object.sys.fade;
        (f.value, f.rate < 0.)
    }

    /// Record every trap the script calls (a development aid).
    pub fn trace(&mut self) {
        self.world.trace.get_or_insert_with(Vec::new);
    }

    /// The traps called since the last call, in order.
    pub fn take_trace(&mut self) -> Vec<(u16, u16)> {
        self.world
            .trace
            .as_mut()
            .map(std::mem::take)
            .unwrap_or_default()
    }

    /// The summon's friend state for the host log: band, speed, walk, run,
    /// target, flags.
    pub fn friend_state(&self) -> Option<(usize, f32, f32, f32, u32, u32)> {
        let f = self.object.friend.as_deref()?;
        Some((
            f.band,
            self.object.sys.move_speed,
            f.walk,
            f.run,
            self.object.sys.target.id,
            f.flags,
        ))
    }

    /// The traps the script called that have no translation, once each.
    pub fn missing_traps(&self) -> Vec<(u16, u16)> {
        let mut m = self.world.missing.clone();
        m.sort_unstable();
        m.dedup();
        m
    }
    pub fn yaw(&self) -> f32 {
        self.object.sys.yaw
    }
    /// FUN_1403d1730's last step for a hit on Sora: the attack's owner's
    /// OBJ_ATTACK (FUN_140410ce0) with Sora and the attack: a script attack
    /// object (`slot`) or a motion window's attack `param_id`.
    pub fn attack_reached_player(&mut self, slot: Option<usize>, param_id: i32) {
        let env = BrainEnv {
            room: None,
            clips: ClipView {
                base: &self.clips,
                captured: &self.captured,
            },
            player: self.player,
            player_yaw: self.player_yaw,
            facts: self.facts,
            commands: &self.commands,
            fx: &self.fx,
        };
        self.object
            .attack_reached(&mut self.world, &env, PLAYER_OBJECT, slot, param_id);
        self.drain_events();
        self.take_player_queue();
        self.record_faults();
    }

    /// The motions the script queued behind Sora's (his stand-in's queue);
    /// a running limit keeps its own.
    fn take_player_queue(&mut self) {
        if self.limit.is_some() || self.world.limit.is_some() {
            return;
        }
        if let Some(p) = self.world.player_proxy.as_mut() {
            self.sora_pushed.append(&mut p.sys.motion_queue);
            let falling = p.sys.falling();
            if falling && !self.proxy_falling {
                let m = p.sys.jump_motions;
                self.sora_falls.push((m.fall, m.land));
            }
            self.proxy_falling = falling;
        }
    }

    /// The lights the script made (FUN_1403f6f80's pool).
    pub fn lights(&self) -> &[Option<enemy::world::Light>] {
        &self.world.lights
    }

    /// The vacuums the script made (FUN_1403f8f20's list).
    pub fn vacuums(&self) -> &[Option<enemy::world::Vacuum>] {
        &self.world.vacuums
    }

    /// FUN_1403ba600 for the object's REACTION shape of `group`.
    pub fn reaction_offered(&self, command: i32, group: i32, default_groups: u32) -> bool {
        let s = &self.object.sys;
        s.reaction_groups
            .offered(s.script_group_bits(), default_groups, command, group)
    }
    /// FUN_1403ba550: whether the object's collision group is on.
    pub fn group_on(&self, group: i32, default_groups: u32) -> bool {
        let s = &self.object.sys;
        s.reaction_groups
            .group_on(s.script_group_bits(), default_groups, group)
    }
    /// The engine state a boss run logs: the revenge gauge and its limit,
    /// the active state pattern, the bound sheet and the class bits.
    pub fn boss_state(&self) -> serde_json::Value {
        let s = &self.object.sys;
        serde_json::json!({"yaw":s.yaw,"move":[s.move_direction[0], s.move_direction[2], s.move_speed],
            "velocity":s.velocity,"target":s.target.id,"mode":format!("{:#x}", s.mode_flags()),
            "revenge":s.revenge,"karma_limit":s.karma_limit,
            "patterns":format!("{:#x}", s.active_patterns()),"sheet":s.sheet,
            "max_hp":s.stats.max_hp,"class":format!("{:#x}", s.class_flags),
            "dead":s.state_flags & enemy::sysobj::state_flag::DEAD_MARKED != 0,
            "airborne":s.airborne,"air_blocked":s.air_blocked(),"jumping":s.is_jump(),
            "motion_flags":format!("{:#x}", s.motion.flags.word),"flags":format!("{:#x}", s.flags),
            "y":s.position[1],"wall_radius":self.wall_radius,
            "map_body":s.map_body.map(|b| [b.radius, b.height, b.offset, b.mode as f32, b.class as f32]),
            "no_floor":s.no_floor,"motion_tick":s.motion.tick,"blend":s.motion.blend,
            "pending_mode":s.pending_mode,"acts_flags":self.object.acts.flags,"turn_rate":s.turn_rate,
            "pc":[self.object.script.main.as_ref().map(|t| t.pc),
                  self.object.script.act_thread.as_ref().map(|t| t.pc)],
            "made":self.world.objects.iter().flatten().filter(|o| !o.sys.gone)
                .map(|o| (o.id, o.sys.model, o.sys.hit_shapes.len())).collect::<Vec<_>>()})
    }
    pub fn hp(&self) -> i32 {
        self.object.sys.hp
    }
    /// A boss-class object (+0x6c8 bit 10).
    pub fn boss(&self) -> bool {
        self.object.sys.class_flags & enemy::sysobj::class_flag::BOSS != 0
    }
    /// +0x4dc: the team.
    pub fn team(&self) -> i32 {
        self.object.sys.team
    }
    /// The ENMP row `btlobj_set_sheet` bound (0 before any).
    pub fn sheet(&self) -> i32 {
        self.object.sys.sheet
    }
    pub fn alive(&self) -> bool {
        !self.object.sys.gone && !self.object.acts.locked()
    }
    pub fn targetable(&self) -> bool {
        self.object.targetable()
    }
    pub fn record_mut(&mut self) -> &mut combat::HitRecord {
        &mut self.object.sys.record
    }
    /// The motion's control word (+0x18c).
    pub fn motion_flags(&self) -> u32 {
        self.object.sys.motion.flags.word
    }
    /// The physics mode word (`SysObj::mode_flags`).
    pub fn mode_flags(&self) -> u32 {
        self.object.sys.mode_flags()
    }
    pub fn motion(&self) -> usize {
        self.object.sys.motion.slot
    }
    pub fn elapsed(&self) -> u32 {
        self.object.sys.motion.tick
    }
    /// The playing motion has a clip (a script can name a slot the motion
    /// set leaves empty).
    pub fn has_clip(&self) -> bool {
        let clips = self
            .motion_table()
            .and_then(|h| self.clips.hook(h))
            .unwrap_or(&self.clips);
        enemy::env::Clips::length(clips, self.motion()) > 0
    }
    pub fn serial(&self) -> u64 {
        self.object.sys.motion.serial
    }
    pub fn scale_y(&self) -> f32 {
        self.object.sys.scale[1]
    }
    pub fn blend_weight(&self) -> Option<f32> {
        self.object.sys.motion.blend_weight()
    }
    pub fn blend(&self) -> f32 {
        self.object.sys.motion.blend
    }
    pub fn gone(&self) -> bool {
        self.object.sys.gone
    }
    pub fn description(&self) -> String {
        self.object
            .acts
            .current
            .map(|i| String::from_utf8_lossy(&self.object.acts.table[i].name).to_string())
            .unwrap_or_default()
    }

    pub fn hit(
        &mut self,
        direction: Vec3,
        high: bool,
        attack: &combat::AttackParams,
        attacker: &combat::AttackerStats,
        difficulty: combat::Difficulty,
        record: super::enemy_battle::HitRecordInfo,
    ) -> Option<i32> {
        let env = BrainEnv {
            room: None,
            clips: ClipView {
                base: &self.clips,
                captured: &self.captured,
            },
            player: self.player,
            player_yaw: self.player_yaw,
            facts: self.facts,
            commands: &self.commands,
            fx: &self.fx,
        };
        let damage = self.object.take_hit(
            &mut self.world,
            &env,
            &enemy::damage::Hit {
                direction,
                high,
                attack,
                attacker,
                difficulty,
                param_id: u32::from(record.param_id),
                revenge: record.revenge,
                pool: record.pool,
                by_leader: record.by_leader,
                by_party: record.by_party,
            },
        );
        self.drain_events();
        self.take_player_queue();
        self.record_faults();
        damage
    }

    /// The objects the script made that have HIT spheres, placed in the
    /// scene by their position and heading.
    pub fn made_targets(&self) -> Vec<MadeTarget> {
        self.world
            .objects
            .iter()
            .flatten()
            .filter(|o| !o.sys.gone && !o.sys.left && !o.sys.hit_shapes.is_empty())
            .map(|o| {
                let at = bevy::math::Vec3::from_array(o.sys.position);
                let turn = bevy::math::Quat::from_rotation_y(o.sys.yaw);
                MadeTarget {
                    id: o.id,
                    spheres: o
                        .sys
                        .hit_shapes
                        .iter()
                        .map(|&(offset, r)| (at + turn * bevy::math::Vec3::from_array(offset), r))
                        .collect(),
                }
            })
            .collect()
    }

    /// A retired attack id leaves the made objects' records too.
    pub fn clear_made_seen(&mut self, id: u8) {
        for o in self.world.objects.iter_mut().flatten() {
            o.sys.record.clear_seen(id);
        }
    }

    /// A made object's hit record (its attack mask).
    pub fn made_record_mut(&mut self, id: u32) -> Option<&mut combat::HitRecord> {
        self.world
            .objects
            .get_mut(id as usize)?
            .as_mut()
            .map(|o| &mut o.sys.record)
    }

    /// A hit on an object the script made (FUN_140410d60 on it): its
    /// OBJ_DAMAGE runs (B_EX360 marks its unit argument 1, which Shadow
    /// Flare's thread reads as the orb struck back).
    pub fn hit_made(&mut self, id: u32, hit: &enemy::damage::Hit<'_>) -> Option<i32> {
        let env = BrainEnv {
            room: None,
            clips: ClipView {
                base: &self.clips,
                captured: &self.captured,
            },
            player: self.player,
            player_yaw: self.player_yaw,
            facts: self.facts,
            commands: &self.commands,
            fx: &self.fx,
        };
        let slot = self.world.objects.get_mut(id as usize)?;
        let mut object = slot.take()?;
        let damage = object.take_hit(&mut self.world, &env, hit);
        if let Some(slot) = self.world.objects.get_mut(id as usize) {
            *slot = Some(object);
        }
        self.drain_events();
        self.record_faults();
        damage
    }

    pub fn reflect(&mut self, motion: usize, back: f32) {
        let env = BrainEnv {
            room: None,
            clips: ClipView {
                base: &self.clips,
                captured: &self.captured,
            },
            player: self.player,
            player_yaw: self.player_yaw,
            facts: self.facts,
            commands: &self.commands,
            fx: &self.fx,
        };
        self.object.reflect(&mut self.world, &env, motion, back);
        self.record_faults();
    }

    /// FUN_1403de610's summon: +0x6c8 |= 0x400000 (`sysobj_is_summon`),
    /// not of the enemy class, on Sora's team [I: the team follows the
    /// object type in the constructor FUN_1401b06e0, not traced].
    pub fn make_summon(&mut self, entry: u32, friend: Option<enemy::friend::Friend>) {
        use enemy::sysobj::class_flag;
        let s = &mut self.object.sys;
        // The objentry id `obj_search_by_entry` matches (+0x6c4's entry);
        // Chicken Little's fireworks find him by it.
        s.entry_id = entry;
        s.class_flags = s.class_flags & !class_flag::ENEMY | class_flag::SUMMON;
        s.team = SORA_TEAM as i32;
        self.object.friend = friend.map(Box::new);
    }

    /// FUN_1401b56e0 mode 1: Sora began attack `attack` at `target` (the
    /// enemy's stand-in, or nothing).
    pub fn note_player_attack(&mut self, attack: u32, target: Option<u32>) {
        enemy::friend::note_player_attack(
            &mut self.world,
            Some(&mut self.object),
            attack,
            target.unwrap_or(0),
        );
    }

    /// FUN_1401b5d60: Sora's attack now (his +0x180), once a frame.
    pub fn age_player_notify(&mut self, current: Option<u32>) {
        enemy::friend::age_player_notify(&mut self.world, current);
    }

    /// FUN_1401b56e0 mode 0: Sora was hit by `attacker`; it joins his
    /// attacker list (once, at most 8, the oldest dropped).
    pub fn note_player_attacker(&mut self, attacker: u32) {
        let list = &mut self.world.player_attackers;
        list.retain(|&o| o != attacker);
        if list.len() == 8 {
            list.remove(0);
        }
        list.push(attacker);
    }

    /// The host builds the objects `sysobj_appear` asks for.
    pub fn set_spawner(&mut self, spawner: Box<dyn enemy::world::Spawner>) {
        self.world.spawner = Some(spawner);
    }

    /// An attack of the script struck `target` (the enemy's stand-in): its
    /// hit callback runs (FUN_1403d1730).
    pub fn mark_attack_hit_on(&mut self, slot: usize, target: u32) {
        let env = BrainEnv {
            room: None,
            clips: ClipView {
                base: &self.clips,
                captured: &self.captured,
            },
            player: self.player,
            player_yaw: self.player_yaw,
            facts: self.facts,
            commands: &self.commands,
            fx: &self.fx,
        };
        self.object
            .attack_struck(&mut self.world, &env, slot, target);
        self.record_faults();
    }

    /// FUN_1403b5fa0 (from FUN_1403a8b10, Sora's decided command): trigger
    /// 11 of the object's script with Sora, the command and the target
    /// record (Sora's reaction slot +0x10, FUN_1403a8b10): a four-word record
    /// whose first word is the object it names and whose third is the
    /// attack (`attack`: its serial) for an attack-borne offer.
    pub fn command(&mut self, command: u16, group: u8, attack: Option<u32>) {
        let record = self.world.static_cell(0xffff, 11, 0);
        let handle = enemy::world::object_handle(self.object.id);
        let slot = attack.and_then(|serial| {
            let live = |a: &Option<enemy::attack::Attack>| {
                a.as_ref().is_some_and(|a| a.serial as u32 == serial)
            };
            self.world.attacks.iter().position(live)
        });
        let third = slot.map_or(handle, |k| enemy::world::ATTACK_HANDLES + 0x100 * k as u32);
        for (k, word) in [handle, u32::from(group), third, u32::MAX]
            .into_iter()
            .enumerate()
        {
            self.world.static_write(record + k as u32 * 4, word);
        }
        let env = BrainEnv {
            room: None,
            clips: ClipView {
                base: &self.clips,
                captured: &self.captured,
            },
            player: self.player,
            player_yaw: self.player_yaw,
            facts: self.facts,
            commands: &self.commands,
            fx: &self.fx,
        };
        self.object.fire(
            &mut self.world,
            &env,
            11,
            &[
                enemy::script::Slot::address(enemy::world::PLAYER_HANDLE),
                enemy::script::Slot::int(i32::from(command)),
                enemy::script::Slot::address(record),
            ],
        );
        self.drain_events();
        self.take_player_queue();
        self.record_faults();
    }

    /// FUN_1401cf370 (Genie's first update): his form command from the
    /// form number (FUN_1403de220).
    pub fn set_genie_form(&mut self, command: u16) {
        if let Some(f) = self.object.friend.as_deref_mut() {
            f.genie_form = command;
        }
    }

    /// The owner object of an attack the script placed.
    pub fn attack_owner(&self, slot: usize) -> Option<u32> {
        self.world.attacks.get(slot)?.as_ref()?.owner
    }

    /// The room's enemy as a stand-in in this script's world (an enemy-class
    /// object at [`ENEMY`], team 2), kept at its place; `None` leaves it at
    /// 0 HP (the slot stays the stand-in's).
    pub fn set_enemy(&mut self, at: Option<Vec3>) {
        use enemy::sysobj::class_flag;
        let slots = &mut self.world.objects;
        if slots.len() <= ENEMY as usize {
            slots.resize_with(ENEMY as usize + 1, || None);
        }
        let o = slots[ENEMY as usize].get_or_insert_with(|| {
            let mut o = Object::bare(ENEMY);
            o.sys.class_flags |= class_flag::ENEMY;
            o.sys.team = 2;
            o.sys.model = u32::MAX;
            Box::new(o)
        });
        o.sys.hp = at.is_some() as i32;
        if let Some(p) = at {
            o.sys.position = p;
        }
    }

    /// Start the named act (FUN_1403cac50 / FUN_1403cae60): the running act
    /// and the queue end first.
    pub fn start_act(&mut self, name: &[u8]) {
        let env = BrainEnv {
            room: None,
            clips: ClipView {
                base: &self.clips,
                captured: &self.captured,
            },
            player: self.player,
            player_yaw: self.player_yaw,
            facts: self.facts,
            commands: &self.commands,
            fx: &self.fx,
        };
        let actions = self.object.acts.start(name);
        self.object
            .apply_actions(Some((&mut self.world, &env)), actions);
        self.record_faults();
    }

    /// Development aid: set pool 0's HP (the tours use it to reach the end
    /// of a long fight).
    pub fn set_hp(&mut self, hp: i32) {
        self.object.sys.set_pool_hp(0, hp);
    }

    /// Development command: force the death path.
    pub fn defeat(&mut self) {
        if !self.object.targetable() {
            return;
        }
        self.object.sys.hp = 0;
        let env = BrainEnv {
            room: None,
            clips: ClipView {
                base: &self.clips,
                captured: &self.captured,
            },
            player: self.player,
            player_yaw: self.player_yaw,
            facts: self.facts,
            commands: &self.commands,
            fx: &self.fx,
        };
        let actions = self.object.acts.start(b"dead");
        self.object
            .apply_actions(Some((&mut self.world, &env)), actions);
    }

    /// Development command: a fresh object at the spawn point.
    pub fn respawn(&mut self) {
        let (stats, sstm, yaw) = (
            self.object.sys.stats,
            self.object.sys.sstm,
            self.object.sys.yaw,
        );
        let spawn = self.spawn;
        let mut rng = ScriptRandom(self.world.rng.0);
        if let Err(e) = self.build(spawn, yaw, stats, sstm, &mut rng) {
            self.fault = Some(e);
        }
    }
}
