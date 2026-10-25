//! Event effects: the script's SeqEffect records (opcode 0x09) start PAX
//! elements of the effect files ReadEffect loaded, run by `effect`, and
//! this module presents their sprite quads and OMD model draws.
//!
//! Space: effect elements are placed in the script's coordinates (the
//! actors' space; the exe's y-down game space) and the camera pose is the
//! player's. The effect VM runs against a view matrix built from that
//! camera, and the view-space output goes back through the camera into
//! the scene: `effect view (x right, y down, z forward)` â†” camera local
//! (x right, y up, âˆ’z forward), `game (x, y, z)` â†” scene `(x, âˆ’y, z)`.
use super::ActorRuntime;
use super::load::{EffectFiles, FrameTrigger};
use ::effect::{Instance, ModelDraw, Quad, Rng, math::Mat};
use bevy::prelude::*;
use data::{
    event_script::Command,
    object_effect::{self, Dpd, OmdHeader, OmdMesh, Pax},
};
use event::{Timeline, event_camera};
use std::{collections::HashMap, sync::Arc};
use visual::effect_draw::{
    Blend, EffectMaterial, TexKey, model_alpha, page_image, report_missing, transparent_image,
};

/// An instance's draws of one kind with its DPD, PDATA texture mask and
/// layer rank.
type Drawn<T> = (Arc<Dpd>, [u8; 16], i8, Vec<T>);

type ParsedModel = (OmdHeader, Vec<OmdMesh>);

#[derive(Default)]
struct Batch {
    positions: Vec<[f32; 3]>,
    uvs: Vec<[f32; 2]>,
    colors: Vec<[f32; 4]>,
}

#[derive(Clone, Copy, PartialEq, Eq, Hash)]
struct BatchKey {
    texture: Option<TexKey>,
    blend: Blend,
    repeat: bool,
    /// Draw layer rank (pppRyjMngFlag): negative behind, positive in front.
    layer: i8,
}

/// An effect that follows an actor: its bone's matrix (or the actor's own
/// for bone 0x4000) times the element's local transform, every tick.
struct Bind {
    actor: i16,
    bone: i16,
    local: Mat,
}

struct Running {
    instance: Instance,
    dpd: Arc<Dpd>,
    bind: Option<Bind>,
}

/// A bone's matrix and the actor's root in scene space.
struct Pose {
    root: Mat4,
    bones: Vec<Mat4>,
}

/// The exe's draw order key (FUN_1402c2720/c27a0, direction inferred):
/// odd layers behind everything, even layers in front, 0 by depth.
fn layer_rank(layer: u8) -> i8 {
    match layer {
        0 => 0,
        v if v % 2 == 1 => -((v as i8 + 1) / 2),
        v => (v / 2) as i8,
    }
}

/// An actor's scene-space pose at `frame`, as the presenter poses it.
fn actor_pose(actors: &[ActorRuntime], timeline: &Timeline, id: i16, frame: f32) -> Option<Pose> {
    let actor = actors.iter().find(|a| a.id == id)?;
    let state = timeline.actor_at(id, frame)?;
    let motion = actor.motions.get(&state.animation.name)?;
    let motion_frame = state.animation.motion_start as f32 + state.elapsed;
    let (_, end, fps, ret) = motion.frame_window().unwrap_or((0., 1., 30., 0.));
    let motion_frame = if motion_frame > end && ret < end {
        ret + (motion_frame - end) % (end - ret)
    } else {
        motion_frame.min(end)
    };
    let (pose, _) = actor
        .model
        .skinning
        .sample_evaluated(motion, motion_frame * 60. / fps)
        .ok()?;
    let bones = pose
        .matrix_bits
        .iter()
        .map(|m| Mat4::from_cols_array(&m.map(f32::from_bits)))
        .collect();
    let root = match state.placement {
        Some(p) => {
            let turned = Quat::from_euler(
                EulerRot::YXZ,
                (-p.rotation[1]).to_radians(),
                p.rotation[0].to_radians(),
                p.rotation[2].to_radians(),
            );
            let base = Quat::from_euler(
                EulerRot::YXZ,
                p.rotation[1].to_radians(),
                p.rotation[0].to_radians(),
                p.rotation[2].to_radians(),
            );
            let n = pose.position_bits.len().max(1) as f32;
            let (mut cx, mut cz) = (0f32, 0f32);
            for q in &pose.position_bits {
                cx += f32::from_bits(q[0]);
                cz += f32::from_bits(q[2]);
            }
            let c = Vec3::new(cx / n, 0., cz / n);
            Transform {
                translation: Vec3::from_array(p.position) + base * c - turned * c,
                rotation: turned,
                scale: Vec3::ONE,
            }
            .to_matrix()
        }
        None => Mat4::IDENTITY,
    };
    Some(Pose { root, bones })
}

/// The scene matrix into the effect's y-down game space (scene = (x, âˆ’y, z)).
fn to_game(m: Mat4) -> Mat {
    let flip = Mat4::from_diagonal(Vec4::new(1., -1., 1., 1.));
    (flip * m * flip).to_cols_array_2d()
}

/// FUN_1402c49d0 (bound instance): the bound matrix times the element's
/// local rotation and translation; scale stays the element's.
fn bound_world(pose: &Pose, bind: &Bind) -> Option<Mat> {
    let m = if bind.bone == 0x4000 {
        pose.root
    } else {
        // FUN_1403b50f0 reads the bone through FUN_1401a7af0, which
        // negates the y and z columns (as the attached actors do).
        let flip = Mat4::from_diagonal(Vec4::new(1., -1., -1., 1.));
        pose.root * *pose.bones.get(bind.bone as usize)? * flip
    };
    Some(::effect::math::mul(&to_game(m), &bind.local))
}

#[derive(Resource)]
pub struct Fx {
    /// ReadEffect id â†’ PAX.
    paxes: HashMap<i16, Arc<Pax>>,
    common: Option<Arc<Dpd>>,
    script: Vec<Command>,
    next: usize,
    /// EffectId â†’ (PaxId, PaxEntryIndex) declared by EndType 4.
    declared: HashMap<i16, (i16, i16)>,
    running: HashMap<i16, Running>,
    /// Effects the actors' motion triggers spawned (no EffectId handle).
    spawned_effects: Vec<Running>,
    /// AttachEffect type 0: EffectId â†’ the actor it is bound to (+0x4c).
    bound: HashMap<i16, i16>,
    /// Actor â†’ PaxId of the container SeqEffect type 5 / 0 gave it
    /// (`obj+0x2a0`), where its motion triggers look elements up.
    containers: HashMap<i16, i16>,
    /// Frame triggers by (actor, animation script path).
    triggers: HashMap<(i16, String), Vec<FrameTrigger>>,
    /// Last motion time seen per (actor, animation start frame).
    trigger_time: HashMap<(i16, i32), f32>,
    rng: Rng,
    models: HashMap<(usize, i16), Option<ParsedModel>>,
    batches: HashMap<BatchKey, Batch>,
    dirty: bool,
    images: HashMap<TexKey, Handle<Image>>,
    materials: HashMap<BatchKey, Handle<EffectMaterial>>,
    missing: Vec<(usize, u16)>,
    spawned: Vec<Entity>,
    reported: Vec<u32>,
    pub log: Vec<serde_json::Value>,
}

/// The event frame a script record waits for.
fn script_frame(c: &Command) -> i16 {
    match c {
        Command::Effect { start, .. } | Command::AttachEffect { start, .. } => *start,
        _ => 0,
    }
}

/// The camera's transform and field of view at an event frame (the exe's
/// camera output negates y and z; actors keep the script coordinates).
pub fn camera_pose(timeline: &Timeline, frame: f32) -> Option<(Transform, f32)> {
    let cut = timeline.cut_at(frame)?;
    let data = timeline.camera(cut.camera)?;
    let seconds = (frame - cut.start as f32).min((cut.end - cut.start) as f32) / super::EVENT_FPS;
    let v = event_camera::view(data, seconds);
    let eye = Vec3::new(v.eye[0], v.eye[1], -v.eye[2]);
    let target = Vec3::new(v.target[0], v.target[1], -v.target[2]);
    let mut transform = Transform::from_translation(eye).looking_at(target, Vec3::Y);
    transform.rotate_local_z(v.roll.to_radians());
    Some((transform, v.fov.to_radians()))
}

impl Fx {
    pub fn new(files: EffectFiles, common: Option<Dpd>, commands: &[Command]) -> Self {
        let mut script: Vec<Command> = commands
            .iter()
            .filter(|c| {
                matches!(
                    c,
                    Command::Effect { .. } | Command::AttachEffect { kind: 0, .. }
                )
            })
            .cloned()
            .collect();
        script.sort_by_key(script_frame);
        Fx {
            paxes: files
                .paxes
                .into_iter()
                .map(|(i, p)| (i, Arc::new(p)))
                .collect(),
            common: common.map(Arc::new),
            script,
            next: 0,
            declared: HashMap::new(),
            running: HashMap::new(),
            spawned_effects: Vec::new(),
            bound: HashMap::new(),
            containers: HashMap::new(),
            triggers: files.triggers,
            trigger_time: HashMap::new(),
            rng: Rng::default(),
            models: HashMap::new(),
            batches: HashMap::new(),
            dirty: false,
            images: HashMap::new(),
            missing: Vec::new(),
            materials: HashMap::new(),
            spawned: Vec::new(),
            reported: Vec::new(),
            log: Vec::new(),
        }
    }

    /// Starts element `entry` of PAX `pax_id` (FUN_1402c6b30): at its own
    /// placement, or following `actor` when the element has the Bind flag.
    fn start_element(
        &self,
        pax_id: i16,
        entry: u16,
        actor: Option<i16>,
        frame: f32,
        label: serde_json::Value,
    ) -> Option<(Running, serde_json::Value)> {
        let pax = self.paxes.get(&pax_id)?.clone();
        let element = pax.by_id(entry)?;
        let dpx = pax.effect(element.effect_number)?;
        let local = ::effect::instance_matrix(element.translation, element.rotation, element.scale);
        let bind = match actor {
            Some(actor) if element.flags & 1 != 0 => Some(Bind {
                actor,
                bone: element.bone,
                local,
            }),
            _ => None,
        };
        // A bound element waits one extra tick (DAT_1406239f8, 0.1).
        let wait = element.start_wait + if bind.is_some() { 0.1 } else { 0. };
        let dpd = pax.dpds[dpx.dpd].clone();
        let instance = Instance::new(dpd.clone(), dpx.pdata, local, wait)?;
        let log = serde_json::json!({"event":"effect_start","frame":frame,
            "element":entry,"dpd":dpx.dpd,"pdata":dpx.pdata,"bound":bind.is_some(),
            "bone":element.bone,"position":element.translation,"source":label});
        Some((
            Running {
                instance,
                dpd,
                bind,
            },
            log,
        ))
    }

    fn run(&mut self, command: &Command, frame: f32) {
        let (effect, pax, pax_entry, end_type, fade) = match *command {
            // Type 0 binds the record `actor` (EffectId) to actor `attach`
            // (FUN_1402d07d0; the bone is unused).
            Command::AttachEffect {
                attach,
                actor,
                kind: 0,
                ..
            } => {
                self.bound.insert(actor, attach);
                return;
            }
            Command::Effect {
                effect,
                pax,
                pax_entry,
                end_type,
                fade,
                ..
            } => (effect, pax, pax_entry, end_type, fade),
            _ => return,
        };
        match end_type {
            // Declare the record.
            4 => {
                self.declared.insert(effect, (pax, pax_entry));
            }
            // Start (FUN_1402d9340) or container re-bind only (type 5,
            // FUN_1402d92a0): both point the bound actor's `obj+0x2a0` at
            // the record's container, where its motion triggers look up
            // elements.
            0 | 5 => {
                let Some(&(pax_id, entry)) = self.declared.get(&effect) else {
                    return;
                };
                let actor = self.bound.get(&effect).copied();
                if let Some(actor) = actor {
                    self.containers.insert(actor, pax_id);
                }
                if end_type == 5 {
                    return;
                }
                let label = serde_json::json!({"seq_effect":effect});
                if let Some((running, log)) =
                    self.start_element(pax_id, entry as u16, actor, frame, label)
                {
                    let mut log = log;
                    log["effect"] = effect.into();
                    self.log.push(log);
                    self.running.insert(effect, running);
                }
            }
            // Stop only if the effect loops.
            1 => {
                if self
                    .running
                    .get(&effect)
                    .is_some_and(|r| r.instance.looping())
                {
                    self.running.remove(&effect);
                }
            }
            // Release: a looping effect ends once its parts are gone.
            2 => {
                if let Some(r) = self.running.get_mut(&effect) {
                    r.instance.release();
                }
            }
            // Fade out over FadeFrame (immediate stop when not positive).
            3 => {
                if fade > 0 {
                    if let Some(r) = self.running.get_mut(&effect) {
                        r.instance.fade_out(fade as f32);
                    }
                } else {
                    self.running.remove(&effect);
                }
            }
            // Stop unconditionally (FadeFrame is ignored).
            6 => {
                self.running.remove(&effect);
            }
            _ => {}
        }
    }

    /// FUN_14040e960 → FUN_14040dd70 case 1: when an actor's motion time
    /// passes a trigger frame, element `parameters[0]` of the container its
    /// SeqEffect type 5 set starts, bound to the actor.
    fn fire_triggers(&mut self, frame: f32, timeline: &Timeline, actors: &[ActorRuntime]) {
        for actor in actors {
            let Some(state) = timeline.actor_at(actor.id, frame) else {
                continue;
            };
            let Some(list) = self.triggers.get(&(actor.id, state.animation.name.clone())) else {
                continue;
            };
            // The controller's time is in 60 Hz ticks (FUN_1403c6740): the
            // trigger frames are compared in those, not in 30 Hz frames.
            let fps = actor
                .motions
                .get(&state.animation.name)
                .and_then(|m| m.frame_window().ok())
                .map_or(30., |w| w.2);
            let to_ticks = 60. / fps;
            let start = state.animation.motion_start as f32 * to_ticks;
            let now = (state.animation.motion_start as f32 + state.elapsed) * to_ticks;
            let key = (actor.id, state.animation.start);
            // The controller starts at the animation's first motion tick.
            let before = self.trigger_time.insert(key, now).unwrap_or(start - 1.);
            let Some(&pax_id) = self.containers.get(&actor.id) else {
                continue;
            };
            let due: Vec<(u8, Vec<i16>, f32)> = list
                .iter()
                .filter(|t| t.frame > before && t.frame <= now)
                .map(|t| (t.id, t.parameters.clone(), t.frame))
                .collect();
            for (id, parameters, at) in due {
                if id != 1 {
                    continue;
                }
                let Some(&element) = parameters.first() else {
                    continue;
                };
                let label = serde_json::json!({"trigger":{"actor":actor.id,"motion_frame":at}});
                if let Some((running, log)) =
                    self.start_element(pax_id, element as u16, Some(actor.id), frame, label)
                {
                    self.log.push(log);
                    self.spawned_effects.push(running);
                }
            }
        }
    }

    /// One 60 Hz tick: due script records and motion triggers, then every
    /// instance one game frame, converted to scene-space triangles.
    pub(super) fn step(&mut self, frame: f32, timeline: &Timeline, actors: &[ActorRuntime]) {
        while let Some(c) = self.script.get(self.next).cloned() {
            if script_frame(&c) as f32 > frame {
                break;
            }
            self.next += 1;
            self.run(&c, frame);
        }
        self.fire_triggers(frame, timeline, actors);
        self.batches.clear();
        self.dirty = true;
        let Some((cam, _)) = camera_pose(timeline, frame) else {
            return;
        };
        let to_scene =
            Mat4::from_scale_rotation_translation(Vec3::ONE, cam.rotation, cam.translation)
                * Mat4::from_diagonal(Vec4::new(1., -1., -1., 1.));
        let view_m = Mat4::from_diagonal(Vec4::new(1., -1., -1., 1.))
            * Mat4::from_scale_rotation_translation(Vec3::ONE, cam.rotation, cam.translation)
                .inverse()
            * Mat4::from_diagonal(Vec4::new(1., -1., 1., 1.));
        let view: Mat = view_m.to_cols_array_2d();
        // Bound instances follow their actor's bone this tick.
        let mut poses: HashMap<i16, Option<Pose>> = HashMap::new();
        for r in self
            .running
            .values_mut()
            .chain(self.spawned_effects.iter_mut())
        {
            let Some(bind) = &r.bind else {
                continue;
            };
            let pose = poses
                .entry(bind.actor)
                .or_insert_with(|| actor_pose(actors, timeline, bind.actor, frame));
            if let Some(world) = pose.as_ref().and_then(|p| bound_world(p, bind)) {
                r.instance.world = world;
            }
        }
        let mut quads: Vec<Drawn<Quad>> = Vec::new();
        let mut draws: Vec<Drawn<ModelDraw>> = Vec::new();
        let mut finished = Vec::new();
        let ids: Vec<i16> = self.running.keys().copied().collect();
        let mut step_one = |r: &mut Running, id: i16, rng: &mut Rng| -> bool {
            let q = r.instance.frame(&view, rng);
            for &kind in &r.instance.unsupported {
                if !self.reported.contains(&kind) {
                    self.reported.push(kind);
                    self.log
                        .push(serde_json::json!({"event":"effect_unsupported",
                        "program":kind,"effect":id}));
                }
            }
            // An instance that runs a program this translation lacks is
            // stepped (so its timing holds) but not shown: a partial
            // effect would misrepresent the original.
            let complete = r.instance.unsupported.is_empty();
            let d = std::mem::take(&mut r.instance.model_draws);
            let layer = layer_rank(r.instance.layer());
            let mask = r.instance.use_texture();
            if complete && !q.is_empty() {
                quads.push((r.dpd.clone(), mask, layer, q));
            }
            if complete && !d.is_empty() {
                draws.push((r.dpd.clone(), mask, layer, d));
            }
            r.instance.finished()
        };
        for id in ids {
            let Some(r) = self.running.get_mut(&id) else {
                continue;
            };
            if step_one(r, id, &mut self.rng) {
                finished.push(id);
            }
        }
        for id in finished {
            self.running.remove(&id);
        }
        let mut alive = Vec::new();
        for mut r in std::mem::take(&mut self.spawned_effects) {
            if !step_one(&mut r, -1, &mut self.rng) {
                alive.push(r);
            }
        }
        self.spawned_effects = alive;
        let scene = |p: [f32; 3]| to_scene.transform_point3(Vec3::from_array(p)).to_array();
        for (dpd, mask, layer, list) in quads {
            for q in list {
                let Some(key) = TexKey::pick(&dpd, &mask, self.common.as_ref(), q.tex0) else {
                    let at = Arc::as_ptr(&dpd) as usize;
                    report_missing(&mut self.missing, at, q.tex0, "cutscene sprite");
                    continue;
                };
                let size = key.size();
                let blend = Blend::of(q.blend);
                let batch = self
                    .batches
                    .entry(BatchKey {
                        texture: Some(key),
                        blend,
                        repeat: true,
                        layer,
                    })
                    .or_default();
                let grade = |c: [f32; 4]| c.map(|v| v.clamp(0., 1.99));
                for i in [0, 1, 2, 0, 2, 3] {
                    let colour = grade(q.corner_colours.map_or(q.colour, |c| c[i]));
                    batch.positions.push(scene(q.corners[i]));
                    batch
                        .uvs
                        .push([q.uv[i][0] / size[0] as f32, q.uv[i][1] / size[1] as f32]);
                    batch.colors.push(colour);
                }
            }
        }
        for (dpd, mask, layer, list) in draws {
            for d in list {
                self.model(&dpd, &mask, &d, layer, &scene);
            }
        }
    }

    fn model(
        &mut self,
        dpd: &Arc<Dpd>,
        mask: &[u8; 16],
        d: &ModelDraw,
        layer: i8,
        scene: &impl Fn([f32; 3]) -> [f32; 3],
    ) {
        // A depth-only draw writes no colour.
        if d.depth_only {
            return;
        }
        let cached = self
            .models
            .entry((Arc::as_ptr(dpd) as usize, d.model))
            .or_insert_with(|| {
                let m = dpd.models.get(&d.model)?;
                let header = object_effect::omd_header(&dpd.bytes, m.hdr).ok()?;
                let meshes = object_effect::omd_meshes(&dpd.bytes, m.hdr).ok()?;
                Some((header, meshes))
            });
        let Some((header, meshes)) = cached.as_ref() else {
            return;
        };
        let vertex = |i: u16| -> Option<[f32; 3]> {
            let v = match &d.vertices {
                Some(o) => o.get(i as usize).copied(),
                None => dpd.vertex(d.model, i),
            }?;
            let p = ::effect::math::apply(&d.a, [v[0] as f32, v[1] as f32, v[2] as f32, 1.]);
            Some(scene([p[0], p[1], p[2]]))
        };
        let blend = Blend::of(model_alpha(header.alpha, d.mode));
        let repeat = header.flags & 4 != 0;
        let part = d.rgba.map(|c| c as f32 / 128.);
        for mesh in meshes {
            let texture = if mesh.textured {
                let Some(t) = TexKey::pick(dpd, mask, self.common.as_ref(), mesh.tex0) else {
                    let at = Arc::as_ptr(dpd) as usize;
                    report_missing(&mut self.missing, at, mesh.tex0, "cutscene model");
                    continue;
                };
                Some(t)
            } else {
                None
            };
            let shift = texture.map_or(4, |t| t.tw as u32 + 4).min(31);
            let batch = self
                .batches
                .entry(BatchKey {
                    texture,
                    blend,
                    repeat,
                    layer,
                })
                .or_default();
            for t in &mesh.triangles {
                let Some(p) = t.index.map(vertex).into_iter().collect::<Option<Vec<_>>>() else {
                    continue;
                };
                for (k, point) in p.iter().enumerate() {
                    batch.positions.push(*point);
                    let (u, v) = (t.uv[k][0] as i32, t.uv[k][1] as i32);
                    let su = d.scroll[0] >> shift;
                    let sv = d.scroll[1] >> (texture.map_or(4, |t| t.th as u32 + 4).min(31));
                    batch.uvs.push([
                        u.wrapping_add(su) as f32 / 4096.,
                        v.wrapping_add(sv) as f32 / 4096.,
                    ]);
                    // VS 005: sat(vertex/255 Â· part/128) Â· 255/128.
                    let c = t.colour[k];
                    let out: [f32; 4] = std::array::from_fn(|i| {
                        ((c[i] as f32 / 255.) * part[i]).min(1.) * (255. / 128.)
                    });
                    batch.colors.push(out);
                }
            }
        }
    }

    fn image(&mut self, key: TexKey, repeat: bool, images: &mut Assets<Image>) -> Handle<Image> {
        if let Some(h) = self.images.get(&key) {
            return h.clone();
        }
        // The DPD whose pointer the key holds is a running instance's, a
        // loaded file's or the common one.
        let image = self
            .running
            .values()
            .map(|r| &r.dpd)
            .chain(self.paxes.values().flat_map(|p| p.dpds.iter()))
            .chain(self.common.as_ref())
            .find(|d| Arc::as_ptr(d) as usize == key.dpd)
            .map_or_else(
                || {
                    error!("effect texture missing: no DPD for {key:?}; drawn transparent");
                    transparent_image()
                },
                |d| page_image(d, key, repeat),
            );
        let handle = images.add(image);
        self.images.insert(key, handle.clone());
        handle
    }
}

/// Rebuilds the effect meshes whenever a tick produced new geometry.
pub fn present(
    mut commands: Commands,
    mut fx: ResMut<Fx>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<EffectMaterial>>,
    mut images: ResMut<Assets<Image>>,
) {
    if !fx.dirty {
        return;
    }
    fx.dirty = false;
    for e in std::mem::take(&mut fx.spawned) {
        commands.entity(e).despawn();
    }
    let batches = std::mem::take(&mut fx.batches);
    for (key, batch) in &batches {
        if batch.positions.is_empty() {
            continue;
        }
        let material = match fx.materials.get(key) {
            Some(m) => m.clone(),
            None => {
                let texture = key.texture.map(|t| fx.image(t, key.repeat, &mut images));
                let m = materials.add(EffectMaterial::new(
                    key.blend,
                    texture,
                    key.layer as f32 * 500.,
                    true,
                ));
                fx.materials.insert(*key, m.clone());
                m
            }
        };
        let mesh = visual::room::mesh(
            batch.positions.clone(),
            batch.uvs.clone(),
            batch.colors.clone(),
        );
        let entity = commands
            .spawn((
                Mesh3d(meshes.add(mesh)),
                MeshMaterial3d(material),
                Transform::default(),
                Visibility::default(),
            ))
            .id();
        fx.spawned.push(entity);
    }
}
