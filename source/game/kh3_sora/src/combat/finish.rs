//! THE BOSS FINISH (docs/sora-bossfinish-spec.md): what Kingdom Hearts III shows when the last
//! blow lands on a boss. In the game this is NOT native code and not a flag of the enemy: every
//! boss map's level blueprint runs the same script on the wipe-out event of the boss's spawn
//! group (128 packages; sheet `sora-bossfinish`, reference graph `tt_01_camera_ms060`):
//!
//! ```text
//! kill   : 2D sound se00000_047 | TresStartBossDefeatEffect | pause menu off | BGM fades out (4 s)
//!          SetGlobalGameSpeed(CH_EVENT, 0.05, 0.1 s) | radial screen blur 0.5 | sound mode BOSS_DEAD
//!          cinematic mode: no movement, no camera stick
//! +1 tick: cut 1: a camera on the ENEMY (bone t_camera), 200 -> 300 cm, turning round it; shake
//! +0.05 s of game time (1 s real): cut 2 on SORA      +0.10 s (2 s): cut 3, a wide shot of the enemy
//! +0.15 s (3 s): white-out, 0.08 s of game time (1.6 s real); when it is white:
//!          game speed 1 at once | the cameras end | every prize flies to the player
//! +1.0 s : the enemy is removed, blur off, sound mode off, control back | fade in from white 0.7 s
//! ```
//!
//! THE PORT: the core raises [`crate::host::EventKind::BossFinish`] when a hit kills a target the
//! host calls finish-eligible ([`crate::host::Targets::finish_kind`]); a HOST then owns one
//! [`FinishSequence`], steps it with the REAL frame time and acts on the [`FinishFrame`] it
//! returns: it scales the `dt` of everything it steps by `rate` (KH3 slows the whole world: the
//! global game speed of host/time.rs), shows the camera cut ([`FinishShot`]: `kh_camera::finish`
//! turns it into a view), draws the blur and the white plate, locks the input and carries out the
//! cues (sounds, sound states, shake, prize collection). A host whose kills are confirmed later
//! (Elden Ring: the damage goes through the game's own pipeline) starts the sequence itself.
//!
//! [`FinishKind::Battle`] is a PORT OPTION, not a KH3 effect (no ordinary encounter slows down):
//! a short slow motion without camera, blur or white.

use crate::host::time::{GameSpeed, TimeTable, CH_EVENT};
use crate::host::FinishKind;

include!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/src/gen/sora_bossfinish.rs"
));

/// The sheet's values.
pub static P: &std::sync::LazyLock<BossFinishParams> = &BOSS_FINISH_PARAMS;

/// The sound events of `TresSoundMode_Start(4)` / `_End(4)` (BOSS_DEAD, 0x141515D60 / 0x141515C90).
pub const SOUND_STATE_START: &str = "STATE_MISSION_RESULT_START";
pub const SOUND_STATE_END: &str = "STATE_MISSION_RESULT_END";
/// The port's last-enemy slow-down uses the slow-motion sound state the shotlock uses.
pub const SOUND_SLOW_START: &str = "STATE_SLOW_START";
pub const SOUND_SLOW_END: &str = "STATE_SLOW_END";

/// Who a cut looks at.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Subject {
    /// The defeated enemy (its bone `t_camera`, or a point of its body).
    Enemy,
    /// Sora (his bone `t_camera`).
    Player,
}

/// One camera cut of the script (`ATresCameraRotTarget` "Battle_End"): the start and end of its
/// move, relative to the subject.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CutDef {
    pub subject: Subject,
    /// Game seconds after the first tick at which it starts.
    pub delay: f32,
    /// cm, start -> end.
    pub distance: [f32; 2],
    /// Pitch, yaw, roll in degrees, yaw relative to the subject's facing; start and end.
    pub rotation: [f32; 3],
    pub rotation_end: [f32; 3],
    /// cm in the subject's frame, UE axes (X forward, Y right, Z up).
    pub local_offset: [f32; 3],
    /// The game's own camera collision for this cut (the first one has it switched off).
    pub collision: bool,
}

fn subject(word: &str) -> Subject {
    if word == "player" {
        Subject::Player
    } else {
        Subject::Enemy
    }
}

/// The three cuts of the sheet.
pub fn sheet_cuts() -> [CutDef; 3] {
    [
        CutDef {
            subject: subject(P.cut_1_subject),
            delay: P.cut_1_delay,
            distance: P.cut_1_distance,
            rotation: P.cut_1_rotation,
            rotation_end: P.cut_1_rotation_end,
            local_offset: P.cut_1_local_offset,
            collision: P.cut_1_collision != 0.0,
        },
        CutDef {
            subject: subject(P.cut_2_subject),
            delay: P.cut_2_delay,
            distance: P.cut_2_distance,
            rotation: P.cut_2_rotation,
            rotation_end: P.cut_2_rotation_end,
            local_offset: P.cut_2_local_offset,
            collision: P.cut_2_collision != 0.0,
        },
        CutDef {
            subject: subject(P.cut_3_subject),
            delay: P.cut_3_delay,
            distance: P.cut_3_distance,
            rotation: P.cut_3_rotation,
            rotation_end: P.cut_3_rotation_end,
            local_offset: P.cut_3_local_offset,
            collision: P.cut_3_collision != 0.0,
        },
    ]
}

/// What a host may change of the script (its keys); the default is the sheet = the game.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct FinishTuning {
    /// The world's rate during the slow motion and the real seconds of the ramp down to it.
    pub rate: f32,
    pub interp: f32,
    /// Real seconds of one script tick (`TresWaitCycle(1)`).
    pub cycle: f32,
    pub cuts: [CutDef; 3],
    /// How many of the cuts are shown (0 = the host's own camera stays).
    pub cut_count: u8,
    /// Scales every cut's distance (a host whose bodies are larger than KH3's).
    pub distance_scale: f32,
    /// Game seconds of a cut's move and the interpolator's type.
    pub move_time: f32,
    pub move_curve: u8,
    /// Game seconds after the first tick at which the white-out starts, its game seconds, and
    /// whether it is shown at all (false: the sequence ends where the white-out would be full).
    pub white_delay: f32,
    pub white_time: f32,
    pub white: bool,
    /// Real seconds under the white after the speed is back, and of the fade in.
    pub hold: f32,
    pub fade_in: f32,
    pub blur: f32,
    pub shake_scale: f32,
    /// `ETresStaticSE` of the 2D sound (22 = se00000_047, 23 = se00000_048; negative = none).
    pub static_se: i32,
    /// The BOSS_DEAD sound mode is sent (false: Sora's sounds keep playing).
    pub sound_mode: bool,
    pub bgm_fade: f32,
    pub lock_input: bool,
    pub hide_hud: bool,
    pub collect_prizes: bool,
    /// Real seconds after which a running sequence is ended whatever its state.
    pub watchdog: f32,
    /// The port's last-enemy slow-down: rate, ramp in, seconds held, ramp out (real time).
    pub battle_rate: f32,
    pub battle_interp: f32,
    pub battle_hold: f32,
    pub battle_out: f32,
}

impl Default for FinishTuning {
    fn default() -> FinishTuning {
        FinishTuning {
            rate: P.speed_rate,
            interp: P.speed_interp,
            cycle: P.cycle,
            cuts: sheet_cuts(),
            cut_count: P.cut_count as u8,
            distance_scale: 1.0,
            move_time: P.cut_move_time,
            move_curve: P.cut_move_curve as u8,
            white_delay: P.white_delay,
            white_time: P.white_time,
            white: true,
            hold: P.after_hold,
            fade_in: P.after_fade_in,
            blur: P.blur_intensity,
            shake_scale: P.shake_scale,
            static_se: P.sound_static_se as i32,
            sound_mode: true,
            bgm_fade: P.sound_bgm_fade,
            lock_input: P.input_lock != 0.0,
            hide_hud: P.hud_hide != 0.0,
            collect_prizes: P.after_prizes != 0.0,
            watchdog: P.port_watchdog,
            battle_rate: P.battle_rate,
            battle_interp: P.battle_interp,
            battle_hold: P.battle_hold,
            battle_out: P.battle_out,
        }
    }
}

/// The cut on screen in one step: the interpolated numbers of its camera.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct FinishShot {
    /// 0, 1, 2.
    pub index: u8,
    pub subject: Subject,
    /// cm from the look point.
    pub distance: f32,
    /// Pitch, yaw, roll (degrees); the yaw is relative to the subject's facing.
    pub rotation: [f32; 3],
    /// cm in the subject's frame (X forward, Y right, Z up).
    pub local_offset: [f32; 3],
    /// The game's camera collision is on for this cut.
    pub collision: bool,
    /// This step is the cut's first: a hard cut (nothing is blended or interpolated across it).
    pub cut: bool,
}

/// Something a host does once.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum FinishCue {
    /// `SpawnSound2D` of a static SE: flat, volume 1, pitch 1, NOT slowed with the world.
    Sound { static_se: u32 },
    /// A sound-system event (`TresSoundMode_Start` / `_End`; the slow-motion state of the port's
    /// last-enemy variant).
    SoundState(&'static str),
    /// The playing music fades out over this many seconds (the map decides what follows).
    BgmFade { seconds: f32 },
    /// `StopAllCameraShake`.
    StopShakes,
    /// `ClientPlayCameraShake(no_speed_kokizami_1s, scale)`: runs on real time.
    Shake { scale: f32 },
    /// The cameras of the cuts ended (`EndAllBPTresCamera`, blend 0): the host's own camera shows.
    CamerasEnd,
    /// `AllPrizeCollection(player)`: every prize on the field flies to him.
    CollectPrizes,
    /// Control is back (`TresEndCinematicMode`).
    InputBack,
    /// The sequence is over (also after an abort).
    End,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum FinishPhase {
    #[default]
    Idle,
    /// The slow motion with the cuts.
    Slow,
    /// The white-out rises (still slow).
    White,
    /// White, normal speed.
    Hold,
    /// The white leaves.
    FadeIn,
}

/// What the host shows and does in one step.
#[derive(Clone, Debug, PartialEq)]
pub struct FinishFrame {
    pub active: bool,
    pub phase: FinishPhase,
    /// The world's rate (1.0 = normal): the host multiplies the `dt` of every step it makes by
    /// it, Sora's own included.
    pub rate: f32,
    pub shot: Option<FinishShot>,
    /// The radial blur's intensity (0 = off).
    pub blur: f32,
    /// The white plate's opacity, 0..1.
    pub white: f32,
    pub hud_hidden: bool,
    /// No movement, no attack, no camera stick, no pause menu.
    pub input_locked: bool,
    pub cues: Vec<FinishCue>,
}

impl FinishFrame {
    pub fn idle() -> FinishFrame {
        FinishFrame {
            active: false,
            phase: FinishPhase::Idle,
            rate: 1.0,
            shot: None,
            blur: 0.0,
            white: 0.0,
            hud_hidden: false,
            input_locked: false,
            cues: Vec::new(),
        }
    }

    /// The rate per object type for the seam of host/time.rs: KH3's global game speed slows every
    /// type alike (player, enemies, gimmicks, map, camera).
    pub fn table(&self) -> TimeTable {
        TimeTable::uniform(self.rate)
    }
}

/// The interpolator's curve by its type (`kh_camera::lockon::interp_curve`, dispatch 0x1411A0E70).
fn curve(kind: u8, t: f32) -> f32 {
    let t = t.clamp(0.0, 1.0);
    match kind {
        2 => (t * std::f32::consts::FRAC_PI_2).sin(),
        3 => 1.0 - (t * std::f32::consts::FRAC_PI_2).cos(),
        4 => (1.0 - (t * std::f32::consts::PI).cos()) * 0.5,
        5 => 1.0 - (1.0 - t) * (1.0 - t),
        6 => t * t,
        _ => t,
    }
}

/// The running presentation. `Default` = nothing runs.
#[derive(Clone, Debug, PartialEq, Default)]
pub struct FinishSequence {
    kind: FinishKind,
    tuning: Option<FinishTuning>,
    speed: GameSpeed,
    phase: FinishPhase,
    begun: bool,
    /// Real and game seconds since the start.
    real: f32,
    game: f32,
    /// Game seconds at the first tick (the delays count from it).
    tick_game: Option<f32>,
    /// The cut on screen (-1 none) and the game seconds at which it started.
    shot: i8,
    shot_game: f32,
    shot_fresh: bool,
    white: f32,
    blur: f32,
    /// Real seconds in the hold; the cameras ended / the prizes were collected.
    hold: f32,
    cameras_ended: bool,
    mode_on: bool,
    locked: bool,
    /// Sequences started so far (tests, a host's log).
    pub started: u32,
}

impl FinishSequence {
    /// Starts the presentation of `kind` (a running one is replaced; `None` does nothing).
    pub fn start(&mut self, kind: FinishKind, tuning: FinishTuning) {
        if kind == FinishKind::None {
            return;
        }
        let started = self.started + 1;
        *self = FinishSequence {
            kind,
            tuning: Some(tuning),
            phase: FinishPhase::Slow,
            shot: -1,
            started,
            ..FinishSequence::default()
        };
    }

    pub fn active(&self) -> bool {
        self.phase != FinishPhase::Idle
    }

    pub fn kind(&self) -> FinishKind {
        if self.active() {
            self.kind
        } else {
            FinishKind::None
        }
    }

    pub fn phase(&self) -> FinishPhase {
        self.phase
    }

    /// The world's rate as of the last step.
    pub fn rate(&self) -> f32 {
        if self.active() {
            self.speed.rate()
        } else {
            1.0
        }
    }

    /// Real seconds since the start.
    pub fn seconds(&self) -> f32 {
        self.real
    }

    /// Ends a running sequence NOW (the host lost the scene: a death, a load, another game's
    /// cutscene, its own stop): the returned frame has rate 1, no plate, and the cues that put
    /// everything back.
    pub fn abort(&mut self) -> FinishFrame {
        let mut frame = FinishFrame::idle();
        if !self.active() {
            return frame;
        }
        self.end(&mut frame.cues);
        frame
    }

    fn end(&mut self, cues: &mut Vec<FinishCue>) {
        if self.shot >= 0 && !self.cameras_ended {
            cues.push(FinishCue::CamerasEnd);
        }
        if self.mode_on {
            cues.push(FinishCue::SoundState(if self.kind == FinishKind::Boss {
                SOUND_STATE_END
            } else {
                SOUND_SLOW_END
            }));
        }
        if self.locked {
            cues.push(FinishCue::InputBack);
        }
        cues.push(FinishCue::End);
        let started = self.started;
        *self = FinishSequence {
            started,
            ..FinishSequence::default()
        };
    }

    const SLICE: f32 = 1.0 / 240.0;

    /// One step of `real_dt` REAL seconds (the host's frame or fixed step, never scaled).
    pub fn step(&mut self, real_dt: f32) -> FinishFrame {
        let Some(t) = self.tuning.filter(|_| self.active()) else {
            return FinishFrame::idle();
        };
        let mut cues = Vec::new();
        let real_dt = real_dt.clamp(0.0, 0.1);
        if self.real + real_dt > t.watchdog {
            self.end(&mut cues);
            return FinishFrame {
                cues,
                ..FinishFrame::idle()
            };
        }
        // The script is stepped in slices of at most 1/240 s so that its real-time line is the same
        // at every frame rate (in the game it shifts by a frame's worth of the ramp: spec 3.2).
        let mut left = real_dt;
        while left > 0.0 && self.active() {
            let slice = left.min(Self::SLICE);
            left -= slice;
            match self.kind {
                FinishKind::Boss => self.step_boss(&t, slice, &mut cues),
                _ => self.step_battle(&t, slice, &mut cues),
            }
        }
        if !self.active() {
            return FinishFrame {
                cues,
                ..FinishFrame::idle()
            };
        }
        let shot = (self.shot >= 0 && !self.cameras_ended).then(|| {
            let def = t.cuts[self.shot as usize];
            let u = if t.move_time > 0.0 {
                curve(t.move_curve, (self.game - self.shot_game) / t.move_time)
            } else {
                1.0
            };
            let mix = |a: f32, b: f32| a + (b - a) * u;
            FinishShot {
                index: self.shot as u8,
                subject: def.subject,
                distance: mix(def.distance[0], def.distance[1]) * t.distance_scale,
                rotation: [
                    mix(def.rotation[0], def.rotation_end[0]),
                    mix(def.rotation[1], def.rotation_end[1]),
                    mix(def.rotation[2], def.rotation_end[2]),
                ],
                local_offset: def.local_offset,
                collision: def.collision,
                cut: self.shot_fresh,
            }
        });
        self.shot_fresh = false;
        FinishFrame {
            active: true,
            phase: self.phase,
            rate: self.speed.rate(),
            shot,
            blur: self.blur,
            white: self.white,
            hud_hidden: t.hide_hud && self.kind == FinishKind::Boss,
            input_locked: self.locked,
            cues,
        }
    }

    fn step_boss(&mut self, t: &FinishTuning, real_dt: f32, cues: &mut Vec<FinishCue>) {
        if !self.begun {
            // The frame of the kill: the script's first node chain.
            self.begun = true;
            if t.static_se >= 0 {
                cues.push(FinishCue::Sound {
                    static_se: t.static_se as u32,
                });
            }
            cues.push(FinishCue::BgmFade {
                seconds: t.bgm_fade,
            });
            self.speed.set(CH_EVENT, t.rate, t.interp);
            self.blur = t.blur;
            if t.sound_mode {
                cues.push(FinishCue::SoundState(SOUND_STATE_START));
                self.mode_on = true;
            }
            self.locked = t.lock_input;
        }
        // The game state's tick moves the speed on real time; the world's clock follows it.
        let before = self.speed.rate();
        let rate = self.speed.step(real_dt);
        self.real += real_dt;
        self.game += real_dt * (before + rate) * 0.5;
        match self.phase {
            FinishPhase::Slow | FinishPhase::White => {
                if self.tick_game.is_none() && self.real >= t.cycle - 1.0e-6 {
                    // `TresWaitCycle(1)`: the cameras, the shake; the delays count from here.
                    self.tick_game = Some(self.game);
                    cues.push(FinishCue::StopShakes);
                    if t.shake_scale > 0.0 {
                        cues.push(FinishCue::Shake {
                            scale: t.shake_scale,
                        });
                    }
                }
                let Some(tick) = self.tick_game else { return };
                let since = self.game - tick;
                let count = (t.cut_count as usize).min(t.cuts.len());
                for k in 0..count {
                    if k as i8 > self.shot && since >= t.cuts[k].delay - 1.0e-6 {
                        (self.shot, self.shot_game, self.shot_fresh) = (k as i8, self.game, true);
                    }
                }
                if self.phase == FinishPhase::Slow && since >= t.white_delay - 1.0e-6 {
                    self.phase = FinishPhase::White;
                } else if self.phase == FinishPhase::White {
                    // `WaitBPTresStartWhiteOut(0, time, ApplyGameSpeed)`: the fade runs on game time.
                    self.white = if t.white_time > 0.0 {
                        (self.white + real_dt * rate / t.white_time).min(1.0)
                    } else {
                        1.0
                    };
                }
                if self.phase == FinishPhase::White && (self.white >= 1.0 || !t.white) {
                    // `ResetGlobalGameSpeed(CH_EVENT, 0)`.
                    self.speed.reset(CH_EVENT, 0.0);
                    self.speed.step(0.0);
                    self.phase = FinishPhase::Hold;
                    self.hold = 0.0;
                }
                if !t.white {
                    self.white = 0.0;
                }
            }
            FinishPhase::Hold => {
                self.hold += real_dt;
                if !self.cameras_ended && self.hold >= t.cycle - 1.0e-6 {
                    // +1 tick: `EndAllBPTresCamera`, `AllPrizeCollection`.
                    self.cameras_ended = true;
                    if self.shot >= 0 {
                        cues.push(FinishCue::CamerasEnd);
                    }
                    if t.collect_prizes {
                        cues.push(FinishCue::CollectPrizes);
                    }
                }
                let hold = if t.white { t.hold } else { 0.0 };
                if self.cameras_ended && self.hold >= t.cycle + hold - 1.0e-6 {
                    // The clean-up after `TresDelay(1.0)`: blur off, sound mode off, control back.
                    self.blur = 0.0;
                    if self.mode_on {
                        cues.push(FinishCue::SoundState(SOUND_STATE_END));
                        self.mode_on = false;
                    }
                    if self.locked {
                        cues.push(FinishCue::InputBack);
                        self.locked = false;
                    }
                    self.phase = FinishPhase::FadeIn;
                }
            }
            FinishPhase::FadeIn => {
                self.white = if t.fade_in > 0.0 && t.white {
                    (self.white - real_dt / t.fade_in).max(0.0)
                } else {
                    0.0
                };
                if self.white <= 0.0 {
                    self.end(cues);
                }
            }
            FinishPhase::Idle => {}
        }
    }

    /// The port's last-enemy slow-down: down to `battle_rate`, held, back; no camera, no plate.
    fn step_battle(&mut self, t: &FinishTuning, real_dt: f32, cues: &mut Vec<FinishCue>) {
        if !self.begun {
            self.begun = true;
            self.speed.set(CH_EVENT, t.battle_rate, t.battle_interp);
            cues.push(FinishCue::SoundState(SOUND_SLOW_START));
            self.mode_on = true;
        }
        let rate = self.speed.step(real_dt);
        self.real += real_dt;
        self.game += real_dt * rate;
        match self.phase {
            FinishPhase::Slow => {
                if self.real >= t.battle_interp + t.battle_hold {
                    self.speed.reset(CH_EVENT, t.battle_out);
                    cues.push(FinishCue::SoundState(SOUND_SLOW_END));
                    self.mode_on = false;
                    self.phase = FinishPhase::FadeIn;
                }
            }
            _ => {
                if rate >= 1.0 {
                    self.end(cues);
                }
            }
        }
    }
}
