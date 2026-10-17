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

/// One Gummi mission record (save 0x1409b9990, 0x40 bytes each,
/// FUN_1402c76f0(route − 1, level, kind)): cleared (+0x1e), rank (+0x1b)
/// and score (+0x20, a float shown as three digits).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Mission {
    pub cleared: bool,
    pub rank: u8,
    pub score: f32,
}

/// The panel's state (+0x2ea4): 0 before the route's first mission is
/// cleared, 1 after it, 2 once its third is cleared too.
pub fn info_state(missions: &[Mission; 3]) -> usize {
    if !missions[0].cleared {
        0
    } else {
        1 + usize::from(missions[2].cleared)
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
        let cleared = Mission {
            cleared: true,
            ..Mission::default()
        };
        let none = Mission::default();
        assert_eq!(info_state(&[none; 3]), 0);
        assert_eq!(info_state(&[cleared, none, none]), 1);
        assert_eq!(info_state(&[cleared, none, cleared]), 2);
    }
}
