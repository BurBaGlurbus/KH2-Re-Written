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
    /// The victim's guard row's reaction to the clash (ATKP +0x13, 2: the
    /// clash flash only).
    guard_refact: u8,
    /// A spell volume's centre (a Magnet draws toward it).
    centre: Option<Vec3>,
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

pub fn tick(mut game: ResMut<Game>, mut previous: Local<HashMap<(u64, usize), Vec3>>) {
    if game.rivals.is_empty() || game.replica {
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
            turn: super::duel_kh3::turn(p.yaw, &game.sora.ext.fighter().kh3),
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
            turn: super::duel_kh3::turn(p.yaw, &r.fighter.ext.fighter().kh3),
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
    // Each Sora's hit record, by index(): an attack whose id the victim's
    // record already holds is refused in land(), so it must not take the
    // victim's one landing this tick (a Magnet's long-lived volume would
    // hide every other spell and swing until it ends).
    let records: Vec<combat::HitRecord> = std::iter::once(game.sora.vitals.record.clone())
        .chain(game.rivals.iter().map(|r| r.fighter.vitals.record.clone()))
        .collect();
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
                if rt
                    .instance_ids
                    .get(instance)
                    .is_some_and(|&(aid, _)| records[index(*victim)].has(aid))
                {
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
                        // The cone takes the vector from the other attack's
                        // owner to this one's (as against the battle enemies).
                        let clashes = matches!(pair, (0, 0) | (0, 2) | (0, 3) | (1, 3))
                            && in_cone(g_row, views[v].facing, -to_attacker)
                            && in_cone(&row, views[a].facing, to_attacker);
                        if !clashes {
                            return None;
                        }
                        points.iter().find_map(|&c| {
                            g_spheres
                                .iter()
                                .find(|(g, gr)| {
                                    combat::spheres_overlap(c, radius, g.to_array(), *gr)
                                })
                                .map(|(g, _)| (*g, g_row.refact_self))
                        })
                    });
                    // A Reflect barrier (the victim's guard volumes) takes it too.
                    let barrier = guard_volume(&spells[v], &points, radius);
                    if let Some((at, refact)) = guard_hit.or(barrier.map(|b| (b.0, 0))) {
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
                            guard_refact: refact,
                            centre: None,
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
                            guard_refact: 0,
                            centre: None,
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
            // A live volume keeps its attack id (the shared retire frees
            // an id whose instance was not seen this tick).
            let instance = magic_room::KEY_BASE | rival::tag_key(vol.key, views[a].slot);
            rt.instance_seen.insert(instance, simulated(tick));
            for (v, (victim, _, bodies)) in all.iter().enumerate() {
                if a == v || !views[v].alive || landed.iter().any(|l| l.victim == *victim) {
                    continue;
                }
                if rt
                    .instance_ids
                    .get(&instance)
                    .is_some_and(|&(aid, _)| records[index(*victim)].has(aid))
                {
                    continue;
                }
                let points = combat::swept_volume(vol.centre, vol.previous, vol.radius, vol.height);
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
                    guard_refact: 0,
                    centre: Some(Vec3::from_array(vol.centre)),
                });
            }
        }
    }
    // Every swing sphere's centre this tick, for next tick's sweep; each live
    // swing keeps its attack id, so it hits a Sora once (and again only as
    // its row's interval allows), not once per tick of contact.
    previous.clear();
    for (_, wins, _) in &all {
        for (instance, _, spheres) in wins {
            rt.instance_seen.insert(*instance, simulated(tick));
            for (i, &(centre, _)) in spheres.iter().enumerate() {
                previous.insert((*instance, i), centre);
            }
        }
    }
    let facts: Vec<(Vec3, Vec3, u8)> = views.iter().map(|v| (v.at, v.facing, v.slot)).collect();
    let sstm = rt.enemy.sstm;
    let rules = game.ext.online().duel_rules;
    for hit in landed {
        land(game, rules, &kit, sstm, &facts, hit, tick);
    }
    // Each rival's record advances once a tick (FUN_1403cf4b0), as the room
    // Sora's does in combat_tick: an attack's interval slots re-arm it.
    for r in &mut game.rivals {
        r.fighter.vitals.record.tick(1., false);
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

/// Whether a Sora cannot be hurt by the other Sora now. [Duel rule] A Sora
/// takes the other's combo as an enemy takes Sora's: his flinches (hurt
/// motions 0-5, ground and air) have no invulnerable frames, so each step
/// that connects lands, the hit record and its combo group deciding as they
/// do against a target. A blow (a finisher's launch or a knock-down), its
/// landing, the recovery and the knock-out keep the frames the game's
/// motions give them, as do dodges and attacks with frames of their own,
/// so a launched Sora gets up before he can be hit again.
fn untouchable(p: &Player, clips: &Clips) -> bool {
    if p.invulnerable > 0. {
        return true;
    }
    if matches!(p.action, Action::Hurt(i) if i < player::hurt::BLOW[0]) {
        return false;
    }
    p.range_invulnerable(clips)
}

/// The tick the field simulated: `simulate` has advanced the room's clock
/// when the duel's hits run, and the shared retire (enemy_battle) keeps the
/// attack ids of instances seen on the simulated tick.
fn simulated(tick: u64) -> u64 {
    tick.saturating_sub(1)
}

fn land(
    game: &mut Game,
    rules: duel_rules::MatchRules,
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
    rt.instance_seen.insert(hit.instance, simulated(tick));
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
    let attacker_combo = {
        let (a, _) = fighter(game, kit, hit.attacker);
        [a.player.combo.step, a.player.combo.finish]
    };
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
    // [KH3 ruleset] A grand magic's spells hit twice as hard; a shotlock
    // volley has its cap (duel_kh3.rs).
    let (grand, volley_dealt) = {
        let (a, _) = fighter(game, kit, hit.attacker);
        (super::duel_kh3::grand(a), a.ext.fighter().kh3.volley_dealt)
    };
    let bolt = hit.spell.is_some_and(super::duel_kh3::is_bolt);
    // [KH3 ruleset] What the hit came from decides its damage kind
    // (player::kh3::hurt).
    let kh3_hit = rules.kh3().then(|| {
        let (a, a_clips) = fighter(game, kit, hit.attacker);
        let source = match hit.spell {
            Some(_) if bolt => player::kh3::Source::Bolt {
                ragnarok: hit.sub == super::duel_kh3::RAGNAROK_SUB,
            },
            Some(_) => player::kh3::Source::Spell {
                heavy: combat::reaction_class(rows[0].reaction) == 4,
                grand,
            },
            None => a.ext.fighter().kh3.source(&a.player, a_clips),
        };
        (source, player::kh3::hurt::strike(source), a.player.in_air())
    });
    let (victim, clips) = fighter(game, kit, hit.victim);
    let refused = |game: &mut Game, why: &str| {
        game.event(serde_json::json!({"event":"duel_hit_refused","tick":tick,
            "attacker":attacker_slot,"victim":victim_slot,"atkp":hit.atkp,"why":why}));
    };
    if victim.vitals.record.has(aid) {
        return;
    }
    if hit.guarded {
        victim.vitals.record.mark(aid);
        // As against the battle enemies (FUN_1403d1f40's callers): a clash
        // flash (system 13) when either row says so or in the air, else the
        // guard flash (11), the guard's recoil and its invulnerability.
        let mut flash = 11;
        if let Some(key) = hit.barrier {
            // Reflect took it: the barrier counts the clash (attack_get_
            // reflect_count, +0xb0), which its counter tiers read.
            if let Some(m) = magic_of(game, hit.victim) {
                *m.reflected.entry(key).or_insert(0) += 1;
            }
        } else {
            let refact_other = rows[0].refact_other;
            let clash_only = hit.guard_refact == 2 || refact_other == 2;
            if !clash_only
                && !victim.player.action.airborne()
                && (hit.guard_refact != 0 || refact_other != 0)
            {
                // Pushed straight back from the attacker, facing him.
                let recoiled = victim
                    .player
                    .guard_recoil([away.x, away.z], clips)
                    .is_some();
                victim.vitals.invulnerable = 80.;
                if recoiled {
                    set_started(game, hit.victim, tick);
                }
            } else {
                flash = 13;
            }
            // [KH3 ruleset] The push-back by the hit's weight; a heavy one
            // offers the Risk Dodge.
            let (victim, _) = fighter(game, kit, hit.victim);
            if let Some((_, strike, attacker_air)) = kh3_hit {
                victim.ext.fighter_mut().kh3.guarded(
                    &mut victim.player,
                    strike,
                    [away.x, away.z],
                    attacker_at.to_array(),
                    attacker_air,
                );
            }
        }
        let repelled = {
            let (a, a_clips) = fighter(game, kit, hit.attacker);
            a.player.connected = true;
            // [KH2 ruleset] The swing's own row repels it (RefactSelf not 2
            // and a reflect motion): the Keyblade's recoil, as off the map
            // [I: the same recoil stands for the attack's reflect motion].
            // The KH3 reference repels only reflect level 3, which no
            // Keyblade swing has.
            !rules.kh3()
                && hit.spell.is_none()
                && rows[0].refact_self != 2
                && rows[0].reflect_motion != 0
                && a.player.wall_recoil(&sstm, a_clips).is_some()
        };
        if repelled {
            set_started(game, hit.attacker, tick);
        }
        game.effect_queue.push(EffectRequest::System {
            id: flash,
            at: hit.contact,
        });
        game.ext
            .online_mut()
            .duel_events
            .push(DuelEvent::Guard(hit.contact.to_array(), flash));
        game.event(serde_json::json!({"event":"duel_guard","tick":tick,
            "attacker":attacker_slot,"victim":victim_slot,"atkp":hit.atkp,
            "flash":flash,"repelled":repelled}));
        return;
    }
    let missed = if rules.kh3() {
        victim.ext.fighter().kh3.untouchable(&victim.player, clips)
    } else {
        untouchable(&victim.player, clips)
    };
    if victim.vitals.invulnerable > 0. || missed {
        // [KH3 ruleset] A dodge that met a hit opens the counter window.
        if rules.kh3() {
            victim.ext.fighter_mut().kh3.dodged();
        }
        victim.vitals.record.hit(aid, group, false, |_| (0, 0));
        let why = format!("invulnerable in {:?}", victim.player.action);
        refused(game, &why);
        return;
    }
    let cure = matches!(rows[0].kind, 5 | 6);
    let (counted, row) = victim.vitals.record.hit(aid, group, cure, |count| {
        let r = data::battle_tables::step_index(&rows, count);
        (r, rows[r].interval)
    });
    if !counted {
        refused(game, "hit record (combo group)");
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
        antiform: victim.player.drive.antiform(),
    };
    // The game's formula, then the duel's damage rule.
    let formula = combat::damage_with(&attacker_stats, &attack, &victim.vitals.target, &state);
    let stats = [
        attacker_stats.strength,
        attacker_stats.magic,
        victim.vitals.target.defense,
    ];
    // Hits on Magic (spells, Reflect's counter, form magic) take the magic
    // multiplier; every hit stops at the rules' cap.
    let magic = attack.stat == 1;
    let mut damage = rules.damage(formula, magic, victim.vitals.target.max_hp);
    if rules.kh3() {
        if grand && hit.spell.is_some() && !bolt {
            let doubled = (formula as f32 * player::kh3::tuning::GRAND_DAMAGE).round() as i32;
            damage = rules.damage(doubled, magic, victim.vitals.target.max_hp);
        }
        if bolt {
            let cap = victim.vitals.target.max_hp * player::kh3::tuning::VOLLEY_CAP_PERCENT / 100;
            damage = damage.min((cap - volley_dealt).max(0));
            if damage == 0 {
                victim.vitals.record.mark(aid);
                refused(game, "volley cap");
                return;
            }
        }
        victim.ext.fighter_mut().kh3.hit_taken();
    }
    let damage_motion = (10..=39).contains(&victim.player.motion_id(clips));
    let veto = combat::sora_veto(ab.has(127), ab.has(128), victim.vitals.hp, 0, damage_motion);
    let max = victim.vitals.target.max_hp;
    let before = victim.vitals.hp;
    victim.vitals.hp = combat::apply_damage(victim.vitals.hp, 0, max, damage, veto);
    let taken = (before - victim.vitals.hp).max(0) as u32;
    let reaction = match hit.centre {
        // [Duel rule] A Magnet holds an enemy at a radius around the spell
        // (reaction 5, the Magnet hold); the player's reactions have no
        // case for it (FUN_1403d2fe0), so a Sora would only take the
        // damage. In a duel he flinches and slides toward the Magnet,
        // stopping at its hold radius (KB1..KB2's middle): the knock
        // speed decays by KNOCK_DECAY a tick, so it covers speed / (1 -
        // decay).
        Some(centre) if combat::reaction_class(attack.reaction) == 5 => {
            let to = (centre - victim_at) * Vec3::new(1., 0., 1.);
            let hold = (f32::from(attack.knockback[0]) + f32::from(attack.knockback[1])) * 0.5;
            let speed = (to.length() - hold).max(0.) * (1. - actor_physics::KNOCK_DECAY);
            let toward = to.normalize_or(-away);
            player::Hit {
                reaction: 3,
                original: 3,
                direction: [toward.x, toward.z],
                high: hit.high,
                knockback: [speed.round() as i16, 0],
            }
        }
        _ => player::Hit {
            reaction: attack.reaction,
            original: attack.reaction,
            direction: [away.x, away.z],
            high: hit.high,
            knockback: attack.knockback,
        },
    };
    // [KH3 ruleset] The reference's reaction for the hit's kind, on KH2's
    // damage motions; flow states and a command's first tenth of a second
    // shrug off light ones (the damage stays). A Magnet keeps its pull.
    let magnet = hit.centre.is_some() && combat::reaction_class(attack.reaction) == 5;
    let mut kh3_log = None;
    let mut started = match kh3_hit {
        Some((source, strike, _)) if !magnet => {
            let shrug = victim
                .ext
                .fighter()
                .kh3
                .shrugs_strike(&victim.player, strike);
            kh3_log = Some((source, strike, shrug));
            // A body already in the air keeps its motion (no new voice).
            !shrug
                && victim.ext.fighter_mut().kh3.hurt(
                    &mut victim.player,
                    strike,
                    [away.x, away.z],
                    hit.high,
                    clips,
                )
        }
        _ => victim.player.react(&reaction, &sstm, clips).is_some(),
    };
    let reacted = (
        victim.ext.fighter().kh3.mode.name(),
        format!("{:?}", victim.player.action),
    );
    // A bolt's own impact: Wisdom's shot spark and its sound, once in a
    // few ticks on one Sora.
    let bolt_spark = bolt && victim.ext.fighter_mut().kh3.spark_ready();
    let hp = victim.vitals.hp;
    if hp == 0 && victim.player.die(clips).is_some() {
        started = true;
    }
    if started {
        set_started(game, hit.victim, tick);
    }
    {
        let a = fighter(game, kit, hit.attacker).0;
        a.player.connected = true;
        if rules.kh3() {
            if bolt {
                a.ext.fighter_mut().kh3.volley_dealt += taken as i32;
            } else {
                a.ext.fighter_mut().kh3.landed(hit.spell.is_none());
            }
            if let Some((source, strike, _)) = kh3_log {
                a.ext.fighter_mut().kh3.struck(source, strike);
            }
        }
    }
    if bolt_spark {
        super::duel_kh3::cues(
            game,
            "bolt_hit",
            attacker_slot,
            drive_fx::Anchor::World {
                at: hit.contact,
                pitch: 0.,
                yaw: away.x.atan2(away.z),
            },
        );
    }
    // [KH3 ruleset] A heavy swing's own burst where it landed, sized by
    // its weight (the Keyblade's spark stays under it).
    if let Some((_, strike, false)) = kh3_log
        && strike.knock.heavy()
        && hit.spell.is_none()
    {
        super::duel_kh3::show(
            game,
            player::kh3::look::Look::HeavyHit,
            attacker_slot,
            Some((hit.contact.to_array(), away.x.atan2(away.z))),
            super::duel_kh3::hit_size(&strike),
        );
    }
    if let Some((source, strike, shrug)) = kh3_log {
        game.event(serde_json::json!({"event":"kh3_react","tick":tick,
            "attacker":attacker_slot,"victim":victim_slot,"source":source.name(),
            "knock":strike.knock.name(),"scale":strike.scale,"shrug":shrug,
            "mode":reacted.0,"action":reacted.1,"hp":hp}));
    }
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
            rival_play::retag_calls(&mut calls, attacker_slot);
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
    game.ext.online_mut().duel_events.extend(events);
    // The match's tally, by the attacker's slot.
    let s = usize::from(attacker_slot);
    if s < 2 {
        game.ext.online_mut().duel_dealt[s] += taken;
        game.ext.online_mut().duel_landed[s] =
            game.ext.online_mut().duel_landed[s].saturating_add(1);
    }
    game.event(
        serde_json::json!({"event":"duel_hit","tick":tick,"attacker":attacker_slot,
        "victim":victim_slot,"atkp":hit.atkp,"step":row,"formula":formula,"damage":damage,"hp":hp,
        "combo":attacker_combo,"reaction":attack.reaction,"spell":hit.spell.is_some(),
        "aid":aid,"group":group,"interval":rows[row].interval,"instance":hit.instance,
        "stats":stats,"power":attack.power,"magic":magic}),
    );
}

/// [Test-room choice, as the room's own KO] A knocked-out rival stands up
/// with full HP after 120 ticks; the room Sora's own KO stays the room's.
fn knock_outs(game: &mut Game, tick: u64) {
    // In a match a knock-out ends the round (duel_match.rs).
    if game.ext.online().duel_match {
        return;
    }
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
        game.ext
            .online_mut()
            .duel_events
            .push(DuelEvent::StoodUp(slot));
        game.event(serde_json::json!({"event":"duel_stood_up","tick":tick,"slot":slot}));
    }
}
