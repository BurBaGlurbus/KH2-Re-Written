//! The duel's match rules. The host chooses them in the lobby (and with
//! `--duel-damage`, `--duel-magic-damage`, `--duel-hit-cap` / F3); the
//! joiner receives them and shows them read-only.
//! The modes are a table, so a new one is a row and its match flow.

/// How a match is won.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Mode {
    /// The first knock-out wins.
    Stock,
    /// Rounds, each to a knock-out; the first to win two takes the match.
    BestOf3,
    /// A knock-out wins; when the time runs out, the larger share of HP
    /// left wins (equal shares draw).
    Timed,
}

/// One row of the mode table.
pub(crate) struct ModeInfo {
    pub mode: Mode,
    /// The lobby's name for it.
    pub name: &'static str,
    /// Whether the match runs against the time limit.
    pub timed: bool,
}

/// The modes, in the order the host's lobby cycles them.
pub(crate) const MODES: [ModeInfo; 3] = [
    ModeInfo {
        mode: Mode::Stock,
        name: "One KO",
        timed: false,
    },
    ModeInfo {
        mode: Mode::BestOf3,
        name: "Best of 3",
        timed: false,
    },
    ModeInfo {
        mode: Mode::Timed,
        name: "Time limit",
        timed: true,
    },
];

/// A mode by its command-line name (`--duel-mode`).
pub(crate) fn mode_named(name: &str) -> Option<Mode> {
    match name {
        "stock" => Some(Mode::Stock),
        "best3" => Some(Mode::BestOf3),
        "time" => Some(Mode::Timed),
        _ => None,
    }
}

/// Which game's moves both Soras fight with [duel choice, T279]: KH2's
/// own controller, or the KH3 layer over it (`player::kh3`: KH3 movement,
/// flowmotion, focus and shotlock, on KH2's motions and effects).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) enum Ruleset {
    #[default]
    Kh2,
    Kh3,
}

/// The rulesets in the lobby's order: (ruleset, lobby name, command-line
/// name).
pub(crate) const RULESETS: [(Ruleset, &str, &str); 2] =
    [(Ruleset::Kh2, "KH2", "kh2"), (Ruleset::Kh3, "KH3", "kh3")];

/// A ruleset by its command-line name (`--duel-ruleset`).
pub(crate) fn ruleset_named(name: &str) -> Option<Ruleset> {
    RULESETS.iter().find(|r| r.2 == name).map(|r| r.0)
}

/// The time limits the host can pick, in seconds.
pub(crate) const TIME_LIMITS: [u16; 5] = [60, 90, 120, 180, 300];
const DEFAULT_TIME: u16 = 180;

/// The default damage rule [duel choice]. Measured with the scripted rival
/// swinging the Kingdom Key at an idle Sora at level 99 (KH2RW_RIVAL_BENCH,
/// T241): 0.46 hits landed a second at 2 damage, so 170 HP lasts about
/// 185 s at x1. x3 makes that about a minute, and 30-45 s for players who
/// land more of their combos.
pub(crate) const DEFAULT_DAMAGE: f32 = 3.;
/// The default magic damage rule [duel choice; T245's benchmark]: hits
/// whose power is the attacker's Magic (spells, Reflect's counter, form
/// magic) take this instead of the attacks' multiplier.
pub(crate) const DEFAULT_MAGIC: f32 = 1.5;
/// The lobby's and F3 menu's step and the range they allow (both
/// multipliers).
pub(crate) const DAMAGE_STEP: f32 = 0.5;
pub(crate) const DAMAGE_RANGE: (f32, f32) = (0.5, 10.);
/// The per-hit caps the host can pick, in percent of the target's max HP
/// (100: no cap).
pub(crate) const HIT_CAPS: [u8; 9] = [20, 25, 30, 35, 40, 50, 60, 75, 100];
const DEFAULT_HIT_CAP: u8 = 40;

/// The duel's rules (the host's; the joiner shows them).
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct MatchRules {
    pub mode: Mode,
    /// What a Sora-against-Sora hit's damage is multiplied by, after the
    /// game's own formula: hits on Strength (attacks) ...
    pub damage: f32,
    /// ... and hits on Magic (spells, Reflect's counter, form magic).
    pub magic: f32,
    /// No single hit takes more than this percent of the target's max HP
    /// (100: no cap).
    pub hit_cap: u8,
    /// Seconds, for the timed mode.
    pub time_limit: u16,
    /// The arena (a row of `duel_arena::ARENAS`).
    pub arena: u8,
    /// KH2's moves or KH3's.
    pub ruleset: Ruleset,
}

impl Default for MatchRules {
    fn default() -> Self {
        MatchRules {
            mode: Mode::Stock,
            damage: DEFAULT_DAMAGE,
            magic: DEFAULT_MAGIC,
            hit_cap: DEFAULT_HIT_CAP,
            time_limit: DEFAULT_TIME,
            arena: 0,
            ruleset: Ruleset::Kh2,
        }
    }
}

impl MatchRules {
    /// A hit's damage under the rules: the formula's number times the
    /// attacks' or the magic multiplier (`magic`: the hit's power is the
    /// attacker's Magic), rounded, then no more than the cap's share of
    /// `max_hp`; a hit that did damage still does at least 1.
    pub(crate) fn damage(&self, formula: i32, magic: bool, max_hp: i32) -> i32 {
        if formula <= 0 {
            return formula;
        }
        let k = if magic { self.magic } else { self.damage };
        let d = ((formula as f32 * k).round() as i32).max(1);
        if self.hit_cap >= 100 {
            return d;
        }
        let cap = (max_hp * i32::from(self.hit_cap) / 100).max(1);
        d.min(cap)
    }

    /// The mode's row.
    pub(crate) fn info(&self) -> &'static ModeInfo {
        MODES
            .iter()
            .find(|m| m.mode == self.mode)
            .unwrap_or(&MODES[0])
    }

    /// The next (or previous) mode in the table.
    pub(crate) fn cycle_mode(&mut self, step: i32) {
        let at = MODES.iter().position(|m| m.mode == self.mode).unwrap_or(0) as i32;
        let n = MODES.len() as i32;
        self.mode = MODES[(at + step).rem_euclid(n) as usize].mode;
    }

    /// The damage multiplier one step up or down, kept in range.
    pub(crate) fn step_damage(&mut self, step: i32) {
        self.damage =
            (self.damage + DAMAGE_STEP * step as f32).clamp(DAMAGE_RANGE.0, DAMAGE_RANGE.1);
    }

    /// The magic multiplier one step up or down, kept in range.
    pub(crate) fn step_magic(&mut self, step: i32) {
        self.magic = (self.magic + DAMAGE_STEP * step as f32).clamp(DAMAGE_RANGE.0, DAMAGE_RANGE.1);
    }

    /// The next (or previous) hit cap in the table.
    pub(crate) fn cycle_cap(&mut self, step: i32) {
        let at = HIT_CAPS
            .iter()
            .position(|&c| c == self.hit_cap)
            .unwrap_or(4) as i32;
        let n = HIT_CAPS.len() as i32;
        self.hit_cap = HIT_CAPS[(at + step).rem_euclid(n) as usize];
    }

    /// The next (or previous) arena.
    pub(crate) fn cycle_arena(&mut self, step: i32) {
        let n = super::duel_arena::ARENAS.len() as i32;
        self.arena = (i32::from(self.arena) + step).rem_euclid(n) as u8;
    }

    /// The other ruleset.
    pub(crate) fn cycle_ruleset(&mut self, step: i32) {
        let at = self.ruleset_code() as i32;
        let n = RULESETS.len() as i32;
        self.ruleset = RULESETS[(at + step).rem_euclid(n) as usize].0;
    }

    /// The ruleset's wire code and back.
    pub(crate) fn ruleset_code(&self) -> u8 {
        RULESETS
            .iter()
            .position(|r| r.0 == self.ruleset)
            .unwrap_or(0) as u8
    }

    pub(crate) fn ruleset_from(code: u8) -> Option<Ruleset> {
        RULESETS.get(code as usize).map(|r| r.0)
    }

    /// The ruleset's lobby name.
    pub(crate) fn ruleset_name(&self) -> &'static str {
        RULESETS[self.ruleset_code() as usize].1
    }

    /// KH3's moves are on.
    pub(crate) fn kh3(&self) -> bool {
        self.ruleset == Ruleset::Kh3
    }

    /// The next (or previous) time limit in the table.
    pub(crate) fn cycle_time(&mut self, step: i32) {
        let at = TIME_LIMITS
            .iter()
            .position(|&t| t == self.time_limit)
            .unwrap_or(3) as i32;
        let n = TIME_LIMITS.len() as i32;
        self.time_limit = TIME_LIMITS[(at + step).rem_euclid(n) as usize];
    }

    /// Whether every field is one the lobby can choose (the joiner's check
    /// of what arrives).
    pub(crate) fn valid(&self) -> bool {
        let in_range = |k: f32| k.is_finite() && (DAMAGE_RANGE.0..=DAMAGE_RANGE.1).contains(&k);
        in_range(self.damage)
            && in_range(self.magic)
            && HIT_CAPS.contains(&self.hit_cap)
            && usize::from(self.arena) < super::duel_arena::ARENAS.len()
            && TIME_LIMITS.contains(&self.time_limit)
    }

    /// The mode's wire code and back.
    pub(crate) fn mode_code(&self) -> u8 {
        MODES.iter().position(|m| m.mode == self.mode).unwrap_or(0) as u8
    }

    pub(crate) fn mode_from(code: u8) -> Option<Mode> {
        MODES.get(code as usize).map(|m| m.mode)
    }
}

/// `m:ss` for a count of ticks (60 a second), rounded up to the second.
pub(crate) fn clock(ticks: u32) -> String {
    let s = ticks.div_ceil(60);
    format!("{}:{:02}", s / 60, s % 60)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_damage_rule_scales_and_rounds() {
        let rules = |damage| MatchRules {
            damage,
            hit_cap: 100,
            ..MatchRules::default()
        };
        assert_eq!(rules(2.5).damage(2, false, 170), 5);
        assert_eq!(rules(2.5).damage(3, false, 170), 8);
        assert_eq!(rules(2.5).damage(0, false, 170), 0);
        assert_eq!(
            rules(0.5).damage(1, false, 170),
            1,
            "a hit that did damage still does some"
        );
        assert_eq!(rules(1.).damage(7, false, 170), 7);
    }

    #[test]
    fn magic_has_its_own_multiplier_and_every_hit_its_cap() {
        let r = MatchRules {
            damage: 3.,
            magic: 1.5,
            hit_cap: 40,
            ..MatchRules::default()
        };
        assert_eq!(r.damage(8, false, 170), 24);
        assert_eq!(r.damage(8, true, 170), 12);
        // 40 % of 170 is 68: a Thundaga bolt of 30 on a high-Magic Keyblade
        // at x3 (90) stops there.
        assert_eq!(r.damage(30, true, 170), 45);
        assert_eq!(MatchRules { magic: 3., ..r }.damage(30, true, 170), 68);
        assert_eq!(
            MatchRules {
                hit_cap: 100,
                magic: 3.,
                ..r
            }
            .damage(30, true, 170),
            90
        );
    }

    #[test]
    fn the_host_cycles_modes_damage_and_time_within_their_tables() {
        let mut r = MatchRules::default();
        assert_eq!(r.info().name, "One KO");
        r.cycle_mode(1);
        assert_eq!(r.mode, Mode::BestOf3);
        r.cycle_mode(-2);
        assert_eq!(r.mode, Mode::Timed);
        r.damage = DAMAGE_RANGE.1;
        r.step_damage(1);
        assert_eq!(r.damage, DAMAGE_RANGE.1);
        r.step_damage(-1);
        assert_eq!(r.damage, DAMAGE_RANGE.1 - DAMAGE_STEP);
        r.step_magic(-100);
        assert_eq!(r.magic, DAMAGE_RANGE.0);
        r.cycle_cap(-5);
        assert_eq!(r.hit_cap, 100);
        r.cycle_cap(1);
        assert_eq!(r.hit_cap, 20);
        assert!(!MatchRules { hit_cap: 41, ..r }.valid());
        r.cycle_time(1);
        assert_eq!(r.time_limit, 300);
        r.cycle_time(1);
        assert_eq!(r.time_limit, 60);
        assert!(r.valid());
        assert_eq!(MatchRules::mode_from(r.mode_code()), Some(r.mode));
        assert!(!MatchRules { time_limit: 7, ..r }.valid());
        assert_eq!(mode_named("best3"), Some(Mode::BestOf3));
        assert!(!r.kh3());
        r.cycle_ruleset(1);
        assert!(r.kh3() && r.ruleset_name() == "KH3");
        assert_eq!(
            MatchRules::ruleset_from(r.ruleset_code()),
            Some(Ruleset::Kh3)
        );
        r.cycle_ruleset(1);
        assert_eq!(r.ruleset, Ruleset::Kh2);
        assert_eq!(ruleset_named("kh3"), Some(Ruleset::Kh3));
        assert_eq!(ruleset_named("kh1"), None);
        assert_eq!(mode_named("rounds"), None);
    }

    #[test]
    fn clocks_round_up_to_the_second() {
        assert_eq!(clock(180 * 60), "3:00");
        assert_eq!(clock(61), "0:02");
        assert_eq!(clock(0), "0:00");
    }
}
