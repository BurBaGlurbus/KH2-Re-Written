//! The Gummi route objects' menu side (#88): the route menu's texts
//! (dk::WM_GLANDING, FUN_140398270) and the values of its info panel
//! dk::WM_GLANDING_INFO (FUN_140398ef0). The route objects' looks are in
//! `objects::gumi_look`; the menu's rules are `landing::Landing::route_menu`.

/// The route names (FUN_140397400: worldmap::Obj::VTABLE<worldmap::Gumi>
/// + 0x1a4, by route): sys 0x8224 + route − 1 for routes 1..=9.
pub fn route_name(route: u8) -> u32 {
    match route {
        1..=9 => 0x8223 + route as u32,
        _ => 0x8adc,
    }
}

/// The menu's first row per route (0x1405c2bce, 'wm' 0x4a4d..0x4a55); the
/// second is the landing menu's 0x4884.
pub fn route_row(route: u8) -> u32 {
    match route {
        1..=9 => 0x4a4c + route as u32,
        _ => 0x4884,
    }
}

/// The mission records: 54 of 0x40 bytes at save +0x100e0 (live
/// 0x1409b9990), nine routes of three levels, then the same for EX.
pub const RECORDS: usize = 0x100e0;
pub const RECORD_BYTES: usize = 0x40;
pub const RECORD_COUNT: usize = 54;

/// A record's index (FUN_1402c76f0): `route` 0..=8, `level` 1..=3.
pub fn record_index(route: u8, level: u8, ex: bool) -> usize {
    usize::from(ex) * 27 + route as usize * 3 + level as usize - 1
}

/// One Gummi mission record (FUN_140209750 writes it on the result screen).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Mission {
    /// +0x00, the best score.
    pub score: u32,
    /// +0x06, ships destroyed (the level 2 rank value).
    pub destroyed: u16,
    /// +0x1a, the best medal value.
    pub medals: u8,
    /// +0x1b, the rank, 0..=16.
    pub rank: u8,
    /// +0x1d.
    pub cleared: bool,
    /// +0x1e: the mission can be flown (FUN_140209b50). Level 1's and
    /// level 2's are set on level 1's first clear.
    pub unlocked: bool,
    /// +0x20, the share of the treasures taken, 0..=100.
    pub treasure: f32,
}

impl Mission {
    pub fn read(record: &[u8]) -> Option<Self> {
        let r = record.get(..RECORD_BYTES)?;
        Some(Self {
            score: u32::from_le_bytes(r[0..4].try_into().ok()?),
            destroyed: u16::from_le_bytes(r[6..8].try_into().ok()?),
            medals: r[0x1a],
            rank: r[0x1b],
            cleared: r[0x1d] != 0,
            unlocked: r[0x1e] != 0,
            treasure: f32::from_le_bytes(r[0x20..0x24].try_into().ok()?),
        })
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

/// The panel's state (+0x2ea4, FUN_140398ef0 reads +0x1e): 0 until the
/// route's first mission is unlocked, which its first clear does, 1 after,
/// 2 once its third is unlocked too.
pub fn info_state(missions: &[Mission; 3]) -> usize {
    if !missions[0].unlocked {
        0
    } else {
        1 + usize::from(missions[2].unlocked)
    }
}

/// The name frame's groups per state (0x1405c2c48 / c58 / c50): appear,
/// idle, close.
pub const NAME_GROUPS: [[usize; 3]; 3] =
    [[0x6a, 0x6b, 0x6c], [0x6d, 0x6e, 0x6f], [0x70, 0x71, 0x72]];

/// Rank sprites by rank byte (0x1405c2c60).
pub const RANK_GROUPS: [usize; 17] = [
    0x79, 0x77, 0x76, 0x75, 0x74, 0x73, 0x7a, 0x7b, 0x7d, 0x7e, 0x7f, 0x80, 0x81, 0x82, 0x83, 0x84,
    0x7c,
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn route_texts_and_states() {
        assert_eq!(route_name(1), 0x8224);
        assert_eq!(route_name(9), 0x822c);
        assert_eq!(route_row(1), 0x4a4d);
        assert_eq!(route_row(9), 0x4a55);
        let open = Mission {
            unlocked: true,
            ..Mission::default()
        };
        let none = Mission::default();
        assert_eq!(info_state(&[none; 3]), 0);
        assert_eq!(info_state(&[open, none, none]), 1);
        assert_eq!(info_state(&[open, none, open]), 2);
    }

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
        save[at + 0x1a] = 3;
        save[at + 0x1b] = 5;
        save[at + 0x1d] = 1;
        save[at + 0x1e] = 1;
        save[at + 0x20..at + 0x24].copy_from_slice(&50f32.to_le_bytes());
        let m = route_missions(&save, 2);
        assert_eq!(
            m[1][2],
            Mission {
                score: 1234,
                destroyed: 7,
                medals: 3,
                rank: 5,
                cleared: true,
                unlocked: true,
                treasure: 50.,
            }
        );
        assert_eq!(m[0][2], Mission::default());
        assert_eq!(route_missions(&save, 1), [[Mission::default(); 3]; 2]);
        assert_eq!(route_missions(&save[..10], 2), [[Mission::default(); 3]; 2]);
    }
}
