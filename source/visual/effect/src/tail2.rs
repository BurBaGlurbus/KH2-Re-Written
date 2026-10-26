//! pppKeShpTailX (153), pppKeShpTail2 (154) and pppKeShpTail2X (155): a
//! tail of shape sprites strung along the part's recent path, with a fixed
//! flip-book frame. Translated from the updates FUN_1404cc750 /
//! FUN_1404cce70 / FUN_1404cd550, the draws FUN_1404cc960 / FUN_1404cd080 /
//! FUN_1404cd760 and the creates FUN_1404cc930 / FUN_1404cd050 /
//! FUN_1404cd730. Tail2 is Tail2X with its world flag at key +0x23 (2X:
//! +0x26) and its sprites drawn one by one rather than batched.
//!
//! Work block u0: +0 ring length (31), +1 ring head, +2 flip-book timer,
//! +4 frame index, +6 the previous index (the frame drawn), then the ring
//! of positions from +0x10. The path walk is the one of
//! [`crate::tail`] (pppKeShpTail3X); TailX takes its head colour from the
//! part colour and a size falling from 1 to a ratio, Tail2X from key bytes
//! and a size falling from key +0xc to +0x10. In Tail2X the first sprite
//! is skipped unless key +0x22 is set.
use crate::math::{self, Mat, Vec4};
use crate::programs::{active, byte, emit_sprites, float, use_val, word};
use crate::{Ctx, Part};
use data::object_effect::{self, Program, i16_at};

pub(crate) const TAIL_X: u32 = 153;
pub(crate) const TAIL_2: u32 = 154;
pub(crate) const TAIL_2X: u32 = 155;
const RING: usize = 0x10;

/// FUN_1404cc930 / FUN_1404cd050 / FUN_1404cd730: ring length 31, head and
/// flip-book 0.
pub(crate) fn create(p: &Program, part: &mut Part, ctx: &Ctx) {
    let o = use_val(ctx, p, 0);
    part.set_u8(o, 0x1f);
    part.set_u8(o + 1, 0);
    part.set_i32(o + 2, 0);
    part.set_i16(o + 6, 0);
}

/// The key byte of the world-transform flag (+0x21 / +0x23 / +0x26).
fn world_flag(kind: u32) -> usize {
    match kind {
        TAIL_X => 0x21,
        TAIL_2 => 0x23,
        _ => 0x26,
    }
}

fn sample(p: &Program, cursor: usize, part: &Part, ctx: &Ctx) -> Vec4 {
    if byte(ctx, cursor + world_flag(p.kind)) == 0 {
        part.cur[3]
    } else {
        let mut v = math::apply(&ctx.world, part.cur[3]);
        v[3] = 1.;
        v
    }
}

pub(crate) fn update(p: &Program, cursor: usize, part: &mut Part, ctx: &mut Ctx) {
    if !active(part) {
        return;
    }
    let o = use_val(ctx, p, 0);
    let n = part.u8(o) as usize;
    if n == 0 {
        return;
    }
    if part.frame == 0 && !part.restarted {
        let mut fill = sample(p, cursor, part, ctx);
        fill[3] = 1.;
        for i in 0..n {
            part.set_vec4(o + RING + 16 * i, fill);
        }
    }
    let head = match part.u8(o + 1) {
        0 => n as u8,
        h => h,
    } - 1;
    part.set_u8(o + 1, head);
    let v = sample(p, cursor, part, ctx);
    part.set_vec4(o + RING + 16 * head as usize, v);
    // The flip-book: the previous index takes the current one, the timer
    // gains the speed and rolls into the next frame at its wait.
    let shape = word(ctx, cursor + 4);
    let Some(anim) = ctx.dpd.shapes.get(&(shape as i16)).map(|s| s.anim) else {
        return;
    };
    let b = &ctx.dpd.bytes;
    let index = part.i16(o + 4) as u16;
    part.set_i16(o + 6, index as i16);
    let timer = (part.i16(o + 2) as u16).wrapping_add(i16_at(b, cursor + 8).unwrap_or(0) as u16);
    part.set_i16(o + 2, timer as i16);
    let (wait, flag) = object_effect::shape_sequence(b, anim, index as usize).unwrap_or((1, 0));
    if wait as i32 <= timer as i32 {
        part.set_i16(o + 4, index.wrapping_add(1) as i16);
        part.set_i16(o + 2, timer.wrapping_sub(wait as u16) as i16);
        let count = object_effect::shape_sequence_count(b, anim).unwrap_or(0);
        if count as i32 <= index.wrapping_add(1) as i32 {
            part.set_i16(o + 2, 0);
            if flag & 0x80 == 0 {
                part.set_i16(o + 4, index as i16);
            } else {
                part.set_i16(o + 4, 0);
            }
        }
    }
}

fn sub(a: Vec4, b: Vec4) -> Vec4 {
    std::array::from_fn(|i| a[i] - b[i])
}

fn length(v: Vec4) -> f32 {
    (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt()
}

pub(crate) fn draw(p: &Program, cursor: usize, part: &mut Part, ctx: &mut Ctx) {
    let shape = word(ctx, cursor + 4);
    if shape == 0xffff {
        return;
    }
    let Some(anim) = ctx.dpd.shapes.get(&(shape as i16)).map(|s| s.anim) else {
        return;
    };
    let (o, colour_var) = (use_val(ctx, p, 0), use_val(ctx, p, 1));
    // Tail2 and Tail2X share the key layout (count +0x20, spacing +0x1c,
    // head and tail colours +0x14 / +0x18, sizes +0xc / +0x10).
    let wide = p.kind != TAIL_X;
    let (count_at, spacing_at) = if wide { (0x20, 0x1c) } else { (0x1c, 0x18) };
    let n = i16_at(&ctx.dpd.bytes, cursor + count_at).unwrap_or(0) as u16 as u32;
    let steps = n as i32 - 1;
    let part_rgba: Vec4 = std::array::from_fn(|i| part.u8(colour_var + 8 + i) as f32);
    let (mut colour, tail, size0, size_end) = if wide {
        let fade = part_rgba[3] * (1. / 128.);
        let mut head: Vec4 = std::array::from_fn(|i| byte(ctx, cursor + 0x14 + i) as f32);
        let mut tail: Vec4 = std::array::from_fn(|i| byte(ctx, cursor + 0x18 + i) as f32);
        head[3] *= fade;
        tail[3] *= fade;
        (
            head,
            tail,
            float(ctx, cursor + 0xc),
            float(ctx, cursor + 0x10),
        )
    } else {
        let tail: Vec4 = std::array::from_fn(|i| {
            ((i16_at(&ctx.dpd.bytes, cursor + 0xc + 2 * i).unwrap_or(0) as i32) >> 7) as f32
        });
        (part_rgba, tail, 1., float(ctx, cursor + 0x14))
    };
    let divisor = if steps <= 0 { 1e-6 } else { steps as f32 };
    let colour_step = sub(colour, tail).map(|c| c / divisor);
    let mut size = size0;
    let mut spacing = float(ctx, cursor + spacing_at);
    let size_step = (size0 - size_end) / steps as f32;
    let spacing_step = size_step / size0 * spacing;
    let mut count = if spacing == 0. { 0 } else { n };
    let n_ring = part.u8(o) as usize;
    if n_ring == 0 {
        return;
    }
    let head = part.u8(o + 1) as usize;
    let ring = |i: usize| part.vec4(o + RING + 16 * i);
    let next = |i: usize| if i != n_ring - 1 { i + 1 } else { 0 };
    let mut point = ring(head);
    let mut start = ring(head);
    let mut end_index = next(head);
    let mut end = ring(end_index);
    let mut segment = sub(end, start);
    let mut reach = length(segment);
    let mut seg_len = reach;
    let mut along = spacing;
    let index = part.i16(o + 6) as u16 as usize;
    let to_view = if byte(ctx, cursor + world_flag(p.kind)) == 0 {
        ctx.ed60
    } else {
        ctx.view
    };
    let axis_x = math::scale(part.cur[0], ctx.scales[0]);
    let axis_y = math::scale(part.cur[1], ctx.scales[1]);
    let mut skip_first = wide && byte(ctx, cursor + 0x22) == 0;
    'sprites: loop {
        if skip_first {
            skip_first = false;
        } else {
            let rgba = colour.map(|c| c as i64 as u8);
            let keep_w = |v: Vec4| [v[0] * size, v[1] * size, v[2] * size, v[3]];
            let w: Mat = [
                keep_w(axis_x),
                keep_w(axis_y),
                [0.; 4],
                math::apply(&to_view, [point[0], point[1], point[2], 1.]),
            ];
            let w = math::mul(&ctx.ede0, &w);
            emit_sprites(ctx, anim, index, &w, rgba);
        }
        count = count.wrapping_sub(1);
        if count == 0 {
            break;
        }
        colour = sub(colour, colour_step);
        spacing -= spacing_step;
        size -= size_step;
        if spacing <= 0. {
            break;
        }
        if reach < spacing {
            loop {
                let kx = next(end_index);
                if kx == head {
                    break 'sprites;
                }
                start = end;
                end = ring(kx);
                segment = sub(end, start);
                along -= seg_len;
                seg_len = length(segment);
                reach += seg_len;
                end_index = kx;
                if reach >= spacing {
                    break;
                }
            }
        }
        let t = along / seg_len;
        point = std::array::from_fn(|i| start[i] + segment[i] * t);
        along += spacing;
        reach -= spacing;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn each_tail_reads_its_own_world_flag() {
        assert_eq!(world_flag(TAIL_X), 0x21);
        assert_eq!(world_flag(TAIL_2), 0x23);
        assert_eq!(world_flag(TAIL_2X), 0x26);
    }
}
