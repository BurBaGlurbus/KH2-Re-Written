//! `kh2fm event`: plays one original event (cutscene) from the selected
//! installation — tt19 event 613 by default, the scene before the Roxas vs
//! Axel fight in Betwixt and Between — from the event script's own cameras,
//! actors, motions, voices, music and subtitles.
pub(crate) mod blur;
mod fx;
mod load;
mod screen;
mod texa;

use crate::test_room::{BgChunk, TexaWorkers, UvScroll, spawn_room, texa_advance, uv_scroll_apply};
use bevy::{
    app::AppExit,
    core_pipeline::tonemapping::{DebandDither, Tonemapping},
    prelude::*,
    render::render_resource::TextureFormat,
    render::view::{
        NoIndirectDrawing,
        screenshot::{Screenshot, ScreenshotCaptured},
    },
};
use data::{
    model_inspection::InspectionModel, model_inspection_textures::TextureMode,
    motion::InterpolatedMotion, probe_source::ProbeSource,
};
use event::Timeline;
use std::{collections::HashMap, fs::OpenOptions, path::PathBuf, sync::Arc};
use visual::area_fog::{FogMaterial, FogPlugin};
use visual::dialogue;
use visual::hud::SystemFont;
use visual::overlay::{LayoutMaterial, Ui2dFrame, Ui2dPlugin};
use visual::room::{mesh, rgba_image};
use visual::sound;

/// Event frames per second.
const EVENT_FPS: f32 = 30.;
/// Simulation ticks per second.
const TICK_HZ: f64 = 60.;

#[derive(Component)]
struct ActorPart {
    actor: usize,
    part: usize,
}
#[derive(Component)]
struct ActorRoot(usize);
#[derive(Component)]
struct EventCamera;

struct ActorRuntime {
    id: i16,
    name: String,
    model: InspectionModel,
    motions: HashMap<String, Arc<InterpolatedMotion>>,
}

#[derive(Resource)]
struct Event {
    timeline: Timeline,
    actors: Vec<ActorRuntime>,
    /// Current event frame (fractional; 30 per second).
    frame: f32,
    tick: u64,
    finished: bool,
    voices_started: usize,
    autoplay: Option<PathBuf>,
    captures: Vec<f32>,
    captures_taken: usize,
    capture_pending: bool,
    rendered_frames: u64,
    log: Vec<serde_json::Value>,
}

#[derive(Resource)]
struct Audio {
    output: Option<audio::Output>,
    voices: HashMap<String, audio::Clip>,
    music: HashMap<i16, sound::MusicSlot>,
    music_started: bool,
    banks: sound::Banks,
    /// (frame, bank, sound number) of the plain sound requests, in order.
    effects: Vec<(i32, i16, i32)>,
    effects_started: usize,
    /// Pause menu: None = running, Some(row) = open on that row.
    pause: Option<usize>,
    pause_labels: [Vec<u8>; 2],
}

#[derive(Resource)]
struct Overlay {
    font: SystemFont,
    font_sheet: Option<usize>,
    white_sheet: Option<usize>,
    scene_sheet: Option<usize>,
    scene: Handle<Image>,
    messages: HashMap<u16, Vec<u8>>,
}

struct Startup {
    room: visual::room::LoadedRoom,
}

#[derive(Resource)]
struct StartupRes(Option<Startup>);

/// The area's background group mask when the event starts; `BgGrupe` toggles
/// bits of it as the event plays.
#[derive(Resource)]
struct BgGroups {
    initial: u64,
    shown: Option<u64>,
}

/// Shows and hides the map's chunks as the event's background groups change,
/// and runs the map's texture and UV animation on the event clock.
#[allow(clippy::too_many_arguments)]
fn room_animation(
    event: Res<Event>,
    mut bg: ResMut<BgGroups>,
    mut chunks: Query<(&BgChunk, &mut Visibility)>,
    added: Query<(), Added<BgChunk>>,
    workers: Option<ResMut<TexaWorkers>>,
    scrolls: Query<&UvScroll>,
    mut images: ResMut<Assets<Image>>,
    mut materials: ResMut<Assets<FogMaterial>>,
) {
    let mask = event.timeline.bg_mask(bg.initial, event.frame);
    if bg.shown != Some(mask) || !added.is_empty() {
        bg.shown = Some(mask);
        for (chunk, mut visibility) in &mut chunks {
            *visibility = if visual::room::chunk_visible(chunk.0, mask) {
                Visibility::Inherited
            } else {
                Visibility::Hidden
            };
        }
    }
    if let Some(mut workers) = workers {
        texa_advance(&mut workers, event.tick, &mut images);
    }
    uv_scroll_apply(event.tick as i64, scrolls.iter(), &mut materials);
}

const VOICE_KEY: u32 = 8_000_000;
const MUSIC_KEY: u32 = 8_100_000;
const SE_KEY: u32 = 8_200_000;

pub(crate) fn decode_voice(bytes: &[u8]) -> Result<audio::Clip, Box<dyn std::error::Error>> {
    let bank = data::scd::read(bytes)?;
    let sound = bank.sound(0)?.ok_or("voice sound absent")?;
    let audio = *sound.audio.first().ok_or("voice audio absent")?;
    let pcm = bank.decode(audio as usize)?.ok_or("empty voice")?;
    Ok(audio::Clip {
        channels: pcm.channels,
        rate: pcm.rate,
        loop_frames: None,
        samples: pcm.samples.into(),
    })
}

#[allow(clippy::too_many_arguments, clippy::type_complexity)]
fn setup(
    mut commands: Commands,
    mut startup: ResMut<StartupRes>,
    event: Res<Event>,
    mut overlay: ResMut<Overlay>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut fog_materials: ResMut<Assets<FogMaterial>>,
    bg: Res<BgGroups>,
    mut layout_materials: ResMut<Assets<LayoutMaterial>>,
    mut images: ResMut<Assets<Image>>,
    mut frame: ResMut<Ui2dFrame>,
    windows: Query<&Window>,
) {
    let Startup { room } = startup.0.take().expect("startup data");
    let world = commands
        .spawn((Transform::default(), Visibility::default()))
        .id();
    // The map is drawn by the field's own room renderer (texture regions,
    // fog, UV scroll, TEXA, background groups).
    spawn_room(
        &mut commands,
        &mut meshes,
        &mut fog_materials,
        &mut images,
        &room,
        world,
        false,
        bg.initial,
    );
    let clear = room.environment.map_or(Color::BLACK, |e| {
        Color::srgb_u8(e.clear[0], e.clear[1], e.clear[2])
    });
    // One image per (actor, texture environment), shared by its parts so a
    // texture animation repaints them all.
    let mut shared: HashMap<(usize, usize), (Handle<Image>, u32)> = HashMap::new();
    for (ai, actor) in event.actors.iter().enumerate() {
        let root = commands
            .spawn((
                ChildOf(world),
                Transform::default(),
                Visibility::Hidden,
                ActorRoot(ai),
            ))
            .id();
        let model = &actor.model;
        let pose = &model.poses[0];
        eprintln!(
            "actor {} {}: mesh groups {:?}, texture patches {}",
            actor.id,
            actor.name,
            model.parts.iter().map(|p| p.mesh_group).collect::<Vec<_>>(),
            model.textures.as_ref().map_or(0, |t| t.play.len())
        );
        for (pi, part) in model.parts.iter().enumerate() {
            if part.indices.is_empty() {
                continue;
            }
            let positions = part
                .indices
                .iter()
                .map(|&i| pose.position_bits[i as usize].map(f32::from_bits))
                .collect();
            let mut uvs = vec![[0., 0.]; part.indices.len()];
            let mut texture = None;
            if let Some(set) = &model.textures {
                let route = &model.texture_routes[0].parts[pi];
                uvs = route
                    .expanded_uv_bits
                    .iter()
                    .map(|uv| uv.map(f32::from_bits))
                    .collect();
                if let Ok(binding) =
                    set.bind(route.environment_ordinal, TextureMode::OriginalIndexed)
                    && let Some(image) = binding.image_index
                {
                    let level = &set.images[image].levels[0];
                    let (handle, _) = shared
                        .entry((ai, route.environment_ordinal))
                        .or_insert_with(|| {
                            (
                                rgba_image(
                                    level.width,
                                    level.height,
                                    level.pixels_rgba.clone(),
                                    &mut images,
                                ),
                                level.width,
                            )
                        });
                    texture = Some(handle.clone());
                }
            }
            commands.spawn((
                ChildOf(root),
                Mesh3d(meshes.add(mesh(positions, uvs, vec![[1.; 4]; part.indices.len()]))),
                MeshMaterial3d(materials.add(StandardMaterial {
                    base_color: Color::WHITE,
                    base_color_texture: texture,
                    unlit: true,
                    cull_mode: None,
                    alpha_mode: if part.source_is_opaque {
                        AlphaMode::Opaque
                    } else {
                        AlphaMode::Mask(0.5)
                    },
                    ..default()
                })),
                Transform::default(),
                ActorPart {
                    actor: ai,
                    part: pi,
                },
            ));
        }
    }
    let size = windows
        .single()
        .map(|w| [w.physical_width().max(1), w.physical_height().max(1)])
        .unwrap_or([1280, 720]);
    overlay.scene = images.add(Image::new_target_texture(
        size[0],
        size[1],
        TextureFormat::Rgba8UnormSrgb,
        None,
    ));
    overlay.scene_sheet = Some(frame.add_image(overlay.scene.clone(), size, true));
    let blur = blur::Blur::spawn(
        &mut commands,
        &mut meshes,
        &mut layout_materials,
        &mut images,
        &mut frame,
        &overlay.scene,
        size,
        blur::Blur::switches(&event.timeline.other),
        blur::ZOOM,
    );
    commands.insert_resource(blur);
    commands.insert_resource(texa::Texa::new(
        &event.actors,
        &shared,
        &event.timeline.other,
    ));
    overlay.font_sheet = Some(frame.add_sheet(
        &mut images,
        overlay.font.rgba.clone(),
        overlay.font.size,
        overlay.font.texel_scale as f32,
    ));
    overlay.white_sheet = Some(frame.add_sheet(&mut images, vec![255; 4], [1, 1], 1.));
    commands.spawn((
        Camera3d::default(),
        Camera {
            order: -2,
            clear_color: ClearColorConfig::Custom(clear),
            ..default()
        },
        bevy::camera::RenderTarget::Image(overlay.scene.clone().into()),
        Projection::Perspective(PerspectiveProjection {
            fov: 30f32.to_radians(),
            near: 5.,
            far: 200_000.,
            ..default()
        }),
        Transform::default(),
        Msaa::Off,
        Tonemapping::None,
        DebandDither::Disabled,
        NoIndirectDrawing,
        EventCamera,
    ));
}

/// Holds or releases every sound of the event (the pause menu freezes
/// voices, effects and music in place, audio-spec section 5).
fn hold_audio(audio: &Audio, held: bool) {
    if let Some(out) = audio.output.as_ref()
        && let Ok(mut mixer) = out.mixer.lock()
    {
        for key in [VOICE_KEY, MUSIC_KEY, SE_KEY] {
            mixer.hold(key, held);
        }
    }
}

/// The event clock: two simulation ticks per event frame; frozen while
/// the pause menu is open.
fn tick(
    mut event: ResMut<Event>,
    keys: Res<ButtonInput<KeyCode>>,
    mut audio: ResMut<Audio>,
    mut fx: ResMut<fx::Fx>,
) {
    // Start (Esc on PC) opens the pause menu: Continue / Skip Scene.
    let start = keys.just_pressed(KeyCode::Escape) || keys.just_pressed(KeyCode::Tab);
    match audio.pause {
        None if start && !event.finished => {
            audio.pause = Some(0);
            hold_audio(&audio, true);
        }
        Some(row) => {
            let up = keys.just_pressed(KeyCode::ArrowUp) || keys.just_pressed(KeyCode::KeyW);
            let down = keys.just_pressed(KeyCode::ArrowDown) || keys.just_pressed(KeyCode::KeyS);
            let row = if up || down { 1 - row } else { row };
            audio.pause = Some(row);
            let decide = keys.just_pressed(KeyCode::Enter) || keys.just_pressed(KeyCode::Space);
            if decide && row == 1 {
                // Skip Scene: the end-of-event path, sounds stopped.
                audio.pause = None;
                event.finished = true;
                if let Some(out) = audio.output.as_ref()
                    && let Ok(mut mixer) = out.mixer.lock()
                {
                    for key in [VOICE_KEY, MUSIC_KEY, SE_KEY] {
                        mixer.stop(key);
                    }
                }
            } else if (decide && row == 0) || start {
                audio.pause = None;
                hold_audio(&audio, false);
            }
            return;
        }
        None => {}
    }
    if event.finished {
        return;
    }
    event.tick += 1;
    event.frame += EVENT_FPS / TICK_HZ as f32;
    let frame = event.frame;
    let Audio {
        output,
        voices,
        music,
        music_started,
        banks,
        effects,
        effects_started,
        ..
    } = &mut *audio;
    // Voices start on their frame.
    while let Some((at, name)) = event.timeline.voices.get(event.voices_started).cloned()
        && at as f32 <= frame
    {
        event.voices_started += 1;
        if let (Some(out), Some(clip)) = (output.as_ref(), voices.get(&name))
            && let Ok(mut mixer) = out.mixer.lock()
        {
            mixer.play(clip.clone(), VOICE_KEY, 0x3fff as f32 / 16383. * 0.8, 1.);
        }
        let tick = event.tick;
        event
            .log
            .push(serde_json::json!({"event":"voice","name":name,"frame":at,"tick":tick}));
    }
    // Sound effects: bank id * 1000 + number (FUN_1402d8be0).
    while let Some(&(at, bank, number)) = effects.get(*effects_started)
        && at as f32 <= frame
    {
        *effects_started += 1;
        let id = bank as u32 * 1000 + number as u32;
        if let Ok(Some(request)) = banks.request(id)
            && let Some(out) = output.as_ref()
            && let Ok(mut mixer) = out.mixer.lock()
        {
            for clip in request.layers {
                mixer.play(clip, SE_KEY, request.volume, 1.);
            }
        }
    }
    // The BGM record at full level (frame 336): music151 at 0x3000.
    let cue = event.timeline.play_bgm.iter().find_map(|c| match c {
        data::event_script::Command::PlayBgm {
            frame: f,
            volume_end: 0,
            ..
        } => Some(*f as i32),
        _ => None,
    });
    if !*music_started
        && let Some(at) = cue
        && at as f32 <= frame
        && let Some(&(_, _, bgm)) = event.timeline.set_bgm.first()
        && let Some(slot) = music.get(&bgm)
        && let Some(Ok(m)) = slot.lock().ok().and_then(|r| {
            r.as_ref().map(|r| {
                r.as_ref()
                    .map(|m| (m.clip.clone(), m.volume))
                    .map_err(|e| e.clone())
            })
        })
    {
        *music_started = true;
        if let Some(out) = output.as_ref()
            && let Ok(mut mixer) = out.mixer.lock()
        {
            mixer.play(m.0, MUSIC_KEY, m.1 * 0x3000 as f32 / 16383., 1.);
        }
    }
    fx.step(frame, &event.timeline, &event.actors);
    let logged = std::mem::take(&mut fx.log);
    event.log.extend(logged);
    if event.timeline.end_frame > 0 && frame >= event.timeline.end_frame as f32 {
        event.finished = true;
    }
}

#[allow(clippy::type_complexity)]
fn present(
    event: Res<Event>,
    fixed: Res<Time<Fixed>>,
    mut roots: Query<(&ActorRoot, &mut Transform, &mut Visibility), Without<ActorPart>>,
    mut parts: Query<(&ActorPart, &Mesh3d, &mut Visibility), Without<ActorRoot>>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut cameras: Query<(&mut Transform, &mut Projection), (With<EventCamera>, Without<ActorRoot>)>,
) {
    let frame = event.frame + fixed.overstep_fraction() * (EVENT_FPS / TICK_HZ as f32);
    let frame = frame.min(event.timeline.end_frame.max(1) as f32);
    let mut poses: HashMap<usize, Vec<[f32; 3]>> = HashMap::new();
    // Bone matrices and root transforms for AttachEffect type 1.
    let mut bones: HashMap<usize, Vec<Mat4>> = HashMap::new();
    let mut placed: HashMap<usize, Transform> = HashMap::new();
    // Visible mesh groups per actor (SeqPart lists), where one applies.
    let mut visible_groups: HashMap<usize, Vec<i16>> = HashMap::new();
    for (root, mut transform, mut visibility) in &mut roots {
        let actor = &event.actors[root.0];
        let Some(state) = event.timeline.actor_at(actor.id, frame) else {
            *visibility = Visibility::Hidden;
            continue;
        };
        let Some(motion) = actor.motions.get(&state.animation.name) else {
            *visibility = Visibility::Hidden;
            continue;
        };
        // The motion free-runs from the record's frame at 30 motion
        // frames per second; past its end it jumps to FrameReturn (equal
        // to the end holds the last pose, earlier loops) [core-spec].
        let motion_frame = state.animation.motion_start as f32 + state.elapsed;
        let (_, end, fps, ret) = motion.frame_window().unwrap_or((0., 1., 30., 0.));
        let motion_frame = if motion_frame > end && ret < end {
            ret + (motion_frame - end) % (end - ret)
        } else {
            motion_frame.min(end)
        };
        let sample = motion_frame * 60. / fps;
        let Ok((pose, _)) = actor.model.skinning.sample_evaluated(motion, sample) else {
            continue;
        };
        bones.insert(
            root.0,
            pose.matrix_bits
                .iter()
                .map(|m| Mat4::from_cols_array(&m.map(f32::from_bits)))
                .collect(),
        );
        poses.insert(
            root.0,
            pose.position_bits
                .iter()
                .map(|p| p.map(f32::from_bits))
                .collect(),
        );
        let listed = event
            .timeline
            .actors
            .iter()
            .find(|a| a.id == actor.id)
            .and_then(|a| a.parts.iter().rev().find(|(at, _)| *at as f32 <= frame))
            .map(|(_, list)| list.as_slice());
        // A part list names the VISIBLE mesh groups (bits 6..10 of the mesh
        // descriptor); every other group is hidden (screen-spec 4).
        if let Some(list) = listed {
            visible_groups.insert(
                root.0,
                list.iter().copied().take_while(|&g| g >= 0).collect(),
            );
        }
        *visibility = Visibility::Inherited;
        if let Some(p) = state.placement {
            transform.translation = Vec3::from_array(p.position);
            // The script's yaw is a heading in the exe's left-handed
            // world; here it is the same angle with the opposite sense
            // (ry 180 stays, -90 becomes +90). The turn is made about the
            // body's centre so the framed body stays where the script's
            // camera expects it.
            let base = Quat::from_euler(
                EulerRot::YXZ,
                p.rotation[1].to_radians(),
                p.rotation[0].to_radians(),
                p.rotation[2].to_radians(),
            );
            let turned = Quat::from_euler(
                EulerRot::YXZ,
                (-p.rotation[1]).to_radians(),
                p.rotation[0].to_radians(),
                p.rotation[2].to_radians(),
            );
            // The body's horizontal centre in model space.
            let n = pose.position_bits.len().max(1) as f32;
            let (mut cx, mut cz) = (0f32, 0f32);
            for q in &pose.position_bits {
                cx += f32::from_bits(q[0]);
                cz += f32::from_bits(q[2]);
            }
            let c = Vec3::new(cx / n, 0., cz / n);
            transform.translation = Vec3::from_array(p.position) + base * c - turned * c;
            transform.rotation = turned;
        } else {
            *transform = Transform::default();
        }
        placed.insert(root.0, *transform);
    }
    // AttachEffect type 1 (FUN_1402d07d0 -> FUN_1403d9fe0): the child's
    // origin sits at the parent's bone; its own motion animates from there.
    for (root, mut transform, _) in &mut roots {
        let child_id = event.actors[root.0].id;
        let Some(att) = event
            .timeline
            .attachments
            .iter()
            .rev()
            .find(|a| a.child == child_id && a.frame as f32 <= frame)
        else {
            continue;
        };
        let Some(parent) = event.actors.iter().position(|a| a.id == att.parent) else {
            continue;
        };
        if let (Some(m), Some(base)) = (
            bones.get(&parent).and_then(|b| b.get(att.bone as usize)),
            placed.get(&parent),
        ) {
            // FUN_1401a7af0 reads the parent's bone matrix and negates its
            // y and z columns (DAT_140623c48 = -1.0): the bone table is in
            // the data's y-down space and the child object is y-up. The
            // child's own vertices are converted the same way before its
            // local matrix (FUN_1403ccb90: identity here, local position
            // +0x670 zeroed by FUN_1403d9fe0), so in the parent's data
            // space the child sits at  v * M_child * F * M_bone  with
            // F = diag(1,-1,-1). Matrices are row-vector words, hence the
            // product order for the column-major Mat4.
            let flip = Mat4::from_diagonal(Vec4::new(1., -1., -1., 1.));
            *transform = *base * Transform::from_matrix(*m * flip);
        }
    }
    for (part, handle, mut visibility) in &mut parts {
        let group = event.actors[part.actor].model.parts[part.part].mesh_group as i16;
        *visibility = match visible_groups.get(&part.actor) {
            Some(list) if !list.contains(&group) => Visibility::Hidden,
            _ => Visibility::Inherited,
        };
        let Some(pose) = poses.get(&part.actor) else {
            continue;
        };
        let positions: Vec<[f32; 3]> = event.actors[part.actor].model.parts[part.part]
            .indices
            .iter()
            .map(|&i| pose[i as usize])
            .collect();
        if let Some(mut mesh) = meshes.get_mut(&handle.0) {
            mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, positions);
        }
    }
    // The exe's camera output negates y and z (FUN_1402d3d40; camera
    // space is y-down), actors use the raw script coordinates.
    if let Some((pose, fov)) = fx::camera_pose(&event.timeline, frame)
        && let Ok((mut transform, mut projection)) = cameras.single_mut()
    {
        *transform = pose;
        if let Projection::Perspective(p) = projection.as_mut() {
            p.fov = fov;
        }
    }
}

fn compose(
    event: Res<Event>,
    overlay: Res<Overlay>,
    audio: Res<Audio>,
    mut frame: ResMut<Ui2dFrame>,
    blur: Res<blur::Blur>,
) {
    let (Some(_), Some(font_sheet)) = (overlay.scene_sheet, overlay.font_sheet) else {
        return;
    };
    // The scene with the SeqBlur feedback pass applied.
    // The exe's world is left-handed; the scene is rendered right-handed,
    // so the image is mirrored in x when composed.
    {
        let sheet = blur.sheet();
        let [w, h] = frame.sheets[sheet].size;
        let half = visual::overlay::WIDE_WIDTH / 2.;
        let (l, r) = (256. - half, 256. + half);
        frame.draws.push((
            sheet,
            vec![data::sprite_sequence::Quad {
                corners: [
                    [r, 0.],
                    [l, 0.],
                    [r, visual::overlay::HEIGHT],
                    [l, visual::overlay::HEIGHT],
                ],
                uv: [0, 0, w as i32, h as i32],
                colors: [[1.; 4]; 4],
                blend: 0,
            }],
        ));
    }
    let f = event.frame;
    // Fade layers B and A, then the letterbox bars on top (screen-spec 1).
    let fades = &event.timeline.fades;
    let layer = |frame: &mut Ui2dFrame, index: usize| {
        let start = if index == 2 {
            event.timeline.fade_in
        } else {
            0
        };
        if let (Some(c), Some(white)) = (
            screen::layer_colour(fades, index, start, f),
            overlay.white_sheet,
        ) {
            let tone = if c.white { 1. } else { 0. };
            let (l, r) = (-86., 598.);
            frame.draws.push((
                white,
                vec![data::sprite_sequence::Quad {
                    corners: [[l, -1.], [r, -1.], [l, 417.], [r, 417.]],
                    uv: [0, 0, 1, 1],
                    colors: [[tone, tone, tone, c.alpha]; 4],
                    blend: u8::from(c.white).into(),
                }],
            ));
        }
    };
    layer(&mut frame, 1);
    layer(&mut frame, 0);
    for (_slot, message) in event.timeline.subtitles_at(f) {
        let Some(bytes) = overlay.messages.get(&message) else {
            continue;
        };
        let mut text = dialogue::Text::new(bytes.clone(), 0x80f0_f0f0, &overlay.font.widths);
        let mut glyphs = Vec::new();
        for _ in 0..4096 {
            glyphs = text.draw(256, 384, &overlay.font.widths);
            if text.complete {
                break;
            }
        }
        for g in &mut glyphs {
            g.rect = [
                256. + (g.rect[0] - 256.) * 0.8,
                400. + (g.rect[1] - 400.) * 0.8,
                256. + (g.rect[2] - 256.) * 0.8,
                400. + (g.rect[3] - 400.) * 0.8,
            ];
        }
        frame
            .draws
            .push((font_sheet, dialogue::glyph_quads(&glyphs)));
    }
    // The pause menu: a plain two-row list (Continue / Skip Scene).
    if let (Some(row), Some(white)) = (audio.pause, overlay.white_sheet) {
        frame.fill(white, [0., 0., 0., 0.55]);
        for (i, label) in audio.pause_labels.iter().enumerate() {
            let y = 190 + i as i32 * 44;
            if i == row {
                let w = 220.;
                frame.draws.push((
                    white,
                    vec![data::sprite_sequence::Quad {
                        corners: [
                            [256. - w / 2., y as f32 - 20.],
                            [256. + w / 2., y as f32 - 20.],
                            [256. - w / 2., y as f32 + 20.],
                            [256. + w / 2., y as f32 + 20.],
                        ],
                        uv: [0, 0, 1, 1],
                        colors: [[0.2, 0.35, 0.75, 0.85]; 4],
                        blend: 0,
                    }],
                ));
            }
            let mut text = dialogue::Text::new(
                label.clone(),
                if i == row { 0x80f0_f0f0 } else { 0x8080_8080 },
                &overlay.font.widths,
            );
            let mut glyphs = Vec::new();
            for _ in 0..512 {
                glyphs = text.draw(256, y, &overlay.font.widths);
                if text.complete {
                    break;
                }
            }
            frame
                .draws
                .push((font_sheet, dialogue::glyph_quads(&glyphs)));
        }
    }
    // The event curtain (layer C): the start fade-in and types 6/7.
    layer(&mut frame, 2);
}

fn finish(
    mut commands: Commands,
    mut event: ResMut<Event>,
    mut time: ResMut<Time<Virtual>>,
    mut exit: MessageWriter<AppExit>,
    mut closed: MessageReader<bevy::window::WindowCloseRequested>,
    audio: Res<Audio>,
) {
    let music_ready = audio
        .music
        .values()
        .all(|slot| slot.lock().map_or(true, |r| r.is_some()));
    if music_ready {
        event.rendered_frames += 1;
    }
    if event.autoplay.is_some()
        && time.is_paused()
        && !event.capture_pending
        && event.rendered_frames >= 30
    {
        time.unpause();
    }
    if let Some(dir) = event.autoplay.clone()
        && !event.capture_pending
        && let Some(&at) = event.captures.get(event.captures_taken)
        && event.frame >= at
    {
        event.capture_pending = true;
        time.pause();
        let index = event.captures_taken;
        let path = dir.join(format!("event-{index:02}.png"));
        let frame = event.frame;
        commands.spawn(Screenshot::primary_window()).observe(
            move |captured: On<ScreenshotCaptured>,
                  mut event: ResMut<Event>,
                  mut time: ResMut<Time<Virtual>>| {
                let image = captured
                    .image
                    .clone()
                    .try_into_dynamic()
                    .expect("event capture");
                let mut file = OpenOptions::new()
                    .write(true)
                    .create_new(true)
                    .open(&path)
                    .expect("new private capture");
                image
                    .write_to(&mut file, image::ImageFormat::Png)
                    .expect("capture encode");
                event.capture_pending = false;
                event.captures_taken += 1;
                event
                    .log
                    .push(serde_json::json!({"event":"capture","index":index,"frame":frame}));
                time.unpause();
            },
        );
    }
    let captures_done = event.captures_taken >= event.captures.len();
    if closed.read().count() > 0 || (event.finished && (event.autoplay.is_none() || captures_done))
    {
        for line in event.log.drain(..) {
            println!("{line}");
        }
        exit.write(AppExit::Success);
    }
}

fn run(args: Vec<PathBuf>) -> Result<AppExit, Box<dyn std::error::Error>> {
    if args.len() < 2 {
        return Err("usage: kh2fm event <Steam-root> <manifest> [--event tt:19:613] [--load-only] [--bg-mask <hex>] [--autoplay <new-capture-dir>] [--captures f1,f2,...]".into());
    }
    let mut id = load::EventId {
        world: "tt".into(),
        area: 19,
        name: "613".into(),
    };
    let mut load_only = false;
    let mut bg_mask = u64::MAX;
    let mut autoplay = None;
    let mut captures = vec![
        10., 100., 200., 300., 400., 520., 600., 700., 760., 900., 1000.,
    ];
    let mut options = args[2..].iter();
    while let Some(option) = options.next() {
        match option.to_str() {
            Some("--load-only") => load_only = true,
            Some("--autoplay") => autoplay = options.next().cloned(),
            Some("--bg-mask") => {
                let value = options
                    .next()
                    .and_then(|v| v.to_str())
                    .ok_or("bg mask absent")?;
                bg_mask = u64::from_str_radix(value.trim_start_matches("0x"), 16)?;
            }
            Some("--captures") => {
                captures = options
                    .next()
                    .and_then(|v| v.to_str())
                    .ok_or("captures absent")?
                    .split(',')
                    .map(|v| v.parse::<f32>())
                    .collect::<Result<_, _>>()?;
            }
            Some("--event") => {
                let spec = options
                    .next()
                    .and_then(|v| v.to_str())
                    .ok_or("event absent")?;
                let parts: Vec<_> = spec.split(':').collect();
                let [world, area, name] = parts[..] else {
                    return Err("--event wants <world>:<area>:<name>".into());
                };
                id = load::EventId {
                    world: world.into(),
                    area: area.parse()?,
                    name: name.into(),
                };
            }
            _ => return Err("unknown/duplicate event option".into()),
        }
    }
    let source = ProbeSource::bind_session(&args[0], &args[1])?;
    if let Some(dir) = &autoplay {
        source.output_path(&dir.join("event-00.png"))?;
        if !dir.is_dir() || std::fs::read_dir(dir)?.next().is_some() {
            return Err("autoplay capture directory must exist and be empty".into());
        }
    }
    eprintln!("Validating the selected KH2FM source, then decoding the event...");
    let loaded = load::load(&source, &id)?;
    println!("{}", loaded.summary);
    if load_only {
        return Ok(AppExit::Success);
    }
    let timeline = Timeline::new(&loaded.commands);
    let mut motions_by_actor: HashMap<i16, HashMap<String, Arc<InterpolatedMotion>>> =
        HashMap::new();
    for ((actor, name), m) in loaded.motions {
        motions_by_actor
            .entry(actor)
            .or_default()
            .insert(name, Arc::new(m));
    }
    let actors = loaded
        .actors
        .into_iter()
        .map(|a| ActorRuntime {
            motions: motions_by_actor.remove(&a.id).unwrap_or_default(),
            id: a.id,
            name: a.name,
            model: a.model,
        })
        .collect::<Vec<_>>();
    let mut voices = HashMap::new();
    for (name, bytes) in &loaded.voices {
        voices.insert(name.clone(), decode_voice(bytes)?);
    }
    let music = loaded
        .music
        .into_iter()
        .map(|(id, bytes)| (id, sound::decode_music_in_background(bytes)))
        .collect();
    let output = match audio::Output::open() {
        Ok(o) => Some(o),
        Err(e) => {
            eprintln!("Sound output unavailable: {e}");
            None
        }
    };
    let mut banks = sound::Banks::default();
    for bytes in &loaded.banks {
        banks.insert(bytes.clone())?;
    }
    let mut effects: Vec<(i32, i16, i32)> = Vec::new();
    let mut bank = 0i16;
    for c in &loaded.commands {
        match c {
            data::event_script::Command::SoundData { bank: b, .. } => bank = *b,
            data::event_script::Command::PlaySound {
                kind: 0,
                number,
                frame,
                ..
            } => effects.push((*frame as i32, bank, *number)),
            _ => {}
        }
    }
    effects.sort_by_key(|e| e.0);
    let names: Vec<_> = actors.iter().map(|a| a.name.clone()).collect();
    eprintln!(
        "Event ready ({} actors: {names:?}). Esc pauses (Continue / Skip Scene).",
        actors.len()
    );
    let mut app = App::new();
    let autoplaying = autoplay.is_some();
    app.insert_resource(Event {
        timeline,
        actors,
        frame: 0.,
        tick: 0,
        finished: false,
        voices_started: 0,
        autoplay,
        captures,
        captures_taken: 0,
        capture_pending: false,
        rendered_frames: 0,
        log: Vec::new(),
    })
    .insert_resource(Audio {
        output,
        voices,
        music,
        music_started: false,
        banks,
        effects,
        effects_started: 0,
        pause: None,
        pause_labels: loaded.pause_labels,
    })
    .insert_resource(Overlay {
        font: loaded.font,
        font_sheet: None,
        white_sheet: None,
        scene_sheet: None,
        scene: Handle::default(),
        messages: loaded.messages,
    })
    .insert_resource(fx::Fx::new(
        loaded.paxes,
        loaded.texcommon,
        &loaded.commands,
    ))
    .insert_resource(StartupRes(Some(Startup { room: loaded.room })))
    .insert_resource(BgGroups {
        initial: bg_mask,
        shown: None,
    })
    .insert_resource(Time::<Fixed>::from_hz(TICK_HZ))
    .insert_resource(ClearColor(Color::BLACK))
    .add_plugins(DefaultPlugins.set(WindowPlugin {
        primary_window: Some(Window {
            title: "Kingdom Hearts II Re:Written (event; rewrite)".into(),
            resolution: (1280, 720).into(),
            ..default()
        }),
        exit_condition: bevy::window::ExitCondition::DontExit,
        close_when_requested: false,
        ..default()
    }))
    .add_plugins(FogPlugin)
    .add_plugins(Ui2dPlugin {
        order: 0,
        overlay: false,
    })
    .add_systems(Startup, setup)
    .add_systems(FixedUpdate, tick)
    .add_systems(
        Update,
        (
            present,
            fx::present,
            texa::animate,
            blur::drive,
            room_animation,
            compose,
            finish,
        )
            .chain(),
    );
    if autoplaying {
        app.add_systems(Startup, |mut time: ResMut<Time<Virtual>>| time.pause());
    }
    Ok(app.run())
}

/// Runs this program with its arguments; `interactive` = started from the
/// launcher menu.
pub fn main(args: Vec<PathBuf>, _interactive: bool) -> std::process::ExitCode {
    match run(args) {
        Ok(AppExit::Success) => std::process::ExitCode::SUCCESS,
        Ok(_) => std::process::ExitCode::FAILURE,
        Err(e) => {
            eprintln!("Event rejected: {e}");
            std::process::ExitCode::FAILURE
        }
    }
}
