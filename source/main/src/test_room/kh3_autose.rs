//! KH3's automatic motion sounds: `SQEXSEADAutoSeComponent` with `BoneSet_Sora1`, `FsTable_Sora` and
//! `DetectioinSetting_Sora`: footsteps, jump and landing sounds and cloth rustle from the bones' motion. No clip of
//! Sora's carries a footstep notify: this detector is where his steps come from. Ported from StrawHatPro's
//! `kh-audio/src/autose.rs` (MIT; analyzer 0x14594E6D0, detectors 0x14597D600 / 0x14597D930 / 0x14597E180, tick
//! 0x145818B70). It works in metres at 60 Hz, which is the game at 60 frames a second.

use glam::Vec3;

/// `ESQEXSEADAutoSeMotionSoundType`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(crate) enum Motion {
    Walk = 1,
    Run = 2,
    Jump = 3,
    LandNormal = 4,
    LandHard = 5,
    FootShuffle = 6,
    RustleShort = 7,
    RustleLong = 8,
    RustleFootShort = 9,
    RustleFootLong = 10,
}

impl Motion {
    pub(crate) fn named(name: &str) -> Option<Motion> {
        Some(match name.rsplit("::").next().unwrap_or(name) {
            "Walk" => Motion::Walk,
            "Run" => Motion::Run,
            "Jump" => Motion::Jump,
            "LandNormal" => Motion::LandNormal,
            "LandHard" => Motion::LandHard,
            "FootShuffle" => Motion::FootShuffle,
            "RustleShort" => Motion::RustleShort,
            "RustleLong" => Motion::RustleLong,
            "RustleFootShort" => Motion::RustleFootShort,
            "RustleFootLong" => Motion::RustleFootLong,
            _ => return None,
        })
    }
}

/// Which side a sound of a foot or an arm is at (`L_ashi1` / `R_ashi1`, `L_ude1` / `R_ude1`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Part {
    Foot(usize),
    Arm(usize),
}

impl Part {
    pub(crate) fn bone(self) -> &'static str {
        match self {
            Part::Foot(0) => "L_ashi1",
            Part::Foot(_) => "R_ashi1",
            Part::Arm(0) => "L_ude1",
            Part::Arm(_) => "R_ude1",
        }
    }
}

/// `BoneSet_Sora1`'s bones, in the order `Frame::of` takes them.
pub(crate) const BONES: [&str; 14] = [
    "Trans", "koshi", "L_momo", "L_sune", "L_ashi1", "L_ashi2", "R_momo", "R_sune", "R_ashi1",
    "R_ashi2", "L_ude1", "L_ude2", "R_ude1", "R_ude2",
];

/// One frame's input: world positions, cm, Z up.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub(crate) struct Frame {
    pub(crate) base: Vec3,
    pub(crate) waist: Vec3,
    /// Femur, tibia, foot per side.
    pub(crate) legs: [[Vec3; 3]; 2],
    pub(crate) elbows: [Vec3; 2],
    /// The movement mode is walking.
    pub(crate) grounded: bool,
}

impl Frame {
    pub(crate) fn of(p: &[Vec3; 14], grounded: bool) -> Frame {
        Frame {
            base: p[0],
            waist: p[1],
            legs: [[p[2], p[3], p[4]], [p[6], p[7], p[8]]],
            elbows: [p[11], p[13]],
            grounded,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct Event {
    pub(crate) motion: Motion,
    pub(crate) part: Part,
    pub(crate) volume: f32,
}

/// The two numbers `DetectioinSetting_Sora` overrides (native defaults 0.5 and 2.0, 0x14597FE40).
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct Settings {
    pub(crate) shuffle_body_velocity: f32,
    pub(crate) shuffle_foot_accel: f32,
}

impl Default for Settings {
    fn default() -> Self {
        Settings {
            shuffle_body_velocity: 0.5,
            shuffle_foot_accel: 2.,
        }
    }
}

// The executable's constants (the addresses are in the module's comment and `sora-audio-spec`'s section 4).
/// No analysis beyond this many metres from the listener; rustles within the second.
const CLIP_DISTANCE: f32 = 20.;
const RUSTLE_DISTANCE: f32 = 10.;
const RUN_SPEED: f32 = 4.166_667_5;
const HEEL_RATIO: f32 = 1.2;
const FOOT_GROUNDED_RATIO: f32 = 0.2;
const LAND_NORMAL_AIR: f32 = 0.3;
const LAND_HARD_AIR: f32 = 1.5;
const STEP_MIN_FOOT_SPEED: f32 = 0.125;
const WALK_VOLUME: [f32; 2] = [0., 1.5];
const RUN_VOLUME: [f32; 2] = [0., 5.];
const SHUFFLE_VOLUME: [f32; 2] = [-0.5, 3.];
const ADD_ENERGY: [f32; 2] = [0., 130.];
const ARM_WALK: f32 = 0.45;
const ARM_RUN: f32 = 0.8;
const ARM_MAX: f32 = 1.;
const ARM_COOLDOWN: f32 = 0.1333;
const LEG_DOT: f32 = 0.6;
const LEG_KNEE: [f32; 2] = [0., 1.];
const PART_COOLDOWN: f32 = 0.333_333_34;
const EVENTS_PER_FRAME: usize = 2;
const WARMUP_FRAMES: i32 = 3;

#[derive(Clone, Copy, Debug, Default)]
struct Analysis {
    grounded: bool,
    waist: Vec3,
    waist_speed: f32,
    running: bool,
    foot: [Vec3; 2],
    heel: [Vec3; 2],
    knee: [Vec3; 2],
    elbow: [Vec3; 2],
    foot_speed: [f32; 2],
    heel_speed: [f32; 2],
    knee_speed: [f32; 2],
    foot_grounded: [bool; 2],
    arm_rel: [f32; 2],
    leg_dot: [f32; 2],
    air_time: f32,
}

/// The detector's state.
#[derive(Clone, Debug)]
pub(crate) struct AutoSe {
    previous: Option<Analysis>,
    warmup: i32,
    cooldown_foot: [f32; 2],
    cooldown_arm: [f32; 2],
}

impl Default for AutoSe {
    fn default() -> Self {
        AutoSe {
            previous: None,
            warmup: WARMUP_FRAMES,
            cooldown_foot: [0.; 2],
            cooldown_arm: [0.; 2],
        }
    }
}

fn clamp01(v: f32) -> f32 {
    v.clamp(0., 1.)
}

fn map(v: f32, range: [f32; 2]) -> f32 {
    (v - range[0]) / (range[1] - range[0])
}

impl AutoSe {
    /// One frame; at most `EVENTS_PER_FRAME` events, in the detectors' order. `enabled`: the
    /// `ChangeAutoSeEnable` windows. `listener`: metres to the listener.
    pub(crate) fn step(
        &mut self,
        dt: f32,
        frame: &Frame,
        listener: f32,
        enabled: bool,
        settings: &Settings,
    ) -> Vec<Event> {
        let mut events = Vec::new();
        for c in self
            .cooldown_foot
            .iter_mut()
            .chain(self.cooldown_arm.iter_mut())
        {
            *c = (*c - dt).max(0.);
        }
        if dt <= 0. {
            return events;
        }
        if listener > CLIP_DISTANCE {
            self.previous = None;
            self.warmup = WARMUP_FRAMES;
            return events;
        }
        const M: f32 = 0.01;
        let prev = self.previous;
        let mut now = Analysis {
            grounded: frame.grounded,
            waist: frame.waist * M,
            ..Analysis::default()
        };
        let base = frame.base * M;
        let speed = |now: Vec3, before: Option<Vec3>| before.map_or(0., |b| (now - b).length() / dt);
        now.waist_speed = speed(now.waist, prev.map(|p| p.waist));
        now.running = now.waist_speed >= RUN_SPEED;
        let spine = (base - now.waist).normalize_or_zero();
        for k in 0..2 {
            let [femur, tibia, foot] = frame.legs[k].map(|v| v * M);
            now.foot[k] = foot;
            now.heel[k] = tibia + (foot - tibia) * HEEL_RATIO;
            now.knee[k] = tibia;
            now.foot_speed[k] = speed(foot, prev.map(|p| p.foot[k]));
            now.heel_speed[k] = speed(now.heel[k], prev.map(|p| p.heel[k]));
            now.knee_speed[k] = speed(tibia, prev.map(|p| p.knee[k]));
            now.foot_grounded[k] = frame.grounded
                && (now.heel[k].z - base.z) < FOOT_GROUNDED_RATIO * (tibia - foot).length();
            now.leg_dot[k] = (tibia - femur).normalize_or_zero().dot(spine);
            now.elbow[k] = frame.elbows[k] * M;
            now.arm_rel[k] = speed(now.elbow[k], prev.map(|p| p.elbow[k])) - now.waist_speed;
        }
        now.air_time = if now.foot_grounded.iter().any(|g| *g) {
            0.
        } else {
            prev.map_or(0., |p| p.air_time) + dt
        };
        self.previous = Some(now);
        let Some(prev) = prev else {
            return events;
        };
        if self.warmup > 0 {
            self.warmup -= 1;
            return events;
        }
        if !enabled {
            return events;
        }
        let take_off = prev.grounded && !now.grounded;
        let landing = !prev.grounded && now.grounded;
        // The landing detector: played at the left foot.
        if take_off {
            events.push(Event {
                motion: Motion::Jump,
                part: Part::Foot(0),
                volume: 1.,
            });
        } else if landing && prev.air_time >= LAND_NORMAL_AIR && self.cooldown_foot[0] <= 0. {
            events.push(Event {
                motion: if prev.air_time >= LAND_HARD_AIR {
                    Motion::LandHard
                } else {
                    Motion::LandNormal
                },
                part: Part::Foot(0),
                volume: 1.,
            });
            self.cooldown_foot[0] = PART_COOLDOWN;
        }
        // The foot step detector: the first foot that matches.
        for k in 0..2 {
            if events.len() >= EVENTS_PER_FRAME {
                break;
            }
            let energy = (0.5 * now.foot_speed[k] * now.foot_speed[k]
                - 0.5 * prev.foot_speed[k] * prev.foot_speed[k])
                .abs();
            let add = clamp01(map(energy, ADD_ENERGY));
            let shuffle = prev.foot_grounded[k]
                && now.foot_grounded[k]
                && now.waist_speed < settings.shuffle_body_velocity
                && ((now.heel_speed[k] - prev.heel_speed[k]) / dt).abs()
                    > settings.shuffle_foot_accel;
            let edge = !prev.foot_grounded[k]
                && now.foot_grounded[k]
                && prev.foot_speed[k] > STEP_MIN_FOOT_SPEED;
            let event = if shuffle {
                Some((
                    Motion::FootShuffle,
                    clamp01(map(now.waist_speed, SHUFFLE_VOLUME) + add),
                ))
            } else if edge {
                let (motion, range) = if now.running {
                    (Motion::Run, RUN_VOLUME)
                } else {
                    (Motion::Walk, WALK_VOLUME)
                };
                Some((motion, clamp01(map(now.waist_speed, range) + add)))
            } else {
                None
            };
            if let Some((motion, volume)) = event {
                if self.cooldown_foot[k] <= 0. {
                    events.push(Event {
                        motion,
                        part: Part::Foot(k),
                        volume,
                    });
                    self.cooldown_foot[k] = PART_COOLDOWN;
                }
                break;
            }
        }
        // The rustle detector.
        if listener <= RUSTLE_DISTANCE {
            if (take_off || landing)
                && events.len() < EVENTS_PER_FRAME
                && self.cooldown_arm[0] <= 0.
            {
                events.push(Event {
                    motion: Motion::RustleLong,
                    part: Part::Arm(0),
                    volume: 1.,
                });
                self.cooldown_arm[0] = PART_COOLDOWN;
            }
            let (threshold, motion) = if now.running {
                (ARM_RUN, Motion::RustleShort)
            } else {
                (ARM_WALK, Motion::RustleLong)
            };
            for k in 0..2 {
                if events.len() >= EVENTS_PER_FRAME {
                    break;
                }
                if prev.arm_rel[k] < threshold
                    && now.arm_rel[k] >= threshold
                    && self.cooldown_arm[k] <= 0.
                {
                    let volume = clamp01((now.arm_rel[k].abs() - threshold) / (ARM_MAX - threshold));
                    events.push(Event {
                        motion,
                        part: Part::Arm(k),
                        volume,
                    });
                    self.cooldown_arm = [ARM_COOLDOWN; 2];
                }
            }
            let leg = if now.running {
                Motion::RustleFootShort
            } else {
                Motion::RustleFootLong
            };
            for k in 0..2 {
                if events.len() >= EVENTS_PER_FRAME {
                    break;
                }
                if prev.leg_dot[k] < LEG_DOT && now.leg_dot[k] >= LEG_DOT {
                    events.push(Event {
                        motion: leg,
                        part: Part::Foot(k),
                        volume: clamp01(map(now.knee_speed[k], LEG_KNEE)),
                    });
                }
            }
        }
        events
    }
}

/// The physical surface an arena's floor would have in KH3 (`EPhysicalSurface`, `FsTable_Sora`'s index) [duel
/// choice: KH2's rooms have no physical materials]: the Mirage Arena's polished checker is marble, the test room's
/// rock (surface 0 uses rock's bank).
pub(crate) fn surface_of_arena(arena: usize) -> usize {
    if arena == super::duel_arena::MIRAGE {
        11
    } else {
        10
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn frame(foot_z: f32, grounded: bool) -> Frame {
        Frame {
            base: Vec3::ZERO,
            waist: Vec3::new(0., 0., 90.),
            legs: [
                [
                    Vec3::new(0., 0., 80.),
                    Vec3::new(0., 0., 40.),
                    Vec3::new(0., 0., foot_z),
                ],
                [
                    Vec3::new(30., 0., 80.),
                    Vec3::new(30., 0., 40.),
                    Vec3::new(30., 0., 60.),
                ],
            ],
            elbows: [Vec3::new(0., 0., 120.); 2],
            grounded,
        }
    }

    #[test]
    fn a_foot_coming_down_is_a_step_and_a_take_off_a_jump() {
        let mut d = AutoSe::default();
        let s = Settings::default();
        // The warm-up and a foot in the air.
        for _ in 0..6 {
            assert!(d.step(1. / 60., &frame(30., true), 1., true, &s).is_empty());
        }
        let events = d.step(1. / 60., &frame(5., true), 1., true, &s);
        assert_eq!(events[0].motion, Motion::Walk);
        assert_eq!(events[0].part, Part::Foot(0));
        // Off the ground.
        let events = d.step(1. / 60., &frame(5., false), 1., true, &s);
        assert_eq!(events[0].motion, Motion::Jump);
        // Disabled by a clip's window: nothing.
        assert!(d.step(1. / 60., &frame(5., true), 1., false, &s).is_empty());
    }

    #[test]
    fn far_from_the_listener_nothing_is_analysed() {
        let mut d = AutoSe::default();
        let s = Settings::default();
        for _ in 0..6 {
            assert!(d.step(1. / 60., &frame(30., true), 25., true, &s).is_empty());
        }
    }
}
