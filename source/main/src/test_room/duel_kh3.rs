//! The duel's KH3 ruleset in the room [T279]: both Soras tick through
//! `player::kh3` instead of the KH2 controller alone; its shotlock bolts ride
//! Wisdom Form's bullets (their effect, their hit volume, their place in
//! the snapshot); a grand magic is the room's own cast without MP; the
//! poles are the narrow pillars among the arena's walls; the lean (wall
//! run, pole, airstep) turns the model and its spheres; and a plain HUD
//! shows focus, the shotlock's locks and the grand magic offer. Single
//! player and the KH2 ruleset never come here.

use super::*;
use player::kh3::{self, Foe, Pad, Scene, surface::Pole};

/// Shotlock bolts' bullet keys: Wisdom's bullet range with this bit.
pub(crate) const SHOT_BIT: u64 = 1 << 59;
const SHOT_KEYS: u64 = 1 << 60 | SHOT_BIT;
/// Wisdom's MAGF bullet element, the bolts' look.
const BOLT_ELEMENT: u16 = 20;
/// Wisdom's ATKP sub-tables: King of Hearts' bolts take the plain shot's
/// (power 20), Ragnarok's the volley's last (25) [duel choice].
const KOH_SUB: u8 = 0;
const RAGNAROK_SUB: u8 = 3;
/// A grand magic keeps the caster's MP and doubles its spell's hits for
/// this long (the cast and its volumes).
const GRAND_TICKS: u32 = 150;
/// Invulnerable from the grand magic's start until about its shot.
const GRAND_GUARD: f32 = 40.;

/// The room collision's poles and its walls' XZ extent, kept with the
/// collision's triangle counts (None for an area's map collision).
pub(crate) type PoleCache = (Option<(usize, usize)>, Arc<Vec<Pole>>, Option<[f32; 4]>);

/// The KH3 ruleset is on (a duel whose rules say so).
pub(crate) fn on(game: &Game) -> bool {
    game.duel && game.duel_rules.kh3()
}

/// The room collision's poles, found once per collision (kept by its
/// triangle counts).
pub(crate) fn poles(game: &mut Game) -> Arc<Vec<Pole>> {
    let key = match &game.world {
        RoomWorld::Triangles(t) => Some(t.counts()),
        RoomWorld::Map(_) => None,
    };
    if let Some((k, poles, _)) = &game.kh3_poles
        && *k == key
    {
        return Arc::clone(poles);
    }
    let found = match &game.world {
        RoomWorld::Triangles(t) => kh3::surface::find_poles(t.walls()),
        RoomWorld::Map(_) => Vec::new(),
    };
    let walls = match &game.world {
        RoomWorld::Triangles(t) => {
            let mut e = [f32::MAX, f32::MAX, f32::MIN, f32::MIN];
            for v in t.walls().iter().flatten() {
                e = [
                    e[0].min(v[0]),
                    e[1].min(v[2]),
                    e[2].max(v[0]),
                    e[3].max(v[2]),
                ];
            }
            (e[0] <= e[2]).then_some((t.walls().len(), e))
        }
        RoomWorld::Map(_) => None,
    };
    let tick = game.tick;
    game.event(
        serde_json::json!({"event":"kh3_poles","tick":tick,"walls":walls,
        "poles":found.iter().map(|p| serde_json::json!({"centre":p.centre,"radius":p.radius,
        "bottom":p.bottom,"top":p.top})).collect::<Vec<_>>()}),
    );
    let poles = Arc::new(found);
    game.kh3_poles = Some((key, Arc::clone(&poles), walls.map(|w| w.1)));
    poles
}

/// The other Sora as the layer sees him.
pub(crate) fn foe(position: [f32; 3], centre: [f32; 3]) -> Foe {
    Foe { position, centre }
}

/// The room Sora's foe: the first rival.
pub(crate) fn room_foe(game: &Game) -> Option<Foe> {
    game.rivals
        .first()
        .filter(|r| r.fighter.vitals.hp > 0)
        .map(|r| foe(r.fighter.player.position, rival::rival_centre(r)))
}

/// The KH3 buttons of the room Sora's own pad: R2 held (logical bit 32)
/// and Triangle.
pub(crate) fn room_pad(game: &mut Game) -> Pad {
    Pad {
        shotlock: game.autoplay.is_none() && game.pad.held & 1 << 32 != 0,
        situation: std::mem::take(&mut game.flow_queued),
    }
}

/// One tick of a Sora through the layer.
pub(crate) fn step(
    fighter: &mut Fighter,
    input: &Input,
    pad: &Pad,
    params: &Params,
    clips: &Clips,
    world: &impl player::World,
    scene: &Scene,
) -> kh3::Outcome {
    fighter
        .kh3
        .tick(&mut fighter.player, input, pad, params, clips, world, scene)
}

/// [duel choice] KH3's moves reach heights the arenas' walls do not cover
/// everywhere (the Mirage Arena's ring is lower in places than a wall run
/// climbs elsewhere): a Sora is kept inside the walls' extent, a circle
/// for the round Mirage Arena, the square for the test room's.
pub(crate) fn contain(at: [f32; 3], extent: [f32; 4], round: bool, radius: f32) -> [f32; 3] {
    let centre = [(extent[0] + extent[2]) * 0.5, (extent[1] + extent[3]) * 0.5];
    let half = [
        ((extent[2] - extent[0]) * 0.5 - radius).max(0.),
        ((extent[3] - extent[1]) * 0.5 - radius).max(0.),
    ];
    let d = [at[0] - centre[0], at[2] - centre[1]];
    let (x, z) = if round {
        let r = half[0].min(half[1]);
        let l = d[0].hypot(d[1]);
        if l > r {
            (d[0] * r / l, d[1] * r / l)
        } else {
            (d[0], d[1])
        }
    } else {
        (d[0].clamp(-half[0], half[0]), d[1].clamp(-half[1], half[1]))
    };
    [centre[0] + x, at[1], centre[1] + z]
}

/// What a tick asked of the room, for the Sora swapped in (`game.sora`,
/// seat `slot`): the arena's bounds, the log, a grand magic, the grand
/// window's MP.
pub(crate) fn after(game: &mut Game, out: &kh3::Outcome, slot: u8) {
    if let Some((_, _, Some(extent))) = &game.kh3_poles {
        let round = game.duel_arena == duel_arena::MIRAGE;
        let p = game.sora.player.position;
        // At the walls' extent itself: a wall run's feet are on the wall.
        let kept = contain(p, *extent, round, 0.);
        if kept != p {
            game.sora.player.position = kept;
        }
    }
    let tick = game.tick;
    for name in &out.began {
        let p = &game.sora.player;
        let record = serde_json::json!({"event":"kh3","tick":tick,"slot":slot,"move":name,
            "position":p.position,"action":format!("{:?}", p.action),
            "focus":game.sora.kh3.focus.round()});
        game.event(record);
    }
    if let Some((left, mp)) = game.sora.kh3_grand {
        // The grand magic costs nothing: the MP stays where it was.
        game.sora.player.mp.cur = mp;
        game.sora.kh3_grand = (left > 1).then_some((left - 1, mp));
    }
    if let Some(spell) = out.grand {
        let mp = game.sora.player.mp.cur;
        game.sora.player.mp.cur = game.sora.player.mp.max;
        let ok = magic_room::request(game, spell, tick);
        game.sora.player.mp.cur = mp;
        if ok {
            game.sora.kh3.grand_used();
            game.sora.kh3_grand = Some((GRAND_TICKS, mp));
            game.sora.player.invulnerable = game.sora.player.invulnerable.max(GRAND_GUARD);
        }
        game.event(
            serde_json::json!({"event":"kh3_grand","tick":tick,"slot":slot,
            "magic":spell,"ok":ok}),
        );
    }
}

/// A spell this Sora's grand magic cast is still out (its hits double).
pub(crate) fn grand(fighter: &Fighter) -> bool {
    fighter.kh3_grand.is_some()
}

/// The bolts of the Sora swapped in onto his Drive state's bullets (before
/// `drive_room::shots` moves them and makes their volumes): a bullet that
/// hit takes its bolt with it; each bolt's bullet is made or put where the
/// bolt is, moving its last step this tick; a bullet whose bolt is gone
/// fades.
pub(crate) fn bolts(game: &mut Game, slot: u8) {
    let Game {
        drive,
        sora,
        magic,
        fx_queue,
        ..
    } = &mut *game;
    let Some(d) = drive.as_mut() else {
        return;
    };
    let tag = (u64::from(slot) + 1) << 40;
    let key_of = |serial: u32| SHOT_KEYS | tag | u64::from(serial);
    if let Some(m) = magic.as_ref() {
        for b in d.bullets.iter().filter(|b| b.key & SHOT_BIT != 0) {
            if m.last_hits.contains(&b.key) {
                sora.kh3.bolt_hit((b.key & 0xffff_ffff) as u32);
            }
        }
    }
    let live: Vec<u64> = sora.kh3.bolts.iter().map(|b| key_of(b.serial)).collect();
    let mut log = Vec::new();
    let ends = &mut d.bullet_ends;
    d.bullets.retain(|b| {
        let keep = b.key & SHOT_BIT == 0 || live.contains(&b.key);
        if !keep {
            ends.push((b.key, true));
        }
        keep
    });
    for bolt in &sora.kh3.bolts {
        let key = key_of(bolt.serial);
        // Shot this tick: the bullet starts a step behind and steps to it.
        let from = std::array::from_fn(|k| bolt.pos[k] - bolt.dir[k] * bolt.speed);
        match d.bullets.iter_mut().find(|b| b.key == key) {
            Some(b) => {
                b.pos = from;
                b.dir = bolt.dir;
                b.speed = bolt.speed;
                b.keep_alive();
            }
            None => {
                let sub = if bolt.ragnarok { RAGNAROK_SUB } else { KOH_SUB };
                let look = d.wisdom_pax();
                if bolt.serial % 8 == 1 {
                    log.push((key, look.is_some()));
                }
                let mut b = player::wisdom_form::Bullet::new(key, sub, from, bolt.dir);
                b.speed = bolt.speed;
                d.bullets.push(b);
                if let Some(pax) = look {
                    fx_queue.push(drive_fx::Placed {
                        req: drive_fx::FxRequest {
                            pax,
                            id: BOLT_ELEMENT,
                            owner: player::forms::WISDOM,
                        },
                        anchor: drive_fx::Anchor::Bullet(key),
                    });
                }
            }
        }
    }
    let tick = game.tick;
    for (key, look) in log {
        game.event(
            serde_json::json!({"event":"kh3_bolt","tick":tick,"slot":slot,
            "key":key,"look":look}),
        );
    }
}

/// Whether a bullet key is a shotlock bolt's.
pub(crate) fn is_bolt(key: u64) -> bool {
    key & SHOT_BIT != 0 && key & 1 << 60 != 0
}

/// The model's turn: the facing, then the layer's lean (pitch, roll).
pub(crate) fn turn(yaw: f32, kh3: &kh3::Kh3) -> Quat {
    Quat::from_rotation_y(yaw)
        * Quat::from_rotation_x(kh3.lean[0])
        * Quat::from_rotation_z(kh3.lean[1])
}

/// [duel choice] While this player's Sora runs on or kicks off a wall the
/// camera stands back from the wall, level with him (the follow camera,
/// behind and below him, would end up in the wall).
const WALL_CAMERA_BACK: f32 = 700.;
const WALL_CAMERA_UP: f32 = 120.;

/// The wall camera, when it applies.
pub(crate) fn flow_view(game: &Game, fov: f32) -> Option<boss::OverlayView> {
    if !on(game) {
        return None;
    }
    let k = &game.sora.kh3;
    let yaw = game.sora.player.yaw;
    let (facing, right) = (
        Vec3::new(yaw.sin(), 0., yaw.cos()),
        Vec3::new(yaw.cos(), 0., -yaw.sin()),
    );
    let n = match k.mode {
        kh3::Mode::WallRun { normal, .. } | kh3::Mode::WallKick { normal, .. }
            if normal != [0.; 2] =>
        {
            Vec3::new(normal[0], 0., normal[1])
        }
        // The joiner's copy has no normal: an up-run (and a kick still on
        // the wall) faces the wall; a side run is rolled away from it.
        kh3::Mode::WallRun { up: false, .. } => right * -k.lean[1].signum(),
        kh3::Mode::WallRun { .. } => -facing,
        kh3::Mode::WallKick { .. } if k.lean[0] < -0.7 => -facing,
        kh3::Mode::WallKick { .. } => facing,
        _ => return None,
    };
    let p = Vec3::from_array(game.sora.player.position);
    Some(boss::OverlayView {
        eye: p + n * WALL_CAMERA_BACK + Vec3::Y * WALL_CAMERA_UP,
        target: p + n * 60. + Vec3::Y * 40.,
        roll: 0.,
        fov,
        collision: true,
    })
}

/// [Test] The KH3 script's buttons for the room Sora of an unattended
/// KH3 test (the rival runs the same script half a cycle later).
pub(crate) fn scripted_room_pad(game: &Game, tick: u64) -> Option<(rival::RivalPad, Pad)> {
    if game.autoplay.is_none() || !on(game) || !game.rival_script || game.net_client {
        return None;
    }
    let foe = game.rivals.first()?.fighter.player.position;
    let pad = rival::kh3_pad(tick, 0, game.sora.player.position, foe);
    let kh3 = Pad {
        shotlock: pad.shotlock_held,
        situation: pad.flow,
    };
    Some((pad, kh3))
}

/// The HUD's sheets, made once.
#[derive(Resource, Default)]
pub(crate) struct Kh3Hud {
    sheets: Option<(usize, usize)>,
}

const BAR: [f32; 4] = [432., 356., 552., 361.];
const LABEL: [f32; 2] = [432., 343.];
const FOCUS_COLOUR: [f32; 4] = [0.45, 1.0, 1.6, 1.];
const BAR_BACK: [f32; 4] = [0., 0., 0., 0.6];
const TEXT: [f32; 4] = [1.6, 1.6, 1.6, 1.];
const LOCKS: [f32; 2] = [256., 150.];
const OFFER: [f32; 2] = [256., 166.];
const SIZE: f32 = 10.;

fn rect(r: [f32; 4], c: [f32; 4]) -> data::sprite_sequence::Quad {
    let [x0, y0, x1, y1] = r;
    data::sprite_sequence::Quad {
        corners: [[x0, y0], [x1, y0], [x0, y1], [x1, y1]],
        uv: [0, 0, 1, 1],
        colors: [c; 4],
        blend: 0,
    }
}

/// The focus gauge (above the HP gauge), the shotlock's locks while aiming
/// and the grand magic's offer, for this player's own Sora: plain text
/// and a bar on the 2D layer.
pub(crate) fn present(
    game: Res<Game>,
    hud: Option<Res<Hud>>,
    camp: Option<Res<CampRes>>,
    mut state: ResMut<Kh3Hud>,
    mut frame: ResMut<Ui2dFrame>,
    mut images: ResMut<Assets<Image>>,
) {
    if !on(&game) || camp.as_ref().is_some_and(|c| c.open.is_some()) {
        return;
    }
    let Some(font) = hud.as_ref().and_then(|h| h.menu.as_ref()).map(|m| &m.font) else {
        return;
    };
    let (sheet, white) = *state.sheets.get_or_insert_with(|| {
        (
            frame.add_sheet(
                &mut images,
                font.rgba.clone(),
                font.size,
                font.texel_scale as f32,
            ),
            frame.add_sheet(&mut images, vec![255; 16], [2, 2], 1.),
        )
    });
    let f = field_command_menu::Font {
        widths: &font.widths,
        pad: 1,
    };
    let k = &game.sora.kh3;
    let share = (k.focus / kh3::tuning::FOCUS_MAX).clamp(0., 1.);
    let fill = [BAR[0], BAR[1], BAR[0] + (BAR[2] - BAR[0]) * share, BAR[3]];
    frame.draws.push((
        white,
        vec![
            rect(
                [BAR[0] - 1., BAR[1] - 1., BAR[2] + 1., BAR[3] + 1.],
                BAR_BACK,
            ),
            rect(fill, FOCUS_COLOUR),
        ],
    ));
    let mut texts = vec![(
        format!("FOCUS {}", k.focus.round()),
        [LABEL[0], LABEL[1]],
        false,
    )];
    if let Some(aim) = k.aim {
        texts.push((
            format!("SHOTLOCK  {} / {}", aim.locks, kh3::tuning::MAX_LOCKS),
            LOCKS,
            true,
        ));
    }
    if let Some((spell, _)) = k.offer {
        let name = match spell {
            player::magic::FIRE => "Grand Fire",
            player::magic::BLIZZARD => "Grand Blizzard",
            _ => "Grand Thunder",
        };
        texts.push((format!("{name}   TRIANGLE"), OFFER, true));
    }
    let mut quads = Vec::new();
    for (text, at, centred) in texts {
        let encoded = pause_menu::encode_text(&text);
        let x = if centred {
            at[0] - field_command_menu::text_width(&encoded, &f, SIZE) / 2.
        } else {
            at[0]
        };
        quads.extend(field_command_menu::text_quads_sized(
            &encoded,
            [x, at[1]],
            TEXT,
            &f,
            SIZE,
        ));
    }
    frame.draws.push((sheet, quads));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn soras_stay_inside_the_walls() {
        let e = [-2200., -2200., 2200., 2200.];
        let p = contain([3000., 500., 0.], e, true, 40.);
        assert!((p[0] - 2160.).abs() < 1e-3 && p[1] == 500. && p[2].abs() < 1e-3);
        let p = contain([2000., 0., 2000.], e, true, 40.);
        assert!((p[0].hypot(p[2]) - 2160.).abs() < 1e-2, "round: {p:?}");
        assert_eq!(
            contain([2000., 0., 2000.], e, false, 40.),
            [2000., 0., 2000.]
        );
        assert_eq!(contain([5000., 0., -10.], e, false, 40.), [2160., 0., -10.]);
        assert_eq!(contain([10., 0., 10.], e, true, 40.), [10., 0., 10.]);
    }

    #[test]
    fn bolt_keys_stay_apart_from_bullets_and_spells() {
        let tag = (u64::from(1u8) + 1) << 40;
        let key = SHOT_KEYS | tag | 77;
        assert!(is_bolt(key));
        assert!(!is_bolt(1 << 60 | 5), "a Wisdom bullet");
        assert_eq!((key & 0xffff_ffff) as u32, 77);
        // A rival's effect keys flip bits 56-57 only.
        assert!(is_bolt(rival::tag_key(key, 1)));
    }
}
