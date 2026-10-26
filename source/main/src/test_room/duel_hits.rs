//! Sora against Sora: each one's Keyblade windows against the other's HIT
//! spheres, with the room's own rules: the attack's ATKP rows, the
//! victim's hit record (hit once, the 30-tick group lock), the damage
//! formula with both sides player-type, guards that clash, the reaction
//! and the knock-out. The duel's difficulty is Standard (×1).
//!
//! Runs on the host (or with a scripted rival), after every Sora moved.

use super::duel_wire::DuelEvent;
use super::*;

/// Who a body belongs to: the room's Sora or the rival at an index.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Who {
    Room,
    Rival(usize),
}

/// What the hit test needs of one Sora this tick.
struct View<'a> {
    who: Who,
    slot: u8,
    at: Vec3,
    turn: Quat,
    facing: Vec3,
    started: u64,
    cp: &'a ClipPoses,
    frame: usize,
    blades: &'a [character_kit::Blade; 2],
    body: &'a [(usize, Vec3, f32, bool)],
    alive: bool,
}

/// An attack window this tick: instance key, ATKP, sphere centres and
/// radii.
type Window = (u64, u16, Vec<(Vec3, f32)>);
/// A HIT sphere this tick: centre, radius, high.
type Sphere = (Vec3, f32, bool);

fn windows(v: &View) -> Vec<Window> {
    let mut out = Vec::new();
    for (hand, (blade, weapon, hits)) in [
        (&v.cp.blade, &v.cp.weapon, &v.cp.hits),
        (&v.cp.blade2, &v.cp.weapon2, &v.cp.hits2),
    ]
    .into_iter()
    .enumerate()
    {
        if blade.is_empty() || weapon.is_empty() {
            continue;
        }
        let f = v.frame.min(blade.len() - 1);
        let to_model = weapon[f.min(weapon.len() - 1)] * blade[f];
        for (w, &(start, end, atkp, group)) in hits.iter().enumerate() {
            if (v.frame as u32) < start || v.frame as u32 > end {
                continue;
            }
            let spheres = v.blades[hand]
                .get(&group)
                .into_iter()
                .flatten()
                .map(|&(offset, radius)| {
                    (v.at + v.turn * to_model.transform_point3(offset), radius)
                })
                .collect();
            // The slot keeps two Soras' windows apart.
            let key = 1 << 62
                | u64::from(v.slot) << 56
                | (v.started & 0xff_ffff_ffff) << 7
                | (hand as u64) << 5
                | w as u64;
            out.push((key, atkp, spheres));
        }
    }
    out
}

fn body(v: &View) -> Vec<Sphere> {
    let Some(bones) =
        v.cp.body
            .get(v.frame.min(v.cp.body.len().saturating_sub(1)))
    else {
        return Vec::new();
    };
    v.body
        .iter()
        .filter_map(|&(b, offset, r, high)| {
            let m = bones.get(&b)?;
            Some((v.at + v.turn * m.transform_point3(offset), r, high))
        })
        .collect()
}

/// A hit found this tick, applied after every test ran.
struct Landed {
    attacker: Who,
    victim: Who,
    instance: u64,
    atkp: u16,
    /// The ATKP sub-table (a spell's row) and the spell volume's key.
    sub: u8,
    spell: Option<u64>,
    /// A moving volume's direction (it pushes the way it travels).
    moving: Option<Vec3>,
    high: bool,
    contact: Vec3,
    /// The victim's guard took it (its own guard ATKP, or his Reflect).
    guarded: bool,
    /// The victim's Reflect volume that took it.
    barrier: Option<u64>,
}

/// The first of a Sora's guard volumes (Reflect) the points meet: the
/// contact and the volume's key.
fn guard_volume(
    volumes: &[magic::Volume],
    points: &[[f32; 3]],
    radius: f32,
) -> Option<(Vec3, u64)> {
    volumes
        .iter()
        .filter(|v| matches!(v.role, magic::Role::Guard))
        .find_map(|g| {
            points.iter().find_map(|&c| {
                combat::volume_hits_sphere(g.centre, g.radius, g.height, c, radius)
                    .then(|| (Vec3::from_array(c), g.key))
            })
        })
}

/// The magic state of the Sora `who`.
fn magic_of(game: &mut Game, who: Who) -> Option<&mut magic_room::MagicRt> {
    match who {
        Who::Room => game.magic.as_mut(),
        Who::Rival(i) => game.rivals.get_mut(i)?.own.magic.as_mut(),
    }
}

/// FUN_1403d1f40's cone: the other attack's owner within the reflect range
/// of this attack's facing.
fn in_cone(row: &data::battle_tables::Attack, facing: Vec3, v: Vec3) -> bool {
    if row.reflect_range == 0 {
        return true;
    }
    let turn = Quat::from_rotation_y((row.reflect_angle as f32 * 2.).to_radians());
    v.dot(turn * facing) < -(row.reflect_range as f32).to_radians().cos()
}

pub(crate) fn tick(mut game: ResMut<Game>, mut previous: Local<HashMap<(u64, usize), Vec3>>) {
    if game.rivals.is_empty() || game.net_client {
        return;
    }
    let game = &mut *game;
    let Some(kit) = game.rival_kit.clone() else {
        return;
    };
    let tick = game.tick;
    let Some(rt) = game.enemy.as_mut() else {
        return;
    };
    // The room Sora's spheres follow his kit (kept by the battle runtime).
    let (room_blades, room_body) = (rt.blade.clone(), rt.sora_body.clone());
    let room_view = {
        let p = &game.sora.player;
        let clip = game.poses[&(p.action, p.armed)];
        View {
            who: Who::Room,
            slot: game.local_slot,
            at: Vec3::from_array(p.position),
            turn: Quat::from_rotation_y(p.yaw),
            facing: Vec3::new(p.yaw.sin(), 0., p.yaw.cos()),
            started: game.action_started,
            cp: &game.clip_poses[clip],
            frame: p.motion_frame(&game.clips) as usize,
            blades: &room_blades,
            body: &room_body,
            alive: game.sora.vitals.hp > 0,
        }
    };
    let mut views = vec![room_view];
    for (i, r) in game.rivals.iter().enumerate() {
        let p = &r.fighter.player;
        let o = &r.own;
        let Some(cp) = o
            .poses
            .get(&(p.action, p.armed))
            .and_then(|&clip| o.clip_poses.get(clip))
        else {
            continue;
        };
        views.push(View {
            who: Who::Rival(i),
            slot: r.slot,
            at: Vec3::from_array(p.position),
            turn: Quat::from_rotation_y(p.yaw),
            facing: Vec3::new(p.yaw.sin(), 0., p.yaw.cos()),
            started: r.started,
            cp,
            frame: p.motion_frame(&o.clips) as usize,
            blades: &o.blades,
            body: &o.body,
            alive: r.fighter.vitals.hp > 0,
        });
    }
    // Each Sora's spell volumes this tick (his own magic state).
    let mut spells: Vec<Vec<magic::Volume>> = vec![
        game.magic
            .as_ref()
            .map(|m| m.volumes.clone())
            .unwrap_or_default(),
    ];
    for r in &game.rivals {
        spells.push(
            r.own
                .magic
                .as_ref()
                .map(|m| m.volumes.clone())
                .unwrap_or_default(),
        );
    }
    let all: Vec<(Who, Vec<Window>, Vec<Sphere>)> =
        views.iter().map(|v| (v.who, windows(v), body(v))).collect();
    let mut landed: Vec<Landed> = Vec::new();
    let mut seen = HashMap::new();
    for (a, (attacker, wins, _)) in all.iter().enumerate() {
        for (instance, atkp, spheres) in wins {
            ensure_attack_rows(rt, *atkp, 0);
            let Some(row) = rt.rows.get(atkp).copied() else {
                continue;
            };
            // Guard windows only clash.
            if matches!(row.kind, 2 | 3) {
                continue;
            }
            for (v, (victim, guards, bodies)) in all.iter().enumerate() {
                if a == v || !views[v].alive || landed.iter().any(|l| l.victim == *victim) {
                    continue;
                }
                for (i, &(centre, radius)) in spheres.iter().enumerate() {
                    let from = *seen
                        .entry((*instance, i))
                        .or_insert_with(|| previous.get(&(*instance, i)).copied());
                    let points =
                        combat::swept(centre.to_array(), from.map(|p| p.to_array()), radius);
                    // The victim's live guard windows meet the swing first.
                    let to_attacker =
                        ((views[a].at - views[v].at) * Vec3::new(1., 0., 1.)).normalize_or_zero();
                    let guard_hit = guards.iter().find_map(|(_, g_atkp, g_spheres)| {
                        let g_row = rt.rows.get(g_atkp)?;
                        if !matches!(g_row.kind, 2 | 3) {
                            return None;
                        }
                        let pair = (g_row.kind.min(row.kind), g_row.kind.max(row.kind));
                        let clashes = matches!(pair, (0, 0) | (0, 2) | (0, 3) | (1, 3))
                            && in_cone(g_row, views[v].facing, to_attacker)
                            && in_cone(&row, views[a].facing, -to_attacker);
                        if !clashes {
                            return None;
                        }
                        points.iter().find_map(|&c| {
                            g_spheres
                                .iter()
                                .find(|(g, gr)| {
                                    combat::spheres_overlap(c, radius, g.to_array(), *gr)
                                })
                                .map(|(g, _)| *g)
                        })
                    });
                    // A Reflect barrier (the victim's guard volumes) takes it too.
                    let barrier = guard_volume(&spells[v], &points, radius);
                    if let Some(at) = guard_hit.or(barrier.map(|b| b.0)) {
                        landed.push(Landed {
                            attacker: *attacker,
                            victim: *victim,
                            instance: *instance,
                            atkp: *atkp,
                            sub: 0,
                            spell: None,
                            moving: None,
                            high: true,
                            contact: at,
                            guarded: true,
                            barrier: barrier.map(|b| b.1),
                        });
                        break;
                    }
                    if let Some(&(b, _, high)) = points.iter().find_map(|&c| {
                        bodies
                            .iter()
                            .find(|(b, r, _)| combat::spheres_overlap(c, radius, b.to_array(), *r))
                    }) {
                        landed.push(Landed {
                            attacker: *attacker,
                            victim: *victim,
                            instance: *instance,
                            atkp: *atkp,
                            sub: 0,
                            spell: None,
                            moving: None,
                            high,
                            contact: b,
                            guarded: false,
                            barrier: None,
                        });
                        break;
                    }
                }
            }
        }
    }
    // Spell volumes (YS::ATTACK_MAGIC): the damaging ones against the other
    // Soras' HIT spheres (one body hit per target per tick), a barrier first.
    for (a, volumes) in spells.iter().enumerate() {
        let attacker = views[a].who;
        for vol in volumes
            .iter()
            .filter(|vol| matches!(vol.role, magic::Role::Damage))
        {
            ensure_attack_rows(rt, vol.atkp, vol.sub);
            for (v, (victim, _, bodies)) in all.iter().enumerate() {
                if a == v || !views[v].alive || landed.iter().any(|l| l.victim == *victim) {
                    continue;
                }
                let points = combat::swept_volume(vol.centre, vol.previous, vol.radius, vol.height);
                let instance = magic_room::KEY_BASE | rival::tag_key(vol.key, views[a].slot);
                let barrier = guard_volume(&spells[v], &points, vol.radius);
                let body_hit = points.iter().find_map(|&c| {
                    bodies.iter().find(|(b, r, _)| {
                        combat::volume_hits_sphere(c, vol.radius, vol.height, b.to_array(), *r)
                    })
                });
                let (contact, high, guarded) = match (barrier, body_hit) {
                    (Some((at, _)), _) => (at, true, true),
                    (None, Some(&(b, _, high))) => (b, high, false),
                    (None, None) => continue,
                };
                landed.push(Landed {
                    attacker,
                    victim: *victim,
                    instance,
                    atkp: vol.atkp,
                    sub: vol.sub,
                    spell: Some(vol.key),
                    moving: vol.moving.map(Vec3::from_array),
                    high,
                    contact,
                    guarded,
                    barrier: barrier.map(|b| b.1),
                });
            }
        }
    }
    // Every swing sphere's centre this tick, for next tick's sweep.
    previous.clear();
    for (_, wins, _) in &all {
        for (instance, _, spheres) in wins {
            for (i, &(centre, _)) in spheres.iter().enumerate() {
                previous.insert((*instance, i), centre);
            }
        }
    }
    let facts: Vec<(Vec3, Vec3, u8)> = views.iter().map(|v| (v.at, v.facing, v.slot)).collect();
    let sstm = rt.enemy.sstm;
    let rules = game.duel_rules;
    for hit in landed {
        land(game, rules, &kit, sstm, &facts, hit, tick);
    }
    knock_outs(game, tick);
}

fn index(who: Who) -> usize {
    match who {
        Who::Room => 0,
        Who::Rival(i) => i + 1,
    }
}

/// The Sora `who` with his clips.
fn fighter<'a>(
    game: &'a mut Game,
    kit: &'a rival::RivalKit,
    who: Who,
) -> (&'a mut Fighter, &'a Clips) {
    let _ = kit;
    match who {
        Who::Room => (&mut game.sora, &game.clips),
        Who::Rival(i) => {
            let r = &mut game.rivals[i];
            (&mut r.fighter, &r.own.clips)
        }
    }
}

fn set_started(game: &mut Game, who: Who, tick: u64) {
    match who {
        Who::Room => game.action_started = tick,
        Who::Rival(i) => game.rivals[i].started = tick,
    }
}

fn land(
    game: &mut Game,
    rules: duel_loadout::MatchRules,
    kit: &rival::RivalKit,
    sstm: actor_physics::Sstm,
    facts: &[(Vec3, Vec3, u8)],
    hit: Landed,
    tick: u64,
) {
    let (attacker_at, _, attacker_slot) = facts[index(hit.attacker)];
    let (victim_at, _, victim_slot) = facts[index(hit.victim)];
    let Some(rt) = game.enemy.as_mut() else {
        return;
    };
    let Some(rows) = rt.steps.get(&(hit.atkp, hit.sub)).cloned() else {
        return;
    };
    let hit_sound = rt.hit_sounds.get(&hit.atkp).copied();
    let spark = match hit.spell {
        None => rt.hit_effects.get(&hit.atkp).copied(),
        // A spell's row names its own spark.
        Some(_) => Some((rows[0].hit_effect, rows[0].hit_effect_random)),
    };
    // The attack's id and combo group, shared with the room's other hits.
    rt.instance_seen.insert(hit.instance, tick);
    let (aid, group) = *rt.instance_ids.entry(hit.instance).or_insert_with(|| {
        let combo = rows[0].combo_group as i32;
        let group = if combo != 0 {
            combo
        } else {
            combat::allocate_group(&mut rt.next_group)
        };
        (rt.ids.allocate(), group)
    });
    let away = (hit.moving.unwrap_or(victim_at - attacker_at) * Vec3::new(1., 0., 1.))
        .normalize_or(Vec3::Z);
    let attacker_stats = {
        let (a, _) = fighter(game, kit, hit.attacker);
        let p = &a.player;
        let ab = &p.abilities;
        a.vitals.attacker.mods = combat::AttackerMods {
            player_type: true,
            mp_charging: p.mp_charging,
            berserk_strength: ab.berserk_strength as i32,
            element_percent: ab.element.map(i32::from),
            combo_step: p.combo.step as i32,
            ground_limit: ab.ground_combo as i32,
            air_limit: ab.air_combo as i32,
            combo_boost: ab.combo_boost as i32,
            air_combo_boost: ab.air_combo_boost as i32,
            reaction_boost: ab.reaction_boost as i32,
            combo_boost_enabled: ab.has(abilities::bit::COMBO_BOOST),
            air_combo_boost_enabled: ab.has(abilities::bit::AIR_COMBO_BOOST),
            reaction_boost_enabled: ab.has(abilities::bit::REACTION_BOOST),
        };
        a.vitals.attacker
    };
    let (victim, clips) = fighter(game, kit, hit.victim);
    if victim.vitals.record.has(aid) {
        return;
    }
    if hit.guarded {
        victim.vitals.record.mark(aid);
        if let Some(key) = hit.barrier {
            // Reflect took it: the barrier counts the clash (attack_get_
            // reflect_count, +0xb0), which its counter tiers read.
            if let Some(m) = magic_of(game, hit.victim) {
                *m.reflected.entry(key).or_insert(0) += 1;
            }
        } else {
            let recoiled = victim
                .player
                .guard_recoil([-away.x, -away.z], clips)
                .is_some();
            victim.vitals.invulnerable = 80.;
            if recoiled {
                set_started(game, hit.victim, tick);
            }
        }
        fighter(game, kit, hit.attacker).0.player.connected = true;
        game.effect_queue.push(EffectRequest::System {
            id: 11,
            at: hit.contact,
        });
        game.duel_events
            .push(DuelEvent::Guard(hit.contact.to_array()));
        game.event(serde_json::json!({"event":"duel_guard","tick":tick,
            "attacker":attacker_slot,"victim":victim_slot,"atkp":hit.atkp}));
        return;
    }
    if victim.vitals.invulnerable > 0. || victim.player.range_invulnerable(clips) {
        victim.vitals.record.hit(aid, group, false, |_| (0, 0));
        return;
    }
    let cure = matches!(rows[0].kind, 5 | 6);
    let (counted, row) = victim.vitals.record.hit(aid, group, cure, |count| {
        let r = data::battle_tables::step_index(&rows, count);
        (r, rows[r].interval)
    });
    if !counted {
        return;
    }
    let attack = params_of(&rows[row]);
    let ab = &victim.player.abilities;
    let state = combat::TargetState {
        hp: victim.vitals.hp,
        player_type: true,
        defender: ab.defender as i32,
        damage_control: ab.damage_control,
        damage_control_ratio: 50,
        // [Duel rule] Standard: damage taken ×1 whatever the room's
        // difficulty.
        difficulty: combat::Difficulty::Standard,
    };
    // The game's formula, then the duel's damage rule.
    let formula = combat::damage_with(&attacker_stats, &attack, &victim.vitals.target, &state);
    let damage = rules.damage(formula);
    let damage_motion = (10..=39).contains(&victim.player.motion_id(clips));
    let veto = combat::sora_veto(ab.has(127), ab.has(128), victim.vitals.hp, 0, damage_motion);
    let max = victim.vitals.target.max_hp;
    victim.vitals.hp = combat::apply_damage(victim.vitals.hp, 0, max, damage, veto);
    let reaction = player::Hit {
        reaction: attack.reaction,
        original: attack.reaction,
        direction: [away.x, away.z],
        high: hit.high,
        knockback: attack.knockback,
    };
    let mut started = victim.player.react(&reaction, &sstm, clips).is_some();
    let hp = victim.vitals.hp;
    if hp == 0 && victim.player.die(clips).is_some() {
        started = true;
    }
    if started {
        set_started(game, hit.victim, tick);
    }
    fighter(game, kit, hit.attacker).0.player.connected = true;
    // Played here and sent to the joiner, who plays the same (duel_sync).
    let mut events = Vec::new();
    if let Some(id) = hit_sound.filter(|&id| id >= 0) {
        game.sound_queue.push((id as u32, "duel_hit"));
        events.push(DuelEvent::Sound(id as u32));
    }
    let spell_spark = match (hit.spell, spark.filter(|s| s.0 != 0)) {
        // A spell's spark comes from the spell's own PAX (an effect call,
        // which reaches the joiner with the others).
        (Some(key), Some((effect, _))) => magic_of(game, hit.attacker).map(|m| {
            m.feedback.hit.push(key);
            magic_room::spark(m, key, effect, hit.contact.to_array(), away.to_array())
        }),
        (Some(key), None) => {
            if let Some(m) = magic_of(game, hit.attacker) {
                m.feedback.hit.push(key);
            }
            Some(None)
        }
        (None, _) => None,
    };
    if let Some(calls) = spell_spark {
        let mut calls = calls.unwrap_or_default();
        if hit.attacker != Who::Room {
            rival::retag_calls(&mut calls, attacker_slot);
        }
        game.fx_calls.extend(calls);
    } else if let Some((effect, random)) = spark.filter(|s| s.0 != 0) {
        let own = (hit.instance >> 5) & 3 == 2;
        game.effect_queue.push(EffectRequest::Hit {
            effect,
            random,
            contact: hit.contact,
            direction: away,
            own,
        });
        events.push(DuelEvent::Spark {
            effect,
            random,
            contact: hit.contact.to_array(),
            direction: away.to_array(),
            own,
        });
    }
    if hp == 0 {
        events.push(DuelEvent::KnockOut(victim_slot));
    }
    game.duel_events.extend(events);
    game.event(
        serde_json::json!({"event":"duel_hit","tick":tick,"attacker":attacker_slot,
        "victim":victim_slot,"atkp":hit.atkp,"step":row,"formula":formula,"damage":damage,"hp":hp}),
    );
}

/// [Test-room choice, as the room's own KO] A knocked-out rival stands up
/// with full HP after 120 ticks; the room Sora's own KO stays the room's.
fn knock_outs(game: &mut Game, tick: u64) {
    let mut stood = Vec::new();
    for r in &mut game.rivals {
        let v = &mut r.fighter.vitals;
        if v.hp > 0 {
            v.ko_ticks = 0;
            continue;
        }
        v.ko_ticks += 1;
        if v.ko_ticks >= 120 {
            v.hp = v.target.max_hp;
            v.ko_ticks = 0;
            if r.fighter.player.dead() {
                r.fighter.player.revive();
                r.started = tick;
            }
            stood.push(r.slot);
        }
    }
    for slot in stood {
        game.duel_events.push(DuelEvent::StoodUp(slot));
        game.event(serde_json::json!({"event":"duel_stood_up","tick":tick,"slot":slot}));
    }
}
