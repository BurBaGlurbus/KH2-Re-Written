//! `--boss-fight`: a boss fight the launcher starts from the area and programs
//! that place the boss. `area_enemies` puts the program's objects in the room,
//! each boss runs its own object script, and the mission (boss.rs) takes the
//! death signal and plays the clear. This module only ends the room.
use super::*;
use enemy_gallery::EnemyGallery;

/// Ticks after the last boss falls, in an area with no mission to clear.
const OUTRO_DELAY: u64 = 240;

#[derive(Resource, Default)]
pub struct Fight {
    /// A boss stood in the room.
    met: bool,
    down_at: Option<u64>,
    ended: bool,
}

pub fn tick(
    mut game: ResMut<Game>,
    gallery: Res<EnemyGallery>,
    mut fight: ResMut<Fight>,
    mut exit: MessageWriter<AppExit>,
) {
    if !game.boss_fight || fight.ended {
        return;
    }
    let bosses: Vec<_> = gallery
        .actors
        .iter()
        .filter_map(|a| a.battle.as_ref())
        .filter(|b| matches!(&b.brain, enemy_battle::Brain::Script(s) if s.boss()))
        .collect();
    fight.met |= !bosses.is_empty();
    let tick = game.tick;
    if fight.met && fight.down_at.is_none() && bosses.iter().all(|b| !b.brain.alive()) {
        fight.down_at = Some(tick);
        game.event(serde_json::json!({"event":"boss_fight_down","tick":tick}));
    }
    // The mission's clear ends with the signal that hands the area on; with no
    // mission the room waits a moment.
    let over = match game.mission.as_ref() {
        Some(_) => game.mission_cleared,
        None => fight.down_at.is_some_and(|at| tick >= at + OUTRO_DELAY),
    };
    if over || game.mission_failed {
        fight.ended = true;
        let name = if over {
            "boss_fight_won"
        } else {
            "boss_fight_failed"
        };
        game.event(serde_json::json!({"event":name,"tick":tick}));
        exit.write(AppExit::Success);
    }
}
