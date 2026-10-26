//! Development inventory derived from installed Sora templates and rewards.
//! Granting these now is a test-room choice, not the campaign unlock schedule.
use super::*;
use data::{battle_tables, item_table::Item};
use std::collections::BTreeMap;

pub struct Inventory {
    pub slots: Vec<u16>,
    pub ap: i32,
    /// Normal Sora's available tiers in each growth family (no form-only MAX).
    pub growth: [Vec<u16>; 5],
}

/// A bounded tour through the actual camp controls, followed by a growth jump.
pub struct Tour {
    pub inputs: Vec<(u64, u64)>,
    pub captures: Vec<u64>,
    pub close: u64,
    pub end: u64,
    pub expected_slots: Vec<u16>,
    /// Ticks the field's Attack button is pressed (the gear tour swings the
    /// new Keyblade).
    pub attacks: Vec<u64>,
}

impl Tour {
    pub fn new(table: &[pause_menu::items::ItemInfo], inventory: &Inventory) -> Self {
        let member = pause_menu::abilities::MemberAbilities {
            slots: inventory.slots.clone(),
            ap_max: inventory.ap,
            weapon_ability: 0,
            ..Default::default()
        };
        let entries = pause_menu::abilities::entries(table, &member);
        let mut inputs = vec![(40, 2), (100, 0x20), (140, 4), (200, 4)];
        let mut captures = vec![260];
        let (mut cursor, mut tick) = (0, 300);
        let mut expected_slots = inventory.slots.clone();
        for (index, entry) in entries.iter().enumerate() {
            if entry.cat != 1
                && !matches!(
                    entry.id,
                    abilities::item::COMBO_PLUS
                        | abilities::item::AIR_COMBO_PLUS
                        | abilities::item::COMBO_BOOST
                        | abilities::item::AIR_COMBO_BOOST
                        | abilities::item::REACTION_BOOST
                        | abilities::item::FINISHING_PLUS
                        | abilities::item::NEGATIVE_COMBO
                        | 0x21b
                )
            {
                continue;
            }
            while cursor < index {
                inputs.push((tick, 0x20));
                cursor += 1;
                tick += 8;
            }
            if !entry.equipped {
                inputs.push((tick, 0x1000));
            }
            if let Some(slot) = entry.slot {
                expected_slots[slot] |= 0x8000;
            }
            tick += 24;
            if entry.cat == 1 {
                captures.push(tick - 1);
            }
        }
        captures.push(tick + 20);
        let close = tick + 60;
        inputs.push((close, 2));
        captures.push(close + 100);
        Self {
            inputs,
            captures,
            close,
            end: close + 200,
            expected_slots,
            attacks: Vec::new(),
        }
    }

    pub fn logical(&self, tick: u64) -> u64 {
        self.inputs
            .iter()
            .filter(|r| r.0 == tick)
            .fold(0, |bits, r| bits | r.1)
    }
}

fn counts(ids: impl IntoIterator<Item = u16>, items: &[Item]) -> BTreeMap<u16, usize> {
    let mut out = BTreeMap::new();
    for raw in ids {
        let id = raw & 0x7fff;
        if items.iter().any(|r| r.id == id && r.kind == 0x13) {
            *out.entry(id).or_default() += 1;
        }
    }
    out
}

fn merge_max(into: &mut BTreeMap<u16, usize>, from: BTreeMap<u16, usize>) {
    for (id, n) in from {
        let old = into.entry(id).or_default();
        *old = (*old).max(n);
    }
}

/// Template availability is unioned; mutually exclusive level choices use
/// maximum multiplicity, and actual form rewards add to that progression.
fn build(
    items: &[Item],
    templates: &[Vec<u16>],
    levels: &[battle_tables::PlayerLevel],
    rewards: &[battle_tables::FormLevel],
) -> Result<Inventory, Box<dyn std::error::Error>> {
    let mut available = BTreeMap::new();
    for list in templates {
        merge_max(&mut available, counts(list.iter().copied(), items));
    }
    let mut progression = BTreeMap::new();
    for choice in 0..3 {
        merge_max(
            &mut progression,
            counts(levels.iter().map(|l| l.abilities[choice]), items),
        );
    }
    for (id, n) in counts(
        rewards
            .iter()
            .filter(|r| (1..=5).contains(&(r.id >> 4)) && r.id & 15 > 1)
            .map(|r| r.ability),
        items,
    ) {
        *progression.entry(id).or_default() += n;
    }
    merge_max(&mut available, progression);
    let mut growth: [Vec<u16>; 5] = std::array::from_fn(|_| Vec::new());
    for (family, bits) in abilities::GROWTH.iter().enumerate() {
        let top = items
            .iter()
            .filter(|r| available.contains_key(&r.id) && r.weapon_stats as u8 == 3)
            .filter_map(|r| bits.iter().position(|&b| b != 0 && r.amount == b as u16))
            .max();
        if let Some(top) = top {
            for &bit in &bits[..=top] {
                let item = items
                    .iter()
                    .find(|r| r.kind == 0x13 && r.weapon_stats as u8 == 3 && r.amount == bit as u16)
                    .ok_or("growth tier missing from installed item table")?;
                growth[family].push(item.id);
            }
            // Original progression replaces the previous tier in a single slot.
            for r in items.iter().filter(|r| {
                r.kind == 0x13
                    && r.weapon_stats as u8 == 3
                    && bits.iter().any(|&b| b != 0 && r.amount == b as u16)
            }) {
                available.remove(&r.id);
            }
            available.insert(growth[family][top], 1);
        }
    }
    let mut slots = Vec::new();
    let mut ap = 0;
    for item in items.iter().filter(|r| r.kind == 0x13) {
        for n in 0..available.get(&item.id).copied().unwrap_or(0) {
            let equipped = n == 0
                && matches!(
                    item.amount,
                    abilities::bit::GUARD | abilities::bit::AERIAL_RECOVERY
                );
            slots.push(item.id | if equipped { 0x8000 } else { 0 });
            ap += (item.weapon_stats >> 8) as i32;
        }
    }
    if slots.len() > 80 {
        return Err("installed Sora development inventory exceeds the 80 member slots".into());
    }
    slots.resize(80, 0);
    Ok(Inventory { slots, ap, growth })
}

pub fn load(items: &[Item], battle: &[u8]) -> Result<Inventory, Box<dyn std::error::Error>> {
    let templates = battle_tables::member_ability_lists(bar_entry(battle, b"plrp")?, 1)?;
    let lvup = bar_entry(battle, b"lvup")?;
    let levels = (1..=99)
        .map(|l| battle_tables::player_level(lvup, 1, l))
        .collect::<std::io::Result<Vec<_>>>()?;
    let rewards = battle_tables::form_levels(bar_entry(battle, b"fmlv")?)?;
    build(items, &templates, &levels, &rewards)
}

impl Inventory {
    /// Select another learned growth tier while preserving its equipped bit.
    pub fn change_growth(&self, slots: &mut [u16], family: usize, step: i32) -> Option<u16> {
        let tiers = self.growth.get(family)?;
        let slot = slots.iter_mut().find(|s| tiers.contains(&(**s & 0x7fff)))?;
        let at = tiers.iter().position(|&id| id == *slot & 0x7fff)?;
        let next = (at as i32 + step).rem_euclid(tiers.len() as i32) as usize;
        *slot = (*slot & 0x8000) | tiers[next];
        Some(tiers[next])
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn item(id: u16, bit: u16, class: u8, ap: u8) -> Item {
        let mut bytes = vec![0u8; 8 + data::item_table::RECORD_SIZE];
        bytes[4..8].copy_from_slice(&1u32.to_le_bytes());
        bytes[8..10].copy_from_slice(&id.to_le_bytes());
        bytes[10] = 0x13;
        bytes[12..14].copy_from_slice(&bit.to_le_bytes());
        bytes[14] = class;
        bytes[15] = ap;
        data::item_table::read(&bytes).unwrap().remove(0)
    }

    fn level(ids: [u16; 3]) -> battle_tables::PlayerLevel {
        battle_tables::PlayerLevel {
            exp: 7,
            strength: 2,
            magic: 4,
            defense: 6,
            ap: 3,
            abilities: ids,
        }
    }

    #[test]
    fn templates_choices_and_form_rewards_preserve_duplicate_sources() {
        let items = [
            item(1001, 1, 2, 5),
            item(1002, 20, 0, 3),
            item(1003, 21, 0, 2),
        ];
        let templates = vec![vec![1001, 1002, 1003], vec![1003, 1003]];
        let levels = [level([1003, 1003, 1003]), level([1003, 0, 0])];
        let rewards = [battle_tables::FormLevel {
            id: 0x12,
            growth: 1,
            ability: 1003,
            exp: 9,
        }];
        let inv = build(&items, &templates, &levels, &rewards).unwrap();
        assert_eq!(inv.slots.len(), 80);
        assert_eq!(
            &inv.slots[..5],
            &[0x8000 | 1001, 0x8000 | 1002, 1003, 1003, 1003]
        );
        assert_eq!(inv.ap, 14);
        assert!(inv.slots[5..].iter().all(|&s| s == 0));
    }

    #[test]
    fn all_five_growth_families_use_one_slot_and_select_learned_tiers() {
        let mut items = Vec::new();
        let mut list = Vec::new();
        for (f, bits) in abilities::GROWTH.iter().enumerate() {
            for (l, &bit) in bits.iter().take(4).enumerate() {
                let id = 1200 + f as u16 * 4 + l as u16;
                items.push(item(id, bit as u16, 3, 2));
                // Normal Sora's template learns through tier 3, not form MAX.
                if l == 2 {
                    list.push(id);
                }
            }
        }
        let inv = build(&items, &[list], &[], &[]).unwrap();
        assert_eq!(inv.slots.iter().filter(|&&s| s != 0).count(), 5);
        assert!(inv.growth.iter().all(|g| g.len() == 3));
        let mut slots = inv.slots.clone();
        slots[2] |= 0x8000;
        assert_eq!(inv.change_growth(&mut slots, 2, 1), Some(1208));
        assert_eq!(slots[2], 0x8000 | 1208);
        assert_eq!(inv.change_growth(&mut slots, 2, -1), Some(1210));
        assert_eq!(slots[2], 0x8000 | 1210);
        assert_eq!(inv.change_growth(&mut slots, 9, 1), None);
    }

    #[test]
    fn missing_non_ability_rewards_are_skipped_and_capacity_never_truncates() {
        let items = [item(1001, 30, 0, 1)];
        let inv = build(&items, &[vec![1001, 9999]], &[level([9999; 3])], &[]).unwrap();
        assert_eq!(inv.slots.iter().filter(|&&s| s != 0).count(), 1);
        assert!(build(&items, &[vec![1001; 81]], &[], &[]).is_err());
    }
}
