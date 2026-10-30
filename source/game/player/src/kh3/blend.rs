//! Motion blends in the KH3 ruleset: how long a new motion crossfades from
//! the pose it replaces, in the room's blend clock (ticks; the clock starts
//! at 3 and a duration of 2 or less is no blend, as KH2's own, see the
//! room's `present`).
//!
//! KH2 starts jumps, landings, attacks, reactions and the growth motions
//! with no blend (`crate::blend_for`), which suits KH2's own moves but made
//! KH3's dodges, slides, flow states and get-ups snap from one pose to the
//! next. KH3 crossfades every state change; its blend times live in its
//! animation assets (UE4 montages and the state machine), not in the
//! executable, so these are [duel choice] values on KH2's clock: short for
//! hit reactions and swings, longer for the flow states.

use super::Mode;
use crate::{Action, growth};

/// A hit reaction (a flinch or a knock): fast, so a hit still reads.
pub const REACTION: f32 = 5.;
/// Swings: a chain's next swing, a flow attack, the counter, the launch.
pub const SWING: f32 = 6.;
/// Jumps, Doubleflight and landings.
pub const JUMP: f32 = 6.;
/// Dodge Roll, Air Slide, the airstep and its hop, the kicks.
pub const DODGE: f32 = 8.;
/// Getting up, Aerial Recovery, the volley, Focus Syphon.
pub const RECOVER: f32 = 10.;
/// Wall run, pole, Superslide and its end.
pub const FLOW: f32 = 10.;
/// Locomotion and the fall (KH2's own, `actor_physics::MOTION_BLEND`).
pub const MOVE: f32 = crate::actor_physics::MOTION_BLEND;

/// The blend for `to` starting in the layer's `mode` (the mode it is shown
/// for), from the action `from` that was shown.
pub fn ticks(from: Action, to: Action, mode: &Mode) -> f32 {
    // Out of a knock-down or a flinch into anything: the recovery blend.
    let from_hurt = matches!(from, Action::Hurt(_));
    let base = match mode {
        Mode::Hurt { .. } => REACTION,
        Mode::Down { .. } | Mode::Recover { .. } | Mode::Volley { .. } | Mode::Syphon { .. } => {
            RECOVER
        }
        Mode::Roll { .. }
        | Mode::Slide { .. }
        | Mode::Airstep { .. }
        | Mode::HopBack { .. }
        | Mode::WallKick { .. }
        | Mode::Risk { .. } => DODGE,
        Mode::Glide { .. } => growth::GLIDE_BLEND,
        Mode::WallRun { .. }
        | Mode::Pole { .. }
        | Mode::Superslide { .. }
        | Mode::SlideEnd { .. } => FLOW,
        Mode::Flow { .. } | Mode::Launch { .. } | Mode::Chase { .. } | Mode::Attack | Mode::Cast => SWING,
        Mode::Takeoff { .. } | Mode::Land { .. } => JUMP,
        Mode::Air { .. } => match to {
            Action::Jump | Action::HighJump | Action::Growth(_) => JUMP,
            _ => MOVE,
        },
        Mode::Ground | Mode::Kh2 => match to {
            Action::Idle | Action::Walk | Action::Run | Action::Fall => MOVE,
            Action::Land | Action::Jump | Action::HighJump => JUMP,
            Action::Hurt(_) => REACTION,
            _ => SWING,
        },
    };
    if from_hurt && !matches!(mode, Mode::Hurt { .. }) {
        base.max(RECOVER)
    } else {
        base
    }
}

/// The least blend of a KH2 controller motion in the KH3 ruleset (its
/// swings, casts, guards and reactions start with none in KH2).
pub fn kh2_floor(to: Action) -> f32 {
    match to {
        Action::Attack(_) | Action::AirAttack(_) => SWING,
        Action::Cast(_) | Action::AirCast(_) | Action::Item => SWING,
        Action::Guard | Action::GuardRecoil | Action::WallRecoil => DODGE,
        Action::Hurt(_) => REACTION,
        Action::Land | Action::Jump | Action::HighJump => JUMP,
        _ => 0.,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_kh3_state_blends_and_reactions_stay_quick() {
        let modes = [
            Mode::Roll { tick: 0 },
            Mode::Slide {
                tick: 0,
                dir: [0.; 2],
            },
            Mode::Glide { tick: 0 },
            Mode::Down {
                tick: 0,
                back: false,
            },
            Mode::Flow { tick: 0 },
            Mode::Land { tick: 0 },
        ];
        for m in modes {
            // More than 2 ticks: the room treats 2 or less as a cut.
            assert!(ticks(Action::Idle, Action::Growth(0), &m) > 2., "{m:?}");
        }
        let hurt = Mode::Hurt {
            knock: super::super::Knock::Small,
            tick: 0,
            back: false,
            air: false,
            high: false,
            apex: 0,
            fell: 0,
        };
        assert!(ticks(Action::Run, Action::Hurt(0), &hurt) < DODGE);
        // Getting up out of a reaction is never a cut.
        assert_eq!(ticks(Action::Hurt(3), Action::Idle, &Mode::Ground), RECOVER);
        assert!(kh2_floor(Action::Attack(1)) > 2.);
        assert_eq!(kh2_floor(Action::Idle), 0.);
    }
}
