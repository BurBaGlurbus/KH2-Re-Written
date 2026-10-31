//! One Sora in the room: the state machine and what the room keeps for him
//! alone (HP, battle stats, level, hit record).

use player::Player;
use player::combat;

pub struct Fighter {
    pub player: Player,
    pub vitals: Vitals,
    /// An extension's state for this Sora (`extension.rs`).
    pub ext: super::extension::Extension,
}

impl Fighter {
    pub fn new(player: Player) -> Fighter {
        Fighter {
            player,
            vitals: Vitals::default(),
            ext: (super::hooks().new_fighter)(),
        }
    }
}

/// Sora's battle numbers (the exe keeps them on his object and its stats):
/// filled from LVUP, PLRP and the equipment when the room has battle
/// tables (a room `enemy`); the defaults otherwise.
#[derive(Clone, Debug, PartialEq)]
pub struct Vitals {
    pub attacker: combat::AttackerStats,
    pub target: combat::TargetStats,
    pub hp: i32,
    /// Ticks spent knocked out (the test room stands Sora up after 120).
    pub ko_ticks: u32,
    pub level: u8,
    /// Invulnerability after a guard (+0xd70), in ticks.
    pub invulnerable: f32,
    /// The hit record (obj+0xc98): the hit-once mask and the 30-tick group
    /// lock of a player-type object.
    pub record: combat::HitRecord,
}

impl Default for Vitals {
    fn default() -> Self {
        Vitals {
            attacker: combat::AttackerStats::default(),
            target: combat::TargetStats {
                max_hp: 0,
                defense: 0,
                cap: 0,
                min: 0,
                weakness: [100; 7],
            },
            hp: 0,
            ko_ticks: 0,
            level: 1,
            invulnerable: 0.,
            record: combat::HitRecord::new(true),
        }
    }
}
