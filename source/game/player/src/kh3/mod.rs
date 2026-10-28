//! KH3's moves as a layer over the KH2 controller, for the LAN duel's KH3
//! ruleset [T279]. Nothing here runs outside that ruleset.
//!
//! The behaviour follows StrawHatPro's MIT reimplementation of KH3's Sora
//! (credited in CREDITS.md), used as a description only: its gravity and
//! jump arc, Doubleflight, Dodge Roll, Air Slide, Glide, Superslide,
//! flowmotion (wall run, wall kick, pole spin, airstep), attack homing and
//! air hover, the launch, the counter window, focus and shotlock. Every
//! motion, effect and sound is KH2's own (Sora's growth motions, his attack
//! records), so the layer drives `Player`'s action and frame and moves him
//! itself while one of its states runs; attacks, spells, items, Drive and
//! damage stay the KH2 controller's, with KH3's hit-confirm, homing and
//! hover laid over them.
//!
//! The numbers are in `tuning`; walls and poles in `surface`.

pub mod cue;
pub mod hurt;
pub mod surface;
pub mod tuning;

pub use hurt::{Knock, Source, Strike};

use crate::{
    Action, Body, Clips, Collided, Input, Params, Player, Step, Vec3, World, actor_physics, growth,
    ptya,
};
use std::f32::consts::{FRAC_PI_2, PI, TAU};
use surface::{Pole, Wall};
use tuning as t;

/// Sora's attack records the KH3 moves play (PTYA ids): the air finishers
/// stand in for the flow attacks (Sonic Slash, the wall-run attack), the
/// spinning air finisher for Buzz Saw, Upper Slash for the launch (Rising
/// Spiral) and the ground chain's first swing (0x05) for the counter [duel
/// choice: the closest KH2 motions; Retaliating Slash is an air record, cut
/// short on the ground, and Counterguard's swing is a guard window that
/// only clashes]. A counter is known by the action it started
/// (`Kh3::counter_swing`), not by its record, which the chain also plays.
pub mod record {
    pub const SONIC: u8 = 7;
    pub const WALL_ATTACK: u8 = 8;
    pub const SPIN: u8 = 36;
    pub const LAUNCH: u8 = 0x0c;
    pub const COUNTER: u8 = 0x05;
}

/// The two KH3 buttons the KH2 pad does not have.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Pad {
    /// The shotlock button is held (R2).
    pub shotlock: bool,
    /// The situation command (Triangle) was pressed: grand magic.
    pub situation: bool,
}

/// The other Sora: his feet and his target point.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Foe {
    pub position: Vec3,
    pub centre: Vec3,
}

/// What the layer reads of the room this tick.
#[derive(Clone, Copy, Debug, Default)]
pub struct Scene<'a> {
    pub foe: Option<Foe>,
    pub poles: &'a [Pole],
}

/// What a tick asks of the room besides the move.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Outcome {
    pub step: Step,
    /// Shotlock bolts spawned this tick (their serials; their state is in
    /// `Kh3::bolts`).
    pub spawned: Vec<u32>,
    /// A grand magic asked for (the magic type).
    pub grand: Option<u8>,
    /// KH3 moves that began this tick, for the log (and their cues).
    pub began: Vec<&'static str>,
    /// Other moments with a sound or an effect (`cue::EVENTS`): a lock,
    /// a bolt's launch, a step of a wall run.
    pub cues: Vec<&'static str>,
}

/// How a jump left.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Jump {
    /// A fall that no jump began (a ledge, the end of a move).
    #[default]
    None,
    Normal,
    High,
    Double,
    /// The flow jump (out of a wall run, a kick, a pole or a Superslide).
    Super,
}

/// The layer's state.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub enum Mode {
    /// The KH2 controller has him (attacks, spells, items, damage, Drive).
    #[default]
    Kh2,
    Ground,
    Air {
        jump: Jump,
        hang: u32,
        released: bool,
        tick: u32,
    },
    Takeoff {
        tick: u32,
        jump: Jump,
    },
    Land {
        tick: u32,
    },
    Roll {
        tick: u32,
    },
    Slide {
        tick: u32,
        dir: [f32; 2],
    },
    Glide {
        tick: u32,
    },
    Superslide {
        tick: u32,
        pole: Option<[f32; 2]>,
    },
    SlideEnd {
        tick: u32,
        speed: f32,
    },
    WallRun {
        normal: [f32; 2],
        up: bool,
        side: f32,
        tick: u32,
        idle: u32,
    },
    WallKick {
        tick: u32,
        normal: [f32; 2],
        guard: bool,
    },
    Pole {
        pole: Pole,
        angle: f32,
        spin: f32,
        tick: u32,
        jump: bool,
        guard: bool,
    },
    Airstep {
        tick: u32,
        speed: f32,
    },
    HopBack {
        tick: u32,
    },
    Volley {
        tick: u32,
        locks: u8,
        ragnarok: bool,
        base: f32,
        spawned: u8,
    },
    Flow {
        tick: u32,
    },
    Launch {
        tick: u32,
    },
    /// A hit reaction (hurt.rs): a flinch on the ground (`air` false), or
    /// a body in the air; `apex` counts the hang after a toss's top, `fell`
    /// the ticks falling.
    Hurt {
        knock: Knock,
        tick: u32,
        back: bool,
        air: bool,
        high: bool,
        apex: u32,
        fell: u32,
    },
    /// Knocked down and getting up.
    Down {
        tick: u32,
        back: bool,
    },
    /// Aerial Recovery.
    Recover {
        tick: u32,
    },
    /// Final Blow's warp.
    Chase {
        tick: u32,
    },
    /// The duel's Risk Dodge: from, to, and whether it is the warp.
    Risk {
        tick: u32,
        from: Vec3,
        to: Vec3,
        warp: bool,
    },
    /// Focus Syphon's pull.
    Syphon {
        tick: u32,
        locks: u8,
    },
}

impl Mode {
    /// A short name for the log and the snapshot.
    pub fn name(&self) -> &'static str {
        match self {
            Mode::Kh2 => "kh2",
            Mode::Ground => "ground",
            Mode::Air {
                jump: Jump::Double, ..
            } => "doubleflight",
            Mode::Air {
                jump: Jump::Super, ..
            } => "flow_jump",
            Mode::Air { .. } => "air",
            Mode::Takeoff { .. } => "takeoff",
            Mode::Land { .. } => "land",
            Mode::Roll { .. } => "dodge_roll",
            Mode::Slide { .. } => "air_slide",
            Mode::Glide { .. } => "glide",
            Mode::Superslide { .. } => "superslide",
            Mode::SlideEnd { .. } => "slide_end",
            Mode::WallRun { up: true, .. } => "wall_run_up",
            Mode::WallRun { .. } => "wall_run_side",
            Mode::WallKick { .. } => "wall_kick",
            Mode::Pole { .. } => "pole_spin",
            Mode::Airstep { .. } => "airstep",
            Mode::HopBack { .. } => "airstep_hop",
            Mode::Volley { ragnarok: true, .. } => "ragnarok",
            Mode::Volley { .. } => "king_of_hearts",
            Mode::Flow { .. } => "flow_attack",
            Mode::Launch { .. } => "launch",
            Mode::Hurt {
                knock: Knock::Small,
                air: false,
                ..
            } => "flinch",
            Mode::Hurt {
                knock: Knock::Small,
                ..
            } => "air_flinch",
            Mode::Hurt {
                knock: Knock::Blow | Knock::SideSpin,
                ..
            } => "blown",
            Mode::Hurt {
                knock: Knock::Beat, ..
            } => "beaten",
            Mode::Hurt { .. } => "tossed",
            Mode::Down { .. } => "down",
            Mode::Recover { .. } => "aerial_recovery",
            Mode::Chase { .. } => "final_blow",
            Mode::Risk { warp: true, .. } => "risk_warp",
            Mode::Risk { .. } => "risk_dodge",
            Mode::Syphon { .. } => "focus_syphon",
        }
    }
}

/// Shotlock aim (an overlay on the ground and air states: no movement or
/// commands while it holds).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Aim {
    pub tick: u32,
    pub locks: u8,
    timer: f32,
    /// Focus Syphon's aim (the gauge was emptied).
    pub syphon: bool,
}

/// A shotlock bolt (it rides a Wisdom bullet in the room, which draws it
/// and hits with it).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Bolt {
    pub serial: u32,
    pub pos: Vec3,
    pub dir: Vec3,
    pub speed: f32,
    pub ragnarok: bool,
    /// Its place in the ring and the ring's size.
    slot: u8,
    count: u8,
    /// Ticks since it spawned, and since it launched.
    age: u32,
    flight: Option<u32>,
    homing: bool,
}

/// One Sora's KH3 state.
#[derive(Clone, Debug, PartialEq)]
pub struct Kh3 {
    pub mode: Mode,
    /// Velocity (x, y up, z), units a tick, while the layer moves him.
    pub velocity: Vec3,
    /// The flow jump's push (XZ), decaying.
    push: [f32; 2],
    pub focus: f32,
    pub aim: Option<Aim>,
    shot_hold: u32,
    pub bolts: Vec<Bolt>,
    bolt_serial: u32,
    /// The target for bolts and homing (the other Sora's target point).
    pub foe: Option<Vec3>,
    /// Doubleflight is available this airtime.
    double: bool,
    slide_cooldown: u32,
    run_ticks: u32,
    /// The counter window (a guard or a dodge met a hit).
    counter: u32,
    /// The launch was used in this chain; air swings since leaving the
    /// ground (the first homes faster).
    launched: bool,
    air_swings: u32,
    traction: f32,
    hover: f32,
    /// What the layer last showed (a reaction that replaces it hands him
    /// back to the KH2 controller).
    shown: Option<Action>,
    /// Presentation: pitch and roll after the facing.
    pub lean: [f32; 2],
    lean_target: [f32; 2],
    /// Grand magic: the style gauge, the elements' points (Fire, Blizzard,
    /// Thunder), the offer and its time left, the gauge's idle time.
    pub style: f32,
    points: [f32; 3],
    pub offer: Option<(u8, u32)>,
    style_idle: u32,
    /// Damage a volley has dealt (its cap).
    pub volley_dealt: i32,
    /// The shotlock button has aimed (or fired) since it was pressed: it
    /// must be let go before it aims again.
    shot_spent: bool,
    /// The last pole left (a Superslide out of it cannot grab it again).
    last_pole: Option<[f32; 2]>,
    /// The flow attack running (its record, and whether it is Final
    /// Blow's chase) and its hits landed so far.
    flow: Option<(u8, bool)>,
    flow_hits: u8,
    /// Final Blow is offered (ticks left).
    chase_offer: u32,
    /// The duel's Risk Dodge is offered: ticks left, the attacker's feet,
    /// whether he is in the air.
    risk_offer: Option<(u32, Vec3, bool)>,
    /// The focus gauge was emptied and has not been full since: the
    /// shotlock is Focus Syphon.
    pub focus_empty: bool,
    /// Presentation cues of this tick that the tick itself cannot see
    /// (a reaction begun by a hit, a lock), handed out with the next
    /// outcome.
    pending: Vec<&'static str>,
    /// Ticks until a bolt's hit on him shows its spark again (a volley's
    /// bolts land in bursts).
    spark_wait: u32,
    /// The swing a counter started (its record is also a chain's).
    counter_swing: Option<Action>,
    /// A record started this tick outside the KH2 controller's own tick
    /// (its swing's hit windows are new: the room keys them by the start).
    record_started: bool,
}

impl Default for Kh3 {
    fn default() -> Self {
        Kh3 {
            mode: Mode::Kh2,
            velocity: [0.; 3],
            push: [0.; 2],
            focus: t::FOCUS_MAX,
            aim: None,
            shot_hold: 0,
            bolts: Vec::new(),
            bolt_serial: 0,
            foe: None,
            double: false,
            slide_cooldown: 0,
            run_ticks: 0,
            counter: 0,
            launched: false,
            air_swings: 0,
            traction: 0.,
            hover: 0.,
            shown: None,
            lean: [0.; 2],
            lean_target: [0.; 2],
            style: 0.,
            points: [0.; 3],
            offer: None,
            style_idle: 0,
            volley_dealt: 0,
            shot_spent: false,
            last_pole: None,
            flow: None,
            flow_hits: 0,
            chase_offer: 0,
            risk_offer: None,
            focus_empty: false,
            pending: Vec::new(),
            spark_wait: 0,
            counter_swing: None,
            record_started: false,
        }
    }
}

/// The KH2 actions the layer takes over: locomotion and the growth motions.
pub fn free(action: Action) -> bool {
    matches!(
        action,
        Action::Idle
            | Action::Walk
            | Action::Run
            | Action::Jump
            | Action::HighJump
            | Action::Fall
            | Action::Land
            | Action::Growth(_)
    )
}

fn wrap(a: f32) -> f32 {
    (a + PI).rem_euclid(TAU) - PI
}

fn yaw_of(d: [f32; 2]) -> f32 {
    d[0].atan2(d[1])
}

fn facing(yaw: f32) -> [f32; 2] {
    [yaw.sin(), yaw.cos()]
}

/// The yaw `share` of the way from `yaw` to `to`.
fn turn(yaw: f32, to: f32, share: f32) -> f32 {
    wrap(yaw + wrap(to - yaw) * share.clamp(0., 1.))
}

/// The stick's magnitude and unit direction.
fn stick(input: &Input) -> (f32, [f32; 2]) {
    let m = input.direction[0].hypot(input.direction[1]).min(1.);
    if m < 1e-6 {
        (0., [0.; 2])
    } else {
        let l = input.direction[0].hypot(input.direction[1]);
        (m, [input.direction[0] / l, input.direction[1] / l])
    }
}

fn len3(v: Vec3) -> f32 {
    (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt()
}

fn unit3(v: Vec3) -> Vec3 {
    let l = len3(v);
    if l > 1e-6 { v.map(|c| c / l) } else { v }
}

/// The Dodge Roll's root distance (units) at a tick.
fn roll_at(tick: u32) -> f32 {
    let f = tick as f32 * 0.5;
    let i = (f as usize).min(t::ROLL_KEYS.len() - 1);
    let j = (i + 1).min(t::ROLL_KEYS.len() - 1);
    let k = f - i as f32;
    (t::ROLL_KEYS[i] * (1. - k) + t::ROLL_KEYS[j] * k) * t::SCALE
}

/// The Air Slide's share of its distance at a tick.
fn slide_share(tick: u32) -> f32 {
    let u = (tick as f32 / t::SLIDE_TICKS as f32).clamp(0., 1.);
    1. - (1. - u).powf(t::SLIDE_CURVE)
}

/// The airstep's speed (units a tick) for a leg of `d` units.
pub fn airstep_speed(d: f32) -> f32 {
    let cm = d / t::SCALE;
    let v0 = (2. * cm / (t::AIRSTEP_LEG / 60.)).max((60. * cm).min(t::AIRSTEP_FAST));
    let v = if cm >= t::AIRSTEP_NEAR {
        v0
    } else {
        v0.min(t::AIRSTEP_NEAR_SPEED)
    };
    v * t::SCALE / 60.
}

impl Kh3 {
    /// A state of the layer's own is running.
    pub fn owns(&self) -> bool {
        self.mode != Mode::Kh2
    }

    /// Light hits do not flinch him: the Superslide, the kick, the pole,
    /// the flow attacks and the flow jump's rise.
    pub fn armoured(&self) -> bool {
        matches!(
            self.mode,
            Mode::Superslide { .. }
                | Mode::SlideEnd { .. }
                | Mode::WallKick { .. }
                | Mode::Pole { .. }
                | Mode::Flow { .. }
        ) || matches!(self.mode, Mode::Air { jump: Jump::Super, .. } if self.velocity[1] > 0.)
    }

    /// The layer's mode code for the snapshot (its index in `MODE_NAMES`).
    pub fn mode_code(&self) -> u8 {
        MODE_NAMES
            .iter()
            .position(|n| *n == self.mode.name())
            .unwrap_or(0) as u8
    }

    /// Whether a hit should not flinch him now: armour against a light
    /// reaction class (the KH2 classes up to 3; a blow always lands).
    pub fn shrugs(&self, class: u8) -> bool {
        self.armoured() && class <= 3
    }

    /// A hit met a Dodge Roll or an Air Slide [duel choice: the duel's
    /// Risk Dodge]: the counter window opens.
    pub fn dodged(&mut self) {
        if matches!(self.mode, Mode::Roll { .. } | Mode::Slide { .. }) {
            self.counter = t::COUNTER_WINDOW;
        }
    }

    /// His swing or spell landed: focus (a small prize) and style.
    pub fn landed(&mut self, swing: bool) {
        self.focus = (self.focus + t::FOCUS_HIT_DEALT).min(t::FOCUS_MAX);
        if swing {
            self.add_style(t::STYLE_SWING, None);
        }
    }

    /// He was hit: focus (a large prize).
    pub fn hit_taken(&mut self) {
        self.focus = (self.focus + t::FOCUS_HIT_TAKEN).min(t::FOCUS_MAX);
    }

    /// A spell of his left his hand: its element's style points
    /// [duel choice: counted at the release, not the hit].
    pub fn spell_released(&mut self, magic_type: u8) {
        let (points, element) = match magic_type {
            crate::magic::FIRE => (t::STYLE_FIRE, Some(0)),
            crate::magic::BLIZZARD => (t::STYLE_BLIZZARD, Some(1)),
            crate::magic::THUNDER => (t::STYLE_OTHER, Some(2)),
            _ => (t::STYLE_OTHER, None),
        };
        self.add_style(points, element);
    }

    fn add_style(&mut self, points: f32, element: Option<usize>) {
        self.style += points;
        self.style_idle = 0;
        if let Some(e) = element {
            self.points[e] += points;
        }
        if self.style >= t::STYLE_MAX {
            self.style = 0.;
            // [duel choice] The element with the most points is offered
            // (the reference rolls by them).
            let best = (0..3)
                .filter(|&e| self.points[e] > 0.)
                .max_by(|&a, &b| self.points[a].total_cmp(&self.points[b]));
            if let Some(e) = best {
                let spell = [
                    crate::magic::FIRE,
                    crate::magic::BLIZZARD,
                    crate::magic::THUNDER,
                ][e];
                self.offer = Some((spell, t::OFFER_TICKS));
                self.pending.push("grand_offer");
            }
            self.points = [0.; 3];
        }
    }

    /// The grand magic asked for was cast.
    pub fn grand_used(&mut self) {
        self.offer = None;
    }

    /// The counter window is open (a guard or a dodge met a hit).
    pub fn counter_open(&self) -> bool {
        self.counter > 0
    }

    /// Ticks left of the counter window.
    pub fn counter_ticks(&self) -> u32 {
        self.counter
    }

    /// Final Blow is offered.
    pub fn chase_open(&self) -> bool {
        self.chase_offer > 0
    }

    /// The Risk Dodge is offered.
    pub fn risk_open(&self) -> bool {
        self.risk_offer.is_some()
    }

    /// A bolt hit him: whether its spark and sound play (at most once in
    /// `cue::SPARK_EVERY` ticks, so a volley's burst does not stack).
    pub fn spark_ready(&mut self) -> bool {
        let ready = self.spark_wait == 0;
        if ready {
            self.spark_wait = cue::SPARK_EVERY;
        }
        ready
    }

    /// A bolt hit (the room removed its bullet).
    pub fn bolt_hit(&mut self, serial: u32) {
        self.bolts.retain(|b| b.serial != serial);
    }

    /// The joiner's copy of the host's numbers (the snapshot).
    pub fn apply_shown(&mut self, shown: &Shown) {
        self.focus = f32::from(shown.focus);
        self.style = f32::from(shown.style);
        self.lean = shown.lean;
        self.aim = shown.aiming.then(|| Aim {
            locks: shown.locks,
            syphon: shown.syphon,
            ..Aim::default()
        });
        self.offer = (shown.offer != 0).then_some((shown.offer, 1));
        self.counter = u32::from(shown.counter);
        self.chase_offer = u32::from(shown.chase);
        self.risk_offer = shown.risk.then_some((1, [0.; 3], false));
        self.mode = MODE_NAMES
            .get(usize::from(shown.mode))
            .and_then(|n| mode_named(n))
            .unwrap_or(Mode::Kh2);
    }

    /// The numbers the joiner needs.
    pub fn shown(&self) -> Shown {
        Shown {
            focus: self.focus.round().clamp(0., 255.) as u8,
            style: self.style.round().clamp(0., 255.) as u8,
            mode: self.mode_code(),
            locks: self.aim.map_or(0, |a| a.locks),
            aiming: self.aim.is_some(),
            syphon: self.aim.is_some_and(|a| a.syphon),
            counter: self.counter_open(),
            chase: self.chase_open(),
            risk: self.risk_open(),
            offer: self.offer.map_or(0, |o| o.0),
            lean: self.lean,
        }
    }

    /// One tick of a Sora in the KH3 ruleset, in place of `Player::tick`.
    #[allow(clippy::too_many_arguments)]
    pub fn tick(
        &mut self,
        player: &mut Player,
        input: &Input,
        pad: &Pad,
        p: &Params,
        clips: &Clips,
        world: &impl World,
        scene: &Scene,
    ) -> Outcome {
        let mut out = Outcome::default();
        out.began.append(&mut self.pending);
        self.foe = scene.foe.map(|f| f.centre);
        self.gauges();
        // A reaction, a death, a cast or a form change replaced what the
        // layer showed: the KH2 controller has him.
        if self.owns() && self.shown != Some(player.action) {
            self.release();
        }
        if pad.situation
            && let Some((spell, _)) = self.offer
            && !player.dead()
        {
            out.grand = Some(spell);
        }
        self.shotlock_button(player, input, pad, scene, &mut out);
        let before = self.mode;
        match self.mode {
            Mode::Kh2 => self.kh2(player, input, p, clips, world, scene, &mut out),
            _ => self.own(player, input, p, clips, world, scene, &mut out),
        }
        if self.mode != before
            && std::mem::discriminant(&self.mode) != std::mem::discriminant(&before)
        {
            let name = self.mode.name();
            if !matches!(name, "kh2" | "ground" | "air" | "land" | "takeoff") {
                out.began.push(name);
            }
        }
        out.cues.extend(cue::periodic(&self.mode));
        if std::mem::take(&mut self.record_started) {
            out.step.started = Some(player.action);
        }
        self.step_bolts();
        for k in 0..2 {
            self.lean[k] += (self.lean_target[k] - self.lean[k]) * t::LEAN_EASE;
        }
        out
    }

    /// The gauges and timers of a tick.
    fn gauges(&mut self) {
        // Emptied (by aiming), the gauge stays "empty" until it is full.
        if self.focus <= 0. {
            self.focus_empty = true;
        } else if self.focus >= t::FOCUS_MAX {
            self.focus_empty = false;
        }
        self.focus = (self.focus + t::FOCUS_REGEN).min(t::FOCUS_MAX);
        self.slide_cooldown = self.slide_cooldown.saturating_sub(1);
        self.counter = self.counter.saturating_sub(1);
        self.style_idle += 1;
        if self.style_idle > t::STYLE_IDLE {
            self.style = (self.style - t::STYLE_DECAY).max(0.);
        }
        if let Some((spell, left)) = self.offer {
            self.offer = (left > 1).then_some((spell, left - 1));
        }
        self.chase_offer = self.chase_offer.saturating_sub(1);
        self.spark_wait = self.spark_wait.saturating_sub(1);
        if let Some((left, at, air)) = self.risk_offer {
            self.risk_offer = (left > 1).then_some((left - 1, at, air));
        }
    }

    /// Back to the KH2 controller (the layer's motion is replaced).
    fn release(&mut self) {
        self.mode = Mode::Kh2;
        self.aim = None;
        self.shown = None;
        self.lean_target = [0.; 2];
    }

    /// Shows `action` (a missing growth motion falls back to the fall or
    /// the run): the frame runs on while it stays, a new one starts at 0.
    fn show(&mut self, player: &mut Player, action: Action, clips: &Clips, step: &mut Step) {
        let action = match action {
            Action::Hurt(i) if i >= clips.hurt.len() => {
                if self.airborne() {
                    Action::Fall
                } else {
                    Action::Idle
                }
            }
            Action::Growth(m) if clips.growth_clip(m).is_none() => {
                if self.airborne() {
                    Action::Fall
                } else {
                    Action::Run
                }
            }
            Action::HighJump if clips.growth_clip(growth::motion::HIGH_JUMP).is_none() => {
                Action::Jump
            }
            a => a,
        };
        if player.action == action && self.shown == Some(action) {
            player.frame = player.frame.saturating_add(1);
        } else {
            player.action = action;
            player.frame = 0;
            player.blend = crate::blend_for(action);
            step.started = Some(action);
        }
        player.armed = true;
        self.shown = Some(action);
    }

    /// Whether the layer's state is in the air.
    fn airborne(&self) -> bool {
        !matches!(
            self.mode,
            Mode::Ground
                | Mode::Land { .. }
                | Mode::Roll { .. }
                | Mode::Kh2
                | Mode::Down { .. }
                | Mode::Hurt { air: false, .. }
                | Mode::Risk { warp: false, .. }
                | Mode::Syphon { .. }
        ) && !matches!(self.mode, Mode::Takeoff { jump, .. } if jump != Jump::Double)
    }

    /// One body-against-map move.
    fn travel(
        player: &mut Player,
        disp: Vec3,
        airborne: bool,
        p: &Params,
        world: &impl World,
    ) -> Collided {
        let body = Body {
            radius: p.radius,
            height: p.height,
            step: p.step_height,
        };
        // KH3's fast moves (Superslide, airstep) go farther in a tick than a
        // body is wide: in steps of at most half the radius, so no thin
        // wall is stepped through.
        let reach = disp[0].hypot(disp[2]);
        let steps = ((reach / (p.radius * 0.5).max(1.)).ceil() as usize).clamp(1, 64);
        let part = disp.map(|c| c / steps as f32);
        let mut moved = Collided {
            position: player.position,
            on_floor: !airborne,
        };
        let mut landed = false;
        for _ in 0..steps {
            moved = world.collide(player.position, part, airborne, &body);
            player.position = moved.position;
            landed |= airborne && moved.on_floor;
            if !airborne && !moved.on_floor {
                // Walked off a ledge: the rest is the fall's.
                break;
            }
        }
        if airborne {
            moved.on_floor = landed;
        }
        moved
    }

    /// The body's span for wall probes at the feet height `y`.
    fn span(y: f32, p: &Params) -> (f32, f32) {
        (y + p.step_height, y + p.height)
    }

    /// The KH2 side of the tick: the room's own controller with KH3's
    /// hit-confirm, launch, counter, cancels, homing and air hover.
    #[allow(clippy::too_many_arguments)]
    fn kh2(
        &mut self,
        player: &mut Player,
        input: &Input,
        p: &Params,
        clips: &Clips,
        world: &impl World,
        scene: &Scene,
        out: &mut Outcome,
    ) {
        if free(player.action) && !player.dead() {
            self.take_over(player, clips);
            self.own(player, input, p, clips, world, scene, out);
            return;
        }
        let (magnitude, dir) = stick(input);
        let swinging = matches!(player.action, Action::Attack(_) | Action::AirAttack(_));
        let finisher = match player.action {
            Action::Attack(i) | Action::AirAttack(i) => clips
                .records
                .get(i)
                .is_some_and(|r| r.flags & ptya::flag::FINISHER != 0),
            _ => false,
        };
        // A successful guard opens the counter window.
        if player.action == Action::GuardRecoil && player.frame <= 1 {
            self.counter = t::COUNTER_WINDOW;
        }
        // The duel's Risk Dodge out of a guarded heavy hit.
        if input.jump
            && matches!(player.action, Action::GuardRecoil | Action::Guard)
            && self.start_risk(player, clips, &mut out.step)
        {
            return;
        }
        // Final Blow: the next attack after a counter that blew him away,
        // once the counter's swing may be left.
        if input.attack
            && self.chase_offer > 0
            && (Self::recovered(player, clips) || (swinging && self.cancel_open(player, clips)))
            && self.start_chase(player, scene, clips, &mut out.step)
        {
            return;
        }
        if input.attack
            && self.counter > 0
            && matches!(player.action, Action::GuardRecoil | Action::Guard)
            && self.play_record(player, record::COUNTER, input, clips)
        {
            self.counter = 0;
            out.began.push("counter");
            return;
        }
        // The launch: Jump in a ground chain that has landed a hit.
        if input.jump
            && matches!(player.action, Action::Attack(_))
            && player.connected
            && !finisher
            && !self.launched
            && self.play_record(player, record::LAUNCH, input, clips)
        {
            self.launched = true;
            player.combo = ptya::Combo::default();
            self.mode = Mode::Launch { tick: 0 };
            self.shown = Some(player.action);
            return;
        }
        // A swing is cancelled by a dodge once its cancel window opens.
        if swinging
            && input.guard
            && magnitude >= t::STICK_ACTIVE
            && !finisher
            && self.cancel_open(player, clips)
        {
            if player.in_air() {
                if self.slide_cooldown == 0 {
                    self.velocity = [0.; 3];
                    self.start_slide(player, dir, clips, &mut out.step);
                    return;
                }
            } else {
                self.start_roll(player, dir, clips, &mut out.step);
                return;
            }
        }
        // Out of a swing's or a reaction's recovery, Jump is the layer's jump.
        if input.jump && !player.in_air() && Self::recovered(player, clips) {
            self.take_over(player, clips);
            self.mode = Mode::Ground;
            self.start_jump(player, clips, &mut out.step);
            return;
        }
        let mut inp = *input;
        inp.jump = false;
        // Hit-confirm: a chain goes on only once one of its hits landed.
        if swinging && !player.connected {
            inp.attack = false;
        }
        // The layer's own buttons are not KH2's: a dodge with the stick is
        // the layer's, and jumps are only the KH2 controller's out of its
        // own states.
        if magnitude >= t::STICK_ACTIVE {
            inp.guard = false;
        }
        let before = player.position;
        let started = player.action;
        out.step = player.tick(&inp, p, clips, world);
        if out.step.started.is_some() {
            self.counter_swing = None;
        }
        if out.step.started.is_some() && matches!(started, Action::Attack(_) | Action::AirAttack(_))
        {
            self.traction = 0.;
        }
        self.after_kh2(player, before, p, clips, world, scene);
    }

    /// A KH2 motion past its "blend to idle" tick (its recovery).
    fn recovered(player: &Player, clips: &Clips) -> bool {
        matches!(
            player.action,
            Action::Attack(_)
                | Action::Cast(_)
                | Action::Guard
                | Action::GuardRecoil
                | Action::WallRecoil
                | Action::Item
                | Action::Hurt(_)
        ) && !player.dead()
            && player
                .clip(clips)
                .blend_to_idle
                .is_some_and(|b| player.frame >= b)
    }

    /// The cancel window of a swing: from its first combo window [duel
    /// choice: KH2's "allow combo" range stands for KH3's 0.1333 s after the
    /// first hit window].
    fn cancel_open(&self, player: &Player, clips: &Clips) -> bool {
        let clip = player.clip(clips);
        clip.combo_windows
            .first()
            .is_some_and(|w| player.frame >= w.0)
    }

    /// KH3's homing and hover over the KH2 controller's swing.
    fn after_kh2(
        &mut self,
        player: &mut Player,
        before: Vec3,
        p: &Params,
        clips: &Clips,
        world: &impl World,
        scene: &Scene,
    ) {
        let air = matches!(player.action, Action::AirAttack(_) | Action::AirCast(_));
        let swing = matches!(player.action, Action::Attack(_) | Action::AirAttack(_));
        if player.frame == 0 && matches!(player.action, Action::AirAttack(_)) {
            self.air_swings += 1;
            self.hover = 0.;
        }
        let frame = player.frame;
        if swing && let Some(foe) = scene.foe {
            let first_air = air && self.air_swings <= 1;
            let (speed, rate) = if first_air {
                (t::TRACTION_AIR_FIRST, t::TRACTION_RATE_AIR_FIRST)
            } else {
                (t::TRACTION, t::TRACTION_RATE)
            };
            let d = [
                foe.position[0] - player.position[0],
                foe.position[2] - player.position[2],
            ];
            let dist = (d[0].hypot(d[1]) - t::length_of(50.)).max(0.);
            let window = (t::TRACTION_FROM..=t::TRACTION_TO).contains(&frame);
            // A counter or the launch closes right in [duel choice: their KH2
            // records reach less far than the reference's swings].
            let stop = if self.counter_swing.is_some() {
                t::CLOSE_STOP
            } else {
                t::TRACTION_STOP
            };
            let target = if window && dist > stop && dist <= t::TRACTION_RANGE {
                speed
            } else {
                0.
            };
            let k = if target > 0. { rate } else { 0.5 };
            self.traction += (target - self.traction) * k;
            if self.traction > 0.05 {
                let l = d[0].hypot(d[1]).max(1e-3);
                let step = self.traction.min((dist - stop).max(0.));
                Self::travel(
                    player,
                    [d[0] / l * step, 0., d[1] / l * step],
                    player.in_air(),
                    p,
                    world,
                );
            }
        }
        // Air-mode swings and casts hover: the height closes on the
        // target's during the window, then a slow fall.
        if air && player.in_air() {
            let vy = if frame <= t::TRACTION_TO {
                let dz = self.foe.map_or(0., |c| c[1] - (before[1] + t::CHEST));
                if swing {
                    dz.clamp(-t::HOVER_CLAMP, t::HOVER_CLAMP) * t::HOVER_GAIN / 60.
                } else {
                    0.
                }
            } else {
                self.hover -= t::HOVER_FALL;
                self.hover
            };
            let mut y = before[1] + vy;
            if let Some(floor) = world.floor(
                player.position[0],
                player.position[2],
                before[1] + p.step_height,
            ) {
                y = y.max(floor);
            }
            player.position[1] = y;
            player.arc = actor_physics::Arc::fall(y, 0.);
            player.vertical_speed = vy;
            self.velocity = [0., vy, 0.];
        }
        let _ = clips;
    }

    /// Takes him over from the KH2 controller's locomotion.
    fn take_over(&mut self, player: &mut Player, clips: &Clips) {
        self.traction = 0.;
        self.shown = Some(player.action);
        if player.action == Action::Land {
            self.velocity = [0.; 3];
            self.mode = Mode::Land {
                tick: player.frame.min(clips.land.end),
            };
        } else if player.in_air() {
            self.velocity = [
                player.air_velocity[0],
                player.vertical_speed.min(0.),
                player.air_velocity[1],
            ];
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

    /// Plays an attack record now (the KH2 controller's swing, with its hop
    /// and hit windows).
    fn play_record(&mut self, player: &mut Player, id: u8, input: &Input, clips: &Clips) -> bool {
        if ptya::find(&clips.records, id).is_none() {
            return false;
        }
        // The reference's attacks snap to the target on their first tick.
        let yaw = player.yaw;
        if let Some(f) = self.foe {
            let d = [f[0] - player.position[0], f[2] - player.position[2]];
            if d[0].hypot(d[1]) > 1. {
                player.yaw = yaw_of(d);
            }
        }
        if !player.ptya_attack(id, input, clips) {
            player.yaw = yaw;
            return false;
        }
        player.connected = false;
        player.invulnerable = 0.;
        self.traction = 0.;
        self.aim = None;
        self.counter_swing = (id == record::COUNTER).then_some(player.action);
        self.record_started = true;
        true
    }

    /// Hands a command to the KH2 controller from one of the layer's
    /// states: on success the KH2 state takes over; otherwise nothing of it
    /// stays.
    #[allow(clippy::too_many_arguments)]
    fn command(
        &mut self,
        player: &mut Player,
        input: &Input,
        p: &Params,
        clips: &Clips,
        world: &impl World,
        scene: &Scene,
        out: &mut Outcome,
    ) -> bool {
        let saved = player.clone();
        let air = self.airborne();
        if air {
            if !player.action.airborne() {
                player.action = Action::Fall;
            }
            player.arc = actor_physics::Arc::fall(player.position[1], self.velocity[1]);
            player.air_velocity = [self.velocity[0], self.velocity[2]];
            player.vertical_speed = self.velocity[1];
            player.pending_jump = None;
            player.sampler.reset();
        } else if !matches!(player.action, Action::Idle | Action::Walk | Action::Run) {
            player.action = Action::Idle;
        }
        let inp = Input {
            jump: false,
            jump_up: true,
            ..*input
        };
        let before = player.position;
        let step = player.tick(&inp, p, clips, world);
        if free(player.action) {
            *player = saved;
            return false;
        }
        out.step = step;
        self.mode = Mode::Kh2;
        self.counter_swing = None;
        self.shown = None;
        self.aim = None;
        self.lean_target = [0.; 2];
        self.traction = 0.;
        if matches!(player.action, Action::AirAttack(_)) {
            self.air_swings = 0;
        }
        self.after_kh2(player, before, p, clips, world, scene);
        true
    }

    /// The layer's own states.
    #[allow(clippy::too_many_arguments)]
    fn own(
        &mut self,
        player: &mut Player,
        input: &Input,
        p: &Params,
        clips: &Clips,
        world: &impl World,
        scene: &Scene,
        out: &mut Outcome,
    ) {
        player.mp.tick(1., player.abilities.mp_haste);
        player.mp_charging = player.mp.charging();
        player.invulnerable = (player.invulnerable - 1.).max(0.);
        // The KH2 command lockout (frame trigger 16) runs out here too, as
        // in the KH2 controller's own tick: else a swing's lockout would
        // outlive it and refuse every attack from the layer's states.
        player.command_lockout(clips);
        // Shotlock aim takes the stick and the buttons.
        let aiming = self.aim.is_some();
        let free_input = Input {
            direction: [0.; 2],
            jump: false,
            attack: false,
            guard: false,
            ..*input
        };
        let input = if aiming { &free_input } else { input };
        match self.mode {
            Mode::Kh2 => {}
            Mode::Ground => self.ground(player, input, p, clips, world, scene, out),
            Mode::Air { .. } => self.air(player, input, p, clips, world, scene, out),
            Mode::Takeoff { tick, jump } => {
                let tick = tick + 1;
                let air = jump == Jump::Double;
                let leave = if air {
                    t::DOUBLEFLIGHT_TAKEOFF
                } else {
                    t::JUMP_TAKEOFF
                };
                if air {
                    self.velocity[1] = (self.velocity[1] - t::GRAVITY).max(-t::FALL_MAX);
                    let motion = Action::Growth(growth::motion::AERIAL_DODGE);
                    self.show(player, motion, clips, &mut out.step);
                } else {
                    // One braking tick out of a run.
                    self.velocity[0] *= 1. - t::BRAKE_FRICTION;
                    self.velocity[2] *= 1. - t::BRAKE_FRICTION;
                    let motion = if jump == Jump::High {
                        Action::HighJump
                    } else {
                        Action::Jump
                    };
                    self.show(player, motion, clips, &mut out.step);
                }
                if tick >= leave {
                    self.velocity[1] = match jump {
                        Jump::High => t::HIGH_JUMP,
                        Jump::Double => t::DOUBLEFLIGHT,
                        _ => t::JUMP,
                    };
                    self.mode = Mode::Air {
                        jump,
                        hang: 0,
                        released: false,
                        tick: 0,
                    };
                } else {
                    self.mode = Mode::Takeoff { tick, jump };
                }
                let v = self.velocity;
                let moved = Self::travel(
                    player,
                    if air { v } else { [v[0], 0., v[2]] },
                    air,
                    p,
                    world,
                );
                if air && moved.on_floor && self.velocity[1] <= 0. {
                    self.land(player, clips, &mut out.step);
                } else if !air && !moved.on_floor {
                    self.fall(player, [v[0], 0., v[2]], clips, &mut out.step);
                }
            }
            Mode::Land { tick } => {
                let tick = tick + 1;
                self.mode = Mode::Land { tick };
                self.velocity = [0.; 3];
                if tick >= t::LAND_JUMP && input.jump {
                    self.start_jump(player, clips, &mut out.step);
                    return;
                }
                if tick >= t::LAND_CONTROL {
                    let (m, _) = stick(input);
                    if m >= t::STICK_ACTIVE || input.attack || input.guard || tick >= clips.land.end
                    {
                        self.mode = Mode::Ground;
                        self.ground(player, input, p, clips, world, scene, out);
                        return;
                    }
                }
                self.show(player, Action::Land, clips, &mut out.step);
            }
            Mode::Roll { tick } => {
                let tick = tick + 1;
                self.mode = Mode::Roll { tick };
                if tick >= t::ROLL_CANCEL {
                    if input.jump {
                        self.start_jump(player, clips, &mut out.step);
                        return;
                    }
                    if self.counter > 0
                        && input.attack
                        && self.play_record(player, record::COUNTER, input, clips)
                    {
                        self.counter = 0;
                        self.mode = Mode::Kh2;
                        out.began.push("counter");
                        return;
                    }
                    // Attack, or Guard with the stick idle (roll to roll never).
                    let guard = input.guard && stick(input).0 < t::STICK_ACTIVE;
                    if (input.attack || guard)
                        && self.command(player, input, p, clips, world, scene, out)
                    {
                        return;
                    }
                }
                let d = roll_at(tick) - roll_at(tick - 1);
                let f = facing(player.yaw);
                self.show(
                    player,
                    Action::Growth(growth::motion::DODGE_ROLL),
                    clips,
                    &mut out.step,
                );
                let moved = Self::travel(player, [f[0] * d, 0., f[1] * d], false, p, world);
                if !moved.on_floor {
                    self.fall(player, [f[0] * d, 0., f[1] * d], clips, &mut out.step);
                } else if tick >= t::ROLL_TICKS {
                    self.velocity = [0.; 3];
                    self.mode = Mode::Ground;
                }
            }
            Mode::Slide { tick, dir } => {
                self.slide(player, input, tick, dir, p, clips, world, scene, out)
            }
            Mode::Glide { tick } => self.glide(player, input, tick, p, clips, world, scene, out),
            Mode::Superslide { tick, pole } => {
                self.superslide(player, input, tick, pole, p, clips, world, scene, out)
            }
            Mode::SlideEnd { tick, speed } => {
                let tick = tick + 1;
                let v = speed * (1. - tick as f32 / t::SLIDE_END_TICKS as f32).max(0.);
                let f = facing(player.yaw);
                self.velocity = [f[0] * v, 0., f[1] * v];
                self.show(
                    player,
                    Action::Growth(growth::motion::GLIDE),
                    clips,
                    &mut out.step,
                );
                let moved = Self::travel(player, self.velocity, true, p, world);
                if moved.on_floor {
                    self.land(player, clips, &mut out.step);
                } else if tick >= t::SLIDE_END_TICKS {
                    self.fall(player, [0.; 3], clips, &mut out.step);
                    self.slide_cooldown = t::SLIDE_COOLDOWN;
                } else {
                    self.mode = Mode::SlideEnd { tick, speed };
                }
            }
            Mode::WallRun { .. } => self.wall_run(player, input, p, clips, world, scene, out),
            Mode::WallKick { .. } => self.wall_kick(player, input, p, clips, world, scene, out),
            Mode::Pole { .. } => self.pole(player, input, p, clips, world, scene, out),
            Mode::Airstep { .. } => self.airstep(player, input, p, clips, world, scene, out),
            Mode::HopBack { tick } => {
                let tick = tick + 1;
                self.mode = Mode::HopBack { tick };
                player.invulnerable = player.invulnerable.max(2.);
                if (t::HOP_INPUT..t::HOP_CHANGE).contains(&tick) {
                    if input.attack && self.start_flow(player, record::SONIC, input, clips) {
                        return;
                    }
                    if input.guard {
                        self.start_superslide(player, None, scene, clips, &mut out.step);
                        return;
                    }
                }
                let n = t::HOP_TICKS as f32;
                let f = facing(player.yaw);
                let back = t::HOP_BACK / n;
                // The rise eases out over the hop.
                let up = t::HOP_UP * 2. * (1. - (tick as f32 - 0.5) / n).max(0.) / n;
                self.velocity = [-f[0] * back, up, -f[1] * back];
                self.show(
                    player,
                    Action::Growth(growth::motion::AERIAL_DODGE),
                    clips,
                    &mut out.step,
                );
                let moved = Self::travel(player, self.velocity, true, p, world);
                if moved.on_floor {
                    self.land(player, clips, &mut out.step);
                } else if tick >= t::HOP_CHANGE {
                    self.fall(player, [0.; 3], clips, &mut out.step);
                }
            }
            Mode::Volley { .. } => self.volley(player, p, clips, world, scene, out),
            Mode::Hurt { .. } => self.hurt_tick(player, input, p, clips, world, scene, out),
            Mode::Down { .. } => self.down_tick(player, input, p, clips, world, scene, out),
            Mode::Recover { .. } => self.recover_tick(player, input, p, clips, world, scene, out),
            Mode::Chase { .. } => self.chase_tick(player, input, p, clips, world, scene, out),
            Mode::Risk { .. } => self.risk_tick(player, p, clips, world, out),
            Mode::Syphon { .. } => self.syphon(player, p, clips, world, out),
            Mode::Flow { tick } => {
                let tick = tick + 1;
                self.mode = Mode::Flow { tick };
                player.frame = player.frame.saturating_add(1);
                let action = player.action;
                self.shown = Some(action);
                let mut v = [0f32; 3];
                if let Some(foe) = scene.foe {
                    let d = [
                        foe.position[0] - player.position[0],
                        foe.position[2] - player.position[2],
                    ];
                    let l = d[0].hypot(d[1]);
                    if tick <= t::FLOW_DASH && l > t::FLOW_STOP {
                        let s = t::FLOW_SPEED.min(l - t::FLOW_STOP);
                        v[0] = d[0] / l * s;
                        v[2] = d[1] / l * s;
                        player.yaw = yaw_of(d);
                    }
                    if tick <= t::FLOW_DASH {
                        let dz = foe.centre[1] - (player.position[1] + t::CHEST);
                        v[1] = dz.clamp(-t::HOVER_CLAMP, t::HOVER_CLAMP) * t::FLOW_GAIN / 60.;
                        self.hover = 0.;
                    }
                }
                if tick > t::FLOW_DASH {
                    self.hover -= t::HOVER_FALL;
                    v[1] = self.hover;
                }
                self.velocity = v;
                let moved = Self::travel(player, v, true, p, world);
                let end = player.clip(clips).end;
                if moved.on_floor && tick > t::FLOW_DASH {
                    self.land(player, clips, &mut out.step);
                } else if player.frame >= end {
                    self.fall(player, [0.; 3], clips, &mut out.step);
                }
            }
            Mode::Launch { tick } => {
                let tick = tick + 1;
                self.mode = Mode::Launch { tick };
                player.frame = player.frame.saturating_add(1);
                self.shown = Some(player.action);
                let homing = (t::LAUNCH_FROM..t::LAUNCH_TO).contains(&tick);
                let rising = (t::LAUNCH_RISE_FROM..t::LAUNCH_END).contains(&tick);
                let vy = if rising { t::LAUNCH_RISE } else { 0. };
                // Rising Spiral homes in (its traction window is 0.1-0.6 s).
                let mut h = [0f32; 2];
                if homing && let Some(foe) = scene.foe {
                    let d = [
                        foe.position[0] - player.position[0],
                        foe.position[2] - player.position[2],
                    ];
                    let l = d[0].hypot(d[1]);
                    if l > t::CLOSE_STOP && l <= t::TRACTION_RANGE {
                        let s = t::TRACTION.min(l - t::CLOSE_STOP);
                        h = [d[0] / l * s, d[1] / l * s];
                        player.yaw = yaw_of(d);
                    }
                }
                self.velocity = [h[0], vy, h[1]];
                Self::travel(player, self.velocity, rising, p, world);
                if tick >= t::LAUNCH_END {
                    // Out into the air: the chain goes on as an air combo.
                    self.air_swings = 0;
                    self.velocity = [0.; 3];
                    self.mode = Mode::Air {
                        jump: Jump::None,
                        hang: 0,
                        released: true,
                        tick: 0,
                    };
                    self.double = false;
                    self.show(player, Action::Fall, clips, &mut out.step);
                }
            }
        }
    }

    /// Ground locomotion and its commands.
    #[allow(clippy::too_many_arguments)]
    fn ground(
        &mut self,
        player: &mut Player,
        input: &Input,
        p: &Params,
        clips: &Clips,
        world: &impl World,
        scene: &Scene,
        out: &mut Outcome,
    ) {
        self.launched = false;
        self.air_swings = 0;
        self.double = true;
        let (m, dir) = stick(input);
        if self.counter > 0
            && input.attack
            && self.play_record(player, record::COUNTER, input, clips)
        {
            self.counter = 0;
            self.mode = Mode::Kh2;
            out.began.push("counter");
            return;
        }
        if input.attack && self.start_chase(player, scene, clips, &mut out.step) {
            return;
        }
        if input.attack && self.command(player, input, p, clips, world, scene, out) {
            return;
        }
        if input.guard {
            if m >= t::STICK_ACTIVE {
                self.start_roll(player, dir, clips, &mut out.step);
                return;
            }
            if self.command(player, input, p, clips, world, scene, out) {
                return;
            }
        }
        if input.jump {
            self.start_jump(player, clips, &mut out.step);
            return;
        }
        let speed = if m < t::STICK_ACTIVE {
            0.
        } else if m < t::RUN_STICK {
            t::WALK
        } else if self.run_ticks > t::SPRINT_AFTER {
            t::SPRINT
        } else {
            t::RUN
        };
        if speed > 0. {
            if speed >= t::RUN {
                self.run_ticks += 1;
            } else {
                self.run_ticks = 0;
            }
            self.velocity = [dir[0] * speed, 0., dir[1] * speed];
            player.yaw = turn(player.yaw, yaw_of(dir), t::RUN_TURN);
        } else {
            self.run_ticks = 0;
            let v = self.velocity[0].hypot(self.velocity[2]);
            let s = (v * (1. - t::BRAKE_FRICTION) - t::BRAKE).max(0.);
            let k = if v > 1e-6 { s / v } else { 0. };
            self.velocity = [self.velocity[0] * k, 0., self.velocity[2] * k];
        }
        // Running into a wall with the stick on it starts a wall run.
        if speed >= t::RUN
            && m > t::WALL_STICK
            && let Some(wall) = surface::wall_near(
                world,
                player.position,
                dir,
                p.radius * 0.5,
                p.radius,
                Self::span(player.position[1], p),
            )
            && self.start_wall_run(player, wall, dir, clips, &mut out.step)
        {
            return;
        }
        let v = self.velocity;
        let moved = Self::travel(player, [v[0], 0., v[2]], false, p, world);
        if !moved.on_floor {
            let h = v[0].hypot(v[2]);
            let k = if h > crate::WALK_OFF_SPEED {
                crate::WALK_OFF_SPEED / h
            } else {
                1.
            };
            self.fall(player, [v[0] * k, 0., v[2] * k], clips, &mut out.step);
            return;
        }
        let v = self.velocity[0].hypot(self.velocity[2]);
        let action = if v >= (t::WALK + t::RUN) * 0.5 {
            Action::Run
        } else if v > 0.2 {
            Action::Walk
        } else {
            Action::Idle
        };
        self.show(player, action, clips, &mut out.step);
    }

    /// A fall with `velocity` (no jump began it).
    fn fall(&mut self, player: &mut Player, velocity: Vec3, clips: &Clips, step: &mut Step) {
        self.velocity = velocity;
        self.mode = Mode::Air {
            jump: Jump::None,
            hang: t::HANG_TICKS,
            released: true,
            tick: 0,
        };
        self.lean_target = [0.; 2];
        self.show(player, Action::Fall, clips, step);
    }

    fn land(&mut self, player: &mut Player, clips: &Clips, step: &mut Step) {
        self.velocity = [0.; 3];
        self.push = [0.; 2];
        self.lean_target = [0.; 2];
        self.mode = Mode::Land { tick: 0 };
        self.double = true;
        self.launched = false;
        self.air_swings = 0;
        step.landed = true;
        self.show(player, Action::Land, clips, step);
    }

    fn start_jump(&mut self, player: &mut Player, clips: &Clips, step: &mut Step) {
        let jump = if player.growth_level(growth::HIGH_JUMP) > 0 {
            Jump::High
        } else {
            Jump::Normal
        };
        self.mode = Mode::Takeoff { tick: 0, jump };
        self.double = true;
        self.show(
            player,
            if jump == Jump::High {
                Action::HighJump
            } else {
                Action::Jump
            },
            clips,
            step,
        );
    }

    fn start_roll(&mut self, player: &mut Player, dir: [f32; 2], clips: &Clips, step: &mut Step) {
        player.yaw = yaw_of(dir);
        player.invulnerable = t::ROLL_CANCEL as f32 + 1.;
        self.velocity = [0.; 3];
        self.aim = None;
        self.mode = Mode::Roll { tick: 0 };
        self.show(
            player,
            Action::Growth(growth::motion::DODGE_ROLL),
            clips,
            step,
        );
    }

    fn start_slide(&mut self, player: &mut Player, dir: [f32; 2], clips: &Clips, step: &mut Step) {
        player.yaw = yaw_of(dir);
        self.velocity = [0.; 3];
        self.aim = None;
        self.mode = Mode::Slide { tick: 0, dir };
        player.invulnerable = player.invulnerable.max(t::SLIDE_TICKS as f32 + 1.);
        self.show(
            player,
            Action::Growth(growth::motion::QUICK_RUN),
            clips,
            step,
        );
    }

    /// The flow jump: up at 2500 cm/s with a push along `dir`.
    fn super_jump(&mut self, player: &mut Player, dir: [f32; 2], clips: &Clips, step: &mut Step) {
        player.yaw = yaw_of(dir);
        self.velocity = [0., t::SUPER_JUMP, 0.];
        self.push = [dir[0] * t::SUPER_PUSH, dir[1] * t::SUPER_PUSH];
        self.double = false;
        self.lean_target = [0.; 2];
        self.mode = Mode::Air {
            jump: Jump::Super,
            hang: 0,
            released: false,
            tick: 0,
        };
        self.show(player, Action::HighJump, clips, step);
    }

    /// Airborne: KH3's gravity curve and air control, and the air's moves.
    #[allow(clippy::too_many_arguments)]
    fn air(
        &mut self,
        player: &mut Player,
        input: &Input,
        p: &Params,
        clips: &Clips,
        world: &impl World,
        scene: &Scene,
        out: &mut Outcome,
    ) {
        let Mode::Air {
            jump,
            mut hang,
            mut released,
            tick,
        } = self.mode
        else {
            return;
        };
        let tick = tick + 1;
        let (m, dir) = stick(input);
        let vy = self.velocity[1];
        // Commands.
        if input.attack {
            if self.counter > 0 && self.play_record(player, record::COUNTER, input, clips) {
                self.counter = 0;
                self.mode = Mode::Kh2;
                out.began.push("counter");
                return;
            }
            if self.start_chase(player, scene, clips, &mut out.step) {
                return;
            }
            if self.command(player, input, p, clips, world, scene, out) {
                return;
            }
        }
        if input.guard && m >= t::STICK_ACTIVE {
            if jump == Jump::Super {
                player.yaw = yaw_of(dir);
                self.start_superslide(player, None, scene, clips, &mut out.step);
                return;
            }
            if self.slide_cooldown == 0 {
                self.start_slide(player, dir, clips, &mut out.step);
                return;
            }
        }
        if input.jump && self.double && vy <= t::DOUBLEFLIGHT_BELOW {
            self.double = false;
            if m >= t::STICK_ACTIVE {
                player.yaw = yaw_of(dir);
            }
            self.mode = Mode::Takeoff {
                tick: 0,
                jump: Jump::Double,
            };
            self.show(
                player,
                Action::Growth(growth::motion::AERIAL_DODGE),
                clips,
                &mut out.step,
            );
            return;
        }
        // Glide: Jump held while a jump's fall runs.
        if !input.jump_up
            && vy <= 0.
            && matches!(jump, Jump::Normal | Jump::High | Jump::Double)
            && tick > 1
        {
            self.velocity = [0.; 3];
            self.slide_cooldown = self.slide_cooldown.max(t::SLIDE_COOLDOWN);
            self.mode = Mode::Glide { tick: 0 };
            self.glide(player, input, 0, p, clips, world, scene, out);
            return;
        }
        // Falling into a wall with the stick on it.
        if vy < 0.
            && m >= t::WALL_STICK
            && let Some(wall) = surface::wall_near(
                world,
                player.position,
                dir,
                p.radius * 0.5,
                p.radius,
                Self::span(player.position[1], p),
            )
            && self.start_wall_run(player, wall, dir, clips, &mut out.step)
        {
            return;
        }
        // The gravity curve.
        if matches!(jump, Jump::Normal | Jump::High) && input.jump_up && vy > 0. {
            released = true;
        }
        let g = if vy > 0. {
            match jump {
                Jump::None => t::GRAVITY,
                Jump::Normal | Jump::High if released => t::GRAVITY_CUT,
                _ => t::GRAVITY_RISE,
            }
        } else if jump != Jump::None && hang < t::HANG_TICKS {
            hang += 1;
            t::GRAVITY_HANG
        } else {
            t::GRAVITY
        };
        self.velocity[1] = (vy - g).max(-t::FALL_MAX);
        // Air control.
        let h = [self.velocity[0], self.velocity[2]];
        let h = if m >= t::STICK_ACTIVE {
            let target = [dir[0] * t::AIR_MAX, dir[1] * t::AIR_MAX];
            let d = [target[0] - h[0], target[1] - h[1]];
            let l = d[0].hypot(d[1]);
            let k = if l > t::AIR_ACCEL {
                t::AIR_ACCEL / l
            } else {
                1.
            };
            player.yaw = turn(player.yaw, yaw_of(dir), t::AIR_TURN);
            [h[0] + d[0] * k, h[1] + d[1] * k]
        } else {
            let l = h[0].hypot(h[1]);
            let s = (l - t::AIR_BRAKE).max(0.);
            let k = if l > 1e-6 { s / l } else { 0. };
            [h[0] * k, h[1] * k]
        };
        let l = h[0].hypot(h[1]);
        let k = if l > t::AIR_MAX { t::AIR_MAX / l } else { 1. };
        self.velocity[0] = h[0] * k;
        self.velocity[2] = h[1] * k;
        self.push = self.push.map(|v| v * t::SUPER_PUSH_DECAY);
        if m >= t::STICK_ACTIVE {
            self.push = [0.; 2];
        }
        self.mode = Mode::Air {
            jump,
            hang,
            released,
            tick,
        };
        let action = match jump {
            Jump::Double
                if player.frame < player.clip(clips).end
                    && self.shown == Some(Action::Growth(growth::motion::AERIAL_DODGE)) =>
            {
                Action::Growth(growth::motion::AERIAL_DODGE)
            }
            Jump::Super if self.velocity[1] > 0. => Action::HighJump,
            Jump::High if self.velocity[1] > t::GRAVITY_RISE * 5. => Action::HighJump,
            Jump::Normal if self.velocity[1] > t::GRAVITY_RISE * 5. => Action::Jump,
            _ => Action::Fall,
        };
        self.show(player, action, clips, &mut out.step);
        let v = [
            self.velocity[0] + self.push[0],
            self.velocity[1],
            self.velocity[2] + self.push[1],
        ];
        let moved = Self::travel(player, v, true, p, world);
        if moved.on_floor && self.velocity[1] <= 0. {
            self.land(player, clips, &mut out.step);
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn slide(
        &mut self,
        player: &mut Player,
        input: &Input,
        tick: u32,
        dir: [f32; 2],
        p: &Params,
        clips: &Clips,
        world: &impl World,
        scene: &Scene,
        out: &mut Outcome,
    ) {
        let tick = tick + 1;
        self.mode = Mode::Slide { tick, dir };
        player.invulnerable = player.invulnerable.max(2.);
        if tick >= t::SLIDE_CANCEL {
            if input.jump && self.double && self.velocity[1] <= t::DOUBLEFLIGHT_BELOW {
                self.double = false;
                self.mode = Mode::Takeoff {
                    tick: 0,
                    jump: Jump::Double,
                };
                self.show(
                    player,
                    Action::Growth(growth::motion::AERIAL_DODGE),
                    clips,
                    &mut out.step,
                );
                return;
            }
            if input.attack && self.command(player, input, p, clips, world, scene, out) {
                return;
            }
        }
        // A pole or a wall in the way: pole spin, wall kick.
        if self.grab(player, scene, clips, &mut out.step) {
            return;
        }
        if let Some(wall) = surface::wall_near(
            world,
            player.position,
            dir,
            p.radius * 0.5,
            p.radius,
            Self::span(player.position[1], p),
        ) && -(wall.normal[0] * dir[0] + wall.normal[1] * dir[1]) > 0.5
        {
            self.start_kick(player, wall, clips, &mut out.step);
            return;
        }
        let d = (slide_share(tick) - slide_share(tick - 1)) * t::SLIDE_DISTANCE;
        self.velocity = [dir[0] * d, 0., dir[1] * d];
        self.show(
            player,
            Action::Growth(growth::motion::QUICK_RUN),
            clips,
            &mut out.step,
        );
        let moved = Self::travel(player, self.velocity, true, p, world);
        if tick >= t::SLIDE_TICKS {
            if moved.on_floor {
                self.land(player, clips, &mut out.step);
            } else {
                self.slide_cooldown = t::SLIDE_COOLDOWN;
                self.fall(player, [0.; 3], clips, &mut out.step);
            }
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn glide(
        &mut self,
        player: &mut Player,
        input: &Input,
        tick: u32,
        p: &Params,
        clips: &Clips,
        world: &impl World,
        scene: &Scene,
        out: &mut Outcome,
    ) {
        if input.jump_up {
            self.fall(player, [0.; 3], clips, &mut out.step);
            return;
        }
        let (m, dir) = stick(input);
        if input.attack && self.command(player, input, p, clips, world, scene, out) {
            return;
        }
        if input.guard && m >= t::STICK_ACTIVE && self.slide_cooldown == 0 {
            self.start_slide(player, dir, clips, &mut out.step);
            return;
        }
        let tick = tick + 1;
        self.mode = Mode::Glide { tick };
        if m > 0.5 {
            player.yaw = turn(player.yaw, yaw_of(dir), t::GLIDE_TURN);
        }
        let speed = if tick <= 1 { t::GLIDE_FIRST } else { t::GLIDE };
        let f = facing(player.yaw);
        let vy = self.velocity[1] - t::GLIDE_SINK;
        self.velocity = [f[0] * speed, vy, f[1] * speed];
        self.show(
            player,
            Action::Growth(growth::motion::GLIDE),
            clips,
            &mut out.step,
        );
        let moved = Self::travel(player, self.velocity, true, p, world);
        // [duel choice] Touching the ground ends it (the reference's reading
        // is that only the button does; it marks that unconfirmed).
        if moved.on_floor {
            self.land(player, clips, &mut out.step);
        }
    }

    /// A pole within reach of the point ahead: the pole spin.
    fn grab(&mut self, player: &mut Player, scene: &Scene, clips: &Clips, step: &mut Step) -> bool {
        let f = facing(player.yaw);
        let at = [
            player.position[0] + f[0] * t::POLE_AHEAD,
            player.position[1] + t::POLE_UP,
            player.position[2] + f[1] * t::POLE_AHEAD,
        ];
        let Some(pole) = surface::pole_near(scene.poles, at, t::POLE_REACH) else {
            return false;
        };
        if self.last_pole == Some(pole.centre) && matches!(self.mode, Mode::Superslide { .. }) {
            return false;
        }
        let r = [
            player.position[0] - pole.centre[0],
            player.position[2] - pole.centre[1],
        ];
        // Spin the way he was going round it.
        let cross = r[0] * f[1] - r[1] * f[0];
        let spin = if cross >= 0. { 1. } else { -1. };
        self.velocity = [0.; 3];
        self.double = true;
        self.mode = Mode::Pole {
            pole,
            angle: r[1].atan2(r[0]),
            spin,
            tick: 0,
            jump: false,
            guard: false,
        };
        let motion = if spin > 0. {
            growth::motion::GLIDE_LEFT
        } else {
            growth::motion::GLIDE_RIGHT
        };
        self.show(player, Action::Growth(motion), clips, step);
        true
    }

    #[allow(clippy::too_many_arguments)]
    fn pole(
        &mut self,
        player: &mut Player,
        input: &Input,
        p: &Params,
        clips: &Clips,
        world: &impl World,
        scene: &Scene,
        out: &mut Outcome,
    ) {
        let Mode::Pole {
            pole,
            angle,
            spin,
            tick,
            mut jump,
            mut guard,
        } = self.mode
        else {
            return;
        };
        let _ = world;
        let tick = tick + 1;
        if input.attack && self.start_flow(player, record::SPIN, input, clips) {
            self.last_pole = Some(pole.centre);
            return;
        }
        jump |= input.jump;
        guard |= input.guard;
        let rate = TAU / t::POLE_TURN as f32 * spin;
        let attach = tick < t::POLE_ATTACH;
        let angle = angle + if attach { rate * 0.5 } else { rate };
        let r = pole.radius + t::POLE_GAP;
        let target = [
            pole.centre[0] + angle.cos() * r,
            pole.centre[1] + angle.sin() * r,
        ];
        let k = if attach {
            (tick as f32 / t::POLE_ATTACH as f32).max(0.2)
        } else {
            1.
        };
        player.position[0] += (target[0] - player.position[0]) * k;
        player.position[2] += (target[1] - player.position[2]) * k;
        let y = player.position[1].clamp(
            pole.bottom + p.step_height,
            (pole.top - p.height).max(pole.bottom),
        );
        player.position[1] = y;
        // Facing along the turn.
        let tangent = [-angle.sin() * spin, angle.cos() * spin];
        player.yaw = yaw_of(tangent);
        self.lean_target = [0., -spin * 0.6];
        let motion = if spin > 0. {
            growth::motion::GLIDE_LEFT
        } else {
            growth::motion::GLIDE_RIGHT
        };
        self.show(player, Action::Growth(motion), clips, &mut out.step);
        let spun = tick.saturating_sub(t::POLE_ATTACH);
        let turn_done = !attach && spun.is_multiple_of(t::POLE_TURN);
        let over = spun >= t::POLE_TURN * t::POLE_TURNS;
        self.mode = Mode::Pole {
            pole,
            angle,
            spin,
            tick,
            jump,
            guard,
        };
        if (jump && turn_done) || (over && jump) {
            self.last_pole = Some(pole.centre);
            let (m, dir) = stick(input);
            let d = if m > t::STICK_ACTIVE { dir } else { tangent };
            self.super_jump(player, d, clips, &mut out.step);
        } else if (guard && turn_done) || over {
            self.last_pole = Some(pole.centre);
            player.yaw = yaw_of(tangent);
            self.start_superslide(player, Some(pole.centre), scene, clips, &mut out.step);
        }
    }

    fn start_superslide(
        &mut self,
        player: &mut Player,
        pole: Option<[f32; 2]>,
        scene: &Scene,
        clips: &Clips,
        step: &mut Step,
    ) {
        // Out of a pole it aims at a target ahead.
        if pole.is_some()
            && let Some(foe) = scene.foe
        {
            let d = [
                foe.position[0] - player.position[0],
                foe.position[2] - player.position[2],
            ];
            let l = d[0].hypot(d[1]);
            let f = facing(player.yaw);
            if l < t::SUPERSLIDE_AIM
                && l > 1e-3
                && (d[0] * f[0] + d[1] * f[1]) / l >= t::SUPERSLIDE_AIM_COS
            {
                player.yaw = yaw_of(d);
            }
        }
        self.velocity = [0.; 3];
        self.lean_target = [0.; 2];
        self.aim = None;
        self.mode = Mode::Superslide { tick: 0, pole };
        self.show(player, Action::Growth(growth::motion::GLIDE), clips, step);
    }

    #[allow(clippy::too_many_arguments)]
    fn superslide(
        &mut self,
        player: &mut Player,
        input: &Input,
        tick: u32,
        pole: Option<[f32; 2]>,
        p: &Params,
        clips: &Clips,
        world: &impl World,
        scene: &Scene,
        out: &mut Outcome,
    ) {
        let tick = tick + 1;
        self.mode = Mode::Superslide { tick, pole };
        if input.jump {
            let f = facing(player.yaw);
            self.super_jump(player, f, clips, &mut out.step);
            return;
        }
        if input.attack && self.start_flow(player, record::SONIC, input, clips) {
            return;
        }
        let (m, dir) = stick(input);
        if m >= t::STICK_ACTIVE {
            player.yaw = turn(player.yaw, yaw_of(dir), t::SUPERSLIDE_TURN);
        }
        let f = facing(player.yaw);
        if tick > t::SUPERSLIDE_CONTACT {
            if self.grab(player, scene, clips, &mut out.step) {
                return;
            }
            if let Some(wall) = surface::wall_near(
                world,
                player.position,
                f,
                p.radius * 0.5,
                p.radius,
                Self::span(player.position[1], p),
            ) && -(wall.normal[0] * f[0] + wall.normal[1] * f[1]) > 0.5
            {
                self.start_kick(player, wall, clips, &mut out.step);
                return;
            }
        }
        let speed = (t::SUPERSLIDE - t::SUPERSLIDE_DECAY * tick as f32).max(0.);
        self.velocity = [f[0] * speed, 0., f[1] * speed];
        self.show(
            player,
            Action::Growth(growth::motion::GLIDE),
            clips,
            &mut out.step,
        );
        Self::travel(player, self.velocity, true, p, world);
        if tick >= t::SUPERSLIDE_TICKS {
            self.mode = Mode::SlideEnd {
                tick: 0,
                speed: 2. * t::SLIDE_END_DISTANCE / t::SLIDE_END_TICKS as f32,
            };
        }
    }

    /// Classifies a wall met with the stick along `dir`: an up-run within
    /// 35° of it, a side run at 50°-130°.
    pub fn wall_kind(wall: &Wall, dir: [f32; 2]) -> Option<(bool, f32)> {
        let into = -(wall.normal[0] * dir[0] + wall.normal[1] * dir[1]);
        if into >= t::WALL_UP_COS {
            return Some((true, 0.));
        }
        if into.abs() <= t::WALL_SIDE_COS {
            // Which way along the wall the stick points.
            let tangent = [-wall.normal[1], wall.normal[0]];
            let side = (tangent[0] * dir[0] + tangent[1] * dir[1]).signum();
            return Some((false, side));
        }
        None
    }

    fn start_wall_run(
        &mut self,
        player: &mut Player,
        wall: Wall,
        dir: [f32; 2],
        clips: &Clips,
        step: &mut Step,
    ) -> bool {
        let Some((up, side)) = Self::wall_kind(&wall, dir) else {
            return false;
        };
        let n = wall.normal;
        player.position[0] = wall.surface[0] + n[0];
        player.position[2] = wall.surface[1] + n[1];
        self.velocity = [0.; 3];
        self.double = true;
        self.aim = None;
        self.mode = Mode::WallRun {
            normal: n,
            up,
            side,
            tick: 0,
            idle: 0,
        };
        self.set_wall_pose(player, n, up, side);
        self.show(player, Action::Run, clips, step);
        true
    }

    /// Facing and lean on a wall: an up-run faces the wall, pitched back so
    /// his body stands out of it; a side run faces along it, rolled away.
    fn set_wall_pose(&mut self, player: &mut Player, n: [f32; 2], up: bool, side: f32) {
        if up {
            player.yaw = yaw_of([-n[0], -n[1]]);
            self.lean_target = [-FRAC_PI_2, 0.];
        } else {
            let tangent = [-n[1] * side, n[0] * side];
            player.yaw = yaw_of(tangent);
            // His right (model +X) in the world: (cos yaw, -sin yaw).
            let right = [player.yaw.cos(), -player.yaw.sin()];
            let away = n[0] * right[0] + n[1] * right[1];
            self.lean_target = [0., -away.signum() * FRAC_PI_2];
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn wall_run(
        &mut self,
        player: &mut Player,
        input: &Input,
        p: &Params,
        clips: &Clips,
        world: &impl World,
        scene: &Scene,
        out: &mut Outcome,
    ) {
        let Mode::WallRun {
            normal: n,
            up,
            side,
            tick,
            idle,
        } = self.mode
        else {
            return;
        };
        let tick = tick + 1;
        let (m, dir) = stick(input);
        let idle = if m < t::STICK_ACTIVE { idle + 1 } else { 0 };
        let off = [n[0] * (p.radius + 1.), n[1] * (p.radius + 1.)];
        if input.jump {
            let d = if m > 0.5 { dir } else { n };
            player.position[0] += off[0];
            player.position[2] += off[1];
            self.super_jump(player, d, clips, &mut out.step);
            return;
        }
        if input.guard || (up && tick > t::WALL_RUN_MAX) {
            let wall = Wall {
                normal: n,
                surface: [player.position[0] - n[0], player.position[2] - n[1]],
            };
            self.start_kick(player, wall, clips, &mut out.step);
            if let Mode::WallKick { guard, .. } = &mut self.mode {
                *guard = input.guard;
            }
            return;
        }
        if input.attack && scene.foe.is_some() {
            player.position[0] += off[0];
            player.position[2] += off[1];
            if self.start_flow(player, record::WALL_ATTACK, input, clips) {
                return;
            }
            player.position[0] -= off[0];
            player.position[2] -= off[1];
        }
        if idle > t::WALL_GRACE {
            player.position[0] += off[0];
            player.position[2] += off[1];
            let v = [
                self.velocity[0] * 0.5,
                self.velocity[1].max(0.) * 0.5,
                self.velocity[2] * 0.5,
            ];
            self.fall(player, v, clips, &mut out.step);
            return;
        }
        let tangent = [-n[1], n[0]];
        let v = if up {
            let drift = if m >= t::STICK_ACTIVE {
                (tangent[0] * dir[0] + tangent[1] * dir[1]) * t::WALL_DRIFT
            } else {
                0.
            };
            [tangent[0] * drift, t::WALL_RUN, tangent[1] * drift]
        } else {
            [
                tangent[0] * side * t::WALL_RUN,
                0.,
                tangent[1] * side * t::WALL_RUN,
            ]
        };
        self.velocity = v;
        let next = [
            player.position[0] + v[0],
            player.position[1] + v[1],
            player.position[2] + v[2],
        ];
        // The wall must go on under his feet: a probe circle half in it
        // (centred off it) is pushed out along its normal.
        let span = (
            next[1] + p.step_height,
            next[1] + p.step_height + t::length_of(60.),
        );
        match surface::wall_near(world, next, n, p.radius * 0.5, p.radius, span) {
            Some(w) if w.normal[0] * n[0] + w.normal[1] * n[1] > 0.82 => {
                player.position = [
                    w.surface[0] + w.normal[0],
                    next[1],
                    w.surface[1] + w.normal[1],
                ];
                self.mode = Mode::WallRun {
                    normal: w.normal,
                    up,
                    side,
                    tick,
                    idle,
                };
                self.set_wall_pose(player, w.normal, up, side);
                self.show(player, Action::Run, clips, &mut out.step);
            }
            _ if up => {
                // Over the top edge: the roll over, up [duel choice: and back
                // off it, so an arena's wall never puts him outside].
                let vy = (2. * t::GRAVITY_RISE * t::WALL_TOP_RISE).sqrt();
                player.position[0] += off[0];
                player.position[2] += off[1];
                self.velocity = [n[0] * t::RUN * 0.3, vy, n[1] * t::RUN * 0.3];
                self.lean_target = [0.; 2];
                self.mode = Mode::Air {
                    jump: Jump::Super,
                    hang: 0,
                    released: false,
                    tick: 0,
                };
                self.double = true;
                out.began.push("wall_top");
                self.show(
                    player,
                    Action::Growth(growth::motion::AERIAL_DODGE),
                    clips,
                    &mut out.step,
                );
            }
            _ => {
                // The wall ends ahead: off it, with half the speed.
                player.position[0] += off[0];
                player.position[2] += off[1];
                self.fall(player, v.map(|c| c * 0.5), clips, &mut out.step);
            }
        }
    }

    fn start_kick(&mut self, player: &mut Player, wall: Wall, clips: &Clips, step: &mut Step) {
        let n = wall.normal;
        player.position[0] = wall.surface[0] + n[0];
        player.position[2] = wall.surface[1] + n[1];
        player.yaw = yaw_of([-n[0], -n[1]]);
        self.velocity = [0.; 3];
        self.double = true;
        self.lean_target = [-FRAC_PI_2, 0.];
        self.mode = Mode::WallKick {
            tick: 0,
            normal: n,
            guard: false,
        };
        self.show(player, Action::Fall, clips, step);
    }

    #[allow(clippy::too_many_arguments)]
    fn wall_kick(
        &mut self,
        player: &mut Player,
        input: &Input,
        p: &Params,
        clips: &Clips,
        world: &impl World,
        scene: &Scene,
        out: &mut Outcome,
    ) {
        let Mode::WallKick {
            tick,
            normal: n,
            mut guard,
        } = self.mode
        else {
            return;
        };
        let tick = tick + 1;
        guard |= input.guard;
        let kicked = tick > t::KICK_HOLD;
        if tick == t::KICK_HOLD + 1 {
            // 80 cm out and turned round.
            let out_by = p.radius + t::KICK_OUT;
            Self::travel(player, [n[0] * out_by, 0., n[1] * out_by], true, p, world);
            player.yaw = yaw_of(n);
            self.lean_target = [0.; 2];
        }
        if tick > t::KICK_JUMP && input.jump {
            if !kicked {
                Self::travel(
                    player,
                    [n[0] * (p.radius + 1.), 0., n[1] * (p.radius + 1.)],
                    true,
                    p,
                    world,
                );
            }
            let (m, dir) = stick(input);
            self.super_jump(player, if m > 0.5 { dir } else { n }, clips, &mut out.step);
            return;
        }
        if input.attack && scene.foe.is_some() {
            if !kicked {
                Self::travel(
                    player,
                    [n[0] * (p.radius + 1.), 0., n[1] * (p.radius + 1.)],
                    true,
                    p,
                    world,
                );
            }
            if self.start_flow(player, record::SONIC, input, clips) {
                return;
            }
        }
        self.mode = Mode::WallKick {
            tick,
            normal: n,
            guard,
        };
        let motion = if kicked {
            Action::Growth(growth::motion::AERIAL_DODGE)
        } else {
            Action::Fall
        };
        self.show(player, motion, clips, &mut out.step);
        if tick >= t::KICK_TICKS {
            if guard {
                player.yaw = yaw_of(n);
                self.start_superslide(player, None, scene, clips, &mut out.step);
            } else {
                self.fall(player, [0.; 3], clips, &mut out.step);
            }
        }
    }

    /// A flow attack: the record's swing, moved by the layer.
    fn start_flow(&mut self, player: &mut Player, id: u8, input: &Input, clips: &Clips) -> bool {
        let id = if ptya::find(&clips.records, id).is_some() {
            id
        } else {
            record::SONIC
        };
        if !self.play_record(player, id, input, clips) {
            return false;
        }
        self.lean_target = [0.; 2];
        self.hover = 0.;
        self.mode = Mode::Flow { tick: 0 };
        self.shown = Some(player.action);
        self.flow = Some((id, false));
        self.flow_hits = 0;
        true
    }

    #[allow(clippy::too_many_arguments)]
    fn airstep(
        &mut self,
        player: &mut Player,
        input: &Input,
        p: &Params,
        clips: &Clips,
        world: &impl World,
        scene: &Scene,
        out: &mut Outcome,
    ) {
        let Mode::Airstep { tick, speed } = self.mode else {
            return;
        };
        let tick = tick + 1;
        let _ = input;
        player.invulnerable = player.invulnerable.max(2.);
        let Some(foe) = scene.foe else {
            self.fall(player, [0.; 3], clips, &mut out.step);
            return;
        };
        let me = [
            player.position[0],
            player.position[1] + t::CHEST,
            player.position[2],
        ];
        let d = [
            foe.centre[0] - me[0],
            foe.centre[1] - me[1],
            foe.centre[2] - me[2],
        ];
        let dist = len3(d);
        if dist < t::AIRSTEP_ARRIVE || tick > t::AIRSTEP_MAX {
            // Arrived: the hop back.
            self.velocity = [0.; 3];
            self.lean_target = [0.; 2];
            self.mode = Mode::HopBack { tick: 0 };
            self.show(
                player,
                Action::Growth(growth::motion::AERIAL_DODGE),
                clips,
                &mut out.step,
            );
            return;
        }
        let near = dist / t::SCALE < t::AIRSTEP_NEAR;
        let v = if near {
            speed.min(t::AIRSTEP_NEAR_SPEED * t::SCALE / 60.)
        } else {
            speed
        };
        let step_len = v.min(dist - t::AIRSTEP_ARRIVE * 0.5);
        let u = unit3(d);
        player.yaw = turn(player.yaw, yaw_of([u[0], u[2]]), t::AIRSTEP_TURN);
        let flat = u[0].hypot(u[2]);
        self.lean_target = [(-u[1].atan2(flat)).clamp(-1.2, 1.2), 0.];
        self.velocity = u.map(|c| c * step_len);
        self.mode = Mode::Airstep { tick, speed };
        self.show(
            player,
            Action::Growth(growth::motion::GLIDE),
            clips,
            &mut out.step,
        );
        let before = player.position;
        Self::travel(player, self.velocity, true, p, world);
        let moved = len3([
            player.position[0] - before[0],
            player.position[1] - before[1],
            player.position[2] - before[2],
        ]);
        // Blocked (under half the speed): give up.
        if tick > 12 && moved < step_len * 0.5 {
            self.fall(player, [0.; 3], clips, &mut out.step);
        }
    }

    /// The shotlock button: hold to aim, Attack fires, Guard airsteps,
    /// letting go cancels.
    fn shotlock_button(
        &mut self,
        player: &mut Player,
        input: &Input,
        pad: &Pad,
        scene: &Scene,
        out: &mut Outcome,
    ) {
        if !pad.shotlock {
            self.shot_hold = 0;
            self.shot_spent = false;
            if self.aim.take().is_some() {
                out.began.push("aim_cancel");
            }
            return;
        }
        self.shot_hold = self.shot_hold.saturating_add(1);
        let can_aim = matches!(
            self.mode,
            Mode::Ground | Mode::Air { .. } | Mode::Land { .. }
        ) || (self.mode == Mode::Kh2 && free(player.action));
        let range = scene.foe.map_or(f32::MAX, |f| {
            let d = [
                f.centre[0] - player.position[0],
                f.centre[1] - player.position[1],
                f.centre[2] - player.position[2],
            ];
            len3(d)
        });
        if self.aim.is_none() {
            // Held long enough, in a state that may aim (from then on, not
            // only the tick the hold is reached).
            if self.shot_hold >= t::AIM_HOLD
                && !self.shot_spent
                && can_aim
                && (self.focus > 0. || self.focus_empty)
                && range <= t::AIM_RANGE
            {
                self.shot_spent = true;
                self.aim = Some(Aim {
                    syphon: self.focus_empty,
                    ..Aim::default()
                });
                if let Some(f) = scene.foe {
                    player.yaw = yaw_of([
                        f.position[0] - player.position[0],
                        f.position[2] - player.position[2],
                    ]);
                }
                out.began.push("aim");
            }
            return;
        }
        if !can_aim {
            self.aim = None;
            return;
        }
        let Some(mut aim) = self.aim else {
            return;
        };
        aim.tick += 1;
        if aim.syphon {
            // Focus Syphon: no cost; each lock waits on the distance.
            if aim.tick > t::AIM_DELAY && aim.locks < t::SYPHON_LOCKS {
                aim.timer += 1.;
                let wait = (t::SYPHON_PER_CM * range / t::SCALE).max(1.);
                if range <= t::AIM_RANGE && (aim.locks == 0 || aim.timer >= wait) {
                    aim.locks += 1;
                    aim.timer = 0.;
                    out.cues.push(if aim.locks == t::SYPHON_LOCKS {
                        "max_locks"
                    } else {
                        "lock"
                    });
                }
            }
            self.aim = Some(aim);
            if input.attack && aim.locks > 0 {
                self.aim = None;
                self.velocity = [0.; 3];
                self.mode = Mode::Syphon {
                    tick: 0,
                    locks: aim.locks,
                };
                self.shown = Some(player.action);
                out.began.push("focus_syphon");
            } else if input.guard && scene.foe.is_some() && range <= t::AIRSTEP_RANGE {
                self.aim = None;
                self.velocity = [0.; 3];
                self.mode = Mode::Airstep {
                    tick: 0,
                    speed: airstep_speed(range),
                };
                self.shown = Some(player.action);
                out.began.push("airstep");
            }
            return;
        }
        if aim.tick > t::AIM_DELAY && aim.locks < t::MAX_LOCKS {
            self.focus = (self.focus - t::FOCUS_AIM).max(0.);
            aim.timer += 1.;
            if range <= t::AIM_RANGE && (aim.locks == 0 || aim.timer >= t::LOCK_INTERVAL) {
                aim.locks += 1;
                aim.timer = (aim.timer - t::LOCK_INTERVAL).max(0.);
                out.cues.push(if aim.locks == t::MAX_LOCKS {
                    "max_locks"
                } else {
                    "lock"
                });
            }
        }
        self.aim = Some(aim);
        let fire = input.attack || (self.focus <= 0. && aim.locks > 0);
        if fire && aim.locks > 0 {
            self.fire(player, aim.locks, out);
        } else if input.guard && scene.foe.is_some() && range <= t::AIRSTEP_RANGE {
            self.aim = None;
            let d = scene.foe.map_or(0., |_| range);
            self.velocity = [0.; 3];
            self.mode = Mode::Airstep {
                tick: 0,
                speed: airstep_speed(d),
            };
            self.shown = Some(player.action);
            out.began.push("airstep");
        } else if self.focus <= 0. {
            self.aim = None;
            out.began.push("aim_empty");
        }
    }

    fn fire(&mut self, player: &mut Player, locks: u8, out: &mut Outcome) {
        self.aim = None;
        let ragnarok = locks >= t::MAX_LOCKS;
        self.velocity = [0.; 3];
        self.volley_dealt = 0;
        self.mode = Mode::Volley {
            tick: 0,
            locks,
            ragnarok,
            base: player.position[1],
            spawned: 0,
        };
        self.shown = Some(player.action);
        out.began.push(if ragnarok {
            "ragnarok"
        } else {
            "king_of_hearts"
        });
    }

    /// The volley: lifted, invulnerable, the ring of bolts, the launch.
    fn volley(
        &mut self,
        player: &mut Player,
        p: &Params,
        clips: &Clips,
        world: &impl World,
        scene: &Scene,
        out: &mut Outcome,
    ) {
        let Mode::Volley {
            tick,
            locks,
            ragnarok,
            mut base,
            mut spawned,
        } = self.mode
        else {
            return;
        };
        let tick = tick + 1;
        player.invulnerable = player.invulnerable.max(2.);
        if tick == 1 {
            let floor = world
                .floor(
                    player.position[0],
                    player.position[2],
                    player.position[1] + 1.,
                )
                .unwrap_or(player.position[1]);
            base = floor + t::VOLLEY_LIFT;
        }
        // Up to the lift's height.
        let left = t::VOLLEY_LIFT_TICKS.saturating_sub(tick) as f32 + 1.;
        let dy = (base - player.position[1]) / left;
        Self::travel(player, [0., dy, 0.], true, p, world);
        if let Some(f) = scene.foe {
            let to = yaw_of([
                f.position[0] - player.position[0],
                f.position[2] - player.position[2],
            ]);
            player.yaw = if tick == 1 {
                to
            } else {
                turn(player.yaw, to, 0.2)
            };
        }
        let (launch, end) = if ragnarok {
            (t::RAGNAROK_LAUNCH, t::RAGNAROK_END)
        } else {
            (t::KOH_LAUNCH, t::KOH_END)
        };
        let count = if ragnarok { t::MAX_LOCKS } else { locks };
        if tick == launch {
            out.cues.push("bolt_launch");
        }
        let ring_from = launch.saturating_sub(t::RING_OPEN + 4);
        if tick == ring_from {
            out.cues.push("bolt_ring");
        }
        if tick >= ring_from && spawned < count {
            let n = t::VOLLEY_PER_TICK.min(count - spawned);
            for _ in 0..n {
                self.bolt_serial = self.bolt_serial.wrapping_add(1);
                let serial = self.bolt_serial;
                self.bolts.push(Bolt {
                    serial,
                    pos: [
                        player.position[0],
                        player.position[1] + t::CHEST,
                        player.position[2],
                    ],
                    dir: [0., 0., 1.],
                    speed: 0.,
                    ragnarok,
                    slot: spawned,
                    count,
                    age: 0,
                    flight: None,
                    homing: true,
                });
                out.spawned.push(serial);
                spawned += 1;
            }
        }
        // The ring around his chest, opening and spinning, until the launch.
        let f = facing(player.yaw);
        let centre = [
            player.position[0] + f[0] * t::length_of(60.),
            player.position[1] + t::CHEST,
            player.position[2] + f[1] * t::length_of(60.),
        ];
        let right = [f[1], 0., -f[0]];
        let spin = if ragnarok { -1. } else { 1. };
        for b in self.bolts.iter_mut().filter(|b| b.flight.is_none()) {
            let open = (b.age as f32 / t::RING_OPEN as f32).min(1.);
            let a = TAU * f32::from(b.slot) / f32::from(b.count.max(1))
                + spin * t::RING_SPIN * b.age as f32;
            let r = t::RING_RADIUS * open;
            let off = [right[0] * a.cos() * r, a.sin() * r, right[2] * a.cos() * r];
            b.pos = [centre[0] + off[0], centre[1] + off[1], centre[2] + off[2]];
            if tick >= launch {
                // Out along the cone (60°) toward the target.
                let ahead = [f[0], 0., f[1]];
                let o = unit3(off);
                b.dir = unit3([
                    ahead[0] + o[0] * 0.58,
                    ahead[1] + o[1] * 0.58,
                    ahead[2] + o[2] * 0.58,
                ]);
                b.speed = if ragnarok {
                    t::RAGNAROK_SPEED
                } else {
                    t::KOH_SPEED
                };
                b.flight = Some(0);
            }
        }
        self.show(
            player,
            Action::Growth(growth::motion::GLIDE_STAND),
            clips,
            &mut out.step,
        );
        if tick >= end {
            self.fall(player, [0.; 3], clips, &mut out.step);
        } else {
            self.mode = Mode::Volley {
                tick,
                locks,
                ragnarok,
                base,
                spawned,
            };
        }
    }

    /// Focus Syphon: in place and invulnerable while the pull plays, then
    /// the focus.
    fn syphon(
        &mut self,
        player: &mut Player,
        p: &Params,
        clips: &Clips,
        world: &impl World,
        out: &mut Outcome,
    ) {
        let Mode::Syphon { tick, locks } = self.mode else {
            return;
        };
        let tick = tick + 1;
        player.invulnerable = player.invulnerable.max(2.);
        self.show(
            player,
            Action::Growth(growth::motion::GLIDE_STAND),
            clips,
            &mut out.step,
        );
        if tick < t::SYPHON_TICKS {
            self.mode = Mode::Syphon { tick, locks };
            return;
        }
        self.focus = (self.focus + (t::SYPHON_RATE * f32::from(locks)).trunc()).min(t::FOCUS_MAX);
        out.cues.push("syphon_gain");
        let below = world.floor(
            player.position[0],
            player.position[2],
            player.position[1] + p.step_height,
        );
        if below.is_some_and(|f| player.position[1] - f < 1.) {
            self.velocity = [0.; 3];
            self.mode = Mode::Ground;
            self.show(player, Action::Idle, clips, &mut out.step);
        } else {
            self.fall(player, [0.; 3], clips, &mut out.step);
        }
    }

    /// The bolts' flight: homing on the target point.
    fn step_bolts(&mut self) {
        let foe = self.foe;
        self.bolts.retain_mut(|b| {
            b.age += 1;
            let Some(flight) = b.flight.as_mut() else {
                // A ring that outlived its volley (a hit broke it) fades.
                return b.age < t::RING_OPEN * 4;
            };
            *flight += 1;
            let life = if b.ragnarok {
                t::RAGNAROK_LIFE
            } else {
                t::KOH_LIFE
            };
            if *flight > life {
                return false;
            }
            let rate = if b.ragnarok {
                b.speed = (b.speed + t::RAGNAROK_ACCEL).min(t::RAGNAROK_MAX);
                t::RAGNAROK_TURN
            } else if *flight >= t::KOH_HOMING_FROM {
                (t::KOH_TURN + t::KOH_TURN_RISE * (*flight - t::KOH_HOMING_FROM) as f32)
                    .min(t::KOH_TURN_MAX)
            } else {
                0.
            };
            if let Some(c) = foe
                && b.homing
            {
                let to = [c[0] - b.pos[0], c[1] - b.pos[1], c[2] - b.pos[2]];
                if len3(to) < t::HOMING_OFF {
                    b.homing = false;
                } else if rate > 0. {
                    b.dir = home(b.dir, to, rate);
                }
            }
            for k in 0..3 {
                b.pos[k] += b.dir[k] * b.speed;
            }
            true
        });
    }
}

/// Turns `dir` toward `to` by at most `rate` radians.
fn home(dir: Vec3, to: Vec3, rate: f32) -> Vec3 {
    let u = unit3(to);
    let cos = (dir[0] * u[0] + dir[1] * u[1] + dir[2] * u[2]).clamp(-1., 1.);
    let angle = cos.acos();
    if angle <= rate || angle < 1e-6 {
        return u;
    }
    let s = angle.sin();
    if s.abs() < 1e-6 {
        return dir;
    }
    let a = (angle - rate).sin() / s;
    let b = rate.sin() / s;
    unit3([
        dir[0] * a + u[0] * b,
        dir[1] * a + u[1] * b,
        dir[2] * a + u[2] * b,
    ])
}

/// The modes by snapshot code.
pub const MODE_NAMES: [&str; 33] = [
    "kh2",
    "ground",
    "air",
    "doubleflight",
    "flow_jump",
    "takeoff",
    "land",
    "dodge_roll",
    "air_slide",
    "glide",
    "superslide",
    "slide_end",
    "wall_run_up",
    "wall_run_side",
    "wall_kick",
    "pole_spin",
    "airstep",
    "airstep_hop",
    "king_of_hearts",
    "ragnarok",
    "flow_attack",
    "launch",
    "flinch",
    "air_flinch",
    "blown",
    "beaten",
    "tossed",
    "down",
    "aerial_recovery",
    "final_blow",
    "risk_warp",
    "risk_dodge",
    "focus_syphon",
];

/// A mode as the joiner shows it (its HUD and camera read only the kind;
/// a wall's normal comes back as zero, for the camera to take from the
/// facing).
fn mode_named(name: &str) -> Option<Mode> {
    Some(match name {
        "kh2" => Mode::Kh2,
        "ground" => Mode::Ground,
        "wall_run_up" | "wall_run_side" => Mode::WallRun {
            normal: [0.; 2],
            up: name == "wall_run_up",
            side: 0.,
            tick: 0,
            idle: 0,
        },
        "wall_kick" => Mode::WallKick {
            tick: 0,
            normal: [0.; 2],
            guard: false,
        },
        "king_of_hearts" | "ragnarok" => Mode::Volley {
            tick: 0,
            locks: 0,
            ragnarok: name == "ragnarok",
            base: 0.,
            spawned: 0,
        },
        _ => Mode::Air {
            jump: Jump::None,
            hang: 0,
            released: true,
            tick: 0,
        },
    })
}

/// What the joiner is told of a Sora's KH3 state.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Shown {
    pub focus: u8,
    pub style: u8,
    pub mode: u8,
    pub locks: u8,
    pub aiming: bool,
    /// The aim is Focus Syphon's.
    pub syphon: bool,
    /// The counter, Final Blow and the Risk Dodge are offered.
    pub counter: bool,
    pub chase: bool,
    pub risk: bool,
    /// The offered grand magic's type (0: none).
    pub offer: u8,
    pub lean: [f32; 2],
}

#[cfg(test)]
mod tests;
