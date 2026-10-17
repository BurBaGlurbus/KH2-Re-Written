//! Story progress flags and the per-area program table, driven by the
//! 00progress.bin flag scripts (FUN_14039afc0 / FUN_14039a980 /
//! FUN_140152e80).
use crate::world_state::{WorldState, WorldTables};
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
            scripts,
            skipped: Vec::new(),
            dsa: Vec::new(),
            dsa_state: Vec::new(),
            unblocked: false,
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

    pub fn programs(&self, world: u8, area: u8) -> [i16; 3] {
        self.programs[world as usize * AREAS_PER_WORLD + area as usize]
    }

    /// FUN_14039afc0: sets `flag`, runs its script, then the link table,
    /// the world events and the world points (`world_state`). The item and
    /// ability checks after the script (FUN_1403c4720 tests and the flags
    /// they set), FUN_1403ee4a0 and FUN_1403e66c0 are not carried out.
    pub fn set(&mut self, flag: u32) {
        let (w, bit) = Self::word(flag);
        if let Some(v) = self.flags.get_mut(w) {
            *v |= bit;
        }
        self.run_flag_script(flag);
        self.after_flag(flag);
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
