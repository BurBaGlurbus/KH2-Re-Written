//! The Sephiroth fight: objentry 2230 (`N_HB630`), placed by spawn set `b_60`
//! of Hollow Bastion area 1 (`ard/us/hb01.ard`). The boss runs from his own
//! object script (`enemy_script`); this module only puts him in the room the
//! way the area script does and watches the fight end.
use super::*;
use enemy_gallery::EnemyGallery;

/// The summon waits for the room's first ticks to settle.
const START_TICK: u64 = 20;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
enum Stage {
    #[default]
    Waiting,
    Summoned,
    Fighting,
    Won,
}

/// Ticks between the boss's last blow and the room closing for the outro.
const OUTRO_DELAY: u64 = 240;

/// The unattended tour (`--sephiroth-tour`): its length in ticks
/// (`KH2RW_SEPH_TOUR_END` overrides) and its captures (`KH2RW_SEPH_TOUR_CAPS`
/// adds ticks).
pub fn tour_end() -> u64 {
    std::env::var("KH2RW_SEPH_TOUR_END")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(7200)
}

pub fn tour_captures() -> Vec<u64> {
    let end = tour_end();
    let mut c: Vec<u64> = [60, 240, 420]
        .into_iter()
        .chain((600..end).step_by(600))
        .collect();
    // `KH2RW_SEPH_TOUR_SHOTS=<tick>,<tick>,...` adds captures (e.g. during
    // the clear overlays).
    if let Ok(v) = std::env::var("KH2RW_SEPH_TOUR_SHOTS") {
        c.extend(v.split(',').filter_map(|t| t.trim().parse::<u64>().ok()));
    }
    c.retain(|&t| t < end);
    c.sort_unstable();
    c.dedup();
    c.push(end);
    // More capture ticks (`KH2RW_SEPH_TOUR_CAPS`, comma-separated).
    if let Ok(v) = std::env::var("KH2RW_SEPH_TOUR_CAPS") {
        c.extend(v.split(',').filter_map(|t| t.trim().parse::<u64>().ok()));
        c.sort_unstable();
        c.dedup();
    }
    c
}

/// The tour's pad: walk at the boss (the camera looks his way once Sora
/// locks on), then a combo press every 10 ticks within reach. [Tour
/// setup, not a game rule.]
/// The tour presses Triangle on every other command offered (the rest
/// show what happens without it), when Sora stands on the ground (a
/// landing cuts a captured motion short, as the land motion replaces it).
/// [Tour setup.]
pub fn tour_reaction(tick: u64, airborne: bool, offers: u32) -> bool {
    tick >= 240 && !airborne && offers % 2 == 1
}

pub fn tour_input(game: &Game, tick: u64) -> (Vec2, bool, bool) {
    if tick < 240 {
        return (Vec2::ZERO, false, false);
    }
    let sora = Vec3::from_array(game.sora.player.position);
    let near = game
        .gallery_targets
        .first()
        .map(|t| (t.position - sora).with_y(0.).length());
    match near {
        Some(d) if d > 220. => (Vec2::new(0., 1.), false, false),
        Some(_) => (Vec2::ZERO, false, tick.is_multiple_of(10)),
        None => (Vec2::ZERO, false, false),
    }
}

#[derive(Resource, Default)]
pub struct Fight {
    stage: Stage,
    actor: Option<u64>,
    won_at: Option<u64>,
}

pub fn tick(
    mut game: ResMut<Game>,
    mut gallery: ResMut<EnemyGallery>,
    area: Option<Res<AreaRes>>,
    mut fight: ResMut<Fight>,
    mut exit: MessageWriter<AppExit>,
    mut camp: Option<ResMut<CampRes>>,
) {
    if !game.sephiroth {
        return;
    }
    let tick = game.tick;
    if game.sephiroth_tour {
        if tick == 60
            && let Err(e) = enemy_target::set_scan(&mut game, camp.as_deref_mut(), true)
        {
            game.event(serde_json::json!({"event":"sephiroth_tour_scan_failed","reason":e}));
        }
        // [Tour setup] `KH2RW_SEPH_TOUR_LOW=<tick>[:<hp>]`: the boss drops to
        // `hp` (30 by default) at that tick, so the run reaches his last
        // phase or his death (a finisher must still land the last blow).
        let low = std::env::var("KH2RW_SEPH_TOUR_LOW").ok().and_then(|v| {
            let mut it = v.split(':');
            let at = it.next()?.parse::<u64>().ok()?;
            let hp = it.next().map_or(Some(30), |h| h.parse::<i32>().ok())?;
            Some((at, hp))
        });
        if let Some((_, hp)) = low.filter(|l| l.0 == tick)
            && let Some(b) = fight
                .actor
                .and_then(|id| gallery.actors.iter_mut().find(|a| a.id == id))
                .and_then(|a| a.battle.as_mut())
        {
            b.brain.set_hp(hp);
            game.event(serde_json::json!({"event":"sephiroth_tour_low","tick":tick,"hp":hp}));
        }
        // [Tour setup] The room's Sora is level 1; he gets the HP to last
        // the run (his hits stay vulnerable, so the grab can happen).
        if tick == 60 && game.enemy.is_some() {
            game.sora.vitals.target.max_hp = 9999;
            game.sora.vitals.hp = 9999;
        }
        // [Tour setup] Heartless Angel leaves him at 1 HP; two seconds later
        // he is refilled so the run goes on (the game would need a heal).
        if tick > 60
            && tick.is_multiple_of(120)
            && game.enemy.is_some()
            && (1..1000).contains(&game.sora.vitals.hp)
        {
            game.sora.vitals.hp = game.sora.vitals.target.max_hp;
            game.event(serde_json::json!({"event":"sephiroth_tour_refill","tick":tick}));
        }
        // Lock on once he is there, and again whenever the lock was lost.
        if tick >= 200 && tick.is_multiple_of(60) && !game.target_locked {
            game.lock_queued = true;
        }
        let every = std::env::var("KH2RW_SEPH_TOUR_LOG")
            .ok()
            .and_then(|v| v.parse::<u64>().ok())
            .unwrap_or(120)
            .max(1);
        if tick.is_multiple_of(every) {
            let boss = fight
                .actor
                .and_then(|id| gallery.actors.iter().find(|a| a.id == id))
                .and_then(|a| a.battle.as_ref())
                .map(|b| {
                    serde_json::json!({"hp":b.brain.hp(),"act":b.brain.description(),
                        "motion":b.brain.motion(),"position":b.brain.position(),
                        "state":b.brain.boss_state(),"error":b.error})
                });
            let sora_hp = game.enemy.as_ref().map(|_| game.sora.vitals.hp);
            let (sora, locked) = (game.sora.player.position, game.target_locked);
            game.event(serde_json::json!({"event":"sephiroth_tour","tick":tick,
                "boss":boss,"sora_hp":sora_hp,"sora":sora,"locked":locked}));
        }
    }
    match fight.stage {
        Stage::Waiting if tick >= START_TICK => {
            let Some(entry) = gallery
                .catalog
                .entries
                .iter()
                .position(|e| e.object_id == enemy_battle::SEPHIROTH)
            else {
                game.autoplay_failed = true;
                game.event(serde_json::json!({"event":"sephiroth_missing","tick":tick}));
                fight.stage = Stage::Won;
                return;
            };
            // The area's own placement (scene coordinates, scene yaw), else
            // the flat development arena: in front of Sora.
            let placed = area.as_ref().and_then(|a| {
                a.current
                    .spawns
                    .iter()
                    .find(|s| s.entity.object == enemy_battle::SEPHIROTH)
                    .map(|s| {
                        // The spawn's engine yaw Y (pi - rotY, FUN_1403b6f20)
                        // faces (sin, cos) in the engine; the scene's yaw is
                        // pi - Y (b_60: rotY 0, facing Sora).
                        let [x, y, z] = s.entity.position;
                        (
                            Vec3::new(x, -y, -z),
                            ::area::wrap(std::f32::consts::PI - s.yaw),
                        )
                    })
            });
            let (at, yaw) = placed.unwrap_or_else(|| {
                (
                    Vec3::from_array(game.sora.player.position) + Vec3::new(0., 0., 750.),
                    std::f32::consts::PI,
                )
            });
            gallery.queue_summon(entry, at, yaw);
            game.event(serde_json::json!({"event":"sephiroth_summon","tick":tick,
                "position":at.to_array(),"yaw":yaw,"from_area":placed.is_some()}));
            fight.stage = Stage::Summoned;
        }
        Stage::Summoned => {
            if let Some(id) = gallery.actors.iter().map(|a| a.id).next() {
                gallery.select(id);
                gallery.queue_battle();
                fight.actor = Some(id);
                fight.stage = Stage::Fighting;
                game.event(serde_json::json!({"event":"sephiroth_enable","tick":tick,"actor":id}));
            }
        }
        Stage::Fighting => {
            let alive = gallery
                .actors
                .iter()
                .find(|a| Some(a.id) == fight.actor)
                .and_then(|a| a.battle.as_ref())
                .map(|b| b.brain.alive());
            if alive == Some(false) {
                fight.stage = Stage::Won;
                fight.won_at = Some(tick);
                game.event(serde_json::json!({"event":"sephiroth_defeated","tick":tick}));
            }
        }
        Stage::Won => {
            // The mission's clear (boss.rs) ends with the signal that starts
            // event 602; without a mission the room waits a moment.
            let over = match game.mission.as_ref() {
                Some(_) => game.mission_cleared,
                None => fight.won_at.is_some_and(|at| tick >= at + OUTRO_DELAY),
            };
            if let (Some(_), Some(file)) = (fight.won_at, game.sephiroth_result.clone())
                && over
            {
                fight.won_at = None;
                match std::fs::write(&file, b"won") {
                    Ok(()) => {
                        exit.write(AppExit::Success);
                    }
                    Err(e) => {
                        game.autoplay_failed = true;
                        game.event(serde_json::json!({"event":"sephiroth_result_failed","reason":e.to_string()}));
                    }
                }
            }
        }
        _ => {}
    }
}
