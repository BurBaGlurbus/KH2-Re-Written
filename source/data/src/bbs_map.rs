//! Birth by Sleep field maps (the LAN duel's Mirage Arena): the ARC archive
//! an area ships in (`arc/map/<world><nn>.arc`), its map model (PMP: placed
//! PMO models and their TIM2 textures) and its collision (BCD).
//!
//! Formats from the installed files [installed-build observation, T248],
//! the map's layout checked against the BBS FM executable: FUN_1401017e0
//! fixes a PMP up (magic "PMP\0" or "PST\0"; u16 object count at +0x10;
//! texture table offset at +0x1C, count at +0x1A, entries 0x20 apart with
//! the TIM2 offset first; objects 0x30 apart with the PMO offset at +0x24,
//! from +0x44), FUN_140107a10 each object's PMO, FUN_1401ac3c0 checks the
//! "TIM2" magic.
//!
//! - ARC: "ARC\0", u16 version (1) at +4, u16 count at +6, then 32-byte
//!   entries from +0x10: u32 link (non-zero: a sound bank named, not
//!   stored), u32 offset, u32 size, u32 zero, char name[16].
//! - PMP: "PMP\0"; u16 object count at +0x10, u16 texture count at +0x1A,
//!   u32 texture table at +0x1C; objects from +0x20, 0x30 each: position,
//!   rotation (radians, X then Y then Z), scale (3 floats each), u32 PMO
//!   offset (0: none), u32 zero, u16 flags (bit 0: sky, drawn around the
//!   camera), u16 id. Texture table entries are 0x20: u32 TIM2 offset,
//!   char name[12], then the UV scroll (two floats, texture widths a
//!   frame at 60 a second).
//! - PMO: "PMO\0"; u16 texture slot count at +8, u32 skeleton offset at
//!   +0xC, u32 opaque mesh offset at +0x10, f32 scale at +0x18, u32
//!   translucent mesh offset at +0x1C; texture slots from +0xA0 (0x20 each,
//!   name at +4). A mesh section: u16 vertex count (0 ends the list), u8
//!   texture slot, u8 vertex size, u32 PSP GE vertex type, u8 group, u8
//!   strip count, u16 attributes, a u32 colour when type bit 24 is set,
//!   the strip lengths, then the vertices; sections are 4-byte aligned.
//!   The primitive is type bits 28-30 (3 triangles, 4 strip).
//! - TIM2: 4- or 8-bit indexed pictures with a 32-bit palette, unswizzled,
//!   alpha 0..=255; or 16/24/32-bit direct colour.
//! - BCD: "@bcd"; bounds at +0x10; u16 vertex count at +0x34, u16 face
//!   count at +0x36; vertex offset at +0x38 (x, y, z, 1 floats), face offset
//!   at +0x3C; faces are 48 bytes: plane (normal, d with n·p = d), u16
//!   corners[4] (0xFFFF: a triangle), then flags.
//!
//! Vertex colours: 128 is full brightness (the GE's colour doubling).

use std::io;

type Result<T> = io::Result<T>;

fn invalid(m: impl Into<String>) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, m.into())
}

fn u16_at(b: &[u8], o: usize) -> Result<u16> {
    b.get(o..o + 2)
        .map(|s| u16::from_le_bytes([s[0], s[1]]))
        .ok_or_else(|| invalid("read past the end"))
}

fn u32_at(b: &[u8], o: usize) -> Result<u32> {
    b.get(o..o + 4)
        .map(|s| u32::from_le_bytes([s[0], s[1], s[2], s[3]]))
        .ok_or_else(|| invalid("read past the end"))
}

fn f32_at(b: &[u8], o: usize) -> Result<f32> {
    let v = f32::from_bits(u32_at(b, o)?);
    if v.is_finite() {
        Ok(v)
    } else {
        Err(invalid("not a finite number"))
    }
}

fn vec3_at(b: &[u8], o: usize) -> Result<[f32; 3]> {
    Ok([f32_at(b, o)?, f32_at(b, o + 4)?, f32_at(b, o + 8)?])
}

fn name_at(b: &[u8], o: usize, len: usize) -> Result<String> {
    let raw = b
        .get(o..o + len)
        .ok_or_else(|| invalid("name past the end"))?;
    let end = raw.iter().position(|&c| c == 0).unwrap_or(len);
    Ok(String::from_utf8_lossy(&raw[..end]).into_owned())
}

/// One stored file of an ARC archive.
pub struct ArcFile<'a> {
    pub name: String,
    pub bytes: &'a [u8],
}

/// The stored files of an ARC archive (named sound banks it only links are
/// left out).
pub fn arc_files(b: &[u8]) -> Result<Vec<ArcFile<'_>>> {
    if b.get(..4) != Some(b"ARC\0") {
        return Err(invalid("not an ARC archive"));
    }
    let count = usize::from(u16_at(b, 6)?);
    let mut out = Vec::new();
    for i in 0..count {
        let e = 0x10 + i * 0x20;
        if u32_at(b, e)? != 0 {
            continue;
        }
        let (offset, size) = (u32_at(b, e + 4)? as usize, u32_at(b, e + 8)? as usize);
        let bytes = b
            .get(
                offset
                    ..offset
                        .checked_add(size)
                        .ok_or_else(|| invalid("entry overflow"))?,
            )
            .ok_or_else(|| invalid("entry past the archive"))?;
        out.push(ArcFile {
            name: name_at(b, e + 0x10, 16)?,
            bytes,
        });
    }
    Ok(out)
}

/// A decoded picture, rows top first.
#[derive(Clone, Debug, PartialEq)]
pub struct Picture {
    pub width: u32,
    pub height: u32,
    pub rgba: Vec<u8>,
}

/// The first picture of the TIM2 file at `o`.
pub fn tim2(b: &[u8], o: usize) -> Result<Picture> {
    if b.get(o..o + 4) != Some(b"TIM2") {
        return Err(invalid("not a TIM2 picture"));
    }
    let q = o + 16;
    let clut_size = u32_at(b, q + 4)? as usize;
    let image_size = u32_at(b, q + 8)? as usize;
    let header_size = usize::from(u16_at(b, q + 12)?);
    let colours = usize::from(u16_at(b, q + 14)?);
    let image_type = *b.get(q + 19).ok_or_else(|| invalid("TIM2 header"))?;
    let (w, h) = (u32::from(u16_at(b, q + 20)?), u32::from(u16_at(b, q + 22)?));
    if w == 0 || h == 0 || w > 4096 || h > 4096 {
        return Err(invalid("TIM2 picture size"));
    }
    let start = q + header_size;
    let pixels = b
        .get(start..start + image_size)
        .ok_or_else(|| invalid("TIM2 pixels past the end"))?;
    let clut = b
        .get(start + image_size..start + image_size + clut_size)
        .ok_or_else(|| invalid("TIM2 palette past the end"))?;
    let n = (w * h) as usize;
    let palette = |i: usize| -> Result<[u8; 4]> {
        if i >= colours {
            return Err(invalid("palette index past the palette"));
        }
        clut.get(i * 4..i * 4 + 4)
            .map(|c| [c[0], c[1], c[2], c[3]])
            .ok_or_else(|| invalid("palette past the end"))
    };
    let mut rgba = Vec::with_capacity(n * 4);
    match image_type {
        // 4-bit indexed: the low nibble is the left pixel.
        4 => {
            for k in 0..n {
                let byte = *pixels.get(k / 2).ok_or_else(|| invalid("pixels short"))?;
                let i = if k % 2 == 0 { byte & 15 } else { byte >> 4 };
                rgba.extend(palette(usize::from(i))?);
            }
        }
        5 => {
            for k in 0..n {
                let i = *pixels.get(k).ok_or_else(|| invalid("pixels short"))?;
                rgba.extend(palette(usize::from(i))?);
            }
        }
        3 => rgba.extend_from_slice(pixels.get(..n * 4).ok_or_else(|| invalid("pixels short"))?),
        2 => {
            for k in 0..n {
                let p = pixels
                    .get(k * 3..k * 3 + 3)
                    .ok_or_else(|| invalid("pixels short"))?;
                rgba.extend([p[0], p[1], p[2], 255]);
            }
        }
        1 => {
            for k in 0..n {
                let v = u16_at(pixels, k * 2)?;
                let c = |s: u16| (((v >> s) & 31) as u32 * 255 / 31) as u8;
                rgba.extend([c(0), c(5), c(10), if v & 0x8000 != 0 { 255 } else { 0 }]);
            }
        }
        t => return Err(invalid(format!("TIM2 picture type {t}"))),
    }
    Ok(Picture {
        width: w,
        height: h,
        rgba,
    })
}

/// One mesh of a map, already placed: a triangle list.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct MapMesh {
    /// The map's texture (its table index), if the section names one.
    pub texture: Option<usize>,
    /// From the PMO's translucent list.
    pub translucent: bool,
    /// A sky object's: drawn around the camera, behind the map.
    pub sky: bool,
    pub positions: Vec<[f32; 3]>,
    pub uvs: Vec<[f32; 2]>,
    /// 1.0 is full brightness.
    pub colors: Vec<[f32; 4]>,
}

/// A map model: its textures and placed meshes (map units).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct MapModel {
    pub texture_names: Vec<String>,
    pub textures: Vec<Picture>,
    /// Each texture's UV scroll a frame (U, V).
    pub texture_scroll: Vec<[f32; 2]>,
    pub meshes: Vec<MapMesh>,
}

#[derive(Clone, Copy)]
struct Vertex {
    position: [f32; 3],
    uv: [f32; 2],
    color: [f32; 4],
}

/// GE component sizes: 0 none, 1 byte, 2 half, 3 float.
fn component(fmt: u32) -> usize {
    [0, 1, 2, 4][fmt as usize & 3]
}

fn align(o: usize, n: usize) -> usize {
    if n == 0 { o } else { o.div_ceil(n) * n }
}

/// One GE vertex of `flags` at `o` (weights, texture, colour, normal,
/// position, each aligned to its own size).
fn vertex(b: &[u8], mut o: usize, flags: u32, scale: f32, colour: [f32; 4]) -> Result<Vertex> {
    let (tfmt, cfmt, nfmt, pfmt, wfmt) = (
        flags & 3,
        (flags >> 2) & 7,
        (flags >> 5) & 3,
        (flags >> 7) & 3,
        (flags >> 9) & 3,
    );
    if wfmt != 0 {
        let n = ((flags >> 14) & 7) as usize + 1;
        let s = component(wfmt);
        o = align(o, s) + s * n;
    }
    let mut uv = [0.; 2];
    if tfmt != 0 {
        let s = component(tfmt);
        o = align(o, s);
        uv = match tfmt {
            3 => [f32_at(b, o)?, f32_at(b, o + 4)?],
            2 => [
                f32::from(u16_at(b, o)?) / 32768.,
                f32::from(u16_at(b, o + 2)?) / 32768.,
            ],
            _ => {
                let p = b.get(o..o + 2).ok_or_else(|| invalid("vertex short"))?;
                [f32::from(p[0]) / 128., f32::from(p[1]) / 128.]
            }
        };
        o += 2 * s;
    }
    let mut color = colour;
    match cfmt {
        0 => {}
        7 => {
            o = align(o, 4);
            let p = b.get(o..o + 4).ok_or_else(|| invalid("vertex short"))?;
            color = [
                f32::from(p[0]) / 128.,
                f32::from(p[1]) / 128.,
                f32::from(p[2]) / 128.,
                f32::from(p[3]) / 255.,
            ];
            o += 4;
        }
        4..=6 => {
            o = align(o, 2) + 2;
        }
        _ => return Err(invalid("vertex colour format")),
    }
    if nfmt != 0 {
        let s = component(nfmt);
        o = align(o, s) + 3 * s;
    }
    let s = component(pfmt);
    o = align(o, s);
    let p = match pfmt {
        3 => vec3_at(b, o)?,
        2 => {
            let c = |k| -> Result<f32> { Ok(f32::from(u16_at(b, o + k)? as i16) / 32768.) };
            [c(0)?, c(2)?, c(4)?]
        }
        1 => {
            let q = b.get(o..o + 3).ok_or_else(|| invalid("vertex short"))?;
            [q[0], q[1], q[2]].map(|c| f32::from(c as i8) / 128.)
        }
        _ => return Err(invalid("vertex has no position")),
    };
    Ok(Vertex {
        position: p.map(|c| c * scale),
        uv,
        color,
    })
}

/// A PMO's meshes in model space: (texture slot, translucent, triangle
/// corners).
fn pmo_meshes(b: &[u8], base: usize) -> Result<Vec<(usize, bool, Vec<Vertex>)>> {
    if b.get(base..base + 4) != Some(b"PMO\0") {
        return Err(invalid("not a PMO model"));
    }
    let skeleton = u32_at(b, base + 0xC)?;
    let scale = f32_at(b, base + 0x18)?;
    let mut out = Vec::new();
    for (list, translucent) in [
        (u32_at(b, base + 0x10)?, false),
        (u32_at(b, base + 0x1C)?, true),
    ] {
        if list == 0 {
            continue;
        }
        let mut o = base + list as usize;
        for _ in 0..4096 {
            let count = usize::from(u16_at(b, o)?);
            if count == 0 {
                break;
            }
            let slot = usize::from(*b.get(o + 2).ok_or_else(|| invalid("section"))?);
            let size = usize::from(*b.get(o + 3).ok_or_else(|| invalid("section"))?);
            let flags = u32_at(b, o + 4)?;
            let strips = usize::from(*b.get(o + 9).ok_or_else(|| invalid("section"))?);
            o += 12;
            if skeleton != 0 {
                o += 8;
            }
            let mut colour = [1., 1., 1., 1.];
            if flags & 0x0100_0000 != 0 {
                let c = b.get(o..o + 4).ok_or_else(|| invalid("section colour"))?;
                colour = [
                    f32::from(c[0]) / 128.,
                    f32::from(c[1]) / 128.,
                    f32::from(c[2]) / 128.,
                    f32::from(c[3]) / 255.,
                ];
                o += 4;
            }
            let mut lengths = Vec::with_capacity(strips);
            for k in 0..strips {
                lengths.push(usize::from(u16_at(b, o + k * 2)?));
            }
            o += strips * 2;
            if size == 0 {
                return Err(invalid("zero vertex size"));
            }
            let verts = (0..count)
                .map(|k| vertex(b, o + k * size, flags, scale, colour))
                .collect::<Result<Vec<_>>>()?;
            o = align(o + count * size, 4);
            let corners = match (flags >> 28) & 7 {
                3 => verts.into_iter().take(count / 3 * 3).collect(),
                4 => strip_corners(verts, &lengths),
                // Points and lines draw nothing on the floor.
                _ => Vec::new(),
            };
            out.push((slot, translucent, corners));
        }
    }
    Ok(out)
}

/// Triangle-strip runs as a triangle list (every other triangle turned
/// back to the strip's winding).
fn strip_corners(verts: Vec<Vertex>, lengths: &[usize]) -> Vec<Vertex> {
    let total = verts.len();
    let runs: Vec<usize> = if lengths.is_empty() {
        vec![total]
    } else {
        lengths.to_vec()
    };
    let mut out = Vec::new();
    let mut at = 0;
    for n in runs {
        if at + n > total {
            break;
        }
        for i in 0..n.saturating_sub(2) {
            let (a, b, c) = (at + i, at + i + 1, at + i + 2);
            let k = if i % 2 == 0 { [a, b, c] } else { [b, a, c] };
            out.extend(k.map(|j| verts[j]));
        }
        at += n;
    }
    out
}

/// Rotates by X, then Y, then Z (radians).
fn rotate(p: [f32; 3], r: [f32; 3]) -> [f32; 3] {
    let [mut x, mut y, mut z] = p;
    let (s, c) = r[0].sin_cos();
    (y, z) = (y * c - z * s, y * s + z * c);
    let (s, c) = r[1].sin_cos();
    (x, z) = (x * c + z * s, -x * s + z * c);
    let (s, c) = r[2].sin_cos();
    (x, y) = (x * c - y * s, x * s + y * c);
    [x, y, z]
}

/// A PMP map model: its textures and every placed PMO's meshes in map
/// space, one mesh per object section.
pub fn pmp(b: &[u8]) -> Result<MapModel> {
    if b.get(..4) != Some(b"PMP\0") {
        return Err(invalid("not a PMP map"));
    }
    let objects = usize::from(u16_at(b, 0x10)?);
    let tex_count = usize::from(u16_at(b, 0x1A)?);
    let tex_table = u32_at(b, 0x1C)? as usize;
    let mut model = MapModel::default();
    for i in 0..tex_count {
        let e = tex_table + i * 0x20;
        model.texture_names.push(name_at(b, e + 4, 12)?);
        model.textures.push(tim2(b, u32_at(b, e)? as usize)?);
        model
            .texture_scroll
            .push([f32_at(b, e + 16)?, f32_at(b, e + 20)?]);
    }
    for i in 0..objects {
        let e = 0x20 + i * 0x30;
        let (at, rot, scale) = (vec3_at(b, e)?, vec3_at(b, e + 12)?, vec3_at(b, e + 24)?);
        let pmo = u32_at(b, e + 36)? as usize;
        if pmo == 0 {
            continue;
        }
        let sky = u16_at(b, e + 44)? & 1 != 0;
        // The PMO's slots name the map's textures.
        let slots = usize::from(u16_at(b, pmo + 8)?);
        let slot_names = (0..slots)
            .map(|k| name_at(b, pmo + 0xA0 + k * 0x20 + 4, 12))
            .collect::<Result<Vec<_>>>()?;
        for (slot, translucent, corners) in pmo_meshes(b, pmo)? {
            if corners.is_empty() {
                continue;
            }
            let texture = slot_names
                .get(slot)
                .and_then(|n| model.texture_names.iter().position(|t| t == n));
            let mut mesh = MapMesh {
                texture,
                translucent,
                sky,
                ..MapMesh::default()
            };
            for v in &corners {
                let p = rotate([0, 1, 2].map(|k| v.position[k] * scale[k]), rot);
                mesh.positions
                    .push([p[0] + at[0], p[1] + at[1], p[2] + at[2]]);
                mesh.uvs.push(v.uv);
                mesh.colors.push(v.color);
            }
            model.meshes.push(mesh);
        }
    }
    Ok(model)
}

/// An HD image's mip chain: (width, height, RGBA) from the largest.
pub type MipChain = Vec<(u32, u32, Vec<u8>)>;

/// The largest mean RGBA difference (0-255) an HD image may have from the
/// picture it replaces.
const HD_MATCH_LIMIT: f32 = 120.;

/// Which HD image replaces each picture. The PC remaster names its images
/// by an order that is not the map's texture table (an archive's other
/// pictures come first), so each picture takes the image whose mip level of
/// its own size looks most like it (RGBA, mean difference), each image once,
/// best pairs first [installed-build observation, T249].
pub fn match_hd(pictures: &[Picture], hd: &[MipChain]) -> Vec<Option<usize>> {
    let mut pairs = Vec::new();
    for (i, p) in pictures.iter().enumerate() {
        for (k, chain) in hd.iter().enumerate() {
            let Some((_, _, level)) = chain
                .iter()
                .find(|(w, h, _)| *w == p.width && *h == p.height)
            else {
                continue;
            };
            if level.len() != p.rgba.len() || level.is_empty() {
                continue;
            }
            let sum: u64 = level
                .iter()
                .zip(&p.rgba)
                .map(|(&a, &b)| u64::from(a.abs_diff(b)))
                .sum();
            pairs.push((sum as f32 / level.len() as f32, i, k));
        }
    }
    pairs.sort_by(|a, b| a.0.total_cmp(&b.0));
    let mut out = vec![None; pictures.len()];
    let mut used = vec![false; hd.len()];
    for (d, i, k) in pairs {
        if d <= HD_MATCH_LIMIT && out[i].is_none() && !used[k] {
            out[i] = Some(k);
            used[k] = true;
        }
    }
    out
}

/// One collision face: its corners (3 or 4, on its plane) and flags.
#[derive(Clone, Debug, PartialEq)]
pub struct CollisionFace {
    pub corners: Vec<[f32; 3]>,
    /// The plane's unit normal (n·p = d).
    pub normal: [f32; 3],
    /// The 24 bytes after the corners (material and attribute bits).
    pub flags: [u8; 24],
}

/// The faces of a BCD collision file.
pub fn bcd(b: &[u8]) -> Result<Vec<CollisionFace>> {
    if b.get(..4) != Some(b"@bcd") {
        return Err(invalid("not a BCD collision file"));
    }
    let vcount = usize::from(u16_at(b, 0x34)?);
    let fcount = usize::from(u16_at(b, 0x36)?);
    let (voff, foff) = (u32_at(b, 0x38)? as usize, u32_at(b, 0x3C)? as usize);
    let verts = (0..vcount)
        .map(|i| vec3_at(b, voff + i * 16))
        .collect::<Result<Vec<_>>>()?;
    let mut out = Vec::with_capacity(fcount);
    for i in 0..fcount {
        let o = foff + i * 48;
        let normal = vec3_at(b, o)?;
        let mut corners = Vec::with_capacity(4);
        for k in 0..4 {
            let idx = u16_at(b, o + 16 + k * 2)?;
            if idx == 0xFFFF {
                break;
            }
            corners.push(
                *verts
                    .get(usize::from(idx))
                    .ok_or_else(|| invalid("face corner past the vertices"))?,
            );
        }
        if corners.len() < 3 {
            return Err(invalid("face with fewer than three corners"));
        }
        let mut flags = [0; 24];
        flags.copy_from_slice(b.get(o + 24..o + 48).ok_or_else(|| invalid("face short"))?);
        out.push(CollisionFace {
            corners,
            normal,
            flags,
        });
    }
    Ok(out)
}

impl CollisionFace {
    /// The face as triangles (a quad split along its first diagonal).
    pub fn triangles(&self) -> Vec<[[f32; 3]; 3]> {
        let c = &self.corners;
        let mut out = vec![[c[0], c[1], c[2]]];
        if c.len() == 4 {
            out.push([c[0], c[2], c[3]]);
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn put(b: &mut Vec<u8>, at: usize, bytes: &[u8]) {
        if b.len() < at + bytes.len() {
            b.resize(at + bytes.len(), 0);
        }
        b[at..at + bytes.len()].copy_from_slice(bytes);
    }

    fn f(v: f32) -> [u8; 4] {
        v.to_le_bytes()
    }

    #[test]
    fn arc_lists_stored_files_and_skips_links() {
        let mut b = Vec::new();
        put(&mut b, 0, b"ARC\0");
        put(&mut b, 4, &1u16.to_le_bytes());
        put(&mut b, 6, &2u16.to_le_bytes());
        // A stored file at 0x50, 4 bytes; a linked bank.
        put(&mut b, 0x14, &0x50u32.to_le_bytes());
        put(&mut b, 0x18, &4u32.to_le_bytes());
        put(&mut b, 0x20, b"a.bin");
        put(&mut b, 0x30, &0x0f92u32.to_le_bytes());
        put(&mut b, 0x40, b"se.scd");
        put(&mut b, 0x50, b"DATA");
        let files = arc_files(&b).unwrap();
        assert_eq!(files.len(), 1);
        assert_eq!(files[0].name, "a.bin");
        assert_eq!(files[0].bytes, b"DATA");
        put(&mut b, 0x18, &400u32.to_le_bytes());
        assert!(
            arc_files(&b).is_err(),
            "an entry past the archive is refused"
        );
    }

    fn tim2_4bit() -> Vec<u8> {
        // 2x2, 4-bit, 16 colours: pixels 1,0 / 0,1.
        let mut b = Vec::new();
        put(&mut b, 0, b"TIM2");
        let q = 16;
        put(&mut b, q + 4, &64u32.to_le_bytes());
        put(&mut b, q + 8, &2u32.to_le_bytes());
        put(&mut b, q + 12, &0x30u16.to_le_bytes());
        put(&mut b, q + 14, &16u16.to_le_bytes());
        b.resize(q + 0x30, 0);
        b[q + 19] = 4;
        put(&mut b, q + 20, &2u16.to_le_bytes());
        put(&mut b, q + 22, &2u16.to_le_bytes());
        put(&mut b, q + 0x30, &[0x01, 0x10]);
        let clut = q + 0x30 + 2;
        put(&mut b, clut, &[0, 0, 0, 0]);
        put(&mut b, clut + 4, &[255, 128, 0, 255]);
        b.resize(clut + 64, 0);
        b
    }

    #[test]
    fn tim2_indexes_its_palette_low_nibble_first() {
        let p = tim2(&tim2_4bit(), 0).unwrap();
        assert_eq!((p.width, p.height), (2, 2));
        assert_eq!(&p.rgba[..4], &[255, 128, 0, 255]);
        assert_eq!(&p.rgba[4..8], &[0, 0, 0, 0]);
        assert_eq!(&p.rgba[12..16], &[255, 128, 0, 255]);
    }

    /// A PMP with one object (moved by +10 x, scaled 2) whose PMO has one
    /// triangle section of float UVs and positions and a section colour.
    fn pmp_one_triangle() -> Vec<u8> {
        let mut b = Vec::new();
        put(&mut b, 0, b"PMP\0");
        put(&mut b, 0x10, &1u16.to_le_bytes());
        put(&mut b, 0x1A, &1u16.to_le_bytes());
        let pmo = 0x100;
        let tex = 0x400;
        put(&mut b, 0x1C, &(tex as u32).to_le_bytes());
        put(&mut b, 0x20, &f(10.));
        for k in 0..3 {
            put(&mut b, 0x20 + 24 + k * 4, &f(2.));
        }
        put(&mut b, 0x20 + 36, &(pmo as u32).to_le_bytes());
        put(&mut b, pmo, b"PMO\0");
        put(&mut b, pmo + 8, &1u16.to_le_bytes());
        put(&mut b, pmo + 0x10, &0xC0u32.to_le_bytes());
        put(&mut b, pmo + 0x18, &f(1.));
        put(&mut b, pmo + 0xA0 + 4, b"tex");
        let s = pmo + 0xC0;
        put(&mut b, s, &3u16.to_le_bytes());
        put(&mut b, s + 2, &[0, 20]);
        // Float UV, float position, section colour, triangles.
        put(&mut b, s + 4, &0x3100_0183u32.to_le_bytes());
        put(&mut b, s + 12, &[128, 128, 128, 255]);
        let v = s + 16;
        for (k, p) in [[0., 0., 0.], [1., 0., 0.], [0., 0., 1.]]
            .iter()
            .enumerate()
        {
            let o = v + k * 20;
            put(&mut b, o, &f(0.5));
            put(&mut b, o + 4, &f(0.25));
            for (c, &x) in p.iter().enumerate() {
                put(&mut b, o + 8 + c * 4, &f(x));
            }
        }
        // The list ends with a zero count.
        put(&mut b, v + 60, &[0; 4]);
        put(&mut b, tex + 4, b"tex");
        put(&mut b, tex + 16, &f(0.5));
        let tim = tim2_4bit();
        put(&mut b, tex, &0x500u32.to_le_bytes());
        put(&mut b, 0x500, &tim);
        b
    }

    #[test]
    fn pmp_places_its_objects_meshes_with_their_texture() {
        let m = pmp(&pmp_one_triangle()).unwrap();
        assert_eq!(m.texture_names, vec!["tex".to_string()]);
        assert_eq!(m.texture_scroll, vec![[0.5, 0.]]);
        assert_eq!(m.meshes.len(), 1);
        let mesh = &m.meshes[0];
        assert_eq!(mesh.texture, Some(0));
        assert!(!mesh.translucent);
        assert!(!mesh.sky);
        assert_eq!(
            mesh.positions,
            vec![[10., 0., 0.], [12., 0., 0.], [10., 0., 2.]]
        );
        assert_eq!(mesh.uvs[0], [0.5, 0.25]);
        assert_eq!(mesh.colors[0], [1., 1., 1., 1.]);
    }

    #[test]
    fn hd_images_go_to_the_pictures_they_look_like() {
        let pic = |c: u8| Picture {
            width: 2,
            height: 2,
            rgba: vec![c; 16],
        };
        // Two 4x4 images whose 2x2 levels are 200 and 10; and an unrelated
        // 8x2 one.
        let chain = |c: u8| vec![(4, 4, vec![c; 64]), (2, 2, vec![c; 16])];
        let hd = vec![chain(200), chain(10), vec![(8, 2, vec![0; 64])]];
        assert_eq!(match_hd(&[pic(12), pic(198)], &hd), vec![Some(1), Some(0)]);
        // Nothing alike: no replacement.
        assert_eq!(match_hd(&[pic(255)], &[chain(0)]), vec![None]);
    }

    #[test]
    fn strips_alternate_their_winding() {
        let v = |x: f32| Vertex {
            position: [x, 0., 0.],
            uv: [0.; 2],
            color: [1.; 4],
        };
        let t = strip_corners((0..4).map(|i| v(i as f32)).collect(), &[4]);
        let xs: Vec<f32> = t.iter().map(|v| v.position[0]).collect();
        assert_eq!(xs, vec![0., 1., 2., 2., 1., 3.]);
    }

    #[test]
    fn bcd_faces_take_their_corners_and_split_quads() {
        let mut b = Vec::new();
        put(&mut b, 0, b"@bcd");
        put(&mut b, 0x34, &4u16.to_le_bytes());
        put(&mut b, 0x36, &2u16.to_le_bytes());
        put(&mut b, 0x38, &0x50u32.to_le_bytes());
        put(&mut b, 0x3C, &0x90u32.to_le_bytes());
        for (i, p) in [[0., 0., 0.], [1., 0., 0.], [1., 0., 1.], [0., 0., 1.]]
            .iter()
            .enumerate()
        {
            for (c, &x) in p.iter().enumerate() {
                put(&mut b, 0x50 + i * 16 + c * 4, &f(x));
            }
        }
        put(&mut b, 0x90 + 4, &f(1.));
        for (k, i) in [0u16, 1, 2, 3].iter().enumerate() {
            put(&mut b, 0x90 + 16 + k * 2, &i.to_le_bytes());
        }
        put(&mut b, 0xC0 + 4, &f(1.));
        for (k, i) in [0u16, 1, 2, 0xFFFF].iter().enumerate() {
            put(&mut b, 0xC0 + 16 + k * 2, &i.to_le_bytes());
        }
        b.resize(0xF0, 0);
        let faces = bcd(&b).unwrap();
        assert_eq!(faces.len(), 2);
        assert_eq!(faces[0].triangles().len(), 2);
        assert_eq!(faces[1].triangles().len(), 1);
        assert_eq!(faces[0].normal, [0., 1., 0.]);
        put(&mut b, 0x90 + 16, &9u16.to_le_bytes());
        assert!(bcd(&b).is_err(), "a corner past the vertices is refused");
    }
}
