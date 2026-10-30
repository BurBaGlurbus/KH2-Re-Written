//! KH3's attack state, driven by each clip's own notifies, read from the
//! player's paks into a `Book`. The layer asks for an attack by KH3's kind;
//! the clip gives the hit windows, the homing window, the input and change
//! windows and the root motion, the kind table the rest. The rules are from
//! StrawHatPro's reading of KH3 (`crates/kh-core/src/states/attack.rs`,
//! `tools/sora_combat_sheet.js`; MIT): PlayAttack 0x14133A160, Facing
//! 0x14134D0E0, Traction 0x141349CC0, the Tick's cancel and chain rules.

use super::tuning::{self as t, length_of};
use super::visual;
use super::{Jump, Kh3, Mode, Outcome, Scene, wrap, yaw_of};
use crate::{Action, Input, Params, Player, World};
use std::collections::HashMap;

/// A hit window of a clip.
#[derive(Clone, Debug, PartialEq)]
pub struct HitWindow {
    pub from: f32,
    pub to: f32,
    /// `CollisionGrpName`: the shape group.
    pub group: String,
    /// `m_AttackDataIDName`: the attack data row.
    pub row: String,
    /// `AttackInterval`: the hit list clears after this long (multi-hit).
    pub interval: f32,
    /// `m_WeaponHitSEAliasID`: the HITn of the Keyblade's sound set.
    pub hit_se: u8,
}

/// What the simulation needs of a clip, in seconds and KH3's centimetres.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ClipInfo {
    pub length: f32,
    pub root_motion: bool,
    pub windows: Vec<HitWindow>,
    /// `TresAnimNotifyState_Traction`.
    pub traction: Vec<(f32, f32)>,
    /// `TresAnimNotify_EnableInput`, `TresAnimNotify_EnableChange`.
    pub enable_input: Option<f32>,
    pub enable_change: Option<f32>,
    pub face_at: Vec<f32>,
    /// `TresAnimNotify_Shot`.
    pub shots: Vec<f32>,
    /// The root's place (forward, right, up, yaw) at 30 Hz.
    pub root: Vec<[f32; 4]>,
    /// The blade's hit points ATK_00, ATK_01 relative to the feet
    /// (forward, right, up), at 30 Hz.
    pub atk: Vec<[[f32; 3]; 2]>,
}

impl ClipInfo {
    fn sampled<T: Copy>(track: &[T], time: f32, lerp: impl Fn(T, T, f32) -> T) -> Option<T> {
        let last = track.len().checked_sub(1)?;
        let at = (time * 30.).clamp(0., last as f32);
        let i = at as usize;
        let t = at - i as f32;
        Some(lerp(track[i], track[(i + 1).min(last)], t))
    }

    /// The root's movement between two times (forward, right, up, yaw).
    pub fn root_delta(&self, from: f32, to: f32) -> [f32; 4] {
        let lerp = |a: [f32; 4], b: [f32; 4], t: f32| {
            [
                a[0] + (b[0] - a[0]) * t,
                a[1] + (b[1] - a[1]) * t,
                a[2] + (b[2] - a[2]) * t,
                a[3] + (b[3] - a[3]) * t,
            ]
        };
        let (Some(a), Some(b)) = (
            Self::sampled(&self.root, from, lerp),
            Self::sampled(&self.root, to, lerp),
        ) else {
            return [0.; 4];
        };
        [b[0] - a[0], b[1] - a[1], b[2] - a[2], b[3] - a[3]]
    }

    /// The blade's two hit points at a time.
    pub fn atk_at(&self, time: f32) -> Option<[[f32; 3]; 2]> {
        Self::sampled(&self.atk, time, |a, b, t| {
            let m = |x: [f32; 3], y: [f32; 3]| {
                [
                    x[0] + (y[0] - x[0]) * t,
                    x[1] + (y[1] - x[1]) * t,
                    x[2] + (y[2] - x[2]) * t,
                ]
            };
            [m(a[0], b[0]), m(a[1], b[1])]
        })
    }

    fn traction_open(&self, time: f32) -> bool {
        self.traction.iter().any(|&(a, b)| time >= a && time < b)
    }
}

/// `TresAttackDataTable`'s fields the duel uses.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct AttackRow {
    pub power: f32,
    /// `ETresDamageKind` without its prefix (`SMALL`, `BLOW`, ...).
    pub damage_kind: String,
    pub attribute: String,
    pub power_scale: f32,
    pub reflect_level: i32,
    pub form_point: f32,
    pub magic: bool,
    pub ignore_guard: bool,
    pub counter: bool,
    pub damage_min: f32,
}

/// A hit shape of `TresAtkColl0`.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Shape {
    /// `CAPSULE2` follows the two sockets; `SPHERE` the first; `CAPSULE1`
    /// is upright.
    pub capsule: bool,
    pub upright: bool,
    pub radius: f32,
    pub half_length: f32,
    pub grow: f32,
    pub grow_max: f32,
    /// Frozen where it first opened.
    pub frozen: bool,
    /// Off the owner's chest rather than the blade.
    pub from_owner: bool,
    pub offset: [f32; 3],
}

/// Everything the simulation reads from KH3's data.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Book {
    pub clips: HashMap<String, ClipInfo>,
    pub rows: HashMap<String, AttackRow>,
    pub shapes: HashMap<String, Shape>,
    /// Level 99's strength, magic, defense and hit points (the Kingdom Key's
    /// plus theirs).
    pub stats: [f32; 4],
    /// The spells by command (`TresCmdData`, `p_ex001_MagicSet`, blueprints).
    pub spells: HashMap<u16, super::spell::Spell>,
    /// The Kingdom Key's shots: King of Hearts and Ragnarok.
    pub shots: [Option<std::sync::Arc<super::spell::Stage>>; 2],
    /// The Kingdom Key's pawn hit effect.
    pub hit_effect: Option<String>,
    /// The anim set's rows, by name (their index is the wire's id).
    pub row_ids: HashMap<String, u16>,
    /// Every effect and sound the duel may start, sorted (an index is the
    /// wire's id; the same on both PCs of a match, whose installs are checked).
    pub names: Vec<String>,
}

impl Book {
    pub fn row(&self, name: &str) -> Option<&AttackRow> {
        self.rows.get(&name.to_ascii_lowercase())
    }
}

/// The kind table, `tools/sora_combat_sheet.js` in the reference.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Kind {
    pub root_len: Option<f32>,
    pub air_mode: bool,
    pub keep_face: bool,
    pub stop_dist: f32,
    pub z_gain: f32,
}

pub const fn kind_of(id: u8) -> Kind {
    const fn k(root_len: Option<f32>, air_mode: bool, keep_face: bool) -> Kind {
        Kind {
            root_len,
            air_mode,
            keep_face,
            stop_dist: 160.,
            z_gain: 3.,
        }
    }
    const fn with(mut base: Kind, stop_dist: f32, z_gain: f32) -> Kind {
        base.stop_dist = stop_dist;
        base.z_gain = z_gain;
        base
    }
    match id {
        0x03 => k(Some(207.), false, false),
        0x04 => k(Some(523.), false, false),
        0x08 => k(None, true, false),
        0x09 => k(Some(360.), false, true),
        0x0a => k(Some(680.), false, true),
        0x0b => k(Some(260.), false, true),
        0x0c => k(Some(450.), false, true),
        0x0d | 0x0e => with(k(Some(200.), true, true), 250., 3.),
        0x0f => k(Some(414.), true, true),
        0x10 => k(Some(210.), true, true),
        0x11 => k(Some(618.), true, true),
        0x12 => k(Some(210.), true, true),
        0x13 => k(Some(550.), true, true),
        0x14 => with(k(Some(431.), true, true), 160., 4.5),
        0x15 => with(k(Some(722.), true, true), 160., 4.5),
        0x16 => k(Some(323.), true, true),
        0x17 => k(None, true, true),
        0x18 => k(Some(270.), true, true),
        0x19 => k(Some(680.), true, true),
        0x1a => k(Some(260.), true, true),
        0x1b => k(Some(450.), true, true),
        0x1c | 0x1d => k(None, true, true),
        0x1f => k(None, true, false),
        0x20 => k(Some(560.), true, true),
        0x21 => k(Some(102.), false, false),
        0x22 => k(Some(350.), true, false),
        0x24 => with(k(Some(600.), true, false), 180., 3.),
        0x27 => k(Some(1000.), true, false),
        0x28 => k(Some(600.), true, false),
        0x29 | 0x2b => k(None, true, false),
        0x2a => k(Some(330.), true, false),
        0x2f | 0x38 | 0x39 | 0x35 => k(None, true, false),
        0x32 | 0x33 => k(Some(595.), false, false),
        0x34 => with(k(None, false, false), 60., 4.5),
        0x36 => with(k(None, false, false), 160., 4.5),
        0x37 => with(k(None, true, false), 250., 3.),
        _ => k(None, false, false),
    }
}

/// A window's hit list and sweep.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct WindowState {
    /// Last tick's world points, for the sweep.
    pub previous: Option<[[f32; 3]; 2]>,
    /// The frozen shape's place.
    pub frozen: Option<[[f32; 3]; 2]>,
    /// This window hit the other Sora (cleared by its interval).
    pub hit: bool,
    pub since_hit: f32,
    pub open_for: f32,
}

/// A running KH3 attack.
#[derive(Clone, Debug, PartialEq)]
pub struct Attack {
    pub kind: u8,
    pub row: &'static str,
    pub time: f32,
    pub finisher: bool,
    pub finish_end: bool,
    pub queued: bool,
    /// The next attack asked for is the launch.
    pub launch: bool,
    pub windows: Vec<WindowState>,
    k: Kind,
    chan_xy: bool,
    chan_root: bool,
    root_len: f32,
    root_scale: f32,
    speed_override: f32,
    speed_xy: f32,
    vel_z: f32,
    dir_z: f32,
    fixed_dir_z: bool,
    no_vz: bool,
    rearm_z: bool,
    phase_z: u8,
    phase_root: u8,
    hover: bool,
    face_yaw: f32,
    facing_on: bool,
    face_timer: f32,
    face_at: bool,
    first_air: bool,
    swing_started: bool,
    cancel: bool,
    cancel_timer: f32,
}

const DT: f32 = 1. / 60.;
const FACE_RATE: f32 = 12.;
const FACE_TIMER: f32 = 0.2;
const TRACTION_SPEED: f32 = 800.;
const RATE_ON: f32 = 0.2;
const RATE_OFF_GROUND: f32 = 0.5;
const RATE_OFF_AIR: f32 = 0.1;
const FIRST_AIR: f32 = 1.5;
const DIST_CLAMP: f32 = 800.;
const DZ_CLAMP: f32 = 500.;
const Z_DECAY: f32 = 10.;
const CANCEL_AFTER: f32 = 0.133_333;

/// Metrics in KH3's centimetres: (flat distance to the target, height
/// difference from the chest), or none without a target.
fn metrics(player: &Player, scene: &Scene) -> Option<(f32, f32, [f32; 2])> {
    let foe = scene.foe?;
    let d = [
        foe.position[0] - player.position[0],
        foe.position[2] - player.position[2],
    ];
    let flat = d[0].hypot(d[1]) / length_of(1.);
    let dz = (foe.centre[1] - (player.position[1] + t::CHEST)) / length_of(1.);
    Some((flat, dz, d))
}

impl Kh3 {
    /// The attack `id` (`combo::id`), if the clip is in the book.
    #[allow(clippy::too_many_arguments)]
    pub(super) fn start_attack(
        &mut self,
        player: &mut Player,
        id: u8,
        finisher: bool,
        last: bool,
        scene: &Scene,
        step: &mut crate::Step,
        clips: &crate::Clips,
    ) -> bool {
        let Some(book) = scene.book else {
            return false;
        };
        let Some(row) = visual::attack_row(id) else {
            return false;
        };
        let Some(info) = book.clips.get(row) else {
            return false;
        };
        let k = kind_of(id);
        let m = metrics(player, scene);
        let dist = m.map_or(0., |m| (m.0 - 50.).max(0.));
        let (dz, near) = (m.map_or(0., |m| m.1), dist < 300.);
        let mut chan_root = k.root_len.is_some();
        let mut chan_xy = !chan_root;
        let mut root_len = k.root_len.unwrap_or(0.);
        if matches!(id, 3 | 0x12) {
            chan_root = near;
            chan_xy = !near;
            if !near {
                root_len = 0.;
            }
        }
        if matches!(id, 0x32 | 0x33) {
            chan_xy = true;
            chan_root = true;
        }
        let mut root_scale = 1.;
        let flow = |limit: f32, top: f32| {
            if dist != 0. {
                (dist / limit).clamp(0.1, top)
            } else {
                1.
            }
        };
        match id {
            0x36 => root_scale = flow(850., 0.705_882_4),
            0x38 => root_scale = flow(1000., 2.5),
            0x2f => root_scale = flow(815., 2.5),
            _ => {}
        }
        let air = self.airborne() || player.in_air();
        let (mut dir_z, mut fixed_dir_z) = if k.air_mode && dz.abs() > 30. {
            ((dz * 5.).clamp(-1200., 1200.), false)
        } else {
            (0., false)
        };
        if id == 8 || id == 0x1f {
            dir_z = 700.;
            fixed_dir_z = true;
        }
        let no_vz = matches!(id, 0x1c | 0x37);
        if id == 0x37 {
            dir_z = -1000.;
        }
        let windows = vec![WindowState::default(); info.windows.len()];
        let yaw = player.yaw;
        self.attack = Some(Attack {
            kind: id,
            row,
            time: 0.,
            finisher,
            finish_end: last,
            queued: false,
            launch: false,
            windows,
            k,
            chan_xy,
            chan_root,
            root_len,
            root_scale,
            speed_override: if id == 0x17 { TRACTION_SPEED } else { 0. },
            speed_xy: 0.,
            vel_z: 0.,
            dir_z,
            fixed_dir_z,
            no_vz,
            rearm_z: matches!(id, 0x14 | 0x15),
            phase_z: 0,
            phase_root: 0,
            hover: k.air_mode,
            face_yaw: yaw,
            facing_on: false,
            face_timer: 0.,
            face_at: !matches!(id, 0x34 | 0x39),
            first_air: self.chain.n <= 1 && air,
            swing_started: false,
            cancel: false,
            cancel_timer: 0.,
        });
        self.mode = Mode::Attack;
        self.aim = None;
        self.queued = false;
        self.velocity = [0.; 3];
        self.lean_target = [0.; 2];
        // The KH2 controller's side shows nothing: no KH2 hit window runs.
        let shown = if air { Action::Fall } else { Action::Idle };
        self.show(player, shown, clips, step);
        player.armed = true;
        player.invulnerable = 0.;
        self.visual.restart(row, -1., false);
        true
    }

    /// A flow attack out of a traversal state (TryAttack cases 7, 0x14 - 0x17,
    /// 0x141437330): KH3's, or the KH2 stand-in record without the book.
    #[allow(clippy::too_many_arguments)]
    pub(super) fn flow_attack(
        &mut self,
        player: &mut Player,
        forced: u8,
        record: u8,
        ability: super::ability::Ability,
        input: &Input,
        scene: &Scene,
        clips: &crate::Clips,
        step: &mut crate::Step,
    ) -> bool {
        if scene.book.is_none() {
            return self.has(ability) && self.start_flow(player, record, input, clips);
        }
        let kind = self.flow_refine(player, scene, forced);
        self.chain.start(&self.abilities);
        self.start_attack(player, kind, false, false, scene, step, clips)
    }

    /// Slot 124 (0x1413325F0): the flowmotion attack by distance, and an
    /// ordinary air attack for one whose ability is off. A duel has one enemy,
    /// so its "two or more around" Buzz Saw never applies.
    fn flow_refine(&self, player: &Player, scene: &Scene, forced: u8) -> u8 {
        use super::ability::Ability as A;
        let (dist, dz) = metrics(player, scene).map_or((0., 0.), |m| ((m.0 - 50.).max(0.), m.1));
        let mut kind = forced;
        if matches!(kind, 0x36 | 0x37) {
            if dist > 800. && self.has(A::SonicSlash) {
                kind = 0x36;
            } else if dist < dz.abs() && self.has(A::SonicCleave) {
                kind = 0x37;
            }
        }
        let ability = match kind {
            0x36 => A::SonicSlash,
            0x37 => A::SonicCleave,
            _ => A::BuzzSaw,
        };
        if self.has(ability) { kind } else { 0x10 }
    }

    /// The attack out of a wall run of `class` (0 up, 1 and 2 along):
    /// `Decide`'s rule 4, kinds 0x31 - 0x34.
    pub(super) fn wall_run_kind(&self, player: &Player, scene: &Scene, class: u8) -> u8 {
        let Some((flat, dz, d)) = metrics(player, scene) else {
            return 0x31;
        };
        let dist = (flat - 50.).max(0.);
        if dz > 400. && dist <= 400. {
            return 0x31;
        }
        if dz < -250. && dist <= 400. {
            return 0x34;
        }
        let l = d[0].hypot(d[1]).max(1e-3);
        let to = [d[0] / l, d[1] / l];
        let dot = player.yaw.sin() * to[0] + player.yaw.cos() * to[1];
        let right = -player.yaw.cos() * to[0] + player.yaw.sin() * to[1];
        let along = |one: bool| if one { 0x32 } else { 0x33 };
        match class {
            0 if dot.abs() >= 0.643 => 0x31,
            0 => along(right < 0.),
            1 => along(dot >= 0.),
            _ => along(dot < 0.),
        }
    }

    /// The attack's tick. Returns with the mode changed when it ended.
    #[allow(clippy::too_many_arguments)]
    pub(super) fn attack_tick(
        &mut self,
        player: &mut Player,
        input: &Input,
        p: &Params,
        clips: &crate::Clips,
        world: &impl World,
        scene: &Scene,
        out: &mut Outcome,
    ) {
        let Some(mut a) = self.attack.take() else {
            self.mode = Mode::Ground;
            return;
        };
        let Some(book) = scene.book else {
            self.mode = Mode::Ground;
            return;
        };
        let Some(info) = book.clips.get(a.row) else {
            self.mode = Mode::Ground;
            return;
        };
        let before = a.time;
        a.time += DT;
        let time = a.time;
        if before < time && info.face_at.iter().any(|&f| f > before && f <= time) {
            a.face_at = true;
        }
        let m = metrics(player, scene);
        // Facing (0x14134D0E0).
        let mut alpha = DT * FACE_RATE;
        if std::mem::take(&mut a.face_at) {
            a.facing_on = true;
            a.face_timer = FACE_TIMER;
            alpha = 1.;
        }
        if a.facing_on {
            if let Some((_, _, d)) = m
                && d[0].hypot(d[1]) > 0.01
            {
                a.face_yaw = yaw_of(d);
            }
            player.yaw += wrap(a.face_yaw - player.yaw) * alpha.min(1.);
            if !a.k.keep_face {
                a.face_timer -= DT;
                if a.face_timer <= 0. {
                    a.facing_on = false;
                }
            }
        }
        // Traction (0x141349CC0).
        let open = info.traction_open(time);
        let (dxy, dzc) = m.map_or((0., 0.), |m| {
            (m.0.clamp(0., DIST_CLAMP), m.1.clamp(-DZ_CLAMP, DZ_CLAMP))
        });
        let first_air = a.first_air;
        if a.chan_xy {
            let (mut wanted, mut rate) = (0f32, RATE_ON);
            let mut inside = false;
            if open {
                wanted = a.speed_override;
                if wanted == 0. {
                    if dxy > a.k.stop_dist {
                        wanted = TRACTION_SPEED;
                    } else {
                        inside = true;
                    }
                }
                if first_air {
                    wanted *= FIRST_AIR;
                    rate = RATE_ON * FIRST_AIR;
                }
            }
            if wanted <= 0. {
                wanted = 0.;
                rate = if inside || !(a.k.air_mode && !first_air) {
                    RATE_OFF_GROUND
                } else {
                    RATE_OFF_AIR
                };
            }
            let mut d = wanted - a.speed_xy;
            let f = rate * DT * 60.;
            if f < 1. {
                d *= f;
            }
            a.speed_xy = if d.abs() < 0.0003 {
                wanted
            } else {
                a.speed_xy + d
            };
        }
        if a.chan_root {
            if a.phase_root == 0 && open {
                a.phase_root = 1;
            }
            if a.phase_root == 1 {
                if open {
                    let mut scale = match m {
                        Some(_) if a.root_len > 0. => dxy / a.root_len,
                        Some(_) => 0.,
                        None => 1.,
                    };
                    if first_air && m.is_some() {
                        scale *= FIRST_AIR;
                    }
                    a.root_scale = scale;
                } else {
                    a.root_scale = 1.;
                    a.phase_root = 2;
                }
            }
        }
        if a.k.air_mode {
            if a.phase_z == 0 && open {
                a.phase_z = 1;
            }
            if a.phase_z == 1 && open {
                a.vel_z = if a.no_vz {
                    0.
                } else if a.fixed_dir_z {
                    a.dir_z
                } else if a.dir_z != 0. && dzc != 0. && (a.dir_z > 0.) == (dzc > 0.) {
                    a.dir_z = dzc * a.k.z_gain;
                    a.dir_z
                } else {
                    a.vel_z
                };
            } else if a.phase_z == 1 {
                a.phase_z = 2;
            }
            if a.phase_z == 2 {
                if a.dir_z != 0. {
                    let next = a.dir_z - DT * a.dir_z * Z_DECAY;
                    a.dir_z = if (next > 0.) != (a.dir_z > 0.) {
                        0.
                    } else {
                        next
                    };
                    a.vel_z = a.dir_z;
                }
                if a.rearm_z && open {
                    a.phase_z = 1;
                    a.dir_z = if dzc.abs() > 30. {
                        (dzc * 5.).clamp(-1200., 1200.)
                    } else {
                        0.
                    };
                }
            }
        }
        // Root motion in the actor's axes, the normal velocity on top.
        let root = info.root_delta(before, time);
        let (fx, fz) = (player.yaw.sin(), player.yaw.cos());
        let k = length_of(1.);
        let (forward, right, up) = (
            (root[0] * a.root_scale + a.speed_xy * DT) * k,
            root[1] * a.root_scale * k,
            root[2] * k,
        );
        let enable_change = info.enable_change.is_some_and(|c| time >= c);
        // The hover ends at the change notify; then he falls.
        if enable_change && a.hover {
            a.hover = false;
        }
        let air = a.k.air_mode || self.airborne_attack(player, world, p);
        let mut vy = a.vel_z * DT * k + up;
        if air && !a.hover {
            self.velocity[1] -= t::GRAVITY;
            vy += self.velocity[1];
        }
        let disp = [fx * forward - fz * right, vy, fz * forward + fx * right];
        let moved = Self::travel(player, disp, air, p, world);
        // The windows' timers (their hits are the room's).
        for (w, state) in info.windows.iter().zip(a.windows.iter_mut()) {
            if time >= w.from && time < w.to {
                state.open_for += DT;
                if state.hit && w.interval > 0. {
                    state.since_hit += DT;
                    if state.since_hit >= w.interval {
                        state.hit = false;
                        state.since_hit = 0.;
                    }
                }
                a.swing_started = true;
            }
        }
        // The cancel bit, 0.1333 s after the first window or at the change.
        if a.swing_started && !a.cancel {
            if !a.finish_end {
                a.cancel_timer += DT;
                if a.cancel_timer > CANCEL_AFTER {
                    a.cancel = true;
                }
            }
            if enable_change {
                a.cancel = true;
            }
        }
        let enable_input = enable_change || info.enable_input.is_some_and(|i| time >= i);
        // A press asks for the next attack: not buffered, sampled here.
        let may_chain = !a.queued
            && a.swing_started
            && enable_input
            && self.chain.open()
            && (self.chain_hit || self.has(super::ability::Ability::ComboMaster));
        if input.attack && may_chain {
            a.queued = true;
        }
        if input.jump
            && !a.queued
            && a.swing_started
            && enable_input
            && !a.finisher
            && self.chain.open()
            && self.chain_hit
            && self.chain.launch_ready(air || player.in_air())
            && !self.launched
        {
            a.queued = true;
            a.launch = true;
        }
        // Final Blow: the attack button after a counter that blew him away.
        if input.attack && self.chase_offer > 0 && a.swing_started && enable_input {
            let keep = self.velocity;
            self.attack = None;
            if self.start_chase(player, scene, clips, &mut out.step) {
                return;
            }
            self.velocity = keep;
            self.mode = Mode::Attack;
        }
        let ended = time >= info.length;
        let chain_now = a.queued && (enable_change || ended);
        let landed_end = air && moved.on_floor && !a.hover;
        // Guard or a dodge, once the cancel bit is up.
        let (magnitude, dir) = super::stick(input);
        if input.guard && a.cancel {
            let keep = self.velocity;
            self.attack = None;
            self.traction = 0.;
            if air && player.in_air() {
                if magnitude >= t::STICK_ACTIVE
                    && self.slide_cooldown == 0
                    && self.has(super::ability::Ability::AirSlide)
                {
                    self.velocity = [0.; 3];
                    self.start_slide(player, dir, clips, &mut out.step);
                    return;
                }
            } else if magnitude >= t::STICK_ACTIVE && self.has(super::ability::Ability::DodgeRoll) {
                self.mode = Mode::Ground;
                self.start_roll(player, dir, clips, &mut out.step);
                return;
            }
            self.velocity = keep;
            let grounded = !self.airborne_attack(player, world, p);
            self.finish_attack(air && !grounded);
            return;
        }
        if chain_now {
            let launch = a.launch;
            let grounded = moved.on_floor || !self.airborne_attack(player, world, p);
            self.finish_attack(air && !grounded);
            let ctx = self.ctx(player, scene, air);
            let next = if launch {
                self.chain.launch_in(&ctx)
            } else {
                Some(self.chain.next(&self.abilities, &ctx))
            };
            if let Some(pick) = next
                && self.start_attack(
                    player,
                    pick.id,
                    pick.finisher,
                    pick.last,
                    scene,
                    &mut out.step,
                    clips,
                )
            {
                let finisher = pick.finisher;
                if launch {
                    self.launched = true;
                }
                out.began
                    .push(if finisher { "combo_finisher" } else { "combo" });
            }
            return;
        }
        if ended || landed_end {
            let grounded = moved.on_floor || !self.airborne_attack(player, world, p);
            self.finish_attack(air && !grounded);
            self.chain_hit = false;
            self.launched = false;
            return;
        }
        self.attack = Some(a);
    }

    fn airborne_attack(&self, player: &Player, world: &impl World, p: &Params) -> bool {
        let y = player.position[1];
        world
            .floor(player.position[0], player.position[2], y + p.step_height)
            .is_none_or(|floor| y > floor + 1.)
    }

    /// Hands him back to the movement states as the attack's End does.
    fn finish_attack(&mut self, falling: bool) {
        self.attack = None;
        if falling {
            self.mode = Mode::Air {
                jump: Jump::None,
                hang: t::HANG_TICKS,
                released: true,
                tick: 0,
            };
        } else {
            self.velocity = [0.; 3];
            self.mode = Mode::Ground;
        }
    }

    /// The window's world points this tick, in the actor's facing.
    pub fn attack_points(&self, player: &Player, info: &ClipInfo) -> Option<[[f32; 3]; 2]> {
        let a = self.attack.as_ref()?;
        let pts = info.atk_at(a.time)?;
        let k = length_of(1.);
        let (fx, fz) = (player.yaw.sin(), player.yaw.cos());
        let world = |c: [f32; 3]| {
            [
                player.position[0] + (fx * c[0] - fz * c[1]) * k,
                player.position[1] + c[2] * k,
                player.position[2] + (fz * c[0] + fx * c[1]) * k,
            ]
        };
        Some([world(pts[0]), world(pts[1])])
    }
}

impl Attack {
    /// A cast may follow once the swing's first window opened.
    pub fn swing_open(&self) -> bool {
        self.swing_started && !self.queued && !self.finish_end
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn root_delta_interpolates_at_30_hz() {
        let c = ClipInfo {
            length: 0.1,
            root: vec![
                [0.; 4],
                [10., 0., 0., 0.],
                [20., 0., 0., 0.],
                [30., 0., 0., 0.],
            ],
            ..ClipInfo::default()
        };
        let d = c.root_delta(0., 0.05);
        assert!((d[0] - 15.).abs() < 1e-4);
    }

    #[test]
    fn hit_points_interpolate() {
        let c = ClipInfo {
            atk: vec![[[0.; 3]; 2], [[3.; 3]; 2]],
            ..ClipInfo::default()
        };
        let a = c.atk_at(1. / 60.).unwrap();
        assert!((a[0][0] - 1.5).abs() < 1e-4);
    }

    #[test]
    fn the_kind_table_follows_the_sheet() {
        assert_eq!(kind_of(1).root_len, None);
        assert!(!kind_of(1).air_mode);
        assert_eq!(kind_of(4).root_len, Some(523.));
        assert_eq!(kind_of(0x0d).stop_dist, 250.);
        assert_eq!(kind_of(0x14).z_gain, 4.5);
        assert!(kind_of(0x10).keep_face);
    }

    #[test]
    fn traction_windows_are_half_open() {
        let c = ClipInfo {
            traction: vec![(0.1, 0.2)],
            ..ClipInfo::default()
        };
        assert!(!c.traction_open(0.05));
        assert!(c.traction_open(0.1));
        assert!(!c.traction_open(0.2));
    }
}
