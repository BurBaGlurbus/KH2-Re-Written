//! KH3's magic: one cast state for every spell and the spell actors (bolts,
//! columns, barriers, heals), their numbers read from the spell blueprints in
//! the player's paks (`Book::spells`). The rules are from StrawHatPro's reading
//! of KH3 (`crates/kh-core/src/states/fire_magic.rs`, `magic/projectile.rs`,
//! `magic/grand.rs`; MIT): TresPlayerState_FireMagic, homing 0x141481160, the
//! OnShot 0x14137BB30.
//!
//! Lengths in an actor's data are KH3's centimetres; its place is the room's.

use super::tuning::{self as t, length_of};
use super::{Kh3, Mode, Outcome, Scene, Vec3, wrap, yaw_of};
use crate::{Action, Clips, Input, Params, Player, Step, World};
use std::sync::Arc;

/// An actor shape's sizes.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ActorShape {
    pub row: String,
    pub radius: f32,
    pub grow: f32,
    pub grow_max: f32,
    /// A capsule's or column's half height; 0 for a sphere.
    pub half: f32,
    pub half_grow: f32,
    pub half_max: f32,
    pub centre_z: f32,
    /// A curve's start replacing the radius for its time (Firaga's 250 to 60).
    pub burst: Option<(f32, f32)>,
}

impl ActorShape {
    pub fn radius_at(&self, age: f32) -> f32 {
        if let Some((from, time)) = self.burst
            && age < time
        {
            return from + (self.radius - from) * (age / time.max(1e-4));
        }
        let r = (self.radius + self.grow * age).max(0.);
        if self.grow_max > 0. {
            r.min(self.grow_max)
        } else {
            r
        }
    }

    pub fn half_at(&self, age: f32) -> f32 {
        let h = (self.half + self.half_grow * age).max(0.);
        if self.half_max > 0. {
            h.min(self.half_max)
        } else {
            h
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Homing {
    pub delay: f32,
    pub turn: f32,
    pub accel: f32,
    pub cap: f32,
    pub min_dist: f32,
}

/// When a successor spawns.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Respawn {
    #[default]
    Never,
    HitAlways,
    HitAny,
    HitPawn,
    HitMap,
    NoHit,
}

/// One blueprint in a spell's chain.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Stage {
    pub name: String,
    pub life: f32,
    pub speed: f32,
    pub max_speed: f32,
    pub accel: f32,
    pub move_delay: f32,
    pub homing: Option<Homing>,
    /// Stops on a wall within this angle; else it slides along.
    pub map_stop: f32,
    pub shapes: Vec<ActorShape>,
    pub row_finish: Option<String>,
    pub row_grand: Option<String>,
    pub body_effect: Option<String>,
    pub fire_effects: Vec<(Option<String>, Option<String>)>,
    pub hit_effect: Option<String>,
    pub map_hit_effect: Option<String>,
    pub hit_sound: Option<String>,
    pub map_hit_sound: Option<String>,
    pub survives_hit: bool,
    pub respawn: Respawn,
    pub next: Option<Arc<Stage>>,
    /// Cure's `m_AppendCure`, when it is a heal.
    pub cure: Option<f32>,
    /// Water's barrier stands on the caster (`Nml` of a Water family).
    pub barrier: bool,
}

/// A spell of `TresCmdData` and `p_ex001_MagicSet`.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Spell {
    pub cmd: u16,
    /// MP cost (`Mp`); 255 is everything left.
    pub mp: i32,
    /// `Attr`: 1 fire, 2 blizzard, 3 thunder, 4 water, 5 aero, 7 cure.
    pub attr: u8,
    /// `m_bIsCastType`: the Cast and Chant clips.
    pub cast_type: bool,
    /// `m_bIsDirectPos`: spawned at the target.
    pub direct: bool,
    pub grand: bool,
    pub stage: Arc<Stage>,
    pub cast_effect: Option<String>,
}

/// The cast state.
#[derive(Clone, Debug, PartialEq)]
pub struct Cast {
    pub cmd: u16,
    pub row: &'static str,
    pub time: f32,
    pub shot_at: f32,
    pub change_at: f32,
    pub length: f32,
    pub air: bool,
    pub shot: bool,
    pub finish_end: bool,
    pub grand: bool,
    pub moving: Option<f32>,
    pub first: bool,
    pub follow_up: bool,
}

/// A live actor.
#[derive(Clone, Debug, PartialEq)]
pub struct Actor {
    pub serial: u32,
    pub cmd: u16,
    pub stage: Arc<Stage>,
    pub pos: Vec3,
    pub dir: Vec3,
    pub speed: f32,
    pub age: f32,
    pub turn: f32,
    pub homing_on: bool,
    pub finish: bool,
    pub grand: bool,
    /// The other Sora was already hit (each actor hits a victim once).
    pub hit: bool,
    /// Last tick's place, for the sweep.
    pub previous: Vec3,
    pub dead: bool,
}

/// Something for the presenters: an effect or a sound by name, at a place.
#[derive(Clone, Debug, PartialEq)]
pub struct FxRequest {
    pub effect: Option<String>,
    pub sound: Option<String>,
    pub at: Vec3,
    pub yaw: f32,
    /// The effect stays on a Sora until stopped, or follows a bone of his.
    pub attach: Option<FxAttach>,
}

/// An effect on a Sora: `Effects::spawn` with `Attach::Sora`, or the stop of one.
#[derive(Clone, Debug, PartialEq)]
pub struct FxAttach {
    pub seat: u8,
    /// A bone or socket; none: the actor's origin.
    pub bone: Option<String>,
    /// cm, in the bone's frame.
    pub offset: [f32; 3],
    /// The core's handle, which a later stop names.
    pub handle: u32,
    /// The group: which rows of an attach-data asset (`ead_*`) play.
    pub group: u32,
    /// Stops the effect of `handle` (true: at once) instead of starting one.
    pub stop: Option<bool>,
}

/// What a move's clips are per class.
fn cast_row(
    spell: &Spell,
    finish_end: bool,
    grand: bool,
    air: bool,
    moving: Option<f32>,
) -> &'static str {
    let heavy = finish_end || grand;
    if spell.cast_type && !(grand && spell.attr == 7) {
        return match (heavy, air, moving.is_some()) {
            (true, false, _) => "MagicChant",
            (true, true, _) => "MagicAirChant",
            (false, false, false) => "MagicCast",
            (false, false, true) => "MagicCastM",
            (false, true, false) => "MagicAirCast",
            (false, true, true) => "MagicAirCastM",
        };
    }
    if heavy {
        return if air { "MagicAirShoot" } else { "MagicShoot" };
    }
    match (air, moving) {
        (false, None) => "MagicFire",
        (true, None) => "MagicAirFire",
        (false, Some(d)) => {
            if d.abs() < 50. {
                "MagicFireMF"
            } else if d < 0. {
                "MagicFireML"
            } else {
                "MagicFireMR"
            }
        }
        (true, Some(d)) => {
            if d.abs() < 50. {
                "MagicAirFireMF"
            } else if d < 0. {
                "MagicAirFireML"
            } else {
                "MagicAirFireMR"
            }
        }
    }
}

/// The wire's offer: a KH2 spell below this, a KH3 grand command from it.
pub const GRAND_FIRST: u8 = 110;

/// The offer's command one tier above the highest cast (S_FIRA is 110, three
/// a family; Fire is 29, four a family).
pub fn grand_of(cmd: u16) -> Option<u16> {
    let (family, tier) = match cmd {
        29..=31 => (0, cmd - 29),
        33..=35 => (1, cmd - 33),
        37..=39 => (2, cmd - 37),
        41..=43 => (3, cmd - 41),
        45..=47 => (4, cmd - 45),
        _ => return None,
    };
    Some(110 + family * 3 + tier.min(2))
}

/// A stage to spawn: its command, place, direction, finish and grand flags,
/// and whether the victim was already hit.
type Successor = (Arc<Stage>, u16, Vec3, Vec3, bool, bool, bool);

impl Kh3 {
    /// The room's request: the command `cmd`, as the player chose it.
    pub fn request_cast(&mut self, cmd: u16) {
        self.want_cast = Some(cmd);
    }

    fn may_cast(&self, player: &Player) -> bool {
        if player.dead() || player.mp.charging() {
            return false;
        }
        match self.mode {
            Mode::Ground | Mode::Air { .. } | Mode::Glide { .. } | Mode::Kh2 => true,
            Mode::Attack => self.attack.as_ref().is_some_and(|a| a.swing_open()),
            Mode::Cast => self
                .cast
                .as_ref()
                .is_some_and(|c| c.shot && !c.follow_up && !c.finish_end),
            Mode::Roll { .. } | Mode::Slide { .. } => false,
            _ => false,
        }
    }

    /// Takes the room's request at the start of a tick.
    pub(super) fn take_cast_request(
        &mut self,
        player: &mut Player,
        input: &Input,
        scene: &Scene,
        clips: &Clips,
        step: &mut Step,
    ) -> bool {
        let Some(cmd) = self.want_cast.take() else {
            return false;
        };
        if !self.may_cast(player) {
            return false;
        }
        self.start_cast(player, cmd, input, scene, clips, step)
    }

    /// The cast's Begin and first tick.
    pub(super) fn start_cast(
        &mut self,
        player: &mut Player,
        cmd: u16,
        input: &Input,
        scene: &Scene,
        clips: &Clips,
        step: &mut Step,
    ) -> bool {
        let Some(book) = scene.book else { return false };
        let Some(spell) = book.spells.get(&cmd) else {
            return false;
        };
        let grand = spell.grand;
        let air = self.airborne() || player.in_air();
        // The chain's index goes on through a cast (MaxCombo, NumFinish).
        let in_chain = matches!(self.mode, Mode::Attack | Mode::Cast);
        if !in_chain {
            self.chain.start(&self.abilities);
        }
        let max = super::combo::combo_max(&self.abilities, !air);
        let finish_end = !grand && spell.attr != 7 && self.chain.n + 1 >= max;
        self.chain.n += 1;
        if finish_end {
            self.chain.close();
        }
        let (magnitude, dir) = super::stick(input);
        let fast = self.velocity[0].hypot(self.velocity[2]) > t::length_of(200.) / 60.;
        let moving = (!finish_end && !grand && fast && magnitude > 0.5)
            .then(|| wrap(yaw_of(dir) - player.yaw).to_degrees());
        let row = cast_row(spell, finish_end, grand, air, moving);
        let Some(info) = book.clips.get(row) else {
            return false;
        };
        let cast = Cast {
            cmd,
            row,
            time: 0.,
            shot_at: info.shots.first().copied().unwrap_or(0.3),
            change_at: info.enable_change.unwrap_or(info.length),
            length: info.length,
            air,
            shot: false,
            finish_end,
            grand,
            moving,
            first: true,
            follow_up: false,
        };
        self.attack = None;
        self.mode = Mode::Cast;
        self.aim = None;
        self.queued = false;
        if moving.is_none() {
            self.velocity = [0.; 3];
        } else {
            self.velocity[1] = self.velocity[1].min(0.);
        }
        // Grand magic and a finish end are invincible until the shot.
        if grand || finish_end {
            player.invulnerable = player.invulnerable.max(cast.shot_at * 60.);
        }
        self.cast = Some(cast);
        let shown = if air { Action::Fall } else { Action::Idle };
        self.show(player, shown, clips, step);
        player.armed = true;
        self.visual.restart(row, 0., false);
        if let Some(e) = &spell.cast_effect {
            self.fx.push(FxRequest {
                attach: None,
                effect: Some(e.clone()),
                sound: None,
                at: player.position,
                yaw: player.yaw,
            });
        }
        true
    }

    /// The cast's tick.
    #[allow(clippy::too_many_arguments)]
    pub(super) fn cast_tick(
        &mut self,
        player: &mut Player,
        input: &Input,
        p: &Params,
        clips: &Clips,
        world: &impl World,
        scene: &Scene,
        out: &mut Outcome,
    ) {
        let Some(mut c) = self.cast.take() else {
            self.mode = Mode::Ground;
            return;
        };
        let Some(book) = scene.book else {
            self.mode = Mode::Ground;
            return;
        };
        let Some(spell) = book.spells.get(&c.cmd).cloned() else {
            self.mode = Mode::Ground;
            return;
        };
        c.time += 1. / 60.;
        // The turn to the target until the shot: all at once, then 0.2 a tick.
        if !c.shot
            && let Some(f) = scene.foe
        {
            let d = [
                f.position[0] - player.position[0],
                f.position[2] - player.position[2],
            ];
            if d[0].hypot(d[1]) > 0.01 {
                let k = if c.first { 1. } else { 0.2 };
                player.yaw += wrap(yaw_of(d) - player.yaw) * k;
            }
        }
        c.first = false;
        // The shot.
        if !c.shot && c.time >= c.shot_at {
            c.shot = true;
            self.fire_spell(player, &spell, &c, scene);
            // Water's barrier: invincible from the shot to the state's end.
            if spell.stage.barrier {
                player.invulnerable = player.invulnerable.max((c.length - c.time) * 60.);
            }
        }
        // Movement: a moving cast keeps its speed; a standing one stops.
        let change = c.time >= c.change_at;
        let mut disp = [0f32; 3];
        if c.moving.is_some() && !change {
            disp[0] = self.velocity[0];
            disp[2] = self.velocity[2];
        }
        if c.air {
            // No fall until the shot; 0.1 of gravity until the change; then normal.
            let g = if !c.shot {
                0.
            } else if !change {
                t::GRAVITY * 0.1
            } else {
                t::GRAVITY
            };
            self.velocity[1] -= g;
            if !c.shot {
                self.velocity[1] = 0.;
            }
            disp[1] = self.velocity[1];
        }
        let moved = Self::travel(player, disp, c.air, p, world);
        // Follow-ups.
        if c.shot && !c.follow_up && !c.finish_end && input.attack && self.chain.open() {
            c.follow_up = true;
        }
        // A guard or a dodge at the change.
        let (magnitude, dir) = super::stick(input);
        if change && input.guard && !c.follow_up && !c.finish_end {
            self.cast = None;
            if c.air {
                if magnitude >= t::STICK_ACTIVE && self.slide_cooldown == 0 {
                    self.velocity = [0.; 3];
                    self.start_slide(player, dir, clips, &mut out.step);
                    return;
                }
            } else if magnitude >= t::STICK_ACTIVE {
                self.mode = Mode::Ground;
                self.start_roll(player, dir, clips, &mut out.step);
                return;
            }
        }
        let landed = c.air && moved.on_floor && c.shot;
        if landed {
            c.air = false;
            self.velocity = [0.; 3];
        }
        if (change && c.follow_up) || c.time >= c.length {
            let follow = c.follow_up;
            let air = c.air;
            self.cast = None;
            if air {
                self.mode = Mode::Air {
                    jump: super::Jump::None,
                    hang: t::HANG_TICKS,
                    released: true,
                    tick: 0,
                };
            } else {
                self.velocity = [0.; 3];
                self.mode = Mode::Ground;
            }
            if follow {
                let ctx = self.ctx(player, scene, air);
                let pick = self.chain.next(&self.abilities, &ctx);
                self.start_attack(
                    player,
                    pick.id,
                    pick.finisher,
                    pick.last,
                    scene,
                    &mut out.step,
                    clips,
                );
            }
            return;
        }
        self.cast = Some(c);
    }

    /// OnShot: the muzzle, MP, the actor and the fire effects.
    fn fire_spell(&mut self, player: &mut Player, spell: &Spell, c: &Cast, scene: &Scene) {
        let k = length_of(1.);
        let (fx, fz) = (player.yaw.sin(), player.yaw.cos());
        // The muzzle is the blade's tip; its height is limited to +163 cm.
        let muzzle = [
            player.position[0] + fx * 80. * k,
            player.position[1] + 130. * k,
            player.position[2] + fz * 80. * k,
        ];
        // MP at the shot; a grand magic costs none.
        if !spell.grand {
            if spell.mp >= 255 {
                player.mp.start_charge(t::MP_CHARGE);
            } else {
                player.mp.change(-spell.mp, t::MP_CHARGE);
            }
            player.mp_charging = player.mp.charging();
        }
        let target = scene.foe.map(|f| f.centre);
        let (at, dir) = if spell.direct {
            let at = target.map_or(
                [
                    player.position[0] + fx * 500. * k,
                    player.position[1],
                    player.position[2] + fz * 500. * k,
                ],
                |t| [t[0], scene.foe.map_or(t[1], |f| f.position[1]), t[2]],
            );
            (at, [fx, 0., fz])
        } else {
            let d = match target {
                Some(t) => {
                    let v = [t[0] - muzzle[0], t[1] - muzzle[1], t[2] - muzzle[2]];
                    let l = (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt().max(1e-3);
                    // The pitch snaps within 45 degrees of level.
                    let flat = v[0].hypot(v[2]).max(1e-3);
                    let pitch = (v[1] / flat)
                        .atan()
                        .clamp(-45f32.to_radians(), 45f32.to_radians());
                    let _ = l;
                    [fx * pitch.cos(), pitch.sin(), fz * pitch.cos()]
                }
                None => [fx, 0., fz],
            };
            (muzzle, d)
        };
        let stage = Arc::clone(&spell.stage);
        for (effect, sound) in &stage.fire_effects {
            self.fx.push(FxRequest {
                attach: None,
                effect: effect.clone(),
                sound: sound.clone(),
                at: muzzle,
                yaw: player.yaw,
            });
        }
        self.spawn_actor(stage, c.cmd, at, dir, c.finish_end, c.grand, false);
        self.spell_style(spell);
    }

    #[allow(clippy::too_many_arguments)]
    pub(super) fn spawn_actor(
        &mut self,
        stage: Arc<Stage>,
        cmd: u16,
        at: Vec3,
        dir: Vec3,
        finish: bool,
        grand: bool,
        hit: bool,
    ) {
        self.actor_serial = self.actor_serial.wrapping_add(1);
        let speed = stage.speed;
        self.actors.push(Actor {
            serial: self.actor_serial,
            cmd,
            stage,
            pos: at,
            dir,
            speed,
            age: 0.,
            turn: 0.,
            homing_on: true,
            finish,
            grand,
            hit,
            previous: at,
            dead: false,
        });
    }

    /// The actors' tick: life, successors, homing, movement.
    pub(super) fn step_actors(
        &mut self,
        player: &mut Player,
        heal: &mut i32,
        scene: &Scene,
        world: &impl World,
        p: &Params,
    ) {
        let k = length_of(1.);
        let dt = 1. / 60.;
        let target = scene.foe.map(|f| f.centre);
        let mut spawned: Vec<Successor> = Vec::new();
        let (px, py, pz, pyaw) = (
            player.position[0],
            player.position[1],
            player.position[2],
            player.yaw,
        );
        for a in &mut self.actors {
            if a.dead {
                continue;
            }
            a.age += dt;
            a.previous = a.pos;
            if let Some(amount) = a.stage.cure {
                *heal += amount as i32;
                a.dead = true;
                continue;
            }
            if a.stage.barrier {
                // Attached at (50, 0, 100), its forward 60 degrees up.
                let (fx, fz) = (pyaw.sin(), pyaw.cos());
                a.pos = [px + fx * 50. * k, py + 100. * k, pz + fz * 50. * k];
                a.dir = [fx * 0.5, 0.866, fz * 0.5];
            }
            if a.age > a.stage.life {
                if !a.hit
                    && matches!(a.stage.respawn, Respawn::NoHit | Respawn::HitAlways)
                    && let Some(next) = &a.stage.next
                {
                    spawned.push((
                        Arc::clone(next),
                        a.cmd,
                        a.pos,
                        a.dir,
                        a.finish,
                        a.grand,
                        a.hit,
                    ));
                }
                a.dead = true;
                continue;
            }
            if a.stage.barrier || a.speed <= 0. || a.age < a.stage.move_delay {
                continue;
            }
            // Homing 0x141481160.
            if let (Some(h), Some(t), true) = (a.stage.homing, target, a.homing_on)
                && a.age >= a.stage.move_delay + h.delay
            {
                {
                    a.turn = if h.cap > 0. {
                        (a.turn.max(h.turn) + h.accel * dt).min(h.cap)
                    } else {
                        a.turn.max(h.turn) + h.accel * dt
                    };
                    let v = [t[0] - a.pos[0], t[1] - a.pos[1], t[2] - a.pos[2]];
                    let d = (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt();
                    if d / k <= h.min_dist {
                        a.homing_on = false;
                    } else {
                        let u = [v[0] / d, v[1] / d, v[2] / d];
                        let cos =
                            (a.dir[0] * u[0] + a.dir[1] * u[1] + a.dir[2] * u[2]).clamp(-1., 1.);
                        let angle = cos.acos();
                        let max_step = (a.turn * dt).to_radians().min(std::f32::consts::PI);
                        if angle > 1e-4 {
                            let s = (max_step / angle).min(1.);
                            let m = [
                                a.dir[0] + (u[0] - a.dir[0]) * s,
                                a.dir[1] + (u[1] - a.dir[1]) * s,
                                a.dir[2] + (u[2] - a.dir[2]) * s,
                            ];
                            let l = (m[0] * m[0] + m[1] * m[1] + m[2] * m[2]).sqrt().max(1e-4);
                            a.dir = [m[0] / l, m[1] / l, m[2] / l];
                        }
                    }
                }
            }
            a.speed = (a.speed + a.stage.accel * dt).min(if a.stage.max_speed > 0. {
                a.stage.max_speed
            } else {
                f32::MAX
            });
            let step = a.speed * dt * k;
            let next = [
                a.pos[0] + a.dir[0] * step,
                a.pos[1] + a.dir[1] * step,
                a.pos[2] + a.dir[2] * step,
            ];
            // The floor stops a bolt (it explodes; its successor is the map hit's).
            if let Some(floor) = world.floor(next[0], next[2], a.pos[1] + p.step_height)
                && next[1] <= floor
            {
                self.fx.push(FxRequest {
                    attach: None,
                    effect: a.stage.map_hit_effect.clone(),
                    sound: a.stage.map_hit_sound.clone(),
                    at: next,
                    yaw: 0.,
                });
                if matches!(
                    a.stage.respawn,
                    Respawn::HitAny | Respawn::HitMap | Respawn::HitAlways
                ) && let Some(n) = &a.stage.next
                {
                    spawned.push((Arc::clone(n), a.cmd, next, a.dir, a.finish, a.grand, a.hit));
                }
                a.dead = true;
                continue;
            }
            a.pos = next;
        }
        self.actors.retain(|a| !a.dead);
        for (stage, cmd, at, dir, finish, grand, hit) in spawned {
            self.spawn_actor(stage, cmd, at, dir, finish, grand, hit);
        }
    }

    /// An actor hit a body: its effects, whether it dies, its successor.
    pub fn actor_hit(&mut self, index: usize, at: Vec3) {
        let Some(a) = self.actors.get_mut(index) else {
            return;
        };
        a.hit = true;
        let (stage, cmd, dir, finish, grand) =
            (Arc::clone(&a.stage), a.cmd, a.dir, a.finish, a.grand);
        if !stage.survives_hit && stage.speed > 0. && !stage.barrier {
            a.dead = true;
        }
        self.fx.push(FxRequest {
            attach: None,
            effect: stage.hit_effect.clone(),
            sound: stage.hit_sound.clone(),
            at,
            yaw: 0.,
        });
        if matches!(
            stage.respawn,
            Respawn::HitPawn | Respawn::HitAny | Respawn::HitAlways
        ) && let Some(n) = &stage.next
        {
            self.spawn_actor(Arc::clone(n), cmd, at, dir, finish, grand, true);
        }
    }

    /// `m_FormPoint`s of a hit: the situation gauge, by element.
    pub fn form_point(&mut self, points: f32, cmd: Option<u16>) {
        self.style += points;
        self.style_idle = 0;
        if let Some(g) = cmd.and_then(grand_of) {
            self.grand_points.push((g, points));
        }
        if self.style >= t::STYLE_MAX {
            self.style = 0.;
            let mut best: Option<(u16, f32)> = None;
            for &(g, p) in &self.grand_points {
                if best.is_none_or(|b| p > b.1 || (p == b.1 && g > b.0)) {
                    best = Some((g, p));
                }
            }
            if let Some((g, _)) = best {
                self.grand_offer = Some((g, t::OFFER_TICKS));
                self.pending.push("grand_offer");
            }
            self.grand_points.clear();
        }
    }

    fn spell_style(&mut self, _spell: &Spell) {}
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn grand_commands_are_a_tier_above() {
        assert_eq!(grand_of(29), Some(110));
        assert_eq!(grand_of(31), Some(112));
        assert_eq!(grand_of(35), Some(115));
        assert_eq!(grand_of(49), None);
    }

    #[test]
    fn a_burst_shrinks_to_its_size() {
        let s = ActorShape {
            radius: 60.,
            burst: Some((250., 0.07)),
            ..ActorShape::default()
        };
        assert_eq!(s.radius_at(0.), 250.);
        assert_eq!(s.radius_at(0.1), 60.);
    }

    #[test]
    fn a_shape_grows_to_its_limit() {
        let s = ActorShape {
            radius: 100.,
            grow: 1200.,
            grow_max: 400.,
            ..ActorShape::default()
        };
        assert_eq!(s.radius_at(1.), 400.);
    }

    #[test]
    fn clips_follow_the_class_and_the_moment() {
        let shot = Spell::default();
        let cast = Spell {
            cast_type: true,
            ..Spell::default()
        };
        assert_eq!(cast_row(&shot, false, false, false, None), "MagicFire");
        assert_eq!(cast_row(&shot, true, false, true, None), "MagicAirShoot");
        assert_eq!(
            cast_row(&shot, false, false, false, Some(-60.)),
            "MagicFireML"
        );
        assert_eq!(cast_row(&cast, false, false, true, None), "MagicAirCast");
        assert_eq!(cast_row(&cast, false, true, false, None), "MagicChant");
    }
}
