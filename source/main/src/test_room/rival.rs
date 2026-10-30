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
    /// [KH3 ruleset] The shotlock button (R2) is held, and the flowmotion
    /// button (Triangle) was pressed this tick (duel_kh3.rs).
    pub shotlock_held: bool,
    pub flow: bool,
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
            shotlock_held: false,
            flow: false,
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
    /// His swing trails (weapon_trail.rs): live, and each slot's blade length.
    pub trail_live: [bool; 2],
    pub trail_length: [Option<f32>; 2],
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
    pub(crate) shown: Shown,
}

#[derive(Default)]
pub(crate) struct Shown {
    key: Option<(usize, usize)>,
    target: Vec<Vec3>,
    display: Vec<Vec3>,
    action: Option<(Action, u64)>,
    blend_from: Option<(Vec<Vec3>, u64)>,
    spawned: bool,
    /// The model's parts' vertex indices, and each Keyblade's.
    pub(crate) part_indices: Vec<Vec<u32>>,
    pub(crate) weapon_indices: [Vec<Vec<u32>>; 2],
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
        drive: game.drive.as_ref().map(|d| {
            let mut d = d.sibling();
            d.offset_bullet_keys(slot);
            d
        }),
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
        trail_live: [false; 2],
        trail_length: [None; 2],
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
    x(&mut game.trail_live, &mut o.trail_live);
    x(&mut game.trail_length, &mut o.trail_length);
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
            fighter: Fighter {
                player,
                vitals,
                kh3: player::kh3::Kh3::default(),
                kh3_grand: None,
            },
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

/// [KH3 ruleset] The second Sora's KH3 script runs half a cycle later: one
/// Sora acts while the other takes it.
pub(crate) const KH3_SECOND: u64 = KH3_ACT;
/// The KH3 script's cycle: the acting half, then the half taking the other
/// Sora's moves.
const KH3_ACT: u64 = 3200;
const KH3_CYCLE: u64 = 2 * KH3_ACT;

/// What the KH3 script sees of its own Sora (on a joiner, as the host's
/// snapshot shows him): his guard, the counter, Final Blow and the Risk
/// Dodge offered.
#[derive(Clone, Copy, Debug, Default)]
pub(crate) struct Kh3View {
    pub guarding: bool,
    pub counter: bool,
    pub chase: bool,
    /// Knocked into the air by a heavy hit (Aerial Recovery may follow).
    pub flying: bool,
    /// The Risk Dodge is offered.
    pub risk: bool,
    /// The counter offered is the Risk Dodge's (longer than a guard's).
    pub risk_counter: bool,
    /// His own swing has landed (or met a guard): the launch may follow.
    pub connected: bool,
    /// The other Sora's swing and its frame.
    pub foe_swing: Option<u32>,
    /// The other Sora guards (or recoils from a guard).
    pub foe_guarding: bool,
}

/// [Test] The guard goes up from this tick of the other's swing (a joiner
/// sees it some ticks late).
const GUARD_LEAD: u32 = 1;

pub(crate) fn kh3_view(f: &Fighter, foe: Option<&Fighter>) -> Kh3View {
    Kh3View {
        guarding: matches!(f.player.action, Action::Guard | Action::GuardRecoil),
        counter: f.kh3.counter_open(),
        chase: f.kh3.chase_open(),
        risk: f.kh3.risk_open(),
        risk_counter: f.kh3.counter_ticks() > player::kh3::tuning::COUNTER_WINDOW,
        connected: matches!(f.player.action, Action::Attack(_)) && f.player.connected,
        flying: matches!(f.kh3.mode.name(), "blown" | "tossed"),
        foe_swing: foe
            .filter(|o| matches!(o.player.action, Action::Attack(_) | Action::AirAttack(_)))
            .map(|o| o.player.frame),
        foe_guarding: foe
            .is_some_and(|o| matches!(o.player.action, Action::Guard | Action::GuardRecoil)),
    }
}

/// [Test] The KH3 ruleset's script (`--kh3-test`, the scripted rival, and
/// the room Sora of an unattended KH3 run). Acting: a ground chain with the
/// launch and an air combo, a jump with Doubleflight and a glide, a dodge
/// roll, an air slide, a run along the X axis away from the arena's middle
/// into its wall (wall runs, the kick, Superslide, Sonic Slash), a full
/// shotlock (Ragnarok), an airstep with its flow attack, a chain to its
/// finisher, a short shotlock (King of Hearts), a chain into a guard, a
/// launch into a guard, then spells and the grand magic when offered. Taking: standing, Aerial
/// Recovery during the Ragnarok, and against the guarded chain a guard,
/// the Risk Dodge out of a guarded heavy hit, the counter and Final Blow.
/// Fixed by the tick (and what `view` shows) so a run repeats.
pub(crate) fn kh3_pad(
    tick: u64,
    offset: u64,
    me: [f32; 3],
    toward: [f32; 3],
    view: Kh3View,
) -> RivalPad {
    let d = Vec2::new(toward[0] - me[0], toward[2] - me[2]);
    let distance = d.length();
    let chase = d.normalize_or_zero();
    let side = Vec2::new(-chase.y, chase.x);
    let t = (tick + offset) % KH3_CYCLE;
    let mut pad = RivalPad::default();
    let mut stick = Vec2::ZERO;
    if t >= KH3_ACT {
        // Taking the other's moves (he is at `u` of his acting half).
        let u = t - KH3_ACT;
        match u {
            // Aerial Recovery once the Ragnarok's bolts toss him.
            1400..1600 => pad.jump = view.flying && u.is_multiple_of(4),
            // The guarded chain: Guard while he swings; out of a guard's
            // Jump while the Risk Dodge is offered (a heavy hit guarded), then
            // Attack while the counter or Final Blow is offered.
            2150..2700 => {
                // The second chain launches into the guard: only the Risk
                // Dodge's counter is taken there.
                let risk_only = u >= 2470;
                if view.chase || view.counter && (!risk_only || view.risk_counter) {
                    pad.attack = u.is_multiple_of(4);
                } else if view.risk {
                    pad.jump = true;
                } else if !view.guarding {
                    // Guard as his swing comes.
                    // (A joiner sees his frames through snapshots, a few at a
                    // time, and late.)
                    pad.guard = view
                        .foe_swing
                        .is_some_and(|f| (GUARD_LEAD..GUARD_LEAD + 8).contains(&f));
                }
            }
            _ => {}
        }
        pad.direction = [0., 0.];
        return pad;
    }
    // Outward along X: the arenas are round or square about the middle.
    let outward = Vec2::new(if me[0] < 0. { -1. } else { 1. }, 0.);
    match t {
        // Close in, then the ground chain; Jump just after a hit lands is
        // the launch.
        0..230 => {
            if distance > 170. {
                stick = chase;
            }
            pad.attack = distance < 260. && t.is_multiple_of(14);
            pad.jump = matches!(t, 196 | 200 | 204 | 215);
        }
        // The air combo after the launch.
        230..300 => pad.attack = t.is_multiple_of(12),
        // A jump, Doubleflight near the top of it, then the glide.
        330 => pad.jump = true,
        331..470 => {
            pad.jump = t == 353;
            pad.jump_held = t < 460;
            stick = side * 0.9;
        }
        // A dodge roll to the side, once landed.
        530 => {
            pad.guard = true;
            stick = side;
        }
        // A jump, then an air slide.
        580 => pad.jump = true,
        581..640 => {
            pad.jump_held = t < 590;
            if t == 596 {
                pad.guard = true;
                stick = -side;
            }
        }
        // Out to the wall and up it; Guard kicks off into a Superslide,
        // Attack in it is the Sonic Slash.
        660..1020 => {
            stick = outward;
            pad.guard = t == 1000;
        }
        1020..1060 => pad.attack = t == 1035,
        // A full shotlock: hold R2, fire at 24 locks (Ragnarok).
        1070..1270 => {
            pad.shotlock_held = t < 1250;
            pad.attack = t == 1240;
        }
        // Aim again and airstep (Guard), then the flow attack on arrival.
        1520..1600 => {
            pad.shotlock_held = t < 1542;
            pad.guard = t == 1538;
            pad.attack = t > 1544 && t.is_multiple_of(3);
        }
        // Close in and chain to the finisher (a blow, a knock-down).
        1620..1770 => {
            if distance > 170. {
                stick = chase;
            }
            pad.attack = distance < 260. && t.is_multiple_of(14);
        }
        // A short shotlock: a few locks, King of Hearts.
        1780..2050 => {
            pad.shotlock_held = t < 1822;
            pad.attack = t == 1818;
        }
        // A chain into the other's guard, and one into a guard of its
        // finisher (the Risk Dodge).
        2150..2450 | 2470..2700 => {
            if distance > 170. {
                stick = chase;
            }
            // The second chain: Jump once a swing met him (or his guard),
            // the launch, a heavy hit for his guard.
            pad.jump = t >= 2470 && view.connected;
            // Held off while he guards or swings, so his counter has him to
            // hit.
            pad.attack = distance < 260.
                && t.is_multiple_of(12)
                && !view.foe_guarding
                && view.foe_swing.is_none();
        }
        // Spells (style for the grand magic), then the grand magic once
        // it is offered.
        2720..KH3_ACT => {
            pad.command = match t {
                2740 | 2790 | 2840 | 2890 => Command::Cast(magic::FIRE),
                2940 => Command::Cast(magic::THUNDER),
                _ => Command::None,
            };
            pad.flow = t >= 3010 && t.is_multiple_of(10);
        }
        _ => {}
    }
    pad.direction = stick.clamp_length_max(1.).to_array();
    pad
}
/// What the magic bench does, in turn: the five spells that cost MP twice
/// (176 of the duel's 180), the first item slot, Cure (all the MP that is
/// left), then Valor Form.
const BENCH_COMMANDS: [Command; 13] = [
    Command::Cast(magic::FIRE),
    Command::Cast(magic::BLIZZARD),
    Command::Cast(magic::THUNDER),
    Command::Cast(magic::MAGNET),
    Command::Cast(magic::REFLECT),
    Command::Cast(magic::FIRE),
    Command::Cast(magic::BLIZZARD),
    Command::Cast(magic::THUNDER),
    Command::Cast(magic::MAGNET),
    Command::Cast(magic::REFLECT),
    Command::Item(0),
    Command::Cast(magic::CURE),
    Command::Drive(player::forms::VALOR),
];
/// Ticks between the bench's casts (a cast and its recovery fit).
const BENCH_GAP: u64 = 200;
/// The bench's first cast (after the lobby and the countdown).
const BENCH_START: u64 = 1000;

/// [Test] `--duel-magic`: come within 250 of the other Sora and take the
/// next bench command every BENCH_GAP ticks, once through the list
/// (`offset` staggers the two players).
pub(crate) fn magic_pad(tick: u64, offset: u64, me: [f32; 3], toward: [f32; 3]) -> RivalPad {
    let d = Vec2::new(toward[0] - me[0], toward[2] - me[2]);
    let t = tick + offset;
    RivalPad {
        direction: if d.length() > 250. {
            d.normalize_or_zero().to_array()
        } else {
            [0., 0.]
        },
        command: match t.checked_sub(BENCH_START) {
            Some(n) if n.is_multiple_of(BENCH_GAP) => BENCH_COMMANDS
                .get((n / BENCH_GAP) as usize)
                .copied()
                .unwrap_or_default(),
            _ => Command::None,
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
            } else if game.kh3_preview.is_some() {
                RivalPad::default()
            } else if game.duel_rules.kh3() {
                kh3_pad(
                    tick,
                    KH3_SECOND,
                    at,
                    sora_at,
                    kh3_view(&rival.fighter, Some(&game.sora)),
                )
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
                let ok = if super::duel_kh3::on(game) && game.kh3_book.is_some() {
                    super::duel_kh3::kh3_cast(game, t)
                } else {
                    magic_room::request(game, t, tick)
                };
                if ok && super::duel_kh3::on(game) {
                    game.sora.kh3.spell_released(t);
                }
                let after = game.sora.player.mp.cur;
                let levels = game.magic.as_ref().map(|m| m.levels);
                game.event(serde_json::json!({"event":"duel_cast","tick":tick,
                    "slot":rival.slot,"magic":t,"ok":ok,"mp":[mp, after],"levels":levels}));
            }
            Command::Item(slot) => item_room::request(game, slot as usize, tick),
            Command::Drive(form) => {
                if let Some(d) = game.drive.as_mut() {
                    d.request = Some(form);
                }
            }
        }
        // [KH3 ruleset] The layer over the KH2 controller (duel_kh3.rs).
        let kh3 = super::duel_kh3::on(game);
        let book = game.kh3_book.clone();
        let poles = if kh3 {
            super::duel_kh3::poles(game)
        } else {
            Arc::default()
        };
        let kh3_pad = player::kh3::Pad {
            shotlock: pad.shotlock_held,
            situation: pad.flow,
            ..Default::default()
        };
        let foe = sora_shape.map(|s| super::duel_kh3::foe(sora_at, s.centre));
        let mut kh3_out = None;
        let geometry = if kh3 {
            super::duel_kh3::geometry(game)
        } else {
            None
        };
        let foe_body = rival.fighter.kh3.core_foe_body(&rival.fighter.player);
        let view = super::duel_kh3::aim_view(game.sora.player.position, Some(sora_at));
        let step = {
            let Game {
                sora,
                params,
                clips,
                world,
                ..
            } = &mut *game;
            let world = &*world;
            let scene = player::kh3::Scene {
                foe,
                poles: &poles,
                book: book.as_deref(),
                geometry: geometry.as_deref(),
                foe_body,
                view,
            };
            let mut run = |sora: &mut Fighter| {
                if kh3 {
                    let out =
                        super::duel_kh3::step(sora, &input, &kh3_pad, params, clips, world, &scene);
                    let step = out.step;
                    kh3_out = Some(out);
                    step
                } else {
                    sora.player.tick(&input, params, clips, world)
                }
            };
            match world {
                RoomWorld::Map(map) => {
                    let room = map.swap_floor_state(rival.floor.unwrap_or(area::FLOOR_NONE));
                    let step = run(sora);
                    rival.floor = Some(map.swap_floor_state(room));
                    step
                }
                RoomWorld::Triangles(_) => run(sora),
            }
        };
        if let Some(out) = &kh3_out {
            super::duel_kh3::after(game, out, rival.slot);
            super::duel_kh3::bolts(game, rival.slot);
        }
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
    if kh3_model::shows_kh3(&game) {
        return;
    }
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
                transform.rotation = super::duel_kh3::turn(p.yaw, &rival.fighter.kh3);
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
