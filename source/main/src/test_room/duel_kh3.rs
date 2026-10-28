//! The duel's KH3 ruleset in the room [T279]: both Soras tick through
//! `player::kh3` instead of the KH2 controller alone; its shotlock bolts ride
//! Wisdom Form's bullets (their effect, their hit volume, their place in
//! the snapshot); a grand magic is the room's own cast without MP; the
//! poles are the narrow pillars among the arena's walls; the lean (wall
//! run, pole, airstep) turns the model and its spheres; and a plain HUD
//! shows focus, the shotlock's locks and the grand magic offer. Single
//! player and the KH2 ruleset never come here.

use super::duel_wire::DuelEvent;
use super::*;
use player::kh3::{self, Foe, Pad, Scene, look::Look, surface::Pole};

/// Shotlock bolts' bullet keys: Wisdom's bullet range with this bit.
pub(crate) const SHOT_BIT: u64 = 1 << 59;
const SHOT_KEYS: u64 = 1 << 60 | SHOT_BIT;
/// Wisdom's MAGF bullet element, the bolts' look.
const BOLT_ELEMENT: u16 = 20;
/// Wisdom's ATKP sub-tables: King of Hearts' bolts take the plain shot's
/// (power 20), Ragnarok's the volley's last (25) [duel choice].
const KOH_SUB: u8 = 0;
pub(crate) const RAGNAROK_SUB: u8 = 3;
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

/// The room Sora's foe: the first rival, else Test KH3 mode's dummy.
pub(crate) fn room_foe(game: &Game) -> Option<Foe> {
    if let Some(r) = game.rivals.first() {
        return (r.fighter.vitals.hp > 0)
            .then(|| foe(r.fighter.player.position, rival::rival_centre(r)));
    }
    let rt = game
        .enemy
        .as_ref()
        .filter(|rt| game.kh3_dummy && rt.enabled && rt.enemy.targetable())?;
    let at = rt.enemy.position;
    // Its target shape's centre (the lock-on's point), else its middle.
    let centre = enemy_targets(rt)
        .first()
        .map_or([at[0], at[1] + DUMMY_MIDDLE, at[2]], |t| t.centre);
    Some(foe(at, centre))
}

/// [Test] Test KH3 mode: the dummy stands this far in front of Sora, and
/// its middle (without a target shape) is this high.
const DUMMY_DISTANCE: f32 = 500.;
const DUMMY_MIDDLE: f32 = 50.;

/// [Test] Test KH3 mode: the room's Shadow stands as the training dummy in
/// front of Sora, facing him.
pub(crate) fn dummy_setup(mut game: ResMut<Game>) {
    if !game.kh3_dummy {
        return;
    }
    let p = game.sora.player.position;
    let yaw = game.sora.player.yaw;
    let at = [
        p[0] + yaw.sin() * DUMMY_DISTANCE,
        p[1],
        p[2] + yaw.cos() * DUMMY_DISTANCE,
    ];
    let Some(rt) = game.enemy.as_mut() else {
        return;
    };
    rt.enabled = true;
    rt.enemy.dummy = true;
    rt.enemy.spawn = at;
    rt.enemy.position = at;
    rt.enemy.yaw = yaw + std::f32::consts::PI;
    let tick = game.tick;
    game.event(serde_json::json!({"event":"kh3_dummy","tick":tick,"position":at}));
}

/// [Test] Each hit the dummy took since the last tick is a hit Sora landed
/// (focus and style, as a hit on the other Sora gives).
fn dummy_hits(game: &mut Game) {
    let Some(n) = game
        .enemy
        .as_ref()
        .filter(|rt| game.kh3_dummy && rt.enemy.dummy)
        .map(|rt| rt.enemy.dummy_hits)
    else {
        return;
    };
    let new = n.saturating_sub(game.kh3_dummy_seen);
    game.kh3_dummy_seen = n;
    for _ in 0..new.min(8) {
        game.sora.kh3.landed(true);
    }
    if new > 0
        && let Some(rt) = game.enemy.as_ref()
    {
        let e = &rt.enemy;
        let (tick, last, run, hp, motion) = (game.tick, e.dummy_last, e.dummy_run, e.hp, e.motion);
        game.event(serde_json::json!({"event":"kh3_dummy_hit","tick":tick,
            "damage":last,"run":run,"hp":hp,"motion":motion}));
    }
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
    if slot == game.local_slot {
        dummy_hits(game);
    }
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
    for &name in out.began.iter().chain(&out.cues) {
        cues(game, name, slot, drive_fx::Anchor::Sora);
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

/// The sound cause of the KH3 cues (duel_fx sends it to the joiner with
/// the Soras' own sounds).
pub(crate) const CUE_SOUND: &str = "kh3";

/// The KH2 stand-ins of a KH3 moment (`player::kh3::cue`) for the Sora
/// swapped in (`game.sora`, seat `slot`, his own effect containers):
/// sounds to the queue, looks onto him (or `anchor`), each in the log. A
/// shotlock lock's look sits on the target.
pub(crate) fn cues(game: &mut Game, event: &'static str, slot: u8, anchor: drive_fx::Anchor) {
    use kh3::cue::Cue;
    let tick = game.tick;
    for &c in kh3::cue::of(event) {
        let played = match c {
            Cue::Sound(id) => {
                game.sound_queue.push((id, CUE_SOUND));
                true
            }
            Cue::Look(look) => {
                let at = match anchor {
                    drive_fx::Anchor::World { at, yaw, .. } => Some((at.to_array(), yaw)),
                    _ if look == Look::Lock => game.sora.kh3.foe.map(|f| (f, 0.)),
                    _ => None,
                };
                show(game, look, slot, at, 1.)
            }
        };
        game.event(
            serde_json::json!({"event":"kh3_cue","tick":tick,"slot":slot,
            "on":event,"cue":format!("{c:?}"),"played":played}),
        );
    }
}

/// [KH3 ruleset] A look's size by a hit's weight (the knock's scale,
/// 0.3 to 1.6): from 0.85 to 1.15 times its own, so a counter's burst stays
/// about Sora's height.
pub(crate) fn hit_size(strike: &kh3::hurt::Strike) -> f32 {
    (0.8 + strike.scale * 0.2).clamp(0.85, 1.15)
}

/// Starts a look for the Sora swapped in (seat `slot`: on him, or at a
/// place, turned by a yaw) and sends it to the joiner by its id. Whether
/// any of its layers started.
pub(crate) fn show(
    game: &mut Game,
    look: Look,
    slot: u8,
    at: Option<([f32; 3], f32)>,
    size: f32,
) -> bool {
    let anchor = anchor_of(game, slot, at);
    let size = look_size(size);
    let Some(d) = game.drive.as_mut() else {
        return false;
    };
    let (starts, missing) = look_starts(d, look, anchor, f32::from(size) / 10.);
    let started = !starts.is_empty();
    game.fx_styled.extend(starts);
    game.duel_events.push(DuelEvent::Look {
        look: look.id(),
        slot,
        size,
        at,
    });
    log_look(game, look, slot, missing);
    started
}

/// A look's size on the wire, in tenths.
fn look_size(size: f32) -> u8 {
    (size * 10.).round().clamp(1., 40.) as u8
}

/// Where a look goes for seat `slot`: on him (the room Sora's own seat, or
/// a rival's) or at a place.
fn anchor_of(game: &Game, slot: u8, at: Option<([f32; 3], f32)>) -> drive_fx::Anchor {
    match at {
        Some((at, yaw)) => drive_fx::Anchor::World {
            at: Vec3::from_array(at),
            pitch: 0.,
            yaw,
        },
        None if slot == game.local_slot => drive_fx::Anchor::Sora,
        None => drive_fx::Anchor::Rival(slot),
    }
}

fn log_look(game: &mut Game, look: Look, slot: u8, missing: Vec<String>) {
    let tick = game.tick;
    game.event(
        serde_json::json!({"event":"kh3_look","tick":tick,"slot":slot,
        "look":look.name(),"missing":missing}),
    );
}

/// A look's layers as styled starts from a Sora's effect containers (his
/// Drive state), with the layers whose container or element is not there
/// (a form kit still loading).
pub(crate) fn look_starts(
    d: &mut drive_room::DriveRt,
    look: Look,
    anchor: drive_fx::Anchor,
    size: f32,
) -> (Vec<(drive_fx::Placed, drive_fx::Style)>, Vec<String>) {
    layer_starts(d, look.layers(), anchor, size)
}

/// Layers as styled starts (`look_starts`).
fn layer_starts(
    d: &mut drive_room::DriveRt,
    layers: &[kh3::look::Layer],
    anchor: drive_fx::Anchor,
    size: f32,
) -> (Vec<(drive_fx::Placed, drive_fx::Style)>, Vec<String>) {
    use kh3::look::Source;
    let mut starts = Vec::new();
    let mut missing = Vec::new();
    for l in layers {
        let found = match l.source {
            Source::Sora => d.paxes.get(&player::forms::NORMAL).cloned().map(|p| (p, 0)),
            Source::Form(f) => d.form_pax(f).map(|p| (p, f)),
            Source::System => d.src.system_pax.clone().map(|p| (p, 0xff)),
        };
        match found {
            Some((pax, owner)) if pax.by_id(l.element).is_some() => {
                starts.push((
                    drive_fx::Placed {
                        req: drive_fx::FxRequest {
                            pax,
                            id: l.element,
                            owner,
                        },
                        anchor,
                    },
                    style_of(l, size),
                ));
            }
            _ => missing.push(format!("{:?}:{}", l.source, l.element)),
        }
    }
    (starts, missing)
}

/// A layer's style at a look's size.
pub(crate) fn style_of(l: &kh3::look::Layer, size: f32) -> drive_fx::Style {
    drive_fx::Style {
        scale: l.scale * size,
        tint: l.tint,
        offset: l.offset.map(|v| v * size),
        bone: l.bone,
        delay: f32::from(l.delay),
        life: (l.life > 0).then_some(f32::from(l.life)),
        sound: l.sound,
    }
}

/// Joiner: the host's look on this instance's Sora of that seat (or at
/// its place), from that Sora's own containers.
pub(crate) fn play_look(game: &mut Game, id: u8, slot: u8, size: u8, at: Option<([f32; 3], f32)>) {
    let Some(look) = Look::from_id(id) else {
        return;
    };
    let anchor = anchor_of(game, slot, at);
    let size = f32::from(size) / 10.;
    let local = slot == game.local_slot;
    let drive = if local {
        game.drive.as_mut()
    } else {
        game.rivals
            .iter_mut()
            .find(|r| r.slot == slot)
            .and_then(|r| r.own.drive.as_mut())
    };
    let Some(d) = drive else {
        return;
    };
    let (starts, missing) = look_starts(d, look, anchor, size);
    game.fx_styled.extend(starts);
    log_look(game, look, slot, missing);
}

/// [Test] `--kh3-preview <list>`: both Soras stand still in the KH3 test
/// while the room Sora shows, one every `PREVIEW_EVERY` ticks, each item
/// of the list (`looks`, `look:<name>`, `sora:<id>`, `form<n>:<id>`,
/// `system:<id>`, ids as `a-b` ranges too), seen from his side; three
/// captures each. For checking the looks and choosing their elements.
pub(crate) struct Preview {
    items: Vec<PreviewItem>,
}

#[derive(Clone, Copy, Debug)]
enum PreviewItem {
    Look(Look),
    Element(kh3::look::Source, u16),
}

/// When the first item shows (the forms' kits have loaded by then) and
/// how far apart the items are.
const PREVIEW_START: u64 = 600;
const PREVIEW_EVERY: u64 = 60;
/// Ticks after an item's start of its captures.
const PREVIEW_SHOTS: [u64; 3] = [3, 10, 24];

impl Preview {
    pub(crate) fn parse(spec: &str) -> Result<Preview, String> {
        use kh3::look::{ALL, Source};
        let mut items = Vec::new();
        for token in spec.split(',').map(str::trim).filter(|t| !t.is_empty()) {
            if token == "looks" {
                items.extend(ALL.iter().map(|&l| PreviewItem::Look(l)));
                continue;
            }
            let (kind, rest) = token
                .split_once(':')
                .ok_or_else(|| format!("preview item {token}"))?;
            if kind == "look" {
                let l = ALL
                    .iter()
                    .find(|l| l.name() == rest)
                    .ok_or_else(|| format!("no look {rest}"))?;
                items.push(PreviewItem::Look(*l));
                continue;
            }
            let source = match kind {
                "sora" => Source::Sora,
                "system" => Source::System,
                k => Source::Form(
                    k.strip_prefix("form")
                        .and_then(|n| n.parse().ok())
                        .ok_or_else(|| format!("preview source {k}"))?,
                ),
            };
            let (a, b) = rest.split_once('-').unwrap_or((rest, rest));
            let (a, b): (u16, u16) = (
                a.parse().map_err(|_| format!("element {a}"))?,
                b.parse().map_err(|_| format!("element {b}"))?,
            );
            items.extend((a..=b).map(|e| PreviewItem::Element(source, e)));
        }
        if items.is_empty() {
            return Err("an empty preview list".into());
        }
        Ok(Preview { items })
    }

    /// The run's captures, in the items' order.
    pub(crate) fn captures(&self) -> Vec<u64> {
        (0..self.items.len() as u64)
            .flat_map(|i| PREVIEW_SHOTS.map(|s| PREVIEW_START + i * PREVIEW_EVERY + s))
            .collect()
    }

    pub(crate) fn end(&self) -> u64 {
        PREVIEW_START + self.items.len() as u64 * PREVIEW_EVERY + 2
    }
}

/// [Test] The preview's item of this tick, on the room Sora.
pub(crate) fn preview_tick(game: &mut Game, tick: u64) {
    let Some(item) = game.kh3_preview.as_ref().and_then(|p| {
        let i = tick.checked_sub(PREVIEW_START)?;
        i.is_multiple_of(PREVIEW_EVERY)
            .then(|| p.items.get((i / PREVIEW_EVERY) as usize).copied())
            .flatten()
    }) else {
        return;
    };
    let slot = game.local_slot;
    // Looks that land on a target or a hit: a little in front of him.
    let p = game.sora.player.position;
    let yaw = game.sora.player.yaw;
    let front = (
        [p[0] + yaw.sin() * 40., p[1] + 90., p[2] + yaw.cos() * 40.],
        yaw,
    );
    let started = match item {
        PreviewItem::Look(l) => {
            let at = matches!(l, Look::Lock | Look::BoltHit | Look::HeavyHit).then_some(front);
            show(game, l, slot, at, 1.)
        }
        PreviewItem::Element(source, element) => {
            let layer = kh3::look::Layer {
                source,
                element,
                bone: None,
                offset: [0.; 3],
                scale: 1.,
                tint: [1.; 4],
                delay: 0,
                // Its loops end before the next item.
                life: PREVIEW_EVERY as u16 - 15,
                sound: false,
            };
            let look = std::slice::from_ref(&layer);
            match game.drive.as_mut() {
                Some(d) => {
                    let (starts, _) = layer_starts(d, look, drive_fx::Anchor::Sora, 1.);
                    let ok = !starts.is_empty();
                    game.fx_styled.extend(starts);
                    ok
                }
                None => false,
            }
        }
    };
    game.event(serde_json::json!({"event":"kh3_preview","tick":tick,
        "item":format!("{item:?}"),"started":started}));
}

/// [Test] The preview's camera: from Sora's right, a little ahead, level
/// with his chest.
fn preview_view(game: &Game, fov: f32) -> Option<boss::OverlayView> {
    game.kh3_preview.as_ref()?;
    let yaw = game.sora.player.yaw;
    let (facing, right) = (
        Vec3::new(yaw.sin(), 0., yaw.cos()),
        Vec3::new(yaw.cos(), 0., -yaw.sin()),
    );
    let p = Vec3::from_array(game.sora.player.position);
    Some(boss::OverlayView {
        eye: p + right * 380. + facing * 120. + Vec3::Y * 130.,
        target: p + Vec3::Y * 85.,
        roll: 0.,
        fov,
        collision: false,
    })
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
        fx_styled,
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
                    fx_styled.push((
                        drive_fx::Placed {
                            req: drive_fx::FxRequest {
                                pax,
                                id: BOLT_ELEMENT,
                                owner: player::forms::WISDOM,
                            },
                            anchor: drive_fx::Anchor::Bullet(key),
                        },
                        bullet_style(key),
                    ));
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

/// [duel choice] A bolt rides Wisdom's orb, cyan and a little smaller, so
/// a volley never reads as Wisdom Form's own shots (both can be in one
/// duel). Other bullets keep their look.
pub(crate) fn bullet_style(key: u64) -> drive_fx::Style {
    if is_bolt(key) {
        drive_fx::Style {
            tint: kh3::look::colour::LOCK,
            scale: BOLT_SCALE,
            ..drive_fx::Style::default()
        }
    } else {
        drive_fx::Style::default()
    }
}

const BOLT_SCALE: f32 = 0.8;

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
    if let Some(v) = preview_view(game, fov) {
        return Some(v);
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

/// What the shotlock aims at while it aims or its volley flies (the aiming
/// camera's target, kh3_camera.rs): the other Sora or the dummy, else a
/// point ahead.
pub(crate) fn aim_point(game: &Game) -> Option<Vec3> {
    let k = &game.sora.kh3;
    if k.aim.is_none() && !matches!(k.mode, kh3::Mode::Volley { .. }) {
        return None;
    }
    let p = Vec3::from_array(game.sora.player.position);
    let yaw = game.sora.player.yaw;
    Some(k.foe.map(Vec3::from_array).unwrap_or_else(|| {
        p + Vec3::new(yaw.sin(), 0., yaw.cos()) * 1000. + Vec3::Y * kh3::tuning::LOCK_LOOK_UP
    }))
}

/// [Test] The KH3 script's buttons for the room Sora of an unattended
/// KH3 test (the rival runs the same script half a cycle later).
pub(crate) fn scripted_room_pad(game: &Game, tick: u64) -> Option<(rival::RivalPad, Pad)> {
    if game.autoplay.is_none() || !on(game) || game.net_client {
        return None;
    }
    // [Test] Unattended Test KH3 mode: the script's acting half against the
    // dummy (its taking half stands, while kh3_menu's script runs).
    if game.kh3_dummy {
        let foe = room_foe(game)?;
        let view = rival::Kh3View::default();
        let pad = rival::kh3_pad(tick, 0, game.sora.player.position, foe.position, view);
        let kh3 = Pad {
            shotlock: pad.shotlock_held,
            situation: pad.flow,
        };
        return Some((pad, kh3));
    }
    if !game.rival_script {
        return None;
    }

    // The preview stands him still.
    if game.kh3_preview.is_some() {
        return Some((rival::RivalPad::default(), Pad::default()));
    }
    let foe = game.rivals.first()?.fighter.player.position;
    let view = rival::kh3_view(&game.sora, game.rivals.first().map(|r| &r.fighter));
    let pad = rival::kh3_pad(tick, 0, game.sora.player.position, foe, view);
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
const DUMMY_TEXT: [f32; 2] = [432., 330.];
const MENU_TEXT: [f32; 2] = [432., 317.];

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
            if aim.syphon {
                format!(
                    "FOCUS SYPHON  {} / {}",
                    aim.locks,
                    kh3::tuning::SYPHON_LOCKS
                )
            } else {
                format!("SHOTLOCK  {} / {}", aim.locks, kh3::tuning::MAX_LOCKS)
            },
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
    // [Test] Test KH3 mode: the dummy's damage (the last hit, the run of
    // hits now going) and the menu's button.
    if game.kh3_dummy {
        if let Some(e) = game.enemy.as_ref().map(|rt| &rt.enemy)
            && e.dummy_hits > 0
        {
            let run = if e.dummy_idle <= player::shadow::DUMMY_RUN_GAP {
                e.dummy_run
            } else {
                0
            };
            texts.push((
                format!("DUMMY  HIT {}   COMBO {}", e.dummy_last, run),
                DUMMY_TEXT,
                false,
            ));
        }
        texts.push(("START  KH3 ABILITIES".into(), MENU_TEXT, false));
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

    #[test]
    fn bolts_are_cyan_and_wisdom_s_shots_keep_their_look() {
        let bolt = SHOT_KEYS | (2 << 40) | 9;
        let style = bullet_style(bolt);
        assert_eq!(style.tint, kh3::look::colour::LOCK);
        assert!(style.scale < 1.);
        assert_eq!(bullet_style(rival::tag_key(bolt, 1)), style);
        assert_eq!(bullet_style(1 << 60 | 5), drive_fx::Style::default());
    }

    #[test]
    fn a_heavier_hit_has_a_bigger_burst_within_bounds() {
        use kh3::hurt::{Knock, Strike};
        let size = |scale| {
            hit_size(&Strike {
                knock: Knock::Blow,
                scale,
            })
        };
        assert!(size(0.6) < size(1.2) && size(1.2) < size(1.6));
        assert!((0.85..=1.15).contains(&size(0.)) && (0.85..=1.15).contains(&size(3.)));
    }

    #[test]
    fn a_layer_s_style_follows_the_look_s_size() {
        let l = kh3::look::Look::Airstep.layers()[0];
        let s = style_of(&l, 1.1);
        assert!((s.scale - l.scale * 1.1).abs() < 1e-6);
        assert!((s.offset[1] - l.offset[1] * 1.1).abs() < 1e-4);
        assert_eq!(s.life, Some(f32::from(l.life)));
        assert_eq!((s.tint, s.bone, s.sound), (l.tint, l.bone, false));
        // Its size goes in tenths on the wire.
        assert_eq!(look_size(1.15), 12);
        assert_eq!(look_size(0.), 1);
    }
}
