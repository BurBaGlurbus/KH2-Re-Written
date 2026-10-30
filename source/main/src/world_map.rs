//! `kh2fm wm`: the world map (field area wm00, world 15) from the selected
//! installation. The area loads like any other (map, ARD spawns, map
//! objects); the world symbols and Gummi ships take their looks from the
//! story state (worldmap::Symbol / worldmap::Gumi, the symbol slots' map
//! objects and draw groups), and the cursor (worldmap::Player, `WM_CURSOR`)
//! flies with the stick under worldmap::ObjCamera.
//!
//! Near an open world's symbol the cursor gets the landing reaction and the
//! info panel (dk::WM_LANDING_INFO); the reaction button opens the landing
//! menu (dk::WM_LANDING) under worldmap::CloseUpCamera, and a decided world
//! point ends the program with its target (the jump, FUN_1403946d0).
//!
//! The queued world events play on arrival: messages, the event camera,
//! the objects' refresh and the character scenes (worldmap::ObjWidget),
//! which hide the map while they show.
//!
//! The map's effects run per draw group and the world symbols hold their
//! effects by world state.
//!
//! A decided world point opens the field there (`--field`).
//!
//! The cursor's deck (kind 10) offers the dash, or the warp while R1 holds
//! a lock on a world (worldmap::LockOnCamera); its engine effects follow
//! the floor, the total play time shows until the first menu, and the map
//! fades in from black and out to it on a landing (jump mode 1).
mod effects;
mod scene;
mod ui;

use crate::area::{self, AreaSource, LoadedArea};
use crate::test_room::{BgChunk, BobPart, RoomPart, spawn_bobs, spawn_model, spawn_room};
use ::area::world_state;
use bevy::{
    app::AppExit,
    core_pipeline::tonemapping::{DebandDither, Tonemapping},
    prelude::*,
    render::view::{
        NoIndirectDrawing,
        screenshot::{Screenshot, ScreenshotCaptured},
    },
};
use collision::{Body, Query as Move};
use data::{
    bar,
    model_inspection::{self, InspectionModel, ModelItem, ModelOptions},
    model_inspection_textures::{TextureMode, UvProfile},
    model_pose::PoseProfile,
    motion::{self, InterpolatedMotion},
    object_collision, object_effect,
    path_lookup::Library,
    probe_source::ProbeSource,
};
use player::actor_physics::{self, ObjParams, Sampler};
use std::{collections::HashMap, fs::OpenOptions, path::PathBuf, sync::Arc};
use visual::area_fog::{FogMaterial, FogPlugin};
use visual::field_command_menu::{self, CommandMenu, Font, Item, Reaction};
use visual::overlay::{Ui2dFrame, Ui2dPlugin};
use visual::world_map_menu::{self, Info, Menu, RouteInfo, TotalInfo};
use visual::{controller, input, lock_on};
use world_map::{
    camera::{self as wm_camera, LockOnCamera, ObjCamera, Pose},
    cursor::{self, Candidate, WarpPath},
    events,
    landing::{self, Landing, Outcome},
    objects::{self, Switch},
    widgets,
};

type Result<T> = std::result::Result<T, Box<dyn std::error::Error>>;

const TICK_HZ: f64 = 60.;
/// The cursor's motions: idle and the fast one worldmap::ActionShip picks
/// above the mean of the walk and run speeds (FUN_140563880), ActionDash's
/// and ActionWarp's.
const CURSOR_IDLE: u16 = cursor::MOTION_IDLE;
const CURSOR_FAST: u16 = cursor::MOTION_FAST;
/// Op 0x110's motion (FUN_1403c86a0).
const CURSOR_RIDE: u16 = 0x9c;
/// The symbols' motions (FUN_140390510).
const SYMBOL_CLOSED: u16 = 0x97;
const SYMBOL_OPEN: u16 = 0xa1;
/// The story range the program sets by default: Sora's first Twilight Town
/// visit, as the field tests use [test-room choice].
const DEFAULT_STORY: (u32, u32) = (0x800, 0x895);
/// Unattended runs: a fixed input script and the ticks captured.
const AUTOPLAY_CAPTURES: [u64; 14] = [
    40, 130, 190, 270, 333, 360, 430, 560, 700, 790, 815, 860, 960, 1000,
];
const AUTOPLAY_END: u64 = 1060;
const WARM_UP_FRAMES: u32 = 90;
/// The map camera's near plane.
const MAP_NEAR: f32 = 5.;

/// What a world map object is (FUN_140393f50 by objentry sub-kind).
#[derive(Clone, Copy)]
enum Kind {
    Cursor,
    Symbol(objects::SymbolEntry),
    Gumi(u8),
}

struct Actor {
    kind: Kind,
    /// None for the objects whose archive holds no model.
    model: Option<Arc<InspectionModel>>,
    /// Motions by the exe's motion number (FUN_1403c6dc0) and their frame
    /// triggers.
    motions: MotionMap,
    triggers: TriggerMap,
    /// Engine coordinates (Y down) and engine yaw.
    position: [f32; 3],
    yaw: f32,
    motion: u16,
    /// Ticks since the motion started.
    time: f32,
    drawn: bool,
    tint: [f32; 4],
    /// The model's object-collision shapes (BAR type 23).
    shapes: Vec<object_collision::Entry>,
    /// A symbol that offers its reaction (state ≥ 2, record flag bit 1
    /// clear: FUN_140390510's flag 0x40 is off).
    reacts: bool,
    /// The object's own effects (its `.a.us` PAX, BAR type 18).
    pax: Option<Arc<object_effect::Pax>>,
    /// The effects a symbol holds (+0xd58, +0xd60, +0xd70) and the
    /// new-episode one (+0xd68), as effect handles.
    held: [Option<u64>; 3],
    episode_effect: Option<u64>,
    /// Mesh parts hidden by frame triggers 26 / 27 (bit per part) and the
    /// motion controller's last tick with its motion (FUN_14040e960).
    hidden_parts: u64,
    clock: Option<(u16, f32)>,
    /// The world record (state, flags) a symbol last applied (+0xd38), or
    /// the slot state a ship last applied (+0xd40).
    applied: (u8, u8),
    /// The blend of the last motion change (FUN_1403c86a0's third
    /// argument, in ticks; 2 or less switches at once).
    blend: f32,
}

/// worldmap::Player's action (FUN_1403b4da0 with the table 0x140750950).
enum Action {
    /// 0x2f ActionShip.
    Ship,
    /// 0x3b ActionDash.
    Dash,
    /// 0x3a ActionWarp and its path (+0xe08).
    Warp(WarpPath),
}

#[derive(Component)]
struct ActorRoot(usize);
#[derive(Component)]
struct ActorPart {
    actor: usize,
    part: usize,
}
#[derive(Component)]
struct MapCamera;
/// The map (room, map objects and world map objects), hidden while
/// widgets show (FUN_1401625f0).
#[derive(Component)]
struct MapRoot;
/// A widget model (an index into `WorldMap::widget_models`) and its parts.
#[derive(Component)]
struct WidgetRoot(usize);
#[derive(Component)]
struct WidgetPart {
    model: usize,
    part: usize,
}

/// An object model a script puts in a widget slot.
struct WidgetModel {
    slot: usize,
    object: u32,
    model: Arc<InspectionModel>,
}

#[derive(Resource)]
struct WorldMap {
    actors: Vec<Actor>,
    cursor: usize,
    /// obj+0x20 (FUN_1403b7ad0's velocity, X and Z) and the physics
    /// sampler (FUN_1403b89a0).
    velocity: [f32; 2],
    sampler: Sampler,
    params: ObjParams,
    camera: ObjCamera,
    pose: Pose,
    /// The World That Never Was symbol while ObjCamera uses it.
    eh_symbol: Option<[f32; 3]>,
    collision: Option<collision::World>,
    body: Body,
    /// The map's draw groups (DAT_140abcbd8) after the dark set, symbols
    /// and slots, and the map objects the slots and ships hide.
    bg_mask: u64,
    hidden_bobs: Vec<bool>,
    tick: u64,
    /// Frames drawn; the map holds its first tick until a few have shown
    /// (the window's first frames build the render pipelines).
    frames: u32,
    stick: [f32; 3],
    /// The right stick (x right, y down) and the raw bits of the last tick,
    /// for the lock switch's flicks (FUN_14039b760(0) +8 & 0xf0000).
    right: [f32; 3],
    last_raw: u64,
    autoplay: Option<PathBuf>,
    /// Unattended runs decide the landing menu instead of cancelling it.
    autoplay_decide: bool,
    /// Where the landing's field comes from (`--field`).
    field: Option<FieldHandoff>,
    /// `07localset.bin`: per world, program id → area (FUN_14039fbc0).
    program_areas: Vec<Vec<(u16, u16)>>,
    captures_taken: usize,
    capture_pending: bool,
    log: Vec<serde_json::Value>,
    /// Raw PADDATA bits of this tick and the pad record (FUN_14039c4c0).
    raw: u64,
    pad: controller::Pad,
    /// The symbol actor whose reaction the cursor has (FUN_140407de0) and
    /// the 'cmd' id its shape names.
    reaction: Option<usize>,
    reaction_cmd: Option<u16>,
    landing: Option<(Landing, Menu, usize)>,
    info: Option<Info>,
    /// dk::WM_GLANDING_INFO on the reacting (or chosen) Gummi route.
    route_info: Option<RouteInfo>,
    /// The landing's target once a world point is decided.
    jump: Option<world_state::Target>,
    /// 03system 'cmd': each command's execute id (FUN_1403b68b0).
    commands: HashMap<u16, u16>,
    /// Sound requests of this tick (FUN_1401de0c0 ids: se000 1 cursor,
    /// 2 decide, 3 open, 4 cancel, 5 error).
    sounds: Vec<u32>,
    /// The world events still to play (FUN_14038dc60) and the one playing
    /// with its event camera (camera slot 2) and message window.
    event_queue: std::collections::VecDeque<u32>,
    event: Option<events::EventPlayer>,
    event_camera: Option<wm_camera::EventCamera>,
    window: Option<visual::dialogue::TalkWindow>,
    fade: visual::fade::Fade,
    /// The world state the objects show after the queued events' scripts
    /// (op 0x105 / 0x113 refresh to it), the tables and the eh flag.
    after_world: Option<world_state::WorldState>,
    tables: Option<world_state::WorldTables>,
    eh_flag: bool,
    /// Unattended runs start their script once the events are over.
    event_ticks: u64,
    /// The character scene (the event object's widget slots and the
    /// shared view), the events' archives and the widget models.
    scene: widgets::Scene,
    archives: HashMap<u32, scene::EventArchive>,
    widget_models: Vec<WidgetModel>,
    /// Op 0x108 requests of this tick: (event, archive entry).
    voices: Vec<(u32, u32)>,
    /// The map's own effects (its PAX), the object effect starts and stops
    /// of this tick, the next effect handle and the sounds to stop.
    map_pax: Option<Arc<object_effect::Pax>>,
    fx_requests: Vec<FxRequest>,
    next_fx: u64,
    sound_stops: Vec<u32>,
    /// The worlds that list an unlocked episode still marked new, and
    /// whether the arrival's events task has ended (FUN_14038de20).
    new_worlds: Vec<u8>,
    events_ended: bool,
    /// Op 0x110: the cursor rides Hollow Bastion's symbol.
    riding: bool,
    /// worldmap::Player's action, its dash speed (PRTY +0x24) and the
    /// manual lock's range (SSTM +0x4c).
    action: Action,
    dash_speed: f32,
    lock_range: f32,
    /// The held target (lock manager state 2) and worldmap::LockOnCamera
    /// (camera slot 1).
    lock: Option<Candidate>,
    lock_camera: Option<LockOnCamera>,
    /// The cursor's effect holders: +0xe60 the dash's, +0xe68 / +0xe70 the
    /// floor's (FUN_140394900), as effect handles.
    cursor_fx: [Option<u64>; 3],
    /// The floor material under the cursor (obj+0x780).
    floor: u8,
    /// 03system 'cmd' texts and flags (+8, +0xc) by command id (the
    /// reaction's label and FUN_14015d670's flag bit 3).
    command_texts: HashMap<u16, (u32, u32)>,
    /// The cursor's deck on the field command menu (dk::COMMAND_DRAW on
    /// the world map's 'comw' layout) and the lock-on reticle (the field
    /// 2D 'fiel' sheet).
    command: Option<CommandMenu>,
    reticle: lock_on::Reticle,
    reticle_draw: Option<visual::gauge::Draw>,
    /// dk::WM_TOTAL_INFO and the play time it shows (save +0x2444, ticks).
    total: Option<TotalInfo>,
    play_time: u32,
    /// A decided landing waits for its fade to black (FUN_140156520 mode 1,
    /// FUN_1401563f0) before the jump.
    leaving: Option<world_state::Target>,
    /// FUN_1403a92c0's map-attribute contact: the last wall polygon taken
    /// (+0x37c, 0xffff none), the ticks it stays latched (+0x37d) and the
    /// knock-slide the contact gave (obj+0xd24).
    wall: (u16, f32),
    knock: Option<actor_physics::Knock>,
    /// The camp or journal holds the screen (the map is not stepped), the
    /// landing grid asked for the journal, and the input devices last used
    /// (keyboard, pad glyph) for the menus' prompts.
    camp_up: bool,
    journal_request: bool,
    devices: (bool, Option<u8>),
    /// Each world's play time (save +0x244c + 4 × world, ticks): the
    /// world map's own (15) counts while it runs (FUN_1401574e0).
    world_times: [u32; 19],
}

/// The camp (pause) menu and the journal the world map hosts, from the
/// field configuration (`--field`).
#[derive(Resource, Default)]
struct CampHost(Option<crate::test_room::world_map_camp::WorldMapCamp>);

/// An object effect start or stop for the effects tick.
enum FxRequest {
    /// `follow`: a bound element (flag bit 0) started by a motion keeps
    /// to its object.
    Object {
        handle: u64,
        actor: usize,
        id: u16,
        follow: bool,
    },
    Stop(u64),
    /// FUN_1403f1f00 / FUN_1403f1ef0: the element ends as its program does.
    Release(u64),
    /// The object's motion changed (FUN_1403c7300).
    ReleaseMotion(usize),
}

impl WorldMap {
    /// FUN_1403f1f30 → FUN_1403b6670 on the cursor: a bound element keeps
    /// to it as it flies.
    fn start_cursor_effect(&mut self, id: u16) -> u64 {
        self.next_fx += 1;
        let handle = self.next_fx;
        self.fx_requests.push(FxRequest::Object {
            handle,
            actor: self.cursor,
            id,
            follow: true,
        });
        handle
    }

    fn start_effect(&mut self, actor: usize, id: u16) -> u64 {
        self.next_fx += 1;
        let handle = self.next_fx;
        self.fx_requests.push(FxRequest::Object {
            handle,
            actor,
            id,
            follow: false,
        });
        handle
    }
}

/// The sound output, the system bank and the world map's field BGM.
#[derive(Resource)]
struct AudioRes {
    output: Option<audio::Output>,
    banks: visual::sound::Banks,
    music: Option<visual::sound::MusicSlot>,
    bgm_started: bool,
    /// The events' voices by (event, archive entry).
    voices: HashMap<(u32, u32), audio::Clip>,
}

const BGM_KEY: u32 = 0xb6_0000;
/// One voice plays at a time (thunk_FUN_1401df2f0 stops the last).
const VOICE_KEY: u32 = 0xb7_0000;

/// The 2D assets and the sheets they were uploaded to.
#[derive(Resource)]
struct UiRes {
    ui: ui::Ui,
    images: Vec<usize>,
    pictures: Vec<usize>,
    font: usize,
    icon: usize,
    /// The message window art, the event font and a white texel (fades).
    window: Option<usize>,
    event_font: Option<usize>,
    white: usize,
    /// The command layout's and the field 2D sheet's slots.
    command: Option<usize>,
    fiel: Option<usize>,
}

struct Startup {
    room: visual::room::LoadedRoom,
    area: LoadedArea,
    /// FUN_1403a5a80 for the map.
    glow: bool,
}

#[derive(Resource)]
struct StartupRes(Option<Startup>);

fn model_options() -> ModelOptions {
    ModelOptions {
        pose_profiles: vec![PoseProfile::Legacy],
        textures: true,
        texture_modes: vec![TextureMode::OriginalIndexed],
        uv_profiles: vec![UvProfile::Kkdf2IntegerTruncated],
    }
}

/// FUN_1403c7c50: a motion set whose BAR header word +0xc has bit 0 set
/// holds four variants per motion number; the world map's sets do not, so
/// the number is the entry index. Each motion comes with its frame triggers
/// (the ANB's type-16 entry): (60 Hz tick, trigger, parameters).
fn read_motions(mset: &[u8], ids: &[u16]) -> Result<(MotionMap, TriggerMap)> {
    let table = bar::read_bar(mset)?;
    let variants = table.header_field_12 & 1 != 0;
    let mut out = HashMap::new();
    let mut triggers = HashMap::new();
    for &id in ids {
        let present = |slot: usize| {
            table
                .entries
                .get(slot)
                .is_some_and(|e| e.offset > 0 && e.size > 0)
        };
        let slot = if variants {
            motion::motion_set_index(present, id, false, false)
        } else {
            present(id as usize).then_some(id as usize)
        };
        let Some(slot) = slot else { continue };
        let e = &table.entries[slot];
        let anb = &mset[e.offset as usize..e.offset as usize + e.size as usize];
        let sel = motion::read_anb(anb, slot)?;
        let list: Vec<Trigger> = sel
            .triggers
            .iter()
            .flat_map(|t| &t.records.frames)
            .map(|f| (f.frame as f32, f.trigger, f.parameters.clone()))
            .collect();
        triggers.insert(id, Arc::new(list));
        out.insert(id, Arc::new(sel.motion));
    }
    Ok((out, triggers))
}

/// A frame trigger: its tick, kind and parameters.
type Trigger = (f32, u8, Vec<i16>);
type MotionMap = HashMap<u16, Arc<InterpolatedMotion>>;
type TriggerMap = HashMap<u16, Arc<Vec<Trigger>>>;

/// An object's model, or none when its archive holds only collision (the
/// Gummi route objects). A texture set the reader cannot decode yet leaves
/// the model untextured; the reason is returned for the run's log.
fn read_model(
    source: &ProbeSource,
    library: &mut Library,
    name: &str,
) -> Result<(Option<InspectionModel>, Option<String>)> {
    let found = library
        .find(&format!("obj/{name}.mdlx"))
        .ok_or_else(|| format!("obj/{name}.mdlx is not installed"))?;
    let archive = library.read_found(&found)?;
    if !bar::read_bar(&archive)?
        .entries
        .iter()
        .any(|e| e.entry_type == 4)
    {
        return Ok((None, None));
    }
    let items = [ModelItem {
        index: found.index.into(),
        ordinal: found.ordinal,
        name_hash: found.name_hash,
    }];
    let mut options = model_options();
    let (mut models, untextured) = match model_inspection::read_models(source, &items, &options) {
        Ok(models) => (models, None),
        Err(e) => {
            options.textures = false;
            (
                model_inspection::read_models(source, &items, &options)?,
                Some(e.to_string()),
            )
        }
    };
    Ok(((!models.is_empty()).then(|| models.remove(0)), untextured))
}

/// How a mesh group draws, from its header word 0: bit 0 puts it in the
/// translucent pass and bit 21 makes that pass additive. [I: the bit
/// meanings follow the community model layout; the PC renderer's reader is
/// not traced yet. The world symbols' glow cards set both.]
fn blend(attributes: u32) -> AlphaMode {
    if attributes & 1 == 0 {
        AlphaMode::Opaque
    } else if attributes >> 21 & 1 != 0 {
        AlphaMode::Add
    } else {
        AlphaMode::Blend
    }
}

/// An object's own effects: the PAX (BAR type 18) of its `obj/<name>.a.us`.
fn read_object_pax(library: &mut Library, name: &str) -> Option<object_effect::Pax> {
    object_effect::read_file_pax(library, &format!("obj/{name}.a.us"))
}

/// An object's collision shapes (its BAR type-23 entry).
fn read_shapes(archive: &[u8]) -> Vec<object_collision::Entry> {
    bar::read_bar(archive)
        .ok()
        .and_then(|table| {
            let e = table.entries.iter().find(|e| e.entry_type == 23)?;
            let bytes = archive.get(e.offset as usize..e.offset as usize + e.size as usize)?;
            object_collision::read(bytes).ok()
        })
        .unwrap_or_default()
}

/// The map body of an object: its object-collision entry of kind 4 (the
/// cursor's is the same as Sora's: r 90, h 90, centre 115 up).
fn map_body(shapes: &[object_collision::Entry]) -> Option<Body> {
    let s = shapes.iter().find(|s| s.kind == 4)?;
    Some(Body {
        radius: s.radius as f32,
        height: s.height as f32,
        offset: s.position_height as f32,
        mode: 1,
        class: 0,
    })
}

/// Applies the world state to the actors, the draw groups and the map
/// objects, in the order the area start runs it: the dark set
/// (FUN_1403f0b70), the objects' looks (FUN_140390510, FUN_140397490,
/// FUN_14039a410) and the symbol slots (FUN_14038df20).
fn apply_world_state(
    wm: &mut WorldMap,
    state: &world_state::WorldState,
    tables: Option<&world_state::WorldTables>,
    eh_flag: bool,
    bobs: usize,
) {
    let mut switches = Vec::new();
    if let Some(set) = tables.and_then(|t| t.draw_group_set(state.draw_group_set as i32)) {
        for bit in 0..64 {
            if set.on >> bit & 1 != 0 {
                switches.push(Switch::Group {
                    group: bit,
                    on: true,
                });
            } else if set.off >> bit & 1 != 0 {
                switches.push(Switch::Group {
                    group: bit,
                    on: false,
                });
            }
        }
    }
    wm.eh_symbol = None;
    for i in 0..wm.actors.len() {
        // The held effects: a slot's element starts when the slot is empty
        // and goes when the look wants none (FUN_140390510).
        if let Kind::Symbol(entry) = wm.actors[i].kind {
            let record = state.record(entry.world).copied().unwrap_or_default();
            let look = objects::symbol_look(entry.world, &record, eh_flag);
            for (slot, want) in objects::held_effects(&look).into_iter().enumerate() {
                match (want, wm.actors[i].held[slot]) {
                    (Some(id), None) => {
                        let h = wm.start_effect(i, id);
                        wm.actors[i].held[slot] = Some(h);
                    }
                    (None, Some(h)) => {
                        wm.fx_requests.push(FxRequest::Stop(h));
                        wm.actors[i].held[slot] = None;
                    }
                    _ => {}
                }
            }
            wm.actors[i].applied = (record.state, record.flags);
        }
        if let Kind::Gumi(route) = wm.actors[i].kind {
            wm.actors[i].applied = (state.symbol_slot(route), 0);
        }
        let a = &mut wm.actors[i];
        match a.kind {
            Kind::Cursor => {}
            Kind::Symbol(entry) => {
                let record = state.record(entry.world).copied().unwrap_or_default();
                let look = objects::symbol_look(entry.world, &record, eh_flag);
                a.drawn = look.drawn;
                a.reacts = look.drawn && look.reacts;
                a.tint = look.tint.map(|c| c as f32 / 128.);
                if a.motion != look.motion {
                    a.motion = look.motion;
                    a.time = 0.;
                }
                if entry.group != 0 {
                    switches.push(Switch::Group {
                        group: entry.group,
                        on: look.group_on,
                    });
                }
                // FUN_140397c70: the eh symbol bends the camera once its
                // world is open and its record flag bit 1 is clear.
                if entry.world == 0x12 && record.state >= 2 && record.flags & 2 == 0 {
                    wm.eh_symbol = Some(a.position);
                }
            }
            Kind::Gumi(route) => {
                let look = objects::gumi_look(route, state);
                a.drawn = look.drawn;
                switches.extend(look.switches);
            }
        }
    }
    switches.extend(objects::slot_switches(state));
    wm.hidden_bobs = vec![false; bobs];
    for s in switches {
        match s {
            Switch::Group { group, on } if group < 64 => {
                if on {
                    wm.bg_mask |= 1 << group;
                } else {
                    wm.bg_mask &= !(1 << group);
                }
                if let Some(c) = wm.collision.as_mut() {
                    c.set_group(group, on);
                }
            }
            Switch::Group { .. } => {}
            // FUN_14016ec50 / FUN_14016f3a0 count the drawn map objects in
            // list order; here the area's map-object placements [I].
            Switch::Object { index, shown } => {
                if let Some(h) = wm.hidden_bobs.get_mut(index as usize) {
                    *h = !shown;
                }
            }
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn setup(
    mut commands: Commands,
    mut startup: ResMut<StartupRes>,
    wm: Res<WorldMap>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<FogMaterial>>,
    mut images: ResMut<Assets<Image>>,
) {
    let Startup { room, area, glow } = startup.0.take().expect("startup data");
    let root = commands
        .spawn((Transform::default(), Visibility::default(), MapRoot))
        .id();
    // The widget models, outside the map so that hiding it leaves them.
    for (i, w) in wm.widget_models.iter().enumerate() {
        let widget_root = commands
            .spawn((Transform::default(), Visibility::Hidden, WidgetRoot(i)))
            .id();
        let pose: Vec<Vec3> = w.model.poses[0]
            .position_bits
            .iter()
            .map(|v| Vec3::from_array(v.map(f32::from_bits)))
            .collect();
        for (part, entity) in spawn_model(
            &mut commands,
            &w.model,
            &pose,
            widget_root,
            &mut meshes,
            &mut materials,
            &mut images,
        ) {
            if let Some(e) = entity {
                commands.entity(e).insert(WidgetPart { model: i, part });
            }
        }
    }
    spawn_room(
        &mut commands,
        &mut meshes,
        &mut materials,
        &mut images,
        &room,
        root,
        false,
        wm.bg_mask,
    );
    spawn_bobs(
        &mut commands,
        &mut meshes,
        &mut materials,
        &mut images,
        &area,
        root,
        wm.bg_mask,
    );
    for (i, a) in wm.actors.iter().enumerate() {
        let Some(model) = &a.model else { continue };
        let actor_root = commands
            .spawn((
                ChildOf(root),
                Transform::default(),
                Visibility::Hidden,
                ActorRoot(i),
            ))
            .id();
        let pose: Vec<Vec3> = model.poses[0]
            .position_bits
            .iter()
            .map(|v| Vec3::from_array(v.map(f32::from_bits)))
            .collect();
        for (part, entity) in spawn_model(
            &mut commands,
            model,
            &pose,
            actor_root,
            &mut meshes,
            &mut materials,
            &mut images,
        ) {
            if let Some(e) = entity {
                commands.entity(e).insert(ActorPart { actor: i, part });
            }
        }
    }
    commands.insert_resource(BobState(area.bobs));
    commands.spawn((
        Projection::Perspective(PerspectiveProjection {
            fov: horizontal_to_vertical(world_map::camera::FOV_DEGREES.to_radians()),
            near: MAP_NEAR,
            far: 200_000.,
            ..default()
        }),
        Transform::default(),
        Msaa::Off,
        Tonemapping::None,
        DebandDither::Disabled,
        NoIndirectDrawing,
        visual::effect_draw::scene_compositing(),
        visual::glow::camera(glow, None),
        MapCamera,
        RoomPart,
    ));
}

/// Uploads the 2D sheets: the layout's images, the fonts and the
/// world-point pictures.
fn setup_ui(
    mut ui: ResMut<UiRes>,
    mut frame: ResMut<Ui2dFrame>,
    mut images: ResMut<Assets<Image>>,
    mut camp: ResMut<CampHost>,
) {
    if let Some(c) = camp.0.as_mut() {
        c.upload(&mut frame, &mut images);
    }
    let ui = &mut *ui;
    for k in 0..ui.ui.layout.image_sizes.len() {
        if let Some(t) = ui.ui.layout.replacements.get_mut(k).and_then(Option::take) {
            ui.images
                .push(frame.add_sheet(&mut images, t.rgba, t.size, t.texel_scale));
        }
    }
    for t in std::mem::take(&mut ui.ui.pictures) {
        ui.pictures
            .push(frame.add_sheet(&mut images, t.rgba, t.size, t.texel_scale));
    }
    let f = &mut ui.ui.font;
    ui.font = frame.add_sheet(
        &mut images,
        std::mem::take(&mut f.rgba),
        f.size,
        f.texel_scale as f32,
    );
    let i = &mut ui.ui.icons;
    ui.icon = frame.add_sheet(
        &mut images,
        std::mem::take(&mut i.rgba),
        i.size,
        i.texel_scale as f32,
    );
    if let Some(w) = ui.ui.window.as_mut() {
        ui.window = Some(frame.add_sheet(
            &mut images,
            std::mem::take(&mut w.texture_rgba),
            w.texture_size,
            w.texel_scale as f32,
        ));
    }
    if let Some(e) = ui.ui.event_font.as_mut() {
        ui.event_font = Some(frame.add_sheet(
            &mut images,
            std::mem::take(&mut e.rgba),
            e.size,
            e.texel_scale as f32,
        ));
    }
    ui.white = frame.add_sheet(&mut images, vec![255; 4], [1, 1], 1.);
    if let Some((_, c)) = ui.ui.command.as_mut() {
        ui.command = Some(frame.add_sheet(
            &mut images,
            std::mem::take(&mut c.texture_rgba),
            c.texture_size,
            c.texel_scale as f32,
        ));
    }
    if let Some((_, f)) = ui.ui.field.as_mut() {
        ui.fiel = Some(frame.add_sheet(
            &mut images,
            std::mem::take(&mut f.texture_rgba),
            f.texture_size,
            f.texel_scale as f32,
        ));
    }
}

/// The map's draw groups as they change (FUN_140162a00 / FUN_1401629b0):
/// room chunks and map objects whose group is off hide.
fn apply_draw_groups(wm: Res<WorldMap>, mut parts: Query<(&BgChunk, &mut Visibility)>) {
    for (chunk, mut v) in &mut parts {
        let want = if visual::room::chunk_visible(chunk.0, wm.bg_mask) {
            Visibility::Inherited
        } else {
            Visibility::Hidden
        };
        if *v != want {
            *v = want;
        }
    }
}

/// The map objects and their motions, kept for the presentation.
#[derive(Resource)]
struct BobState(visual::scenery::LoadedBobs);

/// The game's field of view is horizontal on a 640x480 screen (as the
/// follow camera's, FollowCamera::vertical_fov).
fn horizontal_to_vertical(fov: f32) -> f32 {
    let focal = 320. / (fov * 0.5).tan();
    2. * (240. / focal).atan()
}

fn read_stick(
    keys: Res<ButtonInput<KeyCode>>,
    mouse: Res<ButtonInput<MouseButton>>,
    pads: Query<(&Gamepad, Option<&Name>)>,
    controls: Option<Res<input::Controls>>,
    mut wm: ResMut<WorldMap>,
) {
    if wm.autoplay.is_some() {
        return;
    }
    let default = input::Controls::default();
    let controls = controls.as_deref().unwrap_or(&default);
    let (data, keyboard, glyph) = input::read_devices(&keys, &mouse, 0., &pads, controls);
    wm.devices = (keyboard, glyph);
    wm.stick = controller::analog(data.left);
    wm.right = controller::analog(data.right);
    wm.raw = controller::raw(&data);
}

/// Scripted input for unattended runs: the landing menu at the start
/// (open, a grid step, the episode list and back, then cancel or decide),
/// then a flight north, east and south-west, back toward the start; then
/// R1 locks on, the back button warps, R1 releases and the back button
/// dashes north.
fn autoplay_input(tick: u64, decide: bool, route: bool) -> ([f32; 3], u16) {
    use controller::button;
    let press = |at: u64, b: u16| if tick == at || tick == at + 1 { b } else { 0 };
    // In the grid, Triangle opens the journal and the back button closes
    // it again; a Gummi route's menu has no journal, so the back button
    // would close the menu before the decide.
    let journal = if route {
        0
    } else {
        press(320, button::TRIANGLE) | press(340, button::CIRCLE)
    };
    let buttons = press(60, button::TRIANGLE)
        | press(150, button::RIGHT)
        | press(210, button::R1)
        | press(290, button::L1)
        | journal
        | press(
            370,
            if decide {
                button::CROSS
            } else {
                button::CIRCLE
            },
        );
    let stick = match tick {
        440..560 => [0., -1., 1.],
        560..680 => [1., 0., 1.],
        680..760 => [-0.7, 0.7, 1.],
        760..775 => [-0.7, 0.7, 1.],
        900..945 => [0., -1., 1.],
        _ => [0., 0., 0.],
    };
    // After the flight (the menu is closed by then).
    // Then START opens the camp, the cursor moves and START closes it.
    let late = press(780, button::R1)
        | press(800, button::CIRCLE)
        | press(880, button::R1)
        | press(900, button::CIRCLE)
        | press(965, button::START)
        | press(985, button::DOWN)
        | press(1010, button::START);
    (stick, if tick >= 440 { late } else { buttons })
}

fn wrap_angle(a: f32) -> f32 {
    ::area::wrap(a)
}

/// A shape's world centre (FUN_1403c9c00) on an actor.
fn shape_at(a: &Actor, s: &object_collision::Entry) -> [f32; 3] {
    landing::shape_centre(
        a.position,
        a.yaw,
        s.position.map(|v| v as f32),
        s.position_height as f32,
    )
}

/// FUN_140407de0 for the world map: the symbols and Gummi route objects
/// whose kind-5 sphere meets the cursor's kind-1 sphere offer their
/// command; the nearest wins (equal priority 100, score = distance).
fn find_reaction(wm: &WorldMap) -> Option<(usize, u16)> {
    let cursor = &wm.actors[wm.cursor];
    let probe = cursor.shapes.iter().find(|s| s.kind == 1 && s.shape == 3)?;
    let at = shape_at(cursor, probe);
    let mut best: Option<(f32, usize, u16)> = None;
    for (i, a) in wm.actors.iter().enumerate() {
        // A symbol offers landing (exec 0x9f) unless its flag 0x40 is on;
        // a route object its route (exec 0x206) while drawn.
        let exec = match a.kind {
            Kind::Symbol(_) if a.reacts => landing::LAND_EXEC,
            Kind::Gumi(_) if a.drawn => landing::ROUTE_EXEC,
            _ => continue,
        };
        // The shape's argument names a 'cmd' entry (FUN_140394fc0).
        for s in a.shapes.iter().filter(|s| {
            s.kind == object_collision::REACTION
                && s.shape == 3
                && wm.commands.get(&(s.argument as u16)) == Some(&exec)
        }) {
            let c = shape_at(a, s);
            if landing::spheres_touch(c, s.radius as f32, at, probe.radius as f32) {
                let d = c
                    .iter()
                    .zip(at)
                    .map(|(p, q)| (p - q) * (p - q))
                    .sum::<f32>()
                    .sqrt();
                if best.is_none_or(|(bd, _, _)| d < bd) {
                    best = Some((d, i, s.argument as u16));
                }
            }
        }
    }
    best.map(|(_, i, c)| (i, c))
}

/// The symbol's world (Symbol actors only).
fn actor_world(a: &Actor) -> Option<u8> {
    match a.kind {
        Kind::Symbol(e) => Some(e.world),
        _ => None,
    }
}

/// The route object's route (FUN_1403973f0: its spawn argument 1).
fn actor_route(a: &Actor) -> Option<u8> {
    match a.kind {
        Kind::Gumi(r) => Some(r),
        _ => None,
    }
}

/// FUN_140396e50 → FUN_140396b80: the close-up of the landing's symbol for
/// the menu mode, from its kind-7 shape, at the current camera's
/// projection distance [I: 320 / tan(fov / 2), the 640-wide screen of the
/// follow camera's conversion].
fn close_up(a: &Actor, mode: u8, now: Pose) -> Pose {
    let offset = wm_camera::CLOSE_UP[match mode {
        landing::ROWS => 0,
        landing::EPISODES => 2,
        _ => 1,
    }];
    let (centre, radius, height) = match a.shapes.iter().find(|s| s.kind == 7) {
        Some(s) => (shape_at(a, s), s.radius as f32, s.height as f32),
        None => (a.position, 0., 0.),
    };
    let focal = 320. / (now.fov * 0.5).tan();
    let mut pose = wm_camera::close_up(centre, radius, height, offset, focal);
    if matches!(a.kind, Kind::Symbol(e) if e.object == 0x456) {
        pose.fov = wm_camera::CLOSE_UP_FOV_EH;
    }
    pose
}

/// The landing reaction, the info panel and the landing menu for one tick
/// (FUN_140394810 / FUN_140394fc0 / FUN_1403931f0). True while the menu
/// holds the cursor.
fn landing_tick(wm: &mut WorldMap, ui: &UiRes) -> bool {
    let layout = &ui.ui.layout.layout;
    let seq = &layout.sequences[world_map_menu::SEQUENCE];
    if let Some((state, menu, symbol)) = wm.landing.as_mut() {
        let symbol = *symbol;
        let pressed = wm.pad.pressed;
        let repeat = wm.pad.repeat;
        let mut mode_changed = false;
        if menu.ready(state) {
            let (sounds, changes) = state.input(pressed, repeat);
            wm.sounds.extend(sounds.iter().map(|&s| s as u32));
            mode_changed = changes
                .iter()
                .any(|c| matches!(c, landing::Change::Scene { .. }));
            if changes.contains(&landing::Change::Journal) {
                wm.journal_request = true;
            }
            menu.apply(layout, state, &changes);
            if !sounds.is_empty() || !changes.is_empty() {
                wm.log
                    .push(serde_json::json!({"event":"landing_input","tick":wm.tick,
                    "sounds":sounds.iter().map(|s| *s as u8).collect::<Vec<_>>(),
                    "changes":format!("{changes:?}"),"mode":state.mode,"cursor":state.cursor}));
            }
        }
        menu.update(layout, state);
        if mode_changed {
            let mode = state.mode;
            wm.pose = close_up(&wm.actors[symbol], mode, wm.pose);
        }
        // FUN_1403931f0 keeps the panel on the landing's world.
        if menu.done() {
            let outcome = state.outcome;
            wm.log
                .push(serde_json::json!({"event":"landing_closed","tick":wm.tick,
                "outcome":format!("{outcome:?}")}));
            wm.landing = None;
            // FUN_140391f50: camera slot 2 goes; the cameras below blend
            // back.
            wm.camera.suspend();
            if let Some(c) = wm.lock_camera.as_mut() {
                c.suspend();
            }
            match outcome {
                // A default landing by program id (wlds area ≥ 0x33) is
                // placed by FUN_14039fbc0. FUN_1403946d0 → FUN_140152990
                // (target, 1): mode 1 fades to black over n = 15
                // (FUN_140156520 → FUN_140156ce0) and the jump waits for it
                // (FUN_1401563f0), the input locked (DAT_142a10500 bit 2).
                Some(Outcome::Land(target)) => {
                    wm.fade.fade_out(15, false);
                    wm.leaving = Some(match target.spawn {
                        Some(p) if target.area == 0xff => {
                            world_state::resolve_program(&wm.program_areas, target.world, p)
                                .unwrap_or(target)
                        }
                        _ => target,
                    })
                }
                // FUN_1403946f0 queues the route's Gummi mission task (not
                // part of this program): the run ends as a jump does.
                Some(Outcome::Route(route)) => {
                    wm.log
                        .push(serde_json::json!({"event":"gummi_mission","route":route}));
                    wm.jump = Some(world_state::Target::NONE);
                }
                _ => {}
            }
        }
    } else {
        let found = find_reaction(wm);
        wm.reaction = found.map(|(i, _)| i);
        wm.reaction_cmd = found.map(|(_, c)| c);
        // The reaction command (field logical 0x400, Triangle) on a route
        // object opens the route menu (FUN_140394fc0 → FUN_140398bd0).
        if let Some(r) = wm.reaction
            && wm.pad.pressed & 0x400 != 0
            && let Some(route) = actor_route(&wm.actors[r])
            && let Some(p) = ui.ui.routes.get(&route)
        {
            let state = Landing::route_menu(route);
            let menu = Menu::new(layout, &state, p.texts.clone(), &[], None);
            let (c, s) = (wm.actors[wm.cursor].position, wm.actors[r].position);
            wm.actors[wm.cursor].yaw = (s[0] - c[0]).atan2(s[2] - c[2]);
            wm.velocity = [0.; 2];
            wm.pose = close_up(&wm.actors[r], landing::ROWS, wm.pose);
            wm.log.push(
                serde_json::json!({"event":"route_menu_opened","tick":wm.tick,"route":route}),
            );
            // FUN_140398bd0 ends with sound 3; FUN_140394fc0 then closes the
            // total play time (FUN_14038e7e0).
            wm.sounds.push(landing::Sound::Open as u32);
            wm.landing = Some((state, menu, r));
            if let Some(t) = wm.total.as_mut() {
                t.close(seq);
            }
        }
        // The reaction command (field logical 0x400, Triangle) lands.
        if wm.landing.is_none()
            && let Some(r) = wm.reaction
            && wm.pad.pressed & 0x400 != 0
            && let Some(world) = actor_world(&wm.actors[r])
            && let Some(p) = ui.ui.worlds.get(&world)
        {
            // FUN_140394fc0: Twilight Town and The World That Never Was play
            // a world event instead while their world-event bits say so
            // (FUN_14038de90), and the panels close (FUN_1403966c0 /
            // FUN_140399cd0) [I: at once; their close animation would run
            // under the event].
            let gated = match wm.actors[r].kind {
                Kind::Symbol(e) => objects::land_event(e.object, |b| {
                    wm.after_world.as_ref().is_some_and(|w| w.event_seen(b))
                }),
                _ => None,
            };
            if let Some(event) = gated {
                wm.event_queue.push_back(event);
                wm.events_ended = false;
                wm.info = None;
                wm.route_info = None;
                wm.velocity = [0.; 2];
                wm.log.push(
                    serde_json::json!({"event":"land_event","tick":wm.tick,"world":world,"id":event}),
                );
                return false;
            }
            let state = Landing::new(world, p.grid.clone(), p.default_target, p.episodes.clone());
            let menu = Menu::new(
                layout,
                &state,
                p.texts.clone(),
                &p.pictures,
                ui.ui.ws_i.clone(),
            );
            // The cursor turns to the symbol (FUN_1403b6420).
            let (c, s) = (wm.actors[wm.cursor].position, wm.actors[r].position);
            wm.actors[wm.cursor].yaw = (s[0] - c[0]).atan2(s[2] - c[2]);
            wm.velocity = [0.; 2];
            wm.pose = close_up(&wm.actors[r], state.mode, wm.pose);
            wm.log.push(
                serde_json::json!({"event":"landing_opened","tick":wm.tick,"world":world,
                "mode":state.mode,"cells":state.cells.len(),"episodes":state.episodes.len()}),
            );
            // FUN_140392e70 ends with sound 3; FUN_140394fc0 then closes the
            // total play time (FUN_14038e7e0).
            wm.sounds.push(landing::Sound::Open as u32);
            wm.landing = Some((state, menu, r));
            if let Some(t) = wm.total.as_mut() {
                t.close(seq);
            }
        }
    }
    // FUN_140394810: the panels follow the object of the land or route
    // reaction, else the locked target (holder state 2): the route info
    // (FUN_140399db0 / FUN_140399cd0) for a route object, the world's for
    // a symbol.
    let focus = wm.reaction.or(wm.lock.map(|c| c.object));
    let route = match &wm.landing {
        Some((state, _, _)) => state.route,
        None => focus.and_then(|r| actor_route(&wm.actors[r])),
    };
    match (route, wm.route_info.as_mut()) {
        (Some(r), Some(info)) if info.route == r && !info.closing() => {}
        (Some(r), _) => {
            if let Some(p) = ui.ui.routes.get(&r) {
                wm.route_info = Some(RouteInfo::new(seq, r, p.name.clone(), p.missions));
            }
        }
        (None, Some(info)) => info.close(seq),
        (None, None) => {}
    }
    if let Some(info) = wm.route_info.as_mut() {
        info.update(seq);
        if info.done() {
            wm.route_info = None;
        }
    }
    // FUN_140396a00 / FUN_1403966c0: the info panel follows that (or the
    // landing) symbol's world.
    let shown = match &wm.landing {
        Some((state, _, _)) if state.route.is_none() => Some(state.world),
        Some(_) => None,
        None => focus.and_then(|r| actor_world(&wm.actors[r])),
    };
    match (shown, wm.info.as_mut()) {
        (Some(w), Some(info)) if info.world == w && !info.closing() => {}
        (Some(w), _) => {
            if let Some(p) = ui.ui.worlds.get(&w) {
                // The world's play time (FUN_140157430: save +0x244c + 4 ×
                // world in ticks, as hours, minutes and seconds).
                let t = wm.world_times.get(w as usize).copied().unwrap_or(0) / 60;
                wm.info = Some(Info::new(
                    seq,
                    w,
                    p.name.clone(),
                    p.visits,
                    p.battle_level,
                    [t / 3600, t % 3600 / 60, t % 60],
                ));
            }
        }
        (None, Some(info)) => info.close(seq),
        (None, None) => {}
    }
    if let Some(info) = wm.info.as_mut() {
        info.update(seq);
        if info.done() {
            wm.info = None;
        }
    }
    wm.landing.is_some()
}

/// FUN_140394050 (with FUN_1403f0b70's draw group set): the objects re-read
/// their looks from the world state after the events' scripts.
fn refresh_objects(wm: &mut WorldMap) {
    if let Some(after) = wm.after_world.clone() {
        let tables = wm.tables.clone();
        let (eh, bobs) = (wm.eh_flag, wm.hidden_bobs.len());
        apply_world_state(wm, &after, tables.as_ref(), eh, bobs);
    }
    wm.log
        .push(serde_json::json!({"event":"event_refresh","tick":wm.tick}));
}

/// FUN_1403940b0: op 0x105's pre-refresh, per object (FUN_1403900d0 for
/// symbols, FUN_140397240 for ships) against the state after the scripts.
fn pre_refresh(wm: &mut WorldMap) {
    let Some(after) = wm.after_world.clone() else {
        return;
    };
    for i in 0..wm.actors.len() {
        let (state, flags) = wm.actors[i].applied;
        let starts = match wm.actors[i].kind {
            Kind::Symbol(entry) => {
                let before = world_state::WorldRecord {
                    state,
                    flags,
                    ..Default::default()
                };
                let now = after.record(entry.world).copied().unwrap_or_default();
                objects::symbol_pre_refresh(&before, &now)
            }
            Kind::Gumi(route) => objects::gumi_pre_refresh(state, after.symbol_slot(route)),
            Kind::Cursor => false,
        };
        if starts {
            wm.start_effect(i, objects::effect::PRE_REFRESH);
        }
    }
    wm.log
        .push(serde_json::json!({"event":"event_pre_refresh","tick":wm.tick}));
}

/// FUN_14038de20 → FUN_140390460, when the arrival's events task ends:
/// each open symbol whose world has a new episode starts element 4 once.
fn new_episode_effects(wm: &mut WorldMap) {
    for i in 0..wm.actors.len() {
        let Kind::Symbol(entry) = wm.actors[i].kind else {
            continue;
        };
        let any_new = wm.new_worlds.contains(&entry.world);
        if wm.actors[i].episode_effect.is_none()
            && objects::new_episode(wm.actors[i].applied.0, any_new)
        {
            let h = wm.start_effect(i, objects::effect::NEW_EPISODE);
            wm.actors[i].episode_effect = Some(h);
        }
    }
}

/// FUN_14038dc60 / FUN_14038fae0: the queued world events play one after
/// another; while one plays the cursor is held and no reaction or info
/// panel runs (FUN_14038da50). True while an event plays.
fn event_tick(wm: &mut WorldMap, ui: &UiRes) -> bool {
    if wm.event.is_none() {
        let Some(id) = wm.event_queue.pop_front() else {
            if !wm.events_ended {
                wm.events_ended = true;
                new_episode_effects(wm);
            }
            return false;
        };
        let script = ui.ui.events.get(&(id as u16)).cloned().unwrap_or_default();
        wm.log.push(
            serde_json::json!({"event":"world_event_start","tick":wm.tick,
            "id":id,"ops":script.len()}),
        );
        wm.event = Some(events::EventPlayer::new(id, script));
    }
    // The message window: the field's advance bits (trigger & 0x40c).
    if let Some(w) = wm.window.as_mut() {
        let advance = wm.pad.pressed & 0x40c != 0;
        let sounds = w.step(advance);
        if w.state == 3 {
            w.close();
        }
        let deleted = w.deleted();
        wm.sounds.extend(sounds);
        if deleted {
            wm.window = None;
            if let Some(p) = wm.event.as_mut() {
                p.message_closed();
            }
        }
    }
    let Some(player) = wm.event.as_mut() else {
        return false;
    };
    let actions = player.tick();
    let done = player.done;
    let id = player.id;
    let mut changes = Vec::new();
    for a in actions {
        match a {
            events::Action::Message { id, tutorial } => {
                let text = ui.ui.message(id);
                match (&ui.ui.window, &ui.ui.event_font) {
                    (Some(sheet), Some(font)) => {
                        wm.window = Some(visual::dialogue::TalkWindow::balloon(
                            &sheet.sequence,
                            text,
                            ui.ui.colours[usize::from(tutorial)],
                            &font.widths,
                        ));
                    }
                    _ => {
                        if let Some(p) = wm.event.as_mut() {
                            p.message_closed();
                        }
                    }
                }
                wm.log
                    .push(serde_json::json!({"event":"event_message","tick":wm.tick,
                    "message":id,"tutorial":tutorial}));
            }
            events::Action::Fade { out, white, n } => {
                if out {
                    wm.fade.fade_out(n, white);
                } else {
                    wm.fade.fade_in(n, white);
                }
            }
            events::Action::Camera {
                target,
                offset,
                frames,
            } => {
                wm.event_camera =
                    Some(wm_camera::EventCamera::new(wm.pose, target, offset, frames));
            }
            events::Action::Refresh => refresh_objects(wm),
            events::Action::PreRefresh => pre_refresh(wm),
            events::Action::ShipToHollowBastion => {
                let hb = wm.actors.iter().position(
                    |a| matches!(a.kind, Kind::Symbol(e) if e.object == objects::SYMBOLS[1].object),
                );
                if let Some(hb) = hb {
                    let (position, yaw) = (wm.actors[hb].position, wm.actors[hb].yaw);
                    let c = &mut wm.actors[wm.cursor];
                    c.position = position;
                    c.yaw = wrap_angle(yaw + std::f32::consts::PI);
                    c.motion = CURSOR_RIDE;
                    c.time = 0.;
                    c.blend = 0.;
                    wm.riding = true;
                }
                wm.log
                    .push(serde_json::json!({"event":"ship_rides","tick":wm.tick,"symbol":hb}));
            }
            events::Action::Sound(s) => wm.sounds.push(s),
            events::Action::Widgets(op) => {
                let WorldMap {
                    scene,
                    archives,
                    sounds,
                    ..
                } = &mut *wm;
                let clips = archives.get(&id).map_or(&[][..], |a| &a.clips[..]);
                changes.extend(scene.apply(&op, clips, sounds));
                wm.log
                    .push(serde_json::json!({"event":"widget_op","tick":wm.tick,
                    "op":format!("{op:?}")}));
            }
            events::Action::Voice(entry) => {
                wm.voices.push((id, entry));
            }
            events::Action::Jump { world, area } => {
                wm.jump = Some(world_state::Target::at(world, area, 0));
            }
            events::Action::JumpProgram { world, program } => {
                let t = world_state::resolve_program(&wm.program_areas, world, program);
                wm.log
                    .push(serde_json::json!({"event":"jump_program","tick":wm.tick,
                    "world":world,"program":program,"area":t.map(|t| t.area)}));
                if t.is_some() {
                    wm.jump = t;
                }
            }
            other => wm
                .log
                .push(serde_json::json!({"event":"event_op","tick":wm.tick,
                "op":format!("{other:?}")})),
        }
    }
    // The widgets' motions (their tasks run after the event's).
    {
        let WorldMap {
            scene,
            archives,
            sounds,
            ..
        } = &mut *wm;
        let clips = archives.get(&id).map_or(&[][..], |a| &a.clips[..]);
        scene.update(clips, sounds);
    }
    if done {
        // The event object's teardown (FUN_14038eda0) clears the widgets
        // (FUN_14038f150): the map shows and the objects refresh.
        changes.extend(wm.scene.clear());
    }
    for c in changes {
        match c {
            widgets::Change::HideMap => wm
                .log
                .push(serde_json::json!({"event":"map_hidden","tick":wm.tick})),
            widgets::Change::ShowMap => refresh_objects(wm),
        }
    }
    if done {
        wm.log
            .push(serde_json::json!({"event":"world_event_end","tick":wm.tick,"id":id}));
        wm.event = None;
        // FUN_14038dc60's end: camera slot 2 clears and ObjCamera blends
        // back.
        if wm.event_camera.take().is_some() {
            wm.camera.suspend();
            if let Some(c) = wm.lock_camera.as_mut() {
                c.suspend();
            }
        }
    }
    true
}

/// One 60 Hz tick: the cursor (worldmap::Player with worldmap::ActionShip),
/// the landing, the objects' motion clocks and the camera.
fn tick(mut wm: ResMut<WorldMap>, ui: Res<UiRes>) {
    if wm.frames < WARM_UP_FRAMES {
        return;
    }
    wm.tick += 1;
    if wm.autoplay.is_some() {
        let (stick, buttons) = if wm.event.is_some() || !wm.event_queue.is_empty() {
            // During world events the script is held: messages advance
            // every 90 ticks.
            let t = wm.tick;
            let advance = if t % 90 < 2 {
                controller::button::CROSS
            } else {
                0
            };
            ([0.; 3], advance)
        } else {
            let route = wm.landing.as_ref().is_some_and(|(l, ..)| l.route.is_some());
            let (mut stick, buttons) =
                autoplay_input(wm.tick - wm.event_ticks, wm.autoplay_decide, route);
            // [Tour aid] KH2RW_WM_FLY=<x>,<y>: after the menu, fly this way
            // instead of the scripted flight.
            if let Some((x, y)) = std::env::var("KH2RW_WM_FLY").ok().and_then(|v| {
                let (x, y) = v.split_once(',')?;
                Some((x.trim().parse::<f32>().ok()?, y.trim().parse::<f32>().ok()?))
            }) && wm.tick - wm.event_ticks >= 440
            {
                stick = [x, y, x.hypot(y).min(1.)];
            }
            (stick, buttons)
        };
        if wm.event.is_some() || !wm.event_queue.is_empty() {
            wm.event_ticks = wm.tick;
        }
        wm.stick = stick;
        wm.raw = controller::raw(&controller::PadData {
            buttons,
            ..Default::default()
        });
    }
    // The world map reads the pad in field mode 1; the menu in mode 3
    // (FUN_14039b580). While a jump waits for its fade the input is locked
    // (FUN_1403b2340 with DAT_142a10500 set).
    let raw = if wm.leaving.is_some() { 0 } else { wm.raw };
    if wm.leaving.is_some() {
        wm.stick = [0.; 3];
        wm.right = [0.; 3];
    }
    if wm.landing.is_some() {
        wm.pad.update_raw(raw, landing::pad::MODE, 1.);
    } else {
        wm.pad.update_raw(raw, controller::MODE_FIELD, 1.);
    }
    let flicks = raw & !wm.last_raw & STICK_FLICKS;
    wm.last_raw = raw;
    // The camp or journal holds the map: the field is not stepped (the pad
    // above still follows the buttons, so none reads as new afterwards).
    if wm.camp_up {
        return;
    }
    wm.fade.update(1.);
    // FUN_1401563f0: the jump goes once the fade is opaque.
    if wm.leaving.is_some() && wm.fade.state == visual::fade::State::Held {
        wm.jump = wm.leaving.take();
    }
    // FUN_1401574e0: the total play time counts every frame (to 999:59:59).
    wm.play_time = (wm.play_time + 1).min(0xcdf_e5c4);
    wm.world_times[WORLD_MAP_WORLD] = (wm.world_times[WORLD_MAP_WORLD] + 1).min(0xcdf_e5c4);
    let eventing = wm.landing.is_none() && event_tick(&mut wm, &ui);
    let held = eventing || landing_tick(&mut wm, &ui) || wm.riding || wm.leaving.is_some();
    let ci = wm.cursor;
    // The lock manager (FUN_1403dcd70; the cursor takes no automatic
    // target), then the deck's row (FUN_1403947a0) and its command, taken by
    // its selector's button (FUN_1403b1590 → FUN_140394fc0).
    lock_tick(&mut wm, !held, flicks);
    let (command, _) = cursor::command(wm.lock.is_some());
    if !held && wm.pad.pressed & SELECTOR_BIT != 0 {
        execute(&mut wm, command);
    }
    // FUN_1403a8fa0's move request (+0x10): the stick direction (up = away
    // from the camera, which looks along +Z) at the walk speed below
    // magnitude 0.8 and the run speed from it; with the stick at rest the
    // facing at no speed.
    let [x, y, magnitude] = if held { [0.; 3] } else { wm.stick };
    let len = x.hypot(y);
    let yaw = wm.actors[ci].yaw;
    let direction = if len > 0. {
        [x / len, -y / len]
    } else {
        [yaw.sin(), yaw.cos()]
    };
    let speed = if magnitude <= 0. {
        0.
    } else if magnitude < actor_physics::RUN_MAGNITUDE {
        wm.params.walk
    } else {
        wm.params.run
    };
    // FUN_1403b4780: the facing turns at +0x134 per tick toward the move.
    if speed > 0. && !matches!(wm.action, Action::Warp(_)) {
        let target = direction[0].atan2(direction[1]);
        let delta = wrap_angle(target - yaw);
        let turn = wm.params.turn;
        wm.actors[ci].yaw = wrap_angle(yaw + delta.clamp(-turn, turn));
    }
    let start = wm.actors[ci].position;
    let mut warped = None;
    match &mut wm.action {
        Action::Ship | Action::Dash => {
            // ActionShip: FUN_140563840 → FUN_1403b7ad0 with the PRTY
            // record's +0x28 inertia and +0x2c drag; ActionDash: FUN_1405639e0
            // asks for the dash speed (+0x24) and FUN_1405639b0 steers with
            // 0.95 / 0.95. FUN_1403b89a0 runs them every second frame below
            // its speed limit.
            let (speed, inertia, drag) = if matches!(wm.action, Action::Dash) {
                (wm.dash_speed, cursor::DASH_STEER, cursor::DASH_STEER)
            } else {
                (speed, wm.params.word10, wm.params.word11)
            };
            if wm.sampler.due(speed) {
                let velocity = actor_physics::steer(wm.velocity, direction, speed, inertia, drag);
                wm.velocity = velocity;
                let dt = actor_physics::PHYSICS_STEP;
                wm.sampler.store([velocity[0] * dt, 0., velocity[1] * dt]);
            }
        }
        // ActionWarp: FUN_1405638f0 runs the path (FUN_140397820 →
        // FUN_1403b64d0 places the cursor) and the ship takes over once it
        // is spent; FUN_1405638e0 moves nothing.
        Action::Warp(path) => {
            warped = Some((path.step(), path.done()));
        }
    }
    let mut d = if warped.is_some() {
        [0.; 3]
    } else {
        wm.sampler.displacement()
    };
    // FUN_1403d4570: a knock-slide adds its own displacement.
    if let Some(k) = wm.knock.as_mut() {
        match k.step() {
            Some(s) if warped.is_none() => {
                d[0] += s[0];
                d[2] += s[2];
            }
            Some(_) => {}
            None => wm.knock = None,
        }
    }
    let solved = wm.collision.as_ref().map(|c| {
        collision::solve(
            c,
            &wm.body,
            &Move {
                start,
                displacement: d,
                airborne: true,
                extra_snap: 0.,
            },
        )
    });
    // The floor under the cursor (obj+0x780, the polygon's material).
    if let (Some(c), Some(s)) = (wm.collision.as_ref(), solved.as_ref()) {
        wm.floor = collision::floor_info(c, s).ground;
    }
    // FUN_1403a92c0: a wall with attribute bit 11 (+0x732 from the solve's
    // contact) stops the cursor and knocks it back along the wall's normal
    // at strength 1000 (vtable +0x130 → FUN_140394cc0 → FUN_1403d4960) once
    // per new polygon; the map's element of category bit 0 would play
    // there (FUN_1402c6730(map, 1)), and wm00 has none. The contact stays
    // latched 30 ticks after the cursor leaves the wall.
    let touch = solved.as_ref().and_then(|s| s.touch_11);
    match touch {
        None => {
            if 0. < wm.wall.1 {
                wm.wall.1 -= 1.;
                if wm.wall.1 <= 0. {
                    wm.wall.0 = 0xffff;
                }
            }
        }
        Some((poly, _)) => {
            if poly != wm.wall.0 {
                wm.wall.0 = poly;
                wm.velocity = [0.; 2];
                let normal = wm.collision.as_ref().and_then(|c| c.polygon_normal(poly));
                if let Some(n) = normal {
                    wm.knock = actor_physics::Knock::new(n, 1000., 0.);
                }
                let tick = wm.tick;
                wm.log.push(serde_json::json!({"event":"wall","tick":tick,
                    "polygon":poly,"normal":normal}));
            }
            wm.wall.1 = 30.;
        }
    }
    match warped {
        Some((at, done)) => {
            wm.actors[ci].position = at;
            if done {
                wm.action = Action::Ship;
                let tick = wm.tick;
                wm.log
                    .push(serde_json::json!({"event":"warp_end","tick":tick,"at":at}));
            }
        }
        None => {
            let moved = match solved {
                Some(s) if d[0] != 0. || d[2] != 0. => s.position,
                _ => [start[0] + d[0], start[1], start[2] + d[2]],
            };
            // The cursor holds its height (ActionShip moves it in the plane).
            wm.actors[ci].position = [moved[0], start[1], moved[2]];
        }
    }
    // The motions: ActionShip's FUN_140563880 picks idle or fast with an
    // 8-tick blend (FUN_1403c6dc0); ActionDash leaves for the ship when its
    // motion has ended and the back button is up (FUN_1405639e0, mgr+0xc
    // bit 0), its exit (FUN_140563a30) blending to idle and releasing its
    // effect (FUN_140395670). Riding a symbol (op 0x110) keeps the
    // scripted motion.
    if matches!(wm.action, Action::Dash)
        && motion_ended(&wm.actors[ci])
        && wm.pad.held & BACK_HELD == 0
    {
        wm.action = Action::Ship;
        set_motion(&mut wm.actors[ci], CURSOR_IDLE, cursor::BLEND);
        if let Some(h) = wm.cursor_fx[0] {
            wm.fx_requests.push(FxRequest::Release(h));
        }
        let tick = wm.tick;
        wm.log
            .push(serde_json::json!({"event":"dash_end","tick":tick}));
    }
    let wanted = cursor::ship_motion(speed, wm.params.walk, wm.params.run);
    if !wm.riding && matches!(wm.action, Action::Ship) && wm.actors[ci].motion != wanted {
        set_motion(&mut wm.actors[ci], wanted, cursor::BLEND);
    }
    // FUN_140394900: the floor's engine effect (element 5 on material 1,
    // 6 on material 2); the other is released.
    let want = cursor::engine_effect(wm.floor);
    for (slot, element) in [(0, 5), (1, 6)] {
        if want == Some((slot, element)) {
            if wm.cursor_fx[1 + slot].is_none() {
                let h = wm.start_cursor_effect(element);
                wm.cursor_fx[1 + slot] = Some(h);
            }
        } else if let Some(h) = wm.cursor_fx[1 + slot].take() {
            wm.fx_requests.push(FxRequest::Release(h));
        }
    }
    for a in &mut wm.actors {
        a.time += actor_physics::DT;
    }
    fire_triggers(&mut wm);
    let cursor = wm.actors[ci].position;
    // Camera slot 2 (the close-up or the event camera) wins over slot 1
    // (worldmap::LockOnCamera), which wins over ObjCamera.
    if let Some(c) = wm.event_camera.as_mut() {
        wm.pose = c.update();
    } else if wm.landing.is_none() {
        let now = wm.pose;
        match (wm.lock, wm.lock_camera.as_mut()) {
            (Some(target), Some(c)) => {
                wm.pose = c.update(cursor, target.centre, now);
                wm.camera.suspend();
            }
            _ => {
                let eh = wm.eh_symbol;
                wm.pose = wm.camera.update(cursor, eh, now);
            }
        }
    }
    // The deck's row and the reaction above it (FUN_14015df90 /
    // FUN_14015d670); the menu hides while the landing menu, an event or a
    // jump holds the cursor [I].
    let WorldMap {
        command: deck,
        lock,
        reaction_cmd,
        command_texts,
        ..
    } = &mut *wm;
    if let (Some(menu), Some((_, sheet))) = (deck.as_mut(), ui.ui.command.as_ref()) {
        let (_, message) = cursor::command(lock.is_some());
        menu.items = vec![Item {
            label: ui.ui.message(message),
            icon: 0,
            colour: 0,
        }];
        // The symbols' and routes' reactions are offered in display mode 0
        // (FUN_140407de0 with no angle gate).
        menu.reaction = reaction_cmd.and_then(|c| {
            command_texts.get(&c).map(|&(text, flags)| Reaction {
                command: c,
                label: ui.ui.message(text),
                mode: 0,
                flag3: flags & 8 != 0,
                restart: false,
            })
        });
        menu.update(&sheet.sequence);
    }
    // The reticle on the held target's centre (FUN_1403dd770: group 0x1b,
    // a manual lock on pool 0).
    if let Some((_, fiel)) = ui.ui.field.as_ref() {
        let held = wm.lock.map(|c| {
            (
                lock_on::Held {
                    key: c.object as u64,
                    manual: true,
                    primary_pool: true,
                    ..lock_on::Held::default()
                },
                c.centre,
            )
        });
        let pose = wm.pose;
        let draw = wm
            .reticle
            .step(&fiel.sequence, held, &|at| project(&pose, at), 0);
        wm.reticle_draw = draw;
    }
    // dk::WM_TOTAL_INFO's tick (FUN_14038e8c0) with FUN_140157390's hours
    // and minutes.
    let seconds = wm.play_time / 60;
    let seq = &ui.ui.layout.layout.sequences[world_map_menu::SEQUENCE];
    if let Some(t) = wm.total.as_mut() {
        t.update(seq, seconds / 3600, seconds % 3600 / 60);
        if t.done() {
            wm.total = None;
        }
    }
    let t = wm.tick;
    if t.is_multiple_of(60) {
        let velocity = wm.velocity;
        let reaction = wm.reaction.and_then(|r| actor_world(&wm.actors[r]));
        let motion = wm.actors[ci].motion;
        let action = match wm.action {
            Action::Ship => "ship",
            Action::Dash => "dash",
            Action::Warp(_) => "warp",
        };
        let (floor, lock) = (wm.floor, wm.lock.map(|c| c.object));
        wm.log.push(serde_json::json!({"event":"cursor","tick":t,
            "position":cursor,"motion":motion,"velocity":velocity,"reaction":reaction,
            "action":action,"floor":floor,"lock":lock}));
    }
}

/// The logical bit of the deck row's selector 1 (DAT_1405c4ed8: the back
/// button, logical bit 24) and the held back button (logical bit 8) that
/// keeps a dash going.
const SELECTOR_BIT: u64 = 1 << 24;
const BACK_HELD: u64 = 1 << 8;
/// R1 (logical 0x800) toggles the manual lock; the raw right-stick
/// directions (bits 16..19) flick it to another target.
const LOCK_BIT: u64 = 0x800;
const STICK_FLICKS: u64 = 0xf_0000;
/// The lock manager's sounds (FUN_1401dd170 7 and 8).
const SE_LOCK: u32 = 7;
const SE_UNLOCK: u32 = 8;

/// FUN_1403c86a0: the motion restarts with `blend` ticks of crossfade.
fn set_motion(a: &mut Actor, motion: u16, blend: f32) {
    a.motion = motion;
    a.time = 0.;
    a.blend = blend;
}

/// FUN_1403c7e70: no blend runs and the motion's clock reached its end.
fn motion_ended(a: &Actor) -> bool {
    let Some(m) = a.motions.get(&a.motion) else {
        return true;
    };
    let (start, end, fps, _) = m.frame_window().unwrap_or((0., 1., 30., 0.));
    let length = (end - start) * 60. / fps.max(1.);
    a.blend.max(0.) <= a.time && length <= a.time
}

/// The lock-on candidates: the TARGET shapes of the drawn symbols and route
/// objects (FUN_1403ba260: object flag 0x20 clear and a kind-3 shape).
fn lock_candidates(wm: &WorldMap) -> Vec<Candidate> {
    wm.actors
        .iter()
        .enumerate()
        .filter(|(_, a)| a.drawn && matches!(a.kind, Kind::Symbol(_) | Kind::Gumi(_)))
        .flat_map(|(i, a)| {
            a.shapes
                .iter()
                .filter(|s| s.kind == object_collision::TARGET && s.parts & 1 == 0)
                .map(move |s| Candidate {
                    object: i,
                    centre: shape_at(a, s),
                    radius: s.radius as f32,
                })
        })
        .collect()
}

/// FUN_1403dcd70 for worldmap::Player (flag 0x20000000 skips the automatic
/// target): R1 locks on the nearest candidate in range of the look point
/// (FUN_1403bea30) with sound 7 and worldmap::LockOnCamera
/// (FUN_1403dd650 → FUN_1405635d0); R1 again, or the target leaving,
/// releases it with sound 8 (FUN_1403dd510 → FUN_140563430); a right-stick
/// flick moves it (FUN_1403bd750) with a new camera.
fn lock_tick(wm: &mut WorldMap, free: bool, flicks: u64) {
    let candidates = lock_candidates(wm);
    let position = wm.actors[wm.cursor].position;
    let pressed = free && wm.pad.pressed & LOCK_BIT != 0;
    match wm.lock {
        Some(held) => {
            let now = candidates.iter().find(|c| c.object == held.object).copied();
            match now {
                Some(c) if !pressed => {
                    wm.lock = Some(c);
                    if free && flicks != 0 {
                        let r = wm.right;
                        if let Some(next) = cursor::switch_target(
                            position,
                            wm.lock_range,
                            &c,
                            [r[0], r[1]],
                            &candidates,
                        ) {
                            wm.lock = Some(next);
                            wm.lock_camera = Some(LockOnCamera::default());
                            wm.log.push(serde_json::json!({"event":"lock_switch",
                                "tick":wm.tick,"object":next.object}));
                        }
                    }
                }
                _ => {
                    wm.sounds.push(SE_UNLOCK);
                    wm.lock = None;
                    wm.lock_camera = None;
                    wm.camera.suspend();
                    wm.log
                        .push(serde_json::json!({"event":"lock_release","tick":wm.tick}));
                }
            }
        }
        None if pressed => {
            if let Some(c) = cursor::lock_search(position, wm.lock_range, &candidates) {
                wm.sounds.push(SE_LOCK);
                wm.lock = Some(c);
                wm.lock_camera = Some(LockOnCamera::default());
                wm.log
                    .push(serde_json::json!({"event":"lock","tick":wm.tick,
                    "object":c.object,"centre":c.centre,"radius":c.radius,"cursor":position}));
            } else {
                wm.log
                    .push(serde_json::json!({"event":"lock_none","tick":wm.tick}));
            }
        }
        None => {}
    }
}

/// FUN_140394fc0 for the deck's commands: 0x247 enters ActionDash
/// (FUN_1403b4da0(obj, 0x3b): the exit of the action it leaves, then
/// FUN_140563a70: the velocity along the move request at the dash speed,
/// motion 0x9a at once and element 2 (FUN_140395950)); 0x246 with a held
/// target kills that effect (FUN_1403f1ec0) and warps (FUN_1403978c0 then
/// action 0x3a, FUN_140563950: no velocity, motion 0xa1 at once).
fn execute(wm: &mut WorldMap, command: u16) {
    let ci = wm.cursor;
    if command == cursor::CMD_DASH {
        if matches!(wm.action, Action::Dash) {
            // FUN_140563a30: the dash's own exit first.
            set_motion(&mut wm.actors[ci], CURSOR_IDLE, cursor::BLEND);
            if let Some(h) = wm.cursor_fx[0] {
                wm.fx_requests.push(FxRequest::Release(h));
            }
        }
        let [x, y, magnitude] = wm.stick;
        let len = x.hypot(y);
        let yaw = wm.actors[ci].yaw;
        let dir = if magnitude > 0. && len > 0. {
            [x / len, -y / len]
        } else {
            [yaw.sin(), yaw.cos()]
        };
        wm.velocity = [dir[0] * wm.dash_speed, dir[1] * wm.dash_speed];
        wm.sampler.reset();
        wm.action = Action::Dash;
        set_motion(&mut wm.actors[ci], cursor::MOTION_DASH, 0.);
        let h = wm.start_cursor_effect(cursor::DASH_EFFECT);
        wm.cursor_fx[0] = Some(h);
        wm.log
            .push(serde_json::json!({"event":"dash","tick":wm.tick}));
    } else if command == cursor::CMD_WARP
        && let Some(target) = wm.lock
    {
        if let Some(h) = wm.cursor_fx[0].take() {
            wm.fx_requests.push(FxRequest::Stop(h));
        }
        let from = wm.actors[ci].position;
        let body = wm.body;
        // FUN_1403f3dd0 → FUN_14040c2b0: the map's first hit on the
        // segment.
        let path = WarpPath::new(from, target.centre, target.radius, |top, bottom| {
            wm.collision
                .as_ref()?
                .ray(top, bottom, body.class)
                .map(|(p, _)| p)
        });
        let end = path.end();
        wm.velocity = [0.; 2];
        wm.sampler.reset();
        wm.action = Action::Warp(path);
        set_motion(&mut wm.actors[ci], cursor::MOTION_WARP, 0.);
        wm.log
            .push(serde_json::json!({"event":"warp","tick":wm.tick,
            "object":target.object,"from":from,"to":end}));
    }
}

/// FUN_14016b350 for the world map camera: a game-space point to layout
/// units and the clip depth (as follow_camera's projection).
fn project(pose: &Pose, point: [f32; 3]) -> [f32; 3] {
    let sub = |a: [f32; 3], b: [f32; 3]| [a[0] - b[0], a[1] - b[1], a[2] - b[2]];
    let dot = |a: [f32; 3], b: [f32; 3]| a[0] * b[0] + a[1] * b[1] + a[2] * b[2];
    let norm = |v: [f32; 3], or: [f32; 3]| {
        let l = dot(v, v).sqrt();
        if l > 1e-6 { v.map(|c| c / l) } else { or }
    };
    let forward = norm(sub(pose.target, pose.eye), [0., 0., 1.]);
    let down = norm(
        sub([0., 1., 0.], forward.map(|c| c * forward[1])),
        [0., 1., 0.],
    );
    let right = [
        down[1] * forward[2] - down[2] * forward[1],
        down[2] * forward[0] - down[0] * forward[2],
        down[0] * forward[1] - down[1] * forward[0],
    ];
    let o = sub(point, pose.eye);
    let (x, y, z) = (dot(o, right), dot(o, down), dot(o, forward));
    let focal = 320. / (pose.fov * 0.5).tan();
    let (x, y) = if z != 0. { (x / z, y / z) } else { (x, y) };
    let (near, far) = (1., 4_000_000.);
    let depth = (far + near) / (far - near) * z - 2. * far * near / (far - near);
    [
        x * focal * 0.8 + 256.,
        y * focal * 0.866_666_7 + 208.,
        depth,
    ]
}

/// The world map's game world (DAT_140717008 while it runs).
const WORLD_MAP_WORLD: usize = 15;

/// The camp chain for one tick (the camp task FUN_14031d3c0): the landing
/// grid's journal request, then START opening the camp while the map is
/// free (FUN_1401572b0 waits for no load and an idle fade).
fn camp_tick(
    mut wm: ResMut<WorldMap>,
    mut host: ResMut<CampHost>,
    mut frame: ResMut<Ui2dFrame>,
    mut images: ResMut<Assets<Image>>,
) {
    let Some(camp) = host.0.as_mut() else {
        return;
    };
    if wm.frames < WARM_UP_FRAMES {
        return;
    }
    let mut sounds = Vec::new();
    if std::mem::take(&mut wm.journal_request) {
        sounds.extend(camp.open_journal());
    }
    let can_open = wm.landing.is_none()
        && wm.event.is_none()
        && wm.event_queue.is_empty()
        && wm.leaving.is_none()
        && !wm.riding
        && wm.fade.state == visual::fade::State::Idle;
    let raw = if wm.leaving.is_some() { 0 } else { wm.raw };
    let (stick, devices, play_time) = (wm.stick, wm.devices, wm.play_time);
    sounds.extend(camp.tick(
        raw,
        stick,
        devices,
        can_open,
        play_time,
        &mut frame,
        &mut images,
    ));
    wm.camp_up = camp.menu_up();
    wm.sounds.extend(sounds);
    let log = std::mem::take(&mut camp.log);
    wm.log.extend(log);
}

/// Plays this tick's sound requests and starts the field BGM once decoded.
fn play_audio(mut wm: ResMut<WorldMap>, mut audio: ResMut<AudioRes>) {
    let audio = &mut *audio;
    let tick = wm.tick;
    for id in std::mem::take(&mut wm.sounds) {
        let resolved = audio.banks.request(id).ok().flatten();
        wm.log
            .push(serde_json::json!({"event":"sound","tick":tick,"id":id,
            "layers":resolved.as_ref().map(|r| r.layers.len())}));
        if let (Some(output), Some(r)) = (audio.output.as_ref(), resolved)
            && let Ok(mut mixer) = output.mixer.lock()
        {
            for layer in r.layers {
                mixer.play(layer, id, r.volume, 1.);
            }
        }
    }
    // Map effect sounds stop with their draw group.
    for id in std::mem::take(&mut wm.sound_stops) {
        if let Some(output) = audio.output.as_ref()
            && let Ok(mut mixer) = output.mixer.lock()
        {
            mixer.stop(id);
        }
    }
    // Op 0x108: the last voice stops and this one plays at volume 0x3fff.
    for (event, entry) in std::mem::take(&mut wm.voices) {
        let clip = audio.voices.get(&(event, entry)).cloned();
        wm.log.push(
            serde_json::json!({"event":"voice","tick":tick,"event_id":event,
            "entry":entry,"found":clip.is_some()}),
        );
        if let (Some(output), Some(clip)) = (audio.output.as_ref(), clip)
            && let Ok(mut mixer) = output.mixer.lock()
        {
            mixer.stop(VOICE_KEY);
            mixer.play(clip, VOICE_KEY, 0x3fff as f32 / 16383., 1.);
        }
    }
    if audio.bgm_started {
        return;
    }
    let decoded = audio
        .music
        .as_ref()
        .and_then(|slot| slot.lock().ok().and_then(|mut m| m.take()));
    match decoded {
        Some(Ok(m)) => {
            if let Some(output) = audio.output.as_ref()
                && let Ok(mut mixer) = output.mixer.lock()
            {
                mixer.play(m.clip.clone(), BGM_KEY, m.volume, 1.);
            }
            audio.bgm_started = true;
            wm.log
                .push(serde_json::json!({"event":"bgm_start","tick":tick,"stats":m.stats}));
        }
        Some(Err(e)) => {
            audio.bgm_started = true;
            wm.log
                .push(serde_json::json!({"event":"bgm_failed","tick":tick,"error":e}));
        }
        None => {}
    }
}

/// This frame's 2D draws: the lock-on reticle (priority 4000), the
/// cursor's deck, the info panels, the total play time and the landing
/// menu, the message window and the fade.
fn draw_ui(
    mut wm: ResMut<WorldMap>,
    ui: Res<UiRes>,
    mut frame: ResMut<Ui2dFrame>,
    mut camp: ResMut<CampHost>,
) {
    let layout = &ui.ui.layout.layout;
    let seq = &layout.sequences[world_map_menu::SEQUENCE];
    let font = Font {
        widths: &ui.ui.font.widths,
        pad: 1,
    };
    if let (Some(visual::gauge::Draw::Quads { quads, .. }), Some(sheet)) =
        (wm.reticle_draw.as_ref(), ui.fiel)
    {
        frame.draws.push((sheet, quads.clone()));
    }
    // The deck hides while the landing menu, an event or a jump holds the
    // cursor [I].
    let shown = wm.landing.is_none()
        && wm.event.is_none()
        && wm.event_queue.is_empty()
        && wm.leaving.is_none();
    if shown
        && let (Some(menu), Some((_, sheet)), Some(slot)) =
            (wm.command.as_mut(), ui.ui.command.as_ref(), ui.command)
    {
        for d in menu.draw(&sheet.sequence, &font, slot, ui.font, ui.icon) {
            if let visual::gauge::Draw::Quads { quads, sheet, .. } = d {
                frame.draws.push((sheet, quads));
            }
        }
    }
    let mut items = Vec::new();
    if let Some((state, menu, _)) = &wm.landing {
        items.extend(menu.draw(layout, state, &font));
    }
    if let Some(info) = &wm.info {
        items.extend(info.draw(seq, &font));
    }
    if let Some(info) = &wm.route_info {
        items.extend(info.draw(seq, &font));
    }
    if let Some(total) = &wm.total {
        items.extend(total.draw(seq, &font));
    }
    for (sheet, quads) in items.drain(..) {
        let slot = match sheet {
            world_map_menu::Sheet::Image(k) => ui.images.get(k).copied(),
            world_map_menu::Sheet::Picture(k) => ui.pictures.get(k).copied(),
            world_map_menu::Sheet::Font => Some(ui.font),
            world_map_menu::Sheet::Icon => Some(ui.icon),
        };
        if let Some(s) = slot {
            frame.draws.push((s, quads));
        }
    }
    if let (Some(w), Some(window), Some(evt)) = (&wm.window, ui.window, ui.event_font) {
        for d in w.draw(window, evt) {
            if let visual::gauge::Draw::Quads { quads, sheet, .. } = d {
                frame.draws.push((sheet, quads));
            }
        }
    }
    // The camp, its curtain and the journal over the map.
    if let Some(c) = camp.0.as_mut() {
        c.draw(&mut frame);
    }
    // dk::Fade on top (black blends, white adds).
    if wm.fade.state != visual::fade::State::Idle {
        let ([r, g, b, a], additive) = wm.fade.colour();
        let (l, rr) = (-200., 712.);
        frame.draws.push((
            ui.white,
            vec![data::sprite_sequence::Quad {
                corners: [[l, -10.], [rr, -10.], [l, 426.], [rr, 426.]],
                uv: [0, 0, 1, 1],
                colors: [[r, g, b, a]; 4],
                blend: additive as i32,
            }],
        ));
    }
}

/// FUN_14040e960 → FUN_14040dd70 for the world map objects: the frame
/// triggers the motion controller's clock passed this tick (in 60 Hz
/// ticks of the motion; a motion starts just before its first frame and a
/// loop resumes after its return frame). Trigger 1 starts an element of
/// the object's PAX, 8 plays a sound (`p0 × 1000 + p1`), 26 shows and 27
/// hides mesh part `p0` (FUN_1403cd670 / FUN_1403cd4c0 fade the part over
/// `p1` frames; the installed world map motions use 0, at once).
fn fire_triggers(wm: &mut WorldMap) {
    for i in 0..wm.actors.len() {
        let a = &wm.actors[i];
        let (Some(m), Some(list)) = (a.motions.get(&a.motion), a.triggers.get(&a.motion)) else {
            continue;
        };
        let (start, end, fps, ret) = m.frame_window().unwrap_or((0., 1., 30., 0.));
        let to_ticks = 60. / fps.max(1.);
        let now = motion_time(m, a.time) * to_ticks;
        let changed = a.clock.is_some_and(|(motion, _)| motion != a.motion);
        let before = match a.clock {
            Some((motion, t)) if motion == a.motion => t,
            _ => start * to_ticks - 1.,
        };
        let passed = |f: f32| {
            if now >= before {
                before < f && f <= now
            } else {
                // Wrapped: the end of the pass, then from the return frame.
                (before < f && f <= end * to_ticks) || (ret * to_ticks < f && f <= now)
            }
        };
        let due: Vec<(u8, Vec<i16>)> = list
            .iter()
            .filter(|t| passed(t.0))
            .map(|t| (t.1, t.2.clone()))
            .collect();
        let motion = a.motion;
        wm.actors[i].clock = Some((motion, now));
        if changed {
            wm.fx_requests.push(FxRequest::ReleaseMotion(i));
        }
        for (kind, p) in due {
            let p0 = p.first().copied().unwrap_or(0);
            match kind {
                1 => {
                    wm.next_fx += 1;
                    let handle = wm.next_fx;
                    wm.fx_requests.push(FxRequest::Object {
                        handle,
                        actor: i,
                        id: p0 as u16,
                        follow: true,
                    });
                }
                8 if p.len() >= 2 => wm.sounds.push(p0 as u16 as u32 * 1000 + p[1] as u16 as u32),
                26 if (0..64).contains(&p0) => wm.actors[i].hidden_parts &= !(1 << p0),
                27 if (0..64).contains(&p0) => wm.actors[i].hidden_parts |= 1 << p0,
                _ => {}
            }
        }
    }
}

/// Key time of a looping motion `ticks` after its start.
fn motion_time(motion: &InterpolatedMotion, ticks: f32) -> f32 {
    let (start, end, fps, ret) = motion.frame_window().unwrap_or((0., 1., 30., 0.));
    let t = start + ticks * fps / 60.;
    if t <= end {
        t
    } else if ret < end {
        ret + (t - end) % (end - ret)
    } else {
        end
    }
}

fn to_scene(p: [f32; 3]) -> Vec3 {
    Vec3::new(p[0], -p[1], -p[2])
}

#[allow(clippy::type_complexity, clippy::too_many_arguments)]
fn present(
    wm: Res<WorldMap>,
    bobs: Option<Res<BobState>>,
    mut roots: Query<(&ActorRoot, &mut Transform, &mut Visibility), Without<ActorPart>>,
    mut parts: Query<(
        &ActorPart,
        &Mesh3d,
        &MeshMaterial3d<FogMaterial>,
        &mut Visibility,
    )>,
    bob_parts: Query<
        (
            &BobPart,
            &Mesh3d,
            &MeshMaterial3d<FogMaterial>,
            &mut Visibility,
        ),
        (Without<ActorRoot>, Without<ActorPart>),
    >,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<FogMaterial>>,
    mut cameras: Query<(&mut Transform, &mut Projection), (With<MapCamera>, Without<ActorRoot>)>,
    mut shown: Local<Option<u64>>,
    mut blends: Local<HashMap<usize, BlendTrack>>,
) {
    if *shown == Some(wm.tick) {
        return;
    }
    *shown = Some(wm.tick);
    let mut poses: HashMap<usize, Vec<[f32; 3]>> = HashMap::new();
    for (root, mut transform, mut visibility) in &mut roots {
        let a = &wm.actors[root.0];
        *visibility = if a.drawn {
            Visibility::Inherited
        } else {
            Visibility::Hidden
        };
        // Engine yaw Y faces (sin Y, 0, cos Y); in the scene that is yaw
        // π − Y about +Y (area.rs).
        *transform = Transform::from_translation(to_scene(a.position))
            .with_rotation(Quat::from_rotation_y(std::f32::consts::PI - a.yaw));
        if let Some(model) = &a.model
            && let Some(m) = a.motions.get(&a.motion)
            && let Ok((pose, evaluated)) =
                model.skinning.sample_evaluated(m, motion_time(m, a.time))
        {
            let bones = model.skinning.bones.len();
            let mut joints: Vec<_> = evaluated.joints.iter().take(bones).cloned().collect();
            let mut positions: Vec<[f32; 3]> = pose
                .position_bits
                .iter()
                .map(|p| p.map(f32::from_bits))
                .collect();
            // A motion change with a blend over 2 ticks crossfades from the
            // last shown joints (FUN_1403c86a0; the clock starts at 2 and
            // counts one tick before the first frame, as the field's Sora).
            let track = blends.entry(root.0).or_default();
            if track.motion != Some(a.motion) {
                track.from = (track.motion.is_some() && a.blend > 2. && !track.joints.is_empty())
                    .then(|| (std::mem::take(&mut track.joints), wm.tick, a.blend));
                track.motion = Some(a.motion);
            }
            if let Some((from, start, length)) = &track.from {
                let weight = ((wm.tick.saturating_sub(*start) as f32 + 3.) / length).clamp(0., 1.);
                if weight >= 1. {
                    track.from = None;
                } else {
                    let parents: Vec<i32> = model
                        .skinning
                        .bones
                        .iter()
                        .map(|b| b.header_i32[1])
                        .collect();
                    let blended = data::motion_eval::blend_poses(&parents, from, &joints, weight);
                    if let Ok(p) = model.skinning.pose_from_worlds(&blended) {
                        positions = p
                            .position_bits
                            .iter()
                            .map(|v| v.map(f32::from_bits))
                            .collect();
                        joints = blended;
                    }
                }
            }
            track.joints = joints;
            poses.insert(root.0, positions);
        }
    }
    for (part, handle, material, mut visibility) in &mut parts {
        let a = &wm.actors[part.actor];
        let Some(model) = &a.model else { continue };
        // Frame triggers 26 / 27 [I: the part mask counts the model's
        // mesh parts in order].
        let want = if part.part < 64 && a.hidden_parts >> part.part & 1 != 0 {
            Visibility::Hidden
        } else {
            Visibility::Inherited
        };
        if *visibility != want {
            *visibility = want;
        }
        // The state tint (FUN_140390510's colour) and the part's blending.
        let color = Color::srgba(a.tint[0], a.tint[1], a.tint[2], 1.);
        let alpha = blend(model.parts[part.part].attributes);
        if let Some(m) = materials.get(&material.0)
            && (m.base.base_color != color || m.base.alpha_mode != alpha)
            && let Some(mut m) = materials.get_mut(&material.0)
        {
            m.base.base_color = color;
            m.base.alpha_mode = alpha;
        }
        let Some(pose) = poses.get(&part.actor) else {
            continue;
        };
        let positions: Vec<[f32; 3]> = model.parts[part.part]
            .indices
            .iter()
            .map(|&i| pose[i as usize])
            .collect();
        visual::room::set_positions(&mut meshes, &handle.0, positions);
    }
    // The map objects: looping motions and the slots' hiding.
    if let Some(bobs) = bobs {
        let mut bob_parts = bob_parts;
        for (part, handle, material, mut visibility) in &mut bob_parts {
            if wm.hidden_bobs.get(part.placement).copied().unwrap_or(false) {
                *visibility = Visibility::Hidden;
                continue;
            }
            let p = &bobs.0.placements[part.placement];
            let Some(Some(bob)) = bobs.0.models.get(p.model) else {
                continue;
            };
            let alpha = blend(bob.model.parts[part.ordinal].attributes);
            if let Some(m) = materials.get(&material.0)
                && m.base.alpha_mode != alpha
                && let Some(mut m) = materials.get_mut(&material.0)
            {
                m.base.alpha_mode = alpha;
            }
            if p.motion < 0 {
                continue;
            }
            let Some(anim) = bob.anims.get(p.motion as usize) else {
                continue;
            };
            let tick = (wm.tick as f32 + p.start) as usize % anim.frames.len().max(1);
            let frame = &anim.frames[tick];
            let positions = bob.model.parts[part.ordinal]
                .indices
                .iter()
                .map(|&i| frame[i as usize].to_array());
            visual::room::set_positions(&mut meshes, &handle.0, positions);
        }
    }
    if let Ok((mut transform, mut projection)) = cameras.single_mut() {
        *transform = Transform::from_translation(to_scene(wm.pose.eye))
            .looking_at(to_scene(wm.pose.target), Vec3::Y);
        if let Projection::Perspective(p) = projection.as_mut() {
            let fov = horizontal_to_vertical(wm.pose.fov);
            if (p.fov - fov).abs() > 1e-6 {
                p.fov = fov;
            }
        }
    }
}

/// An actor's last shown motion and joints, and the crossfade from the
/// joints of the motion it left (joints, start tick, length).
#[derive(Default)]
struct BlendTrack {
    motion: Option<u16>,
    joints: Vec<data::motion_eval::World>,
    from: Option<(Vec<data::motion_eval::World>, u64, f32)>,
}

/// An object's matrix for its effects: its position and the turn its model
/// is drawn with (π − yaw in the scene, a game turn of yaw − π) [I].
fn object_matrix(a: &Actor) -> ::effect::math::Mat {
    ::effect::instance_matrix(a.position, [0., a.yaw - std::f32::consts::PI, 0.], [1.; 3])
}

/// One tick of the effects: the map's elements follow the draw groups
/// (FUN_1402c7010 per group switched on, in bit then table order;
/// FUN_1402c65a0 per group switched off), the objects' starts and stops
/// apply, and every instance steps against the camera.
fn effects_tick(
    mut wm: ResMut<WorldMap>,
    mut fx: ResMut<effects::Effects>,
    mut last: Local<Option<u64>>,
) {
    if *last == Some(wm.tick) {
        return;
    }
    *last = Some(wm.tick);
    let mut step = effects::Step::default();
    let (mask, before) = (wm.bg_mask, fx.map_mask);
    if let Some(pax) = wm.map_pax.clone() {
        for bit in 0..64u8 {
            let (now, was) = (mask >> bit & 1 != 0, before >> bit & 1 != 0);
            if now && !was {
                for (index, element) in pax.elements.iter().enumerate() {
                    if element.group == bit {
                        wm.next_fx += 1;
                        let handle = wm.next_fx;
                        fx.start(handle, &pax, effects::Start::Map { index }, &mut step);
                    }
                }
            } else if was && !now {
                fx.stop_group(bit, &mut step);
            }
        }
    }
    fx.map_mask = mask;
    for request in std::mem::take(&mut wm.fx_requests) {
        match request {
            FxRequest::Stop(handle) => fx.stop(handle, &mut step),
            FxRequest::Release(handle) => fx.release(handle),
            FxRequest::ReleaseMotion(actor) => fx.release_motion(actor),
            FxRequest::Object {
                handle,
                actor,
                id,
                follow,
            } => {
                let a = &wm.actors[actor];
                let Some(pax) = a.pax.clone() else {
                    step.log.push(serde_json::json!({"event":"effect_no_pax",
                        "actor":actor,"element":id}));
                    continue;
                };
                let object = object_matrix(a);
                fx.start(
                    handle,
                    &pax,
                    effects::Start::Object {
                        object,
                        id,
                        follow: follow.then_some(actor),
                    },
                    &mut step,
                );
            }
        }
    }
    fx.follow(|i| wm.actors.get(i).map(object_matrix));
    let view = effects::view(wm.pose.eye, wm.pose.target);
    fx.step(&view, &mut step);
    let tick = wm.tick;
    wm.sounds.extend(step.sounds);
    wm.sound_stops.extend(step.stops);
    for mut v in step.log {
        if let Some(o) = v.as_object_mut() {
            o.insert("tick".into(), tick.into());
        }
        wm.log.push(v);
    }
    if tick.is_multiple_of(120) && fx.running() > 0 {
        let (running, vertices) = (fx.running(), fx.vertices());
        wm.log
            .push(serde_json::json!({"event":"effects_state","tick":tick,
            "running":running,"vertices":vertices,"parts":fx.parts()}));
    }
}

/// The effect meshes, under the map so that they hide with it.
fn present_effects(
    mut commands: Commands,
    mut fx: ResMut<effects::Effects>,
    maps: Query<Entity, With<MapRoot>>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<visual::effect_draw::EffectMaterial>>,
    mut images: ResMut<Assets<Image>>,
) {
    effects::present(
        &mut commands,
        &mut fx,
        maps.single().ok(),
        &mut meshes,
        &mut materials,
        &mut images,
    );
}

/// The character scene: while widgets exist the map hides and each widget
/// model draws at its matrix (FUN_1403939f0) on its motion (FUN_140535450)
/// through the widget camera (FUN_1405352e0: at the origin looking along
/// +Z, focal 512).
#[allow(clippy::type_complexity, clippy::too_many_arguments)]
fn present_widgets(
    wm: Res<WorldMap>,
    mut maps: Query<&mut Visibility, With<MapRoot>>,
    mut roots: Query<(&WidgetRoot, &mut Transform, &mut Visibility), Without<MapRoot>>,
    parts: Query<(&WidgetPart, &Mesh3d, &MeshMaterial3d<FogMaterial>)>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<FogMaterial>>,
    mut cameras: Query<(&mut Transform, &mut Projection), (With<MapCamera>, Without<WidgetRoot>)>,
    mut shown_tick: Local<Option<u64>>,
) {
    if *shown_tick == Some(wm.tick) {
        return;
    }
    *shown_tick = Some(wm.tick);
    let shown = wm.scene.shown();
    for mut v in &mut maps {
        let want = if shown {
            Visibility::Hidden
        } else {
            Visibility::Inherited
        };
        if *v != want {
            *v = want;
        }
    }
    let archive = wm.event.as_ref().and_then(|e| wm.archives.get(&e.id));
    let mut poses: HashMap<usize, Vec<[f32; 3]>> = HashMap::new();
    for (root, mut transform, mut visibility) in &mut roots {
        let w = &wm.widget_models[root.0];
        let widget = wm.scene.slots[w.slot]
            .as_ref()
            .filter(|s| s.object == w.object);
        let (Some(widget), Some(m)) = (widget, wm.scene.matrix(w.slot)) else {
            *visibility = Visibility::Hidden;
            continue;
        };
        *visibility = Visibility::Inherited;
        *transform = Transform::from_matrix(Mat4::from_cols_array_2d(&scene::scene_matrix(&m)));
        let motion = widget
            .track
            .entry
            .and_then(|e| archive?.motions.get(e as usize)?.as_ref());
        if let Some(motion) = motion
            && let Ok((pose, _)) = w
                .model
                .skinning
                .sample_evaluated(motion, scene::key_time(motion, widget.track.time))
        {
            poses.insert(
                root.0,
                pose.position_bits
                    .iter()
                    .map(|p| p.map(f32::from_bits))
                    .collect(),
            );
        }
    }
    for (part, handle, material) in &parts {
        let model = &wm.widget_models[part.model].model;
        let alpha = blend(model.parts[part.part].attributes);
        if let Some(m) = materials.get(&material.0)
            && m.base.alpha_mode != alpha
            && let Some(mut m) = materials.get_mut(&material.0)
        {
            m.base.alpha_mode = alpha;
        }
        let Some(pose) = poses.get(&part.model) else {
            continue;
        };
        let positions: Vec<[f32; 3]> = model.parts[part.part]
            .indices
            .iter()
            .map(|&i| pose[i as usize])
            .collect();
        visual::room::set_positions(&mut meshes, &handle.0, positions);
    }
    if shown && let Ok((mut transform, mut projection)) = cameras.single_mut() {
        *transform = Transform::IDENTITY;
        if let Projection::Perspective(p) = projection.as_mut() {
            p.fov = horizontal_to_vertical(widgets::horizontal_fov());
            p.near = widgets::NEAR;
        }
    } else if let Ok((_, mut projection)) = cameras.single_mut()
        && let Projection::Perspective(p) = projection.as_mut()
        && p.near != MAP_NEAR
    {
        p.near = MAP_NEAR;
    }
}

fn finish(
    mut commands: Commands,
    mut wm: ResMut<WorldMap>,
    mut exit: MessageWriter<AppExit>,
    mut closed: MessageReader<bevy::window::WindowCloseRequested>,
    keys: Res<ButtonInput<KeyCode>>,
) {
    wm.frames = wm.frames.saturating_add(1);
    if let Some(dir) = wm.autoplay.clone()
        && !wm.capture_pending
        && let Some(&at) = AUTOPLAY_CAPTURES.get(wm.captures_taken)
        && wm.tick >= at
    {
        wm.capture_pending = true;
        let index = wm.captures_taken;
        let path = dir.join(format!("world-map-{index:02}.png"));
        let tick = wm.tick;
        commands.spawn(Screenshot::primary_window()).observe(
            move |captured: On<ScreenshotCaptured>, mut wm: ResMut<WorldMap>| {
                if let Ok(image) = captured.image.clone().try_into_dynamic()
                    && let Ok(mut file) =
                        OpenOptions::new().write(true).create_new(true).open(&path)
                {
                    let _ = image.write_to(&mut file, image::ImageFormat::Png);
                }
                wm.capture_pending = false;
                wm.captures_taken += 1;
                wm.log
                    .push(serde_json::json!({"event":"capture","index":index,"tick":tick}));
            },
        );
    }
    // The jump (FUN_1403946d0 → FUN_140152990): the world map ends and,
    // given a field config to start from, the field opens at the target.
    let jumped = wm.jump.is_some() && wm.autoplay.is_none();
    let done = jumped
        || wm.autoplay.is_some()
            && wm.tick >= AUTOPLAY_END
            && wm.captures_taken >= AUTOPLAY_CAPTURES.len();
    if closed.read().count() > 0 || keys.just_pressed(KeyCode::Escape) || done {
        for line in wm.log.drain(..) {
            println!("{line}");
        }
        if let Some(t) = wm.jump {
            println!(
                "{}",
                serde_json::json!({"event":"jump","world":t.world,"area":t.area,"entrance":t.entrance})
            );
            if let Some(h) = &wm.field {
                println!("{}", start_field(h, t, wm.autoplay.is_some()));
            }
        }
        exit.write(AppExit::Success);
    }
}

/// What the landing needs to open the field: the arena config whose
/// sources the field uses, the selected install and the story ranges.
struct FieldHandoff {
    arena: PathBuf,
    root: PathBuf,
    manifest: PathBuf,
    story: Vec<[u32; 2]>,
}

/// Writes the field config for `target` and, unless the run is unattended,
/// starts the field there as a new `kh2fm room` process (the game's area
/// jump FUN_140152990 loads it in place; this rewrite's programs run one
/// window each). A target placed by program id (FUN_14039fbc0) starts the
/// area with that id as its map, battle and event programs.
fn start_field(
    h: &FieldHandoff,
    target: world_state::Target,
    unattended: bool,
) -> serde_json::Value {
    let Some(code) = ::area::progress::WORLDS.get(target.world as usize) else {
        return serde_json::json!({"event":"field_rejected","reason":"world outside the table"});
    };
    if target.area == 0xff {
        return serde_json::json!({"event":"field_rejected","reason":"no area","spawn":target.spawn});
    }
    let programs = target.spawn.map(|p| [p as i16; 3]);
    let config = match crate::launcher::landing_room_config(
        &h.arena,
        code,
        target.area,
        target.entrance,
        programs,
        &h.story,
    ) {
        Ok(c) => c,
        Err(e) => return serde_json::json!({"event":"field_rejected","reason":e}),
    };
    let started = !unattended
        && std::env::current_exe()
            .and_then(|exe| {
                std::process::Command::new(exe)
                    .arg("room")
                    .args([&h.root, &h.manifest, &config])
                    .spawn()
            })
            .is_ok();
    serde_json::json!({"event":"field_start","world":code,"area":target.area,
        "entrance":target.entrance,"programs":programs,"config":config.file_name().map(|n| n.to_string_lossy().into_owned()),
        "started":started})
}

fn parse_story(spec: &str) -> Result<(u32, u32)> {
    let num = |s: &str| -> Result<u32> {
        let s = s.trim();
        Ok(match s.strip_prefix("0x") {
            Some(h) => u32::from_str_radix(h, 16)?,
            None => s.parse()?,
        })
    };
    let (a, b) = spec.split_once(':').ok_or("--story wants <first>:<last>")?;
    Ok((num(a)?, num(b)?))
}

fn run(args: Vec<PathBuf>) -> Result<AppExit> {
    if args.len() < 2 {
        return Err("usage: kh2fm wm <Steam-root> <manifest> [--from <world code>] [--story <first>:<last>]... [--touch <world point>,...] [--seen <world-event bit>,...] [--slot <n>:<state>]... [--at <x>,<z>] [--event <id>]... [--field <test-room config>] [--play-time <h>:<m>] [--world-time <world>:<h>:<m>]... [--load-only] [--autoplay <new-capture-dir> [--decide]]".into());
    }
    // The GPU device starts while the data decodes (used by the app below).
    let early_renderer = crate::early_renderer::EarlyRenderer::start();
    let mut from = "tt".to_owned();
    let mut story = Vec::new();
    let mut load_only = false;
    let mut autoplay = None;
    let mut decide = false;
    let mut touched: Vec<u32> = Vec::new();
    let mut slots: Vec<(usize, u8)> = Vec::new();
    let mut extra_events: Vec<u32> = Vec::new();
    let mut at: Option<[f32; 2]> = None;
    let mut field: Option<PathBuf> = None;
    let mut seen: Vec<u32> = Vec::new();
    let mut play_time: u32 = 0;
    let mut world_times = [0u32; 19];
    let mut options = args[2..].iter();
    while let Some(option) = options.next() {
        let mut value = || -> Result<String> {
            Ok(options
                .next()
                .and_then(|v| v.to_str())
                .ok_or("option value absent")?
                .to_owned())
        };
        match option.to_str() {
            Some("--from") => from = value()?,
            Some("--story") => story.push(parse_story(&value()?)?),
            Some("--load-only") => load_only = true,
            Some("--autoplay") => autoplay = Some(PathBuf::from(value()?)),
            Some("--decide") => decide = true,
            // Play this libretto-wm.bar event after the queued ones.
            Some("--event") => {
                let v = value()?;
                let v = v.trim();
                extra_events.push(match v.strip_prefix("0x") {
                    Some(h) => u32::from_str_radix(h, 16)?,
                    None => v.parse()?,
                });
            }
            // World points to mark touched (save points visited), as a
            // save would hold them.
            // A symbol slot's state (save +0x4158 + n − 1), as a save would
            // hold it (the Gummi routes use slots 1..=9).
            Some("--slot") => {
                let v = value()?;
                let (n, s) = v.split_once(':').ok_or("--slot wants <n>:<state>")?;
                slots.push((n.trim().parse()?, s.trim().parse()?));
            }
            // Start the cursor at this ground position instead.
            Some("--at") => {
                let v = value()?;
                let (x, z) = v.split_once(',').ok_or("--at wants <x>,<z>")?;
                at = Some([x.trim().parse()?, z.trim().parse()?]);
            }
            // A test-room config whose sources the landing's field uses.
            Some("--field") => field = Some(PathBuf::from(value()?)),
            // World-event bits to mark seen (FUN_1403deab0), as a save would
            // hold them.
            Some("--seen") => {
                for b in value()?.split(',') {
                    let b = b.trim();
                    seen.push(match b.strip_prefix("0x") {
                        Some(h) => u32::from_str_radix(h, 16)?,
                        None => b.parse()?,
                    });
                }
            }
            // The total play time a save would hold (save +0x2444), as
            // <hours>:<minutes>.
            Some("--play-time") => {
                let v = value()?;
                let (h, m) = v.split_once(':').ok_or("--play-time wants <h>:<m>")?;
                let (h, m): (u32, u32) = (h.trim().parse()?, m.trim().parse()?);
                play_time = (h * 3600 + m * 60) * 60;
            }
            // A world's own play time (save +0x244c + 4 × world) as
            // <world>:<hours>:<minutes>.
            Some("--world-time") => {
                let v = value()?;
                let mut it = v.split(':').map(|x| x.trim().parse::<u32>());
                let (Some(Ok(w)), Some(Ok(h)), Some(Ok(m))) = (it.next(), it.next(), it.next())
                else {
                    return Err("--world-time wants <world>:<h>:<m>".into());
                };
                if let Some(t) = world_times.get_mut(w as usize) {
                    *t = (h * 3600 + m * 60) * 60;
                }
            }
            Some("--touch") => {
                for p in value()?.split(',') {
                    touched.push(p.trim().parse()?);
                }
            }
            _ => return Err("unknown world map option".into()),
        }
    }
    if story.is_empty() {
        story.push(DEFAULT_STORY);
    }
    let source = Arc::new(ProbeSource::bind_session(&args[0], &args[1])?);
    if let Some(dir) = &autoplay {
        source.output_path(&dir.join("world-map-00.png"))?;
        if !dir.is_dir() || std::fs::read_dir(dir)?.next().is_some() {
            return Err("autoplay capture directory must exist and be empty".into());
        }
    }
    eprintln!("Validating the selected KH2FM source, then loading the world map...");
    source.verify_session()?;
    let from_world = ::area::progress::world_index(&from).ok_or("unknown world code")?;
    let target = world_state::world_map_entry(from_world);
    let mut src = AreaSource::open(Arc::clone(&source), &story)?;
    let (room, mut loaded) = src
        .load(target.world, target.area, target.entrance, [-1; 3])
        .map_err(|e| format!("world map area: {e}"))?;
    let glow = src.glow(target.world, target.area);
    // FUN_14038cf20 → FUN_14038de90(0) → FUN_14038dc60: the queued world
    // events' scripts run on arrival; their world map events (#89) are
    // listed, not played.
    let before_world = src.progress().world.clone();
    let (mut events_queued, set) = src.progress_mut().play_queued_world_events();
    events_queued.extend(extra_events.iter().copied());
    let events = events_queued.clone();
    for &p in &touched {
        src.progress_mut().world.touch_point(p);
    }
    for &b in &seen {
        if let Some(w) = src.progress_mut().world.event_bits.get_mut(b as usize / 32) {
            *w |= 1 << (b % 32);
        }
    }
    for &(n, s) in &slots {
        if let Some(slot) = n
            .checked_sub(1)
            .and_then(|i| src.progress_mut().world.symbol_slots.get_mut(i))
        {
            *slot = s;
        }
    }
    let mut library = Library::open(&source)?;
    let mut actors = Vec::new();
    let mut cursor = None;
    type Loaded = (
        Option<Arc<InspectionModel>>,
        MotionMap,
        TriggerMap,
        Vec<object_collision::Entry>,
    );
    let mut models: HashMap<String, Loaded> = HashMap::new();
    let mut paxes: HashMap<String, Option<Arc<object_effect::Pax>>> = HashMap::new();
    let mut untextured = Vec::new();
    for s in &loaded.spawns {
        let Some(entry) = src.object(s.entity.object) else {
            continue;
        };
        if entry.object_type != 0x11 {
            continue;
        }
        let kind = match entry.sub_type {
            0 => match objects::symbol(s.entity.object) {
                Some(e) => Kind::Symbol(*e),
                None => continue,
            },
            1 => Kind::Gumi(s.entity.arg1 as u8),
            2 => Kind::Cursor,
            _ => continue,
        };
        let name = String::from_utf8_lossy(&entry.model_name).into_owned();
        if !models.contains_key(&name) {
            let (model, failed) = read_model(&source, &mut library, &name)
                .map_err(|e| format!("obj/{name}.mdlx: {e}"))?;
            if let Some(reason) = failed {
                untextured.push(serde_json::json!({"model": name, "reason": reason}));
            }
            let model = model.map(Arc::new);
            let ids: &[u16] = match kind {
                Kind::Cursor => &[
                    CURSOR_IDLE,
                    CURSOR_FAST,
                    CURSOR_RIDE,
                    cursor::MOTION_DASH,
                    cursor::MOTION_WARP,
                ],
                Kind::Symbol(_) => &[0, SYMBOL_CLOSED, SYMBOL_OPEN],
                Kind::Gumi(_) => &[0],
            };
            let motions = match library.find(&format!("obj/{name}.mset")) {
                Some(f) => read_motions(&library.read_found(&f)?, ids)?,
                None => (HashMap::new(), HashMap::new()),
            };
            let (motions, triggers) = motions;
            let shapes = library
                .read(&format!("obj/{name}.mdlx"))
                .map(|a| read_shapes(&a))
                .unwrap_or_default();
            models.insert(name.clone(), (model, motions, triggers, shapes));
            paxes.insert(
                name.clone(),
                read_object_pax(&mut library, &name).map(Arc::new),
            );
        }
        let (model, motions, triggers, shapes) = &models[&name];
        if matches!(kind, Kind::Cursor) {
            cursor = Some(actors.len());
        }
        actors.push(Actor {
            kind,
            model: model.clone(),
            motions: motions.clone(),
            triggers: triggers.clone(),
            position: s.entity.position,
            yaw: s.yaw,
            motion: match kind {
                Kind::Symbol(_) => SYMBOL_CLOSED,
                _ => 0,
            },
            time: 0.,
            drawn: true,
            tint: [1.; 4],
            shapes: shapes.clone(),
            reacts: false,
            pax: paxes.get(&name).cloned().flatten(),
            hidden_parts: 0,
            clock: None,
            held: [None; 3],
            episode_effect: None,
            applied: (0, 0),
            blend: 0.,
        });
    }
    let cursor = cursor.ok_or("the world map has no cursor at this entrance")?;
    if let Some([x, z]) = at {
        actors[cursor].position[0] = x;
        actors[cursor].position[2] = z;
    }
    // The cursor's movement block (FUN_14040a440 defaults, then the PRTY
    // record its objentry +0x4e names, FUN_14040a480).
    let system = library.read("03system.bin")?;
    let system_bar = bar::read_bar(&system)?;
    let entry = |bytes: &[u8], table: &bar::BarInventory, name: &[u8; 4]| -> Result<Vec<u8>> {
        let e = table
            .entries
            .iter()
            .find(|e| e.name_bytes == *name)
            .ok_or("03system entry missing")?;
        Ok(bytes[e.offset as usize..e.offset as usize + e.size as usize].to_vec())
    };
    // 'cmd': 0x30-byte records {u16 id, u16 execute, u16, u8, u8, i32
    // text, u32 flags, ...} after a version and a count.
    let (commands, command_texts): (HashMap<u16, u16>, HashMap<u16, (u32, u32)>) = {
        let c = entry(&system, &system_bar, b"cmd\0")?;
        let n = u32::from_le_bytes(c.get(4..8).ok_or("cmd count")?.try_into()?) as usize;
        let records: Vec<(u16, u16, u32, u32)> = (0..n)
            .filter_map(|k| {
                let r = c.get(8 + k * 0x30..8 + k * 0x30 + 16)?;
                Some((
                    u16::from_le_bytes([r[0], r[1]]),
                    u16::from_le_bytes([r[2], r[3]]),
                    u32::from_le_bytes([r[8], r[9], r[10], r[11]]),
                    u32::from_le_bytes([r[12], r[13], r[14], r[15]]),
                ))
            })
            .collect();
        (
            records.iter().map(|&(id, exec, _, _)| (id, exec)).collect(),
            records
                .iter()
                .map(|&(id, _, text, flags)| (id, (text, flags)))
                .collect(),
        )
    };
    let pref = entry(&system, &system_bar, b"pref")?;
    let pref_bar = bar::read_bar(&pref)?;
    let sstm_bytes = entry(&pref, &pref_bar, b"sstm")?;
    let sstm_offset = u32::from_le_bytes(sstm_bytes.get(4..8).ok_or("SSTM pointer")?.try_into()?);
    let sstm = sstm_bytes
        .get(sstm_offset as usize..)
        .and_then(actor_physics::Sstm::read)
        .ok_or("SSTM record truncated")?;
    // SSTM +0x4c: the manual lock's range (FUN_1403bea30).
    let lock_range = sstm_bytes
        .get(sstm_offset as usize + 0x4c..sstm_offset as usize + 0x50)
        .map(|b| f32::from_le_bytes([b[0], b[1], b[2], b[3]]))
        .ok_or("SSTM lock range absent")?;
    let mut params = ObjParams::defaults(sstm.fall_max);
    let mut dash_speed = 0.;
    let cursor_entry = src
        .object(objects::CURSOR)
        .ok_or("cursor objentry missing")?;
    let prty_index = cursor_entry.neo_moveset as i16 as i32;
    if prty_index != 0 {
        let prty = data::party_records::read_prty(&entry(&pref, &pref_bar, b"prty")?)?;
        let offset =
            actor_physics::prty_offset(&prty.offsets, prty_index).ok_or("PRTY record absent")?;
        let record = prty
            .records
            .iter()
            .find(|r| r.offset == offset && !r.overlaps_table)
            .ok_or("PRTY record absent")?;
        params.apply_prty(&record.words);
        // +0x24: ActionDash's speed (FUN_140563a70 / FUN_1405639e0).
        dash_speed = f32::from_bits(record.words[9]);
    }
    let body = map_body(&actors[cursor].shapes).unwrap_or(area::SORA_BODY);
    let collision = Some(loaded.take_collision());
    let start = actors[cursor].position;
    let initial = Pose {
        eye: [start[0], start[1] - 2300., start[2] - 3000.],
        target: [start[0], start[1] - 500., start[2]],
        fov: world_map::camera::FOV_DEGREES.to_radians(),
    };
    let mut wm = WorldMap {
        actors,
        cursor,
        velocity: [0.; 2],
        sampler: Sampler::default(),
        params,
        camera: ObjCamera::new(initial),
        pose: initial,
        eh_symbol: None,
        collision,
        body,
        bg_mask: loaded.state.setup.bg_mask,
        hidden_bobs: Vec::new(),
        tick: 0,
        frames: 0,
        stick: [0.; 3],
        right: [0.; 3],
        last_raw: 0,
        autoplay: autoplay.clone(),
        autoplay_decide: decide,
        program_areas: library
            .read("07localset.bin")
            .ok()
            .and_then(|b| data::world_map::read_program_areas(&b).ok())
            .unwrap_or_default(),
        field: field.map(|arena| FieldHandoff {
            arena,
            root: args[0].clone(),
            manifest: args[1].clone(),
            story: story.iter().map(|&(a, b)| [a, b]).collect(),
        }),
        captures_taken: 0,
        capture_pending: false,
        log: Vec::new(),
        raw: 0,
        pad: controller::Pad::default(),
        reaction: None,
        reaction_cmd: None,
        landing: None,
        info: None,
        route_info: None,
        jump: None,
        commands,
        sounds: Vec::new(),
        event_queue: events_queued.iter().copied().collect(),
        event: None,
        event_camera: None,
        window: None,
        fade: visual::fade::Fade::default(),
        after_world: None,
        tables: None,
        eh_flag: false,
        event_ticks: 0,
        scene: widgets::Scene::default(),
        archives: HashMap::new(),
        widget_models: Vec::new(),
        voices: Vec::new(),
        map_pax: loaded.pax.clone(),
        fx_requests: Vec::new(),
        next_fx: 0,
        sound_stops: Vec::new(),
        new_worlds: Vec::new(),
        events_ended: false,
        riding: false,
        action: Action::Ship,
        dash_speed,
        lock_range,
        lock: None,
        lock_camera: None,
        cursor_fx: [None; 3],
        floor: 0,
        command_texts,
        command: None,
        reticle: lock_on::Reticle::default(),
        reticle_draw: None,
        total: None,
        play_time,
        leaving: None,
        wall: (0xffff, 0.),
        knock: None,
        camp_up: false,
        journal_request: false,
        devices: (true, None),
        world_times,
    };
    // The jump that opened the map was mode 1 (FUN_140154e20 → FUN_140152990
    // with flags | 1): the area fades in from black over n = 15
    // (FUN_1401561b0 → FUN_140156c90).
    wm.fade.fade_in(15, false);
    let progress = src.progress();
    // FUN_14038cf20 creates the objects before its task runs the queued
    // events' scripts: they show the state from before the scripts until
    // an event refreshes them (op 0x105 / 0x113).
    let eh_flag = progress.test(objects::EH_EFFECT_FLAG);
    apply_world_state(
        &mut wm,
        &before_world,
        progress.world_tables(),
        eh_flag,
        loaded.bobs.placements.len(),
    );
    if wm.event_queue.is_empty() {
        apply_world_state(
            &mut wm,
            &progress.world,
            progress.world_tables(),
            eh_flag,
            loaded.bobs.placements.len(),
        );
    }
    wm.after_world = Some(progress.world.clone());
    wm.tables = progress.world_tables().cloned();
    wm.eh_flag = eh_flag;
    let records: Vec<_> = progress
        .world
        .records
        .iter()
        .map(|r| (r.state, r.landing_set, r.flags))
        .collect();
    let summary = serde_json::json!({"event":"world_map_loaded","entrance":target.entrance,
        "from":from,"story":story,"queued_events":events,"draw_group_set":set,
        "records":records,"symbol_slots":progress.world.symbol_slots,
        "bg_mask":format!("{:#018x}",wm.bg_mask),
        "hidden_map_objects":wm.hidden_bobs.iter().enumerate().filter(|(_, h)| **h).map(|(i, _)| i).collect::<Vec<_>>(),
        "actors":wm.actors.iter().map(|a| serde_json::json!({
            "kind":match a.kind { Kind::Cursor => "cursor".to_owned(), Kind::Symbol(e) => format!("symbol {:#x} world {}", e.object, e.world), Kind::Gumi(r) => format!("gumi {r}") },
            "drawn":a.drawn,"reacts":a.reacts,"yaw":a.yaw,"motion":a.motion,"motions":a.motions.keys().collect::<Vec<_>>(),"position":a.position,
            "reaction":a.shapes.iter().filter(|s| s.kind == object_collision::REACTION).map(|s| (shape_at(a, s), s.radius, s.argument)).collect::<Vec<_>>()})).collect::<Vec<_>>(),
        "cursor_motions":wm.actors[cursor].motions.iter().map(|(k, m)| (k, m.frame_window().ok())).collect::<Vec<_>>(),
        "cursor_params":{"walk":params.walk,"run":params.run,"turn":params.turn,"inertia":params.word10,"drag":params.word11,"prty":prty_index,"dash":wm.dash_speed,"lock_range":wm.lock_range},
        "body":{"radius":body.radius,"height":body.height,"offset":body.offset},
        "map_objects":loaded.bobs.placements.iter().enumerate().map(|(i, p)| serde_json::json!({
            "index":i,"model":p.model,"group":p.group,"at":p.translation.to_array().map(|v| v.round()),
            "shown":!wm.hidden_bobs[i] && wm.bg_mask >> p.group & 1 != 0})).collect::<Vec<_>>(),
        "untextured":untextured});
    println!("{summary}");
    let worlds: Vec<u8> = wm.actors.iter().filter_map(actor_world).collect();
    let routes: Vec<u8> = wm.actors.iter().filter_map(actor_route).collect();
    let ui = ui::Ui::load(&source, &mut library, &src, &worlds, &routes)?;
    // FUN_14038cf20 → FUN_14038e880: dk::WM_TOTAL_INFO at the area start.
    wm.total = Some(TotalInfo::new(
        &ui.layout.layout.sequences[world_map_menu::SEQUENCE],
    ));
    // The cursor's deck (FUN_1403f3ac0(mgr, 0, 0) → FUN_1403b2800(mgr, 10)):
    // a list deck of one row, rebuilt each tick (FUN_1403947a0).
    wm.command = ui.command.as_ref().map(|(name, sheet)| {
        let mut menu = CommandMenu::new(&sheet.sequence, Vec::new());
        menu.list = Some(vec![cursor::SELECTOR]);
        menu.list_mode = field_command_menu::WORLD_MAP_DECK;
        menu.layout_style = Some(field_command_menu::layout_style(name, true));
        menu
    });
    println!(
        "{}",
        serde_json::json!({"event":"hud_prepared",
            "command":ui.command.as_ref().map(|(n, s)| (n.clone(), s.sequence.groups.len())),
            "field":ui.field.as_ref().map(|(n, s)| (n.clone(), s.sequence.groups.len())),
            "dash":String::from_utf8_lossy(&ui.message(cursor::MSG_DASH)),
            "warp":String::from_utf8_lossy(&ui.message(cursor::MSG_WARP)),
            "land":String::from_utf8_lossy(&ui.message(0x46a5)),
            "route":String::from_utf8_lossy(&ui.message(0x46a6))})
    );
    wm.new_worlds = ui
        .worlds
        .iter()
        .filter(|(_, p)| p.episodes.iter().any(|e| e.new))
        .map(|(&w, _)| w)
        .collect();
    wm.new_worlds.sort_unstable();
    println!(
        "{}",
        serde_json::json!({"event":"effects_prepared","new_worlds":wm.new_worlds,
            "map_pax":wm.map_pax.as_ref().map(|p| p.elements.iter().map(|e| (e.id, e.group, e.effect_number, e.sound, e.category)).collect::<Vec<_>>()),
            "triggers":wm.actors.iter().map(|a| a.triggers.iter().map(|(m, l)| (*m, l.iter().map(|t| (t.0, t.1, t.2.clone())).collect::<Vec<_>>())).collect::<Vec<_>>()).collect::<Vec<_>>(),
            "objects":wm.actors.iter().map(|a| a.pax.as_ref().map(|p| (p.name.clone(), p.elements.iter().map(|e| (e.id, e.effect_number, e.flags, e.sound)).collect::<Vec<_>>()))).collect::<Vec<_>>()})
    );
    let prepared: Vec<_> = worlds
        .iter()
        .filter_map(|w| ui.worlds.get(w).map(|p| (w, p)))
        .map(|(w, p)| serde_json::json!({"world":w,"cells":p.grid.cells.iter().map(|c| c.map(|c| (c.point, c.picture, c.blocked.is_some()))).collect::<Vec<_>>(),
            "default":format!("{:?}",p.default_target),"episodes":p.episodes.len(),"visits":p.visits,"battle_level":p.battle_level,"name_bytes":p.name.len()}))
        .collect();
    println!(
        "{}",
        serde_json::json!({"event":"landing_prepared","worlds":prepared,
        "layout":{"props":ui.layout.layout.props.len(),"scenes":ui.layout.layout.scenes.len(),"images":ui.layout.image_sizes},
        "pictures":ui.pictures.len(),"ws_i":ui.ws_i.is_some()})
    );
    // The events' character scenes: each event's archive (op 0x101), its
    // voices, and the models its widgets show (op 0x102).
    let mut voice_files: Vec<((u32, u32), String)> = Vec::new();
    let mut widget_names: HashMap<u32, Option<Arc<InspectionModel>>> = HashMap::new();
    let mut scene_log = Vec::new();
    // The queued events, and those the land reaction may play.
    let mut scene_events = events.clone();
    scene_events.extend(objects::LAND_EVENTS.iter().map(|e| e.3));
    for &id in &scene_events {
        let Some(script) = ui.events.get(&(id as u16)) else {
            continue;
        };
        if script.contains(&events::Op::Archive) && !wm.archives.contains_key(&id) {
            let name = events::archive_name(id, scene::LANGUAGE);
            match library.read(&name) {
                Ok(bytes) => {
                    let archive = scene::read_archive(&bytes, &mut scene_log)?;
                    for (&entry, file) in &archive.voices {
                        voice_files.push(((id, entry), file.clone()));
                    }
                    scene_log.push(serde_json::json!({"event":"event_archive","id":id,
                        "name":name,"entries":archive.clips.len(),
                        "motions":archive.clips.iter().filter(|c| c.is_some()).count(),
                        "voices":archive.voices.len()}));
                    wm.archives.insert(id, archive);
                }
                Err(e) => scene_log.push(serde_json::json!({"event":"event_archive_absent",
                    "id":id,"name":name,"error":e.to_string()})),
            }
        }
        for (slot, object) in scene::widget_objects(script) {
            if wm
                .widget_models
                .iter()
                .any(|w| w.slot == slot && w.object == object)
            {
                continue;
            }
            if let std::collections::hash_map::Entry::Vacant(v) = widget_names.entry(object) {
                let model = match src.object(object) {
                    Some(entry) => {
                        let name = String::from_utf8_lossy(&entry.model_name).into_owned();
                        let (model, failed) = read_model(&source, &mut library, &name)
                            .map_err(|e| format!("obj/{name}.mdlx: {e}"))?;
                        if let Some(reason) = failed {
                            untextured.push(serde_json::json!({"model": name, "reason": reason}));
                        }
                        model.map(Arc::new)
                    }
                    None => None,
                };
                v.insert(model);
            }
            if let Some(model) = widget_names[&object].clone() {
                wm.widget_models.push(WidgetModel {
                    slot,
                    object,
                    model,
                });
            }
        }
    }
    println!(
        "{}",
        serde_json::json!({"event":"scenes_prepared","archives":scene_log,
        "widgets":wm.widget_models.iter().map(|w| (w.slot, format!("{:#x}", w.object))).collect::<Vec<_>>(),
        "voices":voice_files.len(),"untextured":untextured})
    );
    if load_only {
        return Ok(AppExit::Success);
    }
    // The system sounds (se000) and the area's field BGM (its arif pair,
    // FUN_1403a5960), decoded on a worker thread as the title does.
    let mut banks = visual::sound::Banks::default();
    // se000 (menus) and se016 (op 0x105's 16008) [I: the banks the world
    // map loads were not traced; these are the ones its requests name].
    for bank in ["se/se000.win32.scd", "se/se016.win32.scd"] {
        if let Ok(bytes) = library.read(bank) {
            banks.insert(bytes)?;
        }
    }
    // The banks the effects' own sounds name.
    let mut effect_banks: Vec<u32> = wm
        .map_pax
        .iter()
        .chain(wm.actors.iter().filter_map(|a| a.pax.as_ref()))
        .flat_map(|p| p.elements.iter())
        .filter(|e| e.sound > 0)
        .map(|e| e.sound as u32 / 1000)
        .collect();
    effect_banks.sort_unstable();
    effect_banks.dedup();
    for bank in effect_banks {
        if !banks.numbers().contains(&(bank as u16))
            && let Ok(bytes) = library.read(&format!("se/se{bank:03}.win32.scd"))
        {
            banks.insert(bytes)?;
        }
    }
    let texcommon = object_effect::read_dpd_file(&mut library, "effect/texcommon.dpd").ok();
    // The banks the widget motions' sound triggers name.
    let mut trigger_banks: Vec<u32> = wm
        .archives
        .values()
        .flat_map(|a| a.clips.iter().flatten())
        .flat_map(|c| c.sounds.iter().map(|&(_, id)| id / 1000))
        .collect();
    trigger_banks.sort_unstable();
    trigger_banks.dedup();
    for bank in trigger_banks {
        if banks.numbers().contains(&(bank as u16)) {
            continue;
        }
        if let Ok(bytes) = library.read(&format!("se/se{bank:03}.win32.scd")) {
            banks.insert(bytes)?;
        }
    }
    let mut voices = HashMap::new();
    for (key, file) in voice_files {
        match library
            .read(&file)
            .map_err(|e| e.to_string())
            .and_then(|b| crate::cutscene::decode_voice(&b).map_err(|e| e.to_string()))
        {
            Ok(clip) => {
                voices.insert(key, clip);
            }
            Err(e) => eprintln!("{file}: voice unreadable: {e}"),
        }
    }
    let music = match std::mem::take(&mut loaded.music.slots[0]) {
        Some(Some(bytes)) => Some(visual::sound::decode_music_in_background(bytes)),
        _ => None,
    };
    let output = match audio::Output::open() {
        Ok(o) => Some(o),
        Err(e) => {
            eprintln!("Sound output unavailable: {e}");
            None
        }
    };
    // The camp and journal from the field configuration (the launcher's
    // `--field`); without one START does nothing here.
    let camp = match &wm.field {
        Some(h) => match crate::test_room::world_map_camp::load(
            Arc::clone(&source),
            &h.arena,
            wm.autoplay.is_some(),
        ) {
            Ok(c) => Some(c),
            Err(e) => {
                eprintln!("Camp menu unavailable on the world map: {e}");
                None
            }
        },
        None => None,
    };
    println!(
        "{}",
        serde_json::json!({"event":"camp_prepared","camp":camp.is_some()})
    );
    let mut app = App::new();
    app.insert_resource(CampHost(camp));
    app.insert_resource(wm)
        .insert_resource(AudioRes {
            output,
            banks,
            music,
            bgm_started: false,
            voices,
        })
        .insert_resource(UiRes {
            ui,
            images: Vec::new(),
            pictures: Vec::new(),
            font: 0,
            icon: 0,
            window: None,
            event_font: None,
            white: 0,
            command: None,
            fiel: None,
        })
        .insert_resource(StartupRes(Some(Startup {
            room,
            area: loaded,
            glow,
        })))
        .insert_resource(effects::Effects::new(texcommon))
        .insert_resource(Time::<Fixed>::from_hz(TICK_HZ))
        .insert_resource(ClearColor(Color::BLACK))
        .add_plugins(DefaultPlugins.set(WindowPlugin {
            primary_window: Some(Window {
                title: "Kingdom Hearts II Re:Written (world map; rewrite) | WASD/stick fly, F land, R lock on, Space dash/warp, Enter/Space/arrows/R/LShift menu, Esc exits"
                    .into(),
                resolution: (1280, 720).into(),
                ..default()
            }),
            exit_condition: bevy::window::ExitCondition::DontExit,
            close_when_requested: false,
            ..default()
        }).set(early_renderer.plugin()))
        .add_plugins(FogPlugin)
        .add_plugins(visual::effect_draw::EffectPlugin)
        .add_plugins(Ui2dPlugin {
            order: 1,
            overlay: true,
        })
        .add_systems(Startup, (setup, setup_ui))
        .add_systems(
            FixedUpdate,
            (read_stick, tick, camp_tick, effects_tick, play_audio).chain(),
        )
        .add_systems(
            Update,
            (
                apply_draw_groups,
                present,
                present_widgets,
                present_effects,
                draw_ui,
                finish,
            )
                .chain(),
        );
    Ok(app.run())
}

/// Runs this program with its arguments; `interactive` = started from the
/// launcher menu.
pub fn main(args: Vec<PathBuf>, _interactive: bool) -> std::process::ExitCode {
    match run(args) {
        Ok(AppExit::Success) => std::process::ExitCode::SUCCESS,
        Ok(_) => std::process::ExitCode::FAILURE,
        Err(e) => {
            eprintln!("World map rejected: {e}");
            std::process::ExitCode::FAILURE
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fov_conversion_matches_the_follow_camera() {
        let v = horizontal_to_vertical(65f32.to_radians());
        assert!((v.to_degrees() - 51.08).abs() < 0.01, "{}", v.to_degrees());
    }

    #[test]
    fn story_ranges_parse_hex_and_decimal() {
        assert_eq!(parse_story("0x800:0x895").unwrap(), (0x800, 0x895));
        assert_eq!(parse_story("2048:2049").unwrap(), (2048, 2049));
        assert!(parse_story("0x800").is_err());
    }

    #[test]
    fn raw_motion_sets_index_entries_directly() {
        // A two-entry BAR without the variant flag: number 1 is entry 1.
        let anb = vec![0u8; 4];
        let mut bytes = b"BAR\x01".to_vec();
        bytes.extend(2u32.to_le_bytes());
        bytes.extend([0u8; 8]);
        for i in 0..2u32 {
            bytes.extend(17u16.to_le_bytes());
            bytes.extend(0u16.to_le_bytes());
            bytes.extend(b"00A\0");
            bytes.extend((0x30 + i * 4).to_le_bytes());
            bytes.extend((if i == 1 { 4u32 } else { 0 }).to_le_bytes());
        }
        bytes.extend(&anb);
        bytes.extend(&anb);
        // Entry 1 is not a valid ANB, so reading it fails; entry 0 is absent
        // (size 0) and skipped without an error.
        assert!(read_motions(&bytes, &[0]).unwrap().0.is_empty());
        assert!(read_motions(&bytes, &[1]).is_err());
    }
}
