//! The duel's lobby in the room: the host keeps the lobby (duel_lobby.rs)
//! and sends it on every change; the joiner sends its choices and shows
//! what comes back. On screen the lobby is the game's own pause menu (the
//! camp) opened in a duel mode: its top level has Duel (the Config screen
//! with the duel's pages: Match, Bans and Ready), and while the builds are
//! made the camp's own Abilities and Items (Equip); its art, panels, cursor,
//! help line, transitions and sounds are the game's. The builds are read
//! live from the open camp, over the duel's inventory with the match's
//! banned abilities taken out. Both Soras stand still until the match.

use super::duel_lobby::{self, Intent, Limits, Loadout, Lobby, MAX_BANS, Stage};
use super::duel_net::{DuelNet, Status, fighter_slots};
use super::duel_wire::{INTENT, LOBBY, REFUSED};
use super::*;
use net::{RELIABLE, Role};
use visual::pause_menu::{DuelPages, DuelRow};

/// se000: open, refused.
const SE_OPEN: u32 = 3;
const SE_ERROR: u32 = 5;

/// Sora's AP in a duel [duel rule: level 99 with every AP Boost]: the
/// LVUP row's 66 at level 99 plus the save's AP bonus at its cap of 100
/// (FUN_1403c1d90 / FUN_1403eda00, the cap in FUN_1403c0950; Standard
/// difficulty). The 61 boosts the game hands out once would give 129; the
/// cap takes the Skill Recipe's repeatable ones (private trace `ap-max-01`).
pub(crate) const DUEL_AP: i32 = 166;
/// Type-2 records that are not Sora's Keyblades: the Struggle weapons (the
/// camp's menu flag 0x77), and the unused Detection Saber, Frontier of
/// Ultima, Antiform Dummy and unnamed 0x51 (the last has no weapon model
/// the kit can load).
const NOT_KEYBLADES: [u16; 7] = [0x180, 0x1f5, 0x1f6, 0x2c, 0x2d, 0x47, 0x51];
/// Item table categories: consumable, Keyblade, ability.
const KIND_CONSUMABLE: u8 = 0;
const KIND_KEYBLADE: u8 = 2;
const KIND_ABILITY: u8 = 0x13;
/// How long a refusal stays in the help line.
const NOTE_TICKS: u64 = 300;

/// The camp's state the duel replaces, put back when it ends.
struct SavedCamp {
    ability_slots: Vec<u16>,
    equipment: pause_menu::items::Equipment,
    stock: std::collections::BTreeMap<u16, u8>,
    ap: i32,
    weapon_ability: u16,
}

/// What a duel row does when Left / Right steps it.
#[derive(Clone, Copy, Debug, PartialEq)]
enum RowKind {
    Mode,
    Damage,
    Magic,
    HitCap,
    Time,
    Ban(usize),
    ConfirmBans,
    Keyblade,
    Ready,
    /// Shown only.
    Info,
}

#[derive(Resource, Default)]
pub(crate) struct LobbyRoom {
    /// The host's lobby; on the joiner, the last one received.
    pub lobby: Lobby,
    pub limits: Limits,
    /// The lobby is on screen (between matches).
    pub up: bool,
    /// The session the lobby belongs to is running.
    active: bool,
    /// This player's seat (0 host, 1 joiner).
    seat: usize,
    /// This player's two bans being chosen (indices into the ban pool;
    /// 0 = none).
    picks: [usize; MAX_BANS],
    /// A refusal or notice and the tick it came.
    note: Option<(String, u64)>,
    /// Host: the lobby the joiner has.
    sent: Option<Lobby>,
    /// The build last sent.
    built: Option<Loadout>,
    /// The stage this instance last prepared for.
    prepared: Option<(u32, Stage)>,
    /// The match whose builds were prepared (after its Bans camp closed).
    builds_for: Option<u32>,
    /// Ticks in the current stage (the unattended script's clock).
    stage_ticks: u64,
    saved: Option<SavedCamp>,
    /// Ability and Keyblade names (game text) by item id.
    names: HashMap<u16, Vec<u8>>,
    /// The match put the Soras at the arena's starts: the camera turns
    /// behind this player's Sora once it is there (where it stood before).
    recenter: Option<[f32; 3]>,
    /// The section the other player was told.
    section_sent: Option<u8>,
}

impl LobbyRoom {
    /// The lobby holds the pad (its camp is up, or about to be).
    pub(crate) fn menu_up(&self) -> bool {
        self.up
    }
}

/// The duel's limits from the installed item table and the duel build.
fn limits_of(game: &Game, camp: &CampRes) -> (Limits, HashMap<u16, Vec<u8>>) {
    let table = &camp.source.items.table;
    let mut names = HashMap::new();
    let mut cost = std::collections::BTreeMap::new();
    let mut keyblades = Vec::new();
    let mut items = Vec::new();
    for r in table {
        match r.kind {
            KIND_ABILITY => {
                cost.insert(r.id, i32::from(r.param >> 8));
                names.insert(r.id, r.name.clone());
            }
            KIND_KEYBLADE if !NOT_KEYBLADES.contains(&r.id) => {
                keyblades.push(r.id);
                names.insert(r.id, r.name.clone());
            }
            KIND_CONSUMABLE
                if game
                    .items
                    .as_ref()
                    .is_some_and(|i| i.stock.contains_key(&r.id)) =>
            {
                items.push(r.id);
            }
            _ => {}
        }
    }
    // Sora's abilities: the slots that hold an ability record.
    let abilities = game
        .ability_slots
        .iter()
        .map(|s| s & 0x7fff)
        .filter(|s| cost.contains_key(s))
        .collect();
    let limits = Limits {
        abilities,
        cost,
        ap_max: DUEL_AP,
        keyblades,
        items,
        item_slots: game.items.as_ref().map_or(0, |i| i.slots.len()),
    };
    (limits, names)
}

/// This instance's build as the field has it now.
fn build_of(game: &Game) -> Loadout {
    Loadout {
        keyblade: game.equipment.weapon,
        abilities: game
            .ability_slots
            .iter()
            .copied()
            .filter(|s| s & 0x7fff != 0)
            .collect(),
        items: game
            .items
            .as_ref()
            .map(|i| i.slots.clone())
            .unwrap_or_default(),
    }
}

/// The builds open: the banned abilities leave Sora's slots (the camp
/// cannot show them), the rest keep what was equipped while it fits the
/// AP limit, and the camp shows the duel's numbers and stock.
fn prepare_builds(room: &mut LobbyRoom, game: &mut Game, camp: &mut CampRes) {
    let banned = room.lobby.banned();
    let mut slots: Vec<u16> = game
        .ability_slots
        .iter()
        .copied()
        .filter(|s| {
            let id = s & 0x7fff;
            room.limits.cost.contains_key(&id) && !banned.contains(&id)
        })
        .collect();
    // The pool's abilities a ban did not take, as many as Sora has.
    let mut pool = room.limits.slots_without(&banned);
    for s in &slots {
        if let Some(i) = pool.iter().position(|p| *p == s & 0x7fff) {
            pool.swap_remove(i);
        }
    }
    slots.extend(pool);
    // Unequip from the end until the equipped abilities fit.
    while room.limits.ap_used(&slots) > room.limits.ap_max {
        match slots.iter().rposition(|s| s & 0x8000 != 0) {
            Some(i) => slots[i] &= 0x7fff,
            None => break,
        }
    }
    let mut padded = slots.clone();
    padded.resize(game.ability_slots.len().max(slots.len()), 0);
    game.ability_slots = padded;
    drive_room::rebuild_abilities(game, None);
    if room.saved.is_none() {
        room.saved = Some(SavedCamp {
            ability_slots: camp.ability_slots.clone(),
            equipment: camp.equipment.clone(),
            stock: camp.stock.clone(),
            ap: camp.source.numbers.ap,
            weapon_ability: camp.source.weapon_ability,
        });
    }
    camp.ability_slots = game.ability_slots.clone();
    camp.equipment = game.equipment.clone();
    // The duel's stock: its consumables, one of every Keyblade, and the
    // element items at the spell levels the duel gives.
    let mut stock = game
        .items
        .as_ref()
        .map(|i| i.stock.clone())
        .unwrap_or_default();
    for &k in &room.limits.keyblades {
        stock.insert(k, 1);
    }
    if let Some(m) = game.magic.as_ref() {
        for (t, item) in ELEMENT_ITEMS.iter().enumerate() {
            stock.insert(*item, m.levels[t]);
        }
    }
    // The battle's stock too: the camp shows it and writes it back on
    // close, and the spell levels follow its element items.
    if let Some(it) = game.items.as_mut() {
        it.stock.clone_from(&stock);
    }
    camp.stock = stock;
    camp.source.numbers.ap = room.limits.ap_max - i32::from(game.gear.ap);
    if let Some(rt) = game.enemy.as_mut() {
        rt.numbers_dirty = true;
    }
}

/// What a Keyblade adds (Strength, Magic, ...) on this Sora.
fn keyblade_gear(game: &Game, k: u16) -> player::equipment_stats::Totals {
    let mut e = game.equipment.clone();
    e.weapon = k;
    game.ability_source.gear(&e, None)
}

/// The camp's state from before the duel comes back.
fn restore_camp(room: &mut LobbyRoom, camp: &mut CampRes) {
    if let Some(s) = room.saved.take() {
        camp.ability_slots = s.ability_slots;
        camp.equipment = s.equipment;
        camp.stock = s.stock;
        camp.source.numbers.ap = s.ap;
        camp.source.weapon_ability = s.weapon_ability;
    }
}

/// A build the host accepted goes on the Sora it belongs to: the host's
/// own on the field, the joiner's on his rival state.
fn apply_build(game: &mut Game, slot: u8, build: &Loadout) {
    if slot == game.local_slot {
        put_build(game, build);
        return;
    }
    let mut rivals = std::mem::take(&mut game.rivals);
    if let Some(r) = rivals.iter_mut().find(|r| r.slot == slot) {
        rival::swap(game, r);
        put_build(game, build);
        rival::swap(game, r);
    }
    game.rivals = rivals;
}

/// A build on the Sora swapped in: ability slots, item slots, Keyblade.
fn put_build(game: &mut Game, build: &Loadout) {
    let mut slots = build.abilities.clone();
    slots.resize(game.ability_slots.len().max(slots.len()), 0);
    game.ability_slots = slots;
    if let Some(it) = game.items.as_mut() {
        it.slots.clone_from(&build.items);
        it.originals.clone_from(&build.items);
    }
    if game.equipment.weapon != build.keyblade {
        game.equipment.weapon = build.keyblade;
        if let Some(d) = game.drive.as_mut() {
            d.keyblades_changed(build.keyblade);
        }
    }
    drive_room::rebuild_abilities(game, None);
}

/// Sends an intent: the host applies it to its own seat, the joiner sends
/// it to the host.
fn intend(room: &mut LobbyRoom, net: &mut DuelNet, game: &mut Game, intent: Intent) {
    if net.role == Role::Host {
        let limits = room.limits.clone();
        if let Err(why) = room.lobby.apply(0, intent, &limits) {
            room.note = Some((why, game.tick));
            game.sound_queue.push((SE_ERROR, "lobby"));
        }
    } else {
        net.session
            .send(RELIABLE, duel_lobby::encode_intent(INTENT, &intent));
    }
}

/// One fixed step of the lobby: messages, stage changes, the camp.
pub(crate) fn tick(
    net: Option<ResMut<DuelNet>>,
    room: Option<ResMut<LobbyRoom>>,
    camp: Option<ResMut<CampRes>>,
    mut rig: ResMut<CameraRig>,
    mut game: ResMut<Game>,
) {
    let (Some(mut net), Some(mut room), Some(mut camp)) = (net, room, camp) else {
        return;
    };
    // The camera behind Sora once the arena's start has reached him (the
    // joiner's comes with a snapshot).
    if let Some(before) = room.recenter {
        let p = game.sora.player.position;
        let moved = (p[0] - before[0]).hypot(p[2] - before[2]) > 1.;
        if moved || net.role == Role::Host {
            rig.recenter_queued = true;
            room.recenter = None;
        }
    }
    let (net, room, camp, game) = (&mut *net, &mut *room, &mut *camp, &mut *game);
    let playing = matches!(net.status, Status::Playing);
    if !playing {
        if room.active {
            // The session ended (the other player left, even mid-menu): an
            // open camp closes without writing the duel's build back, and
            // the camp and the field are the room's again.
            camp.open = None;
            camp.journal = None;
            camp.curtain = None;
            restore_camp(room, camp);
            *room = LobbyRoom {
                names: std::mem::take(&mut room.names),
                ..LobbyRoom::default()
            };
            game.duel_frozen = false;
            game.duel_camp = None;
        }
        return;
    }
    if !room.active {
        let (limits, names) = limits_of(game, camp);
        *room = LobbyRoom {
            limits,
            names,
            active: true,
            up: true,
            seat: usize::from(net.role == Role::Join),
            ..LobbyRoom::default()
        };
        // Slots that hold no ability record (the room's catalog) stay out.
        let other: Vec<u16> = game
            .ability_slots
            .iter()
            .map(|s| s & 0x7fff)
            .filter(|s| *s != 0 && !room.limits.cost.contains_key(s))
            .collect();
        let blades: Vec<[i32; 3]> = room
            .limits
            .keyblades
            .iter()
            .map(|&k| {
                let g = keyblade_gear(game, k);
                [i32::from(k), i32::from(g.strength), i32::from(g.magic)]
            })
            .collect();
        game.event(serde_json::json!({"event":"lobby_open","tick":game.tick,
            "abilities":room.limits.abilities.len(),"keyblades":room.limits.keyblades.len(),
            "ap":room.limits.ap_max,"not_abilities":other,"keyblade_stats":blades}));
        // The camp's status panel shows the duel's Sora (level 99).
        if let Some(rt) = game.enemy.as_mut() {
            rt.numbers_dirty = true;
        }
    }
    // Messages.
    for m in std::mem::take(&mut net.inbox) {
        match (net.role, m.first().copied()) {
            (Role::Host, Some(INTENT)) => {
                let Some(intent) = duel_lobby::decode_intent(INTENT, &m) else {
                    continue;
                };
                let limits = room.limits.clone();
                if let Err(why) = room.lobby.apply(1, intent, &limits) {
                    let mut w = net::wire::Writer::default();
                    w.u8(REFUSED);
                    w.text(&why, 120);
                    net.session.send(RELIABLE, w.finish());
                }
            }
            (Role::Join, Some(LOBBY)) => {
                if let Some(lobby) = duel_lobby::decode_lobby(LOBBY, &m) {
                    room.lobby = lobby;
                }
            }
            (Role::Join, Some(REFUSED)) => {
                let mut r = net::wire::Reader::new(&m);
                let _ = r.u8();
                if let Some(why) = r.text(120) {
                    room.note = Some((why, game.tick));
                    game.sound_queue.push((SE_ERROR, "lobby"));
                }
            }
            _ => net.inbox.push(m),
        }
    }
    // Stage changes.
    let key = (room.lobby.match_no, room.lobby.stage);
    if room.prepared != Some(key) {
        room.prepared = Some(key);
        room.stage_ticks = 0;
        room.picks = [0; MAX_BANS];
        game.event(serde_json::json!({"event":"lobby_stage","tick":game.tick,
            "stage":format!("{:?}", key.1),"match":key.0,"banned":room.lobby.banned()}));
        match key.1 {
            Stage::Bans | Stage::Loadout => room.up = true,
            Stage::Start => {
                // Both builds go on their Soras; the match begins (the camp
                // closes and writes this player's own build back too).
                for (seat, slot) in [(0, fighter_slots::HOST), (1, fighter_slots::JOIN)] {
                    if let Some(b) = room.lobby.seats[seat].loadout.clone() {
                        apply_build(game, slot, &b);
                    }
                }
                // The match (duel_match_room.rs) refills and places them.
                room.recenter = Some(game.sora.player.position);
                room.up = false;
            }
        }
    }
    room.stage_ticks += 1;
    // [Duel choice] While the builds are made both Soras stand armed, so
    // the Keyblade each picks is in his hand (the host's state; the joiner
    // sees it in the snapshots).
    if net.role == Role::Host && room.up && room.lobby.stage == Stage::Loadout {
        game.sora.player.armed = true;
        for r in &mut game.rivals {
            r.fighter.player.armed = true;
        }
    }
    game.duel_frozen = room.up;
    run_camp(room, net, camp, game);
    if room.up && game.autoplay.is_some() {
        script(room, net, camp, game);
    }
    // Host: the lobby to the joiner on every change.
    if net.role == Role::Host && room.sent.as_ref() != Some(&room.lobby) {
        room.sent = Some(room.lobby.clone());
        net.session
            .send(RELIABLE, duel_lobby::encode_lobby(LOBBY, &room.lobby));
    }
}

/// The lobby's camp: opened for the stage (closed first when its entries
/// change, so it writes its build back before the next stage's builds are
/// prepared), its pages filled from the lobby, this player's steps and
/// build taken from it.
fn run_camp(room: &mut LobbyRoom, net: &mut DuelNet, camp: &mut CampRes, game: &mut Game) {
    let builds = room.lobby.stage == Stage::Loadout;
    // One version of the lobby's camp a match and stage (the builds add
    // the camp's Abilities and Items); a vote takes the camp meanwhile.
    let version = room.lobby.match_no * 2 + u32::from(builds);
    let want = (room.up && !game.duel_vote_up).then_some(version);
    // The builds start from the duel's inventory once the camp before them
    // has closed (its close writes the camp back).
    let free = camp.open.is_none() && game.duel_camp.is_none();
    if builds && want.is_some() && free && room.builds_for != Some(room.lobby.match_no) {
        room.builds_for = Some(room.lobby.match_no);
        prepare_builds(room, game, camp);
        let build = build_of(game);
        room.built = Some(build.clone());
        intend(room, net, game, Intent::Loadout(build));
    }
    let asked = game.duel_camp.is_some();
    // The pages as they stand, for a camp about to open.
    let first = (want.is_some() && camp.open.is_none() && !asked).then(|| {
        let ap = room.limits.ap_used(&build_of(game).abilities);
        let (names, rows, _) = pages(room, net, game, ap);
        DuelPages {
            names,
            rows,
            ..DuelPages::default()
        }
    });
    let menu = duel_camp::hold(camp, game, duel_camp::LOBBY, want, || {
        let mut entries = vec![duel_camp::entry(
            7,
            "Duel",
            "The match's rules, the bans and Ready.",
        )];
        if builds {
            entries.extend([duel_camp::Item::Own(0), duel_camp::Item::Own(1)]);
        }
        duel_camp::Request {
            entries,
            on_config: true,
            pages: first.unwrap_or_default(),
        }
    });
    if !asked && game.duel_camp.is_some() {
        game.sound_queue.push((SE_OPEN, "lobby"));
    }
    let Some(menu) = menu else {
        return;
    };
    // This player's build as the camp has it now (its Abilities and Equip
    // pages), sent when it changes; a new Keyblade goes on Sora at once.
    let live = (builds).then(|| {
        let mut b = build_of(game);
        if let Some(m) = menu.data.abilities.members.first() {
            b.abilities = m
                .slots
                .iter()
                .copied()
                .filter(|s| s & 0x7fff != 0)
                .collect();
        }
        if let Some(e) = menu.data.items.equipment.first() {
            b.keyblade = e.weapon;
            b.items.clone_from(&e.items);
        }
        b
    });
    if let Some(b) = &live {
        if b.keyblade != game.equipment.weapon {
            game.equipment.weapon = b.keyblade;
            if let Some(d) = game.drive.as_mut() {
                d.keyblades_changed(b.keyblade);
            }
            game.refresh_gear();
        }
        if room.built.as_ref() != Some(b) {
            room.built = Some(b.clone());
            intend(room, net, game, Intent::Loadout(b.clone()));
        }
    }
    // Where this player is, for the other's screen.
    let section = match menu.state {
        pause_menu::State::Config => duel_lobby::SECTION_BANS,
        pause_menu::State::Top => duel_lobby::SECTION_LIST,
        _ => duel_lobby::SECTION_CAMP,
    };
    if room.section_sent != Some(section) {
        room.section_sent = Some(section);
        intend(room, net, game, Intent::Section(section));
    }
    // This player's steps, then the pages as they now stand.
    let steps = menu
        .data
        .duel
        .as_mut()
        .map(|d| std::mem::take(&mut d.steps))
        .unwrap_or_default();
    let ap = live
        .as_ref()
        .map(|b| room.limits.ap_used(&b.abilities))
        .unwrap_or(0);
    let (_, _, kinds) = pages(room, net, game, ap);
    for (page, row, forward) in steps {
        if let Some(&kind) = kinds.get(page).and_then(|r| r.get(row)) {
            step(room, net, game, menu, kind, forward);
        }
    }
    let (names, rows, _) = pages(room, net, game, ap);
    if let Some(d) = menu.data.duel.as_mut() {
        d.names = names;
        d.rows = rows;
    }
}

/// Left / Right on a duel row.
fn step(
    room: &mut LobbyRoom,
    net: &mut DuelNet,
    game: &mut Game,
    menu: &mut pause_menu::Camp,
    kind: RowKind,
    forward: bool,
) {
    let s = if forward { 1 } else { -1 };
    let rules_before = game.duel_rules;
    match kind {
        RowKind::Mode => game.duel_rules.cycle_mode(s),
        RowKind::Damage => game.duel_rules.step_damage(s),
        RowKind::Magic => game.duel_rules.step_magic(s),
        RowKind::HitCap => game.duel_rules.cycle_cap(s),
        RowKind::Time => game.duel_rules.cycle_time(s),
        RowKind::Ban(i) => {
            let n = room.limits.ban_pool().len() + 1;
            room.picks[i] = (room.picks[i] as i32 + s).rem_euclid(n as i32) as usize;
        }
        RowKind::ConfirmBans => {
            let pool = room.limits.ban_pool();
            let mut bans: Vec<u16> = room
                .picks
                .iter()
                .filter(|&&p| p > 0)
                .filter_map(|&p| pool.get(p - 1).copied())
                .collect();
            bans.sort_unstable();
            bans.dedup();
            intend(room, net, game, Intent::Ban(bans));
        }
        RowKind::Keyblade => {
            let blades = &room.limits.keyblades;
            let at = blades
                .iter()
                .position(|&k| k == game.equipment.weapon)
                .unwrap_or(0) as i32;
            if !blades.is_empty() {
                let next = blades[(at + s).rem_euclid(blades.len() as i32) as usize];
                if let Some(e) = menu.data.items.equipment.first_mut() {
                    e.weapon = next;
                }
            }
        }
        RowKind::Ready => {
            let ready = !room.lobby.seats[room.seat].ready;
            intend(room, net, game, Intent::Ready(ready));
        }
        RowKind::Info => {}
    }
    if game.duel_rules != rules_before {
        room.lobby.rules_changed();
        let tick = game.tick;
        let rules = game.duel_rules;
        game.event(serde_json::json!({"event":"duel_rules","tick":tick,
            "mode":rules.info().name,"damage":rules.damage,"magic":rules.magic,
            "hit_cap":rules.hit_cap,"time_limit":rules.time_limit}));
    }
}

/// Text in the system font's encoding; game text (item names) is used as
/// it is.
fn text(s: &str) -> Vec<u8> {
    pause_menu::encode_text(s)
}

/// An item's name in game text (a number when the table has none).
fn name(room: &LobbyRoom, id: u16) -> Vec<u8> {
    room.names
        .get(&id)
        .cloned()
        .unwrap_or_else(|| text(&format!("? {id}")))
}

/// Where a player is, in words.
fn section_words(seat: &duel_lobby::Seat, stage: Stage) -> &'static str {
    if seat.ready {
        return "Ready";
    }
    match (stage, seat.section) {
        (Stage::Bans, _) if seat.bans_done => "Bans chosen",
        (_, duel_lobby::SECTION_BANS) => "On the Duel pages",
        (_, duel_lobby::SECTION_CAMP) => "In Abilities or Items",
        (Stage::Bans, _) => "Choosing bans",
        _ => "Making a build",
    }
}

fn row(
    label: &str,
    choices: Vec<Vec<u8>>,
    selected: usize,
    help: Vec<u8>,
    locked: bool,
) -> DuelRow {
    DuelRow {
        label: text(label),
        choices,
        selected,
        help,
        locked,
    }
}

/// Page names, their rows, and what each row does.
type Pages = (Vec<Vec<u8>>, Vec<Vec<DuelRow>>, Vec<Vec<RowKind>>);

/// The damage rows' choices: x0.5 to x10 by 0.5.
fn multipliers() -> Vec<Vec<u8>> {
    let n = ((duel_rules::DAMAGE_RANGE.1 - duel_rules::DAMAGE_RANGE.0) / duel_rules::DAMAGE_STEP)
        as usize
        + 1;
    (0..n)
        .map(|i| {
            text(&format!(
                "x{}",
                duel_rules::DAMAGE_RANGE.0 + duel_rules::DAMAGE_STEP * i as f32
            ))
        })
        .collect()
}

/// A multiplier's place among the choices.
fn multiplier_at(k: f32) -> usize {
    ((k - duel_rules::DAMAGE_RANGE.0) / duel_rules::DAMAGE_STEP).round() as usize
}

/// The duel's pages for this player: names, rows, and what each row does.
fn pages(room: &LobbyRoom, net: &DuelNet, game: &Game, ap: i32) -> Pages {
    let host = net.role == Role::Host;
    let them = net.peer.as_deref().unwrap_or("Opponent");
    let rules = game.duel_rules;
    let stage = room.lobby.stage;
    let mine = &room.lobby.seats[room.seat];
    let theirs = &room.lobby.seats[1 - room.seat];
    // A refusal stands in every row's help line for a while.
    let note = room
        .note
        .as_ref()
        .filter(|(_, at)| game.tick.saturating_sub(*at) < NOTE_TICKS)
        .map(|(n, _)| duel_camp::help_text(n));
    let help = |h: &str| note.clone().unwrap_or_else(|| duel_camp::help_text(h));
    let by_host = if host { "" } else { " The host sets it." };
    let mut names = vec![text("Match"), text("Bans")];
    let mut rows = Vec::new();
    let mut kinds = Vec::new();
    // Match: the host's rules and where the other player is.
    let mut r = vec![
        row(
            "Mode",
            duel_rules::MODES.iter().map(|m| text(m.name)).collect(),
            usize::from(rules.mode_code()),
            help(&format!(
                "One KO. Best of 3: two rounds win. Time limit: more HP left wins.{by_host}"
            )),
            !host,
        ),
        row(
            "Damage",
            multipliers(),
            multiplier_at(rules.damage),
            help(&format!(
                "Attacks' damage is the game's times this.{by_host}"
            )),
            !host,
        ),
        row(
            "Magic damage",
            multipliers(),
            multiplier_at(rules.magic),
            help(&format!(
                "Spells, Reflect's counter and form magic: the game's damage times this.{by_host}"
            )),
            !host,
        ),
        row(
            "Hit cap",
            duel_rules::HIT_CAPS
                .iter()
                .map(|&c| text(&duel_net::cap_text(c)))
                .collect(),
            duel_rules::HIT_CAPS
                .iter()
                .position(|&c| c == rules.hit_cap)
                .unwrap_or(0),
            help(&format!(
                "No single hit takes more than this share of max HP.{by_host}"
            )),
            !host,
        ),
    ];
    let mut k = vec![
        RowKind::Mode,
        RowKind::Damage,
        RowKind::Magic,
        RowKind::HitCap,
    ];
    if rules.info().timed {
        r.push(row(
            "Time limit",
            duel_rules::TIME_LIMITS
                .iter()
                .map(|&t| text(&duel_rules::clock(u32::from(t) * 60)))
                .collect(),
            duel_rules::TIME_LIMITS
                .iter()
                .position(|&t| t == rules.time_limit)
                .unwrap_or(0),
            help(&format!("How long a round lasts.{by_host}")),
            !host,
        ));
        k.push(RowKind::Time);
    }
    r.push(row(
        them,
        vec![text(section_words(theirs, stage))],
        0,
        help(&format!("Where {them} is.")),
        true,
    ));
    k.push(RowKind::Info);
    rows.push(r);
    kinds.push(k);
    // Bans: two choices each, for both players; then shown.
    let pool = room.limits.ban_pool();
    let choosing = stage == Stage::Bans && !mine.bans_done;
    let mut r = Vec::new();
    let mut k = Vec::new();
    for i in 0..MAX_BANS {
        let (choices, selected) = if choosing {
            let mut c = vec![text("None")];
            c.extend(pool.iter().map(|&id| name(room, id)));
            (c, room.picks[i])
        } else {
            (
                vec![
                    mine.bans
                        .get(i)
                        .map_or_else(|| text("None"), |&b| name(room, b)),
                ],
                0,
            )
        };
        r.push(row(
            &format!("Ban {}", i + 1),
            choices,
            selected,
            help("An ability neither player can equip this match. Left and Right choose."),
            !choosing,
        ));
        k.push(RowKind::Ban(i));
    }
    r.push(row(
        "Bans",
        vec![text(if mine.bans_done { "Chosen" } else { "Confirm" })],
        0,
        help("Right confirms your bans. None is a choice too."),
        !choosing,
    ));
    k.push(RowKind::ConfirmBans);
    let theirs_text = if !theirs.bans_done {
        text("Choosing")
    } else if theirs.bans.is_empty() {
        text("None")
    } else {
        let mut v = Vec::new();
        for (n, &b) in theirs.bans.iter().enumerate() {
            if n > 0 {
                v.extend(text(", "));
            }
            v.extend(name(room, b));
        }
        v
    };
    r.push(row(
        &format!("{them}'s"),
        vec![theirs_text],
        0,
        help(&format!("The abilities {them} banned.")),
        true,
    ));
    k.push(RowKind::Info);
    rows.push(r);
    kinds.push(k);
    // Ready: the Keyblade, the AP, Ready.
    if stage == Stage::Loadout {
        names.push(text("Ready"));
        let blades = &room.limits.keyblades;
        let over = ap > room.limits.ap_max;
        let mut r = vec![
            row(
                "Keyblade",
                blades.iter().map(|&b| name(room, b)).collect(),
                blades
                    .iter()
                    .position(|&b| b == game.equipment.weapon)
                    .unwrap_or(0),
                help("Each Keyblade has its own Strength, Magic and ability. Equip shows them."),
                false,
            ),
            row(
                "AP",
                vec![text(&format!("{ap} / {}", room.limits.ap_max))],
                0,
                if over {
                    duel_camp::help_text("Too many abilities equipped: unequip some in Abilities.")
                } else {
                    help("The AP your equipped abilities use.")
                },
                true,
            ),
            row(
                "Ready",
                vec![text("Not yet"), text("Ready")],
                usize::from(mine.ready),
                help("Right when your build is done. The match starts when both are ready."),
                false,
            ),
        ];
        r.push(row(
            them,
            vec![text(section_words(theirs, stage))],
            0,
            help(&format!("Where {them} is.")),
            true,
        ));
        rows.push(r);
        kinds.push(vec![
            RowKind::Keyblade,
            RowKind::Info,
            RowKind::Ready,
            RowKind::Info,
        ]);
    }
    (names, rows, kinds)
}

/// Menu keys in mode 7: Right, R2 (the next page).
const KEY_RIGHT: u64 = 0x80;
const KEY_R2: u64 = 0x1_0000;

/// [Test] Unattended runs: the joiner bans the pool's first ability and
/// takes the table's last Keyblade (in the magic bench the one with the
/// most Magic, against the host's Kingdom Key), the host skips; both ready up a second and a
/// half into the builds (their camp opens about half a second in), once
/// the test vote the host puts to both is over. The pages turn on the way,
/// for the captures: Bans while the bans are chosen, Ready once the
/// builds are up.
fn script(room: &mut LobbyRoom, net: &mut DuelNet, camp: &mut CampRes, game: &mut Game) {
    let seat = &room.lobby.seats[room.seat];
    let on_lobby = camp
        .open
        .as_ref()
        .and_then(|m| m.data.duel.as_ref())
        .is_some_and(|d| d.owner.0 == duel_camp::LOBBY);
    // With `--duel-shots` each page stays up longer (a capture lands a
    // second or so after it is asked for).
    let slow = if game.duel_shots { 2 } else { 1 };
    if on_lobby {
        game.duel_camp_press = match (room.lobby.stage, room.stage_ticks) {
            (Stage::Bans, 30) | (Stage::Loadout, 40 | 46) => KEY_R2,
            (Stage::Bans, 40) if room.seat == 1 => KEY_RIGHT,
            _ => 0,
        };
        // The Match page, the Bans page, the Ready page.
        if matches!(
            (room.lobby.stage, room.stage_ticks),
            (Stage::Bans, 20 | 80) | (Stage::Loadout, 70)
        ) {
            duel_camp::shot(game, 0);
        }
    }
    match room.lobby.stage {
        Stage::Bans if room.stage_ticks == 60 * slow && !seat.bans_done => {
            let bans = if room.seat == 1 {
                room.limits.ban_pool().into_iter().take(1).collect()
            } else {
                Vec::new()
            };
            intend(room, net, game, Intent::Ban(bans));
        }
        Stage::Loadout if room.stage_ticks == 52 && room.seat == 1 => {
            let pick = if game.duel_magic {
                room.limits
                    .keyblades
                    .iter()
                    .copied()
                    .max_by_key(|&k| keyblade_gear(game, k).magic)
            } else {
                room.limits.keyblades.last().copied()
            };
            if let (Some(k), Some(e)) = (
                pick,
                camp.open
                    .as_mut()
                    .and_then(|m| m.data.items.equipment.first_mut()),
            ) {
                e.weapon = k;
            }
        }
        // The test vote to both players while the builds are made (a tie:
        // the coin flips; the lobby gives the camp to it meanwhile).
        Stage::Loadout if room.stage_ticks == 75 * slow && net.role == Role::Host => {
            game.duel_test_vote = true;
        }
        Stage::Loadout if room.stage_ticks >= 90 * slow && !seat.ready && !game.duel_vote_up => {
            intend(room, net, game, Intent::Ready(true));
        }
        _ => {}
    }
}
