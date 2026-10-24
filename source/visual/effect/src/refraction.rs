//! pppKeShpDtt (160) and pppKeMdlDtt (145): redraws of a disc or a model
//! textured from the frame buffer, each vertex sampling the picture behind
//! it moved by a strength (Quick Run's heat haze, several form and boss
//! elements).
//!
//! Both integrate one strength in their draw (FUN_1404cfa90 /
//! FUN_1404ca9e0: value, rate, acceleration at u0, integrated then the key
//! added, as pppSnoMdlDttNorm). 160 draws an eight-sided disc facing the
//! screen at its centre's depth, fading to its rim, sampling the current
//! frame (a copy made as it draws); 145 draws a model's untextured
//! triangles, sampling the previous frame. Each vertex here carries the
//! draw-space point it samples: the host projects it as it projects the
//! vertex.
use crate::programs::{byte, float, key, read_matrix, use_val, word};
use crate::{Ctx, Part, Program, math};

pub(crate) const KE_MDL_DTT: u32 = 145;
pub(crate) const KE_SHP_DTT: u32 = 160;

/// Which picture a refraction samples.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FrameSource {
    /// A copy of the colour target made as the draw is reached (160).
    Current,
    /// The previous output frame (145).
    Previous,
}

/// One refraction vertex: its draw-space position, the draw-space point
/// whose screen place it samples, and its colour (RGB 0..255, alpha 0x80 =
/// 1.0).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RefractionVertex {
    pub position: [f32; 3],
    pub sample: [f32; 3],
    pub rgba: [u8; 4],
}

/// Where a refraction draws in the effect ordering table.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum RefractionOrder {
    /// The layer list's bucket 4 (160 in screen mode).
    Layer(u8),
    /// After every depth-sorted effect (160: its bucket is computed from the
    /// depth's raw float bits, the last one for any positive depth).
    Last,
    /// Sorted among the depth-sorted effects by the triangle's corner that
    /// is farthest away (145: the smallest GS depth of the three, × 16 /
    /// 12288); the host ranks it as it ranks the other effect geometry.
    Sorted,
}

/// A frame-buffer redraw: a fan (160) or a triangle list (145).
#[derive(Debug, Clone, PartialEq)]
pub struct Refraction {
    pub source: FrameSource,
    pub fan: bool,
    pub vertices: Vec<RefractionVertex>,
    pub depth_test: bool,
    pub order: RefractionOrder,
}

pub(crate) fn handles(kind: u32) -> bool {
    matches!(kind, KE_MDL_DTT | KE_SHP_DTT)
}

/// FUN_1404ca9a0 / FUN_1404cfa70 (create and restart): the strength block
/// zeroed.
pub(crate) fn create(p: &Program, part: &mut Part, ctx: &Ctx) {
    let o = use_val(ctx, p, 0);
    for k in 0..3 {
        part.set_f32(o + 4 * k, 0.);
    }
}

/// The strength (value, rate, acceleration at u0): integrated, then the
/// key's adds at `adds` (the acceleration's only when `accel`), unless the
/// effects or the instance are paused or this is an odd half tick.
fn integrate(
    p: &Program,
    cursor: usize,
    part: &mut Part,
    ctx: &Ctx,
    adds: usize,
    accel: bool,
) -> f32 {
    let o = use_val(ctx, p, 0);
    if ctx.flags[1] == 0 && crate::programs::active(part) {
        let a2 = part.f32(o + 8);
        let a1 = part.f32(o + 4) + a2;
        let mut a = part.f32(o) + a1;
        let (mut r, mut c) = (a1, a2);
        if key(ctx, part, cursor) {
            a += float(ctx, cursor + adds);
            r += float(ctx, cursor + adds + 4);
            if accel {
                c += float(ctx, cursor + adds + 8);
            }
        }
        part.set_f32(o, a);
        part.set_f32(o + 4, r);
        part.set_f32(o + 8, c);
    }
    part.f32(o)
}

fn colour(part: &Part, o: usize) -> [u8; 4] {
    std::array::from_fn(|i| part.u8(o + 8 + i))
}

/// FUN_1404cfa90 (pppKeShpDtt): the disc.
fn shape(p: &Program, cursor: usize, part: &mut Part, ctx: &mut Ctx) {
    let a = integrate(p, cursor, part, ctx, 4, true);
    let rgba = colour(part, use_val(ctx, p, 1));
    let m = read_matrix(part, use_val(ctx, p, 2));
    let flags = byte(ctx, cursor + 0x19);
    let screen = flags & 2 != 0;
    // [I] Screen mode's centre (u3, at depth 512) is taken as a draw-space
    // point at that depth.
    let centre = if screen {
        let u3 = use_val(ctx, p, 3);
        [part.f32(u3), part.f32(u3 + 4), 512.]
    } else {
        [m[3][0], m[3][1], m[3][2]]
    };
    // The shift −a·M[1]: each vertex samples its own place moved by it.
    let s = math::scale(m[1], -a);
    let (rw, rh) = (
        ctx.scales[0] * float(ctx, cursor + 0x10),
        ctx.scales[1] * float(ctx, cursor + 0x14),
    );
    let rim_alpha = if flags & 1 != 0 { rgba[3] } else { 0 };
    let mut vertices = vec![RefractionVertex {
        position: centre,
        sample: [centre[0] - s[0], centre[1] - s[1], centre[2]],
        rgba,
    }];
    // N, NE, E, SE, S, SW, W, NW, then N again to close the fan.
    let half = std::f32::consts::FRAC_1_SQRT_2;
    let dirs = [
        (0., 1.),
        (half, half),
        (1., 0.),
        (half, -half),
        (0., -1.),
        (-half, -half),
        (-1., 0.),
        (-half, half),
        (0., 1.),
    ];
    for (sin, cos) in dirs {
        let (dx, dy) = (sin * rw, cos * rh);
        // The rim also samples along its own direction by s.z.
        vertices.push(RefractionVertex {
            position: [centre[0] + dx, centre[1] + dy, centre[2]],
            sample: [
                centre[0] + dx - s[0] + sin * s[2],
                centre[1] + dy - s[1] + cos * s[2],
                centre[2],
            ],
            rgba: [rgba[0], rgba[1], rgba[2], rim_alpha],
        });
    }
    ctx.refractions.push(Refraction {
        source: FrameSource::Current,
        fan: true,
        vertices,
        depth_test: flags & 4 == 0,
        order: if screen {
            RefractionOrder::Layer(4)
        } else {
            RefractionOrder::Last
        },
    });
}

/// FUN_1404ca9e0 (pppKeMdlDtt): the model's untextured triangles.
fn model(p: &Program, cursor: usize, part: &mut Part, ctx: &mut Ctx) {
    let mode = byte(ctx, cursor + 0x14);
    debug_assert!(mode < 4, "pppKeMdlDtt's UV cache (mode ≥ 4) is not built");
    let a = integrate(p, cursor, part, ctx, 8, mode < 4);
    let rgba = colour(part, use_val(ctx, p, 1));
    let id = word(ctx, cursor + 4);
    if id == 0xffff || rgba[0] == 0 {
        return;
    }
    let Some(model) = ctx.dpd.models.get(&(id as i16)) else {
        return;
    };
    let m = read_matrix(part, use_val(ctx, p, 2));
    let sigma = ctx.scales[0] * a;
    let weight_z = byte(ctx, cursor + 0x15) == 1;
    let bytes = &ctx.dpd.bytes;
    let vertex = |i: usize| -> [f32; 3] {
        if let Some(v) = part.vertex_override.as_ref().and_then(|v| v.get(i)) {
            return v.map(f32::from);
        }
        let at = model.vertices + i * 6;
        let r = |k: usize| {
            bytes
                .get(at + 2 * k..at + 2 * k + 2)
                .map_or(0., |b| i16::from_le_bytes([b[0], b[1]]) as f32)
        };
        [r(0), r(1), r(2)]
    };
    let Ok(meshes) = data::object_effect::omd_meshes(bytes, model.hdr) else {
        return;
    };
    let mut out = Vec::new();
    // Type-0 blocks only; the first other block ends the model.
    for mesh in meshes.iter().take_while(|m| m.kind == 0) {
        for t in &mesh.triangles {
            let mut tri = [RefractionVertex {
                position: [0.; 3],
                sample: [0.; 3],
                rgba: [0; 4],
            }; 3];
            for (k, v) in tri.iter_mut().enumerate() {
                let local = vertex(t.index[k] as usize);
                let w = math::apply(&m, [local[0], local[1], local[2], 1.]);
                let mut d = [w[0] - m[3][0], w[1] - m[3][1], w[2] - m[3][2]];
                let len = (d[0] * d[0] + d[1] * d[1] + d[2] * d[2]).sqrt();
                if len > 0. {
                    d = d.map(|c| c / len);
                }
                if weight_z {
                    d[0] *= d[2];
                    d[1] *= d[2];
                }
                // Colour: the mesh corner × the packet colour / 255, cut.
                let c = t.colour[k];
                v.position = [w[0], w[1], w[2]];
                v.sample = [w[0] + sigma * d[0], w[1] + sigma * d[1], w[2]];
                v.rgba = std::array::from_fn(|i| (c[i] as u32 * rgba[i] as u32 / 255) as u8);
            }
            out.push(tri);
        }
    }
    // [I] The effect list's default state: depth test, no depth write.
    for tri in out {
        ctx.refractions.push(Refraction {
            source: FrameSource::Previous,
            fan: false,
            vertices: tri.to_vec(),
            depth_test: true,
            order: RefractionOrder::Sorted,
        });
    }
}

pub(crate) fn draw(p: &Program, cursor: usize, part: &mut Part, ctx: &mut Ctx) {
    match p.kind {
        KE_SHP_DTT => shape(p, cursor, part, ctx),
        KE_MDL_DTT => model(p, cursor, part, ctx),
        _ => {}
    }
}
