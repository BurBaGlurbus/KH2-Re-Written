//! Sora's Drive gauge, translated from the PC executable: the integer
//! gauge add FUN_1403c0ad0, the payment FUN_1403c2780, the gain from hits
//! FUN_1403a8800, the central gain FUN_1403a9cf0, and in a Drive Form the
//! form time: the add FUN_1403c0b50, the per-frame drain in the player tick
//! FUN_1403a92c0 and the entry FUN_1403c17c0. One bar is 100 points; one
//! form bar is 600 time units.

/// SSTM Drive fields (`DAT_142ae5760`): DriveEnemy +0x60 (hits on ZAKO
/// ×0.75) and the MP-charge multiplier +0x78 (2.0).
pub const DRIVE_ENEMY: f32 = 0.75;
pub const CHARGE_MULTIPLIER: f32 = 2.0;
/// SSTM +0x64: a hit on a ZAKO drains form time ×0.2.
pub const FORM_ZAKO_DRAIN: f32 = 0.2;
/// SSTM +0x68: form (and summon) time per bar.
pub const FORM_BAR_TIME: f32 = 600.;
/// SSTM +0x6c: out of battle the form time drains at half rate.
pub const FIELD_DRAIN: f32 = 0.5;
/// SSTM +0x70: a drive gain refills form time ×12.
pub const FORM_GAIN: f32 = 12.;
/// SSTM +0x74: each DriveDrain point of a hit drains 15 form time.
pub const FORM_HIT_DRAIN: f32 = 15.;
/// SSTM +0x8c (i32 5): Antiform's bars are 5 − 2 + 3.
pub const ANTI_LEVEL: u8 = 5;
/// SSTM +0x94 AntiFormDamageRate (1.5): damage the lead player takes in
/// Antiform (FUN_1403c12f0).
pub const ANTI_DAMAGE_RATE: f32 = 1.5;

/// Gauge mode, stat+0x1af.
pub mod mode {
    pub const NONE: u8 = 0;
    pub const DRIVE: u8 = 1;
    pub const FORM: u8 = 2;
    pub const SUMMON: u8 = 3;
    pub const LOCKED: u8 = 4;
}

/// stat+0x1af..+0x1b8.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Gauge {
    /// stat+0x1af: [`mode`].
    pub mode: u8,
    /// Partial bar, 0..=100 (100 only when full at the maximum).
    pub partial: u8,
    /// Full bars.
    pub level: u8,
    /// Maximum bars.
    pub max: u8,
    /// stat+0x1b3: form (or summon) bars.
    pub bars: u8,
    /// stat+0x1b4 / +0x1b8: form time left and its maximum.
    pub time: f32,
    pub max_time: f32,
    /// The owner's form (obj+0xde0): Antiform (6) turns every refill into a
    /// drain, form 0xc keeps its time on drive gains.
    pub form: u8,
}

impl Gauge {
    pub fn new(max: u8) -> Self {
        Gauge {
            mode: mode::DRIVE,
            partial: 0,
            level: 0,
            max,
            bars: 0,
            time: 0.,
            max_time: 0.,
            form: 0,
        }
    }

    /// FUN_1403c0ad0: a negative add borrows at most one bar; a positive
    /// one carries into bars up to the maximum.
    pub fn add(&mut self, delta: i32) {
        let mut v = self.partial as i32 + delta;
        if delta < 0 {
            if v < 0 {
                if self.level != 0 {
                    self.level -= 1;
                    self.partial = (v + 100) as u8;
                } else {
                    self.partial = 0;
                }
                return;
            }
        } else if v > 99 {
            let mut lv = self.level;
            loop {
                if self.max <= lv {
                    self.level = lv;
                    self.partial = 100;
                    return;
                }
                lv += 1;
                self.level = lv;
                if self.max <= lv {
                    self.partial = 100;
                    return;
                }
                v -= 100;
                if v <= 99 {
                    break;
                }
            }
        }
        self.partial = v as u8;
    }

    /// FUN_1403c2780: pay a form's cost in bars; a full gauge also loses
    /// its partial bar, and paying more than held empties it.
    pub fn pay(&mut self, cost: u8) {
        if self.level == self.max || self.level < cost {
            self.partial = 0;
        }
        self.level = self.level.saturating_sub(cost);
    }

    /// FUN_1403a9cf0: in mode 1, while MP charges the gain is ×2 × Drive
    /// Boost and the result truncates to whole points (`area` is the event
    /// script's rate, 1.0 by default); in a form (or summon) it refills the
    /// time ×12.
    pub fn gain(&mut self, v: f32, mp_charging: bool, drive_boost: f32, area: f32) {
        match self.mode {
            mode::DRIVE => {
                let v = if mp_charging {
                    v * CHARGE_MULTIPLIER * drive_boost
                } else {
                    v
                };
                self.add((v * area) as i32);
            }
            mode::FORM if self.form != 0xc => {
                self.add_time(v * FORM_GAIN);
            }
            mode::SUMMON => {
                self.add_time(v * FORM_GAIN);
            }
            _ => {}
        }
    }

    /// FUN_1403a8800: a hit's DriveDrain on an enemy. In mode 1 (and in a
    /// summon with time left) it is a gain, ×0.75 on ZAKO; in a form it
    /// drains DriveDrain × 15 form time, ×0.2 on ZAKO.
    pub fn hit(&mut self, drain: u8, zako: bool, mp_charging: bool, drive_boost: f32) {
        if drain == 0 {
            return;
        }
        let mut v = drain as f32;
        match self.mode {
            mode::DRIVE => {}
            mode::FORM => {
                v = -(v * FORM_HIT_DRAIN);
                if zako {
                    v *= FORM_ZAKO_DRAIN;
                }
                self.add_time(v);
                return;
            }
            mode::SUMMON if self.time != 0. => {}
            _ => return,
        }
        if zako {
            v *= DRIVE_ENEMY;
        }
        self.gain(v, mp_charging, drive_boost, 1.);
    }

    /// FUN_1403a85f0: damage taken in mode 1 gives `damage / MaxHP × Damage
    /// Drive`.
    pub fn damaged(&mut self, damage: i32, max_hp: i32, damage_drive: f32) {
        if self.mode == mode::DRIVE && damage > 0 && max_hp > 0 && damage_drive > 0. {
            self.gain(damage as f32 / max_hp as f32 * damage_drive, false, 1., 1.);
        }
    }

    /// FUN_1403c0b50: form (or summon) time, clamped to 0..=max. Antiform
    /// turns a refill into a drain. Returns the whole bars used up (a
    /// summon's EXP; FUN_14040d530 is not ported).
    pub fn add_time(&mut self, d: f32) -> i32 {
        let d = if self.form == 6 && d > 0. { -d } else { d };
        let bar = |t: f32, mx: f32| (t / FORM_BAR_TIME) as i32 - (t == mx) as i32;
        let before = bar(self.time, self.max_time);
        self.time = (self.time + d).clamp(0., self.max_time.max(0.));
        before - bar(self.time, self.max_time)
    }

    /// FUN_1403a92c0's form/summon part, once per frame while the owner is
    /// active and no event pauses play: `dt` (1.0), halved out of battle,
    /// × Form Boost in a form (× Summon Boost in a summon).
    pub fn drain_frame(&mut self, battle: bool, form_boost: f32, summon_boost: f32) {
        if !matches!(self.mode, mode::FORM | mode::SUMMON) {
            return;
        }
        let mut d = 1.;
        if !battle {
            d *= FIELD_DRAIN;
        }
        d *= if self.mode == mode::FORM {
            form_boost
        } else {
            summon_boost
        };
        self.add_time(-d);
    }

    /// FUN_1403c17c0's gauge part: entering form `form` at level `level`
    /// (the form record's byte +2; Antiform uses 5 − 2) gives level + 3
    /// bars of 600 time each. The caller refills HP and MP and cancels an
    /// MP charge.
    pub fn enter_form(&mut self, form: u8, level: u8) {
        let level = if form == 6 { ANTI_LEVEL - 2 } else { level };
        self.mode = mode::FORM;
        self.form = form;
        self.bars = level + 3;
        self.time = self.bars as f32 * FORM_BAR_TIME;
        self.max_time = self.time;
    }

    /// FUN_1403c2740 with mode 3: a summon at `level` gives level + 3 bars of
    /// SSTM +0x68 time each.
    pub fn enter_summon(&mut self, level: u8) {
        self.mode = mode::SUMMON;
        self.bars = level + 3;
        self.time = self.bars as f32 * FORM_BAR_TIME;
        self.max_time = self.time;
    }

    /// The YS::SORA constructor after a revert (FUN_140405030): the gauge
    /// is in Drive mode again; the bars and time keep their values.
    pub fn leave_form(&mut self) {
        self.mode = mode::DRIVE;
        self.form = 0;
    }

    /// FUN_1403c2020: a refill (level = max, partial 100, form time 0).
    pub fn refill(&mut self) {
        self.level = self.max;
        self.time = 0.;
        self.max_time = 0.;
        self.partial = 100;
    }

    /// Whether the owner is in Antiform (FUN_1403c12f0's damage-taken
    /// term reads obj+0xde0 = 6).
    pub fn antiform(&self) -> bool {
        self.mode == mode::FORM && self.form == 6
    }

    /// The HUD digit in form mode (hud-01 §6.3): `min(time / per, bars − 1)`
    /// with `per = (int)(max time / bars)`.
    pub fn form_digit(&self) -> u8 {
        let per = (self.max_time / self.bars.max(1) as f32) as i32;
        if per <= 0 {
            return 0;
        }
        (self.time as i32 / per).min(self.bars as i32 - 1).max(0) as u8
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn adds_carry_into_bars_and_stop_at_the_maximum() {
        let mut g = Gauge::new(3);
        g.add(60);
        g.add(60);
        assert_eq!((g.level, g.partial), (1, 20));
        g.add(250);
        assert_eq!((g.level, g.partial), (3, 100));
        g.add(10);
        assert_eq!((g.level, g.partial), (3, 100));
    }

    #[test]
    fn a_negative_add_borrows_one_bar() {
        let mut g = Gauge {
            partial: 10,
            level: 2,
            ..Gauge::new(3)
        };
        g.add(-30);
        assert_eq!((g.level, g.partial), (1, 80));
        let mut z = Gauge::new(3);
        z.add(-5);
        assert_eq!((z.level, z.partial), (0, 0));
    }

    #[test]
    fn paying_empties_the_partial_bar_of_a_full_gauge() {
        let mut g = Gauge {
            partial: 100,
            level: 3,
            ..Gauge::new(3)
        };
        g.pay(3);
        assert_eq!((g.level, g.partial), (0, 0));
        let mut h = Gauge {
            partial: 40,
            level: 5,
            ..Gauge::new(9)
        };
        h.pay(3);
        assert_eq!((h.level, h.partial), (2, 40));
        let mut short = Gauge {
            partial: 40,
            level: 2,
            ..Gauge::new(9)
        };
        short.pay(3);
        assert_eq!((short.level, short.partial), (0, 0));
    }

    #[test]
    fn hits_gain_by_drain_with_zako_and_charge_factors() {
        let mut g = Gauge::new(3);
        g.hit(8, true, false, 1.);
        assert_eq!(g.partial, 6);
        g.hit(8, false, true, 1.5);
        assert_eq!(g.partial, 6 + 24);
        g.hit(0, true, false, 1.);
        assert_eq!(g.partial, 30);
        g.damaged(10, 20, 20.);
        assert_eq!(g.partial, 40);
    }

    #[test]
    fn a_form_runs_on_time_that_hits_drain_and_orbs_refill() {
        let mut g = Gauge::new(5);
        g.level = 4;
        g.pay(3);
        g.enter_form(3, 1);
        assert_eq!(
            (g.mode, g.bars, g.time, g.max_time),
            (mode::FORM, 4, 2400., 2400.)
        );
        assert_eq!(g.form_digit(), 3);
        g.drain_frame(true, 1., 1.);
        assert_eq!(g.time, 2399.);
        g.drain_frame(false, 0.8, 1.);
        assert_eq!(g.time, 2398.6);
        g.hit(10, false, false, 1.);
        assert_eq!(g.time, 2248.6);
        g.hit(10, true, false, 1.);
        assert_eq!(g.time, 2218.6);
        g.gain(3., false, 1., 1.);
        assert_eq!(g.time, 2254.6);
        // Drive points stay as they were while in a form.
        assert_eq!((g.level, g.partial), (1, 0));
        g.damaged(50, 100, 50.);
        assert_eq!((g.level, g.partial), (1, 0));
        g.add_time(-5000.);
        assert_eq!(g.time, 0.);
        g.leave_form();
        assert_eq!(g.mode, mode::DRIVE);
    }

    #[test]
    fn antiform_has_six_bars_and_refills_drain() {
        let mut g = Gauge::new(9);
        g.enter_form(6, 7);
        assert_eq!((g.bars, g.max_time), (6, 3600.));
        g.add_time(-100.);
        g.gain(1., false, 1., 1.);
        assert_eq!(g.time, 3600. - 112.);
    }

    #[test]
    fn the_time_add_counts_bars_used_up() {
        let mut g = Gauge::new(9);
        g.enter_form(1, 1);
        // At the maximum the bar count is one less (4 bars, 2400).
        assert_eq!(g.add_time(-1.), 0);
        assert_eq!(g.add_time(-600.), 1);
    }

    #[test]
    fn a_summon_drains_at_its_boost_and_takes_gains() {
        let mut g = Gauge::new(9);
        g.enter_summon(0);
        assert_eq!((g.mode, g.bars, g.max_time), (mode::SUMMON, 3, 1800.));
        g.drain_frame(true, 2., 0.5);
        assert_eq!(g.time, 1799.5);
        g.gain(1., false, 1., 1.);
        assert_eq!(g.time, 1800.);
    }
}
