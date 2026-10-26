//! Sora's battle items in the test room: the item slots, the Items
//! submenu rows, the use command (motion 9, `Player::use_item`) and the
//! heal that lands 30 ticks after the use trigger (FUN_1403f6790,
//! FUN_1403f6960, FUN_1403f6d00, FUN_1403c4b80).
use super::*;
use std::collections::BTreeMap;

/// The wait between the use trigger and the heal (`DAT_140623bcc`).
const HEAL_DELAY: f32 = 30.;
/// Effect keys of the item elements Sora's party items start on the
/// members.
const ITEM_FX: u64 = 1 << 55;

/// One consumable's 03system record fields.
#[derive(Clone, Copy, Debug)]
pub struct ItemInfo {
    /// +2: the category (0 a consumable, 1 a boost, Tent or Drive
    /// Recovery).
    pub kind: u8,
    /// +4: the amount, in percent of max HP / MP.
    pub amount: u16,
    /// +0x10: the command that uses it.
    pub command: u16,
    /// +6: the system effect element it starts on the target (1..4).
    pub effect: u16,
}

#[derive(Clone)]
pub struct ItemSource {
    pub items: HashMap<u16, ItemInfo>,
    /// Every item's treasure box style (record byte +0x16).
    pub box_styles: HashMap<u16, u8>,
}

/// 03system `item`: the consumables (type 0 and 1) that name a command.
pub fn load(system: &[u8]) -> Result<ItemSource, Box<dyn std::error::Error>> {
    let table = data::item_table::read(bar_entry(system, b"item")?)?;
    let items = table
        .iter()
        .filter(|r| r.kind <= 1 && r.command != 0)
        .map(|r| {
            (
                r.id,
                ItemInfo {
                    kind: r.kind,
                    amount: r.amount,
                    command: r.command,
                    effect: r.weapon_stats,
                },
            )
        })
        .collect();
    let box_styles = table.iter().map(|r| (r.id, r.box_style)).collect();
    Ok(ItemSource { items, box_styles })
}

pub struct ItemRt {
    pub src: ItemSource,
    /// Sora's item slots (character record +0x34) and their auto-reload
    /// items (+0x44).
    pub slots: Vec<u16>,
    pub originals: Vec<u16>,
    /// The stock counts the camp shows and auto-reload draws on.
    pub stock: BTreeMap<u16, u8>,
    /// The slot being used and the heals waiting out their delay.
    using: Option<usize>,
    pending: Vec<(f32, u16)>,
    /// The open Items submenu's rows: the slot each row stands for.
    pub menu_slots: Vec<usize>,
}

impl ItemRt {
    pub fn new(
        src: ItemSource,
        slots: Vec<u16>,
        originals: Vec<u16>,
        stock: BTreeMap<u16, u8>,
    ) -> Self {
        ItemRt {
            src,
            slots,
            originals,
            stock,
            using: None,
            pending: Vec::new(),
            menu_slots: Vec::new(),
        }
    }

    /// The command id of the item in a slot.
    pub fn command_of(&self, slot: usize) -> Option<u16> {
        let id = *self.slots.get(slot)?;
        self.src.items.get(&id).map(|i| i.command)
    }

    /// The slot holding the item a command uses (shortcuts).
    pub fn slot_for_command(&self, command: u16) -> Option<usize> {
        (0..self.slots.len()).find(|&j| self.command_of(j) == Some(command))
    }

    /// The Items submenu rows: every filled slot, with the menu colour
    /// (5 in the air, FUN_1403b6cf0).
    pub fn menu_list(&self, airborne: bool) -> Vec<(usize, usize)> {
        (0..self.slots.len())
            .filter(|&j| self.slots[j] != 0)
            .map(|j| (j, if airborne { 5 } else { 0 }))
            .collect()
    }

    /// FUN_1403a1610: a used slot refills from the stock when the battle
    /// state drops.
    pub fn reload(&mut self) {
        for j in 0..self.slots.len() {
            let auto = self.originals.get(j).copied().unwrap_or(0);
            if self.slots[j] == 0
                && auto != 0
                && let Some(n) = self.stock.get_mut(&auto)
                && *n > 0
            {
                *n -= 1;
                self.slots[j] = auto;
            }
        }
    }
}

/// The use command for a slot (FUN_1403d6fc0 execute 3): plays motion 9
/// when Sora can take the command.
pub fn request(game: &mut Game, slot: usize, tick: u64) {
    let Game {
        sora: Fighter { player, .. },
        clips,
        items,
        action_started,
        ..
    } = game;
    let Some(rt) = items.as_mut() else { return };
    if rt.slots.get(slot).copied().unwrap_or(0) == 0 {
        return;
    }
    // A refused state gate is silent: the "cannot" sound (se000 #5) plays
    // only on deciding a dimmed row, which the command menu already does.
    if player.use_item(clips).is_some() {
        rt.using = Some(slot);
        *action_started = tick;
    }
}

/// One tick: the use trigger consumes the slot and queues the heal; due
/// heals land.
pub fn tick(game: &mut Game, tick: u64) {
    let Game {
        sora: Fighter { player, vitals, .. },
        items,
        enemy,
        log,
        drive,
        fx_queue,
        fx_calls,
        party,
        ..
    } = game;
    let system = drive.as_ref().and_then(|d| d.src.system_pax.clone());
    let Some(rt) = items.as_mut() else { return };
    if player.take_item()
        && let Some(slot) = rt.using.take()
        && let Some(id) = rt.slots.get_mut(slot).filter(|s| **s != 0)
    {
        // FUN_1403f6790: the slot empties at the trigger.
        rt.pending.push((HEAL_DELAY, *id));
        *id = 0;
        // FUN_1403f6b30's first update: system element 0 (se000 #100) at
        // the user.
        if let Some(pax) = system.clone() {
            fx_queue.push(drive_fx::Placed {
                req: drive_fx::FxRequest {
                    pax,
                    id: 0,
                    owner: 0xff,
                },
                anchor: drive_fx::Anchor::Sora,
            });
        }
    }
    // FUN_1403f6960: the heal waits out its delay whatever Sora does.
    let mut due = Vec::new();
    rt.pending.retain_mut(|(t, id)| {
        *t -= 1.;
        if *t <= 0. {
            due.push(*id);
            false
        } else {
            true
        }
    });
    for id in due {
        let Some(info) = rt.src.items.get(&id).copied() else {
            continue;
        };
        // Thirty updates later the item's own element (101..104) starts
        // on the target, bound to bone 0x4009.
        if let Some(pax) = system.clone() {
            fx_queue.push(drive_fx::Placed {
                req: drive_fx::FxRequest {
                    pax,
                    id: info.effect,
                    owner: 0xff,
                },
                anchor: drive_fx::Anchor::Sora,
            });
        }
        if enemy.is_none() {
            continue;
        }
        let e = &mut *vitals;
        // FUN_1403c4b80: percent of the maxima, Item Boost adding to 100.
        let pct = 100 + player.abilities.item_boost as i32;
        let amount = info.amount as i32;
        let share = |max: i32| (max * amount * pct + 9999) / 10000;
        let hp = share(e.target.max_hp);
        let mp = share(player.mp.max);
        let (heal_hp, heal_mp) = match id {
            1 | 2 | 5 => (hp, 0),
            3 | 6 => (0, mp),
            4 | 7 => (hp, mp),
            0x83 => (e.target.max_hp, mp),
            _ => (0, 0),
        };
        if heal_hp > 0 {
            e.hp = (e.hp + heal_hp).min(e.target.max_hp);
        }
        if heal_mp > 0 {
            player.mp.add(heal_mp);
        }
        let mut records = vec![
            serde_json::json!({"event":"item_heal","tick":tick,"item":id,
            "hp":heal_hp,"mp":heal_mp,"sora_hp":e.hp,"mp_now":player.mp.cur}),
        ];
        // FUN_1403c4ab0: 5..7 and 0x83 heal every member of the party too
        // (no range), each by its own maxima with the user's Item Boost,
        // and start the item's element on each.
        if matches!(id, 5..=7 | 0x83) {
            for (k, m) in party.iter_mut().enumerate() {
                let (hp, mp) = (share(m.max_hp), share(m.mp.max));
                let (heal_hp, heal_mp) = match id {
                    5 => (hp, 0),
                    6 => (0, mp),
                    7 => (hp, mp),
                    _ => (m.max_hp, mp),
                };
                let was_down = super::party_room::heal_member(m, heal_hp);
                if heal_mp > 0 {
                    m.mp.add(heal_mp);
                }
                records.push(
                    serde_json::json!({"event":"item_heal","tick":tick,"item":id,
                    "member":m.kind.name,"hp":heal_hp,"mp":heal_mp,"member_hp":m.hp,
                    "mp_now":m.mp.cur,"was_down":was_down}),
                );
                fx_calls.push(super::spell_fx::Call {
                    pax: system.clone(),
                    fx: player::magic::Fx::Start {
                        key: ITEM_FX | (k as u64 + 1) << 32 | tick,
                        id: info.effect,
                        bound: Some(player::magic::Anchor::Party(k + 1)),
                        at: None,
                    },
                });
            }
        }
        for record in records {
            if let Some(file) = log.as_mut() {
                let _ = serde_json::to_writer(&mut *file, &record);
                let _ = std::io::Write::write_all(file, b"\n");
            }
        }
    }
    // The battle state drops once the enemy cannot be hit.
    if enemy.as_ref().is_some_and(|e| !e.enemy.targetable()) {
        rt.reload();
    }
}
