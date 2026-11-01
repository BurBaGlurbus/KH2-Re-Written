//! 03system `memt`: who the party is in each world. `u32` version, `u32` row
//! count, 0x34-byte rows, then `u32` member presets (FUN_1403e2e90).
//! FUN_1403e2eb0 lays the rows that hold over each other into 18 member
//! objects; a world's four member bytes (FUN_1403e2e20, save +0x3534) index
//! them, and ARD op 0x0f reorders those bytes by a preset (FUN_1403e31f0).
use std::io;

/// A member byte's "no one" (FUN_1403c32e0).
pub const NO_MEMBER: u8 = 0x12;
pub const MEMBERS: usize = 18;
const ROW: usize = 0x34;

/// A new world's member bytes (FUN_1403e2e10): Sora alone.
pub const NEW_RECORD: [u8; 4] = [0, NO_MEMBER, NO_MEMBER, NO_MEMBER];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Row {
    /// +0: the world, 0 for all.
    pub world: u16,
    /// +2: this story flag is set (0 for no test).
    pub flag: u16,
    /// +4: this story flag is clear (0 for no test).
    pub not_flag: u16,
    /// +6: the room's arif byte +0x34 plus 1, 0 for no test (FUN_1403a58c0).
    pub variant: u8,
    /// +0x10: the objects a holding row overwrites where not 0.
    pub members: [u16; MEMBERS],
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct MemberTable {
    pub rows: Vec<Row>,
    pub presets: Vec<u32>,
}

impl MemberTable {
    pub fn read(bytes: &[u8]) -> io::Result<Self> {
        let invalid = || io::Error::new(io::ErrorKind::InvalidData, "memt truncated");
        let count = bytes
            .get(4..8)
            .map(|w| i32::from_le_bytes(w.try_into().unwrap()))
            .ok_or_else(invalid)?;
        let count = usize::try_from(count).map_err(|_| invalid())?;
        let end = 8 + count * ROW;
        let body = bytes.get(8..end).ok_or_else(invalid)?;
        let word = |r: &[u8], at: usize| u16::from_le_bytes([r[at], r[at + 1]]);
        Ok(Self {
            rows: body
                .as_chunks::<ROW>()
                .0
                .iter()
                .map(|r| Row {
                    world: word(r, 0),
                    flag: word(r, 2),
                    not_flag: word(r, 4),
                    variant: r[6],
                    members: std::array::from_fn(|k| word(r, 0x10 + k * 2)),
                })
                .collect(),
            presets: bytes[end..]
                .as_chunks::<4>()
                .0
                .iter()
                .map(|w| u32::from_le_bytes(*w))
                .collect(),
        })
    }

    /// FUN_1403e2eb0's member objects for the place the game is in.
    pub fn objects(
        &self,
        world: u8,
        variant: u8,
        flag_set: impl Fn(u32) -> bool,
    ) -> [u16; MEMBERS] {
        let mut out = [0; MEMBERS];
        for r in &self.rows {
            if (r.world == 0 || r.world == u16::from(world))
                && (r.flag == 0 || flag_set(u32::from(r.flag)))
                && (r.not_flag == 0 || !flag_set(u32::from(r.not_flag)))
                && (r.variant == 0 || variant == r.variant - 1)
            {
                for (to, &from) in out.iter_mut().zip(&r.members) {
                    if from != 0 {
                        *to = from;
                    }
                }
            }
        }
        out
    }
}

/// FUN_1403e31f0: the preset's first member stays first. A preset byte with
/// bit 6 or 7 takes its place now; the members already in `record` follow in
/// their order if the preset holds them; the rest of the preset comes last.
pub fn apply_preset(record: [u8; 4], preset: u32) -> [u8; 4] {
    let mut wanted = preset.to_le_bytes();
    let mut out = [NO_MEMBER; 4];
    out[0] = wanted[0] & 0x1f;
    wanted[0] = NO_MEMBER;
    let mut n = 1;
    for b in wanted.iter_mut().skip(1) {
        if *b & 0xc0 != 0 {
            out[n] = *b & 0x9f;
            n += 1;
            *b = NO_MEMBER;
        }
    }
    for &old in &record[1..] {
        let member = old & 0x1f;
        if member == NO_MEMBER {
            continue;
        }
        if let Some(b) = wanted.iter_mut().find(|b| **b & 0x1f == member) {
            *b = NO_MEMBER;
            out[n] = member;
            n += 1;
        }
    }
    for &b in &wanted[1..] {
        if b & 0x1f != NO_MEMBER {
            out[n] = b & 0x9f;
            n += 1;
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn row(world: u16, flag: u16, not_flag: u16, variant: u8, slots: &[(usize, u16)]) -> Vec<u8> {
        let mut r = vec![0u8; ROW];
        r[0..2].copy_from_slice(&world.to_le_bytes());
        r[2..4].copy_from_slice(&flag.to_le_bytes());
        r[4..6].copy_from_slice(&not_flag.to_le_bytes());
        r[6] = variant;
        for &(k, v) in slots {
            r[0x10 + k * 2..0x12 + k * 2].copy_from_slice(&v.to_le_bytes());
        }
        r
    }

    fn table() -> MemberTable {
        let mut b = vec![0u8; 4];
        b.extend(4u32.to_le_bytes());
        b.extend(row(0, 0, 0, 0, &[(0, 0x54), (1, 0x5c), (2, 0x5d)]));
        b.extend(row(14, 0, 0, 0, &[(0, 0x2b5), (3, 0x5f)]));
        b.extend(row(14, 0, 0, 2, &[(0, 0x955)]));
        b.extend(row(14, 0x383a, 0x383b, 0, &[(3, 0x60)]));
        b.extend(0x1212_1200u32.to_le_bytes());
        b.extend(0x1202_0100u32.to_le_bytes());
        MemberTable::read(&b).unwrap()
    }

    #[test]
    fn rows_that_hold_overwrite_what_they_name() {
        let t = table();
        assert_eq!(t.presets, vec![0x1212_1200, 0x1202_0100]);
        let none = |_: u32| false;
        let o = t.objects(2, 0, none);
        assert_eq!((o[0], o[1], o[3]), (0x54, 0x5c, 0));
        let o = t.objects(14, 0, none);
        assert_eq!((o[0], o[1], o[3]), (0x2b5, 0x5c, 0x5f));
        // The room's variant picks the second costume.
        assert_eq!(t.objects(14, 1, none)[0], 0x955);
        // A flag that must be set, and one that must be clear.
        assert_eq!(t.objects(14, 0, |f| f == 0x383a)[3], 0x60);
        assert_eq!(t.objects(14, 0, |f| f == 0x383a || f == 0x383b)[3], 0x5f);
    }

    #[test]
    fn presets_keep_the_order_already_held() {
        // Preset 1: Donald and Goofy on a new world.
        assert_eq!(apply_preset(NEW_RECORD, 0x1202_0100), [0, 1, 2, NO_MEMBER]);
        // Preset 2 (a guest, Goofy, Donald): the two already held keep their
        // places and the guest comes after.
        assert_eq!(
            apply_preset([0, 1, 2, NO_MEMBER], 0x0102_0300),
            [0, 1, 2, 3]
        );
        // Preset 3: bit 6 forces the guest to the front.
        assert_eq!(
            apply_preset([0, 1, 2, NO_MEMBER], 0x0102_4300),
            [0, 3, 1, 2]
        );
    }
}
