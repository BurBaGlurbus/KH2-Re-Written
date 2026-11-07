//! Shared actor physics translated from the KH2FM executable (Steam build
//! 15194255): the per-object movement parameters, the closed-form jump and
//! fall curve, air steering, the knock-slide and the blow flight.
//!
//! The executable works in Y-down space (a negative Y velocity rises). These
//! routines keep its arithmetic and take or return Y-up values at the edges,
//! so callers stay in the simulation's Y-up space. One tick is one 60 Hz
//! frame; `DT` is the frame time `DAT_140717480` (1.0 at normal speed).

/// Frame time per tick (`DAT_140717480`): FUN_140154740 sets it to the
/// vsyncs elapsed × game speed, 1.0 per 60 Hz frame.
pub const DT: f32 = 1.0;
/// FUN_1403b89a0 runs a physics mode's velocity routine only every
/// `PHYSICS_STEP` frames (`DAT_1405b18ec` = 2.0), passing it that span as
/// its time step, and spreads the result evenly over the frames until the
/// next run. A move request faster than `EVERY_FRAME_SPEED` (`DAT_140623be4`
/// = 100) runs it every frame.
pub const PHYSICS_STEP: f32 = 2.0;
pub const EVERY_FRAME_SPEED: f32 = 100.0;
/// Gravity, units per tick squared (`DAT_1405ce184` = 0x3ed0fac7).
pub const G: f32 = 0.408_163_28;
/// Air steering: the share of last tick's velocity kept while the stick is
/// held (`DAT_1405aaa40`, passed by FUN_1403cb360).
pub const AIR_INERTIA: f32 = 0.9;
/// Air drag with the stick released (`DAT_1405b1c74`, FUN_1403cb360).
pub const AIR_DRAG: f32 = 0.95;
/// Knock-slide: strength × 0.1 (`DAT_1406239f8`, FUN_1403d4960), decaying
/// ×0.9 per tick (`DAT_1405aaa40`) until below 1.0 (`DAT_140623a80`,
/// FUN_1403d4570).
pub const KNOCK_SCALE: f32 = 0.1;
pub const KNOCK_DECAY: f32 = 0.9;
pub const KNOCK_STOP: f32 = 1.0;
/// Motion blend for locomotion and the fall motion (`DAT_140623bac`,
/// FUN_140422520 / FUN_1403cb610), in motion-clock ticks: FUN_1403c6dc0
/// adds it to the motion time, which advances by DT per tick.
pub const MOTION_BLEND: f32 = 8.0;
/// The fall motion starts this many ticks before the apex
/// (`DAT_140623b98`, FUN_1403cb680).
pub const FALL_MOTION_LEAD: f32 = 5.0;
/// A released jump is cut only once the arc's time is past this
/// (`DAT_140623b9c`, FUN_1403cb680).
pub const JUMP_CUT_AFTER: f32 = 6.0;
/// The rise a cut jump is relaunched with (`DAT_142a243d8`, set at start-up
/// by FUN_1400259b0 to g × 0.5 × 5 × 5): a 5-tick rise of about 5.1 units.
pub const JUMP_CUT_HOP: f32 = G * 0.5 * FALL_MOTION_LEAD * FALL_MOTION_LEAD;

/// Default stick magnitude from which the run speed is used
/// (`DAT_140585c7c`, FUN_1403a8fa0).
pub const RUN_MAGNITUDE: f32 = 0.8;

/// The movement block at obj+0x128 (FUN_14040a440 defaults, overwritten by
/// FUN_14040a480 from the object's PRTY record).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ObjParams {
    /// +0x128, units per tick.
    pub walk: f32,
    /// +0x12c, units per tick.
    pub run: f32,
    /// +0x130, jump apex height.
    pub jump: f32,
    /// +0x134, radians per tick.
    pub turn: f32,
    /// +0x138, maximum downward speed per tick (SSTM FallMax).
    pub fall_max: f32,
    /// +0x13c (PRTY word 8).
    pub word8: f32,
    /// +0x140 (PRTY word 10); FUN_140422350 also reads it from the record
    /// (+0x28) as the inertia of range-42 movement.
    pub word10: f32,
    /// PRTY word 11 (record +0x2c): the drag of range-42 movement. Not part
    /// of the init block; 0 without a record.
    pub word11: f32,
    /// PRTY word 12 (record +0x30): the head's height above the root, which
    /// the first-person camera (FUN_1403b5c50) puts the eye at. 0 without a
    /// record.
    pub head: f32,
}

impl ObjParams {
    /// FUN_14040a440: walk 2, run 8, jump 160, turn π/16, fall max from
    /// SSTM +0x14, +0x140 = 0.95.
    pub fn defaults(sstm_fall_max: f32) -> Self {
        ObjParams {
            walk: f32::from_bits(0x4000_0000),
            run: f32::from_bits(0x4100_0000),
            jump: f32::from_bits(0x4320_0000),
            turn: f32::from_bits(0x3e49_0fdb),
            fall_max: sstm_fall_max,
            word8: 0.,
            word10: f32::from_bits(0x3f73_3333),
            word11: 0.,
            head: 0.,
        }
    }

    /// FUN_14040a480: PRTY words 0–3 to +0x128..+0x134, word 8 to +0x13c,
    /// word 10 to +0x140. +0x138 (fall max) is left alone.
    pub fn apply_prty(&mut self, words: &[u32; 13]) {
        self.walk = f32::from_bits(words[0]);
        self.run = f32::from_bits(words[1]);
        self.jump = f32::from_bits(words[2]);
        self.turn = f32::from_bits(words[3]);
        self.word8 = f32::from_bits(words[8]);
        self.word10 = f32::from_bits(words[10]);
        self.word11 = f32::from_bits(words[11]);
        self.head = f32::from_bits(words[12]);
    }
}

/// FUN_1403e9590: the record offset for a PRTY index. `offsets` is the raw
/// table after the count word; an index past the table or naming a zero
/// offset falls back to index 1. Returns None when that is zero too.
pub fn prty_offset(offsets: &[u32], index: i32) -> Option<u32> {
    let pick = |i: i32| -> u32 {
        usize::try_from(i)
            .ok()
            .and_then(|i| offsets.get(i).copied())
            .unwrap_or(0)
    };
    let i = if (offsets.len() as i64) <= index as i64 || pick(index) == 0 {
        1
    } else {
        index
    };
    Some(pick(i)).filter(|&o| o != 0)
}

/// The SSTM fields the movement code reads (`DAT_142ae5760`, set by
/// FUN_1403e9360 from 03system pref/sstm).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Sstm {
    /// +0x14 FallMax.
    pub fall_max: f32,
    /// +0x18 BlowBrakeXZ.
    pub blow_brake_xz: f32,
    /// +0x1c BlowMinXZ.
    pub blow_min_xz: f32,
    /// +0x20 BlowBrakeUp.
    pub blow_brake_up: f32,
    /// +0x24 BlowUp, +0x28 BlowSpeed (defaults when an ATKP's KB1 and KB2
    /// are both 0).
    pub blow_up: f32,
    pub blow_speed: f32,
    /// +0x2c BlowToHitBack: the knock strength of a blow is its speed
    /// vector's length × this.
    pub blow_to_hit_back: f32,
    /// +0x30 HitBack, +0x34 HitBackSmall (KB1 0x7fff).
    pub hit_back: f32,
    pub hit_back_small: f32,
    /// +0x38 HitBackToJump: the horizontal speed of the hop an aerial hit
    /// gives, per unit of knock strength.
    pub hit_back_to_jump: f32,
}

impl Sstm {
    /// Reads the fields from the SSTM record (the bytes at the record
    /// offset). None when the record is too short.
    pub fn read(record: &[u8]) -> Option<Self> {
        let f = |at: usize| {
            record
                .get(at..at + 4)
                .map(|b| f32::from_le_bytes(b.try_into().unwrap()))
        };
        Some(Sstm {
            fall_max: f(0x14)?,
            blow_brake_xz: f(0x18)?,
            blow_min_xz: f(0x1c)?,
            blow_brake_up: f(0x20)?,
            blow_up: f(0x24)?,
            blow_speed: f(0x28)?,
            blow_to_hit_back: f(0x2c)?,
            hit_back: f(0x30)?,
            hit_back_small: f(0x34)?,
            hit_back_to_jump: f(0x38)?,
        })
    }

    /// FUN_1403ceab0: knock-slide strength for an ATKP KB1. 0x7fff selects
    /// HitBackSmall for reaction class 3 and HitBack otherwise; reactions
    /// 12 and 13 halve it.
    pub fn knock_strength(&self, kb1: i16, class: u8, reaction: u8) -> f32 {
        let base = if kb1 == 0x7fff {
            if class == 3 {
                self.hit_back_small
            } else {
                self.hit_back
            }
        } else {
            kb1 as f32
        };
        if matches!(reaction, 12 | 13) {
            base * 0.5
        } else {
            base
        }
    }
}

/// The jump/fall state at obj+0xf0..+0xfc, kept in the executable's
/// Y-down terms.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Arc {
    /// +0xf0: ticks since the arc began.
    pub t: f32,
    /// +0xf4: rise duration √(2h/g); 0 for a plain fall.
    pub rise: f32,
    /// +0xf8: Y-down base height.
    pub base: f32,
    /// +0xfc: Y-down rise span (−h).
    pub span: f32,
}

impl Arc {
    /// FUN_1403cbb00 / FUN_1403cbc30 with the base offset FUN_1403cbd30
    /// adds on entering the jump mode. `height` is the Y-down request: a
    /// value ≥ 0 rises by that much; a negative value starts a fall already
    /// moving down as if dropped from that far above. `y` is Y-up.
    pub fn launch(y: f32, height: f32) -> Self {
        let mut arc = if height < 0. {
            // fVar1 < 0: t = √|y·(−2.0) / g| (DAT_140623c60 = −2.0), base = y.
            Arc {
                t: ((height * -2.0) / G).abs().sqrt(),
                rise: 0.,
                base: height,
                span: 0.,
            }
        } else {
            Arc {
                t: 0.,
                rise: rise_time(height),
                base: 0.,
                span: -height,
            }
        };
        arc.base += -y;
        arc
    }

    /// FUN_1403b8150 / FUN_1403b4780: leaving the ground with a Y-up
    /// vertical speed. The parabola continues from that speed
    /// (t = v_down / g when moving down, else 0, as in FUN_1403b4780).
    pub fn fall(y: f32, vertical_up: f32) -> Self {
        let v_down = -vertical_up;
        let t = if v_down <= 0. { 0. } else { v_down / G };
        Arc {
            t,
            rise: 0.,
            base: -y - t * G * t * 0.5,
            span: 0.,
        }
    }

    /// FUN_1403cb500's target height (Y-down) at the current `t`.
    fn target(&self) -> f32 {
        if self.rise <= self.t {
            let k = self.t - self.rise;
            self.span + self.base + G * k * k * 0.5
        } else {
            let u = if self.rise != 0. {
                self.t / self.rise
            } else {
                self.t
            };
            let pi = std::f32::consts::PI;
            let s = (u * pi * 0.5).sin();
            let s = (s * pi * 0.5).sin();
            s * self.span + self.base
        }
    }

    /// FUN_1403cb360 / FUN_1403cb500 over `dt`: advance `t`, then return
    /// the Y-up vertical displacement toward the curve, with downward
    /// motion capped at `fall_max` per tick. `frozen` is the no-gravity
    /// state (obj+0x18c & 0x1000020 or bit 6), which holds the height while
    /// `t` keeps running.
    pub fn step(&mut self, y: f32, fall_max: f32, frozen: bool, dt: f32) -> f32 {
        self.t += dt;
        if frozen {
            return 0.;
        }
        let dy = (self.target() - -y).min(fall_max * dt);
        -dy
    }

    /// FUN_1403cb680: true once `rise ≤ t + 5`, when the fall motion plays.
    pub fn falling_motion(&self) -> bool {
        self.rise <= self.t + FALL_MOTION_LEAD
    }

    /// FUN_1403cb680: a released jump (obj+0x120 bit 18) is cut while
    /// `6 < t < rise − 5`.
    pub fn cuttable(&self) -> bool {
        JUMP_CUT_AFTER < self.t && self.t < self.rise - FALL_MOTION_LEAD
    }
}

/// FUN_1403b73f0: √(2h/g).
pub fn rise_time(height: f32) -> f32 {
    ((height + height) / G).abs().sqrt()
}

/// FUN_1403b7ad0 (horizontal part): with a stick velocity (`speed` > 0)
/// the velocity moves toward `direction × speed` keeping `inertia` of
/// itself; otherwise it is scaled by `drag`. Returns the new velocity; the
/// displacement is velocity × DT.
pub fn steer(
    velocity: [f32; 2],
    direction: [f32; 2],
    speed: f32,
    inertia: f32,
    drag: f32,
) -> [f32; 2] {
    if speed <= 0. {
        velocity.map(|v| v * drag)
    } else {
        let target = [direction[0] * speed, direction[1] * speed];
        // FUN_140141ae0(out, a, b, t) = b·(1 − t) + a·t.
        [
            target[0] * (1. - inertia) + velocity[0] * inertia,
            target[1] * (1. - inertia) + velocity[1] * inertia,
        ]
    }
}

/// The velocity sampling of FUN_1403b89a0 (timer obj+0xba8, per-frame
/// velocity obj+0xb98).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Sampler {
    pub timer: f32,
    /// Displacement per frame until the next run.
    pub velocity: [f32; 3],
}

impl Sampler {
    /// FUN_1403b4da0 clears the timer on every physics-mode change, so the
    /// new mode's routine runs on the next frame.
    pub fn reset(&mut self) {
        self.timer = 0.;
    }

    /// One frame: true when the mode's velocity routine runs now (with
    /// `PHYSICS_STEP` as its time step); its result goes to `store`.
    pub fn due(&mut self, move_speed: f32) -> bool {
        self.timer -= DT;
        if EVERY_FRAME_SPEED < move_speed {
            self.timer = -PHYSICS_STEP;
        }
        self.timer < 0.
    }

    /// Keeps the routine's displacement over `PHYSICS_STEP` as a per-frame
    /// velocity (× 1/step) and restarts the timer.
    pub fn store(&mut self, displacement: [f32; 3]) {
        self.velocity = displacement.map(|c| c * (1. / PHYSICS_STEP));
        self.timer += PHYSICS_STEP;
    }

    /// This frame's displacement (velocity × DT).
    pub fn displacement(&self) -> [f32; 3] {
        self.velocity.map(|c| c * DT)
    }
}

/// A knock-slide (obj+0xd24 direction, +0xd30 speed).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Knock {
    pub direction: [f32; 3],
    pub speed: f32,
}

impl Knock {
    /// FUN_1403d4960: a negative strength flips the direction; the
    /// direction is normalised and the speed is (strength + bonus) × 0.1.
    pub fn new(direction: [f32; 3], strength: f32, bonus: f32) -> Option<Self> {
        let (mut d, mut s) = (direction, strength);
        if s < 0. {
            s = -s;
            d = d.map(|c| -c);
        }
        let len = (d[0] * d[0] + d[1] * d[1] + d[2] * d[2]).sqrt();
        if len <= 0. || !len.is_finite() {
            return None;
        }
        Some(Knock {
            direction: d.map(|c| c / len),
            speed: (s + bonus) * KNOCK_SCALE,
        })
    }

    /// FUN_1403d4570, one tick: the displacement, or None once the speed
    /// has dropped below 1 (the slide then ends).
    pub fn step(&mut self) -> Option<[f32; 3]> {
        if self.speed <= 0. || self.speed < KNOCK_STOP {
            self.speed = 0.;
            return None;
        }
        let v = self.speed * DT;
        let out = self.direction.map(|c| c * v);
        self.speed *= KNOCK_DECAY.powf(DT);
        Some(out)
    }
}

/// Blow flight (physics mode 4), Y-up velocity.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Blow {
    pub velocity: [f32; 3],
    /// +0x10c: the horizontal speed floor.
    pub min_xz: f32,
}

impl Blow {
    /// FUN_1404228b0 (mode 4 enter): floor = |v_xz| × BlowMinXZ.
    pub fn new(velocity: [f32; 3], sstm: &Sstm) -> Self {
        Blow {
            velocity,
            min_xz: velocity[0].hypot(velocity[2]) * sstm.blow_min_xz,
        }
    }

    /// FUN_140422780, one tick: the horizontal speed brakes by BlowBrakeXZ
    /// down to the floor; a rising speed brakes by BlowBrakeUp; gravity is
    /// added with the downward speed capped at `fall_max`. Returns the
    /// displacement.
    pub fn step(&mut self, sstm: &Sstm, fall_max: f32, dt: f32) -> [f32; 3] {
        let [x, y, z] = self.velocity;
        let len = x.hypot(z);
        let (nx, nz) = if len > 0. {
            (x / len, z / len)
        } else {
            (0., 0.)
        };
        let speed = (len * sstm.blow_brake_xz).max(self.min_xz);
        // Y-down: rising is negative.
        let mut down = -y;
        if down < 0. {
            down *= sstm.blow_brake_up;
        }
        let down = (down + G).min(fall_max);
        self.velocity = [speed * nx, -down, speed * nz];
        self.velocity.map(|c| c * dt)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_and_prty_follow_the_init_routines() {
        let mut p = ObjParams::defaults(16.);
        assert_eq!((p.walk, p.run, p.jump), (2., 8., 160.));
        assert!((p.turn - std::f32::consts::PI / 16.).abs() < 1e-6);
        let mut words = [0u32; 13];
        for (w, v) in words.iter_mut().zip([2.5f32, 9., 185., 0.25]) {
            *w = v.to_bits();
        }
        words[8] = 3f32.to_bits();
        p.apply_prty(&words);
        assert_eq!(
            (p.walk, p.run, p.jump, p.turn, p.word8),
            (2.5, 9., 185., 0.25, 3.)
        );
        assert_eq!(p.fall_max, 16.);
    }

    #[test]
    fn prty_index_falls_back_to_record_one() {
        let offsets = [0, 40, 92, 0];
        assert_eq!(prty_offset(&offsets, 2), Some(92));
        assert_eq!(prty_offset(&offsets, 3), Some(40));
        assert_eq!(prty_offset(&offsets, 9), Some(40));
        assert_eq!(prty_offset(&[0, 0], 5), None);
    }

    #[test]
    fn jump_rises_on_the_double_sine_and_falls_on_the_parabola() {
        let mut arc = Arc::launch(0., 185.);
        assert!((arc.rise - 30.108_14).abs() < 1e-3);
        let mut y = 0.;
        let mut heights = Vec::new();
        let mut landed = None;
        for tick in 1..200 {
            y += arc.step(y, 16., false, DT);
            heights.push(y);
            if y <= 0. && tick > 1 {
                landed = Some(tick);
                break;
            }
        }
        // h·sin(π/2·sin(π/2·t/T)) at t = 5, 10, 15, 20.
        for (t, want) in [(5, 72.9), (10, 130.5), (15, 165.5), (20, 180.8)] {
            assert!(
                (heights[t - 1] - want).abs() < 0.1,
                "t {t}: {}",
                heights[t - 1]
            );
        }
        let apex = heights.iter().copied().fold(0., f32::max);
        assert!(apex > 184.9 && apex <= 185.0 + 1e-3, "{apex}");
        // Falls back over ≈ √(2h/g) ticks: lands at tick 61 (t ≈ 60.2).
        assert_eq!(landed, Some(61));
    }

    #[test]
    fn long_falls_cap_at_fall_max() {
        let mut arc = Arc::fall(1000., 0.);
        let mut y = 1000.;
        let mut last = 0.;
        for _ in 0..80 {
            last = arc.step(y, 16., false, DT);
            y += last;
        }
        assert_eq!(last, -16.);
    }

    #[test]
    fn frozen_ticks_hold_height_but_advance_time() {
        let mut arc = Arc::launch(0., 185.);
        assert_eq!(arc.step(0., 16., true, DT), 0.);
        assert_eq!(arc.t, 1.);
    }

    #[test]
    fn negative_launch_starts_already_falling() {
        let mut arc = Arc::launch(50., -100.);
        assert_eq!(arc.rise, 0.);
        let dy = arc.step(50., 16., false, DT);
        // Starts at speed g·t0 = √(2·100·g) ≈ 9.0 downward.
        assert!(dy < -9. && dy > -9.5, "{dy}");
    }

    #[test]
    fn air_steering_lerps_and_drags() {
        let v = steer([0., 0.], [1., 0.], 8., AIR_INERTIA, AIR_DRAG);
        assert!((v[0] - 0.8).abs() < 1e-6);
        let v = steer([8., 0.], [0., 0.], 0., AIR_INERTIA, AIR_DRAG);
        assert!((v[0] - 7.6).abs() < 1e-6);
    }

    #[test]
    fn knock_slide_decays_and_stops_below_one() {
        let mut k = Knock::new([0., 0., -2.], 200., 0.).unwrap();
        let mut total = 0.;
        let mut n = 0;
        while let Some(d) = k.step() {
            total += -d[2];
            n += 1;
        }
        assert_eq!(n, 29);
        assert!((total - 190.58).abs() < 0.05, "{total}");
        let flipped = Knock::new([1., 0., 0.], -50., 0.).unwrap();
        assert_eq!(flipped.direction, [-1., 0., 0.]);
    }

    #[test]
    fn blow_brakes_to_its_floor_and_falls() {
        let s = Sstm {
            fall_max: 16.,
            blow_brake_xz: 0.8,
            blow_min_xz: 0.1,
            blow_brake_up: 0.8,
            blow_up: 24.,
            blow_speed: 40.,
            hit_back: 200.,
            hit_back_small: 200.,
            blow_to_hit_back: 8.,
            hit_back_to_jump: 0.048,
        };
        let mut b = Blow::new([40., 24., 0.], &s);
        assert!((b.min_xz - 4.).abs() < 1e-6);
        let d = b.step(&s, 16., DT);
        assert!((d[0] - 32.).abs() < 1e-5);
        assert!((d[1] - (24. * 0.8 - G)).abs() < 1e-5);
        for _ in 0..60 {
            b.step(&s, 16., DT);
        }
        assert!((b.velocity[0] - 4.).abs() < 1e-5);
        assert_eq!(b.velocity[1], -16.);
        assert_eq!(s.knock_strength(0x7fff, 3, 3), 200.);
        assert_eq!(s.knock_strength(80, 3, 12), 40.);
    }

    #[test]
    fn sampler_runs_every_second_frame() {
        let mut s = Sampler::default();
        let mut runs = Vec::new();
        for frame in 0..6 {
            if s.due(8.) {
                runs.push(frame);
                s.store([16., 0., 0.]);
            }
            assert_eq!(s.displacement(), [8., 0., 0.]);
        }
        assert_eq!(runs, [0, 2, 4]);
        let mut fast = Sampler::default();
        assert!((0..3).all(|_| {
            let due = fast.due(150.);
            fast.store([0.; 3]);
            due
        }));
    }
}
