//! pppRyjMegaBirthModel (type 208): the 3D-model sibling of pppRyjMegaBirth.
//! Translated from update FUN_1404be1f0, slot spawn FUN_1404bc9e0, slot
//! step FUN_1404bd910, slot matrix FUN_1404be590, draw FUN_1404bdd50,
//! create FUN_1404be490; and pppRyjMngFlag (202, FUN_1404be8c0).
//!
//! Work variables: `u1` colour variable (bytes +8..+0xb), `u2` emitter
//! block (the drift axis lives in [`Pool`] here). Keyframe fields are read
//! from the raw DPD bytes at the cursor offsets the exe uses.
//!
//! Uncertain:
//! - The random sequence (CRT rand, seed and call history) is not the
//!   original's, so particle placement differs in detail.
//! - `eda0` (the billboard basis, FUN_1402c3310) is rebuilt from the
//!   instance matrix as the decompilation shows: Y axis of view·world,
//!   its XY perpendicular and their cross product.
//! - FUN_140140560's argument order (float XYZ rotation) is taken as the
//!   instance matrix convention (RotX, RotY, RotZ applied in that order).
//! - The per-slot colour accumulator (key +0x101, FUN_1404bd910) and the
//!   spawn-time world matrix snapshot (key +0x106, FUN_1404be590) are
//!   modelled in [`Slot`].
//! - Part pause (`inst+0xc1`) and the +0xc2 camera offset are not
//!   interpreted (613 sets only the draw layer through pppRyjMngFlag).
use crate::math::{self, IDENTITY, Mat, Vec4};
use crate::models::cvtt;
use crate::programs::{byte, float, key, use_val, word};
use crate::{Ctx, ModelDraw, Part};
use data::object_effect::{Program, i16_at};

const POOL_MAX: usize = 4096;

#[derive(Debug, Clone, Copy, PartialEq)]
struct Slot {
    /// +0 v axis, +0x10 direction, +0x20 u axis, +0x30 position.
    frame: Mat,
    life: i16,
    colour: [i8; 4],
    angle: [i32; 3],
    angle_vel: [i32; 3],
    angle_rnd: [i32; 3],
    scale: [f32; 3],
    scale_vel: [f32; 3],
    scale_rnd: [f32; 3],
    speed: f32,
    drift: f32,
    scroll: [f32; 2],
    scroll_vel: [f32; 2],
    alpha_reduction: f32,
    age: u8,
    fade_in: u8,
    fade_out: u8,
    /// The colour accumulator (key +0x101): the added colour and its rate.
    acc: Option<[f32; 8]>,
    /// The instance world matrix at spawn (key +0x106).
    snapshot: Option<Mat>,
}

impl Default for Slot {
    fn default() -> Self {
        Slot {
            frame: IDENTITY,
            life: 0,
            colour: [0; 4],
            angle: [0; 3],
            angle_vel: [0; 3],
            angle_rnd: [0; 3],
            scale: [0.; 3],
            scale_vel: [0.; 3],
            scale_rnd: [0.; 3],
            speed: 0.,
            drift: 0.,
            scroll: [0.; 2],
            scroll_vel: [0.; 2],
            alpha_reduction: 0.,
            age: 0,
            fade_in: 0,
            fade_out: 0,
            acc: None,
            snapshot: None,
        }
    }
}

#[derive(Debug, Clone, Default)]
pub(crate) struct Pool {
    slots: Vec<Slot>,
    axis: Vec4,
    timer: u16,
    vsf_cursor: u16,
}

fn normalize3(v: Vec4) -> Vec4 {
    let l = (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt();
    if l > 0. {
        [v[0] / l, v[1] / l, v[2] / l, v[3]]
    } else {
        v
    }
}

fn cross(a: Vec4, b: Vec4) -> Vec4 {
    [
        a[1] * b[2] - a[2] * b[1],
        a[2] * b[0] - a[0] * b[2],
        a[0] * b[1] - a[1] * b[0],
        0.,
    ]
}

pub(crate) fn create(p: &Program, part: &mut Part, ctx: &mut Ctx) {
    let u2 = use_val(ctx, p, 2);
    part.zero(u2, 0x24);
    part.mega_model.remove(&p.use_val);
}

pub(crate) fn destroy(p: &Program, part: &mut Part, _ctx: &mut Ctx) {
    part.mega_model.remove(&p.use_val);
}

fn wrap_angle(a: i32) -> i32 {
    if (a as u32).wrapping_add(0x8000) > 0xffff {
        let mut a = a;
        while a > 0x7fff {
            a -= 0x10000;
        }
        while a < -0x8000 {
            a += 0x10000;
        }
        a
    } else {
        a
    }
}

/// FUN_1404be1f0.
pub(crate) fn update(p: &Program, cursor: usize, part: &mut Part, ctx: &mut Ctx) {
    let colour_var = use_val(ctx, p, 1);
    let mut pool = match part.mega_model.remove(&p.use_val) {
        Some(pool) => pool,
        None => {
            let n =
                (i16_at(&ctx.dpd.bytes, cursor + 0x1c).unwrap_or(0) as u16 as usize).min(POOL_MAX);
            let axis = [
                float(ctx, cursor + 0xd0),
                float(ctx, cursor + 0xd4),
                float(ctx, cursor + 0xd8),
                0.,
            ];
            Pool {
                slots: vec![Slot::default(); n],
                axis: normalize3(axis),
                timer: 0,
                vsf_cursor: 0,
            }
        }
    };
    if !ctx.skip && !pool.slots.is_empty() {
        pool.timer = pool.timer.wrapping_add(1);
        let interval = half(ctx, cursor + 0x20);
        let max_births = half(ctx, cursor + 0x1e) as usize;
        let mut births = 0;
        for i in 0..pool.slots.len() {
            if pool.slots[i].life == 0 {
                if interval <= pool.timer && births < max_births {
                    let slot = spawn(ctx, cursor, part, colour_var, &mut pool);
                    pool.slots[i] = slot;
                    births += 1;
                }
            } else {
                let axis = pool.axis;
                step(ctx, cursor, part, colour_var, axis, &mut pool.slots[i]);
            }
        }
        if births > 0 {
            pool.timer = 0;
        }
    }
    part.mega_model.insert(p.use_val, pool);
}

fn half(ctx: &Ctx, at: usize) -> u16 {
    i16_at(&ctx.dpd.bytes, at).unwrap_or(0) as u16
}

fn random_signs(ctx: &mut Ctx, flags: u8, v: &mut [f32; 3], shared: bool) {
    if flags & 3 == 3 {
        if shared {
            if ctx.rng.unit() > 0.5 {
                v.iter_mut().for_each(|x| *x = -*x);
            }
        } else {
            for x in v.iter_mut() {
                if ctx.rng.unit() > 0.5 {
                    *x = -*x;
                }
            }
        }
    } else if flags & 2 != 0 {
        v.iter_mut().for_each(|x| *x = -*x);
    }
}

/// FUN_1404bc9e0 (modes 0..9).
fn spawn(ctx: &mut Ctx, cursor: usize, part: &Part, colour_var: usize, pool: &mut Pool) -> Slot {
    let mode = byte(ctx, cursor + 0x26);
    let mult = [
        float(ctx, cursor + 0xf4),
        float(ctx, cursor + 0xf8),
        float(ctx, cursor + 0xfc),
    ];
    let radius = float(ctx, cursor + 0xf0);
    let distribution = byte(ctx, cursor + 0x100);
    let mut s = Slot::default();
    if mode < 8 {
        let c = byte(ctx, cursor + 0x27) as f32;
        let base = [
            float(ctx, cursor + 0xc4),
            float(ctx, cursor + 0xc8),
            float(ctx, cursor + 0xcc),
            1.,
        ];
        let mut r: [f32; 3] = std::array::from_fn(|_| {
            let u = ctx.rng.unit();
            ((u * (c + c) - c) * std::f32::consts::PI) / 180.
        });
        if (2..4).contains(&mode) {
            r[0] = 0.;
        }
        let rot = math::rot_radians(&IDENTITY, r);
        let mut dir = math::apply(&rot, base);
        dir[0] *= mult[0];
        dir[1] *= mult[1];
        dir[2] *= mult[2];
        dir[3] = 0.;
        dir = normalize3(dir);
        let v = s.frame[0];
        let u = normalize3(cross(dir, v));
        let v = normalize3(cross(u, dir));
        s.frame[0] = v;
        s.frame[1] = dir;
        s.frame[2] = u;
    }
    let u = |ctx: &mut Ctx| ctx.rng.unit();
    match mode {
        0..=3 | 10.. => {
            if radius != 0. {
                let rr = match distribution {
                    1 => {
                        u(ctx);
                        u(ctx) * radius
                    }
                    2 => {
                        let a = u(ctx);
                        let b = u(ctx);
                        a * radius * b
                    }
                    3 => {
                        let a = u(ctx);
                        let b = u(ctx);
                        radius - radius * a * b * 0.7
                    }
                    4 => {
                        let a = u(ctx);
                        let b = u(ctx);
                        let c = u(ctx);
                        let d = u(ctx);
                        a * radius * b * c * d
                    }
                    5 => {
                        let a = u(ctx);
                        let b = u(ctx);
                        let c = u(ctx);
                        radius - radius * a * b * c * 0.5
                    }
                    _ => u(ctx) * radius,
                };
                let d = s.frame[1];
                s.frame[3] = [d[0] * rr, d[1] * rr, d[2] * rr, 1.];
            }
        }
        4 | 5 => {
            if radius != 0. {
                let half = radius * 0.5;
                let mut pos = [0.; 3];
                for (k, slot) in pos.iter_mut().enumerate() {
                    *slot = match distribution {
                        1 => {
                            if k == 0 {
                                u(ctx);
                            }
                            u(ctx) * radius - half
                        }
                        2 => {
                            let a = u(ctx);
                            let b = u(ctx);
                            a * radius * b - half
                        }
                        3 => {
                            let a = u(ctx);
                            let b = u(ctx);
                            (radius - radius * a * b * 0.7) - half
                        }
                        4 => {
                            let a = u(ctx);
                            let b = u(ctx);
                            let c = u(ctx);
                            let d = u(ctx);
                            a * radius * b * c * d - half
                        }
                        5 => {
                            let a = u(ctx);
                            let b = u(ctx);
                            let c = u(ctx);
                            (radius - radius * a * b * c * 0.5) - half
                        }
                        _ => u(ctx) * radius - half,
                    };
                }
                s.frame[3] = [pos[0] * mult[0], pos[1] * mult[1], pos[2] * mult[2], 1.];
            }
        }
        _ => {
            let vsf_id = i16_at(&ctx.dpd.bytes, cursor + 0x108).unwrap_or(-1);
            if vsf_id >= 0
                && let Some(vsf) = ctx.dpd.vsf.get(&(vsf_id as i32))
                && !vsf.indices.is_empty()
            {
                let n = vsf.indices.len();
                let pick = match distribution {
                    1 => {
                        u(ctx);
                        Some(u(ctx))
                    }
                    2 => {
                        let a = u(ctx);
                        let b = u(ctx);
                        let c = u(ctx);
                        Some(c * b * a)
                    }
                    3 => {
                        let a = u(ctx);
                        let b = u(ctx);
                        let c = u(ctx);
                        Some(1. - c * b * a)
                    }
                    4 => {
                        let a = u(ctx);
                        let b = u(ctx);
                        let c = u(ctx);
                        let d = u(ctx);
                        Some(d * b * a * c)
                    }
                    5 => {
                        let a = u(ctx);
                        let b = u(ctx);
                        let c = u(ctx);
                        let d = u(ctx);
                        let e = u(ctx);
                        Some(1. - e * b * a * c * d)
                    }
                    _ => None,
                };
                let index = match pick {
                    Some(f) => {
                        if n <= pool.vsf_cursor as usize {
                            pool.vsf_cursor = 0;
                        }
                        vsf.indices[(((n as f32) * f) as usize).min(n - 1)]
                    }
                    None => {
                        let mut c = pool.vsf_cursor as usize;
                        if n <= c {
                            c = 0;
                        }
                        pool.vsf_cursor = (c + 1) as u16;
                        vsf.indices[c]
                    }
                };
                let v = match &part.vertex_override {
                    Some(m) => m.get(index as usize).copied(),
                    None => ctx.dpd.vertex(vsf.model, index),
                };
                if let Some(v) = v {
                    s.frame[3] = [
                        v[0] as f32 * mult[0],
                        v[1] as f32 * mult[1],
                        v[2] as f32 * mult[2],
                        1.,
                    ];
                    if mode >= 8 {
                        let dir = normalize3([
                            s.frame[1][0] * mult[0],
                            s.frame[1][1] * mult[1],
                            s.frame[1][2] * mult[2],
                            0.,
                        ]);
                        let up = normalize3(cross(dir, s.frame[0]));
                        let side = normalize3(cross(up, dir));
                        s.frame[1] = dir;
                        s.frame[2] = up;
                        s.frame[0] = side;
                    }
                }
            }
        }
    }
    // Colour offsets (+0x48..+0x4b ranges).
    for i in 0..4 {
        let b = byte(ctx, cursor + 0x48 + i) as f32;
        u(ctx);
        let r = u(ctx);
        s.colour[i] = (r * (b * 2.) - (byte(ctx, cursor + 0x48 + i) >> 1) as f32) as i32 as i8;
    }
    let alpha = part.u8(colour_var + 0xb) as f32;
    if byte(ctx, cursor + 0x24) != 0 {
        s.alpha_reduction = alpha;
        s.fade_in = byte(ctx, cursor + 0x24);
    }
    if byte(ctx, cursor + 0x25) != 0 {
        s.fade_out = byte(ctx, cursor + 0x25);
    }
    // Angles.
    for i in 0..3 {
        s.angle[i] = word(ctx, cursor + 0x4c + 4 * i) as i32;
        s.angle_vel[i] = word(ctx, cursor + 0x58 + 4 * i) as i32;
    }
    let flags = byte(ctx, cursor + 0x103);
    if flags != 0 {
        let range = |ctx: &Ctx, i: usize| word(ctx, cursor + 0x70 + 4 * i) as i32 as f32;
        let r0 = ctx.rng.unit() * range(ctx, 0);
        let mut rnd: [f32; 3];
        if flags & 0x20 == 0 {
            let r1 = range(ctx, 1) * ctx.rng.unit();
            let r2 = range(ctx, 2) * ctx.rng.unit();
            rnd = [r0.trunc(), r1.trunc(), r2.trunc()];
            random_signs(ctx, flags, &mut rnd, false);
        } else {
            rnd = [r0.trunc(); 3];
            random_signs(ctx, flags, &mut rnd, true);
        }
        s.angle_rnd = rnd.map(|v| v as i32);
    }
    if flags & 4 != 0 {
        for i in 0..3 {
            s.angle[i] = s.angle[i].wrapping_add(s.angle_rnd[i]);
        }
    }
    if flags & 8 != 0 {
        for i in 0..3 {
            s.angle_vel[i] = s.angle_vel[i].wrapping_add(s.angle_rnd[i]);
        }
    }
    for a in s.angle.iter_mut() {
        *a = wrap_angle(*a);
    }
    // Scale.
    for i in 0..3 {
        s.scale[i] = float(ctx, cursor + 0x7c + 4 * i);
        s.scale_vel[i] = float(ctx, cursor + 0x88 + 4 * i);
    }
    let sflags = byte(ctx, cursor + 0x102);
    if sflags != 0 {
        let r0 = float(ctx, cursor + 0xa0) * ctx.rng.unit();
        let mut rnd: [f32; 3];
        if sflags & 0x20 == 0 {
            let r1 = ctx.rng.unit() * float(ctx, cursor + 0xa4);
            let r2 = ctx.rng.unit() * float(ctx, cursor + 0xa8);
            rnd = [r0, r1, r2];
            random_signs(ctx, sflags, &mut rnd, false);
        } else {
            rnd = [r0; 3];
            random_signs(ctx, sflags, &mut rnd, true);
        }
        s.scale_rnd = rnd;
    }
    if sflags & 4 != 0 {
        for i in 0..3 {
            s.scale[i] += s.scale_rnd[i];
        }
    }
    if sflags & 8 != 0 {
        for i in 0..3 {
            s.scale_vel[i] += s.scale_rnd[i];
        }
    }
    // Speed, drift and scroll.
    s.speed = float(ctx, cursor + 0xdc);
    s.drift = float(ctx, cursor + 0xe8);
    let jitter = float(ctx, cursor + 0xe4);
    if jitter != 0. {
        s.speed += ((jitter + jitter) * ctx.rng.unit() - jitter) + 0.;
    }
    s.scroll = [float(ctx, cursor + 0xac), float(ctx, cursor + 0xb8)];
    s.scroll_vel = [float(ctx, cursor + 0xb0), float(ctx, cursor + 0xbc)];
    match byte(ctx, cursor + 0x104) {
        1 => s.scroll[0] *= ctx.rng.unit(),
        2 => {
            let r = ctx.rng.unit();
            s.scroll[1] *= r;
        }
        3 => {
            let r = ctx.rng.unit();
            s.scroll[0] *= r;
            s.scroll[1] *= r;
        }
        4 => {
            s.scroll[0] *= ctx.rng.unit();
            let r = ctx.rng.unit();
            s.scroll[1] *= r;
        }
        _ => {}
    }
    let life = i16_at(&ctx.dpd.bytes, cursor + 0x22).unwrap_or(0);
    s.life = if life == 0 { -1 } else { life };
    // Spawn: the colour accumulator starts at zero with the key's rate
    // (+0x28..), the snapshot is the instance world matrix.
    if byte(ctx, cursor + 0x101) != 0 {
        let mut acc = [0f32; 8];
        for (i, a) in acc[4..].iter_mut().enumerate() {
            *a = float(ctx, cursor + 0x28 + 4 * i);
        }
        s.acc = Some(acc);
    }
    if byte(ctx, cursor + 0x106) != 0 {
        s.snapshot = Some(ctx.world);
    }
    s
}

/// FUN_1404bd910.
fn step(ctx: &mut Ctx, cursor: usize, part: &Part, colour_var: usize, axis: Vec4, s: &mut Slot) {
    let mut alpha = part.u8(colour_var + 0xb) as f32;
    // FUN_1404bd910's colour accumulator: value += rate, rate += key; the
    // fade reads the part alpha plus the accumulated alpha, capped at 255.
    if let Some(acc) = s.acc.as_mut() {
        for i in 0..4 {
            acc[i] += acc[4 + i];
        }
        for i in 0..4 {
            acc[4 + i] += float(ctx, cursor + 0x38 + 4 * i);
        }
        alpha = (acc[3] as i32 + part.u8(colour_var + 0xb) as i32).min(0xff) as f32;
    }
    let flags = byte(ctx, cursor + 0x103);
    for i in 0..3 {
        s.angle[i] = s.angle[i].wrapping_add(s.angle_vel[i]);
    }
    for i in 0..3 {
        let mut add = word(ctx, cursor + 0x64 + 4 * i) as i32;
        if flags & 0x10 != 0 {
            add = add.wrapping_add(s.angle_rnd[i]);
        }
        s.angle_vel[i] = s.angle_vel[i].wrapping_add(add);
    }
    for a in s.angle.iter_mut() {
        *a = wrap_angle(*a);
    }
    let sflags = byte(ctx, cursor + 0x102);
    for i in 0..3 {
        s.scale[i] += s.scale_vel[i];
    }
    for i in 0..3 {
        let mut add = float(ctx, cursor + 0x94 + 4 * i);
        if sflags & 0x10 != 0 {
            add += s.scale_rnd[i];
        }
        s.scale_vel[i] += add;
    }
    let initial = float(ctx, cursor + 0xdc);
    let accel = float(ctx, cursor + 0xe0);
    s.speed += accel;
    if byte(ctx, cursor + 0x10b) == 0 {
        let stop = if initial > 0. && accel < 0. {
            Some(s.speed > 0.)
        } else if initial < 0. && accel > 0. {
            Some(s.speed < 0.)
        } else {
            None
        };
        if let Some(moving) = stop
            && !moving
            && s.speed != 0.
        {
            s.speed = 0.;
        }
    }
    s.drift += float(ctx, cursor + 0xec);
    let d = s.frame[1];
    for i in 0..3 {
        s.frame[3][i] += d[i] * s.speed + axis[i] * s.drift;
    }
    s.scroll[0] += s.scroll_vel[0];
    s.scroll[1] += s.scroll_vel[1];
    s.scroll_vel[0] += float(ctx, cursor + 0xb4);
    s.scroll_vel[1] += float(ctx, cursor + 0xc0);
    if i16_at(&ctx.dpd.bytes, cursor + 0x22).unwrap_or(0) != 0 {
        s.life = s.life.wrapping_sub(1);
    }
    s.age = s.age.wrapping_add(1);
    if s.fade_in != 0 && s.age <= s.fade_in {
        s.alpha_reduction -= alpha / s.fade_in as f32;
    }
    if s.fade_out != 0 && (s.life as u16) <= s.fade_out as u16 {
        let out = byte(ctx, cursor + 0x25) as f32;
        s.alpha_reduction += alpha / out;
    }
}

/// FUN_1404be590: the slot's draw matrix.
fn slot_matrix(
    ctx: &Ctx,
    cur: &Mat,
    mode: u8,
    matrix_mode: u8,
    emitter_world: &Mat,
    s: &Slot,
) -> Mat {
    let mut m = if matrix_mode == 0 {
        let mut m = IDENTITY;
        m[3] = s.frame[3];
        m
    } else {
        s.frame
    };
    if s.angle.iter().any(|&a| a != 0) {
        let k = std::f32::consts::PI * (1. / 32768.);
        let r = math::rot_radians(&IDENTITY, s.angle.map(|a| a as f32 * k));
        m = math::mul(&m, &r);
    }
    for (c, col) in m.iter_mut().take(3).enumerate() {
        *col = math::scale(*col, s.scale[c]);
    }
    if matches!(mode, 1 | 3 | 5 | 7 | 9) {
        let t = math::apply(emitter_world, m[3]);
        let mut b = billboard(&ctx.ed60);
        b[3] = [0., 0., 0., 1.];
        m = math::mul(&b, &m);
        m[3] = t;
    } else {
        let t = m[3];
        m = math::mul(&ctx.ed60, &m);
        // With a snapshot the position goes through view · snapshot · cur.
        m[3] = match &s.snapshot {
            Some(snap) => math::apply(&math::mul(&ctx.view, &math::mul(snap, cur)), t),
            None => math::apply(emitter_world, t),
        };
    }
    math::mul(&ctx.ede0, &m)
}

/// DAT_142b0eda0: the camera-facing basis of FUN_1402c3310.
fn billboard(ed60: &Mat) -> Mat {
    let yv = normalize3([ed60[1][0], ed60[1][1], ed60[1][2], 0.]);
    let right = normalize3([yv[1], -yv[0], 0., 0.]);
    let third = normalize3(cross(right, yv));
    [right, yv, third, [0., 0., 0., 1.]]
}

/// FUN_1404bdd50.
pub(crate) fn draw(p: &Program, cursor: usize, part: &mut Part, ctx: &mut Ctx) {
    let colour_var = use_val(ctx, p, 1);
    let Some(pool) = part.mega_model.get(&p.use_val) else {
        return;
    };
    let model = word(ctx, cursor + 4);
    if model == 0xffff || !ctx.dpd.models.contains_key(&(model as i16)) {
        return;
    }
    let mode = byte(ctx, cursor + 0x26);
    let matrix_mode = byte(ctx, cursor + 0x105);
    let emitter_world = math::mul(&ctx.ed60, &part.cur);
    let base: [i32; 4] = std::array::from_fn(|i| part.u8(colour_var + 8 + i) as i32);
    for s in &pool.slots {
        if s.life == 0 {
            continue;
        }
        let a = slot_matrix(ctx, &part.cur, mode, matrix_mode, &emitter_world, s);
        let added: [i32; 4] = s
            .acc
            .map_or([0; 4], |a| std::array::from_fn(|i| a[i] as i32));
        let c = |i: usize| (s.colour[i] as i32 + base[i] + added[i]).clamp(0, 0xff) as u8;
        let alpha = ((s.colour[3] as f32 + base[3] as f32) - s.alpha_reduction) as i32 + added[3];
        ctx.model_draws.push(ModelDraw {
            model: model as i16,
            vertices: part.vertex_override.clone(),
            a,
            b: None,
            rgba: [c(0), c(1), c(2), alpha.clamp(0, 0x7f) as u8],
            scroll: [cvtt(s.scroll[0] * 65536.), cvtt(s.scroll[1] * 65536.)],
            mode: None,
            depth_only: false,
        });
    }
}

/// FUN_1404be8c0 (pppRyjMngFlag): at its keyframe time the instance flag
/// bytes change (+4 camera offset, +5 pause, +6 draw layer).
pub(crate) fn mng_flag(cursor: usize, part: &Part, ctx: &mut Ctx) {
    if key(ctx, part, cursor) {
        *ctx.flags = [
            byte(ctx, cursor + 6),
            byte(ctx, cursor + 5),
            byte(ctx, cursor + 4),
        ];
    }
}
