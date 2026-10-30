//! KH3 attacks' hits [T282]: each open window's shape, from KH3's own shape
//! table and the clip's blade track, swept against the other Sora's body
//! capsule, then KH3's damage and reaction. The host (or a local run) is the
//! only one who runs this. Rules from StrawHatPro's reading of KH3
//! (`crates/kh-core/src/combat/hit.rs`, `status/damage.rs`; MIT).

use super::*;
use player::kh3::attack::{AttackRow, Book, ClipInfo, Shape};
use player::kh3::tuning::length_of;
use player::kh3::{Knock, Strike};

/// The body capsule: radius 50 and half-height 90 at the centre 91 above the
/// feet (`TresBodyCollPrimitive2`).
const BODY_RADIUS: f32 = 50.;
const BODY_FROM: f32 = 51.;
const BODY_TO: f32 = 131.;
/// The owner's chest point for the shapes that start there.
const OWNER_HEIGHT: f32 = 91.;

fn sub(a: [f32; 3], b: [f32; 3]) -> [f32; 3] {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}

fn dot(a: [f32; 3], b: [f32; 3]) -> f32 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}

fn lerp3(a: [f32; 3], b: [f32; 3], t: f32) -> [f32; 3] {
    [
        a[0] + (b[0] - a[0]) * t,
        a[1] + (b[1] - a[1]) * t,
        a[2] + (b[2] - a[2]) * t,
    ]
}

/// The closest points of two segments, and their distance.
fn segments(p1: [f32; 3], q1: [f32; 3], p2: [f32; 3], q2: [f32; 3]) -> (f32, [f32; 3]) {
    let (d1, d2, r) = (sub(q1, p1), sub(q2, p2), sub(p1, p2));
    let (a, e, f) = (dot(d1, d1), dot(d2, d2), dot(d2, r));
    let (s, t);
    if a <= 1e-6 && e <= 1e-6 {
        (s, t) = (0., 0.);
    } else if a <= 1e-6 {
        s = 0.;
        t = (f / e).clamp(0., 1.);
    } else {
        let c = dot(d1, r);
        if e <= 1e-6 {
            t = 0.;
            s = (-c / a).clamp(0., 1.);
        } else {
            let b = dot(d1, d2);
            let denom = a * e - b * b;
            let mut ss = if denom > 1e-6 {
                ((b * f - c * e) / denom).clamp(0., 1.)
            } else {
                0.
            };
            let mut tt = (b * ss + f) / e;
            if tt < 0. {
                tt = 0.;
                ss = (-c / a).clamp(0., 1.);
            } else if tt > 1. {
                tt = 1.;
                ss = ((b - c) / a).clamp(0., 1.);
            }
            (s, t) = (ss, tt);
        }
    }
    let on1 = lerp3(p1, q1, s);
    let on2 = lerp3(p2, q2, t);
    let d = sub(on1, on2);
    (dot(d, d).sqrt(), on1)
}

/// A shape's radius after its growth (`size = max(m_Size + m_IncSize*t, 0)`,
/// then `min(size, m_IncMaxSize)` if that is above 0).
fn grown(shape: &Shape, t: f32) -> f32 {
    let r = (shape.radius + shape.grow * t).max(0.);
    if shape.grow_max > 0. {
        r.min(shape.grow_max)
    } else {
        r
    }
}

/// The shape's world segment and radius this tick.
fn shape_points(
    shape: &Shape,
    pts: [[f32; 3]; 2],
    player: &Player,
    open_for: f32,
) -> ([[f32; 3]; 2], f32) {
    let k = length_of(1.);
    let radius = grown(shape, open_for) * k;
    let at = if shape.from_owner {
        let (fx, fz) = (player.yaw.sin(), player.yaw.cos());
        let o = shape.offset;
        [
            player.position[0] + (fx * o[0] - fz * o[1]) * k,
            player.position[1] + (OWNER_HEIGHT + o[2]) * k,
            player.position[2] + (fz * o[0] + fx * o[1]) * k,
        ]
    } else {
        pts[0]
    };
    let seg = if shape.capsule {
        pts
    } else if shape.upright {
        let h = shape.half_length * k;
        [[at[0], at[1] - h, at[2]], [at[0], at[1] + h, at[2]]]
    } else {
        [at, at]
    };
    (seg, radius)
}

/// `compute_damage` for a duel of two Soras, in KH3's own numbers.
pub(crate) fn damage(
    book: &Book,
    row: &AttackRow,
    set: &player::kh3::ability::Set,
    step: u32,
    air: bool,
) -> f32 {
    let ceil = |v: f32| v.ceil();
    let a = if row.magic {
        book.stats[1]
    } else {
        book.stats[0]
    };
    let mut x = (a.max(0.) - book.stats[2]).max(1.);
    // A player victim's floor and cap (level above 70: 13, at most 80).
    let lo = if row.damage_min > 0. {
        row.damage_min
    } else {
        13.
    };
    x = x.max(lo);
    if x > 80. && lo < 80. {
        x = 80.;
    }
    x = ceil((x - 1e-6) * (row.power - 1e-6));
    use player::kh3::ability::Ability as A;
    if !row.magic {
        let n = set.count(if air { A::AirComboBoost } else { A::ComboBoost });
        x += ceil(step as f32 * f32::from(n) * 0.05 * x);
    }
    if x == 0. { 0.1 } else { x }
}

/// The reaction of a row's damage kind; on Sora's victim side the spins fall
/// into the small handler (`docs/sora-combat-spec`).
fn strike_of(row: &AttackRow) -> Strike {
    let knock = match row.damage_kind.as_str() {
        "BLOW" | "SHIELD_BLOW" => Knock::Blow,
        "TOSS" | "SHIELD_TOSS" => Knock::Toss,
        "LAUNCH" => Knock::Launch,
        "BEAT" => Knock::Beat,
        "PARABOLA" => Knock::Parabola,
        _ => Knock::Small,
    };
    Strike {
        knock,
        scale: row.power_scale,
    }
}

/// One Sora's attack this tick against one body. Returns the windows' results
/// so the borrow of the attacker ends before the hit lands.
struct Contact {
    window: usize,
    row: String,
    at: [f32; 3],
    hit_se: u8,
}

fn sweep(
    fighter: &mut Fighter,
    info: &ClipInfo,
    book: &Book,
    body: [[f32; 3]; 2],
    body_radius: f32,
) -> Vec<Contact> {
    let mut out = Vec::new();
    let Some(pts) = fighter.kh3.attack_points(&fighter.player, info) else {
        return out;
    };
    let player = fighter.player.clone();
    let Some(attack) = fighter.kh3.attack.as_mut() else {
        return out;
    };
    let time = attack.time;
    for (i, w) in info.windows.iter().enumerate() {
        let Some(state) = attack.windows.get_mut(i) else {
            continue;
        };
        if time < w.from || time >= w.to {
            continue;
        }
        let Some(shape) = book.shapes.get(&w.group) else {
            continue;
        };
        let (mut seg, radius) = shape_points(shape, pts, &player, state.open_for);
        if shape.frozen {
            seg = *state.frozen.get_or_insert(seg);
        }
        let from = state.previous.unwrap_or(seg);
        state.previous = Some(seg);
        if state.hit {
            continue;
        }
        // Four samples of the shape between last tick and this one.
        for s in 1..=4 {
            let t = s as f32 / 4.;
            let (a, b) = (lerp3(from[0], seg[0], t), lerp3(from[1], seg[1], t));
            let (d, on) = segments(a, b, body[0], body[1]);
            if d <= radius + body_radius {
                state.hit = true;
                state.since_hit = 0.;
                out.push(Contact {
                    window: i,
                    row: w.row.clone(),
                    at: on,
                    hit_se: w.hit_se,
                });
                break;
            }
        }
    }
    out
}

fn body_of(position: [f32; 3]) -> [[f32; 3]; 2] {
    let k = length_of(1.);
    [
        [position[0], position[1] + BODY_FROM * k, position[2]],
        [position[0], position[1] + BODY_TO * k, position[2]],
    ]
}

/// One hit on a Sora: guard, dodge, damage, reaction.
#[allow(clippy::too_many_arguments)]
fn land_on_sora(
    game: &mut Game,
    book: &Book,
    row: &AttackRow,
    attacker_at: [f32; 3],
    attacker_air: bool,
    victim: usize,
    step: u32,
    set: player::kh3::ability::Set,
    contact: [f32; 3],
    tick: u64,
) -> bool {
    let rules = game.duel_rules;
    let (fighter, clips) = if victim == 0 {
        (&mut game.sora, &game.clips)
    } else {
        let r = &mut game.rivals[victim - 1];
        (&mut r.fighter, &r.own.clips)
    };
    let at = fighter.player.position;
    let mut away = [at[0] - attacker_at[0], at[2] - attacker_at[2]];
    let l = away[0].hypot(away[1]);
    away = if l > 1e-3 {
        [away[0] / l, away[1] / l]
    } else {
        [0., 1.]
    };
    let strike = strike_of(row);
    // Guard: the arc's half angle is 120 degrees (`guard.arc`).
    let guarding = matches!(fighter.player.action, Action::Guard | Action::GuardRecoil);
    if guarding && !row.ignore_guard {
        let (sin, cos) = fighter.player.yaw.sin_cos();
        let toward = -(sin * away[0] + cos * away[1]);
        if toward >= (120f32).to_radians().cos() {
            fighter
                .kh3
                .guarded(&mut fighter.player, strike, away, attacker_at, attacker_air);
            let _ = fighter.player.guard_recoil(away, clips);
            let c = Vec3::from_array(contact);
            game.effect_queue
                .push(EffectRequest::System { id: 11, at: c });
            game.duel_events
                .push(duel_wire::DuelEvent::Guard(contact, 11));
            return false;
        }
    }
    if fighter.vitals.invulnerable > 0. || fighter.kh3.untouchable(&fighter.player, clips) {
        fighter.kh3.dodged();
        return false;
    }
    let raw = damage(book, row, &set, step, attacker_air);
    // KH3's HP against the duel's gauge.
    let max = fighter.vitals.target.max_hp;
    let scaled = ((raw / book.stats[3].max(1.)) * max as f32).round().max(1.) as i32;
    let dealt = rules.damage(scaled, row.magic, max);
    let before = fighter.vitals.hp;
    fighter.vitals.hp = (fighter.vitals.hp - dealt).max(0);
    fighter.kh3.hit_taken();
    let shrug = fighter.kh3.shrugs_strike(&fighter.player, strike);
    if !shrug {
        fighter
            .kh3
            .hurt(&mut fighter.player, strike, away, true, clips);
    }
    let hp = fighter.vitals.hp;
    if hp == 0 {
        fighter.player.die(clips);
    }
    let mode = fighter.kh3.mode.name();
    game.event(
        serde_json::json!({"event":"kh3_hit","tick":tick,"victim":victim,
        "row_power":row.power,"kind":row.damage_kind,"raw":raw,"dealt":dealt,"before":before,
        "hp":hp,"shrug":shrug,"mode":mode}),
    );
    true
}

/// The dummy takes a hit as the Shadow it is.
fn land_on_dummy(
    game: &mut Game,
    book: &Book,
    row: &AttackRow,
    attacker_at: [f32; 3],
    step: u32,
    set: player::kh3::ability::Set,
) -> bool {
    let air = false;
    let raw = damage(book, row, &set, step, air);
    let Some(rt) = game.enemy.as_mut() else {
        return false;
    };
    let at = rt.enemy.position;
    let dir = [at[0] - attacker_at[0], 0., at[2] - attacker_at[2]];
    let attack = player::combat::AttackParams {
        reaction: match strike_of(row).knock {
            Knock::Small => 1,
            _ => 4,
        },
        ..Default::default()
    };
    rt.enemy
        .take_damage(raw.round() as i32, dir, true, &attack, true)
        .is_some()
}

/// A spell actor's shapes against a body: the first that touches, its row.
fn actor_contact(
    a: &player::kh3::spell::Actor,
    body: [[f32; 3]; 2],
    body_radius: f32,
) -> Option<(String, [f32; 3])> {
    let k = length_of(1.);
    for shape in &a.stage.shapes {
        let radius = shape.radius_at(a.age) * k;
        if radius <= 0. {
            continue;
        }
        let half = shape.half_at(a.age) * k;
        let z = shape.centre_z * k;
        for s in 1..=4 {
            let t = s as f32 / 4.;
            let c = lerp3(a.previous, a.pos, t);
            let (top, bottom) = ([c[0], c[1] + z + half, c[2]], [c[0], c[1] + z - half, c[2]]);
            let (d, on) = segments(bottom, top, body[0], body[1]);
            if d <= radius + body_radius {
                let row = if a.grand {
                    a.stage
                        .row_grand
                        .clone()
                        .unwrap_or_else(|| shape.row.clone())
                } else if a.finish {
                    a.stage
                        .row_finish
                        .clone()
                        .unwrap_or_else(|| shape.row.clone())
                } else {
                    shape.row.clone()
                };
                return Some((row, on));
            }
        }
    }
    None
}

/// A Keyblade hit's effect and sound.
fn hit_fx(book: &Book, hit_se: u8, at: [f32; 3]) -> player::kh3::spell::FxRequest {
    player::kh3::spell::FxRequest {
        effect: book.hit_effect.clone(),
        sound: Some(format!(
            "/Game/Sound/SE/weapon/02001/se02001_00{}",
            hit_se.clamp(1, 7)
        )),
        at,
        yaw: 0.,
    }
}

/// The shotlock's shots against the other body.
fn bolts_tick(game: &mut Game, book: &Book, count: usize, tick: u64) {
    let radius = BODY_RADIUS * length_of(1.);
    let shot_radius = 50. * length_of(1.);
    for attacker in 0..count {
        let dummy = count == 1;
        let target = (0..count).find(|v| *v != attacker);
        let body_at = if dummy {
            game.enemy
                .as_ref()
                .filter(|rt| game.kh3_dummy && rt.enabled)
                .map(|rt| rt.enemy.position)
        } else {
            target.map(|v| {
                if v == 0 {
                    game.sora.player.position
                } else {
                    game.rivals[v - 1].fighter.player.position
                }
            })
        };
        let Some(body_at) = body_at else { continue };
        let body = body_of(body_at);
        let f = if attacker == 0 {
            &mut game.sora
        } else {
            &mut game.rivals[attacker - 1].fighter
        };
        let (at, air, set) = (f.player.position, f.player.in_air(), f.kh3.abilities);
        let hits: Vec<(u32, bool, [f32; 3])> = f
            .kh3
            .bolts
            .iter()
            .filter_map(|b| {
                let (d, on) = segments(b.pos, b.pos, body[0], body[1]);
                (d <= shot_radius + radius).then_some((b.serial, b.ragnarok, on))
            })
            .collect();
        for (serial, ragnarok, on) in hits {
            let f = if attacker == 0 {
                &mut game.sora
            } else {
                &mut game.rivals[attacker - 1].fighter
            };
            f.kh3.bolt_hit(serial);
            let stage = book.shots[usize::from(ragnarok)].clone();
            f.kh3.fx.push(player::kh3::spell::FxRequest {
                effect: stage.as_ref().and_then(|s| s.hit_effect.clone()),
                sound: stage.as_ref().and_then(|s| s.hit_sound.clone()),
                at: on,
                yaw: 0.,
            });
            let row = stage
                .as_ref()
                .and_then(|s| s.shapes.first())
                .and_then(|s| book.row(&s.row))
                .cloned()
                .or_else(|| {
                    book.row(if ragnarok {
                        "SF_KCRAGNAROK"
                    } else {
                        "SF_KCSHOT"
                    })
                    .cloned()
                });
            let Some(row) = row else { continue };
            if dummy {
                land_on_dummy(game, book, &row, at, 0, set);
            } else if let Some(v) = target {
                land_on_sora(game, book, &row, at, air, v, 0, set, on, tick);
            }
        }
    }
}

/// What the layers asked for this tick: for the joiner, and this PC's presenter.
fn send_fx(game: &mut Game, book: &Book) {
    let id = |n: &Option<String>| {
        n.as_ref()
            .and_then(|n| book.names.binary_search(n).ok())
            .map_or(u16::MAX, |i| i as u16)
    };
    let mut list = std::mem::take(&mut game.sora.kh3.fx);
    for r in &mut game.rivals {
        list.append(&mut r.fighter.kh3.fx);
    }
    for r in &list {
        game.duel_events.push(duel_wire::DuelEvent::Kh3Fx {
            effect: id(&r.effect),
            sound: id(&r.sound),
            at: r.at,
            yaw: r.yaw,
        });
    }
    game.kh3_fx_remote.append(&mut list);
}

/// Spell actors against the other body.
fn actors_tick(game: &mut Game, book: &Book, count: usize, tick: u64) {
    let radius = BODY_RADIUS * length_of(1.);
    for attacker in 0..count {
        let dummy = count == 1;
        let (at, air, set) = {
            let f = if attacker == 0 {
                &game.sora
            } else {
                &game.rivals[attacker - 1].fighter
            };
            (f.player.position, f.player.in_air(), f.kh3.abilities)
        };
        let targets: Vec<usize> = (0..count).filter(|v| *v != attacker).collect();
        // The dummy, or the other Sora.
        let body_at = if dummy {
            game.enemy
                .as_ref()
                .filter(|rt| game.kh3_dummy && rt.enabled)
                .map(|rt| rt.enemy.position)
        } else {
            targets.first().map(|v| {
                if *v == 0 {
                    game.sora.player.position
                } else {
                    game.rivals[*v - 1].fighter.player.position
                }
            })
        };
        let Some(body_at) = body_at else { continue };
        let body = body_of(body_at);
        let f = if attacker == 0 {
            &mut game.sora
        } else {
            &mut game.rivals[attacker - 1].fighter
        };
        let hits: Vec<(usize, String, [f32; 3])> = f
            .kh3
            .actors
            .iter()
            .enumerate()
            .filter(|(_, a)| !a.hit)
            .filter_map(|(i, a)| actor_contact(a, body, radius).map(|(r, on)| (i, r, on)))
            .collect();
        for (index, row_name, on) in hits {
            let cmd = {
                let f = if attacker == 0 {
                    &mut game.sora
                } else {
                    &mut game.rivals[attacker - 1].fighter
                };
                let cmd = f.kh3.actors.get(index).map(|a| a.cmd);
                f.kh3.actor_hit(index, on);
                cmd
            };
            let Some(row) = book.row(&row_name) else {
                continue;
            };
            let landed = if dummy {
                land_on_dummy(game, book, row, at, 0, set)
            } else {
                targets
                    .first()
                    .is_some_and(|v| land_on_sora(game, book, row, at, air, *v, 0, set, on, tick))
            };
            if landed {
                let f = if attacker == 0 {
                    &mut game.sora
                } else {
                    &mut game.rivals[attacker - 1].fighter
                };
                f.kh3.form_point(row.form_point, cmd);
            }
        }
    }
}

/// Runs on the fixed tick after the Soras moved.
pub(crate) fn tick(mut game: ResMut<Game>) {
    let game = &mut *game;
    if !duel_kh3::on(game) || game.net_client || game.duel_frozen {
        return;
    }
    let Some(book) = game.kh3_book.clone() else {
        return;
    };
    let tick = game.tick;
    // Every fighter by index: 0 the room Sora, then the rivals.
    let count = 1 + game.rivals.len();
    actors_tick(game, &book, count, tick);
    bolts_tick(game, &book, count, tick);
    // A Cure's heal, on the duel's gauge.
    let hp_scale = book.stats[3].max(1.);
    let magic = book.stats[1].max(1.);
    for i in 0..count {
        let f = if i == 0 {
            &mut game.sora
        } else {
            &mut game.rivals[i - 1].fighter
        };
        let heal = std::mem::take(&mut f.kh3.heal);
        if heal > 0 {
            let max = f.vitals.target.max_hp;
            let add = ((heal as f32 + magic) / hp_scale * max as f32).round() as i32;
            f.vitals.hp = (f.vitals.hp + add).min(max);
        }
    }
    for attacker in 0..count {
        let fighter = if attacker == 0 {
            &mut game.sora
        } else {
            &mut game.rivals[attacker - 1].fighter
        };
        let Some(row_name) = fighter.kh3.attack.as_ref().map(|a| a.row) else {
            continue;
        };
        let Some(info) = book.clips.get(row_name) else {
            continue;
        };
        let attacker_at = fighter.player.position;
        let attacker_air = fighter.player.in_air();
        let step = fighter.kh3.chain.n;
        let set = fighter.kh3.abilities;
        let radius = BODY_RADIUS * length_of(1.);
        // The dummy, when it stands instead of a second Sora.
        if count == 1 {
            let dummy = game
                .enemy
                .as_ref()
                .filter(|rt| game.kh3_dummy && rt.enabled)
                .map(|rt| rt.enemy.position);
            let Some(at) = dummy else {
                continue;
            };
            let contacts = sweep(&mut game.sora, info, &book, body_of(at), radius);
            for c in contacts {
                if let Some(row) = book.row(&c.row)
                    && land_on_dummy(game, &book, row, attacker_at, step, set)
                {
                    game.sora.kh3.landed(true);
                    game.sora.kh3.form_point(row.form_point, None);
                    game.sora.kh3.fx.push(hit_fx(&book, c.hit_se, c.at));
                }
                let _ = c.window;
                let _ = c.at;
            }
            continue;
        }
        for victim in 0..count {
            if victim == attacker {
                continue;
            }
            let body_at = if victim == 0 {
                game.sora.player.position
            } else {
                game.rivals[victim - 1].fighter.player.position
            };
            let attacker_fighter = if attacker == 0 {
                &mut game.sora
            } else {
                &mut game.rivals[attacker - 1].fighter
            };
            let contacts = sweep(attacker_fighter, info, &book, body_of(body_at), radius);
            for c in contacts {
                let Some(row) = book.row(&c.row) else {
                    continue;
                };
                if land_on_sora(
                    game,
                    &book,
                    row,
                    attacker_at,
                    attacker_air,
                    victim,
                    step,
                    set,
                    c.at,
                    tick,
                ) {
                    let a = if attacker == 0 {
                        &mut game.sora
                    } else {
                        &mut game.rivals[attacker - 1].fighter
                    };
                    a.kh3.fx.push(hit_fx(&book, c.hit_se, c.at));
                    a.kh3.landed(true);
                    a.kh3.form_point(row.form_point, None);
                    a.player.connected = true;
                }
            }
        }
    }
    send_fx(game, &book);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn segments_measure_the_gap() {
        let (d, _) = segments([0., 0., 0.], [1., 0., 0.], [0., 2., 0.], [1., 2., 0.]);
        assert!((d - 2.).abs() < 1e-5);
        let (d, on) = segments([0., 0., 0.], [0., 0., 0.], [-1., 3., 0.], [1., 3., 0.]);
        assert!((d - 3.).abs() < 1e-5);
        assert_eq!(on, [0., 0., 0.]);
    }

    #[test]
    fn shapes_grow_to_their_limit() {
        let shape = Shape {
            radius: 100.,
            grow: 700.,
            grow_max: 350.,
            ..Shape::default()
        };
        assert_eq!(grown(&shape, 0.), 100.);
        assert_eq!(grown(&shape, 1.), 350.);
    }

    #[test]
    fn damage_follows_the_formula() {
        let book = Book {
            stats: [66., 67., 59., 90.],
            ..Book::default()
        };
        let row = AttackRow {
            power: 0.6,
            ..AttackRow::default()
        };
        // (66 - 59) is under the floor 13; 13 x 0.6 rounds up to 8.
        let set = player::kh3::ability::Set::default();
        assert_eq!(damage(&book, &row, &set, 0, false), 8.);
    }

    #[test]
    fn a_rows_kind_names_its_reaction() {
        let row = AttackRow {
            damage_kind: "BLOW".into(),
            power_scale: 1.5,
            ..AttackRow::default()
        };
        let s = strike_of(&row);
        assert_eq!(s.knock, Knock::Blow);
        assert_eq!(s.scale, 1.5);
        let row = AttackRow {
            damage_kind: "SIDESPIN".into(),
            ..AttackRow::default()
        };
        assert_eq!(strike_of(&row).knock, Knock::Small);
    }
}
