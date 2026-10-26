//! Sora's equipment in the test room: the stat rows each item adds to his
//! status totals (FUN_1403c1d90 / FUN_1403c1190, rebuilt whenever the equipment
//! or the active Drive form's weapon changes, FUN_1402fc760).
use super::*;
use player::equipment_stats::{Row, Totals};

/// The stat row of every weapon, armor and accessory item, keyed by item id
/// (FUN_1403e3ef0 selects the table key by category).
pub(super) fn rows(
    table: &[data::item_table::Item],
    stats: &[data::item_table::Stats],
) -> HashMap<u16, Row> {
    table
        .iter()
        .filter_map(|item| {
            let key = item.equipment_stats_id()?;
            let s = stats.iter().find(|s| s.id == key)?;
            Some((
                item.id,
                Row {
                    strength: s.strength,
                    magic: s.magic,
                    defense: s.defense,
                    ap: s.ap,
                    resist: s.resist,
                },
            ))
        })
        .collect()
}

/// The development room owns one of every Keyblade, staff, shield, armor and
/// accessory the item table gives a picture (the debug and dummy weapons have
/// none), minus the Kingdom Key, which the room starts with in the member's
/// weapon and each owned form's weapon slot [test-room choice, not campaign
/// progression].
pub(super) fn owned(table: &[data::item_table::Item]) -> Vec<u16> {
    table
        .iter()
        .filter(|i| matches!(i.kind, 2..=4 | 14 | 15) && i.picture != 0 && i.id != KINGDOM_KEY)
        .map(|i| i.id)
        .collect()
}

/// The room's stock with the owned equipment added.
pub(super) fn with_owned_equipment(
    mut stock: std::collections::BTreeMap<u16, u8>,
    owned: &[u16],
) -> std::collections::BTreeMap<u16, u8> {
    for &id in owned {
        stock.insert(id, 1);
    }
    stock
}

/// FUN_1403c1d90's item pass over a zeroed start: what the equipment adds to
/// Strength, Magic, Defense and AP, and the resists it leaves.
pub(super) fn totals(
    rows: &HashMap<u16, Row>,
    equipment: &pause_menu::items::Equipment,
    form_weapon: Option<u16>,
) -> Totals {
    let items = std::iter::once(equipment.weapon)
        .chain(std::iter::once(equipment.secondary_weapon))
        .chain(form_weapon)
        .chain(equipment.armor.iter().copied())
        .chain(equipment.accessories.iter().copied());
    player::equipment_stats::totals(
        Totals::start(0, 0, 0, 0),
        items.filter_map(|id| rows.get(&id).copied()),
    )
}

impl Game {
    /// The Drive form's own weapon while a form is active (stats +0x254).
    pub(super) fn form_weapon(&self) -> Option<u16> {
        self.drive
            .as_ref()
            .filter(|d| d.active)
            .and_then(|d| d.forms.record(d.form))
            .map(|r| r.keyblade)
    }

    /// FUN_1402fc760 → FUN_1403c2100: rebuilds the equipment's share of the
    /// status totals and moves the combat numbers by what changed.
    pub(super) fn refresh_gear(&mut self) {
        let new = self
            .ability_source
            .gear(&self.equipment, self.form_weapon());
        let old = self.gear;
        if new == old {
            return;
        }
        if self.enemy.is_some() {
            let v = &mut self.sora.vitals;
            v.attacker.strength += i32::from(new.strength) - i32::from(old.strength);
            v.attacker.magic += i32::from(new.magic) - i32::from(old.magic);
            v.target.defense += i32::from(new.defense) - i32::from(old.defense);
            v.target.weakness = new.resist.map(i32::from);
        }
        self.gear = new;
        self.event(
            serde_json::json!({"event":"equipment_stats","strength":new.strength,
            "magic":new.magic,"defense":new.defense,"ap":new.ap,"resist":new.resist}),
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn row(strength: u8, defense: u8, resist: [u8; 7]) -> Row {
        Row {
            strength,
            magic: 0,
            defense,
            ap: 0,
            resist,
        }
    }

    #[test]
    fn every_equipped_slot_contributes_in_the_executable_order() {
        let rows: HashMap<u16, Row> = [
            (1, row(3, 0, [100; 7])),
            (2, row(0, 3, [100, 80, 100, 100, 100, 100, 100])),
            (3, row(0, 2, [100, 80, 100, 100, 100, 100, 100])),
            (4, row(5, 0, [100; 7])),
        ]
        .into();
        let equipment = pause_menu::items::Equipment {
            weapon: 1,
            secondary_weapon: 0,
            armor: vec![2, 3],
            accessories: vec![0, 99],
            ..Default::default()
        };
        let t = totals(&rows, &equipment, Some(4));
        assert_eq!((t.strength, t.defense), (8, 5));
        // 80% of 80% rounds up: (80*80+99)/100 = 64.
        assert_eq!(t.resist[1], 64);
        let without_form = totals(&rows, &equipment, None);
        assert_eq!(without_form.strength, 3);
    }
}
