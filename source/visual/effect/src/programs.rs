//! Program functions from the table at 0x140779a00 (0x50-byte records:
//! update +8, draw +0x18, create +0x38, destroy +0x48), translated from
//! their decompilations. `V[o]` is work offset 0xa0 + o (`Part::vars[o]`);
//! `u[k]` is the program's k-th work-variable offset.
use crate::math::{self, Mat, Vec4};
use crate::models;
use crate::{Ctx, Part, Quad, key_i32};
use data::object_effect::{self, Program, f32_at, i16_at, u32_at};

pub(crate) struct Birth {
    pub entry: usize,
    pub position_var: usize,
    pub position: Vec4,
    /// Floats written at `position_var` (3 = xyz only, 4 = with w).
    pub position_len: usize,
    pub velocity_var: Option<usize>,
    pub velocity: Vec4,
    /// Further i32 words the birth writes into the child (work offset, value).
    pub ints: Vec<(usize, i32)>,
}

/// pppKeThRes16 / 16x4 / 32: create only (bolt node pools for pppKeTh).
const KE_TH_RES8: u32 = 3;
const KE_TH_RES24: u32 = 9;
const KE_TH_RES40: u32 = 14;
const KE_TH_RES16: u32 = 6;
const KE_TH_RES16X4: u32 = 7;
const KE_TH_RES32: u32 = 11;
const ACCELE: u32 = 28;
const ANG_ACCELE: u32 = 29;
const SCL_ACCELE: u32 = 30;
const COL_ACCELE: u32 = 31;
const MOVE: u32 = 36;
const ANG_MOVE: u32 = 37;
const SCL_MOVE: u32 = 38;
const COL_MOVE: u32 = 39;
const POINT: u32 = 40;
const ANGLE: u32 = 41;
const SCALE: u32 = 42;
const COLOR: u32 = 43;
const KE_DRCT: u32 = 48;
const RAND_FV: u32 = 63;
const RAND_IV: u32 = 66;
const S_RAND_FV: u32 = 75;
const S_RAND_DOWN_FV: u32 = 77;
const S_MATRIX: u32 = 84;
const MATRIX_XYZ: u32 = 87;
const MATRIX_YXZ: u32 = 90;
const MATRIX_LOC: u32 = 93;
const MATRIX_SCL: u32 = 94;
const PAR_MATRIX: u32 = 105;
const KE_PAR_MAT_R: u32 = 106;
const DRAW_MATRIX_FRONT: u32 = 113;
const DRAW_MATRIX_WOOD: u32 = 115;
const KE_D_MAT: u32 = 116;
const KE_D_MAT_FR: u32 = 117;
const KE_Z_CRCT: u32 = 124;
const KE_Z_CRCT_SHP: u32 = 125;
const KE_TH_TP: u32 = 126;
const KE_TH_SFT: u32 = 128;
const DRAW_SHAPE_X: u32 = 142;
const POINT_AP: u32 = 172;
const KE_BORN_RND3: u32 = 181;
const KE_BORN_RND5: u32 = 183;
const KE_BORN_RND6: u32 = 184;
pub(crate) const MEGA_BIRTH: u32 = 201;
pub(crate) const MNG_FLAG: u32 = 202;
pub(crate) const MEGA_BIRTH_MODEL: u32 = 208;
const POINT_LOOP: u32 = 213;
const ANGLE_LOOP: u32 = 214;
const SCALE_LOOP: u32 = 215;
const MATRIX_LOOP: u32 = 216;
const KE_MV_YP_EFF: u32 = 47;
const RAND_FLOAT: u32 = 51;
const RAND_DOWN_FV: u32 = 65;
const KE_MAT_SN: u32 = 85;
const MATRIX_YZX: u32 = 89;
const MATRIX_NO_ROT: u32 = 96;
const DRAW_MATRIX_WORLD: u32 = 120;
const DRAW_MATRIX_WORLD_FRONT: u32 = 121;
const DRAW_MATRIX_WORLD_WOOD: u32 = 123;
const ANG_MOVE_LOOP: u32 = 211;
const SCL_MOVE_LOOP: u32 = 212;
const MATRIX_LOOP_ZXY: u32 = 234;
/// pppMatrixLoopYXZ (FUN_1404c4d20): the same code as pppMatrixYXZ.
const MATRIX_LOOP_YXZ: u32 = 232;

/// Whether the table record has an update function (+8).
pub(crate) fn has_update(kind: u32) -> bool {
    !crate::extra::without_update(kind)
        && kind != crate::shipoly::KIND
        && !matches!(
            kind,
            KE_TH_RES8
                | KE_TH_RES24
                | KE_TH_RES40
                | KE_TH_RES16
                | KE_TH_RES16X4
                | KE_TH_RES32
                | models::DRAW_MDL3
                | models::DRAW_MDL_SEMI3
                | models::SNO_MDL_DTT_NORM
        )
}

pub(crate) fn has_draw(kind: u32) -> bool {
    crate::extra::draws(kind)
        || crate::lens::draws(kind)
        || crate::refraction::handles(kind)
        || matches!(
            kind,
            DRAW_SHAPE_X
                | MEGA_BIRTH
                | MEGA_BIRTH_MODEL
                | models::VT_MIME
                | models::DRAW_MDL3
                | models::DRAW_MDL_TS3
                | models::DRAW_MDL_SEMI3
                | models::DRAW_MDL_LOOP
                | models::DRAW_MDL_LOOP_Z
                | models::SNO_MDL_DTT_NORM
                | crate::tail::KIND
                | crate::tail2::TAIL_X
                | crate::tail2::TAIL_2X
                | crate::shipoly::KIND
                | crate::thunder::KIND
                | crate::ribbon::KIND
        )
}

/// The update functions listed at PTR_FUN_140779990, which a loop
/// restart's catch-up (FUN_1404b0550) does not run.
pub(crate) fn skips_catch_up(kind: u32) -> bool {
    matches!(kind, MEGA_BIRTH | MEGA_BIRTH_MODEL | crate::ribbon::KIND)
}

pub(crate) fn use_val(ctx: &Ctx, p: &Program, k: usize) -> usize {
    key_i32(ctx.dpd, p.use_val + 4 * k) as usize
}

pub(crate) fn word(ctx: &Ctx, at: usize) -> u32 {
    u32_at(&ctx.dpd.bytes, at).unwrap_or(0)
}

pub(crate) fn float(ctx: &Ctx, at: usize) -> f32 {
    f32_at(&ctx.dpd.bytes, at).unwrap_or(0.)
}

pub(crate) fn byte(ctx: &Ctx, at: usize) -> u8 {
    ctx.dpd.bytes.get(at).copied().unwrap_or(0)
}

/// Updates run only on even part frames outside pause (every program
/// checks `(frame & 1) == 0`).
pub(crate) fn active(part: &Part) -> bool {
    part.frame & 1 == 0
}

pub(crate) fn key(ctx: &Ctx, part: &Part, cursor: usize) -> bool {
    word(ctx, cursor) == part.frame
}

pub(crate) fn create(p: &Program, part: &mut Part, ctx: &mut Ctx) {
    match p.kind {
        // Integrators clear their rate variable u1.
        ACCELE | ANG_ACCELE | SCL_ACCELE | MOVE | ANG_MOVE | SCL_MOVE => {
            let o = use_val(ctx, p, 1);
            part.zero(o, 12);
        }
        COL_ACCELE | COL_MOVE => {
            let o = use_val(ctx, p, 1);
            part.zero(o, 8);
        }
        POINT | ANGLE | SCALE => {
            let o = use_val(ctx, p, 0);
            part.zero(o, 12);
        }
        COLOR => {
            let o = use_val(ctx, p, 0);
            part.zero(o, 8);
        }
        DRAW_SHAPE_X => {
            let o = use_val(ctx, p, 0);
            part.zero(o, 6);
        }
        // FUN_1404d5260: cursor u16 and countdown u8 cleared.
        KE_BORN_RND5 | KE_BORN_RND6 => {
            let o = use_val(ctx, p, 0);
            part.zero(o, 3);
        }
        MEGA_BIRTH => crate::mega_birth::create(p, part, ctx),
        MEGA_BIRTH_MODEL => crate::mega_model::create(p, part, ctx),
        crate::ribbon::KIND => crate::ribbon::create(p, part, ctx),
        crate::tail::KIND => crate::tail::create(p, part, ctx),
        crate::tail2::TAIL_X | crate::tail2::TAIL_2X => crate::tail2::create(p, part, ctx),
        crate::shipoly::KIND => crate::shipoly::create(p, part),
        crate::thunder::KIND => crate::thunder::create(p, part, ctx),
        k if crate::refraction::handles(k) => crate::refraction::create(p, part, ctx),
        models::VT_MIME
        | models::DRAW_MDL_TS3
        | models::VERTEX_AP
        | models::VERTEX_AP_LC
        | models::DRAW_MDL_LOOP_Z
        | models::SNO_MDL_DTT_NORM => models::create(p, part, ctx),
        // FUN_1404bf080 / FUN_1404bf0a0 / FUN_1404bf160: pool header (node
        // count, points per bolt, node size) and the free list of nodes from
        // +0x10; the exe's handles are stood in by work offsets + 1.
        KE_TH_RES8 => ke_th_res(part, use_val(ctx, p, 0), 1, 8, 0x1e0),
        KE_TH_RES24 => ke_th_res(part, use_val(ctx, p, 0), 1, 0x18, 0x4e0),
        KE_TH_RES40 => ke_th_res(part, use_val(ctx, p, 0), 1, 0x28, 0x7e0),
        KE_TH_RES16 => ke_th_res(part, use_val(ctx, p, 0), 1, 0x10, 0x360),
        KE_TH_RES16X4 => ke_th_res(part, use_val(ctx, p, 0), 4, 0x10, 0x360),
        KE_TH_RES32 => ke_th_res(part, use_val(ctx, p, 0), 1, 0x20, 0x660),
        // FUN_1404d37f0: the birth countdown.
        POINT_AP => {
            let o = use_val(ctx, p, 1);
            part.set_u8(o + 1, 0);
        }
        // FUN_1404c1790: the direction's w = 1.
        KE_DRCT => {
            let o = use_val(ctx, p, 0);
            part.set_f32(o + 0xc, 1.);
        }
        // FUN_1404bf440.
        KE_TH_TP => {
            let o = use_val(ctx, p, 0);
            part.set_i32(o + 0x1c, 0);
            part.set_i32(o + 0xc, 0);
        }
        // FUN_1404c78f0: the rate block.
        KE_TH_SFT => {
            let o = use_val(ctx, p, 0);
            part.zero(o, 0x30);
        }
        // FUN_1404d4810: the VSF cursor.
        KE_BORN_RND3 => {
            let o = use_val(ctx, p, 0);
            part.zero(o, 2);
        }
        // FUN_1404c4440: the previous angles of pppKeMatSN.
        KE_MAT_SN => {
            let o = use_val(ctx, p, 0);
            part.zero(o + 0x80, 12);
        }
        // FUN_1404c06c0 / FUN_1404c07f0: the loop move rate.
        ANG_MOVE_LOOP | SCL_MOVE_LOOP | crate::extra::MOVE_LOOP => {
            let o = use_val(ctx, p, 1);
            part.zero(o, 12);
        }
        crate::extra::SNO_SCALE_ALL
        | crate::extra::DRAW_MDL_TS
        | crate::extra::VERTEX_AP_AT
        | crate::extra::MATRIX_WORLD_XYZ => crate::extra::create(p, part, ctx),
        k if crate::lens::handles(k) => crate::lens::create(p, part, ctx),
        // FUN_1404c6b90 / FUN_1404c6ed0: the position and the axis lengths.
        DRAW_MATRIX_WORLD_FRONT | DRAW_MATRIX_WORLD_WOOD => world_create(p, part, ctx),
        // FUN_1404c0a70 / FUN_1404c0cd0 / FUN_1404c0eb0: no first key yet.
        POINT_LOOP | ANGLE_LOOP | SCALE_LOOP => {
            let o = use_val(ctx, p, 0);
            part.set_i32(o + 0xc, 0);
        }
        _ => {}
    }
}

/// Restart hooks (table +0x40), run by FUN_1404b0120 when a loop range
/// sends a part back to frame 0. Most repeat their create function.
pub(crate) fn restart(p: &Program, part: &mut Part, ctx: &mut Ctx) {
    match p.kind {
        ACCELE
        | ANG_ACCELE
        | SCL_ACCELE
        | MOVE
        | ANG_MOVE
        | SCL_MOVE
        | COL_ACCELE
        | COL_MOVE
        | POINT
        | ANGLE
        | SCALE
        | COLOR
        | DRAW_SHAPE_X
        | KE_TH_SFT
        | ANG_MOVE_LOOP
        | SCL_MOVE_LOOP
        | crate::extra::MOVE_LOOP
        | crate::extra::SNO_SCALE_ALL => create(p, part, ctx),
        POINT_LOOP | ANGLE_LOOP | SCALE_LOOP => loop_restart(p, part, ctx),
        crate::ribbon::KIND => crate::ribbon::restart(p, part, ctx),
        crate::tail::KIND => crate::tail::restart(p, part, ctx),
        crate::thunder::KIND => crate::thunder::restart(p, part, ctx),
        k if crate::lens::handles(k) => crate::lens::restart(p, part, ctx),
        k if crate::refraction::handles(k) => crate::refraction::create(p, part, ctx),
        models::VT_MIME
        | models::DRAW_MDL_TS3
        | models::VERTEX_AP_LC
        | models::DRAW_MDL_LOOP_Z
        | models::SNO_MDL_DTT_NORM => models::restart(p, part, ctx),
        _ => {}
    }
}

pub(crate) fn destroy(p: &Program, part: &mut Part, ctx: &mut Ctx) {
    match p.kind {
        MEGA_BIRTH => crate::mega_birth::destroy(p, part, ctx),
        MEGA_BIRTH_MODEL => crate::mega_model::destroy(p, part, ctx),
        crate::ribbon::KIND => crate::ribbon::destroy(p, part, ctx),
        crate::shipoly::KIND => crate::shipoly::destroy(p, part),
        _ => {}
    }
}

pub(crate) fn update(p: &Program, cursor: usize, part: &mut Part, ctx: &mut Ctx) {
    match p.kind {
        ACCELE | SCL_ACCELE | MOVE | SCL_MOVE => integrate_f32(p, cursor, part, ctx),
        ANG_ACCELE | ANG_MOVE => integrate_i32(p, cursor, part, ctx),
        COL_ACCELE | COL_MOVE => integrate_i16(p, cursor, part, ctx),
        POINT | SCALE => {
            if active(part) && key(ctx, part, cursor) {
                let o = use_val(ctx, p, 0);
                for i in 0..3 {
                    let v = part.f32(o + 4 * i) + float(ctx, cursor + 4 + 4 * i);
                    part.set_f32(o + 4 * i, v);
                }
            }
        }
        ANGLE => {
            if active(part) && key(ctx, part, cursor) {
                let o = use_val(ctx, p, 0);
                for i in 0..3 {
                    let v = part
                        .i32(o + 4 * i)
                        .wrapping_add(word(ctx, cursor + 4 + 4 * i) as i32);
                    part.set_i32(o + 4 * i, v);
                }
            }
        }
        COLOR => color(p, cursor, part, ctx),
        RAND_FV => rand_fv(p, cursor, part, ctx),
        RAND_IV => rand_iv(p, cursor, part, ctx),
        // FUN_1404c4460; pppMatrixLoop FUN_1404c4540 is the same code.
        MATRIX_XYZ | MATRIX_LOOP => matrix_from(p, part, ctx, math::rot_xyz),
        // FUN_1404c47e0 with FUN_1404e0150 (Rz·Rx·Ry).
        MATRIX_YXZ | MATRIX_LOOP_YXZ => matrix_from(p, part, ctx, math::rot_yxz),
        // FUN_1404c4fc0: identity with translation u0.
        MATRIX_LOC => {
            let u0 = use_val(ctx, p, 0);
            let mut m = math::IDENTITY;
            m[3] = [part.f32(u0), part.f32(u0 + 4), part.f32(u0 + 8), 1.];
            part.cur = m;
        }
        DRAW_MATRIX_WOOD => draw_matrix_wood(part, ctx),
        KE_D_MAT => {
            // FUN_1404c66f0: view·world·cur, kept in u1, then draw space.
            let d = math::mul(&ctx.ed60, &part.cur);
            write_matrix(part, use_val(ctx, p, 1), &d);
            part.draw = math::mul(&ctx.ede0, &d);
        }
        // FUN_1404c1750: at its key (no tick gate) u0 xyz = key words.
        KE_DRCT => {
            if key(ctx, part, cursor) {
                let o = use_val(ctx, p, 0);
                for i in 0..3 {
                    part.set_i32(o + 4 * i, word(ctx, cursor + 4 + 4 * i) as i32);
                }
            }
        }
        S_RAND_DOWN_FV => s_rand_down_fv(p, cursor, part, ctx),
        POINT_LOOP | ANGLE_LOOP | SCALE_LOOP => loop_add(p, cursor, part, ctx),
        KE_TH_TP => ke_th_tp(p, cursor, part, ctx),
        KE_TH_SFT => ke_th_sft(p, cursor, part, ctx),
        KE_BORN_RND3 => born_rnd3(p, cursor, part, ctx),
        POINT_AP => point_ap(p, cursor, part, ctx),
        KE_MV_YP_EFF => ke_mv_yp_eff(p, part, ctx),
        RAND_FLOAT => rand_float(p, cursor, part, ctx),
        RAND_DOWN_FV => rand_down_fv(p, cursor, part, ctx),
        KE_MAT_SN => ke_mat_sn(p, part, ctx),
        MATRIX_YZX => matrix_from(p, part, ctx, math::rot_yzx),
        MATRIX_LOOP_ZXY => matrix_from(p, part, ctx, math::rot_zxy),
        ANG_MOVE_LOOP => loop_move_i32(p, cursor, part, ctx),
        SCL_MOVE_LOOP | crate::extra::MOVE_LOOP => loop_move_f32(p, cursor, part, ctx),
        k if crate::extra::handles(k) => crate::extra::update(p, cursor, part, ctx),
        k if crate::lens::handles(k) => crate::lens::update(p, cursor, part, ctx),
        MATRIX_NO_ROT => matrix_no_rot(p, part, ctx),
        DRAW_MATRIX_WORLD => draw_matrix_world(p, part, ctx),
        DRAW_MATRIX_WORLD_FRONT => draw_matrix_world_front(p, part, ctx),
        DRAW_MATRIX_WORLD_WOOD => draw_matrix_world_wood(p, part, ctx),
        crate::tail::KIND => crate::tail::update(p, cursor, part, ctx),
        crate::tail2::TAIL_X | crate::tail2::TAIL_2X => crate::tail2::update(p, cursor, part, ctx),
        crate::thunder::KIND => crate::thunder::update(p, cursor, part, ctx),
        MATRIX_SCL => {
            let (u0, u1) = (use_val(ctx, p, 0), use_val(ctx, p, 1));
            let mut m = math::IDENTITY;
            m[0][0] = part.f32(u1);
            m[1][1] = part.f32(u1 + 4);
            m[2][2] = part.f32(u1 + 8);
            m[3] = [part.f32(u0), part.f32(u0 + 4), part.f32(u0 + 8), 1.];
            part.cur = m;
        }
        DRAW_MATRIX_FRONT => {
            // FUN_1404c6490: axes × world scale, translation through
            // view·world, then the draw-space matrix.
            let mut d = part.cur;
            for (col, s) in d.iter_mut().zip(ctx.scales) {
                *col = math::scale(*col, s);
            }
            d[3] = math::apply(&ctx.ed60, part.cur[3]);
            part.draw = math::mul(&ctx.ede0, &d);
        }
        KE_D_MAT_FR => {
            // FUN_1404c6760: DrawMatrixFront, with the pre-ede0 matrix
            // also copied to u1.
            let mut d = part.cur;
            for (col, s) in d.iter_mut().zip(ctx.scales) {
                *col = math::scale(*col, s);
            }
            d[3] = math::apply(&ctx.ed60, part.cur[3]);
            write_matrix(part, use_val(ctx, p, 1), &d);
            part.draw = math::mul(&ctx.ede0, &d);
        }
        S_RAND_FV => s_rand_fv(p, cursor, part, ctx),
        // FUN_1404c4360 only stores a pointer in DAT_142b0efb8, which no
        // other routine of the exe reads.
        S_MATRIX => {}
        PAR_MATRIX => {
            // FUN_1404c60c0: the position into the parent's space.
            if let Some(parent) = ctx.parent_cur {
                part.cur[3] = math::apply(&parent, part.cur[3]);
            }
        }
        KE_PAR_MAT_R => ke_par_mat_r(part, ctx),
        KE_Z_CRCT => ke_z_crct(p, cursor, part, ctx),
        KE_Z_CRCT_SHP => ke_z_crct_shp(p, cursor, part, ctx),
        DRAW_SHAPE_X => shape_advance(p, cursor, part, ctx),
        KE_BORN_RND5 => born_rnd5(p, cursor, part, ctx),
        KE_BORN_RND6 => born_rnd6(p, cursor, part, ctx),
        MEGA_BIRTH => crate::mega_birth::update(p, cursor, part, ctx),
        MEGA_BIRTH_MODEL => crate::mega_model::update(p, cursor, part, ctx),
        MNG_FLAG => crate::mega_model::mng_flag(cursor, part, ctx),
        crate::ribbon::KIND => crate::ribbon::update(p, cursor, part, ctx),
        models::VT_MIME
        | models::MATRIX_ZXY
        | models::DRAW_MATRIX
        | models::DRAW_MATRIX_LOOP
        | models::DRAW_MDL_TS3
        | models::DRAW_MDL_LOOP
        | models::DRAW_MDL_LOOP_Z
        | models::VERTEX_AP
        | models::VERTEX_AP_LC
        | models::VERTEX_AP_LC_LOOP => models::update(p, cursor, part, ctx),
        _ => {
            if !ctx.unsupported.contains(&p.kind) {
                ctx.unsupported.push(p.kind);
            }
        }
    }
}

pub(crate) fn draw(p: &Program, cursor: usize, part: &mut Part, ctx: &mut Ctx) {
    match p.kind {
        DRAW_SHAPE_X => draw_shape(p, cursor, part, ctx),
        MEGA_BIRTH => crate::mega_birth::draw(p, cursor, part, ctx),
        MEGA_BIRTH_MODEL => crate::mega_model::draw(p, cursor, part, ctx),
        crate::ribbon::KIND => crate::ribbon::draw(p, cursor, part, ctx),
        crate::tail::KIND => crate::tail::draw(p, cursor, part, ctx),
        crate::tail2::TAIL_X | crate::tail2::TAIL_2X => crate::tail2::draw(p, cursor, part, ctx),
        crate::shipoly::KIND => crate::shipoly::draw(p, cursor, part, ctx),
        crate::thunder::KIND => crate::thunder::draw(p, cursor, part, ctx),
        k if crate::extra::draws(k) => crate::extra::draw(p, cursor, part, ctx),
        k if crate::lens::draws(k) => crate::lens::draw(p, cursor, part, ctx),
        k if crate::refraction::handles(k) => crate::refraction::draw(p, cursor, part, ctx),
        models::VT_MIME
        | models::DRAW_MDL3
        | models::DRAW_MDL_TS3
        | models::DRAW_MDL_SEMI3
        | models::DRAW_MDL_LOOP
        | models::DRAW_MDL_LOOP_Z
        | models::SNO_MDL_DTT_NORM => models::draw(p, cursor, part, ctx),
        _ => {
            if !ctx.unsupported.contains(&p.kind) {
                ctx.unsupported.push(p.kind);
            }
        }
    }
}

/// FUN_1404bfcf0 family (Accele/Move/SclAccele/SclMove): at a key the rate
/// u1 gains the keyframe vector; every tick the value u0 gains the rate.
fn integrate_f32(p: &Program, cursor: usize, part: &mut Part, ctx: &mut Ctx) {
    if !active(part) {
        return;
    }
    let (u0, u1) = (use_val(ctx, p, 0), use_val(ctx, p, 1));
    let hit = key(ctx, part, cursor);
    for i in 0..3 {
        if hit {
            let r = part.f32(u1 + 4 * i) + float(ctx, cursor + 4 + 4 * i);
            part.set_f32(u1 + 4 * i, r);
        }
        let v = part.f32(u0 + 4 * i) + part.f32(u1 + 4 * i);
        part.set_f32(u0 + 4 * i, v);
    }
}

fn integrate_i32(p: &Program, cursor: usize, part: &mut Part, ctx: &mut Ctx) {
    if !active(part) {
        return;
    }
    let (u0, u1) = (use_val(ctx, p, 0), use_val(ctx, p, 1));
    let hit = key(ctx, part, cursor);
    for i in 0..3 {
        if hit {
            let r = part
                .i32(u1 + 4 * i)
                .wrapping_add(word(ctx, cursor + 4 + 4 * i) as i32);
            part.set_i32(u1 + 4 * i, r);
        }
        let v = part.i32(u0 + 4 * i).wrapping_add(part.i32(u1 + 4 * i));
        part.set_i32(u0 + 4 * i, v);
    }
}

fn integrate_i16(p: &Program, cursor: usize, part: &mut Part, ctx: &mut Ctx) {
    if !active(part) {
        return;
    }
    let (u0, u1) = (use_val(ctx, p, 0), use_val(ctx, p, 1));
    let hit = key(ctx, part, cursor);
    for i in 0..4 {
        if hit {
            let k = i16_at(&ctx.dpd.bytes, cursor + 4 + 2 * i).unwrap_or(0);
            let r = part.i16(u1 + 2 * i).wrapping_add(k);
            part.set_i16(u1 + 2 * i, r);
        }
        let v = part.i16(u0 + 2 * i).wrapping_add(part.i16(u1 + 2 * i));
        part.set_i16(u0 + 2 * i, v);
    }
}

/// FUN_1404c0f60: keyed RGBA16 adds, then every call the 8-bit colour
/// bytes at u0+8: (u16 value × instance colour), clamped to 32767, /128.
fn color(p: &Program, cursor: usize, part: &mut Part, ctx: &mut Ctx) {
    let u0 = use_val(ctx, p, 0);
    if active(part) && key(ctx, part, cursor) {
        for i in 0..4 {
            let k = i16_at(&ctx.dpd.bytes, cursor + 4 + 2 * i).unwrap_or(0);
            let v = part.i16(u0 + 2 * i).wrapping_add(k);
            part.set_i16(u0 + 2 * i, v);
        }
    }
    for i in 0..4 {
        let c = (part.i16(u0 + 2 * i) as u16 as f32 * ctx.colour[i]).min(32767.);
        part.set_u8(u0 + 8 + i, (c * (1. / 128.)) as i32 as u8);
    }
}

/// FUN_1404c2610: at frame 0 draws r (rnd·2, or rnd+rnd when byte +0x14 is
/// set) into u0; at frame 0 or a key, target V[p1] (−1: scratch) gains
/// p·r − p per axis.
fn rand_fv(p: &Program, cursor: usize, part: &mut Part, ctx: &mut Ctx) {
    if !active(part) {
        return;
    }
    let u0 = use_val(ctx, p, 0);
    if part.frame == 0 {
        let r = if byte(ctx, cursor + 0x14) == 0 {
            ctx.rng.unit() * 2.
        } else {
            ctx.rng.unit() + ctx.rng.unit()
        };
        part.set_f32(u0, r);
    } else if !key(ctx, part, cursor) {
        return;
    }
    let r = part.f32(u0);
    let target = word(ctx, cursor + 4) as i32;
    if target == -1 {
        return;
    }
    let t = target as usize;
    for i in 0..3 {
        let k = float(ctx, cursor + 8 + 4 * i);
        let v = (k * r - k) + part.f32(t + 4 * i);
        part.set_f32(t + 4 * i, v);
    }
}

/// FUN_1404c2950: the integer variant, only at keys.
fn rand_iv(p: &Program, cursor: usize, part: &mut Part, ctx: &mut Ctx) {
    if !active(part) || !key(ctx, part, cursor) {
        return;
    }
    let r = if byte(ctx, cursor + 0x14) == 0 {
        ctx.rng.unit() * 2.
    } else {
        ctx.rng.unit() + ctx.rng.unit()
    };
    let u0 = use_val(ctx, p, 0);
    part.set_f32(u0, r);
    let target = word(ctx, cursor + 4) as i32;
    if target == -1 {
        return;
    }
    let t = target as usize;
    for i in 0..3 {
        let k = word(ctx, cursor + 8 + 4 * i) as i32 as f32;
        let v = part.i32(t + 4 * i).wrapping_add((k * r - k) as i32);
        part.set_i32(t + 4 * i, v);
    }
}

fn params_vec3(ctx: &Ctx, at: usize) -> [f32; 3] {
    [float(ctx, at), float(ctx, at + 4), float(ctx, at + 8)]
}

/// The basis `eda0..edd0` (FUN_1404c6ed0, FUN_1402c3310): the parent's y axis
/// (edb0), the horizontal vector (y, −x, 0) perpendicular to it (eda0), their
/// cross product (edc0) and the parent's translation (edd0). The cross
/// product's operand order [I: FUN_140478db0 read as a × b].
pub(crate) fn parent_basis(parent: &Mat) -> Mat {
    let up = math::normalize([parent[1][0], parent[1][1], parent[1][2], 0.]);
    let side = math::normalize([up[1], -up[0], 0., 0.]);
    let third = math::normalize([
        side[1] * up[2] - side[2] * up[1],
        side[2] * up[0] - side[0] * up[2],
        side[0] * up[1] - side[1] * up[0],
        0.,
    ]);
    [side, up, third, parent[3]]
}

/// FUN_1404c7200: a model-view matrix into u0. Modes: 0 offset before
/// view·world, 1 offset after it, 2 world then view with scaled offset.
fn ke_z_crct(p: &Program, cursor: usize, part: &mut Part, ctx: &mut Ctx) {
    let offset = params_vec3(ctx, cursor + 4);
    let s = params_vec3(ctx, cursor + 0x10);
    let mut m = part.cur;
    for (col, s) in m.iter_mut().zip(s) {
        *col = math::scale(*col, s);
    }
    let m = match byte(ctx, cursor + 0x1c) {
        0 => {
            for i in 0..3 {
                m[3][i] += offset[i];
            }
            math::mul(&ctx.ed60, &m)
        }
        2 => {
            let mut w = math::mul(&ctx.world, &m);
            for i in 0..3 {
                w[3][i] += ctx.scales[i] * offset[i];
            }
            math::mul(&ctx.view, &w)
        }
        // Mode 3: the parent basis eda0..edd0 of FUN_1404c6ed0 /
        // FUN_1402c3310 in place of ed60, the offset added after.
        3 => {
            let mut w = math::mul(&parent_basis(&ctx.ed60), &m);
            for i in 0..3 {
                w[3][i] += offset[i];
            }
            w
        }
        mode => {
            if mode != 1 && !ctx.unsupported.contains(&(0x2000_0000 | mode as u32)) {
                ctx.unsupported.push(0x2000_0000 | mode as u32);
            }
            let mut w = math::mul(&ctx.ed60, &m);
            for i in 0..3 {
                w[3][i] += offset[i];
            }
            w
        }
    };
    write_matrix(part, use_val(ctx, p, 0), &math::mul(&ctx.ede0, &m));
}

/// FUN_1404c73d0: shape variant: axes × world scale stay in draw space,
/// only the translation goes through the parent.
fn ke_z_crct_shp(p: &Program, cursor: usize, part: &mut Part, ctx: &mut Ctx) {
    let offset = params_vec3(ctx, cursor + 4);
    let s = params_vec3(ctx, cursor + 0x10);
    let mut m = part.cur;
    for (col, s) in m.iter_mut().zip(ctx.scales) {
        *col = math::scale(*col, s);
    }
    let mut t = part.cur[3];
    for i in 0..3 {
        t[i] *= s[i];
    }
    m[3] = match byte(ctx, cursor + 0x1c) {
        0 => {
            for i in 0..3 {
                t[i] += offset[i];
            }
            math::apply(&ctx.ed60, t)
        }
        1 => {
            let mut w = math::apply(&ctx.ed60, t);
            for i in 0..3 {
                w[i] += offset[i];
            }
            w
        }
        2 => {
            let mut w = math::apply(&ctx.world, t);
            for i in 0..3 {
                w[i] += ctx.scales[i] * offset[i];
            }
            math::apply(&ctx.view, w)
        }
        _ => [0., 0., 0., 1.],
    };
    write_matrix(part, use_val(ctx, p, 0), &math::mul(&ctx.ede0, &m));
}

pub(crate) fn write_matrix(part: &mut Part, o: usize, m: &Mat) {
    for (c, col) in m.iter().enumerate() {
        part.set_vec4(o + 16 * c, *col);
    }
}

pub(crate) fn read_matrix(part: &Part, o: usize) -> Mat {
    std::array::from_fn(|c| part.vec4(o + 16 * c))
}

fn shape_anim(ctx: &Ctx, cursor: usize) -> Option<usize> {
    let shape = word(ctx, cursor + 4);
    if shape == 0xffff {
        return None;
    }
    ctx.dpd.shapes.get(&(shape as i16)).map(|s| s.anim)
}

/// FUN_1404c9b30: flip-book timer u0: [t, index, previous index]. The wait
/// read is the previous index's, and the index drawn is the previous one.
fn shape_advance(p: &Program, cursor: usize, part: &mut Part, ctx: &mut Ctx) {
    if !active(part) {
        return;
    }
    let Some(anim) = shape_anim(ctx, cursor) else {
        return;
    };
    let b = &ctx.dpd.bytes;
    let o = use_val(ctx, p, 0);
    let index = part.i16(o + 2) as u16;
    let previous = part.i16(o + 4) as u16;
    let (wait, flag) = object_effect::shape_sequence(b, anim, previous as usize).unwrap_or((1, 0));
    part.set_i16(o + 4, index as i16);
    let speed = i16_at(b, cursor + 8).unwrap_or(0);
    let t = (part.i16(o) as u16).wrapping_add(speed as u16);
    part.set_i16(o, t as i16);
    if wait as i32 <= t as i32 {
        part.set_i16(o, t.wrapping_sub(wait as u16) as i16);
        part.set_i16(o + 2, index.wrapping_add(1) as i16);
        let count = object_effect::shape_sequence_count(b, anim).unwrap_or(0);
        if count as i32 <= index.wrapping_add(1) as i32 {
            part.set_i16(o, 0);
            if flag & 0x80 != 0 {
                part.set_i16(o + 2, 0);
                return;
            }
            part.set_i16(o + 2, index as i16);
        }
    }
}

/// FUN_1404c99c0 → FUN_1404ae220 → FUN_1404ada10/FUN_1404ad7b0: the
/// previous flip-book frame's sprites through the draw matrix (or the u3
/// matrix when byte +0xe is set), coloured by the part colour bytes.
fn draw_shape(p: &Program, cursor: usize, part: &mut Part, ctx: &mut Ctx) {
    let Some(anim) = shape_anim(ctx, cursor) else {
        return;
    };
    let o = use_val(ctx, p, 0);
    let index = part.i16(o + 4) as u16 as usize;
    let colour_var = use_val(ctx, p, 1);
    let m = if byte(ctx, cursor + 0xe) != 0 {
        read_matrix(part, use_val(ctx, p, 3))
    } else {
        part.draw
    };
    let packet = std::array::from_fn(|i| part.u8(colour_var + 8 + i));
    emit_sprites(ctx, anim, index, &m, packet);
}

/// FUN_1404ae220 with FUN_1404ada10/FUN_1404ad7b0: sprites of flip-book
/// frame `index` of the shape at `anim`, through matrix `m`, modulated by
/// the packet colour bytes (0x80 = 1.0).
pub(crate) fn emit_sprites(ctx: &mut Ctx, anim: usize, index: usize, m: &Mat, packet: [u8; 4]) {
    let quads = sprite_quads(&ctx.dpd.bytes, anim, index, m, packet);
    ctx.quads.extend(quads);
}

/// The quads of flip-book frame `index` of the shape at `anim` (see
/// [`emit_sprites`]).
pub(crate) fn sprite_quads(
    b: &[u8],
    anim: usize,
    index: usize,
    m: &Mat,
    packet: [u8; 4],
) -> Vec<Quad> {
    let mut out = Vec::new();
    let packet: [f32; 4] = packet.map(|c| c as f32 / 128.);
    let Ok(sprites) = object_effect::shape_frame(b, anim, index) else {
        return out;
    };
    let address = i16_at(b, anim + 0x10 + index * 8).unwrap_or(0) as isize;
    let frame = (anim as isize + address) as usize;
    let tex0 = u32_at(b, frame + 0x10).unwrap_or(0) as u64
        | (u32_at(b, frame + 0x14).unwrap_or(0) as u64) << 32;
    let tbp = (tex0 & 0x3fff) as u16;
    for s in sprites {
        let corners = s.corners.map(|[x, y]| {
            let v = math::apply(m, [x as f32 / 16., y as f32 / 16., 0., 1.]);
            [v[0], v[1], v[2]]
        });
        let [su, tv, w, h] = s.uv.map(|x| x as f32);
        let uv = [
            [su / 16., tv / 8.],
            [(su + w) / 16., tv / 8.],
            [(su + w) / 16., (tv + h) / 8.],
            [su / 16., (tv + h) / 8.],
        ];
        let colour = std::array::from_fn(|i| s.colour[i] as f32 / 128. * packet[i]);
        out.push(Quad {
            corners,
            uv,
            colour,
            blend: s.colour[4],
            corner_colours: None,
            tbp,
            tex0,
        });
    }
    out
}

/// FUN_1404d4ea0 → FUN_1404e1780 → FUN_1404d4f80: every `interval` ticks
/// `count` children of entry +0x10 are born at VSF vertices (sequential or
/// random), placed through the emitter's matrix, moving away from its
/// centre at `speed`.
fn born_rnd5(p: &Program, cursor: usize, part: &mut Part, ctx: &mut Ctx) {
    let interval = byte(ctx, cursor + 0xd);
    if !active(part) || interval == 0 {
        return;
    }
    let o = use_val(ctx, p, 0);
    let mut countdown = part.u8(o + 2);
    if countdown == 0 {
        part.set_u8(o + 2, interval);
        let child = word(ctx, cursor + 0x10) as i32;
        let velocity_var = word(ctx, cursor + 0x1c) as i32;
        if child == -1 || word(ctx, cursor + 0x18) as i32 == -1 {
            return;
        }
        let vsf_index = i16_at(&ctx.dpd.bytes, cursor + 4).unwrap_or(0) as i32;
        let Some(vsf) = ctx.dpd.vsf.get(&vsf_index) else {
            return;
        };
        let n = vsf.indices.len();
        let random = byte(ctx, cursor + 0xe) != 0;
        let mut k = if random {
            (ctx.rng.rand() as usize) % n.max(1)
        } else {
            part.i16(o) as u16 as usize
        };
        let speed = float(ctx, cursor + 8);
        let position_var = word(ctx, cursor + 0x14) as usize;
        for _ in 0..byte(ctx, cursor + 0xc) {
            let vertex = vsf
                .indices
                .get(k)
                .and_then(|&i| ctx.dpd.vertex(vsf.model, i))
                .unwrap_or([0; 3]);
            let at = math::apply(
                &part.cur,
                [vertex[0] as f32, vertex[1] as f32, vertex[2] as f32, 1.],
            );
            let t = part.cur[3];
            let away = math::normalize([at[0] - t[0], at[1] - t[1], at[2] - t[2], at[3] - t[3]]);
            ctx.births.push(Birth {
                entry: child as usize,
                position_var,
                position: at,
                position_len: 4,
                velocity_var: Some(velocity_var as usize),
                velocity: math::scale(away, speed),
                ints: Vec::new(),
            });
            k = if random {
                (ctx.rng.rand() as usize) % n.max(1)
            } else if k + 1 >= n {
                0
            } else {
                k + 1
            };
        }
        part.set_i16(o, k as i16);
        countdown = interval;
    }
    part.set_u8(o + 2, countdown.wrapping_sub(1));
}

/// FUN_1404c3370: like RandFV but one draw per axis (r = 2u, or u + u when
/// byte +0x14 is set). At frame 0 the three randoms are drawn into u0;
/// at frame 0 or a key the target (−1: scratch) gains k·r − k per axis.
fn s_rand_fv(p: &Program, cursor: usize, part: &mut Part, ctx: &mut Ctx) {
    if !active(part) {
        return;
    }
    let u0 = use_val(ctx, p, 0);
    if part.frame == 0 {
        let mode = byte(ctx, cursor + 0x14) != 0;
        for i in 0..3 {
            let r = if mode {
                ctx.rng.unit() + ctx.rng.unit()
            } else {
                ctx.rng.unit() * 2.
            };
            part.set_f32(u0 + 4 * i, r);
        }
    } else if !key(ctx, part, cursor) {
        return;
    }
    let target = word(ctx, cursor + 4) as i32;
    if target == -1 {
        return;
    }
    let t = target as usize;
    for i in 0..3 {
        let k = float(ctx, cursor + 8 + 4 * i);
        let v = (k * part.f32(u0 + 4 * i) - k) + part.f32(t + 4 * i);
        part.set_f32(t + 4 * i, v);
    }
}

/// FUN_1404c60f0: the position into the parent's space, the rotation by the
/// parent's axes normalised (no parent scale).
fn ke_par_mat_r(part: &mut Part, ctx: &Ctx) {
    let Some(parent) = ctx.parent_cur else {
        return;
    };
    let r: Mat = [
        math::normalize([parent[0][0], parent[0][1], parent[0][2], 0.]),
        math::normalize([parent[1][0], parent[1][1], parent[1][2], 0.]),
        math::normalize([parent[2][0], parent[2][1], parent[2][2], 0.]),
        [0., 0., 0., 1.],
    ];
    let t = math::apply(&parent, part.cur[3]);
    part.cur = math::mul(&r, &part.cur);
    part.cur[3] = t;
}

/// atan2 of the exe's open-coded quadrant rules as an angle in 0x10000 =
/// 360° units; `None` where both arguments are zero (the var is left).
fn angle_units(y: f32, x: f32) -> Option<i32> {
    if y == 0. && x == 0. {
        return None;
    }
    let a = (y as f64).atan2(x as f64) as f32;
    Some(models::cvtt(a * 32768. / std::f32::consts::PI))
}

/// FUN_1404d51a0 with the birth callback FUN_1404d5280: every `interval`
/// ticks `count` children of entry +0xc are born at VSF vertices (raw model
/// coordinates), with two angle words giving the direction of the vertex
/// from the model origin (the child's pppMatrixZXY turns shards along it).
fn born_rnd6(p: &Program, cursor: usize, part: &mut Part, ctx: &mut Ctx) {
    let interval = byte(ctx, cursor + 7);
    if !active(part) || interval == 0 {
        return;
    }
    let o = use_val(ctx, p, 0);
    let mut countdown = part.u8(o + 2);
    if countdown == 0 {
        part.set_u8(o + 2, interval);
        let child = word(ctx, cursor + 0xc) as i32;
        if child == -1 || word(ctx, cursor + 0x14) as i32 == -1 {
            return;
        }
        let vsf_index = i16_at(&ctx.dpd.bytes, cursor + 4).unwrap_or(0) as i32;
        let Some(vsf) = ctx.dpd.vsf.get(&vsf_index) else {
            return;
        };
        let n = vsf.indices.len();
        let random = byte(ctx, cursor + 8) != 0;
        let mut k = if random {
            (ctx.rng.rand() as usize) % n.max(1)
        } else {
            part.i16(o) as u16 as usize
        };
        let position_var = word(ctx, cursor + 0x10) as usize;
        let angle_var = word(ctx, cursor + 0x18) as usize;
        for _ in 0..byte(ctx, cursor + 6) {
            let index = vsf.indices.get(k).copied().unwrap_or(0);
            let vertex = match &part.vertex_override {
                Some(m) => m.get(index as usize).copied(),
                None => ctx.dpd.vertex(vsf.model, index),
            }
            .unwrap_or([0; 3]);
            let [x, y, z] = vertex.map(|c| c as f32);
            let mut ints = Vec::new();
            // x/−y and −z over the horizontal radius.
            if let Some(a) = angle_units(x, -y) {
                ints.push((angle_var + 8, a));
            }
            let r = (x * x + y * y).sqrt();
            if let Some(a) = angle_units(-z, r) {
                ints.push((angle_var, a));
            }
            ints.push((angle_var + 4, 0));
            ctx.births.push(Birth {
                entry: child as usize,
                position_var,
                position: [x, y, z, 1.],
                position_len: 4,
                velocity_var: None,
                velocity: [0.; 4],
                ints,
            });
            k = if random {
                (ctx.rng.rand() as usize) % n.max(1)
            } else if k + 1 >= n {
                0
            } else {
                k + 1
            };
        }
        part.set_i16(o, k as i16);
        countdown = interval;
    }
    part.set_u8(o + 2, countdown.wrapping_sub(1));
}

/// FUN_1404c3620 (pppSRandDownFV): like SRandFV with r = −u, or
/// (−u − u)·0.5 when byte +0x14 is set (FUN_1404c3720); at frame 0 the
/// three draws go to u0, and at frame 0 or a key the target (−1: scratch)
/// gains k·r per axis.
fn s_rand_down_fv(p: &Program, cursor: usize, part: &mut Part, ctx: &mut Ctx) {
    if !active(part) {
        return;
    }
    let u0 = use_val(ctx, p, 0);
    if part.frame == 0 {
        let mode = byte(ctx, cursor + 0x14) != 0;
        for i in 0..3 {
            let mut r = -ctx.rng.unit();
            if mode {
                r = (r - ctx.rng.unit()) * 0.5;
            }
            part.set_f32(u0 + 4 * i, r);
        }
    } else if !key(ctx, part, cursor) {
        return;
    }
    let target = word(ctx, cursor + 4) as i32;
    if target == -1 {
        return;
    }
    let t = target as usize;
    for i in 0..3 {
        let k = float(ctx, cursor + 8 + 4 * i);
        let v = part.f32(u0 + 4 * i) * k + part.f32(t + 4 * i);
        part.set_f32(t + 4 * i, v);
    }
}

/// The cur matrix of the MatrixXYZ family: `rotation` of the u1 angles,
/// columns scaled by u2, translation u0 (FUN_1404c4460 and siblings).
fn matrix_from(p: &Program, part: &mut Part, ctx: &Ctx, rotation: fn([i32; 3]) -> Mat) {
    let (u0, u1, u2) = (use_val(ctx, p, 0), use_val(ctx, p, 1), use_val(ctx, p, 2));
    let angles = [part.i32(u1), part.i32(u1 + 4), part.i32(u1 + 8)];
    let mut m = rotation(angles);
    for (c, col) in m.iter_mut().take(3).enumerate() {
        *col = math::scale(*col, part.f32(u2 + 4 * c));
    }
    m[3] = [part.f32(u0), part.f32(u0 + 4), part.f32(u0 + 8), m[3][3]];
    part.cur = m;
}

/// FUN_1402c3310's "wood" matrix (DAT_142b0eda0): the view-space Y axis of
/// `ed60` kept, X turned to face the camera, Z their cross product,
/// translation that of `ed60`.
pub(crate) fn wood(ed60: &Mat) -> Mat {
    let n = math::normalize([ed60[1][0], ed60[1][1], ed60[1][2], 0.]);
    let x = math::normalize([n[1], -n[0], 0., 0.]);
    let z = math::normalize([
        x[1] * n[2] - x[2] * n[1],
        x[2] * n[0] - x[0] * n[2],
        x[0] * n[1] - x[1] * n[0],
        0.,
    ]);
    [x, n, z, ed60[3]]
}

/// FUN_1404c6630 (pppDrawMatrixWood): axes × world scale turned by
/// ede0·wood (DAT_142b0ee60), translation through ede0·view·world
/// (DAT_142b0ee20): an upright billboard.
fn draw_matrix_wood(part: &mut Part, ctx: &Ctx) {
    let mut d = part.cur;
    for (col, s) in d.iter_mut().zip(ctx.scales) {
        *col = math::scale(*col, s);
    }
    let ee60 = math::mul(&ctx.ede0, &wood(&ctx.ed60));
    let mut d = math::mul(&ee60, &d);
    d[3] = math::apply(&math::mul(&ctx.ede0, &ctx.ed60), part.cur[3]);
    part.draw = d;
}

/// FUN_1404c09a0 / FUN_1404c0ba0 / FUN_1404c0de0 (pppPointLoop,
/// pppAngleLoop, pppScaleLoop): u0 = value xyz, u0+0xc = the first
/// keyframe seen (the exe keeps a handle; the cursor stands in for it).
/// The first update clears the value; keys add (angles wrap above 360°).
fn loop_add(p: &Program, cursor: usize, part: &mut Part, ctx: &mut Ctx) {
    if !active(part) {
        return;
    }
    let u0 = use_val(ctx, p, 0);
    if part.i32(u0 + 0xc) == 0 {
        part.zero(u0, 12);
        part.set_i32(u0 + 0xc, cursor as i32);
    }
    if !key(ctx, part, cursor) {
        return;
    }
    for i in 0..3 {
        let o = u0 + 4 * i;
        if p.kind == ANGLE_LOOP {
            let mut v = part
                .i32(o)
                .wrapping_add(word(ctx, cursor + 4 + 4 * i) as i32);
            while v > 0x10000 {
                v -= 0x10000;
            }
            part.set_i32(o, v);
        } else {
            let v = float(ctx, cursor + 4 + 4 * i) + part.f32(o);
            part.set_f32(o, v);
        }
    }
}

/// FUN_1404c0aa0 / FUN_1404c0d00 / FUN_1404c0ee0: a loop restart takes the
/// first keyframe's add back out (it is added again at frame 0).
fn loop_restart(p: &Program, part: &mut Part, ctx: &Ctx) {
    let u0 = use_val(ctx, p, 0);
    let first = part.i32(u0 + 0xc);
    if first == 0 {
        return;
    }
    let k = first as usize;
    for i in 0..3 {
        let o = u0 + 4 * i;
        if p.kind == ANGLE_LOOP {
            let v = part.i32(o).wrapping_sub(word(ctx, k + 4 + 4 * i) as i32);
            part.set_i32(o, v);
        } else {
            let v = part.f32(o) - float(ctx, k + 4 + 4 * i);
            part.set_f32(o, v);
        }
    }
}

/// FUN_1404bf320 (pppKeThTp): at a key the KeTh target parameters
/// (+0x10 = key +8, +0x14 = key +0xc, +0x18 = key +4) and the target
/// variable (+0x1c: entry key +0x10's first part, variable key +0x14; 0 for
/// −1). The exe keeps a handle there; this stores [`crate::thunder::encode`].
/// Every tick u0 gets the part's translation.
fn ke_th_tp(p: &Program, cursor: usize, part: &mut Part, ctx: &Ctx) {
    let u0 = use_val(ctx, p, 0);
    if key(ctx, part, cursor) {
        part.set_i32(u0 + 0x10, word(ctx, cursor + 8) as i32);
        part.set_i32(u0 + 0x18, word(ctx, cursor + 4) as i32);
        part.set_i32(u0 + 0x14, word(ctx, cursor + 0xc) as i32);
        let entry = word(ctx, cursor + 0x10) as i32;
        let target = if entry == -1 {
            0
        } else {
            crate::thunder::encode(entry, word(ctx, cursor + 0x14))
        };
        part.set_i32(u0 + 0x1c, target);
    }
    if active(part) {
        for i in 0..3 {
            part.set_f32(u0 + 4 * i, part.cur[3][i]);
        }
    }
}

/// FUN_1404c75d0 (pppKeThSft): keys add to the rate block u0 (16 i16
/// colours at +0, floats +0x20/+0x24/+0x28/+0x2c and the byte at u1+0x50);
/// every tick (except a first frame-0 key) the rates feed the KeTh block at
/// u1 (+0x70.. colours, +0x1c, +0x34, +0x6c, +0x3c).
fn ke_th_sft(p: &Program, cursor: usize, part: &mut Part, ctx: &Ctx) {
    if !active(part) {
        return;
    }
    let (u0, u1) = (use_val(ctx, p, 0), use_val(ctx, p, 1));
    if key(ctx, part, cursor) {
        part.set_u8(u1 + 0x50, word(ctx, cursor + 0x34) as u8);
        for (o, k) in [(0x28, 0x2c), (0x20, 0x24), (0x24, 0x28), (0x2c, 0x30)] {
            let v = float(ctx, cursor + k) + part.f32(u0 + o);
            part.set_f32(u0 + o, v);
        }
        for i in 0..16 {
            let k = i16_at(&ctx.dpd.bytes, cursor + 4 + 2 * i).unwrap_or(0);
            let v = part.i16(u0 + 2 * i).wrapping_add(k);
            part.set_i16(u0 + 2 * i, v);
        }
        if part.frame == 0 && !part.restarted {
            return;
        }
    }
    for (to, from) in [(0x6c, 0x28), (0x1c, 0x20), (0x34, 0x24), (0x3c, 0x2c)] {
        let v = part.f32(u0 + from) + part.f32(u1 + to);
        part.set_f32(u1 + to, v);
    }
    for i in 0..16 {
        let v = part
            .i16(u1 + 0x70 + 2 * i)
            .wrapping_add(part.i16(u0 + 2 * i));
        part.set_i16(u1 + 0x70 + 2 * i, v);
    }
}

/// FUN_1404d4760 with FUN_1404e1780 and the birth callback FUN_1404d4830
/// (pppKeBornRnd3): outside catch-up, when `(rand() >> 8) & key +7` is 0,
/// `key +6` children of entry +0xc are born at VSF vertices (sequential
/// from the u16 cursor u0, or random when +8 is set): position (vertex
/// through the cur matrix) to child var +0x10, the unit direction from the
/// emitter to it to var +0x18 (xyz only, as the exe copies).
fn born_rnd3(p: &Program, cursor: usize, part: &mut Part, ctx: &mut Ctx) {
    let mask = byte(ctx, cursor + 7);
    if ctx.catch_up || !active(part) || mask == 0xff {
        return;
    }
    let r = ctx.rng.rand();
    if (r >> 8) as u8 & mask != 0 {
        return;
    }
    let child = word(ctx, cursor + 0xc) as i32;
    if child == -1 || word(ctx, cursor + 0x14) as i32 == -1 {
        return;
    }
    let vsf_index = i16_at(&ctx.dpd.bytes, cursor + 4).unwrap_or(0) as i32;
    let Some(vsf) = ctx.dpd.vsf.get(&vsf_index) else {
        return;
    };
    let n = vsf.indices.len();
    if n == 0 {
        return;
    }
    let u0 = use_val(ctx, p, 0);
    let random = byte(ctx, cursor + 8) != 0;
    let mut k = if random {
        ctx.rng.rand() as usize % n
    } else {
        part.i16(u0) as u16 as usize
    };
    let position_var = word(ctx, cursor + 0x10) as usize;
    let direction_var = word(ctx, cursor + 0x18) as usize;
    for _ in 0..byte(ctx, cursor + 6) {
        let index = vsf.indices.get(k).copied().unwrap_or(0);
        let vertex = match &part.vertex_override {
            Some(m) => m.get(index as usize).copied(),
            None => ctx.dpd.vertex(vsf.model, index),
        }
        .unwrap_or([0; 3]);
        let at = math::apply(
            &part.cur,
            [vertex[0] as f32, vertex[1] as f32, vertex[2] as f32, 1.],
        );
        let t = part.cur[3];
        let away = math::normalize([at[0] - t[0], at[1] - t[1], at[2] - t[2], 0.]);
        ctx.births.push(Birth {
            entry: child as usize,
            position_var,
            position: at,
            position_len: 3,
            velocity_var: None,
            velocity: [0.; 4],
            ints: (0..3)
                .map(|i| (direction_var + 4 * i, away[i].to_bits() as i32))
                .collect(),
        });
        k = if random {
            ctx.rng.rand() as usize % n
        } else if k + 1 >= n {
            0
        } else {
            k + 1
        };
    }
    part.set_i16(u0, k as i16);
}

/// The KeThRes pool header: `nodes` nodes of `size` bytes from +0x10,
/// linked through their first word (offset + 1; 0 ends the list).
fn ke_th_res(part: &mut Part, o: usize, nodes: u8, points: u8, size: u16) {
    part.set_u8(o, nodes);
    part.set_u8(o + 1, points);
    part.set_i16(o + 2, size as i16);
    part.set_i32(o + 4, (o + 0x10 + 1) as i32);
    for n in 0..nodes as usize {
        let at = o + 0x10 + n * size as usize;
        let next = if n + 1 < nodes as usize {
            (at + size as usize + 1) as i32
        } else {
            0
        };
        part.set_i32(at, next);
    }
}

/// FUN_1404d36e0 (pppPointAp): every `key +0xc` ticks, `key +0xd` children
/// of entry +4 are born with var +8 = this part's u0 xyz (countdown at
/// u1+1).
fn point_ap(p: &Program, cursor: usize, part: &mut Part, ctx: &mut Ctx) {
    if !active(part) {
        return;
    }
    let (u0, u1) = (use_val(ctx, p, 0), use_val(ctx, p, 1));
    let mut countdown = part.u8(u1 + 1);
    if countdown == 0 {
        let child = word(ctx, cursor + 4) as i32;
        let var = word(ctx, cursor + 8) as usize;
        for _ in 0..byte(ctx, cursor + 0xd) {
            if child < 0 {
                break;
            }
            ctx.births.push(Birth {
                entry: child as usize,
                position_var: var,
                position: part.vec4(u0),
                position_len: 3,
                velocity_var: None,
                velocity: [0.; 4],
                ints: Vec::new(),
            });
        }
        countdown = byte(ctx, cursor + 0xc);
    }
    part.set_u8(u1 + 1, countdown.wrapping_sub(1));
}

/// FUN_1404c1ae0 (pppRandFloat): the one-component RandFV. At frame 0 u0
/// takes rnd·2 (rnd + rnd when byte +0xc is set); at frame 0 or a key the
/// target (−1: scratch) gains k·r − k.
fn rand_float(p: &Program, cursor: usize, part: &mut Part, ctx: &mut Ctx) {
    if !active(part) {
        return;
    }
    let u0 = use_val(ctx, p, 0);
    if part.frame == 0 {
        let r = if byte(ctx, cursor + 0xc) == 0 {
            ctx.rng.unit() * 2.
        } else {
            ctx.rng.unit() + ctx.rng.unit()
        };
        part.set_f32(u0, r);
    } else if !key(ctx, part, cursor) {
        return;
    }
    let target = word(ctx, cursor + 4) as i32;
    if target == -1 {
        return;
    }
    let k = float(ctx, cursor + 8);
    let t = target as usize;
    let v = (k * part.f32(u0) - k) + part.f32(t);
    part.set_f32(t, v);
}

/// FUN_1404c2840 (pppRandDownFV): u0 takes −rnd at frame 0, or
/// (−rnd − rnd)·0.5 when byte +0x14 is set. At frame 0 or a key the target
/// xyz (−1: scratch) gains k_i·r.
fn rand_down_fv(p: &Program, cursor: usize, part: &mut Part, ctx: &mut Ctx) {
    if !active(part) {
        return;
    }
    let u0 = use_val(ctx, p, 0);
    if part.frame == 0 {
        let mut r = -ctx.rng.unit();
        if byte(ctx, cursor + 0x14) != 0 {
            r = (r - ctx.rng.unit()) * 0.5;
        }
        part.set_f32(u0, r);
    } else if !key(ctx, part, cursor) {
        return;
    }
    let target = word(ctx, cursor + 4) as i32;
    if target == -1 {
        return;
    }
    let t = target as usize;
    let r = part.f32(u0);
    for i in 0..3 {
        let k = float(ctx, cursor + 8 + 4 * i);
        let v = k * r + part.f32(t + 4 * i);
        part.set_f32(t + 4 * i, v);
    }
}

/// FUN_1404c1510: atan2(p1, p0) in radians, branch for branch as the exe
/// computes it (constants: DAT_1405b2e48 = −π/2, DAT_140623ab0 = π/2,
/// DAT_140623c78 = −π, DAT_140623b78 = π). `None` at the origin, where the
/// exe writes nothing.
pub(crate) fn ke_angle(p0: f32, p1: f32) -> Option<f32> {
    use std::f32::consts::{FRAC_PI_2, PI};
    let prod = p0 * p1;
    let a = if prod <= 0. {
        if 0. <= prod {
            if p0 == 0. {
                if p1 == 0. {
                    return None;
                }
                if 0. < p1 { FRAC_PI_2 } else { -FRAC_PI_2 }
            } else if p0 <= 0. {
                PI
            } else {
                0.
            }
        } else {
            let base = if 0. < p1 { FRAC_PI_2 } else { -FRAC_PI_2 };
            (base as f64 - ((p0 / p1) as f64).atan()) as f32
        }
    } else {
        let base = if p0 <= 0. { -PI } else { 0. };
        (((p1 / p0) as f64).atan() + base as f64) as f32
    };
    Some(a)
}

/// The 0x10000-per-turn word of an angle in radians: `(int)(a · 32768 / π)`.
fn angle_word(a: f32) -> i32 {
    models::cvtt(a * 32768. / std::f32::consts::PI)
}

/// FUN_1404c15f0 (pppKeMvYpEff, even frames): the velocity in u1 becomes two
/// angles, u0 = pitch atan2(−z, √(x²+y²)) and u0+8 = yaw atan2(x, −y). A
/// zero velocity, or an atan2 at the origin, leaves its target alone.
fn ke_mv_yp_eff(p: &Program, part: &mut Part, ctx: &Ctx) {
    if !active(part) {
        return;
    }
    let (u0, u1) = (use_val(ctx, p, 0), use_val(ctx, p, 1));
    let (x, y, z) = (part.f32(u1), part.f32(u1 + 4), part.f32(u1 + 8));
    if x != 0. || y != 0. || z != 0. {
        if let Some(a) = ke_angle(-y, x) {
            part.set_i32(u0 + 8, angle_word(a));
        }
        let len = (x * x + y * y).abs().sqrt();
        if let Some(a) = ke_angle(len, -z) {
            part.set_i32(u0, angle_word(a));
        }
    }
}

/// FUN_1404c4380 (pppKeMatSN, once per VM tick): the matrix at u0+0x40 turns
/// by the change of the u1 angles since the last tick (old − new, in the
/// pppMatrixXYZ order); u0+0x80 keeps the new angles. The exe runs it only on
/// VM ticks (the even part frames), so a second call would see no change.
fn ke_mat_sn(p: &Program, part: &mut Part, ctx: &Ctx) {
    if !active(part) {
        return;
    }
    let (u0, u1) = (use_val(ctx, p, 0), use_val(ctx, p, 1));
    let new = [part.i32(u1), part.i32(u1 + 4), part.i32(u1 + 8)];
    let old = [
        part.i32(u0 + 0x80),
        part.i32(u0 + 0x84),
        part.i32(u0 + 0x88),
    ];
    for (i, n) in new.into_iter().enumerate() {
        part.set_i32(u0 + 0x80 + 4 * i, n);
    }
    let delta: [i32; 3] = std::array::from_fn(|i| old[i].wrapping_sub(new[i]));
    let mut m = math::rot_xyz(delta);
    m[3] = [0., 0., 0., 1.];
    write_matrix(part, u0 + 0x40, &m);
}

/// FUN_1404c0640 (pppAngMoveLoop): the Move integrator with u0 gaining the
/// rate only outside a loop catch-up (DAT_142b0efe4).
fn loop_move_i32(p: &Program, cursor: usize, part: &mut Part, ctx: &mut Ctx) {
    if !active(part) {
        return;
    }
    let (u0, u1) = (use_val(ctx, p, 0), use_val(ctx, p, 1));
    if key(ctx, part, cursor) {
        for i in 0..3 {
            let r = part
                .i32(u1 + 4 * i)
                .wrapping_add(word(ctx, cursor + 4 + 4 * i) as i32);
            part.set_i32(u1 + 4 * i, r);
        }
    }
    if !ctx.catch_up {
        for i in 0..3 {
            let v = part.i32(u0 + 4 * i).wrapping_add(part.i32(u1 + 4 * i));
            part.set_i32(u0 + 4 * i, v);
        }
    }
}

/// FUN_1404c0700 (pppSclMoveLoop): the float variant of [`loop_move_i32`].
fn loop_move_f32(p: &Program, cursor: usize, part: &mut Part, ctx: &mut Ctx) {
    if !active(part) {
        return;
    }
    let (u0, u1) = (use_val(ctx, p, 0), use_val(ctx, p, 1));
    if key(ctx, part, cursor) {
        for i in 0..3 {
            let r = float(ctx, cursor + 4 + 4 * i) + part.f32(u1 + 4 * i);
            part.set_f32(u1 + 4 * i, r);
        }
    }
    if !ctx.catch_up {
        for i in 0..3 {
            let v = part.f32(u0 + 4 * i) + part.f32(u1 + 4 * i);
            part.set_f32(u0 + 4 * i, v);
        }
    }
}

/// FUN_1404c50e0 (pppRyjMatrixNoRot): the cur matrix is the inverse of the
/// view (FUN_140478ce0), its axes scaled by u1 and its translation u0.
fn matrix_no_rot(p: &Program, part: &mut Part, ctx: &Ctx) {
    let (u0, u1) = (use_val(ctx, p, 0), use_val(ctx, p, 1));
    let mut m = math::rigid_inverse(&ctx.view);
    for (c, col) in m.iter_mut().take(3).enumerate() {
        *col = math::scale(*col, part.f32(u1 + 4 * c));
    }
    m[3] = [part.f32(u0), part.f32(u0 + 4), part.f32(u0 + 8), m[3][3]];
    part.cur = m;
}

/// The translation matrix [I | u1..u1+2] that the pppRyjDrawMatrixWorld
/// family keeps in its u1 block (the instance position, set at create).
fn world_point(part: &Part, u1: usize) -> Mat {
    let mut m = math::IDENTITY;
    m[3] = [part.f32(u1), part.f32(u1 + 4), part.f32(u1 + 8), 1.];
    m
}

/// FUN_1404c6b90 / FUN_1404c6ed0 (create of pppRyjDrawMatrixWorldFront and
/// Wood): u1 takes the instance position (DAT_142b0eff0, the xyz of the
/// copy) and u1+0x10 the world axis lengths (DAT_142b0f010..18).
fn world_create(p: &Program, part: &mut Part, ctx: &Ctx) {
    let u1 = use_val(ctx, p, 1);
    for i in 0..3 {
        part.set_f32(u1 + 4 * i, ctx.origin[i]);
        part.set_f32(u1 + 0x10 + 4 * i, ctx.scales[i]);
    }
}

/// FUN_1404c69e0 (pppRyjDrawMatrixWorld): the draw matrix is
/// ede0·(ec60·M)·cur, M the matrix in the u1 block.
fn draw_matrix_world(p: &Program, part: &mut Part, ctx: &Ctx) {
    let u1 = use_val(ctx, p, 1);
    let m = read_matrix(part, u1);
    let d = math::mul(&ctx.ede0, &math::mul(&ctx.view, &m));
    part.draw = math::mul(&d, &part.cur);
}

/// FUN_1404c6a70 (pppRyjDrawMatrixWorldFront): the axes are cur's scaled by
/// u1+0x10.., the translation ec60·(u1 + cur t); the draw matrix is
/// ede0 · that.
fn draw_matrix_world_front(p: &Program, part: &mut Part, ctx: &Ctx) {
    let u1 = use_val(ctx, p, 1);
    let view = math::mul(&ctx.view, &world_point(part, u1));
    let mut d = part.cur;
    for (c, col) in d.iter_mut().take(3).enumerate() {
        *col = math::scale(*col, part.f32(u1 + 0x10 + 4 * c));
    }
    d[3] = math::apply(&view, part.cur[3]);
    part.draw = math::mul(&ctx.ede0, &d);
}

/// FUN_1404c6d90 (pppRyjDrawMatrixWorldWood): the upright billboard of
/// [`draw_matrix_wood`] with axes scaled by u1+0x10.. and the translation
/// ede0·ec60·(u1 + cur t).
fn draw_matrix_world_wood(p: &Program, part: &mut Part, ctx: &Ctx) {
    let u1 = use_val(ctx, p, 1);
    let view = math::mul(&ctx.ede0, &math::mul(&ctx.view, &world_point(part, u1)));
    let mut d = part.cur;
    for (c, col) in d.iter_mut().take(3).enumerate() {
        *col = math::scale(*col, part.f32(u1 + 0x10 + 4 * c));
    }
    let ee60 = math::mul(&ctx.ede0, &wood(&ctx.ed60));
    let mut d = math::mul(&ee60, &d);
    d[3] = math::apply(&view, part.cur[3]);
    part.draw = d;
}
