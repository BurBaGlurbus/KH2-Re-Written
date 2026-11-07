//! The victim's own hit sound. `FUN_140410d60` (the damage routine, victim
//! side) plays `FUN_1403dadf0(victim, bank * 1000 + (n & 1) + class, position)`
//! once per hit unless the hit record's flags bit 2 is set, where `n` is the
//! victim's hit counter byte (`obj+0x26f*4`, incremented after each sound),
//! `class` follows the ATKP reaction through `FUN_1403ce980`, and `bank` is the
//! sound bank the victim's object was loaded with (the record behind
//! `obj+0xbb0`).
//!
//! The bank is the number in the header of the sound bank file the object was
//! loaded with: FUN_14039e900 passes the record's file to FUN_1401dcef0, which
//! keeps `FUN_1401e0d70`'s value for it (the `SEDBSSCF` header's u16 at +0x36),
//! and that value is what the record's bank short holds. The room registers
//! banks under the same header number (`visual::sound::Banks::insert`), so the
//! object archive's `se/obj/seNNN` asset gives it directly.
use std::collections::HashMap;

/// `FUN_1403ce980`: the reaction's class.
fn class_of(reaction: u8) -> u8 {
    match reaction {
        1 | 0xc => 1,
        3 | 6 | 8 | 10 | 0xd | 0x10 | 0x12 => 3,
        4 | 7 | 9 | 0xb | 0xf | 0x11 => 4,
        other => other,
    }
}

/// The id offset of a reaction: classes 1, 3 and 5 are 0, class 4 is 2, any
/// other reaction plays nothing.
pub fn offset(reaction: u8) -> Option<u32> {
    match class_of(reaction) {
        1 | 3 | 5 => Some(0),
        4 => Some(2),
        _ => None,
    }
}

/// The sound id for one hit on victim `key`, advancing its counter.
pub fn sound(bank: u16, reaction: u8, hits: &mut HashMap<u64, u8>, key: u64) -> Option<u32> {
    let offset = offset(reaction)?;
    let n = hits.entry(key).or_insert(0);
    let id = bank as u32 * 1000 + (*n & 1) as u32 + offset;
    *n = n.wrapping_add(1);
    Some(id)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reactions_map_to_the_id_offset() {
        for (reaction, want) in [
            (1, Some(0)),
            (0xc, Some(0)),
            (3, Some(0)),
            (0x12, Some(0)),
            (5, Some(0)),
            (4, Some(2)),
            (7, Some(2)),
            (0x11, Some(2)),
            (0, None),
            (2, None),
            (0x20, None),
        ] {
            assert_eq!(offset(reaction), want, "reaction {reaction}");
        }
    }

    #[test]
    fn the_id_alternates_within_the_victims_bank() {
        let shadow = 201;
        let mut hits = HashMap::new();
        let ids: Vec<_> = (0..4)
            .map(|_| sound(shadow, 3, &mut hits, 9).unwrap())
            .collect();
        assert_eq!(ids, [201000, 201001, 201000, 201001]);
        // A blow takes the heavy pair; a victim of its own counts alone.
        assert_eq!(sound(shadow, 7, &mut hits, 9), Some(201002));
        assert_eq!(sound(shadow, 7, &mut hits, 10), Some(201002));
        assert_eq!(sound(shadow, 7, &mut hits, 10), Some(201003));
        // A reaction without a hit sound leaves the counter alone.
        assert_eq!(sound(shadow, 0, &mut hits, 10), None);
        assert_eq!(hits[&10], 2);
    }
}
