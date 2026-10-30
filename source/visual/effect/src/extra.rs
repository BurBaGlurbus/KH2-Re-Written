//! Further ppp programs, translated from their decompilations (table at
//! 0x140779a00, update +8): the random-draw variants, the matrix rotation
//! orders, the scale integrator and a few loop movers. Every program here
//! follows the shapes of [`crate::programs`]; `V[o]` is work offset 0xa0 + o.
use crate::math::{self, Mat};
use crate::programs::{Birth, active, byte, float, key, use_val, word};
use crate::{Ctx, Part};
use data::object_effect::{Program, i16_at};

pub(crate) const RAND_UP_FLOAT: u32 = 52;
pub(crate) const RAND_DOWN_FLOAT: u32 = 53;
pub(crate) const RAND_UP_FV: u32 = 64;
pub(crate) const RAND_UP_IV: u32 = 67;
pub(crate) const RAND_DOWN_IV: u32 = 68;
pub(crate) const RAND_HCV: u32 = 72;
pub(crate) const RAND_DOWN_HCV: u32 = 74;
pub(crate) const S_RAND_UP_FV: u32 = 76;
pub(crate) const S_RAND_HCV: u32 = 81;
pub(crate) const MATRIX_XZY: u32 = 88;
pub(crate) const MATRIX_ZYX: u32 = 92;
pub(crate) const MOVE_LOOP: u32 = 210;
pub(crate) const MATRIX_LOOP_XYZ: u32 = 230;
pub(crate) const MATRIX_LOOP_XZY: u32 = 231;
pub(crate) const MATRIX_LOOP_YZX: u32 = 233;
pub(crate) const MATRIX_LOOP_ZYX: u32 = 235;
pub(crate) const SNO_SCALE_ALL: u32 = 237;
pub(crate) const DRAW_MDL: u32 = 131;
pub(crate) const DRAW_MDL_TS: u32 = 133;
pub(crate) const DRAW_MATRIX_NO_ROT: u32 = 114;
/// pppRyjMatrixWorldXYZ (update FUN_1404c52e0, create FUN_1404c5460).
pub(crate) const MATRIX_WORLD_XYZ: u32 = 99;
pub(crate) const VERTEX_ATTEND: u32 = 45;
pub(crate) const VERTEX_AP_AT: u32 = 178;

/// Kinds with no update function in the program table (or none that
/// matters here).
pub(crate) fn without_update(kind: u32) -> bool {
    // pppKeMdlDtt / pppKeShpDtt integrate in their draw (refraction.rs).
    matches!(
        kind,
        DRAW_MDL | crate::refraction::KE_MDL_DTT | crate::refraction::KE_SHP_DTT
    )
}

/// Kinds [`draw`] draws.
pub(crate) fn draws(kind: u32) -> bool {
    matches!(kind, DRAW_MDL | DRAW_MDL_TS)
}

/// Kinds [`update`] runs.
pub(crate) fn handles(kind: u32) -> bool {
    matches!(
        kind,
        RAND_UP_FLOAT
            | RAND_DOWN_FLOAT
            | RAND_UP_FV
            | RAND_UP_IV
            | RAND_DOWN_IV
            | RAND_HCV
            | RAND_DOWN_HCV
            | S_RAND_UP_FV
            | S_RAND_HCV
            | MATRIX_XZY
            | MATRIX_ZYX
            | MATRIX_LOOP_XYZ
            | MATRIX_LOOP_XZY
            | MATRIX_LOOP_YZX
            | MATRIX_LOOP_ZYX
            | SNO_SCALE_ALL
            | DRAW_MDL_TS
            | DRAW_MATRIX_NO_ROT
            | VERTEX_ATTEND
            | VERTEX_AP_AT
            | MATRIX_WORLD_XYZ
    )
}

/// The instance's own placement (`DAT_142b0f000` angles, `DAT_142b0eff0`
/// position): the world's axes without their lengths, at the origin.
/// [I] Taken from the instance matrix: the exe turns the angles through the
/// sine table (`FUN_1404be820` after `· 32768 / π`).
fn placement(ctx: &Ctx) -> Mat {
    let mut m = ctx.world;
    for (c, s) in ctx.scales.iter().enumerate() {
        if *s != 0. {
            m[c] = math::scale(m[c], 1. / s);
        }
    }
    m[3] = [ctx.origin[0], ctx.origin[1], ctx.origin[2], 1.];
    m
}

/// The inverse of a rotation-and-translation matrix (`FUN_14013f3a0` for
/// such a matrix).
fn rigid_inverse(m: &Mat) -> Mat {
    let mut r = math::IDENTITY;
    for (i, col) in r.iter_mut().take(3).enumerate() {
        *col = [m[0][i], m[1][i], m[2][i], 0.];
    }
    let t = math::apply(&r, [m[3][0], m[3][1], m[3][2], 0.]);
    r[3] = [-t[0], -t[1], -t[2], 1.];
    r
}

/// FUN_1404c52e0 (pppRyjMatrixWorldXYZ): cur = Rxyz(u1 angles) at the u0
/// position, then cur = inverse(the instance's placement now) · cur · (its
/// placement at creation, u3), its axes scaled by u2: the part keeps to the
/// world as the instance moves.
fn matrix_world_xyz(p: &Program, part: &mut Part, ctx: &Ctx) {
    let (u0, u1, u2, u3) = (
        use_val(ctx, p, 0),
        use_val(ctx, p, 1),
        use_val(ctx, p, 2),
        use_val(ctx, p, 3),
    );
    let mut cur = math::rot_xyz([part.i32(u1), part.i32(u1 + 4), part.i32(u1 + 8)]);
    cur[3] = [part.f32(u0), part.f32(u0 + 4), part.f32(u0 + 8), cur[3][3]];
    let created = crate::programs::read_matrix(part, u3);
    let cur = math::mul(&rigid_inverse(&placement(ctx)), &math::mul(&cur, &created));
    let mut cur = cur;
    for (c, col) in cur.iter_mut().take(3).enumerate() {
        let s = part.f32(u2 + 4 * c);
        *col = [col[0] * s, col[1] * s, col[2] * s, col[3]];
    }
    part.cur = cur;
}

/// FUN_1404c35e0 / the up draws: u, or (u + u) · 0.5 when `average`.
fn up(ctx: &mut Ctx, average: bool) -> f32 {
    let first = ctx.rng.unit();
    if average {
        let second = ctx.rng.unit();
        (second + first) * 0.5
    } else {
        first
    }
}

/// FUN_1404c3720 / the down draws: −u, or (−u − u) · 0.5 when `average`.
fn down(ctx: &mut Ctx, average: bool) -> f32 {
    let first = -ctx.rng.unit();
    if average {
        let second = ctx.rng.unit();
        (first - second) * 0.5
    } else {
        first
    }
}

/// FUN_1404c3d30 / the HCV draws: u + u, or u · 2.
fn wide(ctx: &mut Ctx, pair: bool) -> f32 {
    let first = ctx.rng.unit();
    if pair {
        first + ctx.rng.unit()
    } else {
        first * 2.
    }
}

/// The float variants (pppRand{Up,Down}Float, pppRandUpFV): at frame 0 the
/// draw goes to u0; at frame 0 or a key the target (−1: scratch) gains
/// k·r per component.
fn rand_float_family(
    p: &Program,
    cursor: usize,
    part: &mut Part,
    ctx: &mut Ctx,
    components: usize,
    flag_at: usize,
    sign_down: bool,
) {
    if !active(part) {
        return;
    }
    let u0 = use_val(ctx, p, 0);
    if part.frame == 0 {
        let average = byte(ctx, cursor + flag_at) != 0;
        let r = if sign_down {
            down(ctx, average)
        } else {
            up(ctx, average)
        };
        part.set_f32(u0, r);
    } else if !key(ctx, part, cursor) {
        return;
    }
    let target = word(ctx, cursor + 4) as i32;
    if target == -1 {
        return;
    }
    let r = part.f32(u0);
    let t = target as usize;
    for i in 0..components {
        let k = float(ctx, cursor + 8 + 4 * i);
        let v = k * r + part.f32(t + 4 * i);
        part.set_f32(t + 4 * i, v);
    }
}

/// pppRandUpIV / pppRandDownIV (FUN_1404c2a60 / FUN_1404c2b60): at the key
/// only, u0 takes the draw and the target words gain `(int)(k · r)`.
fn rand_int(p: &Program, cursor: usize, part: &mut Part, ctx: &mut Ctx, sign_down: bool) {
    if !active(part) || !key(ctx, part, cursor) {
        return;
    }
    let average = byte(ctx, cursor + 0x14) != 0;
    let r = if sign_down {
        down(ctx, average)
    } else {
        up(ctx, average)
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
        let v = part.i32(t + 4 * i).wrapping_add((k * r) as i32);
        part.set_i32(t + 4 * i, v);
    }
}

fn key_i16(ctx: &Ctx, at: usize) -> i16 {
    i16_at(&ctx.dpd.bytes, at).unwrap_or(0)
}

/// pppRandHCV (FUN_1404c2fe0): at the key, one draw (u · 2, or u + u when
/// byte +0x10 is set) goes to u0 and the four short target components gain
/// `(int)(k · r − k)`.
fn rand_hcv(p: &Program, cursor: usize, part: &mut Part, ctx: &mut Ctx) {
    if !active(part) || !key(ctx, part, cursor) {
        return;
    }
    let r = wide(ctx, byte(ctx, cursor + 0x10) != 0);
    let u0 = use_val(ctx, p, 0);
    part.set_f32(u0, r);
    let target = word(ctx, cursor + 4) as i32;
    if target == -1 {
        return;
    }
    let t = target as usize;
    for i in 0..4 {
        let k = key_i16(ctx, cursor + 8 + 2 * i) as i32 as f32;
        let v = (part.i16(t + 2 * i)).wrapping_add((k * r - k) as i32 as i16);
        part.set_i16(t + 2 * i, v);
    }
}

/// pppRandDownHCV (FUN_1404c3240): the draw is −u (or (−u − u) · 0.5 with
/// byte +0x10); the short components gain `(int)(k · r)`.
fn rand_down_hcv(p: &Program, cursor: usize, part: &mut Part, ctx: &mut Ctx) {
    if !active(part) || !key(ctx, part, cursor) {
        return;
    }
    let r = down(ctx, byte(ctx, cursor + 0x10) != 0);
    let u0 = use_val(ctx, p, 0);
    part.set_f32(u0, r);
    let target = word(ctx, cursor + 4) as i32;
    if target == -1 {
        return;
    }
    let t = target as usize;
    for i in 0..4 {
        let k = key_i16(ctx, cursor + 8 + 2 * i) as i32 as f32;
        let v = part.i16(t + 2 * i).wrapping_add((k * r) as i32 as i16);
        part.set_i16(t + 2 * i, v);
    }
}

/// pppSRandUpFV (FUN_1404c34e0): one draw per axis at frame 0 (u0 xyz),
/// the target gains k_i · r_i at frame 0 or a key.
fn s_rand_up_fv(p: &Program, cursor: usize, part: &mut Part, ctx: &mut Ctx) {
    if !active(part) {
        return;
    }
    let u0 = use_val(ctx, p, 0);
    if part.frame == 0 {
        let average = byte(ctx, cursor + 0x14) != 0;
        for i in 0..3 {
            let r = up(ctx, average);
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
        let v = k * part.f32(u0 + 4 * i) + part.f32(t + 4 * i);
        part.set_f32(t + 4 * i, v);
    }
}

/// pppSRandHCV (FUN_1404c3be0): at the key four draws (FUN_1404c3d30) go to
/// u0 and the short components gain `(int)(k_j · r_j − k_j)`.
fn s_rand_hcv(p: &Program, cursor: usize, part: &mut Part, ctx: &mut Ctx) {
    if !active(part) || !key(ctx, part, cursor) {
        return;
    }
    let u0 = use_val(ctx, p, 0);
    let pair = byte(ctx, cursor + 0x10) != 0;
    for j in 0..4 {
        let r = wide(ctx, pair);
        part.set_f32(u0 + 4 * j, r);
    }
    let target = word(ctx, cursor + 4) as i32;
    if target == -1 {
        return;
    }
    let t = target as usize;
    for j in 0..4 {
        let k = key_i16(ctx, cursor + 8 + 2 * j) as i32 as f32;
        let r = part.f32(u0 + 4 * j);
        let v = part.i16(t + 2 * j).wrapping_add((k * r - k) as i32 as i16);
        part.set_i16(t + 2 * j, v);
    }
}

/// FUN_1404e0010 (Ry·Rz, then Rx): the XZY order.
fn rot_xzy(a: [i32; 3]) -> Mat {
    math::mul(
        &math::mul(&math::rot_y(a[1]), &math::rot_z(a[2])),
        &math::rot_x(a[0]),
    )
}

/// FUN_1404e0290 (Rx·Ry, then Rz): the ZYX order.
fn rot_zyx(a: [i32; 3]) -> Mat {
    math::mul(
        &math::mul(&math::rot_x(a[0]), &math::rot_y(a[1])),
        &math::rot_z(a[2]),
    )
}

/// pppSnoScaleAll (FUN_1404c4080): three scale rates, an overall rate and
/// their accelerations. V offsets are relative to u0 (+0x00 = 0xa0).
fn sno_scale_all(p: &Program, cursor: usize, part: &mut Part, ctx: &mut Ctx) {
    if !active(part) {
        return;
    }
    let o = use_val(ctx, p, 0);
    let at = |off: usize| o + off - 0xa0;
    // The variables by their exe offsets: rates (ac, b0, b4), their
    // accelerations (b8, bc, c0), scales (c4, c8, cc), the overall scale
    // (d0), its rate (d4) and acceleration (d8).
    let mut v: [f32; 12] = [
        0xac, 0xb0, 0xb4, 0xb8, 0xbc, 0xc0, 0xc4, 0xc8, 0xcc, 0xd0, 0xd4, 0xd8,
    ]
    .map(|off| part.f32(at(off)));
    if key(ctx, part, cursor) {
        // Key words 4..6 -> rates, 7..9 -> accelerations, 1..3 -> scales,
        // 10..12 -> overall.
        for (slot, word_index) in [
            (0, 4),
            (1, 5),
            (2, 6),
            (3, 7),
            (4, 8),
            (5, 9),
            (6, 1),
            (7, 2),
            (8, 3),
            (9, 10),
            (10, 11),
            (11, 12),
        ] {
            v[slot] += float(ctx, cursor + 4 * word_index);
        }
    }
    let [
        mut r_x,
        mut r_y,
        mut r_z,
        a_x,
        a_y,
        a_z,
        mut s_x,
        mut s_y,
        mut s_z,
        mut g,
        g_rate,
        g_acc,
    ] = v;
    let g_rate_new = g_acc + g_rate;
    g += g_rate_new;
    let r_z_new = r_z + a_z;
    s_z += r_z_new;
    r_z = r_z_new;
    let r_y_new = a_y + r_y;
    s_y += r_y_new;
    r_y = r_y_new;
    let r_x_new = a_x + r_x;
    s_x += r_x_new;
    r_x = r_x_new;
    for (off, value) in [
        (0xac, r_x),
        (0xb0, r_y),
        (0xb4, r_z),
        (0xb8, a_x),
        (0xbc, a_y),
        (0xc0, a_z),
        (0xc4, s_x),
        (0xc8, s_y),
        (0xcc, s_z),
        (0xd0, g),
        (0xd4, g_rate_new),
        (0xd8, g_acc),
        (0xa0, g * s_x),
        (0xa4, s_y * g),
        (0xa8, s_z * g),
    ] {
        part.set_f32(at(off), value);
    }
}

/// The cur matrix of the MatrixXZY family (columns scaled by u2).
fn matrix_with(p: &Program, part: &mut Part, ctx: &Ctx, rotation: fn([i32; 3]) -> Mat) {
    let (u0, u1, u2) = (use_val(ctx, p, 0), use_val(ctx, p, 1), use_val(ctx, p, 2));
    let angles = [part.i32(u1), part.i32(u1 + 4), part.i32(u1 + 8)];
    let mut m = rotation(angles);
    for (c, col) in m.iter_mut().take(3).enumerate() {
        *col = math::scale(*col, part.f32(u2 + 4 * c));
    }
    m[3] = [part.f32(u0), part.f32(u0 + 4), part.f32(u0 + 8), m[3][3]];
    part.cur = m;
}

pub(crate) fn update(p: &Program, cursor: usize, part: &mut Part, ctx: &mut Ctx) {
    match p.kind {
        MATRIX_WORLD_XYZ => matrix_world_xyz(p, part, ctx),
        RAND_UP_FLOAT => rand_float_family(p, cursor, part, ctx, 1, 0xc, false),
        RAND_DOWN_FLOAT => rand_float_family(p, cursor, part, ctx, 1, 0xc, true),
        RAND_UP_FV => rand_float_family(p, cursor, part, ctx, 3, 0x14, false),
        RAND_UP_IV => rand_int(p, cursor, part, ctx, false),
        RAND_DOWN_IV => rand_int(p, cursor, part, ctx, true),
        RAND_HCV => rand_hcv(p, cursor, part, ctx),
        RAND_DOWN_HCV => rand_down_hcv(p, cursor, part, ctx),
        S_RAND_UP_FV => s_rand_up_fv(p, cursor, part, ctx),
        S_RAND_HCV => s_rand_hcv(p, cursor, part, ctx),
        MATRIX_XZY | MATRIX_LOOP_XZY => matrix_with(p, part, ctx, rot_xzy),
        MATRIX_ZYX | MATRIX_LOOP_ZYX => matrix_with(p, part, ctx, rot_zyx),
        MATRIX_LOOP_XYZ => matrix_with(p, part, ctx, math::rot_xyz),
        MATRIX_LOOP_YZX => matrix_with(p, part, ctx, math::rot_yzx),
        SNO_SCALE_ALL => sno_scale_all(p, cursor, part, ctx),
        DRAW_MDL_TS => crate::models::ts3_update(p, cursor, part, ctx),
        DRAW_MATRIX_NO_ROT => draw_matrix_no_rot(part, ctx),
        VERTEX_ATTEND => vertex_attend(p, cursor, part, ctx),
        VERTEX_AP_AT => vertex_ap_at(p, cursor, part, ctx),
        _ => {}
    }
}

/// FUN_1404c6530 (pppDrawMatrixNoRot): the axes of cur scaled by the world
/// axis lengths, the translation scaled likewise and moved to the
/// instance's view-space position (ed60's translation), then ede0.
fn draw_matrix_no_rot(part: &mut Part, ctx: &Ctx) {
    let mut d = part.cur;
    for (col, s) in d.iter_mut().take(3).zip(ctx.scales) {
        *col = math::scale(*col, s);
    }
    let t = part.cur[3];
    d[3] = [
        t[0] * ctx.scales[0] + ctx.ed60[3][0],
        t[1] * ctx.scales[1] + ctx.ed60[3][1],
        t[2] * ctx.scales[2] + ctx.ed60[3][2],
        1.,
    ];
    part.draw = math::mul(&ctx.ede0, &d);
}

pub(crate) fn draw(p: &Program, cursor: usize, part: &mut Part, ctx: &mut Ctx) {
    match p.kind {
        // FUN_1404c7940: the model's own draw mode and no extra matrix.
        DRAW_MDL => {
            let u0 = use_val(ctx, p, 0);
            crate::models::push_draw(ctx, part, word(ctx, cursor + 4), u0, None, [0, 0], None);
        }
        // FUN_1404c7c70: the scroll pair from u2's first and fourth floats.
        DRAW_MDL_TS => {
            let u0 = use_val(ctx, p, 0);
            let s = use_val(ctx, p, 2);
            let scroll = [
                crate::models::cvtt(part.f32(s) * 65536.),
                crate::models::cvtt(part.f32(s + 12) * 65536.),
            ];
            crate::models::push_draw(ctx, part, word(ctx, cursor + 4), u0, None, scroll, None);
        }
        _ => {}
    }
}

/// The create / restart hook of [`SNO_SCALE_ALL`]: 0x3c bytes cleared.
pub(crate) fn create(p: &Program, part: &mut Part, ctx: &Ctx) {
    match p.kind {
        // FUN_1404c5460: u3 takes the instance's placement at creation.
        MATRIX_WORLD_XYZ => {
            let u3 = use_val(ctx, p, 3);
            crate::programs::write_matrix(part, u3, &placement(ctx));
        }
        SNO_SCALE_ALL => {
            let o = use_val(ctx, p, 0);
            part.zero(o, 0x3c);
        }
        // FUN_1404d41c0: the slot cursor.
        VERTEX_AP_AT => {
            let o = use_val(ctx, p, 0);
            part.zero(o, 4);
        }
        // FUN_1404c7f50: the scroll block u2 cleared.
        DRAW_MDL_TS => {
            let o = use_val(ctx, p, 2);
            part.zero(o, 0x18);
        }
        _ => {}
    }
}

/// pppVertexApAt (FUN_1404d4080): every `interval` (+7) ticks `births` (+6)
/// children of entry +0xc start, each told which VSF slot to follow in its
/// 16-bit variable +0x10: the next slot in order (+8 = 0), or a random one
/// (+8 = 1). The vsf (+4) must be set. Create (FUN_1404d41c0) clears the
/// slot cursor.
fn vertex_ap_at(p: &Program, cursor: usize, part: &mut Part, ctx: &mut Ctx) {
    let vsf_id = key_i16(ctx, cursor + 4);
    if !active(part) || ctx.catch_up || vsf_id < 0 {
        return;
    }
    let u0 = use_val(ctx, p, 0);
    let mut countdown = part.i16(u0 + 2) as u16;
    if countdown == 0 {
        let births = byte(ctx, cursor + 6);
        let mode = byte(ctx, cursor + 8);
        let child = word(ctx, cursor + 0xc) as i32;
        let var = word(ctx, cursor + 0x10) as usize;
        let count = ctx
            .dpd
            .vsf
            .get(&(vsf_id as i32))
            .map_or(0, |v| v.indices.len());
        for _ in 0..births {
            let slot = match mode {
                0 => {
                    let c = part.i16(u0) as u16 as usize;
                    let c = if count <= c { 0 } else { c };
                    part.set_i16(u0, (c + 1) as i16);
                    c
                }
                1 => (count as f32 * ctx.rng.unit()) as usize,
                _ => break,
            };
            if child >= 0 {
                ctx.births.push(Birth {
                    entry: child as usize,
                    position_var: 0,
                    position: [0.; 4],
                    position_len: 0,
                    velocity_var: None,
                    velocity: [0.; 4],
                    ints: vec![(var, slot as i32)],
                });
            }
        }
        countdown = byte(ctx, cursor + 7) as u16;
    }
    part.set_i16(u0 + 2, countdown.wrapping_sub(1) as i16);
}

/// pppVertexAttend (FUN_1404c1240): u1 xyz takes the vertex of VSF slot
/// u0 (wrapped to the vsf's count) through the parent's cur matrix, each
/// update, when the vsf (+0xc) is set.
fn vertex_attend(p: &Program, cursor: usize, part: &mut Part, ctx: &mut Ctx) {
    let vsf_id = key_i16(ctx, cursor + 0xc);
    if vsf_id < 0 {
        return;
    }
    let Some(vsf) = ctx.dpd.vsf.get(&(vsf_id as i32)) else {
        return;
    };
    let (u0, u1) = (use_val(ctx, p, 0), use_val(ctx, p, 1));
    let count = vsf.indices.len();
    if count == 0 {
        return;
    }
    let mut slot = part.i16(u0) as u16 as usize;
    if slot >= count {
        slot %= count;
        part.set_i16(u0, slot as i16);
    }
    let Some(v) = ctx.dpd.vertex(vsf.model, vsf.indices[slot]) else {
        return;
    };
    let local = [v[0] as f32, v[1] as f32, v[2] as f32, 1.];
    let at = match ctx.parent_cur {
        Some(parent) => math::apply(&parent, local),
        None => local,
    };
    for (i, v) in at.iter().take(3).enumerate() {
        part.set_f32(u1 + 4 * i, *v);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn close(a: &Mat, b: &Mat) -> bool {
        a.iter()
            .flatten()
            .zip(b.iter().flatten())
            .all(|(x, y)| (x - y).abs() < 1e-5)
    }

    #[test]
    fn rotation_orders_apply_the_last_factor_first() {
        let a = [0x1000, 0x2000, 0x3000];
        // XZY is Ry·Rz·Rx and ZYX is Rx·Ry·Rz (FUN_1404e0010 / FUN_1404e0290).
        let xzy = math::mul(
            &math::mul(&math::rot_y(a[1]), &math::rot_z(a[2])),
            &math::rot_x(a[0]),
        );
        assert!(close(&rot_xzy(a), &xzy));
        let zyx = math::mul(
            &math::mul(&math::rot_x(a[0]), &math::rot_y(a[1])),
            &math::rot_z(a[2]),
        );
        assert!(close(&rot_zyx(a), &zyx));
        // The orders differ, so a wrong table entry would show.
        assert!(!close(&rot_xzy(a), &rot_zyx(a)));
    }
}

#[cfg(test)]
mod world_xyz_tests {
    use super::*;

    #[test]
    fn the_rigid_inverse_undoes_a_turn_and_a_move() {
        let mut m = math::rot_xyz([0x1000, 0x2000, 0x3000]);
        m[3] = [10., -20., 30., 1.];
        let back = math::mul(&rigid_inverse(&m), &m);
        for (c, col) in back.iter().enumerate() {
            for (r, v) in col.iter().enumerate() {
                let want = if c == r { 1. } else { 0. };
                assert!((v - want).abs() < 1e-4, "{c},{r}: {v}");
            }
        }
    }
}
