//! The part of the save image the rewrite models, read out of and written
//! into the 0x10fc0-byte image. Everything else in the image is left as it
//! was loaded, so a save from the original game keeps its other fields
//! when the rewrite saves over it.
//!
//! Offsets are `address - 0x1409a98b0` (the live structure, FUN_140152380);
//! each cites where the exe reads or writes it.

use crate::{data::SaveData, slots::field};

/// Program table `{map, btl, evt}` i16 per `world * 0x40 + area`
/// (`DAT_1409a98c0`, cleared by FUN_140152030 as 0x4c0 entries of 6 bytes).
pub const PROGRAMS: usize = 0x10;
pub const PROGRAM_ENTRIES: usize = 0x4c0;
/// Story flag words (`DAT_1409ab540`; 19 worlds of 8 words are saved, the
/// exe's flag test can index further, which only matters to the flags
/// outside the table).
pub const FLAGS: usize = 0x1c90;
pub const FLAG_WORDS: usize = 19 * 8;
/// Per-world arif set byte (FUN_1403ab470: `DAT_1409abc9c[world]`).
pub const ARIF: usize = 0x23ec;
/// Per-world record words (FUN_1403f1e20: `DAT_1409ad89c + world * 4`:
/// world map state, landing set, progress value, flags).
pub const WORLD_RECORDS: usize = 0x3fec;
/// World-point (save point) bits (FUN_1403f2270: `DAT_1409ab7a0`).
pub const WORLD_POINTS: usize = 0x1ef0;
pub const WORLD_POINT_WORDS: usize = 2;
/// Menu flag words (FUN_1403e38f0: `DAT_1409abc40`).
pub const MENU_FLAGS: usize = 0x2390;
pub const MENU_FLAG_WORDS: usize = 7;
/// World-event bits (FUN_1403deab0: `DAT_1409ad8e8`).
pub const WORLD_EVENTS: usize = 0x4038;
pub const WORLD_EVENT_WORDS: usize = 8;
/// Queued world events (`DAT_1409ad908`) and their `u8` count
/// (`DAT_1409ada07`).
pub const EVENT_QUEUE: usize = 0x4058;
pub const EVENT_QUEUE_COUNT: usize = 0x4157;
/// Symbol slots 1..=0x15 (FUN_1403f1e10: `DAT_1409ada08 + slot − 1`).
pub const SYMBOL_SLOTS: usize = 0x4158;
pub const SYMBOL_SLOT_COUNT: usize = 0x15;
/// Episode open and NEW bits (`DAT_1409ada28`, `DAT_1409ada38`).
pub const EPISODES: usize = 0x4178;
pub const EPISODES_NEW: usize = 0x4188;
pub const EPISODE_WORDS: usize = 4;
/// The world map's draw-group set (`DAT_1409ada49`, FUN_1403f0b70).
pub const DRAW_GROUP_SET: usize = 0x4199;
/// Shared EXP (camp status "Experience", FUN_140354340).
pub const EXP: usize = 0x36e0;
/// Shortcut command ids (camp Customize, `DAT_1409acfa8`).
pub const SHORTCUTS: usize = 0x36f8;
/// Sora's party record (record 1): equipped weapon item `u16`, level at
/// +0xf, 80 ability slots `u16` at +0x54 (bit 15 equipped).
pub const SORA: usize = field::PARTY;
pub const ABILITY_SLOTS: usize = 80;
pub const ABILITIES: usize = SORA + 0x54;
/// The party records (FUN_1403a0e70 resets 13; FUN_1403a14e0 finds record
/// `character - 1`).
pub const RECORDS: usize = 13;
/// Form records (FUN_1403a1410: `+0x24f0 + 0xe04 + 0x38 * (form - 1)`).
pub const FORMS: usize = field::PARTY + 0xe04;
pub const FORM_BYTES: usize = 0x38;
pub const FORM_COUNT: usize = 10;
pub const FORM_ABILITY_SLOTS: usize = 0x18;
/// The form Sora is in (`DAT_1409acdd4`).
pub const CURRENT_FORM: usize = 0x3524;
/// Sora's gauge as his status reads it (FUN_1403c1d90 through +0x1038 of
/// the save's party block): partial, full bars, maximum bars.
pub const DRIVE: usize = field::PARTY + 0x1038;
/// Each world's party, 4 bytes (FUN_1403e2e20: `DAT_1409acde4 + world * 4`).
pub const PARTY_TABLE: usize = 0x3534;
/// Item counts by a record's slot (FUN_1403c4720: `DAT_1409ace30`) and the
/// one-bit items after them (`DAT_1409acf70`).
pub const ITEM_COUNTS: usize = 0x3580;
pub const ITEM_COUNT_BYTES: usize = 0x140;
pub const ITEM_BITS: usize = 0x36c0;
pub const ITEM_BIT_BYTES: usize = 0x20;
/// Bonus levels taken and their bits by id (FUN_1403edca0: `DAT_1409acfb0`,
/// `DAT_1409acfb4`).
pub const BONUS_COUNT: usize = 0x3700;
pub const BONUS_BITS: usize = 0x3704;
pub const BONUS_BIT_BYTES: usize = 0x20;
/// "Don't MP Charge" by magic type (camp Customize, `DAT_1409acff4`).
pub const NO_CHARGE: usize = 0x3744;
/// World play time, one `u32` per world (FUN_1401574e0).
pub const WORLD_FRAMES: usize = 0x244c;
/// Opened-chest bits (journal treasure pages).
pub const CHESTS: usize = 0x23ac;
pub const CHEST_BYTES: usize = 0x40;
/// Destination block words (`DAT_1409ab7a8`, two `u16` per 'dsa' entry).
pub const DSA: usize = 0x1ef8;
/// Limit use counters (`u16` by record), minigame records (`{state, value}`
/// of 8 bytes) and the synthesis fields the journal reads.
pub const LIMIT_COUNTS: usize = 0x3d48;
pub const MINIGAMES: usize = 0x3db4;
pub const MINIGAME_COUNT: usize = 0x40;
pub const SYNTHESIS_MATERIALS: usize = 0x49f4;
pub const SYNTHESIS_COLLECTIONS: usize = 0x49fc;
pub const SYNTHESIS_RECIPES: usize = 0x4a10;
pub const SYNTHESIS_SEEN: usize = 0x4a28;
pub const SYNTHESIS_LEVEL: usize = 0x4c34;
/// Seen bits of the stock list and the abilities (camp badges, T255).
pub const SEEN_ITEMS: usize = 0x41a8;
pub const SEEN_ITEM_BYTES: usize = 0x50;
pub const SEEN_SORA_ABILITIES: usize = 0x41f8;
pub const SEEN_PARTY_ABILITIES: usize = 0x4204;
pub const SEEN_ABILITY_BYTES: usize = 8;
pub const ORDER_SORA: usize = 0x4278;
pub const ORDER_PARTY: usize = 0x4318;
pub const ORDER_PARTY_BYTES: usize = 0x60;
/// The forms' obtained-order lists, 48 ids per block (`+0x60 * (form - 1)`,
/// Lion's block 5) and their seen bits (FUN_140368ed0, FUN_14036a0b0).
pub const ORDER_FORMS: usize = 0x4798;
pub const SEEN_FORM_ABILITIES: usize = 0x424c;
/// The tutorial notices seen (FUN_140304e50 rows).
pub const NOTICES_SEEN: usize = 0x4270;
/// The forms' (bits 0..4) and Summons' (5..8) seen bits (FUN_14036a260).
pub const STATUS_SEEN: usize = 0x4274;
/// The journal block `J` and its seen bits (FUN_140345420).
pub const JOURNAL: usize = 0x4c38;
pub const SEEN_CHARS: usize = JOURNAL;
pub const SEEN_ALBUMS: usize = 0x4c7c;
pub const SEEN_LIMITS: usize = 0x4cbc;
pub const SEEN_MINIGAMES: usize = 0x4cc0;
pub const SEEN_QUESTS: usize = 0x4ccc;
pub const SEEN_STORY: usize = 0x4ce4;
pub const SEEN_CHESTS: usize = 0x4d24;
pub const SEEN_MAPS: usize = 0x4d64;
pub const SEEN_REPORTS: usize = 0x4d74;
pub const SEEN_SYNTHESIS: usize = 0x4d78;
/// Puzzles: pieces owned (`u32` at `(puzzle * 2 + (piece >> 5)) * 4`), seen
/// on a board (J+0x144) and in the list (J+0x194), the completed mask
/// (J+0x1e4), the tutorial bit (J+0x1e6) and 10 x 48 slots of 4 bytes
/// (J+0x1e8: piece, rotation).
pub const PIECES_OWNED: usize = 0x24a0;
pub const PIECES_SEEN_BOARD: usize = JOURNAL + 0x144;
pub const PIECES_SEEN_LIST: usize = JOURNAL + 0x194;
pub const PUZZLE_COUNT: usize = 10;
pub const PUZZLE_COMPLETED: usize = JOURNAL + 0x1e4;
pub const PUZZLE_TUTORIAL: usize = JOURNAL + 0x1e6;
pub const PUZZLE_SLOTS: usize = JOURNAL + 0x1e8;
pub const PUZZLE_SLOT_COUNT: usize = 48;

/// A party record, 0x114 bytes (FUN_1403c1d90 reads the status numbers and
/// the gear; FUN_1403a0e70 is its reset).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Record {
    pub weapon: u16,
    pub second_weapon: u16,
    pub hp: u8,
    pub max_hp: u8,
    pub mp: u8,
    pub max_mp: u8,
    /// +8..+0xb: what the items of bonus add to the level row.
    pub ap: u8,
    pub strength: u8,
    pub magic: u8,
    pub defense: u8,
    /// +0xc / +0xd: the numbers FUN_1403c1d90 scales into its +0x1bc / +0x1c0.
    pub scaled: [u8; 2],
    /// +0xe: the level-up ability column (FUN_1403ecf30).
    pub growth_column: u8,
    pub level: u8,
    pub armor_slots: u8,
    pub accessory_slots: u8,
    pub item_slots: u8,
    pub armor: [u16; 8],
    pub accessories: [u16; 8],
    pub items: [u16; 8],
    /// +0x44: the auto-reload originals.
    pub originals: [u16; 8],
    /// Bit 15 set: equipped.
    pub abilities: [u16; ABILITY_SLOTS],
    /// +0xf4: the battle style, +0xf5: the frequencies.
    pub style: u8,
    pub frequency: [u8; 31],
}

impl Default for Record {
    fn default() -> Self {
        Self {
            weapon: 0,
            second_weapon: 0,
            hp: 0,
            max_hp: 0,
            mp: 0,
            max_mp: 0,
            ap: 0,
            strength: 0,
            magic: 0,
            defense: 0,
            scaled: [0; 2],
            growth_column: 0,
            level: 1,
            armor_slots: 0,
            accessory_slots: 0,
            item_slots: 0,
            armor: [0; 8],
            accessories: [0; 8],
            items: [0; 8],
            originals: [0; 8],
            abilities: [0; ABILITY_SLOTS],
            style: 0,
            frequency: [0; 31],
        }
    }
}

fn u16s<const N: usize>(data: &SaveData, at: usize) -> [u16; N] {
    std::array::from_fn(|i| data.u16_at(at + i * 2))
}

fn set_u16s(data: &mut SaveData, at: usize, values: &[u16]) {
    for (i, v) in values.iter().enumerate() {
        data.set_u16(at + i * 2, *v);
    }
}

impl Record {
    /// Record `index` (`character - 1`, Sora 0).
    pub fn read(data: &SaveData, index: usize) -> Self {
        let at = field::PARTY + index * field::PARTY_RECORD;
        Self {
            weapon: data.u16_at(at),
            second_weapon: data.u16_at(at + 2),
            hp: data.u8_at(at + 4),
            max_hp: data.u8_at(at + 5),
            mp: data.u8_at(at + 6),
            max_mp: data.u8_at(at + 7),
            ap: data.u8_at(at + 8),
            strength: data.u8_at(at + 9),
            magic: data.u8_at(at + 0xa),
            defense: data.u8_at(at + 0xb),
            scaled: [data.u8_at(at + 0xc), data.u8_at(at + 0xd)],
            growth_column: data.u8_at(at + 0xe),
            level: data.u8_at(at + field::RECORD_LEVEL),
            armor_slots: data.u8_at(at + 0x10),
            accessory_slots: data.u8_at(at + 0x11),
            item_slots: data.u8_at(at + 0x12),
            armor: u16s(data, at + 0x14),
            accessories: u16s(data, at + 0x24),
            items: u16s(data, at + 0x34),
            originals: u16s(data, at + 0x44),
            abilities: u16s(data, at + 0x54),
            style: data.u8_at(at + 0xf4),
            frequency: std::array::from_fn(|i| data.u8_at(at + 0xf5 + i)),
        }
    }

    pub fn write(&self, data: &mut SaveData, index: usize) {
        let at = field::PARTY + index * field::PARTY_RECORD;
        data.set_u16(at, self.weapon);
        data.set_u16(at + 2, self.second_weapon);
        for (k, v) in [
            self.hp,
            self.max_hp,
            self.mp,
            self.max_mp,
            self.ap,
            self.strength,
            self.magic,
            self.defense,
            self.scaled[0],
            self.scaled[1],
            self.growth_column,
            self.level,
            self.armor_slots,
            self.accessory_slots,
            self.item_slots,
        ]
        .into_iter()
        .enumerate()
        {
            data.set_u8(at + 4 + k, v);
        }
        set_u16s(data, at + 0x14, &self.armor);
        set_u16s(data, at + 0x24, &self.accessories);
        set_u16s(data, at + 0x34, &self.items);
        set_u16s(data, at + 0x44, &self.originals);
        set_u16s(data, at + 0x54, &self.abilities);
        data.set_u8(at + 0xf4, self.style);
        for (i, v) in self.frequency.iter().enumerate() {
            data.set_u8(at + 0xf5 + i, *v);
        }
    }
}

/// A form record: its Keyblade, level, EXP and 24 ability slots (FUN_1403e8380).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct FormRecord {
    pub weapon: u16,
    pub level: u8,
    pub exp: i32,
    pub abilities: [u16; FORM_ABILITY_SLOTS],
}

impl FormRecord {
    /// `form` is 1-based, as in FUN_1403a1410.
    pub fn read(data: &SaveData, form: usize) -> Self {
        let at = FORMS + (form - 1) * FORM_BYTES;
        Self {
            weapon: data.u16_at(at),
            level: data.u8_at(at + 2),
            exp: data.u32_at(at + 4) as i32,
            abilities: u16s(data, at + 8),
        }
    }

    pub fn write(&self, data: &mut SaveData, form: usize) {
        let at = FORMS + (form - 1) * FORM_BYTES;
        data.set_u16(at, self.weapon);
        data.set_u8(at + 2, self.level);
        data.set_u32(at + 4, self.exp as u32);
        set_u16s(data, at + 8, &self.abilities);
    }
}

/// The set bit numbers of `bytes` bytes at `at`.
pub fn read_bits(data: &SaveData, at: usize, bytes: usize) -> Vec<u16> {
    (0..bytes * 8)
        .filter(|&n| data.u8_at(at + n / 8) >> (n % 8) & 1 != 0)
        .map(|n| n as u16)
        .collect()
}

/// Rewrites `bytes` bytes at `at` to hold exactly the bit numbers `bits`;
/// numbers past the end are dropped.
pub fn write_bits(data: &mut SaveData, at: usize, bytes: usize, bits: &[u16]) {
    for i in 0..bytes {
        data.set_u8(at + i, 0);
    }
    for &n in bits {
        let n = n as usize;
        if n < bytes * 8 {
            let b = data.u8_at(at + n / 8);
            data.set_u8(at + n / 8, b | 1 << (n % 8));
        }
    }
}
/// Config bits (vibration bit 0, camera bits 1..2; FUN_1402e5000).
pub const CONFIG: usize = 0x41a4;

/// The rewrite's persistent state.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Persistent {
    pub world: u8,
    pub area: u8,
    pub entrance: u8,
    pub play_frames: u32,
    pub munny: u32,
    pub difficulty: u32,
    pub roxas: bool,
    pub exp: u32,
    /// The party records, Sora's first.
    pub records: Vec<Record>,
    pub forms: Vec<FormRecord>,
    pub current_form: u8,
    pub drive: [u8; 3],
    /// Each world's party bytes.
    pub party_table: [[u8; 4]; 19],
    pub bonus_count: u8,
    pub no_charge: u32,
    pub world_frames: [u32; 19],
    pub shortcuts: [u16; 4],
    pub config: u32,
    pub programs: Vec<[i16; 3]>,
    pub flags: Vec<u32>,
    pub arif: [u8; 19],
    pub world_records: [[u8; 4]; 19],
    pub world_points: [u32; WORLD_POINT_WORDS],
    pub menu_flags: [u32; MENU_FLAG_WORDS],
    pub world_events: [u32; WORLD_EVENT_WORDS],
    pub event_queue: Vec<u8>,
    pub symbol_slots: [u8; SYMBOL_SLOT_COUNT],
    pub episodes: [u32; EPISODE_WORDS],
    pub episodes_new: [u32; EPISODE_WORDS],
    pub draw_group_set: u8,
}

impl Default for Persistent {
    fn default() -> Self {
        Self {
            world: 0,
            area: 0,
            entrance: 0,
            play_frames: 0,
            munny: 0,
            difficulty: 1,
            roxas: false,
            exp: 0,
            records: vec![Record::default(); RECORDS],
            forms: vec![FormRecord::default(); FORM_COUNT],
            current_form: 0,
            drive: [0; 3],
            party_table: [[0; 4]; 19],
            bonus_count: 0,
            no_charge: 0,
            world_frames: [0; 19],
            shortcuts: [0; 4],
            config: 0,
            programs: vec![[0; 3]; PROGRAM_ENTRIES],
            flags: vec![0; FLAG_WORDS],
            arif: [0; 19],
            world_records: [[0; 4]; 19],
            world_points: [0; WORLD_POINT_WORDS],
            menu_flags: [0; MENU_FLAG_WORDS],
            world_events: [0; WORLD_EVENT_WORDS],
            event_queue: Vec::new(),
            symbol_slots: [0; SYMBOL_SLOT_COUNT],
            episodes: [0; EPISODE_WORDS],
            episodes_new: [0; EPISODE_WORDS],
            draw_group_set: 0,
        }
    }
}

fn words<const N: usize>(data: &SaveData, at: usize) -> [u32; N] {
    std::array::from_fn(|i| data.u32_at(at + i * 4))
}

fn set_words(data: &mut SaveData, at: usize, words: &[u32]) {
    for (i, w) in words.iter().enumerate() {
        data.set_u32(at + i * 4, *w);
    }
}

impl Persistent {
    pub fn read(data: &SaveData) -> Self {
        let mut shortcuts = [0u16; 4];
        for (i, s) in shortcuts.iter_mut().enumerate() {
            *s = data.u16_at(SHORTCUTS + i * 2);
        }
        let programs = (0..PROGRAM_ENTRIES)
            .map(|i| {
                let at = PROGRAMS + i * 6;
                [
                    data.u16_at(at) as i16,
                    data.u16_at(at + 2) as i16,
                    data.u16_at(at + 4) as i16,
                ]
            })
            .collect();
        let flags = (0..FLAG_WORDS)
            .map(|i| data.u32_at(FLAGS + i * 4))
            .collect();
        let mut arif = [0u8; 19];
        for (i, a) in arif.iter_mut().enumerate() {
            *a = data.u8_at(ARIF + i);
        }
        let world_records =
            std::array::from_fn(|i| std::array::from_fn(|k| data.u8_at(WORLD_RECORDS + i * 4 + k)));
        let queued = (data.u8_at(EVENT_QUEUE_COUNT) as usize).min(EVENT_QUEUE_COUNT - EVENT_QUEUE);
        let event_queue = (0..queued).map(|i| data.u8_at(EVENT_QUEUE + i)).collect();
        Self {
            world: data.u8_at(field::WORLD),
            area: data.u8_at(field::AREA),
            entrance: data.u8_at(field::ENTRANCE),
            play_frames: data.u32_at(field::PLAY_FRAMES),
            munny: data.u32_at(field::MUNNY),
            difficulty: data.u32_at(field::DIFFICULTY),
            roxas: data.u8_at(field::ROXAS) & 1 != 0,
            exp: data.u32_at(EXP),
            records: (0..RECORDS).map(|i| Record::read(data, i)).collect(),
            forms: (1..=FORM_COUNT)
                .map(|f| FormRecord::read(data, f))
                .collect(),
            current_form: data.u8_at(CURRENT_FORM),
            drive: std::array::from_fn(|i| data.u8_at(DRIVE + i)),
            party_table: std::array::from_fn(|w| {
                std::array::from_fn(|k| data.u8_at(PARTY_TABLE + w * 4 + k))
            }),
            bonus_count: data.u8_at(BONUS_COUNT),
            no_charge: data.u32_at(NO_CHARGE),
            world_frames: words(data, WORLD_FRAMES),
            shortcuts,
            config: data.u32_at(CONFIG),
            programs,
            flags,
            arif,
            world_records,
            world_points: words(data, WORLD_POINTS),
            menu_flags: words(data, MENU_FLAGS),
            world_events: words(data, WORLD_EVENTS),
            event_queue,
            symbol_slots: std::array::from_fn(|i| data.u8_at(SYMBOL_SLOTS + i)),
            episodes: words(data, EPISODES),
            episodes_new: words(data, EPISODES_NEW),
            draw_group_set: data.u8_at(DRAW_GROUP_SET),
        }
    }

    /// Writes the modeled fields and seals the checksum.
    pub fn write(&self, data: &mut SaveData) {
        data.set_u8(field::WORLD, self.world);
        data.set_u8(field::AREA, self.area);
        data.set_u8(field::ENTRANCE, self.entrance);
        data.set_u32(field::PLAY_FRAMES, self.play_frames);
        data.set_u32(field::MUNNY, self.munny);
        data.set_u32(field::DIFFICULTY, self.difficulty);
        let roxas = data.u8_at(field::ROXAS) & !1 | u8::from(self.roxas);
        data.set_u8(field::ROXAS, roxas);
        data.set_u32(EXP, self.exp);
        for (i, r) in self.records.iter().take(RECORDS).enumerate() {
            r.write(data, i);
        }
        for (i, f) in self.forms.iter().take(FORM_COUNT).enumerate() {
            f.write(data, i + 1);
        }
        data.set_u8(CURRENT_FORM, self.current_form);
        for (i, v) in self.drive.iter().enumerate() {
            data.set_u8(DRIVE + i, *v);
        }
        for (w, row) in self.party_table.iter().enumerate() {
            for (k, v) in row.iter().enumerate() {
                data.set_u8(PARTY_TABLE + w * 4 + k, *v);
            }
        }
        data.set_u8(BONUS_COUNT, self.bonus_count);
        data.set_u32(NO_CHARGE, self.no_charge);
        set_words(data, WORLD_FRAMES, &self.world_frames);
        for (i, s) in self.shortcuts.iter().enumerate() {
            data.set_u16(SHORTCUTS + i * 2, *s);
        }
        data.set_u32(CONFIG, self.config);
        for (i, p) in self.programs.iter().take(PROGRAM_ENTRIES).enumerate() {
            let at = PROGRAMS + i * 6;
            for (k, v) in p.iter().enumerate() {
                data.set_u16(at + k * 2, *v as u16);
            }
        }
        for (i, w) in self.flags.iter().take(FLAG_WORDS).enumerate() {
            data.set_u32(FLAGS + i * 4, *w);
        }
        for (i, a) in self.arif.iter().enumerate() {
            data.set_u8(ARIF + i, *a);
        }
        for (i, r) in self.world_records.iter().enumerate() {
            for (k, b) in r.iter().enumerate() {
                data.set_u8(WORLD_RECORDS + i * 4 + k, *b);
            }
        }
        set_words(data, WORLD_POINTS, &self.world_points);
        set_words(data, MENU_FLAGS, &self.menu_flags);
        set_words(data, WORLD_EVENTS, &self.world_events);
        let queued = self.event_queue.len().min(EVENT_QUEUE_COUNT - EVENT_QUEUE);
        for (i, id) in self.event_queue.iter().take(queued).enumerate() {
            data.set_u8(EVENT_QUEUE + i, *id);
        }
        data.set_u8(EVENT_QUEUE_COUNT, queued as u8);
        for (i, s) in self.symbol_slots.iter().enumerate() {
            data.set_u8(SYMBOL_SLOTS + i, *s);
        }
        set_words(data, EPISODES, &self.episodes);
        set_words(data, EPISODES_NEW, &self.episodes_new);
        data.set_u8(DRAW_GROUP_SET, self.draw_group_set);
        data.seal();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn write_then_read_round_trips_and_keeps_other_bytes() {
        let mut image = SaveData::blank();
        // A byte this module does not model survives a write.
        image.set_u8(0x5000, 0x77);
        image.seal();
        let mut p = Persistent {
            world: 2,
            area: 8,
            entrance: 3,
            play_frames: 216_000 + 3600 * 7,
            munny: 4321,
            difficulty: 2,
            roxas: true,
            exp: 9999,
            shortcuts: [3, 0, 7, 9],
            config: 0b101,
            ..Persistent::default()
        };
        p.records[0].level = 14;
        p.records[0].weapon = 0x29;
        p.records[0].abilities[0] = 0x8000 | 0x62;
        p.records[0].abilities[79] = 5;
        p.records[2].armor[3] = 0x30;
        p.records[2].frequency[30] = 2;
        p.forms[3] = FormRecord {
            weapon: 0x7a,
            level: 4,
            exp: 77,
            abilities: [9; FORM_ABILITY_SLOTS],
        };
        p.current_form = 5;
        p.drive = [40, 3, 7];
        p.party_table[2] = [1, 2, 0x12, 0x12];
        p.bonus_count = 2;
        p.no_charge = 5;
        p.world_frames[18] = 600;
        p.programs[2 * 0x40 + 8] = [3, -1, 7];
        p.flags[2 * 8 + 1] = 0xdead_beef;
        p.arif[2] = 4;
        p.world_records[2] = [2, 1, 9, 3];
        p.world_points = [1 << 13, 1 << 9];
        p.menu_flags[6] = 0x8000_0001;
        p.world_events[0] = 1;
        p.event_queue = vec![2, 7];
        p.symbol_slots[20] = 2;
        p.episodes[1] = 2;
        p.episodes_new[3] = 0x10;
        p.draw_group_set = 9;
        p.write(&mut image);
        assert!(SaveData::from_bytes(image.bytes()).is_ok());
        assert_eq!(Persistent::read(&image), p);
        assert_eq!(image.u8_at(0x5000), 0x77);
    }

    #[test]
    fn bits_round_trip_and_only_their_block_changes() {
        let mut image = SaveData::blank();
        image.set_u8(0x101, 0xff);
        write_bits(&mut image, 0x100, 1, &[0, 7, 8, 500]);
        assert_eq!(image.u8_at(0x100), 0x81);
        assert_eq!(image.u8_at(0x101), 0xff);
        assert_eq!(read_bits(&image, 0x100, 1), vec![0, 7]);
    }

    #[test]
    fn writing_the_roxas_flag_keeps_the_other_bits() {
        let mut image = SaveData::blank();
        image.set_u8(field::ROXAS, 0b10);
        let mut p = Persistent::read(&image);
        p.roxas = true;
        p.write(&mut image);
        assert_eq!(image.u8_at(field::ROXAS), 0b11);
        p.roxas = false;
        p.write(&mut image);
        assert_eq!(image.u8_at(field::ROXAS), 0b10);
    }

    #[test]
    fn reading_then_writing_gives_the_image_back_byte_for_byte() {
        // A sealed image with every byte set to something.
        let mut image = SaveData::blank();
        let junk: Vec<u8> = (0..image.bytes().len())
            .map(|i| (i * 31 + i / 7) as u8)
            .collect();
        image.set_slice(12, &junk[12..]);
        image.set_u8(EVENT_QUEUE_COUNT, 5);
        image.seal();
        let p = Persistent::read(&image);
        let mut copy = image.clone();
        p.write(&mut copy);
        assert_eq!(copy, image);
        // The bytes `write` sets come from the values alone: overwrite them
        // (the ones that differ between writes over 0x00 and 0xff images
        // are left alone by `write`) and they come back.
        let over = |b: u8| {
            let mut d = image.clone();
            d.set_slice(12, &vec![b; image.bytes().len() - 12]);
            p.write(&mut d);
            d.bytes().to_vec()
        };
        let (zeros, ones) = (over(0), over(0xff));
        let mut scrambled = image.clone();
        for i in 12..zeros.len() {
            if zeros[i] == ones[i] {
                scrambled.set_u8(i, image.u8_at(i) ^ 0x5a);
            }
        }
        p.write(&mut scrambled);
        assert_eq!(scrambled, image);
    }
}
