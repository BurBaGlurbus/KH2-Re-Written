//! Bounded replacement payloads, located by pinned OpenKH sequential alignment.
//! Declared offsets remain opaque; this is a compatibility sample, not a name map.
use crate::{
    HedEntry,
    original_payload::{MAX_ORIGINAL_BYTES, decode_payload},
    read_asset_header,
};
use serde::Serialize;
use std::io::{self, Read, Seek, SeekFrom};

#[derive(Debug, Clone, Serialize)]
pub struct ReplacementMetadata {
    pub ordinal: u32,
    pub name_bytes: [u8; 32],
    pub declared_offset: i32,
    pub original_asset_offset: i32,
    pub declared_decompressed_length: u32,
    pub compression_tag: i32,
    pub package_offset: u64,
    pub padded_stored_length: u64,
    pub compressed_bytes_consumed: Option<u64>,
    pub uncompressed_padding_retained: bool,
}

pub struct ReplacementSample {
    pub metadata: ReplacementMetadata,
    pub bytes: Vec<u8>,
}

pub fn read_first_replacements<R: Read + Seek>(
    reader: &mut R,
    entry: &HedEntry,
) -> io::Result<Vec<ReplacementSample>> {
    read_replacements(reader, entry, None)
}

/// Read every declared replacement, or reject the container before returning a
/// partial list. Limits apply to count and cumulative padded/output allocation.
pub fn read_all_replacements<R: Read + Seek>(
    reader: &mut R,
    entry: &HedEntry,
) -> io::Result<Vec<ReplacementSample>> {
    read_replacements(reader, entry, Some(Limits::INSPECTION))
}

/// `read_all_replacements` for a field map, whose textures and effects carry
/// more remaster images than a model (hundreds, not 64) under larger, still
/// finite bounds.
pub fn read_map_replacements<R: Read + Seek>(
    reader: &mut R,
    entry: &HedEntry,
) -> io::Result<Vec<ReplacementSample>> {
    read_replacements(reader, entry, Some(Limits::MAP))
}

/// Count and cumulative allocation bounds of a complete read.
#[derive(Clone, Copy)]
struct Limits {
    count: u32,
    bytes: u64,
}

impl Limits {
    const INSPECTION: Self = Self {
        count: 64,
        bytes: 128 * 1024 * 1024,
    };
    const MAP: Self = Self {
        count: 1024,
        bytes: 1024 * 1024 * 1024,
    };
}

fn read_replacements<R: Read + Seek>(
    reader: &mut R,
    entry: &HedEntry,
    all: Option<Limits>,
) -> io::Result<Vec<ReplacementSample>> {
    let limits = all.unwrap_or(Limits::INSPECTION);
    let invalid = |message| io::Error::new(io::ErrorKind::InvalidData, message);
    let package_end = reader.seek(SeekFrom::End(0))?;
    let indexed_end = entry
        .package_offset
        .checked_add(entry.stored_length as u64)
        .filter(|end| *end <= package_end)
        .ok_or_else(|| invalid("replacement indexed range exceeds package"))?;
    if entry.stored_length < 16 {
        return Err(invalid("replacement missing original header"));
    }
    reader.seek(SeekFrom::Start(entry.package_offset))?;
    let mut seed = [0; 16];
    reader.read_exact(&mut seed)?;
    let header = read_asset_header(&seed, entry.stored_length)?;
    if all.is_some() && header.remastered_asset_count > limits.count {
        return Err(invalid("replacement count exceeds the entry limit"));
    }
    let original_stored = if header.compression_tag >= 0 {
        header.compression_tag as u64
    } else {
        header.decompressed_length as u64
    };
    let mut payload_start =
        entry.package_offset + 16 + header.remastered_asset_count as u64 * 48 + original_stored;
    let mut samples = Vec::new();
    let count = if all.is_some() {
        header.remastered_asset_count
    } else {
        header.remastered_asset_count.min(2)
    };
    let mut total_allocation = 0u64;
    for ordinal in 0..count {
        reader.seek(SeekFrom::Start(
            entry.package_offset + 16 + ordinal as u64 * 48,
        ))?;
        let mut record = [0; 48];
        reader.read_exact(&mut record)?;
        let expected = i32::from_le_bytes(record[40..44].try_into().unwrap());
        let tag = i32::from_le_bytes(record[44..48].try_into().unwrap());
        if expected < 0 || tag < -2 {
            return Err(invalid("invalid replacement size/tag"));
        }
        let stored = if tag >= 0 {
            tag as u64
        } else {
            expected as u64
        };
        let padded = stored.div_ceil(16) * 16;
        if padded > MAX_ORIGINAL_BYTES as u64 || expected as u32 > MAX_ORIGINAL_BYTES {
            return Err(invalid("replacement exceeds 32 MiB inspection limit"));
        }
        // Include both input and output for compressed data, even though the
        // input can be released between entries. Plain output retains padding.
        total_allocation += padded + if tag >= 0 { expected as u64 } else { 0 };
        if total_allocation > limits.bytes {
            return Err(invalid("replacement allocation exceeds the limit"));
        }
        payload_start = payload_start
            .checked_add(15)
            .ok_or_else(|| invalid("replacement alignment overflow"))?
            / 16
            * 16;
        let payload_end = payload_start
            .checked_add(padded)
            .filter(|end| *end <= indexed_end)
            .ok_or_else(|| invalid("replacement payload exceeds indexed range"))?;
        reader.seek(SeekFrom::Start(payload_start))?;
        let mut bytes = vec![0; padded as usize];
        reader.read_exact(&mut bytes)?;
        let (bytes, consumed) = decode_payload(&seed, tag, expected as u32, bytes)?;
        samples.push(ReplacementSample {
            metadata: ReplacementMetadata {
                ordinal,
                name_bytes: record[..32].try_into().unwrap(),
                declared_offset: i32::from_le_bytes(record[32..36].try_into().unwrap()),
                original_asset_offset: i32::from_le_bytes(record[36..40].try_into().unwrap()),
                declared_decompressed_length: expected as u32,
                compression_tag: tag,
                package_offset: payload_start,
                padded_stored_length: padded,
                compressed_bytes_consumed: consumed,
                uncompressed_padding_retained: tag < 0,
            },
            bytes,
        });
        payload_start = payload_end;
    }
    Ok(samples)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::original_payload::transform_prefix;
    use flate2::{Compression, write::ZlibEncoder};
    use std::io::{Cursor, Write};

    fn fixture() -> (Cursor<Vec<u8>>, HedEntry) {
        let mut seed = [0; 16];
        seed[..4].copy_from_slice(&3i32.to_le_bytes());
        seed[4..8].copy_from_slice(&2i32.to_le_bytes());
        seed[8..12].copy_from_slice(&(-2i32).to_le_bytes());
        let mut encoder = ZlibEncoder::new(Vec::new(), Compression::default());
        encoder.write_all(b"synthetic replacement").unwrap();
        let mut compressed = encoder.finish().unwrap();
        let compressed_size = compressed.len() as i32;
        compressed.resize(compressed.len().div_ceil(16) * 16, 0);
        transform_prefix(&seed, &mut compressed);
        let mut bytes = seed.to_vec();
        for (expected, tag) in [(3i32, -2i32), (21, compressed_size)] {
            let mut record = [0; 48];
            record[..4].copy_from_slice(b"name");
            record[32..36].copy_from_slice(&(-123i32).to_le_bytes());
            record[40..44].copy_from_slice(&expected.to_le_bytes());
            record[44..48].copy_from_slice(&tag.to_le_bytes());
            bytes.extend(record);
        }
        bytes.extend(b"src");
        bytes.resize(128, 0);
        bytes.extend(b"abc");
        bytes.extend([0x42; 13]);
        bytes.extend(compressed);
        let entry = HedEntry {
            name_hash: [0; 16],
            package_offset: 0,
            stored_length: bytes.len() as u32,
            original_length: 3,
        };
        (Cursor::new(bytes), entry)
    }

    #[test]
    fn locates_aligned_samples_preserves_offsets_and_plain_padding() {
        let (mut reader, entry) = fixture();
        let samples = read_first_replacements(&mut reader, &entry).unwrap();
        assert_eq!(samples.len(), 2);
        assert_eq!(samples[0].metadata.package_offset, 128);
        assert_eq!(samples[0].metadata.declared_offset, -123);
        assert_eq!(samples[0].bytes.len(), 16);
        assert_eq!(&samples[0].bytes[..3], b"abc");
        assert_eq!(&samples[0].bytes[3..], &[0x42; 13]);
        assert!(samples[0].metadata.uncompressed_padding_retained);
        assert_eq!(samples[1].metadata.package_offset, 144);
        assert_eq!(samples[1].bytes, b"synthetic replacement");
    }

    #[test]
    fn rejects_bad_tags_lengths_and_truncated_aligned_storage() {
        for (field, value) in [(40, -1i32), (44, -3), (40, i32::MAX)] {
            let (mut reader, entry) = fixture();
            reader.get_mut()[16 + field..20 + field].copy_from_slice(&value.to_le_bytes());
            assert!(read_first_replacements(&mut reader, &entry).is_err());
        }
        let (mut reader, mut entry) = fixture();
        entry.stored_length = 130;
        assert!(read_first_replacements(&mut reader, &entry).is_err());
    }

    fn plain_fixture(count: u32) -> (Cursor<Vec<u8>>, HedEntry) {
        let mut bytes = vec![0; 16 + count as usize * 48];
        bytes[..4].copy_from_slice(&3i32.to_le_bytes());
        bytes[4..8].copy_from_slice(&count.to_le_bytes());
        bytes[8..12].copy_from_slice(&(-2i32).to_le_bytes());
        for ordinal in 0..count as usize {
            let start = 16 + ordinal * 48;
            bytes[start + 32..start + 36].copy_from_slice(&(-99i32).to_le_bytes());
            bytes[start + 40..start + 44].copy_from_slice(&3i32.to_le_bytes());
            bytes[start + 44..start + 48].copy_from_slice(&(-2i32).to_le_bytes());
        }
        bytes.extend(b"src");
        bytes.resize(bytes.len().div_ceil(16) * 16, 0);
        for ordinal in 0..count {
            bytes.extend([ordinal as u8; 16]);
        }
        let entry = HedEntry {
            name_hash: [0; 16],
            package_offset: 0,
            stored_length: bytes.len() as u32,
            original_length: 3,
        };
        (Cursor::new(bytes), entry)
    }

    #[test]
    fn complete_reader_includes_third_entry_and_rejects_invalid_tail() {
        let (mut reader, mut entry) = plain_fixture(3);
        assert_eq!(
            read_first_replacements(&mut reader, &entry).unwrap().len(),
            2
        );
        let all = read_all_replacements(&mut reader, &entry).unwrap();
        assert_eq!(all.len(), 3);
        assert_eq!(all[2].metadata.ordinal, 2);
        assert_eq!(all[2].metadata.declared_offset, -99);
        assert_eq!(all[2].metadata.package_offset, 208);
        assert_eq!(all[2].bytes, [2; 16]);
        entry.stored_length -= 1;
        assert!(read_all_replacements(&mut reader, &entry).is_err());
        assert_eq!(
            read_first_replacements(&mut reader, &entry).unwrap().len(),
            2
        );
        let (mut reader, entry) = plain_fixture(3);
        reader.get_mut()[16 + 2 * 48 + 44..16 + 3 * 48].copy_from_slice(&(-3i32).to_le_bytes());
        assert!(read_all_replacements(&mut reader, &entry).is_err());
        assert!(read_first_replacements(&mut reader, &entry).is_ok());
    }

    #[test]
    fn complete_reader_count_boundary_and_legacy_storage_agree() {
        for count in [0, 1, 64] {
            let (mut reader, entry) = plain_fixture(count);
            assert_eq!(
                read_all_replacements(&mut reader, &entry).unwrap().len(),
                count as usize
            );
        }
        let (mut reader, entry) = plain_fixture(65);
        assert!(read_all_replacements(&mut reader, &entry).is_err());
        assert_eq!(
            read_map_replacements(&mut reader, &entry).unwrap().len(),
            65
        );
        assert_eq!(
            read_first_replacements(&mut reader, &entry).unwrap().len(),
            2
        );
        let (mut reader, entry) = fixture();
        let all = read_all_replacements(&mut reader, &entry).unwrap();
        assert_eq!(all[0].bytes, [b"abc".as_slice(), &[0x42; 13]].concat());
        assert_eq!(all[1].bytes, b"synthetic replacement");
    }
}
