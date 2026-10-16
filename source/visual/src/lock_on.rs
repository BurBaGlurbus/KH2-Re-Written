//! The lock-on reticle, translated from the executable's dk::LockOn object
//! (vtable 0x1405b24f8, a dk::Sprite3D, priority 4000, pool of 34 created by
//! FUN_14017c1e0): creation FUN_14017c570 / FUN_14017bc60, per-frame
//! position FUN_14017c520 and projection FUN_14016b350 / FUN_140166a20, scale
//! FUN_14017c630, and the lock manager's choice of group in FUN_1403dd770.
//!
//! The art is the `fiel` sequence's groups 1, 5, 23 and 27 (additive rings and
//! four corner marks). Auto target: group 5 for the primary HP pool, 1 for
//! others. Manual lock: 27 (blue) for the primary pool, 23 (red) for others.
use crate::gauge::Draw;
use data::sprite_sequence::{Modifiers, Player, Sequence};

/// Groups by (manual, primary pool): FUN_1403dd770 / FUN_1403dd650.
pub fn group_for(manual: bool, primary_pool: bool) -> usize {
    match (manual, primary_pool) {
        (false, false) => 1,
        (false, true) => 5,
        (true, false) => 0x17,
        (true, true) => 0x1b,
    }
}

/// Scale applied by FUN_14017c630: the projected depth times 1e-5 plus 0.4
/// (0x1405b2530, 0x1405b1d8c), kept within 0.5..=1.
fn scale_for(depth: i32) -> f32 {
    let scale = depth as f32 * 1.0e-5 + 0.4;
    let upper = if scale <= 1. { scale } else { 1. };
    if 0.5 <= upper { upper } else { 0.5 }
}

pub struct LockOn {
    group: usize,
    player: Player,
    /// Projected position truncated to integers, as the object stores it
    /// (+0x1c0, +0x1c4, +0x208).
    screen: [i32; 3],
    scale: f32,
}

impl LockOn {
    pub fn new(seq: &Sequence, group: usize) -> Self {
        Self {
            group,
            player: Player::new(seq, group),
            screen: [0, 0, 0],
            scale: 1.,
        }
    }

    pub fn group(&self) -> usize {
        self.group
    }

    /// FUN_14016b350: the target's projected screen position (layout units and
    /// depth) is truncated, the depth sets the scale, and the group plays one
    /// frame.
    pub fn update(&mut self, seq: &Sequence, screen: [f32; 3]) {
        self.screen = screen.map(|v| v as i32);
        self.scale = scale_for(self.screen[2]);
        self.player.update(seq, 1.);
    }

    /// Drawn only while the depth is within the projection's near and far
    /// bounds (FUN_14016abc0). The X origin is zero: the projection already
    /// is in layout units.
    pub fn draw(&self, seq: &Sequence, sheet: usize) -> Option<Draw> {
        let [x, y, depth] = self.screen;
        if !(1..=4_000_000).contains(&depth) {
            return None;
        }
        let modifiers = Modifiers {
            scale: self.scale,
            ..Modifiers::default()
        };
        Some(Draw::Quads {
            quads: seq.quads(&self.player, [x as f32, y as f32], &modifiers),
            z: 0,
            sheet,
        })
    }
}

/// The lock manager's reticle: created when a target is acquired, replaced
/// when the target or the lock state changes (the animation restarts), and
/// destroyed with the target.
#[derive(Default)]
pub struct Reticle {
    shown: Option<(u64, LockOn)>,
}

/// What the lock manager is holding this frame.
pub struct Held {
    /// Identity of the held target and shape; a change replaces the reticle.
    pub key: u64,
    pub manual: bool,
    pub primary_pool: bool,
}

impl Reticle {
    /// One frame: `screen` is the target shape centre projected through the
    /// camera. Returns the reticle's drawable when it exists.
    pub fn step(
        &mut self,
        seq: &Sequence,
        held: Option<(Held, [f32; 3])>,
        sheet: usize,
    ) -> Option<Draw> {
        let Some((held, screen)) = held else {
            self.shown = None;
            return None;
        };
        let group = group_for(held.manual, held.primary_pool);
        let key = held.key << 8 | group as u64;
        if self.shown.as_ref().is_none_or(|(shown, _)| *shown != key) {
            self.shown = Some((key, LockOn::new(seq, group)));
        }
        let (_, lock) = self.shown.as_mut()?;
        lock.update(seq, screen);
        lock.draw(seq, sheet)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use data::sprite_sequence::{Animation, AnimationGroup, Sprite, SpritePart};

    fn synthetic() -> Sequence {
        let animation = Animation {
            flags: data::sprite_sequence::IS_ACTIVE | data::sprite_sequence::LINEAR,
            sprite_group: 0,
            frames: [0, 69],
            translate: [0; 4],
            pivot: [0; 4],
            rotation: [0.; 6],
            scale: [1.; 2],
            scale_x: [1.; 2],
            scale_y: [1.; 2],
            curve: [0.; 4],
            bounce: [0.; 4],
            bounce_count: [0; 2],
            blend: 1,
            color: [0x8080_8080; 2],
        };
        let group = AnimationGroup {
            animations: 0..1,
            play_count: 0,
            loop_frames: [12, 0],
            light_position_x: 0,
            text_position_y: 0,
            text_scale: 0,
            ui_padding: 0,
            text_position_x: 0,
        };
        Sequence {
            sprites: vec![Sprite {
                uv: [0, 0, 10, 10],
                scroll: [0.; 2],
                colors: [0x8080_8080; 4],
            }],
            parts: vec![SpritePart {
                rect: [-14, -14, 14, 14],
                sprite: 0,
            }],
            sprite_groups: std::iter::once(0..1).collect(),
            animations: vec![animation],
            groups: vec![group; 40],
        }
    }

    #[test]
    fn groups_follow_the_lock_state_and_hp_pool() {
        assert_eq!(group_for(false, false), 1);
        assert_eq!(group_for(false, true), 5);
        assert_eq!(group_for(true, false), 23);
        assert_eq!(group_for(true, true), 27);
    }

    #[test]
    fn scale_grows_with_depth_between_one_half_and_one() {
        assert_eq!(scale_for(500), 0.5);
        assert_eq!(scale_for(10_000), 0.5);
        assert!((scale_for(40_000) - 0.8).abs() < 1e-6);
        assert_eq!(scale_for(1_000_000), 1.);
    }

    #[test]
    fn position_truncates_toward_zero_and_distant_points_are_not_drawn() {
        let seq = synthetic();
        let mut lock = LockOn::new(&seq, 27);
        lock.update(&seq, [300.9, 120.9, 800.9]);
        assert_eq!(lock.screen, [300, 120, 800]);
        let Some(Draw::Quads { quads, .. }) = lock.draw(&seq, 2) else {
            panic!("reticle quads")
        };
        // 28 wide at half scale, centred on the truncated position.
        assert!((quads[0].corners[0][0] - (300. - 7.)).abs() < 1e-3);
        lock.update(&seq, [0., 0., 0.5]);
        assert!(lock.draw(&seq, 2).is_none());
        lock.update(&seq, [0., 0., 5_000_000.]);
        assert!(lock.draw(&seq, 2).is_none());
    }

    #[test]
    fn the_reticle_restarts_on_a_new_target_and_vanishes_without_one() {
        let seq = synthetic();
        let mut reticle = Reticle::default();
        let held = |key| {
            Some((
                Held {
                    key,
                    manual: true,
                    primary_pool: true,
                },
                [256., 208., 600.],
            ))
        };
        assert!(reticle.step(&seq, held(1), 3).is_some());
        for _ in 0..30 {
            reticle.step(&seq, held(1), 3);
        }
        let time = reticle.shown.as_ref().unwrap().1.player.time;
        assert!(time > 12.);
        reticle.step(&seq, held(2), 3);
        assert!(reticle.shown.as_ref().unwrap().1.player.time < 2.);
        assert!(reticle.step(&seq, None, 3).is_none());
        assert!(reticle.shown.is_none());
    }
}
