//! Synthetic event tables only.
use super::*;

fn range(kind: u8, params: &[u16]) -> RangeEvent {
    RangeEvent {
        start: 0,
        end: u16::MAX,
        kind,
        params: params.to_vec(),
    }
}

#[test]
fn range_events_set_the_documented_flag_bits() {
    let mut f = Flags::default();
    let mut out = Vec::new();
    for kind in [3u8, 9, 0x1e, 0x10, 0x12, 0x2d] {
        f.range_event(&range(kind, &[]), 4, &mut out);
    }
    assert_eq!(f.word, 0x20 | 0x200 | 0x100_0000);
    assert_eq!(f.ranges, 0b0101);
    assert_eq!(f.word3, 1);
    assert!(out.is_empty());
}

#[test]
fn reset_keeps_only_the_persistent_bits_and_clears_the_rest() {
    let mut f = Flags {
        word: 0xffff_ffff,
        word2: 0xff,
        word3: 0xff,
        ranges: 0xf,
        pulses: 0xf,
    };
    f.reset();
    assert_eq!(f.word, flag::KEPT);
    assert_eq!(f.word2, 0xfc);
    assert_eq!(f.word3, 0);
    assert_eq!(f.ranges, 0);
    // The pulse bits are cleared by the advance, not the reset.
    assert_eq!(f.pulses, 0xf);
}

#[test]
fn a_range_naming_the_playing_motion_sets_its_bit() {
    let mut f = Flags::default();
    let mut out = Vec::new();
    f.range_event(&range(0x23, &[7]), 7, &mut out);
    f.range_event(&range(0x23, &[8]), 7, &mut out);
    assert_eq!(f.word, 0x400);
}

#[test]
fn frame_events_pulse_or_ask_the_engine() {
    let mut f = Flags::default();
    let mut out = Vec::new();
    for kind in [9u8, 0xb, 0xc] {
        f.frame_event(
            &FrameEvent {
                frame: 3,
                kind,
                params: Vec::new(),
            },
            1,
            &mut out,
        );
    }
    assert_eq!(f.pulses, 0b1101);
    f.frame_event(
        &FrameEvent {
            frame: 0,
            kind: 0,
            params: Vec::new(),
        },
        1,
        &mut out,
    );
    f.frame_event(
        &FrameEvent {
            frame: 9,
            kind: 7,
            params: vec![5, 6],
        },
        1,
        &mut out,
    );
    assert_eq!(out, [Effect::Takeoff, Effect::ScriptEvent { p0: 5, p1: 6 }]);
}

#[test]
fn frame_events_13_and_14_request_a_voice() {
    // FUN_14040dd70 cases 0xd / 0xe: p0 and, with two parameters, p1.
    let mut f = Flags::default();
    let mut out = Vec::new();
    for (kind, params) in [(0xd, vec![12]), (0xe, vec![12, 3])] {
        f.frame_event(
            &FrameEvent {
                frame: 4,
                kind,
                params,
            },
            1,
            &mut out,
        );
    }
    assert_eq!(
        out,
        [
            Effect::Voice { id: 12, arg: 0 },
            Effect::Voice { id: 12, arg: 3 }
        ]
    );
}
#[test]
fn frame_events_fire_once_as_the_clock_passes_them() {
    // Length 10, loop from 4: frame 0 fires at tick 0; the first step is two ticks.
    assert!(frame_event_fires(0, 0, 10, 4));
    assert!(frame_event_fires(1, 2, 10, 4));
    assert!(frame_event_fires(2, 2, 10, 4));
    assert!(!frame_event_fires(1, 3, 10, 4));
    assert!(frame_event_fires(10, 10, 10, 4));
    assert!(frame_event_fires(5, 11, 10, 4));
    assert!(!frame_event_fires(4, 10, 10, 4));
}

#[test]
fn the_clock_wraps_into_the_loop_window() {
    assert_eq!(native_clock(63, 64, 8), 63);
    assert_eq!(native_clock(64, 64, 8), 8);
    assert_eq!(native_clock(120, 64, 8), 8);
    assert_eq!(native_clock(65, 64, 64), 64);
}
