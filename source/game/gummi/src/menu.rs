//! The Gummi menu's screen rules: the row selector (gm::GUMIMENU_SELECTOR,
//! FUN_140274850) and what the top menu's rows lead to (FUN_14024e4e0).

/// Pad bits the selector reads: pressed this frame (`pressed`, pad +8) and
/// pressed or repeating (`repeat`, pad +0x18).
pub mod pad {
    pub const DECIDE: u64 = 0x4;
    pub const CANCEL: u64 = 0x8;
    pub const UP: u64 = 0x10;
    pub const DOWN: u64 = 0x20;
}

/// The menu's sound ids: bank × 1000 + entry, so 0xf3689 is se997 #1
/// (FUN_140271ca0).
pub mod sound {
    pub const MOVE: u32 = 0xf3689;
    pub const DECIDE: u32 = 0xf368a;
    pub const CANCEL: u32 = 0xf368b;
    pub const REFUSED: u32 = 0xf368c;
    pub const MISSION: u32 = 0xf3701;
    pub const OPEN: u32 = 0xf36fe;
}

/// What one frame of the selector did.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Step {
    None,
    Moved,
    Decide(usize),
    Cancel,
}

/// FUN_140274850: decide, then cancel, then the cursor. A move repeats while
/// held, but wraps past either end only on a fresh press.
pub fn selector(cursor: &mut usize, rows: usize, pressed: u64, repeat: u64) -> Step {
    if pressed & pad::DECIDE != 0 {
        return Step::Decide(*cursor);
    }
    if pressed & pad::CANCEL != 0 {
        return Step::Cancel;
    }
    let before = *cursor;
    if repeat & pad::DOWN != 0 {
        if *cursor + 1 < rows {
            *cursor += 1;
        } else if pressed & pad::DOWN != 0 {
            *cursor = 0;
        }
    } else if repeat & pad::UP != 0 {
        if *cursor > 0 {
            *cursor -= 1;
        } else if pressed & pad::UP != 0 {
            *cursor = rows - 1;
        }
    }
    if *cursor != before {
        Step::Moved
    } else {
        Step::None
    }
}

/// The top menu's rows: their `menuentry.bar` record and help text.
pub const TOP_ROWS: [(usize, u32); 3] = [(0, 0x2102), (1, 0x2103), (4, 0x2119)];
/// Cancel on the top menu moves the cursor here ("World Map").
pub const TOP_CANCEL_ROW: usize = 2;

/// Where a top menu row leads.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TopChoice {
    /// Mission select (screen 1).
    Mission,
    /// Straight to ship select (screen 2) while level 1 was never cleared.
    ShipSelect,
    /// The editor: its entry screen (5) when blueprints are owned, else
    /// its ship select (6).
    Editor { blueprints: bool },
    /// The leave window (message 0x4ab0), then the world map.
    WorldMap,
}

pub fn top_choice(row: usize, level1_cleared: bool, blueprints: bool) -> TopChoice {
    match row {
        0 if level1_cleared => TopChoice::Mission,
        0 => TopChoice::ShipSelect,
        1 => TopChoice::Editor { blueprints },
        _ => TopChoice::WorldMap,
    }
}

/// The world map entrance a Gummi route returns to (FUN_140153160,
/// DAT_1405b15e8): 13 + route, `route` 0..=8.
pub fn return_entrance(route: u8) -> u8 {
    13 + route
}

/// Mission select's extra pad bits (pad +0x18): the carousel's next and
/// previous level, and the EX toggle (pad +8; which button it is isn't
/// traced).
pub mod mission_pad {
    pub const NEXT: u64 = 0x80;
    pub const PREVIOUS: u64 = 0x40;
    pub const EX: u64 = 0x10000;
}

/// Mission select's sound ids.
pub mod mission_sound {
    pub const NEXT: u32 = 0xf3705;
    pub const PREVIOUS: u32 = 0xf3706;
    pub const EX: u32 = 0xf36f6;
    pub const OPEN: u32 = 0xf3704;
}

/// Levels on the carousel (FUN_14024ee90): two, and the third once it's
/// unlocked. `normal` is the route's three normal records.
pub fn level_count(normal: &[crate::Mission; 3]) -> usize {
    2 + usize::from(normal[2].unlocked)
}

/// FUN_140277b50: EX can be chosen for a level whose EX record is unlocked.
pub fn ex_available(ex: &[crate::Mission; 3], index: usize) -> bool {
    ex.get(index).is_some_and(|m| m.unlocked)
}

/// The carousel's turn (FUN_140276dd0, 5° a frame): input waits this long.
pub fn turn_frames(levels: usize) -> u32 {
    (360 / levels as u32) / 5
}

/// The level panel's records: MISSION_L1..L3.
pub const LEVEL_PANELS: [usize; 3] = [0x10, 0x11, 0x12];

/// The panel's re-skin for a level change (tables 0x1405b77e0 for next,
/// 0x1405b7760 for previous): the record whose groups slide it.
pub fn level_slide(from: usize, to: usize, next: bool) -> Option<usize> {
    Some(match (from, to, next) {
        (0, 1, true) => 0x14,
        (0, 1, false) => 0x248,
        (1, 0, true) => 0x249,
        (1, 0, false) => 0x15,
        (1, 2, _) => 0x16,
        (2, 1, _) => 0x17,
        (2, 0, _) => 0x152,
        (0, 2, _) => 0x153,
        _ => return None,
    })
}

/// What the level panel's counter shows (FUN_140277e30): its record
/// (MISSION_EMBLEM, MISSION_DESTROY, MISSION_SCORE), digit style, digit
/// count and value.
pub fn level_value(level: usize, m: &crate::Mission) -> (usize, usize, usize, u32) {
    match level {
        0 => (0x156, 1, 2, u32::from(m.medals)),
        1 => (0x158, 3, 7, u32::from(m.destroyed())),
        _ => (0x159, 0, 10, m.score),
    }
}

/// The rank counter: style 2 below rank 10, else 5, one digit.
pub fn rank_digit(rank: u8) -> (usize, u32) {
    (if rank < 10 { 2 } else { 5 }, u32::from(rank % 10))
}

/// The level's description: the route table's text, 0x483a + level for a
/// normal mission, the EX condition 0x4f8d + 3 × route + level for EX.
pub fn level_text(route: u8, level: usize, ex: bool) -> u32 {
    if ex {
        0x4f8d + 3 * route as u32 + level as u32
    } else {
        0x483a + level as u32
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_selector_wraps_only_on_a_fresh_press() {
        let mut c = 0;
        assert_eq!(selector(&mut c, 3, 0, pad::DOWN), Step::Moved);
        assert_eq!(selector(&mut c, 3, 0, pad::DOWN), Step::Moved);
        assert_eq!(c, 2);
        // Held at the end: no wrap.
        assert_eq!(selector(&mut c, 3, 0, pad::DOWN), Step::None);
        assert_eq!(selector(&mut c, 3, pad::DOWN, pad::DOWN), Step::Moved);
        assert_eq!(c, 0);
        assert_eq!(selector(&mut c, 3, 0, pad::UP), Step::None);
        assert_eq!(selector(&mut c, 3, pad::UP, pad::UP), Step::Moved);
        assert_eq!(c, 2);
        assert_eq!(
            selector(&mut c, 3, pad::DECIDE | pad::CANCEL, 0),
            Step::Decide(2)
        );
        assert_eq!(selector(&mut c, 3, pad::CANCEL, pad::DOWN), Step::Cancel);
    }

    #[test]
    fn mission_select_rules() {
        let mut normal = [crate::Mission::default(); 3];
        assert_eq!(level_count(&normal), 2);
        normal[2].unlocked = true;
        assert_eq!(level_count(&normal), 3);
        assert!(!ex_available(&normal, 3));
        assert!(ex_available(&normal, 2));
        assert_eq!((turn_frames(2), turn_frames(3)), (36, 24));
        assert_eq!(level_slide(0, 1, true), Some(0x14));
        assert_eq!(level_slide(0, 1, false), Some(0x248));
        assert_eq!(level_slide(0, 2, false), Some(0x153));
        assert_eq!(level_slide(3, 0, true), None);
        let mut m = crate::Mission {
            medals: 12,
            score: 4321,
            ..Default::default()
        };
        m.counters[1] = 77;
        assert_eq!(level_value(0, &m), (0x156, 1, 2, 12));
        assert_eq!(level_value(1, &m), (0x158, 3, 7, 77));
        assert_eq!(level_value(2, &m), (0x159, 0, 10, 4321));
        assert_eq!(rank_digit(6), (2, 6));
        assert_eq!(rank_digit(12), (5, 2));
        assert_eq!(level_text(0, 1, false), 0x483b);
        assert_eq!(level_text(2, 1, true), 0x4f94);
    }

    #[test]
    fn top_rows_lead_on() {
        assert_eq!(top_choice(0, false, false), TopChoice::ShipSelect);
        assert_eq!(top_choice(0, true, false), TopChoice::Mission);
        assert_eq!(
            top_choice(1, true, true),
            TopChoice::Editor { blueprints: true }
        );
        assert_eq!(top_choice(2, true, true), TopChoice::WorldMap);
        assert_eq!(return_entrance(0), 13);
        assert_eq!(return_entrance(8), 21);
    }
}
