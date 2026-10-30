//! `UAnimSequence` exports: the compressed tracks decoded to keys, sampled the
//! way KH3's decoders do. Layout and decoders from StrawHatPro's
//! `docs/formats/animsequence.md` (MIT), which reads them from KH3's own
//! `GetBoneAtom` (0x143F120E0) and per-track decoders (0x143F1BF10,
//! 0x143F15D00, 0x143F19580).
use super::package::{Package, Rd};
use super::props::{Props, read_props};
use super::{Result, bad};

const F96: u8 = 1;
const F48: u8 = 2;
const INTERVAL: u8 = 3;
const IDENTITY: u8 = 6;
const NONE: u8 = 0;
const SCALE_MISSING: f32 = 0.000_099_999_997;
const Q16: f32 = 3.051_851e-5;
const Q16SQ: f32 = 9.313_794e-10;
const Q11: f32 = 0.000_977_517_1;
const Q10: f32 = 0.001_956_947_1;

/// Keys of one component.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Channel<const N: usize> {
    pub keys: Vec<[f32; N]>,
    /// Each key's frame, or empty when the keys are spread evenly.
    pub frames: Vec<u16>,
}

impl<const N: usize> Channel<N> {
    /// The two keys around `p` (0..1 through the clip) and the blend between.
    fn bracket(&self, p: f32, num_frames: u32) -> (usize, usize, f32) {
        let n = self.keys.len();
        if n <= 1 {
            return (0, 0, 0.);
        }
        if self.frames.is_empty() {
            let at = p * (n - 1) as f32;
            let i = (at as usize).min(n - 1);
            return (i, (i + 1).min(n - 1), at - i as f32);
        }
        let frame = p * num_frames.saturating_sub(1) as f32;
        let hi = self
            .frames
            .partition_point(|&f| f32::from(f) <= frame)
            .min(n - 1);
        let lo = hi.saturating_sub(1);
        let (a, b) = (f32::from(self.frames[lo]), f32::from(self.frames[hi]));
        if b <= a {
            return (hi, hi, 0.);
        }
        (lo, hi, ((frame - a) / (b - a)).clamp(0., 1.))
    }
}

/// One bone's keys. `None` means the component has no block.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Track {
    pub bone: u32,
    pub translation: Option<Channel<3>>,
    pub rotation: Option<Channel<4>>,
    pub scale: Option<Channel<3>>,
}

#[derive(Clone, Debug, Default)]
pub struct AnimSequence {
    pub props: Props,
    pub length: f32,
    pub num_frames: u32,
    pub has_scale_table: bool,
    pub tracks: Vec<Track>,
    /// Holds the earlier key (`Step`, `m_DisableInterpolation`).
    pub step: bool,
}

/// A bone's local pose.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct BonePose {
    pub rotation: [f32; 4],
    pub translation: [f32; 3],
    pub scale: [f32; 3],
}

fn wrap_w(x: f32, y: f32, z: f32) -> f32 {
    let w = 1. - x * x - y * y - z * z;
    if w > 0. { w.sqrt() } else { 0. }
}

/// How many components a block's key has: format, then flags & 7.
fn comps(format: u8, flags: u8) -> usize {
    match format {
        F96 | F48 => [3, 1, 1, 2, 1, 2, 2, 3][usize::from(flags & 7)],
        INTERVAL => [6, 2, 2, 4, 2, 4, 4, 6][usize::from(flags & 7)],
        NONE => 4,
        4 | 5 => 1,
        _ => 0,
    }
}

fn bytes_per_key(format: u8, flags: u8) -> usize {
    match format {
        INTERVAL | 4 | 5 => 4,
        F48 => comps(format, flags) * 2,
        IDENTITY => 0,
        _ => comps(format, flags) * 4,
    }
}

fn pad4(r: &mut Rd, start: usize) -> Result<()> {
    let n = (r.p - start) % 4;
    if n != 0 {
        r.skip(4 - n)?;
    }
    Ok(())
}

struct Block {
    format: u8,
    flags: u8,
    keys: usize,
    bounds: Vec<f32>,
    data: Vec<u8>,
    frames: Vec<u16>,
}

fn read_block(r: &mut Rd, num_frames: u32) -> Result<Block> {
    let start = r.p;
    let head = r.u32()?;
    let format = (head >> 28) as u8;
    let flags = ((head >> 24) & 15) as u8;
    let keys = (head & 0xff_ffff) as usize;
    let mut bounds = Vec::new();
    if format == INTERVAL {
        for bit in 0..3 {
            if flags & (1 << bit) != 0 {
                bounds.push(r.f32()?);
                bounds.push(r.f32()?);
            }
        }
    }
    let data = r.take(keys * bytes_per_key(format, flags))?.to_vec();
    pad4(r, start)?;
    let mut frames = Vec::new();
    if flags & 8 != 0 {
        let table = r.p;
        frames.reserve(keys);
        for _ in 0..keys {
            frames.push(if num_frames > 255 {
                r.u16()?
            } else {
                u16::from(r.u8()?)
            });
        }
        pad4(r, table)?;
    }
    Ok(Block {
        format,
        flags,
        keys,
        bounds,
        data,
        frames,
    })
}

/// Rotation keys; the three stored values and the derived W.
fn decode_rotation(b: &Block) -> Result<Channel<4>> {
    let mut r = Rd::new(&b.data);
    let mut keys = Vec::with_capacity(b.keys);
    for _ in 0..b.keys {
        keys.push(match b.format {
            IDENTITY => [0., 0., 0., 1.],
            F96 => {
                let (x, y, z) = (r.f32()?, r.f32()?, r.f32()?);
                [x, y, z, wrap_w(x, y, z)]
            }
            F48 => {
                let mut v = [0f32; 3];
                let mut sum = 0f32;
                for (bit, c) in v.iter_mut().enumerate() {
                    if b.flags & (1 << bit) != 0 || b.flags & 7 == 0 {
                        let d = f32::from(r.u16()?) - 32767.;
                        *c = d * Q16;
                        sum += d * d;
                    }
                }
                let w = 1. - sum * Q16SQ;
                [v[0], v[1], v[2], if w > 0. { w.sqrt() } else { 0. }]
            }
            INTERVAL => {
                let v = r.u32()?;
                let at = |i: usize| (b.bounds[i * 2], b.bounds[i * 2 + 1]);
                let (x0, xr) = at(0);
                let (y0, yr) = at(1);
                let (z0, zr) = at(2);
                let x = ((v >> 21) as f32 - 1023.) * Q11 * xr + x0;
                let y = (((v >> 10) & 0x7ff) as f32 - 1023.) * Q11 * yr + y0;
                let z = ((v & 0x3ff) as f32 - 511.) * Q10 * zr + z0;
                [x, y, z, wrap_w(x, y, z)]
            }
            f => return Err(bad(format!("rotation format {f}"))),
        });
    }
    Ok(Channel {
        keys,
        frames: b.frames.clone(),
    })
}

/// Translation or scale keys. A component not stored reads `missing`.
fn decode_vector(b: &Block, missing: f32) -> Result<Channel<3>> {
    let mut r = Rd::new(&b.data);
    let mut keys = Vec::with_capacity(b.keys);
    let set = |bit: u8| b.flags & (1 << bit) != 0 || b.flags & 7 == 0;
    for _ in 0..b.keys {
        keys.push(match b.format {
            IDENTITY => [missing; 3],
            F96 => {
                let mut v = [missing; 3];
                for (bit, c) in v.iter_mut().enumerate() {
                    if set(bit as u8) {
                        *c = r.f32()?;
                    }
                }
                v
            }
            F48 => {
                let mut v = [missing; 3];
                for (bit, c) in v.iter_mut().enumerate() {
                    if set(bit as u8) {
                        *c = f32::from(r.u16()?) - 255.;
                    }
                }
                v
            }
            INTERVAL => {
                let v = r.u32()?;
                // Bounds hold only the flagged components, in order.
                let mut o = 0;
                let mut next = |bit: u8| -> (f32, f32) {
                    if b.flags & (1 << bit) != 0 {
                        let pair = (b.bounds[o], b.bounds[o + 1]);
                        o += 2;
                        pair
                    } else {
                        (f32::NAN, 0.)
                    }
                };
                let (x0, xr) = next(0);
                let (y0, yr) = next(1);
                let (z0, zr) = next(2);
                let pick = |min: f32, value: f32| if min.is_nan() { missing } else { value };
                [
                    pick(x0, ((v & 0x3ff) as f32 - 511.) * Q10 * xr + x0),
                    pick(y0, (((v >> 10) & 0x7ff) as f32 - 1023.) * Q11 * yr + y0),
                    pick(z0, ((v >> 21) as f32 - 1023.) * Q11 * zr + z0),
                ]
            }
            f => return Err(bad(format!("vector format {f}"))),
        });
    }
    Ok(Channel {
        keys,
        frames: b.frames.clone(),
    })
}

fn read_i32s(r: &mut Rd) -> Result<Vec<i32>> {
    let n = r.count(4)?;
    let mut out = Vec::with_capacity(n);
    for _ in 0..n {
        out.push(r.i32()?);
    }
    Ok(out)
}

fn key_lerp_channel3(
    r: &mut Rd,
    keys: usize,
    variable: bool,
    num_frames: u32,
) -> Result<Channel<3>> {
    let start = r.p;
    let mut out = Vec::with_capacity(keys);
    for _ in 0..keys {
        out.push([r.f32()?, r.f32()?, r.f32()?]);
    }
    pad4(r, start)?;
    let frames = lerp_frames(r, keys, variable, num_frames)?;
    Ok(Channel { keys: out, frames })
}

fn key_lerp_channel4(
    r: &mut Rd,
    keys: usize,
    variable: bool,
    num_frames: u32,
) -> Result<Channel<4>> {
    let start = r.p;
    let mut out = Vec::with_capacity(keys);
    for _ in 0..keys {
        let (x, y, z) = (r.f32()?, r.f32()?, r.f32()?);
        out.push([x, y, z, wrap_w(x, y, z)]);
    }
    pad4(r, start)?;
    let frames = lerp_frames(r, keys, variable, num_frames)?;
    Ok(Channel { keys: out, frames })
}

fn lerp_frames(r: &mut Rd, keys: usize, variable: bool, num_frames: u32) -> Result<Vec<u16>> {
    let mut frames = Vec::new();
    if variable && keys > 1 {
        let start = r.p;
        for _ in 0..keys {
            frames.push(if num_frames > 255 {
                r.u16()?
            } else {
                u16::from(r.u8()?)
            });
        }
        pad4(r, start)?;
    }
    Ok(frames)
}

impl AnimSequence {
    /// Reads export `i` of the package.
    pub fn read(p: &Package, i: usize) -> Result<Self> {
        Self::parse(p.export_bytes(i)?, &p.names)
    }

    pub fn parse(bytes: &[u8], names: &[String]) -> Result<Self> {
        let mut r = Rd::new(bytes);
        let props = read_props(&mut r, names)?;
        r.i32()?;
        r.skip(16)?;
        r.skip(2)?;
        if r.i32()? != 1 {
            return Err(bad("animation without compressed data"));
        }
        let key_format = r.u8()?;
        r.skip(3)?;
        let offsets = read_i32s(&mut r)?;
        let scale_offsets = read_i32s(&mut r)?;
        let strip = r.i32()?.max(1) as usize;
        let bones = read_i32s(&mut r)?;
        read_props(&mut r, names)?;
        r.i32()?;
        let stream_len = usize::try_from(r.i32()?).map_err(|_| bad("stream length"))?;
        let stream = r.take(stream_len)?;
        let length = props.f32("SequenceLength").unwrap_or(0.);
        let num_frames = props.int("NumFrames").unwrap_or(0).max(0) as u32;
        let step = props.bool("m_DisableInterpolation").unwrap_or(false)
            || props
                .str("Interpolation")
                .is_some_and(|s| s.ends_with("Step"));
        let has_scale_table = !scale_offsets.is_empty();
        let mut s = Rd::new(stream);
        let mut tracks = Vec::with_capacity(bones.len());
        match key_format {
            2 => {
                for (t, &bone) in bones.iter().enumerate() {
                    let mut track = Track {
                        bone: bone as u32,
                        ..Track::default()
                    };
                    if offsets.get(2 * t).copied().unwrap_or(-1) != -1 {
                        let b = read_block(&mut s, num_frames)?;
                        track.translation = Some(decode_vector(&b, 0.)?);
                    }
                    if offsets.get(2 * t + 1).copied().unwrap_or(-1) != -1 {
                        let b = read_block(&mut s, num_frames)?;
                        track.rotation = Some(decode_rotation(&b)?);
                    }
                    if has_scale_table && scale_offsets.get(t * strip).copied().unwrap_or(-1) != -1
                    {
                        let b = read_block(&mut s, num_frames)?;
                        track.scale = Some(decode_vector(&b, SCALE_MISSING)?);
                    }
                    tracks.push(track);
                }
            }
            0 | 1 => {
                let variable = key_format == 1;
                for (t, &bone) in bones.iter().enumerate() {
                    let tk = offsets.get(4 * t + 1).copied().unwrap_or(0).max(0) as usize;
                    let rk = offsets.get(4 * t + 3).copied().unwrap_or(0).max(0) as usize;
                    let translation = key_lerp_channel3(&mut s, tk, variable, num_frames)?;
                    let rotation = key_lerp_channel4(&mut s, rk, variable, num_frames)?;
                    let scale = if has_scale_table {
                        let sk = scale_offsets.get(2 * t + 1).copied().unwrap_or(0).max(0) as usize;
                        Some(key_lerp_channel3(&mut s, sk, variable, num_frames)?)
                    } else {
                        None
                    };
                    tracks.push(Track {
                        bone: bone as u32,
                        translation: Some(translation),
                        rotation: Some(rotation),
                        scale,
                    });
                }
            }
            k => return Err(bad(format!("key encoding {k}"))),
        }
        Ok(Self {
            props,
            length,
            num_frames,
            has_scale_table,
            tracks,
            step,
        })
    }

    /// One track's local pose at `time` seconds: translation linear, rotation
    /// a normalised blend along the shorter arc, scale geometric per
    /// component. Missing components: translation and rotation identity;
    /// scale (0,0,0) with a scale table, (1,1,1) without.
    pub fn sample_track(&self, track: &Track, time: f32) -> BonePose {
        let p = if self.length > 0. {
            (time / self.length).clamp(0., 1.)
        } else {
            0.
        };
        let translation = track.translation.as_ref().map_or([0.; 3], |c| {
            let (a, b, t) = c.bracket(p, self.num_frames);
            let t = if self.step { 0. } else { t };
            let (a, b) = (c.keys.get(a).copied(), c.keys.get(b).copied());
            match (a, b) {
                (Some(a), Some(b)) => [
                    a[0] + (b[0] - a[0]) * t,
                    a[1] + (b[1] - a[1]) * t,
                    a[2] + (b[2] - a[2]) * t,
                ],
                _ => [0.; 3],
            }
        });
        let rotation = track.rotation.as_ref().map_or([0., 0., 0., 1.], |c| {
            let (a, b, t) = c.bracket(p, self.num_frames);
            let t = if self.step { 0. } else { t };
            match (c.keys.get(a).copied(), c.keys.get(b).copied()) {
                (Some(a), Some(b)) => nlerp(a, b, t),
                _ => [0., 0., 0., 1.],
            }
        });
        let scale = match &track.scale {
            None => {
                if self.has_scale_table {
                    [0.; 3]
                } else {
                    [1.; 3]
                }
            }
            Some(c) => {
                let (a, b, t) = c.bracket(p, self.num_frames);
                let t = if self.step { 0. } else { t };
                match (c.keys.get(a).copied(), c.keys.get(b).copied()) {
                    (Some(a), Some(b)) => [
                        geometric(a[0], b[0], t),
                        geometric(a[1], b[1], t),
                        geometric(a[2], b[2], t),
                    ],
                    _ => [1.; 3],
                }
            }
        };
        BonePose {
            rotation,
            translation,
            scale,
        }
    }
}

fn geometric(a: f32, b: f32, t: f32) -> f32 {
    if a * b > 0. {
        a * (b / a).powf(t)
    } else {
        a + (b - a) * t
    }
}

pub fn nlerp(a: [f32; 4], b: [f32; 4], t: f32) -> [f32; 4] {
    let dot = a[0] * b[0] + a[1] * b[1] + a[2] * b[2] + a[3] * b[3];
    let s = if dot < 0. { -t } else { t };
    let q = [
        a[0] * (1. - t) + b[0] * s,
        a[1] * (1. - t) + b[1] * s,
        a[2] * (1. - t) + b[2] * s,
        a[3] * (1. - t) + b[3] * s,
    ];
    let n = (q[0] * q[0] + q[1] * q[1] + q[2] * q[2] + q[3] * q[3]).sqrt();
    if n > 1e-8 {
        [q[0] / n, q[1] / n, q[2] / n, q[3] / n]
    } else {
        [0., 0., 0., 1.]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn block(format: u8, flags: u8, keys: u32, body: &[u8]) -> Vec<u8> {
        let mut b = ((u32::from(format) << 28) | (u32::from(flags) << 24) | keys)
            .to_le_bytes()
            .to_vec();
        b.extend_from_slice(body);
        while b.len() % 4 != 0 {
            b.push(0x55);
        }
        b
    }

    fn f32s(v: &[f32]) -> Vec<u8> {
        v.iter().flat_map(|f| f.to_le_bytes()).collect()
    }

    /// An export: no properties but what sampling needs.
    fn export(
        names: &[String],
        stream: &[u8],
        offsets: &[i32],
        scale: &[i32],
        bones: &[i32],
    ) -> Vec<u8> {
        let name = |s: &str| names.iter().position(|n| n == s).unwrap() as i32;
        let mut e = Vec::new();
        let tag = |e: &mut Vec<u8>, n: &str, kind: &str, size: i32| {
            e.extend(name(n).to_le_bytes());
            e.extend(0i32.to_le_bytes());
            e.extend(name(kind).to_le_bytes());
            e.extend(0i32.to_le_bytes());
            e.extend(size.to_le_bytes());
            e.extend(0i32.to_le_bytes());
            e.push(0);
        };
        tag(&mut e, "NumFrames", "IntProperty", 4);
        e.extend(4i32.to_le_bytes());
        tag(&mut e, "SequenceLength", "FloatProperty", 4);
        e.extend(0.1f32.to_le_bytes());
        e.extend(name("None").to_le_bytes());
        e.extend(0i32.to_le_bytes());
        e.extend(0i32.to_le_bytes());
        e.extend([0; 16]);
        e.extend([1, 0]);
        e.extend(1i32.to_le_bytes());
        e.push(2);
        e.extend([6, 6, 6]);
        let arr = |e: &mut Vec<u8>, v: &[i32]| {
            e.extend((v.len() as i32).to_le_bytes());
            for x in v {
                e.extend(x.to_le_bytes());
            }
        };
        arr(&mut e, offsets);
        arr(&mut e, scale);
        e.extend(1i32.to_le_bytes());
        arr(&mut e, bones);
        e.extend(name("None").to_le_bytes());
        e.extend(0i32.to_le_bytes());
        e.extend(0i32.to_le_bytes());
        e.extend((stream.len() as i32).to_le_bytes());
        e.extend_from_slice(stream);
        e.extend(0i32.to_le_bytes());
        e
    }

    fn names() -> Vec<String> {
        [
            "None",
            "NumFrames",
            "SequenceLength",
            "IntProperty",
            "FloatProperty",
        ]
        .iter()
        .map(|s| (*s).to_owned())
        .collect()
    }

    #[test]
    fn per_track_blocks_are_decoded_and_sampled() {
        let names = names();
        let mut stream = Vec::new();
        // Translation: two float keys, X and Z stored; Y is 0.
        stream.extend(block(F96, 0b101, 2, &f32s(&[0., 10., 4., 20.])));
        // Rotation: one Fixed48 key, all three: the identity.
        let q: Vec<u8> = [32767u16, 32767, 32767]
            .iter()
            .flat_map(|v| v.to_le_bytes())
            .collect();
        stream.extend(block(F48, 0b111, 1, &q));
        let e = export(&names, &stream, &[0, 16], &[], &[7]);
        let a = AnimSequence::parse(&e, &names).unwrap();
        assert_eq!(a.tracks.len(), 1);
        assert_eq!(a.tracks[0].bone, 7);
        let pose = a.sample_track(&a.tracks[0], 0.05);
        assert!((pose.translation[0] - 2.).abs() < 1e-5);
        assert_eq!(pose.translation[1], 0.);
        assert!((pose.translation[2] - 15.).abs() < 1e-5);
        assert_eq!(pose.scale, [1.; 3]);
        assert!((pose.rotation[3] - 1.).abs() < 1e-5);
    }

    #[test]
    fn fixed48_vectors_have_no_scale_factor() {
        let b = Block {
            format: F48,
            flags: 7,
            keys: 1,
            bounds: Vec::new(),
            data: [255u16, 256, 355]
                .iter()
                .flat_map(|v| v.to_le_bytes())
                .collect(),
            frames: Vec::new(),
        };
        assert_eq!(decode_vector(&b, 0.).unwrap().keys[0], [0., 1., 100.]);
    }

    #[test]
    fn a_missing_scale_component_is_tiny() {
        let b = Block {
            format: F96,
            flags: 1,
            keys: 1,
            bounds: Vec::new(),
            data: f32s(&[2.]),
            frames: Vec::new(),
        };
        let k = decode_vector(&b, SCALE_MISSING).unwrap().keys[0];
        assert_eq!(k[0], 2.);
        assert_eq!(k[1], SCALE_MISSING);
    }

    #[test]
    fn interval_keys_use_their_bounds() {
        let b = Block {
            format: INTERVAL,
            flags: 7,
            keys: 1,
            bounds: vec![1., 2., 3., 4., 5., 6.],
            data: ((1023u32 << 21) | (1023 << 10) | 511)
                .to_le_bytes()
                .to_vec(),
            frames: Vec::new(),
        };
        let k = decode_vector(&b, 0.).unwrap().keys[0];
        assert_eq!(k, [1., 3., 5.]);
    }

    #[test]
    fn frame_tables_bracket_keys() {
        let c = Channel::<3> {
            keys: vec![[0.; 3], [10.; 3]],
            frames: vec![0, 9],
        };
        let (a, b, t) = c.bracket(0.5, 10);
        assert_eq!((a, b), (0, 1));
        assert!((t - 0.5).abs() < 1e-6);
    }

    #[test]
    fn nlerp_takes_the_short_way() {
        let q = nlerp([0., 0., 0., 1.], [0., 0., 0., -1.], 0.5);
        assert!((q[3].abs() - 1.).abs() < 1e-5);
    }

    #[test]
    fn scale_blend_is_geometric() {
        assert!((geometric(1., 4., 0.5) - 2.).abs() < 1e-5);
        assert!((geometric(-1., 1., 0.5)).abs() < 1e-5);
    }
}
