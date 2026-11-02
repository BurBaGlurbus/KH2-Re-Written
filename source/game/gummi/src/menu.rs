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

/// The menu's sound ids (0xf36xx; how they map to the se997 bank isn't
/// traced).
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
