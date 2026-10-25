//! What a kill gives in the test room: EXP and level-ups (FUN_140410af0,
//! FUN_1403ecf30), the prizes scattered on the ground (FUN_1403dc170) and
//! their pickup effects (FUN_1403dca50).
use super::*;
use player::enemy_prizes::{self, Orb};

/// The owner the prize heal effect (system element 6) runs under, so its
/// release ends only it.
pub const HEAL_OWNER: u8 = 0xf7;

/// One line of the unattended log.
pub fn log_event(log: &mut Option<std::fs::File>, record: serde_json::Value) {
    if let Some(file) = log.as_mut() {
        let _ = serde_json::to_writer(&mut *file, &record);
        let _ = std::io::Write::write_all(file, b"\n");
    }
}

/// The unattended `--prize-tour`: one prize of every kind lands in a ring
/// round Sora (tick 20), the HP, MP and Drive ones are then moved onto him
/// (tick 140) to show the pickup and the heal effect.
pub const CAPTURES: [u64; 8] = [70, 110, 150, 160, 175, 190, 220, 260];
pub const END: u64 = 300;

/// The `--badge-tour`: a box lands on Sora at tick 20 (the item is picked
/// up at once), START opens the camp at tick 100 (the Items row shows
/// NEW!), DECIDE enters Items at 180 (the Stock row's badge).
pub fn badge_tour(
    inventory: &super::ability_inventory::Inventory,
) -> super::ability_inventory::Tour {
    super::ability_inventory::Tour {
        inputs: vec![(100, 2), (180, 4)],
        captures: vec![150, 230],
        close: 10_000,
        end: 260,
        expected_slots: inventory.slots.clone(),
        attacks: Vec::new(),
    }
}

pub fn tour(game: &mut Game) {
    let tick = game.tick;
    let Game {
        player,
        enemy,
        world,
        ..
    } = game;
    let Some(rt) = enemy.as_mut() else { return };
    let at = player.position;
    if game.badge_tour {
        if tick == 20 {
            let id = rt.next_drop;
            rt.next_drop += 1;
            if let Some(s) = enemy_prizes::box_style(0) {
                rt.drops.push(enemy_prizes::ItemDrop::spawn(
                    id,
                    280,
                    s,
                    [at[0], at[1] + 5., at[2]],
                    0.5,
                ));
            }
        }
        return;
    }
    if tick == 20 {
        for kind in 0..9u8 {
            let angle = (kind as f32 - 4.) * 0.35;
            let (x, z) = (at[0] + 260. * angle.sin(), at[2] + 260. * angle.cos());
            let floor = world.floor(x, z, at[1] + 30.).unwrap_or(at[1]);
            rt.orbs
                .push(Orb::spawn(kind, [x, floor + 20., z], [0.5, 0.5, 0.5]));
        }
    }
    if tick == 20 {
        // One item box of each colour, three on the ground in front.
        for (i, style) in [0u8, 4, 8, 11].into_iter().enumerate() {
            let (x, z) = (at[0] - 120. + 80. * i as f32, at[2] + 330.);
            let floor = world.floor(x, z, at[1] + 30.).unwrap_or(at[1]);
            let id = rt.next_drop;
            rt.next_drop += 1;
            if let Some(s) = enemy_prizes::box_style(style) {
                rt.drops.push(enemy_prizes::ItemDrop::spawn(
                    id,
                    280,
                    s,
                    [x, floor + 20., z],
                    0.3 * i as f32,
                ));
            }
        }
    }
    if tick == 140 {
        for orb in rt
            .orbs
            .iter_mut()
            .filter(|o| enemy_prizes::heal_ticks(o.kind) > 0.)
        {
            orb.position = [at[0] + 40., at[1], at[2]];
            orb.landed = true;
        }
    }
}

pub fn tick(game: &mut Game, tick: u64) {
    let Game {
        player,
        enemy,
        world,
        log,
        autoplay,
        drive,
        sound_queue,
        fx_queue,
        fx_release,
        items,
        ..
    } = game;
    let system = drive.as_ref().and_then(|d| d.src.system_pax.clone());
    // The Drive form Sora is in (0 without one).
    let form = drive.as_ref().filter(|d| d.active).map_or(0, |d| d.form);
    let Some(rt) = enemy.as_mut() else { return };
    if autoplay.is_some() && tick == 1 {
        // The unattended run starts two EXP short of level 2.
        rt.exp_total = 38;
    }
    let dead = rt.enemy.current_act() == Some(shadow::ActKind::Dead);
    if dead && !rt.was_dead {
        let styles = items.as_ref().map(|i| &i.src.box_styles);
        kill(player, rt, world, styles, log, tick);
        // FUN_1403e8780: Wisdom counts Heartless, Final Nobodies.
        if let Some(d) = drive.as_mut() {
            d.pending_exp += player::forms::kill_exp(form, d.enemy_character);
        }
    }
    rt.was_dead = dead;
    // The item boxes fall, rest and are picked up (FUN_1403f7df0 →
    // FUN_1403f8770 adds the item).
    let body = player.position;
    let mut taken = Vec::new();
    for (i, drop) in rt.drops.iter_mut().enumerate() {
        let floor = world
            .floor(drop.position[0], drop.position[2], drop.position[1] + 30.)
            .unwrap_or(0.);
        drop.tick(floor);
        if !player.dead() && drop.collected_by(body) {
            taken.push(i);
        }
    }
    for i in taken.into_iter().rev() {
        let drop = rt.drops.remove(i);
        if let Some(i) = items.as_mut() {
            let n = i.stock.entry(drop.item).or_insert(0);
            *n = n.saturating_add(1).min(99);
        }
        log_event(
            log,
            serde_json::json!({"event":"item_pickup","tick":tick,"item":drop.item}),
        );
    }
    // The prizes fall and wait to be collected.
    let at = player.position;
    let draw = player.abilities.draw;
    let mut collected = Vec::new();
    for (i, orb) in rt.orbs.iter_mut().enumerate() {
        let floor = world
            .floor(orb.position[0], orb.position[2], orb.position[1] + 30.)
            .unwrap_or(0.);
        if let Some(id) = orb.tick(floor) {
            sound_queue.push((id, "prize_bounce"));
        }
        // A dead Sora collects nothing (FUN_1403dc500: 0x9b8 bit 2).
        if !player.dead() && orb.collected_by(at, draw) {
            collected.push(i);
        }
    }
    // FUN_1403d4cc0: the heal effect's time runs down every frame; at zero
    // its handle is released (FUN_1402be8f0, the looping element drains)
    // and forgotten.
    if rt.heal_ticks > 0. {
        rt.heal_ticks -= 1.;
        if rt.heal_ticks <= 0. {
            rt.heal_ticks = 0.;
            fx_release.push(HEAL_OWNER);
        }
    }
    for i in collected.into_iter().rev() {
        let orb = rt.orbs.remove(i);
        let e = enemy_prizes::effect(orb.kind);
        // FUN_1403dca50 → FUN_1403d7bd0: HP, MP and Drive prizes start the
        // heal effect (system element 6) unless it still plays, which only
        // extends its time.
        let heal = enemy_prizes::heal_ticks(orb.kind);
        if heal > 0. {
            if rt.heal_ticks <= 0.
                && let Some(pax) = system.clone()
            {
                fx_queue.push(super::drive_fx::Placed {
                    req: super::drive_fx::FxRequest {
                        pax,
                        id: 6,
                        owner: HEAL_OWNER,
                    },
                    anchor: super::drive_fx::Anchor::Sora,
                });
            }
            rt.heal_ticks = rt.heal_ticks.max(heal);
        }
        // FUN_140413650: the descriptor's pickup sound.
        sound_queue.push((enemy_prizes::sounds(orb.kind).1, "prize_pickup"));
        // Antiform's pickup skips HP orbs (formcmd-01 §5).
        if e.hp > 0 && form != player::forms::ANTI {
            rt.sora_hp = (rt.sora_hp + e.hp).min(rt.sora_target.max_hp);
        }
        if e.mp > 0 {
            player.mp.add(e.mp);
        }
        if e.drive > 0 {
            let (charging, boost) = (player.mp_charging, player.abilities.drive_boost);
            player.drive.gain(e.drive as f32, charging, boost, 1.);
            // FUN_1403e8760: Master Form EXP is the orb's drive value.
            if form == player::forms::MASTER
                && let Some(d) = drive.as_mut()
            {
                d.pending_exp += e.drive as i32;
            }
        }
        rt.munny += e.munny as u32;
        log_event(
            log,
            serde_json::json!({"event":"prize","tick":tick,"kind":orb.kind,
                "hp":rt.sora_hp,"mp":player.mp.cur,"drive":player.drive.level,
                "drive_partial":player.drive.partial,"munny":rt.munny}),
        );
    }
}

/// The kill (FUN_140410af0): EXP, level-ups, then the prize scatter.
fn kill(
    player: &mut Player,
    rt: &mut EnemyRuntime,
    world: &RoomWorld,
    styles: Option<&std::collections::HashMap<u16, u8>>,
    log: &mut Option<std::fs::File>,
    tick: u64,
) {
    let (enemy_exp, level_exp) = rt.exp_rule;
    let critical = rt.difficulty == combat::Difficulty::Critical;
    let low = rt.sora_hp * 2 <= rt.sora_target.max_hp;
    let boost = low.then_some(player.abilities.experience_boost as u32);
    let exp = enemy_prizes::kill_exp(enemy_exp, level_exp, critical, boost);
    // FUN_1403ed110: No Experience (ability bit 116) takes the kill's EXP.
    if !player.abilities.has(abilities::bit::NO_EXPERIENCE) {
        grant_exp(rt, exp, log, tick);
    }
    // FUN_1403dc170: counts scaled by Jackpot, munny converted by Drive
    // Converter, three item rolls with Lucky Lucky.
    let ab = &player.abilities;
    let jackpot = 1. + ab.jackpot;
    let converter = ab.drive_converter;
    let at = rt.enemy.position;
    let floor = world
        .floor(at[0], at[2], at[1] + 30.)
        .unwrap_or(at[1].max(0.));
    for (kind, n) in enemy_prizes::drops(&rt.prize.counts, jackpot, converter) {
        for _ in 0..n {
            let r = [
                rt.rng.range(0., 1.),
                rt.rng.range(0., 1.),
                rt.rng.range(0., 1.),
            ];
            rt.orbs
                .push(Orb::spawn(kind, [at[0], floor.max(at[1]) + 20., at[2]], r));
        }
    }
    let luck = 1. + ab.lucky_lucky;
    for (item, chance) in rt.prize.items {
        if item != 0 && rt.rng.range(0., 100.) < chance as f32 * luck {
            log_event(
                log,
                serde_json::json!({"event":"item_drop","tick":tick,"item":item}),
            );
            // FUN_1403f8170: the item's treasure box, at the kill.
            if let Some(style) = styles
                .and_then(|s| s.get(&item))
                .and_then(|&s| enemy_prizes::box_style(s))
            {
                let id = rt.next_drop;
                rt.next_drop += 1;
                let r = rt.rng.range(0., 1.);
                rt.drops.push(enemy_prizes::ItemDrop::spawn(
                    id,
                    item,
                    style,
                    [at[0], floor.max(at[1]) + 20., at[2]],
                    r,
                ));
            }
        }
    }
}

/// EXP into Sora's total, then each level gained (FUN_140410af0's level
/// loop). The debug menu's level-up uses it too.
pub fn grant_exp(rt: &mut EnemyRuntime, exp: u32, log: &mut Option<std::fs::File>, tick: u64) {
    if exp == 0 {
        return;
    }
    rt.exp_total = (rt.exp_total + exp).min(9_999_999);
    let before = rt.level;
    let levels = &rt.sora_levels;
    let after = enemy_prizes::level_for(rt.exp_total, before, |l| {
        levels.get(l as usize - 1).map(|r| r.exp)
    });
    for l in before + 1..=after {
        // Each level's row sets Strength, Magic, Defence and AP; the
        // room's equipment bonuses stay on top.
        let (old, new) = (levels[l as usize - 2], levels[l as usize - 1]);
        // The level's panel (FUN_1403ed7d0): the raised stats, then the
        // growth ability of the first path [I: no save picks one].
        let critical = rt.difficulty == combat::Difficulty::Critical;
        rt.panels.push(super::level_growth::Panel {
            kind: visual::level_up::Kind::Level,
            character: 0,
            level: l,
            lines: super::level_growth::level_lines(&old, &new, critical, 0),
        });
        rt.level_effect = true;
        rt.sora.strength += new.strength as i32 - old.strength as i32;
        rt.sora.magic += new.magic as i32 - old.magic as i32;
        rt.sora_target.defense += new.defense as i32 - old.defense as i32;
        // [I] the first growth choice's ability.
        let learned = new.abilities[0];
        if learned != 0 {
            rt.learned.push(learned);
        }
        log_event(
            log,
            serde_json::json!({"event":"level_up","tick":tick,"level":l,
                "strength":rt.sora.strength,"magic":rt.sora.magic,
                "defense":rt.sora_target.defense,"learned":learned}),
        );
    }
    if after != before {
        rt.level = after;
        rt.numbers_dirty = true;
    }
    log_event(
        log,
        serde_json::json!({"event":"exp","tick":tick,"gain":exp,"total":rt.exp_total,
            "level":rt.level}),
    );
}
