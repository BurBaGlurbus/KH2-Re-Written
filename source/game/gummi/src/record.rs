//! The mission records: 54 of 0x40 bytes at save +0x100e0 (live
//! 0x1409b9990), nine routes of three levels, then the same for EX. The
//! flight's live result block (0x140af7140) has the same layout.

pub const RECORDS: usize = 0x100e0;
pub const RECORD_BYTES: usize = 0x40;
pub const RECORD_COUNT: usize = 54;

/// A record's index (FUN_1402c76f0): `route` 0..=8, `level` 1..=3.
pub fn record_index(route: u8, level: u8, ex: bool) -> usize {
    usize::from(ex) * 27 + route as usize * 3 + level as usize - 1
}

/// One mission record (FUN_140209750 writes it on the result screen).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Mission {
    /// +0x00, the best score.
    pub score: u32,
    /// +0x04..+0x17: u16 counters kept at their best. +0x06 (index 1) is
    /// ships destroyed, the level 2 rank value; +0x12 (index 7) counts
    /// medals picked up [I]; the others aren't traced.
    pub counters: [u16; 10],
    /// +0x18, the route 0..=8 (the result block's; FUN_1402098e0).
    pub route: u8,
    /// +0x19, the level 1..=3 (as +0x18).
    pub level: u8,
    /// +0x1a, the best medal value.
    pub medals: u8,
    /// +0x1b, the rank, 0..=16.
    pub rank: u8,
    /// +0x1c: the rank went up in a run (FUN_1402099a0).
    pub rank_up: bool,
    /// +0x1d.
    pub cleared: bool,
    /// +0x1e: the mission can be flown (FUN_140209b50). Level 1's and
    /// level 2's are set on level 1's first clear.
    pub unlocked: bool,
    /// +0x20, the share of the treasures taken, 0..=100.
    pub treasure: f32,
}

impl Mission {
    /// Ships destroyed (+0x06).
    pub fn destroyed(&self) -> u16 {
        self.counters[1]
    }

    pub fn read(record: &[u8]) -> Option<Self> {
        let r = record.get(..RECORD_BYTES)?;
        let mut counters = [0u16; 10];
        for (k, c) in counters.iter_mut().enumerate() {
            *c = u16::from_le_bytes([r[4 + k * 2], r[5 + k * 2]]);
        }
        Some(Self {
            score: u32::from_le_bytes(r[0..4].try_into().ok()?),
            counters,
            route: r[0x18],
            level: r[0x19],
            medals: r[0x1a],
            rank: r[0x1b],
            rank_up: r[0x1c] != 0,
            cleared: r[0x1d] != 0,
            unlocked: r[0x1e] != 0,
            treasure: f32::from_le_bytes(r[0x20..0x24].try_into().ok()?),
        })
    }

    /// The fields back into a record; bytes it doesn't name stay as they are.
    pub fn write(&self, record: &mut [u8]) {
        let Some(r) = record.get_mut(..RECORD_BYTES) else {
            return;
        };
        r[0..4].copy_from_slice(&self.score.to_le_bytes());
        for (k, c) in self.counters.iter().enumerate() {
            r[4 + k * 2..6 + k * 2].copy_from_slice(&c.to_le_bytes());
        }
        r[0x18] = self.route;
        r[0x19] = self.level;
        r[0x1a] = self.medals;
        r[0x1b] = self.rank;
        r[0x1c] = u8::from(self.rank_up);
        r[0x1d] = u8::from(self.cleared);
        r[0x1e] = u8::from(self.unlocked);
        r[0x20..0x24].copy_from_slice(&self.treasure.to_le_bytes());
    }
}

/// The obtained-treasure bits (save +0x10f60, u32 words; DAT_1409ba810).
pub const TREASURE_BITS: usize = 0x10f60;

/// Whether treasure `bit` is obtained in a save image.
pub fn obtained(save: &[u8], bit: u32) -> bool {
    let at = TREASURE_BITS + (bit as usize >> 5) * 4;
    save.get(at..at + 4)
        .map(|w| u32::from_le_bytes([w[0], w[1], w[2], w[3]]))
        .is_some_and(|w| w >> (bit & 31) & 1 != 0)
}

/// All 54 records of a save image.
pub fn read_records(save: &[u8]) -> Option<[Mission; RECORD_COUNT]> {
    let mut out = [Mission::default(); RECORD_COUNT];
    for (k, m) in out.iter_mut().enumerate() {
        *m = Mission::read(save.get(RECORDS + k * RECORD_BYTES..)?)?;
    }
    Some(out)
}

/// The 54 records back into a save image.
pub fn write_records(save: &mut [u8], records: &[Mission; RECORD_COUNT]) {
    for (k, m) in records.iter().enumerate() {
        if let Some(r) = save.get_mut(RECORDS + k * RECORD_BYTES..) {
            m.write(r);
        }
    }
}

/// A route's normal and EX records out of a save image, `route` 1..=9 as
/// the world map numbers them. Missing bytes read as unplayed.
pub fn route_missions(save: &[u8], route: u8) -> [[Mission; 3]; 2] {
    let mut out = [[Mission::default(); 3]; 2];
    if !(1..=9).contains(&route) {
        return out;
    }
    for (kind, row) in out.iter_mut().enumerate() {
        for (level, m) in row.iter_mut().enumerate() {
            let at = RECORDS + record_index(route - 1, level as u8 + 1, kind == 1) * RECORD_BYTES;
            if let Some(read) = save.get(at..).and_then(Mission::read) {
                *m = read;
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn records_by_route_level_and_kind() {
        assert_eq!(record_index(0, 1, false), 0);
        assert_eq!(record_index(8, 3, false), 26);
        assert_eq!(record_index(0, 1, true), 27);
        assert_eq!(record_index(8, 3, true), 53);
        let mut save = vec![0u8; RECORDS + RECORD_COUNT * RECORD_BYTES];
        // Route 2, level 3, EX: score 1234, rank 5, cleared, unlocked, 50%.
        let at = RECORDS + record_index(1, 3, true) * RECORD_BYTES;
        save[at..at + 4].copy_from_slice(&1234u32.to_le_bytes());
        save[at + 6..at + 8].copy_from_slice(&7u16.to_le_bytes());
        save[at + 0x18] = 1;
        save[at + 0x19] = 3;
        save[at + 0x1a] = 3;
        save[at + 0x1b] = 5;
        save[at + 0x1d] = 1;
        save[at + 0x1e] = 1;
        save[at + 0x20..at + 0x24].copy_from_slice(&50f32.to_le_bytes());
        save[at + 0x30] = 0xaa;
        let m = route_missions(&save, 2);
        let mut counters = [0; 10];
        counters[1] = 7;
        assert_eq!(
            m[1][2],
            Mission {
                score: 1234,
                counters,
                route: 1,
                level: 3,
                medals: 3,
                rank: 5,
                rank_up: false,
                cleared: true,
                unlocked: true,
                treasure: 50.,
            }
        );
        assert_eq!(m[1][2].destroyed(), 7);
        assert_eq!(m[0][2], Mission::default());
        assert_eq!(route_missions(&save, 1), [[Mission::default(); 3]; 2]);
        assert_eq!(route_missions(&save[..10], 2), [[Mission::default(); 3]; 2]);
        // All 54 round-trip, and treasure bits read by word and bit.
        let all = read_records(&save).unwrap();
        let mut copy_all = save.clone();
        write_records(&mut copy_all, &all);
        assert_eq!(copy_all, save);
        let mut bits = vec![0u8; TREASURE_BITS + 8];
        bits[TREASURE_BITS + 4] = 0b100;
        assert!(obtained(&bits, 34));
        assert!(!obtained(&bits, 33));
        assert!(!obtained(&bits[..10], 34));
        // Written back, the record is the same bytes.
        let mut copy = save[at..at + RECORD_BYTES].to_vec();
        copy[..0x24].fill(0);
        m[1][2].write(&mut copy);
        assert_eq!(copy, save[at..at + RECORD_BYTES]);
    }
}
