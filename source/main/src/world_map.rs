//! `kh2fm wm`: the world map (field area wm00, world 15) from the selected
//! installation. The area loads like any other (map, ARD spawns, map
//! objects); the world symbols and Gummi ships take their looks from the
//! story state (worldmap::Symbol / worldmap::Gumi, the symbol slots' map
//! objects and draw groups), and the cursor (worldmap::Player, `WM_CURSOR`)
//! flies with the stick under worldmap::ObjCamera.
//!
//! Not here yet: landing on a world and the room select (dk::WM_LANDING,
//! #87), the info panels, the Gummi routes (#88), the world-opening events
//! and the objects' effects (#89).
use crate::area::{self, AreaSource, LoadedArea};
use crate::test_room::{BobPart, RoomPart, spawn_bobs, spawn_model, spawn_room};
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
    object_collision,
    path_lookup::Library,
    probe_source::ProbeSource,
};
use player::actor_physics::{self, ObjParams, Sampler};
use std::{collections::HashMap, fs::OpenOptions, path::PathBuf, sync::Arc};
use visual::area_fog::{FogMaterial, FogPlugin};
use visual::{controller, input};
use world_map::{
    camera::{ObjCamera, Pose},
    objects::{self, Switch},
};

type Result<T> = std::result::Result<T, Box<dyn std::error::Error>>;

const TICK_HZ: f64 = 60.;
/// The cursor's motions: idle and the fast one worldmap::ActionShip picks
/// above the mean of the walk and run speeds (FUN_140563880).
const CURSOR_IDLE: u16 = 0;
const CURSOR_FAST: u16 = 0x99;
/// The symbols' motions (FUN_140390510).
const SYMBOL_CLOSED: u16 = 0x97;
const SYMBOL_OPEN: u16 = 0xa1;
/// The story range the program sets by default: Sora's first Twilight Town
/// visit, as the field tests use [test-room choice].
const DEFAULT_STORY: (u32, u32) = (0x800, 0x895);
/// Unattended runs: a fixed input script and the ticks captured.
const AUTOPLAY_CAPTURES: [u64; 6] = [30, 150, 240, 360, 480, 600];
const AUTOPLAY_END: u64 = 620;
const WARM_UP_FRAMES: u32 = 90;

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
    /// Motions by the exe's motion number (FUN_1403c6dc0).
    motions: HashMap<u16, Arc<InterpolatedMotion>>,
    /// Engine coordinates (Y down) and engine yaw.
    position: [f32; 3],
    yaw: f32,
    motion: u16,
    /// Ticks since the motion started.
    time: f32,
    drawn: bool,
    tint: [f32; 4],
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
    autoplay: Option<PathBuf>,
    captures_taken: usize,
    capture_pending: bool,
    log: Vec<serde_json::Value>,
}

struct Startup {
    room: visual::room::LoadedRoom,
    area: LoadedArea,
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
/// the number is the entry index.
fn read_motions(mset: &[u8], ids: &[u16]) -> Result<HashMap<u16, Arc<InterpolatedMotion>>> {
    let table = bar::read_bar(mset)?;
    let variants = table.header_field_12 & 1 != 0;
    let mut out = HashMap::new();
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
        out.insert(id, Arc::new(motion::read_anb(anb, slot)?.motion));
    }
    Ok(out)
}

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

/// The map body of an object: its object-collision entry of kind 4 (the
/// cursor's is the same as Sora's: r 90, h 90, centre 115 up).
fn map_body(archive: &[u8]) -> Option<Body> {
    let table = bar::read_bar(archive).ok()?;
    let e = table.entries.iter().find(|e| e.entry_type == 23)?;
    let shapes =
        object_collision::read(&archive[e.offset as usize..e.offset as usize + e.size as usize])
            .ok()?;
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
    progress: &::area::progress::Progress,
    tables: Option<&world_state::WorldTables>,
    bobs: usize,
) {
    let state = &progress.world;
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
    let eh_flag = progress.test(objects::EH_EFFECT_FLAG);
    wm.eh_symbol = None;
    for a in &mut wm.actors {
        match a.kind {
            Kind::Cursor => {}
            Kind::Symbol(entry) => {
                let record = state.record(entry.world).copied().unwrap_or_default();
                let look = objects::symbol_look(entry.world, &record, eh_flag);
                a.drawn = look.drawn;
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
    let Startup { room, area } = startup.0.take().expect("startup data");
    let root = commands
        .spawn((Transform::default(), Visibility::default()))
        .id();
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
        Camera3d::default(),
        Projection::Perspective(PerspectiveProjection {
            fov: horizontal_to_vertical(world_map::camera::FOV_DEGREES.to_radians()),
            near: 5.,
            far: 200_000.,
            ..default()
        }),
        Transform::default(),
        Msaa::Off,
        Tonemapping::None,
        DebandDither::Disabled,
        NoIndirectDrawing,
        MapCamera,
        RoomPart,
    ));
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
    mut wm: ResMut<WorldMap>,
) {
    if wm.autoplay.is_some() {
        return;
    }
    let (data, _, _) = input::read_devices(&keys, &mouse, 0., &pads);
    wm.stick = controller::analog(data.left);
}

/// Scripted stick for unattended runs: north, east, then south-west.
fn autoplay_stick(tick: u64) -> [f32; 3] {
    match tick {
        40..160 => [0., -1., 1.],
        160..280 => [1., 0., 1.],
        280..400 => [-0.7, 0.7, 1.],
        400..460 => [0., -1., 0.5],
        _ => [0., 0., 0.],
    }
}

fn wrap_angle(a: f32) -> f32 {
    ::area::wrap(a)
}

/// One 60 Hz tick: the cursor (worldmap::Player with worldmap::ActionShip),
/// the objects' motion clocks and the camera.
fn tick(mut wm: ResMut<WorldMap>) {
    if wm.frames < WARM_UP_FRAMES {
        return;
    }
    wm.tick += 1;
    if wm.autoplay.is_some() {
        let t = wm.tick;
        wm.stick = autoplay_stick(t);
    }
    // FUN_1403a8fa0's move request: the stick direction (up = away from
    // the camera, which looks along +Z) at the walk speed below magnitude
    // 0.8 and the run speed from it.
    let [x, y, magnitude] = wm.stick;
    let len = x.hypot(y);
    let direction = if len > 0. {
        [x / len, -y / len]
    } else {
        [0.; 2]
    };
    let speed = if magnitude <= 0. {
        0.
    } else if magnitude < actor_physics::RUN_MAGNITUDE {
        wm.params.walk
    } else {
        wm.params.run
    };
    let ci = wm.cursor;
    // FUN_1403b4780: the facing turns at +0x134 per tick toward the move.
    if speed > 0. {
        let target = direction[0].atan2(direction[1]);
        let yaw = wm.actors[ci].yaw;
        let delta = wrap_angle(target - yaw);
        let turn = wm.params.turn;
        wm.actors[ci].yaw = wrap_angle(yaw + delta.clamp(-turn, turn));
    }
    // FUN_140563840 → FUN_1403b7ad0 with the PRTY record's +0x28 inertia
    // and +0x2c drag, run by FUN_1403b89a0 every second frame.
    if wm.sampler.due(speed) {
        let (inertia, drag) = (wm.params.word10, wm.params.word11);
        let velocity = actor_physics::steer(wm.velocity, direction, speed, inertia, drag);
        wm.velocity = velocity;
        let dt = actor_physics::PHYSICS_STEP;
        wm.sampler.store([velocity[0] * dt, 0., velocity[1] * dt]);
    }
    let d = wm.sampler.displacement();
    let start = wm.actors[ci].position;
    let moved = match &wm.collision {
        Some(c) if d[0] != 0. || d[2] != 0. => {
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
            .position
        }
        _ => [start[0] + d[0], start[1], start[2] + d[2]],
    };
    // The cursor holds its height (ActionShip moves it in the plane).
    wm.actors[ci].position = [moved[0], start[1], moved[2]];
    // FUN_140563880: the fast motion above (walk + run) / 2, else idle,
    // with an 8-tick blend [the blend is not modelled].
    let fast = speed > 0. && (wm.params.walk + wm.params.run) * 0.5 <= speed;
    let wanted = if fast { CURSOR_FAST } else { CURSOR_IDLE };
    if wm.actors[ci].motion != wanted {
        wm.actors[ci].motion = wanted;
        wm.actors[ci].time = 0.;
    }
    for a in &mut wm.actors {
        a.time += actor_physics::DT;
    }
    let cursor = wm.actors[ci].position;
    let (eh, now) = (wm.eh_symbol, wm.pose);
    wm.pose = wm.camera.update(cursor, eh, now);
    let t = wm.tick;
    if t.is_multiple_of(60) {
        let velocity = wm.velocity;
        wm.log.push(serde_json::json!({"event":"cursor","tick":t,
            "position":cursor,"motion":wanted,"velocity":velocity}));
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
    parts: Query<(&ActorPart, &Mesh3d, &MeshMaterial3d<FogMaterial>)>,
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
    mut cameras: Query<&mut Transform, (With<MapCamera>, Without<ActorRoot>)>,
    mut shown: Local<Option<u64>>,
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
            && let Ok((pose, _)) = model.skinning.sample_evaluated(m, motion_time(m, a.time))
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
        let a = &wm.actors[part.actor];
        let Some(model) = &a.model else { continue };
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
        if let Some(mut mesh) = meshes.get_mut(&handle.0) {
            visual::room::update_positions(&mut mesh, positions);
        }
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
            if let Some(mut mesh) = meshes.get_mut(&handle.0) {
                visual::room::update_positions(&mut mesh, positions);
            }
        }
    }
    if let Ok(mut transform) = cameras.single_mut() {
        *transform = Transform::from_translation(to_scene(wm.pose.eye))
            .looking_at(to_scene(wm.pose.target), Vec3::Y);
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
    let done = wm.autoplay.is_some()
        && wm.tick >= AUTOPLAY_END
        && wm.captures_taken >= AUTOPLAY_CAPTURES.len();
    if closed.read().count() > 0 || keys.just_pressed(KeyCode::Escape) || done {
        for line in wm.log.drain(..) {
            println!("{line}");
        }
        exit.write(AppExit::Success);
    }
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
        return Err("usage: kh2fm wm <Steam-root> <manifest> [--from <world code>] [--story <first>:<last>]... [--load-only] [--autoplay <new-capture-dir>]".into());
    }
    let mut from = "tt".to_owned();
    let mut story = Vec::new();
    let mut load_only = false;
    let mut autoplay = None;
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
    // FUN_14038cf20 → FUN_14038de90(0) → FUN_14038dc60: the queued world
    // events' scripts run on arrival; their world map events (#89) are
    // listed, not played.
    let (events, set) = src.progress_mut().play_queued_world_events();
    let mut library = Library::open(&source)?;
    let mut actors = Vec::new();
    let mut cursor = None;
    type Loaded = (
        Option<Arc<InspectionModel>>,
        HashMap<u16, Arc<InterpolatedMotion>>,
    );
    let mut models: HashMap<String, Loaded> = HashMap::new();
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
                Kind::Cursor => &[CURSOR_IDLE, CURSOR_FAST],
                Kind::Symbol(_) => &[0, SYMBOL_CLOSED, SYMBOL_OPEN],
                Kind::Gumi(_) => &[0],
            };
            let motions = match library.find(&format!("obj/{name}.mset")) {
                Some(f) => read_motions(&library.read_found(&f)?, ids)?,
                None => HashMap::new(),
            };
            models.insert(name.clone(), (model, motions));
        }
        let (model, motions) = &models[&name];
        if matches!(kind, Kind::Cursor) {
            cursor = Some(actors.len());
        }
        actors.push(Actor {
            kind,
            model: model.clone(),
            motions: motions.clone(),
            position: s.entity.position,
            yaw: s.yaw,
            motion: match kind {
                Kind::Symbol(_) => SYMBOL_CLOSED,
                _ => 0,
            },
            time: 0.,
            drawn: true,
            tint: [1.; 4],
        });
    }
    let cursor = cursor.ok_or("the world map has no cursor at this entrance")?;
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
    let pref = entry(&system, &system_bar, b"pref")?;
    let pref_bar = bar::read_bar(&pref)?;
    let sstm_bytes = entry(&pref, &pref_bar, b"sstm")?;
    let sstm_offset = u32::from_le_bytes(sstm_bytes.get(4..8).ok_or("SSTM pointer")?.try_into()?);
    let sstm = sstm_bytes
        .get(sstm_offset as usize..)
        .and_then(actor_physics::Sstm::read)
        .ok_or("SSTM record truncated")?;
    let mut params = ObjParams::defaults(sstm.fall_max);
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
    }
    let body = library
        .read("obj/WM_CURSOR.mdlx")
        .ok()
        .and_then(|a| map_body(&a))
        .unwrap_or(area::SORA_BODY);
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
        autoplay: autoplay.clone(),
        captures_taken: 0,
        capture_pending: false,
        log: Vec::new(),
    };
    let progress = src.progress();
    apply_world_state(
        &mut wm,
        progress,
        progress.world_tables(),
        loaded.bobs.placements.len(),
    );
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
            "drawn":a.drawn,"motion":a.motion,"motions":a.motions.keys().collect::<Vec<_>>(),"position":a.position})).collect::<Vec<_>>(),
        "cursor_params":{"walk":params.walk,"run":params.run,"turn":params.turn,"inertia":params.word10,"drag":params.word11,"prty":prty_index},
        "body":{"radius":body.radius,"height":body.height,"offset":body.offset},
        "map_objects":loaded.bobs.placements.iter().enumerate().map(|(i, p)| serde_json::json!({
            "index":i,"model":p.model,"group":p.group,"at":p.translation.to_array().map(|v| v.round()),
            "shown":!wm.hidden_bobs[i] && wm.bg_mask >> p.group & 1 != 0})).collect::<Vec<_>>(),
        "untextured":untextured});
    println!("{summary}");
    if load_only {
        return Ok(AppExit::Success);
    }
    let mut app = App::new();
    app.insert_resource(wm)
        .insert_resource(StartupRes(Some(Startup { room, area: loaded })))
        .insert_resource(Time::<Fixed>::from_hz(TICK_HZ))
        .insert_resource(ClearColor(Color::BLACK))
        .add_plugins(DefaultPlugins.set(WindowPlugin {
            primary_window: Some(Window {
                title: "Kingdom Hearts II Re:Written (world map; rewrite) | WASD/stick fly, Esc exits"
                    .into(),
                resolution: (1280, 720).into(),
                ..default()
            }),
            exit_condition: bevy::window::ExitCondition::DontExit,
            close_when_requested: false,
            ..default()
        }))
        .add_plugins(FogPlugin)
        .add_systems(Startup, setup)
        .add_systems(FixedUpdate, (read_stick, tick).chain())
        .add_systems(Update, (present, finish).chain());
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
        assert!(read_motions(&bytes, &[0]).unwrap().is_empty());
        assert!(read_motions(&bytes, &[1]).is_err());
    }
}
