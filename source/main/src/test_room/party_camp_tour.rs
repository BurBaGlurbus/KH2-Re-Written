//! The unattended `--party-camp-tour <dir>` (in the ability tour's setup,
//! a room with Donald and Goofy): the camp's Customize, Abilities, Status
//! and Equip pages of Donald, each switched to Goofy with R2, the top
//! level's and Party's member models, and a check that Donald's record
//! took every change.
//!
//! Customize: △ on Battle Style (Technic → Target), △ on Thunder (Free →
//! Well-Balanced), its popup's Rare, △ twice on Potion (OFF+Often →
//! ON+Often → OFF+Last hope!), its popup's ON+Last hope!. Abilities: △
//! twice on Thunder (Rare → unequipped → Free), its popup's Well-Balanced.
use super::ability_inventory::Tour;

/// Logical camp bits (pad mode 7).
const OPEN: u64 = 0x2;
const UP: u64 = 0x10;
const DOWN: u64 = 0x20;
const DECIDE: u64 = 0x4;
const CANCEL: u64 = 0x8;
const TRIANGLE: u64 = 0x1000;
const R2: u64 = 0x1_0000;

/// Waits before each press: a cursor move, and a new screen's opening.
const MOVE: u64 = 15;
const SCREEN: u64 = 90;

/// (wait, press, capture 35 ticks after it).
const STEPS: &[(u64, u64, bool)] = &[
    // Top: the party frame with the three models; Items → Abilities →
    // Customize.
    (20, DOWN, true),
    (MOVE, DOWN, false),
    (20, DECIDE, false),
    // 0x19: Donald's preview.
    (SCREEN, DOWN, true),
    (20, DECIDE, false),
    // 0x1b: the style, Thunder, its popup's Rare.
    (SCREEN, TRIANGLE, true),
    (20, DOWN, false),
    (MOVE, TRIANGLE, true),
    (30, DECIDE, true),
    (SCREEN, DOWN, false),
    (MOVE, DECIDE, true),
    // Potion: two △, then its popup's ON+Last hope!.
    (SCREEN, DOWN, false),
    (MOVE, DOWN, false),
    (MOVE, TRIANGLE, true),
    (40, TRIANGLE, true),
    (40, DECIDE, true),
    (SCREEN, DOWN, false),
    (MOVE, DECIDE, true),
    // R2: Goofy's list; back to the top.
    (SCREEN, R2, true),
    (SCREEN, CANCEL, false),
    (SCREEN, CANCEL, false),
    // Abilities: Donald's preview, list, △ twice, popup's Well-Balanced.
    (SCREEN, UP, false),
    (20, DECIDE, false),
    (SCREEN, DOWN, true),
    (20, DECIDE, false),
    (SCREEN, TRIANGLE, true),
    (40, TRIANGLE, true),
    (40, DECIDE, true),
    (SCREEN, DOWN, false),
    (MOVE, DECIDE, true),
    (SCREEN, R2, true),
    (SCREEN, CANCEL, false),
    (SCREEN, CANCEL, false),
    // Status: from Sora up to Goofy, then Donald, his stats.
    (SCREEN, DOWN, false),
    (MOVE, DOWN, false),
    (MOVE, DOWN, false),
    (20, DECIDE, true),
    (SCREEN, UP, true),
    (20, UP, true),
    (20, DECIDE, true),
    (SCREEN, CANCEL, false),
    (SCREEN, CANCEL, false),
    // Items: Donald → Equip, his weapon list, R2 to Goofy.
    (SCREEN, UP, false),
    (MOVE, UP, false),
    (MOVE, UP, false),
    (MOVE, UP, false),
    (20, DECIDE, false),
    (SCREEN, DOWN, true),
    (20, DECIDE, true),
    (SCREEN, DECIDE, true),
    (SCREEN, CANCEL, false),
    (SCREEN, R2, true),
    (SCREEN, CANCEL, false),
    (SCREEN, CANCEL, false),
    // Party: the members' cards with their models.
    (SCREEN, DOWN, false),
    (MOVE, DOWN, false),
    (MOVE, DOWN, false),
    (20, DECIDE, false),
    (SCREEN, UP, true),
    (SCREEN, CANCEL, false),
    (SCREEN, CANCEL, false),
];

/// The camp opens at this tick.
const START: u64 = 40;

pub fn tour(slots: Vec<u16>) -> Tour {
    let mut inputs = vec![(START, OPEN)];
    let mut captures = Vec::new();
    let mut tick = START;
    for &(wait, press, capture) in STEPS {
        tick += wait;
        inputs.push((tick, press));
        if capture {
            captures.push(tick + 35);
        }
    }
    Tour {
        inputs,
        captures,
        close: tick,
        end: tick + 100,
        expected_slots: slots,
        attacks: Vec::new(),
    }
}

/// Donald's record after the tour: Target Attack, Thunder equipped at
/// Well-Balanced, Potion on Auto-Reload at Last hope!.
pub fn donald_passes(s: &player::party_members::Save) -> bool {
    let thunder = s.abilities.contains(&0x80a7);
    let potion = s.originals[..usize::from(s.item_slots)].contains(&1);
    s.style == 1 && thunder && s.frequency[9] == 1 && potion && s.frequency[0] == 2
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn inputs_and_captures_end_before_the_close() {
        let t = tour(Vec::new());
        assert!(t.inputs.windows(2).all(|w| w[0].0 < w[1].0));
        assert!(t.captures.iter().all(|&c| c < t.close));
        assert_eq!(t.inputs.last().map(|i| i.0), Some(t.close));
    }
}
