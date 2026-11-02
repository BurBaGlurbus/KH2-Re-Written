//! 03system `went`: the weapon object table. A set is chosen by the player
//! object entry's neo-moveset index (objentry +0x4e) and holds one weapon
//! object id per item; FUN_1403ef8b0(set, item) returns
//! `went[went[set] + item.amount]` (item record +4), the size word of the set
//! counting as entry 0.
use std::io;

fn invalid(m: &str) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, m.to_owned())
}

fn word(bytes: &[u8], at: usize) -> io::Result<u32> {
    bytes
        .get(at..at + 4)
        .map(|w| u32::from_le_bytes(w.try_into().unwrap()))
        .ok_or_else(|| invalid("WENT entry out of range"))
}

/// FUN_1403ef750: the name of weapon slot `slot`'s motion set (`obj/%s.mset`):
/// `wmst` row set x 2 + slot. An empty row leaves the weapon without motions.
pub fn motion_set_name(wmst: &[u8], set: usize, slot: usize) -> Option<String> {
    let at = (set * 2 + slot) * 0x20;
    let raw = wmst.get(at..at + 0x20)?;
    let end = raw.iter().position(|&b| b == 0).unwrap_or(raw.len());
    (end > 0).then(|| String::from_utf8_lossy(&raw[..end]).into_owned())
}

/// One WENT set: pointers (4-byte units) at the start, each to a set of
/// `size` words, the first being `size` itself. Returns the entries after it.
pub fn set(went: &[u8], index: usize) -> io::Result<Vec<u32>> {
    let pointer = word(went, index * 4)? as usize;
    // The pointer table ends where the first set begins.
    let mut first = usize::MAX;
    let mut i = 0;
    while i < first && i < 4096 {
        match word(went, i * 4)? as usize {
            0 => {}
            p => first = first.min(p),
        }
        i += 1;
    }
    if pointer == 0 || index >= first {
        return Err(invalid("WENT set index outside the table"));
    }
    let size = word(went, pointer * 4)? as usize;
    if size == 0 || size > 4096 {
        return Err(invalid("WENT set size out of range"));
    }
    (1..size).map(|i| word(went, (pointer + i) * 4)).collect()
}

/// The weapon object id of an item whose record +4 is `amount`.
pub fn object_id(set: &[u32], amount: u16) -> Option<u32> {
    usize::from(amount)
        .checked_sub(1)
        .and_then(|i| set.get(i))
        .copied()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn table() -> Vec<u8> {
        // Pointers: set 0 at word 3, set 1 at word 7. Set 0 = [4, 0x10, 0x11, 0x12],
        // set 1 = [3, 0x20, 0x21].
        let words = [3u32, 7, 0, 4, 0x10, 0x11, 0x12, 3, 0x20, 0x21];
        words.iter().flat_map(|w| w.to_le_bytes()).collect()
    }

    #[test]
    fn a_motion_set_comes_from_the_row_of_the_set_and_slot() {
        let mut wmst = vec![0u8; 0x20 * 6];
        wmst[(2 * 2) * 0x20..(2 * 2) * 0x20 + 7].copy_from_slice(b"W_EX020");
        assert_eq!(
            super::motion_set_name(&wmst, 2, 0).as_deref(),
            Some("W_EX020")
        );
        assert_eq!(super::motion_set_name(&wmst, 2, 1), None);
        assert_eq!(super::motion_set_name(&wmst, 40, 0), None);
    }
    #[test]
    fn a_set_lists_its_entries_after_the_size_word() {
        let t = table();
        assert_eq!(set(&t, 0).unwrap(), vec![0x10, 0x11, 0x12]);
        assert_eq!(set(&t, 1).unwrap(), vec![0x20, 0x21]);
        assert!(set(&t, 2).is_err());
        assert!(set(&t, 3).is_err());
    }

    #[test]
    fn two_pointers_may_share_one_set() {
        // Two pointers (the second repeats the first), then one set of
        // size 3 holding object ids 103 and 104.
        let words = [2u32, 2, 3, 103, 104];
        let went: Vec<u8> = words.iter().flat_map(|w| w.to_le_bytes()).collect();
        assert_eq!(set(&went, 0).unwrap(), [103, 104]);
        assert_eq!(set(&went, 1).unwrap(), [103, 104]);
        assert!(set(&went, 2).is_err());
    }

    #[test]
    fn an_item_amount_counts_from_one() {
        let s = vec![0x10, 0x11, 0x12];
        assert_eq!(object_id(&s, 1), Some(0x10));
        assert_eq!(object_id(&s, 3), Some(0x12));
        assert_eq!(object_id(&s, 0), None);
        assert_eq!(object_id(&s, 4), None);
    }
}
