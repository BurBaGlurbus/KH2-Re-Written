//! pppRyjMegaBirthModelFilter (type 229): models scattered in a ball around
//! the camera, such as snow or dust. Translated from update FUN_1404db2c0,
//! tick FUN_1404da3d0, spawn FUN_1404da600, step FUN_1404d9dd0, draw
//! FUN_1404dad90, create FUN_1404db460 and the view hook FUN_1404db540.
//!
//! A slot starts at a random point within radius `+0xd8 + +0xdc` of the camera
//! and drifts along a fixed axis. Once outside that radius it comes back on
//! the camera's far side, along its own line turned by a small random angle,
//! with its look drawn again. It fades over `+0xd4` from `+0xd8` out, and near
//! the camera over the same width below view depth `+0xd0`; slots behind the
//! camera aren't drawn. Each sits at its view-space position with no view
//! rotation, so it faces the screen. `+0xf0` fades the whole effect in by the
//! camera's distance from the instance.
//!
//! Uncertain:
//! - `DAT_142b0ed20` / `ece0` are read as the camera's position and its
//!   inverse view; the view hook's forward as the inverse view's z axis.
//! - FUN_140140200 is read as a three-component dot.
//! - The draw's flag 0x200 (key byte +8) and `inst+0xc3`, which create sets,
//!   aren't interpreted. `inst+0xc1` (pause) is not read, as in mega_model.
use crate::math::{self, IDENTITY, Mat, Vec4};
use crate::models::cvtt;
use crate::programs::{byte, float, use_val, word};
use crate::{Ctx, ModelDraw, Part};
use data::object_effect::{Program, i16_at};

pub(crate) const KIND: u32 = 229;

const POOL_MAX: usize = 4096;
/// FUN_1404da600's coin (0.5).
const COIN: f32 = 0.5;

/// A slot of 0xa0 bytes.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub(crate) struct Slot {
    pub(crate) position: Vec4,
    direction: Vec4,
    life: u16,
    colour: [i8; 4],
    angle: [i32; 3],
    angle_vel: [i32; 3],
    angle_rnd: [i32; 3],
    scale: [f32; 3],
    scale_vel: [f32; 3],
    scale_rnd: [f32; 3],
    pub(crate) speed: f32,
    scroll: [f32; 2],
    scroll_vel: [f32; 2],
    alpha: f32,
    fade: u8,
    flags: u8,
    /// The second pool (key byte +0xf2): the added colour and its rate.
    added: [f32; 4],
    rate: [f32; 4],
}

#[derive(Debug, Clone)]
pub(crate) struct Pool {
    pub(crate) slots: Vec<Slot>,
    timer: u16,
    /// +0xc and +0x12: the camera-distance fade.
    alpha: f32,
    shown: bool,
}

/// The camera's world position (`DAT_142b0ed20`).
fn eye(ctx: &Ctx) -> Vec4 {
    math::inverse(&ctx.view)[3]
}

fn sub(a: Vec4, b: Vec4) -> Vec4 {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2], a[3] - b[3]]
}

fn length(v: Vec4) -> f32 {
    (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt()
}

/// FUN_1404db460.
pub(crate) fn create(p: &Program, part: &mut Part, _ctx: &mut Ctx) {
    part.model_filter.insert(
        p.use_val,
        Pool {
            slots: Vec::new(),
            timer: 0,
            alpha: 1.,
            shown: true,
        },
    );
}

/// FUN_1404db4d0.
pub(crate) fn destroy(p: &Program, part: &mut Part, _ctx: &mut Ctx) {
    part.model_filter.remove(&p.use_val);
}

/// FUN_1404db2c0.
pub(crate) fn update(p: &Program, cursor: usize, part: &mut Part, ctx: &mut Ctx) {
    let Some(mut pool) = part.model_filter.remove(&p.use_val) else {
        return;
    };
    if pool.slots.is_empty() {
        let n = i16_at(&ctx.dpd.bytes, cursor + 0xa).unwrap_or(0) as u16 as usize;
        pool.slots = vec![Slot::default(); n.min(POOL_MAX)];
    }
    if byte(ctx, cursor + 0xf0) != 0 {
        let within = float(ctx, cursor + 0xe8);
        let ramp = float(ctx, cursor + 0xec);
        let distance = length(sub(eye(ctx), ctx.origin));
        pool.shown = distance <= within;
        if pool.shown {
            let v = within - distance;
            pool.alpha = if ramp <= v { 1. } else { v / ramp };
        }
    }
    if pool.shown {
        tick(cursor, &mut pool, ctx);
    }
    part.model_filter.insert(p.use_val, pool);
}

/// FUN_1404da3d0.
fn tick(cursor: usize, pool: &mut Pool, ctx: &mut Ctx) {
    if ctx.skip || word(ctx, cursor + 4) == 0xffff {
        return;
    }
    pool.timer = pool.timer.wrapping_add(1);
    let every = half(ctx, cursor + 0xe);
    let most = half(ctx, cursor + 0xc);
    let radius = float(ctx, cursor + 0xd8) + float(ctx, cursor + 0xdc);
    let to_world = math::inverse(&ctx.view);
    let mut born = 0;
    for slot in &mut pool.slots {
        if slot.life != 0 {
            step(ctx, cursor, slot);
        } else if every <= pool.timer && born < most {
            *slot = Slot::default();
            slot.direction = math::normalize([
                float(ctx, cursor + 0xb8),
                float(ctx, cursor + 0xbc),
                float(ctx, cursor + 0xc0),
                0.,
            ]);
            // Rejection within the ball.
            let point = loop {
                let v = [
                    ctx.rng.unit() * (radius + radius) - radius,
                    ctx.rng.unit() * (radius + radius) - radius,
                    ctx.rng.unit() * (radius + radius) - radius,
                    1.,
                ];
                if length(v) <= radius {
                    break v;
                }
            };
            slot.position = math::apply(&to_world, point);
            spawn(ctx, cursor, slot);
            born += 1;
        }
    }
    if born > 0 {
        pool.timer = 0;
    }
}

fn half(ctx: &Ctx, at: usize) -> u16 {
    i16_at(&ctx.dpd.bytes, at).unwrap_or(0) as u16
}

/// FUN_1404da600 reduces an angle out of range by C's `% 0x8000`.
fn wrap_spawned(a: i32) -> i32 {
    if (a as u32).wrapping_add(0x8000) > 0xffff {
        a % 0x8000
    } else {
        a
    }
}

/// The random signs of a look's rate: `flags & 3 == 3` each its own coin (or
/// one for all with bit 0x20), bit 2 alone turns them all.
fn signs<const N: usize>(ctx: &mut Ctx, flags: u8, v: &mut [f32; N]) {
    if flags & 3 == 3 {
        if flags & 0x20 == 0 {
            for x in v.iter_mut() {
                if ctx.rng.unit() > COIN {
                    *x = -*x;
                }
            }
        } else if ctx.rng.unit() > COIN {
            *v = v.map(|x| -x);
        }
    } else if flags & 2 != 0 {
        *v = v.map(|x| -x);
    }
}

/// The rate of a look (key flags, base key): shared by all axes with 0x20.
fn drawn<const N: usize>(ctx: &mut Ctx, cursor: usize, flags: u8, keys: [usize; N]) -> [f32; N] {
    let first = ctx.rng.unit() * float(ctx, cursor + keys[0]);
    let mut v = [first; N];
    if flags & 0x20 == 0 {
        for (x, k) in v.iter_mut().zip(keys).skip(1) {
            *x = ctx.rng.unit() * float(ctx, cursor + k);
        }
    }
    signs(ctx, flags, &mut v);
    v
}

/// FUN_1404da600.
fn spawn(ctx: &mut Ctx, cursor: usize, s: &mut Slot) {
    for i in 0..4 {
        let b = byte(ctx, cursor + 0x34 + i);
        let r = ctx.rng.unit();
        s.colour[i] = (r * f32::from(u16::from(b) * 2) - f32::from(b >> 1)) as i32 as i8;
    }
    let fade_in = byte(ctx, cursor + 0x12);
    if fade_in != 0 {
        s.flags |= 2;
        s.fade = fade_in;
    }
    let int = |ctx: &Ctx, at: usize| word(ctx, cursor + at) as i32;
    s.angle = [int(ctx, 0x38), int(ctx, 0x3c), int(ctx, 0x40)];
    s.angle_vel = [int(ctx, 0x44), int(ctx, 0x48), int(ctx, 0x4c)];
    let flags = byte(ctx, cursor + 0xf4);
    if flags != 0 {
        let keys = [int(ctx, 0x5c), int(ctx, 0x60), int(ctx, 0x64)];
        let first = (ctx.rng.unit() * keys[0] as f32) as i32 as f32;
        let mut v = [first; 3];
        if flags & 0x20 == 0 {
            v[1] = (ctx.rng.unit() * keys[1] as f32) as i32 as f32;
            v[2] = (ctx.rng.unit() * keys[2] as f32) as i32 as f32;
        }
        signs(ctx, flags, &mut v);
        s.angle_rnd = v.map(|x| x as i32);
    }
    for i in 0..3 {
        if flags & 4 != 0 {
            s.angle[i] = s.angle[i].wrapping_add(s.angle_rnd[i]);
        }
        if flags & 8 != 0 {
            s.angle_vel[i] = s.angle_vel[i].wrapping_add(s.angle_rnd[i]);
        }
        s.angle[i] = wrap_spawned(s.angle[i]);
    }
    s.scale = [0x68, 0x6c, 0x70].map(|k| float(ctx, cursor + k));
    s.scale_vel = [0x74, 0x78, 0x7c].map(|k| float(ctx, cursor + k));
    let flags = byte(ctx, cursor + 0xf3);
    if flags != 0 {
        s.scale_rnd = drawn(ctx, cursor, flags, [0x8c, 0x90, 0x94]);
    }
    for i in 0..3 {
        if flags & 4 != 0 {
            s.scale[i] += s.scale_rnd[i];
        }
        if flags & 8 != 0 {
            s.scale_vel[i] += s.scale_rnd[i];
        }
    }
    s.speed = float(ctx, cursor + 0xc4);
    let spread = float(ctx, cursor + 0xcc);
    if spread != 0. {
        s.speed += (spread + spread) * ctx.rng.unit() - spread;
    }
    s.scroll = [float(ctx, cursor + 0x98), float(ctx, cursor + 0xa8)];
    s.scroll_vel = [float(ctx, cursor + 0x9c), float(ctx, cursor + 0xac)];
    let flags = byte(ctx, cursor + 0xf5);
    if flags != 0 {
        let v = drawn(ctx, cursor, flags, [0xa4, 0xb4]);
        for (i, d) in v.into_iter().enumerate() {
            if flags & 4 != 0 {
                s.scroll[i] += d;
            }
            if flags & 8 != 0 {
                s.scroll_vel[i] += d;
            }
        }
    }
    let life = half(ctx, cursor + 0x10);
    s.life = if life == 0 { 0xffff } else { life };
    s.rate = [0x14, 0x18, 0x1c, 0x20].map(|k| float(ctx, cursor + k));
    s.alpha = 0.;
    let mirror = byte(ctx, cursor + 0xf6);
    if mirror & 1 != 0 && ctx.rng.unit() > COIN {
        s.scale[0] = -s.scale[0];
    }
    if mirror & 2 != 0 && ctx.rng.unit() > COIN {
        s.scale[1] = -s.scale[1];
    }
}

/// FUN_1404d9dd0.
fn step(ctx: &mut Ctx, cursor: usize, s: &mut Slot) {
    for i in 0..4 {
        s.added[i] += s.rate[i];
        s.rate[i] += float(ctx, cursor + 0x24 + 4 * i);
    }
    let own_angle = byte(ctx, cursor + 0xf4) & 0x10 != 0;
    let own_scale = byte(ctx, cursor + 0xf3) & 0x10 != 0;
    for i in 0..3 {
        s.angle[i] = s.angle[i].wrapping_add(s.angle_vel[i]);
        let extra = if own_angle { s.angle_rnd[i] } else { 0 };
        s.angle_vel[i] = s.angle_vel[i]
            .wrapping_add(extra)
            .wrapping_add(word(ctx, cursor + 0x50 + 4 * i) as i32);
        while s.angle[i] > 0x7fff {
            s.angle[i] -= 0x10000;
        }
        s.scale[i] += s.scale_vel[i];
        let extra = if own_scale { s.scale_rnd[i] } else { 0. };
        s.scale_vel[i] += extra + float(ctx, cursor + 0x80 + 4 * i);
    }
    s.speed += float(ctx, cursor + 0xc8);
    let p = s.position;
    let d = s.direction;
    s.position = [
        p[0] + d[0] * s.speed,
        p[1] + d[1] * s.speed,
        p[2] + d[2] * s.speed,
        1.,
    ];
    let from_eye = sub(s.position, eye(ctx));
    let distance = length(from_eye);
    let unit = math::normalize([from_eye[0], from_eye[1], from_eye[2], 0.]);
    for i in 0..2 {
        s.scroll[i] += s.scroll_vel[i];
        while s.scroll[i] > 2048. {
            s.scroll[i] -= 2048.;
        }
        s.scroll_vel[i] += float(ctx, cursor + [0xa0, 0xb0][i]);
        if s.scroll_vel[i] > 2048. {
            s.scroll_vel[i] = 2048.;
        }
    }
    if half(ctx, cursor + 0x10).wrapping_sub(1) < 0xfffe {
        s.life = s.life.wrapping_sub(1);
    }
    let width = float(ctx, cursor + 0xd4);
    let full = float(ctx, cursor + 0xd8);
    let radius = full + float(ctx, cursor + 0xdc);
    let span = f32::from(byte(ctx, cursor + 0x12));
    let mut f = 1.;
    if s.flags & 2 != 0 {
        if s.fade == 0 {
            s.flags &= !2;
            s.alpha = 1.;
        } else {
            f = 1. - f32::from(s.fade) / span;
            s.fade -= 1;
            s.alpha = f;
        }
    } else if s.flags & 4 != 0 {
        if s.fade == 0 {
            s.flags &= !4;
            s.alpha = 0.;
            f = 0.;
        } else {
            f = f32::from(s.fade) / span;
            s.fade -= 1;
            s.alpha = f;
        }
    } else {
        let out = byte(ctx, cursor + 0x13);
        if out != 0 && s.life <= u16::from(out) {
            s.fade = out;
            s.flags |= 4;
        }
        s.alpha = 1.;
    }
    if distance <= radius {
        if full < distance {
            s.alpha = (1. - (distance - full) / width) * f;
        }
    } else {
        s.alpha = 0.;
    }
    let to_world = math::inverse(&ctx.view);
    let forward = to_world[2];
    let facing = unit[0] * forward[0] + unit[1] * forward[1] + unit[2] * forward[2];
    if facing < 0. {
        s.flags |= 1;
    } else {
        s.flags &= !1;
    }
    if radius < distance {
        let turn = [0, 0, 0].map(|_: i32| ctx.rng.unit() * 1.256_637_1 - 0.628_318_55);
        let r = math::rot_radians(&IDENTITY, turn);
        let v = math::scale(math::apply(&r, unit), -radius);
        let e = eye(ctx);
        s.position = [e[0] + v[0], e[1] + v[1], e[2] + v[2], 1.];
        spawn(ctx, cursor, s);
    }
}

/// The slot's draw matrix and its fade by view depth.
fn slot_matrix(ctx: &Ctx, cursor: usize, s: &mut Slot) -> Mat {
    let mut m = IDENTITY;
    if s.angle.iter().any(|&a| a != 0) {
        let k = std::f32::consts::PI * (1. / 32768.);
        m = math::rot_radians(&IDENTITY, s.angle.map(|a| a as f32 * k));
    }
    for (c, col) in m.iter_mut().take(3).enumerate() {
        *col = math::scale(*col, s.scale[c]);
    }
    let mut at = math::apply(&ctx.view, [s.position[0], s.position[1], s.position[2], 1.]);
    at[1] = at[1] * float(ctx, cursor + 0xe0) + float(ctx, cursor + 0xe4);
    m[3] = [at[0], at[1], at[2], 1.];
    let near = float(ctx, cursor + 0xd0);
    let width = float(ctx, cursor + 0xd4);
    if at[2] < near - width {
        s.alpha = 0.;
    } else if at[2] < near {
        s.alpha *= 1. - (near - at[2]) / width;
    }
    math::mul(&ctx.ede0, &m)
}

/// FUN_1404dad90.
pub(crate) fn draw(p: &Program, cursor: usize, part: &mut Part, ctx: &mut Ctx) {
    let colour_var = use_val(ctx, p, 1);
    let model = word(ctx, cursor + 4);
    if model == 0xffff || !ctx.dpd.models.contains_key(&(model as i16)) {
        return;
    }
    let base: [i32; 4] = std::array::from_fn(|i| part.u8(colour_var + 8 + i) as i32);
    let Some(pool) = part.model_filter.get_mut(&p.use_val) else {
        return;
    };
    if !pool.shown {
        return;
    }
    let extra = byte(ctx, cursor + 0xf2) != 0;
    let whole = pool.alpha;
    for s in &mut pool.slots {
        if s.life == 0 || s.flags & 1 != 0 {
            continue;
        }
        let a = slot_matrix(ctx, cursor, s);
        let added: [i32; 4] = if extra {
            s.added.map(|v| v as i32)
        } else {
            [0; 4]
        };
        let c = |i: usize| (s.colour[i] as i32 + base[i] + added[i]).clamp(0, 0xff) as u8;
        let alpha_base = (s.colour[3] as i32 + base[3] + added[3]).clamp(0, 0x7f);
        let alpha = (alpha_base as f32 * s.alpha * whole) as i32;
        if alpha == 0 {
            continue;
        }
        ctx.model_draws.push(ModelDraw {
            model: model as i16,
            vertices: part.vertex_override.clone(),
            a,
            b: None,
            rgba: [c(0), c(1), c(2), alpha as u8],
            scroll: [cvtt(s.scroll[0] * 65536.), cvtt(s.scroll[1] * 65536.)],
            mode: None,
            depth_only: false,
        });
    }
}
