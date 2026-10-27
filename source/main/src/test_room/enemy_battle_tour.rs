//! Bounded debug-room fight exercise. It checks actual damage dispatch and
//! source-driven action progress; it does not establish installed equivalence.
use super::*;

pub const END: u64 = 1800;
pub const CAPTURES: &[u64] = &[100, 160, 200, 250, 320, 600, 800, 1100, 1400, 1800];
const DEFAULT_MODELS: &[&str] = &["M_EX020", "M_EX990", "M_EX010"];

/// `KH2RW_TOUR_ROSTER=1`: every enemy that can fight, one after another
/// (`KH2RW_TOUR_MODELS` narrows the list). Each gets [`ROSTER_SLOT`] ticks:
/// summoned 250 in front of Sora (healed below a third), its AI on, Sora still until
/// [`ROSTER_ATTACK`], then attacking every 20 ticks, a summary at the end.
pub fn roster() -> bool {
    std::env::var_os("KH2RW_TOUR_ROSTER").is_some()
}
const ROSTER_SLOT: u64 = 720;
const ROSTER_ENABLE: u64 = 10;
const ROSTER_ATTACK: u64 = 390;
const ROSTER_SUMMARY: u64 = ROSTER_SLOT - 10;
/// The tour ends itself when the roster is done.
pub fn end() -> u64 {
    if roster() { u64::MAX / 2 } else { END }
}
pub fn captures() -> Vec<u64> {
    if roster() {
        Vec::new()
    } else {
        CAPTURES.to_vec()
    }
}
/// Sora's Attack press (the plain tour: every 20 ticks from 700 to 1300).
pub fn attack_pressed(tick: u64) -> bool {
    if roster() {
        let start = ROSTER_START.load(std::sync::atomic::Ordering::Relaxed);
        tick >= start + ROSTER_ATTACK && tick.is_multiple_of(20)
    } else {
        (700..1300).contains(&tick) && tick.is_multiple_of(20)
    }
}

/// The tick the running roster slot started (for Sora's presses).
static ROSTER_START: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(u64::MAX / 4);

/// What one roster slot saw.
#[derive(Default)]
struct Slot {
    entry: usize,
    start: u64,
    /// The AI was asked for; the summary was written.
    enabling: bool,
    summarized: bool,
    acts: Vec<String>,
    motions: std::collections::BTreeSet<usize>,
    incoming: u64,
    outgoing: u64,
    orbs: usize,
    spawn: Option<[f32; 3]>,
    travelled: f32,
    closest: f32,
    died: Option<u64>,
    fault: Option<String>,
}

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
    /// The lock before the tour's cycle press, and whether the press moved
    /// it to another actor while it stayed locked.
    before_cycle: Option<enemy_target::TargetId>,
    cycled: bool,
    /// The roster's catalog entries still to run, and the one running.
    queue: Option<std::collections::VecDeque<usize>>,
    slot: Option<Slot>,
    ran: u32,
    clean: u32,
}

/// One tick of the roster (see [`roster`]).
fn roster_tick(game: &mut Game, gallery: &mut enemy_gallery::EnemyGallery, checks: &mut Checks) {
    let tick = game.tick;
    if tick < 20 {
        return;
    }
    // Sora takes the hits (so they show) but is topped up before he falls.
    let v = &mut game.sora.vitals;
    if v.hp * 3 < v.target.max_hp {
        v.hp = v.target.max_hp;
    }
    let queue = checks.queue.get_or_insert_with(|| {
        let only: Vec<String> = std::env::var("KH2RW_TOUR_MODELS")
            .map(|l| l.split(',').map(str::to_owned).collect())
            .unwrap_or_default();
        gallery
            .catalog
            .entries
            .iter()
            .enumerate()
            .filter(|(_, e)| {
                e.model_available
                    && e.motions_available
                    && enemy_battle::supported(e)
                    && e.object_id != enemy_battle::SEPHIROTH
                    && (only.is_empty() || only.contains(&e.model_name))
            })
            .map(|(k, _)| k)
            .collect()
    });
    // The field can stand still for a tick or more (a menu, a load), so the
    // steps go by the ticks since the slot started.
    if checks
        .slot
        .as_ref()
        .is_none_or(|s| s.summarized && tick >= s.start + ROSTER_SLOT)
    {
        gallery.clear();
        let Some(entry) = queue.pop_front() else {
            let (ran, clean) = (checks.ran, checks.clean);
            game.event(serde_json::json!({"event":"enemy_roster_done","tick":tick,
                "ran":ran,"clean":clean,"installed_comparison":"NOT RUN"}));
            game.autoplay_end = tick;
            return;
        };
        let at = Vec3::from_array(game.sora.player.position) + Vec3::new(0., 0., 250.);
        gallery.queue_summon(entry, at, std::f32::consts::PI);
        ROSTER_START.store(tick, std::sync::atomic::Ordering::Relaxed);
        checks.slot = Some(Slot {
            entry,
            start: tick,
            closest: f32::MAX,
            orbs: game.enemy.as_ref().map_or(0, |rt| rt.orbs.len()),
            incoming: game.enemy_incoming_hits,
            outgoing: game.enemy_outgoing_hits,
            ..Default::default()
        });
        return;
    }
    let Some(slot) = checks.slot.as_mut() else {
        return;
    };
    let elapsed = tick - slot.start;
    if !slot.enabling
        && elapsed >= ROSTER_ENABLE
        && let Some(id) = gallery.actors.first().map(|a| a.id)
    {
        slot.enabling = true;
        gallery.select(id);
        gallery.queue_battle();
    }
    let sora = game.sora.player.position;
    if let Some(actor) = gallery.actors.first() {
        if slot.fault.is_none() {
            slot.fault = actor
                .error
                .clone()
                .or_else(|| actor.battle.as_ref().and_then(|b| b.error.clone()));
        }
        if let Some(b) = actor.battle.as_ref() {
            let p = b.brain.position();
            let spawn = *slot.spawn.get_or_insert(p);
            slot.travelled = slot
                .travelled
                .max(((p[0] - spawn[0]).powi(2) + (p[2] - spawn[2]).powi(2)).sqrt());
            slot.closest = slot
                .closest
                .min(((p[0] - sora[0]).powi(2) + (p[2] - sora[2]).powi(2)).sqrt());
            let act = b.brain.description();
            if slot.acts.last() != Some(&act) && slot.acts.len() < 40 {
                slot.acts.push(act);
            }
            slot.motions.insert(b.brain.motion());
            if slot.died.is_none() && !b.brain.alive() {
                slot.died = Some(tick - slot.start);
            }
        }
    }
    if !slot.summarized && elapsed >= ROSTER_SUMMARY {
        slot.summarized = true;
        let e = &gallery.catalog.entries[slot.entry];
        let actor = gallery.actors.first();
        let (unsupported, services): (Vec<_>, Vec<_>) = actor
            .and_then(|a| a.battle.as_ref())
            .and_then(|b| match &b.brain {
                enemy_battle::Brain::Script(s) => Some((
                    s.unsupported.iter().copied().collect(),
                    s.services.iter().copied().collect(),
                )),
                _ => None,
            })
            .unwrap_or_default();
        let enabled = actor.is_some_and(|a| a.battle.is_some());
        if !enabled && slot.fault.is_none() {
            // The summon or the AI's load failed; the gallery says why.
            slot.fault = Some(gallery.status.clone());
        }
        let incoming = game.enemy_incoming_hits - slot.incoming;
        let outgoing = game.enemy_outgoing_hits - slot.outgoing;
        let orbs = game
            .enemy
            .as_ref()
            .map_or(0, |rt| rt.orbs.len())
            .saturating_sub(slot.orbs);
        checks.ran += 1;
        let clean = enabled && slot.fault.is_none();
        checks.clean += u32::from(clean);
        game.event(serde_json::json!({"event":"enemy_roster_slot","tick":tick,
            "model":e.model_name,"object":e.object_id,"name":e.name,"type":e.object_type,
            "enabled":enabled,"fault":slot.fault,"acts":slot.acts,
            "motions":slot.motions,"hits_on_sora":incoming,"hits_on_enemy":outgoing,
            "orbs":orbs,"died_after":slot.died,"travelled":slot.travelled,
            "closest_to_sora":slot.closest,"unsupported":unsupported,"services":services}));
    }
}

/// The lock is engaged at 150 and toggled off at 180; the cycle presses
/// land between: right, then left if right found no other actor.
const CYCLE: u64 = 165;
const CYCLE_AGAIN: u64 = 170;
/// Logical bits of the cycle presses (`enemy_target::cycle_direction`).
const CYCLE_LEFT: u64 = 1 << 36;
const CYCLE_RIGHT: u64 = 1 << 37;

pub fn tick(
    mut game: ResMut<Game>,
    mut gallery: ResMut<enemy_gallery::EnemyGallery>,
    mut debug: ResMut<debug_menu::DebugMenu>,
    mut camp: Option<ResMut<CampRes>>,
    mut checks: ResMut<Checks>,
    mut targeting: ResMut<enemy_target::Targeting>,
) {
    if !game.enemy_battle_tour {
        return;
    }
    if roster() {
        roster_tick(&mut game, &mut gallery, &mut checks);
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
                let at = Vec3::from_array(game.sora.player.position) + offset;
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
    // Switching the lock between the actors (cycle right).
    if tick == CYCLE {
        checks.before_cycle = targeting.holder.target;
        targeting.queue_cycle(CYCLE_RIGHT);
        game.cycle_queued = true;
    }
    if tick == CYCLE_AGAIN && !checks.cycled {
        targeting.queue_cycle(CYCLE_LEFT);
        game.cycle_queued = true;
    }
    if [CYCLE + 1, CYCLE_AGAIN + 1].contains(&tick) {
        let locked = game.target_locked;
        game.event(
            serde_json::json!({"event":"enemy_combat_tour_cycle","tick":tick,
            "target":format!("{:?}",targeting.holder.target),"locked":locked,
            "limitation":targeting.limitation}),
        );
    }
    if (CYCLE + 1..180).contains(&tick)
        && game.target_locked
        && targeting.holder.target.is_some()
        && checks.before_cycle.is_some()
        && targeting.holder.target != checks.before_cycle
    {
        checks.cycled = true;
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
        let scan_enabled = game.sora.player.abilities.has(enemy_target::SCAN_BIT);
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
            && checks.lock_released
            && checks.cycled;
        game.autoplay_failed |= !pass;
        let incoming = game.enemy_incoming_hits;
        let outgoing = game.enemy_outgoing_hits;
        game.event(serde_json::json!({"event":"enemy_combat_tour_verdict",
            "tick":tick,"pass":pass,"positive_damage_to_player":incoming,
            "positive_damage_to_enemies":outgoing,
            "scan_hidden":checks.scan_hidden,"scan_locked":checks.scan_locked,
            "lock_released":checks.lock_released,"lock_cycled":checks.cycled,
            "boundary":"three ordinary actors in an authored debug arena",
            "installed_comparison":"NOT RUN"}));
        debug.show_enemy_actors();
    }
}
