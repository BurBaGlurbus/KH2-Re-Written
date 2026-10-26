//! The duel's match rules. The host chooses them in the lobby (and with
//! `--duel-damage` / F3); the joiner receives them and shows them read-only.
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

/// The time limits the host can pick, in seconds.
pub(crate) const TIME_LIMITS: [u16; 5] = [60, 90, 120, 180, 300];
const DEFAULT_TIME: u16 = 180;

/// The default damage rule [duel choice]. Measured with the scripted rival
/// swinging the Kingdom Key at an idle Sora at level 99 (KH2RW_RIVAL_BENCH,
/// T241): 0.46 hits landed a second at 2 damage, so 170 HP lasts about
/// 185 s at x1. x3 makes that about a minute, and 30-45 s for players who
/// land more of their combos.
pub(crate) const DEFAULT_DAMAGE: f32 = 3.;
/// The lobby's and F3 menu's step and the range they allow.
pub(crate) const DAMAGE_STEP: f32 = 0.5;
pub(crate) const DAMAGE_RANGE: (f32, f32) = (0.5, 10.);

/// The duel's rules (the host's; the joiner shows them).
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct MatchRules {
    pub mode: Mode,
    /// What every Sora-against-Sora hit's damage is multiplied by, after
    /// the game's own formula (attacks, magic and forms alike).
    pub damage: f32,
    /// Seconds, for the timed mode.
    pub time_limit: u16,
}

impl Default for MatchRules {
    fn default() -> Self {
        MatchRules {
            mode: Mode::Stock,
            damage: DEFAULT_DAMAGE,
            time_limit: DEFAULT_TIME,
        }
    }
}

impl MatchRules {
    /// A hit's damage under the rules: the formula's number times the
    /// multiplier, rounded; a hit that did damage still does at least 1.
    pub(crate) fn damage(&self, formula: i32) -> i32 {
        if formula <= 0 {
            return formula;
        }
        ((formula as f32 * self.damage).round() as i32).max(1)
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
        self.damage.is_finite()
            && (DAMAGE_RANGE.0..=DAMAGE_RANGE.1).contains(&self.damage)
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
            ..MatchRules::default()
        };
        assert_eq!(rules(2.5).damage(2), 5);
        assert_eq!(rules(2.5).damage(3), 8);
        assert_eq!(rules(2.5).damage(0), 0);
        assert_eq!(
            rules(0.5).damage(1),
            1,
            "a hit that did damage still does some"
        );
        assert_eq!(rules(1.).damage(7), 7);
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
        r.cycle_time(1);
        assert_eq!(r.time_limit, 300);
        r.cycle_time(1);
        assert_eq!(r.time_limit, 60);
        assert!(r.valid());
        assert_eq!(MatchRules::mode_from(r.mode_code()), Some(r.mode));
        assert!(!MatchRules { time_limit: 7, ..r }.valid());
        assert_eq!(mode_named("best3"), Some(Mode::BestOf3));
        assert_eq!(mode_named("rounds"), None);
    }

    #[test]
    fn clocks_round_up_to_the_second() {
        assert_eq!(clock(180 * 60), "3:00");
        assert_eq!(clock(61), "0:02");
        assert_eq!(clock(0), "0:00");
    }
}
