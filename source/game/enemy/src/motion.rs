//! The motion player's event flags and the dispatch of a clip's trigger tables,
//! translated from the executable (Steam build 15194255): the range-event
//! switch FUN_14040d810, the frame-event switch FUN_14040dd70, the per-tick
//! flag reset FUN_1403c83b0 / FUN_14040e5c0 and the range pass FUN_14040e740.
//!
//! A motion clip carries two tables (BAR type 16): *range* events active while
//! the clock is inside `[start, end)` and *frame* events fired when the clock
//! passes a frame. The player keeps their effects in flag words that scripts
//! read through traps (`+0x18c` bits, the `+0x2ac` range bits, the `+0x2ad`
//! pulse bits). Offsets in comments are the sysobj's.

/// A range trigger record.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RangeEvent {
    pub start: u16,
    /// `0xffff` = open ended.
    pub end: u16,
    pub kind: u8,
    pub params: Vec<u16>,
}

/// A frame trigger record.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FrameEvent {
    pub frame: u16,
    pub kind: u8,
    pub params: Vec<u16>,
}

/// One clip's trigger tables.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct MotionEvents {
    pub ranges: Vec<RangeEvent>,
    pub frames: Vec<FrameEvent>,
}

/// Bits of the motion flag word (`+0x18c`).
pub mod flag {
    /// The player is paused (nothing advances).
    pub const PAUSED: u32 = 1;
    /// The clip finished on this tick's step (the end pulse).
    pub const ENDED: u32 = 2;
    /// Gravity is frozen (range type 3).
    pub const GRAVITY_FROZEN: u32 = 0x20;
    /// Range type 0x18 (also freezes the arc).
    pub const HOLD: u32 = 0x40;
    /// The clip holds the actor out of control (range type 9).
    pub const NO_CONTROL: u32 = 0x200;
    /// Range type 0x1e.
    pub const GRAVITY_FROZEN_2: u32 = 0x100_0000;
    /// The bits that survive the per-tick reset (FUN_1403c83b0 `& 0x7183`).
    pub const KEPT: u32 = 0x7183;
}

/// FUN_1403c6740's clock wrap: a tick inside `[0, duration)` maps to itself, a
/// later one into the loop window.
pub fn native_clock(t: u32, duration: u32, loop_top: u32) -> u32 {
    if t < duration {
        t
    } else if loop_top < duration {
        loop_top + (t - duration) % (duration - loop_top)
    } else {
        duration
    }
}

/// A frame event at `frame` fires on the step that produced tick `t`
/// (FUN_14040e960's `previous < frame <= current` with the first step of a
/// motion jumping from 0 to 2, and the loop seam handled).
pub fn frame_event_fires(frame: u16, t: u32, duration: u32, loop_top: u32) -> bool {
    let frame = u32::from(frame);
    if t == 0 {
        return frame == 0;
    }
    let clock = native_clock(t, duration, loop_top);
    let step = if t == 2 || (loop_top == 0 && clock == 2) {
        2
    } else {
        1
    };
    let previous = native_clock(t.saturating_sub(step), duration, loop_top);
    if previous < clock {
        previous < frame && frame <= clock
    } else if previous > clock {
        (previous < frame && frame <= duration) || (loop_top < frame && frame <= clock)
    } else {
        false
    }
}

/// The per-motion event state (sysobj +0x34, +0x38, +0x3c, +0x154, +0x155).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Flags {
    pub word: u32,
    pub word2: u32,
    pub word3: u32,
    /// Range events 0x10..=0x13 as bits 0..=3 (`trap_obj_motion_check_range`).
    pub ranges: u8,
    /// Frame events 9..=12 as bits 0..=3 (`trap_obj_motion_check_trigger`);
    /// cleared at the start of every advance.
    pub pulses: u8,
}

/// What a dispatch asks the rest of the engine to do.
#[derive(Clone, Debug, PartialEq)]
pub enum Effect {
    /// Frame event 0: leave the ground into the pending physics mode.
    Takeoff,
    /// Frame event 3: FUN_1403b4da0(obj, 7), the jump of fixed motions
    /// 155-157 (Dusk's), on the jump the last `obj_flyjump` /
    /// `method_obj_jump` armed.
    JumpMode7,
    /// Frame event 1: effect or sound `id`; the script's trigger 0x10 gets
    /// `(object, id, effect handle)`.
    EffectOrSound { id: u16 },
    /// Frame event 7: the script's trigger 0x11 gets `(object, p0, p1)`.
    ScriptEvent { p0: u16, p1: u16 },
    /// Frame events 0x17 / 0x18: the alpha fade (+0xa08) down or up over
    /// `ticks`.
    AlphaFade { to: f32, ticks: u16 },
    /// Frame events 0xd / 0xe: the class's voice slot (vt+0xa0, as
    /// `obj_voice`) with voice `id` and `arg` (0 when the record has fewer
    /// than two parameters).
    Voice { id: u16, arg: u16 },
    /// Range event 0x18: `+0x108` = `value * scale`.
    SetSpeed { value: u16 },
    /// Range events 0x19 / 0x1a: sysobj +0x638 = value.
    SetParameter { value: u16 },
    /// Range event 0x24 / 0x25: bit `n` of the pattern words (+0xd14 / +0xd18).
    PatternBit { word: u8, bit: u8 },
    /// Frame event 0x1c: start the `revenge` act when the revenge gauge has
    /// reached its limit.
    Revenge,
    /// Range events 4..7: the reaction-command enables of the object's
    /// collision groups (see [`crate::reaction::Groups::range_event`]).
    ReactionGroup { kind: u8, p0: u16, p1: u16 },
    /// An event whose engine routine belongs to another subsystem (sound,
    /// effects, camera, physics modes): reported to the host unchanged.
    Other {
        range: bool,
        kind: u8,
        params: Vec<u16>,
    },
}

impl Flags {
    /// FUN_1403c83b0 / FUN_14040e5c0: the reset before the range pass.
    pub fn reset(&mut self) {
        self.word &= flag::KEPT;
        self.word2 &= !3;
        self.word3 = 0;
        self.ranges = 0;
    }

    /// FUN_14040d810 for one active range record.
    pub fn range_event(&mut self, e: &RangeEvent, slot: usize, out: &mut Vec<Effect>) {
        let p = |k: usize| e.params.get(k).copied().unwrap_or(0);
        let w = &mut self.word;
        match e.kind {
            0 => *w |= 4,
            1 => *w |= 8,
            2 => *w |= 0x10,
            3 => *w |= 0x20,
            9 => *w |= 0x200,
            0xb => *w |= 0x400,
            0xc => *w |= 0x800,
            0xd => *w |= 0x8000,
            0xe => *w |= 0x8_0000,
            0xf => *w |= 0x1_0000,
            0x10 => self.ranges |= 1,
            0x11 => self.ranges |= 2,
            0x12 => self.ranges |= 4,
            0x13 => self.ranges |= 8,
            0x15 => *w |= 0x4_0000,
            0x18 => {
                *w |= 0x40;
                out.push(Effect::SetSpeed { value: p(0) });
            }
            0x19 => {
                *w |= 0x10_0000;
                out.push(Effect::SetParameter { value: p(0) });
            }
            0x1a => {
                *w |= 0x20_0000;
                out.push(Effect::SetParameter { value: p(0) });
            }
            0x1b => *w |= 0x40_0000,
            0x1d => *w |= 0x80_0000,
            0x1e => *w |= 0x100_0000,
            0x1f => *w |= 0x200_0000,
            0x20 => *w |= 0x400_0000,
            0x22 => *w |= 0x800_0000,
            0x23 => {
                if usize::from(p(0)) == slot {
                    *w |= 0x400;
                }
            }
            0x24 => out.push(Effect::PatternBit {
                word: 0,
                bit: (p(0) & 31) as u8,
            }),
            0x25 => out.push(Effect::PatternBit {
                word: 1,
                bit: (p(0) & 31) as u8,
            }),
            0x26 => *w |= 0x1000_0000,
            0x27 => *w |= 0x2000_0000,
            0x28 => *w |= 0x4000_0000,
            0x29 => *w |= 0x8000_0000,
            0x2a => self.word2 |= 1,
            0x2c => self.word2 |= 2,
            0x2d => self.word3 |= 1,
            0x2e => self.word3 |= 2,
            0x2f => self.word3 |= 4,
            0x31 => self.word3 |= 0x10,
            0x32 => self.word3 |= 0x20,
            0x34 => self.word3 |= 0x40,
            0x35 => self.word3 |= 0x80,
            // 8, 0x2b, 0x30 do nothing.
            8 | 0x2b | 0x30 => {}
            // The remaining kinds reach other subsystems (damage-timers at
            // +0x4e0/+0x5a0, voices, the player's lock-on, hit marks).
            4..=7 => out.push(Effect::ReactionGroup {
                kind: e.kind,
                p0: p(0),
                p1: p(1),
            }),
            0xa | 0x14 | 0x16 | 0x17 | 0x1c | 0x21 | 0x33 => out.push(Effect::Other {
                range: true,
                kind: e.kind,
                params: e.params.clone(),
            }),
            _ => {}
        }
    }

    /// FUN_14040dd70 for one frame event (the parts that only set flags;
    /// everything else is returned as an [`Effect`]).
    pub fn frame_event(&mut self, e: &FrameEvent, slot: usize, out: &mut Vec<Effect>) {
        let p = |k: usize| e.params.get(k).copied().unwrap_or(0);
        match e.kind {
            0 => out.push(Effect::Takeoff),
            3 => out.push(Effect::JumpMode7),
            1 => out.push(Effect::EffectOrSound { id: p(0) }),
            7 => out.push(Effect::ScriptEvent { p0: p(0), p1: p(1) }),
            0x17 => out.push(Effect::AlphaFade {
                to: 0.,
                ticks: p(0),
            }),
            0x18 => out.push(Effect::AlphaFade {
                to: 1.,
                ticks: p(0),
            }),
            0xd | 0xe => out.push(Effect::Voice {
                id: p(0),
                arg: if e.params.len() < 2 { 0 } else { p(1) },
            }),
            9 => self.pulses |= 1,
            10 => self.pulses |= 2,
            0xb => self.pulses |= 4,
            0xc => self.pulses |= 8,
            0x1c => out.push(Effect::Revenge),
            0x12 | 0x14 | 0x20 | 0x21 => {}
            _ => out.push(Effect::Other {
                range: false,
                kind: e.kind,
                params: std::iter::once(slot as u16)
                    .chain(e.params.iter().copied())
                    .collect(),
            }),
        }
    }
}

#[cfg(test)]
mod tests;
