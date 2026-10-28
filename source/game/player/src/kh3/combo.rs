//! KH3's ground and air combos in the KH3 ruleset, translated from the KH3
//! executable: the chain's length (0x141433570: 4 attacks, one more a
//! Combo Plus or Air Combo Plus), its finishers (0x141433a60: one, one more
//! for Speed Slash or Triple Rush, one more for Magic Flash) and the choice
//! of each attack (the attack state's selector, 0x14134aca0): a pattern
//! picked at the chain's start by distance, the regular hits alternating by
//! it, the jump to a target above, the finishers last, and the launch as a
//! one-shot per chain that starts the count again. KH3's attacks are named
//! by their ids in that selector; each plays a KH2 attack record of Sora's
//! as its stand-in (`stand_in`).
//!
//! Distances are KH3's (cm) scaled like the layer's.

use super::ability::{Ability as A, Set};
use super::tuning::length_of as cm;

/// KH3's attack ids (the selector's), by the motion each plays.
pub mod id {
    /// The regular hits (motions 1301-1303) and the standard finisher
    /// (1304).
    pub const HIT_1: u8 = 1;
    pub const HIT_2: u8 = 2;
    pub const HIT_3: u8 = 3;
    pub const FINISH: u8 = 4;
    /// Speed Slash and Triple Rush's finishers (1305, 1306), Magic Flash's
    /// (1307).
    pub const SPEED_SLASH: u8 = 5;
    pub const TRIPLE_RUSH: u8 = 6;
    pub const MAGIC_FLASH: u8 = 7;
    /// The jump up to a target above (1308).
    pub const JUMP_UP: u8 = 8;
    /// The aerial down slash (1310).
    pub const AIR_DOWN: u8 = 0x0f;
    /// The aerial versions are the ground ids plus this.
    pub const AERIAL: u8 = 0x0f;
    /// Rising Spiral's launch (1319).
    pub const LAUNCH: u8 = 0x1c;
    /// The post-guard counter, Counter Slash's (RevengeSlash).
    pub const COUNTER: u8 = 0x21;
    /// Payback Strike's attack out of the recovery (BlowCounter).
    pub const BLOW_COUNTER: u8 = 0x27;
}

/// 0x141330d30 / 0x14134aca0: the close / far line (after 50 cm off the
/// distance) and the height lines.
pub const FAR: f32 = 400.;
pub const DISTANCE_OFF: f32 = 50.;
pub const BELOW: f32 = -100.;
/// [I] The "target is high" line (pawn +0xba8, a Blueprint value): the
/// selector's launch line, 100 cm.
pub const HIGH: f32 = 100.;

/// 0x141433570: the chain's length.
pub fn combo_max(set: &Set, grounded: bool) -> u32 {
    4 + u32::from(set.count(if grounded {
        A::ComboPlus
    } else {
        A::AirComboPlus
    }))
}

/// 0x141433a60: how many of the chain's last attacks are finishers.
pub fn finisher_count(set: &Set) -> u32 {
    1 + u32::from(set.has(A::SpeedSlash) || set.has(A::TripleRush))
        + u32::from(set.has(A::MagicFlash))
}

/// What the selector reads of the moment (0x141330d30's geometry).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Ctx {
    /// The target's distance in XZ (scaled units), before the 50 cm off.
    pub distance: f32,
    /// The target's height over Sora's (scaled units).
    pub dz: f32,
    /// Sora is in the air.
    pub air: bool,
    /// Enemies near the target (the duel has one).
    pub crowd: u32,
}

impl Ctx {
    fn dist(&self) -> f32 {
        (self.distance - cm(DISTANCE_OFF)).max(0.)
    }
    fn far(&self) -> bool {
        self.dist() > cm(FAR)
    }
    fn high(&self) -> bool {
        self.dz > cm(HIGH)
    }
}

/// One chain's state (the attack state's fields and the pawn's that carry
/// it from attack to attack).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Chain {
    /// Attacks already done (st+0x128).
    pub n: u32,
    /// The pattern picked at the start: 0 normal, 1 far, 2 crowd.
    pattern: u8,
    /// The last attack's id (pawn+0xe10) and the finisher index (+0xe14).
    pub prev: u8,
    finisher: u8,
    /// Rising Spiral: 1 available this chain, 2 used.
    launch: u8,
    /// The chain's last attack was a finisher that ends it.
    pub done: bool,
}

/// What the selector chose.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Pick {
    pub id: u8,
    pub finisher: bool,
    /// The last attack of the chain.
    pub last: bool,
}

impl Chain {
    /// A chain begins (from a state that is not an attack).
    pub fn start(&mut self, set: &Set) {
        *self = Chain {
            launch: u8::from(set.has(A::RisingSpiral)),
            ..Chain::default()
        };
    }

    /// The chain may go on (not past its last attack).
    pub fn open(&self) -> bool {
        !self.done
    }

    /// The launch (Jump during a ground chain, Rising Spiral): once a
    /// chain; the count starts over after it (0x14134aca0's launch path).
    pub fn launch(&mut self) -> Option<Pick> {
        if self.launch != 1 {
            return None;
        }
        self.launch = 2;
        self.n = 0;
        self.finisher = 0;
        self.prev = id::LAUNCH;
        self.done = false;
        Some(Pick {
            id: id::LAUNCH,
            finisher: false,
            last: false,
        })
    }

    /// 0x14134aca0: the next attack of the chain (the n-th), then counted.
    pub fn next(&mut self, set: &Set, c: &Ctx) -> Pick {
        let n = self.n;
        let max = combo_max(set, !c.air);
        let fin = finisher_count(set);
        let mut first = false;
        if n == 0 || self.prev == id::LAUNCH {
            self.pattern = if c.far() {
                1
            } else if c.crowd > 1 {
                2
            } else {
                0
            };
            first = true;
        }
        let slot_a = set.has(A::SpeedSlash) || set.has(A::TripleRush);
        // The second and third finishers: the standard one moves second
        // when slot A takes the first; Magic Flash comes last.
        let (second, third) = match (slot_a, set.has(A::MagicFlash)) {
            (true, true) => (id::FINISH, id::MAGIC_FLASH),
            (false, true) => (id::MAGIC_FLASH, 0),
            (true, false) => (id::FINISH, 0),
            (false, false) => (0, 0),
        };
        let prev = self.prev;
        let regular = n + fin < max;
        let mut pick;
        if regular {
            pick = if c.high() && !matches!(prev, id::JUMP_UP | 0x17 | 0x1f) && !c.far() {
                id::JUMP_UP
            } else if !c.air || c.dz >= cm(BELOW) {
                let p = self.pattern;
                let mut id = match prev {
                    1 | 9 | 0x18 => {
                        if p == 1 {
                            3
                        } else {
                            2
                        }
                    }
                    2 | 0x0a | 0x11 | 0x19 => {
                        if p == 1 {
                            1
                        } else {
                            3
                        }
                    }
                    3 | 0x0b | 0x12 | 0x1a => {
                        if p == 1 {
                            1
                        } else {
                            2
                        }
                    }
                    8 | 0x1f => 1,
                    0x0f | 0x10 | 0x20 => 8,
                    0x17 => 3,
                    _ if first => match p {
                        1 => 2,
                        2 => 3,
                        _ => 1,
                    },
                    _ => match n % 3 {
                        0 => p + 1,
                        1 => {
                            if p == 0 {
                                2
                            } else {
                                1
                            }
                        }
                        _ => {
                            if p == 2 {
                                2
                            } else {
                                3
                            }
                        }
                    },
                };
                if c.air || (n > 0 && c.high()) {
                    id += id::AERIAL;
                }
                id
            } else {
                id::AIR_DOWN
            };
            self.finisher = 0;
        } else {
            let k = self.finisher;
            pick = match k {
                0 if slot_a => {
                    if set.has(A::TripleRush)
                        && (!set.has(A::SpeedSlash) || c.far() || c.crowd == 1)
                    {
                        id::TRIPLE_RUSH
                    } else {
                        id::SPEED_SLASH
                    }
                }
                0 => id::FINISH,
                1 => second,
                2 => third,
                _ => 1,
            };
            if pick == 0 {
                pick = id::FINISH;
            }
            if c.air || c.high() {
                pick += id::AERIAL;
            }
            self.finisher = k + 1;
        }
        let last = !regular && (u32::from(self.finisher) >= fin || n + 1 >= max);
        self.prev = pick;
        self.n = n + 1;
        self.done = last;
        Pick {
            id: pick,
            finisher: !regular,
            last,
        }
    }
}

/// The KH2 attack record (PTYA id) of Sora's that stands in for a KH3
/// attack: the ground chain's swings for the regular hits (151 near, 153
/// the dash for the long one), its finishers for KH3's, the air chain's
/// swings and finishers for the aerial ones, Upper Slash for the launch.
pub fn stand_in(id: u8) -> u8 {
    match id {
        id::HIT_1 => 0x05,
        id::HIT_2 => 0x06,
        id::HIT_3 => 0x04,
        id::FINISH => 0x02,
        id::SPEED_SLASH => 0x03,
        id::TRIPLE_RUSH => 0x1b,
        id::MAGIC_FLASH => 0x1a,
        id::JUMP_UP | 0x17 => 0x0a,
        id::AIR_DOWN => 0x0a,
        0x10 => 0x09,
        0x11 => 0x0a,
        0x12 => 0x24,
        0x13 => 0x07,
        0x14 => 0x08,
        0x15 => 0x1d,
        0x16 => 0x31,
        id::LAUNCH => 0x0c,
        id::COUNTER => 0x05,
        id::BLOW_COUNTER => 0x08,
        _ => 0x05,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn near() -> Ctx {
        Ctx {
            distance: cm(150.),
            dz: 0.,
            air: false,
            crowd: 1,
        }
    }

    #[test]
    fn a_plain_chain_is_three_hits_and_the_finisher() {
        let set = Set::NONE;
        let mut c = Chain::default();
        c.start(&set);
        let ids: Vec<u8> = (0..4).map(|_| c.next(&set, &near()).id).collect();
        assert_eq!(ids, vec![1, 2, 3, id::FINISH]);
        assert!(c.done && !c.open());
    }

    #[test]
    fn combo_plus_lengthens_and_finisher_abilities_take_the_last_slots() {
        let mut set = Set::NONE;
        set.put(A::ComboPlus, 2);
        assert_eq!(combo_max(&set, true), 6);
        assert_eq!(combo_max(&set, false), 4, "Air Combo Plus is the air's");
        set.put(A::TripleRush, 1);
        set.put(A::MagicFlash, 1);
        assert_eq!(finisher_count(&set), 3);
        let mut c = Chain::default();
        c.start(&set);
        let picks: Vec<Pick> = (0..6).map(|_| c.next(&set, &near())).collect();
        let ids: Vec<u8> = picks.iter().map(|p| p.id).collect();
        assert_eq!(&ids[3..], &[id::TRIPLE_RUSH, id::FINISH, id::MAGIC_FLASH]);
        assert!(picks[..3].iter().all(|p| !p.finisher));
        assert!(picks[5].last && c.done);
    }

    #[test]
    fn far_starts_with_the_long_hit_and_the_air_uses_aerial_ids() {
        let set = Set::NONE;
        let mut c = Chain::default();
        c.start(&set);
        let far = Ctx {
            distance: cm(900.),
            ..near()
        };
        assert_eq!(c.next(&set, &far).id, 2);
        let mut c = Chain::default();
        c.start(&set);
        let air = Ctx {
            air: true,
            ..near()
        };
        assert_eq!(c.next(&set, &air).id, 1 + id::AERIAL);
        // A target well below an airborne Sora: the down slash.
        let below = Ctx {
            dz: cm(-200.),
            ..air
        };
        assert_eq!(c.next(&set, &below).id, id::AIR_DOWN);
        // A target above a grounded Sora: the jump up to it.
        let mut c = Chain::default();
        c.start(&set);
        let high = Ctx {
            dz: cm(250.),
            ..near()
        };
        assert_eq!(c.next(&set, &high).id, id::JUMP_UP);
    }

    #[test]
    fn the_launch_is_once_a_chain_and_starts_the_count_again() {
        let mut set = Set::NONE;
        let mut c = Chain::default();
        c.start(&set);
        assert_eq!(c.launch(), None, "without Rising Spiral");
        set.put(A::RisingSpiral, 1);
        c.start(&set);
        c.next(&set, &near());
        c.next(&set, &near());
        assert_eq!(c.launch().map(|p| p.id), Some(id::LAUNCH));
        assert_eq!(c.n, 0);
        assert_eq!(c.launch(), None);
        // Four more attacks after it.
        let air = Ctx {
            air: true,
            ..near()
        };
        for _ in 0..3 {
            assert!(!c.next(&set, &air).finisher);
        }
        assert!(c.next(&set, &air).last);
    }

    #[test]
    fn every_kh3_attack_has_a_stand_in() {
        for i in 0..0x40 {
            assert_ne!(stand_in(i), 0);
        }
        assert_eq!(stand_in(id::LAUNCH), crate::kh3::record::LAUNCH);
    }
}
