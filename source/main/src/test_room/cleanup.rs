//! What a long session keeps: a census of everything that can grow (one
//! `census` line in the run's log every few seconds), so a long run shows
//! whether anything grows without bound.
use super::*;
use bevy::ecs::entity::Entities;
use bevy::ecs::system::SystemParam;
use visual::effect_draw::EffectMaterial;

/// Real seconds between census lines.
const CENSUS_EVERY: f64 = 5.;

/// How many of one effect presenter's things are alive.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct FxCount {
    pub running: usize,
    pub spawned: usize,
    pub models: usize,
    pub materials: usize,
    pub pages: usize,
    pub dpds: usize,
    pub batches: usize,
    pub parts: usize,
}

impl FxCount {
    fn json(&self) -> serde_json::Value {
        serde_json::json!({"running":self.running,"spawned":self.spawned,"models":self.models,
            "materials":self.materials,"pages":self.pages,"dpds":self.dpds,
            "batches":self.batches,"parts":self.parts})
    }
}

/// The resources the census reads.
#[derive(SystemParam)]
pub(crate) struct Census<'w> {
    meshes: Res<'w, Assets<Mesh>>,
    images: Res<'w, Assets<Image>>,
    fog: Res<'w, Assets<FogMaterial>>,
    effect_materials: Res<'w, Assets<EffectMaterial>>,
    standard: Res<'w, Assets<StandardMaterial>>,
    drive: Res<'w, drive_fx::DriveFx>,
    spell: Res<'w, spell_fx::SpellFx>,
    effects: Res<'w, Effects>,
    audio: Res<'w, Audio>,
    gallery: Res<'w, enemy_gallery::EnemyGallery>,
    area: Option<Res<'w, AreaRes>>,
    net: Option<Res<'w, duel_net::DuelNet>>,
}

/// One Sora's Drive state, for the census.
fn sora_line(slot: u8, drive: Option<&drive_room::DriveRt>) -> serde_json::Value {
    let Some(d) = drive else {
        return serde_json::json!({"slot":slot});
    };
    serde_json::json!({"slot":slot,"form":d.form,"shown":d.shown,"active":d.active,
        "change":d.change.is_some(),"after":d.afterimage(),"overlay":d.overlay,
        "bullets":d.bullets.len(),"requests":d.fx_requests.len()})
}

/// Writes a `census` line every few seconds of real time.
pub(crate) fn census(
    census: Census,
    entities: &Entities,
    real: Res<Time<Real>>,
    mut game: ResMut<Game>,
    mut next: Local<f64>,
) {
    let now = real.elapsed_secs_f64();
    if now < *next || game.log.is_none() {
        return;
    }
    *next = now + CENSUS_EVERY;
    let (banks, bank_bytes, clips, clip_bytes) = census.audio.banks.census();
    let voices = census
        .audio
        .output
        .as_ref()
        .and_then(|o| o.mixer.lock().ok().map(|m| m.voices()));
    let mut soras = vec![sora_line(game.local_slot, game.drive.as_ref())];
    soras.extend(
        game.rivals
            .iter()
            .map(|r| sora_line(r.slot, r.own.drive.as_ref())),
    );
    let net = census.net.as_deref().map(|n| {
        serde_json::json!({"snapshots":n.snapshots.len(),"events":n.events.len(),
            "inputs":n.inputs.len(),"inbox":n.inbox.len(),"warnings":n.warnings.len()})
    });
    let assets = serde_json::json!({"meshes":census.meshes.len(),"images":census.images.len(),
        "fog_materials":census.fog.len(),"effect_materials":census.effect_materials.len(),
        "standard_materials":census.standard.len()});
    let effects = serde_json::json!({"drive":census.drive.count().json(),
        "owners":census.drive.owners(),"spell":census.spell.count().json(),
        "legacy":census.effects.running.len(),"enemy_paxes":census.effects.enemy_paxes.len(),
        "enemy_banks":census.effects.enemy_banks.len()});
    let sound = serde_json::json!({"banks":banks,"bank_bytes":bank_bytes,"clips":clips,
        "clip_bytes":clip_bytes,"voices":voices});
    let place = serde_json::json!({"actors":census.gallery.actors.len(),
        "actor_garbage":census.gallery.garbage_len(),
        "npc_kinds":census.area.as_ref().map(|a| a.source.kinds.len()),
        "npcs":census.area.as_ref().map(|a| a.current.npcs.len()),
        "world":census.area.as_ref().map(|a| a.current.world),
        "area":census.area.as_ref().map(|a| a.current.area)});
    let maps = serde_json::json!({"victim_hits":game.victim_hits.len(),
        "actor_poses":game.actor_poses.len(),"actor_bones":game.actor_bones.len(),
        "actor_scan":game.actor_scan.len(),"voice_slots":game.enemy_voice_slots.len(),
        "fx_live":game.fx_live.len(),
        "reflected":game.magic.as_ref().map(|m| m.reflected.len()),
        "spells":game.magic.as_ref().map(|m| m.spells.len()),
        "enemy_seen":game.enemy.as_ref().map(|r| r.instance_seen.len()),
        "enemy_previous":game.enemy.as_ref().map(|r| r.previous.len() + r.claw_previous.len())});
    let queues = serde_json::json!({"sound":game.sound_queue.len(),
        "sound_stops":game.sound_stops.len(),"fx":game.fx_queue.len(),
        "styled":game.fx_styled.len(),"calls":game.fx_calls.len(),
        "release":game.fx_release.len(),"effects":game.effect_queue.len(),
        "duel_events":game.duel_events.len(),"party_items":game.party_items.len()});
    let line = serde_json::json!({"event":"census","tick":game.tick,"seconds":now,
        "entities":entities.count_spawned(),"assets":assets,"effects":effects,"sound":sound,
        "place":place,"maps":maps,"queues":queues,"soras":soras,"net":net});
    game.event(line);
}

/// What a cleanup is for.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Scope {
    /// A duel round was decided: the result holds on screen.
    RoundEnd,
    /// The next round's (or the next match's) countdown begins.
    RoundStart,
    /// The match is over, or its session went.
    MatchEnd,
    /// A new area was loaded.
    RoomChange,
}

/// What each scope ends; all of it data, so the table says what a round's
/// end does.
#[derive(Clone, Copy, Debug, PartialEq)]
struct Policy {
    /// The Soras' elements end over this many event frames (0: at once).
    fade: f32,
    /// Forms end on the instance that simulates them; off, a form's own
    /// effects stay on Sora with it.
    forms: bool,
    /// Effects queued but not started are dropped.
    queues: bool,
    /// The battle actors' per-actor maps are forgotten.
    actors: bool,
    /// Caches let go of what nothing else holds.
    caches: bool,
}

const fn policy(scope: Scope) -> Policy {
    match scope {
        Scope::RoundEnd => Policy {
            fade: FIGHTER_FADE,
            forms: true,
            queues: false,
            actors: false,
            caches: false,
        },
        Scope::RoundStart => Policy {
            fade: 0.,
            forms: true,
            queues: true,
            actors: false,
            caches: true,
        },
        Scope::MatchEnd => Policy {
            fade: FIGHTER_FADE,
            forms: true,
            queues: false,
            actors: false,
            caches: true,
        },
        // A form stays through a door (the owner's rule, 2026-10-30); what it
        // left in the old room does not.
        Scope::RoomChange => Policy {
            fade: 0.,
            forms: false,
            queues: true,
            actors: true,
            caches: true,
        },
    }
}

/// A round's leftovers fade quickly (event frames: two ticks each).
const FIGHTER_FADE: f32 = 8.;
/// Looping sounds nothing holds fade over this (seconds).
const LOOP_FADE: f32 = 0.25;
/// The ceiling on one presenter's running elements (the unattended soaks
/// peak near 20) and on the meshes it draws them with (they peak near 15).
pub(crate) const MAX_RUNNING: usize = 512;
pub(crate) const MAX_EFFECT_ENTITIES: usize = 256;
/// The ceiling on a request queue that is emptied every tick the field runs;
/// past it (a field held for long) the oldest requests go.
const MAX_QUEUE: usize = 1024;
/// Per-actor and per-attack maps are forgotten past this many entries.
const MAX_MAP: usize = 4096;
/// Decoded sounds kept past a cleanup (the soaks reach about 40 MiB).
const MAX_CLIP_BYTES: usize = 256 << 20;

/// What a presenter's caches let go of.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct Released {
    pub dpds: usize,
    pub pages: usize,
    pub materials: usize,
    pub models: usize,
}

/// Takes the DPDs only this list still holds out of it; returns their
/// addresses, the caches' keys.
pub(crate) fn stale_dpds(dpds: &mut Vec<Arc<data::object_effect::Dpd>>) -> Vec<usize> {
    let mut stale = Vec::new();
    dpds.retain(|d| {
        let only = Arc::strong_count(d) == 1;
        if only {
            stale.push(Arc::as_ptr(d) as usize);
        }
        !only
    });
    stale
}

/// Past `max` the oldest entries go; returns how many.
fn cap<T>(queue: &mut Vec<T>, max: usize) -> usize {
    let over = queue.len().saturating_sub(max);
    queue.drain(..over);
    over
}

/// Runs `f` on each Sora in turn, his state swapped in.
pub(crate) fn each_sora(game: &mut Game, mut f: impl FnMut(&mut Game)) {
    f(game);
    let mut rivals = std::mem::take(&mut game.rivals);
    for r in &mut rivals {
        rival::swap(game, r);
        f(game);
        rival::swap(game, r);
    }
    game.rivals = rivals;
}

/// The queues' ceilings, every frame; returns how many requests went.
fn cap_queues(game: &mut Game, net: Option<&mut duel_net::DuelNet>) -> usize {
    let mut over = cap(&mut game.sound_queue, MAX_QUEUE)
        + cap(&mut game.sound_stops, MAX_QUEUE)
        + cap(&mut game.fx_queue, MAX_QUEUE)
        + cap(&mut game.fx_styled, MAX_QUEUE)
        + cap(&mut game.fx_calls, MAX_QUEUE)
        + cap(&mut game.fx_release, MAX_QUEUE)
        + cap(&mut game.fx_clear_map, MAX_QUEUE)
        + cap(&mut game.effect_queue, MAX_QUEUE)
        + cap(&mut game.duel_events, MAX_QUEUE)
        + cap(&mut game.party_items, MAX_QUEUE);
    if let Some(net) = net {
        while net.events.len() > MAX_QUEUE {
            net.events.pop_front();
            over += 1;
        }
        while net.warnings.len() > MAX_QUEUE {
            net.warnings.pop_front();
            over += 1;
        }
        while net.inputs.len() > MAX_QUEUE {
            net.inputs.pop_first();
            over += 1;
        }
    }
    if let Some(rt) = game.enemy.as_mut()
        && rt.previous.len() + rt.claw_previous.len() > MAX_MAP
    {
        rt.previous.clear();
        rt.claw_previous.clear();
    }
    if game.victim_hits.len() > MAX_MAP {
        game.victim_hits.clear();
    }
    over
}

/// The central pass: the ceilings every frame, then the cleanups asked for,
/// one `cleanup` line each.
#[allow(clippy::too_many_arguments)]
pub(crate) fn run(
    mut game: ResMut<Game>,
    mut drive: ResMut<drive_fx::DriveFx>,
    mut spell: ResMut<spell_fx::SpellFx>,
    mut legacy: ResMut<Effects>,
    mut audio: ResMut<Audio>,
    mut net: Option<ResMut<duel_net::DuelNet>>,
    mut evicted: Local<usize>,
) {
    let game = &mut *game;
    *evicted += cap_queues(game, net.as_deref_mut());
    *evicted += cap(&mut legacy.running, MAX_RUNNING);
    if game.cleanups.is_empty() {
        return;
    }
    for scope in std::mem::take(&mut game.cleanups) {
        let started = std::time::Instant::now();
        let plan = policy(scope);
        let mut forms = 0;
        if !game.net_client {
            each_sora(game, |g| {
                if plan.forms {
                    forms +=
                        usize::from(g.drive.as_ref().is_some_and(|d| d.shown != 0 || d.active));
                    drive_room::end_form(g);
                } else if let Some(d) = g.drive.as_mut() {
                    d.end_shots();
                }
                if let Some(m) = g.magic.as_mut() {
                    m.end_spells();
                }
            });
        }
        // The kit whose elements stay bound to Sora through a door.
        let kept = (!plan.forms)
            .then(|| game.drive.as_ref().map(|d| d.shown))
            .flatten()
            .filter(|&shown| shown != 0);
        let mut stops = std::mem::take(&mut game.sound_stops);
        let drive_fx = drive.end_fighters(plan.fade, &mut stops, kept);
        game.sound_stops = stops;
        let spell_fx = spell.end_all(plan.fade);
        let legacy_fx = legacy.running.len();
        for r in &mut legacy.running {
            r.instance.fade_out(plan.fade);
        }
        let held = drive.held_sounds();
        let loops = audio.output.as_ref().map_or(0, |o| {
            o.mixer.lock().map_or(0, |mut m| {
                m.stop_loops(LOOP_FADE, |key| {
                    MUSIC_KEYS.contains(&key)
                        || key == duel_music::KEY
                        || key & AMBIENT_KEY != 0
                        || key == SE_LOW_HP
                        || held.contains(&key)
                })
            })
        });
        let mut dropped = 0;
        if plan.queues {
            dropped += game.fx_queue.len()
                + game.fx_styled.len()
                + game.fx_calls.len()
                + game.effect_queue.len();
            game.fx_queue.clear();
            game.fx_styled.clear();
            game.fx_calls.clear();
            game.effect_queue.clear();
        }
        if plan.actors {
            game.actor_poses.clear();
            game.actor_bones.clear();
            game.actor_scan.clear();
            game.victim_hits.clear();
            game.enemy_voice_slots.clear();
        }
        let (mut caches, mut clips) = (Released::default(), 0);
        if plan.caches {
            let (a, b) = (drive.release_unused(), spell.release_unused());
            caches = Released {
                dpds: a.dpds + b.dpds,
                pages: a.pages + b.pages,
                materials: a.materials + b.materials,
                models: a.models + b.models,
            };
            clips = audio.banks.trim_clips(MAX_CLIP_BYTES);
        }
        let ms = started.elapsed().as_secs_f64() * 1e3;
        info!(
            "Cleanup {scope:?}: {forms} forms, {drive_fx}+{spell_fx}+{legacy_fx} effects, \
             {loops} loops, {dropped} queued, {} containers, {} pages, {} materials, {} models, \
             {clips} clips, {} over the ceilings ({ms:.2} ms)",
            caches.dpds, caches.pages, caches.materials, caches.models, *evicted
        );
        let tick = game.tick;
        let line = serde_json::json!({"event":"cleanup","tick":tick,"scope":format!("{scope:?}"),
            "forms":forms,"drive_fx":drive_fx,"spell_fx":spell_fx,"legacy_fx":legacy_fx,
            "loops":loops,"queued":dropped,"dpds":caches.dpds,"pages":caches.pages,
            "materials":caches.materials,"models":caches.models,"clips":clips,
            "over_ceiling":*evicted,"ms":ms});
        game.event(line);
        *evicted = 0;
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use data::object_effect::{Dpd, read_dpd};

    fn put(b: &mut [u8], at: usize, v: u32) {
        b[at..at + 4].copy_from_slice(&v.to_le_bytes());
    }

    /// A synthetic container: one PDATA whose instance runs for ages unless
    /// something ends it.
    pub(crate) fn effect_dpd() -> Arc<Dpd> {
        let mut b = vec![0u8; 0x300];
        put(&mut b, 0, 0x96);
        put(&mut b, 4, 1);
        put(&mut b, 8, 0x40);
        let block = 0x40 + 0x110;
        put(&mut b, block, 0x7000_0000);
        put(&mut b, block + 0xc, 0x120);
        put(&mut b, block + 0x120, 1);
        put(&mut b, block + 0x124, 0x10);
        let e = block + 0x10;
        put(&mut b, e + 0x14, 0x3000);
        put(&mut b, e + 0x18, 0x7000_0000);
        put(&mut b, e + 0x1c, 0x7000_0000);
        put(&mut b, e + 0x20, 0xa0 + 32);
        b[e + 0x26] = 1;
        put(&mut b, e + 0x28, 36);
        b[e + 0x2c] = 16;
        b[e + 0x2e] = 1;
        put(&mut b, e + 0x30, 0x80);
        put(&mut b, e + 0x34, 0x94);
        put(&mut b, block + 0x84, 1f32.to_bits());
        put(&mut b, block + 0x88, 2f32.to_bits());
        put(&mut b, block + 0x8c, 3f32.to_bits());
        put(&mut b, block + 0x90, u32::MAX);
        put(&mut b, block + 0x94, 0);
        put(&mut b, block + 0x98, 16);
        Arc::new(read_dpd(&b).unwrap())
    }

    #[test]
    fn a_round_and_a_match_end_forms_a_door_does_not() {
        for scope in [Scope::RoundEnd, Scope::RoundStart, Scope::MatchEnd] {
            assert!(policy(scope).forms, "{scope:?}");
        }
        // A form goes through a door with Sora.
        assert!(!policy(Scope::RoomChange).forms);
        assert!(!policy(Scope::RoundEnd).caches);
        assert!(policy(Scope::RoomChange).actors);
        assert_eq!(policy(Scope::RoundStart).fade, 0.);
    }

    #[test]
    fn the_queue_ceiling_takes_the_oldest() {
        let mut q = vec![1, 2, 3, 4];
        assert_eq!(cap(&mut q, 2), 2);
        assert_eq!(q, [3, 4]);
        assert_eq!(cap(&mut q, 2), 0);
    }

    #[test]
    fn only_a_container_nothing_else_holds_is_stale() {
        let held = effect_dpd();
        let mut list = vec![held.clone(), effect_dpd()];
        let gone = Arc::as_ptr(&list[1]) as usize;
        assert_eq!(stale_dpds(&mut list), [gone]);
        assert_eq!(list.len(), 1);
    }
}
