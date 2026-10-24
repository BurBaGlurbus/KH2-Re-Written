//! Model programs of the ppp VM, translated from their decompilations:
//! pppVtMime (49: update FUN_1404c1940, draw FUN_1404c17b0), pppMatrixZXY
//! (91: FUN_1404c48c0), pppDrawMatrix (112: FUN_1404c6450), pppDrawMdl3
//! (137: draw FUN_1404c8660), pppDrawMdlTs3 (139: update FUN_1404c8da0,
//! draw FUN_1404c8b10, create FUN_1404c8ea0) and pppVertexAp (174: update
//! FUN_1404d3b30, create FUN_1404d3d10); pppDrawMatrixLoop (217:
//! FUN_1404c6470, the same code as pppDrawMatrix), pppDrawMdlSemi3 (138:
//! draw FUN_1404c88c0), pppDrawMdlLoopZ (219: update FUN_1404c9350, draw
//! FUN_1404c9540, create FUN_1404c94e0, restart FUN_1404c9510) and
//! pppSnoMdlDttNorm (238: draw FUN_1404cbba0, create/restart
//! FUN_1404cbb60/FUN_1404cbb80).
//!
//! The exe's model draw (FUN_1404dc760) is not run here: a [`ModelDraw`]
//! carries what it reads, and the presenting side expands the OMD meshes.
//!
//! Uncertain:
//! - pppVertexAp's birth parent handle is not kept, so a child's
//!   pppParMatrix has nothing to read (no 613 effect before el7 needs it).
//! - The "extra matrix" (key byte +0xa, ed60·cur) and the b80/b81 depth
//!   state tweaks only steer the exe's GS state and are not represented.
//! - Global pause flags (`DAT_142b0efc0`, instance +0xc1) and
//!   `DAT_142b0efe4` are taken as clear.
//! - pppSnoMdlDttNorm's draw (FUN_1404cad20) redraws the model with its
//!   vertices pushed along their normals by the u0 value, textured from the
//!   frame buffer (a refraction "heat haze"). Only its value integrator is
//!   run here; the frame-buffer pass has no [`ModelDraw`] form.
use crate::math::{self, Mat};
use crate::programs::{Birth, active, byte, float, key, read_matrix, use_val, word};
use crate::{Ctx, ModelDraw, Part};
use data::object_effect::{Program, i16_at};
use std::sync::Arc;

pub(crate) const VT_MIME: u32 = 49;
pub(crate) const MATRIX_ZXY: u32 = 91;
pub(crate) const DRAW_MATRIX: u32 = 112;
pub(crate) const DRAW_MDL3: u32 = 137;
pub(crate) const DRAW_MDL_SEMI3: u32 = 138;
pub(crate) const DRAW_MDL_TS3: u32 = 139;
pub(crate) const VERTEX_AP: u32 = 174;
/// pppVertexApLc (FUN_1404d3e00): VertexAp with raw (untransformed) vertices.
pub(crate) const VERTEX_AP_LC: u32 = 177;
pub(crate) const DRAW_MATRIX_LOOP: u32 = 217;
/// pppDrawMdlLoop (218): the scroll update FUN_1404c8f00 (the code of
/// FUN_1404c9350) and draw FUN_1404c90f0.
pub(crate) const DRAW_MDL_LOOP: u32 = 218;
pub(crate) const DRAW_MDL_LOOP_Z: u32 = 219;
/// pppVertexApLcLoop (236): the table points at FUN_1404d3e00 like 177.
pub(crate) const VERTEX_AP_LC_LOOP: u32 = 236;
pub(crate) const SNO_MDL_DTT_NORM: u32 = 238;

/// x86 `cvttss2si`: out-of-range and NaN give 0x80000000.
pub(crate) fn cvtt(f: f32) -> i32 {
    if f.is_nan() || f >= 2_147_483_648. || f < -2_147_483_648. {
        i32::MIN
    } else {
        f as i32
    }
}

pub(crate) fn create(p: &Program, part: &mut Part, ctx: &mut Ctx) {
    match p.kind {
        VT_MIME => {
            let o = use_val(ctx, p, 0);
            part.zero(o, 12);
            part.vertex_override = None;
        }
        DRAW_MDL_TS3 => {
            let o = use_val(ctx, p, 2);
            part.zero(o, 0x18);
        }
        VERTEX_AP | VERTEX_AP_LC => {
            let o = use_val(ctx, p, 0);
            part.zero(o, 4);
        }
        // FUN_1404c94e0: no scroll set-up yet.
        DRAW_MDL_LOOP_Z => {
            let o = use_val(ctx, p, 2);
            part.set_i32(o, 0);
        }
        SNO_MDL_DTT_NORM => {
            let o = use_val(ctx, p, 0);
            part.zero(o, 12);
        }
        _ => {}
    }
}

/// Restart hooks (table +0x40) of the model programs.
pub(crate) fn restart(p: &Program, part: &mut Part, ctx: &mut Ctx) {
    match p.kind {
        // FUN_1404c1a40: the morph integrator, without the vertex handle.
        VT_MIME => {
            let o = use_val(ctx, p, 0);
            part.zero(o, 12);
        }
        // FUN_1404c9510: the scroll velocities and accelerations.
        DRAW_MDL_LOOP_Z => {
            let o = use_val(ctx, p, 2);
            part.zero(o + 8, 8);
            part.zero(o + 0x14, 8);
        }
        DRAW_MDL_TS3 | VERTEX_AP_LC | SNO_MDL_DTT_NORM => create(p, part, ctx),
        _ => {}
    }
}

pub(crate) fn update(p: &Program, cursor: usize, part: &mut Part, ctx: &mut Ctx) {
    match p.kind {
        VT_MIME => vt_mime_update(p, cursor, part, ctx),
        MATRIX_ZXY => {
            let (u0, u1, u2) = (use_val(ctx, p, 0), use_val(ctx, p, 1), use_val(ctx, p, 2));
            let angles = [part.i32(u1), part.i32(u1 + 4), part.i32(u1 + 8)];
            let mut m = math::rot_zxy(angles);
            for (c, col) in m.iter_mut().take(3).enumerate() {
                *col = math::scale(*col, part.f32(u2 + 4 * c));
            }
            m[3] = [part.f32(u0), part.f32(u0 + 4), part.f32(u0 + 8), m[3][3]];
            part.cur = m;
        }
        DRAW_MATRIX | DRAW_MATRIX_LOOP => {
            // FUN_1404c6450: ede0 · (ec60 · ec20) · cur.
            part.draw = math::mul(&ctx.ede0, &math::mul(&ctx.ed60, &part.cur));
        }
        DRAW_MDL_TS3 => ts3_update(p, cursor, part, ctx),
        DRAW_MDL_LOOP | DRAW_MDL_LOOP_Z => loop_z_update(p, cursor, part, ctx),
        VERTEX_AP | VERTEX_AP_LC | VERTEX_AP_LC_LOOP => vertex_ap(p, cursor, part, ctx),
        _ => {}
    }
}

pub(crate) fn draw(p: &Program, cursor: usize, part: &mut Part, ctx: &mut Ctx) {
    match p.kind {
        VT_MIME => vt_mime_draw(p, cursor, part, ctx),
        DRAW_MDL3 => {
            let u0 = use_val(ctx, p, 0);
            let b = (byte(ctx, cursor + 0xc) != 0).then(|| read_matrix(part, use_val(ctx, p, 2)));
            // Key byte +0xd: a depth-only draw (FUN_1404c8660 → descriptor
            // +0x80, FUN_1404dc760).
            let depth_only = byte(ctx, cursor + 0xd) != 0;
            let n = ctx.model_draws.len();
            push_draw(ctx, part, word(ctx, cursor + 4), u0, b, [0, 0], None);
            if let Some(d) = ctx.model_draws.get_mut(n) {
                d.depth_only = depth_only;
            }
        }
        // FUN_1404c88c0: DrawMdl3 with its flags at +0xa..+0xe and the
        // draw-mode word +8 in place of the model's.
        DRAW_MDL_SEMI3 => {
            let u0 = use_val(ctx, p, 0);
            let b = (byte(ctx, cursor + 0xe) != 0).then(|| read_matrix(part, use_val(ctx, p, 2)));
            let mode = i16_at(&ctx.dpd.bytes, cursor + 8).unwrap_or(0) as u16;
            push_draw(ctx, part, word(ctx, cursor + 4), u0, b, [0, 0], Some(mode));
        }
        // FUN_1404c9540: the u2 scroll pair as cvttss2si(x · 65536), the
        // mode word +0x20 unless its low byte is 0xff.
        DRAW_MDL_LOOP_Z => {
            let u0 = use_val(ctx, p, 0);
            let s = use_val(ctx, p, 2);
            let b = (byte(ctx, cursor + 0x26) != 0).then(|| read_matrix(part, use_val(ctx, p, 3)));
            let mode = (byte(ctx, cursor + 0x20) != 0xff)
                .then(|| i16_at(&ctx.dpd.bytes, cursor + 0x20).unwrap_or(0) as u16);
            let scroll = [
                cvtt(part.f32(s + 4) * 65536.),
                cvtt(part.f32(s + 0x10) * 65536.),
            ];
            push_draw(ctx, part, word(ctx, cursor + 4), u0, b, scroll, mode);
        }
        // FUN_1404c90f0: the same scroll pair; the extra matrix (byte +0x25)
        // is the parent × cur, the mode word +0x20 unless its low byte is 0xff.
        DRAW_MDL_LOOP => {
            let u0 = use_val(ctx, p, 0);
            let s = use_val(ctx, p, 2);
            let b = (byte(ctx, cursor + 0x25) != 0).then(|| math::mul(&ctx.ed60, &part.cur));
            let mode = (byte(ctx, cursor + 0x20) != 0xff)
                .then(|| i16_at(&ctx.dpd.bytes, cursor + 0x20).unwrap_or(0) as u16);
            let scroll = [
                cvtt(part.f32(s + 4) * 65536.),
                cvtt(part.f32(s + 0x10) * 65536.),
            ];
            push_draw(ctx, part, word(ctx, cursor + 4), u0, b, scroll, mode);
        }
        SNO_MDL_DTT_NORM => sno_integrate(p, cursor, part, ctx),
        DRAW_MDL_TS3 => {
            let u0 = use_val(ctx, p, 0);
            let s = use_val(ctx, p, 2);
            let b = (byte(ctx, cursor + 0x24) != 0).then(|| read_matrix(part, use_val(ctx, p, 3)));
            let scroll = [cvtt(part.f32(s) * 65536.), cvtt(part.f32(s + 12) * 65536.)];
            push_draw(ctx, part, word(ctx, cursor + 4), u0, b, scroll, None);
        }
        _ => {}
    }
}

pub(crate) fn push_draw(
    ctx: &mut Ctx,
    part: &Part,
    model: u32,
    u0: usize,
    b: Option<Mat>,
    scroll: [i32; 2],
    mode: Option<u16>,
) {
    let alpha = part.u8(u0 + 0xb);
    if model == 0xffff || alpha == 0 || !ctx.dpd.models.contains_key(&(model as i16)) {
        return;
    }
    ctx.model_draws.push(ModelDraw {
        model: model as i16,
        vertices: part.vertex_override.clone(),
        a: part.draw,
        b,
        rgba: std::array::from_fn(|i| part.u8(u0 + 8 + i)),
        scroll,
        mode,
        depth_only: false,
    });
}

fn vt_mime_update(p: &Program, cursor: usize, part: &mut Part, ctx: &mut Ctx) {
    if !active(part) {
        return;
    }
    let u0 = use_val(ctx, p, 0);
    let acc = part.f32(u0 + 8);
    let rate = part.f32(u0 + 4) + acc;
    part.set_f32(u0 + 4, rate);
    let t = part.f32(u0) + rate;
    part.set_f32(u0, t);
    if key(ctx, part, cursor) {
        let t = part.f32(u0) + float(ctx, cursor + 0xc);
        part.set_f32(u0, t);
        // 0xff7fffff (-FLT_MAX) means "keep the rate".
        let r = float(ctx, cursor + 0x10);
        if r as f64 >= -1.0e38 {
            let rate = part.f32(u0 + 4) + r;
            part.set_f32(u0 + 4, rate);
        }
        part.set_f32(u0 + 8, acc + float(ctx, cursor + 0x14));
    }
}

fn model_vertex(ctx: &Ctx, model: u32, i: usize) -> [i16; 3] {
    ctx.dpd.vertex(model as i16, i as u16).unwrap_or([0; 3])
}

fn vt_mime_draw(p: &Program, cursor: usize, part: &mut Part, ctx: &mut Ctx) {
    let (a, b) = (word(ctx, cursor + 4), word(ctx, cursor + 8));
    if a == 0xffff && b == 0xffff {
        return;
    }
    let t = part.f32(use_val(ctx, p, 0));
    let n = ctx
        .dpd
        .models
        .get(&(a as i16))
        .map_or(0, |m| m.vertex_count);
    let morphed = (0..n)
        .map(|i| {
            let (va, vb) = (model_vertex(ctx, a, i), model_vertex(ctx, b, i));
            std::array::from_fn(|k| {
                let d = (vb[k] as i32 - va[k] as i32) as f32;
                cvtt(d * t + va[k] as f32) as i16
            })
        })
        .collect();
    part.vertex_override = Some(Arc::new(morphed));
}

/// FUN_1404c8da0: U/V scroll integrators in the u2 block
/// (U, Uvel, Uacc, V, Vvel, Vacc), keyed adds at the part frame.
pub(crate) fn ts3_update(p: &Program, cursor: usize, part: &mut Part, ctx: &mut Ctx) {
    if !active(part) {
        return;
    }
    let s = use_val(ctx, p, 2);
    let f: [f32; 6] = std::array::from_fn(|i| part.f32(s + 4 * i));
    let mut f = f;
    f[1] += f[2];
    f[4] += f[5];
    f[0] += f[1];
    f[3] += f[4];
    if key(ctx, part, cursor) {
        for (i, v) in f.iter_mut().enumerate() {
            *v += float(ctx, cursor + 8 + 4 * i);
        }
    }
    for (i, v) in f.into_iter().enumerate() {
        part.set_f32(s + 4 * i, v);
    }
}

/// FUN_1404c9350: the u2 block (+0 first key, +4 U, +8 U rate, +0xc U
/// acceleration, +0x10 V, +0x14 V rate, +0x18 V acceleration). The first
/// call (on any frame) seeds U/V from key +8/+0x14; keys add to the rates;
/// outside catch-up the rates integrate and U/V wrap at 32768.
fn loop_z_update(p: &Program, cursor: usize, part: &mut Part, ctx: &mut Ctx) {
    let s = use_val(ctx, p, 2);
    if part.i32(s) == 0 {
        part.set_i32(s + 4, word(ctx, cursor + 8) as i32);
        part.set_i32(s + 0x10, word(ctx, cursor + 0x14) as i32);
        part.zero(s + 0x14, 8);
        part.zero(s + 8, 8);
        part.set_i32(s, cursor as i32);
    }
    if !active(part) {
        return;
    }
    if key(ctx, part, cursor) {
        for (o, k) in [(8, 0xc), (0xc, 0x10), (0x14, 0x18), (0x18, 0x1c)] {
            let v = float(ctx, cursor + k) + part.f32(s + o);
            part.set_f32(s + o, v);
        }
    }
    if !ctx.catch_up {
        let u_rate = part.f32(s + 0xc) + part.f32(s + 8);
        let v_rate = part.f32(s + 0x18) + part.f32(s + 0x14);
        part.set_f32(s + 8, u_rate);
        part.set_f32(s + 0x14, v_rate);
        part.set_f32(s + 4, (u_rate + part.f32(s + 4)) % 32768.);
        part.set_f32(s + 0x10, (v_rate + part.f32(s + 0x10)) % 32768.);
    }
}

/// FUN_1404cbba0's integrator (it runs in the draw, every game frame):
/// u0 = value, +4 rate, +8 acceleration; the key adds +8/+0xc/+0x10.
fn sno_integrate(p: &Program, cursor: usize, part: &mut Part, ctx: &mut Ctx) {
    let o = use_val(ctx, p, 0);
    let acc = part.f32(o + 8);
    let rate = acc + part.f32(o + 4);
    let value = rate + part.f32(o);
    let (mut v, mut r, mut a) = (value, rate, acc);
    if key(ctx, part, cursor) {
        v += float(ctx, cursor + 8);
        r += float(ctx, cursor + 0xc);
        a += float(ctx, cursor + 0x10);
    }
    part.set_f32(o, v);
    part.set_f32(o + 4, r);
    part.set_f32(o + 8, a);
}

/// FUN_1404d3b30 (VertexAp) / FUN_1404d3e00 (VertexApLc: raw vertices):
/// every `interval` ticks `births` children of entry +0xc start at VSF
/// vertices (sequential or random), the vertex through the emitter matrix.
fn vertex_ap(p: &Program, cursor: usize, part: &mut Part, ctx: &mut Ctx) {
    let vsf_id = i16_at(&ctx.dpd.bytes, cursor + 4).unwrap_or(-1);
    if !active(part) || vsf_id < 0 {
        return;
    }
    let u0 = use_val(ctx, p, 0);
    let mut countdown = part.i16(u0 + 2) as u16;
    if countdown == 0 {
        let births = byte(ctx, cursor + 6);
        let interval = byte(ctx, cursor + 7);
        let random = byte(ctx, cursor + 8) != 0;
        let child = word(ctx, cursor + 0xc) as i32;
        let var = word(ctx, cursor + 0x10) as usize;
        if let Some(vsf) = ctx.dpd.vsf.get(&(vsf_id as i32)) {
            let count = vsf.indices.len();
            for _ in 0..births {
                if count == 0 {
                    break;
                }
                let k = if random {
                    (count as f32 * ctx.rng.unit()) as usize % count
                } else {
                    let c = part.i16(u0) as u16 as usize;
                    let c = if c >= count { 0 } else { c };
                    part.set_i16(u0, (c + 1) as i16);
                    c
                };
                let index = vsf.indices[k];
                let v = match &part.vertex_override {
                    Some(m) => m.get(index as usize).copied(),
                    None => ctx.dpd.vertex(vsf.model, index),
                };
                let Some(v) = v else { continue };
                if child < 0 {
                    continue;
                }
                let local = [v[0] as f32, v[1] as f32, v[2] as f32, 1.];
                let at = if p.kind == VERTEX_AP {
                    math::apply(&part.cur, local)
                } else {
                    local
                };
                ctx.births.push(Birth {
                    entry: child as usize,
                    position_var: var,
                    position: at,
                    position_len: 3,
                    velocity_var: None,
                    velocity: [0.; 4],
                    ints: Vec::new(),
                });
            }
        }
        countdown = interval as u16;
    }
    part.set_i16(u0 + 2, countdown.wrapping_sub(1) as i16);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cvtt_matches_cvttss2si_edges() {
        assert_eq!(cvtt(1.9), 1);
        assert_eq!(cvtt(-1.9), -1);
        assert_eq!(cvtt(f32::MAX), i32::MIN);
        assert_eq!(cvtt(f32::NAN), i32::MIN);
        assert_eq!(cvtt(-3.0e9), i32::MIN);
        // The VtMime overflow case: the low 16 bits of 0x80000000 are 0.
        assert_eq!(cvtt(f32::INFINITY) as i16, 0);
    }
}
