//! pppEiDrawShipoly (241): a ribbon that follows the part. Translated from
//! update FUN_1404b9c00, create FUN_1404ba790, restart FUN_1404ba8c0,
//! destroy FUN_1404baa40, draw FUN_1404b9260 with the spline FUN_1404b8f90 /
//! FUN_1404baa90 and the strip builder FUN_1404b8380.
//!
//! Each update tick (single mode, key byte +9 != 0) stores the part's
//! matrix translation in a ring of `+0xb` samples. The draw fits a natural
//! cubic spline through the newest `count` samples, evaluates it at `+0x12`
//! steps in view space, offsets every point by Ã‚Â± the width perpendicular
//! to the path in the view plane and emits `+0x13 + 1` strips of quads
//! across the width, textured with the first mesh of model `+4`.
//!
//! In single mode three optional passes then move the older samples:
//! +0x1c pushes them away from their centroid by each sample's weight (the
//! keyed value +0x2c, stored in w), +0x1d drifts them by a keyed velocity,
//! +0x1e pulls them toward a keyed point (in the part's space unless the
//! byte is 2) with a strength that fades out at distance +0x8c. The strip
//! builder fades alpha across the width when +0x16 is set and swaps U and V
//! for +0xa0 >= 2. The exe's vertex alphas are averaged per quad here.
//!
//! Pair mode (+9 == 0) stores two samples each tick, the part's matrix on
//! (0, ±width / 2, 0), and fits one spline through each rail in view space;
//! the rails are the strip's two edges. +9 >= 2 averages the path's direction
//! over neighbouring steps. +0x10 draws the edges in to the middle at the
//! newest row. Object spaces: 0 the instance, 1 the world, 3 the view's
//! inverse (the draw multiplies the view in twice); 2 the object list that
//! only B_EX170_LAST's script fills, flagged unsupported where met.
//!
//! +8 != 0 draws the strip through FUN_1404b7530 instead: a refraction, each
//! vertex sampling the frame at the point across the strip scaled about its
//! middle by a strength (u3 block +0x18). Its blends other than the plain one
//! (+0x15) are flagged unsupported where met. The restart hook's +0xb8 term
//! feeds nothing the update or draw reads and is not kept.
use crate::math::{self, Mat, Vec4};
use crate::programs::{active, byte, float, use_val, word};
use crate::{Ctx, Part, Quad};
use data::object_effect::{self, Program};

pub(crate) const KIND: u32 = 241;

#[derive(Default)]
pub(crate) struct Ribbon {
    /// Ring of sample points (work +0xa8 buffer) and its size (+0xa4).
    ring: Vec<Vec4>,
    /// Pair mode's ring: the two rails of each tick.
    rails: Vec<[Vec4; 2]>,
    cursor: usize,
    count: usize,
    /// Texture scroll (work +0x24 / +0x28 of the u3 block).
    scroll: [f32; 2],
    /// The part matrix copied each update (+0x30 of the u3 block).
    saved: Option<Mat>,
    /// The first keyframe the update saw (+0xdc handle of the u3 block).
    first_key: Option<usize>,
    /// +0x1c weight: value, rate, acceleration (+0xc/+0x10/+0x14).
    weight: [f32; 3],
    /// +0x1d drift: velocity, acceleration, jerk (+0x70/+0x80/+0x90).
    drift: [Vec4; 3],
    /// +0x1e pull: point, rate, acceleration (+0xa0/+0xb0/+0xc0) and the
    /// strength value, rate, acceleration (+0xd0/+0xd4/+0xd8).
    pull: [Vec4; 3],
    strength: [f32; 3],
    /// The refraction strip's strength: value, rate, acceleration (+0x18..).
    refract: [f32; 3],
}

/// FUN_1404ba8c0: the first key's adds come back out of the integrated
/// values, the rates stop.
pub(crate) fn restart(p: &Program, part: &mut Part, ctx: &mut Ctx) {
    let u3 = use_val(ctx, p, 3);
    let Some(r) = part.ribbons.get_mut(&u3) else {
        return;
    };
    let Some(k) = r.first_key else {
        return;
    };
    for i in 0..3 {
        r.pull[0][i] -= float(ctx, k + 0x5c + 4 * i);
        r.drift[0][i] -= float(ctx, k + 0x38 + 4 * i);
    }
    r.pull[1] = [0.; 4];
    r.pull[2] = [0.; 4];
    r.strength = [r.strength[0] - float(ctx, k + 0x80), 0., 0.];
    r.drift[1] = [0.; 4];
    r.drift[2] = [0.; 4];
    r.weight = [r.weight[0] - float(ctx, k + 0x2c), 0., 0.];
}

fn add4(a: Vec4, b: Vec4) -> Vec4 {
    std::array::from_fn(|i| a[i] + b[i])
}

/// FUN_140140490 + FUN_140142080: (xyz, 0) normalised, and its length.
fn unit_and_length(v: Vec4) -> (Vec4, f32) {
    let l = (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt();
    if l == 0. {
        return ([v[0], v[1], v[2], 0.], l);
    }
    ([v[0] / l, v[1] / l, v[2] / l, 0.], l)
}

/// Ring index of the sample `back` updates older than slot `cursor`.
fn older(cursor: usize, back: usize, size: usize) -> usize {
    (cursor + size - back % size) % size
}

/// The single-mode passes of FUN_1404b9c00 over the samples behind the
/// newest (ring slot `r.cursor`), then their integrators and keys.
fn passes(r: &mut Ribbon, cursor: usize, part: &Part, ctx: &Ctx) {
    let size = r.ring.len();
    let hit = crate::programs::key(ctx, part, cursor);
    if byte(ctx, cursor + 0x1c) != 0 {
        let mut centre = [0.; 4];
        for i in 0..r.count {
            centre = add4(centre, r.ring[older(r.cursor, i, size)]);
        }
        let inv = 1. / r.count as f32;
        centre = [centre[0] * inv, centre[1] * inv, centre[2] * inv, centre[3]];
        for i in 1..r.count.saturating_sub(2) {
            let at = older(r.cursor, i, size);
            let sample = r.ring[at];
            let w = sample[3];
            let d = math::normalize([
                sample[0] - centre[0],
                sample[1] - centre[1],
                sample[2] - centre[2],
                0.,
            ]);
            let mut moved = add4(sample, [d[0] * w, d[1] * w, d[2] * w, 0.]);
            moved[3] = w;
            r.ring[at] = moved;
        }
        let [value, rate, acc] = r.weight;
        let rate = acc + rate;
        let value = rate + value;
        r.weight = [value, rate, acc];
        if hit {
            for i in 0..3 {
                r.weight[i] += float(ctx, cursor + 0x2c + 4 * i);
            }
        }
    }
    if byte(ctx, cursor + 0x1d) != 0 {
        for i in 1..r.count {
            let at = older(r.cursor, i, size);
            r.ring[at] = add4(r.ring[at], r.drift[0]);
        }
        r.drift[1] = add4(r.drift[1], r.drift[2]);
        r.drift[0] = add4(r.drift[0], r.drift[1]);
        if hit {
            for (j, at) in [0x38, 0x44, 0x50].into_iter().enumerate() {
                for i in 0..3 {
                    r.drift[j][i] += float(ctx, cursor + at + 4 * i);
                }
            }
        }
    }
    let mode = byte(ctx, cursor + 0x1e);
    if mode != 0 {
        let range = float(ctx, cursor + 0x8c);
        for i in 1..r.count {
            let at = older(r.cursor, i, size);
            let target = if mode == 2 {
                r.pull[0]
            } else {
                math::apply(&r.saved.unwrap_or(math::IDENTITY), r.pull[0])
            };
            let sample = r.ring[at];
            let (d, distance) = unit_and_length(std::array::from_fn(|k| target[k] - sample[k]));
            let f = distance / range;
            let f = if 0. <= f { f.min(1.) } else { 0. };
            let s = (1. - f) * r.strength[0];
            r.ring[at] = add4(sample, [d[0] * s, d[1] * s, d[2] * s, 0.]);
        }
        r.pull[1] = add4(r.pull[1], r.pull[2]);
        r.pull[0] = add4(r.pull[0], r.pull[1]);
        r.strength[1] += r.strength[2];
        r.strength[0] += r.strength[1];
        if hit {
            for (j, at) in [0x5c, 0x68, 0x74].into_iter().enumerate() {
                for i in 0..3 {
                    r.pull[j][i] += float(ctx, cursor + at + 4 * i);
                }
            }
            for i in 0..3 {
                r.strength[i] += float(ctx, cursor + 0x80 + 4 * i);
            }
        }
    }
}

fn flag(ctx: &mut Ctx, code: u32) {
    if !ctx.unsupported.contains(&code) {
        ctx.unsupported.push(code);
    }
}

pub(crate) fn create(p: &Program, part: &mut Part, ctx: &mut Ctx) {
    let u3 = use_val(ctx, p, 3);
    part.ribbons.insert(u3, Ribbon::default());
}

pub(crate) fn destroy(p: &Program, part: &mut Part, ctx: &mut Ctx) {
    let u3 = use_val(ctx, p, 3);
    part.ribbons.remove(&u3);
}

/// A sample in its object space (+0x11).
fn placed(point: Vec4, space: u8, ctx: &Ctx) -> Vec4 {
    match space {
        1 => math::apply(&ctx.world, point),
        3 => math::apply(&math::inverse(&ctx.view), math::apply(&ctx.world, point)),
        _ => point,
    }
}

pub(crate) fn update(p: &Program, cursor: usize, part: &mut Part, ctx: &mut Ctx) {
    if !active(part) {
        return;
    }
    let single = byte(ctx, cursor + 9) != 0;
    let frozen = byte(ctx, cursor + 0xa) != 0;
    let space = byte(ctx, cursor + 0x11);
    if byte(ctx, cursor + 8) != 0 && byte(ctx, cursor + 0x15) != 0 {
        flag(ctx, 0x4000_0241);
    }
    if space == 2 {
        flag(ctx, 0x4000_0243);
    }
    let u3 = use_val(ctx, p, 3);
    let mut r = part.ribbons.remove(&u3).unwrap_or_default();
    if !frozen {
        r.saved = Some(part.cur);
    }
    if r.first_key.is_none() {
        r.first_key = Some(cursor);
    }
    let size = (byte(ctx, cursor + 0xb) as usize).clamp(4, 0x80);
    if r.ring.is_empty() {
        r.ring = vec![[0.; 4]; size];
    }
    if !frozen && !single {
        if r.rails.is_empty() {
            r.rails = vec![[[0.; 4]; 2]; size];
        }
        let half = float(ctx, cursor + 0xc) * 0.5;
        let rail = |side: f32| {
            placed(
                math::apply(&part.cur, [0., half * side, 0., 1.]),
                space,
                ctx,
            )
        };
        let rails = [rail(1.), rail(-1.)];
        let size = r.rails.len();
        r.rails[r.cursor % size] = rails;
        r.count = (r.count + 1).min(size);
    }
    if !frozen && single {
        let mut sample = placed(part.cur[3], space, ctx);
        if byte(ctx, cursor + 0x1c) != 0 {
            sample[3] = r.weight[0];
        }
        let size = r.ring.len();
        r.ring[r.cursor % size] = sample;
        r.count = (r.count + 1).min(size);
    }
    if single {
        passes(&mut r, cursor, part, ctx);
    }
    if !frozen {
        r.cursor += 1;
        if r.cursor >= r.ring.len() {
            r.cursor = 0;
        }
    }
    part.ribbons.insert(u3, r);
}

/// Second derivatives / 6 of the natural cubic spline through `y` at
/// uniform parameter steps (the system FUN_1404b8f90 solves: C[i-1] + 4C[i]
/// + C[i+1] = y[i+1] - 2y[i] + y[i-1], C = 0 at both ends).
fn natural_spline(y: &[f32]) -> Vec<f32> {
    let n = y.len();
    let mut c = vec![0.; n];
    if n < 3 {
        return c;
    }
    let m = n - 2;
    let mut upper = vec![0.; m];
    let mut rhs: Vec<f32> = (1..n - 1)
        .map(|i| y[i + 1] - 2. * y[i] + y[i - 1])
        .collect();
    let mut pivot = 4.;
    rhs[0] /= pivot;
    upper[0] = 1. / pivot;
    for i in 1..m {
        pivot = 4. - upper[i - 1];
        upper[i] = 1. / pivot;
        rhs[i] = (rhs[i] - rhs[i - 1]) / pivot;
    }
    for i in (0..m - 1).rev() {
        rhs[i] -= upper[i] * rhs[i + 1];
    }
    c[1..n - 1].copy_from_slice(&rhs);
    c
}

/// FUN_1404baa90: P(s) = P0 + sÃ‚Â·(sÃ‚Â·(sÃ‚Â·(C1Ã¢Ë†â€™C0) + 3Ã‚Â·C0) + (P1Ã¢Ë†â€™P0) Ã¢Ë†â€™ 2Ã‚Â·C0 Ã¢Ë†â€™ C1).
fn spline_at(points: &[[f32; 3]], coeff: &[[f32; 3]], t: f32) -> [f32; 3] {
    let last = points.len() - 1;
    let i = (t.max(0.) as usize).min(last.saturating_sub(1));
    let s = t - i as f32;
    std::array::from_fn(|k| {
        let (p0, p1) = (points[i][k], points[i + 1][k]);
        let (c0, c1) = (coeff[i][k], coeff[i + 1][k]);
        p0 + s * (s * (s * (c1 - c0) + 3. * c0) + (p1 - p0) - 2. * c0 - c1)
    })
}

fn wrap01(mut v: f32) -> f32 {
    while v > 1. {
        v -= 1.;
    }
    while v < 0. {
        v += 1.;
    }
    v
}

/// FUN_1404b7530: the strip as a refraction. Across a row a corner at `t`
/// (0..1) stands at `t` between the edges and samples the frame at
/// `(t - 0.5) · k + 0.5`, with `k = strength + 1`, or `strength · fade + 1`
/// under the row fade (+0x17). The strength integrates here (u3 +0x18..+0x20,
/// the key's adds at +0x20..+0x28); alpha is the colour's, across the width
/// under +0x16; +0x14 turns the depth test off.
fn refract(
    cursor: usize,
    part: &mut Part,
    ctx: &mut Ctx,
    u2: usize,
    u3: usize,
    edges: &[(Vec4, Vec4)],
) {
    let hit = crate::programs::key(ctx, part, cursor);
    let moving = active(part);
    let Some(r) = part.ribbons.get_mut(&u3) else {
        return;
    };
    if moving {
        r.refract[1] += r.refract[2];
        r.refract[0] += r.refract[1];
        if hit {
            for i in 0..3 {
                r.refract[i] += float(ctx, cursor + 0x20 + 4 * i);
            }
        }
    }
    let strength = r.refract[0];
    let rows = edges.len().saturating_sub(1);
    let cross = byte(ctx, cursor + 0x13) as usize + 1;
    if rows == 0 || cross < 2 {
        return;
    }
    let fade_on = byte(ctx, cursor + 0x17) != 0;
    let edge_fade = byte(ctx, cursor + 0x16) != 0;
    let n = rows as f32;
    let up = if fade_on {
        (n * float(ctx, cursor + 0x18)) as i32 as f32
    } else {
        0.
    };
    let down = n - up;
    let (rise, fall) = (
        if up > 0. { 1. / up } else { 0. },
        if down > 0. { 1. / down } else { 0. },
    );
    let mut fade = if fade_on && up > 0. { 0f32 } else { 1. };
    let mut falling = false;
    let rgba: [u8; 4] = std::array::from_fn(|i| part.u8(u2 + 8 + i));
    let depth_test = byte(ctx, cursor + 0x14) == 0;
    let lerp = |a: Vec4, b: Vec4, t: f32| -> [f32; 3] {
        std::array::from_fn(|k| b[k] + (a[k] - b[k]) * t)
    };
    let span = |t: f32, k: f32| (t - 0.5) * k + 0.5;
    let width_alpha = |j: usize| -> u8 {
        if !edge_fade {
            return rgba[3];
        }
        // Rises to 1 at the middle and falls back.
        let t = j as f32 / (cross - 1) as f32;
        (f32::from(rgba[3]) * (1. - (2. * t - 1.).abs())) as u8
    };
    for row in 0..rows {
        let k0 = if fade_on {
            strength * fade + 1.
        } else {
            strength + 1.
        };
        if fade_on {
            if !falling && fade < 1. {
                fade += rise;
            } else {
                falling = true;
                fade -= fall;
            }
            fade = fade.clamp(0., 1.);
        }
        let k1 = if fade_on {
            strength * fade + 1.
        } else {
            strength + 1.
        };
        let (a0, b0) = edges[row];
        let (a1, b1) = edges[row + 1];
        let vertex = |row_a: Vec4, row_b: Vec4, k: f32, j: usize| {
            let t = j as f32 / (cross - 1) as f32;
            crate::RefractionVertex {
                position: lerp(row_a, row_b, t),
                sample: lerp(row_a, row_b, span(t, k)),
                rgba: [rgba[0], rgba[1], rgba[2], width_alpha(j)],
            }
        };
        let mut vertices = Vec::with_capacity((cross - 1) * 6);
        for j in 0..cross - 1 {
            let (p, q) = (vertex(a0, b0, k0, j), vertex(a0, b0, k0, j + 1));
            let (s, u) = (vertex(a1, b1, k1, j), vertex(a1, b1, k1, j + 1));
            vertices.extend([p, q, s, q, u, s]);
        }
        ctx.refractions.push(crate::Refraction {
            source: crate::FrameSource::Current,
            fan: false,
            vertices,
            depth_test,
            order: crate::RefractionOrder::Sorted,
        });
    }
}

pub(crate) fn draw(p: &Program, cursor: usize, part: &mut Part, ctx: &mut Ctx) {
    let (u1, u2, u3) = (use_val(ctx, p, 1), use_val(ctx, p, 2), use_val(ctx, p, 3));
    let Some(r) = part.ribbons.get(&u3) else {
        return;
    };
    let (count, size) = (r.count, r.ring.len());
    let steps = byte(ctx, cursor + 0x12) as usize;
    let mode = byte(ctx, cursor + 9);
    if count < 3 || steps == 0 {
        return;
    }
    let space = byte(ctx, cursor + 0x11);
    let view_matrix = match space {
        0 => ctx.ed60,
        3 => math::mul(&ctx.view, &ctx.view),
        _ => ctx.view,
    };
    let uv_length = float(ctx, cursor + 0x90);
    // The ticks' ring slots, newest first.
    let slot = |k: usize| {
        let mut i = r.cursor as isize - 1 - k as isize;
        if i < 0 {
            i += size as isize;
        }
        i as usize
    };
    let curve = |points: &[[f32; 3]]| -> Vec<[f32; 3]> {
        let per: Vec<Vec<f32>> = (0..3)
            .map(|k| natural_spline(&points.iter().map(|q| q[k]).collect::<Vec<_>>()))
            .collect();
        let coeff: Vec<[f32; 3]> = (0..count)
            .map(|i| [per[0][i], per[1][i], per[2][i]])
            .collect();
        (0..=steps)
            .map(|i| {
                let t = (i as f32 / steps as f32) * (count - 1) as f32;
                spline_at(points, &coeff, t)
            })
            .collect()
    };
    let (q, mut edges): (Vec<Vec4>, Vec<(Vec4, Vec4)>) = if mode == 0 {
        // The rails, each fitted in view space.
        let rail = |side: usize| {
            let points: Vec<[f32; 3]> = (0..count)
                .map(|k| {
                    let s = r.rails.get(slot(k)).map_or([0., 0., 0., 1.], |p| p[side]);
                    let v = math::apply(&view_matrix, [s[0], s[1], s[2], 1.]);
                    [v[0], v[1], v[2]]
                })
                .collect();
            curve(&points)
        };
        let (one, two) = (rail(0), rail(1));
        let vec = |p: [f32; 3]| [p[0], p[1], p[2], 1.];
        (
            one.iter().map(|p| vec(*p)).collect(),
            one.iter()
                .zip(&two)
                .map(|(a, b)| (vec(*a), vec(*b)))
                .collect(),
        )
    } else {
        // Control points, newest first.
        let points: Vec<[f32; 3]> = (0..count)
            .map(|k| {
                let s = r.ring[slot(k)];
                [s[0], s[1], s[2]]
            })
            .collect();
        // Path samples in view space; w = length of the segment to the next.
        let mut q: Vec<Vec4> = curve(&points)
            .into_iter()
            .map(|s| math::apply(&view_matrix, [s[0], s[1], s[2], 1.]))
            .collect();
        if uv_length > 0. {
            for i in 0..steps {
                let d: [f32; 3] = std::array::from_fn(|k| q[i][k] - q[i + 1][k]);
                q[i][3] = (d[0] * d[0] + d[1] * d[1] + d[2] * d[2]).sqrt();
            }
            q[steps][3] = 0.;
        }
        // Edge points: the path ± the width, perpendicular in the view plane.
        let half = float(ctx, cursor + 0xc) * part.f32(u1) * ctx.scales[0];
        let unit = |x: f32, y: f32| {
            let d = math::normalize([x, y, 0., 0.]);
            [d[0] * half, d[1] * half]
        };
        let mut previous = [0.; 2];
        let edges = (0..=steps)
            .map(|i| {
                let d = if mode == 1 {
                    let (a, b) = if i != steps {
                        (q[i], q[i + 1])
                    } else {
                        (q[i - 1], q[i])
                    };
                    let d = math::normalize([a[0] - b[0], a[1] - b[1], a[2] - b[2], 0.]);
                    [d[0] * half, d[1] * half]
                } else {
                    // The sum of this step's and the last step's directions;
                    // the last row's own has no x.
                    let here = if i != steps {
                        unit(q[i][0] - q[i + 1][0], q[i][1] - q[i + 1][1])
                    } else {
                        unit(0., previous[1])
                    };
                    let sum = [previous[0] + here[0], previous[1] + here[1]];
                    previous = here;
                    sum
                };
                // (x, y) -> (-y, x)
                let perp = [-d[1], d[0]];
                let c = q[i];
                (
                    [c[0] + perp[0], c[1] + perp[1], c[2], c[3]],
                    [c[0] - perp[0], c[1] - perp[1], c[2], c[3]],
                )
            })
            .collect();
        (q, edges)
    };
    // +0x10: the edges drawn in to their middle, all the way at the newest
    // row and not at all at the oldest.
    if byte(ctx, cursor + 0x10) != 0 {
        let mut t = 1f32;
        for edge in &mut edges {
            let middle: Vec4 = std::array::from_fn(|k| (edge.0[k] + edge.1[k]) * 0.5);
            let inward =
                |e: Vec4| -> Vec4 { std::array::from_fn(|k| e[k] + (middle[k] - e[k]) * t) };
            *edge = (inward(edge.0), inward(edge.1));
            t = (t - 1. / steps as f32).max(0.);
        }
    }
    if byte(ctx, cursor + 8) != 0 {
        refract(cursor, part, ctx, u2, u3, &edges);
        return;
    }
    // The ribbon's texture: the first mesh of model +4.
    let model = word(ctx, cursor + 4);
    let Some(tex0) = ctx
        .dpd
        .models
        .get(&(model as i16))
        .and_then(|m| object_effect::omd_meshes(&ctx.dpd.bytes, m.hdr).ok())
        .and_then(|m| m.first().map(|m| m.tex0))
    else {
        return;
    };
    let tex_w = (1u32 << (((tex0 >> 26) & 0xf) as u32).min(10)) as f32;
    let tex_h = (1u32 << (((tex0 >> 30) & 0xf) as u32).min(10)) as f32;
    let swap = byte(ctx, cursor + 0xa0) > 1;
    let edge_fade = byte(ctx, cursor + 0x16) != 0;
    // FUN_1404b8380: UV window and fade set-up.
    let pick = |b: u8| match b {
        1 => 0.5,
        2 => 0.25,
        _ => 1.,
    };
    let mut u_max = pick(byte(ctx, cursor + 0x9c));
    let v_max = pick(byte(ctx, cursor + 0x9d));
    let mut u_min = 0.;
    match byte(ctx, cursor + 0x9e) {
        1 => {
            u_max += 0.25;
            u_min = 0.5;
        }
        2 => {
            u_max += 0.5;
            u_min = 0.5;
        }
        3 => {
            u_max += 0.75;
            u_min = 0.75;
        }
        _ => {}
    }
    u_min += 0.01;
    u_max -= 0.01;
    let v_range = v_max - 0.02;
    let n = steps as f32;
    let (mut u_run, u_step) = {
        let fraction = if byte(ctx, cursor + 0xa1) == 0 {
            1.
        } else {
            count as f32 / size as f32
        };
        match byte(ctx, cursor + 0xa0) {
            1 | 3 => {
                let span = fraction * (u_max - u_min);
                (u_max, -(span / n))
            }
            _ => (u_min, fraction * (u_max - u_min) / n),
        }
    };
    let cross = byte(ctx, cursor + 0x13) as usize + 1;
    let fade_on = byte(ctx, cursor + 0x17) != 0;
    let (mut fade, rise, fall) = if fade_on {
        let up = (n * float(ctx, cursor + 0x18)) as i32 as f32;
        let down = n - up;
        (
            if up > 0. { 0. } else { 1. },
            if up > 0. { 1. / up } else { 0. },
            if down > 0. { 1. / down } else { 0. },
        )
    } else {
        (1., 0., 0.)
    };
    let rgba: [f32; 4] = std::array::from_fn(|i| part.u8(u2 + 8 + i) as f32);
    let blend = match byte(ctx, cursor + 0x15) {
        1 => 0x48,
        _ => 0x44,
    };
    let (su, sv) = {
        let r = &part.ribbons[&u3];
        (r.scroll[0], r.scroll[1])
    };
    let mut done = false;
    for (k, segment) in q.iter().enumerate().take(steps) {
        let (mut a_row, mut a_next) = (rgba[3], rgba[3]);
        if fade_on {
            let previous = rgba[3] * fade;
            let next = if !done && fade < 1. {
                fade + rise
            } else {
                done = true;
                fade - fall
            };
            if next >= 0. {
                fade = next.min(1.);
                a_next = rgba[3] * fade;
            } else {
                fade = 0.;
                a_next = 0.;
            }
            a_row = previous;
        }
        // u at the segment's two rows (running position, then advanced).
        let mut u_next = if uv_length > 0. {
            u_run + (segment[3] / uv_length) * (u_max - u_min)
        } else {
            u_run + u_step
        };
        let raw_next = u_next;
        if uv_length > 0. {
            if u_next > u_max {
                u_next = (u_next - u_max) + u_min;
            }
        } else if u_next > u_max {
            u_next = u_max;
        }
        let (u0, u1v) = (u_run + su, raw_next + su);
        let colour = |a: f32| {
            [
                rgba[0] / 128.,
                rgba[1] / 128.,
                rgba[2] / 128.,
                a.floor() / 128.,
            ]
        };
        // +0x16: alpha across the width rises 0 → 1 and falls back, in steps
        // of 2 / (samples − 1), restarting every row.
        let edge: Vec<f32> = {
            let (mut e, mut step) = (0f32, 2. / (cross as i32 - 1) as f32);
            (0..cross)
                .map(|_| {
                    if !edge_fade {
                        return 1.;
                    }
                    let here = e;
                    let up = e + step;
                    if up < 1. {
                        e = if 0. <= up { up } else { 0. };
                    } else {
                        step = -step;
                        e = 1.;
                    }
                    here
                })
                .collect()
        };
        // Cells between cross-section samples j and j+1 of rows k, k+1.
        for j in 0..cross.saturating_sub(1) {
            let f = |j: usize| j as f32 / (cross - 1) as f32;
            let at = |row: usize, j: usize| -> [f32; 3] {
                let (a, b) = edges[row];
                let t = f(j);
                [
                    b[0] + (a[0] - b[0]) * t,
                    b[1] + (a[1] - b[1]) * t,
                    b[2] + (a[2] - b[2]) * t,
                ]
            };
            let v = |j: usize| v_range * f(j) + 0.01 + sv;
            // +0xa0 >= 2: the texture runs across the strip instead.
            let st = |u: f32, j: usize| {
                if swap {
                    [v(j) * tex_w, u * tex_h]
                } else {
                    [u * tex_w, v(j) * tex_h]
                }
            };
            let uv = [st(u0, j), st(u0, j + 1), st(u1v, j + 1), st(u1v, j)];
            let alpha = (a_row + a_next) * (edge[j] + edge[j + 1]) * 0.25;
            ctx.quads.push(Quad {
                corners: [at(k, j), at(k, j + 1), at(k + 1, j + 1), at(k + 1, j)],
                uv,
                colour: colour(alpha),
                blend,
                corner_colours: None,
                tbp: (tex0 & 0x3fff) as u16,
                tex0,
            });
        }
        u_run = u_next;
    }
    // Texture scroll advances once per VM tick.
    if !ctx.skip {
        let (du, dv) = (float(ctx, cursor + 0x94), float(ctx, cursor + 0x98));
        if let Some(r) = part.ribbons.get_mut(&u3) {
            r.scroll[0] = wrap01(r.scroll[0] + du);
            r.scroll[1] = wrap01(r.scroll[1] + dv);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_inverse_undoes_a_view() {
        let view: Mat = [
            [0., 0., 1., 0.],
            [0., 1., 0., 0.],
            [-1., 0., 0., 0.],
            [10., -20., 30., 1.],
        ];
        let back = math::mul(&math::inverse(&view), &view);
        for (r, row) in back.iter().enumerate() {
            for (c, v) in row.iter().enumerate() {
                let want = if r == c { 1. } else { 0. };
                assert!((v - want).abs() < 1e-5, "{r},{c}: {v}");
            }
        }
    }

    #[test]
    fn natural_spline_interpolates_and_is_linear_on_lines() {
        let line = [0., 2., 4., 6., 8.];
        assert!(natural_spline(&line).iter().all(|c| c.abs() < 1e-5));
        let y = [0., 1., 4., 9., 16., 20.];
        let c = natural_spline(&y);
        assert_eq!((c[0], c[y.len() - 1]), (0., 0.));
        let points: Vec<[f32; 3]> = y.iter().map(|&v| [v, 0., 0.]).collect();
        let coeff: Vec<[f32; 3]> = c.iter().map(|&v| [v, 0., 0.]).collect();
        for (i, &v) in y.iter().enumerate() {
            let p = spline_at(&points, &coeff, i as f32);
            assert!((p[0] - v).abs() < 1e-4, "{i}: {p:?}");
        }
        // Between knots the curve stays near the data.
        let mid = spline_at(&points, &coeff, 2.5);
        assert!(mid[0] > 4. && mid[0] < 9.);
    }
}
