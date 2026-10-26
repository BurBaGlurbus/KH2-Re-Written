//! The duel's baseline: for the length of a duel both Soras start from the
//! same build, made from the installed tables rather than a save. Level 99
//! (the LVUP row's Strength, Magic and Defence), every bonus level's HP,
//! MP, Drive bar and ability, every Drive form at its level cap with what
//! its levels teach, the top level of every spell, the room's Keyblade's
//! stats and ability on top, a full Drive gauge, and a stocked item set.
//! Nothing is read from or written to save data; the room keeps what it had
//! and puts it back when the duel ends.

use super::*;
use std::collections::BTreeMap;

/// The level both players fight at.
pub(crate) const DUEL_LEVEL: u8 = 99;
/// Consumables in stock for a duel, and the order they fill Sora's item
/// slots: Potion, Hi-Potion, Mega-Potion, Ether, Mega-Ether, Elixir,
/// Megalixir [duel choice: the healing items 1..=7 of the item table].
const DUEL_ITEMS: [u16; 7] = [1, 2, 5, 3, 6, 4, 7];
const DUEL_STOCK: u8 = 99;
/// Item slots before any bonus level adds one [duel choice: the room's
/// two], and the most Sora holds.
const BASE_ITEM_SLOTS: usize = 2;
const MAX_ITEM_SLOTS: usize = 8;

/// What the duel changed, to be put back when it ends.
pub(crate) struct Saved {
    player: Player,
    vitals: Vitals,
    ability_slots: Vec<u16>,
    exp_total: u32,
    bonus_taken: Vec<u8>,
    difficulty: combat::Difficulty,
    forms: Option<player::forms::Forms>,
    items: Option<ItemState>,
}

/// Sora's item slots, their auto-reload items and the stock.
type ItemState = (Vec<u16>, Vec<u16>, BTreeMap<u16, u8>);

/// The duel's loadout (match-scoped) and the room's own state it replaced.
#[derive(Resource, Default)]
pub(crate) struct DuelLoadout {
    saved: Option<Saved>,
}

/// Builds the baseline on the room Sora (the rivals copy him when they
/// join). Returns what the event log records, or why it could not.
pub(crate) fn apply(
    game: &mut Game,
    loadout: &mut DuelLoadout,
) -> Result<serde_json::Value, String> {
    let tick = game.tick;
    let Some(rt) = game.enemy.as_mut() else {
        return Err("the duel needs the room's battle tables (a room `enemy`)".into());
    };
    if loadout.saved.is_none() {
        loadout.saved = Some(Saved {
            player: game.sora.player.clone(),
            vitals: game.sora.vitals.clone(),
            ability_slots: game.ability_slots.clone(),
            exp_total: rt.exp_total,
            bonus_taken: rt.bonus_taken.clone(),
            difficulty: rt.difficulty,
            forms: game.drive.as_ref().map(|d| d.forms.clone()),
            items: game
                .items
                .as_ref()
                .map(|i| (i.slots.clone(), i.originals.clone(), i.stock.clone())),
        });
    }
    // [Duel rule] Standard difficulty: no bonus halving, damage x1.
    rt.difficulty = combat::Difficulty::Standard;
    // Level 99: the LVUP row's numbers over the level-1 row's (FUN_140410af0
    // adds the differences the same way).
    let levels = rt.sora_levels.clone();
    let (Some(first), Some(top)) = (
        levels.first().copied(),
        levels.get(DUEL_LEVEL as usize - 1).copied(),
    ) else {
        return Err(format!("LVUP has {} rows, not {DUEL_LEVEL}", levels.len()));
    };
    let current = levels
        .get(game.sora.vitals.level.max(1) as usize - 1)
        .copied()
        .unwrap_or(first);
    let v = &mut game.sora.vitals;
    v.attacker.strength += i32::from(top.strength) - i32::from(current.strength);
    v.attacker.magic += i32::from(top.magic) - i32::from(current.magic);
    v.target.defense += i32::from(top.defense) - i32::from(current.defense);
    v.level = DUEL_LEVEL;
    rt.exp_total = top.exp;
    // Every bonus level Sora has a row in (FUN_1403edca0): HP, MP, the
    // Drive bars and the abilities they teach.
    let mut bonuses = 0;
    let mut item_slots = 0usize;
    if let Some(bons) = game.drive.as_ref().map(|d| d.src.growth.bons.clone()) {
        for id in 1..=u8::MAX {
            let Ok(rows) = data::battle_tables::bonus_rows(&bons, id) else {
                continue;
            };
            if !rows.iter().any(|r| r.character == 1) {
                continue;
            }
            item_slots += rows
                .iter()
                .filter(|r| r.character == 1)
                .map(|r| r.item_slots as usize)
                .sum::<usize>();
            let Game { enemy, sora, .. } = &mut *game;
            if let Some(rt) = enemy.as_mut()
                && level_growth::apply_bonus(rt, sora, &bons, id).unwrap_or(false)
            {
                bonuses += 1;
            }
        }
    }
    // The bonus panels and level-up effect are the camp's and the HUD's; a
    // duel starts without them. Taught abilities go into Sora's slots.
    let learned = game
        .enemy
        .as_mut()
        .map(|rt| {
            rt.panels.clear();
            rt.level_effect = false;
            rt.numbers_dirty = false;
            std::mem::take(&mut rt.learned)
        })
        .unwrap_or_default();
    for a in learned {
        learn(&mut game.ability_slots, a);
    }
    // Every Drive form to its level cap (FUN_1403e8380 with the form
    // owned), with what each level teaches Sora.
    let mut form_levels = Vec::new();
    if let Some(d) = game.drive.as_mut() {
        let src = Arc::clone(&d.src);
        for f in 1..=5u8 {
            if !d.forms.owned.get(f as usize).copied().unwrap_or(false) {
                continue;
            }
            let ups = d
                .forms
                .add_exp(f, i32::MAX / 4, &src.fmlv, &mut game.ability_slots, false);
            for up in &ups {
                if let Some(a) = up.ability {
                    learn(&mut game.ability_slots, a);
                }
            }
            form_levels.push(d.forms.record(f).map_or(0, |r| r.level));
        }
        d.level_ups.clear();
    }
    drive_room::rebuild_abilities(game, None);
    // Full MP and a full Drive gauge at the start.
    let p = &mut game.sora.player;
    p.mp = magic::Mp::new(p.mp.max);
    p.mp_charging = false;
    p.drive.refill();
    let v = &mut game.sora.vitals;
    v.hp = v.target.max_hp;
    v.ko_ticks = 0;
    // Items: a stock of every healing item and the slots filled in order.
    if let Some(it) = game.items.as_mut() {
        let slots = (BASE_ITEM_SLOTS + item_slots).min(MAX_ITEM_SLOTS);
        for id in DUEL_ITEMS {
            it.stock.insert(id, DUEL_STOCK);
        }
        it.slots = DUEL_ITEMS.iter().copied().cycle().take(slots).collect();
        it.originals = it.slots.clone();
    }
    let v = &game.sora.vitals;
    Ok(
        serde_json::json!({"event":"duel_loadout","tick":tick,"level":v.level,
        "strength":v.attacker.strength,"magic":v.attacker.magic,"defense":v.target.defense,
        "max_hp":v.target.max_hp,"max_mp":game.sora.player.mp.max,
        "drive_max":game.sora.player.drive.max,"bonuses":bonuses,"form_levels":form_levels,
        "item_slots":game.items.as_ref().map(|i| i.slots.len())}),
    )
}

/// Puts an ability item in Sora's first empty slot unless he has it.
fn learn(slots: &mut [u16], ability: u16) {
    if ability == 0 || slots.iter().any(|&s| s & 0x7fff == ability) {
        return;
    }
    if let Some(s) = slots.iter_mut().find(|s| **s == 0) {
        *s = ability;
    }
}

/// The duel ended on this instance: the room's own Sora comes back.
pub(crate) fn restore(game: &mut Game, loadout: &mut DuelLoadout) {
    let Some(saved) = loadout.saved.take() else {
        return;
    };
    // He keeps where he stands and what he is doing.
    let (position, yaw, action, frame) = {
        let p = &game.sora.player;
        (p.position, p.yaw, p.action, p.frame)
    };
    game.sora.player = saved.player;
    let p = &mut game.sora.player;
    (p.position, p.yaw, p.action, p.frame) = (position, yaw, action, frame);
    game.sora.vitals = saved.vitals;
    game.ability_slots = saved.ability_slots;
    if let Some(rt) = game.enemy.as_mut() {
        rt.exp_total = saved.exp_total;
        rt.bonus_taken = saved.bonus_taken;
        rt.difficulty = saved.difficulty;
    }
    if let (Some(d), Some(forms)) = (game.drive.as_mut(), saved.forms) {
        d.forms = forms;
    }
    if let (Some(it), Some((slots, originals, stock))) = (game.items.as_mut(), saved.items) {
        (it.slots, it.originals, it.stock) = (slots, originals, stock);
    }
    drive_room::rebuild_abilities(game, None);
    let tick = game.tick;
    game.event(serde_json::json!({"event":"duel_loadout_restored","tick":tick}));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn learning_fills_the_first_empty_slot_once() {
        let mut slots = vec![0x8001, 0, 5, 0];
        learn(&mut slots, 7);
        assert_eq!(slots, vec![0x8001, 7, 5, 0]);
        learn(&mut slots, 7);
        learn(&mut slots, 1);
        learn(&mut slots, 0);
        assert_eq!(slots, vec![0x8001, 7, 5, 0]);
    }
}
