//! pppRyjMegaBirth (type 201): a pooled sub-particle emitter. Translated
//! from update FUN_1404bc3b0 (with the pool tick FUN_1404bbbe0, the spawn
//! FUN_1404bac40 and the per-slot move FUN_1404bb8b0), draw FUN_1404bbdb0
//! (with the slot matrix FUN_1404bc710 and the shape draw FUN_1404ae550 →
//! FUN_1404ae220), create FUN_1404bc5f0 and destroy FUN_1404bc670.
//!
//! Work variables: `u1` is the colour variable (bytes +8..+0xb as pppColor
//! writes them); `u2` is the 0x64-byte emitter block: +0 emitter matrix,
//! +0x40 drift axis, +0x50/+0x54/+0x58 handles of the slot pool, the side
//! matrices and the colour buffers (held here in `Part::mega`), +0x5c pool
//! size, +0x60 birth timer (i16), +0x62 sequential VSF vertex counter.
//!
//! Keyframe fields are read from the raw DPD bytes at the cursor offsets
//! the exe uses, so the reads past a 212-byte keyframe (+0xd4 shrink-out,
//! +0xd8 drift random) see the following bytes exactly as in the original.
//!
//! Uncertain:
//! - Allocation is assumed to succeed. A pool size of 0 at the first update
//!   gives an empty pool (the exe's FUN_1404af240 with 0 bytes was not
//!   traced; a null handle there would retry on later keyframes).
//! - Spawn modes 6–9 take vertices from the VSF's own model; the exe first
//!   tries the part's bound object (`DAT_142b0ec10 + 0x90`), which this VM
//!   does not model. A VSF index missing from the DPD skips the placement.
//! - FUN_140142080 (the normalise behind FUN_140478d80) was not
//!   disassembled; a zero-length vector is left unchanged here.
//! - With byte +0xe set the exe passes the offset matrix as the packet's
//!   second matrix (+0x28) and the slot matrix as the first (+0x10); like
//!   pppDrawShapeX here, the sprites are placed through the second one.
//!   The draw-time depth branch depends on `DAT_140abb920` (a world/mode
//!   global, 1 and 6 special) and on `DAT_14071c658/65c`; their runtime
//!   values were not traced, so [`WORLD`] is 0 and the statics' initial
//!   values (−1.0) are used.
//! - The packet flags (+0xc, +0xd, +0xf, +0xcd == 1) only steer sorting
//!   and blending in the renderer and are not represented in [`Quad`].
//! - The emitter's colour alpha read at spawn/move time is the byte the
//!   colour program left in this frame; the effect-global pause flags
//!   (`DAT_142b0efc0`, engine +0xc1) are taken as clear.
use crate::math::{self, IDENTITY, Mat, Vec4};
use crate::programs::{byte, emit_sprites, float, read_matrix, use_val, word, write_matrix};
use crate::{Ctx, Part};
use data::object_effect::{self, Program, i16_at};

/// Emitter block fields (offsets inside `V[u2]`).
const E_AXIS: usize = 0x40;
const E_HANDLES: usize = 0x50;
const E_POOL_SIZE: usize = 0x5c;
const E_TIMER: usize = 0x60;
const E_VSF_NEXT: usize = 0x62;

/// `DAT_140abb920` as seen by the draw (see the module notes).
const WORLD: u32 = 0;
/// `DAT_14071c658` / `DAT_14071c65c` static values.
const DEPTH_NEAR: f32 = -1.;
const DEPTH_FAR: f32 = -1.;

const ZERO: Mat = [[0.; 4]; 4];

/// One 0x60-byte pool slot.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
struct Slot {
    /// +0x00 position.
    pos: Vec4,
    /// +0x10 direction of travel (w = 0).
    dir: Vec4,
    /// +0x20 flip-book timer, +0x22 frame index, +0x24 previous index.
    flip_t: u16,
    flip_index: u16,
    flip_prev: u16,
    /// +0x26 remaining ticks; 0 = free, 0xffff = no life limit.
    life: u16,
    /// +0x28 RGBA offsets.
    colour: [i8; 4],
    /// +0x2c ticks since birth.
    age: u8,
    /// +0x2e roll, +0x30 roll velocity, +0x32 random roll (0x10000 = 360°).
    roll: i16,
    roll_vel: i16,
    roll_rnd: i16,
    /// +0x34 scale x/y, +0x3c scale velocity, +0x44 random scale.
    scale: [f32; 2],
    scale_vel: [f32; 2],
    scale_rnd: [f32; 2],
    /// +0x4c speed along `dir`, +0x50 drift along the emitter axis.
    speed: f32,
    drift: f32,
    /// +0x54 alpha reduction, +0x58 size factor.
    fade: f32,
    size: f32,
    /// +0x5c fade-in, +0x5d fade-out, +0x5e grow-in, +0x5f shrink-out ticks.
    fade_in: u8,
    fade_out: u8,
    grow_in: u8,
    shrink_out: u8,
}

/// The emitter's slot pool and its optional side buffers.
#[derive(Debug, Default)]
pub(crate) struct Pool {
    slots: Vec<Slot>,
    /// 0x40-byte matrices, when +0xcc ∈ 1..=3 at allocation.
    side: Option<Vec<Mat>>,
    /// 0x20-byte colour buffers (value, velocity), when +0xc9 at allocation.
    colour: Option<Vec<[Vec4; 2]>>,
}

/// Program context: keyframe cursor, colour variable, emitter block.
#[derive(Clone, Copy)]
struct Key {
    cursor: usize,
    colour: usize,
    e: usize,
}

fn half(ctx: &Ctx, at: usize) -> u16 {
    i16_at(&ctx.dpd.bytes, at).unwrap_or(0) as u16
}

/// CVTTSS2SI: truncation, with the "integer indefinite" for NaN/overflow.
fn cvtt(x: f32) -> i32 {
    if x.is_nan() || !(-2_147_483_648.0..2_147_483_648.0).contains(&x) {
        i32::MIN
    } else {
        x as i32
    }
}

/// thunk_FUN_140140e00: xyz × s, w copied.
fn scale3(v: Vec4, s: f32) -> Vec4 {
    [v[0] * s, v[1] * s, v[2] * s, v[3]]
}

fn add4(a: Vec4, b: Vec4) -> Vec4 {
    std::array::from_fn(|i| a[i] + b[i])
}

/// FUN_140478d80: xyz normalised, w = 0.
fn normalize0(v: Vec4) -> Vec4 {
    let mut n = math::normalize(v);
    n[3] = 0.;
    n
}

/// thunk_FUN_140140560: `m` turned by float radians, Z first, then Y,
/// then X (Rx·Ry·Rz·m); a zero angle skips its turn.
fn rot_zyx(m: &Mat, a: [f32; 3]) -> Mat {
    let mut out = *m;
    if a[2] != 0. {
        let (s, c) = a[2].sin_cos();
        let r = [
            [c, s, 0., 0.],
            [-s, c, 0., 0.],
            [0., 0., 1., 0.],
            [0., 0., 0., 1.],
        ];
        out = math::mul(&r, &out);
    }
    if a[1] != 0. {
        let (s, c) = a[1].sin_cos();
        let r = [
            [c, 0., -s, 0.],
            [0., 1., 0., 0.],
            [s, 0., c, 0.],
            [0., 0., 0., 1.],
        ];
        out = math::mul(&r, &out);
    }
    if a[0] != 0. {
        let (s, c) = a[0].sin_cos();
        let r = [
            [1., 0., 0., 0.],
            [0., c, s, 0.],
            [0., -s, c, 0.],
            [0., 0., 0., 1.],
        ];
        out = math::mul(&r, &out);
    }
    out
}

fn shape_anim(ctx: &Ctx, cursor: usize) -> Option<usize> {
    ctx.dpd
        .shapes
        .get(&(word(ctx, cursor + 4) as i16))
        .map(|s| s.anim)
}

/// FUN_1404bc5f0: identity emitter matrix, zero axis, no pool yet.
pub(crate) fn create(p: &Program, part: &mut Part, ctx: &mut Ctx) {
    let e = use_val(ctx, p, 2);
    write_matrix(part, e, &IDENTITY);
    part.set_vec4(e + E_AXIS, [0.; 4]);
    part.zero(e + E_HANDLES, 0x14);
    part.mega.remove(&p.use_val);
}

/// FUN_1404bc670: the pool and side buffers are released.
pub(crate) fn destroy(p: &Program, part: &mut Part, ctx: &mut Ctx) {
    let e = use_val(ctx, p, 2);
    part.zero(e + E_HANDLES, 0xc);
    part.mega.remove(&p.use_val);
}

/// FUN_1404bc3b0: allocate on first use (pool size, side buffers and drift
/// axis from the current keyframe), rebuild the emitter matrix, then tick.
pub(crate) fn update(p: &Program, cursor: usize, part: &mut Part, ctx: &mut Ctx) {
    let k = Key {
        cursor,
        colour: use_val(ctx, p, 1),
        e: use_val(ctx, p, 2),
    };
    let spaced = byte(ctx, cursor + 0xcc).wrapping_sub(1) < 3;
    let buffered = byte(ctx, cursor + 0xc9) != 0;
    let mut pool = match part.mega.remove(&p.use_val) {
        Some(pool) => pool,
        None => {
            let n = half(ctx, cursor + 0x1c);
            part.set_i32(k.e + E_POOL_SIZE, n as i32);
            let n = n as usize;
            let pool = Pool {
                slots: vec![Slot::default(); n],
                side: spaced.then(|| vec![ZERO; n]),
                colour: buffered.then(|| vec![[[0.; 4]; 2]; n]),
            };
            let axis = [
                float(ctx, cursor + 0x98),
                float(ctx, cursor + 0x9c),
                float(ctx, cursor + 0xa0),
                0.,
            ];
            part.set_vec4(k.e + E_AXIS, normalize0(axis));
            pool
        }
    };
    let ready = !(spaced && pool.side.is_none()) && !(buffered && pool.colour.is_none());
    if ready {
        let m = if matches!(byte(ctx, cursor + 0x26), 1 | 3 | 5 | 7 | 9) {
            // Unrotated: identity axes × world scale at the world origin.
            let mut m = IDENTITY;
            for (c, col) in m.iter_mut().take(3).enumerate() {
                *col = scale3(*col, ctx.scales[c]);
            }
            m[3] = ctx.world[3];
            m[3][3] = 1.;
            m
        } else {
            ctx.world
        };
        write_matrix(part, k.e, &m);
        tick(ctx, k, part, &mut pool);
    }
    part.mega.insert(p.use_val, pool);
}

/// FUN_1404bbbe0: births into free slots once the timer reaches the
/// interval (at most +0x1e per tick), live slots move and advance their
/// flip-book; any birth resets the timer.
fn tick(ctx: &mut Ctx, k: Key, part: &mut Part, pool: &mut Pool) {
    let cursor = k.cursor;
    if ctx.skip || word(ctx, cursor + 4) == 0xffff {
        return;
    }
    let timer = part.i16(k.e + E_TIMER).wrapping_add(1);
    part.set_i16(k.e + E_TIMER, timer);
    let n = part.i32(k.e + E_POOL_SIZE);
    if n <= 0 {
        return;
    }
    let interval = half(ctx, cursor + 0x20);
    let max_births = half(ctx, cursor + 0x1e) as i32;
    let speed = half(ctx, cursor + 8);
    let anim = shape_anim(ctx, cursor);
    let mut births = 0;
    for i in 0..(n as usize).min(pool.slots.len()) {
        let side = pool.side.as_mut().and_then(|s| s.get_mut(i));
        let buffer = pool.colour.as_mut().and_then(|c| c.get_mut(i));
        let slot = &mut pool.slots[i];
        if slot.life == 0 {
            if interval <= timer as u16 && births < max_births {
                spawn(ctx, k, part, slot, side, buffer);
                births += 1;
            }
            continue;
        }
        move_slot(ctx, k, part, slot, buffer);
        let Some(anim) = anim else { continue };
        let b = &ctx.dpd.bytes;
        let index = slot.flip_index;
        let (wait, flag) =
            object_effect::shape_sequence(b, anim, slot.flip_prev as usize).unwrap_or((0, 0));
        slot.flip_prev = index;
        let t = speed.wrapping_add(slot.flip_t);
        slot.flip_t = t;
        if wait as i32 <= t as i32 {
            slot.flip_t = t.wrapping_sub(wait as u16);
            slot.flip_index = index.wrapping_add(1);
            let count = object_effect::shape_sequence_count(b, anim).unwrap_or(0);
            if count as i32 <= index.wrapping_add(1) as i32 {
                slot.flip_t = 0;
                slot.flip_index = if (flag as i8) < 0 { 0 } else { index };
            }
        }
    }
    if births > 0 {
        part.set_i16(k.e + E_TIMER, 0);
    }
}

/// The spawn-radius distributions of byte +0xc8 (r_i independent draws).
fn distributed(ctx: &mut Ctx, kind: u8, r: f32) -> f32 {
    let rng = &mut *ctx.rng;
    match kind {
        1 => {
            rng.unit();
            rng.unit() * r
        }
        2 => {
            let a = rng.unit();
            let b = rng.unit();
            a * r * b
        }
        3 => {
            let a = rng.unit();
            let b = rng.unit();
            r - r * a * b * 0.7
        }
        4 => {
            let a = rng.unit();
            let b = rng.unit();
            let c = rng.unit();
            let d = rng.unit();
            a * r * b * c * d
        }
        5 => {
            let a = rng.unit();
            let b = rng.unit();
            let c = rng.unit();
            r - r * a * b * c * 0.5
        }
        _ => r,
    }
}

/// FUN_1404bac40: a new particle in `slot` from the current keyframe.
fn spawn(
    ctx: &mut Ctx,
    k: Key,
    part: &mut Part,
    slot: &mut Slot,
    side: Option<&mut Mat>,
    buffer: Option<&mut [Vec4; 2]>,
) {
    let c = k.cursor;
    let f = |ctx: &Ctx, o: usize| float(ctx, c + o);
    let cone = byte(ctx, c + 0x27) as f32;
    *slot = Slot::default();
    let mut side = side;
    if let Some(m) = side.as_deref_mut() {
        *m = ZERO;
    }
    let mut buffer = buffer;
    if let Some(b) = buffer.as_deref_mut() {
        *b = [[0.; 4]; 2];
    }
    let mut mode = byte(ctx, c + 0x26);
    if mode < 8 {
        // Direction: the base direction turned by random angles within
        // ±cone degrees (only Z for modes 2 and 3), ⊙ multipliers.
        let base = [f(ctx, 0x8c), f(ctx, 0x90), f(ctx, 0x94), 0.];
        let mut a = [0f32; 3];
        for x in &mut a {
            *x = ((ctx.rng.unit() * (cone + cone) - cone) * std::f32::consts::PI) / 180.;
        }
        if byte(ctx, c + 0x26).wrapping_sub(2) < 2 {
            a[0] = 0.;
            a[1] = 0.;
        }
        let m = rot_zyx(&IDENTITY, a);
        let mut d = math::apply(&m, base);
        d[0] *= f(ctx, 0xbc);
        d[1] *= f(ctx, 0xc0);
        d[2] *= f(ctx, 0xc4);
        slot.dir = normalize0(d);
        mode = byte(ctx, c + 0x26);
    }
    let radius = f(ctx, 0xb8);
    let kind = byte(ctx, c + 0xc8);
    if !(4..=9).contains(&mode) {
        // Sphere: along the direction at a distributed distance.
        if radius != 0. {
            let r = distributed(ctx, kind, radius);
            slot.pos = scale3(slot.dir, r);
        }
    } else if mode < 6 {
        // Box: a distributed offset per axis, centred, ⊙ multipliers.
        if radius != 0. {
            let centre = radius * 0.5;
            if kind == 1 {
                ctx.rng.unit();
            }
            for i in 0..3 {
                let v = match kind {
                    2..=5 => distributed(ctx, kind, radius),
                    _ => ctx.rng.unit() * radius,
                };
                slot.pos[i] = v - centre;
            }
            slot.pos[0] *= f(ctx, 0xbc);
            slot.pos[1] *= f(ctx, 0xc0);
            slot.pos[2] *= f(ctx, 0xc4);
        }
    } else {
        vsf_position(ctx, k, part, slot, kind);
    }
    for i in 0..4 {
        let range = byte(ctx, c + 0x48 + i);
        let r = ctx.rng.unit();
        let v = r * ((range as u32 * 2) as f32) - (range >> 1) as f32;
        slot.colour[i] = cvtt(v) as i8;
    }
    let fade_in = byte(ctx, c + 0x24);
    if fade_in != 0 {
        slot.fade = part.u8(k.colour + 0xb) as f32;
        slot.fade_in = fade_in;
    }
    let fade_out = byte(ctx, c + 0x25);
    if fade_out != 0 {
        slot.fade_out = fade_out;
    }
    slot.size = 1.;
    let grow_in = byte(ctx, c + 0xd3);
    if grow_in != 0 {
        slot.size = 0.;
        slot.grow_in = grow_in;
    }
    let shrink_out = byte(ctx, c + 0xd4);
    if shrink_out != 0 {
        slot.shrink_out = shrink_out;
    }
    // Degrees → 0x10000 = 360°.
    let angle = |deg: f32| cvtt((deg * 32768.) / 180.) as i16;
    slot.roll = angle(f(ctx, 0x7c));
    slot.roll_vel = angle(f(ctx, 0x80));
    let roll_bits = byte(ctx, c + 0xcb);
    if roll_bits != 0 {
        let r = angle(ctx.rng.unit() * f(ctx, 0x88));
        slot.roll_rnd = r;
        if roll_bits & 3 == 3 {
            if ctx.rng.unit() > 0.5 {
                slot.roll_rnd = slot.roll_rnd.wrapping_neg();
            }
        } else if roll_bits & 2 != 0 {
            slot.roll_rnd = r.wrapping_neg();
        }
    }
    if roll_bits & 4 != 0 {
        slot.roll = slot.roll.wrapping_add(slot.roll_rnd);
    }
    if roll_bits & 8 != 0 {
        slot.roll_vel = slot.roll_vel.wrapping_add(slot.roll_rnd);
    }
    slot.scale = [f(ctx, 0x4c), f(ctx, 0x50)];
    slot.scale_vel = [f(ctx, 0x58), f(ctx, 0x5c)];
    random_scale(ctx, c, slot);
    match byte(ctx, c + 0xd2) {
        1 => {
            if ctx.rng.unit() > 0.5 {
                slot.scale[0] *= -1.;
            }
        }
        2 => {
            if ctx.rng.unit() > 0.5 {
                slot.scale[1] *= -1.;
            }
        }
        3 => {
            if ctx.rng.unit() > 0.5 {
                slot.scale[0] *= -1.;
            }
            if ctx.rng.unit() > 0.5 {
                slot.scale[1] *= -1.;
            }
        }
        _ => {}
    }
    slot.speed = f(ctx, 0xa4);
    slot.drift = f(ctx, 0xb0);
    let spread = f(ctx, 0xac);
    if spread != 0. {
        let r = ctx.rng.unit();
        slot.speed += (spread + spread) * r - spread;
    }
    // +0xd8 lies past a 212-byte keyframe; read as the exe does.
    let spread = f(ctx, 0xd8);
    if spread != 0. {
        let r = ctx.rng.unit();
        slot.drift += (spread + spread) * r - spread;
    }
    let life = half(ctx, c + 0x22);
    slot.life = if life == 0 { 0xffff } else { life };
    slot.age = 0;
    if let Some(m) = side {
        match byte(ctx, c + 0xcc) {
            1 => *m = read_matrix(part, k.e),
            2 => *m = part.cur,
            3 => *m = math::mul(&read_matrix(part, k.e), &part.cur),
            _ => {}
        }
    }
    if let Some(b) = buffer {
        b[1] = [f(ctx, 0x28), f(ctx, 0x2c), f(ctx, 0x30), f(ctx, 0x34)];
    }
}

/// Spawn modes 6–9: a VSF vertex (sequential, or picked by a distributed
/// draw) ⊙ multipliers; modes 8–9 also travel along it.
fn vsf_position(ctx: &mut Ctx, k: Key, part: &mut Part, slot: &mut Slot, kind: u8) {
    let c = k.cursor;
    let index = i16_at(&ctx.dpd.bytes, c + 0xd0).unwrap_or(-1);
    if index < 0 {
        return;
    }
    let Some(vsf) = ctx.dpd.vsf.get(&(index as i32)).cloned() else {
        return;
    };
    let count = vsf.indices.len() as i16 as i32;
    let pick = |r: f32, part: &mut Part| {
        if count <= part.i16(k.e + E_VSF_NEXT) as u16 as i32 {
            part.set_i16(k.e + E_VSF_NEXT, 0);
        }
        cvtt(count as f32 * r)
    };
    let rng = &mut *ctx.rng;
    let at = match kind {
        1 => {
            rng.unit();
            let r = rng.unit();
            pick(r, part)
        }
        2 => {
            let a = rng.unit();
            let b = rng.unit();
            let r = rng.unit();
            pick(r * b * a, part)
        }
        3 => {
            let a = rng.unit();
            let b = rng.unit();
            let r = rng.unit();
            pick(1. - r * b * a, part)
        }
        4 => {
            let a = rng.unit();
            let b = rng.unit();
            let cc = rng.unit();
            let r = rng.unit();
            pick(r * b * a * cc, part)
        }
        5 => {
            let a = rng.unit();
            let b = rng.unit();
            let cc = rng.unit();
            let d = rng.unit();
            let r = rng.unit();
            pick(1. - r * b * a * cc * d, part)
        }
        _ => {
            let mut next = part.i16(k.e + E_VSF_NEXT) as u16;
            if count <= next as i32 {
                next = 0;
            }
            part.set_i16(k.e + E_VSF_NEXT, next.wrapping_add(1) as i16);
            next as i32
        }
    };
    let vertex = usize::try_from(at)
        .ok()
        .and_then(|at| vsf.indices.get(at))
        .and_then(|&i| ctx.dpd.vertex(vsf.model, i))
        .unwrap_or([0; 3]);
    slot.pos[0] = vertex[0] as f32 * float(ctx, c + 0xbc);
    slot.pos[1] = vertex[1] as f32 * float(ctx, c + 0xc0);
    slot.pos[2] = vertex[2] as f32 * float(ctx, c + 0xc4);
    if byte(ctx, c + 0x26).wrapping_sub(8) < 2 {
        slot.dir = normalize0(slot.pos);
    }
}

/// The random scale of byte +0xca: 0x20 one value for both axes, &3 == 3
/// random signs, &3 == 2 negated, 4 added to the scale, 8 to its velocity.
fn random_scale(ctx: &mut Ctx, c: usize, slot: &mut Slot) {
    let bits = byte(ctx, c + 0xca);
    if bits != 0 {
        let x = float(ctx, c + 0x70) * ctx.rng.unit();
        slot.scale_rnd[0] = x;
        if bits & 0x20 == 0 {
            slot.scale_rnd[1] = ctx.rng.unit() * float(ctx, c + 0x74);
            if bits & 3 == 3 {
                if ctx.rng.unit() > 0.5 {
                    slot.scale_rnd[0] *= -1.;
                }
                if ctx.rng.unit() > 0.5 {
                    slot.scale_rnd[1] *= -1.;
                }
            } else if bits & 2 != 0 {
                slot.scale_rnd[0] *= -1.;
                slot.scale_rnd[1] *= -1.;
            }
        } else {
            slot.scale_rnd[1] = x;
            if bits & 3 == 3 {
                if ctx.rng.unit() > 0.5 {
                    slot.scale_rnd[0] *= -1.;
                    slot.scale_rnd[1] *= -1.;
                }
            } else if bits & 2 != 0 {
                slot.scale_rnd = [-x, -x];
            }
        }
    }
    let bits = byte(ctx, c + 0xca);
    if bits & 4 != 0 {
        slot.scale[0] += slot.scale_rnd[0];
        slot.scale[1] += slot.scale_rnd[1];
    }
    if bits & 8 != 0 {
        slot.scale_vel[0] += slot.scale_rnd[0];
        slot.scale_vel[1] += slot.scale_rnd[1];
    }
}

/// FUN_1404bb8b0: one tick of a live slot.
fn move_slot(ctx: &Ctx, k: Key, part: &Part, slot: &mut Slot, buffer: Option<&mut [Vec4; 2]>) {
    let c = k.cursor;
    let f = |o: usize| float(ctx, c + o);
    let base_alpha = part.u8(k.colour + 0xb) as i32;
    let mut alpha = base_alpha;
    if let Some(b) = buffer {
        b[0] = add4(b[0], b[1]);
        b[1] = add4([f(0x38), f(0x3c), f(0x40), f(0x44)], b[1]);
        alpha = cvtt(b[0][3]).wrapping_add(base_alpha).min(0xff);
    }
    slot.roll = slot.roll.wrapping_add(slot.roll_vel);
    let accel = if byte(ctx, c + 0xcb) & 0x10 == 0 {
        f(0x84)
    } else {
        slot.roll_rnd as f32 + f(0x84)
    };
    slot.roll_vel = cvtt(slot.roll_vel as f32 + accel) as i16;
    let [vx, vy] = slot.scale_vel;
    slot.scale[0] += vx;
    slot.scale[1] += vy;
    let y_accel = if byte(ctx, c + 0xca) & 0x10 == 0 {
        slot.scale_vel[0] = vx + f(0x64);
        f(0x68)
    } else {
        slot.scale_vel[0] = slot.scale_rnd[0] + f(0x64) + vx;
        slot.scale_rnd[1] + f(0x68)
    };
    slot.scale_vel[1] = vy + y_accel;
    let mut speed = slot.speed + f(0xa8);
    slot.speed = speed;
    if byte(ctx, c + 0xce) == 0 {
        // The speed stops at zero instead of reversing.
        let (initial, accel) = (f(0xa4), f(0xa8));
        let crossed = if initial > 0. && accel < 0. {
            speed < 0.
        } else {
            initial < 0. && accel > 0. && speed > 0.
        };
        if crossed {
            speed = 0.;
            slot.speed = 0.;
        }
    }
    slot.drift += f(0xb4);
    slot.pos = add4(slot.pos, scale3(slot.dir, speed));
    slot.pos = add4(slot.pos, scale3(part.vec4(k.e + E_AXIS), slot.drift));
    if half(ctx, c + 0x22) != 0 {
        slot.life = slot.life.wrapping_sub(1);
    }
    slot.age = slot.age.wrapping_add(1);
    if slot.fade_in != 0 && slot.age <= slot.fade_in {
        slot.fade -= alpha as f32 / slot.fade_in as f32;
    }
    if slot.fade_out != 0 && slot.life <= slot.fade_out as u16 {
        slot.fade += alpha as f32 / byte(ctx, c + 0x25) as f32;
    }
    if slot.grow_in != 0 && slot.age <= slot.grow_in {
        slot.size = slot.age as f32 / slot.grow_in as f32;
    }
    if slot.shrink_out != 0 && slot.life <= slot.shrink_out as u16 {
        slot.size = slot.life as f32 / slot.shrink_out as f32;
    }
}

/// Matrices FUN_1404bbdb0 prepares once per draw (its 0x1e0 scratch).
struct Scratch {
    emitter: Mat,
    cur: Mat,
    /// +0x40: view · emitter · current (particle space 0 only).
    local: Mat,
    /// The pre-draw-space matrix kept for the offset draw (+0xe).
    extra: Mat,
}

/// FUN_1404bc710: per-slot matrix: axes = world scale × slot scale (× size
/// when below 1), turned by the roll; translation = slot position through
/// the particle space; then the draw space. `keep` stores the matrix
/// before the draw space for the offset draw.
fn slot_matrix(
    ctx: &Ctx,
    space: u8,
    s: &mut Scratch,
    slot: &Slot,
    side: Option<&Mat>,
    keep: bool,
) -> Mat {
    let mut v = [
        ctx.scales[0] * slot.scale[0],
        0.,
        ctx.scales[1] * slot.scale[1],
        0.,
    ];
    if slot.size < 1. {
        v = scale3(v, slot.size);
    }
    let mut out = ZERO;
    out[3][3] = 1.;
    out[0][0] = v[0];
    out[2][2] = v[0];
    out[1][1] = v[2];
    // The exe tests (float)roll != 0.
    if slot.roll != 0 {
        out = math::mul(&out, &math::rot_z(slot.roll as i32));
    }
    out[3] = add4(out[3], slot.pos);
    let space_matrix = match space {
        0 => {
            let mut l = out;
            l[3] = math::apply(&s.local, out[3]);
            if keep {
                s.extra = l;
            }
            return math::mul(&ctx.ede0, &l);
        }
        1 | 3 => math::mul(side.unwrap_or(&ZERO), &s.cur),
        2 => math::mul(&s.emitter, side.unwrap_or(&ZERO)),
        _ => return out,
    };
    let m = math::mul(&ctx.view, &space_matrix);
    out[3] = math::apply(&m, out[3]);
    if keep {
        s.extra = out;
    }
    math::mul(&ctx.ede0, &out)
}

/// FUN_1404bbdb0: every live slot's previous flip-book frame drawn through
/// its matrix, coloured by the emitter colour + slot offsets (+ colour
/// buffer), alpha less the fade, clamped to 0..255 / 0..127.
pub(crate) fn draw(p: &Program, cursor: usize, part: &mut Part, ctx: &mut Ctx) {
    let colour_var = use_val(ctx, p, 1);
    let e = use_val(ctx, p, 2);
    let Some(pool) = part.mega.get(&p.use_val) else {
        return;
    };
    let space = byte(ctx, cursor + 0xcc);
    if space.wrapping_sub(1) < 3 && pool.side.is_none() {
        return;
    }
    if byte(ctx, cursor + 0xc9) != 0 && pool.colour.is_none() {
        return;
    }
    if word(ctx, cursor + 4) == 0xffff {
        return;
    }
    let emitter = read_matrix(part, e);
    let mut s = Scratch {
        emitter,
        cur: part.cur,
        local: ZERO,
        extra: ZERO,
    };
    if space == 0 {
        let world = math::mul(&emitter, &part.cur);
        s.local = math::mul(&ctx.view, &world);
    }
    let Some(anim) = shape_anim(ctx, cursor) else {
        return;
    };
    let offset_draw = byte(ctx, cursor + 0xe) != 0;
    let mut offset = [
        float(ctx, cursor + 0x10),
        float(ctx, cursor + 0x14),
        float(ctx, cursor + 0x18),
        0.,
    ];
    let base: [i32; 4] = std::array::from_fn(|i| part.u8(colour_var + 8 + i) as i32);
    let n = part.i32(e + E_POOL_SIZE).max(0) as usize;
    for i in 0..n.min(pool.slots.len()) {
        let slot = &pool.slots[i];
        if slot.life == 0 {
            continue;
        }
        let side = pool.side.as_ref().and_then(|m| m.get(i));
        let mut main = slot_matrix(ctx, space, &mut s, slot, side, offset_draw);
        main[2] = [0.; 4];
        if offset_draw {
            offset_matrix(ctx, &main, &mut s.extra, &mut offset);
        }
        let mut rgb = [
            slot.colour[0] as i32 + base[0],
            slot.colour[1] as i32 + base[1],
            slot.colour[2] as i32 + base[2],
        ];
        let mut a = cvtt((slot.colour[3] as i32 as f32 + base[3] as f32) - slot.fade);
        if let Some(b) = pool.colour.as_ref().and_then(|c| c.get(i)) {
            for (c, v) in rgb.iter_mut().zip(b[0]) {
                *c = c.wrapping_add(cvtt(v));
            }
            a = a.wrapping_add(cvtt(b[0][3]));
        }
        let rgba = [
            rgb[0].clamp(0, 0xff) as u8,
            rgb[1].clamp(0, 0xff) as u8,
            rgb[2].clamp(0, 0xff) as u8,
            a.clamp(0, 0x7f) as u8,
        ];
        let m = if offset_draw { s.extra } else { main };
        emit_sprites(ctx, anim, slot.flip_prev as usize, &m, rgba);
    }
}

/// The +0xe offset matrix: world 1 pulls the depth by a perspective-scaled
/// offset; otherwise the offset is added in view space (−100 z reads as
/// −32.5 in world 6) and the result taken to draw space.
fn offset_matrix(ctx: &Ctx, main: &Mat, extra: &mut Mat, offset: &mut Vec4) {
    if WORLD == 1 {
        let pull = (10_000_000. / main[3][3]) * offset[2];
        let near = if DEPTH_NEAR > 0. { DEPTH_NEAR } else { 1. };
        let far = if DEPTH_FAR > 0. {
            DEPTH_FAR
        } else {
            16_770_000.
        };
        *extra = *main;
        extra[3][2] -= (1. / (near - far)) * pull;
        extra[2] = [0.; 4];
    } else {
        if WORLD == 6 && offset[2] == -100. {
            offset[2] = -32.5;
        }
        extra[3] = add4(extra[3], *offset);
        *extra = math::mul(&ctx.ede0, extra);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Quad, Rng};
    use data::object_effect::{Dpd, ShapeRef};
    use std::collections::HashMap;

    const CURSOR: usize = 0x20;
    const ANIM: usize = 0x200;

    fn put(b: &mut [u8], at: usize, v: u32) {
        b[at..at + 4].copy_from_slice(&v.to_le_bytes());
    }

    fn putf(b: &mut [u8], at: usize, v: f32) {
        put(b, at, v.to_bits());
    }

    /// Synthetic DPD bytes: use_val list [56, 44, 64] at 0, one MegaBirth
    /// keyframe at `CURSOR`, and a one-sprite shape 0 at `ANIM`.
    fn dpd(edit: impl FnOnce(&mut [u8])) -> Dpd {
        let mut b = vec![0u8; 0x300];
        put(&mut b, 0, 56);
        put(&mut b, 4, 44);
        put(&mut b, 8, 64);
        let k = CURSOR;
        put(&mut b, k + 0xd0, 0xffff); // no VSF
        putf(&mut b, k + 0x4c, 1.);
        putf(&mut b, k + 0x50, 1.);
        putf(&mut b, k + 0xbc, 1.);
        putf(&mut b, k + 0xc0, 1.);
        putf(&mut b, k + 0xc4, 1.);
        edit(&mut b[k..k + 0xdc]);
        // Shape: one sequence (wait 0x200, loop) → frame of one sprite.
        b[ANIM + 6] = 1;
        b[ANIM + 0x10] = 0x20;
        b[ANIM + 0x13] = 0x02;
        b[ANIM + 0x14] = 0x80;
        let f = ANIM + 0x20;
        b[f + 2] = 1;
        put(&mut b, f + 4, 0x30);
        for (i, v) in [-16i16, -16, 16, 16, 0, 0, 16, 16].into_iter().enumerate() {
            b[f + 0x20 + 2 * i..f + 0x22 + 2 * i].copy_from_slice(&v.to_le_bytes());
        }
        b[f + 0x30..f + 0x38].copy_from_slice(&[0x80, 0x80, 0x80, 0x80, 0x48, 0, 0, 0]);
        let mut shapes = HashMap::new();
        shapes.insert(
            0,
            ShapeRef {
                anim: ANIM,
                packet_size: 0,
            },
        );
        Dpd {
            bytes: b,
            pdata: Vec::new(),
            shapes,
            model_vertices: HashMap::new(),
            models: HashMap::new(),
            vsf: HashMap::new(),
            textures: Vec::new(),
        }
    }

    fn program() -> Program {
        Program {
            kind: crate::programs::MEGA_BIRTH,
            param_size: 0xd4,
            keys: 1,
            params: CURSOR,
            use_val: 0,
        }
    }

    fn part() -> Part {
        let mut part = Part {
            frame: 0,
            cur: IDENTITY,
            draw: IDENTITY,
            just_created: false,
            vars: vec![0; 164],
            cursors: vec![CURSOR],
            mega: HashMap::new(),
            mega_model: HashMap::new(),
            vertex_override: None,
            ribbons: HashMap::new(),
            shipoly: HashMap::new(),
            thunder: HashMap::new(),
            serial: 1,
            parent: None,
            restarted: false,
            restart_pending: false,
        };
        // Colour variable u1 = 44: bytes +8..+0xb = 1.0 each.
        part.vars[52..56].fill(0x80);
        part
    }

    /// Runs `ticks` VM ticks (update, then draw) and returns the live slot
    /// count after each and the last tick's quads.
    fn run(
        dpd: &Dpd,
        ticks: usize,
        part: &mut Part,
        skip_between: bool,
    ) -> (Vec<usize>, Vec<Quad>) {
        let mut rng = Rng::default();
        let mut unsupported = Vec::new();
        let mut live = Vec::new();
        let mut quads = Vec::new();
        let p = program();
        let mut first = true;
        for _ in 0..ticks {
            for skip in [true, false] {
                if skip && !skip_between {
                    continue;
                }
                quads.clear();
                let mut ctx = Ctx {
                    dpd,
                    world: IDENTITY,
                    view: IDENTITY,
                    ed60: IDENTITY,
                    ede0: IDENTITY,
                    scales: [1.; 3],
                    origin: [0., 0., 0., 1.],
                    colour: [1.; 4],
                    parent_cur: None,
                    skip,
                    rng: &mut rng,
                    quads: &mut quads,
                    model_draws: &mut Vec::new(),
                    triangles: &mut Vec::new(),
                    refractions: &mut Vec::new(),
                    screen: crate::DEFAULT_SCREEN_SCALE,
                    heads: Vec::new(),
                    births: Vec::new(),
                    unsupported: &mut unsupported,
                    catch_up: false,
                    flags: &mut [0; 3],
                    trail: None,
                };
                if first {
                    create(&p, part, &mut ctx);
                    first = false;
                }
                update(&p, CURSOR, part, &mut ctx);
                draw(&p, CURSOR, part, &mut ctx);
            }
            live.push(part.mega[&0].slots.iter().filter(|s| s.life != 0).count());
        }
        assert!(unsupported.is_empty());
        (live, quads)
    }

    #[test]
    fn births_follow_the_interval_and_cap_and_slots_expire() {
        // Pool 4, 2 births per tick, every 2 ticks, life 3.
        let dpd = dpd(|k| {
            k[0x1c] = 4;
            k[0x1e] = 2;
            k[0x20] = 2;
            k[0x22] = 3;
        });
        let mut part = part();
        let (live, _) = run(&dpd, 7, &mut part, true);
        // Skip halves leave the timer alone; births reset it; slots born at
        // tick 2 free at tick 5 and are reused at tick 6.
        assert_eq!(live, [0, 2, 2, 4, 2, 4, 2]);
        assert_eq!(part.i16(64 + E_TIMER), 1);
        assert_eq!(part.i32(64 + E_POOL_SIZE), 4);
        let mut part2 = self::part();
        let (live, _) = run(&dpd, 3, &mut part2, false);
        assert_eq!(live, [0, 2, 2]);
    }

    #[test]
    fn motion_fade_and_draw() {
        // One slot: sphere radius 10 along (0,1,0), speed 2, drift accel
        // 0.5 along axis (2,0,0), life 4, fade-in 2.
        let dpd = dpd(|k| {
            k[0x1c] = 1;
            k[0x1e] = 1;
            k[0x22] = 4;
            k[0x24] = 2;
            k[0x08] = 0x00;
            k[0x09] = 0x02;
            k[0x90..0x94].copy_from_slice(&1f32.to_le_bytes());
            k[0x98..0x9c].copy_from_slice(&2f32.to_le_bytes());
            k[0xa4..0xa8].copy_from_slice(&2f32.to_le_bytes());
            k[0xb4..0xb8].copy_from_slice(&0.5f32.to_le_bytes());
            k[0xb8..0xbc].copy_from_slice(&10f32.to_le_bytes());
        });
        let mut part = part();
        let (live, quads) = run(&dpd, 2, &mut part, true);
        assert_eq!(live, [1, 1]);
        let slot = part.mega[&0].slots[0];
        assert_eq!(slot.pos, [0.5, 12., 0., 0.]);
        assert_eq!((slot.life, slot.age, slot.fade), (3, 1, 64.));
        assert_eq!(part.vec4(64 + E_AXIS), [1., 0., 0., 0.]);
        // Drawn at the slot position, alpha 0x80 − 64 = 0.5.
        assert_eq!(quads.len(), 1);
        let q = quads[0];
        assert_eq!(q.corners[0], [-0.5, 11., 0.]);
        assert_eq!(q.corners[2], [1.5, 13., 0.]);
        assert_eq!(q.colour, [1., 1., 1., 0.5]);
        assert_eq!(q.blend, 0x48);
    }
}
