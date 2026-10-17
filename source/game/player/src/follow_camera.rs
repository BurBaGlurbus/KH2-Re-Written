//! Field follow camera (mode 0) and lock-on camera (mode 2), from the installed
//! PC executable's camera update: the per-mode table (field of view, radius,
//! lock radius, radius limits, height curve), the yaw follow that swings
//! behind a moving player, the radius return, and the spring that eases the
//! current eye and look-at points toward their targets. The lock-on mode
//! translates FUN_140176c70 (mode 2 update), FUN_140176ec0 (look point,
//! screen-edge tests and yaw), FUN_140176710 (screen flags), FUN_140176bc0
//! (zoom), FUN_140176a70 (bearing), FUN_140164520 (yaw ease and eye target),
//! FUN_140164290 (radius return) and FUN_140176e40 (eye collision); the
//! spring is FUN_140165700 / FUN_140165bd0.
//!
//! First-person mode (mode 1, toggled with Select) translates FUN_140165e30
//! (its update and exit), the entry in FUN_140165110 and the mode-1 collision of
//! FUN_140165700.
//!
//! Positions here are in the game's own world space: Y points down, so
//! "above the player" is negative Y. One `tick` is one 60 Hz frame (the
//! game's frame-time scale of 1). Camera collision uses the map's camera
//! collision mesh: the eye is pulled to the first hit on the segment from the
//! look-at point, 5 units off the surface.

pub type Vec3 = [f32; 3];

/// One row of the camera mode table.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Mode {
    /// Horizontal field of view in radians across a 640-unit-wide screen.
    pub fov: f32,
    pub radius: f32,
    pub lock_radius: f32,
    pub radius_min: f32,
    pub radius_max: f32,
    /// Eye height grows with the square of (radius - radius_min) times this.
    pub up_curve: f32,
}

/// Mode 0 (default/outdoor) row as stored in the executable's table.
pub const OUTDOOR: Mode = Mode {
    fov: 1.5,
    radius: 420.,
    lock_radius: 380.,
    radius_min: 250.,
    radius_max: 500.,
    up_curve: 0.008,
};

/// Look-at point height above the player's position.
pub const LOOK_AT_HEIGHT: f32 = 170.;
/// Eye target sits this far below the look-at point before the radius curve.
pub const EYE_DROP: f32 = 150.;
/// Yaw easing toward the yaw target, radians per frame.
pub const YAW_EASE: f32 = 0.174_532_9;
/// Behind-the-player follow speed, radians per frame, and its per-unit
/// change with the player's speed parameter (relative to 8).
pub const FOLLOW_SPEED: f32 = 0.004_363_32;
pub const FOLLOW_SPEED_PER_UNIT: f32 = 0.000_174_533;
pub const SPEED_REFERENCE: f32 = 8.;
/// Stick "toward the camera" component at or above which follow pauses.
pub const FOLLOW_STICK_LIMIT: f32 = 0.9;
/// Radius return toward the mode radius, units per frame.
pub const RADIUS_RETURN: f32 = 0.5;
/// Manual rotation, radians per frame at full input (default setting).
pub const MANUAL_TURN: f32 = 0.052_359_88;
/// Manual zoom, units per frame at full input.
pub const MANUAL_ZOOM: f32 = 8.;
/// Spring gains (default camera setting) and their per-unit speed change.
pub const SPRING_GAIN: f32 = 0.07;
pub const SPRING_GAIN_PER_UNIT: f32 = 0.0006;
pub const LOOK_AT_EXTRA_GAIN: f32 = 0.03;
pub const SPRING: f32 = 2.5;
pub const DAMPING: f32 = 0.5;
pub const REST_LENGTH: f32 = 1.;
/// Distance the eye is kept off a camera collision surface.
pub const COLLISION_OFFSET: f32 = 5.;

/// Lock-on look point: the fraction of the way from the player's upper body
/// toward the target's camera anchor (0x140623a28).
pub const LOCK_LOOK_BLEND: f32 = 0.3;
/// Height of the player's head point used for the off-screen test and the
/// far-and-above test (0x1405b22e8, Y down).
pub const HEAD_HEIGHT: f32 = 100.;
/// The look point stays on the player when the head is farther than this from
/// it (0x140623bf8) ...
pub const FAR_LOOK_DISTANCE: f32 = 300.;
/// ... and its direction points down more steeply than this (0x140623a4c).
pub const FAR_LOOK_SLOPE: f32 = 0.5;
/// Radius lost per frame for each tested point below the screen (0x140623b18).
pub const EDGE_ZOOM: f32 = 2.;
/// Yaw bias when the target leaves the left or right edge: 20 degrees
/// (0x1405b22ec / 0x1405b22f0).
pub const EDGE_YAW: f32 = 0.349_065_85;
/// The screen test's layout-unit bounds: x, y (0x14071a478) and depth
/// (near 1, far 4,000,000 as truncated from 0x1405b2bf0 / 0x1405b2bf4).
pub const SCREEN_X: [f32; 2] = [32., 480.];
pub const SCREEN_Y: [f32; 2] = [8., 408.];
pub const SCREEN_DEPTH: [f32; 2] = [1., 4_000_000.];
/// Screen test flags (FUN_140176710).
pub const OFF_TOP: u32 = 1;
pub const OFF_BOTTOM: u32 = 2;
pub const OFF_LEFT: u32 = 4;
pub const OFF_RIGHT: u32 = 8;
pub const OFF_NEAR: u32 = 0x10;
pub const OFF_FAR: u32 = 0x20;

/// First person: how far ahead of the head the look point sits (`0x1405b1da8`
/// as the vector (0, 0, 350)).
pub const FIRST_PERSON_REACH: f32 = 350.;
/// First person: the pitch's limit, 70 degrees (`DAT_140718bf8` / `bfc`).
pub const FIRST_PERSON_PITCH_LIMIT: f32 = 1.221_730_5;
/// Frames Sora takes to fade out on entering first person and back in on
/// leaving it (`DAT_140623bcc`).
pub const FIRST_PERSON_FADE_FRAMES: f32 = 30.;

/// A camera segment hit: the point, the polygon's unit normal facing the
/// segment's start and its surface nibble (5 marks the re-cast surface type).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct RayHit {
    pub point: Vec3,
    pub normal: Vec3,
    pub surface: u32,
}

/// First hit on the segment `from` -> `to` (see `triangle_hit`, or the COCT
/// query for a map's camera collision).
pub type Cast<'a> = &'a dyn Fn(Vec3, Vec3) -> Option<RayHit>;

fn cross(a: Vec3, b: Vec3) -> Vec3 {
    [
        a[1] * b[2] - a[2] * b[1],
        a[2] * b[0] - a[0] * b[2],
        a[0] * b[1] - a[1] * b[0],
    ]
}

/// Nearest intersection of a segment with a triangle soup.
pub fn triangle_hit(triangles: &[[Vec3; 3]], from: Vec3, to: Vec3) -> Option<RayHit> {
    let dir = sub(to, from);
    let mut best: Option<(f32, Vec3)> = None;
    for [a, b, c] in triangles {
        let (e1, e2) = (sub(*b, *a), sub(*c, *a));
        let p = cross(dir, e2);
        let det = dot(e1, p);
        if det.abs() < 1e-8 {
            continue;
        }
        let inv = 1. / det;
        let t0 = sub(from, *a);
        let u = dot(t0, p) * inv;
        if !(0. ..=1.).contains(&u) {
            continue;
        }
        let q = cross(t0, e1);
        let v = dot(dir, q) * inv;
        if v < 0. || u + v > 1. {
            continue;
        }
        let t = dot(e2, q) * inv;
        if (0. ..=1.).contains(&t) && best.is_none_or(|(bt, _)| t < bt) {
            best = Some((t, cross(e1, e2)));
        }
    }
    let (t, n) = best?;
    let len = dot(n, n).sqrt();
    let mut n = n.map(|c| c / len);
    if dot(n, dir) > 0. {
        n = n.map(|c| -c);
    }
    Some(RayHit {
        point: [0, 1, 2].map(|k| from[k] + dir[k] * t),
        normal: n,
        surface: 0,
    })
}

/// FUN_140164940 with the default push-off: the segment `from` -> `to` is cast
/// against the camera collision; on a hit the end moves to the hit point plus
/// the polygon normal times 5. A surface-5 polygon re-casts from `from` to the
/// end's horizontal position at the hit's height (the end offset by the
/// previous target `prev`), and the re-cast point replaces the hit. Returns
/// the clipped end.
pub fn clip(prev: Vec3, from: Vec3, to: Vec3, cast: Cast) -> Option<Vec3> {
    let hit = cast(from, to)?;
    let mut end = hit.point;
    if hit.surface == 5 {
        let delta = sub(to, prev);
        let probe = [delta[0] + prev[0], hit.point[1], delta[2] + prev[2]];
        end = cast(from, probe).map_or(probe, |second| second.point);
    }
    Some([0, 1, 2].map(|k| end[k] + hit.normal[k] * COLLISION_OFFSET))
}

/// Per-frame inputs. Stick axes follow the pad convention: x right, y
/// toward the player (down on the stick); `magnitude` is the stick's
/// analog strength.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct CameraInput {
    /// Native mode-2 camera anchor, in Y-down world coordinates.
    pub lock_target: Option<Vec3>,
    pub player_position: Vec3,
    /// Player facing as a horizontal direction in world space.
    pub player_forward: Vec3,
    /// The player's horizontal position changed this frame.
    pub moved: bool,
    pub stick: [f32; 2],
    pub magnitude: f32,
    /// Manual rotation (-1..1, positive increases yaw).
    pub turn: f32,
    /// Manual zoom (-1..1, positive moves away).
    pub zoom: f32,
    /// Swing directly behind the player and restore the mode radius.
    pub recenter: bool,
    /// The player's speed parameter (PRTY run speed in this test room).
    pub speed: f32,
    /// Select was pressed this frame (logical pad bit 22).
    pub select: bool,
    /// R3 was pressed this frame (logical pad bit 17): first person levels
    /// its pitch.
    pub reset_pitch: bool,
    /// The right stick's vertical deflection times its magnitude (pad
    /// `+0x24` x `+0x2c`, up is negative), zero unless a camera up/down bit
    /// (0xc0000) is held.
    pub look: f32,
    /// FUN_1403ba300: the player is in a state that allows first person (a
    /// physics mode with flag bit 7: ground, air; not Quick Run or a glide).
    pub first_person_ok: bool,
    /// The head's height above the player's root (PRTY word 12).
    pub head_lift: f32,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct FollowCamera {
    pub mode: Mode,
    pub yaw: f32,
    pub yaw_target: f32,
    pub radius: f32,
    pub eye: Vec3,
    pub look_at: Vec3,
    eye_target: Vec3,
    eye_target_prev: Vec3,
    look_at_target: Vec3,
    look_at_target_prev: Vec3,
    /// Camera mode 1: the eye sits at Sora's head and looks along his facing.
    pub first_person: bool,
    /// First person's look pitch (+0xc8), positive looks up.
    pub pitch: f32,
}

/// The executable's angle wrap into [-pi, pi].
pub fn wrap(a: f32) -> f32 {
    use std::f32::consts::{PI, TAU};
    if a > 0. {
        (a + PI) % TAU - PI
    } else {
        PI - (PI - a) % TAU
    }
}

/// Moves `from` toward `to` by at most `step` along the shorter arc.
fn approach(from: f32, to: f32, step: f32) -> f32 {
    let d = wrap(to - from);
    wrap(from + d.clamp(-step, step))
}

fn sub(a: Vec3, b: Vec3) -> Vec3 {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}

fn normalize_or(a: Vec3, fallback: Vec3) -> Vec3 {
    let length = dot(a, a).sqrt();
    if length > 0. && length.is_finite() {
        a.map(|c| c / length)
    } else {
        fallback
    }
}

fn dot(a: Vec3, b: Vec3) -> f32 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}

/// Eases `current` toward `target`: a spring on the distance with damping
/// against the target's own motion (`target_prev` is last frame's target).
pub fn spring(current: Vec3, target: Vec3, target_prev: Vec3, gain: f32) -> Vec3 {
    spring_with(
        current,
        target,
        target_prev,
        gain,
        [SPRING, DAMPING, REST_LENGTH],
    )
}

/// FUN_140165bd0 with explicit constants `[spring, damping, rest length]`
/// (the camera uses 2.5, 0.5, 1; the HUD slot stack 1.2, 0.5, 0.1).
pub fn spring_with(
    current: Vec3,
    target: Vec3,
    target_prev: Vec3,
    gain: f32,
    [spring_k, damping, rest]: [f32; 3],
) -> Vec3 {
    let d = sub(current, target);
    let length = dot(d, d).sqrt();
    if length == 0. {
        return current;
    }
    let v = sub(target_prev, target).map(|c| c * gain);
    let f = dot(d, v);
    let scale = ((f / length) * damping + (rest - length) * spring_k) * gain / length;
    let out = [
        current[0] + d[0] * scale,
        current[1] + d[1] * scale,
        current[2] + d[2] * scale,
    ];
    out.map(|c| if c.is_nan() { 0. } else { c })
}

/// Yaw that places the eye behind a player facing `forward`.
pub fn behind(forward: Vec3) -> f32 {
    (-forward[0]).atan2(-forward[2])
}

impl FollowCamera {
    /// A camera snapped behind the player at the mode radius.
    pub fn new(mode: Mode, input: &CameraInput) -> Self {
        let yaw = behind(input.player_forward);
        let mut camera = Self {
            mode,
            yaw,
            yaw_target: yaw,
            radius: mode.radius,
            eye: [0.; 3],
            look_at: [0.; 3],
            eye_target: [0.; 3],
            eye_target_prev: [0.; 3],
            look_at_target: [0.; 3],
            look_at_target_prev: [0.; 3],
            first_person: false,
            pitch: 0.,
        };
        camera.place_targets(input.player_position, true);
        camera
    }

    /// Vertical field of view for a 640x480 screen at the mode's horizontal
    /// field of view.
    pub fn vertical_fov(&self) -> f32 {
        let focal = 320. / (self.mode.fov * 0.5).tan();
        2. * (240. / focal).atan()
    }

    fn place_targets(&mut self, player: Vec3, snap: bool) {
        self.look_at_target = [player[0], player[1] - LOOK_AT_HEIGHT, player[2]];
        self.yaw = if snap {
            self.yaw_target
        } else {
            approach(self.yaw, self.yaw_target, YAW_EASE)
        };
        self.place_eye_target(self.look_at_target);
        if snap {
            self.eye = self.eye_target;
            self.look_at = self.look_at_target;
            self.eye_target_prev = self.eye_target;
            self.look_at_target_prev = self.look_at_target;
        }
    }

    /// FUN_140164520's eye target: the radius back along the yaw from the
    /// anchor, 150 below it less the radius curve.
    fn place_eye_target(&mut self, at: Vec3) {
        let d = self.radius - self.mode.radius_min;
        let (s, c) = self.yaw.sin_cos();
        self.eye_target = [
            at[0] + self.radius * s,
            at[1] + EYE_DROP - d * d * self.mode.up_curve,
            at[2] + self.radius * c,
        ];
    }

    /// A world point on the 512x416 layout screen and its depth, from the
    /// camera as last applied (eye, look-at, field of view): the view and
    /// projection matrices (FUN_14019cfc0 / FUN_14019ce40) then FUN_140166a20,
    /// which divides X and Y by depth and re-centres them. The focal scales
    /// follow the 640x480 virtual screen: 0.8 wide, 0.8667 high.
    pub fn project(&self, point: Vec3) -> Vec3 {
        let forward = normalize_or(sub(self.look_at, self.eye), [0., 0., 1.]);
        let down = normalize_or(
            sub([0., 1., 0.], forward.map(|c| c * forward[1])),
            [0., 1., 0.],
        );
        let right = cross(down, forward);
        let offset = sub(point, self.eye);
        let (x, y, z) = (dot(offset, right), dot(offset, down), dot(offset, forward));
        let focal = 320. / (self.mode.fov * 0.5).tan();
        let (x, y) = if z != 0. {
            (x * (1. / z), y * (1. / z))
        } else {
            (x, y)
        };
        [x * focal * 0.8 + 256., y * focal * 0.866_666_7 + 208., z]
    }

    /// FUN_140176710: which screen edges a world point is beyond.
    pub fn screen_flags(&self, point: Vec3) -> u32 {
        let [x, y, z] = self.project(point);
        let mut flags = 0;
        if x < SCREEN_X[0] {
            flags |= OFF_LEFT;
        }
        if SCREEN_X[1] < x {
            flags |= OFF_RIGHT;
        }
        if y < SCREEN_Y[0] {
            flags |= OFF_TOP;
        }
        if SCREEN_Y[1] < y {
            flags |= OFF_BOTTOM;
        }
        if z < SCREEN_DEPTH[0] {
            flags |= OFF_NEAR;
        }
        if SCREEN_DEPTH[1] < z {
            flags |= OFF_FAR;
        }
        flags
    }

    /// FUN_140164290: the radius moves toward the mode's lock radius by one
    /// frame's step, within the radius limits.
    fn return_to_lock_radius(&mut self) {
        let mode = self.mode;
        if (self.radius - mode.lock_radius).abs() >= 1. {
            if self.radius <= mode.lock_radius {
                self.radius += 1.;
                if self.radius >= mode.radius_max {
                    self.radius = mode.radius_max;
                }
            } else {
                self.radius -= 1.;
                if self.radius <= mode.radius_min {
                    self.radius = mode.radius_min;
                }
            }
        }
    }

    /// FUN_140166430 / FUN_140166450: clamp to the mode's radius limits.
    fn clamp_radius(&mut self) {
        if self.radius <= self.mode.radius_min {
            self.radius = self.mode.radius_min;
        }
        if self.mode.radius_max <= self.radius {
            self.radius = self.mode.radius_max;
        }
    }

    /// One frame. `cast` returns the first camera-collision hit on a segment
    /// (see `triangle_hit`).
    pub fn tick(&mut self, input: &CameraInput, cast: Cast) {
        self.look_at_target_prev = self.look_at_target;
        self.eye_target_prev = self.eye_target;
        let speed_offset = input.speed - SPEED_REFERENCE;
        // At 60 Hz the gain is halved; the look-at point adds its extra gain
        // afterwards.
        let gain = (SPRING_GAIN + speed_offset * SPRING_GAIN_PER_UNIT) * 0.5;
        if self.first_person {
            if input.lock_target.is_none() {
                self.tick_first_person(input, gain, cast);
                return;
            }
            // FUN_140164870: another camera mode drops first person.
            self.first_person = false;
        }
        if let Some(anchor) = input.lock_target {
            self.tick_lock(input, anchor, cast);
            let feet_up = [
                input.player_position[0],
                input.player_position[1] - LOOK_AT_HEIGHT,
                input.player_position[2],
            ];
            self.spring_to_targets(gain);
            // FUN_140176e40: mode 2 clips only the eye, from the player's
            // upper body, and does not feed the clip back to the target.
            if let Some(eye) = clip(self.eye_target_prev, feet_up, self.eye, cast) {
                self.eye = eye;
            }
            return;
        }
        let mode = self.mode;
        // FUN_140165110: Select, while the player can look around, enters mode
        // 1 and skips the follow and manual turns for this frame; the first
        // person update starts on the next.
        let entering = input.select && input.first_person_ok;
        let masked = CameraInput {
            turn: 0.,
            zoom: 0.,
            recenter: false,
            moved: false,
            ..*input
        };
        let input = if entering { &masked } else { input };
        let mut snap = false;
        if input.turn != 0. {
            self.yaw = wrap(self.yaw + input.turn * MANUAL_TURN);
            self.yaw_target = self.yaw;
        } else if input.recenter {
            self.radius = mode.radius;
            self.yaw_target = behind(input.player_forward);
            self.eye = self.eye_target;
            snap = input.magnitude != 0.;
        }
        if input.zoom > 0. {
            self.radius = (self.radius + input.zoom * MANUAL_ZOOM).min(mode.radius_max);
        } else if input.zoom < 0. {
            self.radius = (self.radius + input.zoom * MANUAL_ZOOM).max(mode.radius_min);
        }
        if !snap
            && input.moved
            && input.magnitude != 0.
            && input.turn == 0.
            && input.stick[1] < FOLLOW_STICK_LIMIT
        {
            let step = 2.
                * (input.stick[0].abs() + 1.)
                * (input.stick[1].abs() + 1.)
                * input.magnitude
                * (speed_offset * FOLLOW_SPEED_PER_UNIT + FOLLOW_SPEED);
            self.yaw = approach(self.yaw, behind(input.player_forward), step);
            self.yaw_target = self.yaw;
            if (self.radius - mode.radius).abs() >= 1. {
                self.radius = if self.radius <= mode.radius {
                    (self.radius + RADIUS_RETURN).min(mode.radius_max)
                } else {
                    (self.radius - RADIUS_RETURN).max(mode.radius_min)
                };
            }
        }
        self.place_targets(input.player_position, snap);
        if let Some(end) = clip(
            self.eye_target_prev,
            self.look_at_target,
            self.eye_target,
            cast,
        ) {
            self.eye_target = end;
        }
        self.spring_to_targets(gain);
        if let Some(eye) = clip(self.eye_target_prev, self.look_at, self.eye, cast) {
            self.eye = eye;
            self.eye_target = eye;
        }
        if entering {
            self.first_person = true;
            self.pitch = 0.;
        }
    }

    /// Mode 1 (FUN_140165e30): the eye target is the head, the look target 350
    /// units ahead of it along the player's facing, tilted by the pitch; the
    /// right stick tilts it (clamped to 70 degrees) and R3 levels it. Select,
    /// or a state that no longer allows it, leaves: the yaw snaps behind the
    /// player and the field camera's targets return (FUN_140164520), eased by
    /// the spring.
    fn tick_first_person(&mut self, input: &CameraInput, gain: f32, cast: Cast) {
        if !input.first_person_ok || input.select {
            self.first_person = false;
            self.yaw_target = behind(input.player_forward);
            self.yaw = self.yaw_target;
            self.place_targets(input.player_position, false);
            self.spring_to_targets(gain);
            if let Some(eye) = clip(self.eye_target_prev, self.look_at, self.eye, cast) {
                self.eye = eye;
                self.eye_target = eye;
            }
            return;
        }
        use std::f32::consts::PI;
        if input.reset_pitch {
            self.pitch = 0.;
        }
        let player = input.player_position;
        let head = [player[0], player[1] - input.head_lift, player[2]];
        self.eye_target = head;
        // The yaw base is the player's facing: behind him, turned half way
        // round (+0xc4 = +0x5c + pi).
        self.yaw = behind(input.player_forward);
        self.yaw_target = self.yaw;
        let facing = self.yaw + PI;
        self.pitch = wrap(self.pitch - input.look * MANUAL_TURN);
        self.pitch = self
            .pitch
            .clamp(-FIRST_PERSON_PITCH_LIMIT, FIRST_PERSON_PITCH_LIMIT);
        // (0, 0, 350) turned by the pitch about X then the facing about Y.
        let (sp, cp) = self.pitch.sin_cos();
        let (sf, cf) = facing.sin_cos();
        let ahead = [
            FIRST_PERSON_REACH * cp * sf,
            -FIRST_PERSON_REACH * sp,
            FIRST_PERSON_REACH * cp * cf,
        ];
        self.look_at_target = [0, 1, 2].map(|k| head[k] + ahead[k]);
        self.spring_to_targets(gain);
        // FUN_140165700 case 1: the look point stops at the first surface on
        // the segment from it to the eye.
        if let Some(hit) = cast(self.look_at, self.eye) {
            self.look_at = hit.point;
        }
    }

    /// FUN_140165700: the current look-at and eye ease toward their targets.
    fn spring_to_targets(&mut self, gain: f32) {
        self.look_at = spring(
            self.look_at,
            self.look_at_target,
            self.look_at_target_prev,
            gain + LOOK_AT_EXTRA_GAIN,
        );
        self.eye = spring(self.eye, self.eye_target, self.eye_target_prev, gain);
    }

    /// Mode 2 (FUN_140176c70): the look point blends toward the locked
    /// target's camera anchor, the radius and yaw react to the target and the
    /// player leaving the screen, and the eye target stays on the player.
    fn tick_lock(&mut self, input: &CameraInput, anchor: Vec3, cast: Cast) {
        let player = input.player_position;
        let feet_up = [player[0], player[1] - LOOK_AT_HEIGHT, player[2]];
        // FUN_140176ec0 with k = 0.3.
        let old_look_target = self.look_at_target;
        let toward = sub(anchor, feet_up).map(|c| LOCK_LOOK_BLEND * c);
        let look = [0, 1, 2].map(|i| toward[i] + feet_up[i]);
        let head = [player[0], player[1] - HEAD_HEIGHT, player[2]];
        let target_flags = self.screen_flags(anchor);
        let head_flags = self.screen_flags(head);
        let target_low = target_flags & (OFF_TOP | OFF_BOTTOM) == OFF_BOTTOM;
        let head_low = head_flags & (OFF_TOP | OFF_BOTTOM) == OFF_BOTTOM;
        if target_low {
            self.radius -= EDGE_ZOOM;
        }
        if head_low {
            self.radius -= EDGE_ZOOM;
        }
        self.clamp_radius();
        let to_head = sub(head, look);
        let distance = dot(to_head, to_head).sqrt();
        let slope = if distance != 0. {
            to_head[1] * (1. / distance)
        } else {
            to_head[1]
        };
        // The bearing from the old look target to the player (FUN_140176a70).
        let bearing = (player[0] - old_look_target[0]).atan2(player[2] - old_look_target[2]);
        if distance > FAR_LOOK_DISTANCE && slope > FAR_LOOK_SLOPE {
            self.look_at_target = feet_up;
        } else {
            if !target_low && !head_low {
                self.return_to_lock_radius();
            }
            self.look_at_target = look;
            let flags = if target_flags != 0 {
                target_flags
            } else {
                head_flags
            };
            if flags != 0 {
                let edge = if flags & OFF_LEFT != 0 {
                    EDGE_YAW
                } else if flags & OFF_RIGHT != 0 {
                    -EDGE_YAW
                } else {
                    0.
                };
                self.yaw_target = wrap(edge + bearing);
            }
        }
        if input.turn != 0. {
            self.yaw = wrap(self.yaw + MANUAL_TURN * input.turn);
            self.yaw_target = self.yaw;
        }
        // FUN_140176bc0: zoom with the right stick, one step per frame.
        if input.zoom < 0. {
            self.radius += input.zoom * MANUAL_ZOOM;
            if self.radius <= self.mode.radius_min {
                self.radius = self.mode.radius_min;
            }
        } else if input.zoom > 0. {
            self.radius += input.zoom * MANUAL_ZOOM;
            if self.mode.radius_max <= self.radius {
                self.radius = self.mode.radius_max;
            }
        }
        if input.recenter {
            self.yaw_target = bearing;
            self.radius = self.mode.lock_radius;
        }
        // FUN_140164520 with the player's upper body as the anchor, then the
        // eye-target collision from that point (FUN_140164910).
        let look_target = self.look_at_target;
        self.yaw = approach(self.yaw, self.yaw_target, YAW_EASE);
        self.place_eye_target(feet_up);
        if let Some(end) = clip(self.eye_target_prev, feet_up, self.eye_target, cast) {
            self.eye_target = end;
        }
        self.look_at_target = look_target;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn input(position: Vec3, forward: Vec3) -> CameraInput {
        CameraInput {
            player_position: position,
            player_forward: forward,
            speed: 8.,
            ..Default::default()
        }
    }

    fn locked(position: Vec3, forward: Vec3, anchor: Vec3) -> (FollowCamera, CameraInput) {
        let mut step = input(position, forward);
        let camera = FollowCamera::new(OUTDOOR, &step);
        step.lock_target = Some(anchor);
        (camera, step)
    }

    #[test]
    fn lock_looks_between_actors_but_keeps_the_eye_on_the_player_anchor() {
        let (mut camera, step) = locked([0.; 3], [0., 0., 1.], [100., -50., 200.]);
        camera.tick(&step, &|_, _| None);
        // On screen: the radius returns toward the lock radius one unit a frame.
        assert_eq!(camera.radius, 419.);
        for (got, want) in camera.look_at_target.into_iter().zip([30., -134., 60.]) {
            assert!((got - want).abs() < 1e-3);
        }
        assert!(camera.eye_target[0].abs() < 0.001);
        assert!((camera.eye_target[2] + 419.).abs() < 0.001);
        for _ in 0..60 {
            camera.tick(&step, &|_, _| None);
        }
        assert_eq!(camera.radius, 380.);
    }

    #[test]
    fn a_distant_target_far_above_the_player_keeps_the_player_look_anchor() {
        // Head to look point farther than 300 and pointing down: the target is
        // high above, so the look point stays on the player's upper body.
        let (mut camera, step) = locked([0.; 3], [0., 0., 1.], [0., -1500., 100.]);
        camera.tick(&step, &|_, _| None);
        assert_eq!(camera.look_at_target, [0., -170., 0.]);
        // A target far below does not trigger the rule.
        let (mut camera, step) = locked([0.; 3], [0., 0., 1.], [0., 600., 100.]);
        camera.tick(&step, &|_, _| None);
        assert_ne!(camera.look_at_target, [0., -170., 0.]);
    }

    fn first_person_input(forward: Vec3) -> CameraInput {
        CameraInput {
            first_person_ok: true,
            head_lift: 150.,
            ..input([10., 0., 20.], forward)
        }
    }

    #[test]
    fn select_enters_first_person_only_where_the_player_may_look_around() {
        let mut step = first_person_input([0., 0., 1.]);
        let mut camera = FollowCamera::new(OUTDOOR, &step);
        step.select = true;
        step.first_person_ok = false;
        camera.tick(&step, &|_, _| None);
        assert!(!camera.first_person);
        step.first_person_ok = true;
        camera.tick(&step, &|_, _| None);
        assert!(camera.first_person);
        assert_eq!(camera.pitch, 0.);
    }

    #[test]
    fn first_person_puts_the_eye_target_at_the_head_and_looks_along_the_facing() {
        let mut step = first_person_input([0., 0., 1.]);
        let mut camera = FollowCamera::new(OUTDOOR, &step);
        step.select = true;
        camera.tick(&step, &|_, _| None);
        step.select = false;
        for _ in 0..400 {
            camera.tick(&step, &|_, _| None);
        }
        // The eye settles at the head (the spring rests one unit from its
        // target), the look point 350 ahead of it.
        assert!(camera.first_person);
        for (got, want) in camera.eye.into_iter().zip([10., -150., 20.]) {
            assert!((got - want).abs() < 1.5, "{:?}", camera.eye);
        }
        // The player faces +Z in the camera's space, so the look point is
        // 350 further along it.
        for (got, want) in camera.look_at.into_iter().zip([10., -150., 370.]) {
            assert!((got - want).abs() < 1.5, "{:?}", camera.look_at);
        }
    }

    #[test]
    fn first_person_pitch_follows_the_stick_and_stops_at_seventy_degrees() {
        let mut step = first_person_input([1., 0., 0.]);
        let mut camera = FollowCamera::new(OUTDOOR, &step);
        step.select = true;
        camera.tick(&step, &|_, _| None);
        step.select = false;
        // Stick up (negative) tilts up: positive pitch, the look target rises
        // (negative Y).
        step.look = -1.;
        camera.tick(&step, &|_, _| None);
        assert!((camera.pitch - MANUAL_TURN).abs() < 1e-6);
        assert!(camera.look_at_target[1] < -150.);
        for _ in 0..100 {
            camera.tick(&step, &|_, _| None);
        }
        assert!((camera.pitch - FIRST_PERSON_PITCH_LIMIT).abs() < 1e-6);
        // R3 levels it again.
        step.look = 0.;
        step.reset_pitch = true;
        camera.tick(&step, &|_, _| None);
        assert_eq!(camera.pitch, 0.);
        assert!((camera.look_at_target[1] + 150.).abs() < 1e-3);
    }

    #[test]
    fn first_person_look_point_stops_at_a_wall_and_select_leaves() {
        let mut step = first_person_input([0., 0., 1.]);
        let mut camera = FollowCamera::new(OUTDOOR, &step);
        step.select = true;
        camera.tick(&step, &|_, _| None);
        step.select = false;
        let wall = |_: Vec3, _: Vec3| {
            Some(RayHit {
                point: [10., -150., 100.],
                normal: [0., 0., -1.],
                surface: 0,
            })
        };
        camera.tick(&step, &wall);
        assert_eq!(camera.look_at, [10., -150., 100.]);
        // Select, or a state that forbids it, leaves and puts the follow
        // camera's yaw behind the player.
        step.select = true;
        camera.tick(&step, &|_, _| None);
        assert!(!camera.first_person);
        assert!(wrap(camera.yaw - behind([0., 0., 1.])).abs() < 1e-5);
        let mut camera = FollowCamera::new(OUTDOOR, &step);
        camera.first_person = true;
        step.select = false;
        step.first_person_ok = false;
        camera.tick(&step, &|_, _| None);
        assert!(!camera.first_person);
    }

    #[test]
    fn projection_matches_the_virtual_screen() {
        let camera = FollowCamera::new(OUTDOOR, &input([0.; 3], [0., 0., 1.]));
        // The look-at point is on the optical axis: the layout centre.
        let [x, y, z] = camera.project(camera.look_at);
        assert!((x - 256.).abs() < 1e-2 && (y - 208.).abs() < 1e-2);
        assert!(z > 0.);
        // 1.5 radians span the 512-wide layout: the edge is at tan(0.75).
        let forward = normalize_or(sub(camera.look_at, camera.eye), [0., 0., 1.]);
        let down = normalize_or(
            sub([0., 1., 0.], forward.map(|c| c * forward[1])),
            [0., 1., 0.],
        );
        let right = cross(down, forward);
        let edge = camera.project(
            [0, 1, 2].map(|i| camera.eye[i] + forward[i] * 500. + right[i] * 500. * 0.931_596_3),
        );
        assert!((edge[0] - 512.).abs() < 0.1, "{edge:?}");
        assert!((edge[1] - 208.).abs() < 0.1, "{edge:?}");
        assert_eq!(camera.screen_flags(camera.look_at), 0);
        assert_eq!(camera.screen_flags([5000., -170., 0.]), OFF_RIGHT);
        assert_eq!(camera.screen_flags([-5000., -170., 0.]), OFF_LEFT);
        assert_eq!(
            camera.screen_flags([0., -170., -2000.]) & OFF_NEAR,
            OFF_NEAR
        );
    }

    #[test]
    fn a_target_beyond_a_side_edge_swings_the_yaw_target_with_a_bias() {
        let (mut camera, step) = locked([0.; 3], [0., 0., 1.], [3000., -100., 0.]);
        let flags = camera.screen_flags([3000., -100., 0.]);
        assert_eq!(flags, OFF_RIGHT);
        let look_before = camera.look_at_target;
        camera.tick(&step, &|_, _| None);
        // Bearing from the old look target to the player, 20 degrees off.
        let bearing = (0. - look_before[0]).atan2(0. - look_before[2]);
        assert!((camera.yaw_target - wrap(bearing - EDGE_YAW)).abs() < 1e-5);
        // The yaw follows by at most 0.1745 a frame.
        assert!(
            (wrap(camera.yaw - camera.yaw_target)).abs() >= 0. && camera.yaw != camera.yaw_target
        );
    }

    #[test]
    fn a_target_below_the_screen_zooms_in_two_units_a_frame_down_to_the_minimum() {
        // Straight below the optical axis, near the player: off the bottom.
        let (mut camera, step) = locked([0.; 3], [0., 0., 1.], [0., 3000., -300.]);
        assert_eq!(camera.screen_flags([0., 3000., -300.]) & 3, OFF_BOTTOM);
        camera.tick(&step, &|_, _| None);
        assert_eq!(camera.radius, 418.);
        for _ in 0..200 {
            camera.tick(&step, &|_, _| None);
            assert!((OUTDOOR.radius_min..=OUTDOOR.radius_max).contains(&camera.radius));
        }
    }

    #[test]
    fn lock_accepts_manual_turn_zoom_and_recenter() {
        let (mut camera, mut step) = locked([0.; 3], [0., 0., 1.], [0., -50., 300.]);
        step.turn = 1.;
        let yaw = camera.yaw;
        camera.tick(&step, &|_, _| None);
        assert!((camera.yaw - wrap(yaw + MANUAL_TURN)).abs() < 1e-6);
        assert_eq!(camera.yaw_target, camera.yaw);
        step.turn = 0.;
        step.zoom = -1.;
        camera.radius = 420.;
        camera.tick(&step, &|_, _| None);
        assert_eq!(camera.radius, 420. - 1. - MANUAL_ZOOM);
        step.zoom = 0.;
        step.recenter = true;
        camera.tick(&step, &|_, _| None);
        assert_eq!(camera.radius, 380.);
    }

    #[test]
    fn lock_clips_the_eye_from_the_players_upper_body_without_moving_its_target() {
        let wall = [[
            [-1000., -1000., -100.],
            [1000., -1000., -100.],
            [0., 1000., -100.],
        ]];
        let (mut camera, step) = locked([0.; 3], [0., 0., 1.], [0., -50., 400.]);
        camera.tick(&step, &|a, b| triangle_hit(&wall, a, b));
        assert!((camera.eye[2] - (-95.)).abs() < 1e-3);
        // The eye target was clipped from the same point, 5 units off the
        // wall; the post-spring clip never rewrites it.
        assert!((camera.eye_target[2] - (-95.)).abs() < 1e-3);
        camera.eye_target[2] = -300.;
        camera.tick(&step, &|_, _| None);
        assert!((camera.eye_target[2] + 419.).abs() < 1.5);
    }

    #[test]
    fn snapped_camera_sits_behind_and_above() {
        let camera = FollowCamera::new(OUTDOOR, &input([0., 0., 0.], [0., 0., 1.]));
        // Facing +Z puts the eye at -Z, 420 back; Y is down, so above is
        // negative: look-at 170 up, eye 20 + 170^2 * 0.008 up.
        assert!((camera.eye[2] + 420.).abs() < 1e-3);
        assert!(camera.eye[0].abs() < 1e-3);
        assert!((camera.look_at[1] + 170.).abs() < 1e-3);
        assert!((camera.eye[1] + (20. + 170. * 170. * 0.008)).abs() < 1e-2);
        assert!((camera.vertical_fov() - 1.2197).abs() < 1e-3);
    }

    #[test]
    fn wrap_matches_the_executable_branches() {
        use std::f32::consts::PI;
        assert!((wrap(PI + 0.5) - (-PI + 0.5)).abs() < 1e-5);
        assert!((wrap(-PI - 0.5) - (PI - 0.5)).abs() < 1e-5);
        assert_eq!(wrap(0.25), 0.25);
        assert!((approach(3.0, -3.0, 0.1) - 3.1).abs() < 1e-5);
    }

    #[test]
    fn spring_eases_toward_a_still_target() {
        let out = spring([100., 0., 0.], [0., 0., 0.], [0., 0., 0.], 0.035);
        // (1 - 100) * 2.5 * 0.035 = -8.6625 along the offset.
        assert!((out[0] - (100. - 8.6625)).abs() < 1e-3);
        assert_eq!(
            spring([1., 2., 3.], [1., 2., 3.], [0.; 3], 0.5),
            [1., 2., 3.]
        );
    }

    #[test]
    fn a_surface_five_hit_is_recast_at_the_hit_height_toward_the_clipped_end() {
        let second = std::cell::Cell::new(None);
        let cast = |from: Vec3, to: Vec3| {
            if to[1] == -50. {
                Some(RayHit {
                    point: [0., -100., -50.],
                    normal: [0., 0., 1.],
                    surface: 5,
                })
            } else {
                second.set(Some((from, to)));
                Some(RayHit {
                    point: [3., -100., -80.],
                    normal: [1., 0., 0.],
                    surface: 0,
                })
            }
        };
        // Previous target (1, -40, 2) and new end (10, -50, -300).
        let end = clip([1., -40., 2.], [0., -170., 0.], [10., -50., -300.], &cast).unwrap();
        // The re-cast runs from the origin to the end's X and Z at the hit's Y.
        assert_eq!(second.get(), Some(([0., -170., 0.], [10., -100., -300.])));
        // The re-cast point replaces the hit; the first polygon's normal pushes it.
        assert_eq!(end, [3. + 0., -100., -80. + 5.]);
        // Other surfaces use the first hit directly.
        let plain = |_: Vec3, _: Vec3| {
            Some(RayHit {
                point: [0., -10., -50.],
                normal: [0., 0., 1.],
                surface: 3,
            })
        };
        assert_eq!(
            clip([0.; 3], [0.; 3], [0., 0., -300.], &plain),
            Some([0., -10., -45.])
        );
    }

    #[test]
    fn wall_between_look_at_and_eye_pulls_the_eye_in() {
        // A wall at z = -100 across the path behind a player facing +Z.
        let wall = [[
            [-1000., -1000., -100.],
            [1000., -1000., -100.],
            [0., 1000., -100.],
        ]];
        let mut camera = FollowCamera::new(OUTDOOR, &input([0., 0., 0.], [0., 0., 1.]));
        camera.tick(&input([0., 0., 0.], [0., 0., 1.]), &|a, b| {
            triangle_hit(&wall, a, b)
        });
        assert!((camera.eye[2] - (-95.)).abs() < 1e-3);
        assert!(triangle_hit(&wall, [0., 0., 0.], [0., 0., -50.]).is_none());
    }

    #[test]
    fn follow_swings_behind_a_moving_player_and_pauses_when_running_at_it() {
        let mut camera = FollowCamera::new(OUTDOOR, &input([0., 0., 0.], [0., 0., 1.]));
        let mut step = input([0., 0., 0.], [1., 0., 0.]);
        step.moved = true;
        step.magnitude = 1.;
        camera.tick(&step, &|_, _| None);
        // Behind +Z is -pi; behind +X is -pi/2. Full stick, no tilt turns
        // 2 * 1 * 1 * 1 * FOLLOW_SPEED per frame toward it.
        let expected = -std::f32::consts::PI + 2. * FOLLOW_SPEED;
        assert!((camera.yaw - expected).abs() < 1e-5);
        let before = camera.yaw;
        step.stick = [0., 1.];
        camera.tick(&step, &|_, _| None);
        assert_eq!(camera.yaw, before);
    }
}
