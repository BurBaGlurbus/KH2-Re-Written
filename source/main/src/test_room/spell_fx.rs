//! The spells' own effects in the test room: the PAX of the `magic/*.mag`
//! file of the spell's level (`FUN_1402c6b30` on the spell's container),
//! started and steered by the spell scripts' `pax_start`, `pax_start_bind`
//! and `effect_*` calls (`player::magic::Fx`).
//!
//! An element is placed as the original's instance creator does
//! (`FUN_1402c6b30`, `FUN_1402c5f70`, `FUN_1402c49d0`): unbound elements
//! take the PAX translation, rotation and scale as the instance transform,
//! which `effect_set_pos` / `effect_set_dir` / `effect_set_scale` then
//! replace; bound elements follow the object's bone matrix (flag 1), only
//! its position (flag 4, BindOnlyPos) or only at the start (flag 2,
//! BindOnlyStart). Bone codes from 0x4000 are the object's own frames
//! (`FUN_1403b50f0`): 0x4000 its root, 0x4001 / 0x4009 its body centre and
//! 0x4003 / 0x4004 the Keyblade's top. An element's own sound plays at its
//! start, after its start wait.
//!
//! The drawing follows the Drive effects' presenter (`drive_fx.rs`): the
//! PC's texture for each TEX0 ([`visual::effect_draw`]), OMD model draws,
//! blend and layer batches.
use super::*;
use ::effect::{Instance, ModelDraw, Quad, Rng, math::Mat};
use data::object_effect::{Dpd, OmdHeader, OmdMesh, Pax};
use player::magic::{Anchor, Fx, FxLive};
use visual::effect_draw::{
    Blend, EffectMaterial, TexKey, model_alpha, page_image, report_missing, transparent_image,
};

/// A script's effect call with the PAX of the spell it belongs to.
pub struct Call {
    pub pax: Option<Arc<Pax>>,
    pub fx: Fx,
}

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
    layer: i8,
}

/// Odd layers behind everything, even layers in front, 0 by depth
/// (FUN_1402c2720 / FUN_1402c27a0).
fn layer_rank(layer: u8) -> i8 {
    match layer {
        0 => 0,
        v if v % 2 == 1 => -((v as i8 + 1) / 2),
        v => (v / 2) as i8,
    }
}

/// Scene matrix into the effect space (x, −y, −z).
fn to_effect(m: Mat4) -> Mat {
    let flip = Mat4::from_diagonal(Vec4::new(1., -1., -1., 1.));
    (flip * m * flip).to_cols_array_2d()
}

/// A simulation position in the effect space.
fn point_to_effect(p: [f32; 3]) -> [f32; 3] {
    [p[0], -p[1], -p[2]]
}

fn point_from_effect(p: [f32; 3]) -> [f32; 3] {
    [p[0], -p[1], -p[2]]
}

/// `effect_set_dir` (FUN_14041e100, FUN_1403b7710): the pair of Euler
/// angles that turn the effect's +Z onto `dir` (effect space, y down).
fn dir_euler(dir: [f32; 3]) -> [f32; 3] {
    let d = point_to_effect(dir);
    let h = (d[0] * d[0] + d[2] * d[2]).sqrt();
    [(-d[1]).atan2(h), d[0].atan2(d[2]), 0.]
}

struct Running {
    key: u64,
    instance: Instance,
    dpd: Arc<Dpd>,
    bone: i16,
    flags: u32,
    local: ([f32; 3], [f32; 3], [f32; 3]),
    anchor: Option<Anchor>,
    /// `effect_set_pos` (simulation space), `effect_set_dir` and
    /// `effect_set_scale` (effect space).
    pos: Option<[f32; 3]>,
    rot: Option<[f32; 3]>,
    scl: Option<[f32; 3]>,
    /// BindOnlyStart: the world matrix once taken.
    fixed: Option<Mat>,
    /// The element's sound and the ticks left before it plays.
    sound: Option<u32>,
    sound_wait: f32,
}

/// Sora's frames of this tick.
struct Frames {
    root: Mat4,
    hand: Option<Mat4>,
    tip: Vec3,
    bones: Option<HashMap<usize, Mat4>>,
    body_lift: f32,
    heal_lift: f32,
    members: Vec<MemberFrame>,
    /// The summon that is out (its drawn root and heading).
    summon: Option<MemberFrame>,
    /// Battle actors by gallery id (their root and heading) and their bones.
    actors: HashMap<u64, MemberFrame>,
    actor_bones: HashMap<u64, super::enemy_combat::ActorBones>,
}

/// A party member's frame of this tick: origin, facing, the staff's top as
/// a bone matrix (model space) and the heights of its type-1 and type-9
/// collision centres.
struct MemberFrame {
    pos: Vec3,
    yaw: f32,
    staff: Option<Mat4>,
    body_lift: f32,
    heal_lift: f32,
}

impl Frames {
    fn of(game: &Game) -> Frames {
        let p = game.player.position;
        let root = Mat4::from_rotation_translation(
            Quat::from_rotation_y(game.player.yaw),
            Vec3::from_array(p),
        );
        let clip = game
            .poses
            .get(&(game.player.action, game.player.armed))
            .and_then(|&c| game.clip_poses.get(c));
        let frame = game.player.motion_frame(&game.clips) as usize;
        let hand = clip.and_then(|c| c.weapon.get(frame.min(c.weapon.len().saturating_sub(1))));
        let bones = clip
            .and_then(|c| c.body.get(frame.min(c.body.len().saturating_sub(1))))
            .cloned();
        // The Keyblade's top (WEAPON_TOP, FUN_1403b5450): the end of bone 2's
        // reach, bone 2's origin plus its 78-unit offset along the blade
        // (Kingdom Key, measured: the farthest vertex lies 77.96 from the
        // bone). Without the weapon's own motion it is the vertex farthest
        // from the grip.
        let origin = clip
            .and_then(|c| c.blade.get(frame.min(c.blade.len().saturating_sub(1))))
            .map_or(Vec3::ZERO, |b| b.w_axis.truncate());
        let tip = game
            .weapon_display
            .iter()
            .copied()
            .max_by(|a, b| {
                a.distance_squared(origin)
                    .total_cmp(&b.distance_squared(origin))
            })
            .unwrap_or(Vec3::ZERO);
        let members = game
            .party
            .iter()
            .map(|m| {
                let data = m.kind.motions.get(&m.follower.motion());
                // A motion without its own weapon pose keeps the idle one.
                let staff = data
                    .filter(|d| !d.staff.is_empty())
                    .map(|d| (d, d.frame_of(m.follower.tick)))
                    .or_else(|| m.kind.motions.get(&0).map(|d| (d, 0)))
                    .and_then(|(d, f)| d.staff.get(f).copied());
                MemberFrame {
                    pos: Vec3::from_array(m.follower.position),
                    yaw: m.follower.yaw,
                    staff,
                    body_lift: m.kind.body_lift,
                    heal_lift: m.kind.heal_lift,
                }
            })
            .collect();
        // [I] A summon's bones are not sampled: its effects take its root,
        // lifted by Sora's heights for the body-centre codes.
        let summon = super::summon::frame(game).map(|(pos, yaw)| MemberFrame {
            pos,
            yaw,
            staff: None,
            body_lift: 85.,
            heal_lift: 105.,
        });
        // A battle actor: its root and heading, and its bones of this tick
        // (`actor_bones`); [I] Sora's heights for the body-centre codes.
        let actors = game
            .actor_poses
            .iter()
            .map(|(&id, &(pos, yaw))| {
                (
                    id,
                    MemberFrame {
                        pos,
                        yaw,
                        staff: None,
                        body_lift: 85.,
                        heal_lift: 105.,
                    },
                )
            })
            .collect();
        Frames {
            root,
            hand: hand.copied(),
            tip,
            bones,
            body_lift: game.magic.as_ref().map_or(85., |m| m.src.body_lift),
            heal_lift: game.magic.as_ref().map_or(105., |m| m.src.heal_lift),
            members,
            summon,
            actors,
            actor_bones: game.actor_bones.clone(),
        }
    }

    /// The bound object's bone matrix (FUN_1403b50f0), in the scene space.
    fn bone(&self, anchor: Anchor, bone: i16) -> Mat4 {
        let member = match anchor {
            Anchor::Party(i) if i > 0 => self.members.get(i - 1),
            Anchor::Summon => self.summon.as_ref(),
            Anchor::Actor(id) => self.actors.get(&id),
            _ => None,
        };
        let root = match member {
            Some(m) => Mat4::from_rotation_translation(Quat::from_rotation_y(m.yaw), m.pos),
            None => self.root,
        };
        let (body_lift, heal_lift) = member.map_or((self.body_lift, self.heal_lift), |m| {
            (m.body_lift, m.heal_lift)
        });
        let own = matches!(anchor, Anchor::Caster | Anchor::Party(0));
        // A battle actor's own bone (FUN_1403b50f0): its model-space matrix
        // with the root its position already carries taken out, read with
        // the y and z columns negated as Sora's are.
        if let Anchor::Actor(id) = anchor
            && let Ok(b) = usize::try_from(bone)
            && let Some(m) = self.actor_bones.get(&id).and_then(|a| {
                let m = a.matrices.get(b)?;
                Some(Mat4::from_translation(-a.removed_root) * *m)
            })
        {
            return root * m * Mat4::from_diagonal(Vec4::new(1., -1., -1., 1.));
        }
        match bone {
            0x4000 => root,
            0x4001 => root * Mat4::from_translation(Vec3::new(0., body_lift, 0.)),
            0x4009 => root * Mat4::from_translation(Vec3::new(0., heal_lift, 0.)),
            0x4003 | 0x4004 if own => match self.hand {
                Some(h) => root * h * Mat4::from_translation(self.tip),
                None => root * Mat4::from_translation(Vec3::new(0., body_lift, 0.)),
            },
            // A party member's weapon slot 0 top (FUN_1403b5450): the
            // staff's bone 2 with its top vertex.
            0x4003 | 0x4004 => match member.and_then(|m| m.staff) {
                Some(staff) => root * staff,
                None => root * Mat4::from_translation(Vec3::new(0., body_lift, 0.)),
            },
            0x4003..=0x4009 => root * Mat4::from_translation(Vec3::new(0., body_lift, 0.)),
            b if b >= 0 && own => match self.bones.as_ref().and_then(|m| m.get(&(b as usize))) {
                // The bone read through FUN_1401a7af0 negates its y and z
                // columns.
                Some(m) => root * *m * Mat4::from_diagonal(Vec4::new(1., -1., -1., 1.)),
                None => root,
            },
            _ => root,
        }
    }
}

/// The instance world matrix of an element (FUN_1402c49d0 for a bound
/// element, the instance's own transform otherwise):  is the PAX
/// translation, rotation and scale; ,  and  what the script's
///  calls replaced.
#[allow(clippy::too_many_arguments)]
fn element_world(
    flags: u32,
    anchor: Option<Anchor>,
    bone: i16,
    local: ([f32; 3], [f32; 3], [f32; 3]),
    pos: Option<[f32; 3]>,
    rot: Option<[f32; 3]>,
    scl: Option<[f32; 3]>,
    frames: &Frames,
) -> Mat {
    let (t, r, s) = local;
    let rot = rot.unwrap_or(r);
    let scl = scl.unwrap_or(s);
    match anchor.filter(|_| flags & 1 != 0) {
        Some(anchor) => {
            let bone = to_effect(frames.bone(anchor, bone));
            if flags & 4 != 0 {
                // BindOnlyPos: the position through the bone matrix, the
                // element's own rotation and scale.
                let o = ::effect::math::apply(&bone, [t[0], t[1], t[2], 1.]);
                ::effect::instance_matrix([o[0], o[1], o[2]], rot, scl)
            } else {
                ::effect::math::mul(&bone, &::effect::instance_matrix(t, rot, scl))
            }
        }
        None => ::effect::instance_matrix(pos.map_or(t, point_to_effect), rot, scl),
    }
}

impl Running {
    /// The instance world matrix this tick.
    fn world(&mut self, frames: &Frames) -> Mat {
        if let Some(f) = self.fixed {
            return f;
        }
        let world = element_world(
            self.flags,
            self.anchor,
            self.bone,
            self.local,
            self.pos,
            self.rot,
            self.scl,
            frames,
        );
        // BindOnlyStart: bound once, at the start.
        if self.anchor.is_some() && self.flags & 1 != 0 && self.flags & 2 != 0 {
            self.fixed = Some(world);
        }
        world
    }

    fn sim_pos(&self) -> [f32; 3] {
        let m = self.instance.world;
        point_from_effect([m[3][0], m[3][1], m[3][2]])
    }
}

#[derive(Resource, Default)]
pub struct SpellFx {
    common: Option<Arc<Dpd>>,
    running: Vec<Running>,
    rng: Rng,
    models: HashMap<(usize, i16), Option<ParsedModel>>,
    batches: HashMap<BatchKey, Batch>,
    dirty: bool,
    images: HashMap<TexKey, Handle<Image>>,
    materials: HashMap<BatchKey, Handle<EffectMaterial>>,
    missing: Vec<(usize, u16)>,
    spawned: Vec<Entity>,
    dpds: Vec<Arc<Dpd>>,
    reported: Vec<u32>,
}

impl SpellFx {
    pub fn new(common: Option<Arc<Dpd>>) -> Self {
        SpellFx {
            common,
            ..Default::default()
        }
    }
}

/// One tick: the scripts' calls, then every instance steps and batches.
pub fn tick(mut game: ResMut<Game>, rig: Res<CameraRig>, mut fx: ResMut<SpellFx>) {
    let calls = std::mem::take(&mut game.fx_calls);
    let tick = game.tick;
    let frames = Frames::of(&game);
    let mut sounds = Vec::new();
    let mut log = Vec::new();
    let mut live: HashMap<u64, FxLive> = HashMap::new();
    // Waiting effect sounds count down one tick at a time and play when
    // their element starts.
    for r in fx.running.iter_mut() {
        if let Some(id) = r.sound
            && r.sound_wait > 0.
        {
            r.sound_wait -= 1.;
            if r.sound_wait <= 0. {
                sounds.push(id);
                r.sound = None;
            }
        }
    }
    for call in calls {
        match call.fx {
            Fx::Start { key, id, bound, at } => {
                let started = call.pax.as_ref().and_then(|pax| {
                    let element = pax.by_id(id)?;
                    let dpx = pax.effect(element.effect_number)?;
                    let dpd = pax.dpds.get(dpx.dpd)?.clone();
                    Some((element.clone(), dpx, dpd))
                });
                let Some((element, dpx, dpd)) = started else {
                    log.push(serde_json::json!({"event":"spell_effect_missing","tick":tick,"key":key,"element":id}));
                    live.insert(
                        key,
                        FxLive {
                            alive: false,
                            pos: at.unwrap_or_default(),
                            tracked: false,
                        },
                    );
                    continue;
                };
                // A bound element waits one extra tick (DAT_1406239f8, 0.1).
                let wait = element.start_wait + if element.flags & 1 != 0 { 0.1 } else { 0. };
                let local = (element.translation, element.rotation, element.scale);
                let mut r = Running {
                    key,
                    instance: match Instance::new(
                        dpd.clone(),
                        dpx.pdata,
                        ::effect::math::IDENTITY,
                        wait,
                    ) {
                        Some(i) => i,
                        None => continue,
                    },
                    dpd: dpd.clone(),
                    bone: element.bone,
                    flags: element.flags,
                    local,
                    anchor: bound,
                    pos: at,
                    rot: None,
                    scl: None,
                    fixed: None,
                    sound: (element.sound > 0).then_some(element.sound as u32),
                    sound_wait: wait,
                };
                r.instance.world = r.world(&frames);
                if let Some(id) = r.sound
                    && wait <= 0.
                {
                    sounds.push(id);
                    r.sound = None;
                }
                let w = r.instance.world;
                let at = point_from_effect([w[3][0], w[3][1], w[3][2]]);
                log.push(
                    serde_json::json!({"event":"spell_effect","tick":tick,"key":key,
                    "element":element.id,"effect":element.effect_number,"bone":element.bone,
                    "flags":element.flags,"bound":bound.is_some(),"dpd":dpx.dpd,"pdata":dpx.pdata,
                    "position":at}),
                );
                if !fx.dpds.iter().any(|x| Arc::ptr_eq(x, &dpd)) {
                    fx.dpds.push(dpd);
                }
                // A handle started again replaces the old effect.
                fx.running.retain(|o| o.key != key);
                fx.running.push(r);
            }
            Fx::Pos(key, p) => {
                for r in fx.running.iter_mut().filter(|r| r.key == key) {
                    r.pos = Some(p);
                }
            }
            Fx::Dir(key, d) => {
                let e = dir_euler(d);
                for r in fx.running.iter_mut().filter(|r| r.key == key) {
                    r.rot = Some(e);
                }
            }
            Fx::Scale(key, s) => {
                for r in fx.running.iter_mut().filter(|r| r.key == key) {
                    r.scl = Some([s, s, s]);
                }
            }
            Fx::Fade(key, frames) => {
                for r in fx.running.iter_mut().filter(|r| r.key == key) {
                    r.instance.fade_out(frames);
                }
            }
            Fx::Kill(key) => {
                for r in fx.running.iter_mut().filter(|r| r.key == key) {
                    r.instance.fade_out(0.);
                }
            }
            Fx::LoopEnd(key) => {
                for r in fx.running.iter_mut().filter(|r| r.key == key) {
                    r.instance.release();
                }
            }
        }
    }
    for s in sounds {
        game.sound_queue.push((s, "spell_effect_se"));
    }
    let view = game_view(rig.camera.eye, rig.camera.look_at);
    let view_inverse = Mat4::from_cols_array_2d(&view).inverse();
    let fx = &mut *fx;
    fx.batches.clear();
    fx.dirty = true;
    let mut quads: Vec<Drawn<Quad>> = Vec::new();
    let mut draws: Vec<Drawn<ModelDraw>> = Vec::new();
    let mut triangles: Vec<(i8, Vec<::effect::Triangle>)> = Vec::new();
    let mut unsupported = Vec::new();
    let trace = std::env::var_os("KH2_FX_TRACE").is_some();
    for r in fx.running.iter_mut() {
        let world = r.world(&frames);
        r.instance.world = world;
        let q = r.instance.frame(&view, &mut fx.rng);
        for &k in &r.instance.unsupported {
            if !fx.reported.contains(&k) {
                fx.reported.push(k);
                unsupported.push(k);
            }
        }
        // A program this translation lacks: stepped, not shown.
        let complete = r.instance.unsupported.is_empty();
        let d = std::mem::take(&mut r.instance.model_draws);
        if trace {
            let models: Vec<_> = d
                .iter()
                .map(|m| serde_json::json!([m.model, m.rgba, m.mode, m.depth_only]))
                .collect();
            let near = q
                .iter()
                .flat_map(|q| q.corners)
                .map(|c| c[2])
                .fold(f32::MAX, f32::min);
            log.push(
                serde_json::json!({"event":"fx_trace","tick":tick,"key":r.key,
                "quads":q.len(),"quad_near":near,"models":models}),
            );
        }
        let layer = layer_rank(r.instance.layer());
        let mask = r.instance.use_texture();
        if complete && !q.is_empty() {
            quads.push((r.dpd.clone(), mask, layer, q));
        }
        if complete && !d.is_empty() {
            draws.push((r.dpd.clone(), mask, layer, d));
        }
        let t = std::mem::take(&mut r.instance.triangles);
        if complete && !t.is_empty() {
            triangles.push((layer, t));
        }
    }
    // What the scripts read next tick: alive and the effect position.
    for r in fx.running.iter() {
        live.insert(
            r.key,
            FxLive {
                alive: !r.instance.finished(),
                pos: r.sim_pos(),
                tracked: true,
            },
        );
    }
    fx.running.retain(|r| !r.instance.finished());
    // A finished effect stays known as dead for one tick so the loops see it.
    game.fx_live = live;
    for k in unsupported {
        game.event(serde_json::json!({"event":"spell_effect_unsupported","tick":tick,"program":k}));
    }
    for v in log {
        game.event(v);
    }
    let scene = |p: [f32; 3]| {
        from_world(
            view_inverse
                .transform_point3(Vec3::from_array(p))
                .to_array(),
        )
        .to_array()
    };
    for (dpd, mask, layer, list) in quads {
        for q in list {
            let Some(key) = TexKey::pick(&dpd, &mask, fx.common.as_ref(), q.tex0) else {
                report_missing(
                    &mut fx.missing,
                    Arc::as_ptr(&dpd) as usize,
                    q.tex0,
                    "spell sprite",
                );
                continue;
            };
            let size = key.size();
            let blend = Blend::of(q.blend);
            let batch = fx
                .batches
                .entry(BatchKey {
                    texture: Some(key),
                    blend,
                    repeat: true,
                    layer,
                })
                .or_default();
            let colour = q.colour.map(|c| c.clamp(0., 1.99));
            for i in [0, 1, 2, 0, 2, 3] {
                batch.positions.push(scene(q.corners[i]));
                batch
                    .uvs
                    .push([q.uv[i][0] / size[0] as f32, q.uv[i][1] / size[1] as f32]);
                batch.colors.push(colour);
            }
        }
    }
    for (layer, list) in triangles {
        for t in list {
            let blend = Blend::of(t.blend);
            let batch = fx
                .batches
                .entry(BatchKey {
                    texture: None,
                    blend,
                    repeat: false,
                    layer,
                })
                .or_default();
            for i in 0..3 {
                let c = t.colours[i].map(|v| v as f32 / 128.);
                batch.positions.push(scene(t.corners[i]));
                batch.uvs.push([0.; 2]);
                batch.colors.push(c.map(|v| v.clamp(0., 1.99)));
            }
        }
    }
    for (dpd, mask, layer, list) in draws {
        for d in list {
            model(fx, &dpd, &mask, &d, layer, &scene);
        }
    }
}

/// An OMD model draw (pppDrawMdl3), as the event presenter batches it.
fn model(
    fx: &mut SpellFx,
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
    let cached = fx
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
    let common = fx.common.clone();
    let mut out_batches: Vec<(BatchKey, Batch)> = Vec::new();
    for mesh in meshes {
        let texture = if mesh.textured {
            let Some(t) = TexKey::pick(dpd, mask, common.as_ref(), mesh.tex0) else {
                report_missing(
                    &mut fx.missing,
                    Arc::as_ptr(dpd) as usize,
                    mesh.tex0,
                    "spell model",
                );
                continue;
            };
            Some(t)
        } else {
            None
        };
        let shift = texture.map_or(4, |t| t.tw as u32 + 4).min(31);
        let mut batch = Batch::default();
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
                let c = t.colour[k];
                let out: [f32; 4] = std::array::from_fn(|i| {
                    ((c[i] as f32 / 255.) * part[i]).min(1.) * (255. / 128.)
                });
                batch.colors.push(out);
            }
        }
        out_batches.push((
            BatchKey {
                texture,
                blend,
                repeat,
                layer,
            },
            batch,
        ));
    }
    for (key, b) in out_batches {
        let e = fx.batches.entry(key).or_default();
        e.positions.extend(b.positions);
        e.uvs.extend(b.uvs);
        e.colors.extend(b.colors);
    }
}

fn image(fx: &mut SpellFx, key: TexKey, repeat: bool, images: &mut Assets<Image>) -> Handle<Image> {
    if let Some(h) = fx.images.get(&key) {
        return h.clone();
    }
    let image = fx
        .dpds
        .iter()
        .chain(fx.common.as_ref())
        .find(|d| Arc::as_ptr(d) as usize == key.dpd)
        .map_or_else(
            || {
                error!("effect texture missing: no DPD for {key:?}; drawn transparent");
                transparent_image()
            },
            |d| page_image(d, key, repeat),
        );
    let handle = images.add(image);
    fx.images.insert(key, handle.clone());
    handle
}

/// Rebuilds the effect meshes from this tick's batches.
pub fn present(
    mut commands: Commands,
    fx: Option<ResMut<SpellFx>>,
    scene: Option<Res<SceneRoot>>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<EffectMaterial>>,
    mut images: ResMut<Assets<Image>>,
) {
    let Some(mut fx) = fx else { return };
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
                let texture = key
                    .texture
                    .map(|t| image(&mut fx, t, key.repeat, &mut images));
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
        let mut e = commands.spawn((
            Mesh3d(meshes.add(mesh)),
            MeshMaterial3d(material),
            Transform::default(),
            Visibility::default(),
        ));
        if let Some(s) = &scene {
            e.insert(ChildOf(s.0));
        }
        fx.spawned.push(e.id());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn frames() -> Frames {
        Frames {
            root: Mat4::from_rotation_translation(
                Quat::from_rotation_y(std::f32::consts::FRAC_PI_2),
                Vec3::new(100., 0., 50.),
            ),
            hand: Some(Mat4::from_translation(Vec3::new(0., 40., 0.))),
            tip: Vec3::new(0., 0., 30.),
            bones: None,
            body_lift: 70.,
            heal_lift: 90.,
            members: vec![MemberFrame {
                pos: Vec3::new(-200., 0., 0.),
                yaw: 0.,
                staff: None,
                body_lift: 70.,
                heal_lift: 90.,
            }],
            summon: None,
            actors: HashMap::new(),
            actor_bones: HashMap::new(),
        }
    }

    fn pos(m: &Mat) -> [f32; 3] {
        point_from_effect([m[3][0], m[3][1], m[3][2]])
    }

    fn close(a: [f32; 3], b: [f32; 3]) {
        assert!(
            a.iter().zip(b).all(|(x, y)| (x - y).abs() < 1e-3),
            "{a:?} vs {b:?}"
        );
    }

    #[test]
    fn an_unbound_element_sits_where_the_script_put_it() {
        let f = frames();
        let local = ([0., -10., 0.], [0.; 3], [1.; 3]);
        // No set_pos: the PAX translation (y down).
        let m = element_world(0, None, 0, local, None, None, None, &f);
        close(pos(&m), [0., 10., 0.]);
        let m = element_world(0, None, 0, local, Some([5., 6., 7.]), None, None, &f);
        close(pos(&m), [5., 6., 7.]);
        // An element with the Bind flag but no object is still unbound.
        let m = element_world(1, None, 0x4000, local, Some([1., 2., 3.]), None, None, &f);
        close(pos(&m), [1., 2., 3.]);
    }

    #[test]
    fn a_bound_element_follows_the_bone_and_bind_only_pos_keeps_its_own_rotation() {
        let f = frames();
        // Fire's ring: bone 0x4000, flags 5, 92.88 up (T.y = -92.88).
        let local = ([0., -92.88, 0.], [0.; 3], [1.; 3]);
        let m = element_world(5, Some(Anchor::Caster), 0x4000, local, None, None, None, &f);
        close(pos(&m), [100., 92.88, 50.]);
        // Rotation stays the element's own (identity) with BindOnlyPos ...
        close([m[0][0], m[0][1], m[0][2]], [1., 0., 0.]);
        // ... and follows Sora's yaw with a full bind (flag 1).
        let full = element_world(1, Some(Anchor::Caster), 0x4000, local, None, None, None, &f);
        close(pos(&full), [100., 92.88, 50.]);
        assert!((full[0][2].abs() - 1.).abs() < 1e-3);
        // Bone 0x4003 is the Keyblade's top: root · hand · tip.
        let tip = element_world(
            3,
            Some(Anchor::Caster),
            0x4003,
            ([0.; 3], [0.; 3], [1.; 3]),
            None,
            None,
            None,
            &f,
        );
        // tip (0,0,30) at hand height 40, then Sora turned a quarter.
        close(pos(&tip), [130., 40., 50.]);
        // A party member's frame.
        let m = element_world(
            1,
            Some(Anchor::Party(1)),
            0x4000,
            ([0.; 3], [0.; 3], [1.; 3]),
            None,
            None,
            None,
            &f,
        );
        close(pos(&m), [-200., 0., 0.]);
        // 0x4001 is the body centre (70 here), 0x4009 the heal point (90).
        let m = element_world(
            1,
            Some(Anchor::Party(1)),
            0x4009,
            ([0.; 3], [0.; 3], [1.; 3]),
            None,
            None,
            None,
            &f,
        );
        close(pos(&m), [-200., 90., 0.]);
    }

    #[test]
    fn an_actor_anchor_follows_that_actors_root() {
        let mut f = frames();
        f.actors.insert(
            7,
            MemberFrame {
                pos: Vec3::new(300., 0., -50.),
                yaw: 0.,
                staff: None,
                body_lift: 85.,
                heal_lift: 105.,
            },
        );
        let m = element_world(
            1,
            Some(Anchor::Actor(7)),
            0x4001,
            ([0.; 3], [0.; 3], [1.; 3]),
            None,
            None,
            None,
            &f,
        );
        close(pos(&m), [300., 85., -50.]);
        // Its bone 3, less the root translation its position already has.
        f.actor_bones.insert(
            7,
            super::super::enemy_combat::ActorBones {
                matrices: Arc::new(vec![
                    Mat4::IDENTITY,
                    Mat4::IDENTITY,
                    Mat4::IDENTITY,
                    Mat4::from_translation(Vec3::new(10., 120., 30.)),
                ]),
                removed_root: Vec3::new(0., 20., 0.),
            },
        );
        let m = element_world(
            1,
            Some(Anchor::Actor(7)),
            3,
            ([0.; 3], [0.; 3], [1.; 3]),
            None,
            None,
            None,
            &f,
        );
        close(pos(&m), [310., 100., -20.]);
        // An actor the room does not know takes Sora's root.
        let m = element_world(
            1,
            Some(Anchor::Actor(8)),
            0x4000,
            ([0.; 3], [0.; 3], [1.; 3]),
            None,
            None,
            None,
            &f,
        );
        close(pos(&m), pos(&to_effect(f.root)));
    }

    #[test]
    fn effect_set_dir_turns_the_effect_onto_the_direction() {
        // Along +Z: no turn about Y (the flip of z gives yaw 180°); up
        // tilts the pitch.
        let e = dir_euler([1., 0., 0.]);
        assert!((e[1] - std::f32::consts::FRAC_PI_2).abs() < 1e-5 && e[0].abs() < 1e-5);
        let up = dir_euler([0., 1., 0.]);
        assert!((up[0] - std::f32::consts::FRAC_PI_2).abs() < 1e-5);
        let m = ::effect::instance_matrix([0.; 3], dir_euler([0., 0., 1.]), [1.; 3]);
        // The effect's +Z axis lands on effect-space −Z (sim +Z).
        assert!(m[2][2] < -0.99);
    }
}
