//! Bounded debug-room fight exercise. It checks actual damage dispatch and
//! source-driven action progress; it does not establish installed equivalence.
use super::*;

pub const END: u64 = 1800;
pub const CAPTURES: &[u64] = &[100, 160, 200, 250, 320, 600, 800, 1100, 1400, 1800];
const DEFAULT_MODELS: &[&str] = &["M_EX020", "M_EX990", "M_EX010"];

/// The models the tour summons: `KH2RW_TOUR_MODELS=N_HB630` (comma
/// separated) replaces the three ordinary enemies, for one-boss runs.
fn models() -> Vec<String> {
    match std::env::var("KH2RW_TOUR_MODELS") {
        Ok(list) if !list.is_empty() => list.split(',').map(str::to_owned).collect(),
        _ => DEFAULT_MODELS.iter().map(|m| (*m).to_owned()).collect(),
    }
}

#[derive(Resource, Default)]
pub struct Checks {
    scan_hidden: bool,
    scan_locked: bool,
    lock_released: bool,
}

pub fn tick(
    mut game: ResMut<Game>,
    mut gallery: ResMut<enemy_gallery::EnemyGallery>,
    mut debug: ResMut<debug_menu::DebugMenu>,
    mut camp: Option<ResMut<CampRes>>,
    mut checks: ResMut<Checks>,
) {
    if !game.enemy_battle_tour {
        return;
    }
    let tick = game.tick;
    if tick == 5 {
        for (model, offset) in models().iter().zip([
            Vec3::new(-80., 0., 150.),
            Vec3::new(80., 0., 180.),
            Vec3::new(150., 0., 250.),
        ]) {
            if let Some(entry) = gallery.catalog.entries.iter().position(|e| {
                e.model_name == model.as_str() && e.model_available && e.motions_available
            }) {
                let at = Vec3::from_array(game.player.position) + offset;
                gallery.queue_summon(entry, at, std::f32::consts::PI);
                game.event(serde_json::json!({"event":"enemy_combat_tour_summon",
                    "tick":tick,"model":model,"debug_position":at.to_array(),
                    "authored_debug_placement":true}));
            } else {
                game.autoplay_failed = true;
                game.event(
                    serde_json::json!({"event":"enemy_combat_tour_missing_model",
                    "tick":tick,"model":model}),
                );
            }
        }
    }
    if tick == 60 {
        if let Err(error) = enemy_target::set_scan(&mut game, camp.as_deref_mut(), true) {
            game.autoplay_failed = true;
            game.event(
                serde_json::json!({"event":"combat_tour_scan_failed","tick":tick,"reason":error}),
            );
        }
        let ids: Vec<_> = gallery.actors.iter().map(|actor| actor.id).collect();
        for id in ids {
            gallery.select(id);
            gallery.queue_battle();
            game.event(serde_json::json!({"event":"enemy_combat_tour_enable",
                "tick":tick,"actor":id}));
        }
    }
    if [150, 180].contains(&tick) {
        game.lock_queued = true;
    }
    if [80, 140].contains(&tick) {
        let enabled = tick == 140;
        if let Err(error) = enemy_target::set_scan(&mut game, camp.as_deref_mut(), enabled) {
            game.autoplay_failed = true;
            game.event(
                serde_json::json!({"event":"combat_tour_scan_failed","tick":tick,"reason":error}),
            );
        }
        game.event(
            serde_json::json!({"event":"combat_tour_scan_toggle","tick":tick,"enabled":enabled}),
        );
    }
    if [100, 160, 300, 600, 800, 1100, 1400, END].contains(&tick) {
        let actors = gallery
            .actors
            .iter()
            .map(|actor| {
                let battle = actor.battle.as_ref().map(|battle| {
                    serde_json::json!({"hp":battle.brain.hp(),
                        "alive":battle.brain.alive(),"targetable":battle.brain.targetable(),
                        "action":battle.brain.description(),"motion":battle.brain.motion(),
                        "serial":battle.brain.serial(),"error":battle.error})
                });
                serde_json::json!({"actor":actor.id,
                    "model":gallery.catalog.entries[actor.entry].model_name,
                    "battle_enabled":actor.battle.is_some(),
                    "motion_choice":actor.motion_choice,"frame":actor.frame,
                    "position":actor.position.to_array(),"error":actor.error,"battle":battle})
            })
            .collect::<Vec<_>>();
        let incoming = game.enemy_incoming_hits;
        let outgoing = game.enemy_outgoing_hits;
        let scan_enabled = game.player.abilities.has(enemy_target::SCAN_BIT);
        let locked = game.target_locked;
        let target = game.selected_target;
        match tick {
            100 => checks.scan_hidden = !scan_enabled,
            160 => checks.scan_locked = scan_enabled && locked && target.is_some(),
            600 => checks.lock_released = !locked,
            _ => {}
        }
        game.event(serde_json::json!({"event":"enemy_combat_tour_checkpoint",
            "tick":tick,"actors":actors,"positive_damage_to_player":incoming,
            "positive_damage_to_enemies":outgoing,
            "scan_enabled":scan_enabled,
            "locked":locked,"target":target,
            "installed_comparison":"NOT RUN"}));
    }
    if tick == END {
        // This finite exercise requires both directions of actual damage.
        // It does not pass an AI, timing, enemy inventory or gameplay gate.
        let models = models();
        let pass = gallery.actors.len() == models.len()
            && models.iter().all(|model| {
                gallery.actors.iter().any(|actor| {
                    gallery.catalog.entries[actor.entry].model_name == model.as_str()
                        && actor.battle.is_some()
                        && actor.error.is_none()
                        && actor
                            .battle
                            .as_ref()
                            .is_some_and(|battle| battle.error.is_none())
                })
            })
            && game.enemy_incoming_hits > 0
            && game.enemy_outgoing_hits > 0
            && checks.scan_hidden
            && checks.scan_locked
            && checks.lock_released;
        game.autoplay_failed |= !pass;
        let incoming = game.enemy_incoming_hits;
        let outgoing = game.enemy_outgoing_hits;
        game.event(serde_json::json!({"event":"enemy_combat_tour_verdict",
            "tick":tick,"pass":pass,"positive_damage_to_player":incoming,
            "positive_damage_to_enemies":outgoing,
            "scan_hidden":checks.scan_hidden,"scan_locked":checks.scan_locked,
            "lock_released":checks.lock_released,
            "boundary":"three ordinary actors in an authored debug arena",
            "installed_comparison":"NOT RUN"}));
        debug.show_enemy_actors();
    }
}
