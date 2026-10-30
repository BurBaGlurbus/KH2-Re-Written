//! The KH3 ruleset on KH3's own Sora (`kh3_sora`, a port of StrawHatPro's kh-core): when the player's KH3 files
//! are read the whole simulation (controller, movement component, states, combat, magic, status) is that crate's;
//! this file is its host seam in the duel: the room's triangles as its world, the other Sora as its target, and
//! its results copied onto `Player` and `Kh3` for the room and the presenters.
//!
//! Axes: KH3 is X forward, Y right, Z up, left-handed, in centimetres; the room is x, y up, z, right-handed, where
//! a Sora at yaw 0 faces +z and his right is -x. So X = z, Y = -x, Z = y, and a room yaw is minus KH3's (in
//! radians). KH3's Sora is drawn 160 units tall for 180 cm (`tuning::SCALE`): one cm is `SCALE` units.

use super::tuning::SCALE;
use super::{Jump, Kh3, Knock, Mode, Pad, Scene, spell, surface};
use crate::{Action, Input, Player, Vec3 as RoomVec3};
use kh3_sora as ks;
use ks::host::{
    Attach, BodyReaction, EventKind, FinishKind, HitRecord, HitResult, HurtShape, PushBody,
    ReactionResult, TargetBody, TargetDefense, TargetFlags, TargetId, TargetInfo, Targets,
};
use ks::input::{PadButton, PadButtons, PadState};
use ks::states::{HitOnPlayer, StateId};
use ks::{Hit, Vec3, World};
use std::collections::HashMap;
use std::sync::Arc;

/// A room point in KH3's space.
pub fn to_core(v: RoomVec3) -> Vec3 {
    Vec3::new(v[2], -v[0], v[1]) / SCALE
}

/// A KH3 point in the room.
pub fn from_core(v: Vec3) -> RoomVec3 {
    [-v.y * SCALE, v.z * SCALE, v.x * SCALE]
}

/// A room direction (a stick's world direction, no scale).
fn dir_to_core(d: [f32; 2]) -> (f32, f32) {
    // (forward = X = z, right = Y = -x)
    (d[1], -d[0])
}

pub fn yaw_to_core(yaw: f32) -> f32 {
    ks::sora::wrap_degrees(-yaw.to_degrees())
}

pub fn yaw_from_core(yaw: f32) -> f32 {
    -yaw.to_radians()
}

#[derive(Debug)]
struct Tri {
    a: Vec3,
    b: Vec3,
    c: Vec3,
    n: Vec3,
    component: u32,
    min: Vec3,
    max: Vec3,
}

/// The room's collision in KH3's space: every sweep and line the core asks. Built once a room.
#[derive(Debug)]
pub struct Geometry {
    tris: Vec<Tri>,
    grid: HashMap<(i32, i32), Vec<u32>>,
    poles: Vec<ks::Pole>,
    /// The first component of the arena's invisible walls.
    bounds_from: u32,
}

/// [duel choice] How high the invisible wall above an arena's own walls goes: KH3's flow moves reach heights the
/// KH2 arenas were not built for, and a level of KH3's has such walls too.
const BOUND_TOP: f32 = 20000.;
const BOUND_BOTTOM: f32 = -3000.;

const CELL: f32 = 400.;
const SKIN: f32 = 0.01;

fn cell_of(v: f32) -> i32 {
    (v / CELL).floor() as i32
}

/// The closest points of a segment and a triangle.
fn segment_triangle(p: Vec3, q: Vec3, t: &Tri) -> (f32, Vec3, Vec3) {
    let d = q - p;
    let mut best = (f32::MAX, p, t.a);
    let mut consider = |s: Vec3, on: Vec3| {
        let l = (s - on).length_squared();
        if l < best.0 {
            best = (l, s, on);
        }
    };
    // The segment through the face.
    let denom = t.n.dot(d);
    if denom.abs() > 1e-6 {
        let k = t.n.dot(t.a - p) / denom;
        if (0. ..=1.).contains(&k) {
            let s = p + d * k;
            if inside(s, t) {
                consider(s, s);
            }
        }
    }
    // Each end over the face.
    for s in [p, q] {
        let on = s - t.n * t.n.dot(s - t.a);
        if inside(on, t) {
            consider(s, on);
        }
    }
    // Against each edge.
    for (a, b) in [(t.a, t.b), (t.b, t.c), (t.c, t.a)] {
        let (s, on) = segment_segment(p, q, a, b);
        consider(s, on);
    }
    (best.0.sqrt(), best.1, best.2)
}

fn inside(s: Vec3, t: &Tri) -> bool {
    let edge = |a: Vec3, b: Vec3| (b - a).cross(s - a).dot(t.n);
    edge(t.a, t.b) >= -1e-3 && edge(t.b, t.c) >= -1e-3 && edge(t.c, t.a) >= -1e-3
}

fn segment_segment(p1: Vec3, q1: Vec3, p2: Vec3, q2: Vec3) -> (Vec3, Vec3) {
    let (d1, d2, r) = (q1 - p1, q2 - p2, p1 - p2);
    let (a, e, f) = (d1.dot(d1), d2.dot(d2), d2.dot(r));
    let (s, t);
    if a <= 1e-8 && e <= 1e-8 {
        (s, t) = (0., 0.);
    } else if a <= 1e-8 {
        s = 0.;
        t = (f / e).clamp(0., 1.);
    } else {
        let c = d1.dot(r);
        if e <= 1e-8 {
            t = 0.;
            s = (-c / a).clamp(0., 1.);
        } else {
            let b = d1.dot(d2);
            let denom = a * e - b * b;
            let mut ss = if denom > 1e-8 {
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
    (p1 + d1 * s, p2 + d2 * t)
}

impl Geometry {
    /// From the room's triangles (its coordinates) and its poles.
    ///
    /// `bounds`: the outline the Soras stay inside (room x, z), walled from far below to far above.
    pub fn build(
        triangles: impl Iterator<Item = [RoomVec3; 3]>,
        poles: &[surface::Pole],
        bounds: &[[f32; 2]],
    ) -> Geometry {
        let mut tris = Vec::new();
        let mut planes: HashMap<(i32, i32, i32, i32), u32> = HashMap::new();
        let add = |t: [RoomVec3; 3],
                   tris: &mut Vec<Tri>,
                   planes: &mut HashMap<(i32, i32, i32, i32), u32>| {
            let (a, b, c) = (to_core(t[0]), to_core(t[1]), to_core(t[2]));
            let n = (b - a).cross(c - a);
            if n.length_squared() < 1e-6 {
                return;
            }
            let n = n.normalize();
            // Triangles of one plane are one component: the edge checks ignore "the floor's component".
            let up = if n.z < -1e-3 { -n } else { n };
            let key = (
                (up.x * 100.).round() as i32,
                (up.y * 100.).round() as i32,
                (up.z * 100.).round() as i32,
                (up.dot(a) * 0.5).round() as i32,
            );
            let next = planes.len() as u32 + 1;
            let component = *planes.entry(key).or_insert(next);
            tris.push(Tri {
                a,
                b,
                c,
                n,
                component,
                min: a.min(b).min(c),
                max: a.max(b).max(c),
            });
        };
        for t in triangles {
            add(t, &mut tris, &mut planes);
        }
        let bounds_from = planes.len() as u32 + 1;
        for k in 0..bounds.len() {
            let (p, q) = (bounds[k], bounds[(k + 1) % bounds.len()]);
            let at = |v: [f32; 2], y: f32| [v[0], y * SCALE, v[1]];
            add(
                [at(p, BOUND_BOTTOM), at(q, BOUND_BOTTOM), at(q, BOUND_TOP)],
                &mut tris,
                &mut planes,
            );
            add(
                [at(p, BOUND_BOTTOM), at(q, BOUND_TOP), at(p, BOUND_TOP)],
                &mut tris,
                &mut planes,
            );
        }
        let mut grid: HashMap<(i32, i32), Vec<u32>> = HashMap::new();
        for (i, t) in tris.iter().enumerate() {
            for x in cell_of(t.min.x)..=cell_of(t.max.x) {
                for y in cell_of(t.min.y)..=cell_of(t.max.y) {
                    grid.entry((x, y)).or_default().push(i as u32);
                }
            }
        }
        let poles = poles
            .iter()
            .enumerate()
            .map(|(i, p)| {
                let bottom = to_core([p.centre[0], p.bottom, p.centre[1]]);
                let top = to_core([p.centre[0], p.top, p.centre[1]]);
                ks::Pole {
                    id: i as u32 + 1,
                    base1: bottom,
                    base2: top,
                    check_radius: 50.,
                    attach_radius: p.radius / SCALE + 55.,
                    horizontal: false,
                    active: true,
                }
            })
            .collect();
        Geometry {
            tris,
            grid,
            poles,
            bounds_from,
        }
    }

    fn candidates(&self, min: Vec3, max: Vec3, out: &mut Vec<u32>) {
        out.clear();
        for x in cell_of(min.x)..=cell_of(max.x) {
            for y in cell_of(min.y)..=cell_of(max.y) {
                if let Some(list) = self.grid.get(&(x, y)) {
                    out.extend_from_slice(list);
                }
            }
        }
        out.sort_unstable();
        out.dedup();
    }

    /// The first contact of the capsule with one triangle.
    fn sweep_one(&self, t: &Tri, start: Vec3, motion: Vec3, radius: f32, half: f32) -> Option<Hit> {
        let axis = Vec3::Z * (half - radius).max(0.);
        let length = motion.length();
        let moves = length > 1e-5;
        let (d0, s0, on0) = segment_triangle(start - axis, start + axis, t);
        if d0 < radius - SKIN {
            // An overlap at the start: reported when the move goes into it.
            let normal = if d0 > 1e-4 {
                (s0 - on0) / d0
            } else if t.n.dot(start - t.a) >= 0. {
                t.n
            } else {
                -t.n
            };
            if moves && motion.dot(normal) >= 0. {
                return None;
            }
            return Some(Hit {
                time: 0.,
                position: on0,
                normal,
                impact_normal: Self::impact(t, s0, normal),
                start_penetrating: true,
                penetration_depth: radius - d0,
                component: t.component,
            });
        }
        if !moves {
            return None;
        }
        // Conservative advancement: the capsule closes on the triangle by at most its speed.
        let mut k = 0f32;
        let mut near = (d0, s0, on0);
        for _ in 0..48 {
            if near.0 <= radius + SKIN {
                let normal = if near.0 > 1e-4 {
                    (near.1 - near.2) / near.0
                } else {
                    -motion / length
                };
                if motion.dot(normal) >= 0. {
                    // Moving along or away: no blocking contact.
                    return None;
                }
                return Some(Hit {
                    time: k,
                    position: near.2,
                    normal,
                    impact_normal: Self::impact(t, near.1, normal),
                    start_penetrating: false,
                    penetration_depth: 0.,
                    component: t.component,
                });
            }
            let step = (near.0 - radius).max(SKIN * 0.5) / length;
            k += step;
            if k > 1. {
                return None;
            }
            let c = start + motion * k;
            near = segment_triangle(c - axis, c + axis, t);
        }
        None
    }

    /// The surface's own normal, out of the side the shape is on.
    fn impact(t: &Tri, on: Vec3, normal: Vec3) -> Vec3 {
        if t.n.dot(on - t.a) > 0. || t.n.dot(normal) > 0. {
            t.n
        } else {
            -t.n
        }
    }

    fn sweep_list(&self, start: Vec3, end: Vec3, radius: f32, half: f32) -> Vec<Hit> {
        let pad = Vec3::new(radius + 1., radius + 1., half + 1.);
        let (min, max) = (start.min(end) - pad, start.max(end) + pad);
        let mut list = Vec::new();
        self.candidates(min, max, &mut list);
        let motion = end - start;
        let mut hits: Vec<Hit> = list
            .iter()
            .filter_map(|&i| {
                let t = &self.tris[i as usize];
                let overlap = t.max.cmpge(min).all() && t.min.cmple(max).all();
                overlap.then(|| self.sweep_one(t, start, motion, radius, half))?
            })
            .collect();
        hits.sort_by(|a, b| a.time.total_cmp(&b.time));
        hits
    }
}

impl World for Geometry {
    fn sweep_capsule(&self, start: Vec3, end: Vec3, radius: f32, half_height: f32) -> Option<Hit> {
        self.sweep_list(start, end, radius, half_height)
            .into_iter()
            .next()
    }

    fn sweep_capsule_all(&self, start: Vec3, end: Vec3, radius: f32, half_height: f32) -> Vec<Hit> {
        let mut seen: Vec<u32> = Vec::new();
        self.sweep_list(start, end, radius, half_height)
            .into_iter()
            .filter(|h| {
                let new = !seen.contains(&h.component);
                seen.push(h.component);
                new
            })
            .collect()
    }

    fn line_trace(&self, start: Vec3, end: Vec3) -> Option<Hit> {
        let (min, max) = (start.min(end) - Vec3::ONE, start.max(end) + Vec3::ONE);
        let mut list = Vec::new();
        self.candidates(min, max, &mut list);
        let d = end - start;
        let mut best: Option<Hit> = None;
        for &i in &list {
            let t = &self.tris[i as usize];
            let denom = t.n.dot(d);
            if denom.abs() < 1e-7 {
                continue;
            }
            let k = t.n.dot(t.a - start) / denom;
            if !(0. ..=1.).contains(&k) || best.is_some_and(|b| b.time <= k) {
                continue;
            }
            let on = start + d * k;
            if inside(on, t) {
                let normal = if denom < 0. { t.n } else { -t.n };
                best = Some(Hit {
                    time: k,
                    position: on,
                    normal,
                    impact_normal: normal,
                    start_penetrating: false,
                    penetration_depth: 0.,
                    component: t.component,
                });
            }
        }
        best
    }

    /// [duel choice] The arenas are KH2's: they have no wall-run or dangle marks, so every wall can be run on,
    /// kicked off and every top edge hung from.
    fn surface_flags(&self, component: u32) -> u32 {
        if component >= self.bounds_from {
            ks::surface::WALL_KICK
        } else {
            ks::surface::WALL_RUN | ks::surface::WALL_KICK | ks::surface::DANGLE
        }
    }

    fn poles(&self) -> &[ks::Pole] {
        &self.poles
    }
}

/// The other Sora (or the test dummy), as this Sora's target.
#[derive(Clone, Copy, Debug)]
pub struct FoeBody {
    pub position: Vec3,
    pub yaw: f32,
    pub velocity: Vec3,
    pub hp: f32,
    pub hp_max: f32,
    pub airborne: bool,
    pub alive: bool,
    /// His reaction type toward a hit (`Sora::target_body`).
    pub body: TargetBody,
    pub defense: TargetDefense,
    pub reaction_state: u8,
    pub push_level: u8,
    pub rush_suppression: bool,
}

const FOE: TargetId = TargetId(1);
/// The body capsule: `TresBodyCollPrimitive2`, radius 50, half height 90, 91 above the feet.
const BODY_RADIUS: f32 = 50.;
const BODY_HALF: f32 = 90.;
const BODY_CENTRE: f32 = 91.;
/// The push body (`TresRoot`'s capsule in the reference: combat strand B.4).
const PUSH_RADIUS: f32 = 60.;
const PUSH_HALF: f32 = 89.;

struct CoreTargets<'a> {
    foe: Option<FoeBody>,
    hits: &'a mut Vec<HitRecord>,
}

impl Targets for CoreTargets<'_> {
    fn enumerate(&self, out: &mut Vec<TargetInfo>) {
        if let Some(i) = self.info(FOE) {
            out.push(i);
        }
    }

    fn info(&self, id: TargetId) -> Option<TargetInfo> {
        let f = self.foe.filter(|_| id == FOE)?;
        let mut flags = TargetFlags::HITTABLE | TargetFlags::LOCKABLE | TargetFlags::ENEMY;
        if f.alive {
            flags |= TargetFlags::ALIVE;
        }
        if f.airborne {
            flags |= TargetFlags::AIRBORNE;
        }
        Some(TargetInfo {
            id: FOE,
            position: f.position,
            center: f.position + Vec3::Z * BODY_CENTRE,
            velocity: f.velocity,
            yaw: f.yaw,
            radius: BODY_RADIUS,
            half_height: BODY_HALF,
            flags: TargetFlags(flags),
            hp: f.hp,
            hp_max: f.hp_max,
        })
    }

    fn hurt_shapes(&self, id: TargetId, out: &mut Vec<HurtShape>) {
        if let Some(f) = self.foe.filter(|_| id == FOE) {
            let c = f.position + Vec3::Z * BODY_CENTRE;
            let d = Vec3::Z * (BODY_HALF - BODY_RADIUS);
            out.push(HurtShape {
                a: c - d,
                b: c + d,
                radius: BODY_RADIUS,
                part: 0,
            });
        }
    }

    fn apply_hit(&mut self, hit: &HitRecord) -> HitResult {
        let Some(f) = self.foe.filter(|f| hit.target == FOE && f.alive) else {
            return HitResult::default();
        };
        self.hits.push(*hit);
        let taken = if hit.takes_hp() { hit.damage } else { 0. };
        HitResult {
            accepted: true,
            damage_dealt: taken,
            hp_after: (f.hp - taken).max(0.),
            killed: taken >= f.hp && f.hp > 0.,
            guarded: !hit.takes_hp(),
        }
    }

    fn defense(&self, id: TargetId) -> Option<TargetDefense> {
        self.foe.filter(|_| id == FOE).map(|f| f.defense)
    }

    /// A duel's other Sora counts as the enemy engaged with him: the battle stance, as long as he lives.
    fn engaged_count(&self) -> u32 {
        u32::from(self.foe.is_some_and(|f| f.alive))
    }

    fn body(&self, id: TargetId, _part: u32) -> TargetBody {
        self.foe
            .filter(|_| id == FOE)
            .map_or(TargetBody::DEFAULT, |f| f.body)
    }

    fn push_body(&self, id: TargetId) -> Option<PushBody> {
        self.foe.filter(|_| id == FOE).map(|f| PushBody {
            center: f.position + Vec3::Z * BODY_CENTRE,
            radius: PUSH_RADIUS,
            half_height: PUSH_HALF,
            level: f.push_level,
        })
    }

    fn rush_suppression(&self, id: TargetId) -> bool {
        self.foe
            .filter(|_| id == FOE)
            .is_some_and(|f| f.rush_suppression)
    }

    fn reaction_state(&self, id: TargetId) -> u8 {
        self.foe
            .filter(|_| id == FOE)
            .map_or(0, |f| f.reaction_state)
    }

    fn finish_kind(&self, _id: TargetId) -> FinishKind {
        FinishKind::None
    }
}

/// KH3's Sora in a `Kh3` (a save state is not compared).
#[derive(Clone, Debug, Default)]
pub struct CoreSlot(pub Option<Box<ks::Sora>>);

impl PartialEq for CoreSlot {
    fn eq(&self, other: &Self) -> bool {
        self.0.is_some() == other.0.is_some()
    }
}

impl std::ops::Deref for CoreSlot {
    type Target = Option<Box<ks::Sora>>;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl std::ops::DerefMut for CoreSlot {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

/// What the room reads of a step besides the copies on `Player` and `Kh3`.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct CoreOut {
    /// Hits this Sora landed, for the room to deliver.
    pub hits: Vec<HitRecord>,
    /// Effects and sounds the core asked for, by name, at a point of the room (notifies' own are the
    /// presenter's: `kh3_fx` replays them from the clip).
    pub spawned: Vec<Spawned>,
    /// He died this step.
    pub died: bool,
    /// The states he left and entered this step.
    pub states: Vec<(&'static str, &'static str)>,
    pub on_floor: bool,
    pub velocity: [f32; 3],
    pub position: [f32; 3],
    pub row: &'static str,
    /// `Effects::stop`: the handle, and whether at once.
    pub stopped: Vec<(u32, bool)>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Spawned {
    pub name: &'static str,
    /// The seam's id: says what a name of a spell actor's class means.
    pub id: u32,
    pub at: RoomVec3,
    pub yaw: f32,
    pub sound: bool,
    pub persistent: bool,
    /// On Sora: at this bone or socket (none: the actor's origin), `offset` in its frame, cm.
    pub on_sora: bool,
    pub bone: Option<&'static str>,
    pub offset: [f32; 3],
    pub handle: u32,
}

/// The fire effects of a spell actor's class (`m_FireEffects`), in `Spawned::id`.
pub const FIRE_EFFECTS: u32 = ks::magic::projectile::FIRE_EFFECTS_ID;
/// A class's own hit sounds on a pawn and on the map.
pub const PAWN_HIT: u32 = ks::host::audio::ACTOR_PAWN_HIT.0;
pub const MAP_HIT: u32 = ks::host::audio::ACTOR_MAP_HIT.0;

/// A level 99 Sora at `at`: the abilities equipped are the duel's list.
pub fn new_sora(at: RoomVec3, yaw: f32, set: &super::ability::Set) -> Box<ks::Sora> {
    let mut sora = ks::Sora::new(
        *ks::SORA_PARAMS,
        ks::Pose {
            position: to_core(at),
            yaw: yaw_to_core(yaw),
        },
    )
    .with_tuning(ks::Tuning::GAME_EXACT);
    sora.set_tuning(ks::Tuning::GAME_EXACT);
    match loadout(set) {
        Ok(l) => *sora.status_mut() = ks::status::Status::from_loadout(&l),
        Err(why) => eprintln!("KH3 Sora's loadout: {why}"),
    }
    let m = sora.magic_mut();
    m.shotlock_unlocked = true;
    m.airstep_unlocked = true;
    Box::new(sora)
}

/// Every ability a Sora owns at level 99 with every story bonus, as the reference's sheet lists them.
const OWNED: &[&str] = &[
    "AIR_RECOVERY",
    "BLOW_COUNTER",
    "REFLECT_GUARD",
    "GUARD_COUNTER",
    "REVENGEIMPACT",
    "REVENGEDIVE",
    "REVENGE_EX",
    "RISKDODGE",
    "QUICK_SHAFT",
    "FLASH_STEP",
    "RADIAL_CUT",
    "AERIAL_SWEEP",
    "AERIAL_DIVE",
    "AIR_DOWN",
    "TRIPPLE_SLASH",
    "CHARGE_THRUST",
    "MAGICFLUSH",
    "FINAL_HEAVEN",
    "SLASH_UPPER",
    "AIR_ROLL_BEAT",
    "FOCUS_ASPIR x3",
    "SONIC_SLASH",
    "TURN_CUTTER",
    "SUMMERSALT",
    "SONIC_DOWN",
    "HIGHJUMP",
    "DOUBLEFLIGHT",
    "DODGE",
    "AIRSLIDE x3",
    "AIRDODGE",
    "GLIDE",
    "WALL_KICK",
    "POLE_SPIN",
    "POLE_SWING",
    "SUPERJUMP",
    "SUPERSLIDE x3",
    "EXPZERO",
    "LIBRA",
    "MP_SAFETY",
    "AUTO_LOCK_MAGIC",
    "COMBO_PLUS x2",
    "AIRCOMBO_PLUS x2",
    "COMBO_UP",
    "AIRCOMBO_UP",
    "COMBO_MASTER",
    "COUNTER_UP",
    "LEAF_VEIL",
    "LAST_LEAVE",
    "COMBO_LEAVE",
    "MAGIC_COMBO_SAVE",
    "MAGIC_COMBO_UP",
    "AUTO_FINISH",
    "FIRE_UP",
    "BLIZZARD_UP",
    "THUNDER_UP",
    "WATER_UP",
    "AERO_UP",
    "GUARD_REGENE",
    "MP_HASTE",
    "MAGIC_TIME x3",
    "FORM_TIME",
    "CRITICAL_HALF x2",
];

/// The menu changed the abilities: the status is built on them again, the gauges staying.
pub fn reequip(sora: &mut ks::Sora, set: &super::ability::Set) {
    let list = match loadout(set) {
        Ok(l) => l,
        Err(why) => {
            eprintln!("KH3 Sora's loadout: {why}");
            return;
        }
    };
    let (hp, mp, focus) = {
        let s = sora.status();
        (s.hp, s.mp, s.focus)
    };
    let status = sora.status_mut();
    *status = ks::status::Status::from_loadout(&list);
    status.hp = hp.min(status.hp_max);
    status.mp = mp;
    status.focus = focus.min(status.focus_max);
}

fn loadout(set: &super::ability::Set) -> Result<ks::status::Loadout, String> {
    use ks::status::LoadoutValue as V;
    let list = |v: &[&str]| V::List(v.iter().map(|s| (*s).to_owned()).collect());
    let mut l = ks::status::Loadout::new("duel");
    let n = |x: f64| V::Number(x);
    l.set("level", &n(99.))?;
    l.set("game_level", &V::Text("STANDARD".into()))?;
    l.set(
        "choice.default",
        &V::Text("balance + ability path A".into()),
    )?;
    // Every story and Re Mind bonus with the balance opening (`TresVBonusData`, the status computes HP and MP).
    l.bonuses = Some(ks::status::BonusCounts::STORY_AND_REMIND);
    l.set("focus", &n(100.))?;
    l.set("weapon", &V::Text("WEP_KEYBLADE_SO_00".into()))?;
    l.set("armour", &V::List(Vec::new()))?;
    l.set("accessories", &V::List(Vec::new()))?;
    l.set("item_slots", &n(8.))?;
    l.set(
        "items",
        &list(&[
            "ITEM_POTION",
            "ITEM_POTION",
            "ITEM_HIGHPOTION",
            "ITEM_HIGHPOTION",
            "ITEM_ETHER",
            "ITEM_ETHER",
        ]),
    )?;
    l.set(
        "magic.known",
        &V::Map(
            ["fire", "blizzard", "thunder", "water", "aero", "cure"]
                .iter()
                .map(|s| ((*s).to_owned(), 3.))
                .collect(),
        ),
    )?;
    l.set(
        "shotlocks",
        &list(&["TEXT_SHOOTFLOW_KB00_00", "TEXT_SHOOTFLOW_KB00_01"]),
    )?;
    l.set(
        "shortcuts.a",
        &list(&["FIRAGA", "BLIZZAGA", "THUNDAGA", "CURAGA"]),
    )?;
    l.set(
        "shortcuts.b",
        &list(&["WATAGA", "AEROGA", "POTION", "ETHER"]),
    )?;
    l.set("shortcuts.c", &list(&["", "", "", ""]))?;
    l.set("abilities.owned", &list(OWNED))?;
    l.set("abilities.equipped", &V::List(set.kinds()))?;
    Ok(l)
}

fn has(b: PadButton, on: bool, mut held: PadButtons) -> PadButtons {
    held.set(b, on);
    held
}

impl Kh3 {
    /// Whether the core runs this Sora.
    pub fn core_on(&self) -> bool {
        self.core.is_some()
    }

    /// A tick on the core. The room has swapped this Sora in; `foe` says who he fights.
    pub(super) fn core_tick(
        &mut self,
        player: &mut Player,
        input: &Input,
        pad: &Pad,
        scene: &Scene,
        geometry: &Geometry,
    ) -> CoreOut {
        // The room moved him (a round's start): the core's Sora goes there.
        let moved = (0..3).any(|k| (player.position[k] - self.core_at[k]).abs() > 5.);
        if moved && let Some(sora) = self.core.as_mut() {
            sora.teleport(ks::Pose {
                position: to_core(player.position),
                yaw: yaw_to_core(player.yaw),
            });
        }
        let camera_yaw = 0.;
        let (forward, right) = dir_to_core(input.direction);
        let mut held = PadButtons::NONE;
        held = has(PadButton::JUMP, !input.jump_up, held);
        held = has(PadButton::GUARD, input.guard_held || input.guard, held);
        held = has(PadButton::ATTACK, input.attack, held);
        held = has(PadButton::RightShoulder, pad.shotlock, held);
        held = has(PadButton::SITUATION, pad.situation, held);
        held = has(PadButton::LeftTrigger, pad.situation_next, held);
        let before = self.core_held;
        self.core_held = held;
        let mut state = PadState::from_held(held, before);
        // The room latches its presses to a tick; a press holds no button in the core's sense otherwise.
        if input.jump {
            state.pressed.set(PadButton::JUMP, true);
        }
        if input.guard {
            state.pressed.set(PadButton::GUARD, true);
        }
        if input.attack {
            state.pressed.set(PadButton::ATTACK, true);
        }
        let core_input = ks::Input {
            move_stick: ks::Vec2::new(right, forward),
            jump_pressed: state.pressed.has(PadButton::JUMP),
            jump_held: state.held.has(PadButton::JUMP),
            camera_yaw,
            pad: state,
            view: scene.view,
            raw_axes: true,
        };
        let Some(sora) = self.core.as_mut() else {
            return CoreOut::default();
        };
        if let Some(cmd) = self.want_cast.take() {
            sora.command_request(ks::command::CommandRequest::Execute(cmd as u8));
        }
        let mut hits = Vec::new();
        let mut targets = CoreTargets {
            foe: scene.foe_body,
            hits: &mut hits,
        };
        let (mut audio, mut effects, mut ui) =
            (ks::host::NullAudio, ks::host::NullEffects, ks::host::NullUi);
        let mut host = ks::Host {
            world: geometry,
            targets: &mut targets,
            audio: &mut audio,
            effects: &mut effects,
            ui: &mut ui,
        };
        let out = sora.step_hosted(1. / 60., &core_input, &mut host);
        let mut result = CoreOut {
            hits,
            ..CoreOut::default()
        };
        for e in &out.events {
            match e.kind {
                EventKind::EffectSpawn { handle, spawn } => {
                    let (on_sora, bone, offset) = match spawn.at {
                        Attach::Sora { bone, offset } => (true, bone, offset.to_array()),
                        _ => (false, None, [0.; 3]),
                    };
                    let at = match spawn.at {
                        Attach::World(p) => from_core(p),
                        // The other body, the one thing a target is here.
                        Attach::Target { offset, .. } => {
                            let foe = scene.foe_body.map_or(out.position, |f| f.position);
                            from_core(foe + offset + Vec3::Z * BODY_CENTRE)
                        }
                        _ => from_core(out.position + Vec3::Z * BODY_CENTRE),
                    };
                    result.spawned.push(Spawned {
                        name: spawn.name,
                        id: spawn.id.0,
                        at,
                        yaw: yaw_from_core(spawn.rotation.y),
                        sound: false,
                        persistent: spawn.persistent,
                        on_sora,
                        bone,
                        offset,
                        handle: handle.0,
                    });
                }
                EventKind::EffectStop { handle, immediate } => {
                    result.stopped.push((handle.0, immediate));
                }
                EventKind::SoundPlay { play, .. } => {
                    let at = match play.at {
                        Attach::World(p) => from_core(p),
                        _ => from_core(out.position + Vec3::Z * BODY_CENTRE),
                    };
                    result.spawned.push(Spawned {
                        name: play.name,
                        id: play.id.0,
                        at,
                        yaw: 0.,
                        sound: true,
                        persistent: false,
                        on_sora: false,
                        bone: None,
                        offset: [0.; 3],
                        handle: 0,
                    });
                }
                EventKind::PlayerDied { .. } => result.died = true,
                EventKind::StateChange { from, to } => result.states.push((from.name(), to.name())),
                _ => {}
            }
        }
        self.core_copy(player, &out);
        result.on_floor = out.on_floor;
        result.velocity = out.velocity.to_array();
        result.position = out.position.to_array();
        result.row = out.anim.name.name();
        result
    }

    /// The results onto `Player` and `Kh3`, for the room and the presenters.
    fn core_copy(&mut self, player: &mut Player, out: &ks::Output) {
        let Some(sora) = self.core.as_ref() else {
            return;
        };
        player.position = from_core(out.position);
        player.yaw = yaw_from_core(out.yaw);
        player.vertical_speed = out.velocity.z * SCALE / 60.;
        let v = from_core(out.velocity);
        self.velocity = [v[0] / 60., v[1] / 60., v[2] / 60.];
        player.action = match out.state {
            StateId::Die | StateId::VanishDie => player.action,
            _ if !out.on_floor => Action::Fall,
            StateId::Run if out.velocity.truncate().length() > 200. => Action::Run,
            StateId::Run => Action::Walk,
            _ => Action::Idle,
        };
        player.armed = sora.weapon_out();
        let status = sora.status();
        player.mp.cur = status.mp;
        player.mp.max = status.max_mp();
        self.focus = status.focus as f32;
        self.core_hp = (status.hp, status.max_hp());
        self.chain.n = sora.attack().map_or(0, |a| a.combo as u32);
        let aim = sora.aim();
        self.aim = (aim.mode != 0).then(|| super::Aim {
            locks: aim.total.clamp(0, 255) as u8,
            syphon: aim.kind == 36,
            ..super::Aim::default()
        });
        // The situation command on offer: the HUD's line and the joiner's (a finish command before a list entry).
        let hud = sora.ui_snapshot().situation;
        self.grand_offer = hud
            .slot_b
            .or(hud.slot_a)
            .or_else(|| hud.list.first().copied())
            .map(|e| (e.kind.value() as u16, 1));
        self.mode = mode_of(sora, out);
        self.attack = None;
        self.cast = None;
        self.visual = visual_of(&out.anim, self.visual);
        self.remote_visual = None;
        self.core_at = player.position;
    }

    /// The spell actors and shots for the presenters and the wire, by the blueprint each is of.
    pub(super) fn core_present(&mut self, book: &super::attack::Book) {
        self.actors.clear();
        let Some(sora) = self.core.as_ref() else {
            return;
        };
        for p in &sora.magic().projectiles {
            if p.dead {
                continue;
            }
            let v = p.view();
            let Some(stage) = book.stages.get(v.class.trim_end_matches("_C")) else {
                continue;
            };
            let pos = from_core(v.position);
            let f = from_core(v.forward);
            let cmd = if std::sync::Arc::ptr_eq(stage, book.shots[1].as_ref().unwrap_or(stage)) {
                super::SHOT_RAGNAROK
            } else if std::sync::Arc::ptr_eq(stage, book.shots[0].as_ref().unwrap_or(stage))
                && book.shots[0].is_some()
            {
                super::SHOT_KOH
            } else {
                u16::from(p.cmd)
            };
            self.actors.push(spell::Actor {
                serial: v.id,
                cmd,
                speed: 0.,
                stage: std::sync::Arc::clone(stage),
                pos,
                dir: [f[0] / SCALE, f[1] / SCALE, f[2] / SCALE],
                age: v.age,
                turn: 0.,
                homing_on: false,
                finish: p.finish,
                grand: p.situation,
                hit: true,
                previous: pos,
                dead: false,
            });
        }
    }

    /// A hit on this Sora, from the other (`HitRecord` to KH3's own victim side).
    ///
    /// [duel choice] KH3 offers a Risk Dodge from an enemy clip's `TresAnimNotify_DamageCaution`; Sora's clips carry
    /// none, so here a guarded heavy hit (a blow or harder) offers it. Its type by where the attacker is: in the
    /// air the warp, beside him the slide turn, far off the over-slide, else the under-slide.
    pub fn core_take_hit(
        &mut self,
        hit: &HitRecord,
        attacker_yaw: f32,
        attacker_at: Vec3,
        airborne: bool,
    ) {
        let Some(sora) = self.core.as_mut() else {
            return;
        };
        let i = &hit.info;
        let heavy = !matches!(
            hit.damage_kind,
            ks::enums::ETresDamageKind::Small | ks::enums::ETresDamageKind::None
        );
        if heavy {
            let me = sora.output();
            let to = attacker_at - me.position;
            let distance = to.truncate().length();
            let a = me.yaw.to_radians();
            let ahead = to
                .truncate()
                .normalize_or_zero()
                .dot(ks::Vec2::new(a.cos(), a.sin()));
            use ks::states::risk_dodge::kind;
            let k = if airborne {
                kind::WARP
            } else if ahead < 0.5 {
                kind::SLIDETURN
            } else if distance > 300. {
                kind::OVERSLIDE
            } else {
                kind::UNDERSLIDE
            };
            sora.offer_risk_dodge(FOE, k, 1.);
        }
        sora.take_hit(HitOnPlayer {
            amount: hit.damage,
            kind: hit.damage_kind,
            attribute: hit.attribute,
            direction: hit.direction,
            causer: FOE,
            causer_position: attacker_at,
            causer_yaw: attacker_yaw,
            causer_radius: PUSH_RADIUS,
            power_scale: i.power_scale,
            brake: i.brake,
            parabola_angle: i.parabola_angle,
            move_length: i.move_length,
            effect_time: i.effect_time,
            src_power: i.power,
            reaction_type: match i.victim_result {
                ReactionResult::NoReaction => 1,
                ReactionResult::None => 0,
                _ => 2,
            },
            ignore_guard: i.flags50 & ks::host::HitInfo::FLAG50_IGNORE_GUARD != 0,
            weak_guard: i.flags51 & ks::host::HitInfo::FLAG51_WEAK_GUARD != 0,
            rapid_fire: i.flags50 & ks::host::HitInfo::FLAG50_RAPID_FIRE != 0,
            reflect_level: if heavy {
                i.reflect_level.max(3)
            } else {
                i.reflect_level
            },
            reflect_reaction: i.reflect_reaction,
            bad_status: i.bad_status,
            bad_time: i.bad_time,
            bad_param: i.bad_param,
        });
    }

    /// This Sora as the other's target.
    pub fn core_foe_body(&self, player: &Player) -> Option<FoeBody> {
        let sora = self.core.as_ref()?;
        let out = sora.output();
        let status = sora.status();
        let d = status.as_defender();
        Some(FoeBody {
            position: out.position,
            yaw: out.yaw,
            velocity: out.velocity,
            hp: status.hp as f32,
            hp_max: status.max_hp() as f32,
            airborne: !out.on_floor,
            alive: status.hp > 0 && !player.dead(),
            body: sora.target_body(),
            defense: TargetDefense {
                defense: d.defense,
                max_hp: d.max_hp,
                hp_percent: d.hp_percent,
                enemy_kind: false,
                damage_min: d.damage_min,
                damage_max: d.damage_max,
                resist: d.resist,
                bio_type: d.bio_type,
            },
            reaction_state: sora.reaction_state(),
            push_level: 4,
            rush_suppression: false,
        })
    }

    /// The spell actors and shots on the wire and for the presenters.
    pub fn core_actors(&self) -> Vec<ks::magic::ProjectileView> {
        self.core
            .as_ref()
            .map_or_else(Vec::new, |s| s.magic().projectile_views().collect())
    }
}

/// The running state as the layer's mode (what the cameras, the HUD and the log read).
fn mode_of(sora: &ks::Sora, out: &ks::Output) -> Mode {
    let air = |jump| Mode::Air {
        jump,
        hang: 0,
        released: false,
        tick: 0,
    };
    match out.state {
        StateId::Idle | StateId::Run => Mode::Ground,
        StateId::Jump => match sora.jump_kind() {
            Some(1) => air(Jump::High),
            Some(2) => air(Jump::Super),
            Some(3) => air(Jump::Double),
            _ if out.on_floor => Mode::Land { tick: 0 },
            _ => air(Jump::Normal),
        },
        StateId::Glide => Mode::Glide { tick: 0 },
        StateId::AirSlide => Mode::Slide {
            tick: 0,
            dir: [0.; 2],
        },
        StateId::Dodgeroll => Mode::Roll { tick: 0 },
        StateId::SuperSlide => Mode::Superslide {
            tick: 0,
            pole: None,
        },
        StateId::Wallrun => Mode::WallRun {
            normal: [0.; 2],
            up: true,
            side: 0.,
            tick: 0,
            idle: 0,
        },
        StateId::WallKick => Mode::WallKick {
            tick: 0,
            normal: [0.; 2],
            guard: false,
        },
        StateId::Athleticflow => Mode::Airstep { tick: 0, speed: 0. },
        StateId::Shootflow => Mode::Volley {
            tick: 0,
            locks: 0,
            ragnarok: false,
            base: 0.,
            spawned: 0,
        },
        StateId::Attack => Mode::Attack,
        StateId::Firemagic => Mode::Cast,
        StateId::Damage => Mode::Hurt {
            knock: Knock::Small,
            tick: 0,
            back: false,
            air: !out.on_floor,
            high: false,
            apex: 0,
            fell: 0,
        },
        _ => Mode::Kh2,
    }
}

/// The animation for the presenter.
fn visual_of(a: &ks::AnimRequest, before: super::visual::Visual) -> super::visual::Visual {
    super::visual::Visual {
        row: a.name.name(),
        serial: if before.row == a.name.name() && before.serial & 0xffff == a.play_id & 0xffff {
            before.serial
        } else {
            a.play_id
        },
        time: a.time,
        at: [a.axis, 0.],
        blend: a.blend_in,
        looped: a.looping,
    }
}

/// [Test] The training dummy as a target: a level 99 Sora's defense, that never dies.
pub fn dummy_body(at: RoomVec3, yaw: f32) -> FoeBody {
    let defense = ks::status::tables::level_row(99).defense;
    FoeBody {
        position: to_core(at),
        yaw: yaw_to_core(yaw),
        velocity: Vec3::ZERO,
        hp: 99_999.,
        hp_max: 99_999.,
        airborne: false,
        alive: true,
        body: TargetBody {
            reaction: BodyReaction::Default,
            ..TargetBody::DEFAULT
        },
        defense: TargetDefense {
            defense,
            max_hp: 99_999,
            hp_percent: 100.,
            enemy_kind: false,
            damage_min: 0,
            damage_max: 255,
            resist: [0; 8],
            bio_type: 0,
        },
        reaction_state: 0,
        push_level: 4,
        rush_suppression: false,
    }
}

pub use ks::sheets;
pub use spell::Actor as SpellActor;
pub type SharedGeometry = Arc<Geometry>;
