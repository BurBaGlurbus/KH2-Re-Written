//! The command manager's reaction command in the room (FUN_1403b2340): each
//! tick, before Sora's update, one search over everything that may fill the
//! Triangle slot (`player::reaction_command`, FUN_1404081c0), the slot it
//! keeps with its target marker (FUN_140408520), then Triangle. What wins
//! decides who carries a press out:
//!
//! - a battle actor's REACTION shape or attack: its script gets trigger 11
//!   (FUN_1403a8b10 → FUN_1403b5fa0) in the same tick;
//! - the Talk NPC's shape: the field talk starts (`talk_tick`);
//! - the manager's record 1 (a Limit Form limit's, a summon's or a party
//!   limit's follow-up): that module's follow-up;
//! - Auto Summon, an auto form or Auto Limit: the summon, the form or the
//!   limit starts as the deck's command would.
//!
//! A taken command is held (+0xdd8, FUN_1403a89a0) and shows in mode 8 until
//! another source outranks it or Sora's next command or hit clears it.
use super::*;
use enemy_gallery::{EnemyGallery, GalleryActor};
use player::reaction_command as rc;

/// Keys of the room's non-actor offerers in the search (actor ids are
/// small).
pub const SORA: u32 = 0xffff_0000;
pub const TALK: u32 = 0xffff_0001;
pub const SUMMON: u32 = 0xffff_0002;
/// Save point `k` of the area is `SAVE_POINT + k`.
pub const SAVE_POINT: u32 = 0xffff_0100;
/// Command 0x37 "Save" (exec 0x37).
const SAVE_COMMAND: u16 = 0x37;
/// Effect owners of the save points' glow (element 1, started with the
/// object, FUN_140418e30) and of the pulse while Save heals (element 2 with
/// se000 #29, FUN_140418a00).
pub const SAVE_GLOW: u8 = 0xf7;
pub const SAVE_PULSE: u8 = 0xf6;
/// FUN_140418f30: the heal's step (DAT_140717480, 1.0 a frame) and the
/// timer it fires at (DAT_140623b18, 2.0).
const SAVE_HEAL_STEP: f32 = 1.;
const SAVE_HEAL_AT: f32 = 2.;

/// The `--save-point-tour`: Sora stands on the area's first save point from
/// tick 20 with 1 MP (and the party at 1 HP), and presses Triangle at 240;
/// the file menu opens, Save, up to an empty slot, save there, back out.
/// [Tour setup.]
pub const SAVE_TOUR_CAPTURES: [u64; 8] = [30, 120, 245, 290, 350, 410, 436, 520];
pub const SAVE_TOUR_END: u64 = 640;
/// The tour's presses in the file menu (pad mode 7 logical bits).
const SAVE_TOUR_MENU: [(u64, u64); 5] =
    [(300, 0x4), (360, 0x10), (420, 0x4), (540, 0x8), (590, 0x8)];

/// The save point tour's file menu press at `tick`.
pub fn save_tour_logical(tick: u64) -> u64 {
    SAVE_TOUR_MENU
        .iter()
        .filter(|(at, _)| *at == tick)
        .fold(0, |acc, &(_, b)| acc | b)
}
/// The townspeople's and the test NPC's reaction (cmd 31 "Talk").
const TALK_COMMAND: u16 = 0x1f;

/// An auto ability's `item` record (03system, 0x18-byte rows): its kind
/// (+2), its status bit (+4) and its command (+0x10).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct AutoRecord {
    pub kind: u8,
    pub bit: u16,
    pub command: u16,
}

/// The command records and names the search reads (03system `cmd`), and the
/// auto abilities' item records.
pub struct Commands {
    pub rows: HashMap<u16, rc::Row>,
    pub names: HashMap<u16, Vec<u8>>,
    pub autos: HashMap<u16, AutoRecord>,
}

/// The records of 03system `cmd`, the names (messages) of the commands that
/// can show in the slot, and the auto items' records.
pub fn load(
    system: &[u8],
    names: impl FnOnce(&[u32]) -> Result<Vec<Option<Vec<u8>>>, String>,
) -> Result<Commands, String> {
    let table = data::command_table::read(bar_entry(system, b"cmd\0").map_err(|e| e.to_string())?)
        .map_err(|e| e.to_string())?;
    let rows: HashMap<u16, rc::Row> = table
        .iter()
        .map(|c| {
            (
                c.id,
                rc::Row {
                    id: c.id,
                    exec: c.execute,
                    argument: c.argument,
                    submenu: c.submenu,
                    flags: c.flags,
                    facing_cos: c.reaction.facing_cos,
                    target_yaw: c.reaction.target_yaw,
                    target_cos: c.reaction.target_cos,
                    cost: c.cost,
                    camera: c.reaction.camera,
                    priority: c.reaction.priority,
                    decider: c.decider,
                    chain: c.chain,
                    mark: c.reaction.mark,
                    action: c.reaction.action,
                    distance: c.reaction.distance,
                    form_mask: c.form_mask,
                    group: c.group,
                },
            )
        })
        .collect();
    let named: Vec<&data::command_table::Command> = table.iter().filter(|c| c.name != 0).collect();
    let ids: Vec<u32> = named.iter().map(|c| c.name).collect();
    let texts = names(&ids)?;
    let names = named
        .iter()
        .zip(texts)
        .filter_map(|(c, t)| Some((c.id, t?)))
        .collect();
    let mut autos = HashMap::new();
    if let Ok(items) = bar_entry(system, b"item") {
        let count = items
            .get(4..8)
            .map_or(0, |w| u32::from_le_bytes(w.try_into().unwrap()) as usize);
        let wanted: Vec<u16> = rc::AUTO_FORMS
            .iter()
            .chain(&rc::AUTO_SUMMONS)
            .copied()
            .chain([rc::AUTO_LIMIT, AUTO_SUMMON])
            .collect();
        for r in items
            .get(8..)
            .unwrap_or_default()
            .as_chunks::<0x18>()
            .0
            .iter()
            .take(count)
        {
            let id = u16::from_le_bytes([r[0], r[1]]);
            if wanted.contains(&id) {
                autos.insert(
                    id,
                    AutoRecord {
                        kind: r[2],
                        bit: u16::from_le_bytes([r[4], r[5]]),
                        command: u16::from_le_bytes([r[0x10], r[0x11]]),
                    },
                );
            }
        }
    }
    Ok(Commands { rows, names, autos })
}

/// The Auto Summon ability (item 0x185: status bit 56, FUN_140407a90).
pub const AUTO_SUMMON: u16 = 0x185;

/// The manager's reaction state across ticks.
pub struct State {
    pub slot: rc::Slot,
    /// +0xdd8 / +0xbf8: the command taken last and its target.
    pub hold: Option<(u16, rc::Target)>,
    /// The search's winner this tick.
    pub offer: Option<rc::Offer>,
    /// A command Triangle took this tick, for its source to carry out.
    pub press: Option<rc::Candidate>,
    /// `DAT_140783ca0` for the auto offers.
    pub seed: u32,
    /// The target point of each shape offered this tick (for the marker).
    pub points: Vec<(rc::Target, [f32; 3])>,
    /// The Talk NPC's label.
    pub talk_label: Vec<u8>,
    /// The last marker the run log recorded.
    pub marker_logged: u32,
    /// The actor whose script captured Sora last (the partner).
    pub captor: Option<u32>,
    /// The save point's heal timer (+0xc0c).
    pub save_timer: f32,
    /// The area whose save points have their glow started, and whether the
    /// pulse element runs (the save point's +0xc08).
    pub glow_area: Option<(u8, u8)>,
    pub pulse: bool,
    /// Sora asked for a command since the last tick (his update's request).
    pub request_made: bool,
}

impl Default for State {
    fn default() -> Self {
        Self {
            slot: rc::Slot::default(),
            hold: None,
            offer: None,
            press: None,
            seed: 1,
            points: Vec::new(),
            talk_label: Vec::new(),
            marker_logged: 0,
            captor: None,
            save_timer: 0.,
            glow_area: None,
            pulse: false,
            request_made: false,
        }
    }
}

impl State {
    /// The press this tick if it took `command` from `source`.
    pub fn pressed(&self, source: rc::Source) -> Option<u16> {
        self.press.filter(|p| p.source == source).map(|p| p.command)
    }

    /// The press this tick on a shape of `owner`.
    pub fn pressed_on(&self, owner: u32) -> Option<u16> {
        self.press
            .filter(|p| p.source == rc::Source::Shape && p.target.owner == owner)
            .map(|p| p.command)
    }

    /// The slot's command for the HUD.
    pub fn shown(&self) -> Option<rc::Candidate> {
        self.slot.shown.valid().then_some(self.slot.shown)
    }
}

/// The search's view of the room.
struct World<'a> {
    commands: &'a Commands,
    actors: &'a [GalleryActor],
    /// FUN_140401d60's cases with state: the room's colour of the Drive,
    /// summon and limit commands.
    modes: HashMap<u16, u8>,
    abilities: &'a player::abilities::Abilities,
    stock: Option<&'a std::collections::BTreeMap<u16, u8>>,
}

impl rc::Env for World<'_> {
    fn row(&self, id: u16) -> Option<rc::Row> {
        self.commands.rows.get(&id).copied()
    }

    fn exec_mode(&self, row: &rc::Row) -> u8 {
        self.modes
            .get(&row.id)
            .copied()
            .unwrap_or_else(|| rc::exec_mode(row))
    }

    fn offered(&self, shape: &rc::Shape) -> bool {
        if shape.owner >= SORA {
            // The Talk NPC's and the save points' shapes: their groups are
            // on by default (the table's mask), nothing switches them.
            return true;
        }
        self.actors
            .iter()
            .find(|a| a.id as u32 == shape.owner)
            .and_then(|a| a.battle.as_ref())
            .is_some_and(|b| {
                b.brain.reaction_offered(
                    i32::from(shape.command),
                    i32::from(shape.group),
                    b.assets.default_groups,
                )
            })
    }

    fn inactive(&self, owner: u32) -> bool {
        if owner >= SORA {
            return false;
        }
        self.actors
            .iter()
            .find(|a| a.id as u32 == owner)
            .and_then(|a| a.battle.as_ref())
            .is_none_or(|b| b.error.is_some() || !b.brain.reaction_active())
    }

    /// FUN_1403e39a0 with FUN_1403d7470 (an ability's status bit) and
    /// FUN_1403c4720 (an item owned).
    fn auto_item(&self, item: u16) -> Option<rc::AutoItem> {
        let r = self.commands.autos.get(&item)?;
        let held = match r.kind {
            0x13 => self.abilities.has(r.bit),
            _ => self
                .stock
                .is_some_and(|s| s.get(&item).is_some_and(|&n| n > 0)),
        };
        Some(rc::AutoItem {
            kind: r.kind,
            held,
            command: r.command,
        })
    }
}

/// Sora's FUN_1403ba070 state as the room keeps it: an ANB range 0..2 or a
/// reaction's tail for the window, range 26 for the state, also of the
/// motions a summon's limit captures him in (Genie's 543 chains to the 535
/// of capture 253, Stitch's 527 to the 600 of his).
fn sora_actor(game: &Game) -> rc::Actor {
    rc::Actor {
        no_control: game.player.no_control(&game.clips),
        state: game.player.reaction_state(&game.clips),
        window: game.player.can_command(&game.clips),
        ..Default::default()
    }
}

/// FUN_1403b2340's reaction part, every tick before Sora's update.
pub fn tick(
    mut game: ResMut<Game>,
    gallery: Option<ResMut<EnemyGallery>>,
    talk: Option<Res<Talk>>,
    hud: Option<ResMut<Hud>>,
    area: Option<Res<AreaRes>>,
) {
    let tick = game.tick;
    let mut gallery = gallery;
    let actors: &mut [GalleryActor] = match gallery.as_mut() {
        Some(g) => &mut g.actors,
        None => &mut [],
    };
    let Some(commands) = game.reaction_commands.take() else {
        return;
    };
    let mut state = std::mem::take(&mut game.reaction);
    state.press = None;
    if let Some(t) = talk.as_deref() {
        state.talk_label.clone_from(&t.label);
    }
    // The hold ends with Sora's next request (FUN_1403a89a0 clears +0xdd8
    // for any request before it holds a Triangle one), and with his object's
    // reset or KO (FUN_1403a7330, slot 0 of the player classes; FUN_1403a81c0
    // from the KO handlers, e.g. FUN_140404d60 at Sora's vt+0xb0). A talk
    // does not end it (FUN_1403a8b10 case 0x1f and FUN_140401410 leave
    // +0xdd8 alone).
    if state.hold.is_some() && (state.request_made || game.player.dead()) {
        state.hold = None;
    }
    state.request_made = false;
    let menu_kind = hud
        .as_ref()
        .and_then(|h| h.menu.as_ref())
        .map(|m| match m.menu.sub.as_ref() {
            Some(sub) => sub.kind as u8,
            None => m.menu.kind as u8,
        });
    let save_points = area
        .as_deref()
        .map_or(&[][..], |a| a.current.save_points.as_slice());
    // The place a save written here records.
    if !save_points.is_empty() {
        game.save_place = area.as_deref().map(|a| (a.current.world, a.current.area));
    }
    if game.save_point_tour
        && let Some(&(at, _)) = save_points.first()
    {
        if tick >= 20 {
            game.player.position = (at + Vec3::Z * 20.).to_array();
        }
        if tick == 20 {
            game.player.mp.cur = 1;
            for m in &mut game.party {
                m.hp = 1;
            }
        }
        if tick == 240 {
            game.rc_queued = true;
        }
        if tick == SAVE_TOUR_END - 1 {
            let saved = game.files_saved;
            let pass = saved > 0;
            game.autoplay_failed |= !pass;
            game.event(
                serde_json::json!({"event":"save_point_tour_verdict","tick":tick,
                "pass":pass,"files_saved":saved}),
            );
        }
    }
    let mut inputs = inputs(&game, actors, talk.as_deref(), &state, menu_kind);
    // The area's save points (REACTION entries of SAVE_POINT, command 0x37).
    for (k, &(at, e)) in save_points.iter().enumerate() {
        inputs.shapes.push(rc::Shape {
            owner: SAVE_POINT + k as u32,
            group: e.group,
            command: rc::shape_command(5, e.argument as u16, 0, None),
            centre: (at + Vec3::Y * f32::from(e.position_height)).to_array(),
            radius: f32::from(e.radius),
            height: f32::from(e.height),
            owner_yaw: 0.,
            attack: None,
        });
    }
    let world = World {
        commands: &commands,
        actors,
        modes: exec_modes(&game, &commands),
        abilities: &game.player.abilities,
        stock: game.items.as_ref().map(|i| &i.stock),
    };
    let offer = rc::search(&world, &inputs, &mut state.seed);
    // FUN_140408bd0: a source other than the hold clears it.
    if offer.is_some_and(|o| o.candidate.source != rc::Source::Hold) {
        state.hold = None;
    }
    // FUN_140408520: +0xde0 == 6 (Antiform) shows the slot dimmed.
    let force_dim = game
        .drive
        .as_ref()
        .is_some_and(|d| d.active && d.form == player::forms::ANTI);
    state.slot.update(
        &world,
        offer,
        rc::SlotState {
            force_dim,
            ..Default::default()
        },
    );
    state.points = inputs
        .shapes
        .iter()
        .chain(&inputs.attacks)
        .map(|s| (s.target(), s.centre))
        .collect();
    // FUN_140408520: while "Save" shows usable, FUN_140418f30 heals.
    let healing = state.slot.shown.command == SAVE_COMMAND && state.slot.shown.mode < 4;
    if healing {
        save_heal(&mut game, &mut state.save_timer);
    }
    save_point_effects(&mut game, &mut state, area.as_deref(), healing, offer);
    if offer.map(|o| o.command()) != state.offer.map(|o| o.command()) {
        if offer.is_some() {
            game.offers_seen += 1;
        }
        game.event(serde_json::json!({"event":"reaction_offer","tick":tick,
            "command":offer.map(|o| o.command()),"owner":offer.map(|o| o.candidate.target.owner),
            "source":offer.map(|o| format!("{:?}", o.candidate.source)),
            "priority":offer.map(|o| o.priority),"mode":offer.map(|o| o.candidate.mode)}));
    }
    state.offer = offer;
    // Triangle (logical bit 10), and the tours' presses on their own offers.
    let shown = state.shown();
    let triangle = std::mem::take(&mut game.rc_queued) || tour_press(&game, shown, tick);
    let row = shown.and_then(|c| commands.rows.get(&c.command).copied());
    let sora = sora_actor(&game);
    let battle = inputs.sora.battle;
    let press = rc::press(
        &mut state.slot,
        rc::Manager::default(),
        triangle,
        row.as_ref(),
        |row| {
            // FUN_1403b6950: Sora's windows, the target's (the room's
            // others take commands at any time [I]), or both.
            let target = rc::Actor {
                window: true,
                ..Default::default()
            };
            rc::usable(row, &sora, Some(&target), battle)
        },
    );
    match press {
        Some(rc::Press::Taken { command, .. }) => {
            let c = state.slot.shown;
            state.press = Some(c);
            // FUN_14015d660: the HUD's burst.
            if let Some(mut hud) = hud
                && let Some(m) = hud.menu.as_mut()
            {
                m.menu.burst();
            }
            // FUN_1403a89a0: the taken command is held unless its record's
            // flag bit 9 says not to.
            if let Some(row) = row {
                state.hold = rc::hold_after(&row, true).map(|cmd| (cmd, c.target));
            }
            game.event(serde_json::json!({"event":"reaction_taken","tick":tick,
                "command":command,"source":format!("{:?}", c.source),"owner":c.target.owner}));
            carry_out(&mut game, actors, c, tick);
        }
        Some(rc::Press::Refused) => game.sound_queue.push((5, "menu_denied")),
        Some(rc::Press::Ignored) => {
            // [Run log] Why a shown command was not taken.
            let action = format!(
                "{:?} frame {}",
                game.player.action,
                game.player.motion_frame(&game.clips)
            );
            game.event(serde_json::json!({"event":"reaction_ignored","tick":tick,
                "command":shown.map(|c| c.command),"mode":shown.map(|c| c.mode),
                "flags":row.map(|r| r.flags),"decider":row.map(|r| r.decider),
                "chain":row.map(|r| r.chain),"sora":format!("{sora:?}"),
                "action":action}));
        }
        _ => {}
    }
    game.reaction = state;
    game.reaction_commands = Some(commands);
}

/// The search's inputs from the room.
fn inputs(
    game: &Game,
    actors: &[GalleryActor],
    talk: Option<&Talk>,
    state: &State,
    menu_kind: Option<u8>,
) -> rc::Inputs {
    let mut shapes = Vec::new();
    for actor in actors {
        let Some(battle) = actor.battle.as_ref() else {
            continue;
        };
        if battle.error.is_some() || !battle.brain.reaction_active() {
            continue;
        }
        for &(ref shape, kind, argument) in &battle.pose.reactions {
            // Kind 14 offers the command a clashed attack left (+0xcf0) and
            // argument 0x83 the spawn record's: [U] neither is kept by the
            // room's actors (no clash command, no spawn record).
            if kind != data::object_collision::REACTION {
                continue;
            }
            let Ok(argument) = u16::try_from(argument) else {
                continue;
            };
            let command = rc::shape_command(5, argument, 0, None);
            if command == 0x83 {
                continue;
            }
            shapes.push(rc::Shape {
                owner: actor.id as u32,
                group: shape.group,
                command,
                centre: shape.centre.to_array(),
                radius: shape.radius,
                height: shape.height,
                owner_yaw: actor.yaw,
                attack: None,
            });
        }
    }
    // FUN_140407b40: attacks carrying a command (`attack_set_rc`), a sphere
    // at the attack's position plus its offset, offered for the object that
    // owns it. [U] An owner set by `attack_set_rc_owner` to another object
    // is not resolved to that actor; the facing gate turns with the actor.
    let mut attacks = Vec::new();
    for actor in actors {
        let Some(battle) = actor.battle.as_ref() else {
            continue;
        };
        if battle.error.is_some() {
            continue;
        }
        for a in battle.brain.script_attacks() {
            let Some(r) = a.rc.filter(|r| r.own) else {
                continue;
            };
            let at = Vec3::from_array(a.position) + Vec3::from_array(r.offset);
            attacks.push(rc::Shape {
                owner: actor.id as u32,
                group: 0,
                command: r.command,
                centre: at.to_array(),
                radius: r.radius,
                height: r.height,
                owner_yaw: actor.yaw,
                attack: Some(a.serial as u32),
            });
        }
    }
    // The Talk NPC's REACTION shape while nobody talks: a townsperson's own
    // entry (kind 5; argument 0x83 takes its spawn record's command, +0x2c,
    // FUN_140407de0), centred on the root raised by its height offset [I:
    // the root bone, the offset turned by the NPC's heading]; the test NPC
    // keeps the measured sphere (r 200, h 200, 200 up) and command 31.
    if let Some(t) = talk
        && t.active.is_none()
        && !game.talking
    {
        let shape = match t.reaction_shape {
            Some(e) => {
                let offset = Quat::from_rotation_y(t.yaw)
                    * Vec3::new(
                        f32::from(e.position[0]),
                        -f32::from(e.position[1]),
                        f32::from(e.position[2]),
                    );
                let spawn = (t.spawn_command != 0).then_some(t.spawn_command);
                let command = rc::shape_command(5, e.argument as u16, 0, spawn);
                (command != 0x83).then(|| rc::Shape {
                    owner: TALK,
                    group: e.group,
                    command,
                    centre: (t.position + offset + Vec3::Y * f32::from(e.position_height))
                        .to_array(),
                    radius: f32::from(e.radius),
                    height: f32::from(e.height),
                    owner_yaw: t.yaw,
                    attack: None,
                })
            }
            None => Some(rc::Shape {
                owner: TALK,
                group: 0,
                command: TALK_COMMAND,
                centre: (t.position + Vec3::Y * REACTION_SPHERE[2]).to_array(),
                radius: REACTION_SPHERE[0],
                height: REACTION_SPHERE[1],
                owner_yaw: t.yaw,
                attack: None,
            }),
        };
        shapes.extend(shape);
    }
    let p = Vec3::from_array(game.player.position);
    let drive = game.drive.as_ref();
    let battle = game.enemy.is_some() || second_deck::battle(game);
    let sora = rc::Sora {
        centre: (p + Vec3::Y * SORA_OBJ[2]).to_array(),
        radius: SORA_OBJ[0],
        height: SORA_OBJ[1],
        yaw: game.player.yaw,
        battle,
        no_control: game.player.no_control(&game.clips),
        state: sora_actor(game).state,
        form: drive.filter(|d| d.active).map_or(0, |d| d.form),
        // FUN_1403b6a10 refuses everything for +0xde0 == 0xc, a state the
        // room's forms never take [U: which]; Antiform (6) only dims the
        // slot (FUN_140408520, below).
        anti: false,
        class: 1,
        // +0x6cb bit 0 with +0xe14: a limit runs (Limit Form's limits run
        // as limit scenes too, T210).
        limit: game.party_limit.is_some(),
        won: game.battle_won,
        leader: true,
        hold: state.hold,
        // Range 25 of Sora's motion (+0x18c bit 20, +0x638) with the
        // partner (+0x2b0).
        partner: game.player.partner_command(&game.clips).zip(state.captor),
        no_shapes: game.party_limit.is_some(),
        ..Default::default()
    };
    let sora_target = rc::Target {
        owner: SORA,
        ..Default::default()
    };
    // The lock-on target: the actor whose TARGET point is the one held.
    let locked = game
        .selected_target
        .filter(|_| game.target_locked)
        .and_then(|at| {
            actors.iter().find_map(|a| {
                let t = a.battle.as_ref()?.pose.targets.first()?;
                (t.centre.distance(Vec3::from_array(at)) < 1.).then_some(a.id as u32)
            })
        });
    // The manager's record 1: whichever of a limit scene (a party limit or,
    // since T210, a Limit Form limit) or a summon offered last tick.
    let offered = game
        .party_limit
        .as_ref()
        .and_then(|l| l.offers[1])
        .map(|c| (c, sora_target))
        .or_else(|| {
            summon::offers(game).and_then(|o| o[1]).map(|c| {
                (
                    c,
                    rc::Target {
                        owner: SUMMON,
                        ..Default::default()
                    },
                )
            })
        });
    rc::Inputs {
        sora,
        sora_target,
        shapes,
        attacks,
        offered,
        shown: state.shown().map(|c| (c.command, c.target)),
        locked,
        auto: auto(game, menu_kind),
    }
}

/// The auto tail's inputs (FUN_1404081c0 after the shapes).
fn auto(game: &Game, menu_kind: Option<u8>) -> rc::Auto {
    let hp = game
        .enemy
        .as_ref()
        .map(|rt| (rt.sora_hp, rt.sora_target.max_hp));
    let members: Vec<_> = game.party.iter().filter(|m| !m.stored).collect();
    let sora = Vec3::from_array(game.player.position);
    // FUN_1403e7950: the limits the deck lists usable whose members carry
    // Auto Limit and stand within 500 of Sora. The room's members carry no
    // ability record: their Auto Limit counts as equipped [test-room choice,
    // as the Limit list's].
    let limits = game
        .second_deck
        .as_ref()
        .map(|sd| {
            let room = second_deck::room(game, None);
            sd.limit_list(&room)
                .into_iter()
                .filter(|&(_, colour)| colour < 4)
                .filter_map(|(cmd, _)| {
                    let row = sd.limits.iter().find(|l| l.command == cmd)?;
                    let member = members.iter().position(|m| {
                        m.kind.character == row.members[1]
                            && Vec3::from_array(m.follower.position).distance(sora)
                                <= rc::AUTO_LIMIT_DISTANCE
                    })?;
                    Some((
                        cmd,
                        rc::Target {
                            owner: member as u32,
                            ..Default::default()
                        },
                    ))
                })
                .collect()
        })
        .unwrap_or_default();
    rc::Auto {
        menu_kind,
        blocked: false,
        status: hp.is_some(),
        auto_summon: game.player.abilities.has(56),
        member_down: members.iter().any(|m| m.hp <= 0),
        hp_low: hp.is_some_and(|(cur, max)| cur <= max / 4),
        // FUN_1403e75d0: Sora up, no party limit, in battle, no MP charge.
        limit_ready: game.party_limit.is_none()
            && second_deck::battle(game)
            && !game.player.mp_charging
            && hp.is_some_and(|(cur, _)| cur > 0),
        limits,
    }
}

/// FUN_140401d60's cases that read the room: the Drive command of a form
/// (case 4, the Drive submenu's colour), the summons (0x48) and the limits
/// (0x2c3) as the second deck colours them.
fn exec_modes(game: &Game, commands: &Commands) -> HashMap<u16, u8> {
    let mut modes = HashMap::new();
    if let Some(d) = game.drive.as_ref() {
        for (&id, row) in &commands.rows {
            if row.exec == 4 && (1..6).contains(&row.argument) {
                let ok = d.selectable(row.argument as u8, &game.player.drive);
                modes.insert(id, if ok { 0 } else { rc::MODE_DIM });
            }
        }
    }
    if let Some(sd) = game.second_deck.as_ref() {
        let room = second_deck::room(game, None);
        for (cmd, colour) in sd
            .summon_list(&room)
            .into_iter()
            .chain(sd.limit_list(&room))
        {
            modes.insert(cmd, colour.min(9) as u8);
        }
    }
    modes
}

/// The tours' Triangle presses, each on its own source's offer as before.
fn tour_press(game: &Game, shown: Option<rc::Candidate>, tick: u64) -> bool {
    let Some(c) = shown else { return false };
    match c.source {
        rc::Source::Offered if game.party_limit.is_some() => {
            // The autoplay taps a Limit Form limit's follow-up every 10
            // ticks (T210).
            let form = game.autoplay.is_some()
                && tick.is_multiple_of(10)
                && game
                    .party_limit
                    .as_ref()
                    .is_some_and(|r| player::limit_form::LIMITS.contains(&r.command));
            form || game.deck_tour && deck_tour::reaction(tick)
                || game.limit_tour && limit_tour::reaction(tick)
        }
        rc::Source::Offered if c.target.owner == SUMMON => {
            game.summon_tour && summon_tour::mash(tick)
        }
        rc::Source::Offered => game.autoplay.is_some(),
        rc::Source::Shape if c.target.owner == TALK => {
            game.autoplay.is_some() && AUTOPLAY_TALK.contains(&tick)
        }
        rc::Source::Shape => {
            game.sephiroth_tour
                && super::sephiroth::tour_reaction(
                    tick,
                    game.player.action.airborne(),
                    game.offers_seen,
                )
        }
        _ => false,
    }
}

/// The request's execution (FUN_1403d6fc0's default branch for a reaction
/// row). Talk and the record-1 follow-ups are carried out by their modules
/// from [`State::press`].
fn carry_out(game: &mut Game, actors: &mut [GalleryActor], c: rc::Candidate, tick: u64) {
    match c.source {
        rc::Source::Shape | rc::Source::Partner if c.target.owner < SORA => {
            // Sora's update (FUN_1403a89a0) runs the request the same tick
            // (FUN_1403d6fc0 → vt+0x138 → FUN_1403a8b10); the press already
            // tested his windows (FUN_1403b6950). FUN_1401b15f0, which takes
            // a request only in a command window, is the party members'
            // update (+0xe18 flags), not his.
            if let Some(battle) = actors
                .iter_mut()
                .find(|a| a.id as u32 == c.target.owner)
                .and_then(|a| a.battle.as_mut())
            {
                // FUN_1403b5fa0: trigger 11 on the target's script.
                battle.brain.command(c.command);
                let action = format!("{:?}", game.player.action);
                let frame = game.player.frame;
                game.event(serde_json::json!({"event":"reaction_executed","tick":tick,
                    "command":c.command,"owner":c.target.owner,"sora_action":action,"sora_frame":frame}));
            }
        }
        // FUN_1403a8b10 case 0x37: the save point's menu (FUN_1403a5650(7),
        // FUN_1402e4950); the camp hosts it.
        rc::Source::Shape if c.command == SAVE_COMMAND => {
            game.save_menu_wanted = true;
            game.event(
                serde_json::json!({"event":"save_menu_requested","tick":tick,
                "save_point":c.target.owner - SAVE_POINT}),
            );
        }
        rc::Source::Auto(item) => {
            let Some(row) = game
                .reaction_commands
                .as_ref()
                .and_then(|r| r.rows.get(&c.command))
                .copied()
            else {
                return;
            };
            match row.exec {
                // An auto form: the Drive command of its form.
                4 => {
                    if let Some(d) = game.drive.as_mut() {
                        d.request = Some(row.argument as u8);
                    }
                }
                // Auto Summon: the summon's command.
                0x48 => {
                    if let Err(e) = summon::start(game, c.command, tick) {
                        game.event(serde_json::json!({"event":"auto_summon_refused",
                            "tick":tick,"command":c.command,"error":e}));
                    }
                }
                // Auto Limit: the limit's command.
                _ => game.limit_request = Some(c.command),
            }
            game.event(serde_json::json!({"event":"reaction_auto","tick":tick,
                "item":item,"command":c.command}));
        }
        _ => {}
    }
}

/// The slot's marker for the HUD: its creation serial, `fiel` group and the
/// target point (Sora's body for his own offers).
pub fn marker(game: &Game) -> Option<(u32, usize, [f32; 3])> {
    let (serial, group) = game.reaction.slot.marker_group()?;
    let target = game.reaction.slot.shown.target;
    let at = game
        .reaction
        .points
        .iter()
        .find(|(t, _)| *t == target)
        .map(|(_, at)| *at)
        .unwrap_or_else(|| {
            let p = Vec3::from_array(game.player.position);
            (p + Vec3::Y * SORA_OBJ[2]).to_array()
        });
    Some((serial, usize::from(group), at))
}

/// The slot as the command menu shows it (FUN_14015d670's inputs).
pub fn menu_reaction(game: &Game) -> Option<field_command_menu::Reaction> {
    // The room hides the slot while a field talk runs.
    if game.talking {
        return None;
    }
    let c = game.reaction.shown()?;
    let commands = game.reaction_commands.as_ref()?;
    let row = commands.rows.get(&c.command)?;
    Some(field_command_menu::Reaction {
        command: c.command,
        label: commands
            .names
            .get(&c.command)
            .or((c.target.owner == TALK).then_some(&game.reaction.talk_label))
            .cloned()
            .unwrap_or_default(),
        mode: c.mode,
        flag3: row.flags & 8 != 0,
        restart: c.flags & 2 != 0,
    })
}

/// The save points' effects: each starts its element 1 when the area loads
/// (FUN_140418e30); while the heal sets a save point's pulse (+0xc08,
/// FUN_140418f30) FUN_140418a00 keeps its element 2 running (se000 #29) and
/// ends it once the pulse is gone. An area change ends them all.
fn save_point_effects(
    game: &mut Game,
    state: &mut State,
    area: Option<&AreaRes>,
    healing: bool,
    offer: Option<rc::Offer>,
) {
    let place = area.map(|a| (a.current.world, a.current.area));
    if place != state.glow_area {
        if state.glow_area.is_some() {
            game.fx_release.extend([SAVE_GLOW, SAVE_PULSE]);
        }
        state.glow_area = place;
        state.pulse = false;
        if let Some(a) = area
            && let Some(pax) = &a.current.save_point_pax
        {
            for &(at, _) in &a.current.save_points {
                game.fx_queue.push(drive_fx::Placed {
                    req: drive_fx::FxRequest {
                        pax: pax.clone(),
                        id: 1,
                        owner: SAVE_GLOW,
                    },
                    anchor: drive_fx::Anchor::World {
                        at,
                        pitch: 0.,
                        yaw: 0.,
                    },
                });
            }
        }
    }
    let point = offer
        .map(|o| o.candidate.target.owner)
        .filter(|&o| o >= SAVE_POINT)
        .and_then(|o| area?.current.save_points.get((o - SAVE_POINT) as usize))
        .map(|&(at, _)| at);
    let pulse = healing && point.is_some();
    if pulse
        && !state.pulse
        && let (Some(at), Some(pax)) = (point, area.and_then(|a| a.current.save_point_pax.clone()))
    {
        game.fx_queue.push(drive_fx::Placed {
            req: drive_fx::FxRequest {
                pax,
                id: 2,
                owner: SAVE_PULSE,
            },
            anchor: drive_fx::Anchor::World {
                at,
                pitch: 0.,
                yaw: 0.,
            },
        });
        let tick = game.tick;
        game.event(serde_json::json!({"event":"save_point_pulse","tick":tick,"on":true}));
    } else if !pulse && state.pulse {
        game.fx_release.push(SAVE_PULSE);
        let tick = game.tick;
        game.event(serde_json::json!({"event":"save_point_pulse","tick":tick,"on":false}));
    }
    state.pulse = pulse;
}

/// FUN_140418f30: the save point's heal. Its timer gains 1.0 a frame; at
/// 2.0 each party member gets 1 HP and 1 MP (FUN_1403c0c30 / FUN_1403c0cd0,
/// which shortens a running MP charge instead) unless already full, a member
/// whose object is not out is refilled at once (FUN_1403c2040 /
/// FUN_1403c0860), and the timer starts again.
fn save_heal(game: &mut Game, timer: &mut f32) {
    *timer += SAVE_HEAL_STEP;
    if *timer < SAVE_HEAL_AT {
        return;
    }
    *timer = 0.;
    let mut healed = 0;
    // Sora (his HP is kept by the battle runtime, when there is one).
    let hp_full = game
        .enemy
        .as_ref()
        .is_none_or(|rt| rt.sora_hp >= rt.sora_target.max_hp);
    let mp = &game.player.mp;
    if !(hp_full && !mp.charging() && mp.cur >= mp.max) {
        if let Some(rt) = game.enemy.as_mut() {
            rt.sora_hp = (rt.sora_hp + 1).min(rt.sora_target.max_hp);
        }
        game.player.mp.add(1);
        healed += 1;
    }
    for m in &mut game.party {
        if m.stored {
            m.hp = m.max_hp;
            m.mp.cur = m.mp.max;
            continue;
        }
        let full = m.hp >= m.max_hp && !m.mp.charging() && m.mp.cur >= m.mp.max;
        if !full {
            m.hp = (m.hp + 1).min(m.max_hp);
            m.mp.add(1);
            healed += 1;
        }
    }
    if healed > 0 {
        let tick = game.tick;
        let hp = game.enemy.as_ref().map(|rt| rt.sora_hp);
        let mp = game.player.mp.cur;
        game.event(serde_json::json!({"event":"save_heal","tick":tick,"sora_hp":hp,"sora_mp":mp}));
    }
}
