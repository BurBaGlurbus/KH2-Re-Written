//! Source-slot inspection actors and explicitly enabled source-bound combat:
//! loading a motion does not qualify its installed action or event semantics.
//! Only live kinds and one decoded motion per actor are retained; poses are
//! evaluated on demand rather than expanding entire motion sets into frames.
use super::{
    AreaRes, SceneRoot,
    enemy_catalog::{Catalog, Kind},
    spawn_model,
};
use bevy::prelude::*;
use data::{model_pose::ModelPose, motion, probe_source::ProbeSource};
use std::sync::Arc;
use visual::area_fog::{AddFogged, FogMaterial};

pub const MAX_ACTORS: usize = 16;
/// The playback speeds the preview steps through.
pub const SPEEDS: [f32; 9] = [0.1, 0.25, 0.5, 0.75, 1., 1.5, 2., 3., 4.];
const MAX_PENDING: usize = 32;

#[derive(Component)]
pub struct GalleryRoot(pub u64);

#[derive(Component)]
pub struct GalleryPart {
    actor: u64,
    ordinal: usize,
    revision: u64,
}

/// The bone lines drawn over (or instead of) an actor's model.
#[derive(Component)]
pub struct GallerySkeleton {
    actor: u64,
    /// The pose revision the lines show; `u64::MAX` until first drawn.
    revision: u64,
}

/// What the preview draws besides the model.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct PreviewOverlays {
    /// Bone lines, joint to parent joint.
    pub skeleton: bool,
    /// The model's meshes are hidden, so the skeleton shows alone.
    pub hide_model: bool,
}

pub struct GalleryActor {
    pub battle: Option<super::enemy_battle::BattleActor>,
    pub id: u64,
    pub entry: usize,
    pub kind: Arc<Kind>,
    pub position: Vec3,
    pub yaw: f32,
    pub motion_choice: Option<usize>,
    /// Source key time, not an inferred simulation/action frame.
    pub frame: f32,
    /// Where each bone's joint sits in model space (root motion removed
    /// when `apply_root` is off), for the skeleton overlay.
    pub joints: Vec<Vec3>,
    pub paused: bool,
    pub looping: bool,
    pub rate: f32,
    pub apply_root: bool,
    pub error: Option<String>,
    root: Entity,
    selected_motion: Option<Arc<motion::SelectedMotion>>,
    timeline: Option<Timeline>,
    pose: Vec<Vec3>,
    dirty: bool,
    revision: u64,
    /// Optional source-resolved idle after a one-shot appearance preview.
    preview_idle: Option<usize>,
}

/// A motion's playable range, in source key frames.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Timeline {
    /// First playable key frame (can be negative).
    pub start: f32,
    /// Last playable key frame.
    pub end: f32,
    /// Key frames per second.
    pub fps: f32,
    /// Where a looping motion jumps back to after `end`.
    pub loop_start: f32,
}

impl GalleryActor {
    /// The loaded motion's playable range, if a motion is loaded.
    pub fn timeline(&self) -> Option<Timeline> {
        self.timeline
    }

    pub(super) fn battle_pose(
        &mut self,
        choice: usize,
        key: f32,
        forward_kinematics_only: bool,
    ) -> Result<(), String> {
        let motion = self.battle.as_ref().ok_or("battle absent")?.motion.clone();
        let sampled = sample(&self.kind, &motion, key, false, forward_kinematics_only)?;
        self.pose = sampled.points;
        self.joints = sampled.joints;
        self.motion_choice = Some(choice);
        self.selected_motion = Some(motion);
        self.frame = key;
        self.revision += 1;
        self.dirty = false;
        Ok(())
    }
}

impl Timeline {
    fn read(motion: &motion::SelectedMotion) -> Result<Self, String> {
        let (start, end, fps, loop_start) =
            motion.motion.frame_window().map_err(|e| e.to_string())?;
        if [start, end, fps, loop_start].iter().any(|n| !n.is_finite())
            || start.abs() > 16_777_216.
            || end < start
            || end.abs() > 16_777_216.
            || fps <= 0.
            || fps > 240.
            || loop_start.abs() > 16_777_216.
            || loop_start > end
        {
            return Err("motion frame window outside inspection profile".into());
        }
        Ok(Self {
            start,
            end,
            fps,
            // A return time before the playable start repeats at the start;
            // this is an explicit inspection playback choice.
            loop_start: loop_start.clamp(start, end),
        })
    }

    fn advance(self, frame: f32, ticks: f32, rate: f32, looping: bool) -> f32 {
        let next = frame + ticks * self.fps / 60. * rate;
        if next <= self.end {
            next
        } else if looping && self.loop_start < self.end {
            self.loop_start + (next - self.end).rem_euclid(self.end - self.loop_start)
        } else {
            self.end
        }
    }

    /// Key frames in the playable range.
    pub fn length(self) -> f32 {
        self.end - self.start
    }

    /// Seconds the playable range lasts at the source rate.
    pub fn seconds(self) -> f32 {
        self.length() / self.fps
    }

    /// The key frame `fraction` (0 to 1) of the way through the range.
    fn at_fraction(self, fraction: f32) -> f32 {
        self.start + self.length() * fraction.clamp(0., 1.)
    }

    /// The whole key frame one step from `frame`: forward takes the next
    /// frame boundary, back the previous one. A looping motion wraps at
    /// its ends; otherwise the step stops there.
    fn step_frame(self, frame: f32, forward: bool, looping: bool) -> f32 {
        let next = if forward {
            frame.floor() + 1.
        } else {
            frame.ceil() - 1.
        };
        if (self.start..=self.end).contains(&next) {
            next
        } else if !looping {
            next.clamp(self.start, self.end)
        } else if forward {
            self.start
        } else {
            self.end
        }
    }
}

enum Request {
    Battle {
        actor: u64,
    },
    Summon {
        entry: usize,
        position: Vec3,
        yaw: f32,
    },
    Motion {
        actor: u64,
        choice: usize,
    },
}

#[derive(Resource)]
pub struct EnemyGallery {
    pub target_params: Option<player::targeting::Params>,
    pub lock_range: Option<f32>,
    pub source: Arc<ProbeSource>,
    pub catalog: Catalog,
    pub actors: Vec<GalleryActor>,
    pub selected: Option<u64>,
    pub status: String,
    /// Skeleton and model visibility for every actor.
    pub overlays: PreviewOverlays,
    /// Only the plain bone chain poses the model (no IK or constraints).
    pub(super) forward_kinematics_only: bool,
    next_id: u64,
    pending: Vec<Request>,
    garbage: Vec<(u64, Entity)>,
    location: Option<(Entity, Option<(u8, u8)>)>,
}

impl EnemyGallery {
    pub fn new(source: Arc<ProbeSource>, catalog: Catalog, forward_kinematics_only: bool) -> Self {
        Self {
            target_params: None,
            lock_range: None,
            source,
            catalog,
            actors: Vec::new(),
            selected: None,
            status: "Select a source enemy and summon an inspection actor".into(),
            overlays: PreviewOverlays::default(),
            forward_kinematics_only,
            next_id: 1,
            pending: Vec::new(),
            garbage: Vec::new(),
            location: None,
        }
    }

    pub fn selected_actor(&self) -> Option<&GalleryActor> {
        self.actors.iter().find(|a| Some(a.id) == self.selected)
    }

    fn selected_actor_mut(&mut self) -> Option<&mut GalleryActor> {
        self.actors.iter_mut().find(|a| Some(a.id) == self.selected)
    }

    pub fn select(&mut self, id: u64) {
        if self.actors.iter().any(|a| a.id == id) {
            self.selected = Some(id);
        }
    }

    pub fn queue_summon(&mut self, entry: usize, position: Vec3, yaw: f32) {
        if self.pending.len() >= MAX_PENDING {
            self.status = "Inspection request queue is full".into();
        } else if position.is_finite() && yaw.is_finite() {
            self.pending.push(Request::Summon {
                entry,
                position,
                yaw,
            });
        } else {
            self.status = "Inspection placement must be finite".into();
        }
    }

    pub fn queue_motion(&mut self, choice: usize) {
        if let Some(actor) = self.selected
            && self.pending.len() < MAX_PENDING
        {
            self.pending.push(Request::Motion { actor, choice });
        }
    }

    pub fn queue_battle(&mut self) {
        if let Some(actor) = self.selected
            && self.pending.len() < MAX_PENDING
        {
            self.pending.push(Request::Battle { actor });
        }
    }

    pub fn defeat_selected(&mut self) {
        if let Some(a) = self.selected_actor_mut()
            && let Some(b) = a.battle.as_mut()
        {
            b.brain.defeat();
        }
    }
    /// Every summoned actor stands again (a game-over Continue).
    pub fn respawn_all(&mut self) {
        for a in &mut self.actors {
            if let Some(b) = a.battle.as_mut() {
                b.brain.respawn();
                b.contacts = super::enemy_battle_hits::ContactState::default();
                b.error = None;
            }
            a.error = None;
        }
    }

    pub fn respawn_selected(&mut self) {
        if let Some(a) = self.selected_actor_mut()
            && let Some(b) = a.battle.as_mut()
        {
            b.brain.respawn();
            b.contacts = super::enemy_battle_hits::ContactState::default();
            b.error = None;
            a.error = None;
        }
    }

    pub fn toggle_paused(&mut self) {
        if let Some(actor) = self.selected_actor_mut() {
            actor.paused = !actor.paused;
        }
    }

    /// Plays the motion again from its first frame.
    pub fn restart(&mut self) {
        if let Some(actor) = self.selected_actor_mut()
            && let Some(timeline) = actor.timeline
        {
            actor.frame = timeline.start;
            actor.paused = false;
            actor.dirty = true;
        }
    }

    /// Moves one whole key frame, forward or back, and pauses there.
    pub fn step_frame(&mut self, forward: bool) {
        if let Some(actor) = self.selected_actor_mut()
            && let Some(timeline) = actor.timeline
        {
            actor.paused = true;
            actor.frame = timeline.step_frame(actor.frame, forward, actor.looping);
            actor.dirty = true;
        }
    }

    /// Pauses on `frame`, held inside the motion's range.
    pub fn go_to_frame(&mut self, frame: f32) {
        if frame.is_finite()
            && let Some(actor) = self.selected_actor_mut()
            && let Some(timeline) = actor.timeline
        {
            actor.paused = true;
            actor.frame = frame.clamp(timeline.start, timeline.end);
            actor.dirty = true;
        }
    }

    /// Pauses `fraction` (0 to 1) of the way through the motion.
    pub fn go_to_fraction(&mut self, fraction: f32) {
        if let Some(timeline) = self.selected_actor().and_then(|a| a.timeline) {
            self.go_to_frame(timeline.at_fraction(fraction));
        }
    }

    /// Moves the paused frame by `frames` (negative goes back).
    pub fn scrub(&mut self, frames: f32) {
        if let Some(actor) = self.selected_actor() {
            let target = actor.frame + frames;
            self.go_to_frame(target);
        }
    }

    pub fn toggle_loop(&mut self) {
        if let Some(actor) = self.selected_actor_mut() {
            actor.looping = !actor.looping;
        }
    }

    pub fn set_rate(&mut self, rate: f32) {
        if rate.is_finite()
            && let Some(actor) = self.selected_actor_mut()
        {
            actor.rate = rate.clamp(0.1, 4.);
        }
    }

    /// Moves the playback speed to the next preset above (`direction` > 0)
    /// or below the current one.
    pub fn change_speed(&mut self, direction: i32) {
        let Some(rate) = self.selected_actor().map(|a| a.rate) else {
            return;
        };
        let next = if direction > 0 {
            SPEEDS.iter().copied().find(|&s| s > rate + 1e-3)
        } else {
            SPEEDS.iter().rev().copied().find(|&s| s < rate - 1e-3)
        };
        if let Some(next) = next {
            self.set_rate(next);
        }
    }

    pub fn toggle_root(&mut self) {
        if let Some(actor) = self.selected_actor_mut() {
            actor.apply_root = !actor.apply_root;
            actor.dirty = true;
        }
    }

    pub fn remove_selected(&mut self) {
        let Some(index) = self.actors.iter().position(|a| Some(a.id) == self.selected) else {
            return;
        };
        let actor = self.actors.remove(index);
        self.pending.retain(
            |request| !matches!(request, Request::Motion { actor: id, .. } if *id == actor.id),
        );
        self.garbage.push((actor.id, actor.root));
        self.selected = self.actors.last().map(|a| a.id);
        self.status = "Inspection actor removed".into();
    }

    pub fn clear(&mut self) {
        self.pending.clear();
        self.garbage
            .extend(self.actors.drain(..).map(|a| (a.id, a.root)));
        self.selected = None;
        self.status = "Inspection actors cleared".into();
    }

    fn observe_location(&mut self, location: (Entity, Option<(u8, u8)>)) {
        if self.location.is_some_and(|previous| previous != location) {
            self.clear();
            self.status = "Inspection actors cleared on room change".into();
        }
        self.location = Some(location);
    }
}

/// Load requests run on the app thread through the validated source reader.
/// Each spawn gets independent meshes; live actors of a kind share CPU data.
#[allow(clippy::too_many_arguments)]
pub fn service(
    mut commands: Commands,
    mut gallery: ResMut<EnemyGallery>,
    world: Option<Res<SceneRoot>>,
    area: Option<Res<AreaRes>>,
    parts: Query<(&GalleryPart, &Mesh3d, &MeshMaterial3d<FogMaterial>)>,
    skeletons: Query<(&GallerySkeleton, &Mesh3d, &MeshMaterial3d<FogMaterial>)>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<FogMaterial>>,
    mut images: ResMut<Assets<Image>>,
    mut game: Option<ResMut<super::Game>>,
) {
    if let Some(world) = world.as_ref() {
        gallery.observe_location((
            world.0,
            area.as_ref().map(|a| (a.current.world, a.current.area)),
        ));
    }
    // Removal is serviced after the preceding frame's spawn commands have
    // materialized, so clearing immediately after summoning also releases GPU
    // resources instead of accumulating hidden meshes/textures.
    for (id, root) in gallery.garbage.drain(..) {
        let owned = parts
            .iter()
            .map(|(part, mesh, material)| (part.actor, mesh, material))
            .chain(
                skeletons
                    .iter()
                    .map(|(lines, mesh, material)| (lines.actor, mesh, material)),
            );
        for (actor, mesh, material) in owned {
            if actor == id {
                meshes.remove(mesh.0.id());
                if let Some(material) = materials.remove(material.0.id())
                    && let Some(image) = material.base.base_color_texture
                {
                    images.remove(image.id());
                }
            }
        }
        commands.entity(root).despawn();
    }
    let Some(world) = world else { return };
    let pending = std::mem::take(&mut gallery.pending);
    for request in pending {
        let battle_actor = match &request {
            Request::Battle { actor } => Some(*actor),
            _ => None,
        };
        let result = match request {
            Request::Battle { actor: id } => (|| {
                if area.is_some() {
                    return Err("AI host currently requires the generated flat test room".into());
                }
                let game = game.as_deref_mut().ok_or("battle session absent")?;
                let rt = game
                    .enemy
                    .as_ref()
                    .ok_or("source-bound Sora combat sheet absent in this room")?;
                let actor = gallery
                    .actors
                    .iter()
                    .find(|a| a.id == id)
                    .ok_or("actor absent")?;
                let battle = super::enemy_battle::BattleActor::load(
                    &gallery.source,
                    &gallery.catalog.entries[actor.entry],
                    &actor.kind,
                    actor.position,
                    actor.yaw,
                    rt.battle_level,
                    rt.difficulty,
                )?;
                let params = super::enemy_battle::target_params(&gallery.source)?;
                gallery.target_params = Some(params);
                game.event(serde_json::json!({"event":"enemy_battle_enabled","tick":game.tick,"actor":id,"source":battle.assets.summary(),"host":"generated flat arena; ordinary encounter context"}));
                let actor = gallery
                    .actors
                    .iter_mut()
                    .find(|a| a.id == id)
                    .ok_or("actor absent")?;
                actor.battle = Some(battle);
                actor.paused = false;
                actor.apply_root = false;
                actor.error = None;
                gallery.status = format!("Actor {id}: AI and combat enabled");
                Ok(())
            })(),
            Request::Summon {
                entry,
                position,
                yaw,
            } => summon(
                &mut commands,
                &mut gallery,
                entry,
                position,
                yaw,
                world.0,
                &mut meshes,
                &mut materials,
                &mut images,
            ),
            Request::Motion { actor, choice } => load_motion(&mut gallery, actor, choice),
        };
        if let Err(error) = result {
            if let Some(id) = battle_actor
                && let Some(game) = game.as_deref_mut()
            {
                game.event(serde_json::json!({"event":"enemy_battle_rejected","actor":id,"tick":game.tick,"reason":error}));
            }
            gallery.status = format!("Inspection load failed: {error}");
            if let Some(kind) = gallery.selected_actor().map(|a| Arc::clone(&a.kind)) {
                append_warnings(&mut gallery.status, &kind);
            }
        }
    }
    // Debug menus pause the field schedule. Restart, root toggle and single
    // step must still show their requested pose while that menu is open.
    advance(&mut gallery, 0.);
}

#[allow(clippy::too_many_arguments)]
fn summon(
    commands: &mut Commands,
    gallery: &mut EnemyGallery,
    entry: usize,
    position: Vec3,
    yaw: f32,
    world: Entity,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<FogMaterial>,
    images: &mut Assets<Image>,
) -> Result<(), String> {
    if gallery.actors.len() >= MAX_ACTORS {
        return Err(format!(
            "maximum {MAX_ACTORS} live inspection actors; remove one first"
        ));
    }
    let kind = match gallery.actors.iter().find(|a| a.entry == entry) {
        Some(actor) => Arc::clone(&actor.kind),
        None => Arc::new(
            gallery
                .catalog
                .load_kind(&gallery.source, entry)
                .map_err(|e| e.to_string())?,
        ),
    };
    let pose = kind.model.poses.first().ok_or("model bind pose absent")?;
    let pose: Vec<Vec3> = pose
        .position_bits
        .iter()
        .map(|p| Vec3::from_array(p.map(f32::from_bits)))
        .collect();
    if kind
        .model
        .parts
        .iter()
        .any(|part| part.indices.iter().any(|&i| i as usize >= pose.len()))
    {
        return Err("model part references absent pose vertex".into());
    }
    let id = gallery.next_id;
    gallery.next_id += 1;
    let root = commands
        .spawn((
            GalleryRoot(id),
            ChildOf(world),
            Transform::from_translation(position).with_rotation(Quat::from_rotation_y(yaw)),
            Visibility::default(),
        ))
        .id();
    for (ordinal, entity) in spawn_model(
        commands,
        &kind.model,
        &pose,
        root,
        meshes,
        materials,
        images,
    ) {
        if let Some(entity) = entity {
            commands.entity(entity).insert(GalleryPart {
                actor: id,
                ordinal,
                revision: 0,
            });
        }
    }
    let joints = joint_origins(&kind.model.poses[0], Vec3::ZERO);
    commands.spawn((
        GallerySkeleton {
            actor: id,
            revision: u64::MAX,
        },
        ChildOf(root),
        Mesh3d(meshes.add(skeleton_mesh(&bone_links(&kind), &joints))),
        MeshMaterial3d(materials.add_plain(StandardMaterial {
            base_color: Color::WHITE,
            unlit: true,
            cull_mode: None,
            ..default()
        })),
        Transform::default(),
        Visibility::Hidden,
    ));
    gallery.actors.push(GalleryActor {
        battle: None,
        id,
        entry,
        kind,
        position,
        yaw,
        motion_choice: None,
        frame: 0.,
        joints,
        paused: false,
        looping: true,
        rate: 1.,
        apply_root: true,
        error: None,
        root,
        selected_motion: None,
        timeline: None,
        pose,
        dirty: false,
        revision: 0,
        preview_idle: None,
    });
    gallery.selected = Some(id);
    gallery.status = format!("Summoned inspection actor {id}; select a source motion slot");
    let actor = gallery
        .actors
        .last()
        .expect("inspection actor just inserted");
    let appearance = actor.kind.unique_action_motion(b"appear");
    let idle = actor.kind.unique_action_motion(b"idle");
    append_warnings(&mut gallery.status, &actor.kind);
    if let Some(choice) = appearance {
        match load_motion(gallery, id, choice) {
            Ok(()) => {
                let actor = gallery
                    .actors
                    .last_mut()
                    .expect("inspection actor just inserted");
                actor.looping = false;
                actor.preview_idle = idle;
                gallery.status =
                    format!("Actor {id}: source appear candidate preview; gallery playback policy");
                append_warnings(&mut gallery.status, &actor.kind);
            }
            Err(error) => {
                gallery.status =
                    format!("Actor {id} bind pose; source appear candidate failed: {error}");
                let actor = gallery
                    .actors
                    .last()
                    .expect("inspection actor just inserted");
                append_warnings(&mut gallery.status, &actor.kind);
            }
        }
    }
    Ok(())
}

fn append_warnings(status: &mut String, kind: &Kind) {
    if kind.model.excluded_linked_groups != 0 {
        status.push_str(&format!(
            "; {} linked mesh groups unrendered",
            kind.model.excluded_linked_groups
        ));
    }
    if !kind.extra_models.is_empty() {
        status.push_str(&format!(
            "; {} additional archive models unrendered",
            kind.extra_models.len()
        ));
    }
    if let Some(issue) = &kind.material_issue {
        status.push_str(&format!("; {issue}"));
    }
}

fn load_motion(gallery: &mut EnemyGallery, id: u64, choice: usize) -> Result<(), String> {
    let actor = gallery
        .actors
        .iter()
        .find(|a| a.id == id)
        .ok_or("inspection actor absent")?;
    if actor.battle.is_some() {
        return Err("AI controls motion; summon another actor for previews".into());
    }
    let motion = gallery
        .actors
        .iter()
        .find(|a| a.entry == actor.entry && a.motion_choice == Some(choice))
        .and_then(|a| a.selected_motion.clone())
        .map_or_else(
            || actor.kind.load_motion(choice).map_err(|e| e.to_string()),
            Ok,
        )?;
    let timeline = Timeline::read(&motion)?;
    // Evaluate before replacing the working clip. An unsupported candidate
    // leaves the actor's previous pose/playback intact and reports the failure.
    let sampled = sample(
        &actor.kind,
        &motion,
        timeline.start,
        actor.apply_root,
        gallery.forward_kinematics_only,
    )?;
    let actor = gallery
        .actors
        .iter_mut()
        .find(|a| a.id == id)
        .ok_or("inspection actor absent")?;
    actor.motion_choice = Some(choice);
    actor.selected_motion = Some(motion);
    actor.timeline = Some(timeline);
    actor.frame = timeline.start;
    actor.pose = sampled.points;
    actor.joints = sampled.joints;
    actor.revision += 1;
    actor.error = None;
    actor.dirty = false;
    actor.preview_idle = None;
    gallery.status = format!(
        "Actor {id}: source slot {} inspection playback",
        actor.kind.motions[choice].slot
    );
    append_warnings(&mut gallery.status, &actor.kind);
    Ok(())
}

/// One evaluated pose: the model's vertices and where each bone's joint is.
struct Sampled {
    points: Vec<Vec3>,
    joints: Vec<Vec3>,
}

/// Each bone's joint position (the translation row of its matrix), moved by
/// `offset`.
fn joint_origins(pose: &ModelPose, offset: Vec3) -> Vec<Vec3> {
    pose.matrix_bits
        .iter()
        .map(|m| {
            Vec3::new(
                f32::from_bits(m[12]),
                f32::from_bits(m[13]),
                f32::from_bits(m[14]),
            ) - offset
        })
        .collect()
}

/// Pairs of (parent, child) joint indices, one per bone with a parent.
fn bone_links(kind: &Kind) -> Vec<(usize, usize)> {
    let bones = &kind.model.skinning.bones;
    bones
        .iter()
        .enumerate()
        .filter_map(|(child, bone)| {
            let parent = usize::try_from(bone.header_i32[1]).ok()?;
            (parent < bones.len() && parent != child).then_some((parent, child))
        })
        .collect()
}

/// Line segments joining each joint to its parent, orange at the parent
/// end and yellow at the child end.
fn skeleton_mesh(links: &[(usize, usize)], joints: &[Vec3]) -> Mesh {
    const PARENT_END: [f32; 4] = [1., 0.45, 0.1, 1.];
    const CHILD_END: [f32; 4] = [1., 1., 0.3, 1.];
    let mut positions = Vec::new();
    let mut colors = Vec::new();
    for &(parent, child) in links {
        if let (Some(from), Some(to)) = (joints.get(parent), joints.get(child)) {
            positions.extend([from.to_array(), to.to_array()]);
            colors.extend([PARENT_END, CHILD_END]);
        }
    }
    visual::room::line_mesh(positions, colors)
}

fn sample(
    kind: &Kind,
    motion: &motion::SelectedMotion,
    frame: f32,
    apply_root: bool,
    forward_kinematics_only: bool,
) -> Result<Sampled, String> {
    let (pose, root) = if forward_kinematics_only {
        if frame < 0. {
            return Err(
                "basic FK cannot sample negative source keys; use the evaluated profile".into(),
            );
        }
        (
            kind.model
                .skinning
                .sample_fk(&motion.motion, frame.floor() as u32)
                .map_err(|e| e.to_string())?,
            Vec3::ZERO,
        )
    } else {
        let (pose, evaluated) = kind
            .model
            .skinning
            .sample_evaluated(&motion.motion, frame)
            .map_err(|e| e.to_string())?;
        (pose, Vec3::from_array(evaluated.root_translation))
    };
    if pose.position_bits.len() != kind.model.poses[0].position_bits.len() {
        return Err("motion pose vertex count differs from model".into());
    }
    let offset = if apply_root { Vec3::ZERO } else { root };
    let points: Vec<Vec3> = pose
        .position_bits
        .iter()
        .map(|p| Vec3::from_array(p.map(f32::from_bits)) - offset)
        .collect();
    if points.iter().any(|p| !p.is_finite()) {
        return Err("motion produced a nonfinite vertex".into());
    }
    let joints = joint_origins(&pose, offset);
    if joints.iter().any(|p| !p.is_finite()) {
        return Err("motion produced a nonfinite joint".into());
    }
    Ok(Sampled { points, joints })
}

/// Called by the parent simulation with its elapsed inspection ticks. This is
/// playback at the source FPS, not a claim about the native enemy scheduler.
pub fn tick(mut gallery: ResMut<EnemyGallery>) {
    advance(&mut gallery, 1.);
}

pub fn advance(gallery: &mut EnemyGallery, ticks: f32) {
    if !ticks.is_finite() || ticks < 0. {
        return;
    }
    let mut idle_previews = Vec::new();
    for actor in &mut gallery.actors {
        if actor.battle.is_some() {
            continue;
        }
        if !actor.paused
            && !actor.looping
            && ticks > 0.
            && actor
                .timeline
                .is_some_and(|timeline| actor.frame >= timeline.end)
            && let Some(choice) = actor.preview_idle.take()
        {
            idle_previews.push((actor.id, choice));
        }
        if let Some(timeline) = actor.timeline
            && !actor.paused
            && ticks > 0.
        {
            let next = timeline.advance(actor.frame, ticks, actor.rate, actor.looping);
            actor.dirty |= next != actor.frame;
            actor.frame = next;
        }
        if actor.dirty
            && let Some(motion) = &actor.selected_motion
        {
            match sample(
                &actor.kind,
                motion,
                actor.frame,
                actor.apply_root,
                gallery.forward_kinematics_only,
            ) {
                Ok(sampled) => {
                    actor.pose = sampled.points;
                    actor.joints = sampled.joints;
                    actor.revision += 1;
                }
                Err(error) => {
                    actor.paused = true;
                    gallery.status = format!("Actor {} playback failed: {error}", actor.id);
                    append_warnings(&mut gallery.status, &actor.kind);
                    actor.error = Some(error);
                }
            }
            actor.dirty = false;
        }
    }
    // This preview sequencing is a development-room policy. The source only
    // resolves the appearance/idle callback's motion, not their scheduling.
    for (id, choice) in idle_previews {
        match load_motion(gallery, id, choice) {
            Ok(()) => {
                let actor = gallery
                    .actors
                    .iter_mut()
                    .find(|a| a.id == id)
                    .expect("live preview actor");
                actor.looping = true;
                gallery.status =
                    format!("Actor {id}: source idle candidate preview; gallery playback policy");
                append_warnings(&mut gallery.status, &actor.kind);
            }
            Err(error) => {
                gallery.status = format!("Actor {id}: source idle preview failed: {error}");
                if let Some(actor) = gallery.actors.iter().find(|a| a.id == id) {
                    append_warnings(&mut gallery.status, &actor.kind);
                }
            }
        }
    }
}

type PartQuery = (
    &'static mut GalleryPart,
    &'static Mesh3d,
    &'static mut Visibility,
);
type SkeletonQuery = (
    &'static mut GallerySkeleton,
    &'static Mesh3d,
    &'static mut Visibility,
);

pub fn present(
    gallery: Res<EnemyGallery>,
    mut roots: Query<(&GalleryRoot, &mut Transform, &mut Visibility)>,
    mut parts: Query<PartQuery, (Without<GalleryRoot>, Without<GallerySkeleton>)>,
    mut skeletons: Query<SkeletonQuery, Without<GalleryRoot>>,
    mut meshes: ResMut<Assets<Mesh>>,
) {
    for (root, mut transform, mut visibility) in &mut roots {
        if let Some(actor) = gallery.actors.iter().find(|a| a.id == root.0) {
            transform.translation = actor.position;
            transform.rotation = Quat::from_rotation_y(actor.yaw);
            transform.scale.y = actor.battle.as_ref().map_or(1., |b| b.brain.scale_y());
            *visibility = if actor.battle.as_ref().is_some_and(|b| b.brain.gone()) {
                Visibility::Hidden
            } else {
                Visibility::Inherited
            };
        }
    }
    let model_visibility = if gallery.overlays.hide_model {
        Visibility::Hidden
    } else {
        Visibility::Inherited
    };
    for (mut part, mesh, mut visibility) in &mut parts {
        visibility.set_if_neq(model_visibility);
        if let Some(actor) = gallery.actors.iter().find(|a| a.id == part.actor)
            && part.revision != actor.revision
            && let Some(mut mesh) = meshes.get_mut(mesh.0.id())
        {
            let positions = actor.kind.model.parts[part.ordinal]
                .indices
                .iter()
                .map(|&i| actor.pose[i as usize].to_array());
            visual::room::update_positions(&mut mesh, positions);
            part.revision = actor.revision;
        }
    }
    let skeleton_visibility = if gallery.overlays.skeleton {
        Visibility::Inherited
    } else {
        Visibility::Hidden
    };
    for (mut lines, mesh, mut visibility) in &mut skeletons {
        visibility.set_if_neq(skeleton_visibility);
        // Hidden lines are not redrawn; they catch up when shown.
        if !gallery.overlays.skeleton {
            continue;
        }
        if let Some(actor) = gallery.actors.iter().find(|a| a.id == lines.actor)
            && lines.revision != actor.revision
            && let Some(mut mesh) = meshes.get_mut(mesh.0.id())
        {
            *mesh = skeleton_mesh(&bone_links(&actor.kind), &actor.joints);
            lines.revision = actor.revision;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::super::enemy_catalog::MotionChoice;
    use super::*;
    use data::model_inspection::InspectionModel;
    use data::model_pose::{ModelPose, PoseProfile, Skinning};
    use std::{
        fs,
        path::PathBuf,
        time::{SystemTime, UNIX_EPOCH},
    };

    // Generated bytes only. Never binds or modifies an installed source.
    struct Fixture {
        base: PathBuf,
        source: Arc<ProbeSource>,
    }

    impl Fixture {
        fn new() -> Self {
            let base = std::env::temp_dir().join(format!(
                "kh2-gallery-{}-{}",
                std::process::id(),
                SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .unwrap()
                    .as_nanos()
            ));
            let root = base.join("source");
            fs::create_dir_all(root.join("Image/dt")).unwrap();
            for (path, _) in validation::required_files() {
                let bytes = if path.ends_with(".hed") {
                    let mut bytes = vec![0; 24];
                    bytes.extend(16i32.to_le_bytes());
                    bytes.extend(0i32.to_le_bytes());
                    bytes
                } else {
                    vec![0; 16]
                };
                fs::write(root.join(path), bytes).unwrap();
            }
            let manifest = base.join("manifest.json");
            fs::write(
                &manifest,
                serde_json::to_vec(&validation::scan(&root).unwrap()).unwrap(),
            )
            .unwrap();
            let source = Arc::new(ProbeSource::bind_session(&root, &manifest).unwrap());
            source.verify_session().unwrap();
            Self { base, source }
        }

        fn gallery(&self) -> EnemyGallery {
            EnemyGallery::new(
                self.source.clone(),
                Catalog {
                    entries: Vec::new(),
                    issues: Vec::new(),
                },
                true,
            )
        }
    }

    impl Drop for Fixture {
        fn drop(&mut self) {
            fs::remove_dir_all(&self.base).unwrap();
        }
    }

    fn actor(id: u64, root: Entity) -> GalleryActor {
        let model = InspectionModel {
            index: String::new(),
            ordinal: 0,
            bar_ordinal: 0,
            excluded_linked_groups: 0,
            parts: Vec::new(),
            poses: vec![ModelPose {
                profile: PoseProfile::Legacy,
                arithmetic_profile: "synthetic",
                matrix_bits: Vec::new(),
                position_bits: Vec::new(),
                runtime_verified: false,
            }],
            skinning: Skinning::default(),
            textures: None,
            texture_routes: Vec::new(),
        };
        let kind = Kind::synthetic(
            model,
            vec![MotionChoice {
                slot: 77,
                name: "synthetic".into(),
                status: Some("synthetic unsupported payload".into()),
            }],
            Vec::new(),
        );
        let table = || motion::Table {
            offset: 0,
            stride: 0,
            bytes: Vec::new(),
        };
        let motion = motion::SelectedMotion {
            slot: 3,
            sha256: String::new(),
            triggers: Vec::new(),
            motion: motion::InterpolatedMotion {
                raw: None,
                header_words: [0; 4],
                interpolated_header_words: {
                    let mut words = vec![0; 40];
                    for (i, value) in [(32, 4f32), (33, 24.), (34, 30.), (35, 8.)] {
                        words[i] = value.to_bits();
                    }
                    words
                },
                bone_count: 0,
                total_bone_count: 0,
                initial_poses: Vec::new(),
                forward_curves: Vec::new(),
                inverse_curves: Vec::new(),
                keys: Vec::new(),
                key_tail_bytes: Vec::new(),
                time_bits: Vec::new(),
                value_bits: Vec::new(),
                tangent_bits: Vec::new(),
                constraints: table(),
                activations: table(),
                limiters: table(),
                expressions: table(),
                expression_nodes: table(),
                helpers: table(),
                joints: table(),
                root: table(),
                external_effectors: table(),
                extra_sha256: String::new(),
                extra_size: 0,
            },
        };
        GalleryActor {
            battle: None,
            id,
            entry: 0,
            kind: Arc::new(kind),
            position: Vec3::ZERO,
            yaw: 0.,
            motion_choice: None,
            frame: 5.,
            joints: Vec::new(),
            paused: false,
            looping: true,
            rate: 1.,
            apply_root: true,
            error: None,
            root,
            selected_motion: Some(Arc::new(motion)),
            timeline: Some(Timeline {
                start: 4.,
                end: 24.,
                fps: 30.,
                loop_start: 8.,
            }),
            pose: Vec::new(),
            dirty: false,
            revision: 0,
            preview_idle: None,
        }
    }

    #[test]
    fn linked_geometry_and_material_warnings_survive_status_updates() {
        let mut a = actor(1, Entity::PLACEHOLDER);
        let kind = Arc::get_mut(&mut a.kind).unwrap();
        kind.model.excluded_linked_groups = 2;
        kind.material_issue = Some("synthetic material limitation".into());
        let mut status = "Selected motion".to_string();
        append_warnings(&mut status, kind);
        assert!(status.contains("2 linked mesh groups unrendered"));
        assert!(status.contains("synthetic material limitation"));
    }
    #[test]
    fn negative_declared_start_remains_a_signed_source_key() {
        let mut a = actor(1, Entity::PLACEHOLDER);
        let motion = Arc::get_mut(a.selected_motion.as_mut().unwrap()).unwrap();
        motion.motion.interpolated_header_words[32] = (-5_f32).to_bits();
        let timeline = Timeline::read(motion).unwrap();
        assert_eq!(timeline.start, -5.);
        assert_eq!(timeline.advance(-5., 2., 1., false), -4.);
        assert!(sample(&a.kind, motion, -5., true, true).is_err());
    }

    #[test]
    fn source_fps_and_return_window_control_playback() {
        let w = Timeline {
            start: 4.,
            end: 24.,
            fps: 30.,
            loop_start: 8.,
        };
        assert_eq!(w.advance(w.start, 2., 1., true), 5.);
        assert_eq!(w.advance(23., 6., 1., true), 10.);
        assert_eq!(w.advance(23., 6., 1., false), 24.);
        assert_eq!(w.advance(23., 70., 1., true), 10.);
        assert_eq!(w.advance(4., 2., 0.5, true), 4.5);
    }

    #[test]
    fn zero_length_or_nonlooping_motion_holds_its_last_pose() {
        let w = Timeline {
            start: 4.,
            end: 4.,
            fps: 30.,
            loop_start: 4.,
        };
        assert_eq!(w.advance(4., 2., 1., true), 4.);
        let w = Timeline {
            start: 0.,
            end: 8.,
            fps: 30.,
            loop_start: 8.,
        };
        assert_eq!(w.advance(7., 20., 1., true), 8.);
    }

    #[test]
    fn selected_controls_are_independent_and_step_evaluates_while_paused() {
        let fixture = Fixture::new();
        let mut world = World::new();
        let mut gallery = fixture.gallery();
        gallery.actors.push(actor(1, world.spawn_empty().id()));
        gallery.actors.push(actor(2, world.spawn_empty().id()));
        gallery.select(1);
        gallery.toggle_paused();
        gallery.select(2);
        gallery.step_frame(true);
        advance(&mut gallery, 0.);
        assert!(gallery.actors.iter().all(|a| a.paused));
        assert_eq!(gallery.actors[0].frame, 5.);
        assert_eq!(gallery.actors[0].revision, 0);
        assert_eq!(gallery.actors[1].frame, 6.);
        assert_eq!(gallery.actors[1].revision, 1);
        assert!(!gallery.actors[1].dirty);
        gallery.restart();
        advance(&mut gallery, 0.);
        assert_eq!(gallery.actors[1].frame, 4.);
        assert!(!gallery.actors[1].paused);
        assert_eq!(gallery.actors[1].revision, 2);
        gallery.set_rate(f32::NAN);
        assert_eq!(gallery.actors[1].rate, 1.);
        gallery.set_rate(80.);
        assert_eq!(gallery.actors[1].rate, 4.);
        gallery.toggle_loop();
        assert!(!gallery.actors[1].looping);
        assert!(gallery.actors[0].looping);
    }

    #[test]
    fn stepping_and_jumping_land_on_whole_frames_inside_the_range() {
        let t = Timeline {
            start: 4.,
            end: 24.,
            fps: 30.,
            loop_start: 8.,
        };
        assert_eq!(t.step_frame(5.5, true, false), 6.);
        assert_eq!(t.step_frame(5.5, false, false), 5.);
        assert_eq!(t.step_frame(6., false, false), 5.);
        assert_eq!(t.step_frame(24., true, false), 24.);
        assert_eq!(t.step_frame(24., true, true), 4.);
        assert_eq!(t.step_frame(4., false, true), 24.);
        assert_eq!(t.step_frame(4., false, false), 4.);
        assert_eq!((t.length(), t.seconds()), (20., 20. / 30.));
        assert_eq!(t.at_fraction(0.5), 14.);
        assert_eq!(t.at_fraction(2.), 24.);
    }

    #[test]
    fn go_to_frame_pauses_and_playback_speed_moves_between_presets() {
        let fixture = Fixture::new();
        let mut world = World::new();
        let mut gallery = fixture.gallery();
        gallery.actors.push(actor(1, world.spawn_empty().id()));
        gallery.select(1);
        gallery.go_to_fraction(0.5);
        assert!(gallery.actors[0].paused);
        assert_eq!(gallery.actors[0].frame, 14.);
        gallery.scrub(-100.);
        assert_eq!(gallery.actors[0].frame, 4.);
        gallery.go_to_frame(f32::NAN);
        assert_eq!(gallery.actors[0].frame, 4.);
        gallery.change_speed(1);
        assert_eq!(gallery.actors[0].rate, 1.5);
        gallery.change_speed(-1);
        gallery.change_speed(-1);
        assert_eq!(gallery.actors[0].rate, 0.75);
        for _ in 0..20 {
            gallery.change_speed(1);
        }
        assert_eq!(gallery.actors[0].rate, 4.);
    }

    #[test]
    fn failed_motion_load_preserves_working_actor_and_room_change_clears_queue() {
        let fixture = Fixture::new();
        let mut world = World::new();
        let root = world.spawn_empty().id();
        let mut gallery = fixture.gallery();
        gallery.actors.push(actor(1, root));
        gallery.select(1);
        let selected = Arc::clone(gallery.actors[0].selected_motion.as_ref().unwrap());
        assert!(
            load_motion(&mut gallery, 1, 0)
                .unwrap_err()
                .contains("unsupported")
        );
        assert!(load_motion(&mut gallery, 1, 99).is_err());
        assert_eq!(gallery.actors[0].frame, 5.);
        assert!(Arc::ptr_eq(
            &selected,
            gallery.actors[0].selected_motion.as_ref().unwrap()
        ));
        for _ in 0..MAX_PENDING + 1 {
            gallery.queue_summon(0, Vec3::ZERO, 0.);
        }
        assert_eq!(gallery.pending.len(), MAX_PENDING);
        gallery.observe_location((root, Some((2, 1))));
        assert_eq!(gallery.actors.len(), 1);
        gallery.observe_location((root, Some((2, 2))));
        assert!(gallery.actors.is_empty());
        assert!(gallery.pending.is_empty());
        assert_eq!(gallery.garbage, vec![(1, root)]);
        assert_eq!(gallery.selected, None);
    }

    #[test]
    fn appearance_preview_reaches_end_before_idle_and_manual_motion_cancels_policy() {
        let fixture = Fixture::new();
        let mut world = World::new();
        let mut gallery = fixture.gallery();
        let mut preview = actor(1, world.spawn_empty().id());
        Arc::get_mut(&mut preview.kind).unwrap().motions[0].status = None;
        Arc::get_mut(&mut preview.kind).unwrap().motions[0].slot = 3;
        preview.motion_choice = Some(0);
        preview.frame = 23.5;
        preview.looping = false;
        preview.preview_idle = Some(0);
        gallery.actors.push(preview);
        advance(&mut gallery, 1.);
        assert_eq!(gallery.actors[0].frame, 24.);
        assert_eq!(gallery.actors[0].preview_idle, Some(0));
        advance(&mut gallery, 0.);
        assert_eq!(gallery.actors[0].frame, 24.);
        advance(&mut gallery, 1.);
        assert_eq!(gallery.actors[0].frame, 4.);
        assert!(gallery.actors[0].looping);
        assert_eq!(gallery.actors[0].preview_idle, None);
        assert!(gallery.status.contains("gallery playback policy"));
        gallery.actors[0].preview_idle = Some(0);
        load_motion(&mut gallery, 1, 0).unwrap();
        assert_eq!(gallery.actors[0].preview_idle, None);
    }

    #[test]
    fn clear_releases_actor_entities_and_owned_render_assets() {
        let fixture = Fixture::new();
        let mut app = App::new();
        app.init_resource::<Assets<Mesh>>()
            .init_resource::<Assets<FogMaterial>>()
            .init_resource::<Assets<Image>>()
            .add_systems(Update, service);
        let scene = app.world_mut().spawn_empty().id();
        let root = app.world_mut().spawn((GalleryRoot(1), ChildOf(scene))).id();
        let mesh = app
            .world_mut()
            .resource_mut::<Assets<Mesh>>()
            .add(Cuboid::default());
        let image = app
            .world_mut()
            .resource_mut::<Assets<Image>>()
            .add(Image::default());
        let material = app
            .world_mut()
            .resource_mut::<Assets<FogMaterial>>()
            .add(FogMaterial {
                base: StandardMaterial {
                    base_color_texture: Some(image.clone()),
                    ..default()
                },
                extension: default(),
            });
        let part = app
            .world_mut()
            .spawn((
                GalleryPart {
                    actor: 1,
                    ordinal: 0,
                    revision: 0,
                },
                ChildOf(root),
                Mesh3d(mesh.clone()),
                MeshMaterial3d(material.clone()),
            ))
            .id();
        let mut gallery = fixture.gallery();
        gallery.actors.push(actor(1, root));
        gallery.clear();
        app.insert_resource(SceneRoot(scene))
            .insert_resource(gallery);
        app.update();
        assert!(app.world().get_entity(root).is_err());
        assert!(app.world().get_entity(part).is_err());
        assert!(
            app.world()
                .resource::<Assets<Mesh>>()
                .get(mesh.id())
                .is_none()
        );
        assert!(
            app.world()
                .resource::<Assets<FogMaterial>>()
                .get(material.id())
                .is_none()
        );
        assert!(
            app.world()
                .resource::<Assets<Image>>()
                .get(image.id())
                .is_none()
        );
    }
}
