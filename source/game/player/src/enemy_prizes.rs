//! Enemy prizes and EXP, translated from the PC executable: the drop
//! routine FUN_1403dc170 (przt counts scaled by Jackpot, munny turned into
//! drive by Drive Converter), the pickup rule FUN_1403dc500, the effect
//! table at 0x1405c7420 and the kill EXP of FUN_140410af0.

use crate::Vec3;
use crate::actor_physics;

/// Prize kinds, in przt count order.
pub mod kind {
    pub const SMALL_HP: u8 = 0;
    pub const BIG_HP: u8 = 1;
    pub const BIG_MUNNY: u8 = 2;
    pub const MEDIUM_MUNNY: u8 = 3;
    pub const SMALL_MUNNY: u8 = 4;
    pub const SMALL_MP: u8 = 5;
    pub const BIG_MP: u8 = 6;
    pub const SMALL_DRIVE: u8 = 7;
    pub const BIG_DRIVE: u8 = 8;
}

/// What a prize does when collected (table 0x1405c7420 bytes +4..+7).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Effect {
    pub hp: i32,
    pub mp: i32,
    pub drive: i32,
    pub munny: i32,
}

pub fn effect(kind: u8) -> Effect {
    let e = |hp, mp, drive, munny| Effect {
        hp,
        mp,
        drive,
        munny,
    };
    match kind {
        kind::SMALL_HP => e(1, 0, 0, 0),
        kind::BIG_HP => e(5, 0, 0, 0),
        kind::BIG_MUNNY => e(0, 0, 0, 10),
        kind::MEDIUM_MUNNY => e(0, 0, 0, 5),
        kind::SMALL_MUNNY => e(0, 0, 0, 1),
        kind::SMALL_MP => e(0, 1, 0, 0),
        kind::BIG_MP => e(0, 3, 0, 0),
        kind::SMALL_DRIVE => e(0, 0, 1, 0),
        kind::BIG_DRIVE => e(0, 0, 3, 0),
        _ => Effect::default(),
    }
}

/// How many ticks the heal effect (system element 6) lasts after the prize
/// is collected (table 0x1405c7420, float at +0): none for munny.
pub fn heal_ticks(kind: u8) -> f32 {
    match kind {
        kind::SMALL_HP | kind::SMALL_MP | kind::SMALL_DRIVE => 30.,
        kind::BIG_HP | kind::BIG_MP | kind::BIG_DRIVE => 60.,
        _ => 0.,
    }
}

/// The pickup centre's height above the prize's origin (descriptor +4).
pub fn centre_height(kind: u8) -> f32 {
    const H: [f32; 9] = [12.6, 27., 38.4, 25.6, 12.8, 16., 28.8, 14.4, 27.2];
    H.get(kind as usize).copied().unwrap_or(0.)
}

/// The prize descriptor's sounds (table 0x1405c7390, bytes +8 and +0xc):
/// se000 ids played on a ground bounce and on pickup.
pub fn sounds(kind: u8) -> (u32, u32) {
    match kind {
        kind::SMALL_HP | kind::BIG_HP => (12, 16),
        kind::BIG_MUNNY | kind::MEDIUM_MUNNY | kind::SMALL_MUNNY => (11, 15),
        kind::SMALL_MP | kind::BIG_MP => (13, 17),
        _ => (30, 31),
    }
}

/// Munny turns into drive with Drive Converter: big and medium into big
/// drive, small into small drive (table 0x1405c7978).
fn converted(kind: u8) -> u8 {
    match kind {
        kind::BIG_MUNNY | kind::MEDIUM_MUNNY => kind::BIG_DRIVE,
        _ => kind::SMALL_DRIVE,
    }
}

/// FUN_1403ecef0: ceil(count x Jackpot rate).
fn scaled(count: u8, jackpot: f32) -> i32 {
    (count as f32 * jackpot).ceil() as i32
}

/// The prizes one przt record drops: (kind, count) pairs. `jackpot` is
/// 1.0 + the party's Jackpot terms; `converter` the product of the Drive
/// Converter factors (1.0 without it).
pub fn drops(counts: &[u8; 9], jackpot: f32, converter: f32) -> Vec<(u8, i32)> {
    let mut out = Vec::new();
    for (k, &c) in counts.iter().enumerate() {
        let k = k as u8;
        let n = scaled(c, jackpot);
        if (kind::BIG_MUNNY..=kind::SMALL_MUNNY).contains(&k) {
            let keep = (n as f32 * converter) as i32;
            if n - keep > 0 {
                out.push((converted(k), n - keep));
            }
            if keep > 0 {
                out.push((k, keep));
            }
        } else if n > 0 {
            out.push((k, n));
        }
    }
    out
}

/// [I] The drawn velocities (+-20 / 20..40) are scaled by 0.1 per tick,
/// like the knock-slide's strength; the prize physics FUN_1404132d0 was not
/// traced.
const VELOCITY_SCALE: f32 = 0.1;

/// A prize in the world: it falls and bounces, then waits to be collected.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Orb {
    pub kind: u8,
    pub position: Vec3,
    pub velocity: Vec3,
    pub landed: bool,
    /// Ground bounces so far (FUN_1404132d0 +0x40): the first two play the
    /// descriptor's bounce sound.
    pub bounces: u8,
    /// Ticks since the prize appeared (its sprite's flip-book step).
    pub age: u32,
    /// The starting point of a looping flip-book, from the spawn draws.
    pub seed: f32,
}

impl Orb {
    /// FUN_1403dc3d0: the initial velocity from three draws in [0, 1):
    /// x = r3 x 40 - 20, up 20..40 (the exe's Y-down -40..-20), z = r1 x 40 - 20.
    pub fn spawn(kind: u8, at: Vec3, r: [f32; 3]) -> Orb {
        Orb {
            kind,
            position: at,
            velocity: [
                (r[2] * 40. - 20.) * VELOCITY_SCALE,
                (40. - r[1] * 20.) * VELOCITY_SCALE,
                (r[0] * 40. - 20.) * VELOCITY_SCALE,
            ],
            landed: false,
            bounces: 0,
            age: 0,
            seed: r[1],
        }
    }

    /// One tick of the fall and bounce [I: the exe's bounce constants were
    /// not traced; gravity is the actors' 0.408 and a bounce keeps half the
    /// speed]. `floor` is the ground height below the orb.
    pub fn tick(&mut self, floor: f32) -> Option<u32> {
        self.age = self.age.saturating_add(1);
        if self.landed {
            return None;
        }
        let mut sound = None;
        self.velocity[1] -= actor_physics::G;
        for i in 0..3 {
            self.position[i] += self.velocity[i];
        }
        if self.position[1] <= floor {
            self.position[1] = floor;
            if self.velocity[1].abs() < 2. {
                self.velocity = [0.; 3];
                self.landed = true;
            } else {
                self.velocity[1] = -self.velocity[1] * 0.5;
                self.velocity[0] *= 0.5;
                self.velocity[2] *= 0.5;
                // FUN_1404132d0: each bounce while the counter is below 2
                // requests the descriptor's sound at the prize.
                if self.bounces < 2 {
                    sound = Some(sounds(self.kind).0);
                }
                self.bounces = self.bounces.saturating_add(1);
            }
        }
        sound
    }

    /// FUN_1403dc500: within 120 (+ Draw) horizontally and 30 vertically
    /// of the prize's centre.
    pub fn collected_by(&self, body: Vec3, draw: f32) -> bool {
        if !self.landed {
            return false;
        }
        let centre = [
            self.position[0],
            self.position[1] + centre_height(self.kind),
            self.position[2],
        ];
        let d = [
            centre[0] - body[0],
            centre[1] - body[1],
            centre[2] - body[2],
        ];
        d[1].abs() <= 30. && d[0].hypot(d[2]) <= 120. + draw
    }
}

/// The treasure box an item drops as (table 0x1405c96c0, indexed by the
/// item record's byte +0x16): the object id of the box and its scale.
/// Objects 0x3b8..0x3bb are the yellow, red, blue and green boxes.
pub fn box_style(index: u8) -> Option<(u32, f32)> {
    const TABLE: [(u32, f32); 12] = [
        (0x3b9, 1.6),
        (0x3b9, 2.3),
        (0x3b9, 3.2),
        (0x3bb, 1.6),
        (0x3bb, 2.3),
        (0x3bb, 3.2),
        (0x3ba, 1.6),
        (0x3ba, 2.3),
        (0x3ba, 3.2),
        (0x3b8, 1.6),
        (0x3b8, 2.3),
        (0x3b8, 3.2),
    ];
    TABLE.get(index as usize).copied()
}

/// YS::ACTION_PRIZEBOX's tick FUN_1403f8500: gravity, its cap and the drag.
const BOX_GRAVITY: f32 = 1.5;
const BOX_FALL_MAX: f32 = 25.;
const BOX_DRAG: f32 = 0.97;
/// FUN_1403f88c0: a landing this fast bounces back at this rate; the first two
/// bounces sound (FUN_1401de150(0xe, ..)).
const BOX_BOUNCE_SPEED: f32 = 2.5;
const BOX_BOUNCE: f32 = 0.8;
pub const BOX_BOUNCE_SOUND: u32 = 14;
/// FUN_1403f8a10: the flight's start (FUN_1401de150(0x12, ..)).
pub const BOX_FLY_SOUND: u32 = 18;
/// FUN_1403f8590: the height the flight arcs by, and the spin at its end.
const BOX_ARC: f32 = 50.;
const BOX_SPIN: f32 = std::f32::consts::FRAC_PI_6;

/// An item dropped by a kill (FUN_1403f8170): a treasure box (YS::PRIZEBOX)
/// that falls and bounces (state 0xc), then flies to whoever picks it up
/// (state 0xd), which added the item when it took off (FUN_1403f8770).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ItemDrop {
    /// Unique within a room, for the presenter.
    pub id: u32,
    pub item: u16,
    /// The box's object id and the scale shown (`box_style`).
    pub object: u32,
    pub scale: f32,
    /// +0xbb4: the base scale.
    pub base_scale: f32,
    pub position: Vec3,
    /// +0x20, with +y up here (the game's is down).
    pub velocity: Vec3,
    /// A random heading (FUN_1403f8170: a draw of 2π − π).
    pub yaw: f32,
    /// +0x120 bit 1 clear: it has started to fall (FUN_1403f8500).
    pub collectable: bool,
    /// +0xbd0 bit 0: at rest.
    pub rested: bool,
    /// +0xbd4.
    pub bounces: u8,
    /// State 0xd.
    pub flight: Option<BoxFlight>,
}

/// The flight (FUN_1403f8770's fields): +0xbc0, +0xbb8 and +0xbbc.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct BoxFlight {
    pub offset: Vec3,
    pub left: f32,
    pub total: f32,
}

impl ItemDrop {
    /// `r` is the four draws in [0, 1) of FUN_1403f8170 and FUN_1403f8950: the
    /// heading, then x, z and y of the start velocity (±5 across, 10..15 up).
    pub fn spawn(id: u32, item: u16, style: (u32, f32), at: Vec3, r: [f32; 4]) -> ItemDrop {
        ItemDrop {
            id,
            item,
            object: style.0,
            scale: style.1,
            base_scale: style.1,
            position: at,
            velocity: [r[1] * 10. - 5., 15. - r[3] * 5., r[2] * 10. - 5.],
            yaw: r[0] * std::f32::consts::TAU - std::f32::consts::PI,
            collectable: false,
            rested: false,
            bounces: 0,
            flight: None,
        }
    }

    /// One tick of the fall to `floor` (FUN_1403f8500, FUN_1403f88c0); the
    /// sound of a bounce.
    pub fn tick(&mut self, floor: f32) -> Option<u32> {
        if self.rested || self.flight.is_some() {
            return None;
        }
        if self.velocity[1] < 0. {
            self.collectable = true;
        }
        self.velocity[1] = (self.velocity[1] - BOX_GRAVITY).max(-BOX_FALL_MAX);
        self.velocity[0] *= BOX_DRAG;
        self.velocity[2] *= BOX_DRAG;
        for (p, v) in self.position.iter_mut().zip(self.velocity) {
            *p += v;
        }
        if self.position[1] > floor {
            return None;
        }
        self.position[1] = floor;
        if -self.velocity[1] > BOX_BOUNCE_SPEED {
            self.velocity[1] *= -BOX_BOUNCE;
            let sound = (self.bounces < 2).then_some(BOX_BOUNCE_SOUND);
            self.bounces = self.bounces.saturating_add(1);
            sound
        } else {
            self.rested = true;
            self.velocity = [0.; 3];
            None
        }
    }

    /// FUN_1403f7df0 → FUN_1403dc500 with its defaults: within 120
    /// horizontally and 30 vertically of the box's position.
    pub fn collected_by(&self, body: Vec3) -> bool {
        let d = [
            self.position[0] - body[0],
            self.position[1] - body[1],
            self.position[2] - body[2],
        ];
        self.collectable && self.flight.is_none() && d[1].abs() <= 30. && d[0].hypot(d[2]) <= 120.
    }

    /// FUN_1403f8770's flight: toward `anchor`, |offset| × 0.05 ticks within
    /// 30..60.
    pub fn take_off(&mut self, anchor: Vec3) {
        let offset = [
            self.position[0] - anchor[0],
            self.position[1] - anchor[1],
            self.position[2] - anchor[2],
        ];
        let length = (offset[0] * offset[0] + offset[1] * offset[1] + offset[2] * offset[2]).sqrt();
        let total = (length * 0.05).clamp(30., 60.);
        self.velocity = [0.; 3];
        self.flight = Some(BoxFlight {
            offset,
            left: total,
            total,
        });
    }

    /// FUN_1403f8590, one tick toward `anchor` (read live); false when the
    /// flight is over and the box goes.
    pub fn fly(&mut self, anchor: Vec3) -> bool {
        let Some(f) = self.flight.as_mut() else {
            return true;
        };
        f.left -= 1.;
        if f.left <= 0. {
            return false;
        }
        let eased = ((1. - f.left / f.total) * std::f32::consts::FRAC_PI_2).sin();
        let t = 1. - eased;
        self.position = [
            anchor[0] + f.offset[0] * t,
            anchor[1] + f.offset[1] * t + (t * std::f32::consts::PI).sin() * BOX_ARC,
            anchor[2] + f.offset[2] * t,
        ];
        self.yaw += (1. - t) * BOX_SPIN;
        if t < 0.1 {
            self.scale = t * 10. * self.base_scale;
        }
        true
    }
}

/// FUN_140410af0: EXP of a kill: (enemy EXP x the level row's EXP + 99) /
/// 100, three quarters on Critical (at least 1), times Experience Boost
/// copies at or below half HP.
pub fn kill_exp(enemy_exp: u16, level_exp: u16, critical: bool, boost: Option<u32>) -> u32 {
    let mut exp = (enemy_exp as u32 * level_exp as u32).div_ceil(100);
    if exp > 0 {
        if critical {
            exp = (exp * 75 / 100).max(1);
        }
        if let Some(b) = boost {
            exp *= b;
        }
    }
    exp
}

/// FUN_1403ecf30's loop for one character: the level reached by `exp`
/// total, given the EXP needed to leave each level (None past the table).
pub fn level_for(exp: u32, mut level: u8, needed: impl Fn(u8) -> Option<u32>) -> u8 {
    while level < 99 {
        match needed(level) {
            Some(n) if exp >= n => level += 1,
            _ => break,
        }
    }
    level
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_shadow_drops_one_hp_orb_and_one_munny() {
        let d = drops(&[1, 0, 0, 0, 1, 0, 0, 0, 0], 1., 1.);
        assert_eq!(d, vec![(kind::SMALL_HP, 1), (kind::SMALL_MUNNY, 1)]);
        // Jackpot rounds counts up; Drive Converter moves munny to drive.
        let j = drops(&[1, 0, 0, 0, 1, 0, 0, 0, 0], 1.5, 0.5);
        assert_eq!(
            j,
            vec![
                (kind::SMALL_HP, 2),
                (kind::SMALL_DRIVE, 1),
                (kind::SMALL_MUNNY, 1)
            ]
        );
    }

    #[test]
    fn kill_exp_follows_the_row_difficulty_and_boost() {
        // The Shadow: 15 x 4 -> 1; 15 x 16 -> 3.
        assert_eq!(kill_exp(15, 4, false, None), 1);
        assert_eq!(kill_exp(15, 16, false, None), 3);
        assert_eq!(kill_exp(15, 16, true, None), 2);
        assert_eq!(kill_exp(15, 4, true, None), 1);
        assert_eq!(kill_exp(15, 16, false, Some(2)), 6);
        assert_eq!(kill_exp(0, 16, false, Some(2)), 0);
    }

    #[test]
    fn levels_rise_while_exp_passes_the_thresholds() {
        let need = |l: u8| [40, 100, 184].get(l as usize - 1).copied();
        assert_eq!(level_for(39, 1, need), 1);
        assert_eq!(level_for(40, 1, need), 2);
        assert_eq!(level_for(190, 1, need), 4);
    }

    #[test]
    fn item_boxes_follow_the_style_table_and_are_picked_up_in_range() {
        assert_eq!(box_style(0), Some((0x3b9, 1.6)));
        assert_eq!(box_style(11), Some((0x3b8, 3.2)));
        assert_eq!(box_style(12), None);
        let mut d = ItemDrop::spawn(1, 280, (0x3b9, 1.6), [0., 50., 0.], [0.5; 4]);
        // Thrown up: not collectable until it falls.
        assert_eq!(d.velocity, [0., 12.5, 0.]);
        assert!(!d.collected_by([0., 50., 0.]));
        let mut sounds = 0;
        for _ in 0..200 {
            sounds += d.tick(0.).map_or(0, |_| 1);
        }
        assert_eq!(sounds, 2, "the first two bounces sound");
        assert!(d.rested && d.position[1] == 0. && d.bounces > 2);
        assert!(d.collected_by([100., 0., 0.]));
        assert!(!d.collected_by([130., 0., 0.]));
    }

    #[test]
    fn a_taken_box_arcs_to_its_target_and_shrinks_at_the_end() {
        let mut d = ItemDrop::spawn(1, 280, (0x3b9, 2.), [0., 0., 100.], [0.5; 4]);
        d.take_off([0., 0., 0.]);
        let f = d.flight.unwrap();
        // |v| × 0.05 is under the 30-tick floor.
        assert_eq!((f.left, f.total), (30., 30.));
        assert!(!d.collected_by([0., 0., 100.]));
        let mut high = 0f32;
        let mut ticks = 0;
        while d.fly([0., 0., 0.]) {
            high = high.max(d.position[1]);
            ticks += 1;
        }
        assert_eq!(ticks, 29);
        assert!(high > 45. && high <= 50.);
        assert!(d.position[2].abs() < 1. && d.scale < 0.2);
    }

    #[test]
    fn only_hp_mp_and_drive_prizes_start_the_heal_effect() {
        assert_eq!(heal_ticks(kind::SMALL_HP), 30.);
        assert_eq!(heal_ticks(kind::BIG_DRIVE), 60.);
        assert_eq!(heal_ticks(kind::BIG_MUNNY), 0.);
    }

    #[test]
    fn an_orb_falls_bounces_rests_and_is_collected_in_range() {
        let mut o = Orb::spawn(kind::SMALL_HP, [0., 10., 0.], [0.5, 0.5, 0.5]);
        for _ in 0..400 {
            o.tick(0.);
        }
        assert!(o.landed);
        assert_eq!(o.position[1], 0.);
        assert!(o.collected_by([o.position[0] + 100., 0., o.position[2]], 0.));
        assert!(!o.collected_by([o.position[0] + 130., 0., o.position[2]], 0.));
        assert!(o.collected_by([o.position[0] + 200., 0., o.position[2]], 125.));
    }
}
