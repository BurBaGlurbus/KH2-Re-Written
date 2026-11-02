//! What a flight counts and what its result writes: the rank against
//! rank.bin (FUN_140207400), the medal value (FUN_140207c30), the score
//! (FUN_140207a50), the rank prizes (FUN_1402082e0), the treasure share
//! (FUN_1402083d0), the unlocks (FUN_140209b50) and the record write
//! (FUN_140209750). At the result (FUN_140210730 state 4) the unlocks run
//! before the record is written, so "never cleared" reads the save as it
//! was before the flight.
use crate::record::{Mission, RECORD_COUNT, record_index};

/// The highest rank: 6 on levels 1 and 2, 16 on level 3.
pub fn max_rank(level: u8) -> u8 {
    if level < 3 { 6 } else { 16 }
}

/// The value a rank.bin row's kind ranks: 0 the medal value, 1 ships
/// destroyed (result +0x06), 2 the score.
pub fn rank_value(kind: u8, medals: i32, destroyed: u16, score: i32) -> i32 {
    match kind {
        0 => medals,
        1 => i32::from(destroyed),
        2 => score,
        _ => 0,
    }
}

/// One frame of ranking: at most one step up, when the value reaches the
/// threshold of the current rank. The exe ranks only before the goal and
/// not on a route's first level 1 flight; the caller checks both. True
/// when the rank went up (the HUD shows it, and result +0x1c is set).
pub fn rank_step(rank: &mut u8, level: u8, value: i32, thresholds: &[i32; 16]) -> bool {
    if *rank < max_rank(level) && thresholds[*rank as usize] <= value {
        *rank += 1;
        true
    } else {
        false
    }
}

/// The threshold the result panel shows for a saved rank (FUN_140207660).
pub fn shown_threshold(saved_rank: u8, thresholds: &[i32; 16]) -> i32 {
    thresholds[saved_rank.min(15) as usize]
}

/// The medal counter: every tenth pickup raises the value, up to 30.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Medals {
    /// Pickups towards the next value (DAT_14072b2c0).
    pub count: i32,
    /// DAT_14072b2c4.
    pub value: i32,
    /// The best value this flight (DAT_14072b2c8), kept in result +0x1a.
    pub best: i32,
    /// At 30 nothing more counts (DAT_14072b2cc bit 0).
    pub full: bool,
}

impl Medals {
    /// One medal picked up. The first makes the value 1; the sound of a
    /// value step comes back.
    pub fn pick(&mut self) -> Option<u32> {
        if self.full {
            return None;
        }
        if self.value < 1 && self.count < 1 {
            self.value = 1;
        }
        self.count += 1;
        let mut sound = None;
        if self.count > 9 {
            self.count = 0;
            sound = Some(match self.value {
                4 | 9 | 14 | 19 | 24 => 0xf36b2,
                25 => 0xf36b3,
                26 => 0xf36b4,
                27 => 0xf36b5,
                28 => 0xf36b6,
                29 => 0xf36b7,
                _ => 0xf36b1,
            });
            self.value += 1;
        }
        if self.value > 29 {
            self.full = true;
            self.value = 30;
            self.count = 9;
        }
        self.best = self.best.max(self.value);
        sound
    }
}

/// The score's ceiling: past it the score keeps only its last digit.
pub const SCORE_CAP: i32 = 99_999_990;

/// Points added (FUN_140207a50; the caller skips this while the score is
/// frozen).
pub fn add_score(score: &mut i32, points: i32) {
    *score += points;
    if *score > SCORE_CAP {
        *score = *score % 10 + SCORE_CAP;
    }
}

/// FUN_140207a90: the last digit goes up by one, to 9 at most.
pub fn bump_last_digit(score: &mut i32) {
    if *score % 10 != 9 {
        *score += 1;
    }
}

/// FUN_140209980: a level 1 record never cleared. No rank is kept and
/// only the first prize is given on that flight; its clear returns
/// straight to the world map.
pub fn first_flight(record: &Mission) -> bool {
    record.level == 1 && !record.cleared
}

/// The rank prizes' slots in the order the exe looks them up
/// (DAT_1405b5550): the nth prize is slot `PRIZE_SLOTS[n]`.
pub const PRIZE_SLOTS: [u8; 7] = [6, 5, 4, 3, 2, 1, 0];

/// The slots whose prizes a flight that reached the goal gives (each
/// once): ranks 0..=rank, or only the first on a first flight.
pub fn prize_slots(rank: u8, first: bool) -> Vec<u8> {
    if first {
        vec![PRIZE_SLOTS[0]]
    } else {
        PRIZE_SLOTS[..=(rank as usize).min(6)].to_vec()
    }
}

/// The items a prize gives (FUN_1402081e0): item 0x62 brings 0x6a..=0x6d.
pub fn prize_items(item: u32) -> Vec<u32> {
    let mut items = vec![item];
    if item == 0x62 {
        items.extend(0x6a..=0x6d);
    }
    items
}

/// The treasure.bin level byte of a flight: EX adds 100.
pub fn table_level(level: u8, ex: bool) -> u8 {
    if ex { level + 100 } else { level }
}

/// FUN_1402083d0: of a route and level's treasures, normal and EX
/// together, the share obtained, 0..=100. `entries` gives each treasure.bin
/// row's (route, level byte, obtained bit).
pub fn treasure_share(
    entries: impl IntoIterator<Item = (u8, u8, u32)>,
    route: u8,
    level: u8,
    obtained: impl Fn(u32) -> bool,
) -> f32 {
    let (mut all, mut owned) = (0, 0);
    for (r, l, bit) in entries {
        if r == route && (l == level || u32::from(l) == u32::from(level) + 100) {
            all += 1;
            if obtained(bit) {
                owned += 1;
            }
        }
    }
    owned as f32 / all as f32 * 100.
}

/// The notices the unlocks raise (DAT_140af7210..).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Unlocked {
    /// Level 1's first clear opened levels 1 and 2.
    pub first: bool,
    /// Level 3 opened.
    pub third: bool,
    /// The EX level of each level opened.
    pub ex: [bool; 3],
}

/// FUN_140209b50 over the 54 records, for the flight of `route` 0..=8,
/// `level`, `ex` whose result block is `run`.
pub fn unlock(
    records: &mut [Mission; RECORD_COUNT],
    route: u8,
    level: u8,
    ex: bool,
    run: &Mission,
) -> Unlocked {
    let mut out = Unlocked::default();
    if first_flight(&records[record_index(route, level, ex)]) {
        for l in [1, 2] {
            records[record_index(route, l, false)].unlocked = true;
        }
        out.first = true;
        return out;
    }
    if !ex && run.rank > 5 {
        let r = &mut records[record_index(route, level, true)];
        if !r.unlocked {
            r.unlocked = true;
            out.ex[level as usize - 1] = true;
        }
    }
    // This flight's rank stands for its own level's record.
    let rank = |l: u8| {
        if level == l {
            run.rank
        } else {
            records[record_index(route, l, false)].rank
        }
    };
    if rank(1) > 2 && rank(2) > 2 {
        let r = &mut records[record_index(route, 3, false)];
        if !r.unlocked {
            r.unlocked = true;
            out.third = true;
        }
    }
    out
}

/// FUN_140209750 on the result screen's decide: the flight's record is
/// cleared and keeps the best of each value, and the treasure share goes
/// to it and to its other-kind twin.
pub fn write_clear(
    records: &mut [Mission; RECORD_COUNT],
    route: u8,
    level: u8,
    ex: bool,
    run: &Mission,
    share: f32,
) {
    let r = &mut records[record_index(route, level, ex)];
    r.cleared = true;
    r.score = r.score.max(run.score);
    for (saved, &new) in r.counters.iter_mut().zip(&run.counters) {
        *saved = (*saved).max(new);
    }
    r.medals = r.medals.max(run.medals);
    r.rank = r.rank.max(run.rank);
    if run.rank_up {
        r.rank_up = true;
    }
    r.treasure = share;
    records[record_index(route, level, !ex)].treasure = share;
}

#[cfg(test)]
mod tests {
    use super::*;

    const LEVEL1: [i32; 16] = [10, 14, 18, 22, 26, 30, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0];

    #[test]
    fn rank_steps_once_a_frame_to_the_cap() {
        let mut rank = 0;
        // 30 medals reach every threshold, one step at a time.
        let mut steps = 0;
        while rank_step(&mut rank, 1, 30, &LEVEL1) {
            steps += 1;
        }
        assert_eq!((rank, steps), (6, 6));
        let mut rank = 0;
        assert!(!rank_step(&mut rank, 1, 9, &LEVEL1));
        assert!(rank_step(&mut rank, 1, 10, &LEVEL1));
        assert!(!rank_step(&mut rank, 1, 13, &LEVEL1));
        // Level 3 goes to 16; zero thresholds past a row's end pass.
        let mut rank = 6;
        assert!(rank_step(&mut rank, 3, 0, &LEVEL1));
        assert_eq!(max_rank(3), 16);
        assert_eq!(shown_threshold(20, &LEVEL1), 0);
        assert_eq!(shown_threshold(2, &LEVEL1), 18);
        assert_eq!(rank_value(1, 5, 7, 9), 7);
    }

    #[test]
    fn medal_value_every_ten_pickups() {
        let mut m = Medals::default();
        assert_eq!(m.pick(), None);
        assert_eq!((m.value, m.count), (1, 1));
        for _ in 0..8 {
            assert_eq!(m.pick(), None);
        }
        assert_eq!(m.pick(), Some(0xf36b1));
        assert_eq!((m.value, m.count, m.best), (2, 0, 2));
        let mut sounds = Vec::new();
        for _ in 0..400 {
            sounds.extend(m.pick());
        }
        assert!(m.full);
        assert_eq!((m.value, m.count, m.best), (30, 9, 30));
        assert_eq!(sounds.iter().filter(|&&s| s == 0xf36b2).count(), 5);
        assert_eq!(
            &sounds[sounds.len() - 5..],
            &[0xf36b3, 0xf36b4, 0xf36b5, 0xf36b6, 0xf36b7]
        );
        assert_eq!(m.pick(), None);
    }

    #[test]
    fn score_cap_and_last_digit() {
        let mut s = 99_999_985;
        add_score(&mut s, 23);
        assert_eq!(s, 99_999_998);
        let mut s = 12;
        bump_last_digit(&mut s);
        assert_eq!(s, 13);
        let mut s = 19;
        bump_last_digit(&mut s);
        assert_eq!(s, 19);
    }

    #[test]
    fn prizes_and_share() {
        assert_eq!(prize_slots(0, false), vec![6]);
        assert_eq!(prize_slots(3, false), vec![6, 5, 4, 3]);
        assert_eq!(prize_slots(16, false), PRIZE_SLOTS.to_vec());
        assert_eq!(prize_slots(16, true), vec![6]);
        assert_eq!(prize_items(0x62), vec![0x62, 0x6a, 0x6b, 0x6c, 0x6d]);
        assert_eq!(table_level(2, true), 102);
        // Route 0 level 1: three normal, one EX, one of another level.
        let rows = [
            (0, 1, 0),
            (0, 1, 1),
            (0, 101, 2),
            (0, 2, 3),
            (1, 1, 4),
            (0, 1, 5),
        ];
        let share = treasure_share(rows, 0, 1, |b| b == 1 || b == 2 || b == 3);
        assert_eq!(share, 50.);
    }

    fn records() -> Box<[Mission; RECORD_COUNT]> {
        let mut r = Box::new([Mission::default(); RECORD_COUNT]);
        for (i, m) in r.iter_mut().enumerate() {
            m.route = (i % 27 / 3) as u8;
            m.level = (i % 3) as u8 + 1;
        }
        r
    }

    #[test]
    fn first_clear_opens_levels_one_and_two_only() {
        let mut r = records();
        let run = Mission {
            rank: 6,
            ..Mission::default()
        };
        let u = unlock(&mut r, 4, 1, false, &run);
        assert_eq!(
            u,
            Unlocked {
                first: true,
                ..Unlocked::default()
            }
        );
        assert!(r[record_index(4, 1, false)].unlocked && r[record_index(4, 2, false)].unlocked);
        assert!(!r[record_index(4, 1, true)].unlocked);
        write_clear(&mut r, 4, 1, false, &run, 25.);
        let m = r[record_index(4, 1, false)];
        assert!(m.cleared);
        assert_eq!((m.rank, m.treasure), (6, 25.));
        assert_eq!(r[record_index(4, 1, true)].treasure, 25.);
        assert!(!first_flight(&m));
    }

    #[test]
    fn later_clears_open_ex_and_level_three() {
        let mut r = records();
        for l in [1, 2] {
            let m = &mut r[record_index(0, l, false)];
            m.cleared = true;
            m.unlocked = true;
        }
        r[record_index(0, 1, false)].rank = 3;
        // Level 2 at rank 6: its EX opens, and with level 1 above 2 so does
        // level 3, the flight's rank standing for level 2's record.
        let run = Mission {
            rank: 6,
            ..Mission::default()
        };
        let u = unlock(&mut r, 0, 2, false, &run);
        assert_eq!(
            u,
            Unlocked {
                third: true,
                ex: [false, true, false],
                ..Unlocked::default()
            }
        );
        // Again: nothing new.
        assert_eq!(unlock(&mut r, 0, 2, false, &run), Unlocked::default());
        // An EX flight opens no EX, and rank 5 opens none either.
        let mut r = records();
        r[record_index(1, 2, false)].cleared = true;
        let run = Mission {
            rank: 5,
            ..Mission::default()
        };
        assert_eq!(unlock(&mut r, 1, 2, false, &run), Unlocked::default());
        let run = Mission {
            rank: 6,
            ..Mission::default()
        };
        r[record_index(1, 2, true)].cleared = true;
        assert_eq!(unlock(&mut r, 1, 2, true, &run), Unlocked::default());
    }

    #[test]
    fn clear_keeps_the_best() {
        let mut r = records();
        let at = record_index(2, 3, true);
        r[at].score = 500;
        r[at].counters = [9; 10];
        r[at].rank = 10;
        let mut run = Mission {
            score: 400,
            medals: 4,
            rank: 12,
            rank_up: true,
            ..Mission::default()
        };
        run.counters[1] = 20;
        write_clear(&mut r, 2, 3, true, &run, 80.);
        let m = r[at];
        assert_eq!((m.score, m.counters[1], m.counters[0]), (500, 20, 9));
        assert_eq!(
            (m.medals, m.rank, m.rank_up, m.cleared),
            (4, 12, true, true)
        );
        assert_eq!(r[record_index(2, 3, false)].treasure, 80.);
    }
}
