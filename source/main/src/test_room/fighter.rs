//! One Sora in the room: the state machine and what the room keeps for him
//! alone (HP, battle stats, level, hit record).

use player::Player;
use player::combat;

pub(crate) struct Fighter {
    pub(crate) player: Player,
    pub(crate) vitals: Vitals,
    /// [KH3 ruleset] The KH3 layer's state (duel_kh3.rs), and a grand
    /// magic's window: ticks left and the MP it keeps.
    pub(crate) kh3: player::kh3::Kh3,
    pub(crate) kh3_grand: Option<(u32, i32)>,
}

impl Fighter {
    pub(crate) fn new(player: Player) -> Fighter {
        Fighter {
            player,
            vitals: Vitals::default(),
            kh3: player::kh3::Kh3::default(),
            kh3_grand: None,
        }
    }
}

/// Sora's battle numbers (the exe keeps them on his object and its stats):
/// filled from LVUP, PLRP and the equipment when the room has battle
/// tables (a room `enemy`); the defaults otherwise.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Vitals {
    pub(crate) attacker: combat::AttackerStats,
    pub(crate) target: combat::TargetStats,
    pub(crate) hp: i32,
    /// Ticks spent knocked out (the test room stands Sora up after 120).
    pub(crate) ko_ticks: u32,
    pub(crate) level: u8,
    /// Invulnerability after a guard (+0xd70), in ticks.
    pub(crate) invulnerable: f32,
    /// The hit record (obj+0xc98): the hit-once mask and the 30-tick group
    /// lock of a player-type object.
    pub(crate) record: combat::HitRecord,
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
