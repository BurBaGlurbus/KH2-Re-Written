//! The Gummi flight's rank and treasure tables (`gumibattle/rank.bin`,
//! `gumibattle/treasure.bin`). Each is u32 version, u32 count and the
//! records from +8 (FUN_14014c780 reads them with the exe's own format
//! strings, "4c 16i 60c" and "2i 4c 1i 16c").

/// A rank.bin record (0x80 bytes): the thresholds of one route and level.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RankRow {
    /// 0..=8.
    pub route: u8,
    /// 1..=3.
    pub level: u8,
    /// What is ranked (FUN_140207400): 0 the medal value, 1 ships
    /// destroyed, 2 the score.
    pub kind: u8,
    /// The value that reaches rank n + 1 is `thresholds[n]`.
    pub thresholds: [i32; 16],
}

/// A treasure.bin record (0x20 bytes).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Treasure {
    /// The obtained bit (save +0x10f60, a bitset of u32 words).
    pub bit: u32,
    pub item: u32,
    pub route: u8,
    /// 1..=3, plus 100 for EX.
    pub level: u8,
    /// 0 for a rank prize, else the treasure's place in the stage.
    pub stage: u8,
    /// The prize slot of a rank prize, 0..=6.
    pub slot: u8,
    pub flag: u32,
}

fn records(bytes: &[u8], size: usize) -> Option<impl Iterator<Item = &[u8]>> {
    let count = u32::from_le_bytes(bytes.get(4..8)?.try_into().ok()?) as usize;
    let body = bytes.get(8..8 + count.checked_mul(size)?)?;
    Some(body.chunks_exact(size))
}

pub fn read_rank(bytes: &[u8]) -> Option<Vec<RankRow>> {
    records(bytes, 0x80)?
        .map(|r| {
            let mut thresholds = [0i32; 16];
            for (k, t) in thresholds.iter_mut().enumerate() {
                *t = i32::from_le_bytes(r[4 + k * 4..8 + k * 4].try_into().ok()?);
            }
            Some(RankRow {
                route: r[0],
                level: r[1],
                kind: r[2],
                thresholds,
            })
        })
        .collect()
}

pub fn read_treasure(bytes: &[u8]) -> Option<Vec<Treasure>> {
    records(bytes, 0x20)?
        .map(|r| {
            let word = |at: usize| u32::from_le_bytes(r[at..at + 4].try_into().unwrap());
            Some(Treasure {
                bit: word(0),
                item: word(4),
                route: r[8],
                level: r[9],
                stage: r[10],
                slot: r[11],
                flag: word(12),
            })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rows_from_generated_bytes() {
        let mut rank = vec![1, 0, 0, 0, 2, 0, 0, 0];
        for (route, level) in [(4u8, 2u8), (8, 3)] {
            let mut r = vec![route, level, level - 1, 0];
            for k in 0..16i32 {
                r.extend((k * 10 + route as i32).to_le_bytes());
            }
            r.resize(0x80, 0);
            rank.extend(r);
        }
        let rows = read_rank(&rank).unwrap();
        assert_eq!(rows.len(), 2);
        assert_eq!((rows[1].route, rows[1].level, rows[1].kind), (8, 3, 2));
        assert_eq!(rows[1].thresholds[15], 158);
        assert!(read_rank(&rank[..rank.len() - 1]).is_none());

        let mut treasure = vec![1, 0, 0, 0, 1, 0, 0, 0];
        treasure.extend(317u32.to_le_bytes());
        treasure.extend(59u32.to_le_bytes());
        treasure.extend([2, 103, 4, 0]);
        treasure.extend(7u32.to_le_bytes());
        treasure.resize(8 + 0x20, 0);
        assert_eq!(
            read_treasure(&treasure).unwrap(),
            vec![Treasure {
                bit: 317,
                item: 59,
                route: 2,
                level: 103,
                stage: 4,
                slot: 0,
                flag: 7,
            }]
        );
    }
}
