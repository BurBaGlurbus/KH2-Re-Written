//! The unattended `--command-deck-tour <dir>` (in the magic tour's setup:
//! Sora stands still, the Shadow is frozen nearby): left swaps to the
//! second deck, its Summon, Party and Limit lists open and close, left
//! swaps back, then the second deck is shown on every installed world
//! layout in turn. The tour owns the four summon charms [test-room values:
//! the room has no charms otherwise]. Before that, the Magic list opens and L1
//! is held over it: the shortcut menu takes its place and casts one spell.
use visual::field_command_menu::Buttons;

/// The world layouts (field2d/<lang>/<code>command.2dd) the install holds.
pub const LAYOUTS: [&str; 31] = [
    "zz0", "es0", "tt0", "tt1", "tt2", "tt3", "tt4", "tt5", "tt6", "di0", "hb0", "hb1", "hb2",
    "bb0", "he0", "he1", "al0", "mu0", "po0", "lk0", "lm0", "dc0", "dc1", "wi0", "nm0", "nm1",
    "wm0", "ca0", "tr0", "eh0", "eh1",
];
/// The charms the tour owns (00battle sumn).
pub const CHARMS: [u16; 4] = [0x17f, 0x9f, 0x19, 0xa0];

#[derive(Clone, Copy)]
enum Press {
    Up,
    Down,
    Decide,
    Back,
    Left,
    Right,
}

const INPUTS: [(u64, Press); 15] = [
    // The Magic list, which L1 closes.
    (8, Press::Down),
    (12, Press::Decide),
    // The second deck, then Summon's list.
    (100, Press::Left),
    (250, Press::Down),
    (270, Press::Decide),
    // Party's list, opened with right and closed with left.
    (340, Press::Back),
    (360, Press::Down),
    (380, Press::Right),
    (450, Press::Left),
    // Limit's list.
    (470, Press::Down),
    (490, Press::Decide),
    (560, Press::Back),
    // Back to the top deck (the cursor stays on row 3, Drive), then the
    // second deck again with the cursor up to Party.
    (580, Press::Left),
    (720, Press::Left),
    (740, Press::Up),
];
/// The layout cycle: one layout every `LAYOUT_STEP` ticks from
/// `LAYOUT_START`, captured `LAYOUT_SETTLE` ticks after it loads.
const LAYOUT_START: u64 = 840;
const LAYOUT_STEP: u64 = 60;
const LAYOUT_SETTLE: u64 = 40;

/// L1 is held over these ticks (the shortcut menu): over the Magic list, and
/// over the second deck, with `--magic-form` once the form has come.
const SHORTCUT_HELD: [std::ops::Range<u64>; 2] = [30..70, 200..240];
/// Circle at this tick: the first shortcut.
const SHORTCUT_PRESS: u64 = 62;

pub fn shortcut_held(tick: u64) -> bool {
    SHORTCUT_HELD.iter().any(|r| r.contains(&tick))
}

pub fn shortcut_press(tick: u64) -> bool {
    tick == SHORTCUT_PRESS
}

pub fn press(tick: u64) -> Buttons {
    let mut b = Buttons::default();
    for &(at, p) in &INPUTS {
        if at == tick {
            match p {
                Press::Up => b.up = true,
                Press::Down => b.down = true,
                Press::Decide => b.decide = true,
                Press::Back => b.back = true,
                Press::Left => b.left = true,
                Press::Right => b.right = true,
            }
        }
    }
    b
}

/// The layout loaded at `tick`, if one is.
pub fn layout_at(tick: u64) -> Option<&'static str> {
    let k = tick.checked_sub(LAYOUT_START)?;
    if !k.is_multiple_of(LAYOUT_STEP) {
        return None;
    }
    LAYOUTS.get((k / LAYOUT_STEP) as usize).copied()
}

/// The reaction button the tour presses during a limit's follow-up windows:
/// every 12 ticks (the tour does not run a limit yet).
pub fn reaction(tick: u64) -> bool {
    tick.is_multiple_of(12)
}

pub fn captures() -> Vec<u64> {
    let mut c = vec![
        20, 31, 34, 40, 50, 60, 71, 74, 80, 90, 104, 140, 215, 230, 245, 320, 430, 540, 600, 700,
        800,
    ];
    c.extend((0..LAYOUTS.len() as u64).map(|k| LAYOUT_START + k * LAYOUT_STEP + LAYOUT_SETTLE));
    c
}

pub fn end() -> u64 {
    LAYOUT_START + LAYOUTS.len() as u64 * LAYOUT_STEP + 20
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_layout_gets_a_capture_after_it_loads() {
        let caps = captures();
        for (k, code) in LAYOUTS.iter().enumerate() {
            let at = LAYOUT_START + k as u64 * LAYOUT_STEP;
            assert_eq!(layout_at(at), Some(*code));
            assert!(caps.contains(&(at + LAYOUT_SETTLE)));
        }
        assert!(layout_at(LAYOUT_START + 1).is_none());
        assert!(caps.iter().all(|&c| c < end()));
        assert!(press(100).left && !press(101).left);
    }
}
