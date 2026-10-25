//! The prizes' sprites: program 203 of `obj/PRIZE.mdlx` (program table
//! record 0x14077d970, draw FUN_1404d7ed0). Its parameter block is a
//! four-byte header and ten twelve-byte groups: the shape id (0xffff =
//! none), the colour bytes (0x80 = 1.0) and a float scale. A prize of kind
//! `k` draws group `k`; each tick it steps one flip-book frame, and the
//! shape's last sequence entry says whether the flip-book loops (starting
//! at a random frame) or holds its last frame.
use crate::programs::sprite_quads;
use crate::{Quad, math::Mat};
use data::object_effect::{self, Dpd, f32_at, u32_at};

/// The effect program number of the prize draw.
pub const PROGRAM: u32 = 203;
const GROUPS: usize = 10;

#[derive(Clone, Copy, Debug, PartialEq)]
struct Group {
    anim: usize,
    colour: [u8; 3],
    scale: f32,
    count: usize,
    looping: bool,
}

/// The ten prize groups of a PRIZE effect.
#[derive(Debug, Default)]
pub struct Prize {
    groups: [Option<Group>; GROUPS],
}

/// The flip-book frame a prize shows `age` ticks after it appeared:
/// looping shapes start at `seed` (0..1) of their length, others at frame 0
/// and hold the last frame.
pub fn frame_at(count: usize, looping: bool, age: u32, seed: f32) -> usize {
    if count == 0 {
        return 0;
    }
    if looping {
        let start = ((count - 1) as f32 * seed.clamp(0., 1.)) as usize;
        (start + age as usize) % count
    } else {
        (age as usize).min(count - 1)
    }
}

impl Prize {
    /// Reads the groups of the DPD's prize draw program; `None` when the
    /// DPD has no such program.
    pub fn new(dpd: &Dpd) -> Option<Prize> {
        let program = dpd
            .pdata
            .iter()
            .flat_map(|p| p.entries.iter())
            .flat_map(|e| e.programs.iter())
            .find(|p| p.kind == PROGRAM)?;
        let b = &dpd.bytes;
        let mut prize = Prize::default();
        for (g, slot) in prize.groups.iter_mut().enumerate() {
            let at = program.params + 4 + g * 12;
            let id = u32_at(b, at).ok()? as i32;
            let Some(shape) = i16::try_from(id).ok().and_then(|i| dpd.shapes.get(&i)) else {
                continue;
            };
            let anim = shape.anim;
            let count = object_effect::shape_sequence_count(b, anim).ok()?.max(0) as usize;
            let looping = count > 0
                && object_effect::shape_sequence(b, anim, count - 1)
                    .map(|(_, flag)| flag & 0x80 != 0)
                    .unwrap_or(false);
            *slot = Some(Group {
                anim,
                colour: [*b.get(at + 4)?, *b.get(at + 5)?, *b.get(at + 6)?],
                scale: f32_at(b, at + 8).ok()?,
                count,
                looping,
            });
        }
        Some(prize)
    }

    /// The frame of a prize of `kind` at `age`, or `None` for a kind
    /// without a shape.
    pub fn frame(&self, kind: u8, age: u32, seed: f32) -> Option<usize> {
        let g = self.groups.get(kind as usize)?.as_ref()?;
        Some(frame_at(g.count, g.looping, age, seed))
    }

    /// FUN_1404d7ed0's frame of one point of group `kind` whose frame byte
    /// is `byte`: 0xff starts a looping shape at `seed` (0..1) of its length.
    /// Returns the frame drawn and the byte the draw leaves: one on, and
    /// past the end 0 for a looping shape, the same frame otherwise.
    pub fn point_frame(&self, kind: u8, byte: u8, seed: f32) -> Option<(usize, u8)> {
        let g = self.groups.get(kind as usize)?.as_ref()?;
        if g.count == 0 {
            return None;
        }
        let mut frame = byte as usize;
        if byte == 0xff && g.looping {
            frame = ((g.count - 1) as f32 * seed) as usize;
            if frame >= g.count {
                frame = 0;
            }
        }
        let frame = frame.min(g.count - 1);
        let next = if frame + 1 < g.count {
            frame + 1
        } else if g.looping {
            0
        } else {
            frame
        };
        Some((frame, next as u8))
    }

    /// The sprites of a prize of `kind` at flip-book `frame`, centred at
    /// `centre` in the effect view space (x right, y down, z forward), with
    /// the point's `alpha` (0x80 = 1.0).
    pub fn quads(
        &self,
        dpd: &Dpd,
        kind: u8,
        frame: usize,
        centre: [f32; 3],
        alpha: u8,
    ) -> Vec<Quad> {
        let Some(Some(g)) = self.groups.get(kind as usize) else {
            return Vec::new();
        };
        let s = g.scale;
        let m: Mat = [
            [s, 0., 0., 0.],
            [0., s, 0., 0.],
            [0., 0., s, 0.],
            [centre[0], centre[1], centre[2], 1.],
        ];
        sprite_quads(
            &dpd.bytes,
            g.anim,
            frame,
            &m,
            [g.colour[0], g.colour[1], g.colour[2], alpha],
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn looping_shapes_wrap_and_others_hold() {
        assert_eq!(frame_at(24, true, 30, 0.), 6);
        assert_eq!(frame_at(24, true, 0, 1.), 23);
        assert_eq!(frame_at(24, false, 30, 0.5), 23);
        assert_eq!(frame_at(24, false, 5, 0.5), 5);
        assert_eq!(frame_at(0, true, 5, 0.), 0);
    }

    #[test]
    fn a_point_starts_looping_shapes_at_the_seed_and_steps_one_frame() {
        let group = |looping| {
            Some(Group {
                anim: 0,
                colour: [0x80; 3],
                scale: 1.,
                count: 4,
                looping,
            })
        };
        let mut prize = Prize::default();
        prize.groups[7] = group(true);
        prize.groups[2] = group(false);
        assert_eq!(prize.point_frame(7, 0xff, 0.5), Some((1, 2)));
        assert_eq!(prize.point_frame(7, 3, 0.5), Some((3, 0)));
        assert_eq!(prize.point_frame(2, 3, 0.5), Some((3, 3)));
        assert_eq!(prize.point_frame(2, 0xff, 0.5), Some((3, 3)));
        assert_eq!(prize.point_frame(5, 0, 0.5), None);
    }
}
