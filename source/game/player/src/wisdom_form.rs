//! Wisdom Form's shots, translated from the form's own BDX program
//! (`wisdomform`, P_EX100_MAGF.mdlx 'wisd', trigger 16): frame trigger 1 of
//! the Shoot motions (FUN_14040dd70 case 1) sends event 0x10 with the
//! effect number; the program fires one homing bullet of ATKP 859.

use crate::Vec3;

/// ATKP 859 (Magic stat): sub 0 (Power 20), 3 (25) and 5 (100).
pub const ATKP: u16 = 859;
/// attack_set_radius(atk, 50.0, 0.0).
pub const RADIUS: f32 = 50.;
/// Speed 20, +2 a tick up to 80.
pub const START_SPEED: f32 = 20.;
pub const ACCELERATION: f32 = 2.;
pub const MAX_SPEED: f32 = 80.;
/// vector_homing's rate (rad a tick) and the distance inside which the
/// bullet stops steering.
pub const HOMING_RATE: f32 = 0.031416;
pub const HOMING_STOP: f32 = 100.;
/// The bullet flies 24 ticks, then its effect fades over 8 (the hit
/// volume lives until the effect is gone).
pub const LIFE: f32 = 24.;
pub const FADE: f32 = 8.;

/// TR16's sub-table for effect `n`: 40 is the finisher's last shot (5),
/// 43, 27, 32 and 35 the volleys' last (3), the rest of 25..=43 sub 0.
pub fn shot_sub(n: i16) -> Option<u8> {
    match n {
        40 => Some(5),
        43 | 27 | 32 | 35 => Some(3),
        25..=43 => Some(0),
        _ => None,
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Bullet {
    pub key: u64,
    pub sub: u8,
    pub pos: Vec3,
    /// Unit direction (scene space).
    pub dir: Vec3,
    pub speed: f32,
    homing: bool,
    life: f32,
    fade: f32,
    previous: Option<Vec3>,
}

fn len(v: Vec3) -> f32 {
    (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt()
}

fn unit(v: Vec3) -> Vec3 {
    let l = len(v);
    if l > 1e-6 {
        [v[0] / l, v[1] / l, v[2] / l]
    } else {
        v
    }
}

/// vector_homing: turn `dir` toward `to` by at most `rate` radians.
fn home(dir: Vec3, to: Vec3, rate: f32) -> Vec3 {
    let t = unit(to);
    let cos = (dir[0] * t[0] + dir[1] * t[1] + dir[2] * t[2]).clamp(-1., 1.);
    let angle = cos.acos();
    if angle <= rate || angle < 1e-6 {
        return t;
    }
    // Slerp by rate / angle.
    let s = angle.sin();
    let a = ((angle - rate).sin()) / s;
    let b = rate.sin() / s;
    unit([
        dir[0] * a + t[0] * b,
        dir[1] * a + t[1] * b,
        dir[2] * a + t[2] * b,
    ])
}

impl Bullet {
    /// spawn_bullet: at the muzzle, along the facing.
    pub fn new(key: u64, sub: u8, pos: Vec3, dir: Vec3) -> Self {
        Bullet {
            key,
            sub,
            pos,
            dir: unit(dir),
            speed: START_SPEED,
            homing: true,
            life: LIFE,
            fade: FADE,
            previous: None,
        }
    }

    /// One tick of the bullet thread. Returns false once the effect is
    /// gone (or the attack hit something: the caller drops it).
    pub fn step(&mut self, target: Option<Vec3>) -> bool {
        if self.life <= 0. {
            self.fade -= 1.;
            if self.fade <= 0. {
                return false;
            }
        }
        if self.homing
            && let Some(t) = target
        {
            let v = [t[0] - self.pos[0], t[1] - self.pos[1], t[2] - self.pos[2]];
            if len(v) < HOMING_STOP {
                self.homing = false;
            } else {
                self.dir = home(self.dir, v, HOMING_RATE);
            }
        }
        self.previous = Some(self.pos);
        for k in 0..3 {
            self.pos[k] += self.dir[k] * self.speed;
        }
        self.speed = (self.speed + ACCELERATION).min(MAX_SPEED);
        self.life -= 1.;
        true
    }

    pub fn previous(&self) -> Option<Vec3> {
        self.previous
    }

    /// [KH3 ruleset] A shotlock bolt rides a bullet (its effect and its
    /// hit volume) while the duel's KH3 layer steers it: the bullet lives
    /// as long as the bolt does.
    pub fn keep_alive(&mut self) {
        self.life = LIFE;
        self.fade = FADE;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn subs_follow_the_effect_numbers() {
        assert_eq!(shot_sub(25), Some(0));
        assert_eq!(shot_sub(27), Some(3));
        assert_eq!(shot_sub(40), Some(5));
        assert_eq!(shot_sub(44), None);
    }

    #[test]
    fn bullets_speed_up_home_and_expire() {
        let mut b = Bullet::new(1, 0, [0.; 3], [0., 0., 1.]);
        assert!(b.step(Some([1000., 0., 0.])));
        assert_eq!(b.speed, 22.);
        assert!(b.dir[0] > 0. && (b.dir[0] - HOMING_RATE.sin()).abs() < 1e-4);
        let mut ticks = 1;
        while b.step(None) {
            ticks += 1;
        }
        assert_eq!(ticks, 24 + 7);
        assert_eq!(b.speed, MAX_SPEED);
    }
}
