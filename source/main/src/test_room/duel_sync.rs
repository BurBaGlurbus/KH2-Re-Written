//! The duel's state over the session, once per fixed tick. The host takes
//! the joiner's buttons for its Sora, then sends a snapshot of both Soras
//! and the tick's events; the joiner shows the snapshots a couple of ticks
//! behind (to ride out jitter) and plays the events' sounds and sparks
//! itself. Nothing cosmetic is replicated.

use super::duel_net::{DuelNet, fighter_slots, warn};
use super::duel_wire::{self, DuelEvent, FighterState, Snapshot};
use super::rival::{Command, RivalPad};
use super::*;
use net::{RELIABLE, Role, UNRELIABLE};

/// Joiner ticks of buttons the host lets wait before it catches up.
const INPUT_BACKLOG: usize = 4;
/// Snapshots the joiner holds back, and the most it lets pile up.
const SNAPSHOT_DELAY: usize = 2;
const SNAPSHOT_BACKLOG: usize = 8;

/// The buttons that only last one tick are kept when ticks are merged.
fn merge(older: RivalPad, newer: RivalPad) -> RivalPad {
    RivalPad {
        jump: older.jump || newer.jump,
        attack: older.attack || newer.attack,
        guard: older.guard || newer.guard,
        flow: older.flow || newer.flow,
        // A menu command is never dropped either.
        command: match newer.command {
            Command::None => older.command,
            c => c,
        },
        ..newer
    }
}

/// Without new buttons the stick and the held buttons stay; presses do not
/// repeat.
fn held(pad: RivalPad) -> RivalPad {
    RivalPad {
        jump: false,
        attack: false,
        guard: false,
        flow: false,
        command: Command::None,
        ..pad
    }
}

/// The joiner's buttons for this host tick: the oldest unused one, older
/// ones merged in when too many wait.
pub(crate) fn next_input(
    inputs: &mut std::collections::BTreeMap<u32, RivalPad>,
) -> Option<(u32, RivalPad)> {
    while inputs.len() > INPUT_BACKLOG {
        let (_, old) = inputs.pop_first()?;
        if let Some(mut next) = inputs.first_entry() {
            let merged = merge(old, *next.get());
            *next.get_mut() = merged;
        }
    }
    inputs.pop_first()
}

/// Host, before the rivals move: the joiner's buttons for its Sora.
pub(crate) fn host_take_inputs(net: Option<ResMut<DuelNet>>, mut game: ResMut<Game>) {
    let Some(mut net) = net else { return };
    if net.role != Role::Host || !net.playing() {
        return;
    }
    while let Some(m) = net.session.receive(UNRELIABLE) {
        let Some(list) = duel_wire::decode_input(&m) else {
            continue;
        };
        for (tick, pad) in list {
            if net.used_input.is_none_or(|used| tick > used) {
                net.inputs.entry(tick).or_insert(pad);
            }
        }
    }
    let slot = fighter_slots::JOIN;
    let Some(rival) = game.rivals.iter_mut().find(|r| r.slot == slot) else {
        return;
    };
    let last = rival.pad;
    let before = net.inputs.len();
    match next_input(&mut net.inputs) {
        Some((tick, pad)) => {
            if before > INPUT_BACKLOG + 1 {
                net.skipped += (before - INPUT_BACKLOG - 1) as u64;
            }
            net.used_input = Some(tick);
            rival.pad = pad;
            net.starve_run = 0;
        }
        None => {
            rival.pad = held(last);
            net.starved += 1;
            net.starve_run += 1;
            if net.starve_run == LATE_WARNING {
                let tick = game.tick;
                warn(
                    &mut net,
                    format!("Tick {tick}: no buttons from the other player for half a second"),
                );
            }
        }
    }
}

/// Ticks without new buttons or snapshots before the overlay warns.
const LATE_WARNING: u32 = 30;

fn state_of(
    slot: u8,
    fighter: &Fighter,
    kh3: bool,
    started: u64,
    drive: Option<&drive_room::DriveRt>,
    items: Option<&item_room::ItemRt>,
    book: Option<&player::kh3::attack::Book>,
) -> FighterState {
    let (player, vitals) = (&fighter.player, &fighter.vitals);
    FighterState {
        slot,
        position: player.position,
        yaw: player.yaw,
        action: player.action,
        frame: player.frame,
        armed: player.armed,
        blend: player.blend,
        started,
        hp: vitals.hp,
        max_hp: vitals.target.max_hp,
        mp: [player.mp.cur, player.mp.max],
        drive: [
            player.drive.level,
            player.drive.partial,
            player.drive.max,
            player.drive.bars,
        ],
        invulnerable: vitals.invulnerable,
        form: drive.map_or(0, |d| d.shown),
        items: items.map(|i| i.slots.clone()).unwrap_or_default(),
        bullets: drive
            .map(|d| d.bullets.iter().map(|b| (b.key, b.pos, b.dir)).collect())
            .unwrap_or_default(),
        kh3: kh3.then(|| match book {
            Some(book) => fighter.kh3.shown_in(book),
            None => fighter.kh3.shown(),
        }),
    }
}

/// Host, after the field's tick: both Soras and the tick's events.
pub(crate) fn host_send(net: Option<ResMut<DuelNet>>, mut game: ResMut<Game>) {
    // Out of a LAN duel nobody reads the events.
    let events = std::mem::take(&mut game.duel_events);
    let Some(mut net) = net else { return };
    if net.role != Role::Host || !net.playing() {
        return;
    }
    // The rules, whenever the joiner does not have the current ones.
    if net.sent_rules != Some(game.duel_rules) {
        net.sent_rules = Some(game.duel_rules);
        net.session
            .send(RELIABLE, duel_wire::encode_rules(&game.duel_rules));
    }
    let kh3 = game.duel_rules.kh3();
    let book = game.kh3_book.clone();
    let book = book.as_deref();
    let mut fighters = vec![state_of(
        fighter_slots::HOST,
        &game.sora,
        kh3,
        game.action_started,
        game.drive.as_ref(),
        game.items.as_ref(),
        book,
    )];
    fighters.extend(game.rivals.iter().map(|r| {
        state_of(
            r.slot,
            &r.fighter,
            kh3,
            r.started,
            r.own.drive.as_ref(),
            r.own.items.as_ref(),
            book,
        )
    }));
    let snapshot = Snapshot {
        tick: game.tick,
        fighters,
    };
    net.session
        .send(UNRELIABLE, duel_wire::encode_snapshot(&snapshot));
    if !events.is_empty() {
        net.session
            .send(RELIABLE, duel_wire::encode_events(game.tick, &events));
    }
}

/// Puts a snapshot's numbers on a Sora. Returns whether a new action began
/// (the presenter restarts its blend).
fn apply_state(
    s: &FighterState,
    fighter: &mut Fighter,
    book: Option<&player::kh3::attack::Book>,
) -> bool {
    if let Some(k) = &s.kh3 {
        fighter.kh3.apply_shown(k);
        if let Some(book) = book {
            fighter.kh3.apply_actors(k, book);
        }
    }
    let (player, vitals) = (&mut fighter.player, &mut fighter.vitals);
    let new_action = player.action != s.action || player.frame > s.frame;
    player.position = s.position;
    player.yaw = s.yaw;
    player.action = s.action;
    player.frame = s.frame;
    player.armed = s.armed;
    player.blend = s.blend;
    player.mp.cur = s.mp[0];
    player.mp.max = s.mp[1];
    [
        player.drive.level,
        player.drive.partial,
        player.drive.max,
        player.drive.bars,
    ] = s.drive;
    vitals.hp = s.hp;
    vitals.target.max_hp = s.max_hp;
    vitals.invulnerable = s.invulnerable;
    new_action
}

/// What the joiner does with the host's events: the same sounds and
/// sparks it would have played itself.
fn play_event(game: &mut Game, event: &DuelEvent) {
    match *event {
        DuelEvent::Sound(id) => game.sound_queue.push((id, "duel")),
        DuelEvent::Spark {
            effect,
            random,
            contact,
            direction,
            own,
        } => game.effect_queue.push(EffectRequest::Hit {
            effect,
            random,
            contact: Vec3::from_array(contact),
            direction: Vec3::from_array(direction),
            own,
        }),
        DuelEvent::Guard(at, flash) => game.effect_queue.push(EffectRequest::System {
            id: flash,
            at: Vec3::from_array(at),
        }),
        DuelEvent::KnockOut(slot) => {
            let tick = game.tick;
            game.event(serde_json::json!({"event":"duel_knock_out","tick":tick,"slot":slot}));
        }
        DuelEvent::StoodUp(slot) => {
            let tick = game.tick;
            game.event(serde_json::json!({"event":"duel_stood_up","tick":tick,"slot":slot}));
        }
        DuelEvent::Spell { .. } | DuelEvent::DriveFx { .. } => duel_fx::play(game, event),
        DuelEvent::Look {
            look,
            slot,
            size,
            at,
        } => duel_kh3::play_look(game, look, slot, size, at),
        DuelEvent::Kh3Fx {
            effect,
            sound,
            at,
            yaw,
            attach,
        } => {
            let name = |id: u16| {
                game.kh3_book
                    .as_ref()
                    .and_then(|b| b.names.get(usize::from(id)).cloned())
            };
            let request = player::kh3::spell::FxRequest {
                effect: name(effect),
                sound: name(sound),
                at,
                yaw,
                attach: attach.map(|a| player::kh3::spell::FxAttach {
                    seat: a.seat,
                    bone: name(a.bone),
                    offset: a.offset,
                    handle: a.handle,
                    group: a.group,
                    stop: match a.stop {
                        0 => None,
                        1 => Some(false),
                        _ => Some(true),
                    },
                }),
            };
            game.kh3_fx_remote.push(request);
        }
    }
}

/// Joiner, before its field tick: the host's snapshots and events.
pub(crate) fn join_apply(net: Option<ResMut<DuelNet>>, mut game: ResMut<Game>) {
    let Some(mut net) = net else { return };
    if net.role != Role::Join || !net.playing() {
        return;
    }
    while let Some(m) = net.session.receive(UNRELIABLE) {
        if let Some(s) = duel_wire::decode_snapshot(&m)
            && net.shown_snapshot.is_none_or(|shown| s.tick > shown)
        {
            net.snapshots.insert(s.tick, s);
        }
    }
    for m in std::mem::take(&mut net.inbox) {
        if let Some((tick, events)) = duel_wire::decode_events(&m) {
            // Played with the snapshot of the tick they happened on.
            net.events.push_back((tick, events));
        } else if let Some(rules) = duel_wire::decode_rules(&m) {
            game.duel_rules = rules;
        } else {
            net.inbox.push(m);
        }
    }
    if net.snapshots.len() > SNAPSHOT_BACKLOG {
        let skip = net.snapshots.len() - SNAPSHOT_DELAY;
        for _ in 0..skip {
            net.snapshots.pop_first();
        }
        net.skipped += skip as u64;
        let tick = game.tick;
        warn(
            &mut net,
            format!("Tick {tick}: skipped {skip} snapshots to catch up"),
        );
    }
    let due = net.snapshots.len() > SNAPSHOT_DELAY
        || (net.shown_snapshot.is_none() && !net.snapshots.is_empty());
    let Some((tick, snapshot)) = due.then(|| net.snapshots.pop_first()).flatten() else {
        net.stale += 1;
        net.stale_run += 1;
        if net.stale_run == LATE_WARNING {
            let tick = game.tick;
            warn(
                &mut net,
                format!("Tick {tick}: no snapshot from the host for half a second"),
            );
        }
        // Without snapshots the events still come, a little late.
        if net.stale_run >= LATE_WARNING {
            for (_, events) in std::mem::take(&mut net.events) {
                for e in &events {
                    play_event(&mut game, e);
                }
            }
        }
        return;
    };
    net.stale_run = 0;
    net.shown_snapshot = Some(tick);
    let now = game.tick;
    let game = &mut *game;
    // Every form's kit loads ahead, for the host's changes to show at once;
    // a new Keyblade (the lobby's builds) replaces the old once loaded.
    if let Some(d) = game.drive.as_mut() {
        d.preload();
    }
    drive_room::rebuild_weapons(game);
    // [KH3 ruleset] Wisdom's container is named by the host's KH3 cues
    // (the airstep's afterimage, the bolts' flash and sparks): ready on
    // both Soras before an event names it.
    let kh3 = game.duel_rules.kh3();
    if kh3 && let Some(d) = game.drive.as_mut() {
        d.wisdom_pax();
    }
    let mut rivals = std::mem::take(&mut game.rivals);
    for r in &mut rivals {
        if let Some(d) = r.own.drive.as_mut() {
            d.preload();
            if kh3 {
                d.wisdom_pax();
            }
        }
        rival::swap(game, r);
        drive_room::rebuild_weapons(game);
        rival::swap(game, r);
    }
    game.rivals = rivals;
    for s in &snapshot.fighters {
        if s.slot == fighter_slots::JOIN {
            // The form's kit first: the action names one of its motions.
            show_form(game, s.slot, s.form);
            if let Some(it) = game.items.as_mut() {
                it.slots.clone_from(&s.items);
            }
            if let Some(d) = game.drive.as_mut() {
                d.show_bullets(&s.bullets, &mut game.fx_styled);
            }
            let book = game.kh3_book.clone();
            if apply_state(s, &mut game.sora, book.as_deref()) {
                game.action_started = now;
            }
        } else if let Some(i) = game.rivals.iter().position(|r| r.slot == s.slot) {
            let mut r = game.rivals.swap_remove(i);
            rival::swap(game, &mut r);
            show_form(game, s.slot, s.form);
            rival::swap(game, &mut r);
            if let Some(d) = r.own.drive.as_mut() {
                d.show_bullets(&s.bullets, &mut game.fx_styled);
            }
            let book = game.kh3_book.clone();
            if apply_state(s, &mut r.fighter, book.as_deref()) {
                r.started = now;
            }
            game.rivals.insert(i, r);
        }
    }
    while net.events.front().is_some_and(|(t, _)| *t <= tick) {
        let (_, events) = net.events.pop_front().unwrap();
        for e in &events {
            play_event(game, e);
        }
    }
}

/// The form the host's snapshot names, on the Sora swapped in; the change
/// goes in the log.
fn show_form(game: &mut Game, slot: u8, form: u8) {
    let before = game.drive.as_ref().map(|d| d.shown);
    drive_room::show_form(game, form);
    let after = game.drive.as_ref().map(|d| d.shown);
    if after != before {
        let tick = game.tick;
        game.event(serde_json::json!({"event":"duel_form_shown","tick":tick,
            "slot":slot,"form":after}));
    }
}

/// Joiner, after its field tick: this tick's buttons, with the last few.
pub(crate) fn join_send(net: Option<ResMut<DuelNet>>, game: Res<Game>) {
    let Some(mut net) = net else { return };
    if net.role != Role::Join || !net.playing() {
        return;
    }
    if net.recent.len() >= duel_wire::INPUT_REDUNDANCY {
        net.recent.pop_front();
    }
    let tick = game.tick as u32;
    net.recent.push_back((tick, game.net_pad));
    let recent: Vec<_> = net.recent.iter().copied().collect();
    net.session
        .send(UNRELIABLE, duel_wire::encode_input(&recent));
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeMap;

    fn press(attack: bool, x: f32) -> RivalPad {
        RivalPad {
            direction: [x, 0.],
            attack,
            ..RivalPad::default()
        }
    }

    #[test]
    fn inputs_come_one_per_tick_in_order() {
        let mut q: BTreeMap<u32, RivalPad> = [(5, press(false, 0.1)), (6, press(true, 0.2))].into();
        assert_eq!(next_input(&mut q).unwrap().0, 5);
        assert_eq!(next_input(&mut q).unwrap().0, 6);
        assert!(next_input(&mut q).is_none());
    }

    #[test]
    fn a_backlog_is_merged_without_losing_presses() {
        let mut q: BTreeMap<u32, RivalPad> =
            (0..8u32).map(|t| (t, press(t == 1, t as f32))).collect();
        let (tick, pad) = next_input(&mut q).unwrap();
        // Four wait at most: ticks 0..=3 were folded into tick 4.
        assert_eq!(tick, 4);
        assert!(pad.attack, "the press of tick 1 survives");
        assert_eq!(pad.direction, [4., 0.]);
        assert_eq!(q.len(), 3);
    }

    #[test]
    fn missing_ticks_hold_the_stick_but_not_presses() {
        let p = RivalPad {
            direction: [0.5, 0.5],
            jump: true,
            jump_held: true,
            attack: true,
            guard: true,
            guard_held: true,
            shotlock_held: true,
            flow: true,
            command: Command::Cast(1),
        };
        let h = held(p);
        assert_eq!(h.command, Command::None);
        assert_eq!(h.direction, p.direction);
        assert!(h.jump_held && h.guard_held && h.shotlock_held);
        assert!(!h.jump && !h.attack && !h.guard && !h.flow);
    }
}
