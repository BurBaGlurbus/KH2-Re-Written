//! CRI HCA streams as KH3's SEAD materials store them, decoded to PCM. A port of
//! StrawHatPro's `tools/lib/hca.js` (MIT). The decode tables are the player's
//! own: they are read from KH3's executable (`Tables::find`), not stored here.
//! Single precision throughout, as the game's decoder runs.
use super::{Result, bad};

/// The executable's tables, found by the generated CRC table they sit around.
#[derive(Clone, Debug)]
pub struct Tables {
    invert: Vec<u8>,
    intensity: Vec<f32>,
    scale_conv: Vec<f32>,
    max_bits: Vec<u8>,
    read_val: Vec<f32>,
    read_bits: Vec<u8>,
    crc16: Vec<u16>,
    window: Vec<f32>,
    scale: Vec<f32>,
    range: Vec<f32>,
    sin0: Vec<f32>,
    cos0: Vec<f32>,
}

fn crc_table() -> Vec<u16> {
    (0..256u32)
        .map(|i| {
            let mut c = (i << 8) as u16;
            for _ in 0..8 {
                c = if c & 0x8000 != 0 {
                    (c << 1) ^ 0x8005
                } else {
                    c << 1
                };
            }
            c
        })
        .collect()
}

impl Tables {
    /// Finds the tables in the executable's bytes; the sanity facts the decoder
    /// relies on have to hold, or the build is refused.
    pub fn find(image: &[u8]) -> Result<Self> {
        let crc = crc_table();
        let want: Vec<u8> = crc.iter().flat_map(|c| c.to_le_bytes()).collect();
        let at = image
            .windows(want.len())
            .position(|w| w == want.as_slice())
            .ok_or_else(|| bad("the audio tables are not in this executable"))?;
        let bytes = |offset: isize, n: usize| -> Result<&[u8]> {
            let from = at
                .checked_add_signed(offset)
                .ok_or_else(|| bad("audio table out of the file"))?;
            image
                .get(from..from + n)
                .ok_or_else(|| bad("audio table out of the file"))
        };
        let floats = |offset: isize, n: usize| -> Result<Vec<f32>> {
            Ok(bytes(offset, n * 4)?
                .chunks_exact(4)
                .map(|c| f32::from_le_bytes([c[0], c[1], c[2], c[3]]))
                .collect())
        };
        let t = Tables {
            invert: bytes(-0x23a8, 57)?.to_vec(),
            intensity: floats(-0x2360, 16)?,
            scale_conv: floats(-0x2320, 128)?,
            max_bits: bytes(-0x20c0, 16)?.to_vec(),
            read_val: floats(-0x20b0, 128)?,
            read_bits: bytes(-0x1eb0, 128)?.to_vec(),
            crc16: crc,
            window: floats(0x200, 128)?,
            scale: floats(0x400, 64)?,
            range: floats(0x500, 16)?,
            cos0: floats(0x3050, 448)?,
            sin0: floats(0x3750, 448)?,
        };
        if t.scale_conv[63] != 1. || t.window[64] >= 0. || t.max_bits[15] != 12 {
            return Err(bad("the audio tables do not look right in this executable"));
        }
        Ok(t)
    }

    pub fn crc(&self, b: &[u8]) -> u16 {
        let mut c = 0u16;
        for &x in b {
            c = (c << 8) ^ self.crc16[usize::from((c >> 8) as u8 ^ x)];
        }
        c
    }
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Header {
    pub version: u16,
    pub size: u16,
    pub channels: u8,
    pub rate: u32,
    pub frames: u32,
    pub delay: u16,
    pub padding: u16,
    pub frame_size: u16,
    pub tracks: u8,
    pub config: u8,
    pub total_bands: u8,
    pub base_bands: u8,
    pub stereo_bands: u8,
    pub bands_per_hfr: u8,
    pub ms_stereo: u8,
    pub cipher: u16,
    pub ath: Option<u16>,
    /// Start frame, end frame, start delay, end padding.
    pub looped: Option<(u32, u32, u16, u16)>,
}

impl Header {
    pub fn parse(b: &[u8]) -> Result<Self> {
        let u16_at = |p: usize| -> Result<u16> {
            b.get(p..p + 2)
                .map(|s| u16::from_be_bytes([s[0], s[1]]))
                .ok_or_else(|| bad("short HCA header"))
        };
        let u32_at = |p: usize| -> Result<u32> {
            b.get(p..p + 4)
                .map(|s| u32::from_be_bytes([s[0], s[1], s[2], s[3]]))
                .ok_or_else(|| bad("short HCA header"))
        };
        let tag = |p: usize| -> Result<[u8; 4]> {
            let v = u32_at(p)? & 0x7f7f_7f7f;
            Ok(v.to_be_bytes())
        };
        if &tag(0)? != b"HCA\0" {
            return Err(bad("not an HCA header"));
        }
        let mut h = Header {
            version: u16_at(4)?,
            size: u16_at(6)?,
            ..Header::default()
        };
        let end = usize::from(h.size).saturating_sub(2);
        let mut p = 8;
        while p + 4 <= end {
            let at = |o: usize| b.get(p + o).copied().unwrap_or(0);
            match &tag(p)? {
                b"fmt\0" => {
                    h.channels = at(4);
                    h.rate = u32_at(p + 4)? & 0xff_ffff;
                    h.frames = u32_at(p + 8)?;
                    h.delay = u16_at(p + 12)?;
                    h.padding = u16_at(p + 14)?;
                    p += 16;
                }
                b"comp" => {
                    h.frame_size = u16_at(p + 4)?;
                    h.tracks = at(8);
                    h.config = at(9);
                    h.total_bands = at(10);
                    h.base_bands = at(11);
                    h.stereo_bands = at(12);
                    h.bands_per_hfr = at(13);
                    h.ms_stereo = at(14);
                    p += 16;
                }
                b"vbr\0" => p += 8,
                b"ath\0" => {
                    h.ath = Some(u16_at(p + 4)?);
                    p += 6;
                }
                b"loop" => {
                    h.looped = Some((
                        u32_at(p + 4)?,
                        u32_at(p + 8)?,
                        u16_at(p + 12)?,
                        u16_at(p + 14)?,
                    ));
                    p += 16;
                }
                b"ciph" => {
                    h.cipher = u16_at(p + 4)?;
                    p += 6;
                }
                b"rva\0" => p += 8,
                b"comm" | b"pad\0" => break,
                other => return Err(bad(format!("unknown HCA chunk {other:?}"))),
            }
        }
        if h.channels == 0 || h.frame_size == 0 {
            return Err(bad("HCA header lacks fmt or comp"));
        }
        Ok(h)
    }

    fn hfr_groups(&self) -> usize {
        if self.bands_per_hfr == 0 {
            return 0;
        }
        let rest =
            i32::from(self.total_bands) - i32::from(self.base_bands) - i32::from(self.stereo_bands);
        (rest.max(0) as usize).div_ceil(usize::from(self.bands_per_hfr))
    }

    pub fn samples(&self) -> usize {
        (self.frames as usize * 1024)
            .saturating_sub(usize::from(self.delay) + usize::from(self.padding))
    }
}

struct Channel {
    kind: u8,
    coded: usize,
    scale_factors: [u8; 128],
    hfr: [u8; 128],
    resolution: [u8; 128],
    intensity: [u8; 8],
    gain: [f32; 128],
    spectra: [f32; 128],
    temp: [f32; 128],
    prev: [f32; 128],
}

fn channel_kinds(h: &Header) -> Vec<u8> {
    let channels = usize::from(h.channels);
    let mut types = vec![0u8; channels];
    let tracks = usize::from(h.tracks.max(1));
    let per = channels / tracks;
    if h.stereo_bands > 0 && per > 1 {
        for t in 0..tracks {
            let c = t * per;
            let mut pair = |i: usize| {
                if c + i + 1 < channels {
                    types[c + i] = 1;
                    types[c + i + 1] = 2;
                }
            };
            match per {
                2 | 3 => pair(0),
                4 => {
                    pair(0);
                    if h.config == 0 {
                        pair(2);
                    }
                }
                5 => {
                    pair(0);
                    if h.config <= 2 {
                        pair(3);
                    }
                }
                6 | 7 => {
                    pair(0);
                    pair(4);
                }
                8 => {
                    pair(0);
                    pair(4);
                    pair(6);
                }
                _ => {}
            }
        }
    }
    types
}

struct Bits<'a> {
    b: &'a [u8],
    bit: usize,
    limit: usize,
}

impl Bits<'_> {
    fn peek(&self, n: usize) -> u32 {
        let mut v = 0;
        for k in 0..n {
            let p = self.bit + k;
            let on = p < self.limit && (self.b[p >> 3] >> (7 - (p & 7))) & 1 != 0;
            v = (v << 1) | u32::from(on);
        }
        v
    }

    fn read(&mut self, n: usize) -> u32 {
        let v = self.peek(n);
        self.bit += n;
        v
    }
}

fn imdct(c: &mut Channel, t: &Tables, out: &mut [f32]) {
    // Butterflies, ping-pong between the two buffers.
    let mut a = c.spectra;
    let mut b = c.temp;
    let (mut count1, mut count2) = (1usize, 64usize);
    for _ in 0..7 {
        let (mut s, mut d1, mut d2) = (0, 0, count2);
        for _ in 0..count1 {
            for _ in 0..count2 {
                let (x, y) = (a[s], a[s + 1]);
                s += 2;
                b[d1] = x + y;
                b[d2] = x - y;
                d1 += 1;
                d2 += 1;
            }
            d1 += count2;
            d2 += count2;
        }
        std::mem::swap(&mut a, &mut b);
        count1 <<= 1;
        count2 >>= 1;
    }
    // Rotations: stage s reads its 64 sin and cos at (6 - s) * 64.
    let (mut count1, mut count2) = (64usize, 1usize);
    for stage in 0..7 {
        let mut ti = (6 - stage) * 64;
        let (mut s1, mut s2, mut d1, mut d2) = (0, count2, 0, count2 * 2 - 1);
        for _ in 0..count1 {
            for _ in 0..count2 {
                let (x, y) = (a[s1], a[s2]);
                s1 += 1;
                s2 += 1;
                let (sn, cs) = (t.sin0[ti], t.cos0[ti]);
                ti += 1;
                b[d1] = x * sn - y * cs;
                b[d2] = x * cs + y * sn;
                d1 += 1;
                d2 = d2.wrapping_sub(1);
            }
            s1 += count2;
            s2 += count2;
            d1 += count2;
            d2 = d2.wrapping_add(count2 * 3);
        }
        std::mem::swap(&mut a, &mut b);
        count1 >>= 1;
        count2 <<= 1;
    }
    let (dct, w) = (&a, &t.window);
    for i in 0..64 {
        out[i] = w[i] * dct[64 + i] + c.prev[i];
        out[64 + i] = w[64 + i] * dct[127 - i] - c.prev[64 + i];
    }
    for i in 0..64 {
        c.prev[i] = w[127 - i] * dct[63 - i];
        c.prev[64 + i] = w[63 - i] * dct[i];
    }
}

/// A decoded stream: interleaved i16 samples and its loop, in frames (samples
/// per channel), the end inclusive.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Decoded {
    pub channels: u8,
    pub rate: u32,
    pub samples: Vec<i16>,
    pub looped: Option<(u32, u32)>,
    pub bad_frames: u32,
}

fn decode_frame(
    h: &Header,
    t: &Tables,
    channels: &mut [Channel],
    frame: &[u8],
    out: &mut [Vec<f32>],
    at: usize,
) -> Result<()> {
    let mut r = Bits {
        b: frame,
        bit: 0,
        limit: frame.len() * 8,
    };
    if r.read(16) != 0xffff {
        return Err(bad("HCA frame sync missing"));
    }
    let (noise, boundary) = (r.read(9) as i32, r.read(7) as i32);
    let packed = (noise << 8) - boundary;
    let groups = h.hfr_groups();
    for c in channels.iter_mut() {
        let n = c.coded;
        let delta = r.read(3) as usize;
        if delta >= 6 {
            for i in 0..n {
                c.scale_factors[i] = r.read(6) as u8;
            }
        } else if delta > 0 {
            let esc = (1i32 << delta) - 1;
            let mut v = r.read(6) as i32;
            c.scale_factors[0] = v as u8;
            for i in 1..n {
                let d = r.read(delta) as i32;
                if d == esc {
                    v = r.read(6) as i32;
                } else {
                    v = v - (esc >> 1) + d;
                    if !(0..=63).contains(&v) {
                        return Err(bad("HCA scale factor out of range"));
                    }
                }
                c.scale_factors[i] = v as u8;
            }
        } else {
            c.scale_factors = [0; 128];
        }
        if c.kind == 2 {
            let v = r.read(4) as u8;
            c.intensity[0] = v;
            if v < 15 {
                for i in 1..8 {
                    c.intensity[i] = r.read(4) as u8;
                }
            } else {
                r.bit -= 4;
            }
        } else {
            for g in 0..groups.min(128) {
                c.hfr[g] = r.read(6) as u8;
            }
        }
        for i in 0..n {
            let sf = i32::from(c.scale_factors[i]);
            let mut res = 0u8;
            if sf != 0 {
                let pos = ((packed + i as i32) >> 8) - ((sf * 5) >> 1) + 1;
                res = if pos < 0 {
                    15
                } else if pos < 57 {
                    t.invert[pos as usize]
                } else {
                    1
                };
            }
            c.resolution[i] = res;
            c.gain[i] = t.scale[sf as usize] * t.range[usize::from(res)];
        }
        for slot in c.resolution.iter_mut().skip(n) {
            *slot = 0;
        }
    }
    let (base, stereo, total) = (
        usize::from(h.base_bands),
        usize::from(h.stereo_bands),
        usize::from(h.total_bands),
    );
    for sub in 0..8 {
        for c in channels.iter_mut() {
            let n = c.coded;
            for i in 0..n {
                let res = c.resolution[i];
                let bits = usize::from(t.max_bits[usize::from(res)]);
                if bits == 0 {
                    c.spectra[i] = 0.;
                    continue;
                }
                let code = r.peek(bits);
                let q = if res < 8 {
                    let idx = (usize::from(res) << 4) + code as usize;
                    r.bit += usize::from(t.read_bits[idx]);
                    t.read_val[idx]
                } else {
                    let v = (1 - ((code as i32 & 1) << 1)) * (code as i32 >> 1);
                    r.bit += if v == 0 { bits - 1 } else { bits };
                    v as f32
                };
                c.spectra[i] = q * c.gain[i];
            }
            for slot in c.spectra.iter_mut().skip(n) {
                *slot = 0.;
            }
        }
        if h.bands_per_hfr > 0 {
            let start = base + stereo;
            for c in channels.iter_mut() {
                if c.kind == 2 {
                    continue;
                }
                let (mut high, mut low) = (start, start as isize - 1);
                'groups: for g in 0..groups.min(128) {
                    for _ in 0..h.bands_per_hfr {
                        if high >= total || low < 0 {
                            break 'groups;
                        }
                        let conv =
                            63 + i32::from(c.hfr[g]) - i32::from(c.scale_factors[low as usize]);
                        c.spectra[high] =
                            t.scale_conv[conv.clamp(0, 127) as usize] * c.spectra[low as usize];
                        high += 1;
                        low -= 1;
                    }
                }
                c.spectra[127] = 0.;
            }
        }
        if stereo > 0 {
            for i in 0..channels.len().saturating_sub(1) {
                if channels[i].kind != 1 {
                    continue;
                }
                let rl = t.intensity[usize::from(channels[i + 1].intensity[sub]).min(15)];
                let rr = 2. - rl;
                for k in base..total {
                    let v = channels[i].spectra[k];
                    channels[i].spectra[k] = v * rl;
                    channels[i + 1].spectra[k] = v * rr;
                }
            }
        }
        for (ci, c) in channels.iter_mut().enumerate() {
            let from = at + sub * 128;
            imdct(c, t, &mut out[ci][from..from + 128]);
        }
    }
    Ok(())
}

/// Decodes a stream: the HCA header and its frames, `key` the XOR the material
/// says (already undone when `None`).
pub fn decode(t: &Tables, h: &Header, frames: &[u8]) -> Result<Decoded> {
    if h.cipher != 0 {
        return Err(bad("encrypted HCA is not supported"));
    }
    if h.version > 0x200 {
        return Err(bad("unknown HCA version"));
    }
    if h.ath.unwrap_or(0) != 0 {
        return Err(bad("HCA ath tables are not supported"));
    }
    if h.ms_stereo != 0 {
        return Err(bad("HCA mid/side stereo is not supported"));
    }
    let kinds = channel_kinds(h);
    let mut channels: Vec<Channel> = kinds
        .iter()
        .map(|&kind| Channel {
            kind,
            coded: usize::from(h.base_bands)
                + if kind == 2 {
                    0
                } else {
                    usize::from(h.stereo_bands)
                },
            scale_factors: [0; 128],
            hfr: [0; 128],
            resolution: [0; 128],
            intensity: [0; 8],
            gain: [0.; 128],
            spectra: [0.; 128],
            temp: [0.; 128],
            prev: [0.; 128],
        })
        .collect();
    let size = usize::from(h.frame_size);
    let count = (h.frames as usize).min(frames.len() / size);
    let total = count * 1024;
    let mut out = vec![vec![0f32; total]; channels.len()];
    let mut bad_frames = 0;
    for i in 0..count {
        let frame = &frames[i * size..(i + 1) * size];
        if t.crc(frame) != 0
            || decode_frame(h, t, &mut channels, frame, &mut out, i * 1024).is_err()
        {
            bad_frames += 1;
        }
    }
    let start = usize::from(h.delay).min(total);
    let keep = (total - start).min(h.samples());
    let mut samples = Vec::with_capacity(keep * channels.len());
    for f in start..start + keep {
        for c in &out {
            samples.push((c[f] * 32768.).round().clamp(-32768., 32767.) as i16);
        }
    }
    let looped = h.looped.and_then(|(sf, ef, sd, ep)| {
        let s = (sf as i64 * 1024 + i64::from(sd) - i64::from(h.delay)).max(0);
        let e = (ef as i64 + 1) * 1024 - i64::from(ep) - i64::from(h.delay);
        (s < e).then_some((s as u32, (e - 1) as u32))
    });
    Ok(Decoded {
        channels: h.channels,
        rate: h.rate,
        samples,
        looped,
        bad_frames,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_crc_table_is_the_polynomial() {
        let t = crc_table();
        assert_eq!(t[1], 0x8005);
        assert_eq!(t[0], 0);
    }

    fn be16(v: u16) -> [u8; 2] {
        v.to_be_bytes()
    }

    #[test]
    fn a_header_is_read() {
        let mut b = Vec::new();
        b.extend_from_slice(b"HCA\0");
        b.extend_from_slice(&be16(0x200));
        b.extend_from_slice(&be16(96));
        b.extend_from_slice(b"fmt\0");
        b.push(2);
        b.extend_from_slice(&[0x00, 0xbb, 0x80]);
        b.extend_from_slice(&10u32.to_be_bytes());
        b.extend_from_slice(&be16(128));
        b.extend_from_slice(&be16(64));
        b.extend_from_slice(b"comp");
        b.extend_from_slice(&be16(256));
        b.extend_from_slice(&[1, 15, 1, 0, 128, 20, 30, 2, 0, 0]);
        b.extend_from_slice(b"pad\0");
        b.resize(96, 0);
        let h = Header::parse(&b).unwrap();
        assert_eq!(h.channels, 2);
        assert_eq!(h.rate, 48000);
        assert_eq!(h.frame_size, 256);
        assert_eq!(h.total_bands, 128);
        assert_eq!(h.samples(), 10 * 1024 - 192);
        // hfr groups: (128 - 20 - 30) / 2.
        assert_eq!(h.hfr_groups(), 39);
    }

    #[test]
    fn a_bad_tag_is_refused() {
        assert!(Header::parse(&[0; 32]).is_err());
    }

    #[test]
    fn stereo_pairs_are_typed() {
        let h = Header {
            channels: 2,
            tracks: 1,
            stereo_bands: 10,
            ..Header::default()
        };
        assert_eq!(channel_kinds(&h), [1, 2]);
        let h = Header {
            channels: 1,
            tracks: 1,
            ..Header::default()
        };
        assert_eq!(channel_kinds(&h), [0]);
    }

    #[test]
    fn bits_past_the_end_read_zero() {
        let mut r = Bits {
            b: &[0xff],
            bit: 4,
            limit: 8,
        };
        assert_eq!(r.read(8), 0xf0);
    }
}
