//! The duel's match: the countdown, the fight, each round's end and the
//! result. The host steps it on its tick and sends it on every change; both
//! screens draw it from host ticks (the joiner from the snapshot it shows),
//! so "GO" appears as the Soras are let go. Everything shown is worked out
//! from the time since a phase began; nothing is spawned for it.

use super::duel_rules::{MatchRules, Mode};
use net::wire::{Reader, Writer};

/// Ticks each of 3, 2, 1 is up; the fight starts after them.
pub(crate) const COUNT_STEP: u64 = 60;
pub(crate) const COUNTDOWN: u64 = 3 * COUNT_STEP;
/// Ticks "GO" stays up once the fight has started.
pub(crate) const GO_SHOWN: u64 = 45;
/// Ticks a round's result stays up before the next round or the result.
pub(crate) const ROUND_OVER: u64 = 180;
/// Rounds a Best of 3 can go to when rounds are drawn.
const MAX_ROUNDS: u8 = 5;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Phase {
    Countdown,
    Fight,
    RoundOver,
    Over,
}

/// How a round (or the match) was decided.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Why {
    KnockOut,
    /// The clock ran out: the larger share of HP left won.
    Time,
    /// The other player left.
    Left,
    /// The other player forfeited [duel choice: it ends the match, not the
    /// round].
    Forfeit,
}

/// The match (the host's; the joiner shows the copy it receives).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Match {
    pub phase: Phase,
    /// The host tick the phase began, and the match.
    pub since: u64,
    pub began: u64,
    pub round: u8,
    /// Rounds won by seat 0 (host) and seat 1 (joiner).
    pub wins: [u8; 2],
    /// The last round's winner, then the match's: Some(seat), None a draw.
    pub winner: Option<u8>,
    pub why: Option<Why>,
    /// Timed: the host tick the clock runs out.
    pub clock_end: Option<u64>,
    /// Damage each seat dealt and hits each landed this match.
    pub dealt: [u32; 2],
    pub hits: [u16; 2],
    /// Each seat asked for a rematch on the result screen, and whether a
    /// quick one (the same builds and bans, no lobby).
    pub rematch: [bool; 2],
    pub quick: [bool; 2],
}

/// What the host sees of the fight this tick.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct View {
    pub hp: [i32; 2],
    pub max_hp: [i32; 2],
    /// The seat's player is still in the session.
    pub here: [bool; 2],
}

/// What a step started, for the room to act on (sounds, resets).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Cue {
    /// 3, 2 or 1 came up.
    Count(u8),
    Go,
    /// A round ended (the match's `winner` and `why` say how).
    RoundOver,
    /// The next round starts: the Soras are reset and placed.
    NextRound,
    /// The match is decided.
    Over,
}

/// Round wins that take a match under the rules.
pub(crate) fn wins_needed(rules: &MatchRules) -> u8 {
    match rules.mode {
        Mode::BestOf3 => 2,
        Mode::Stock | Mode::Timed => 1,
    }
}

/// The count showing `elapsed` ticks into the countdown (3, 2, 1), or None
/// once it has run.
pub(crate) fn count_at(elapsed: u64) -> Option<u8> {
    (elapsed < COUNTDOWN).then(|| 3 - (elapsed / COUNT_STEP) as u8)
}

impl Match {
    pub(crate) fn new(now: u64) -> Self {
        Match {
            phase: Phase::Countdown,
            since: now,
            began: now,
            round: 1,
            wins: [0; 2],
            winner: None,
            why: None,
            clock_end: None,
            dealt: [0; 2],
            hits: [0; 2],
            rematch: [false; 2],
            quick: [false; 2],
        }
    }

    /// The Soras may move and fight.
    pub(crate) fn fighting(&self) -> bool {
        self.phase == Phase::Fight
    }

    fn enter(&mut self, phase: Phase, now: u64) {
        self.phase = phase;
        self.since = now;
    }

    /// Host: `seat` gives up the match; the other takes it. False when it
    /// was already decided.
    pub(crate) fn forfeit(&mut self, seat: usize, now: u64) -> bool {
        if self.phase == Phase::Over || seat > 1 {
            return false;
        }
        self.winner = Some(1 - seat as u8);
        self.why = Some(Why::Forfeit);
        self.enter(Phase::Over, now);
        true
    }

    /// Host: one tick of the match.
    pub(crate) fn step(&mut self, now: u64, rules: &MatchRules, v: &View) -> Vec<Cue> {
        let mut cues = Vec::new();
        let elapsed = now.saturating_sub(self.since);
        // A player who leaves loses: the one still here wins.
        if self.phase != Phase::Over
            && let Some(gone) = (0..2).find(|&s| !v.here[s])
            && v.here[1 - gone]
        {
            self.winner = Some(1 - gone as u8);
            self.why = Some(Why::Left);
            self.enter(Phase::Over, now);
            cues.push(Cue::Over);
            return cues;
        }
        match self.phase {
            Phase::Countdown => {
                if elapsed.is_multiple_of(COUNT_STEP)
                    && let Some(n) = count_at(elapsed)
                {
                    cues.push(Cue::Count(n));
                }
                if elapsed >= COUNTDOWN {
                    self.enter(Phase::Fight, now);
                    self.clock_end = rules
                        .info()
                        .timed
                        .then(|| now + u64::from(rules.time_limit) * 60);
                    cues.push(Cue::Go);
                }
            }
            Phase::Fight => {
                let down = [v.hp[0] <= 0, v.hp[1] <= 0];
                let decided = if down[0] || down[1] {
                    Some((
                        match down {
                            [true, true] => None,
                            [true, false] => Some(1),
                            _ => Some(0),
                        },
                        Why::KnockOut,
                    ))
                } else if self.clock_end.is_some_and(|end| now >= end) {
                    // The larger share of HP left (equal shares draw).
                    let a = i64::from(v.hp[0]) * i64::from(v.max_hp[1].max(1));
                    let b = i64::from(v.hp[1]) * i64::from(v.max_hp[0].max(1));
                    Some((
                        match a.cmp(&b) {
                            std::cmp::Ordering::Greater => Some(0),
                            std::cmp::Ordering::Less => Some(1),
                            std::cmp::Ordering::Equal => None,
                        },
                        Why::Time,
                    ))
                } else {
                    None
                };
                if let Some((winner, why)) = decided {
                    self.winner = winner;
                    self.why = Some(why);
                    if let Some(w) = winner {
                        self.wins[usize::from(w)] += 1;
                    }
                    self.enter(Phase::RoundOver, now);
                    cues.push(Cue::RoundOver);
                }
            }
            Phase::RoundOver if elapsed >= ROUND_OVER => {
                let need = wins_needed(rules);
                let taken = self.wins.iter().position(|&w| w >= need);
                if taken.is_some() || need == 1 || self.round >= MAX_ROUNDS {
                    self.winner = match taken {
                        Some(s) => Some(s as u8),
                        None => match self.wins[0].cmp(&self.wins[1]) {
                            std::cmp::Ordering::Greater => Some(0),
                            std::cmp::Ordering::Less => Some(1),
                            std::cmp::Ordering::Equal => None,
                        },
                    };
                    self.enter(Phase::Over, now);
                    cues.push(Cue::Over);
                } else {
                    self.round += 1;
                    self.winner = None;
                    self.why = None;
                    self.clock_end = None;
                    self.enter(Phase::Countdown, now);
                    cues.push(Cue::NextRound);
                }
            }
            Phase::RoundOver | Phase::Over => {}
        }
        cues
    }
}

fn why_code(w: Option<Why>) -> u8 {
    match w {
        None => 0,
        Some(Why::KnockOut) => 1,
        Some(Why::Time) => 2,
        Some(Why::Left) => 3,
        Some(Why::Forfeit) => 4,
    }
}

pub(crate) fn encode_match(kind: u8, m: &Match) -> Vec<u8> {
    let mut w = Writer::default();
    w.u8(kind);
    w.u8(m.phase as u8);
    w.u64(m.since);
    w.u64(m.began);
    w.u8(m.round);
    w.bytes(&m.wins);
    w.u8(m.winner.unwrap_or(u8::MAX));
    w.u8(why_code(m.why));
    w.u64(m.clock_end.unwrap_or(u64::MAX));
    for d in m.dealt {
        w.u32(d);
    }
    for h in m.hits {
        w.u16(h);
    }
    w.u8(u8::from(m.rematch[0])
        | u8::from(m.rematch[1]) << 1
        | u8::from(m.quick[0]) << 2
        | u8::from(m.quick[1]) << 3);
    w.finish()
}

pub(crate) fn decode_match(kind: u8, bytes: &[u8]) -> Option<Match> {
    let mut r = Reader::new(bytes);
    if r.u8()? != kind {
        return None;
    }
    let phase = match r.u8()? {
        0 => Phase::Countdown,
        1 => Phase::Fight,
        2 => Phase::RoundOver,
        3 => Phase::Over,
        _ => return None,
    };
    let since = r.u64()?;
    let began = r.u64()?;
    let round = r.u8()?;
    let wins = r.array()?;
    let winner = match r.u8()? {
        u8::MAX => None,
        s @ (0 | 1) => Some(s),
        _ => return None,
    };
    let why = match r.u8()? {
        0 => None,
        1 => Some(Why::KnockOut),
        2 => Some(Why::Time),
        3 => Some(Why::Left),
        4 => Some(Why::Forfeit),
        _ => return None,
    };
    let clock_end = match r.u64()? {
        u64::MAX => None,
        t => Some(t),
    };
    let dealt = [r.u32()?, r.u32()?];
    let hits = [r.u16()?, r.u16()?];
    let flags = r.u8()?;
    r.is_empty().then_some(Match {
        phase,
        since,
        began,
        round,
        wins,
        winner,
        why,
        clock_end,
        dealt,
        hits,
        rematch: [flags & 1 != 0, flags & 2 != 0],
        quick: [flags & 4 != 0, flags & 8 != 0],
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rules(mode: Mode) -> MatchRules {
        MatchRules {
            mode,
            time_limit: 60,
            ..MatchRules::default()
        }
    }

    fn view(hp: [i32; 2]) -> View {
        View {
            hp,
            max_hp: [170, 170],
            here: [true, true],
        }
    }

    /// Steps the match from `from` to `to` (inclusive) at a fixed view.
    fn run(m: &mut Match, r: &MatchRules, from: u64, to: u64, v: View) -> Vec<(u64, Cue)> {
        (from..=to)
            .flat_map(|t| m.step(t, r, &v).into_iter().map(move |c| (t, c)))
            .collect()
    }

    #[test]
    fn the_countdown_counts_three_two_one_then_go() {
        let r = rules(Mode::Stock);
        let mut m = Match::new(100);
        let cues = run(&mut m, &r, 100, 100 + COUNTDOWN, view([170, 170]));
        assert_eq!(
            cues,
            vec![
                (100, Cue::Count(3)),
                (160, Cue::Count(2)),
                (220, Cue::Count(1)),
                (280, Cue::Go)
            ]
        );
        assert!(m.fighting());
        assert_eq!(m.clock_end, None, "One KO has no clock");
        assert_eq!(count_at(0), Some(3));
        assert_eq!(count_at(COUNTDOWN), None);
    }

    #[test]
    fn a_knock_out_ends_a_one_ko_match() {
        let r = rules(Mode::Stock);
        let mut m = Match::new(0);
        run(&mut m, &r, 0, COUNTDOWN, view([170, 170]));
        let t = COUNTDOWN + 50;
        assert_eq!(m.step(t, &r, &view([0, 40])), vec![Cue::RoundOver]);
        assert_eq!(
            (m.winner, m.why, m.wins),
            (Some(1), Some(Why::KnockOut), [0, 1])
        );
        assert!(m.step(t + ROUND_OVER - 1, &r, &view([0, 40])).is_empty());
        assert_eq!(m.step(t + ROUND_OVER, &r, &view([0, 40])), vec![Cue::Over]);
        assert_eq!(m.winner, Some(1));
    }

    #[test]
    fn best_of_three_plays_rounds_until_two_wins() {
        let r = rules(Mode::BestOf3);
        let mut m = Match::new(0);
        let mut now = 0;
        for (round, loser) in [(1, 0), (2, 1), (3, 0)] {
            assert_eq!(m.round, round);
            run(&mut m, &r, now, now + COUNTDOWN, view([170, 170]));
            now += COUNTDOWN + 10;
            let mut hp = [170, 170];
            hp[loser] = 0;
            m.step(now, &r, &view(hp));
            now += ROUND_OVER;
            let cues = m.step(now, &r, &view(hp));
            now += 1;
            if round < 3 {
                assert_eq!(cues, vec![Cue::NextRound]);
            } else {
                assert_eq!(cues, vec![Cue::Over]);
            }
        }
        assert_eq!((m.wins, m.winner), ([1, 2], Some(1)));
    }

    #[test]
    fn time_up_goes_to_the_larger_share_of_hp_and_equal_shares_draw() {
        let r = rules(Mode::Timed);
        let mut m = Match::new(0);
        run(&mut m, &r, 0, COUNTDOWN, view([170, 170]));
        let end = m.clock_end.unwrap();
        assert_eq!(end, COUNTDOWN + 60 * 60);
        assert!(m.step(end - 1, &r, &view([90, 100])).is_empty());
        assert_eq!(m.step(end, &r, &view([90, 100])), vec![Cue::RoundOver]);
        assert_eq!((m.winner, m.why), (Some(1), Some(Why::Time)));
        let mut d = Match::new(0);
        run(&mut d, &r, 0, COUNTDOWN, view([170, 170]));
        d.step(d.clock_end.unwrap(), &r, &view([85, 85]));
        assert_eq!(d.winner, None, "equal shares");
        d.step(d.since + ROUND_OVER, &r, &view([85, 85]));
        assert_eq!((d.phase, d.winner), (Phase::Over, None));
    }

    #[test]
    fn leaving_mid_match_is_a_win_for_the_one_still_here() {
        let r = rules(Mode::BestOf3);
        let mut m = Match::new(0);
        run(&mut m, &r, 0, COUNTDOWN + 5, view([170, 170]));
        let mut v = view([170, 20]);
        v.here[0] = false;
        assert_eq!(m.step(COUNTDOWN + 6, &r, &v), vec![Cue::Over]);
        assert_eq!((m.winner, m.why), (Some(1), Some(Why::Left)));
    }

    #[test]
    fn a_forfeit_ends_the_match_once() {
        let r = rules(Mode::BestOf3);
        let mut m = Match::new(0);
        run(&mut m, &r, 0, COUNTDOWN + 5, view([170, 170]));
        assert!(m.forfeit(1, COUNTDOWN + 6));
        assert_eq!(
            (m.phase, m.winner, m.why),
            (Phase::Over, Some(0), Some(Why::Forfeit))
        );
        assert!(!m.forfeit(0, COUNTDOWN + 7));
        assert_eq!(m.winner, Some(0));
        assert_eq!(decode_match(10, &encode_match(10, &m)), Some(m));
    }

    #[test]
    fn matches_round_trip() {
        let mut m = Match::new(42);
        m.phase = Phase::RoundOver;
        m.wins = [1, 0];
        m.winner = Some(0);
        m.why = Some(Why::Time);
        m.clock_end = Some(9000);
        m.dealt = [120, 33];
        m.hits = [7, 2];
        m.rematch = [true, false];
        m.quick = [true, false];
        let bytes = encode_match(10, &m);
        assert_eq!(decode_match(10, &bytes), Some(m));
        assert_eq!(decode_match(10, &bytes[..bytes.len() - 1]), None);
        assert_eq!(
            decode_match(10, &encode_match(10, &Match::new(1))),
            Some(Match::new(1))
        );
    }
}
