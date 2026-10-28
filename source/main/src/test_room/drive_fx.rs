//! Sora's own object effects in the test room: the PAX of his object
//! archive (`obj/P_EX100.a.us`, and each form's) started by his motions'
//! frame trigger 1 (FUN_14040dd70 case 1 → FUN_1402c6b30), the revert's
//! elements 15..27 (FUN_140405630) and the form level-up's system element
//! 16 (FUN_1403ed390). Bound elements follow Sora's bone (or his object
//! for bones from 0x4000) every tick; an element's own sound plays at its
//! start.
//!
//! The drawing follows the event player's effect presenter
//! (`cutscene/fx.rs`): the PC's texture for each TEX0
//! ([`visual::effect_draw`]; the element's own textures shadow the
//! resident `effect/texcommon.dpd`), OMD model draws, blend and layer
//! batches.
use super::*;
use ::effect::{Instance, ModelDraw, Quad, Rng, math::Mat};
use data::object_effect::{Dpd, OmdHeader, OmdMesh, Pax};
use visual::effect_draw::{Blend, EffectMaterial, PageCache, TexKey, model_alpha, report_missing};

/// A start request: the PAX, the element id, and whether it binds to Sora.
pub struct FxRequest {
    pub pax: Arc<Pax>,
    pub id: u16,
    /// The kit (form) whose object owns the effect; 0xff for the system
    /// PAX (it stays when the object is swapped).
    pub owner: u8,
}

/// Where an element starts, when it is not Sora's own.
#[derive(Clone, Copy)]
pub enum Anchor {
    /// Sora's own object (his bones, for bound elements).
    Sora,
    /// A fixed place in scene coordinates, turned by the hit direction
    /// (FUN_1403d1b40: translation = contact, rotation = pitch, yaw, 0;
    /// the element's scale stays).
    World { at: Vec3, pitch: f32, yaw: f32 },
    /// The room enemy's object: a bound element follows it.
    Enemy,
    /// One element of the map's own container, by table index, placed by
    /// its own translation, rotation and scale in engine world space
    /// (FUN_1402c7010); it belongs to the element's background group.
    Map { index: usize },
    /// A battle enemy (by gallery actor id): a bound element follows it.
    Actor(u64),
    /// A weapon slot (0 or 1): a trail element (BonePos pair) follows its
    /// blade bone.
    Weapon(usize),
    /// A Wisdom bullet (by key): the element takes its position and
    /// direction every tick (`effect_set_pos` / `effect_set_dir`).
    Bullet(u64),
    /// The duel's other Sora's own object, by slot (as `Sora` for him).
    Rival(u8),
    /// A duel rival's weapon slot (his slot, the weapon slot): his trail.
    RivalWeapon(u8, usize),
}

/// A Wisdom bullet's place and direction, on the room Sora or a rival.
fn bullet_at(game: &Game, key: u64) -> Option<([f32; 3], [f32; 3])> {
    game.drive
        .iter()
        .chain(game.rivals.iter().filter_map(|r| r.own.drive.as_ref()))
        .find_map(|d| d.bullets.iter().find(|b| b.key == key))
        .map(|b| (b.pos, b.dir))
}

/// An instance's world matrix at `at` turned to `dir` (scene coordinates),
/// keeping the element's scale.
fn aimed(at: [f32; 3], dir: [f32; 3], scale: [f32; 3]) -> Mat {
    let d = to_world(dir);
    let pitch = (-d[1]).atan2((d[0] * d[0] + d[2] * d[2]).sqrt());
    let yaw = d[0].atan2(d[2]);
    ::effect::instance_matrix(to_world(at), [pitch, yaw, 0.], scale)
}

/// Slot `slot`'s blade tip in scene coordinates this frame (bone code
/// 0x4003 / 0x4004's point).
pub fn blade_tip(game: &Game, slot: usize) -> Option<Vec3> {
    let pose = sora_pose(game);
    let (_, tip) = pose.slots[slot]?;
    Some(pose.root.transform_point3(tip))
}

/// A start request that names its anchor (Sora's own use [`FxRequest`]).
pub struct Placed {
    pub req: FxRequest,
    pub anchor: Anchor,
}

/// How a composite look starts an element (the KH3 ruleset's looks,
/// `player::kh3::look`): its size, colour, place, bone, start and end. The
/// default leaves the element as its PAX has it.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Style {
    /// Times the element's own scale.
    pub scale: f32,
    /// Times every colour it draws (1.0 = as drawn; additive elements
    /// brighten above 1).
    pub tint: [f32; 4],
    /// Added to the element's own translation, in its bone's frame (scene
    /// axes: x right, y up, z forward).
    pub offset: [f32; 3],
    /// The bone code it starts on instead of its own; an element bound in
    /// its PAX follows that bone.
    pub bone: Option<i16>,
    /// Ticks before it starts, after its own start wait.
    pub delay: f32,
    /// Ticks after its start when it fades out (over a few event frames),
    /// if it is still running.
    pub life: Option<f32>,
    /// Whether its own sound plays.
    pub sound: bool,
}

impl Default for Style {
    fn default() -> Self {
        Style {
            scale: 1.,
            tint: [1.; 4],
            offset: [0.; 3],
            bone: None,
            delay: 0.,
            life: None,
            sound: true,
        }
    }
}

/// The fade of an element whose `Style::life` ran out (event frames).
const LIFE_FADE: f32 = 4.;

/// A draw's colour times a look's tint.
fn tinted(c: [f32; 4], tint: [f32; 4]) -> [f32; 4] {
    std::array::from_fn(|i| c[i] * tint[i])
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

struct Bind {
    bone: i16,
    local: Mat,
}

struct Running {
    instance: Instance,
    dpd: Arc<Dpd>,
    bind: Option<Bind>,
    /// The duel's other Sora whose object a bound element follows.
    rival: Option<u8>,
    owner: u8,
    /// The element, when it follows the room enemy's object.
    enemy_element: Option<object_effect::Element>,
    /// The battle actor an element follows, with the element.
    actor_element: Option<(u64, object_effect::Element)>,
    /// The weapon slot and BonePos pair a swing trail follows on that
    /// slot's blade bone.
    trail: Option<(usize, object_effect::BonePos)>,
    /// The background group a map element belongs to.
    group: Option<u8>,
    /// The element's sound, stopped with the element (map beds loop).
    sound_id: Option<u32>,
    /// The serial it holds in the hit-effect ring (0 = none).
    spark: u64,
    /// The Wisdom bullet it follows, with the element's scale.
    bullet: Option<(u64, [f32; 3])>,
    /// The element's sound (sound > 0) and the ticks until it plays: it
    /// starts with the element, after its start wait.
    sound: Option<u32>,
    sound_wait: f32,
    /// A look's tint ([1; 4] for the element as drawn).
    tint: [f32; 4],
    /// Ticks until a look's element fades.
    life: Option<f32>,
    /// A look's element: the tick it started (its end is logged).
    styled: Option<u64>,
}

/// Odd layers behind everything, even layers in front, 0 by depth
/// (FUN_1402c2720 / FUN_1402c27a0, as the event presenter ranks them).
fn layer_rank(layer: u8) -> i8 {
    match layer {
        0 => 0,
        v if v % 2 == 1 => -((v as i8 + 1) / 2),
        v => (v / 2) as i8,
    }
}

/// Scene matrix into the test room's game space (scene = (x, −y, −z)).
fn to_game(m: Mat4) -> Mat {
    let flip = Mat4::from_diagonal(Vec4::new(1., -1., -1., 1.));
    (flip * m * flip).to_cols_array_2d()
}

#[derive(Resource, Default)]
pub struct DriveFx {
    common: Option<Arc<Dpd>>,
    /// `obj/PRIZE.mdlx`'s sprites for the prizes on the ground.
    prize: Option<(Arc<Dpd>, ::effect::prize::Prize)>,
    running: Vec<Running>,
    rng: Rng,
    models: HashMap<(usize, i16), Option<ParsedModel>>,
    batches: HashMap<BatchKey, Batch>,
    dirty: bool,
    images: PageCache,
    materials: HashMap<BatchKey, Handle<EffectMaterial>>,
    missing: Vec<(usize, u16)>,
    spawned: Vec<Entity>,
    /// DPDs seen, for the texture lookup by key.
    dpds: Vec<Arc<Dpd>>,
    reported: Vec<u32>,
}

impl DriveFx {
    pub fn new(common: Option<Dpd>, prize: Option<Arc<Pax>>) -> Self {
        let prize = prize.and_then(|pax| {
            let dpd = pax.dpds.first()?.clone();
            let sprites = ::effect::prize::Prize::new(&dpd)?;
            Some((dpd, sprites))
        });
        DriveFx {
            common: common.map(Arc::new),
            dpds: prize.iter().map(|(d, _)| d.clone()).collect(),
            prize,
            ..Default::default()
        }
    }
}

/// Sora this frame: his scene-space object matrix, the bone matrices, and
/// each weapon slot's grip matrix and blade tip (model space).
struct SoraPose {
    root: Mat4,
    bones: Option<HashMap<usize, Mat4>>,
    slots: [Option<(Mat4, Vec3)>; 2],
    /// Bone codes 0x4001 / 0x4009 (FUN_1403b50f0): the centres of Sora's
    /// first type-1 and type-9 collision entries, above his origin.
    body_lift: f32,
    heal_lift: f32,
}

/// Each rival's pose (his state swapped in for a moment).
fn rival_poses(game: &mut Game) -> Vec<(u8, SoraPose)> {
    let mut out = Vec::new();
    let mut rivals = std::mem::take(&mut game.rivals);
    for r in &mut rivals {
        super::rival::swap(game, r);
        out.push((r.slot, sora_pose(game)));
        super::rival::swap(game, r);
    }
    game.rivals = rivals;
    out
}

fn sora_pose(game: &Game) -> SoraPose {
    let p = game.sora.player.position;
    let root = Mat4::from_rotation_translation(
        Quat::from_rotation_y(game.sora.player.yaw),
        Vec3::from_array(p),
    );
    let clip = game
        .poses
        .get(&(game.sora.player.action, game.sora.player.armed));
    let frame = game.sora.player.motion_frame(&game.clips) as usize;
    let cp = clip.and_then(|&c| game.clip_poses.get(c));
    let bones = cp
        .and_then(|c| c.body.get(frame.min(c.body.len().saturating_sub(1))))
        .cloned();
    // FUN_1403b5450's slot matrix and the blade length along it: the hand
    // joint holding the weapon, and the weapon vertex farthest from it [I].
    let slot = |grips: &[Mat4], verts: &[Vec<Vec3>]| -> Option<(Mat4, Vec3)> {
        let grip = *grips.get(frame.min(grips.len().checked_sub(1)?))?;
        let v = verts.get(frame.min(verts.len().checked_sub(1)?))?;
        let tip = v
            .iter()
            .copied()
            .max_by(|a, b| a.length_squared().total_cmp(&b.length_squared()))?;
        Some((grip, grip.transform_point3(tip)))
    };
    let slots = cp.map_or([None, None], |c| {
        [
            slot(&c.weapon, &c.weapon_frames),
            slot(&c.weapon2, &c.weapon2_frames),
        ]
    });
    let (body_lift, heal_lift) = game
        .magic
        .as_ref()
        .map_or((85., 105.), |m| (m.src.body_lift, m.src.heal_lift));
    SoraPose {
        root,
        bones,
        slots,
        body_lift,
        heal_lift,
    }
}

/// FUN_1402c49d0 for a bound element: the bone's matrix (the bone read
/// through FUN_1401a7af0 negates its y and z columns) or the object's.
/// FUN_1403b50f0's codes: 0x4003 / 0x4004 a weapon slot's blade tip,
/// 0x4005 / 0x4006 its grip, 0x4007 / 0x4008 the blade's middle (slot 0 /
/// 1); 0x4001 / 0x4009 the body centre and heal point above the
/// object (the collision entries' heights; [I] their bones are not posed);
/// 0x4000 and 0x4002 stay on the object.
/// A weapon code's matrix: the grip's axes at the tip (0x4003 / 0x4004),
/// the grip (0x4005 / 0x4006) or halfway along the blade (0x4007 / 0x4008).
fn slot_matrix(grip: Mat4, tip: Vec3, code: i16) -> Mat4 {
    let along = match code {
        0x4003 | 0x4004 => 1.,
        0x4007 | 0x4008 => 0.5,
        _ => 0.,
    };
    let mut m = grip;
    m.w_axis = grip.w_axis.truncate().lerp(tip, along).extend(1.);
    m
}

fn bound_world(pose: &SoraPose, bind: &Bind) -> Mat {
    let flip = Mat4::from_diagonal(Vec4::new(1., -1., -1., 1.));
    let root = pose.root;
    let m = match bind.bone {
        0x4003..=0x4008 => match pose.slots[(bind.bone - 0x4003) as usize % 2] {
            Some((grip, tip)) => root * slot_matrix(grip, tip, bind.bone) * flip,
            None => root,
        },
        0x4001 => root * Mat4::from_translation(Vec3::new(0., pose.body_lift, 0.)),
        0x4009 => root * Mat4::from_translation(Vec3::new(0., pose.heal_lift, 0.)),
        b if !(0..0x4000).contains(&b) => root,
        b => match pose.bones.as_ref().and_then(|m| m.get(&(b as usize))) {
            Some(m) => root * *m * flip,
            None => root,
        },
    };
    ::effect::math::mul(&to_game(m), &bind.local)
}

/// One tick: start the queued elements, follow Sora, step every instance
/// and batch what it drew.
pub fn tick(
    mut game: ResMut<Game>,
    rig: Res<CameraRig>,
    mut fx: ResMut<DriveFx>,
    mut sparks: ResMut<super::SparkRing>,
) {
    let mut requests: Vec<(Placed, Style)> = std::mem::take(&mut game.fx_queue)
        .into_iter()
        .map(|p| (p, Style::default()))
        .collect();
    requests.append(&mut game.fx_styled);
    let mut swapped = None;
    if let Some(d) = game.drive.as_mut() {
        requests.extend(std::mem::take(&mut d.fx_requests).into_iter().map(|req| {
            (
                Placed {
                    req,
                    anchor: Anchor::Sora,
                },
                Style::default(),
            )
        }));
        swapped = d.fx_owner_left.take();
    }
    let enemy_at = game
        .enemy
        .as_ref()
        .map(|rt| (rt.enemy.position, rt.enemy.yaw));
    // Effects an owner ends (the weapon trail's window closing: FUN_1402be8e0
    // sets the instance's end flag, a looping effect then drains).
    for owner in std::mem::take(&mut game.fx_release) {
        for r in fx.running.iter_mut().filter(|r| r.owner == owner) {
            r.instance.release();
            // The save point's loop (se000 #29) ends with its element.
            if let Some(id) = r.sound_id.take() {
                game.sound_stops.push(id);
            }
        }
    }
    // A Wisdom bullet's element: killed with a hit, faded over 8 frames when
    // its life runs out (`effect_fadeout(fx, 8.0, 0)`).
    let mut ends = game
        .drive
        .as_mut()
        .map(|d| std::mem::take(&mut d.bullet_ends))
        .unwrap_or_default();
    for r in &mut game.rivals {
        if let Some(d) = r.own.drive.as_mut() {
            ends.append(&mut d.bullet_ends);
        }
    }
    for (key, faded) in ends {
        if faded {
            for r in fx
                .running
                .iter_mut()
                .filter(|r| r.bullet.is_some_and(|b| b.0 == key))
            {
                r.instance.fade_out(8.);
                r.bullet = None;
            }
        } else {
            fx.running.retain(|r| r.bullet.is_none_or(|b| b.0 != key));
        }
    }
    // Effects displaced from the hit-effect ring fade out over four frames.
    for serial in std::mem::take(&mut sparks.fade_fx) {
        for r in fx.running.iter_mut().filter(|r| r.spark == serial) {
            r.instance.fade_out(super::SPARK_FADE);
            let tick = game.tick;
            game.event(serde_json::json!({"event":"spark_fade","tick":tick,"kind":"enemy_hit","serial":serial}));
        }
    }
    // Map elements leave with their background group (FUN_1402c65a0).
    for group in std::mem::take(&mut game.fx_clear_map) {
        for r in fx
            .running
            .iter()
            .filter(|r| r.group.is_some() && group.is_none_or(|g| r.group == Some(g)))
        {
            if let Some(id) = r.sound_id {
                game.sound_stops.push(id);
            }
        }
        fx.running
            .retain(|r| !(r.group.is_some() && group.is_none_or(|g| r.group == Some(g))));
    }
    let pose = sora_pose(&game);
    let rival_poses = rival_poses(&mut game);
    let mut sounds = Vec::new();
    let mut log = Vec::new();
    let tick = game.tick;
    // Waiting effect sounds count down one tick at a time, as the instance's
    // start wait does, and play when their element starts.
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
    // An object's effects fade with it when it is replaced (the swap
    // re-points only elements with flag 0x200, FUN_1402c4480).
    if let Some(old) = swapped {
        for r in fx.running.iter_mut().filter(|r| r.owner == old) {
            r.instance.fade_out(15.);
        }
    }
    for (Placed { req, anchor }, style) in requests {
        let element = match anchor {
            Anchor::Map { index } => req.pax.elements.get(index),
            _ => req.pax.by_id(req.id),
        };
        let Some(element) = element else {
            continue;
        };
        let Some(dpx) = req.pax.effect(element.effect_number) else {
            continue;
        };
        // A look's offset is in scene axes; the element's frame is the
        // game's (y and z flipped).
        let t = element.translation;
        let o = style.offset;
        let local = ::effect::instance_matrix(
            [t[0] + o[0], t[1] - o[1], t[2] - o[2]],
            element.rotation,
            element.scale.map(|s| s * style.scale),
        );
        let bone = style.bone.unwrap_or(element.bone);
        let mut enemy_element = None;
        let mut actor_element = None;
        let mut bullet = None;
        let mut rival = None;
        let (bind, world) = match anchor {
            Anchor::Sora | Anchor::Rival(_) => {
                let pose = match anchor {
                    Anchor::Rival(slot) => {
                        rival = Some(slot);
                        match rival_poses.iter().find(|p| p.0 == slot) {
                            Some((_, p)) => p,
                            None => continue,
                        }
                    }
                    _ => &pose,
                };
                let bind = (element.flags & 1 != 0).then_some(Bind { bone, local });
                let world = match &bind {
                    Some(b) => bound_world(pose, b),
                    // A look's bone places an unbound element where it
                    // starts.
                    None if style.bone.is_some() => bound_world(pose, &Bind { bone, local }),
                    None => ::effect::math::mul(&to_game(pose.root), &local),
                };
                (bind, world)
            }
            Anchor::Map { .. } => (None, local),
            Anchor::Weapon(_) => (None, ::effect::math::IDENTITY),
            Anchor::RivalWeapon(slot, _) => {
                rival = Some(slot);
                (None, ::effect::math::IDENTITY)
            }
            Anchor::Actor(id) => {
                let Some(&(position, yaw)) = game.actor_poses.get(&id) else {
                    continue;
                };
                let position = position.to_array();
                if element.flags & 1 != 0 {
                    actor_element = Some((id, element.clone()));
                    (None, super::bound_world(position, yaw, element))
                } else {
                    (
                        None,
                        ::effect::math::mul(
                            &::effect::instance_matrix(to_world(position), [0., -yaw, 0.], [1.; 3]),
                            &local,
                        ),
                    )
                }
            }
            Anchor::World { at, pitch, yaw } => (
                None,
                ::effect::instance_matrix(
                    to_world(at.to_array()),
                    [pitch, yaw, 0.],
                    element.scale.map(|s| s * style.scale),
                ),
            ),
            Anchor::Bullet(key) => {
                let Some((pos, dir)) = bullet_at(&game, key) else {
                    continue;
                };
                let scale = element.scale.map(|s| s * style.scale);
                bullet = Some((key, scale));
                (None, aimed(pos, dir, scale))
            }
            Anchor::Enemy => {
                let Some((position, yaw)) = enemy_at else {
                    continue;
                };
                if element.flags & 1 != 0 {
                    enemy_element = Some(element.clone());
                    (None, super::bound_world(position, yaw, element))
                } else {
                    (
                        None,
                        ::effect::math::mul(
                            &::effect::instance_matrix(to_world(position), [0., -yaw, 0.], [1.; 3]),
                            &local,
                        ),
                    )
                }
            }
        };
        // A bound element waits one extra tick (DAT_1406239f8, 0.1).
        let wait = element.start_wait
            + style.delay
            + if bind.is_some() || enemy_element.is_some() {
                0.1
            } else {
                0.
            };
        let dpd = req.pax.dpds[dpx.dpd].clone();
        // A sound plays only for sound > 0 (the element's SE id), at the
        // element's start: now when it has no wait.
        let sound = (element.sound > 0 && style.sound).then_some(element.sound as u32);
        if let Some(id) = sound
            && wait <= 0.
        {
            sounds.push(id);
        }
        log.push(serde_json::json!({"event":"drive_effect","tick":tick,"element":req.id,
            "owner":req.owner,"bone":element.bone,"flags":element.flags,"dpd":dpx.dpd,"pdata":dpx.pdata}));
        if let Some(instance) = Instance::new(dpd.clone(), dpx.pdata, world, wait) {
            if !fx.dpds.iter().any(|x| Arc::ptr_eq(x, &dpd)) {
                fx.dpds.push(dpd.clone());
            }
            // An attacker's hit spark is one of the eight most recent
            // (FUN_1403d9be0).
            let spark = if req.owner == super::ENEMY_HIT_OWNER {
                sparks.register()
            } else {
                0
            };
            fx.running.push(Running {
                instance,
                dpd,
                bind,
                rival,
                spark,
                owner: req.owner,
                enemy_element,
                actor_element,
                bullet,
                trail: match anchor {
                    Anchor::Weapon(slot) | Anchor::RivalWeapon(_, slot)
                        if element.flags & 0x10000 != 0 =>
                    {
                        Some((slot, element.bone_pos))
                    }
                    _ => None,
                },
                group: matches!(anchor, Anchor::Map { .. }).then_some(element.group),
                sound_id: sound.filter(|_| {
                    matches!(anchor, Anchor::Map { .. })
                        || req.owner == super::reaction_command::SAVE_PULSE
                }),
                sound: sound.filter(|_| wait > 0.),
                sound_wait: wait,
                tint: style.tint,
                life: style.life.map(|l| l + wait),
                styled: style.life.map(|_| tick),
            });
        }
    }
    for s in sounds {
        game.sound_queue.push((s, "drive_effect_se"));
    }
    for v in log {
        game.event(v);
    }
    let view = game_view(rig.camera.eye, rig.camera.look_at);
    let view_inverse = Mat4::from_cols_array_2d(&view).inverse();
    let pose = sora_pose(&game);
    let fx = &mut *fx;
    fx.batches.clear();
    fx.dirty = true;
    let mut quads: Vec<Drawn<Quad>> = Vec::new();
    let mut draws: Vec<Drawn<ModelDraw>> = Vec::new();
    let mut triangles: Vec<(i8, Vec<::effect::Triangle>)> = Vec::new();
    let mut unsupported = Vec::new();
    // The rivals' trails' points (each rival's state swapped in).
    let mut rival_trails = Vec::new();
    let mut rivals = std::mem::take(&mut game.rivals);
    for rv in &mut rivals {
        super::rival::swap(&mut game, rv);
        for (i, r) in fx.running.iter().enumerate() {
            if r.rival == Some(rv.slot)
                && let Some((slot, bone)) = &r.trail
            {
                rival_trails.push((i, super::weapon_trail::points(&game, bone, *slot)));
            }
        }
        super::rival::swap(&mut game, rv);
    }
    game.rivals = rivals;
    for (i, r) in fx.running.iter_mut().enumerate() {
        if let Some(b) = &r.bind {
            let own = match r.rival {
                Some(slot) => rival_poses.iter().find(|p| p.0 == slot).map(|p| &p.1),
                None => Some(&pose),
            };
            if let Some(own) = own {
                r.instance.world = bound_world(own, b);
            }
        }
        if let Some((id, element)) = &r.actor_element
            && let Some(&(position, yaw)) = game.actor_poses.get(id)
        {
            r.instance.world = super::bound_world(position.to_array(), yaw, element);
        }
        if let (Some(element), Some((position, yaw))) = (&r.enemy_element, enemy_at) {
            r.instance.world = super::bound_world(position, yaw, element);
        }
        if let Some((slot, bone)) = &r.trail {
            r.instance.trail = match r.rival {
                None => super::weapon_trail::points(&game, bone, *slot),
                Some(_) => rival_trails.iter().find(|t| t.0 == i).and_then(|t| t.1),
            };
        }
        if let Some((key, scale)) = r.bullet
            && let Some((pos, dir)) = bullet_at(&game, key)
        {
            r.instance.world = aimed(pos, dir, scale);
        }
        if let Some(left) = r.life.as_mut() {
            *left -= 1.;
            if *left <= 0. {
                r.life = None;
                r.instance.fade_out(LIFE_FADE);
            }
        }
        let mut q = r.instance.frame(&view, &mut fx.rng);
        if r.tint != [1.; 4] {
            let tint = r.tint;
            for quad in &mut q {
                quad.colour = tinted(quad.colour, tint);
                if let Some(c) = quad.corner_colours.as_mut() {
                    *c = c.map(|c| tinted(c, tint));
                }
            }
            let byte = |c: [u8; 4]| -> [u8; 4] {
                std::array::from_fn(|i| (f32::from(c[i]) * tint[i]).clamp(0., 255.) as u8)
            };
            for m in &mut r.instance.model_draws {
                m.rgba = byte(m.rgba);
            }
            for t in &mut r.instance.triangles {
                t.colours = t.colours.map(byte);
            }
        }
        for &k in &r.instance.unsupported {
            if !fx.reported.contains(&k) {
                fx.reported.push(k);
                unsupported.push(k);
            }
        }
        // A program this translation lacks: stepped, not shown.
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
        let t = std::mem::take(&mut r.instance.triangles);
        if complete && !t.is_empty() {
            triangles.push((layer, t));
        }
    }
    let ended: Vec<(u16, u64)> = fx
        .running
        .iter()
        .filter(|r| r.instance.finished())
        .filter_map(|r| r.styled.map(|t| (r.owner.into(), tick - t)))
        .collect();
    for (owner, age) in ended {
        game.event(
            serde_json::json!({"event":"look_layer_end","tick":tick,"owner":owner,"age":age}),
        );
    }
    fx.running.retain(|r| !r.instance.finished());
    for (slot, owner) in super::weapon_trail::OWNERS.into_iter().enumerate() {
        game.trail_live[slot] = fx
            .running
            .iter()
            .any(|r| r.owner == owner && !r.instance.is_released());
    }
    for rv in &mut game.rivals {
        for slot in 0..2 {
            let owner = super::weapon_trail::owner(Some(rv.slot), slot);
            rv.own.trail_live[slot] = fx
                .running
                .iter()
                .any(|r| r.owner == owner && !r.instance.is_released());
        }
    }
    // The prizes on the ground (obj/PRIZE.mdlx, program 203): billboards at
    // each prize's centre.
    if let (Some((dpd, prize)), Some(rt)) = (&fx.prize, game.enemy.as_ref()) {
        let mut list = Vec::new();
        for orb in &rt.orbs {
            let Some(frame) = prize.frame(orb.kind, orb.age, orb.seed) else {
                continue;
            };
            let centre = to_world([
                orb.position[0],
                orb.position[1] + player::enemy_prizes::centre_height(orb.kind),
                orb.position[2],
            ]);
            let v = ::effect::math::apply(&view, [centre[0], centre[1], centre[2], 1.]);
            list.extend(prize.quads(dpd, orb.kind, frame, [v[0], v[1], v[2]], 0x80));
        }
        if !list.is_empty() {
            quads.push((dpd.clone(), [0xff; 16], 0, list));
        }
    }
    // A limit's shot pool (Trinity Limit: FUN_140541a00 on object 0x5c3,
    // PRIZE): FUN_140411b90 hands the records' points to the same program
    // 203 draw (FUN_1404d7ed0), which skips a point of alpha 0 and steps its
    // flip-book frame.
    if let (Some((dpd, prize)), Some(run)) = (&fx.prize, game.party_limit.as_mut()) {
        let mut list = Vec::new();
        for p in run.shot_points() {
            if p.alpha == 0 {
                continue;
            }
            let Some((frame, next)) = prize.point_frame(p.group, p.frame, fx.rng.unit()) else {
                continue;
            };
            run.set_shot_frame(p.base, next);
            let centre = to_world(p.position);
            let v = ::effect::math::apply(&view, [centre[0], centre[1], centre[2], 1.]);
            list.extend(prize.quads(dpd, p.group, frame, [v[0], v[1], v[2]], p.alpha));
        }
        if !list.is_empty() {
            quads.push((dpd.clone(), [0xff; 16], 0, list));
        }
    }
    for k in unsupported {
        game.event(serde_json::json!({"event":"drive_effect_unsupported","tick":tick,"program":k}));
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
                    "drive sprite",
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
    // KeTh lightning: untextured gouraud triangles (GS colour bytes, 0x80 =
    // 1.0).
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
    fx: &mut DriveFx,
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
                    "drive model",
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

fn image(fx: &mut DriveFx, key: TexKey, repeat: bool, images: &mut Assets<Image>) -> Handle<Image> {
    let (dpds, common) = (&fx.dpds, fx.common.as_ref());
    fx.images.image(key, repeat, images, || {
        dpds.iter()
            .chain(common)
            .find(|d| Arc::as_ptr(d) as usize == key.dpd)
            .map(|d| &**d)
    })
}

/// Rebuilds the effect meshes from this tick's batches.
pub fn present(
    mut commands: Commands,
    fx: Option<ResMut<DriveFx>>,
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

    #[test]
    fn weapon_codes_sit_on_the_tip_the_grip_and_the_middle() {
        let grip =
            Mat4::from_rotation_translation(Quat::from_rotation_z(0.5), Vec3::new(1., 2., 3.));
        let tip = Vec3::new(1., 12., 3.);
        let at = |code| slot_matrix(grip, tip, code).w_axis.truncate();
        assert_eq!(at(0x4003), tip);
        assert_eq!(at(0x4006), Vec3::new(1., 2., 3.));
        assert_eq!(at(0x4008), Vec3::new(1., 7., 3.));
        // The grip's axes are kept.
        assert_eq!(slot_matrix(grip, tip, 0x4003).x_axis, grip.x_axis);
    }
}
