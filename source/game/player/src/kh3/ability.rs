//! KH3's abilities in the KH3 ruleset: which of the layer's moves a Sora may
//! use, equipped from a list with AP costs (the duel lobby's KH3 page and
//! Test KH3 mode's menu). They replace KH2's abilities for everything the
//! layer owns; the KH2 controller's own gates (Guard, Combo Plus, Combo
//! Master) are set from them every tick (`apply_kh2`).
//!
//! Names and AP costs are KH3's as the game lists them (its ability data
//! table lives in KH3's paks, so they come from the public ability lists).
//! Each ability's `kind` is its `ETresAbilityKind` entry, whose value is
//! the id KH3's own checks use (the enum's constructor, 0x141818490; the
//! checks behind the pawn's ability query, vtable +0x14e0). Stackable
//! abilities count copies, as KH3's equip count does (0x140e8aab0).

use crate::abilities::{self, Abilities};

/// One KH3 ability the ruleset uses.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Ability {
    DodgeRoll,
    AirSlide,
    Doubleflight,
    HighJump,
    Glide,
    Superjump,
    Superslide,
    WallKick,
    PoleSpin,
    Block,
    AerialRecovery,
    CounterSlash,
    PaybackStrike,
    FinalBlow,
    RiskDodge,
    FocusSyphon,
    RisingSpiral,
    SonicSlash,
    BuzzSaw,
    SonicCleave,
    SpeedSlash,
    TripleRush,
    MagicFlash,
    ComboPlus,
    AirComboPlus,
    ComboMaster,
    ComboBoost,
    AirComboBoost,
    QuickSlash,
    FlashStep,
    RadialBlaster,
    LastCharge,
    AerialSweep,
    AerialDive,
    Groundbreaker,
    FallingSlash,
    CounterImpact,
    CounterKick,
    CriticalCounter,
    CounterBoost,
    FireBoost,
    BlizzardBoost,
    ThunderBoost,
    WaterBoost,
    AeroBoost,
    MagicComboBoost,
}

/// An ability's list entry: name, AP a copy, copies allowed, the help line,
/// its `ETresAbilityKind` name.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Info {
    pub ability: Ability,
    pub name: &'static str,
    pub ap: u8,
    pub most: u8,
    pub help: &'static str,
    pub kind: &'static str,
}

const fn info(
    ability: Ability,
    name: &'static str,
    ap: u8,
    most: u8,
    kind: &'static str,
    help: &'static str,
) -> Info {
    Info {
        ability,
        name,
        ap,
        most,
        help,
        kind,
    }
}

/// The list in menu order: movement, then reprisals and actions, then the
/// combo finishers and supports. Block's check was not found in the
/// executable (REFLECT_GUARD is the likeliest entry).
pub const ALL: [Info; 46] = [
    info(
        Ability::DodgeRoll,
        "Dodge Roll",
        3,
        1,
        "DODGE",
        "Guard with the stick held rolls out of the way.",
    ),
    info(
        Ability::AirSlide,
        "Air Slide",
        1,
        1,
        "AIRSLIDE",
        "Guard with the stick held in the air dashes.",
    ),
    info(
        Ability::Doubleflight,
        "Doubleflight",
        2,
        1,
        "DOUBLEFLIGHT",
        "Jump again in the air.",
    ),
    info(
        Ability::HighJump,
        "High Jump",
        2,
        1,
        "HIGHJUMP",
        "Jumps higher.",
    ),
    info(
        Ability::Glide,
        "Glide",
        1,
        1,
        "GLIDE",
        "Hold Jump while falling to glide.",
    ),
    info(
        Ability::Superjump,
        "Superjump",
        2,
        1,
        "SUPERJUMP",
        "Jump out of a wall run, kick, pole or Superslide flies high.",
    ),
    info(
        Ability::Superslide,
        "Superslide",
        1,
        1,
        "SUPERSLIDE",
        "Guard after flowmotion dashes far.",
    ),
    info(
        Ability::WallKick,
        "Wall Kick",
        1,
        1,
        "WALL_KICK",
        "An air dash into a wall kicks off it.",
    ),
    info(
        Ability::PoleSpin,
        "Pole Spin",
        1,
        1,
        "POLE_SPIN",
        "Dash into a pole to spin around it.",
    ),
    info(
        Ability::Block,
        "Block",
        2,
        1,
        "REFLECT_GUARD",
        "Guard blocks attacks from the front.",
    ),
    info(
        Ability::AerialRecovery,
        "Aerial Recovery",
        2,
        1,
        "AIR_RECOVERY",
        "Jump while knocked into the air rights you.",
    ),
    info(
        Ability::CounterSlash,
        "Counter Slash",
        4,
        1,
        "GUARD_COUNTER",
        "Attack within 1 s of a block strikes back.",
    ),
    info(
        Ability::PaybackStrike,
        "Payback Strike",
        5,
        1,
        "BLOW_COUNTER",
        "Attack after an Aerial Recovery strikes back.",
    ),
    info(
        Ability::FinalBlow,
        "Final Blow",
        5,
        1,
        "REVENGE_EX",
        "Attack after a reprisal knocks away: warp after it.",
    ),
    info(
        Ability::RiskDodge,
        "Risk Dodge",
        3,
        1,
        "RISKDODGE",
        "Jump after blocking a heavy hit slips behind.",
    ),
    info(
        Ability::FocusSyphon,
        "Focus Syphon",
        3,
        1,
        "FOCUS_ASPIR",
        "With no focus, the shotlock draws focus back.",
    ),
    info(
        Ability::RisingSpiral,
        "Rising Spiral",
        3,
        1,
        "SLASH_UPPER",
        "Jump during a ground combo launches.",
    ),
    info(
        Ability::SonicSlash,
        "Sonic Slash",
        2,
        1,
        "SONIC_SLASH",
        "Attack in a Superslide or after an airstep.",
    ),
    info(
        Ability::BuzzSaw,
        "Buzz Saw",
        2,
        1,
        "TURN_CUTTER",
        "Attack on a pole spins into the target.",
    ),
    info(
        Ability::SonicCleave,
        "Sonic Cleave",
        2,
        1,
        "SONIC_DOWN",
        "Attack during a wall run.",
    ),
    info(
        Ability::SpeedSlash,
        "Speed Slash",
        3,
        1,
        "TRIPPLE_SLASH",
        "A combo finisher; the regular hits make room for it.",
    ),
    info(
        Ability::TripleRush,
        "Triple Rush",
        4,
        1,
        "CHARGE_THRUST",
        "A combo finisher that closes in; the regular hits make room for it.",
    ),
    info(
        Ability::MagicFlash,
        "Magic Flash",
        5,
        1,
        "MAGICFLUSH",
        "A last combo finisher; the regular hits make room for it.",
    ),
    info(
        Ability::ComboPlus,
        "Combo Plus",
        4,
        3,
        "COMBO_PLUS",
        "One more attack in a ground combo.",
    ),
    info(
        Ability::AirComboPlus,
        "Air Combo Plus",
        4,
        3,
        "AIRCOMBO_PLUS",
        "One more attack in an air combo.",
    ),
    info(
        Ability::ComboMaster,
        "Combo Master",
        4,
        1,
        "COMBO_MASTER",
        "Combos go on when attacks miss.",
    ),
    info(
        Ability::ComboBoost,
        "Combo Boost",
        5,
        2,
        "COMBO_UP",
        "Ground combos hit harder as they go on.",
    ),
    info(
        Ability::AirComboBoost,
        "Air Combo Boost",
        4,
        2,
        "AIRCOMBO_UP",
        "Air combos hit harder as they go on.",
    ),
    info(
        Ability::QuickSlash,
        "Quick Slash",
        3,
        1,
        "QUICK_SHAFT",
        "The first of the combo's swings is a faster one.",
    ),
    info(
        Ability::FlashStep,
        "Flash Step",
        3,
        1,
        "FLASH_STEP",
        "The long reach swing rushes at the target.",
    ),
    info(
        Ability::RadialBlaster,
        "Radial Blaster",
        3,
        1,
        "RADIAL_CUT",
        "The third swing hits all around.",
    ),
    info(
        Ability::LastCharge,
        "Last Charge",
        7,
        1,
        "FINAL_HEAVEN",
        "A last finisher that charges through the target.",
    ),
    info(
        Ability::AerialSweep,
        "Aerial Sweep",
        2,
        1,
        "AERIAL_SWEEP",
        "The rising swing sweeps upward.",
    ),
    info(
        Ability::AerialDive,
        "Aerial Dive",
        2,
        1,
        "AERIAL_DIVE",
        "The swing that falls dives on the target.",
    ),
    info(
        Ability::Groundbreaker,
        "Groundbreaker",
        3,
        1,
        "AIR_ROLL_BEAT",
        "Jump in a combo strikes down from the air.",
    ),
    info(
        Ability::FallingSlash,
        "Falling Slash",
        2,
        1,
        "AIR_DOWN",
        "From high above, the swing dives to the ground.",
    ),
    info(
        Ability::CounterImpact,
        "Counter Impact",
        4,
        1,
        "REVENGEIMPACT",
        "The counter after a guard is a burst around you.",
    ),
    info(
        Ability::CounterKick,
        "Counter Kick",
        4,
        1,
        "REVENGEDIVE",
        "The counter after a guard is a diving kick.",
    ),
    info(
        Ability::CriticalCounter,
        "Critical Counter",
        5,
        1,
        "CRITICAL_COUNTER",
        "A counter after a well timed guard does double damage.",
    ),
    info(
        Ability::CounterBoost,
        "Counter Boost",
        3,
        2,
        "COUNTER_UP",
        "Counters hit harder.",
    ),
    info(
        Ability::FireBoost,
        "Fire Boost",
        4,
        3,
        "FIRE_UP",
        "Fire hits harder.",
    ),
    info(
        Ability::BlizzardBoost,
        "Blizzard Boost",
        4,
        3,
        "BLIZZARD_UP",
        "Blizzard hits harder.",
    ),
    info(
        Ability::ThunderBoost,
        "Thunder Boost",
        4,
        3,
        "THUNDER_UP",
        "Thunder hits harder.",
    ),
    info(
        Ability::WaterBoost,
        "Water Boost",
        4,
        3,
        "WATER_UP",
        "Water hits harder.",
    ),
    info(
        Ability::AeroBoost,
        "Aero Boost",
        4,
        3,
        "AERO_UP",
        "Aero hits harder.",
    ),
    info(
        Ability::MagicComboBoost,
        "Magic Combo Boost",
        5,
        2,
        "MAGIC_COMBO_UP",
        "Spells in a combo hit harder as it goes on.",
    ),
];

/// The AP a KH3 build may use: level 99's `m_AbilityPoint` in KH3's level-up
/// table. How many copies of an ability may be worn (`Info::most`) is the
/// duel's choice: the table's `m_MaxEquip` is 1 even for those KH3 stacks.
pub const AP_MAX: u32 = 180;

/// Copies equipped, two bits an ability, in `ALL` order.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Set(pub u128);

impl Default for Set {
    /// Every move the ruleset had before its menu (the layer's whole kit),
    /// no combo supports.
    fn default() -> Self {
        let mut s = Set(0);
        for i in ALL.iter().take_while(|i| i.ability != Ability::SpeedSlash) {
            s.put(i.ability, 1);
        }
        s
    }
}

fn index(a: Ability) -> usize {
    ALL.iter()
        .position(|i| i.ability == a)
        .expect("every ability is listed")
}

impl Set {
    pub const NONE: Set = Set(0);

    /// Copies of `a` equipped.
    pub fn count(&self, a: Ability) -> u8 {
        (self.0 >> (index(a) * 2) & 3) as u8
    }

    pub fn has(&self, a: Ability) -> bool {
        self.count(a) > 0
    }

    /// Sets the copies of `a` (clamped to its most).
    pub fn put(&mut self, a: Ability, n: u8) {
        let i = index(a);
        let n = u128::from(n.min(ALL[i].most).min(3));
        self.0 = self.0 & !(3 << (i * 2)) | n << (i * 2);
    }

    /// AP used.
    pub fn ap(&self) -> u32 {
        ALL.iter()
            .map(|i| u32::from(i.ap) * u32::from(self.count(i.ability)))
            .sum()
    }

    /// Only listed abilities, each within its copies.
    pub fn valid(&self) -> bool {
        self.0 >> (ALL.len() * 2) == 0 && ALL.iter().all(|i| self.count(i.ability) <= i.most)
    }

    /// Steps `a` to its next (or previous) count, wrapping; refused when the
    /// AP would go over `max`.
    pub fn step(&mut self, a: Ability, forward: bool, max: u32) -> bool {
        let most = ALL[index(a)].most;
        let n = self.count(a);
        let next = if forward {
            if n >= most { 0 } else { n + 1 }
        } else if n == 0 {
            most
        } else {
            n - 1
        };
        let mut t = *self;
        t.put(a, next);
        if t.ap() > max && t.ap() > self.ap() {
            return false;
        }
        *self = t;
        true
    }

    /// The KH2 controller's gates in the KH3 ruleset: Guard is Block, the
    /// combo limits and Combo Master are KH3's, and the KH2 build's own
    /// combo abilities do nothing.
    pub fn apply_kh2(&self, a: &mut Abilities) {
        let mut set = |bit, on| {
            if on { a.set(bit) } else { a.clear(bit) }
        };
        set(abilities::bit::GUARD, self.has(Ability::Block));
        set(
            abilities::bit::AERIAL_RECOVERY,
            self.has(Ability::AerialRecovery),
        );
        set(abilities::bit::COMBO_MASTER, self.has(Ability::ComboMaster));
        set(abilities::bit::BERSERK, false);
        // The KH2 damage terms read the combo limits: KH3's chain's.
        a.ground_combo = super::combo::combo_max(self, true) as i8;
        a.air_combo = super::combo::combo_max(self, false) as i8;
        a.finishers = super::combo::finisher_count(self) as u8;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_default_is_the_whole_kit_within_the_ap() {
        let s = Set::default();
        assert!(s.valid());
        assert!(s.has(Ability::DodgeRoll) && s.has(Ability::SonicCleave));
        assert!(!s.has(Ability::ComboPlus));
        assert!(s.ap() <= AP_MAX, "{}", s.ap());
    }

    #[test]
    fn stepping_wraps_and_respects_the_ap() {
        let mut s = Set::NONE;
        assert!(s.step(Ability::ComboPlus, true, AP_MAX));
        assert!(s.step(Ability::ComboPlus, true, AP_MAX));
        assert_eq!(s.count(Ability::ComboPlus), 2);
        assert!(s.step(Ability::ComboPlus, true, AP_MAX));
        assert!(s.step(Ability::ComboPlus, true, AP_MAX));
        assert_eq!(s.count(Ability::ComboPlus), 0, "wraps after the most");
        assert!(s.step(Ability::ComboPlus, false, AP_MAX));
        assert_eq!(s.count(Ability::ComboPlus), 3, "back wraps to the most");
        // Over the AP: refused, unchanged; taking one off is always allowed.
        let mut tight = Set::NONE;
        assert!(!tight.step(Ability::FinalBlow, true, 4));
        assert_eq!(tight, Set::NONE);
        let mut full = Set::default();
        let before = full.count(Ability::DodgeRoll);
        assert!(full.step(Ability::DodgeRoll, true, 0));
        assert_ne!(full.count(Ability::DodgeRoll), before);
    }

    #[test]
    fn kh2_gates_follow_the_set() {
        let mut a = Abilities::default();
        a.set(abilities::bit::COMBO_MASTER);
        let mut s = Set::NONE;
        s.put(Ability::Block, 1);
        s.put(Ability::ComboPlus, 2);
        s.apply_kh2(&mut a);
        assert!(a.has(abilities::bit::GUARD));
        assert!(!a.has(abilities::bit::COMBO_MASTER));
        assert_eq!(a.ground_combo, 6, "KH3's 4 and two Combo Plus");

        assert!(!Set::NONE.has(Ability::Block));
    }
}
