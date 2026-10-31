//! The duel's effects on the joiner's screen. The host's spell, item and
//! Drive effect starts (and a spell's later effect calls) go out as events
//! naming their effect container, and the joiner starts the same elements
//! on the same Sora. Hit sparks and guard flashes have their own events
//! (duel_hits); weapon trails and the map's own effects are each
//! instance's own. The Soras' own sounds (swings, voices, cloth, footsteps,
//! form changes) go the same way.

/// The sound causes the host sends: what a Sora's motions and commands
/// play. Effect sounds start with their effects on each side; hit sounds
/// have their own events; menu sounds stay with the player who heard them.
const SORA_SOUNDS: [&str; 11] = [
    "swing",
    "voice",
    "move_se",
    "footstep",
    "spell_voice",
    "drive_start",
    "revert",
    "antiform",
    "form_level",
    "wall_hit",
    super::duel_kh3::CUE_SOUND,
];

use super::duel_wire::{DuelEvent, FxAnchor};
use super::*;
use data::object_effect::Pax;
use player::magic::{Anchor, Fx};

/// The name a fighter's effect container goes by on both instances:
/// `magic:<file>`, `system`, `form:<n>`, `weapon:<slot>` /
/// `weapon2:<slot>` (each Sora's own Keyblade) or `map` (the ground dust).
fn name_of(game: &Game, pax: &Arc<Pax>) -> Option<String> {
    let same = |p: &Option<Arc<Pax>>| p.as_ref().is_some_and(|p| Arc::ptr_eq(p, pax));
    if same(&game.map_pax) {
        return Some("map".to_string());
    }
    let fighter = |slot: u8,
                   magic: Option<&magic_room::MagicRt>,
                   drive: Option<&drive_room::DriveRt>,
                   weapons: [&Option<Arc<Pax>>; 2]| {
        if let Some(file) = magic.and_then(|m| {
            m.src
                .paxes
                .iter()
                .find(|(_, p)| Arc::ptr_eq(p, pax))
                .map(|(f, _)| f)
        }) {
            return Some(format!("magic:{file}"));
        }
        if let Some(d) = drive {
            if same(&d.src.system_pax) {
                return Some("system".to_string());
            }
            if let Some(form) = d.paxes.iter().find(|(_, p)| Arc::ptr_eq(p, pax)) {
                return Some(format!("form:{}", form.0));
            }
        }
        if same(weapons[0]) {
            return Some(format!("weapon:{slot}"));
        }
        same(weapons[1]).then(|| format!("weapon2:{slot}"))
    };
    fighter(
        game.local_slot,
        game.magic.as_ref(),
        game.drive.as_ref(),
        [&game.weapon_pax, &game.weapon2_pax],
    )
    .or_else(|| {
        game.rivals.iter().find_map(|r| {
            fighter(
                r.slot,
                r.own.magic.as_ref(),
                r.own.drive.as_ref(),
                [&r.own.weapon_pax, &r.own.weapon2_pax],
            )
        })
    })
}

/// The container a name stands for on this instance.
fn resolve(game: &Game, name: &str) -> Option<Arc<Pax>> {
    let (kind, rest) = name.split_once(':').unwrap_or((name, ""));
    let magic = |m: Option<&magic_room::MagicRt>| m.and_then(|m| m.src.paxes.get(rest).cloned());
    let system = |d: Option<&drive_room::DriveRt>| d.and_then(|d| d.src.system_pax.clone());
    let form = |d: Option<&drive_room::DriveRt>| {
        let n: u8 = rest.parse().ok()?;
        d.and_then(|d| d.paxes.get(&n).cloned())
    };
    let rivals = || game.rivals.iter();
    match kind {
        "map" => game.map_pax.clone(),
        "magic" => magic(game.magic.as_ref())
            .or_else(|| rivals().find_map(|r| magic(r.own.magic.as_ref()))),
        "system" => system(game.drive.as_ref())
            .or_else(|| rivals().find_map(|r| system(r.own.drive.as_ref()))),
        "form" => {
            form(game.drive.as_ref()).or_else(|| rivals().find_map(|r| form(r.own.drive.as_ref())))
        }
        "weapon" | "weapon2" => {
            let slot: u8 = rest.parse().ok()?;
            let second = kind == "weapon2";
            if slot == game.local_slot {
                if second {
                    game.weapon2_pax.clone()
                } else {
                    game.weapon_pax.clone()
                }
            } else {
                let r = rivals().find(|r| r.slot == slot)?;
                if second {
                    r.own.weapon2_pax.clone()
                } else {
                    r.own.weapon_pax.clone()
                }
            }
        }
        _ => None,
    }
}

/// Host, before the effects start: this tick's effect starts and calls,
/// for the joiner.
pub fn host_effects(net: Option<Res<duel_net::DuelNet>>, mut game: ResMut<Game>) {
    let Some(net) = net else { return };
    if net.role != net::Role::Host || !net.playing() {
        return;
    }
    let game = &mut *game;
    let local = game.local_slot;
    let mut events = Vec::new();
    for c in &game.fx_calls {
        let fx = match &c.fx {
            Fx::Start { key, id, bound, at } => Fx::Start {
                key: *key,
                id: *id,
                // The room Sora's own effects name him by slot; the duel has
                // no party, summon or battle enemy.
                bound: match bound {
                    None => None,
                    Some(Anchor::Caster | Anchor::Party(0)) => Some(Anchor::Rival(local)),
                    Some(Anchor::Rival(s)) => Some(Anchor::Rival(*s)),
                    Some(_) => continue,
                },
                at: *at,
            },
            other => other.clone(),
        };
        let pax = match &c.pax {
            Some(p) => match name_of(game, p) {
                Some(n) => Some(n),
                None => continue,
            },
            None => None,
        };
        events.push(DuelEvent::Spell { pax, fx });
    }
    let own = game
        .drive
        .as_ref()
        .into_iter()
        .flat_map(|d| d.fx_requests.iter().map(|r| (r, FxAnchor::Slot(local))));
    let placed = game.fx_queue.iter().filter_map(|p| {
        let anchor = match p.anchor {
            drive_fx::Anchor::Sora => FxAnchor::Slot(local),
            drive_fx::Anchor::Rival(s) => FxAnchor::Slot(s),
            drive_fx::Anchor::World { at, pitch, yaw } => FxAnchor::World {
                at: at.to_array(),
                pitch,
                yaw,
            },
            _ => return None,
        };
        Some((&p.req, anchor))
    });
    for (req, anchor) in placed.chain(own) {
        if let Some(pax) = name_of(game, &req.pax) {
            events.push(DuelEvent::DriveFx {
                pax,
                id: req.id,
                owner: req.owner,
                anchor,
            });
        }
    }
    game.ext.online_mut().duel_events.extend(events);
}

/// Host, before the sounds play: the Soras' own sounds this tick, for the
/// joiner.
pub fn host_sounds(net: Option<Res<duel_net::DuelNet>>, mut game: ResMut<Game>) {
    let Some(net) = net else { return };
    if net.role != net::Role::Host || !net.playing() {
        return;
    }
    let game = &mut *game;
    let sounds: Vec<DuelEvent> = game
        .sound_queue
        .iter()
        .filter(|(_, cause)| SORA_SOUNDS.contains(cause))
        .map(|&(id, _)| DuelEvent::Sound(id))
        .collect();
    game.ext.online_mut().duel_events.extend(sounds);
}

/// Joiner: one of the host's effect events, on this instance's Soras.
pub fn play(game: &mut Game, event: &DuelEvent) {
    let local = game.local_slot;
    match event {
        DuelEvent::Spell { pax, fx } => {
            let pax = match pax {
                Some(name) => match resolve(game, name) {
                    Some(p) => Some(p),
                    None => return,
                },
                None => None,
            };
            let fx = match fx.clone() {
                Fx::Start { key, id, bound, at } => Fx::Start {
                    key,
                    id,
                    bound: bound.map(|b| match b {
                        Anchor::Rival(s) if s == local => Anchor::Caster,
                        b => b,
                    }),
                    at,
                },
                f => f,
            };
            game.fx_calls.push(spell_fx::Call { pax, fx });
        }
        DuelEvent::DriveFx {
            pax,
            id,
            owner,
            anchor,
        } => {
            let Some(pax) = resolve(game, pax) else {
                return;
            };
            let anchor = match *anchor {
                FxAnchor::Slot(s) if s == local => drive_fx::Anchor::Sora,
                FxAnchor::Slot(s) => drive_fx::Anchor::Rival(s),
                FxAnchor::World { at, pitch, yaw } => drive_fx::Anchor::World {
                    at: Vec3::from_array(at),
                    pitch,
                    yaw,
                },
            };
            game.fx_queue.push(drive_fx::Placed {
                req: drive_fx::FxRequest {
                    pax,
                    id: *id,
                    owner: *owner,
                },
                anchor,
            });
        }
        _ => {}
    }
}
