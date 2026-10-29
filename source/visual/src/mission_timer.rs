//! dk::TIMER, the field's time-limit readout, translated from the
//! executable: shown by FUN_14018d1b0, laid out by FUN_14018d5c0, stepped
//! each frame by FUN_14018d830 (the time split FUN_1401575d0, each digit a
//! group through the table at 0x14071bc38). It draws from a mission's own
//! timer sheet (a `ti_?` entry of msn/<lang>/<mission>.bar, which the
//! mission's timer widget hands it, FUN_1403f9ec0 -> FUN_14017b8a0).
//!
//! The functions are stateless: each sprite's player is worked out from
//! its age in frames, so two screens drawing one clock agree.
use data::sprite_sequence::{Modifiers, Player, Quad, Sequence};

/// The frame ("TIME") and the stopwatch label.
pub const FRAME: usize = 3;
pub const LABEL: usize = 9;
/// Digits 0-9 are groups 0xc-0x15 (the table at 0x14071bc38).
pub const DIGIT: usize = 0xc;
/// The marks after the minutes (') and the seconds (").
pub const MINUTE_MARK: usize = 0x17;
pub const SECOND_MARK: usize = 0x18;
/// The groups whose position anchors each side: left, right, centre
/// (FUN_14018d1b0: 0x1a, 0x1b, 0x19); the sides move 0x55 out.
const ANCHOR: [usize; 3] = [0x1a, 0x1b, 0x19];
const SIDE_SHIFT: f32 = 85.;
/// The last seconds' numerals 0, 1, 2, 3 (FUN_14018d830 plays 0x1f, 0x1e,
/// 0x1d as 2, 1, 0 whole seconds are left, then 0x1c at zero).
pub const NUMERAL: [usize; 4] = [0x1c, 0x1d, 0x1e, 0x1f];
/// se000 0x2b with each of 3, 2 and 1.
pub const NUMERAL_SOUND: u32 = 0x2b;
/// Frames a sprite's player is stepped at most (the readout's groups are
/// still after their entrance).
const MAX_AGE: u32 = 240;

/// FUN_1401575d0: hours, minutes, seconds and hundredths of `ticks` (60 a
/// second; the hundredths are the frame times 1.6666666, 0x1405b19c0).
pub fn split(ticks: u32) -> [u32; 4] {
    let hours = ticks / 216_000;
    let minutes = ticks % 216_000 / 3600;
    let rest = ticks % 216_000 % 3600;
    [
        hours,
        minutes,
        rest / 60,
        ((rest % 60) as f32 * 1.666_666_6) as u32,
    ]
}

/// The numeral a counting-down timer shows with `ticks` left (an index
/// into NUMERAL) and the tick it came up at, for its age; None outside the
/// last three seconds.
pub fn numeral(ticks: u32) -> Option<(usize, u32)> {
    let [h, m, s, _] = split(ticks);
    if h != 0 || m != 0 {
        return None;
    }
    match (ticks, s) {
        (0, _) => Some((0, 0)),
        (_, 0..=2) => Some((s as usize + 1, (s + 1) * 60 - 1)),
        _ => None,
    }
}

fn player(seq: &Sequence, group: usize, age: u32) -> Player {
    let mut p = Player::new(seq, group);
    for _ in 0..=age.min(MAX_AGE) {
        p.update(seq, 1.);
    }
    p
}

/// A group of a sheet `age` frames after it was set, drawn at `origin`
/// (a mission's start and end sheets place themselves: origin (0, 0)).
pub fn group_quads(seq: &Sequence, group: usize, age: u32, origin: [f32; 2]) -> Vec<Quad> {
    if group >= seq.groups.len() {
        return Vec::new();
    }
    seq.quads(&player(seq, group, age), origin, &Modifiers::default())
}

/// Which edge the readout sits on (dk::TIMER +0x1ea0).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Side {
    Left = 0,
    Right = 1,
    Centre = 2,
}

/// The readout showing `ticks` (counting down), `age` frames after it was
/// shown, at the top of its side (no other HUD widget stacks above it).
pub fn quads(seq: &Sequence, side: Side, ticks: u32, age: u32) -> Vec<Quad> {
    let needed = [
        FRAME,
        LABEL,
        DIGIT + 9,
        MINUTE_MARK,
        SECOND_MARK,
        ANCHOR[0],
        ANCHOR[1],
        ANCHOR[2],
    ];
    if needed.iter().any(|&g| g >= seq.groups.len()) {
        return Vec::new();
    }
    let m = Modifiers::default();
    let anchor = player(seq, ANCHOR[side as usize], 0);
    let x = seq.child_context(&anchor, &m).unwrap_or([0.; 2])[0]
        + match side {
            Side::Left => -SIDE_SHIFT,
            Side::Right => SIDE_SHIFT,
            Side::Centre => 0.,
        };
    let frame = player(seq, FRAME, age);
    let mut out = seq.quads(&frame, [x, 0.], &m);
    let context = seq.child_context(&frame, &m).unwrap_or([0.; 2]);
    let base = [x + context[0], context[1]];
    // FUN_14018d830: a label whose pitch is 1 shows a running second up.
    let shown = if seq.groups[LABEL].text_position_x == 1 && ticks != 0 {
        ticks + 60
    } else {
        ticks
    };
    let [_, minutes, seconds, hundredths] = split(shown);
    let pair = |v: u32| [DIGIT + (v / 10 % 10) as usize, DIGIT + (v % 10) as usize];
    let [m0, m1] = pair(minutes);
    let [s0, s1] = pair(seconds);
    let [c0, c1] = pair(hundredths);
    // FUN_14018d5c0: each sprite after the last by its group's pitch (the
    // digits as laid out on 0, then the marks), all from the frame.
    let pitch = |g: usize| seq.groups[g].text_position_x as f32;
    let row = [
        (m0, DIGIT),
        (m1, DIGIT),
        (MINUTE_MARK, MINUTE_MARK),
        (s0, DIGIT),
        (s1, DIGIT),
        (SECOND_MARK, SECOND_MARK),
        (c0, DIGIT),
        (c1, DIGIT),
    ];
    out.extend(seq.quads(&player(seq, LABEL, age), base, &m));
    let mut at = 0.;
    for (group, laid) in row {
        out.extend(seq.quads(&player(seq, group, age), [base[0] + at, base[1]], &m));
        at += pitch(laid);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_time_splits_as_the_executable_does() {
        assert_eq!(split(0), [0, 0, 0, 0]);
        // 1 minute 2 seconds and 30 frames: 50 hundredths.
        assert_eq!(split(3600 + 120 + 30), [0, 1, 2, 50]);
        assert_eq!(split(216_000 + 59), [1, 0, 0, 98]);
    }

    #[test]
    fn the_numerals_come_with_the_last_three_seconds() {
        assert_eq!(numeral(181), None);
        // 2 whole seconds left: "3"; 1: "2"; 0: "1"; zero: "0".
        assert_eq!(numeral(179), Some((3, 179)));
        assert_eq!(numeral(120), Some((3, 179)));
        assert_eq!(numeral(119), Some((2, 119)));
        assert_eq!(numeral(1), Some((1, 59)));
        assert_eq!(numeral(0), Some((0, 0)));
        assert_eq!(numeral(3600 + 10), None);
    }
}
