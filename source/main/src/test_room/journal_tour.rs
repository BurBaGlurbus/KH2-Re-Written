//! The unattended `--journal-tour`: START opens the camp, the cursor goes
//! down to Jiminy's Journal, the journal opens on Notes, R1 goes to
//! Collection and its Limits, Minigames and Missions rows open their pages.
//! The tour shows everything (story flag 0x4837) and gives the first limits
//! use counters, some minigame records and a story flag [test-room values:
//! the room has no limit users, minigames or missions yet].
use super::ability_inventory::{Inventory, Tour};

const START: u64 = 0x2;
const DECIDE: u64 = 0x4;
const CANCEL: u64 = 0x8;
const UP: u64 = 0x10;
const DOWN: u64 = 0x20;
const R1: u64 = 0x4000;
const L1: u64 = 0x8000;
const R2: u64 = 0x1_0000;
const RIGHT: u64 = 0x80;
const ROTATE: u64 = 0x4_0000;

/// Use counters given to the first limits (record order).
pub const LIMIT_COUNTS: [u16; 4] = [12, 3, 7, 21];
/// Story flags set for the tour: 0x4837 shows everything, 0x8b0 makes the
/// Mail Delivery mission rows available.
pub const STORY_FLAGS: [u16; 2] = [0x4837, 0x8b0];
/// The Moogle seen at level 3 with some materials, lists and recipes.
pub const SYNTHESIS: super::journal::Synthesis = super::journal::Synthesis {
    moogle_seen: true,
    level: 2,
    materials: 17,
    collections: 4,
    recipes: 9,
};
/// The first capture is the camp's top level (the Journal row's NEW!).
/// From 3780: the puzzle menu and list, the tutorial, the board and its
/// grab, the banner, the solved board, a rotating board, a 6 x 8 board and
/// the List of Pieces.
pub const CAPTURES: [u64; 23] = [
    130, 560, 880, 1000, 1240, 1620, 1740, 2020, 2300, 2820, 2960, 3440, 3780, 3900, 4040, 4300,
    4440, 4700, 5000, 5300, 5600, 5980, 6340,
];
pub const END: u64 = 6400;

/// The tour's pieces: Awakening whole with its first two tiles swapped, half
/// of Heart (they rotate) and some of Sunset (6 x 8).
pub fn puzzles() -> super::journal::PuzzleSave {
    let mut save = super::journal::PuzzleSave::default();
    for k in 0..12 {
        save.owned.insert((0, k));
        save.slots[0][k as usize] = super::journal::PuzzleSlot {
            piece: if k < 2 { 1 - k as i8 } else { k as i8 },
            rot: 0,
        };
    }
    save.owned.extend((0..6).map(|k| (1, k)));
    save.owned.extend((0..20).map(|k| (5, k)));
    save
}

/// The minigame records (state, value) by record index: Mail Delivery
/// (0xe) a time of 0'52"30, Grandstander (0xf) a score, record 0 a round
/// and record 1 a played game.
pub fn minigame_records() -> Vec<(u8, u32)> {
    let mut r = vec![(0, 0); 0x40];
    r[0xe] = (4, 52 * 60 + 18);
    r[0xf] = (3, 1234);
    r[0] = (2, 5);
    r[1] = (1, 0);
    r
}

pub fn tour(inventory: &Inventory) -> Tour {
    let mut inputs = vec![(40, START)];
    // The camp's Journal row is the sixth.
    for k in 0..5 {
        inputs.push((140 + 10 * k, DOWN));
    }
    inputs.push((200, DECIDE));
    // The book opens on Notes; R1 turns to Collection.
    inputs.push((560, R1));
    // Collection's tenth option is Limits.
    for k in 0..9 {
        inputs.push((700 + 10 * k, DOWN));
    }
    inputs.push((810, DECIDE));
    // The list: the first (owned) limit opens; R2 turns to the next one.
    inputs.push((900, DECIDE));
    inputs.push((1020, R2));
    // Back to the list and Collection, then Minigames (the row above).
    inputs.push((1260, CANCEL));
    inputs.push((1340, CANCEL));
    inputs.push((1440, UP));
    inputs.push((1500, DECIDE));
    // Scroll the minigame list a little, then leave.
    for k in 0..3 {
        inputs.push((1640 + 20 * k, DOWN));
    }
    inputs.push((1780, CANCEL));
    // Missions, the row above Minigames.
    inputs.push((1880, UP));
    inputs.push((1940, DECIDE));
    for k in 0..6 {
        inputs.push((2080 + 20 * k, DOWN));
    }
    // Back to Collection, L1 to Notes and its Story row.
    inputs.push((2400, CANCEL));
    inputs.push((2480, L1));
    inputs.push((2640, DOWN));
    inputs.push((2700, DECIDE));
    for k in 0..4 {
        inputs.push((2860 + 20 * k, DOWN));
    }
    // Back to Notes, R1 to Collection and its Synthesis Notes row.
    inputs.push((3000, CANCEL));
    inputs.push((3100, R1));
    for k in 0..3 {
        inputs.push((3220 + 20 * k, DOWN));
    }
    inputs.push((3320, DECIDE));
    // Back to Collection and its Puzzle Pieces row, five above Synthesis.
    inputs.push((3460, CANCEL));
    for k in 0..5 {
        inputs.push((3560 + 10 * k, UP));
    }
    inputs.push((3640, DECIDE));
    // The puzzle list, the first board by way of the tutorial.
    inputs.push((3800, DECIDE));
    inputs.push((3920, DECIDE));
    for k in 0..4 {
        inputs.push((4060 + 10 * k, DOWN));
    }
    inputs.push((4120, DECIDE));
    // Grab the first tile, carry it right, then place it: solved.
    inputs.push((4400, DECIDE));
    inputs.push((4420, RIGHT));
    inputs.push((4460, DECIDE));
    inputs.push((4720, DECIDE));
    // R2 to the next board; turn its first piece.
    inputs.push((5020, R2));
    inputs.push((5280, ROTATE));
    inputs.push((5320, R2));
    // Back to the list, down to Sunset (6 x 8); then the menu and its
    // List of Pieces.
    inputs.push((5620, CANCEL));
    for k in 0..3 {
        inputs.push((5720 + 10 * k, DOWN));
    }
    inputs.push((5760, DECIDE));
    inputs.push((6000, CANCEL));
    inputs.push((6100, CANCEL));
    inputs.push((6180, DOWN));
    inputs.push((6200, DECIDE));
    Tour {
        inputs,
        captures: CAPTURES.to_vec(),
        close: 10_000,
        end: END,
        expected_slots: inventory.slots.clone(),
        attacks: Vec::new(),
    }
}
