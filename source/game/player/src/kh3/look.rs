//! What the KH3 moves look like: each look is a few of KH2FM's own effect
//! elements started together on a Sora, each with its own bone, place,
//! size, colour and time, so no two moves share one plain element. One
//! visual language runs through them: flowmotion is blue-white wind,
//! the shotlock cyan rings and beams, guards and counters gold, KH3's
//! heavy hits white-orange sparks sized by the hit, focus the gauge's own
//! blue. Every layer ends by its `life` (or its own length), so nothing
//! outlasts its move; a lasting move repeats its look now and then
//! (`cue::periodic`). The duel sends a look by its id: the joiner starts
//! the same layers from its own copy of these tables. [duel choice: every
//! look here]

/// Whose effect container an element is in.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Source {
    /// Sora's own object (`P_EX100`, the base form's), whatever form he
    /// is in.
    Sora,
    /// A Drive form's object (`forms::VALOR` ..), once its kit is loaded.
    Form(u8),
    /// The system container (`00effect.bar`).
    System,
}

/// Bone codes a layer can start on (FUN_1403b50f0's, and the object's).
pub mod bone {
    /// The object's origin (his feet).
    pub const FEET: i16 = 0x4000;
    /// The body centre (his first type-1 collision entry's height).
    pub const BODY: i16 = 0x4001;
    /// The main Keyblade's tip.
    pub const BLADE: i16 = 0x4003;
    /// Halfway along the main Keyblade.
    pub const MID_BLADE: i16 = 0x4007;
}

/// One element of a look.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Layer {
    pub source: Source,
    pub element: u16,
    /// The bone it starts on instead of its own (`bone`).
    pub bone: Option<i16>,
    /// Added to its own place, in cm: right, up, forward of the bone.
    pub offset: [f32; 3],
    /// Times its own size.
    pub scale: f32,
    /// Times its colours (RGBA, 1 = as drawn).
    pub tint: [f32; 4],
    /// Ticks after the look starts.
    pub delay: u16,
    /// Ticks it runs before fading (0: its own length; every look's layer
    /// has one, since some elements loop until they are stopped).
    pub life: u16,
    /// Whether the element's own sound plays (the cues carry the moves'
    /// sounds, so mostly not).
    pub sound: bool,
}

impl Layer {
    const fn of(source: Source, element: u16) -> Layer {
        Layer {
            source,
            element,
            bone: None,
            offset: [0.; 3],
            scale: 1.,
            tint: [1.; 4],
            delay: 0,
            life: 0,
            sound: false,
        }
    }
    const fn on(mut self, bone: i16) -> Layer {
        self.bone = Some(bone);
        self
    }
    const fn at(mut self, offset: [f32; 3]) -> Layer {
        self.offset = offset;
        self
    }
    const fn size(mut self, scale: f32) -> Layer {
        self.scale = scale;
        self
    }
    const fn tint(mut self, tint: [f32; 4]) -> Layer {
        self.tint = tint;
        self
    }
    const fn after(mut self, ticks: u16) -> Layer {
        self.delay = ticks;
        self
    }
    const fn lasts(mut self, ticks: u16) -> Layer {
        self.life = ticks;
        self
    }
}

/// The palette.
pub mod colour {
    /// Flowmotion's wind: white with a cold blue.
    pub const WIND: [f32; 4] = [0.62, 0.86, 1.35, 1.];
    /// A paler wind for the long dashes' streaks.
    pub const GUST: [f32; 4] = [0.85, 0.95, 1.2, 0.85];
    /// The shotlock: cyan.
    pub const LOCK: [f32; 4] = [0.35, 1.2, 1.45, 1.];
    /// Guards, counters and the chase: gold.
    pub const GOLD: [f32; 4] = [1.45, 1.08, 0.38, 1.];
    /// KH3's hits: white going orange.
    pub const SPARK: [f32; 4] = [1.4, 0.92, 0.5, 1.];
    /// The focus gauge's blue (the HUD bar's colour).
    pub const FOCUS: [f32; 4] = [0.45, 1.0, 1.6, 1.];
}

use Source::{Form, Sora, System};
use bone::*;
use colour::*;

/// The forms whose containers the looks borrow from.
const VALOR: u8 = crate::forms::VALOR;
const WISDOM: u8 = crate::forms::WISDOM;
const MASTER: u8 = crate::forms::MASTER;
const FINAL: u8 = crate::forms::FINAL;

/// Wisdom's Quick Run swoosh (its level 3 element). Sora's own level 2
/// element (110), the old stand-in, draws a wide white sheet across the
/// screen unless he runs fast, so no look uses it.
const SWOOSH: u16 = 0x6c + 3;

/// Every look, by id (its place in [`LOOKS`]).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum Look {
    AirSlide,
    Doubleflight,
    FlowJump,
    Glide,
    GlideTrail,
    Superslide,
    SlideTrail,
    SlideEnd,
    WallRun,
    WallStep,
    WallKick,
    WallTop,
    PoleSpin,
    PoleTurn,
    Airstep,
    AirstepTrail,
    FlowAttack,
    Launch,
    Counter,
    Aim,
    Lock,
    MaxLocks,
    BoltRing,
    BoltHit,
    Volley,
    FinalBlow,
    ChaseWarp,
    RiskDodge,
    RiskWarp,
    FocusSyphon,
    SyphonGain,
    FocusFull,
    HeavyHit,
    AerialRecovery,
}

/// Every look in id order.
pub const ALL: [Look; 34] = [
    Look::AirSlide,
    Look::Doubleflight,
    Look::FlowJump,
    Look::Glide,
    Look::GlideTrail,
    Look::Superslide,
    Look::SlideTrail,
    Look::SlideEnd,
    Look::WallRun,
    Look::WallStep,
    Look::WallKick,
    Look::WallTop,
    Look::PoleSpin,
    Look::PoleTurn,
    Look::Airstep,
    Look::AirstepTrail,
    Look::FlowAttack,
    Look::Launch,
    Look::Counter,
    Look::Aim,
    Look::Lock,
    Look::MaxLocks,
    Look::BoltRing,
    Look::BoltHit,
    Look::Volley,
    Look::FinalBlow,
    Look::ChaseWarp,
    Look::RiskDodge,
    Look::RiskWarp,
    Look::FocusSyphon,
    Look::SyphonGain,
    Look::FocusFull,
    Look::HeavyHit,
    Look::AerialRecovery,
];

impl Look {
    /// The look of an id from the wire.
    pub fn from_id(id: u8) -> Option<Look> {
        ALL.get(usize::from(id)).copied()
    }

    pub fn id(self) -> u8 {
        self as u8
    }

    /// Its name in the logs.
    pub fn name(self) -> &'static str {
        match self {
            Look::AirSlide => "air_slide",
            Look::Doubleflight => "doubleflight",
            Look::FlowJump => "flow_jump",
            Look::Glide => "glide",
            Look::GlideTrail => "glide_trail",
            Look::Superslide => "superslide",
            Look::SlideTrail => "slide_trail",
            Look::SlideEnd => "slide_end",
            Look::WallRun => "wall_run",
            Look::WallStep => "wall_step",
            Look::WallKick => "wall_kick",
            Look::WallTop => "wall_top",
            Look::PoleSpin => "pole_spin",
            Look::PoleTurn => "pole_turn",
            Look::Airstep => "airstep",
            Look::AirstepTrail => "airstep_trail",
            Look::FlowAttack => "flow_attack",
            Look::Launch => "launch",
            Look::Counter => "counter",
            Look::Aim => "aim",
            Look::Lock => "lock",
            Look::MaxLocks => "max_locks",
            Look::BoltRing => "bolt_ring",
            Look::BoltHit => "bolt_hit",
            Look::Volley => "volley",
            Look::FinalBlow => "final_blow",
            Look::ChaseWarp => "chase_warp",
            Look::RiskDodge => "risk_dodge",
            Look::RiskWarp => "risk_warp",
            Look::FocusSyphon => "focus_syphon",
            Look::SyphonGain => "syphon_gain",
            Look::FocusFull => "focus_full",
            Look::HeavyHit => "heavy_hit",
            Look::AerialRecovery => "aerial_recovery",
        }
    }

    /// Its layers.
    pub fn layers(self) -> &'static [Layer] {
        match self {
            Look::AirSlide => AIR_SLIDE,
            Look::Doubleflight => DOUBLEFLIGHT,
            Look::FlowJump => FLOW_JUMP,
            Look::Glide => GLIDE,
            Look::GlideTrail => GLIDE_TRAIL,
            Look::Superslide => SUPERSLIDE,
            Look::SlideTrail => SLIDE_TRAIL,
            Look::SlideEnd => SLIDE_END,
            Look::WallRun => WALL_RUN,
            Look::WallStep => WALL_STEP,
            Look::WallKick => WALL_KICK,
            Look::WallTop => WALL_TOP,
            Look::PoleSpin => POLE_SPIN,
            Look::PoleTurn => POLE_TURN,
            Look::Airstep => AIRSTEP,
            Look::AirstepTrail => AIRSTEP_TRAIL,
            Look::FlowAttack => FLOW_ATTACK,
            Look::Launch => LAUNCH,
            Look::Counter => COUNTER,
            Look::Aim => AIM,
            Look::Lock => LOCK_ON,
            Look::MaxLocks => MAX_LOCKS,
            Look::BoltRing => BOLT_RING,
            Look::BoltHit => BOLT_HIT,
            Look::Volley => VOLLEY,
            Look::FinalBlow => FINAL_BLOW,
            Look::ChaseWarp => CHASE_WARP,
            Look::RiskDodge => RISK_DODGE,
            Look::RiskWarp => RISK_WARP,
            Look::FocusSyphon => FOCUS_SYPHON,
            Look::SyphonGain => SYPHON_GAIN,
            Look::FocusFull => FOCUS_FULL,
            Look::HeavyHit => HEAVY_HIT,
            Look::AerialRecovery => AERIAL_RECOVERY,
        }
    }

    /// The ticks until its last layer starts fading at the latest (its
    /// delays plus lives).
    pub fn span(self) -> u16 {
        self.layers()
            .iter()
            .map(|l| l.delay + l.life)
            .max()
            .unwrap_or(0)
    }
}

/// A look's place in the visual language: its colours.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Family {
    /// Flowmotion and the dashes: wind.
    Flow,
    /// The shotlock: cyan.
    Shotlock,
    /// Guards, counters, the chase, the Risk Dodge: gold.
    Guard,
    /// The focus gauge: its blue.
    Focus,
    /// KH3's hits: white-orange.
    Hit,
    /// A KH2 move's own look, untinted.
    Kh2,
}

impl Family {
    /// The tints its layers take.
    pub fn palette(self) -> &'static [[f32; 4]] {
        match self {
            Family::Flow => &[WIND, GUST],
            Family::Shotlock => &[LOCK],
            Family::Guard => &[GOLD],
            Family::Focus => &[FOCUS],
            Family::Hit => &[SPARK],
            Family::Kh2 => &[[1.; 4]],
        }
    }
}

impl Look {
    pub fn family(self) -> Family {
        use Look::*;
        match self {
            AirSlide | Doubleflight | FlowJump | Glide | GlideTrail | Superslide | SlideTrail
            | SlideEnd | WallRun | WallStep | WallKick | WallTop | PoleSpin | PoleTurn
            | Airstep | AirstepTrail | FlowAttack => Family::Flow,
            Aim | Lock | MaxLocks | BoltRing | BoltHit | Volley => Family::Shotlock,
            Counter | FinalBlow | ChaseWarp | RiskDodge | RiskWarp => Family::Guard,
            FocusSyphon | SyphonGain | FocusFull => Family::Focus,
            Launch | HeavyHit => Family::Hit,
            AerialRecovery => Family::Kh2,
        }
    }
}

// Flowmotion: blue-white wind. Wisdom's Quick Run swoosh (111), Master's
// speed lines (22), its ground arcs (23, 24), halo rings (25, 26), take-off
// burst (30) and spin (32), Aerial Recovery's arc, Final's star (49).

const AIR_SLIDE: &[Layer] = &[
    Layer::of(Form(WISDOM), SWOOSH).tint(WIND).lasts(22),
    Layer::of(Form(MASTER), 22)
        .tint(GUST)
        .size(0.7)
        .after(2)
        .lasts(16),
];
const DOUBLEFLIGHT: &[Layer] = &[
    Layer::of(Sora, 0).tint(WIND).size(0.6).lasts(22),
    Layer::of(Form(MASTER), 30)
        .tint(GUST)
        .size(0.45)
        .after(2)
        .lasts(18),
];
const FLOW_JUMP: &[Layer] = &[
    Layer::of(Form(VALOR), 22).tint(WIND).size(0.6).lasts(22),
    Layer::of(Form(MASTER), 30).tint(GUST).size(0.6).lasts(20),
];
const GLIDE: &[Layer] = &[
    Layer::of(Form(MASTER), 25)
        .tint(WIND)
        .size(0.55)
        .at([0., 60., 0.])
        .lasts(30),
    Layer::of(Form(MASTER), 22).tint(GUST).size(0.6).lasts(20),
];
const GLIDE_TRAIL: &[Layer] = &[Layer::of(Form(MASTER), 26)
    .tint(GUST)
    .size(0.5)
    .at([0., 60., 0.])
    .lasts(26)];
const SUPERSLIDE: &[Layer] = &[
    Layer::of(Form(MASTER), 23).tint(WIND).size(0.8).lasts(24),
    Layer::of(Form(MASTER), 22)
        .tint(GUST)
        .size(0.8)
        .after(4)
        .lasts(20),
];
const SLIDE_TRAIL: &[Layer] = &[Layer::of(Form(MASTER), 24).tint(GUST).size(0.7).lasts(22)];
const SLIDE_END: &[Layer] = &[Layer::of(Sora, 36).tint(GUST).size(0.8).lasts(20)];
const WALL_RUN: &[Layer] = &[
    Layer::of(Form(VALOR), 24).tint(WIND).size(0.6).lasts(24),
    Layer::of(Sora, 54).tint(GUST).lasts(18),
];
const WALL_STEP: &[Layer] = &[Layer::of(Sora, 54).tint(GUST).size(0.7).lasts(14)];
const WALL_KICK: &[Layer] = &[
    Layer::of(Form(MASTER), 30).tint(WIND).size(0.7).lasts(22),
    Layer::of(Form(VALOR), 22)
        .tint(GUST)
        .size(0.5)
        .after(2)
        .lasts(18),
];
const WALL_TOP: &[Layer] = &[Layer::of(Sora, 45).tint(WIND).size(0.5).lasts(24)];
const POLE_SPIN: &[Layer] = &[Layer::of(Form(MASTER), 32).tint(WIND).size(0.8).lasts(30)];
const POLE_TURN: &[Layer] = &[Layer::of(Form(FINAL), 24).tint(GUST).size(0.5).lasts(18)];
const AIRSTEP: &[Layer] = &[
    Layer::of(Form(FINAL), 49)
        .tint(WIND)
        .size(0.6)
        .at([0., 80., 0.])
        .lasts(18),
    Layer::of(Form(WISDOM), SWOOSH)
        .tint(WIND)
        .after(2)
        .lasts(22),
];
const AIRSTEP_TRAIL: &[Layer] = &[Layer::of(Form(MASTER), 22).tint(GUST).size(0.6).lasts(18)];
const FLOW_ATTACK: &[Layer] = &[
    Layer::of(Sora, 33).tint(WIND).lasts(20),
    Layer::of(Form(FINAL), 23).tint(GUST).size(0.55).lasts(22),
];

// Guards, counters, the chase and the Risk Dodge: gold. Wisdom's hit
// slash (22) and flash (102), Explosion's rings (48, 49), Final's slashes
// (52), star (49) and beam (51), Master's ground arc (24). (Final's ground
// arcs, 27 to 30, draw a wide flat band from the duel's height.)

const COUNTER: &[Layer] = &[
    Layer::of(Form(WISDOM), 22)
        .on(BLADE)
        .tint(GOLD)
        .size(0.8)
        .lasts(24),
    Layer::of(Sora, 48).tint(GOLD).size(0.6).lasts(24),
];
const FINAL_BLOW: &[Layer] = &[
    Layer::of(Form(FINAL), 52).tint(GOLD).size(0.8).lasts(30),
    Layer::of(Form(FINAL), 49)
        .tint(GOLD)
        .size(0.45)
        .at([0., 80., 0.])
        .after(4)
        .lasts(18),
];
const CHASE_WARP: &[Layer] = &[
    Layer::of(Form(WISDOM), 102).tint(GOLD).lasts(20),
    Layer::of(Form(FINAL), 51).tint(GOLD).size(0.6).lasts(20),
];
const RISK_DODGE: &[Layer] = &[
    Layer::of(Form(MASTER), 24).tint(GOLD).size(0.6).lasts(22),
    Layer::of(Sora, 36).tint(GOLD).lasts(18),
];
const RISK_WARP: &[Layer] = &[
    Layer::of(Form(FINAL), 49)
        .tint(GOLD)
        .size(0.7)
        .at([0., 80., 0.])
        .lasts(18),
    Layer::of(Sora, 49).tint(GOLD).size(0.45).lasts(22),
];

// The shotlock: cyan. Wisdom's orbit rings (21), pillars (101), muzzle
// (25) and wide hit (24), Explosion's rings (48, 49), Final's star (49).

const AIM: &[Layer] = &[Layer::of(Form(WISDOM), 21)
    .tint(LOCK)
    .size(0.7)
    .at([0., 50., 0.])
    .lasts(24)];
const LOCK_ON: &[Layer] = &[Layer::of(Form(FINAL), 49).tint(LOCK).size(0.35).lasts(10)];
const MAX_LOCKS: &[Layer] = &[
    Layer::of(Sora, 49).tint(LOCK).size(0.5).lasts(22),
    Layer::of(Form(WISDOM), 21)
        .tint(LOCK)
        .size(0.9)
        .at([0., 50., 0.])
        .after(3)
        .lasts(24),
];
const BOLT_RING: &[Layer] = &[Layer::of(Form(WISDOM), 25).tint(LOCK).size(0.5).lasts(16)];
const BOLT_HIT: &[Layer] = &[Layer::of(Form(WISDOM), 24).tint(LOCK).size(0.4).lasts(16)];
const VOLLEY: &[Layer] = &[
    Layer::of(Form(WISDOM), 101).tint(LOCK).size(0.8).lasts(30),
    Layer::of(Sora, 48).tint(LOCK).size(0.65).lasts(26),
];

// Focus: the gauge's blue. Aerial Finish's ring at the blade (37),
// Master's arrival arcs (100), the prize glow (system 6), the revert's arc
// (15), Explosion's arcs (45).

const FOCUS_SYPHON: &[Layer] = &[
    Layer::of(Sora, 37).tint(FOCUS).lasts(20),
    Layer::of(Form(MASTER), 100).tint(FOCUS).size(0.8).lasts(24),
];
const SYPHON_GAIN: &[Layer] = &[
    Layer::of(System, 6).tint(FOCUS).size(1.2).lasts(20),
    Layer::of(Sora, 15).tint(FOCUS).size(0.6).lasts(24),
];
const FOCUS_FULL: &[Layer] = &[Layer::of(Sora, 45).tint(FOCUS).size(0.6).lasts(26)];

// KH3's hits: white-orange. Sora's own hit burst (14), the alternate hit
// spark (system 18), Valor's rising arc (18), Master's burst (30).

const LAUNCH: &[Layer] = &[
    Layer::of(Form(VALOR), 18).tint(SPARK).size(0.7).lasts(20),
    Layer::of(Form(MASTER), 30).tint(SPARK).size(0.45).lasts(18),
];
const HEAVY_HIT: &[Layer] = &[
    Layer::of(Sora, 14).tint(SPARK).size(0.55).lasts(20),
    Layer::of(System, 18).tint(SPARK).lasts(16),
];
// KH2's own Aerial Recovery (motion 40's element 0), as it was.
const AERIAL_RECOVERY: &[Layer] = &[Layer::of(Sora, 0).lasts(30)];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ids_round_trip_and_names_are_unique() {
        for (i, l) in ALL.iter().enumerate() {
            assert_eq!(usize::from(l.id()), i, "{l:?}");
            assert_eq!(Look::from_id(l.id()), Some(*l));
        }
        assert_eq!(Look::from_id(ALL.len() as u8), None);
        let mut names: Vec<_> = ALL.iter().map(|l| l.name()).collect();
        names.sort_unstable();
        names.dedup();
        assert_eq!(names.len(), ALL.len());
    }

    #[test]
    fn every_layer_ends_soon_and_stays_in_range() {
        for l in ALL {
            let layers = l.layers();
            assert!((1..=3).contains(&layers.len()), "{l:?}");
            for y in layers {
                // Some elements loop until stopped: each layer has a life,
                // within about half a second of the look's start.
                assert!((8..=30).contains(&y.life), "{l:?}: {y:?}");
                assert!(y.delay <= 6, "{l:?}");
                assert!((0.3..=1.3).contains(&y.scale), "{l:?}: {y:?}");
                assert!(y.tint.iter().all(|c| (0. ..=1.6).contains(c)), "{l:?}");
                assert!(y.offset.iter().all(|c| c.abs() <= 100.), "{l:?}");
                // The cues carry the moves' sounds.
                assert!(!y.sound, "{l:?}");
                // Sora's level-2 Quick Run element draws a white sheet.
                assert!(!(y.source == Sora && y.element == 0x6c + 2), "{l:?}");
            }
            assert!(l.span() <= 36, "{l:?}: {}", l.span());
        }
    }

    #[test]
    fn no_two_looks_are_the_same_and_each_keeps_its_family_s_colours() {
        let key = |l: Look| {
            let mut k: Vec<_> = l
                .layers()
                .iter()
                .map(|y| (y.source, y.element, y.tint.map(f32::to_bits)))
                .collect();
            k.sort_unstable_by_key(|x| format!("{x:?}"));
            k
        };
        for (i, a) in ALL.iter().enumerate() {
            for b in &ALL[i + 1..] {
                assert_ne!(key(*a), key(*b), "{a:?} and {b:?}");
                // Nor share their first, most visible layer in one colour.
                let (x, y) = (a.layers()[0], b.layers()[0]);
                assert!(
                    (x.source, x.element, x.tint) != (y.source, y.element, y.tint),
                    "{a:?} and {b:?} lead with the same element"
                );
            }
            for y in a.layers() {
                assert!(a.family().palette().contains(&y.tint), "{a:?}: {y:?}");
            }
        }
    }

    #[test]
    fn the_system_elements_that_mean_other_things_stay_out() {
        // System 0 (an item's flash, the duel's GO), 11 to 13 (the guard
        // and clash flashes), 14 (the KO marker), 15 and 16 (level-ups, the
        // duel's win): a KH3 look would read as one of those.
        for l in ALL {
            for y in l.layers() {
                if y.source == System {
                    assert!(!matches!(y.element, 0 | 11..=16), "{l:?}: {y:?}");
                }
            }
        }
    }
}
