//! KH3's lock-on camera and the shotlock's aiming camera in the KH3 ruleset
//! [T279], run on the fixed tick after the follow camera.
//!
//! The lock-on camera is KH3's `ATresCameraNormalLoc`, translated from the
//! KH3 executable: the free camera spawns it on a lock and blends to it
//! (0x14060ab70, 0.5 s, ease-out with exponent 2, both ways), then each frame
//! (0x14060edb0) turns it with the right stick and a slow auto-turn while
//! Sora moves (0x140611170, 0x1405ffa60), holds it within limits about the
//! Sora-to-target heading (0x140611760), and keeps both in frame by moving
//! its pivot to their midpoint and backing off until the pair fits the
//! field of view (0x1406100d0 with the solver 0x1405defa0 / 0x1405e0590).
//! It never aims at the target, which is what keeps it on Sora. Every
//! smoothing step is the executable's one law: steps of 1/60 s, each moving
//! `rate / 60` of the gap (0x1405d0850, 0x1405d0b30, 0x1405d0650).
//!
//! The aiming camera is KH3's over-the-shoulder camera
//! (`ATresCameraTargetOTS`, 0x14062d850): a pivot offset in camera space from
//! Sora's point, looking at the target, pitch held within +-40. Its distance
//! and shoulder offset are Blueprint values in KH3's paks, so those are
//! [duel choice] numbers here.
//!
//! KH3's units are scaled by 160/180 like the layer's (`kh3::tuning`);
//! angles are degrees as the executable keeps them. Yaw is the scene's
//! (`atan2(x, z)`; a turn to the right lowers it), pitch is up.

use super::*;
use player::kh3::tuning::length_of as cm;
use std::f32::consts::FRAC_PI_2;

/// The approach law's step.
const STEP: f32 = 1. / 60.;
/// 0x14060ab70: lock-on blends in and out over 0.5 s (blend func 3, exp 2).
const LOCK_BLEND: f32 = 0.5;
/// ATresCameraBase: stick speed 120 deg/s, dead zone 0.1, pivot rate 8 with
/// a 100 cm lag limit, rotation rate 8.
const STICK_SPEED: f32 = 120.;
const DEAD_ZONE: f32 = 0.1;
const PIVOT_RATE: f32 = 8.;
const PIVOT_LAG: f32 = 100.;
const ROTATION_RATE: f32 = 8.;
/// ATresCameraNormal's lock parameters as copied into the lock camera
/// (0x14060a930): pitch -20 on the reference, limits -70 / 20, yaw 120;
/// the stick slows from 1 at 200 cm to 0.5 at 1000 cm; 400 cm least
/// distance; auto-turn power 1.2 at most, target speed 800 cm/s, 1.5 s of
/// wait after the stick.
const LOCK_PITCH: f32 = -20.;
const PITCH_FLOOR: f32 = -10.;
const PITCH_MIN: f32 = -70.;
const PITCH_MAX: f32 = 20.;
const YAW_LIMIT: f32 = 120.;
const SLOW_NEAR: f32 = 200.;
const SLOW_FAR: f32 = 1000.;
const SLOW_RATE: f32 = 0.5;
const LOCK_DISTANCE: f32 = 400.;
const AUTO_POWER: f32 = 1.2;
const TARGET_SPEED: f32 = 800.;
const MOVE_WAIT: f32 = 1.5;
/// 0x1405ffa60: while Sora runs at the camera the yaw is kept.
const FACE_DOT: f32 = -0.8;
/// ATresCameraNormalLoc: its distance rate 8; the remembered distance
/// grows back to the least at 2.
const DISTANCE_RATE: f32 = 8.;
const MEMORY_RATE: f32 = 2.;
/// 0x1406100d0: a target point moving over 3000 cm/s is followed at rate 6.
const TARGET_JUMP: f32 = 3000.;
const TARGET_RATE: f32 = 6.;
/// 0x1406100d0: Sora's point leads by his velocity times this times his
/// distance from the screen's middle, when the lead takes him farther from
/// the target on screen.
const LEAD: f32 = 0.24;
/// The two subjects' radii (the lock radius's default, 50 cm).
const RADIUS: f32 = 50.;
/// 0x1405e0590: the framing's horizontal margin and the viewport's aspect
/// (height over width).
const MARGIN: f32 = 1.1;
const ASPECT: f32 = 0.5625;
/// 0x1405defa0: solver passes when the two are apart.
const PASSES: usize = 4;
/// `TresCameraShootFlow`'s constructor: pitch limits -70 / 70, rotation rate
/// 8; blends as the lock's.
const AIM_PITCH: f32 = 70.;
/// Its spring (0x1405FD484) and local offset (0x14141984D): 200 cm back, the
/// look point 50 forward and 100 right of the socket `Zup_Cam` (102.338 up)
/// and its target offset (50 up, 0x14141981D).
const AIM_DISTANCE: f32 = 200.;
const AIM_OFFSET: [f32; 2] = [50., 100.];
const AIM_HEIGHT: f32 = 152.338;

/// The exe's approach: `cur` moves `rate / 60` of the gap a 1/60 s step.
fn approach(cur: f32, target: f32, rate: f32, dt: f32) -> f32 {
    let mut c = cur;
    let mut t = dt;
    while t > 1e-6 {
        let a = (t.min(STEP) * rate).clamp(0., 1.);
        c += (target - c) * a;
        t -= STEP;
    }
    c
}

/// The vector approach, pulled back within `limit` of the target.
fn approach_vec(cur: Vec3, target: Vec3, rate: f32, dt: f32, limit: Option<f32>) -> Vec3 {
    let mut c = cur;
    let mut t = dt;
    while t > 1e-6 {
        let a = (t.min(STEP) * rate).clamp(0., 1.);
        c += (target - c) * a;
        t -= STEP;
    }
    if let Some(l) = limit
        && c.distance(target) > l
    {
        c = target + (c - target).normalize_or_zero() * l;
    }
    c
}

/// Degrees into (-180, 180].
fn norm(a: f32) -> f32 {
    let a = (a + 180.).rem_euclid(360.) - 180.;
    if a <= -180. { a + 360. } else { a }
}

/// A rotation: pitch, yaw (degrees).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
struct Rot {
    pitch: f32,
    yaw: f32,
}

impl Rot {
    fn forward(self) -> Vec3 {
        let (p, y) = (self.pitch.to_radians(), self.yaw.to_radians());
        Vec3::new(p.cos() * y.sin(), p.sin(), p.cos() * y.cos())
    }

    fn look(d: Vec3) -> Rot {
        let flat = Vec2::new(d.x, d.z).length();
        Rot {
            pitch: d.y.atan2(flat).to_degrees(),
            yaw: d.x.atan2(d.z).to_degrees(),
        }
    }

    /// Each axis moves the shorter way.
    fn approach(self, to: Rot, rate: f32, dt: f32) -> Rot {
        let mut r = self;
        let mut t = dt;
        while t > 1e-6 {
            let a = (t.min(STEP) * rate).clamp(0., 1.);
            r.pitch += norm(to.pitch - r.pitch) * a;
            r.yaw = norm(r.yaw + norm(to.yaw - r.yaw) * a);
            t -= STEP;
        }
        r
    }
}

/// The lock camera's own state.
#[derive(Clone, Copy, Debug)]
struct Lock {
    /// The logical rotation (stick and auto-turn) and the shown one.
    rot: Rot,
    shown: Rot,
    look: Vec3,
    distance: f32,
    memory: f32,
    target: Vec3,
    target_smooth: Vec3,
    ramp: f32,
    wait: f32,
    /// The smoothed velocity lead on Sora's point.
    lead: Vec3,
}

/// The aiming camera's own state.
#[derive(Clone, Copy, Debug)]
struct Aim {
    shown: Rot,
    target: Vec3,
}

/// A blend's place: in (toward the camera) or out, and its time.
#[derive(Clone, Copy, Debug, PartialEq)]
struct Blend {
    on: bool,
    q: f32,
}

impl Default for Blend {
    /// Out and gone.
    fn default() -> Self {
        Blend {
            on: false,
            q: LOCK_BLEND,
        }
    }
}

impl Blend {
    /// 0x14060ab70's ease-out, exponent 2: the camera's weight.
    fn weight(self) -> f32 {
        let q = (self.q / LOCK_BLEND).clamp(0., 1.);
        if self.on {
            1. - (1. - q) * (1. - q)
        } else {
            (1. - q) * (1. - q)
        }
    }

    /// Turns toward `on`, carrying the weight on (a blend reversed midway
    /// starts from where it is).
    fn turn(&mut self, on: bool, dt: f32) {
        if on != self.on {
            let w = self.weight();
            let q = if on {
                1. - (1. - w).sqrt()
            } else {
                1. - w.sqrt()
            };
            *self = Blend {
                on,
                q: q * LOCK_BLEND,
            };
        }
        self.q = (self.q + dt).min(LOCK_BLEND);
    }

    fn gone(self) -> bool {
        !self.on && self.q >= LOCK_BLEND
    }
}

/// Both cameras, kept between ticks.
#[derive(Clone, Debug, Default)]
pub struct Kh3Camera {
    lock: Option<Lock>,
    lock_blend: Blend,
    aim: Option<Aim>,
    aim_blend: Blend,
    previous: Option<Vec3>,
}

/// What one tick reads.
pub struct Frame {
    /// Sora's feet and facing, the locked target's point, the aim's.
    pub feet: Vec3,
    pub yaw: f32,
    pub locked: Option<Vec3>,
    pub aim: Option<Vec3>,
    /// The right stick: turn (right +) and height (down +).
    pub turn: f32,
    pub height: f32,
    /// The follow camera's view and vertical field of view.
    pub eye: Vec3,
    pub look: Vec3,
    pub fov: f32,
}

/// Sora's point: his feet raised to the look height (the actor point and
/// the pivot height, 0x140611760; `kh3::tuning::LOCK_LOOK_UP`).
fn player_point(feet: Vec3) -> Vec3 {
    feet + Vec3::Y * player::kh3::tuning::LOCK_LOOK_UP
}

/// 0x140613300: the rotation from Sora's point to the pair's midpoint
/// (Sora's facing when it is right above or below him).
fn reference(p: Vec3, t: Vec3, facing: f32) -> Rot {
    let m = (p + t) * 0.5;
    let d = m - p;
    if Vec2::new(d.x, d.z).length_squared() < cm(1.) * cm(1.) {
        return Rot {
            pitch: 0.,
            yaw: facing.to_degrees(),
        };
    }
    Rot::look(d)
}

/// 0x140611760: pitch from -70 up to the target's elevation (0..20), yaw
/// within 120 of the reference.
fn limit(r: Rot, reference: Rot) -> Rot {
    let ceiling = reference.pitch.clamp(0., PITCH_MAX);
    let off = norm(r.yaw - reference.yaw).clamp(-YAW_LIMIT, YAW_LIMIT);
    Rot {
        pitch: r.pitch.clamp(PITCH_MIN, ceiling),
        yaw: norm(reference.yaw + off),
    }
}

/// 0x140606190: the reference pitched 20 down, not below -10, unless the
/// target is well below (then straight at it).
fn desired(reference: Rot) -> Rot {
    let pitch = if reference.pitch >= PITCH_FLOOR {
        (reference.pitch + LOCK_PITCH).max(PITCH_FLOOR)
    } else {
        reference.pitch
    };
    Rot {
        pitch,
        yaw: reference.yaw,
    }
}

/// The horizontal field of view from the vertical one.
fn horizontal(fov: f32) -> f32 {
    2. * ((fov * 0.5).tan() / ASPECT).atan()
}

/// 0x1405e0590 from camera position `c` looking along `forward`: the
/// distance at which the two subjects (grown by their radii) fit the field
/// of view, and the grown pair's midpoint.
fn solve(c: Vec3, forward: Vec3, hfov: f32, a: (Vec3, f32), b: (Vec3, f32)) -> (f32, Vec3) {
    let (far, near) = if a.0.distance(c) >= b.0.distance(c) {
        (a, b)
    } else {
        (b, a)
    };
    let depth = |p: Vec3| (p - c).dot(forward);
    let s = depth(near.0) / depth(far.0);
    let q = if depth(far.0).abs() > 1e-4 && (0. ..=1.).contains(&s) {
        c + (far.0 - c) * s
    } else {
        far.0
    };
    let right = forward.cross(Vec3::Y).normalize_or(Vec3::X);
    let dir = (near.0 - q).normalize_or(right);
    let e1 = q - dir * far.1;
    let e2 = near.0 + dir * near.1;
    let mid = (e1 + e2) * 0.5;
    let half = e1.distance(e2) * 0.5;
    let up = right.cross(forward);
    let screen = |p: Vec3| {
        let d = (p - c).dot(forward).max(1e-3);
        Vec2::new((p - c).dot(right) / d, (p - c).dot(up) / d)
    };
    let s_y = (screen(e2) - screen(e1)).normalize_or_zero().y.abs();
    let w = 1. - (s_y * FRAC_PI_2).cos();
    let a0 = FRAC_PI_2 - hfov * 0.5;
    let a1 = (half * a0.tan()).atan2(ASPECT * half);
    let angle = a0 + (a1 - a0) * w;
    let margin = half * MARGIN + (half - half * MARGIN) * w;
    (angle.tan() * margin, mid)
}

/// 0x1405defa0: the needed distance, the distance added on the one-pass
/// path, and the pivot.
fn frame(pivot: Vec3, p: Vec3, t: Vec3, shown: Rot, hfov: f32) -> (f32, f32, Vec3) {
    let forward = shown.forward();
    let r = cm(RADIUS);
    let m = (p + t) * 0.5;
    let mut c = m - forward * (0.5 * p.distance(t) / (hfov * 0.5).sin());
    if Vec2::new(t.x - p.x, t.z - p.z).length() >= cm(1.) {
        let mut out = (0., m);
        for _ in 0..PASSES {
            out = solve(c, forward, hfov, (p, r), (t, r));
            c = out.1 - forward * out.0;
        }
        (out.0, 0., out.1)
    } else {
        let (need, mid) = solve(c, forward, hfov, (p, r), (t, r));
        (need, pivot.distance(mid), pivot)
    }
}

impl Lock {
    fn new(f: &Frame, target: Vec3) -> Lock {
        let rot = Rot::look(f.look - f.eye);
        Lock {
            rot,
            shown: rot,
            look: player_point(f.feet),
            distance: cm(LOCK_DISTANCE),
            memory: cm(LOCK_DISTANCE),
            target,
            target_smooth: target,
            ramp: 0.,
            wait: 0.,
            lead: Vec3::ZERO,
        }
    }

    /// One frame of 0x14060edb0; `velocity` is Sora's (units a second).
    fn tick(&mut self, f: &Frame, velocity: Vec3, dt: f32) -> boss::OverlayView {
        let mut p = player_point(f.feet);
        // The target point jumps are followed, not taken.
        let mut t = self.target;
        if t.distance(self.target_smooth) > dt * cm(TARGET_JUMP) {
            self.target_smooth = approach_vec(self.target_smooth, t, TARGET_RATE, dt, None);
            t = self.target_smooth;
        } else {
            self.target_smooth = t;
        }
        // 0x140607390: the stick, slowed with the pair's distance.
        let (lo, hi) = (cm(SLOW_NEAR), cm(SLOW_FAR).max(cm(SLOW_NEAR)));
        let d = p.distance(t);
        let k = SLOW_RATE + (1. - SLOW_RATE) * (hi - d.clamp(lo, hi)) / (hi - lo);
        let dead = |v: f32| if v.abs() < DEAD_ZONE { 0. } else { v };
        let (sx, sy) = (dead(f.turn), dead(f.height));
        let d_pitch = STICK_SPEED * sy * dt * k;
        let d_yaw = -STICK_SPEED * sx * dt * k;
        let stick = d_pitch != 0. || d_yaw != 0.;
        self.rot = Rot {
            pitch: norm(self.rot.pitch + d_pitch),
            yaw: norm(self.rot.yaw + d_yaw),
        };
        let reference = reference(p, t, f.yaw);
        let turned = self.auto_turn(desired(reference), velocity, f.yaw, stick, dt, reference);
        self.rot = limit(self.rot, reference);
        self.shown = self.shown.approach(self.rot, ROTATION_RATE, dt);
        // 0x1406100d0: Sora's lead, measured on the screen as it now is.
        let forward = self.shown.forward();
        let eye = self.look - forward * self.distance;
        let right = forward.cross(Vec3::Y).normalize_or(Vec3::X);
        let up = right.cross(forward);
        let (tx, ty) = ((horizontal(f.fov) * 0.5).tan(), (f.fov * 0.5).tan());
        let screen = |x: Vec3| {
            let d = (x - eye).dot(forward).max(1e-3);
            Vec2::new(
                ((x - eye).dot(right) / d / tx).clamp(-1., 1.),
                ((x - eye).dot(up) / d / ty).clamp(-1., 1.),
            )
        };
        let (sp, st, sv) = (screen(p), screen(t), screen(p + velocity));
        let lead = if sv.distance(st) > sp.distance(st) {
            velocity * LEAD * sp.length()
        } else {
            Vec3::ZERO
        };
        self.lead = approach_vec(self.lead, lead, PIVOT_RATE, dt, None);
        p += self.lead;
        // The least distance, the framing, the smoothing.

        let base = cm(LOCK_DISTANCE);
        let least = if self.memory < base {
            if turned {
                approach(self.memory, base, MEMORY_RATE, dt)
            } else {
                self.memory
            }
        } else {
            base
        };
        let (need, extra, pivot) = frame(self.look, p, t, self.shown, horizontal(f.fov));
        self.memory = need.max(least);
        let want = self.memory + extra;
        self.look = approach_vec(self.look, pivot, PIVOT_RATE, dt, Some(cm(PIVOT_LAG)));
        self.distance = approach(self.distance, want, DISTANCE_RATE, dt);
        boss::OverlayView {
            eye: self.look - self.shown.forward() * self.distance,
            target: self.look,
            roll: 0.,
            fov: f.fov,
            collision: true,
        }
    }

    /// 0x1405ffa60: toward `to` only while Sora moves, at his speed over
    /// 800 cm/s times a ramp to 1.2, 1.5 s after the stick; the yaw stays
    /// while he runs at the camera.
    fn auto_turn(
        &mut self,
        to: Rot,
        velocity: Vec3,
        facing: f32,
        stick: bool,
        dt: f32,
        reference: Rot,
    ) -> bool {
        if stick {
            self.wait = MOVE_WAIT;
        } else if self.wait > 0. {
            self.wait -= dt;
            if self.wait <= 0. {
                self.wait = 0.;
                self.ramp = 0.;
            }
        }
        if stick || self.wait > 0. {
            return false;
        }
        if velocity.length() <= cm(1.) {
            self.ramp = 0.;
            return false;
        }
        self.ramp = (self.ramp + AUTO_POWER * dt).min(AUTO_POWER);
        let rate = Vec2::new(velocity.x, velocity.z).length() / cm(TARGET_SPEED) * self.ramp;
        let mut to = to;
        let mine = Vec3::new(facing.sin(), 0., facing.cos());
        let camera = self.rot.forward();
        if mine.dot(Vec3::new(camera.x, 0., camera.z).normalize_or_zero()) < FACE_DOT {
            to.yaw = self.rot.yaw;
        }
        self.rot = limit(self.rot.approach(to, rate, dt), reference);
        true
    }
}

impl Aim {
    fn new(f: &Frame, target: Vec3) -> Aim {
        let mut r = Rot::look(target - (f.feet + Vec3::Y * cm(AIM_HEIGHT)));
        r.pitch = r.pitch.clamp(-AIM_PITCH, AIM_PITCH);
        Aim { shown: r, target }
    }

    /// The look point in camera space, looking at the target; no distance
    /// smoothing.
    fn tick(&mut self, f: &Frame, dt: f32) -> boss::OverlayView {
        let forward = self.shown.forward();
        let right = forward.cross(Vec3::Y).normalize_or(Vec3::X);
        let pivot = f.feet
            + Vec3::Y * cm(AIM_HEIGHT)
            + forward * cm(AIM_OFFSET[0])
            + right * cm(AIM_OFFSET[1]);
        let mut r = Rot::look(self.target - pivot);
        r.pitch = r.pitch.clamp(-AIM_PITCH, AIM_PITCH);
        self.shown = self.shown.approach(r, ROTATION_RATE, dt);
        boss::OverlayView {
            eye: pivot - self.shown.forward() * cm(AIM_DISTANCE),
            target: pivot,
            roll: 0.,
            fov: f.fov,
            collision: true,
        }
    }
}

/// Blends two views by `w` (0: `a`).
fn mix(a: &boss::OverlayView, b: &boss::OverlayView, w: f32) -> boss::OverlayView {
    boss::OverlayView {
        eye: a.eye.lerp(b.eye, w),
        target: a.target.lerp(b.target, w),
        roll: a.roll + (b.roll - a.roll) * w,
        fov: a.fov + (b.fov - a.fov) * w,
        collision: a.collision || b.collision,
    }
}

impl Kh3Camera {
    /// One fixed tick: the view to show, or None for the follow camera's.
    pub fn tick(&mut self, f: &Frame) -> Option<boss::OverlayView> {
        let dt = STEP;
        let velocity = self.previous.map_or(Vec3::ZERO, |p| (f.feet - p) / dt);
        self.previous = Some(f.feet);
        let follow = boss::OverlayView {
            eye: f.eye,
            target: f.look,
            roll: 0.,
            fov: f.fov,
            collision: false,
        };
        // The lock camera: spawned on a lock, kept while it blends out.
        if let Some(t) = f.locked {
            if self.lock.is_none() {
                self.lock = Some(Lock::new(f, t));
            }
            if let Some(l) = self.lock.as_mut() {
                l.target = t;
            }
        }
        self.lock_blend.turn(f.locked.is_some(), dt);
        if self.lock_blend.gone() {
            self.lock = None;
        }
        let mut view = follow;
        if let Some(l) = self.lock.as_mut() {
            let v = l.tick(f, velocity, dt);
            view = mix(&follow, &v, self.lock_blend.weight());
        }
        // The aiming camera over whichever is shown.
        if let Some(t) = f.aim {
            match self.aim.as_mut() {
                Some(a) => a.target = t,
                None => self.aim = Some(Aim::new(f, t)),
            }
        }
        self.aim_blend.turn(f.aim.is_some(), dt);
        if self.aim_blend.gone() {
            self.aim = None;
        }
        if let Some(a) = self.aim.as_mut() {
            let v = a.tick(f, dt);
            view = mix(&view, &v, self.aim_blend.weight());
        }
        (self.lock.is_some() || self.aim.is_some()).then_some(view)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn frame_at(feet: Vec3, locked: Option<Vec3>) -> Frame {
        Frame {
            feet,
            yaw: 0.,
            locked,
            aim: None,
            turn: 0.,
            height: 0.,
            eye: feet + Vec3::new(0., 200., -600.),
            look: feet + Vec3::Y * 100.,
            fov: 50f32.to_radians(),
        }
    }

    #[test]
    fn the_approach_moves_a_sixtieth_of_the_rate_a_step() {
        assert!((approach(0., 60., 8., STEP) - 8.).abs() < 1e-4);
        // Two steps at 30 fps.
        let two = approach(0., 60., 8., 2. * STEP);
        assert!((two - (8. + 52. * 8. / 60.)).abs() < 1e-3);
        let v = approach_vec(Vec3::ZERO, Vec3::X * 1000., 8., STEP, Some(100.));
        assert!((v.x - 900.).abs() < 1e-3, "the lag limit pulls it in");
        let r = Rot {
            pitch: 0.,
            yaw: 170.,
        }
        .approach(
            Rot {
                pitch: 0.,
                yaw: -170.,
            },
            30.,
            STEP,
        );
        assert!((r.yaw.abs() - 180.).abs() < 1e-3, "the short way round");
    }

    #[test]
    fn the_blend_eases_out_and_reverses_where_it_is() {
        let mut b = Blend::default();
        b.turn(true, 0.25);
        assert!((b.weight() - 0.75).abs() < 1e-4);
        b.turn(false, 0.);
        assert!((b.weight() - 0.75).abs() < 1e-4, "{}", b.weight());
        b.turn(false, 1.);
        assert!(b.gone() && b.weight() == 0.);
    }

    #[test]
    fn limits_and_the_desired_pitch_follow_the_reference() {
        let r = Rot {
            pitch: 5.,
            yaw: 30.,
        };
        assert_eq!(
            desired(r),
            Rot {
                pitch: -10.,
                yaw: 30.
            }
        );
        let below = Rot {
            pitch: -40.,
            yaw: 0.,
        };
        assert_eq!(desired(below).pitch, -40.);
        let l = limit(
            Rot {
                pitch: 50.,
                yaw: 179.,
            },
            Rot {
                pitch: 10.,
                yaw: 0.,
            },
        );
        assert_eq!(l.pitch, 10.);
        assert_eq!(l.yaw, YAW_LIMIT);
    }

    #[test]
    fn the_pair_is_framed_from_its_middle_and_farther_apart_is_farther() {
        let hfov = horizontal(50f32.to_radians());
        let shown = Rot {
            pitch: -10.,
            yaw: 0.,
        };
        let p = Vec3::new(0., 100., 0.);
        let near = frame(p, p, p + Vec3::X * 300., shown, hfov);
        let far = frame(p, p, p + Vec3::X * 1200., shown, hfov);
        assert!(far.0 > near.0 * 2.);
        assert!((near.2.x - 150.).abs() < 30., "{:?}", near.2);
        // Right above him: the one-pass path keeps the pivot.
        let up = frame(p, p, p + Vec3::Y * 300., shown, hfov);
        assert_eq!(up.2, p);
    }

    #[test]
    fn the_lock_camera_stays_on_sora_while_the_target_circles_him() {
        let mut cam = Kh3Camera::default();
        let feet = Vec3::ZERO;
        let mut most: f32 = 0.;
        for i in 0..600 {
            // The target loops round and over him (an air combo's swings).
            let a = i as f32 * 0.05;
            let t = Vec3::new(a.sin() * 150., 100. + a.cos() * 120., a.cos() * 150.);
            let v = cam.tick(&frame_at(feet, Some(t))).expect("locked");
            let shown = player_point(feet);
            // Sora stays in front of the camera, near the middle of the view.
            let dir = (v.target - v.eye).normalize();
            let to_sora = (shown - v.eye).normalize();
            most = most.max(dir.angle_between(to_sora));
        }
        assert!(most < 0.6, "Sora left the view by {most} rad");
    }

    #[test]
    fn standing_still_the_lock_camera_does_not_turn_on_its_own() {
        let mut cam = Kh3Camera::default();
        let t = Vec3::new(400., 100., 0.);
        let first = cam.tick(&frame_at(Vec3::ZERO, Some(t))).unwrap();
        let mut last = first;
        for _ in 0..120 {
            last = cam.tick(&frame_at(Vec3::ZERO, Some(t))).unwrap();
        }
        let yaw = |v: &boss::OverlayView| Rot::look(v.target - v.eye).yaw;
        // Only the framing's pivot moves (toward the pair's middle).
        let lock = cam.lock.unwrap();
        assert!(norm(lock.rot.yaw - Rot::look(first.target - first.eye).yaw).abs() < 1.);
        assert!(yaw(&last).is_finite());
        // Unlocked, it blends back out and is gone.
        for _ in 0..40 {
            cam.tick(&frame_at(Vec3::ZERO, None));
        }
        assert!(cam.tick(&frame_at(Vec3::ZERO, None)).is_none());
    }

    #[test]
    fn aiming_comes_in_over_the_shoulder() {
        let mut cam = Kh3Camera::default();
        let mut f = frame_at(Vec3::ZERO, None);
        f.aim = Some(Vec3::new(0., 100., 2000.));
        let mut v = cam.tick(&f).unwrap();
        for _ in 0..60 {
            v = cam.tick(&f).unwrap();
        }
        // Closer than the follow camera, to his right, looking ahead.
        assert!(v.eye.distance(player_point(Vec3::ZERO)) < f.eye.distance(f.look));
        assert!(v.target.x < 0., "Sora's right is -X: {:?}", v.target);
        assert!((v.target - v.eye).z > 0.);
    }
}
