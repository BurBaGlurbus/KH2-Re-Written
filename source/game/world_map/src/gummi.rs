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

/// The mission records live with the Gummi rules.
pub use ::gummi::{Mission, route_missions};

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
}
