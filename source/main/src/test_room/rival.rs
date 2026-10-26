//! The duel's other Sora: a second [`Fighter`] with a whole Sora's state of
//! his own (kit, magic, items, Drive forms, abilities, equipment), steered
//! by a pad of his own (a test script, or the other player's buttons and
//! commands over the network). For his tick his state is swapped into the
//! room's (`swap`), so the room's own code moves him, casts his spells,
//! uses his items and changes his forms; then it is swapped back. He is
//! drawn with copies of Sora's meshes (a form's model once he changes).
//! The room's camera, HUD and menus stay on the room's own Sora.

use super::*;
use player::ptya;

/// The room's starting kit, which a rival starts from.
pub(crate) struct RivalKit {
    pub params: Params,
    pub clips: Clips,
    pub poses: HashMap<(Action, bool), usize>,
    pub clip_poses: Arc<Vec<ClipPoses>>,
    pub target_shape: Option<object_collision::Entry>,
    /// HIT spheres (bone, offset, radius, high).
    pub body: Vec<(usize, Vec3, f32, bool)>,
    pub target_radius: f32,
}

/// A command the other player chose in their menu (the host runs it).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) enum Command {
    #[default]
    None,
    /// A spell by magic type.
    Cast(u8),
    /// An item slot.
    Item(u8),
    /// A Drive form, or 0 to revert.
    Drive(u8),
}

/// One tick of a rival's buttons: the stick already as a world XZ direction
/// (magnitude 0..=1), the buttons pressed this tick and a menu command.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub(crate) struct RivalPad {
    pub direction: [f32; 2],
    pub jump: bool,
    pub jump_held: bool,
    pub attack: bool,
    pub guard: bool,
    pub guard_held: bool,
    pub command: Command,
}

impl RivalPad {
    /// The buttons of a room Sora's input, for the host to replay.
    pub(crate) fn of(input: &Input) -> RivalPad {
        RivalPad {
            direction: input.direction,
            jump: input.jump,
            jump_held: !input.jump_up,
            attack: input.attack,
            guard: input.guard,
            guard_held: input.guard_held,
            command: Command::None,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Control {
    /// The built-in test script (`--rival-script`, F3 > Sora).
    Script,
    /// The other player's buttons (the host) or the host's snapshots (the
    /// joiner), set each tick by duel_sync.
    Remote,
}

/// What a Sora has of his own besides the [`Fighter`]: the fields of the
/// room's `Game` his tick reads and changes, swapped in for it.
pub(crate) struct Own {
    pub params: Params,
    pub clips: Clips,
    pub poses: HashMap<(Action, bool), usize>,
    pub clip_poses: Arc<Vec<ClipPoses>>,
    pub player_target_shape: Option<object_collision::Entry>,
    pub sora_attack: HashMap<u8, Vec<(usize, Vec3, f32)>>,
    pub magic: Option<magic_room::MagicRt>,
    pub items: Option<item_room::ItemRt>,
    pub drive: Option<drive_room::DriveRt>,
    pub ability_slots: Vec<u16>,
    pub equipment: pause_menu::items::Equipment,
    pub gear: player::equipment_stats::Totals,
    pub selected_target: Option<[f32; 3]>,
    pub target_locked: bool,
    pub cure_target: usize,
    pub weapon_pax: Option<Arc<object_effect::Pax>>,
    pub weapon2_pax: Option<Arc<object_effect::Pax>>,
    /// The battle runtime's spheres for him (they follow his kit).
    pub blades: [character_kit::Blade; 2],
    pub body: Vec<(usize, Vec3, f32, bool)>,
    /// The spell effects' feedback under his own keys.
    pub fx_live: HashMap<u64, magic::FxLive>,
    /// Where his motions' sound and dust triggers were last scanned.
    pub scans: Scans,
}

/// A Sora's frame-trigger scans (swings, voices, cloth, footsteps, dust):
/// the action start and the frame already scanned.
#[derive(Default)]
pub(crate) struct Scans {
    sound_frame: Option<(u64, u32)>,
    voice: Option<(u64, Action, u32)>,
    move_se: Option<(u64, Action, u32)>,
    step: Option<(u64, Action, u32)>,
    dust: Option<(u64, Action, u32)>,
}

pub(crate) struct Rival {
    /// 0 the host's Sora, 1 the joiner's.
    pub slot: u8,
    pub fighter: Fighter,
    /// Tick the current action started (attack instance ids, blends).
    pub started: u64,
    pub control: Control,
    pub pad: RivalPad,
    pub own: Box<Own>,
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
    /// The model's parts' vertex indices, and each Keyblade's.
    part_indices: Vec<Vec<u32>>,
    weapon_indices: [Vec<Vec<u32>>; 2],
}

/// The second slot's id.
pub(crate) const RIVAL_SLOT: u8 = 1;
/// How far in front of the room Sora a rival appears.
const SPAWN_DISTANCE: f32 = 400.;
/// Where the joiner's Sora starts: this far in front of the host's, facing
/// him.
const DUEL_DISTANCE: f32 = 500.;
/// A rival's spell and item effect keys carry his slot (+1) in these bits,
/// so they never meet the room Sora's.
const KEY_SLOT_SHIFT: u32 = 56;

/// The effect key of `key` for a Sora in `slot` (the room Sora's own keys
/// stay as they are).
pub(crate) fn tag_key(key: u64, slot: u8) -> u64 {
    key ^ (u64::from(slot) + 1) << KEY_SLOT_SHIFT
}

/// A new rival's own state: the room's starting kit, a fresh magic, item
/// and Drive state with the room Sora's levels, stock and form records, his
/// abilities and equipment.
fn own_from(game: &Game, kit: &RivalKit, slot: u8) -> Own {
    let blades = game
        .enemy
        .as_ref()
        .map(|rt| rt.blade.clone())
        .unwrap_or_default();
    let mut magic = game
        .magic
        .as_ref()
        .map(|m| magic_room::MagicRt::new(m.src.clone(), m.levels));
    if let Some(m) = magic.as_mut() {
        // Spell serials of their own, so volume keys never meet the room
        // Sora's.
        m.serial = (u64::from(slot) + 1) << 40;
    }
    let items = game.items.as_ref().map(|i| {
        item_room::ItemRt::new(
            i.src.clone(),
            i.slots.clone(),
            i.originals.clone(),
            i.stock.clone(),
        )
    });
    Own {
        params: kit.params,
        clips: kit.clips.clone(),
        poses: kit.poses.clone(),
        clip_poses: Arc::clone(&kit.clip_poses),
        player_target_shape: kit.target_shape,
        sora_attack: game.sora_attack.clone(),
        magic,
        items,
        drive: game.drive.as_ref().map(|d| d.sibling()),
        ability_slots: game.ability_slots.clone(),
        equipment: game.equipment.clone(),
        gear: game.gear,
        selected_target: None,
        target_locked: false,
        cure_target: 0,
        weapon_pax: game.weapon_pax.clone(),
        weapon2_pax: game.weapon2_pax.clone(),
        blades,
        body: kit.body.clone(),
        fx_live: HashMap::new(),
        scans: Scans::default(),
    }
}

/// Exchanges a rival's state with the room's: called in pairs around his
/// tick.
pub(crate) fn swap(game: &mut Game, rival: &mut Rival) {
    use std::mem::swap as x;
    x(&mut game.sora, &mut rival.fighter);
    x(&mut game.action_started, &mut rival.started);
    let o = &mut *rival.own;
    x(&mut game.params, &mut o.params);
    x(&mut game.clips, &mut o.clips);
    x(&mut game.poses, &mut o.poses);
    x(&mut game.clip_poses, &mut o.clip_poses);
    x(&mut game.player_target_shape, &mut o.player_target_shape);
    x(&mut game.sora_attack, &mut o.sora_attack);
    x(&mut game.magic, &mut o.magic);
    x(&mut game.items, &mut o.items);
    x(&mut game.drive, &mut o.drive);
    x(&mut game.ability_slots, &mut o.ability_slots);
    x(&mut game.equipment, &mut o.equipment);
    x(&mut game.gear, &mut o.gear);
    x(&mut game.selected_target, &mut o.selected_target);
    x(&mut game.target_locked, &mut o.target_locked);
    x(&mut game.cure_target, &mut o.cure_target);
    x(&mut game.weapon_pax, &mut o.weapon_pax);
    x(&mut game.weapon2_pax, &mut o.weapon2_pax);
    x(&mut game.fx_live, &mut o.fx_live);
    x(&mut game.sound_frame, &mut o.scans.sound_frame);
    x(&mut game.move_se_scan, &mut o.scans.move_se);
    x(&mut game.step_scan, &mut o.scans.step);
    x(&mut game.dust_scan, &mut o.scans.dust);
    if let Some(v) = game.voices.as_mut() {
        x(&mut v.scan, &mut o.scans.voice);
    }
    if let Some(rt) = game.enemy.as_mut() {
        x(&mut rt.blade, &mut o.blades);
        x(&mut rt.sora_body, &mut o.body);
    }
}

impl Rival {
    fn new(
        game: &Game,
        slot: u8,
        player: Player,
        vitals: Vitals,
        control: Control,
    ) -> Option<Rival> {
        let kit = game.rival_kit.as_ref()?;
        Some(Rival {
            slot,
            fighter: Fighter { player, vitals },
            started: 0,
            control,
            pad: RivalPad::default(),
            own: Box::new(own_from(game, kit, slot)),
            floor: None,
            shown: Shown {
                part_indices: game.part_indices.clone(),
                weapon_indices: [game.weapon_indices.clone(), Vec::new()],
                ..Shown::default()
            },
        })
    }
}

/// A rival like the room Sora at `distance` in front of him, facing him,
/// with his battle numbers at full HP.
fn spawn(game: &mut Game, slot: u8, distance: f32, control: Control) -> Option<[f32; 3]> {
    let p = game.sora.player.position;
    let yaw = game.sora.player.yaw;
    let at = [
        p[0] + yaw.sin() * distance,
        p[1],
        p[2] + yaw.cos() * distance,
    ];
    let mut player = Player::new(at, yaw + std::f32::consts::PI);
    player.abilities = game.sora.player.abilities.clone();
    player.fmab = game.sora.player.fmab.clone();
    player.mp = magic::Mp::new(game.sora.player.mp.max);
    player.drive = game.sora.player.drive;
    let mut vitals = game.sora.vitals.clone();
    vitals.hp = vitals.target.max_hp;
    vitals.ko_ticks = 0;
    // A duel's Sora has a target's hit record (duel_loadout.rs).
    vitals.record = combat::HitRecord::new(!game.duel);
    let rival = Rival::new(game, slot, player, vitals, control)?;
    game.rivals.retain(|r| r.slot != slot);
    game.rivals.push(rival);
    game.rivals.sort_by_key(|r| r.slot);
    Some(at)
}

/// Puts a scripted rival in front of the room Sora. Returns false when one
/// is already there.
pub(crate) fn spawn_scripted(game: &mut Game) -> bool {
    if !game.rivals.is_empty() {
        return false;
    }
    let Some(at) = spawn(game, RIVAL_SLOT, SPAWN_DISTANCE, Control::Script) else {
        return false;
    };
    let tick = game.tick;
    game.event(
        serde_json::json!({"event":"rival_spawn","tick":tick,"slot":RIVAL_SLOT,
        "position":at,"control":"script"}),
    );
    true
}

/// The other player's Sora joins (duel_net): on the host in front of the
/// room Sora, facing him; on the joiner wherever the first snapshot puts
/// him.
pub(crate) fn spawn_remote(game: &mut Game, slot: u8) {
    let at = spawn(game, slot, DUEL_DISTANCE, Control::Remote);
    let tick = game.tick;
    game.event(
        serde_json::json!({"event":"rival_spawn","tick":tick,"slot":slot,
        "position":at,"control":"remote"}),
    );
}

/// `--rival-script`: the scripted rival is there from the start.
pub(crate) fn script_setup(mut game: ResMut<Game>, net: Option<Res<duel_net::DuelNet>>) {
    // In a LAN duel the flag scripts the joiner's buttons instead.
    if game.rival_script && net.is_none() {
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
    // A spell now and then, a potion, and a form change once.
    pad.command = match tick % 1200 {
        330 => Command::Cast(magic::FIRE),
        700 => Command::Cast(magic::THUNDER),
        900 => Command::Item(0),
        _ if tick == 1200 => Command::Drive(player::forms::VALOR),
        _ => Command::None,
    };
    pad
}

/// The spells the magic bench casts, in turn.
const BENCH_SPELLS: [u8; 6] = [
    magic::FIRE,
    magic::BLIZZARD,
    magic::THUNDER,
    magic::CURE,
    magic::MAGNET,
    magic::REFLECT,
];

/// [Test] `--duel-magic`: come within 250 of the other Sora and cast the
/// next spell every 150 ticks (`offset` staggers the two players).
pub(crate) fn magic_pad(tick: u64, offset: u64, me: [f32; 3], toward: [f32; 3]) -> RivalPad {
    let d = Vec2::new(toward[0] - me[0], toward[2] - me[2]);
    let t = tick + offset;
    RivalPad {
        direction: if d.length() > 250. {
            d.normalize_or_zero().to_array()
        } else {
            [0., 0.]
        },
        command: if t.is_multiple_of(150) {
            Command::Cast(BENCH_SPELLS[(t / 150) as usize % BENCH_SPELLS.len()])
        } else {
            Command::None
        },
        ..RivalPad::default()
    }
}

/// `KH2RW_RIVAL_BENCH` (a damage measurement): close in and swing without
/// pause, every press inside the combo's window.
pub(crate) fn bench_pad(tick: u64, me: [f32; 3], toward: [f32; 3]) -> RivalPad {
    let d = Vec2::new(toward[0] - me[0], toward[2] - me[2]);
    // Close in to a ground combo's reach and stand: a press from farther
    // starts the running lunge instead of the combo.
    RivalPad {
        direction: if d.length() > 140. {
            d.normalize_or_zero().to_array()
        } else {
            [0., 0.]
        },
        attack: d.length() < 160. && tick.is_multiple_of(6),
        ..RivalPad::default()
    }
}

/// A Sora's TARGET centre: the kit's TARGET shape on its bone this frame.
pub(crate) fn target_centre(
    shape: Option<&object_collision::Entry>,
    poses: &HashMap<(Action, bool), usize>,
    clip_poses: &[ClipPoses],
    clips: &Clips,
    player: &Player,
) -> [f32; 3] {
    let Some(shape) = shape else {
        return player.position;
    };
    let matrix = usize::try_from(shape.bone)
        .ok()
        .filter(|&b| b != 0x4000)
        .and_then(|bone| {
            let clip = clip_poses.get(*poses.get(&(player.action, player.armed))?)?;
            let frame = player.motion_frame(clips) as usize;
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

/// A rival's TARGET centre with his own kit.
pub(crate) fn rival_centre(r: &Rival) -> [f32; 3] {
    let o = &r.own;
    target_centre(
        o.player_target_shape.as_ref(),
        &o.poses,
        &o.clip_poses,
        &o.clips,
        &r.fighter.player,
    )
}

/// The input a rival's pad makes, aimed at `foe` (a TARGET shape) from his
/// own TARGET centre `me`.
fn input_of(
    pad: &RivalPad,
    me: [f32; 3],
    at: [f32; 3],
    foe: Option<ptya::Target>,
    reach: f32,
) -> Input {
    Input {
        direction: pad.direction,
        jump: pad.jump,
        jump_up: !pad.jump_held,
        attack: pad.attack,
        guard: pad.guard,
        guard_held: pad.guard_held,
        target: foe.map(|s| [s.centre[0] - at[0], s.centre[2] - at[2]]),
        hop_vector: foe.map(|s| {
            [
                s.centre[0] - me[0],
                s.centre[1] - me[1],
                s.centre[2] - me[2],
            ]
        }),
        target_reach: foe.map_or(0., |s| s.radius + reach),
        target_shape: foe,
        nearby: [foe, None, None, None],
        probe_blocked: false,
    }
}

/// One simulation tick of every rival, after the room Sora's (FixedUpdate,
/// in the field chain): his state swapped into the room's, his pad's
/// command run through the room's own requests, his move, his spells,
/// items and Drive form, then the effects he started put on his own
/// object.
pub(crate) fn tick(mut game: ResMut<Game>) {
    // The joiner's rivals stand where the host's snapshots put them.
    if game.rivals.is_empty() || game.net_client {
        return;
    }
    let game = &mut *game;
    let Some(kit) = game.rival_kit.clone() else {
        return;
    };
    let tick = game.tick;
    let bench = std::env::var_os("KH2RW_RIVAL_BENCH").is_some();
    // His foe: the room Sora's TARGET shape (taken before the swap).
    let sora_at = game.sora.player.position;
    let sora_shape = enemy_target::player_targetcentre(game).map(|centre| ptya::Target {
        centre,
        radius: kit.target_radius,
        height: kit.target_radius,
    });
    let mut rivals = std::mem::take(&mut game.rivals);
    for rival in &mut rivals {
        if rival.control == Control::Script {
            let at = rival.fighter.player.position;
            rival.pad = if bench {
                bench_pad(tick, at, sora_at)
            } else {
                script_pad(tick, at, sora_at)
            };
        }
        let pad = rival.pad;
        let me = rival_centre(rival);
        let input = input_of(
            &pad,
            me,
            rival.fighter.player.position,
            sora_shape,
            kit.target_radius,
        );
        let calls_before = game.fx_calls.len();
        let queue_before = game.fx_queue.len();
        let shown_before = rival.own.drive.as_ref().map(|d| d.shown);
        swap(game, rival);
        // His target for spells: the room Sora.
        game.selected_target = sora_shape.map(|s| s.centre);
        match pad.command {
            Command::None => {}
            Command::Cast(t) => {
                let mp = game.sora.player.mp.cur;
                let ok = magic_room::request(game, t, tick);
                let after = game.sora.player.mp.cur;
                game.event(serde_json::json!({"event":"duel_cast","tick":tick,
                    "slot":rival.slot,"magic":t,"ok":ok,"mp":[mp, after]}));
            }
            Command::Item(slot) => item_room::request(game, slot as usize, tick),
            Command::Drive(form) => {
                if let Some(d) = game.drive.as_mut() {
                    d.request = Some(form);
                }
            }
        }
        let step = {
            let Game {
                sora,
                params,
                clips,
                world,
                ..
            } = &mut *game;
            let world = &*world;
            match world {
                RoomWorld::Map(map) => {
                    let room = map.swap_floor_state(rival.floor.unwrap_or(area::FLOOR_NONE));
                    let step = sora.player.tick(&input, params, clips, world);
                    rival.floor = Some(map.swap_floor_state(room));
                    step
                }
                RoomWorld::Triangles(_) => sora.player.tick(&input, params, clips, world),
            }
        };
        magic_room::tick(game, tick);
        drive_room::shots(game);
        item_room::tick(game, tick);
        drive_room::tick(game, None);
        drive_room::limit_tick(game);
        drive_room::motion_effects(game);
        if let Some(d) = game.drive.as_mut() {
            // A duel's forms are loaded before they are needed.
            d.preload();
        }
        game.sora.vitals.invulnerable = (game.sora.vitals.invulnerable - 1.).max(0.);
        if let Some(action) = step.started {
            game.action_started = tick;
            let position = game.sora.player.position;
            game.event(serde_json::json!({"event":"rival_action","tick":tick,
                "slot":rival.slot,"action":format!("{action:?}"),"position":position}));
        }
        // His swings, voices, cloth sounds, footsteps on his own floor and
        // ground dust, as the room's Sora's.
        let room_floor = match &game.world {
            RoomWorld::Map(map) => {
                Some(map.swap_floor_state(rival.floor.unwrap_or(area::FLOOR_NONE)))
            }
            RoomWorld::Triangles(_) => None,
        };
        super::sora_sounds(game);
        super::sora_dust(game);
        if let (Some(room), RoomWorld::Map(map)) = (room_floor, &game.world) {
            map.swap_floor_state(room);
        }
        // The Drive state's own effect starts, for drive_fx on his object.
        let mut placed = Vec::new();
        if let Some(d) = game.drive.as_mut() {
            placed.extend(std::mem::take(&mut d.fx_requests));
        }
        swap(game, rival);
        let shown = rival.own.drive.as_ref().map(|d| d.shown);
        if shown != shown_before {
            game.event(serde_json::json!({"event":"rival_form","tick":tick,
                "slot":rival.slot,"form":shown}));
        }
        retag_calls(&mut game.fx_calls[calls_before..], rival.slot);
        for p in &mut game.fx_queue[queue_before..] {
            if matches!(p.anchor, drive_fx::Anchor::Sora) {
                p.anchor = drive_fx::Anchor::Rival(rival.slot);
            }
        }
        game.fx_queue
            .extend(placed.into_iter().map(|req| drive_fx::Placed {
                req,
                anchor: drive_fx::Anchor::Rival(rival.slot),
            }));
    }
    game.rivals = rivals;
    // The room Sora's forms load ahead too in a duel.
    if game.duel
        && let Some(d) = game.drive.as_mut()
    {
        d.preload();
    }
}

/// Effect calls a rival's tick made: his own object and his own keys.
pub(crate) fn retag_calls(calls: &mut [spell_fx::Call], slot: u8) {
    for c in calls {
        match &mut c.fx {
            magic::Fx::Start { key, bound, .. } => {
                *key = tag_key(*key, slot);
                if matches!(bound, Some(magic::Anchor::Caster | magic::Anchor::Party(0))) {
                    *bound = Some(magic::Anchor::Rival(slot));
                }
            }
            magic::Fx::Pos(key, _)
            | magic::Fx::Dir(key, _)
            | magic::Fx::Scale(key, _)
            | magic::Fx::Fade(key, _)
            | magic::Fx::Kill(key)
            | magic::Fx::LoopEnd(key) => *key = tag_key(*key, slot),
        }
    }
}

#[derive(Component)]
pub(crate) struct RivalRoot(pub u8);
#[derive(Component)]
pub(crate) struct RivalPart(pub u8, pub usize);
/// A rival's Keyblade hand: slot and hand (0, or 1 for a form's second).
#[derive(Component)]
pub(crate) struct RivalHand(pub u8, pub usize);
#[derive(Component)]
pub(crate) struct RivalWeaponPart(pub u8, pub usize, pub usize);

/// Gives each new rival copies of the room Sora's meshes (the same
/// materials) under the scene's root, a form's model when his form
/// changes, and removes the drawing of rivals that left.
#[allow(clippy::type_complexity, clippy::too_many_arguments)]
pub(crate) fn spawn_drawing(
    mut commands: Commands,
    mut game: ResMut<Game>,
    scene: Option<Res<SceneRoot>>,
    parts: Query<(&SoraPart, &Mesh3d, &MeshMaterial3d<FogMaterial>)>,
    weapon_parts: Query<(&WeaponPart, &Mesh3d, &MeshMaterial3d<FogMaterial>)>,
    roots: Query<(Entity, &RivalRoot)>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<FogMaterial>>,
    mut images: ResMut<Assets<Image>>,
) {
    let game = &mut *game;
    for (entity, root) in &roots {
        if !game.rivals.iter().any(|r| r.slot == root.0) {
            commands.entity(entity).despawn();
        }
    }
    let Some(scene) = scene else { return };
    for rival in game.rivals.iter_mut() {
        let respawn = rival.own.drive.as_mut().and_then(|d| d.respawn.take());
        if rival.shown.spawned && respawn.is_none() {
            continue;
        }
        let slot = rival.slot;
        for (entity, root) in &roots {
            if root.0 == slot {
                commands.entity(entity).despawn();
            }
        }
        rival.shown.spawned = true;
        rival.shown.key = None;
        rival.shown.blend_from = None;
        rival.shown.action = None;
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
        let hands = [0, 1].map(|h| {
            commands
                .spawn((
                    RivalHand(slot, h),
                    ChildOf(root),
                    Transform::default(),
                    Visibility::Hidden,
                ))
                .id()
        });
        // A form's model from the rival's own Drive state.
        let model = respawn.and_then(|form| {
            let d = rival.own.drive.as_ref()?;
            let (actor, weapon, weapon2) = d.models.get(&form)?;
            Some((
                actor,
                weapon,
                weapon2.as_ref(),
                d.weapons.get(&form).copied().unwrap_or(1),
            ))
        });
        if let Some((actor, weapon, weapon2, count)) = model {
            let o = &rival.own;
            let pose = o.clip_poses[o.poses[&(Action::Idle, false)]].frame(0);
            rival.shown.part_indices.clear();
            for (ordinal, entity) in spawn_model(
                &mut commands,
                actor,
                &pose,
                root,
                &mut meshes,
                &mut materials,
                &mut images,
            ) {
                rival
                    .shown
                    .part_indices
                    .push(actor.parts[ordinal].indices.clone());
                if let Some(entity) = entity {
                    commands.entity(entity).insert(RivalPart(slot, ordinal));
                }
            }
            let idle = &o.clip_poses[o.poses[&(Action::Idle, true)]];
            rival.shown.weapon_indices = [Vec::new(), Vec::new()];
            for (h, &hand) in hands.iter().enumerate().take(usize::from(count).min(2)) {
                let model = if h == 0 {
                    weapon
                } else {
                    weapon2.unwrap_or(weapon)
                };
                let frames = if h == 0 {
                    &idle.weapon_frames
                } else {
                    &idle.weapon2_frames
                };
                let pose = frames.first().cloned().unwrap_or_default();
                if pose.is_empty() {
                    continue;
                }
                for (ordinal, entity) in spawn_model(
                    &mut commands,
                    model,
                    &pose,
                    hand,
                    &mut meshes,
                    &mut materials,
                    &mut images,
                ) {
                    rival.shown.weapon_indices[h].push(model.parts[ordinal].indices.clone());
                    if let Some(entity) = entity {
                        commands
                            .entity(entity)
                            .insert(RivalWeaponPart(slot, h, ordinal));
                    }
                }
            }
            continue;
        }
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
        rival.shown.part_indices = game.part_indices.clone();
        rival.shown.weapon_indices = [game.weapon_indices.clone(), Vec::new()];
        for (part, mesh, material) in &weapon_parts {
            let Some(copy) = meshes.get(&mesh.0).cloned() else {
                continue;
            };
            commands.spawn((
                RivalWeaponPart(slot, 0, part.0),
                ChildOf(hands[0]),
                Mesh3d(meshes.add(copy)),
                material.clone(),
                Transform::default(),
            ));
        }
    }
}

/// Poses each rival from his own kit at the newest tick, with the motion
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
    let now = game.tick;
    for rival in &mut game.rivals {
        let p = &rival.fighter.player;
        let o = &rival.own;
        let Some(&clip) = o.poses.get(&(p.action, p.armed)) else {
            continue;
        };
        let Some(cp) = o.clip_poses.get(clip) else {
            continue;
        };
        let frame = p.motion_frame(&o.clips) as usize;
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
            if let Some(indices) = shown.part_indices.get(part.1) {
                let positions = indices
                    .iter()
                    .filter_map(|&i| display.get(i as usize).map(|v| v.to_array()));
                visual::room::set_positions(&mut meshes, &handle.0, positions);
            }
        }
        shown.display = display;
        for (h, mut transform, mut visibility) in &mut hands {
            if h.0 != rival.slot {
                continue;
            }
            let matrices = if h.1 == 0 { &cp.weapon } else { &cp.weapon2 };
            let has = !matrices.is_empty() && !shown.weapon_indices[h.1].is_empty();
            if let Some(m) = matrices.get(frame.min(matrices.len().saturating_sub(1))) {
                *transform = Transform::from_matrix(*m);
            }
            *visibility = if p.armed && has {
                Visibility::Inherited
            } else {
                Visibility::Hidden
            };
        }
        for (h, frames) in [&cp.weapon_frames, &cp.weapon2_frames]
            .into_iter()
            .enumerate()
        {
            let Some(pose) = frames.get(frame.min(frames.len().saturating_sub(1))) else {
                continue;
            };
            for (part, handle) in &weapon_parts {
                if part.0 != rival.slot || part.1 != h {
                    continue;
                }
                if let Some(indices) = shown.weapon_indices[h].get(part.2) {
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
        assert_eq!(
            script_pad(330, [0.; 3], [0.; 3]).command,
            Command::Cast(magic::FIRE)
        );
    }

    #[test]
    fn effect_keys_carry_the_slot() {
        assert_ne!(tag_key(5, 0), tag_key(5, 1));
        assert_eq!(tag_key(tag_key(77, 1), 1), 77);
        let mut calls = vec![spell_fx::Call {
            pax: None,
            fx: magic::Fx::Start {
                key: 9,
                id: 3,
                bound: Some(magic::Anchor::Caster),
                at: None,
            },
        }];
        retag_calls(&mut calls, 1);
        assert_eq!(
            calls[0].fx,
            magic::Fx::Start {
                key: tag_key(9, 1),
                id: 3,
                bound: Some(magic::Anchor::Rival(1)),
                at: None,
            }
        );
    }
}
