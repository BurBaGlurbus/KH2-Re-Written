//! A bounded tour through the real camp controls that equips every Keyblade,
//! then every armor and every accessory in turn, so the room's status
//! numbers, abilities and Keyblade model can be compared item by item with
//! the installed tables. Captures and the event log stay outside Git.
use super::ability_inventory::Tour;
use super::*;
use pause_menu::items::ItemInfo;

/// Camp-mode logical bits (mode 7).
const START: u64 = 0x2;
const DECIDE: u64 = 0x4;
const DOWN: u64 = 0x20;

/// Ticks the camp takes to open, to change screen, and for a closed camp to
/// settle; and the wait for a rebuilt Keyblade kit (loaded off-thread).
const OPEN: u64 = 80;
const SCREEN: u64 = 90;
const STEP: u64 = 10;
const CLOSE: u64 = 100;
const KIT_WAIT: u64 = 600;

struct Script {
    inputs: Vec<(u64, u64)>,
    captures: Vec<u64>,
    attacks: Vec<u64>,
    tick: u64,
    /// Capture the screen after each of the first presses (quick mode).
    shots: usize,
}

impl Script {
    fn press(&mut self, bits: u64, wait: u64) {
        self.inputs.push((self.tick, bits));
        self.tick += wait;
        if self.shots > 0 && wait >= SCREEN {
            self.shots -= 1;
            self.captures.push(self.tick - 8);
        }
    }
    fn downs(&mut self, n: usize) {
        for _ in 0..n {
            self.press(DOWN, STEP);
        }
    }
    /// START, then Items → the member → the Equip slot list.
    fn open_equip(&mut self) {
        self.press(START, OPEN);
        self.press(DECIDE, SCREEN);
        self.press(DECIDE, SCREEN);
    }
}

/// The items of `kind` in table order that the room owns (plus `equipped`).
fn of_kind(table: &[ItemInfo], kind: u8, owned: &[u16], equipped: &[u16]) -> Vec<u16> {
    table
        .iter()
        .filter(|r| r.kind == kind && (owned.contains(&r.id) || equipped.contains(&r.id)))
        .map(|r| r.id)
        .collect()
}

/// The camp's choices for one item picked out of `order`, with `current`
/// equipped (and so not in stock): its row in the pick list, below the
/// unequip row when `unequip_row`.
fn row(order: &[u16], current: u16, target: u16, unequip_row: bool) -> usize {
    let stocked: Vec<u16> = order.iter().copied().filter(|&i| i != current).collect();
    let at = stocked.iter().position(|&i| i == target).unwrap_or(0);
    at + usize::from(unequip_row)
}

/// Section rows of Sora's Equip list: the weapon, the three form weapons,
/// four armor and four accessory slots.
const ARMOR_ROW: usize = 4;
const ACCESSORY_ROW: usize = 8;

/// `quick` limits the walk to the first few items of each kind.
pub fn tour(table: &[ItemInfo], owned: &[u16], slots: Vec<u16>, quick: bool) -> Tour {
    let keyblades = of_kind(table, 2, owned, &[KINGDOM_KEY]);
    let mut armors = of_kind(table, 14, owned, &[]);
    let mut accessories = of_kind(table, 15, owned, &[]);
    if quick {
        armors.truncate(3);
        accessories.truncate(3);
    }
    let mut s = Script {
        inputs: Vec::new(),
        captures: Vec::new(),
        attacks: Vec::new(),
        tick: 40,
        shots: if quick { 8 } else { 0 },
    };
    // Every Keyblade, the Kingdom Key last: close the camp so the kit
    // rebuilds, then swing it.
    let mut equipped = KINGDOM_KEY;
    let mut targets: Vec<u16> = keyblades
        .iter()
        .copied()
        .filter(|&k| k != KINGDOM_KEY)
        .collect();
    if quick {
        targets.truncate(2);
    }
    targets.push(KINGDOM_KEY);
    for target in targets {
        s.open_equip();
        // The weapon slot: the pick list's first row is its first Keyblade.
        s.press(DECIDE, SCREEN);
        s.downs(row(&keyblades, equipped, target, false));
        s.press(DECIDE, SCREEN);
        s.press(START, CLOSE);
        equipped = target;
        s.tick += KIT_WAIT;
        s.attacks.push(s.tick);
        s.tick += 24;
        s.captures.push(s.tick);
        s.tick += 90;
    }
    // Armor in the first slot, then empty again.
    s.open_equip();
    s.downs(ARMOR_ROW);
    let mut current = 0;
    for &target in armors.iter().chain([&0]) {
        s.press(DECIDE, SCREEN);
        let at = if target == 0 {
            0
        } else {
            row(&armors, current, target, true)
        };
        s.downs(at);
        s.press(DECIDE, SCREEN);
        current = target;
        s.tick += 20;
    }
    s.press(START, CLOSE);
    s.tick += 60;
    // The same for the accessories.
    s.open_equip();
    // The Equip list reopens with its cursor on the armor row.
    s.downs(ACCESSORY_ROW - ARMOR_ROW);
    let mut current = 0;
    for &target in accessories.iter().chain([&0]) {
        s.press(DECIDE, SCREEN);
        let at = if target == 0 {
            0
        } else {
            row(&accessories, current, target, true)
        };
        s.downs(at);
        s.press(DECIDE, SCREEN);
        current = target;
        s.tick += 20;
    }
    s.press(START, CLOSE);
    s.tick += 120;
    let end = s.tick;
    Tour {
        inputs: s.inputs,
        captures: s.captures,
        close: u64::MAX / 2,
        end,
        expected_slots: slots,
        attacks: s.attacks,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_pick_list_skips_the_equipped_item() {
        let order = [10, 11, 12, 13];
        // 12 is picked with 11 equipped: the list is 10, 12, 13.
        assert_eq!(row(&order, 11, 12, false), 1);
        // Armor lists start with the unequip row.
        assert_eq!(row(&order, 0, 10, true), 1);
        assert_eq!(row(&order, 10, 13, true), 3);
    }
}
