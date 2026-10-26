//! The party limits (Whirli-Goof, Knocksmash, Comet, Duck Flare, Trinity):
//! each one's file (`limit/<lang>/<name>.bar`, the 00battle `limt` row's
//! file) runs its scene program on the object-script engine
//! (`enemy::limit_scene`). Sora and the friends play the file's own motions
//! while the scene drives them; their hits come from those motions' attack
//! windows, the scene's follow-ups go on the reaction button, its effects
//! come from the file's container.
//!
//! The start (FUN_1403d8b40 for a `limt` row): the limit's MP (all of it:
//! cost 0xff, FUN_1403a39b0) leaves Sora, the members not taking part fade
//! out (FUN_140400990), and the scene's trigger 22 runs. The end
//! (FUN_1403d8f40 / FUN_1403d83e0) brings them back.
//!
//! Limit Form's four limits run the same way: they have no `limt` row or
//! file; their program is the form's own (`limi` of the Limit Form model,
//! the `limitform` program), Sora plays his own Limit Form motions, its
//! effects come from the form's container and its tint from the limit's
//! filter light.
use super::second_deck::{self, LimitRow};
use data::{limit_file, motion, path_lookup::Library};
use enemy::motion::{FrameEvent, MotionEvents, RangeEvent};

/// Sora's (character 1) and the friends' motion keys (objentry +7).
const SORA: u8 = 1;
/// Rows of the special deck as decide records (above the override records).
pub const SPECIAL: usize = 100;

/// A participant's clip key: (character, weapon slot or None, clip id).
pub type ClipKey = (u8, Option<u8>, u16);

/// A limit file the room holds: its row, its file and its index (the clip
/// ids of its motions).
pub struct LoadedLimit {
    pub row: LimitRow,
    pub file: limit_file::LimitFile,
    pub index: u8,
    /// Each participant's clips: (character, weapon slot or None, clip id)
    /// with their engine trigger tables.
    pub events: Vec<(ClipKey, MotionEvents)>,
    /// The file's effect container.
    pub pax: Option<std::sync::Arc<data::object_effect::Pax>>,
    /// The gauge and combo counter sheets (`gage`, `comb`; FUN_1403d94d0)
    /// with their HD art.
    pub gauge_sheet: Option<visual::hud::Sheet>,
    pub counter_sheet: Option<visual::hud::Sheet>,
    /// The command menu's sequence and art while the special deck is up
    /// (`comd`, Trinity Limit only: FUN_1403d96d0 -> FUN_1403b2a90 ->
    /// FUN_1404065a0 -> FUN_14015de90 swaps both).
    pub menu_sheet: Option<visual::hud::Sheet>,
    /// Limit Form's: the program plays Sora's own motions (by their ids).
    pub own: bool,
}

/// The Limit Form model whose archive holds the `limitform` program
/// (`limi`) and whose container holds its effects (objentry 0x95d).
const LIMIT_FORM_MODEL: &str = "P_EX100_KH1F";
const LIMIT_FORM_OBJECT: u32 = 0x95d;

#[derive(Default)]
pub struct LimitSource {
    pub limits: Vec<LoadedLimit>,
}

/// The id a limit's motion `motion` takes among a character's clips: the
/// limit's index above the characters' own ids.
pub fn clip_id(index: u8, motion: u16) -> u16 {
    0x4000 | u16::from(index) << 9 | (motion & 0x1ff)
}

/// A motion's triggers as the engine reads them (ticks).
pub fn events_of(m: &motion::SelectedMotion) -> MotionEvents {
    MotionEvents {
        ranges: m
            .triggers
            .iter()
            .flat_map(|t| &t.records.ranges)
            .map(|r| RangeEvent {
                start: r.start_frame as u16,
                end: r.end_frame as u16,
                kind: r.trigger,
                params: r.parameters.iter().map(|&v| v as u16).collect(),
            })
            .collect(),
        frames: m
            .triggers
            .iter()
            .flat_map(|t| &t.records.frames)
            .map(|f| FrameEvent {
                frame: f.frame as u16,
                kind: f.trigger,
                params: f.parameters.iter().map(|&v| v as u16).collect(),
            })
            .collect(),
    }
}

/// The party limits of the `limt` table this room can run: Sora with
/// Donald and Goofy, any world, not a summon's [test-room choice: the
/// Timeless River and world guests' files are left out].
pub fn load(
    source: &data::probe_source::ProbeSource,
    library: &mut Library,
    battle: &[u8],
    language: &str,
) -> LimitSource {
    let Ok(limt) = super::bar_entry(battle, b"limt") else {
        return LimitSource::default();
    };
    let mut out = LimitSource::default();
    let mut names: Vec<String> = Vec::new();
    // The summons' models by character (the `sumn` rows' objects), whose
    // own archive may hold a limit deck's `comd` sheet (FUN_1403db140).
    let summon_models: HashMap<u8, String> = match (
        super::bar_entry(battle, b"sumn"),
        library
            .read("00objentry.bin")
            .ok()
            .and_then(|b| data::objentry::read(&b).ok()),
    ) {
        (Ok(sumn), Some(objects)) => second_deck::read_summons(sumn)
            .iter()
            .filter_map(|r| objects.iter().find(|o| o.object_id == u32::from(r.object)))
            .map(|o| {
                (
                    o.character as u8,
                    String::from_utf8_lossy(&o.model_name).into_owned(),
                )
            })
            .collect(),
        _ => HashMap::new(),
    };
    for row in second_deck::read_limits(limt) {
        // A summon's row (second participant 200-204) has a file of sheets
        // only: its program is the summon's own script (FUN_1403d7fa0).
        let summon = row.members[1].wrapping_add(0x38) < 5;
        let party = row.members.iter().all(|&m| m <= 3);
        let name = row.file_name();
        if !(summon || party) || row.ability == 0 || row.world != 0 || names.contains(&name) {
            continue;
        }
        let path = format!("limit/{language}/{name}");
        let read = if summon {
            limit_file::read_summon
        } else {
            limit_file::read
        };
        let file = match library
            .read(&path)
            .map_err(|e| e.to_string())
            .and_then(|b| read(&b).map_err(|e| e.to_string()))
        {
            Ok(f) => f,
            Err(e) => {
                eprintln!("{path}: {e}");
                continue;
            }
        };
        let index = out.limits.len() as u8;
        let mut events = Vec::new();
        for id in file.motion_ids() {
            for character in [SORA, 2, 3] {
                for weapon in [None, Some(0)] {
                    let Some(anb) = file.anb(id, limit_file::motion_key(character, weapon)) else {
                        continue;
                    };
                    if let Ok(m) = motion::read_anb(&anb, 0) {
                        events.push(((character, weapon, clip_id(index, id)), events_of(&m)));
                    }
                }
            }
        }
        names.push(name);
        let pax = file
            .pax
            .as_ref()
            .and_then(|b| data::object_effect::read_pax(b).ok())
            .map(std::sync::Arc::new);
        let sheet = |name: &[u8; 4]| {
            let found = library.find(&path)?;
            visual::hud::load(source, found.index, found.ordinal, found.name_hash, name)
                .map_err(|e| eprintln!("{path} {}: {e}", String::from_utf8_lossy(name)))
                .ok()
        };
        let (gauge_sheet, counter_sheet) = (sheet(b"gage"), sheet(b"comb"));
        // Trinity Limit's `comd` (FUN_1403d96d0, mode 3), or the summon's
        // own (Chicken Little's FUN_1401cb6c0 -> FUN_1403db140, mode 5).
        let menu_sheet = if summon {
            summon_models.get(&row.members[1]).and_then(|model| {
                // The object's archive (+0x928): its language one, else its
                // model's.
                [
                    format!("obj/{model}.a.{language}"),
                    format!("obj/{model}.mdlx"),
                ]
                .iter()
                .find_map(|path| {
                    let found = library.find(path)?;
                    visual::hud::load(source, found.index, found.ordinal, found.name_hash, b"comd")
                        .ok()
                })
            })
        } else {
            (row.command == 0x1e4).then(|| sheet(b"comd")).flatten()
        };
        out.limits.push(LoadedLimit {
            row,
            file,
            index,
            events,
            pax,
            gauge_sheet,
            counter_sheet,
            menu_sheet,
            own: false,
        });
    }
    // Limit Form's limits: the form's `limitform` program, its effects.
    let program = library
        .read(&format!("obj/{LIMIT_FORM_MODEL}.mdlx"))
        .ok()
        .and_then(|bytes| {
            let bar = data::bar::read_bar(&bytes).ok()?;
            let e = bar
                .entries
                .iter()
                .find(|e| &e.name_bytes == b"limi" && e.size != 0)?;
            bytes
                .get(e.offset as usize..e.offset as usize + e.size as usize)
                .map(<[u8]>::to_vec)
        });
    match program {
        Some(program) => {
            let pax =
                super::drive_room::object_pax(library, LIMIT_FORM_MODEL).map(std::sync::Arc::new);
            for command in player::limit_form::LIMITS {
                let mut file = [0u8; 32];
                file[..9].copy_from_slice(b"limitform");
                let index = out.limits.len() as u8;
                out.limits.push(LoadedLimit {
                    row: LimitRow {
                        id: 0,
                        members: [SORA, 0, 0],
                        command,
                        ability: 0,
                        world: 0,
                        file,
                    },
                    file: limit_file::LimitFile {
                        program: program.clone(),
                        motions: Vec::new(),
                        pax: None,
                        cameras: Vec::new(),
                        sheets: Vec::new(),
                    },
                    index,
                    events: Vec::new(),
                    pax: pax.clone(),
                    gauge_sheet: None,
                    counter_sheet: None,
                    menu_sheet: None,
                    own: true,
                });
            }
        }
        None => eprintln!("obj/{LIMIT_FORM_MODEL}.mdlx: no limitform program"),
    }
    out
}

impl LimitSource {
    /// A character's limit motions: (clip id, body motion, weapon motion),
    /// the body's slot made unique so a kit keeps them apart.
    pub fn motions(
        &self,
        character: u8,
    ) -> Vec<(u16, motion::SelectedMotion, Option<motion::SelectedMotion>)> {
        let mut out = Vec::new();
        for l in &self.limits {
            for id in l.file.motion_ids() {
                let clip = clip_id(l.index, id);
                let slot = 0x1_0000 + clip as usize;
                let Some(body) = l
                    .file
                    .anb(id, limit_file::motion_key(character, None))
                    .and_then(|b| motion::read_anb(&b, slot).ok())
                else {
                    continue;
                };
                // A window the clip builders cannot sample is left out.
                let usable = body
                    .motion
                    .frame_window()
                    .is_ok_and(|(start, end, fps, _)| {
                        (1. ..=120.).contains(&fps) && end > start.max(0.)
                    });
                if !usable {
                    eprintln!(
                        "{}: motion {id} of character {character} left out",
                        l.row.file_name()
                    );
                    continue;
                }
                let weapon = l
                    .file
                    .anb(id, limit_file::motion_key(character, Some(0)))
                    .and_then(|b| motion::read_anb(&b, slot).ok());
                out.push((clip, body, weapon));
            }
        }
        out
    }
}

// --- The running limit ---------------------------------------------------

use super::{Game, RoomWorld};
use enemy::{
    env::{Clips as EngineClips, Env},
    limit_scene::{Participant, Scene, Status},
    sysobj::{Attach, Sysobj},
    traps::flare::Shape as FlareShape,
    traps::limit::{LIMIT_EFFECTS, MOTIONS},
    world::{Event, ObjId, PLAYER_OBJECT},
};
use player::{Action, World as _, combat, magic::Anchor, party_members::Scripted};
use std::collections::HashMap;

/// Engine model ids: the character's own clips, its weapon's 100 above.
const WEAPON_MODEL: u32 = 100;
/// Engine object ids: the friends, the weapons (Sora's first), the enemy.
const FRIEND: ObjId = 10;
const WEAPONS: ObjId = 20;
const ENEMY: ObjId = 2;
/// Sora's side (+0x4dc): team 1 (the enemies' attacks carry it in their masks;
/// `attack_new` gives an attack its owner's team).
const SORA_TEAM: i32 = 1;
/// Effect keys of the scene's handles (above the spells' and the party's).
const FX_KEY: u64 = 1 << 59;

type V3 = [f32; 3];

/// One character's clips as the engine reads them during a limit: length,
/// loop start and root per tick, and the trigger tables.
#[derive(Default)]
struct ClipView {
    clips: HashMap<usize, (u32, u32, Vec<V3>)>,
    events: HashMap<usize, MotionEvents>,
}

impl EngineClips for ClipView {
    fn length(&self, slot: usize) -> u32 {
        self.clips.get(&slot).map_or(0, |c| c.0)
    }
    fn loop_start(&self, slot: usize) -> u32 {
        self.clips.get(&slot).map_or(0, |c| c.1)
    }
    fn root_delta(&self, slot: usize, tick: u32) -> V3 {
        let Some(c) = self.clips.get(&slot) else {
            return [0.; 3];
        };
        let at = |t: u32| c.2.get(t as usize).copied().unwrap_or([0.; 3]);
        let (a, b) = (at(tick), at(tick + 1));
        [b[0] - a[0], b[1] - a[1], b[2] - a[2]]
    }
    fn events(&self, slot: usize) -> Option<&MotionEvents> {
        self.events.get(&slot)
    }
}

struct RoomEnv<'a> {
    world: &'a RoomWorld,
    body: player::Body,
    views: &'a [(u32, ClipView)],
    empty: ClipView,
    stick: V3,
    enemy: Option<V3>,
    /// Where Sora stands (the holder of his Keyblade and of what the scene
    /// attaches to him).
    player: V3,
    /// His heading (`obj_dir` of Sora, FUN_1403b56e0).
    player_yaw: f32,
    /// His TARGET radius plus the Shadow's (FUN_1403d7520).
    enemy_reach: Option<f32>,
    /// The room's target holder (its port of FUN_1403dcd70 / FUN_1403dd650)
    /// holds the Shadow, locked or not: Sora's +0xdc8 record.
    selected: bool,
    /// Sora is locked on to the Shadow (his lock-on record, obj+0xdc8).
    locked: bool,
    /// The Shadow's TARGET shape radius.
    enemy_radius: Option<f32>,
    /// Sora may take a command now (FUN_1403ba070's lockout and free
    /// windows), as his motion stood at the start of the tick.
    can_command: bool,
    /// Commands' +0x22 (the motion window that also takes them), +0x1f
    /// (whose windows decide them) and flags (+0xc).
    chains: &'a HashMap<u16, (u16, u8, u32)>,
    /// Sora's push shapes, relative to his origin.
    pushers: &'a [FlareShape],
    /// The scene's effect handles: their effect keys and start ticks.
    effects: &'a HashMap<u32, (u64, u64)>,
    /// The room's running effects after the last effect update.
    fx_live: &'a HashMap<u64, player::magic::FxLive>,
    /// Sora's PTYA records (`obj_party_attack`).
    records: &'a [player::ptya::Record],
    tick: u64,
    /// Effect handles from here on were asked for during this tick; the
    /// effect update has not taken them yet.
    fresh_effects: u32,
}

impl Env for RoomEnv<'_> {
    fn floor(&self, x: f32, z: f32, top: f32) -> Option<f32> {
        self.world.floor(x, z, top)
    }
    fn line_hit(&self, from: V3, to: V3) -> Option<V3> {
        self.world.line_hit(from, to)
    }
    fn movement(&self, from: V3, wished: V3) -> V3 {
        // Off the floor (a hop, a fall) the body is not held to it.
        let airborne = wished[1] > 0.
            || self
                .world
                .floor(from[0], from[2], from[1] + 1.)
                .is_some_and(|f| from[1] > f + 0.5);
        self.world
            .collide(from, wished, airborne, &self.body)
            .position
    }
    fn check_step(&self, _: V3, _: V3, _: f32, _: f32, _: f32) -> Option<u8> {
        Some(0)
    }
    fn clips(&self, model: u32) -> &dyn EngineClips {
        self.views
            .iter()
            .find(|(m, _)| *m == model)
            .map_or(&self.empty, |(_, v)| v)
    }
    fn player_position(&self) -> V3 {
        self.player
    }
    fn player_yaw(&self) -> f32 {
        self.player_yaw
    }
    fn lockon_target(&self) -> Option<u32> {
        (self.locked && self.enemy.is_some()).then_some(enemy::traps::target::OBJECT_TARGET | ENEMY)
    }
    fn attack_target(&self) -> Option<u32> {
        (self.selected && self.enemy.is_some())
            .then_some(enemy::traps::target::OBJECT_TARGET | ENEMY)
    }
    fn target_radius(&self, id: u32) -> Option<f32> {
        (id == ENEMY).then_some(self.enemy_radius).flatten()
    }
    fn target_position(&self, id: u32) -> Option<V3> {
        (id & !enemy::traps::target::OBJECT_TARGET == ENEMY)
            .then_some(self.enemy)
            .flatten()
    }
    fn pick_target(&self, _: i32, _: V3) -> Option<u32> {
        self.enemy.map(|_| ENEMY)
    }
    fn player_move_input(&self) -> V3 {
        self.stick
    }
    /// Sora's PTYA set (the current kit's records).
    fn party_attack(&self, object: u32, id: u8) -> Option<player::ptya::Record> {
        if object != PLAYER_OBJECT {
            return None;
        }
        self.records.iter().find(|r| r.id == id).copied()
    }
    fn target_reach(&self, id: u32) -> Option<f32> {
        (id & !enemy::traps::target::OBJECT_TARGET == ENEMY)
            .then_some(self.enemy_reach)
            .flatten()
    }
    fn can_decide_command(&self, object: u32, _command: u16) -> bool {
        object == PLAYER_OBJECT && self.can_command
    }
    fn command_chain(&self, command: u16) -> u16 {
        self.chains.get(&command).map_or(0, |c| c.0)
    }
    fn command_decider(&self, command: u16) -> u8 {
        self.chains.get(&command).map_or(0, |c| c.1)
    }
    fn command_flags(&self, command: u16) -> u32 {
        self.chains.get(&command).map_or(0, |c| c.2)
    }
    fn player_push_shapes(&self) -> Vec<FlareShape> {
        self.pushers.to_vec()
    }
    /// `effect_is_active` on a handle of the scene: the room's effect while
    /// it runs; one asked for this tick or the last that the effect update
    /// has not taken yet counts as running.
    fn effect_active(&self, handle: u32) -> bool {
        let Some(&(key, started)) = self.effects.get(&handle) else {
            // Asked for during this tick (a script may test it before its
            // next halt): it runs.
            return handle >= self.fresh_effects;
        };
        match self.fx_live.get(&key) {
            Some(l) if l.tracked => l.alive,
            Some(_) => false,
            None => self.tick <= started + 1,
        }
    }
    fn effect_position(&self, handle: u32) -> Option<V3> {
        let &(key, _) = self.effects.get(&handle)?;
        self.fx_live.get(&key).filter(|l| l.tracked).map(|l| l.pos)
    }
}

/// FUN_1403c9c00 for Sora's kind-4 entries: a shape only when its bone
/// code carries 0x4000 (the other case needs player +0x9b8 bit 10), its
/// centre the entry's offset lifted by its height offset (+Y down). [I]
/// The bone matrix (FUN_1403b50f0) is taken as Sora's origin unrotated;
/// his only entry has no offset.
fn push_shapes(game: &Game) -> Vec<FlareShape> {
    game.player_push_shapes
        .iter()
        .filter(|e| e.bone as u16 & 0x4000 != 0)
        .map(|e| FlareShape {
            kind: u32::from(e.shape),
            radius: f32::from(e.radius),
            height: f32::from(e.height),
            center: [
                f32::from(e.position[0]),
                f32::from(e.position[1]) - f32::from(e.position_height),
                f32::from(e.position[2]),
                1.,
            ],
        })
        .collect()
}

/// A party limit in progress.
pub struct Running {
    scene: Scene,
    limit: usize,
    views: Vec<(u32, ClipView)>,
    /// The friends taking part (party index, engine object).
    members: Vec<(usize, ObjId)>,
    /// The members the limit faded out (FUN_140400990).
    faded: Vec<usize>,
    /// The commands the scene offers this tick (`command_override`'s
    /// records, FUN_1403b2850): 0 takes Attack's row, 1 is the reaction
    /// command, 2 Limit's row, 3 to 6 a whole deck.
    pub offers: [Option<u16>; 8],
    /// Each record's target (the friend it names).
    targets: [Option<ObjId>; 8],
    /// The scene's effect handles: (effect key, tick asked for).
    effects: HashMap<u32, (u64, u64)>,
    pub command: u16,
    chains: HashMap<u16, (u16, u8, u32)>,
    pushers: Vec<FlareShape>,
    /// Commands the scene has offered (each logged once).
    seen: Vec<u16>,
    /// The last tick a reaction command was offered.
    pub last_reaction: u64,
    /// The Shadow's landed-hit tally as last counted.
    hits_seen: u32,
    /// The serial of the motion Sora last started in the scene.
    motion_serial: u64,
    /// The thrown Keyblade's axes (room frame, columns) from the last
    /// `obj_set_xyzrot` on it.
    weapon_axes: Option<[[f32; 3]; 3]>,
}

/// An object's axes from `obj_set_xyzrot`'s angles (the script frame, the
/// rows `euler_xyz` reads: row-vector Rx·Ry·Rz), turned into the room's
/// frame (C·R·C with C = diag(1, -1, -1)) as columns.
fn axes_of(angles: [f32; 3]) -> [[f32; 3]; 3] {
    let ([sa, sb, sc], [ca, cb, cc]) = (angles.map(f32::sin), angles.map(f32::cos));
    let rows = [
        [cb * cc, cb * sc, -sb],
        [sa * sb * cc - ca * sc, sa * sb * sc + ca * cc, sa * cb],
        [ca * sb * cc + sa * sc, ca * sb * sc - sa * cc, ca * cb],
    ];
    [
        [rows[0][0], -rows[0][1], -rows[0][2]],
        [-rows[1][0], rows[1][1], rows[1][2]],
        [-rows[2][0], rows[2][1], rows[2][2]],
    ]
}

/// The character's clip view for a limit: its idle and the limit's motions.
fn view_of(
    l: &LoadedLimit,
    character: u8,
    weapon: bool,
    idle: (u32, u32),
    clip: &dyn Fn(u16) -> Option<(u32, u32, Vec<V3>)>,
) -> ClipView {
    let mut v = ClipView::default();
    v.clips.insert(0, (idle.0, idle.1, Vec::new()));
    for id in l.file.motion_ids() {
        let c = clip_id(l.index, id);
        let slot = MOTIONS + id as usize;
        let w = weapon.then_some(0);
        let Some(events) = l
            .events
            .iter()
            .find(|((ch, wp, cid), _)| *ch == character && *wp == w && *cid == c)
            .map(|(_, e)| e.clone())
        else {
            continue;
        };
        if let Some((length, loop_start, roots)) = clip(c) {
            let roots = if weapon { Vec::new() } else { roots };
            v.clips.insert(slot, (length, loop_start, roots));
            v.events.insert(slot, events);
        }
    }
    v
}

/// Sora's clip view for a limit that plays his own motions (Limit Form's):
/// each scripted motion by its id, with its trigger tables.
fn own_view(
    events: &HashMap<u16, MotionEvents>,
    locomotion: &[(u32, u32); 6],
    clip: &dyn Fn(u16) -> Option<(u32, u32, Vec<V3>)>,
) -> ClipView {
    let mut v = ClipView::default();
    // Motions 0..=5 (idle, walk, run, jump, fall, land): the program queues
    // the fall (4) behind an air finisher. [I] Their trigger tables are not
    // given.
    for (id, &(length, loop_start)) in locomotion.iter().enumerate() {
        v.clips.insert(id, (length, loop_start, Vec::new()));
    }
    for (&id, e) in events {
        if let Some((length, loop_start, roots)) = clip(id) {
            v.clips.insert(usize::from(id), (length, loop_start, roots));
            v.events.insert(usize::from(id), e.clone());
        }
    }
    v
}

fn attached(owner: ObjId, model: u32) -> Sysobj {
    let mut s = Sysobj::bare();
    s.model = model;
    // A weapon of Sora's side is not of the enemy class.
    s.class_flags &= !enemy::sysobj::class_flag::ENEMY;
    s.team = SORA_TEAM;
    s.attached = Some(Attach {
        to: owner,
        bone: 0,
        flags: 0,
        callback: None,
    });
    s
}

fn room_body(game: &Game) -> player::Body {
    player::Body {
        radius: game.params.radius,
        height: game.params.height,
        step: game.params.step_height,
    }
}

pub(super) fn room_enemy_radius(game: &Game) -> Option<f32> {
    game.enemy
        .as_ref()
        .and_then(|rt| rt.targets.first())
        .map(|t| t.2)
}

pub(super) fn room_enemy(game: &Game) -> Option<V3> {
    game.enemy
        .as_ref()
        .filter(|rt| rt.enabled && rt.enemy.targetable())
        .map(|rt| rt.enemy.position)
}

impl Running {
    /// The special deck's rows while its window is up (Trinity Limit).
    pub fn special_deck(&self) -> Option<Vec<(u16, u16)>> {
        let l = self.scene.limit();
        l.special.then(|| l.combo.to_vec())
    }

    /// Where Sora's Keyblade flies while a Limit Form limit has let it go
    /// (its object detached from his hand).
    pub fn thrown_weapon(&self) -> Option<(V3, Option<[[f32; 3]; 3]>)> {
        if !player::limit_form::LIMITS.contains(&self.command) {
            return None;
        }
        let o = self.scene.object(WEAPONS)?;
        o.sys
            .attached
            .is_none()
            .then_some((o.sys.position, self.weapon_axes))
    }

    /// The shot pool's points to draw (Trinity Limit's shots).
    pub fn shot_points(&self) -> Vec<enemy::traps::trinity::Point> {
        self.scene.shot_points()
    }

    /// The frame the draw left in a shot's point.
    pub fn set_shot_frame(&mut self, base: u32, frame: u8) {
        self.scene.set_shot_frame(base, frame);
    }
}

/// FUN_1403d8b40 for a party limit's command.
pub fn start(game: &mut Game, command: u16, tick: u64) -> Result<(), String> {
    let source = game.party_limits.clone();
    let (k, l) = source
        .limits
        .iter()
        .enumerate()
        .find(|(_, l)| l.row.command == command)
        .ok_or("no file for this limit")?;
    // The friends taking part, in the row's order (the leader first; Sora
    // is character 1).
    let mut members = Vec::new();
    for &c in l.row.members.iter().filter(|&&c| c > 1) {
        let i = game
            .party
            .iter()
            .position(|m| m.kind.character == c && !m.stored)
            .ok_or("a member of the limit is not in the field")?;
        members.push((i, FRIEND + members.len() as ObjId));
    }
    // Sora's clips (his kit's scripted ones) and each friend's.
    let sora_idle = (game.clips.idle.end, game.clips.idle.loop_start);
    let sora_clip = |c: u16| {
        let (_, clip, roots) = game.clips.scripted_clip(c)?;
        Some((clip.end, clip.loop_start, roots.clone()))
    };
    let mut views = if l.own {
        // Limit Form's: Sora's own scripted motions with their triggers.
        let events = game
            .drive
            .as_ref()
            .and_then(|d| d.script_events.get(&player::forms::LIMIT))
            .cloned()
            .unwrap_or_default();
        let c = &game.clips;
        let locomotion =
            [&c.idle, &c.walk, &c.run, &c.jump, &c.fall, &c.land].map(|k| (k.end, k.loop_start));
        vec![
            (1, own_view(&events, &locomotion, &sora_clip)),
            (WEAPON_MODEL + 1, ClipView::default()),
        ]
    } else {
        vec![
            (1, view_of(l, 1, false, sora_idle, &sora_clip)),
            (WEAPON_MODEL + 1, view_of(l, 1, true, sora_idle, &sora_clip)),
        ]
    };
    for &(i, _) in &members {
        let kind = game.party[i].kind.clone();
        let c = kind.character;
        let idle = kind
            .motions
            .get(&0)
            .map_or((60, 0), |d| (d.end(), d.loop_start));
        let member_clip = |id: u16| {
            let d = kind.motions.get(&id)?;
            Some((
                d.end(),
                d.loop_start,
                d.root.iter().map(|r| r.to_array()).collect(),
            ))
        };
        views.push((u32::from(c), view_of(l, c, false, idle, &member_clip)));
        views.push((
            WEAPON_MODEL + u32::from(c),
            view_of(l, c, true, idle, &member_clip),
        ));
    }
    // The bodies: Sora, his Keyblade, the friends and theirs, the enemy.
    let mut sora = Sysobj::bare();
    sora.model = 1;
    sora.team = SORA_TEAM;
    sora.entry_part = u16::from(SORA);
    // The `limitform` program acts only for the Limit Form Sora (L1604:
    // objentry 0x95d..0x961, P_EX100_KH1F and its world variants).
    if l.own {
        sora.entry_id = LIMIT_FORM_OBJECT;
    }
    sora.position = game.sora.player.position;
    sora.yaw = game.sora.player.yaw;
    // His heading turns toward `obj_wish_dir` (+0x7a8) at his PRTY turn
    // speed per tick (the sysobj steps twice its rate).
    sora.turn_rate = game.params.turn_speed * 0.5;
    sora.weapons = vec![WEAPONS];
    let mut others = vec![Participant {
        id: WEAPONS,
        sys: attached(PLAYER_OBJECT, WEAPON_MODEL + 1),
    }];
    let mut friends = Vec::new();
    for (n, &(i, id)) in members.iter().enumerate() {
        let m = &game.party[i];
        let mut s = Sysobj::bare();
        s.model = u32::from(m.kind.character);
        s.team = SORA_TEAM;
        // Objentry +0x4c (FUN_1403b5b20): the character (`obj_search_by_part`).
        s.entry_part = u16::from(m.kind.character);
        s.position = m.follower.position;
        s.yaw = m.follower.yaw;
        let weapon = WEAPONS + 1 + n as ObjId;
        s.weapons = vec![weapon];
        others.push(Participant {
            id: weapon,
            sys: attached(id, WEAPON_MODEL + u32::from(m.kind.character)),
        });
        friends.push(Participant { id, sys: s });
    }
    let enemy = room_enemy(game);
    if let Some(p) = enemy {
        let mut s = Sysobj::bare();
        s.position = p;
        // Enemies are team 2 (FUN_1403ab4a0, FUN_1403d0150 test +0x4dc).
        s.team = 2;
        others.push(Participant { id: ENEMY, sys: s });
        let target = enemy::sysobj::Target {
            id: enemy::traps::target::OBJECT_TARGET | ENEMY,
            kind: -1,
            group: 0,
        };
        sora.target = target;
        // The friends fight the same Shadow (their battle target, obj+0xbf8).
        for f in &mut friends {
            f.sys.target = target;
        }
    }
    let row = game
        .second_deck
        .as_ref()
        .and_then(|d| d.commands.get(&command));
    let (cost, time) = row.map_or((0xff, 180.), |c| (c.cost, f32::from(c.time)));
    let chains: HashMap<u16, (u16, u8, u32)> = game
        .second_deck
        .as_ref()
        .map(|d| {
            d.commands
                .iter()
                .map(|(&id, c)| (id, (c.chain, c.decider, c.flags)))
                .collect()
        })
        .unwrap_or_default();
    let pushers = push_shapes(game);
    let env = RoomEnv {
        world: &game.world,
        body: room_body(game),
        views: &views,
        empty: ClipView::default(),
        stick: [0.; 3],
        enemy,
        player: game.sora.player.position,
        player_yaw: game.sora.player.yaw,
        enemy_reach: game.enemy.as_ref().map(|rt| rt.target_reach),
        selected: game.selected_target.is_some(),
        locked: game.target_locked,
        enemy_radius: room_enemy_radius(game),
        can_command: true,
        chains: &chains,
        pushers: &pushers,
        effects: &HashMap::new(),
        fx_live: &game.fx_live,
        records: &game.clips.records,
        tick,
        fresh_effects: u32::MAX,
    };
    let mut scene = Scene::start(&env, &l.file.program, command, time, sora, friends, others)?;
    // Limit Form's program is his object's: his frame events 7 reach it
    // (motion 247's shots, trigger 0x11).
    scene.player_events = l.own;
    // KH2_LIMIT_TRACE: the end event lists every trap the scene called.
    if std::env::var_os("KH2_LIMIT_TRACE").is_some() {
        scene.trace();
    }
    // FUN_1403d8b40: all of Sora's MP (MP Charge starts), the others fade.
    game.sora
        .player
        .mp
        .change(-i32::from(cost), player::magic::CHARGE_LENGTH);
    game.sora.player.mp_charging = game.sora.player.mp.charging();
    let mut faded = Vec::new();
    for (i, m) in game.party.iter_mut().enumerate() {
        if !m.stored && !members.iter().any(|&(j, _)| j == i) {
            m.stored = true;
            faded.push(i);
        }
    }
    game.event(
        serde_json::json!({"event":"party_limit_start","tick":tick,"command":command,
        "file":l.row.file_name(),"time":time,"cost":cost,
        "members":members.iter().map(|m| m.0).collect::<Vec<_>>(),"faded":faded}),
    );
    game.party_limit = Some(Running {
        scene,
        limit: k,
        views,
        members,
        faded,
        offers: [None; 8],
        motion_serial: 0,
        weapon_axes: None,
        targets: [None; 8],
        effects: HashMap::new(),
        command,
        chains,
        pushers,
        seen: Vec::new(),
        last_reaction: tick,
        hits_seen: game.enemy.as_ref().map_or(0, |rt| rt.hits_landed),
    });
    Ok(())
}

/// One tick of the limit in place of Sora's own update: the command taken
/// (the reaction button on record 1's, the decide button on the Attack row
/// record 0 took), the scene, its events, then Sora and the friends stand
/// and play where the scene put them.
pub fn tick(
    game: &mut Game,
    tick: u64,
    rc: bool,
    decide: Option<usize>,
    stick: V3,
) -> player::Step {
    let Some(mut run) = game.party_limit.take() else {
        return player::Step::default();
    };
    let source = game.party_limits.clone();
    let l = &source.limits[run.limit];
    let enemy = room_enemy(game);
    let can_command = game.sora.player.can_command(&game.clips);
    let status = {
        let env = RoomEnv {
            world: &game.world,
            body: room_body(game),
            views: &run.views,
            empty: ClipView::default(),
            stick,
            enemy,
            player: game.sora.player.position,
            player_yaw: game.sora.player.yaw,
            enemy_reach: game.enemy.as_ref().map(|rt| rt.target_reach),
            selected: game.selected_target.is_some(),
            locked: game.target_locked,
            enemy_radius: room_enemy_radius(game),
            can_command,
            chains: &run.chains,
            pushers: &run.pushers,
            effects: &run.effects,
            fx_live: &game.fx_live,
            records: &game.clips.records,
            tick,
            fresh_effects: run.scene.effect_watermark(),
        };
        if let Some(e) = run.scene.object_mut(ENEMY)
            && let Some(p) = enemy
        {
            e.sys.position = p;
        }
        let record = if rc && run.offers[1].is_some() {
            Some(1)
        } else {
            decide
        };
        // FUN_1403b6950 -> FUN_1403ba070: the command is taken only inside
        // the windows of the motions that decide it.
        let special = run.special_deck();
        let taken = record
            .and_then(|r| match r.checked_sub(SPECIAL) {
                Some(k) => Some((special?.get(k)?.0, None)),
                None => Some((run.offers.get(r).copied().flatten()?, run.targets[r])),
            })
            .filter(|&(cmd, target)| run.scene.can_decide(&env, cmd, target))
            .map(|(cmd, _)| cmd);
        let followed = taken.inspect(|&cmd| run.scene.follow_up(&env, cmd));
        (run.scene.tick(&env), followed)
    };
    let (status, followed) = status;
    if let Some(cmd) = followed {
        game.event(serde_json::json!({"event":"party_limit_follow_up","tick":tick,"command":cmd}));
        // Limit Form's finishers give the form EXP (limit_exec's branch for
        // them, limitform-01 §1.6).
        if l.own
            && player::forms::LIMIT_EXP_COMMANDS.contains(&cmd)
            && let Some(d) = game.drive.as_mut()
        {
            d.pending_exp += 1;
        }
    }
    // The scene re-offers its commands every tick a window is open.
    run.offers = [None; 8];
    run.targets = [None; 8];
    let pax = l.pax.clone();
    let members = run.members.clone();
    let anchor = |o: Option<ObjId>| match o {
        Some(PLAYER_OBJECT) => Some(Anchor::Caster),
        Some(id) => members
            .iter()
            .find(|m| m.1 == id)
            .map(|m| Anchor::Party(m.0 + 1)),
        None => None,
    };
    let mut strikes = Vec::new();
    for e in run.scene.take_events() {
        match e {
            Event::Service {
                name: "command_override",
                words,
                other,
                ..
            } => {
                let r = words[0] as usize;
                if r < run.offers.len() {
                    let cmd = words[1] as u16;
                    if !run.seen.contains(&cmd) {
                        run.seen.push(cmd);
                        let c = run.chains.get(&cmd).copied().unwrap_or_default();
                        game.event(serde_json::json!({"event":"party_limit_offer","tick":tick,
                            "record":r,"command":cmd,"target":other,"chain":c.0,
                            "decider":c.1,"flags":c.2}));
                    }
                    run.offers[r] = Some(cmd);
                    run.targets[r] = other;
                    if r == 1 {
                        run.last_reaction = tick;
                    }
                }
            }
            // Limit Form's: what its program starts on Sora or his Keyblade
            // comes from the form's container (FUN_1403cc650 with Sora's
            // object). [I] One on the Keyblade is placed where it is.
            Event::EffectStart {
                object,
                effect,
                handle,
                position,
                ..
            } if l.own && (object == PLAYER_OBJECT || object == WEAPONS) => {
                let key = FX_KEY | u64::from(handle);
                run.effects.insert(handle, (key, tick));
                let (bound, at) = if object == PLAYER_OBJECT {
                    (position.is_none().then_some(Anchor::Caster), position)
                } else {
                    (
                        None,
                        position.or_else(|| run.scene.object(WEAPONS).map(|o| o.sys.position)),
                    )
                };
                game.fx_calls.push(super::spell_fx::Call {
                    pax: pax.clone(),
                    fx: player::magic::Fx::Start {
                        key,
                        id: effect as u16,
                        bound,
                        at,
                    },
                });
            }
            Event::EffectStart {
                object: LIMIT_EFFECTS,
                effect,
                handle,
                position,
                other,
                ..
            } => {
                let key = FX_KEY | u64::from(handle);
                run.effects.insert(handle, (key, tick));
                game.fx_calls.push(super::spell_fx::Call {
                    pax: pax.clone(),
                    fx: player::magic::Fx::Start {
                        key,
                        id: effect as u16,
                        bound: anchor(other),
                        at: position,
                    },
                });
            }
            Event::EffectCommand {
                trap,
                handle,
                vector,
                ..
            } => {
                let Some(&(key, _)) = run.effects.get(&handle) else {
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
                game.fx_calls.push(super::spell_fx::Call {
                    pax: pax.clone(),
                    fx,
                });
            }
            Event::AttackStrike {
                slot,
                target: Some(ENEMY),
                param,
                serial,
                owner,
                ..
            } => strikes.push(Strike {
                slot,
                param,
                serial,
                owner,
            }),
            // The thrown Keyblade's turn (8:2 after 6:68's angles): the
            // vector came through the position conversion; its raw angles
            // are taken back.
            Event::Service {
                name: "obj_set_xyzrot",
                object: Some(WEAPONS),
                vector: Some(v),
                ..
            } => run.weapon_axes = Some(axes_of([v[0], -v[1], -v[2]])),
            Event::Service { name, words, .. } if name != "player_set_exec_rc" => {
                game.event(
                    serde_json::json!({"event":"party_limit_service","tick":tick,
                    "name":name,"words":&words[..4]}),
                );
            }
            _ => {}
        }
    }
    // Limit Form's follow-ups: a range 20 of the motion Sora plays offers
    // its command as the reaction command (FUN_14040d810 case 0x14), and
    // the press reaches the program's trigger 23.
    if l.own
        && let Some((_, v)) = run.views.iter().find(|(m, _)| *m == 1)
    {
        let p = run.scene.player();
        let slot = p.motion.slot;
        let clock = enemy::motion::native_clock(p.motion.tick, v.length(slot), v.loop_start(slot));
        let offer = v.events(slot).and_then(|e| {
            e.ranges
                .iter()
                .find(|r| {
                    r.kind == 20
                        && u32::from(r.start) <= clock
                        && (r.end == u16::MAX || clock < u32::from(r.end))
                })
                .and_then(|r| r.params.first().copied())
        });
        if let Some(cmd) = offer {
            if !run.seen.contains(&cmd) {
                run.seen.push(cmd);
                game.event(serde_json::json!({"event":"party_limit_offer","tick":tick,
                    "record":1,"command":cmd,"range":20}));
            }
            run.offers[1] = Some(cmd);
            run.last_reaction = tick;
        }
    }
    // Sora stands and plays where the scene has him.
    let proxy = run.scene.player();
    let slot = proxy.motion.slot;
    game.sora.player.position = proxy.position;
    game.sora.player.yaw = proxy.yaw;
    game.sora.player.action = if l.own && slot != 0 {
        // Limit Form's: his own motion by its id.
        match slot {
            1 => Action::Walk,
            2 => Action::Run,
            3 => Action::Jump,
            4 => Action::Fall,
            5 => Action::Land,
            _ => u16::try_from(slot)
                .ok()
                .filter(|&m| game.clips.scripted_clip(m).is_some())
                .map_or(Action::Idle, Action::Scripted),
        }
    } else if slot >= MOTIONS {
        Action::Scripted(clip_id(l.index, (slot - MOTIONS) as u16))
    } else {
        Action::Idle
    };
    // Limit Form's tint: the limit's filter light (2:55) as its program set
    // it (`light_set_color`, L4909), its fade the strength.
    if l.own {
        let world = run.scene.world();
        let light = world
            .light_slot(run.scene.limit().filter)
            .and_then(|k| world.lights.get(k)?.as_ref());
        if let (Some(light), Some(d)) = (light, game.drive.as_mut()) {
            d.tint = [light.color[0], light.color[1], light.color[2]];
            d.tint_k = light.fade.value.clamp(0., 1.);
            d.tint_step = 0.;
        }
    }
    game.sora.player.frame = proxy.motion.tick;
    game.sora.player.armed = true;
    // Each motion the scene starts is a new attack for his swing volumes
    // (their hit records are the motion's, FUN_1403c88c0).
    if proxy.motion.serial != run.motion_serial {
        run.motion_serial = proxy.motion.serial;
        game.action_started = tick;
    }
    for &(i, id) in &run.members {
        let Some(o) = run.scene.object(id) else {
            continue;
        };
        let s = &o.sys;
        let motion = if s.motion.slot >= MOTIONS {
            clip_id(l.index, (s.motion.slot - MOTIONS) as u16)
        } else {
            0
        };
        game.party[i].limit = Some(Scripted {
            motion,
            tick: s.motion.tick,
            serial: s.motion.serial,
            blend: s.motion.blend,
            // The scene's queue is not followed by the member.
            queue: [None; 2],
            position: s.position,
            yaw: s.yaw,
            done: false,
        });
    }
    land_attacks(game, &mut run, &strikes, tick);
    // The combo counter counts every hit on the Shadow while the limit runs.
    if let Some(rt) = game.enemy.as_ref() {
        let landed = rt.hits_landed;
        run.scene
            .count_hits(landed.wrapping_sub(run.hits_seen) as i32);
        run.hits_seen = landed;
    }
    if tick.is_multiple_of(10) {
        let live: Vec<_> = super::enemy_script::script_attacks(run.scene.world())
            .iter()
            .map(|a| serde_json::json!([a.id, a.extra, a.position, a.radius, a.hits_sora]))
            .collect();
        if !live.is_empty() {
            game.event(
                serde_json::json!({"event":"party_limit_attacks","tick":tick,
                "attacks":live,"enemy":room_enemy(game)}),
            );
        }
    }
    if tick.is_multiple_of(30) || status == Status::Over {
        let limit = run.scene.limit();
        let o = run.scene.player();
        let sora_state = (
            o.motion.slot,
            o.motion.tick,
            o.motion.flags.pulses,
            o.motion.flags.ranges,
            o.motion_queue.clone(),
        );
        game.event(
            serde_json::json!({"event":"party_limit_tick","tick":tick,"time":limit.time,
            "pending":limit.pending,"sora_motion":slot,"offers":run.offers,
            "position":game.sora.player.position,"missing":run.scene.missing().len(),
            "friends":run.members.iter().filter_map(|m| run.scene.object(m.1)
                .map(|o| (o.sys.motion.slot, o.sys.motion.tick, o.sys.event_parameter,
                    o.sys.motion.flags.word))).collect::<Vec<_>>(),
            "shots":run.scene.shot_points().len(),
            "sora":sora_state,
            "faults":run.scene.faults().len()}),
        );
    }
    if status == Status::Over {
        end(game, run, tick);
    } else {
        game.party_limit = Some(run);
    }
    player::Step::default()
}

/// A direct strike (`attack_strike`) as its event carried it.
struct Strike {
    slot: usize,
    param: [i32; 2],
    serial: u64,
    owner: Option<ObjId>,
}

/// The scene's attack objects (`attack_new`, 2:10) that can reach the
/// enemies land on the Shadow like a member's: the swept volume against its
/// bodies, the ATKP rows' damage and reaction with the owner's stats (Sora's
/// or the friend's), and the attack's hit flag for `attack_is_hit`. A
/// direct strike (`attack_strike`, 2:17) lands where the Shadow stands.
/// [I] Hit callbacks and hit sparks of the scene's attacks are not run.
fn land_attacks(game: &mut Game, run: &mut Running, strikes: &[Strike], tick: u64) {
    let world = run.scene.world();
    let mut volumes: Vec<_> = super::enemy_script::script_attacks(world)
        .into_iter()
        .filter(|a| !a.hits_sora)
        .map(|a| {
            let owner = world
                .attacks
                .get(a.slot)
                .and_then(|x| x.as_ref())
                .and_then(|x| x.owner);
            let at = (a.position, a.previous, a.radius);
            (a.slot, a.serial, [a.id, a.extra], owner, at)
        })
        .collect();
    if let Some(at) = room_enemy(game) {
        for s in strikes {
            game.event(serde_json::json!({"event":"party_limit_strike","tick":tick,
                "atkp":s.param[0],"serial":s.serial}));
            volumes.push((s.slot, s.serial, s.param, s.owner, (at, None, 1.)));
        }
    }
    for (slot, serial, [id, extra], owner, (centre, previous, radius)) in volumes {
        let member = run.members.iter().find(|m| Some(m.1) == owner).map(|m| m.0);
        let Some(rt) = game.enemy.as_mut() else {
            return;
        };
        let (index, name, stats) = match member {
            Some(i) => (
                i,
                game.party[i].kind.name,
                combat::AttackerStats {
                    strength: game.party[i].kind.strength,
                    magic: game.party[i].kind.magic,
                    max: 99999,
                    min: 1,
                    mods: combat::AttackerMods {
                        player_type: true,
                        ..combat::AttackerMods::default()
                    },
                },
            ),
            None => (usize::MAX, "sora", game.sora.vitals.attacker),
        };
        let key = FX_KEY | 1 << 58 | serial;
        let v = player::magic::Volume {
            key,
            atkp: id as u16,
            sub: extra as u8,
            centre,
            radius,
            height: radius,
            previous,
            moving: None,
            role: player::magic::Role::Damage,
        };
        let facing =
            bevy::math::Vec3::new(game.sora.player.yaw.sin(), 0., game.sora.player.yaw.cos());
        let landed = super::party_room::land_volume(
            rt,
            index,
            name,
            &stats,
            &v,
            key,
            facing,
            tick,
            &mut game.sound_queue,
            &mut game.log,
        );
        if landed.is_some() {
            if member.is_none() {
                rt.last_attacker = super::party_room::Attacker::Sora;
            }
            run.scene.struck(slot);
        }
    }
}

/// FUN_1403d8f40 / FUN_1403d83e0: the friends go back to their own control,
/// the faded members return, Sora stands.
fn end(game: &mut Game, run: Running, tick: u64) {
    for &(i, _) in &run.members {
        game.party[i].limit = None;
    }
    for &i in &run.faded {
        game.party[i].stored = false;
    }
    game.sora.player.action = Action::Idle;
    game.sora.player.frame = 0;
    // FUN_1403d83e0: Limit Form's tint ramps out over 30 frames.
    if player::limit_form::LIMITS.contains(&run.command)
        && let Some(d) = game.drive.as_mut()
    {
        d.tint_step = -1. / player::limit_form::TINT_OUT;
        d.limit_running = false;
    }
    game.event(serde_json::json!({"event":"party_limit_end","tick":tick,"command":run.command,
        "missing":run.scene.missing(),"faults":run.scene.faults().len(),
        "first_faults":run.scene.faults().iter().take(4).map(|f| format!("{f:?}")).collect::<Vec<_>>(),
        "traps":run.scene.trace_counts()}));
}

// --- The limit's HUD -----------------------------------------------------

use visual::limit_hud::{self, ComboCounter, LimitGauge};

/// The limit's gauge (YS::MISSION_GAUGE at +0xe0: right side, kind 0xc) and
/// combo counter (YS::COMBOCOUNTER at +0x88: left side, kind 0xc, range 0 to
/// 999), shown while the limit runs (FUN_1403d96d0). Trinity Limit (0x1e4)
/// and 0x26c show no counter. Each draws from its limit file's sheet.
pub struct LimitHud {
    source: std::sync::Arc<LimitSource>,
    limit: usize,
    gauge: Option<LimitGauge>,
    counter: Option<ComboCounter>,
}

impl LimitHud {
    pub fn start(
        source: std::sync::Arc<LimitSource>,
        limit: usize,
        command: u16,
        time: f32,
        slots: &mut visual::hud_slots::HudSlots,
    ) -> Self {
        let l = &source.limits[limit];
        let gauge = l.gauge_sheet.as_ref().and_then(|s| {
            LimitGauge::new(
                &s.sequence,
                limit_hud::Side::Right,
                limit_hud::LIMIT_KIND,
                time,
                time,
                slots,
            )
        });
        let counter = (command != 0x1e4 && command != 0x26c)
            .then_some(l.counter_sheet.as_ref())
            .flatten()
            .and_then(|s| {
                ComboCounter::new(
                    &s.sequence,
                    limit_hud::Side::Left,
                    limit_hud::LIMIT_KIND,
                    limit_hud::digits_of(999),
                    slots,
                )
            });
        LimitHud {
            source,
            limit,
            gauge,
            counter,
        }
    }

    /// The gauge follows the limit's time (+0x118), the counter its hits.
    pub fn update(&mut self, time: f32, hits: i32, slots: &visual::hud_slots::HudSlots) {
        let l = &self.source.limits[self.limit];
        if let (Some(g), Some(s)) = (self.gauge.as_mut(), l.gauge_sheet.as_ref()) {
            g.set(time.max(0.));
            g.update(&s.sequence, slots);
        }
        if let (Some(c), Some(s)) = (self.counter.as_mut(), l.counter_sheet.as_ref()) {
            c.set(hits);
            c.update(&s.sequence, slots);
        }
    }

    /// The limit file whose sheets the widgets draw from.
    pub fn sheets(&self) -> (std::sync::Arc<LimitSource>, usize) {
        (self.source.clone(), self.limit)
    }

    /// The widgets' draws, the gauge's and the counter's sheets at the
    /// texture indices given.
    pub fn draw(&mut self, gauge_at: usize, counter_at: usize) -> Vec<visual::gauge::Draw> {
        let l = &self.source.limits[self.limit];
        let mut out = Vec::new();
        if let (Some(g), Some(s)) = (self.gauge.as_mut(), l.gauge_sheet.as_ref()) {
            out.extend(g.draw(&s.sequence, gauge_at));
        }
        if let (Some(c), Some(s)) = (self.counter.as_mut(), l.counter_sheet.as_ref()) {
            out.extend(c.draw(&s.sequence, counter_at));
        }
        out
    }

    /// The widgets go and their slots empty (FUN_1403d8f40).
    pub fn end(self, slots: &mut visual::hud_slots::HudSlots) {
        if let Some(g) = &self.gauge {
            g.release(slots);
        }
        if let Some(c) = &self.counter {
            c.release(slots);
        }
    }
}

impl Running {
    /// What the limit's HUD shows: its command, the gauge's time and the
    /// combo counter's hits.
    pub fn hud_values(&self) -> (usize, u16, f32, i32) {
        let l = self.scene.limit();
        (self.limit, self.command, l.time, l.hits)
    }
}
