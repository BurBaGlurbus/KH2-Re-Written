//! What the KH3 moves sound and look like: KH2FM's own sounds and effect
//! elements stand in for KH3's, chosen by what each move is (a dash takes
//! Quick Run's afterimage, a shotlock lock the lock-on sound, a bolt
//! Wisdom Form's shot). Moves that play a KH2 motion already have that
//! motion's frame-trigger sounds, voices and effects; the cues here fill
//! what those motions do not have. Each cue is a one-shot: a move that
//! lasts repeats a cue now and then while it runs (`periodic`), so nothing
//! loops past its move. [duel choice: every pairing here]

use super::{Mode, tuning as t};

/// A KH2 stand-in the room plays for a Sora.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Cue {
    /// A sound request id (bank x 1000 + index): bank 0 (system), se500
    /// (Sora's swings and hits), se999 (the resident ability and form
    /// bank).
    Sound(u32),
    /// An element of the Sora's own effect container (his form's), bound
    /// to him.
    Own(u16),
    /// An element of Wisdom Form's container, bound to him.
    Wisdom(u16),
    /// An element of the system container, bound to him.
    System(u16),
}

/// Quick Run's afterimage at level 2 (element 0x6c + 2 of Sora's own
/// effects; its sound is se999's 999018).
pub const AFTERIMAGE: Cue = Cue::Own(0x6c + 2);
/// Wisdom Form's muzzle flash (element 25, SE 999011) and its shot's hit
/// spark (element 21, SE 999012).
pub const MUZZLE: Cue = Cue::Wisdom(25);
pub const BOLT_SPARK: Cue = Cue::Wisdom(21);
/// The clash flash (system element 13, SE 107).
pub const FLASH: Cue = Cue::System(13);

/// The sounds by name: the jump's take-off and the dodge roll's cloth
/// (frame triggers of motions 201 and 209), a running step's cloth (301),
/// the swings' whooshes in se500, the lock-on
/// and lock-off (7, 8), the cursor tick (1), the refusal (5), the Drive
/// gauge's "full" chime (33) and the Drive orb's pickup (31).
pub mod sound {
    pub const JUMP: u32 = 303;
    pub const CLOTH: u32 = 299;
    pub const STEP: u32 = 301;
    pub const WHOOSH: u32 = 500_044;
    pub const WHOOSH_LONG: u32 = 500_046;
    pub const WHOOSH_SHORT: u32 = 500_043;
    pub const WHOOSH_SPIN: u32 = 500_036;
    pub const WHOOSH_BIG: u32 = 500_048;
    pub const LOCK_ON: u32 = 7;
    pub const LOCK_OFF: u32 = 8;
    pub const TICK: u32 = 1;
    pub const REFUSED: u32 = 5;
    pub const READY: u32 = 33;
    pub const ORB: u32 = 31;
    pub const WISDOM_SHOT: u32 = 999_011;
}

/// Every event with cues: the moves' names (`Mode::name`, and the layer's
/// own `began` names) and the moments of `Outcome::cues` and the room.
pub const EVENTS: [&str; 44] = [
    "dodge_roll",
    "air_slide",
    "doubleflight",
    "flow_jump",
    "glide",
    "superslide",
    "slide_end",
    "wall_run_up",
    "wall_run_side",
    "wall_kick",
    "wall_top",
    "pole_spin",
    "airstep",
    "airstep_hop",
    "king_of_hearts",
    "ragnarok",
    "flow_attack",
    "launch",
    "counter",
    "aim",
    "aim_cancel",
    "aim_empty",
    "flinch",
    "air_flinch",
    "blown",
    "beaten",
    "tossed",
    "down",
    "aerial_recovery",
    "final_blow",
    "chase_warp",
    "risk_dodge",
    "risk_warp",
    "focus_syphon",
    "lock",
    "max_locks",
    "bolt_ring",
    "bolt_launch",
    "bolt_hit",
    "syphon_gain",
    "wall_step",
    "dash_trail",
    "pole_turn",
    "grand_offer",
];

/// The cues of an event. Events whose KH2 motion already sounds (the
/// reactions' damage motions and their voices, the flow attacks' and the
/// counter's swings, the dodge roll, the jumps) add only what the motion
/// lacks; an empty list means the motion alone stands in.
pub fn of(event: &str) -> &'static [Cue] {
    use Cue::Sound as S;
    use sound::*;
    match event {
        // The roll's motion has its cloth sound, voice and dust.
        "dodge_roll" => &[],
        "air_slide" => &[AFTERIMAGE, S(WHOOSH)],
        // Aerial Dodge's motion plays 999019.
        "doubleflight" => &[AFTERIMAGE],
        // High Jump's motion plays the take-off.
        "flow_jump" => &[AFTERIMAGE],
        // KH2's Glide has no sound or effect of its own.
        "glide" => &[S(WHOOSH)],
        "superslide" => &[AFTERIMAGE, S(WHOOSH_LONG)],
        "slide_end" => &[S(CLOTH)],
        "wall_run_up" | "wall_run_side" => &[S(JUMP)],
        "wall_kick" => &[S(JUMP), S(WHOOSH_SHORT)],
        "wall_top" => &[S(JUMP)],
        "pole_spin" => &[S(CLOTH)],
        "airstep" => &[AFTERIMAGE, Cue::Wisdom(0x6c + 4), S(WHOOSH_LONG)],
        // Aerial Dodge's motion plays 999019.
        "airstep_hop" => &[],
        "king_of_hearts" | "ragnarok" => &[S(WHOOSH_BIG)],
        "flow_attack" => &[AFTERIMAGE],
        // The record's swing has its voice, whoosh and trail.
        "launch" => &[],
        "counter" => &[FLASH],
        "aim" => &[S(LOCK_ON)],
        "aim_cancel" => &[S(LOCK_OFF)],
        "aim_empty" => &[S(REFUSED)],
        // The damage motions carry their voices; the hit its sound and
        // spark.
        "flinch" | "air_flinch" | "blown" | "beaten" | "tossed" => &[],
        // No KH2 sound marks a knock-down's landing: the roll's cloth.
        "down" => &[S(CLOTH)],
        // Motion 40's element 0 (frame trigger 1 skips element 0).
        "aerial_recovery" => &[Cue::Own(0)],
        "final_blow" => &[AFTERIMAGE, S(WHOOSH_BIG)],
        "chase_warp" | "risk_warp" => &[FLASH],
        "risk_dodge" => &[AFTERIMAGE, S(CLOTH)],
        "focus_syphon" => &[BOLT_SPARK],
        "lock" => &[S(TICK)],
        "max_locks" => &[S(READY)],
        "bolt_ring" => &[MUZZLE],
        "bolt_launch" => &[S(WISDOM_SHOT)],
        "bolt_hit" => &[BOLT_SPARK],
        "syphon_gain" => &[S(ORB)],
        "wall_step" => &[S(STEP)],
        "dash_trail" => &[AFTERIMAGE],
        "pole_turn" => &[S(WHOOSH_SPIN)],
        "grand_offer" => &[S(READY)],
        _ => &[],
    }
}

/// Ticks between a running wall run's steps, and between a dash's
/// afterimages (Superslide, the airstep).
pub const STEP_EVERY: u32 = 12;
pub const TRAIL_EVERY: u32 = 20;
/// Ticks between two bolt sparks on one Sora.
pub const SPARK_EVERY: u32 = 4;

/// The cue a lasting move repeats this tick, if any: a step on the wall,
/// a fresh afterimage along a Superslide or an airstep, a whoosh per turn
/// round a pole. Once a period, so nothing stacks.
pub fn periodic(mode: &Mode) -> Option<&'static str> {
    match *mode {
        Mode::WallRun { tick, .. } if tick > 0 && tick.is_multiple_of(STEP_EVERY) => {
            Some("wall_step")
        }
        Mode::Superslide { tick, .. } | Mode::Airstep { tick, .. }
            if tick > 0 && tick.is_multiple_of(TRAIL_EVERY) =>
        {
            Some("dash_trail")
        }
        Mode::Pole { tick, .. }
            if tick > t::POLE_ATTACH && (tick - t::POLE_ATTACH).is_multiple_of(t::POLE_TURN) =>
        {
            Some("pole_turn")
        }
        _ => None,
    }
}
