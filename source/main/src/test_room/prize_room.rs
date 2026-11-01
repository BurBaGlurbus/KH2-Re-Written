//! What a kill gives in the test room: EXP and level-ups (FUN_140410af0,
//! FUN_1403ecf30), the prizes scattered on the ground (FUN_1403dc170) and
//! their pickup effects (FUN_1403dca50).
use super::*;
use player::enemy_prizes::{self, Orb};

/// The owner the prize heal effect (system element 6) runs under, so its
/// release ends only it.
pub const HEAL_OWNER: u8 = 0xf7;
/// [I] A dropped box against the map's walls.
const BOX_RADIUS: f32 = 30.;

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
        sora: Fighter { player, .. },
        enemy,
        world,
        party,
        log,
        ..
    } = game;
    let Some(rt) = enemy.as_mut() else { return };
    let at = player.position;
    // An HP prize at Donald's feet: he collects it and wears the effect.
    if tick == 200
        && let Some(m) = party.first()
    {
        let mut orb = Orb::spawn(0, m.follower.position, [0.5, 0.5, 0.5]);
        orb.landed = true;
        rt.orbs.push(orb);
    }
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
                    [0.5; 4],
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
                    [0.3 * i as f32, 0.5, 0.5, 0.5],
                ));
            }
        }
    }
    // A box thrown at the nearest wall, which stops it.
    if tick == 30 {
        let from = [at[0], at[1] + 40., at[2]];
        let wall = (0..16)
            .filter_map(|k| {
                let a = k as f32 * std::f32::consts::TAU / 16.;
                let (s, c) = a.sin_cos();
                let to = [from[0] + 1500. * s, from[1], from[2] + 1500. * c];
                let hit = world.line_hit(from, to)?;
                Some((Vec3::from(hit).distance(Vec3::from(from)), s, c, hit))
            })
            .min_by(|a, b| a.0.total_cmp(&b.0));
        if let (Some((_, s, c, hit)), Some(style)) = (wall, enemy_prizes::box_style(0)) {
            let id = rt.next_drop;
            rt.next_drop += 1;
            let at = [hit[0] - 120. * s, hit[1] + 40., hit[2] - 120. * c];
            let mut d = enemy_prizes::ItemDrop::spawn(id, 280, style, at, [0.5; 4]);
            d.velocity = [6. * s, 15., 6. * c];
            log_event(
                log,
                serde_json::json!({"event":"box_thrown","tick":tick,"id":id,"wall":hit,"from":at}),
            );
            rt.drops.push(d);
        }
    }
    if tick == 135 {
        let boxes: Vec<_> = rt
            .drops
            .iter()
            .map(|d| (d.id, d.rested, d.position))
            .collect();
        log_event(
            log,
            serde_json::json!({"event":"boxes","tick":tick,"boxes":boxes}),
        );
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
        // The first box beside him, to show its flight and name window.
        if let Some(d) = rt.drops.first_mut() {
            d.position = [at[0] + 90., at[1], at[2]];
            d.collectable = true;
            d.rested = true;
        }
    }
}

pub fn tick(game: &mut Game, tick: u64) {
    let Game {
        sora: Fighter { player, vitals, .. },
        enemy,
        world,
        log,
        autoplay,
        drive,
        sound_queue,
        fx_queue,
        fx_release,
        items,
        save,
        magic,
        treasure_items,
        party,
        fx_calls,
        defeated_keys,
        ..
    } = game;
    let body_lift = magic.as_ref().map_or(85., |m| m.src.body_lift);
    let system = drive.as_ref().and_then(|d| d.src.system_pax.clone());
    // The Drive form Sora is in (0 without one).
    let form = drive.as_ref().filter(|d| d.active).map_or(0, |d| d.form);
    let Some(rt) = enemy.as_mut() else { return };
    if autoplay.is_some() && tick == 1 && !save.loaded {
        // The unattended run starts two EXP short of level 2.
        rt.exp_total = 38;
    }
    let dead = rt.enemy.current_act() == Some(shadow::ActKind::Dead);
    if dead && !rt.was_dead {
        let styles = items.as_ref().map(|i| &i.src.box_styles);
        kill(player, vitals, rt, world, styles, log, tick);
        // FUN_1403e8780: Wisdom counts Heartless, Final Nobodies.
        if let Some(d) = drive.as_mut() {
            d.pending_exp += player::forms::kill_exp(form, d.enemy_character);
            defeated_keys.push(d.enemy_character);
        }
    }
    rt.was_dead = dead;
    // The item boxes fall, bounce and rest; Sora in range takes one
    // (FUN_1403f7df0 → FUN_1403f8770 adds the item), it flies to his middle
    // and its name shows as it arrives (FUN_1403f8590 → FUN_1401595d0). A full
    // stock adds nothing and the box still flies.
    let body = player.position;
    let anchor = [body[0], body[1] + body_lift, body[2]];
    rt.drops.retain_mut(|drop| {
        if drop.flight.is_some() {
            if !player.dead() && drop.fly(anchor) {
                return true;
            }
            treasure_items.push(drop.item);
            return false;
        }
        let floor = world
            .floor(drop.position[0], drop.position[2], drop.position[1] + 30.)
            .unwrap_or(0.);
        let before = drop.position;
        if let Some(id) = drop.tick(floor) {
            sound_queue.push((id, "box_bounce"));
        }
        // [I] The map's walls stop it, as its object's map body does; the
        // radius is the box's own size.
        let (dx, dz) = (drop.position[0] - before[0], drop.position[2] - before[2]);
        let moved = dx.hypot(dz);
        if moved > 1e-3 {
            let (s, c) = (dx / moved, dz / moved);
            let y = before[1] + BOX_RADIUS;
            let from = [before[0], y, before[2]];
            let to = [
                drop.position[0] + s * BOX_RADIUS,
                y,
                drop.position[2] + c * BOX_RADIUS,
            ];
            if let Some(hit) = world.line_hit(from, to) {
                drop.position[0] = hit[0] - s * BOX_RADIUS;
                drop.position[2] = hit[2] - c * BOX_RADIUS;
                drop.velocity[0] = 0.;
                drop.velocity[2] = 0.;
                log_event(
                    log,
                    serde_json::json!({"event":"box_wall","tick":tick,"id":drop.id,"at":hit}),
                );
            }
        }
        if !player.dead() && drop.collected_by(body) {
            if let Some(i) = items.as_mut() {
                i.give(drop.item);
            }
            game_save::note_obtained(&mut save.base, &save.items, drop.item);
            drop.take_off(anchor);
            sound_queue.push((enemy_prizes::BOX_FLY_SOUND, "box_fly"));
            log_event(
                log,
                serde_json::json!({"event":"item_pickup","tick":tick,"item":drop.item,
                    "slots":items.as_ref().map(|i| i.slots.clone()),
                    "stock":items.as_ref().and_then(|i| i.stock.get(&drop.item).copied())}),
            );
        }
        true
    });
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
        // A dead Sora collects nothing (FUN_1403dc500: 0x9b8 bit 2), and
        // the leader in Antiform passes over HP prizes, kinds 0 and 1
        // (FUN_1403dbea0: +0xde0 == 6 skips kinds below 2); they stay.
        let refused = form == player::forms::ANTI && orb.kind < 2;
        if !player.dead() && !refused && orb.collected_by(at, draw) {
            collected.push((i, None));
            continue;
        }
        // FUN_1403dbea0: then Donald and Goofy, by the same range from where
        // each stands; the first in range collects, and they pass over Drive
        // prizes. The prize acts on the collector only.
        if orb.kind < 7
            && let Some(k) = party
                .iter()
                .position(|m| !m.down() && orb.collected_by(m.follower.position, m.abilities.draw))
        {
            collected.push((i, Some(k)));
        }
    }
    // The members' heal effects run down as Sora's does.
    for (k, m) in party.iter_mut().enumerate() {
        if m.heal_ticks > 0. {
            m.heal_ticks -= 1.;
            if m.heal_ticks <= 0. {
                m.heal_ticks = 0.;
                fx_calls.push(spell_fx::Call {
                    pax: None,
                    fx: player::magic::Fx::LoopEnd(party_room::heal_fx(k)),
                });
            }
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
    for (i, member) in collected.into_iter().rev() {
        let orb = rt.orbs.remove(i);
        let e = enemy_prizes::effect(orb.kind);
        if let Some(m) = member.and_then(|k| party.get_mut(k).map(|m| (k, m))) {
            let (k, m) = m;
            // FUN_1403d7bd0 has no member test: element 6 on that member, a
            // repeat only raising its time.
            let heal = enemy_prizes::heal_ticks(orb.kind);
            if heal > 0. {
                if m.heal_ticks <= 0. {
                    fx_calls.push(spell_fx::Call {
                        pax: system.clone(),
                        fx: player::magic::Fx::Start {
                            key: party_room::heal_fx(k),
                            id: 6,
                            bound: Some(player::magic::Anchor::Party(k + 1)),
                            at: None,
                        },
                    });
                }
                m.heal_ticks = m.heal_ticks.max(heal);
            }
            sound_queue.push((enemy_prizes::sounds(orb.kind).1, "prize_pickup"));
            if e.hp > 0 {
                m.hp = (m.hp + e.hp).min(m.max_hp);
            }
            if e.mp > 0 {
                m.mp.add(e.mp);
            }
            rt.munny += e.munny as u32;
            log_event(
                log,
                serde_json::json!({"event":"prize","tick":tick,"kind":orb.kind,"member":k,"hp":m.hp}),
            );
            continue;
        }
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
        if e.hp > 0 {
            vitals.hp = (vitals.hp + e.hp).min(vitals.target.max_hp);
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
                "hp":vitals.hp,"mp":player.mp.cur,"drive":player.drive.level,
                "drive_partial":player.drive.partial,"munny":rt.munny}),
        );
    }
}

/// The kill (FUN_140410af0): EXP, level-ups, then the prize scatter.
fn kill(
    player: &mut Player,
    vitals: &mut Vitals,
    rt: &mut EnemyRuntime,
    world: &RoomWorld,
    styles: Option<&std::collections::HashMap<u16, u8>>,
    log: &mut Option<std::fs::File>,
    tick: u64,
) {
    let (enemy_exp, level_exp) = rt.exp_rule;
    let critical = rt.difficulty == combat::Difficulty::Critical;
    let low = vitals.hp * 2 <= vitals.target.max_hp;
    let boost = low.then_some(player.abilities.experience_boost as u32);
    let exp = enemy_prizes::kill_exp(enemy_exp, level_exp, critical, boost);
    // FUN_1403ed110: No Experience (ability bit 116) takes the kill's EXP.
    if !player.abilities.has(abilities::bit::NO_EXPERIENCE) {
        grant_exp(rt, vitals, exp, log, tick);
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
                let r = [(); 4].map(|_| rt.rng.range(0., 1.));
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
pub fn grant_exp(
    rt: &mut EnemyRuntime,
    vitals: &mut Vitals,
    exp: u32,
    log: &mut Option<std::fs::File>,
    tick: u64,
) {
    if exp == 0 {
        return;
    }
    rt.exp_total = (rt.exp_total + exp).min(9_999_999);
    let before = vitals.level;
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
        vitals.attacker.strength += new.strength as i32 - old.strength as i32;
        vitals.attacker.magic += new.magic as i32 - old.magic as i32;
        vitals.target.defense += new.defense as i32 - old.defense as i32;
        // [I] the first growth choice's ability.
        let learned = new.abilities[0];
        if learned != 0 {
            rt.learned.push(learned);
        }
        log_event(
            log,
            serde_json::json!({"event":"level_up","tick":tick,"level":l,
                "strength":vitals.attacker.strength,"magic":vitals.attacker.magic,
                "defense":vitals.target.defense,"learned":learned}),
        );
    }
    if after != before {
        vitals.level = after;
        rt.numbers_dirty = true;
    }
    log_event(
        log,
        serde_json::json!({"event":"exp","tick":tick,"gain":exp,"total":rt.exp_total,
            "level":vitals.level}),
    );
}
