//! The duel's lobby in the room: the host keeps the lobby (duel_lobby.rs)
//! and sends it on every change; the joiner sends its choices and shows
//! what comes back. On screen it is a plain list in the system font over
//! the arena, with the menus' se000 sounds; the builds are made in the
//! game's own camp (Start), over the duel's inventory with the match's
//! banned abilities taken out. Both Soras stand still until the match.

use super::duel_lobby::{self, Intent, Limits, Loadout, Lobby, MAX_BANS, Stage};
use super::duel_net::{DuelNet, Status, fighter_slots};
use super::duel_wire::{INTENT, LOBBY, REFUSED};
use super::*;
use net::{RELIABLE, Role};

/// se000: cursor, decide, open, cancel, refused.
const SE_MOVE: u32 = 1;
const SE_DECIDE: u32 = 2;
const SE_OPEN: u32 = 3;
const SE_CANCEL: u32 = 4;
const SE_ERROR: u32 = 5;

/// Logical pad bits of mode 7 (the menus').
const UP: u64 = 0x10;
const DOWN: u64 = 0x20;
const LEFT: u64 = 0x40;
const RIGHT: u64 = 0x80;
const DECIDE: u64 = 0x4;
const BACK: u64 = 0x8;

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

/// Rows of the ban picker on screen at once.
const BAN_ROWS: usize = 12;

/// The camp's state the duel replaces, put back when it ends.
struct SavedCamp {
    ability_slots: Vec<u16>,
    equipment: pause_menu::items::Equipment,
    stock: std::collections::BTreeMap<u16, u8>,
    ap: i32,
    weapon_ability: u16,
}

/// The ban picker: the pool, what this player has marked, the cursor.
struct BanPicker {
    picks: Vec<u16>,
    cursor: usize,
    scroll: usize,
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
    cursor: usize,
    bans: Option<BanPicker>,
    /// A refusal or notice and the tick it came.
    note: Option<(String, u64)>,
    /// Host: the lobby the joiner has.
    sent: Option<Lobby>,
    /// The build last sent.
    built: Option<Loadout>,
    /// The stage this instance last prepared for.
    prepared: Option<(u32, Stage)>,
    /// Ticks in the current stage (the unattended script's clock).
    stage_ticks: u64,
    saved: Option<SavedCamp>,
    /// Ability and Keyblade names (game text) by item id.
    names: HashMap<u16, Vec<u8>>,
    /// The match put the Soras at the arena's starts: the camera turns
    /// behind this player's Sora once it is there (where it stood before).
    recenter: Option<[f32; 3]>,
}

impl LobbyRoom {
    /// The lobby holds the pad (its list is up and no camp is open).
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
    camp.stock = stock;
    camp.source.numbers.ap = room.limits.ap_max - i32::from(game.gear.ap);
    if let Some(rt) = game.enemy.as_mut() {
        rt.numbers_dirty = true;
    }
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

/// One fixed step of the lobby: messages, stage changes, input.
pub(crate) fn tick(
    net: Option<ResMut<DuelNet>>,
    room: Option<ResMut<LobbyRoom>>,
    camp: Option<ResMut<CampRes>>,
    vote: Option<Res<duel_vote::VoteRoom>>,
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
            // The session ended: the camp and the field are the room's
            // again.
            restore_camp(room, camp);
            *room = LobbyRoom {
                names: std::mem::take(&mut room.names),
                ..LobbyRoom::default()
            };
            game.duel_frozen = false;
            game.duel_camp = false;
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
        game.sound_queue.push((SE_OPEN, "lobby"));
        // Slots that hold no ability record (the room's catalog) stay out.
        let other: Vec<u16> = game
            .ability_slots
            .iter()
            .map(|s| s & 0x7fff)
            .filter(|s| *s != 0 && !room.limits.cost.contains_key(s))
            .collect();
        game.event(serde_json::json!({"event":"lobby_open","tick":game.tick,
            "abilities":room.limits.abilities.len(),"keyblades":room.limits.keyblades.len(),
            "ap":room.limits.ap_max,"not_abilities":other}));
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
        room.cursor = 0;
        room.bans = None;
        game.event(serde_json::json!({"event":"lobby_stage","tick":game.tick,
            "stage":format!("{:?}", key.1),"match":key.0,"banned":room.lobby.banned()}));
        match key.1 {
            Stage::Bans => room.up = true,
            Stage::Loadout => {
                prepare_builds(room, game, camp);
                let build = build_of(game);
                room.built = Some(build.clone());
                intend(room, net, game, Intent::Loadout(build));
                game.sound_queue.push((SE_OPEN, "lobby"));
            }
            Stage::Start => {
                // [Test] Unattended runs put the test vote to both players
                // as the match starts (a tie: the coin flips).
                if game.autoplay.is_some() && net.role == Role::Host {
                    game.duel_test_vote = true;
                }
                // Both builds go on their Soras, both to the arena's starts;
                // the match begins.
                for (seat, slot) in [(0, fighter_slots::HOST), (1, fighter_slots::JOIN)] {
                    if let Some(b) = room.lobby.seats[seat].loadout.clone() {
                        apply_build(game, slot, &b);
                    }
                }
                let before = game.sora.player.position;
                if net.role == Role::Host {
                    duel_arena::place(game);
                }
                room.recenter = Some(before);
                room.up = false;
                game.sound_queue.push((SE_DECIDE, "lobby"));
            }
        }
    }
    room.stage_ticks += 1;
    game.duel_frozen = room.up;
    game.duel_camp = room.up && room.lobby.stage == Stage::Loadout;
    // A build made in the camp goes to the host.
    if room.lobby.stage == Stage::Loadout && !camp.menu_up() {
        let build = build_of(game);
        if room.built.as_ref() != Some(&build) {
            room.built = Some(build.clone());
            intend(room, net, game, Intent::Loadout(build));
        }
    }
    let voting = vote.is_some_and(|v| v.menu_up(room.seat));
    if room.up && !camp.menu_up() && !voting {
        if game.autoplay.is_some() {
            script(room, net, game);
        } else {
            let (pressed, repeat) = (game.pad.pressed, game.pad.repeat);
            input(
                room,
                net,
                game,
                pressed | repeat & (UP | DOWN | LEFT | RIGHT),
            );
        }
    }
    // Host: the lobby to the joiner on every change.
    if net.role == Role::Host && room.sent.as_ref() != Some(&room.lobby) {
        room.sent = Some(room.lobby.clone());
        net.session
            .send(RELIABLE, duel_lobby::encode_lobby(LOBBY, &room.lobby));
    }
}

/// [Test] Unattended runs: the joiner bans the pool's first ability and
/// takes the table's last Keyblade, the host skips; both ready up a second
/// into the builds.
fn script(room: &mut LobbyRoom, net: &mut DuelNet, game: &mut Game) {
    let seat = &room.lobby.seats[room.seat];
    match room.lobby.stage {
        Stage::Bans if room.stage_ticks == 60 && !seat.bans_done => {
            let bans = if room.seat == 1 {
                room.limits.ban_pool().into_iter().take(1).collect()
            } else {
                Vec::new()
            };
            intend(room, net, game, Intent::Ban(bans));
        }
        Stage::Loadout if room.stage_ticks == 30 && room.seat == 1 => {
            if let Some(&k) = room.limits.keyblades.last() {
                let mut build = build_of(game);
                build.keyblade = k;
                put_build(game, &build);
            }
        }
        Stage::Loadout if room.stage_ticks == 60 && !seat.ready => {
            intend(room, net, game, Intent::Ready(true));
        }
        _ => {}
    }
}

/// The main list's rows for this player.
#[derive(Clone, Copy, PartialEq)]
enum Row {
    Mode,
    Damage,
    Time,
    Bans,
    Ready,
}

fn rows(room: &LobbyRoom, rules: &duel_rules::MatchRules, host: bool) -> Vec<Row> {
    let mut out = Vec::new();
    if host {
        out.extend([Row::Mode, Row::Damage]);
        if rules.info().timed {
            out.push(Row::Time);
        }
    }
    match room.lobby.stage {
        Stage::Bans if !room.lobby.seats[room.seat].bans_done => out.push(Row::Bans),
        Stage::Loadout => out.push(Row::Ready),
        _ => {}
    }
    out
}

fn input(room: &mut LobbyRoom, net: &mut DuelNet, game: &mut Game, pressed: u64) {
    if pressed == 0 {
        return;
    }
    let host = net.role == Role::Host;
    if let Some(p) = room.bans.as_mut() {
        // The ban picker: row 0 is Done, then the pool.
        let pool = room.limits.ban_pool();
        let n = pool.len() + 1;
        if pressed & UP != 0 {
            p.cursor = (p.cursor + n - 1) % n;
            game.sound_queue.push((SE_MOVE, "lobby"));
        } else if pressed & DOWN != 0 {
            p.cursor = (p.cursor + 1) % n;
            game.sound_queue.push((SE_MOVE, "lobby"));
        } else if pressed & BACK != 0 {
            room.bans = None;
            game.sound_queue.push((SE_CANCEL, "lobby"));
            return;
        } else if pressed & DECIDE != 0 {
            if p.cursor == 0 {
                let picks = p.picks.clone();
                room.bans = None;
                game.sound_queue.push((SE_DECIDE, "lobby"));
                intend(room, net, game, Intent::Ban(picks));
                return;
            }
            let id = pool[p.cursor - 1];
            if let Some(i) = p.picks.iter().position(|&b| b == id) {
                p.picks.remove(i);
                game.sound_queue.push((SE_CANCEL, "lobby"));
            } else if p.picks.len() < MAX_BANS {
                p.picks.push(id);
                game.sound_queue.push((SE_DECIDE, "lobby"));
            } else {
                game.sound_queue.push((SE_ERROR, "lobby"));
            }
        }
        if p.cursor < p.scroll {
            p.scroll = p.cursor;
        } else if p.cursor >= p.scroll + BAN_ROWS {
            p.scroll = p.cursor + 1 - BAN_ROWS;
        }
        return;
    }
    let list = rows(room, &game.duel_rules, host);
    if list.is_empty() {
        return;
    }
    room.cursor = room.cursor.min(list.len() - 1);
    if pressed & UP != 0 {
        room.cursor = (room.cursor + list.len() - 1) % list.len();
        game.sound_queue.push((SE_MOVE, "lobby"));
        return;
    }
    if pressed & DOWN != 0 {
        room.cursor = (room.cursor + 1) % list.len();
        game.sound_queue.push((SE_MOVE, "lobby"));
        return;
    }
    let step = if pressed & RIGHT != 0 {
        1
    } else if pressed & LEFT != 0 {
        -1
    } else {
        0
    };
    let row = list[room.cursor];
    let rules_before = game.duel_rules;
    match row {
        Row::Mode if step != 0 => game.duel_rules.cycle_mode(step),
        Row::Damage if step != 0 => game.duel_rules.step_damage(step),
        Row::Time if step != 0 => game.duel_rules.cycle_time(step),
        Row::Bans if pressed & DECIDE != 0 => {
            room.bans = Some(BanPicker {
                picks: Vec::new(),
                cursor: 0,
                scroll: 0,
            });
            game.sound_queue.push((SE_OPEN, "lobby"));
        }
        Row::Ready if pressed & DECIDE != 0 => {
            let ready = !room.lobby.seats[room.seat].ready;
            game.sound_queue.push((SE_DECIDE, "lobby"));
            intend(room, net, game, Intent::Ready(ready));
        }
        _ => {}
    }
    if game.duel_rules != rules_before {
        game.sound_queue.push((SE_MOVE, "lobby"));
        room.lobby.rules_changed();
        let tick = game.tick;
        let rules = game.duel_rules;
        game.event(serde_json::json!({"event":"duel_rules","tick":tick,
            "mode":rules.info().name,"damage":rules.damage,"time_limit":rules.time_limit}));
    }
}

/// Text in the system font's encoding; game text (item names) is used as
/// it is.
fn text(s: &str) -> Vec<u8> {
    pause_menu::encode_text(s)
}

fn joined(parts: &[&[u8]]) -> Vec<u8> {
    parts.concat()
}

/// The lines the lobby shows: (text, highlighted, warning).
pub(crate) fn lines(room: &LobbyRoom, net: &DuelNet, game: &Game) -> Vec<(Vec<u8>, bool, bool)> {
    let mut out = Vec::new();
    if !room.up {
        return out;
    }
    let host = net.role == Role::Host;
    let me = net.name.as_str();
    let them = net.peer.as_deref().unwrap_or("Opponent");
    let names = |seat: usize| if seat == room.seat { me } else { them };
    let ability = |id: u16| {
        room.names
            .get(&id)
            .cloned()
            .unwrap_or_else(|| text(&format!("? {id}")))
    };
    // The system font has no brackets or arrows: the cursor's row is gold.
    let mark = |_on: bool| "   ";
    out.push((
        text(&format!("Duel lobby   Match {}", room.lobby.match_no + 1)),
        false,
        false,
    ));
    // The ban picker replaces the list while it is open.
    if let Some(p) = &room.bans {
        out.push((
            text(&format!("Ban up to {MAX_BANS} abilities for both players")),
            false,
            false,
        ));
        out.push((
            text(&format!(
                "{}Done: {} chosen",
                mark(p.cursor == 0),
                p.picks.len()
            )),
            p.cursor == 0,
            false,
        ));
        let pool = room.limits.ban_pool();
        for (i, &id) in pool.iter().enumerate().skip(p.scroll).take(BAN_ROWS) {
            let on = p.cursor == i + 1;
            let x = if p.picks.contains(&id) { "* " } else { "   " };
            out.push((
                joined(&[&text(mark(on)), &text(x), &ability(id)]),
                on,
                false,
            ));
        }
        return out;
    }
    // The rules (the host's to change).
    let list = rows(room, &game.duel_rules, host);
    let cursor_row = list.get(room.cursor).copied();
    for (label, value) in game.duel_rules.lines() {
        let row = match label {
            "Mode" => Row::Mode,
            "Damage" => Row::Damage,
            _ => Row::Time,
        };
        let on = host && cursor_row == Some(row);
        let arrows = if on { format!("- {value} +") } else { value };
        out.push((text(&format!("{}{label}   {arrows}", mark(on))), on, false));
    }
    if !host {
        out.push((text(&format!("   Rules set by {them}")), false, false));
    }
    // The bans, both players'.
    for seat in 0..2 {
        let s = &room.lobby.seats[seat];
        let what = if !s.bans_done {
            text("choosing...")
        } else if s.bans.is_empty() {
            text("no bans")
        } else {
            let mut v = Vec::new();
            for (k, &b) in s.bans.iter().enumerate() {
                if k > 0 {
                    v.extend(text(", "));
                }
                v.extend(ability(b));
            }
            v
        };
        out.push((
            joined(&[&text(&format!("   {} bans: ", names(seat))), &what]),
            false,
            false,
        ));
    }
    match room.lobby.stage {
        Stage::Bans => {
            if list.contains(&Row::Bans) {
                let on = cursor_row == Some(Row::Bans);
                out.push((text(&format!("{}Choose bans", mark(on))), on, false));
            } else {
                out.push((text(&format!("   Waiting for {them}'s bans")), false, false));
            }
        }
        Stage::Loadout => {
            out.push((
                text(&format!(
                    "   Start: Abilities ({} AP), Keyblade and items",
                    room.limits.ap_max
                )),
                false,
                false,
            ));
            let mine = room.lobby.seats[room.seat].ready;
            let on = cursor_row == Some(Row::Ready);
            out.push((
                text(&format!(
                    "{}{}",
                    mark(on),
                    if mine { "Ready!" } else { "Ready" }
                )),
                on,
                false,
            ));
            let theirs = room.lobby.seats[1 - room.seat].ready;
            out.push((
                text(&format!(
                    "   {them}: {}",
                    if theirs { "ready" } else { "not ready" }
                )),
                false,
                false,
            ));
        }
        Stage::Start => {}
    }
    if let Some((note, at)) = &room.note
        && game.tick.saturating_sub(*at) < 240
    {
        out.push((text(&format!("   {note}")), false, true));
    }
    out
}
