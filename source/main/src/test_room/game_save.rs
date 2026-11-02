//! The room's picture of the save (issue #79). `capture` is FUN_1402ed110's
//! copy: the running game into the save image a file holds, on top of the
//! image the room started from so what the room does not model stays as
//! loaded. `apply_progress`, `apply_game` and `apply_camp` are FUN_1402ed0e0's
//! copy the other way, FUN_1403a05f0 and FUN_1402e5000's config.
//!
//! The room keeps the same numbers on its own objects (Sora's vitals, the
//! camp, the party's records), so each is written out and read back here.
use super::*;
use ::area::progress::Progress;
use ::area::world_state::WorldRecord;
use data::item_table::Item;
use save::{
    data::SaveData,
    slots::field,
    state::{self, FormRecord, Persistent, Record, read_bits, write_bits},
};

/// What the room keeps for the save: the image it started from, the item
/// table a stock is stored by, the play time up to this run and the world
/// the running time counts for.
#[derive(Clone)]
pub struct SaveState {
    pub base: SaveData,
    pub items: Vec<Item>,
    pub play_start: u32,
    pub world_frames: [u32; 19],
    /// The world the frames since `world_tick` belong to.
    pub world: Option<u8>,
    pub world_tick: u64,
    /// `--save-dump <file>`: each written save also goes here (tests).
    pub dump: Option<PathBuf>,
    /// The room started from a loaded save.
    pub loaded: bool,
    /// Each world's party (save+0x3534 + 4·world, FUN_1403e2e20).
    pub party_table: [[u8; 4]; 19],
    /// The members' order changed: their meshes and gauges follow.
    pub party_moved: bool,
    pub party_order: Vec<usize>,
    /// The room's install, manifest and config: what the world map's row
    /// starts the world map with.
    pub args: Vec<PathBuf>,
}

impl Default for SaveState {
    fn default() -> Self {
        Self {
            base: SaveData::blank(),
            items: Vec::new(),
            play_start: 0,
            world_frames: [0; 19],
            world: None,
            world_tick: 0,
            dump: None,
            loaded: false,
            party_table: [[0; 4]; 19],
            party_moved: false,
            party_order: Vec::new(),
            args: Vec::new(),
        }
    }
}

impl SaveState {
    /// The play time so far (FUN_1401574e0 caps it at 0xcdfe5c4).
    pub fn play_frames(&self, tick: u64) -> u32 {
        (u64::from(self.play_start) + tick).min(0xcdf_e5c4) as u32
    }

    /// FUN_1401574e0's world counter, brought up to `tick`: the frames since
    /// the last call count for the world the room was in.
    pub fn note_world(&mut self, world: u8, tick: u64) {
        if let Some(w) = self.world
            && let Some(f) = self.world_frames.get_mut(w as usize)
        {
            *f = f
                .saturating_add((tick - self.world_tick) as u32)
                .min(0xcdf_e5c4);
        }
        self.world = Some(world);
        self.world_tick = tick;
    }
}

/// A party byte's member type (FUN_1403e3670) and the empty slot.
pub const fn party_type(byte: u8) -> u8 {
    byte & 0x1f
}
const PARTY_EMPTY: u8 = 0x12;
/// A new world's table in the room: Sora, Donald, Goofy, no reserve
/// [test-room value: the exe builds them from DAT_142a252e8 by event].
pub const ROOM_PARTY: [u8; 4] = [0, 1, 2, PARTY_EMPTY];

/// A world's table, the room's own if the save has none for it (an unset one
/// reads as all Soras).
pub fn world_party(table: &[[u8; 4]; 19], world: u8) -> [u8; 4] {
    match table.get(world as usize) {
        Some(row) if *row != [0; 4] => *row,
        _ => ROOM_PARTY,
    }
}

/// A member's type in the row, from its character id.
pub fn character_type(character: u8) -> u8 {
    match character {
        2 => 1,
        3 => 2,
        _ => 3,
    }
}

/// FUN_1403e31f0 over a world's row: an unset row starts as FUN_1403e2e10's new
/// one, and members already in the row keep their order.
pub fn row_with_presets(row: [u8; 4], presets: &[u32]) -> [u8; 4] {
    let start = if row == [0; 4] {
        data::member_table::NEW_RECORD
    } else {
        row
    };
    presets
        .iter()
        .fold(start, |r, &p| data::member_table::apply_preset(r, p))
}

/// FUN_1403e37c0: slot 0 always, else bit 7, else a member FUN_140400880's list
/// holds: the Drive form or Summon that took it out of the field
/// (FUN_140400990, FUN_140400a30). `absent` holds those members' types.
pub fn party_locked(row: &[u8; 4], slot: usize, absent: &[u8]) -> bool {
    slot == 0
        || row
            .get(slot)
            .is_some_and(|&b| b & 0x80 != 0 || absent.contains(&party_type(b)))
}

/// The row's empty byte.
const NO_MEMBER: u8 = 0x12;

/// FUN_140320680: the bytes of `from` whose member `against` doesn't hold.
pub fn party_changes(from: &[u8; 4], against: &[u8; 4]) -> Vec<u8> {
    from[1..]
        .iter()
        .copied()
        .filter(|&b| {
            let t = party_type(b);
            t != NO_MEMBER && !against.iter().any(|&o| party_type(o) == t)
        })
        .collect()
}

/// FUN_1403fd8d0: each member who left gives up its gear and items to the stock,
/// and its abilities are unequipped.
pub fn strip_departing(game: &mut Game, old: &[u8; 4], new: &[u8; 4]) {
    let left: Vec<u8> = party_changes(old, new)
        .into_iter()
        .map(party_type)
        .collect();
    let mut given = Vec::new();
    let mut strip = |save: &mut player::party_members::Save| {
        for list in [&mut save.armor, &mut save.accessories, &mut save.items] {
            for v in list.iter_mut() {
                if *v != 0 {
                    given.push(*v);
                    *v = 0;
                }
            }
        }
        for a in save.abilities.iter_mut() {
            *a &= 0x7fff;
        }
    };
    for m in &mut game.party {
        if left.contains(&character_type(m.kind.character)) {
            strip(&mut m.save);
        }
    }
    if let Some(r) = game.reserve.as_mut()
        && left.contains(&character_type(r.character))
    {
        strip(&mut r.save);
    }
    if let Some(items) = game.items.as_mut() {
        for id in given {
            let n = items.stock.entry(id).or_insert(0);
            *n = n.saturating_add(1).min(99);
        }
    }
}

/// A reserve's record, from the save when it holds one.
pub fn load_reserve(r: &mut party_room::Reserve, base: &SaveData) {
    let p = Persistent::read(base);
    let Some(record) = p
        .records
        .get(r.character as usize - 1)
        .filter(|x| x.max_hp != 0)
    else {
        return;
    };
    let s = &mut r.save;
    s.weapon = record.weapon;
    s.bonus = [record.ap, record.strength, record.magic, record.defense];
    s.armor_slots = record.armor_slots;
    s.accessory_slots = record.accessory_slots;
    s.item_slots = record.item_slots;
    s.armor = record.armor;
    s.accessories = record.accessories;
    s.items = record.items;
    s.originals = record.originals;
    s.abilities = record.abilities;
    s.style = record.style;
    s.frequency = record.frequency;
    r.max_hp = i32::from(record.max_hp);
    r.hp = i32::from(record.hp.min(record.max_hp));
    r.max_mp = i32::from(record.max_mp);
    r.mp = i32::from(record.mp.min(record.max_mp));
}

/// A member with no field object writes its record into the save.
pub fn store_reserve(r: &party_room::Reserve, base: &mut SaveData) {
    let mut p = Persistent::read(base);
    if let Some(to) = p.records.get_mut(r.character as usize - 1) {
        let level = to.level.max(1);
        *to = Record {
            level,
            ..member_of(&r.save, r.hp, r.max_hp, (r.mp, r.max_mp))
        };
    }
    p.write(base);
}

/// A member's gear from its record.
pub fn member_gear(
    source: &super::AbilitySource,
    save: &player::party_members::Save,
) -> player::equipment_stats::Totals {
    let record = member_of(save, 0, 0, (0, 0));
    source.gear(&equipment_of(&record), None)
}

/// A member's own byte of the row, or a fresh one (bit 7 is its lock).
pub fn camp_row(row: &[u8; 4], order: &[u8]) -> [u8; 4] {
    let mut camp = [row[0], NO_MEMBER, NO_MEMBER, NO_MEMBER];
    for (to, &character) in camp[1..].iter_mut().zip(order) {
        let t = character_type(character);
        *to = row
            .iter()
            .copied()
            .find(|&b| party_type(b) == t)
            .unwrap_or(t);
    }
    camp
}

/// The slot a field object leaves, and the type that leaves.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Leaver {
    pub slot: usize,
    pub party_type: u8,
}

/// FUN_14036b460: the world's row takes the camp's, once the camp closes. With
/// both active slots changed they swap (only bytes: the field keeps its list).
/// A reserve who came out replaces whoever now stands in its slot: at the
/// player's own camp (`field_camp`, `DAT_140742da4 == 1`) that also replaces
/// the field object, which is the leaver returned; at an event's camp the bytes
/// alone swap.
pub fn write_party_order(row: &mut [u8; 4], camp: &[u8; 4], field_camp: bool) -> Option<Leaver> {
    if party_type(camp[1]) != party_type(row[1]) && party_type(camp[2]) != party_type(row[2]) {
        row.swap(1, 2);
    }
    let reserve = party_type(row[3]);
    if reserve == NO_MEMBER {
        return None;
    }
    let slot = (1..=3).find(|&k| party_type(camp[k]) == reserve)?;
    if slot == 3 {
        return None;
    }
    if !field_camp {
        row.swap(slot, 3);
        return None;
    }
    let left = party_type(row[slot]);
    to_reserve(row, slot);
    Some(Leaver {
        slot,
        party_type: left,
    })
}

/// FUN_1403e33f0: the reserve takes `slot`, and whoever stood there goes to
/// the reserve. Both lock bits drop.
pub fn to_reserve(row: &mut [u8; 4], slot: usize) {
    let left = party_type(row[slot]);
    row[slot] = party_type(row[3]);
    row[3] = left;
}

/// The room's members in the order of a world's table: by the party slots
/// their types hold, Donald before Goofy when the table has neither.
pub fn party_order(row: &[u8; 4], characters: &[u8]) -> Vec<usize> {
    let slot_of = |character: u8| {
        let t = match character {
            2 => 1,
            3 => 2,
            _ => 3,
        };
        row[1..]
            .iter()
            .position(|&b| party_type(b) == t)
            .unwrap_or(usize::from(character))
    };
    let mut order: Vec<usize> = (0..characters.len()).collect();
    order.sort_by_key(|&i| slot_of(characters[i]));
    order
}

/// The image of `--load-image <file>`, refusing what FUN_1402ecc00 refuses.
pub fn read_image(path: &Path) -> Result<SaveData, String> {
    let bytes = std::fs::read(path).map_err(|e| format!("{}: {e}", path.display()))?;
    SaveData::from_bytes(&bytes).map_err(|e| format!("{}: {e}", path.display()))
}

/// The story state a save holds, on a progress whose scripts are loaded: the
/// load replaces it whole (FUN_1402ed0e0), so no flag script runs.
pub fn apply_progress(progress: &mut Progress, data: &SaveData) {
    let p = Persistent::read(data);
    progress.restore_flags(&p.flags);
    progress.programs.clone_from(&p.programs);
    for (to, from) in progress.arif_set.iter_mut().zip(&p.arif) {
        *to = i32::from(*from as i8);
    }
    let w = &mut progress.world;
    for (to, from) in w.records.iter_mut().zip(&p.world_records) {
        *to = WorldRecord {
            state: from[0],
            landing_set: from[1],
            value: from[2],
            flags: from[3],
        };
    }
    w.symbol_slots = p.symbol_slots;
    w.event_bits = p.world_events;
    w.queue.clone_from(&p.event_queue);
    w.episodes = p.episodes;
    w.episodes_new = p.episodes_new;
    w.draw_group_set = p.draw_group_set;
    w.world_points = p.world_points;
    w.menu_flags = p.menu_flags;
    for (i, word) in progress.battle_bits.iter_mut().enumerate() {
        *word = data.u32_at(state::BATTLE_BITS + i * 4);
    }
    for (i, word) in progress.area_seen.iter_mut().enumerate() {
        let at = state::AREA_SEEN + i * 8;
        *word = u64::from(data.u32_at(at)) | u64::from(data.u32_at(at + 4)) << 32;
    }
    let dsa: Vec<[u16; 2]> = (0..progress.dsa_state().len())
        .map(|i| {
            [
                data.u16_at(state::DSA + i * 4),
                data.u16_at(state::DSA + i * 4 + 2),
            ]
        })
        .collect();
    progress.restore_dsa_state(&dsa);
}

/// The story state into an image (the world map's hand-off starts here).
pub fn story_image(progress: &Progress, base: &SaveData) -> SaveData {
    let mut d = base.clone();
    let mut p = Persistent::read(&d);
    write_progress(progress, &mut p, &mut d);
    p.write(&mut d);
    d
}

fn write_progress(progress: &Progress, p: &mut Persistent, data: &mut SaveData) {
    let words = progress.flag_words();
    for (to, from) in p.flags.iter_mut().zip(words) {
        *to = *from;
    }
    p.programs.clone_from(&progress.programs);
    for (to, from) in p.arif.iter_mut().zip(&progress.arif_set) {
        *to = *from as u8;
    }
    let w = &progress.world;
    for (to, from) in p.world_records.iter_mut().zip(&w.records) {
        *to = [from.state, from.landing_set, from.value, from.flags];
    }
    p.symbol_slots = w.symbol_slots;
    p.world_events = w.event_bits;
    p.event_queue.clone_from(&w.queue);
    p.episodes = w.episodes;
    p.episodes_new = w.episodes_new;
    p.draw_group_set = w.draw_group_set;
    p.world_points = w.world_points;
    p.menu_flags = w.menu_flags;
    for (i, &word) in progress.battle_bits.iter().enumerate() {
        data.set_u32(state::BATTLE_BITS + i * 4, word);
    }
    for (i, &word) in progress.area_seen.iter().enumerate() {
        let at = state::AREA_SEEN + i * 8;
        data.set_u32(at, word as u32);
        data.set_u32(at + 4, (word >> 32) as u32);
    }
    for (i, e) in progress.dsa_state().iter().enumerate() {
        data.set_u16(state::DSA + i * 4, e[0]);
        data.set_u16(state::DSA + i * 4 + 2, e[1]);
    }
}

/// FUN_1403c3f00's tail: a non-key item's bit at save+0x3d88 (rec +0x12).
pub fn note_obtained(base: &mut SaveData, items: &[Item], item: u16) {
    if let Some(r) = items.iter().find(|r| r.id == item && r.flags & 1 == 0) {
        let at = state::OBTAINED_ITEMS + r.slot as usize / 8;
        let b = base.u8_at(at);
        base.set_u8(at, b | 1 << (r.slot % 8));
    }
}

/// FUN_1403c4720 over the table: the count of every item the image holds.
/// Type 0x13 has no count.
pub fn read_stock(items: &[Item], data: &SaveData) -> std::collections::BTreeMap<u16, u8> {
    items
        .iter()
        .filter_map(|r| {
            let count = if r.flags & 1 != 0 {
                let slot = r.slot as usize;
                (slot < state::ITEM_BIT_BYTES * 8)
                    .then(|| data.u8_at(state::ITEM_BITS + slot / 8) >> (slot % 8) & 1)
            } else if r.kind == 0x13 {
                None
            } else {
                ((r.slot as usize) < state::ITEM_COUNT_BYTES)
                    .then(|| data.u8_at(state::ITEM_COUNTS + r.slot as usize))
            }?;
            (count > 0).then_some((r.id, count))
        })
        .collect()
}

/// The stock back into the image. Slots no table row names keep their bytes.
pub fn write_stock(
    items: &[Item],
    stock: &std::collections::BTreeMap<u16, u8>,
    data: &mut SaveData,
) {
    for r in items {
        let slot = r.slot as usize;
        if r.flags & 1 != 0 && slot < state::ITEM_BIT_BYTES * 8 {
            let at = state::ITEM_BITS + slot / 8;
            data.set_u8(at, data.u8_at(at) & !(1 << (slot % 8)));
        } else if r.flags & 1 == 0 && r.kind != 0x13 && slot < state::ITEM_COUNT_BYTES {
            data.set_u8(state::ITEM_COUNTS + slot, 0);
        }
    }
    for r in items {
        let count = stock.get(&r.id).copied().unwrap_or(0);
        if count == 0 {
            continue;
        }
        let slot = r.slot as usize;
        if r.flags & 1 != 0 && slot < state::ITEM_BIT_BYTES * 8 {
            let at = state::ITEM_BITS + slot / 8;
            data.set_u8(at, data.u8_at(at) | 1 << (slot % 8));
        } else if r.flags & 1 == 0 && r.kind != 0x13 && slot < state::ITEM_COUNT_BYTES {
            data.set_u8(state::ITEM_COUNTS + slot, count);
        }
    }
}

/// Rows keyed by table row to the exe's per-world seen bits
/// (`base + ((ord >> 5) + world) * 4`, bit `ord & 31`, ord the row's place
/// among its world's rows, FUN_1403460d0).
fn per_world_bits(worlds: &[i8], rows: &[u16]) -> Vec<u16> {
    rows.iter()
        .filter_map(|&r| {
            let world = *worlds.get(r as usize)?;
            let ord = worlds[..r as usize].iter().filter(|&&w| w == world).count();
            u16::try_from(world).ok().map(|w| w * 32 + ord as u16)
        })
        .collect()
}

fn per_world_rows(worlds: &[i8], bits: &[u16]) -> Vec<u16> {
    let mut seen = [0u16; 128];
    worlds
        .iter()
        .enumerate()
        .filter_map(|(r, &w)| {
            let n = seen.get_mut(w as usize)?;
            let ord = *n;
            *n += 1;
            let world = u16::try_from(w).ok()?;
            bits.contains(&(world * 32 + ord)).then_some(r as u16)
        })
        .collect()
}

/// Pieces as `(puzzle, piece)` to the bitmap of 8 bytes a puzzle.
fn piece_bits(pieces: &std::collections::BTreeSet<(u8, u8)>) -> Vec<u16> {
    pieces
        .iter()
        .map(|&(puzzle, piece)| u16::from(puzzle) * 64 + u16::from(piece))
        .collect()
}

fn piece_set(bits: &[u16]) -> std::collections::BTreeSet<(u8, u8)> {
    bits.iter()
        .map(|&b| ((b / 64) as u8, (b % 64) as u8))
        .collect()
}

fn write_puzzles(puzzles: &journal::PuzzleSave, data: &mut SaveData) {
    let bytes = state::PUZZLE_COUNT * 8;
    write_bits(
        data,
        state::PIECES_OWNED,
        bytes,
        &piece_bits(&puzzles.owned),
    );
    write_bits(
        data,
        state::PIECES_SEEN_BOARD,
        bytes,
        &piece_bits(&puzzles.seen_board),
    );
    write_bits(
        data,
        state::PIECES_SEEN_LIST,
        bytes,
        &piece_bits(&puzzles.seen_list),
    );
    data.set_u16(state::PUZZLE_COMPLETED, puzzles.completed);
    let tutorial = data.u16_at(state::PUZZLE_TUTORIAL) & !1 | u16::from(puzzles.tutorial);
    data.set_u16(state::PUZZLE_TUTORIAL, tutorial);
    for (p, board) in puzzles.slots.iter().enumerate().take(state::PUZZLE_COUNT) {
        for (s, slot) in board.iter().enumerate() {
            let at = state::PUZZLE_SLOTS + (p * state::PUZZLE_SLOT_COUNT + s) * 4;
            data.set_u8(at, slot.piece as u8);
            data.set_u8(at + 1, slot.rot);
        }
    }
}

fn read_puzzles(data: &SaveData) -> journal::PuzzleSave {
    let bytes = state::PUZZLE_COUNT * 8;
    let mut slots = vec![[journal::PuzzleSlot::EMPTY; 48]; state::PUZZLE_COUNT];
    for (p, board) in slots.iter_mut().enumerate() {
        for (s, slot) in board.iter_mut().enumerate() {
            let at = state::PUZZLE_SLOTS + (p * state::PUZZLE_SLOT_COUNT + s) * 4;
            slot.piece = data.u8_at(at) as i8;
            slot.rot = data.u8_at(at + 1);
        }
    }
    journal::PuzzleSave {
        owned: piece_set(&read_bits(data, state::PIECES_OWNED, bytes)),
        seen_board: piece_set(&read_bits(data, state::PIECES_SEEN_BOARD, bytes)),
        seen_list: piece_set(&read_bits(data, state::PIECES_SEEN_LIST, bytes)),
        completed: data.u16_at(state::PUZZLE_COMPLETED),
        tutorial: data.u16_at(state::PUZZLE_TUTORIAL) & 1 != 0,
        slots,
        ..Default::default()
    }
}

fn list(values: &[u16]) -> [u16; 8] {
    std::array::from_fn(|i| values.get(i).copied().unwrap_or(0))
}

fn member_of(save: &player::party_members::Save, hp: i32, max_hp: i32, mp: (i32, i32)) -> Record {
    Record {
        weapon: save.weapon,
        hp: hp.clamp(0, 255) as u8,
        max_hp: max_hp.clamp(0, 255) as u8,
        mp: mp.0.clamp(0, 255) as u8,
        max_mp: mp.1.clamp(0, 255) as u8,
        ap: save.bonus[0],
        strength: save.bonus[1],
        magic: save.bonus[2],
        defense: save.bonus[3],
        armor_slots: save.armor_slots,
        accessory_slots: save.accessory_slots,
        item_slots: save.item_slots,
        armor: save.armor,
        accessories: save.accessories,
        items: save.items,
        originals: save.originals,
        abilities: save.abilities,
        style: save.style,
        frequency: save.frequency,
        ..Record::default()
    }
}

fn apply_member(m: &mut party_room::MemberRt, r: &Record) {
    let s = &mut m.save;
    s.weapon = r.weapon;
    s.bonus = [r.ap, r.strength, r.magic, r.defense];
    s.armor_slots = r.armor_slots;
    s.accessory_slots = r.accessory_slots;
    s.item_slots = r.item_slots;
    s.armor = r.armor;
    s.accessories = r.accessories;
    s.items = r.items;
    s.originals = r.originals;
    s.abilities = r.abilities;
    s.style = r.style;
    s.frequency = r.frequency;
    m.max_hp = i32::from(r.max_hp);
    m.hp = i32::from(r.hp.min(r.max_hp));
    m.mp = player::magic::Mp::new(i32::from(r.max_mp));
    let delta = i32::from(r.mp.min(r.max_mp)) - m.mp.cur;
    if delta != 0 {
        m.mp.add(delta);
    }
}

/// The level row Sora's numbers stand on (FUN_1403ed2e0).
fn level_row(game: &Game, level: u8) -> Option<data::battle_tables::PlayerLevel> {
    game.enemy
        .as_ref()?
        .sora_levels
        .get((level.max(1) - 1) as usize)
        .copied()
}

/// The save point Sora stands at (the search offers only one at a time, the
/// nearest is it).
fn nearest_point(game: &Game, area: &AreaRes) -> Option<usize> {
    let at = Vec3::from_array(game.sora.player.position);
    area.current
        .save_points
        .iter()
        .enumerate()
        .min_by(|x, y| {
            x.1.0
                .distance_squared(at)
                .total_cmp(&y.1.0.distance_squared(at))
        })
        .map(|(i, _)| i)
}

/// A WORLD_POINT's menu has the World Map row (FUN_1402eb490).
pub fn at_world_point(game: &Game, area: Option<&AreaRes>) -> bool {
    area.and_then(|a| {
        let i = nearest_point(game, a)?;
        a.current.world_points.get(i).copied()
    })
    .unwrap_or(false)
}

/// FUN_140154e20(-1, 0x80000000): the save holds the map and the entrance
/// beside the world left; the world map starts on it as a program of its own
/// (FUN_140152990 loads it in place).
pub fn jump_to_world_map(game: &mut Game, c: &CampRes, area: Option<&AreaRes>) {
    let mut image = capture(game, c, area);
    let from = image.u8_at(field::WORLD);
    let target = ::area::world_state::world_map_entry(from);
    image.set_u8(field::WORLD, target.world);
    image.set_u8(field::AREA, target.area);
    image.set_u8(field::ENTRANCE, target.entrance);
    image.seal();
    let tick = game.tick;
    game.event(
        serde_json::json!({"event":"world_map_jump","tick":tick,"from":from,
        "entrance":target.entrance}),
    );
    if let Some(path) = &game.save.dump
        && let Err(e) = std::fs::write(path, image.bytes())
    {
        eprintln!("Save dump rejected: {e}");
    }
    game.limit = Some(tick);
    if game.autoplay.is_some() && !crate::save_files::chain_test() {
        return;
    }
    if let Err(e) = crate::save_files::write_handoff(&image) {
        eprintln!("The world map could not start: {e}");
        return;
    }
    let args = &game.save.args;
    if let (Some(root), Some(manifest), Some(config)) = (args.first(), args.get(1), args.get(2)) {
        crate::save_files::start_next(vec![
            "wm".into(),
            root.to_string_lossy().into_owned(),
            manifest.to_string_lossy().into_owned(),
            "--field".into(),
            config.to_string_lossy().into_owned(),
            "--load-image".into(),
            crate::save_files::handoff_path()
                .to_string_lossy()
                .into_owned(),
        ]);
    }
}

/// A door to another world (FUN_1403a46e0 → FUN_140152990): the save holds the
/// place and the field starts on it as a program of its own, as a landing does
/// (`world_map::start_field`). Unattended runs write the config and the dump
/// and stop there.
pub fn jump_to_field(
    game: &mut Game,
    c: &CampRes,
    area: Option<&AreaRes>,
    (world, number, entrance): (u8, u8, u8),
) {
    game.door_out = None;
    let mut image = capture(game, c, area);
    image.set_u8(field::WORLD, world);
    image.set_u8(field::AREA, number);
    image.set_u8(field::ENTRANCE, entrance);
    image.seal();
    let tick = game.tick;
    if let Some(path) = &game.save.dump
        && let Err(e) = std::fs::write(path, image.bytes())
    {
        eprintln!("Save dump rejected: {e}");
    }
    game.limit = Some(tick);
    let args = game.save.args.clone();
    let config = ::area::progress::WORLDS
        .get(world as usize)
        .zip(args.get(2))
        .and_then(|(code, arena)| {
            crate::launcher::landing_room_config(arena, code, number, entrance, None, &[])
                .map_err(|e| eprintln!("The next world could not start: {e}"))
                .ok()
        });
    let started = (game.autoplay.is_none() || crate::save_files::chain_test())
        && crate::save_files::write_handoff(&image).is_ok()
        && match (args.first(), args.get(1), &config) {
            (Some(root), Some(manifest), Some(config)) => crate::save_files::start_next(vec![
                "room".into(),
                root.to_string_lossy().into_owned(),
                manifest.to_string_lossy().into_owned(),
                config.to_string_lossy().into_owned(),
                "--load-image".into(),
                crate::save_files::handoff_path()
                    .to_string_lossy()
                    .into_owned(),
            ]),
            _ => false,
        };
    game.event(
        serde_json::json!({"event":"field_jump","tick":tick,"world":world,"area":number,
        "entrance":entrance,"started":started}),
    );
}

/// FUN_1402ed110's copy: the running game into the image.
pub fn capture(game: &Game, c: &CampRes, area: Option<&AreaRes>) -> SaveData {
    let mut d = game.save.base.clone();
    let mut p = Persistent::read(&d);
    if let Some(a) = area {
        p.world = a.current.world;
        p.area = a.current.area;
        p.entrance = a.current.state.entrance;
        write_progress(a.source.progress(), &mut p, &mut d);
    }
    // FUN_1403a8b10 case 0x37: the save point's own world and area.
    if let Some((world, area)) = game.save_place {
        p.world = world;
        p.area = area;
    }
    if let Some(a) = area
        && let Some(&entrance) =
            nearest_point(game, a).and_then(|i| a.current.save_entrances.get(i))
    {
        p.entrance = entrance;
    }
    p.party_table = game.save.party_table;
    p.play_frames = game.save.play_frames(game.tick);
    p.world_frames = game.save.world_frames;
    if let Some(w) = game.save.world
        && let Some(f) = p.world_frames.get_mut(w as usize)
    {
        *f = f.saturating_add((game.tick - game.save.world_tick) as u32);
    }
    if let Some(rt) = game.enemy.as_ref() {
        p.munny = rt.munny;
        p.exp = rt.exp_total;
        p.difficulty = rt.difficulty as u32;
        p.bonus_count = rt.bonus_taken.len() as u8;
        let bits: Vec<u16> = rt.bonus_taken.iter().map(|&b| u16::from(b)).collect();
        write_bits(&mut d, state::BONUS_BITS, state::BONUS_BIT_BYTES, &bits);
    } else {
        p.difficulty = c.game_mode;
    }
    // Sora's record, with the numbers as bonus over the level's row.
    let sora = &game.sora;
    let v = &sora.vitals;
    let row = level_row(game, v.level);
    let over = |value: i32, base: u8| (value - i32::from(base)).clamp(0, 255) as u8;
    let mut record = p.records[0];
    write_camp_record(c, &mut record);
    record.level = v.level;
    record.mp = sora.player.mp.cur.clamp(0, 255) as u8;
    record.max_mp = sora.player.mp.max.clamp(0, 255) as u8;
    if game.enemy.is_some() {
        record.hp = v.hp.clamp(0, 255) as u8;
        record.max_hp = v.target.max_hp.clamp(0, 255) as u8;
    }
    if let Some(r) = row {
        let gear = game.gear;
        record.strength = over(v.attacker.strength - i32::from(gear.strength), r.strength);
        record.magic = over(v.attacker.magic - i32::from(gear.magic), r.magic);
        record.defense = over(v.target.defense - i32::from(gear.defense), r.defense);
        record.ap = over(c.source.numbers.ap, r.ap);
    }
    p.records[0] = record;
    p.drive = [
        sora.player.drive.partial,
        sora.player.drive.level,
        sora.player.drive.max,
    ];
    if let Some(drive) = game.drive.as_ref() {
        for f in 1..=6u8 {
            if let Some(r) = drive.forms.record(f) {
                p.forms[f as usize - 1] = FormRecord {
                    weapon: r.keyblade,
                    level: r.level,
                    exp: r.exp,
                    abilities: r.abilities,
                };
            }
        }
        p.current_form = if drive.active { drive.form } else { 0 };
    }
    for m in &game.party {
        let index = m.kind.character as usize - 1;
        if let Some(r) = p.records.get_mut(index) {
            // A new game's records stand at level 1 (FUN_1403a0e70).
            let level = r.level.max(1);
            *r = Record {
                level,
                ..member_of(&m.save, m.hp, m.max_hp, (m.mp.cur, m.mp.max))
            };
        }
    }
    if let Some(m) = &game.reserve
        && let Some(r) = p.records.get_mut(m.character as usize - 1)
    {
        let level = r.level.max(1);
        *r = Record {
            level,
            ..member_of(&m.save, m.hp, m.max_hp, (m.mp, m.max_mp))
        };
    }
    p.write(&mut d);
    if let Some(a) = area {
        let progress = a.source.progress();
        write_background(&mut d, game.tick, |flag| progress.test(flag));
    }
    // The stock (the battle's is the live one) and what the camp has seen.
    let mut stock = game.items.as_ref().map_or(&c.stock, |i| &i.stock).clone();
    // A form is owned through its item (FUN_1403c4720 of FUN_1403c3ef0), which
    // the room's Drive keeps apart from the stock.
    if let Some(drive) = game.drive.as_ref() {
        for (f, item) in player::forms::FORM_ITEMS.iter().enumerate().skip(1) {
            if drive.forms.owned[f] {
                stock.entry(*item).or_insert(1);
            }
        }
    }
    write_camp(c, &game.save.items, &stock, &mut d);
    d.seal();
    d
}

/// Sora's gear and abilities as the camp holds them.
fn write_camp_record(c: &CampRes, record: &mut Record) {
    let e = &c.equipment;
    record.weapon = e.weapon;
    record.second_weapon = e.secondary_weapon;
    record.armor_slots = e.armor.len() as u8;
    record.accessory_slots = e.accessories.len() as u8;
    record.item_slots = e.items.len() as u8;
    record.armor = list(&e.armor);
    record.accessories = list(&e.accessories);
    record.items = list(&e.items);
    record.originals = list(&e.originals);
    record.abilities = std::array::from_fn(|i| c.ability_slots.get(i).copied().unwrap_or(0));
}

/// The world map's share: the camp, and with no save behind it Sora's record
/// at the room's start.
pub fn write_camp_save(
    c: &CampRes,
    items: &[Item],
    stats: (i32, i32),
    level_one: Option<data::battle_tables::PlayerLevel>,
    d: &mut SaveData,
) {
    let mut record = Record::read(d, 0);
    if record.max_hp == 0 {
        record.hp = stats.0.clamp(0, 255) as u8;
        record.max_hp = record.hp;
        record.mp = stats.1.clamp(0, 255) as u8;
        record.max_mp = record.mp;
        record.level = 1;
        if let Some(r) = level_one {
            let n = &c.source.numbers;
            let over = |value: i32, base: u8| (value - i32::from(base)).clamp(0, 255) as u8;
            record.ap = over(n.ap, r.ap);
            record.strength = over(n.strength, r.strength);
            record.magic = over(n.magic, r.magic);
            record.defense = over(n.defense, r.defense);
        }
    }
    write_camp_record(c, &mut record);
    record.write(d, 0);
    write_camp(c, items, &c.stock, d);
}

/// What the camp keeps of the save: config, shortcuts, the stock, seen marks
/// and the journal's. Shared by the room and the world map.
pub fn write_camp(
    c: &CampRes,
    items: &[Item],
    stock: &std::collections::BTreeMap<u16, u8>,
    d: &mut SaveData,
) {
    let mut p = Persistent::read(d);
    p.config = c.config_flags;
    p.shortcuts = c.shortcuts;
    p.no_charge = u32::from(c.no_charge);
    p.write(d);
    write_stock(items, stock, d);
    let mut seen = c.seen.clone();
    seen.extend(&c.starting);
    write_bits(d, state::SEEN_ITEMS, state::SEEN_ITEM_BYTES, &seen);
    d.set_u16(state::NOTICES_SEEN, c.memory.notices_seen);
    d.set_u16(state::STATUS_SEEN, c.status_seen);
    for (&character, (order, seen)) in &c.ability_marks {
        write_ability_marks(d, character, order, seen);
    }
    for (&form, (order, seen)) in &c.form_marks {
        write_form_marks(d, form, order, seen);
    }
    for (&key, &n) in &c.kills {
        if let Some(at) = state::kill_counter_at(key) {
            d.set_u32(at, n);
        }
    }
    for (&counter, &n) in &c.command_uses {
        d.set_u16(state::COMMAND_USES + counter as usize * 2, n);
    }
    // The journal's seen rows.
    write_bits(d, state::SEEN_CHARS, 0x44, &c.seen_chars);
    write_bits(d, state::SEEN_LIMITS, 4, &c.seen_limits);
    write_bits(d, state::SEEN_MINIGAMES, 0xc, &c.seen_minigames);
    write_bits(d, state::SEEN_QUESTS, 0x18, &c.seen_quests);
    write_bits(d, state::SEEN_CHESTS, 0x40, &c.seen_chests);
    write_bits(d, state::SEEN_MAPS, 0x10, &c.seen_maps);
    write_bits(d, state::SEEN_REPORTS, 4, &c.seen_reports);
    let synthesis: Vec<u16> = c.seen_synthesis.iter().map(|&s| u16::from(s)).collect();
    write_bits(d, state::SEEN_SYNTHESIS, 4, &synthesis);
    if let Some(j) = c.journal_source.as_ref() {
        let albums: Vec<i8> = j.albums.iter().map(|a| a.world).collect();
        let bits = per_world_bits(&albums, &c.seen_albums);
        write_bits(d, state::SEEN_ALBUMS, 0x40, &bits);
        let stories: Vec<i8> = j.stories.iter().map(|s| s.world).collect();
        let bits = per_world_bits(&stories, &c.seen_story);
        write_bits(d, state::SEEN_STORY, 0x40, &bits);
    }
    write_puzzles(&c.puzzles, d);
    d.seal();
}

/// FUN_1402ed110's table at 0x1405ba910: the card backgrounds and the flags
/// that offer them.
const BACKGROUNDS: [(u16, u16); 8] = [
    (0x892, 0x4825),
    (0, 0x892),
    (0, 0x892),
    (0, 0x892),
    (0, 0x1076),
    (0, 0x1076),
    (0, 0x1076),
    (0, 0x4825),
];

/// The byte a file's card draws behind it, picked among the rows the story
/// allows (FUN_140157350's draw is not the same stream here [I]), and the
/// bit Hollow Bastion sets once flag 0x108b is on.
fn write_background(d: &mut SaveData, draw: u64, test: impl Fn(u32) -> bool) {
    let rows: Vec<u8> = BACKGROUNDS
        .iter()
        .enumerate()
        .filter(|(_, row)| {
            let (first, second) = **row;
            (first != 0 && !test(u32::from(first))) || test(u32::from(second))
        })
        .map(|(i, _)| i as u8)
        .collect();
    if !rows.is_empty() {
        d.set_u8(field::BACKGROUND, rows[draw as usize % rows.len()]);
    }
    let bit = d.u8_at(field::WORLD) == 4 && test(0x108b);
    let flags = d.u8_at(field::BACKGROUND + 1) & !1 | u8::from(bit);
    d.set_u8(field::BACKGROUND + 1, flags);
}

/// One character's obtained-order list and seen bits (FUN_140368ed0): Sora's
/// at save+0x4278 / +0x41f8, the party's at +0x4318 + 0x60·(t−2) and bit
/// 0x30·(t−2) of +0x4204.
fn write_ability_marks(d: &mut SaveData, character: u8, order: &[u16], seen: &[bool]) {
    let (list, cap) = match character {
        1 => (state::ORDER_SORA, 80),
        2..=13 => (
            state::ORDER_PARTY + state::ORDER_PARTY_BYTES * (character as usize - 2),
            48,
        ),
        _ => return,
    };
    for i in 0..cap {
        d.set_u16(list + i * 2, order.get(i).copied().unwrap_or(0));
    }
    let (base, first) = match character {
        1 => (state::SEEN_SORA_ABILITIES, 0),
        _ => (state::SEEN_PARTY_ABILITIES, 0x30 * (character as usize - 2)),
    };
    for i in 0..cap {
        let n = first + i;
        let at = base + n / 8;
        let bit = 1 << (n % 8);
        let on = seen.get(i).copied().unwrap_or(false);
        let b = d.u8_at(at);
        d.set_u8(at, if on { b | bit } else { b & !bit });
    }
}

/// A form's block in the lists and seen bits: Lion (7) shares block 5, where
/// Antiform would sit (FUN_14036a0b0).
fn form_block(form: u8) -> usize {
    if form == 7 { 5 } else { form as usize - 1 }
}

/// A form's list and seen bits (save+0x4798 + 0x60·block, bit 0x30·block of
/// +0x424c).
fn write_form_marks(d: &mut SaveData, form: u8, order: &[u16], seen: &[bool]) {
    let block = form_block(form);
    for i in 0..48 {
        d.set_u16(
            state::ORDER_FORMS + state::ORDER_PARTY_BYTES * block + i * 2,
            order.get(i).copied().unwrap_or(0),
        );
        let n = 0x30 * block + i;
        let at = state::SEEN_FORM_ABILITIES + n / 8;
        let bit = 1 << (n % 8);
        let on = seen.get(i).copied().unwrap_or(false);
        let b = d.u8_at(at);
        d.set_u8(at, if on { b | bit } else { b & !bit });
    }
}

fn read_form_marks(d: &SaveData, form: u8) -> (Vec<u16>, Vec<bool>) {
    let block = form_block(form);
    let order: Vec<u16> = (0..48)
        .map(|i| d.u16_at(state::ORDER_FORMS + state::ORDER_PARTY_BYTES * block + i * 2))
        .take_while(|&id| id != 0)
        .collect();
    let seen = (0..order.len())
        .map(|i| {
            let n = 0x30 * block + i;
            d.u8_at(state::SEEN_FORM_ABILITIES + n / 8) >> (n % 8) & 1 != 0
        })
        .collect();
    (order, seen)
}

fn read_ability_marks(d: &SaveData, character: u8) -> Option<(Vec<u16>, Vec<bool>)> {
    let (list, cap, base, first) = match character {
        1 => (state::ORDER_SORA, 80, state::SEEN_SORA_ABILITIES, 0),
        2..=13 => (
            state::ORDER_PARTY + state::ORDER_PARTY_BYTES * (character as usize - 2),
            48,
            state::SEEN_PARTY_ABILITIES,
            0x30 * (character as usize - 2),
        ),
        _ => return None,
    };
    let order: Vec<u16> = (0..cap)
        .map(|i| d.u16_at(list + i * 2))
        .take_while(|&id| id != 0)
        .collect();
    let seen = (0..order.len())
        .map(|i| {
            let n = first + i;
            d.u8_at(base + n / 8) >> (n % 8) & 1 != 0
        })
        .collect();
    Some((order, seen))
}

/// FUN_1402ed0e0 for what the field keeps: Sora, the party, the forms, the
/// item slots and the magic. The room's own Shadow stays out of a loaded game.
pub fn apply_game(game: &mut Game, data: &SaveData) {
    let p = Persistent::read(data);
    let stock = read_stock(&game.save.items, data);
    game.save.base = data.clone();
    game.save.play_start = p.play_frames;
    game.save.world_frames = p.world_frames;
    game.save.world = Some(p.world);
    game.save.world_tick = 0;
    game.save.loaded = true;
    game.save.party_table = p.party_table;
    if let Some(rt) = game.enemy.as_mut() {
        rt.enabled = false;
    }
    // An image with no record for Sora (no game behind it) holds only the
    // story: the room keeps its own numbers.
    if p.records[0].max_hp == 0 {
        return;
    }
    game.config_flags = p.config;
    game.shortcuts = p.shortcuts;
    let r = p.records[0];
    let equipment = equipment_of(&r);
    game.ability_slots = r.abilities.to_vec();
    game.sora.vitals.level = r.level.max(1);
    let row = level_row(game, r.level);
    let g = &mut *game;
    if let Some(rt) = g.enemy.as_mut() {
        rt.exp_total = p.exp;
        rt.munny = p.munny;
        rt.difficulty = match p.difficulty {
            0 => combat::Difficulty::Beginner,
            2 => combat::Difficulty::Proud,
            3 => combat::Difficulty::Critical,
            _ => combat::Difficulty::Standard,
        };
        rt.bonus_taken = read_bits(data, state::BONUS_BITS, state::BONUS_BIT_BYTES)
            .into_iter()
            .map(|b| b as u8)
            .collect();
        let v = &mut g.sora.vitals;
        if let Some(row) = row {
            v.attacker.strength = i32::from(row.strength) + i32::from(r.strength);
            v.attacker.magic = i32::from(row.magic) + i32::from(r.magic);
            v.target.defense = i32::from(row.defense) + i32::from(r.defense);
        }
        v.target.max_hp = i32::from(r.max_hp);
        v.hp = i32::from(r.hp.min(r.max_hp));
    }
    let player = &mut g.sora.player;
    player.mp.max = i32::from(r.max_mp);
    player.mp.cur = i32::from(r.mp.min(r.max_mp));
    let [partial, level, max] = p.drive;
    player.drive.partial = partial;
    player.drive.level = level;
    player.drive.max = max;
    // The numbers above stand without the equipment: `refresh_gear` adds it.
    g.gear = player::equipment_stats::Totals::start(0, 0, 0, 0);
    g.equipment = equipment.clone();
    if let Some(d) = g.drive.as_mut() {
        for f in 1..=6u8 {
            let saved = p.forms[f as usize - 1];
            d.forms.owned[f as usize] = stock
                .get(&player::forms::FORM_ITEMS[f as usize])
                .is_some_and(|&n| n > 0);
            if let Some(to) = d.forms.record_mut(f) {
                to.keyblade = saved.weapon;
                to.level = saved.level.max(1);
                to.exp = saved.exp;
                to.abilities = saved.abilities;
            }
        }
        d.keyblades_changed(equipment.weapon);
    }
    if let Some(it) = g.items.as_mut() {
        it.slots.clone_from(&equipment.items);
        it.originals.clone_from(&equipment.originals);
        it.stock.clone_from(&stock);
    }
    if let Some(m) = g.magic.as_mut() {
        for (t, item) in ELEMENT_ITEMS.iter().enumerate() {
            m.levels[t] = stock.get(item).copied().unwrap_or(0);
        }
        m.dont_charge = p.no_charge as u8;
    }
    let Game {
        party,
        ability_source,
        party_reload,
        party_limits,
        ..
    } = g;
    for m in party.iter_mut() {
        let Some(r) = p
            .records
            .get(m.kind.character as usize - 1)
            .filter(|r| r.max_hp != 0)
        else {
            continue;
        };
        apply_member(m, r);
        m.gear = ability_source.gear(&equipment_of(r), None);
        m.abilities = party_room::member_abilities(ability_source, &m.save);
        if let Some(src) = party_reload.as_ref() {
            party_room::start_reweapon(src, party_limits, m);
        }
    }
    let slots = game.ability_slots.clone();
    drive_room::rebuild_abilities(game, Some(&slots));
    party_room::follow_party_table(game, p.world);
    let event = serde_json::json!({"event":"save_applied","world":p.world,"area":p.area,
        "entrance":p.entrance,"level":game.sora.vitals.level,"exp":p.exp,"munny":p.munny,
        "weapon":r.weapon,"difficulty":p.difficulty,"frames":p.play_frames,
        "hp":r.hp,"max_hp":r.max_hp,"mp":r.mp,"max_mp":r.max_mp,"stock":stock.len(),
        "party":game.party.iter().map(|m| m.save.weapon).collect::<Vec<_>>(),
        "flags":p.flags.iter().map(|w| w.count_ones()).sum::<u32>()});
    game.event(event);
}

/// A record's gear as the camp shows it.
fn equipment_of(r: &Record) -> pause_menu::items::Equipment {
    let take = |values: &[u16; 8], n: u8| values[..(n as usize).min(8)].to_vec();
    pause_menu::items::Equipment {
        weapon: r.weapon,
        secondary_weapon: r.second_weapon,
        armor: take(&r.armor, r.armor_slots),
        accessories: take(&r.accessories, r.accessory_slots),
        items: take(&r.items, r.item_slots),
        originals: take(&r.originals, r.item_slots),
        ..Default::default()
    }
}

/// What the camp's half of a load reads besides the image.
pub struct CampContext<'a> {
    pub items: &'a [Item],
    pub levels: &'a [data::battle_tables::PlayerLevel],
    pub weapon_abilities: &'a HashMap<u16, u16>,
    pub limits: &'a [second_deck::LimitRow],
    /// The EXP the Status page counts from.
    pub exp: u32,
}

impl<'a> CampContext<'a> {
    pub fn of(game: &'a Game, data: &SaveData) -> Self {
        Self {
            items: &game.save.items,
            levels: game.enemy.as_ref().map_or(&[], |rt| &rt.sora_levels),
            weapon_abilities: &game.ability_source.equipment_abilities,
            limits: game.second_deck.as_ref().map_or(&[], |d| &d.limits),
            exp: game
                .enemy
                .as_ref()
                .map_or(data.u32_at(state::EXP), |rt| rt.exp_total),
        }
    }
}

/// The same for what the camp keeps: its stock, gear, seen marks and the
/// journal's numbers.
pub fn apply_camp(c: &mut CampRes, ctx: &CampContext, data: &SaveData) {
    let p = Persistent::read(data);
    let r = p.records[0];
    if r.max_hp == 0 {
        return;
    }
    c.config_flags = p.config;
    c.game_mode = p.difficulty;
    c.shortcuts = p.shortcuts;
    c.no_charge = p.no_charge as u8;
    c.stock = read_stock(ctx.items, data);
    c.starting.clear();
    c.seen = read_bits(data, state::SEEN_ITEMS, state::SEEN_ITEM_BYTES);
    c.equipment = equipment_of(&r);
    c.ability_slots = r.abilities.to_vec();
    c.memory.notices_seen = data.u16_at(state::NOTICES_SEEN);
    c.status_seen = data.u16_at(state::STATUS_SEEN);
    c.ability_marks = (1..=13u8)
        .filter_map(|ch| {
            read_ability_marks(data, ch)
                .filter(|(order, _)| !order.is_empty())
                .map(|m| (ch, m))
        })
        .collect();
    c.kills = (1000..=1071u16)
        .chain(3000..=3011)
        .filter_map(|key| {
            let n = data.u32_at(state::kill_counter_at(key)?);
            (n != 0).then_some((key, n))
        })
        .collect();
    c.command_uses = (0..state::COMMAND_USE_COUNT as u16)
        .filter_map(|counter| {
            let n = data.u16_at(state::COMMAND_USES + counter as usize * 2);
            (n != 0).then_some((counter, n))
        })
        .collect();
    c.form_marks = [1u8, 2, 3, 4, 5, 7]
        .into_iter()
        .map(|f| (f, read_form_marks(data, f)))
        .filter(|(_, (order, _))| !order.is_empty())
        .collect();
    let row = ctx.levels.get(r.level.max(1) as usize - 1).copied();
    let exp = ctx.exp;
    let n = &mut c.source.numbers;
    n.level = r.level.max(1);
    if let Some(row) = row {
        n.next_exp = row.exp.saturating_sub(exp);
        n.ap = i32::from(row.ap) + i32::from(r.ap);
        n.strength = i32::from(row.strength) + i32::from(r.strength);
        n.magic = i32::from(row.magic) + i32::from(r.magic);
        n.defense = i32::from(row.defense) + i32::from(r.defense);
    }
    c.source.weapon_ability = ctx.weapon_abilities.get(&r.weapon).copied().unwrap_or(0);
    c.seen_chars = read_bits(data, state::SEEN_CHARS, 0x44);
    c.seen_limits = read_bits(data, state::SEEN_LIMITS, 4);
    c.seen_minigames = read_bits(data, state::SEEN_MINIGAMES, 0xc);
    c.seen_quests = read_bits(data, state::SEEN_QUESTS, 0x18);
    c.seen_chests = read_bits(data, state::SEEN_CHESTS, 0x40);
    c.seen_maps = read_bits(data, state::SEEN_MAPS, 0x10);
    c.seen_reports = read_bits(data, state::SEEN_REPORTS, 4);
    c.seen_synthesis = read_bits(data, state::SEEN_SYNTHESIS, 4)
        .into_iter()
        .map(|b| b as u8)
        .collect();
    if let Some(j) = c.journal_source.as_ref() {
        let albums: Vec<i8> = j.albums.iter().map(|a| a.world).collect();
        c.seen_albums = per_world_rows(&albums, &read_bits(data, state::SEEN_ALBUMS, 0x40));
        let stories: Vec<i8> = j.stories.iter().map(|s| s.world).collect();
        c.seen_story = per_world_rows(&stories, &read_bits(data, state::SEEN_STORY, 0x40));
    }
    c.puzzles = read_puzzles(data);
    c.journal_save = JournalSave::read(data, ctx.limits);
}

/// The journal's numbers from the image: nothing in a new game.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct JournalSave {
    /// Use counts by `limit` row's command (save+0x3d48 + 2 * the `limt`
    /// row's id, FUN_1403e7c30 / FUN_1403e8150).
    pub limit_counts: HashMap<u16, u16>,
    pub minigames: Vec<(u8, u32)>,
    pub menu_flags: Vec<u16>,
    pub synthesis: journal::Synthesis,
    pub opened_bits: Vec<u16>,
    pub story_flags: Vec<u16>,
}

impl JournalSave {
    pub fn read(data: &SaveData, limits: &[second_deck::LimitRow]) -> Self {
        let limit_counts = limits
            .iter()
            .map(|l| {
                (
                    l.command,
                    data.u16_at(state::LIMIT_COUNTS + 2 * usize::from(l.id)),
                )
            })
            .collect();
        let minigames = (0..state::MINIGAME_COUNT)
            .map(|i| {
                let at = state::MINIGAMES + i * 8;
                (data.u32_at(at) as u8, data.u32_at(at + 4))
            })
            .collect();
        let count =
            |at: usize, end: usize| (at..end).map(|a| data.u8_at(a).count_ones()).sum::<u32>();
        Self {
            limit_counts,
            minigames,
            menu_flags: read_bits(data, state::MENU_FLAGS, state::MENU_FLAG_WORDS * 4),
            synthesis: journal::Synthesis {
                moogle_seen: data.u32_at(state::SYNTHESIS_SEEN) & (1 | 1 << 10) != 0,
                level: data.u8_at(state::SYNTHESIS_LEVEL),
                materials: count(state::SYNTHESIS_MATERIALS, state::SYNTHESIS_COLLECTIONS),
                collections: count(state::SYNTHESIS_COLLECTIONS, state::SYNTHESIS_RECIPES),
                recipes: count(state::SYNTHESIS_RECIPES, state::SYNTHESIS_SEEN),
            },
            opened_bits: read_bits(data, state::CHESTS, state::CHEST_BYTES),
            story_flags: read_bits(data, state::FLAGS, state::FLAG_WORDS * 4),
        }
    }
}

#[cfg(test)]
mod party_swap_tests {
    use super::*;

    #[test]
    fn a_notice_names_who_came_and_who_went() {
        let old = [0, 1, 2, NO_MEMBER];
        let new = [0, 3 | 0x80, 2, 1];
        assert_eq!(party_changes(&new, &old), vec![3 | 0x80]);
        assert_eq!(party_changes(&old, &new), Vec::<u8>::new());
        let new = [0, 3, 2, NO_MEMBER];
        assert_eq!(party_changes(&old, &new), vec![1]);
    }

    const S: u8 = 0;
    const D: u8 = 1;
    const G: u8 = 2;
    const B: u8 = 12;

    /// The executable's five orders, worked by hand over FUN_14036b460.
    #[test]
    fn the_row_takes_each_camp_order_as_the_executable_does() {
        let cases = [
            ([S, G, D, B], [S, G, D, B], None),
            ([S, B, G, D], [S, B, G, D], Some((1, D))),
            ([S, D, B, G], [S, D, B, G], Some((2, G))),
            ([S, B, D, G], [S, B, D, G], Some((1, G))),
            ([S, G, B, D], [S, G, B, D], Some((2, D))),
        ];
        for (camp, want, leaver) in cases {
            let mut row = [S, D, G, B];
            let left = write_party_order(&mut row, &camp, true);
            assert_eq!(row, want, "camp {camp:?}");
            assert_eq!(
                left.map(|l| (l.slot, l.party_type)),
                leaver,
                "camp {camp:?}"
            );
        }
    }

    #[test]
    fn an_event_camp_swaps_bytes_only_and_keeps_the_lock() {
        let mut row = [S, D, G, B | 0x80];
        assert_eq!(
            write_party_order(&mut row, &[S, B | 0x80, G, D], false),
            None
        );
        assert_eq!(row, [S, B | 0x80, G, D]);
    }

    #[test]
    fn a_field_swap_drops_both_lock_bits() {
        let mut row = [S, D | 0x80, G, B | 0x80];
        to_reserve(&mut row, 1);
        assert_eq!(row, [S, B, G, D]);
    }

    #[test]
    fn no_reserve_means_no_leaver() {
        let mut row = [S, D, G, NO_MEMBER];
        assert_eq!(
            write_party_order(&mut row, &[S, G, D, NO_MEMBER], true),
            None
        );
        assert_eq!(row, [S, G, D, NO_MEMBER]);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn item(id: u16, kind: u8, flags: u8, slot: u16) -> Item {
        Item {
            id,
            kind,
            flags,
            amount: 0,
            weapon_stats: 0,
            name: 0,
            description: 0,
            buy: 0,
            sell: 0,
            command: 0,
            slot,
            picture: 0,
            box_style: 0,
            icon: 0,
        }
    }

    #[test]
    fn a_preset_keeps_a_swapped_row_and_a_member_held_is_locked() {
        // Sora, Goofy, Donald after a swap; preset 1 names the same members.
        let preset = u32::from_le_bytes([0, 1, 2, 0x12]);
        assert_eq!(
            row_with_presets([0, 2, 1, 0x12], &[preset]),
            [0, 2, 1, 0x12]
        );
        assert_eq!(
            row_with_presets([0; 4], &[preset]),
            data::member_table::apply_preset(data::member_table::NEW_RECORD, preset)
        );
        let row = [0, 1, 2, 0x12];
        assert!(party_locked(&row, 0, &[]));
        assert!(!party_locked(&row, 1, &[]));
        assert!(party_locked(&row, 1, &[1]));
        assert!(party_locked(&[0, 0x81, 2, 0x12], 1, &[]));
    }

    #[test]
    fn the_world_table_takes_a_swap_and_gives_the_order() {
        let mut row = ROOM_PARTY;
        // Goofy, then Donald: both active slots changed.
        let camp = camp_row(&row, &[3, 2]);
        write_party_order(&mut row, &camp, true);
        assert_eq!(row, [0, 2, 1, 0x12]);
        assert_eq!(party_order(&row, &[2, 3]), vec![1, 0]);
        assert_eq!(party_order(&row, &[3, 2]), vec![0, 1]);
        // The same order leaves it.
        let camp = camp_row(&row, &[3, 2]);
        write_party_order(&mut row, &camp, true);
        assert_eq!(row, [0, 2, 1, 0x12]);
        // A guest in the reserve slot; Goofy locked stays with his bit.
        let mut row = [0, 1, 2 | 0x80, 3];
        assert!(
            party_locked(&row, 0, &[]) && party_locked(&row, 2, &[]) && !party_locked(&row, 1, &[])
        );
        let camp = camp_row(&row, &[3, 2]);
        write_party_order(&mut row, &camp, true);
        assert_eq!(row, [0, 2 | 0x80, 1, 3]);
        // No table yet: the room's.
        let table = [[0; 4]; 19];
        assert_eq!(world_party(&table, 2), ROOM_PARTY);
    }

    #[test]
    fn stock_round_trips_by_count_byte_and_flag_bit() {
        let items = [
            item(1, 0, 0, 5),
            item(2, 17, 1, 9),
            item(3, 0x13, 0, 6),
            item(4, 0, 0, 7),
        ];
        let mut stock = std::collections::BTreeMap::new();
        stock.insert(1, 3);
        stock.insert(2, 1);
        // Type 0x13 has no count, whatever the room's map says.
        stock.insert(3, 2);
        let mut d = SaveData::blank();
        // A count byte no row names is left alone; a stale one is cleared.
        d.set_u8(state::ITEM_COUNTS + 8, 9);
        d.set_u8(state::ITEM_COUNTS + 7, 4);
        write_stock(&items, &stock, &mut d);
        assert_eq!(d.u8_at(state::ITEM_COUNTS + 5), 3);
        assert_eq!(d.u8_at(state::ITEM_COUNTS + 6), 0);
        assert_eq!(d.u8_at(state::ITEM_COUNTS + 7), 0);
        assert_eq!(d.u8_at(state::ITEM_COUNTS + 8), 9);
        assert_eq!(d.u8_at(state::ITEM_BITS + 1), 1 << 1);
        let back = read_stock(&items, &d);
        assert_eq!(back.get(&1), Some(&3));
        assert_eq!(back.get(&2), Some(&1));
        assert_eq!(back.get(&3), None);
    }

    #[test]
    fn the_card_background_follows_the_story_flags() {
        let mut d = SaveData::blank();
        d.set_u8(field::WORLD, 4);
        // Nothing set: only the first row (its first flag is clear).
        write_background(&mut d, 5, |_| false);
        assert_eq!(d.u8_at(field::BACKGROUND), 0);
        assert_eq!(d.u8_at(field::BACKGROUND + 1), 0);
        // 0x892 set: the first row needs 0x4825 now; rows 1..=3 open.
        write_background(&mut d, 5, |f| f == 0x892);
        assert_eq!(d.u8_at(field::BACKGROUND), 3);
        write_background(&mut d, 0, |f| f == 0x108b);
        assert_eq!(d.u8_at(field::BACKGROUND + 1), 1);
    }

    #[test]
    fn per_world_rows_go_through_the_save_bits_and_back() {
        // Rows of worlds 2, 3, 2, 2: row 3 is world 2's third.
        let worlds = [2i8, 3, 2, 2];
        let bits = per_world_bits(&worlds, &[1, 3]);
        assert_eq!(bits, vec![3 * 32, 2 * 32 + 2]);
        assert_eq!(per_world_rows(&worlds, &bits), vec![1, 3]);
    }

    #[test]
    fn puzzles_round_trip() {
        let mut save = journal::PuzzleSave::default();
        save.owned.insert((1, 40));
        save.seen_board.insert((9, 63));
        save.seen_list.insert((0, 0));
        save.completed = 0x21;
        save.tutorial = true;
        save.slots[2][47] = journal::PuzzleSlot { piece: 5, rot: 3 };
        let mut d = SaveData::blank();
        write_puzzles(&save, &mut d);
        // `u32 at (puzzle * 2 + (piece >> 5)) * 4`, bit `piece & 31`.
        assert_eq!(d.u32_at(state::PIECES_OWNED + 3 * 4), 1 << 8);
        let back = read_puzzles(&d);
        assert_eq!(back.owned, save.owned);
        assert_eq!(back.seen_board, save.seen_board);
        assert_eq!(back.seen_list, save.seen_list);
        assert_eq!(back.completed, 0x21);
        assert!(back.tutorial);
        assert_eq!(back.slots, save.slots);
    }

    #[test]
    fn form_marks_sit_in_each_forms_block() {
        let mut d = SaveData::blank();
        write_form_marks(&mut d, 2, &[7, 8], &[true, false]);
        write_form_marks(&mut d, 7, &[9], &[true]);
        assert_eq!(d.u16_at(state::ORDER_FORMS + 0x60 + 2), 8);
        assert_eq!(d.u16_at(state::ORDER_FORMS + 5 * 0x60), 9);
        // Bit 0x30 of +0x424c, and bit 0x30·5 for Lion.
        assert_eq!(d.u8_at(state::SEEN_FORM_ABILITIES + 6), 1);
        assert_eq!(d.u8_at(state::SEEN_FORM_ABILITIES + 30), 1);
        assert_eq!(read_form_marks(&d, 2), (vec![7, 8], vec![true, false]));
        assert_eq!(read_form_marks(&d, 7), (vec![9], vec![true]));
    }

    #[test]
    fn ability_marks_sit_in_each_characters_bits() {
        let mut d = SaveData::blank();
        write_ability_marks(&mut d, 1, &[7, 8], &[true, false]);
        write_ability_marks(&mut d, 3, &[9], &[true]);
        assert_eq!(d.u16_at(state::ORDER_SORA + 2), 8);
        assert_eq!(d.u16_at(state::ORDER_PARTY + 0x60), 9);
        // Goofy (3): bit 0x30 of the party's block.
        assert_eq!(d.u8_at(state::SEEN_PARTY_ABILITIES + 6), 1);
        assert_eq!(
            read_ability_marks(&d, 1),
            Some((vec![7, 8], vec![true, false]))
        );
        assert_eq!(read_ability_marks(&d, 3), Some((vec![9], vec![true])));
    }
}
