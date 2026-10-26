//! `--magic-tour <new-capture-dir>` (with `--magic-form <n>` to cast in a
//! Drive form): an unattended run that casts each spell from the ground and
//! the air, then probes how a cast joins the combo: a cast pressed at each
//! distance after an attack, a cast after two attacks (the finisher), and an
//! attack pressed after a cast that hit. Sora is invincible, the enemy
//! stands in front of him and takes every hit. The run's log carries a
//! `cast` / `cast_refused` event for each press. After Sora's casts, Donald
//! casts each of his four spells on order, then the party's AI runs free.
//! Last, every spell goes through three combo probes: Attack, Attack, spell
//! (the finisher), Attack then spell, and a jump, air Attack then spell;
//! Attack is mashed after each cast to show whether the combo goes on.
//! The moves come at the end (alone with `--form-moves`): a run, a walk, a
//! jump, a mashed ground combo and air combo on the enemy, Square on the
//! ground and in the air (the growth abilities), with a `move_probe` event
//! every tick.
use player::magic;
use player::party_members::Spell;
use std::sync::atomic::{AtomicBool, Ordering};

/// `--form-moves`: only the moves, from `MOVES_LEAD` ticks after the start.
static MOVES_ONLY: AtomicBool = AtomicBool::new(false);

pub fn set_moves_only() {
    MOVES_ONLY.store(true, Ordering::Relaxed);
}

/// The schedule's tick for the room's `tick`.
fn at(tick: u64, shift: u64) -> Option<u64> {
    let t = tick.checked_sub(shift)?;
    if MOVES_ONLY.load(Ordering::Relaxed) {
        t.checked_sub(MOVES_LEAD).map(|t| t + MOVES_START)
    } else {
        Some(t)
    }
}

/// Ticks of the tour: the six spells on the ground, then in the air.
const GROUND: [(u64, u8); 6] = [
    (100, magic::FIRE),
    (260, magic::BLIZZARD),
    (420, magic::THUNDER),
    (580, magic::CURE),
    (740, magic::MAGNET),
    (900, magic::REFLECT),
];
const AIR: [(u64, u8); 3] = [
    (1085, magic::FIRE),
    (1225, magic::BLIZZARD),
    (1365, magic::THUNDER),
];
const AIR_JUMPS: [u64; 3] = [1060, 1200, 1340];
/// Attack, then Fire this many ticks later.
const PROBES: [u64; 10] = [6, 10, 14, 18, 22, 26, 30, 36, 44, 56];
const PROBE_START: u64 = 1550;
const PROBE_SPACING: u64 = 160;
/// Attack, attack 30 ticks later, then Fire after these many more (a
/// combo finisher on the third hit).
const FINISH: [u64; 4] = [28, 34, 40, 48];
const FINISH_START: u64 = 3200;
const FINISH_SPACING: u64 = 200;
const SECOND_ATTACK: u64 = 30;
/// Fire, then an attack this many ticks later (the cast hit the enemy).
const AFTER: [u64; 3] = [62, 70, 78];
const AFTER_START: u64 = 4000;
const AFTER_SPACING: u64 = 200;
/// Cure on Donald, then on Goofy (the party list's picks).
const PARTY_CURES: [(u64, usize); 2] = [(4650, 1), (4800, 2)];
/// Donald's four spells on order, then the party's AI casting on its own.
const PARTY_CASTS: [(u64, usize, Spell, u8); 6] = [
    (5050, 0, Spell::Fire, 0),
    (5250, 0, Spell::Thunder, 0),
    (5450, 0, Spell::Blizzard, 0),
    (5650, 0, Spell::Cure, 2),
    (5800, 1, Spell::Turbo, 0),
    (6200, 1, Spell::Tornado, 0),
];
/// The AI's own casts run from here to the end.
pub const FREE_PARTY: u64 = 6800;
/// Magic Lock-On: Fire pressed with the enemy beyond the automatic
/// target's range, with the ability (first) and without it (second).
const LOCK: [(u64, bool); 2] = [(8400, true), (8600, false)];
const LOCK_DISTANCE: f32 = 1400.;
/// The combo probes: per spell (Fire, Blizzard, Thunder, Cure, Magnet,
/// Reflect) a finisher, a mid-combo and an air case, `COMBO_SPACING` apart.
const COMBO_START: u64 = 8800;
const COMBO_SPACING: u64 = 260;
const COMBO_SPELLS: [u8; 6] = [
    magic::FIRE,
    magic::BLIZZARD,
    magic::THUNDER,
    magic::CURE,
    magic::MAGNET,
    magic::REFLECT,
];
/// Per case: the attack ticks, the cast tick and the jump (air case).
const COMBO_CASES: [(&[u64], u64, bool); 3] =
    [(&[0, 30], 78, false), (&[0], 26, false), (&[20], 46, true)];
/// Attack is pressed every `COMBO_MASH` ticks from 16 to 120 after the cast.
const COMBO_MASH: u64 = 4;
const COMBO_END: u64 =
    COMBO_START + COMBO_SPACING * (COMBO_SPELLS.len() * COMBO_CASES.len()) as u64;
/// The moves (relative ticks): a full-stick run, a half-stick walk, a
/// standing jump; the enemy placed in front for a mashed ground combo
/// and a jump with a mashed air combo; a run with Square pressed and held
/// (Dodge Roll, Quick Run); a jump, Jump again in the air (Aerial Dodge),
/// then Square held (Glide).
const MOVES_START: u64 = COMBO_END;
const MOVES_LEAD: u64 = 300;
const RUN: (u64, u64) = (10, 70);
const WALK: (u64, u64) = (90, 150);
const STAND_JUMP: u64 = 170;
const GROUND_COMBO: u64 = 340;
const AIR_COMBO: u64 = 700;
/// Attack every third tick for 200 ticks (ground), 90 (air).
const MASH: (u64, u64, u64) = (2, 200, 3);
const AIR_MASH: u64 = 90;
const SQUARE_RUN: (u64, u64) = (900, 980);
const SQUARE_GROUND: (u64, u64) = (910, 940);
const SQUARE_AIR: u64 = 1050;
/// The air Jump (Aerial Dodge), then the air Square (Glide).
const SQUARE_AIR_PRESS: [u64; 2] = [1070, 1090];
const GLIDE_HOLD: (u64, u64) = (1090, 1170);
const MOVES_LEN: u64 = 1250;

/// The last tick of the schedule.
pub fn end() -> u64 {
    if MOVES_ONLY.load(Ordering::Relaxed) {
        MOVES_LEAD + MOVES_LEN
    } else {
        MOVES_START + MOVES_LEN
    }
}

/// The moves' relative tick at `tick`, if they are running.
fn moves(tick: u64, shift: u64) -> Option<u64> {
    at(tick, shift)?
        .checked_sub(MOVES_START)
        .filter(|&m| m < MOVES_LEN)
}

/// Whether the room logs a `move_probe` this tick.
pub fn probing(tick: u64, shift: u64) -> bool {
    moves(tick, shift).is_some()
}

/// The left stick: x right, y forward.
pub fn stick(tick: u64, shift: u64) -> [f32; 2] {
    let within = |m: u64, (a, b): (u64, u64)| (a..b).contains(&m);
    match moves(tick, shift) {
        Some(m) if within(m, RUN) || within(m, SQUARE_RUN) => [1., 0.],
        Some(m) if within(m, WALK) => [0.5, 0.],
        _ => [0.; 2],
    }
}

/// Square: pressed this tick, and held.
pub fn guard(tick: u64, shift: u64) -> (bool, bool) {
    let Some(m) = moves(tick, shift) else {
        return (false, false);
    };
    let held = (SQUARE_GROUND.0..SQUARE_GROUND.1).contains(&m)
        || (GLIDE_HOLD.0..GLIDE_HOLD.1).contains(&m);
    (m == SQUARE_GROUND.0 || m == SQUARE_AIR_PRESS[1], held)
}

/// The combo probe running at `tick`: its spell, case and base tick.
fn combo_case(tick: u64) -> Option<(u8, usize, u64)> {
    let k = tick.checked_sub(COMBO_START)? / COMBO_SPACING;
    let spell = *COMBO_SPELLS.get(k as usize / COMBO_CASES.len())?;
    let case = k as usize % COMBO_CASES.len();
    Some((spell, case, COMBO_START + k * COMBO_SPACING))
}

/// The party slot Cure targets when it is cast this tick (0 = Sora).
pub fn cure_target(tick: u64, shift: u64) -> usize {
    let Some(tick) = at(tick, shift) else {
        return 0;
    };
    PARTY_CURES.iter().find(|c| c.0 == tick).map_or(0, |c| c.1)
}

/// A party member's ordered cast this tick: member index (0 Donald, 1
/// Goofy), the act, and the party slot Cure heals.
pub fn party_cast(tick: u64, shift: u64) -> Option<(usize, Spell, u8)> {
    let tick = at(tick, shift)?;
    PARTY_CASTS
        .iter()
        .find(|c| c.0 == tick)
        .map(|c| (c.1, c.2, c.3))
}

/// Whether Magic Lock-On is on from this tick on, when a probe changes it.
pub fn lock_on(tick: u64, shift: u64) -> Option<bool> {
    let tick = at(tick, shift)?;
    LOCK.iter().find(|l| l.0 == tick + 1).map(|l| l.1)
}

/// How far ahead of Sora the enemy is put back this tick.
pub fn place_distance(tick: u64, shift: u64) -> f32 {
    let far = at(tick, shift).is_some_and(|t| {
        LOCK.iter().any(|l| l.0 == t + 2)
            || t + 2 >= MOVES_START && ![GROUND_COMBO, AIR_COMBO].contains(&(t + 2 - MOVES_START))
    });
    if far { LOCK_DISTANCE } else { 110. }
}

/// The party's AI may cast spells.
pub fn free_party(tick: u64, shift: u64) -> bool {
    at(tick, shift).is_some_and(|t| (FREE_PARTY..COMBO_START).contains(&t))
}

fn probe_base(i: usize) -> u64 {
    PROBE_START + i as u64 * PROBE_SPACING
}

fn finish_base(i: usize) -> u64 {
    FINISH_START + i as u64 * FINISH_SPACING
}

fn after_base(i: usize) -> u64 {
    AFTER_START + i as u64 * AFTER_SPACING
}

/// The spell cast this tick, if any.
pub fn cast(tick: u64, shift: u64) -> Option<u8> {
    let tick = at(tick, shift)?;
    for (t, m) in GROUND.iter().chain(&AIR) {
        if *t == tick {
            return Some(*m);
        }
    }
    let at = |base: u64, k: u64| base + k == tick;
    if PARTY_CURES.iter().any(|c| c.0 == tick) {
        return Some(magic::CURE);
    }
    if LOCK.iter().any(|l| l.0 == tick) {
        return Some(magic::FIRE);
    }
    if let Some((spell, case, base)) = combo_case(tick)
        && base + COMBO_CASES[case].1 == tick
    {
        return Some(spell);
    }
    if PROBES
        .iter()
        .enumerate()
        .any(|(i, &k)| at(probe_base(i), k))
        || FINISH
            .iter()
            .enumerate()
            .any(|(i, &k)| at(finish_base(i), SECOND_ATTACK + k))
        || (0..AFTER.len()).any(|i| after_base(i) == tick)
    {
        return Some(magic::FIRE);
    }
    None
}

/// The enemy is put back 110 ahead of Sora two ticks before each action.
pub fn place(tick: u64, shift: u64) -> bool {
    let Some(tick) = at(tick, shift) else {
        return tick == 60;
    };
    let starts = GROUND
        .iter()
        .chain(&AIR)
        .map(|s| s.0)
        .chain(AIR_JUMPS)
        .chain((0..PROBES.len()).map(probe_base))
        .chain((0..FINISH.len()).map(finish_base))
        .chain((0..AFTER.len()).map(after_base))
        .chain(PARTY_CURES.iter().map(|c| c.0))
        .chain(PARTY_CASTS.iter().map(|c| c.0))
        .chain(LOCK.iter().map(|l| l.0))
        .chain(
            (0..(COMBO_END - COMBO_START) / COMBO_SPACING).map(|k| COMBO_START + k * COMBO_SPACING),
        )
        .chain([0, GROUND_COMBO, AIR_COMBO, SQUARE_RUN.0, SQUARE_AIR].map(|m| MOVES_START + m));
    tick == 60 || starts.into_iter().any(|b| b == tick + 2)
}

/// The attack pressed this tick.
pub fn attack(tick: u64, shift: u64) -> bool {
    if let Some(m) = moves(tick, shift) {
        let (from, to, every) = MASH;
        return [(GROUND_COMBO, to), (AIR_COMBO + 10, AIR_MASH)]
            .iter()
            .any(|&(b, to)| {
                m.checked_sub(b)
                    .is_some_and(|d| (from..=to).contains(&d) && d % every == 0)
            });
    }
    let Some(tick) = at(tick, shift) else {
        return false;
    };
    (0..PROBES.len()).any(|i| probe_base(i) == tick)
        || (0..FINISH.len()).any(|i| {
            let b = finish_base(i);
            tick == b || tick == b + SECOND_ATTACK
        })
        || AFTER
            .iter()
            .enumerate()
            .any(|(i, &k)| after_base(i) + k == tick)
        || combo_case(tick).is_some_and(|(_, case, base)| {
            let (attacks, cast, _) = COMBO_CASES[case];
            let after = tick.checked_sub(base + cast);
            attacks.iter().any(|&a| base + a == tick)
                || after.is_some_and(|d| (16..=120).contains(&d) && d % COMBO_MASH == 0)
        })
}

pub fn jump(tick: u64, shift: u64) -> bool {
    if let Some(m) = moves(tick, shift) {
        return [STAND_JUMP, AIR_COMBO, SQUARE_AIR, SQUARE_AIR_PRESS[0]].contains(&m);
    }
    at(tick, shift).is_some_and(|t| {
        AIR_JUMPS.contains(&t)
            || combo_case(t).is_some_and(|(_, case, base)| COMBO_CASES[case].2 && base == t)
    })
}

/// Capture ticks: shortly after each cast.
pub fn captures(shift: u64) -> Vec<u64> {
    // The moves: through both combos, the Square run and the air Square.
    let moves = (0..MOVES_LEN)
        .step_by(20)
        .filter(|&m| m >= GROUND_COMBO)
        .map(|m| m + MOVES_START);
    if MOVES_ONLY.load(Ordering::Relaxed) {
        return moves
            .map(|t| t - MOVES_START + MOVES_LEAD + shift)
            .collect();
    }
    let mut at: Vec<u64> = moves.collect();
    for (t, _) in GROUND.iter().chain(&AIR) {
        at.extend([t + 14, t + 26, t + 40, t + 60]);
    }
    for (i, k) in FINISH.iter().enumerate() {
        at.push(finish_base(i) + SECOND_ATTACK + k + 12);
    }
    for (i, k) in AFTER.iter().enumerate() {
        at.push(after_base(i) + k + 16);
    }
    at.extend(PARTY_CURES.iter().flat_map(|c| [c.0 + 20, c.0 + 40]));
    at.extend(
        PARTY_CASTS
            .iter()
            .flat_map(|c| [c.0 + 20, c.0 + 50, c.0 + 80, c.0 + 100, c.0 + 130]),
    );
    at.extend((0..8).map(|i| FREE_PARTY + 100 + i * 120));
    at.extend(LOCK.iter().flat_map(|l| [l.0 + 14, l.0 + 40]));
    at.sort_unstable();
    at.dedup();
    at.iter().map(|t| t + shift).collect()
}
