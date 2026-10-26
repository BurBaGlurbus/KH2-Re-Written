//! The duel's other Sora: a second [`Fighter`] that runs the same state
//! machine on the room's starting kit, steered by a pad of its own (a test
//! script here, the other player's buttons over the network later), drawn
//! with copies of Sora's meshes. The room's camera, HUD and menus stay on
//! the room's own Sora.

use super::*;
use player::ptya;

/// What the rivals move and are drawn with: the room's starting kit (a
/// Drive form changes only the room Sora's own).
pub(crate) struct RivalKit {
    pub params: Params,
    pub clips: Clips,
    pub poses: HashMap<(Action, bool), usize>,
    pub clip_poses: Arc<Vec<ClipPoses>>,
    pub target_shape: Option<object_collision::Entry>,
    pub target_radius: f32,
}

/// One tick of a rival's buttons: the stick already as a world XZ direction
/// (magnitude 0..=1) and the buttons pressed this tick.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub(crate) struct RivalPad {
    pub direction: [f32; 2],
    pub jump: bool,
    pub jump_held: bool,
    pub attack: bool,
    pub guard: bool,
    pub guard_held: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Control {
    /// The built-in test script (`--rival-script`, F3 > Sora).
    Script,
}

pub(crate) struct Rival {
    /// 0 the host's Sora, 1 the joiner's.
    pub slot: u8,
    pub fighter: Fighter,
    /// Tick the current action started (attack instance ids, blends).
    pub started: u64,
    pub control: Control,
    pub pad: RivalPad,
    /// The map collision's last floor normal and material for this body
    /// (the map keeps the room Sora's own).
    floor: Option<([f32; 3], u8)>,
    /// What the presenter showed (crossfades between motions).
    shown: Shown,
}

#[derive(Default)]
struct Shown {
    key: Option<(usize, usize)>,
    target: Vec<Vec3>,
    display: Vec<Vec3>,
    action: Option<(Action, u64)>,
    blend_from: Option<(Vec<Vec3>, u64)>,
    spawned: bool,
}

/// The second slot's id.
pub(crate) const RIVAL_SLOT: u8 = 1;
/// How far in front of the room Sora a rival appears.
const SPAWN_DISTANCE: f32 = 400.;

impl Rival {
    pub(crate) fn new(slot: u8, player: Player, vitals: Vitals, control: Control) -> Rival {
        Rival {
            slot,
            fighter: Fighter { player, vitals },
            started: 0,
            control,
            pad: RivalPad::default(),
            floor: None,
            shown: Shown::default(),
        }
    }
}

/// Puts a scripted rival in front of the room Sora, facing him, with his
/// battle numbers at full HP. Returns false when one is already there.
pub(crate) fn spawn_scripted(game: &mut Game) -> bool {
    if !game.rivals.is_empty() {
        return false;
    }
    let p = game.sora.player.position;
    let yaw = game.sora.player.yaw;
    let at = [
        p[0] + yaw.sin() * SPAWN_DISTANCE,
        p[1],
        p[2] + yaw.cos() * SPAWN_DISTANCE,
    ];
    let mut player = Player::new(at, yaw + std::f32::consts::PI);
    player.abilities = game.sora.player.abilities.clone();
    player.fmab = game.sora.player.fmab.clone();
    let mut vitals = game.sora.vitals.clone();
    vitals.hp = vitals.target.max_hp;
    vitals.record = combat::HitRecord::new(true);
    game.rivals
        .push(Rival::new(RIVAL_SLOT, player, vitals, Control::Script));
    let tick = game.tick;
    game.event(
        serde_json::json!({"event":"rival_spawn","tick":tick,"slot":RIVAL_SLOT,
        "position":at,"control":"script"}),
    );
    true
}

/// `--rival-script`: the scripted rival is there from the start.
pub(crate) fn script_setup(mut game: ResMut<Game>) {
    if game.rival_script {
        spawn_scripted(&mut game);
    }
}

/// The test script: walk toward the room Sora, swing when close, jump and
/// guard now and then, circle when far. Fixed by the tick so a run repeats.
pub(crate) fn script_pad(tick: u64, me: [f32; 3], toward: [f32; 3]) -> RivalPad {
    let d = Vec2::new(toward[0] - me[0], toward[2] - me[2]);
    let distance = d.length();
    let chase = d.normalize_or_zero();
    let side = Vec2::new(-chase.y, chase.x);
    let phase = tick % 600;
    let mut pad = RivalPad::default();
    let direction = if distance > 260. {
        chase
    } else if phase < 300 {
        (chase * 0.2 + side * 0.6).clamp_length_max(1.)
    } else {
        (chase * 0.2 - side * 0.6).clamp_length_max(1.)
    };
    pad.direction = direction.to_array();
    pad.attack = distance < 220. && tick.is_multiple_of(18);
    pad.jump = phase == 120 || phase == 420;
    pad.jump_held = matches!(phase, 120..150 | 420..450);
    pad.guard = phase == 250;
    pad
}

/// The room Sora's TARGET shape as a rival's attacks aim at it.
fn sora_target(game: &Game, radius: f32) -> Option<ptya::Target> {
    let centre = enemy_target::player_targetcentre(game)?;
    Some(ptya::Target {
        centre,
        radius,
        height: radius,
    })
}

/// A rival's TARGET centre: the kit's TARGET shape on its bone this frame.
pub(crate) fn target_centre(kit: &RivalKit, player: &Player) -> [f32; 3] {
    let Some(shape) = kit.target_shape.as_ref() else {
        return player.position;
    };
    let matrix = usize::try_from(shape.bone)
        .ok()
        .filter(|&b| b != 0x4000)
        .and_then(|bone| {
            let clip = kit
                .clip_poses
                .get(*kit.poses.get(&(player.action, player.armed))?)?;
            let frame = player.motion_frame(&kit.clips) as usize;
            clip.body
                .get(frame.min(clip.body.len().saturating_sub(1)))?
                .get(&bone)
                .copied()
        })
        .unwrap_or(Mat4::IDENTITY);
    let offset = Vec3::from_array(shape.position.map(|value| value as f32));
    let local = matrix.transform_point3(offset) + Vec3::Y * shape.position_height as f32;
    let centre = Vec3::from_array(player.position) + Quat::from_rotation_y(player.yaw) * local;
    if centre.is_finite() {
        centre.to_array()
    } else {
        player.position
    }
}

/// One simulation tick of every rival, after the room Sora's (FixedUpdate,
/// in the field chain).
pub(crate) fn tick(mut game: ResMut<Game>) {
    if game.rivals.is_empty() {
        return;
    }
    let game = &mut *game;
    let Some(kit) = game.rival_kit.clone() else {
        return;
    };
    let tick = game.tick;
    let sora_at = game.sora.player.position;
    let sora_shape = sora_target(game, kit.target_radius);
    let mut rivals = std::mem::take(&mut game.rivals);
    for rival in &mut rivals {
        let player = &mut rival.fighter.player;
        if rival.control == Control::Script {
            rival.pad = script_pad(tick, player.position, sora_at);
        }
        let pad = rival.pad;
        let me = target_centre(&kit, player);
        let target = sora_shape.map(|s| {
            [
                s.centre[0] - player.position[0],
                s.centre[2] - player.position[2],
            ]
        });
        let input = Input {
            direction: pad.direction,
            jump: pad.jump,
            jump_up: !pad.jump_held,
            attack: pad.attack,
            guard: pad.guard,
            guard_held: pad.guard_held,
            target,
            hop_vector: sora_shape.map(|s| {
                [
                    s.centre[0] - me[0],
                    s.centre[1] - me[1],
                    s.centre[2] - me[2],
                ]
            }),
            target_reach: sora_shape.map_or(0., |s| s.radius + kit.target_radius),
            target_shape: sora_shape,
            nearby: [sora_shape, None, None, None],
            probe_blocked: false,
        };
        let step = match &game.world {
            RoomWorld::Map(map) => {
                let mine = rival.floor.unwrap_or(area::FLOOR_NONE);
                let room = map.swap_floor_state(mine);
                let step = player.tick(&input, &kit.params, &kit.clips, &game.world);
                rival.floor = Some(map.swap_floor_state(room));
                step
            }
            RoomWorld::Triangles(_) => player.tick(&input, &kit.params, &kit.clips, &game.world),
        };
        rival.fighter.vitals.invulnerable = (rival.fighter.vitals.invulnerable - 1.).max(0.);
        if let Some(action) = step.started {
            rival.started = tick;
            if let Some(file) = game.log.as_mut() {
                let record = serde_json::json!({"event":"rival_action","tick":tick,
                    "slot":rival.slot,"action":format!("{action:?}"),"position":player.position});
                let _ = serde_json::to_writer(&mut *file, &record);
                let _ = std::io::Write::write_all(file, b"\n");
            }
        }
    }
    game.rivals = rivals;
}

#[derive(Component)]
pub(crate) struct RivalRoot(pub u8);
#[derive(Component)]
pub(crate) struct RivalPart(pub u8, pub usize);
#[derive(Component)]
pub(crate) struct RivalHand(pub u8);
#[derive(Component)]
pub(crate) struct RivalWeaponPart(pub u8, pub usize);

/// Gives each new rival copies of the room Sora's meshes (the same
/// materials) under the scene's root; removes the drawing of rivals that
/// left.
#[allow(clippy::type_complexity)]
pub(crate) fn spawn_drawing(
    mut commands: Commands,
    mut game: ResMut<Game>,
    scene: Option<Res<SceneRoot>>,
    parts: Query<(&SoraPart, &Mesh3d, &MeshMaterial3d<FogMaterial>)>,
    weapon_parts: Query<(&WeaponPart, &Mesh3d, &MeshMaterial3d<FogMaterial>)>,
    roots: Query<(Entity, &RivalRoot)>,
    mut meshes: ResMut<Assets<Mesh>>,
) {
    let game = &mut *game;
    for (entity, root) in &roots {
        if !game.rivals.iter().any(|r| r.slot == root.0) {
            commands.entity(entity).despawn();
        }
    }
    let Some(scene) = scene else { return };
    for rival in game.rivals.iter_mut().filter(|r| !r.shown.spawned) {
        rival.shown.spawned = true;
        let slot = rival.slot;
        let p = &rival.fighter.player;
        let root = commands
            .spawn((
                RivalRoot(slot),
                ChildOf(scene.0),
                Transform::from_translation(Vec3::from_array(p.position))
                    .with_rotation(Quat::from_rotation_y(p.yaw)),
                Visibility::default(),
            ))
            .id();
        for (part, mesh, material) in &parts {
            let Some(copy) = meshes.get(&mesh.0).cloned() else {
                continue;
            };
            commands.spawn((
                RivalPart(slot, part.0),
                ChildOf(root),
                Mesh3d(meshes.add(copy)),
                material.clone(),
                Transform::default(),
            ));
        }
        let hand = commands
            .spawn((
                RivalHand(slot),
                ChildOf(root),
                Transform::default(),
                Visibility::default(),
            ))
            .id();
        for (part, mesh, material) in &weapon_parts {
            let Some(copy) = meshes.get(&mesh.0).cloned() else {
                continue;
            };
            commands.spawn((
                RivalWeaponPart(slot, part.0),
                ChildOf(hand),
                Mesh3d(meshes.add(copy)),
                material.clone(),
                Transform::default(),
            ));
        }
    }
}

/// Poses each rival from its kit at the newest tick, with the motion
/// crossfade blended on the body's points.
#[allow(clippy::type_complexity, clippy::too_many_arguments)]
pub(crate) fn present(
    fixed: Res<Time<Fixed>>,
    virt: Res<Time<Virtual>>,
    mut game: ResMut<Game>,
    mut roots: Query<(&RivalRoot, &mut Transform), Without<RivalHand>>,
    mut hands: Query<(&RivalHand, &mut Transform, &mut Visibility), Without<RivalRoot>>,
    parts: Query<(&RivalPart, &Mesh3d)>,
    weapon_parts: Query<(&RivalWeaponPart, &Mesh3d)>,
    mut meshes: ResMut<Assets<Mesh>>,
) {
    let game = &mut *game;
    let Some(kit) = game.rival_kit.clone() else {
        return;
    };
    let now = game.tick;
    for rival in &mut game.rivals {
        let p = &rival.fighter.player;
        let Some(&clip) = kit.poses.get(&(p.action, p.armed)) else {
            continue;
        };
        let cp = &kit.clip_poses[clip];
        let frame = p.motion_frame(&kit.clips) as usize;
        let shown = &mut rival.shown;
        if shown.action != Some((p.action, rival.started)) {
            if shown.action.is_some() && !shown.display.is_empty() {
                shown.blend_from = Some((shown.display.clone(), rival.started));
            }
            shown.action = Some((p.action, rival.started));
        }
        if shown.key != Some((clip, frame)) {
            shown.key = Some((clip, frame));
            shown.target = cp.frame(frame);
        }
        let mut display = shown.target.clone();
        if let Some((from, start)) = &shown.blend_from {
            let overstep = if virt.is_paused() {
                0.
            } else {
                fixed.overstep_fraction()
            };
            let elapsed = now.saturating_sub(*start) as f32 + overstep;
            let weight = if p.blend > 2. {
                ((elapsed + 3.) / p.blend).clamp(0., 1.)
            } else {
                1.
            };
            if weight >= 1. || from.len() != display.len() {
                shown.blend_from = None;
            } else {
                for (d, f) in display.iter_mut().zip(from) {
                    *d = f.lerp(*d, weight);
                }
            }
        }
        for (part, handle) in &parts {
            if part.0 != rival.slot {
                continue;
            }
            if let Some(indices) = game.part_indices.get(part.1) {
                let positions = indices
                    .iter()
                    .filter_map(|&i| display.get(i as usize).map(|v| v.to_array()));
                visual::room::set_positions(&mut meshes, &handle.0, positions);
            }
        }
        shown.display = display;
        let weapons = &cp.weapon;
        let hand = Transform::from_matrix(weapons[frame.min(weapons.len().saturating_sub(1))]);
        for (h, mut transform, mut visibility) in &mut hands {
            if h.0 == rival.slot {
                *transform = hand;
                *visibility = if p.armed {
                    Visibility::Inherited
                } else {
                    Visibility::Hidden
                };
            }
        }
        if !cp.weapon_frames.is_empty() {
            let pose = &cp.weapon_frames[frame.min(cp.weapon_frames.len() - 1)];
            for (part, handle) in &weapon_parts {
                if part.0 != rival.slot {
                    continue;
                }
                if let Some(indices) = game.weapon_indices.get(part.1) {
                    let positions = indices
                        .iter()
                        .filter_map(|&i| pose.get(i as usize).map(|v| v.to_array()));
                    visual::room::set_positions(&mut meshes, &handle.0, positions);
                }
            }
        }
        for (root, mut transform) in &mut roots {
            if root.0 == rival.slot {
                transform.translation = Vec3::from_array(p.position);
                transform.rotation = Quat::from_rotation_y(p.yaw);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_script_chases_then_swings_near() {
        let far = script_pad(0, [0., 0., 0.], [0., 0., 1000.]);
        assert_eq!(far.direction, [0., 1.]);
        assert!(!far.attack);
        let near = script_pad(36, [0., 0., 0.], [0., 0., 100.]);
        assert!(near.attack);
        assert!(Vec2::from_array(near.direction).length() <= 1.);
    }

    #[test]
    fn the_script_repeats_by_tick() {
        for t in [0, 17, 120, 250, 599] {
            assert_eq!(
                script_pad(t, [1., 0., 2.], [300., 0., -40.]),
                script_pad(t, [1., 0., 2.], [300., 0., -40.])
            );
        }
        assert!(script_pad(120, [0.; 3], [0., 0., 50.]).jump);
        assert!(script_pad(250, [0.; 3], [0., 0., 50.]).guard);
    }
}
