//! Story progress flags and the per-area program table, driven by the
//! 00progress.bin flag scripts (FUN_14039afc0 / FUN_14039a980 /
//! FUN_140152e80).
use crate::world_state::{WorldState, WorldTables};
use std::collections::HashMap;
use std::sync::Arc;

/// World codes in the exe's order (`PTR_140717210`): world index → code.
pub const WORLDS: [&str; 19] = [
    "zz", "es", "tt", "di", "hb", "bb", "he", "al", "mu", "po", "lk", "lm", "dc", "wi", "nm", "wm",
    "ca", "tr", "eh",
];

pub fn world_index(code: &str) -> Option<u8> {
    WORLDS.iter().position(|w| *w == code).map(|i| i as u8)
}

/// Areas per world in the save's program table (`world * 0x40 + area`).
pub const AREAS_PER_WORLD: usize = 0x40;

/// A 00battle.bin `btlv` record (0x20 bytes): its bit, the story flag that sets
/// it (FUN_1403ee4a0) and what it adds to each world's enemy level.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct BattleRow {
    pub bit: u32,
    pub flag: u32,
    pub levels: [u8; 19],
}

/// `btlv`: u32 version, u32 count, the records from +8.
pub fn read_battle_rows(bytes: &[u8]) -> Option<Vec<BattleRow>> {
    let count = u32::from_le_bytes(bytes.get(4..8)?.try_into().ok()?) as usize;
    (0..count)
        .map(|k| {
            let r = bytes.get(8 + k * 0x20..8 + (k + 1) * 0x20)?;
            Some(BattleRow {
                bit: u32::from_le_bytes(r[0..4].try_into().ok()?),
                flag: u32::from_le_bytes(r[4..8].try_into().ok()?),
                levels: r[8..27].try_into().ok()?,
            })
        })
        .collect()
}

/// The save table entry FUN_140152e80 guards (`DAT_1409a9f2c`, index
/// (0x1409a9f2c − 0x1409a98c0) / 6 = world 4 (hb) area 18): a map program
/// of 1 is not replaced by 3 there.
const GUARDED_ENTRY: usize = (0x1409a9f2c - 0x1409a98c0) / 6;

/// Flag words: `DAT_1409ab540[(f >> 10) * 8 + ((f & 0x3ff) >> 5)]`. The
/// slot part reaches 31 words while the world stride is 8, so high slots of
/// one world share words with the next, as in the exe.
const FLAG_WORDS: usize = 0x40 * 8 + 32;

/// Operations the interpreter met but does not carry out: `(op, words)`.
pub type Skipped = Vec<(u8, Vec<u16>)>;

/// FUN_1403a53a0 / FUN_1403a5420's table (0x1405c4320): in world 0x10, once
/// `flag` is set the listed areas are blocked with `message`.
const WORLD_10_BLOCKS: [(u32, [u8; 3], u16); 4] = [
    (0x4040, [0x08, 0x0b, 0x10], 0xc2c0),
    (0x4042, [0x00, 0x0b, 0x10], 0xc2c1),
    (0x4043, [0x00, 0x08, 0x10], 0xc2c2),
    (0x4049, [0x00, 0x08, 0x0b], 0xc2c3),
];

/// What the tail of FUN_14039afc0 reads of the save. A room without a save
/// holds nothing, so no item check fires.
#[derive(Clone, Debug, Default)]
pub struct Holdings {
    /// The counts of the items the checks read (0x17, 0x20, 0x57).
    pub items: HashMap<u16, u8>,
    /// save+0x352a; [U] what it counts.
    pub byte_352a: u8,
    /// FormRecord.level of forms 1..=5.
    pub form_levels: [u8; 5],
    /// save+0x3526.
    pub summon_level: u8,
}

impl Holdings {
    pub fn count(&self, item: u16) -> u8 {
        self.items.get(&item).copied().unwrap_or(0)
    }
}

/// [I] Item 0x20, the Torn Page: these flags take one.
const TORN_PAGE: u16 = 0x20;
const TORN_PAGE_TAKEN: [u32; 5] = [0x2409, 0x2411, 0x2419, 0x2421, 0x2428];
const TORN_PAGE_PAIRS: [(u32, u32); 5] = [
    (0x2408, 0x103b),
    (0x2410, 0x2434),
    (0x2418, 0x2435),
    (0x2420, 0x2436),
    (0x2427, 0x2437),
];
/// {flag, world, area} of the table at 0x1405c3370.
const AREA_SEEN_FLAGS: [(u32, u8, u8); 14] = [
    (0x1409, 5, 7),
    (0x1425, 5, 6),
    (0x2827, 10, 0xd),
    (0x899, 2, 3),
    (0x899, 2, 4),
    (0x899, 2, 6),
    (0x899, 2, 7),
    (0x899, 2, 0xd),
    (0x3804, 0xe, 0),
    (0x380f, 0xe, 9),
    (0x102b, 4, 2),
    (0x1c25, 7, 3),
    (0x1415, 5, 4),
    (0x8df, 2, 8),
];
/// FUN_1403e66c0's third check.
const LEVEL_FLAGS: [u32; 11] = [
    0x1847, 0x184d, 0x184f, 0x200f, 0x1412, 0x4050, 0x3837, 0x1c20, 0x1826, 0x4433, 0x2830,
];
/// The world of FUN_1403a5330's stages.
const STAGE_WORLD: u8 = 0x10;

#[derive(Clone)]
pub struct Progress {
    flags: Vec<u32>,
    /// `{map, btl, evt}` per `world * 0x40 + area` (`DAT_1409a98c0`).
    pub programs: Vec<[i16; 3]>,
    /// Per-world arif set index (op 6, FUN_1403ab470: value − 1).
    pub arif_set: [i32; 19],
    /// World records, symbol slots, world events and world points (op 13
    /// writes record bytes 1 and 2).
    pub world: WorldState,
    /// The world map tables; without them the link table and the world
    /// events are not run.
    pub(crate) world_tables: Option<Arc<WorldTables>>,
    battle_rows: Vec<BattleRow>,
    /// save+0x3724: which `btlv` rows count.
    pub battle_bits: Vec<u32>,
    /// Flag scripts by world index (00progress.bin BAR entries named by
    /// world code).
    scripts: Vec<Option<Vec<u8>>>,
    pub skipped: Skipped,
    /// 00progress.bin 'dsa': `{world, area}` per blockable destination
    /// (`DAT_142ae5ab8`, FUN_1403f0560).
    dsa: Vec<[u8; 2]>,
    /// `{message/flag, message2}` per 'dsa' entry (`DAT_1409ab7a8`); a
    /// non-zero first word blocks the destination.
    dsa_state: Vec<[u16; 2]>,
    /// Set by `unblock_all`: also ignore the world-0x10 table.
    unblocked: bool,
    /// What the flag tail reads of the stock and the levels.
    pub holdings: Holdings,
    /// Items a flag took, until the room removes them.
    pub taken: Vec<u16>,
    /// save+0x22f8: the areas seen, one word per world.
    pub area_seen: [u64; 19],
    /// DAT_142ae7cc8: the Port Royal ship stage (FUN_1403a5540, FUN_1403a5330).
    pub stage: u32,
}

impl Progress {
    /// `scripts[w]` is world `w`'s 00progress entry, if present.
    pub fn new(scripts: Vec<Option<Vec<u8>>>) -> Self {
        Self {
            flags: vec![0; FLAG_WORDS],
            programs: vec![[0; 3]; WORLDS.len() * AREAS_PER_WORLD],
            arif_set: [0; 19],
            world: WorldState::default(),
            world_tables: None,
            battle_rows: Vec::new(),
            battle_bits: Vec::new(),
            scripts,
            skipped: Vec::new(),
            dsa: Vec::new(),
            dsa_state: Vec::new(),
            unblocked: false,
            holdings: Holdings::default(),
            taken: Vec::new(),
            area_seen: [0; 19],
            stage: 0,
        }
    }

    /// Gives the progress the world map tables, so that setting a flag also
    /// runs the link table and marks world events (FUN_14039afc0's tail).
    pub fn set_world_tables(&mut self, tables: Arc<WorldTables>) {
        self.world_tables = Some(tables);
    }

    pub fn world_tables(&self) -> Option<&WorldTables> {
        self.world_tables.as_deref()
    }

    /// 00battle.bin 'btlv' (FUN_1403f4c30 → FUN_1403ee490).
    pub fn set_battle_rows(&mut self, rows: Vec<BattleRow>) {
        let words = rows
            .iter()
            .map(|r| r.bit as usize / 32 + 1)
            .max()
            .unwrap_or(0);
        self.battle_bits.resize(words, 0);
        self.battle_rows = rows;
    }

    /// FUN_1403ee4a0: a row's bit stays once its story flag has been set, also
    /// when a script clears the flag again.
    pub(crate) fn note_battle_flag(&mut self, flag: u32) {
        for r in &self.battle_rows {
            if r.flag == flag
                && let Some(w) = self.battle_bits.get_mut(r.bit as usize / 32)
            {
                *w |= 1 << (r.bit % 32);
            }
        }
    }

    /// FUN_1403ee560: the worlds' levels, byte sums over the rows whose bit is
    /// set.
    pub fn battle_sums(&self) -> [u8; 19] {
        let mut sums = [0u8; 19];
        for r in &self.battle_rows {
            let on = self
                .battle_bits
                .get(r.bit as usize / 32)
                .is_some_and(|w| w >> (r.bit % 32) & 1 != 0);
            if on {
                for (s, l) in sums.iter_mut().zip(r.levels) {
                    *s = s.wrapping_add(l);
                }
            }
        }
        sums
    }

    /// FUN_1403ee1d0 at the end of the area loader: the area's op 0x1e level,
    /// else the world's sum; 0 becomes 1.
    pub fn battle_level(&self, op: u32, world: u8) -> u32 {
        let level = if op != 0 {
            op
        } else {
            self.battle_sums()
                .get(world as usize)
                .copied()
                .map_or(0, u32::from)
        };
        level.max(1)
    }

    /// FUN_140418c20 / FUN_140418e30: a world point object marks the point
    /// of the area it is created in (FUN_1403f2270).
    pub fn enter_world_point(&mut self, world: u8, area: u8) {
        if let Some(tables) = self.world_tables.clone() {
            self.world.enter_area_with_world_point(&tables, world, area);
        }
    }

    /// The 'dsa' entry: `u16 count`, then `count` `{world, area}` bytes.
    pub fn set_dsa(&mut self, bytes: &[u8]) {
        let count = bytes
            .get(..2)
            .map_or(0, |b| u16::from_le_bytes([b[0], b[1]]) as usize);
        self.dsa = bytes
            .get(2..)
            .unwrap_or(&[])
            .as_chunks::<2>()
            .0
            .iter()
            .take(count)
            .copied()
            .collect();
        self.dsa_state = vec![[0; 2]; self.dsa.len()];
    }

    /// Opens every 'dsa' destination (all areas reachable regardless of
    /// the story point).
    pub fn unblock_all(&mut self) {
        self.dsa_state.fill([0; 2]);
        self.unblocked = true;
    }

    /// FUN_1403a53a0: world 0x10's flag-blocked areas and their message.
    fn world_10_block(&self, world: u8, area: u8) -> Option<u16> {
        if world != 0x10 || self.unblocked {
            return None;
        }
        WORLD_10_BLOCKS
            .iter()
            .find(|(flag, areas, _)| self.test(*flag) && areas.contains(&area))
            .map(|&(_, _, message)| message)
            .filter(|&m| m != 0)
    }

    fn dsa_index(&self, world: u8, area: u8) -> Option<usize> {
        self.dsa.iter().position(|&[w, a]| w == world && a == area)
    }

    /// FUN_1403f0560: the destination has a 'dsa' entry that is blocked,
    /// by its state word or (world 0x10) the flag table.
    pub fn destination_blocked(&self, world: u8, area: u8) -> bool {
        self.dsa_index(world, area).is_some_and(|i| {
            self.dsa_state[i][0] != 0 || self.world_10_block(world, area).is_some()
        })
    }

    /// FUN_1403f0470(world, area, 1) as FUN_1403f20c0 asks it: the message
    /// of a blocked world point (the entry's second word, else world
    /// 0x10's table; 0 when neither has one).
    pub fn blocked_message(&self, world: u8, area: u8) -> u16 {
        let Some(i) = self.dsa_index(world, area) else {
            return 0;
        };
        match self.dsa_state[i][1] {
            0 => self.world_10_block(world, area).unwrap_or(0),
            m => m,
        }
    }

    /// Destinations whose 'dsa' state word is set.
    pub fn blocked_destinations(&self) -> Vec<(u8, u8)> {
        self.dsa
            .iter()
            .zip(&self.dsa_state)
            .filter(|(_, s)| s[0] != 0)
            .map(|(&[w, a], _)| (w, a))
            .collect()
    }

    /// FUN_1403f0560 / FUN_1403f0470: the message word of the first 'dsa'
    /// entry for the destination while it is blocked; with a zero state
    /// word, world 0x10's flag table (FUN_1403a53a0) can still block it.
    pub fn blocked(&self, world: u8, area: u8) -> Option<u16> {
        let i = self.dsa_index(world, area)?;
        match self.dsa_state[i][0] {
            0 => self.world_10_block(world, area),
            m => Some(m),
        }
    }

    fn word(flag: u32) -> (usize, u32) {
        let world = (flag as i32 >> 10) as usize;
        (
            world * 8 + ((flag & 0x3ff) >> 5) as usize,
            1 << (flag & 0x1f),
        )
    }

    /// FUN_14039a980: 0xffff is never set.
    pub fn test(&self, flag: u32) -> bool {
        if flag == 0xffff {
            return false;
        }
        let (w, bit) = Self::word(flag);
        self.flags.get(w).is_some_and(|v| v & bit != 0)
    }

    /// Every world's flags as `first..=last`, from the size of its script
    /// table (`FUN_14039afc0` indexes it by `flag & 0x3ff`). World 0 holds the
    /// new-game setup, not story.
    pub fn story_ranges(&self) -> Vec<(u32, u32)> {
        self.scripts
            .iter()
            .enumerate()
            .skip(1)
            .filter_map(|(world, script)| {
                let script = script.as_ref()?;
                let table = u16::from_le_bytes([*script.first()?, *script.get(1)?]);
                let slots = u32::from(table / 2);
                let first = world as u32 * 0x400;
                (slots != 0).then_some((first, first + slots - 1))
            })
            .collect()
    }

    /// The flag words a save holds (`DAT_1409ab540`).
    pub fn flag_words(&self) -> &[u32] {
        &self.flags
    }

    /// FUN_1402ed0e0's share: a loaded save's flag words replace the running
    /// ones; words past the saved ones stay clear.
    pub fn restore_flags(&mut self, words: &[u32]) {
        self.flags.fill(0);
        for (to, from) in self.flags.iter_mut().zip(words) {
            *to = *from;
        }
    }

    /// The 'dsa' state words (`DAT_1409ab7a8`).
    pub fn dsa_state(&self) -> &[[u16; 2]] {
        &self.dsa_state
    }

    pub fn restore_dsa_state(&mut self, words: &[[u16; 2]]) {
        for (to, from) in self.dsa_state.iter_mut().zip(words) {
            *to = *from;
        }
    }

    pub fn programs(&self, world: u8, area: u8) -> [i16; 3] {
        self.programs[world as usize * AREAS_PER_WORLD + area as usize]
    }

    /// FUN_14039afc0: sets `flag`, runs its script, then its tail in the
    /// executable's order: the Torn Pages, Atlantica, the area table and the
    /// 0x102b rule here, then the link table, the world events, the battle
    /// bits, the level checks and the world points (`world_state`).
    pub fn set(&mut self, flag: u32) {
        let (w, bit) = Self::word(flag);
        if let Some(v) = self.flags.get_mut(w) {
            *v |= bit;
        }
        self.run_flag_script(flag);
        if TORN_PAGE_TAKEN.contains(&flag) {
            self.take_item(TORN_PAGE);
        } else if self.holdings.count(TORN_PAGE) != 0 {
            self.check_torn_pages();
        }
        self.check_atlantica();
        for &(f, world, area) in &AREA_SEEN_FLAGS {
            if f == flag {
                self.see_area(world, area);
            }
        }
        if self.test(0x102b) && !self.test(0x1807) && !self.test(0x1809) && self.test(0x1822) {
            let next = if self.test(0x1865) { 0x1809 } else { 0x1807 };
            self.set(next);
        }
        self.after_flag(flag);
    }

    /// FUN_1403c49c0(item, 100): one leaves the stock. The room drains `taken`.
    fn take_item(&mut self, item: u16) {
        if let Some(n) = self.holdings.items.get_mut(&item) {
            *n = n.saturating_sub(1);
        }
        self.taken.push(item);
    }

    /// FUN_14039aa80: a page held opens its chapter once the chapter before
    /// it is done. Also when item 0x20 is gained.
    pub fn check_torn_pages(&mut self) {
        for (a, b) in TORN_PAGE_PAIRS {
            if !self.test(a) && self.test(b) {
                self.set(a);
            }
        }
    }

    /// FUN_14039a9c0: one step of Atlantica's chain, by the items held. Also
    /// when an item of kind 0x12 or a bonus level is gained.
    pub fn check_atlantica(&mut self) {
        let next = if !self.test(0x2c26) {
            (self.test(0x2c31) && self.holdings.count(0x57) >= 1).then_some(0x2c26)
        } else if !self.test(0x2c27) {
            (self.test(0x2c34) && self.holdings.byte_352a >= 5).then_some(0x2c27)
        } else if !self.test(0x2c28) {
            (self.test(0x2c37) && self.holdings.count(0x57) >= 2).then_some(0x2c28)
        } else if !self.test(0x2c2d) {
            (self.test(0x2c3a) && self.holdings.count(0x17) >= 3).then_some(0x2c2d)
        } else {
            None
        };
        if let Some(flag) = next {
            self.set(flag);
        }
    }

    /// FUN_1403baf20, and FUN_1403baf00 clears them all at flag 0x8d0: one
    /// area of a world has been seen.
    pub fn see_area(&mut self, world: u8, area: u8) {
        if let Some(w) = self.area_seen.get_mut(world as usize) {
            *w |= 1u64 << (area & 63);
        }
    }

    /// FUN_1403a5330, on entering an area: the Port Royal ship stages.
    pub fn enter_stage_area(&mut self, world: u8, area: u8) {
        if world != STAGE_WORLD {
            return;
        }
        let at = match area {
            1 => (0x15, 0x4040),
            8 => (0x16, 0x4042),
            0xe => (0x14, 0x4043),
            0x10 => (0x13, 0x4049),
            _ => return,
        };
        self.stage = at.0;
        self.set(at.1);
    }

    /// FUN_1403e66c0, after a flag and each level-up: the menu flags that
    /// count the Drive forms' and the summons' levels.
    pub fn check_levels(&mut self) {
        let h = &self.holdings;
        let forms_at = |n: usize, least: u8| h.form_levels[..n].iter().all(|&l| l >= least);
        let (four, summons) = (forms_at(4, 5), h.summon_level >= 5);
        let all = forms_at(5, 7) && h.summon_level >= 7;
        let open = self.world.menu_flag(0x65);
        if !self.world.menu_flag(0x6a) && open && four {
            self.set_menu_flag(0x6a);
        }
        if !self.world.menu_flag(0x67) && open && summons {
            self.set_menu_flag(0x67);
        }
        if !self.world.menu_flag(0x68) && all && LEVEL_FLAGS.iter().all(|&f| self.test(f)) {
            self.set_menu_flag(0x68);
        }
    }

    fn run_flag_script(&mut self, flag: u32) {
        let world = (flag as i32 >> 10) as usize;
        let Some(Some(script)) = self.scripts.get(world).cloned() else {
            return;
        };
        let slot = (flag & 0x3ff) as usize;
        let read = |at: usize| -> Option<u16> {
            script
                .get(at..at + 2)
                .map(|b| u16::from_le_bytes([b[0], b[1]]))
        };
        // The exe indexes the offset table without a bound; the table ends
        // where the first script begins.
        let Some(first) = read(0) else { return };
        if slot * 2 >= first as usize {
            return;
        }
        let Some(mut at) = read(slot * 2).map(usize::from) else {
            return;
        };
        // A slot of offset 0 has no script: the exe would run the table
        // itself as one, and its story never sets such a flag.
        if at == 0 {
            return;
        }
        while let Some(&op) = script.get(at) {
            if op == 0 {
                break;
            }
            let Some(&n) = script.get(at + 1) else { break };
            let words: Vec<u16> = (0..n as usize)
                .map_while(|i| read(at + 2 + i * 2))
                .collect();
            match op {
                // FUN_1403f0430 / FUN_1403f0450 / FUN_1403f0600 /
                // FUN_1403f0650 on 'dsa' entry `words[0]`.
                2..=5 => {
                    if let Some(e) = words
                        .first()
                        .and_then(|&i| self.dsa_state.get_mut(i as usize))
                    {
                        e[0] = match op {
                            2 => words.get(1).copied().unwrap_or(0),
                            3 | 5 => 0,
                            _ => 0xffff,
                        };
                        // FUN_14039b0e4: op 2 also stores the second message.
                        if op == 2 {
                            e[1] = words.get(2).copied().unwrap_or(0);
                        }
                    }
                    // FUN_1403f0600 / FUN_1403f0650: the destination's world
                    // point is forgotten too.
                    if matches!(op, 4 | 5)
                        && let Some(&[w, a]) = words.first().and_then(|&i| self.dsa.get(i as usize))
                        && let Some(tables) = self.world_tables.clone()
                        && let Some(p) = tables.world_point_at(w, a)
                    {
                        self.world.forget_point(p as u32);
                    }
                }
                1 => {
                    for r in words.as_chunks::<4>().0.iter().take((n >> 2) as usize) {
                        self.write_programs(world * AREAS_PER_WORLD + r[0] as usize, &r[1..]);
                    }
                }
                6 => {
                    if let (Some(set), Some(&v)) = (self.arif_set.get_mut(world), words.first()) {
                        *set = v as i32 - 1;
                    }
                }
                7 => {
                    for &f in &words {
                        let (w, bit) = Self::word(f as u32);
                        if let Some(v) = self.flags.get_mut(w) {
                            *v &= !bit;
                        }
                    }
                }
                // FUN_1403e3930 / FUN_1403e3910 per word.
                8 => {
                    for &f in &words {
                        self.set_menu_flag(f as u32);
                    }
                }
                9 => {
                    for &f in &words {
                        self.clear_menu_flag(f as u32);
                    }
                }
                12 => {
                    for &f in &words {
                        if f == 0xffff || !self.test(f as u32) {
                            self.set(f as u32);
                        }
                    }
                }
                13 => {
                    if let (Some(r), Some(&b3), Some(&w0)) = (
                        self.world.records.get_mut(world),
                        script.get(at + 3),
                        words.first(),
                    ) {
                        r.landing_set = b3;
                        r.value = w0 as u8;
                    }
                }
                _ => self.skipped.push((op, words)),
            }
            at += n as usize * 2 + 2;
        }
    }

    /// FUN_140152e80.
    fn write_programs(&mut self, index: usize, values: &[u16]) {
        let Some(entry) = self.programs.get_mut(index) else {
            return;
        };
        let [map, btl, evt] = [values[0], values[1], values[2]].map(|v| v as i16);
        if map != -1 && !(entry[0] == 1 && map == 3 && index == GUARDED_ENTRY) {
            entry[0] = map;
        }
        if btl != -1 {
            entry[1] = btl;
        }
        if evt != -1 {
            entry[2] = evt;
        }
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn the_enemy_level_sums_the_rows_a_flag_has_set_and_stays() {
        let mut bytes = vec![0u8; 8];
        bytes[4] = 2;
        for (bit, flag, level) in [(1u32, 0x10u32, 3u8), (33, 0x11, 4)] {
            let mut r = [0u8; 0x20];
            r[0..4].copy_from_slice(&bit.to_le_bytes());
            r[4..8].copy_from_slice(&flag.to_le_bytes());
            r[8 + 2] = level;
            bytes.extend(r);
        }
        let rows = read_battle_rows(&bytes).unwrap();
        let mut p = Progress::new(Vec::new());
        p.set_battle_rows(rows);
        assert_eq!(p.battle_bits.len(), 2);
        // Nothing set yet: floor 1. An op 0x1e level wins.
        assert_eq!(p.battle_level(0, 2), 1);
        assert_eq!(p.battle_level(80, 2), 80);
        p.note_battle_flag(0x10);
        p.note_battle_flag(0x11);
        p.note_battle_flag(0x12);
        assert_eq!(p.battle_bits, [2, 2]);
        assert_eq!(p.battle_level(0, 2), 7);
        assert_eq!(p.battle_level(0, 3), 1);
    }

    use super::*;

    /// A world script table with scripts for slots 0 and 1.
    fn scripts() -> Vec<Option<Vec<u8>>> {
        let mut d: Vec<u8> = Vec::new();
        d.extend(4u16.to_le_bytes());
        d.extend(0u16.to_le_bytes()); // patched below
        // slot 0: op1 tt08 = {4, -1, 18}; op6 arif 2; op12 [flag 0x801]
        d.extend([1, 4]);
        for v in [8u16, 4, 0xffff, 18] {
            d.extend(v.to_le_bytes());
        }
        d.extend([6, 1]);
        d.extend(2u16.to_le_bytes());
        d.extend([12, 1]);
        d.extend(0x801u16.to_le_bytes());
        d.push(0);
        d.push(0);
        let slot1 = d.len() as u16;
        d[2..4].copy_from_slice(&slot1.to_le_bytes());
        // slot 1: op1 tt08 btl 22; op7 clear 0x800
        d.extend([1, 4]);
        for v in [8u16, 0xffff, 22, 0xffff] {
            d.extend(v.to_le_bytes());
        }
        d.extend([7, 1]);
        d.extend(0x800u16.to_le_bytes());
        d.push(0);
        let mut v = vec![None; 19];
        v[2] = Some(d);
        v
    }

    #[test]
    fn flag_script_writes_programs_and_recurses() {
        let mut p = Progress::new(scripts());
        p.set(0x800);
        assert_eq!(p.programs(2, 8), [4, 22, 18]);
        assert_eq!(p.arif_set[2], 1);
        assert!(p.test(0x801));
        // Slot 1's op 7 cleared the flag that started the chain.
        assert!(!p.test(0x800));
        assert!(!p.test(0xffff));
    }

    #[test]
    fn story_ranges_follow_the_script_tables() {
        let mut v = scripts();
        v[0] = Some(vec![2, 0, 0, 0]);
        let p = Progress::new(v);
        assert_eq!(p.story_ranges(), vec![(0x800, 0x801)]);
    }

    #[test]
    fn restored_flags_replace_the_running_ones() {
        let mut p = Progress::new(vec![None; 19]);
        p.set(0x800);
        p.restore_flags(&[0, 0, 1 << 3]);
        assert!(!p.test(0x800));
        assert!(p.test(2 * 0x20 + 3));
        assert_eq!(p.flag_words()[2], 1 << 3);
    }

    #[test]
    fn guarded_hb18_entry() {
        let mut p = Progress::new(vec![None; 19]);
        p.programs[GUARDED_ENTRY] = [1, 0, 0];
        p.write_programs(GUARDED_ENTRY, &[3, 0xffff, 0xffff]);
        assert_eq!(p.programs[GUARDED_ENTRY][0], 1);
        p.write_programs(GUARDED_ENTRY - 1, &[3, 0xffff, 0xffff]);
        assert_eq!(p.programs[GUARDED_ENTRY - 1][0], 3);
        assert_eq!(GUARDED_ENTRY, 4 * 0x40 + 18);
    }

    #[test]
    fn dsa_ops_block_and_unblock_destinations() {
        // 'dsa': two entries {tt, 3} and {tt, 4}; slot 0 blocks entry 0
        // (op 4) and sets entry 1's message (op 2); slot 1 unblocks 0.
        let mut d: Vec<u8> = vec![4, 0, 0, 0];
        let slot0 = d.len() as u16;
        d.extend([4, 1, 0, 0, 2, 2, 1, 0, 0x34, 0x12, 0, 0]);
        let slot1 = d.len() as u16;
        d.extend([5, 1, 0, 0, 0, 0]);
        d[0..2].copy_from_slice(&slot0.to_le_bytes());
        d[2..4].copy_from_slice(&slot1.to_le_bytes());
        let mut v = vec![None; 19];
        v[2] = Some(d);
        let mut p = Progress::new(v);
        p.set_dsa(&[2, 0, 2, 3, 2, 4]);
        assert_eq!(p.blocked(2, 3), None);
        p.set(0x800);
        assert_eq!(p.blocked(2, 3), Some(0xffff));
        assert_eq!(p.blocked(2, 4), Some(0x1234));
        assert_eq!(p.blocked_destinations(), vec![(2, 3), (2, 4)]);
        p.set(0x801);
        assert_eq!(p.blocked(2, 3), None);
        assert_eq!(p.blocked(2, 9), None);
    }
}

#[cfg(test)]
mod tail_tests {
    use super::*;

    fn progress() -> Progress {
        Progress::new(Vec::new())
    }

    #[test]
    fn a_page_flag_takes_one_page_and_a_held_page_opens_its_chapter() {
        let mut p = progress();
        p.holdings.items.insert(TORN_PAGE, 2);
        p.set(0x103b);
        assert!(p.test(0x2408));
        p.set(0x2409);
        assert_eq!(p.holdings.count(TORN_PAGE), 1);
        assert_eq!(p.taken, [TORN_PAGE]);
    }

    #[test]
    fn with_nothing_held_no_item_check_fires() {
        let mut p = progress();
        p.set(0x103b);
        assert!(!p.test(0x2408));
        p.set(0x2c31);
        assert!(!p.test(0x2c26));
        assert!(p.taken.is_empty());
    }

    #[test]
    fn atlantica_takes_one_step_per_flag() {
        let mut p = progress();
        p.holdings.items.insert(0x57, 2);
        p.set(0x2c31);
        assert!(p.test(0x2c26));
        p.holdings.byte_352a = 5;
        p.set(0x2c34);
        assert!(p.test(0x2c27));
        p.set(0x2c37);
        assert!(p.test(0x2c28));
        assert!(!p.test(0x2c2d));
    }

    #[test]
    fn a_flag_marks_its_areas_and_0x8d0_clears_them() {
        let mut p = progress();
        p.set(0x899);
        assert_eq!(p.area_seen[2], 1 << 3 | 1 << 4 | 1 << 6 | 1 << 7 | 1 << 0xd);
        p.set(0x8d0);
        assert_eq!(p.area_seen, [0; 19]);
    }

    #[test]
    fn the_0x102b_rule_picks_by_0x1865() {
        let mut p = progress();
        p.set(0x1822);
        p.set(0x102b);
        assert!(p.test(0x1807) && !p.test(0x1809));
        let mut p = progress();
        p.set(0x1865);
        p.set(0x1822);
        p.set(0x102b);
        assert!(p.test(0x1809) && !p.test(0x1807));
    }

    #[test]
    fn the_stage_follows_its_flags_and_areas() {
        let mut p = progress();
        p.set(0x4019);
        assert_eq!(p.stage, 0x16);
        p.enter_stage_area(STAGE_WORLD, 1);
        assert_eq!(p.stage, 0x15);
        assert!(p.test(0x4040));
        p.enter_stage_area(2, 8);
        assert_eq!(p.stage, 0x15);
    }

    #[test]
    fn the_level_checks_need_the_opening_flag_and_the_levels() {
        let mut p = progress();
        p.holdings.form_levels = [5, 5, 5, 5, 1];
        p.holdings.summon_level = 5;
        p.check_levels();
        assert!(!p.world.menu_flag(0x6a));
        p.set_menu_flag(0x65);
        p.check_levels();
        assert!(p.world.menu_flag(0x6a) && p.world.menu_flag(0x67));
        assert!(!p.world.menu_flag(0x68));
        p.holdings.form_levels = [7; 5];
        p.holdings.summon_level = 7;
        for f in LEVEL_FLAGS {
            p.set(f);
        }
        p.check_levels();
        assert!(p.world.menu_flag(0x68));
    }
}
