//! Type-one skeletal motion records. Layout follows pinned OpenKH Motion.cs;
//! playback is an explicit stored-matrix inspection profile, not native timing.
use serde::Serialize;
use sha2::{Digest, Sha256};
use std::io;
pub type Matrix = [[f32; 4]; 4];
pub type Sample = (Vec<Matrix>, [f32; 3]);
pub const PROFILE: &str = "raw_stored_matrix_floor_inspection_v2";

#[derive(Clone, Debug, Serialize)]
pub struct RawMotion {
    pub header_words: [u32; 4],
    pub raw_header_words: [u32; 20],
    pub bone_count: usize,
    pub frame_count: usize,
    pub total_frame_count: usize,
    /// Frame-major, then bone-major, sixteen row-vector matrix words.
    pub animation_matrix_bits: Vec<[u32; 16]>,
    pub position_matrix_bits: Vec<[u32; 16]>,
    pub extra_sha256: String,
    pub extra_size: usize,
}

fn invalid(message: &str) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, message)
}
fn range(body: &[u8], offset: u32, length: usize) -> io::Result<&[u8]> {
    if offset > i32::MAX as u32 {
        return Err(invalid("negative raw motion offset"));
    }
    body.get(
        offset as usize
            ..(offset as usize)
                .checked_add(length)
                .ok_or_else(|| invalid("raw range overflow"))?,
    )
    .ok_or_else(|| invalid("raw motion range outside payload"))
}
fn words<const N: usize>(bytes: &[u8]) -> [u32; N] {
    std::array::from_fn(|i| u32::from_le_bytes(bytes[i * 4..i * 4 + 4].try_into().unwrap()))
}
pub fn read(bytes: &[u8]) -> io::Result<RawMotion> {
    if bytes.len() > 32 * 1024 * 1024 {
        return Err(invalid("raw motion exceeds 32 MiB"));
    }
    let body = bytes
        .get(0x90..)
        .ok_or_else(|| invalid("raw motion reserved area absent"))?;
    let header = words::<4>(range(body, 0, 16)?);
    if header[0] != 1 {
        return Err(invalid("not a type-one raw motion"));
    }
    let raw = words::<20>(range(body, 16, 80)?);
    let (bones, frames, total) = (raw[0] as usize, raw[4] as usize, raw[5] as usize);
    if bones == 0 || bones > 2048 || frames > 262144 || total == 0 || total > 262144 {
        return Err(invalid("raw motion counts outside inspection bounds"));
    }
    let matrix_count = bones
        .checked_mul(total)
        .ok_or_else(|| invalid("raw matrix count overflow"))?;
    let animation_length = matrix_count
        .checked_mul(64)
        .filter(|&n| n <= 32 * 1024 * 1024)
        .ok_or_else(|| invalid("raw matrix table exceeds bound"))?;
    if raw[6] < 96 {
        return Err(invalid("raw matrix table overlaps header"));
    }
    let animation_end = (raw[6] as usize)
        .checked_add(animation_length)
        .ok_or_else(|| invalid("raw matrix extent overflow"))?;
    if animation_end > header[2] as usize {
        return Err(invalid("raw matrix table overlaps extra"));
    }
    let animation = range(body, raw[6], animation_length)?;
    let positions = if raw[7] == 0 {
        &[][..]
    } else {
        if (raw[7] as usize) < animation_end {
            return Err(invalid("raw position table overlaps animation"));
        }
        let length = header[2]
            .checked_sub(raw[7])
            .ok_or_else(|| invalid("raw position table order invalid"))?
            as usize;
        if !length.is_multiple_of(64) {
            return Err(invalid("raw position table matrix stride invalid"));
        }
        range(body, raw[7], length)?
    };
    let extra = range(body, header[2], header[3] as usize)?;
    Ok(RawMotion {
        header_words: header,
        raw_header_words: raw,
        bone_count: bones,
        frame_count: frames,
        total_frame_count: total,
        animation_matrix_bits: animation
            .as_chunks::<64>()
            .0
            .iter()
            .map(|b| words::<16>(b))
            .collect(),
        position_matrix_bits: positions
            .as_chunks::<64>()
            .0
            .iter()
            .map(|b| words::<16>(b))
            .collect(),
        extra_sha256: format!("{:x}", Sha256::digest(extra)),
        extra_size: extra.len(),
    })
}
impl RawMotion {
    /// A one-frame table of the given model-space matrices (row-vector words).
    /// It is no source record: it holds the bind pose of an object whose
    /// objentry row names no motion set.
    pub fn held(matrices: Vec<[u32; 16]>) -> Self {
        let mut raw_header_words = [0; 20];
        raw_header_words[4] = 1;
        raw_header_words[17] = 1f32.to_bits();
        raw_header_words[18] = 60f32.to_bits();
        RawMotion {
            header_words: [0; 4],
            raw_header_words,
            bone_count: matrices.len(),
            frame_count: 1,
            total_frame_count: 1,
            animation_matrix_bits: matrices,
            position_matrix_bits: Vec::new(),
            extra_sha256: String::new(),
            extra_size: 0,
        }
    }

    pub fn frame_window(&self) -> io::Result<(f32, f32, f32, f32)> {
        let values: [f32; 4] =
            std::array::from_fn(|i| f32::from_bits(self.raw_header_words[16 + i]));
        if values.iter().any(|v| !v.is_finite()) {
            return Err(invalid("nonfinite raw frame window"));
        }
        Ok((values[0], values[1], values[2], values[3]))
    }
    /// Preserve a stored pose exactly. Fractional source frame indices hold the
    /// preceding matrix sample, clamped at the table boundaries. No rotation
    /// interpolation or installed clock/root semantics are inferred.
    pub fn sample(&self, frame: f32) -> io::Result<Sample> {
        if !frame.is_finite() {
            return Err(invalid("nonfinite raw sample index"));
        }
        let (start, end, _, _) = self.frame_window()?;
        let last = self
            .total_frame_count
            .checked_sub(1)
            .ok_or_else(|| invalid("raw sample table empty"))?;
        // Source tables use both inclusive and one-past-the-last endpoints.
        // Keep the declared window; this inspection profile holds the final
        // stored pose at an exclusive end instead of inventing an extra pose.
        if end - start != last as f32 && end - start != self.total_frame_count as f32 {
            return Err(invalid("raw timeline does not span stored matrix frames"));
        }
        let index = (frame - start).floor().clamp(0., last as f32) as usize;
        let from = index
            .checked_mul(self.bone_count)
            .ok_or_else(|| invalid("raw sample extent overflow"))?;
        let to = from
            .checked_add(self.bone_count)
            .ok_or_else(|| invalid("raw sample extent overflow"))?;
        let matrices: Vec<_> = self
            .animation_matrix_bits
            .get(from..to)
            .ok_or_else(|| invalid("raw animation sample absent"))?
            .iter()
            .map(|m| {
                std::array::from_fn(|row| {
                    std::array::from_fn(|col| f32::from_bits(m[row * 4 + col]))
                })
            })
            .collect();
        let root = if self.position_matrix_bits.is_empty() {
            [0.; 3]
        } else {
            let m = self
                .position_matrix_bits
                .get(index)
                .ok_or_else(|| invalid("raw position sample absent"))?;
            [
                f32::from_bits(m[12]),
                f32::from_bits(m[13]),
                f32::from_bits(m[14]),
            ]
        };
        if matrices
            .iter()
            .flatten()
            .flatten()
            .chain(root.iter())
            .any(|v| !v.is_finite())
        {
            return Err(invalid("nonfinite raw matrix sample"));
        }
        Ok((matrices, root))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn fixture() -> Vec<u8> {
        let mut bytes = vec![0; 0x90 + 352];
        let body = &mut bytes[0x90..];
        body[..4].copy_from_slice(&1u32.to_le_bytes());
        body[8..12].copy_from_slice(&352u32.to_le_bytes());
        for (word, value) in [
            (0, 1u32),
            (4, 2),
            (5, 2),
            (6, 96),
            (7, 224),
            (17, 1f32.to_bits()),
            (18, 30f32.to_bits()),
        ] {
            body[16 + word * 4..20 + word * 4].copy_from_slice(&value.to_le_bytes());
        }
        for (i, x) in [2f32, 9., 10., 20.].into_iter().enumerate() {
            let at = 96 + i * 64;
            for (word, value) in [(0, 1f32), (5, 1.), (10, 1.), (15, 1.), (12, x)] {
                body[at + word * 4..at + word * 4 + 4].copy_from_slice(&value.to_le_bytes());
            }
        }
        bytes
    }
    #[test]
    fn stored_pose_and_position_words_survive_and_sample_discretely() {
        let m = read(&fixture()).unwrap();
        assert_eq!(m.frame_window().unwrap(), (0., 1., 30., 0.));
        assert_eq!(m.sample(0.75).unwrap().0[0][3][0], 2.);
        assert_eq!(m.sample(1.).unwrap().0[0][3][0], 9.);
        assert_eq!(m.sample(999.).unwrap().1, [20., 0., 0.]);
        assert_eq!(m.sample(-99.).unwrap().1, [10., 0., 0.]);
        assert!(m.sample(f32::NAN).is_err());
    }
    #[test]
    fn rejects_counts_overlap_stride_and_truncation() {
        let good = fixture();
        for length in [0, 0x90, 0xe0, good.len() - 1] {
            assert!(read(&good[..length]).is_err());
        }
        for (at, value) in [
            (0xa0, 0u32),
            (0xb4, 0),
            (0xb8, 80),
            (0xbc, 223),
            (0x98, 351),
        ] {
            let mut b = good.clone();
            b[at..at + 4].copy_from_slice(&value.to_le_bytes());
            assert!(read(&b).is_err());
        }
        let mut b = good;
        b[0x90 + 96..0x90 + 100].copy_from_slice(&f32::INFINITY.to_bits().to_le_bytes());
        assert!(read(&b).unwrap().sample(0.).is_err());
    }
    #[test]
    fn zero_position_offset_means_no_position_matrices() {
        let mut b = fixture();
        b[0xbc..0xc0].fill(0);
        let m = read(&b).unwrap();
        assert!(m.position_matrix_bits.is_empty());
        assert_eq!(m.sample(0.).unwrap().1, [0.; 3]);
    }
    #[test]
    fn nonzero_declared_start_anchors_the_first_stored_matrix() {
        let mut b = fixture();
        b[0xe0..0xe4].copy_from_slice(&10f32.to_bits().to_le_bytes());
        b[0xe4..0xe8].copy_from_slice(&11f32.to_bits().to_le_bytes());
        let m = read(&b).unwrap();
        assert_eq!(m.sample(10.).unwrap().0[0][3][0], 2.);
        assert_eq!(m.sample(10.99).unwrap().0[0][3][0], 2.);
        assert_eq!(m.sample(11.).unwrap().0[0][3][0], 9.);
    }
    #[test]
    fn exclusive_declared_end_holds_the_final_stored_matrix() {
        let mut b = fixture();
        b[0xe0..0xe4].copy_from_slice(&10f32.to_bits().to_le_bytes());
        b[0xe4..0xe8].copy_from_slice(&12f32.to_bits().to_le_bytes());
        let m = read(&b).unwrap();
        assert_eq!(m.sample(10.).unwrap().0[0][3][0], 2.);
        assert_eq!(m.sample(11.5).unwrap().0[0][3][0], 9.);
        assert_eq!(m.sample(12.).unwrap().0[0][3][0], 9.);
        assert_eq!(m.sample(12.).unwrap().1, [20., 0., 0.]);
    }

    #[test]
    fn unknown_timeline_relationship_remains_an_inspection_failure() {
        let mut b = fixture();
        b[0xe4..0xe8].copy_from_slice(&3f32.to_bits().to_le_bytes());
        let m = read(&b).unwrap();
        assert_eq!(m.frame_window().unwrap().1, 3.);
        assert!(m.sample(0.).is_err());
    }
}
