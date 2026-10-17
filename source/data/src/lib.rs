//! Bounded package inspection/decoding. Originals stay in memory; the probe can
//! write derived font previews only to guarded private destinations.
//! Layout reference: OpenKH Hed.cs, commit 7a3b945c538d32c6a285128c98aefba093f52ceb.

use serde::Serialize;
use std::{collections::HashSet, io};

pub mod ard;
pub mod bar;
pub mod battle_tables;
pub mod bdscript;
pub mod command_table;
pub mod enemy_actions;
pub mod event_script;
pub mod font;
pub mod font_cell;
pub mod hd_image;
pub mod image_structure;
pub mod inspection;
pub mod item_table;
pub mod journal_table;
pub mod menu_layout;
pub mod message;
pub mod message_text;
pub mod model_geometry;
pub mod model_inspection;
pub mod model_inspection_textures;
pub mod model_packets;
pub mod model_pose;
pub mod model_skeleton;
pub mod model_texture;
pub mod model_texture_animation;
pub mod model_texture_pixels;
pub mod model_texture_play;
pub mod model_texture_routing;
pub mod motion;
pub mod motion_eval;
pub mod motion_trigger;
pub mod name_hash;
pub mod object_collision;
pub mod object_effect;
pub mod objentry;
pub mod ogg;
pub mod original_payload;
pub mod party_records;
pub mod path_lookup;
pub mod pause_definition;
pub mod probe_source;
pub mod ptya;
pub mod raw_motion;
pub mod replacement_payload;
pub mod room_collision;
pub mod room_geometry;
pub mod scd;
pub mod sklt;
pub mod sprite_sequence;
pub mod unencrypted_payload;
pub mod vif;
pub mod vpu;
pub mod weapon_entry;
pub mod world_map;

pub const RECORD_SIZE: usize = 32;
pub const MAX_INDEX_BYTES: usize = 32 * 1024 * 1024;

#[derive(Debug, Serialize, PartialEq, Eq)]
pub struct HedEntry {
    pub name_hash: [u8; 16],
    pub package_offset: u64,
    pub stored_length: u32,
    pub original_length: u32,
}

#[derive(Debug, Serialize)]
pub struct IndexInventory {
    pub record_count: usize,
    pub duplicate_name_hashes: usize,
    pub unaligned_offsets: usize,
    pub overlapping_ranges: usize,
    pub name_resolution: String,
    pub entries: Vec<HedEntry>,
}

#[derive(Debug, Serialize, PartialEq, Eq)]
pub struct AssetHeader {
    pub decompressed_length: u32,
    pub remastered_asset_count: u32,
    pub compression_tag: i32,
    pub creation_date: i32,
}

/// Inspect the public package header structure; payloads remain untouched.
pub fn read_asset_header(bytes: &[u8; 16], stored_length: u32) -> io::Result<AssetHeader> {
    let decompressed = i32::from_le_bytes(bytes[0..4].try_into().unwrap());
    let count = i32::from_le_bytes(bytes[4..8].try_into().unwrap());
    let compression = i32::from_le_bytes(bytes[8..12].try_into().unwrap());
    let date = i32::from_le_bytes(bytes[12..16].try_into().unwrap());
    if decompressed < 0 || count < 0 || compression < -2 {
        return Err(invalid("invalid signed package header field"));
    }
    let table_end = 16u64 + count as u64 * 48;
    let original_stored = if compression >= 0 {
        compression as u64
    } else {
        decompressed as u64
    };
    if table_end + original_stored > stored_length as u64 {
        return Err(invalid(
            "package header/table/original payload exceed indexed range",
        ));
    }
    Ok(AssetHeader {
        decompressed_length: decompressed as u32,
        remastered_asset_count: count as u32,
        compression_tag: compression,
        creation_date: date,
    })
}

fn invalid(message: &str) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, message)
}

pub fn read_hed(bytes: &[u8], package_length: u64) -> io::Result<IndexInventory> {
    if bytes.is_empty() || bytes.len() > MAX_INDEX_BYTES || !bytes.len().is_multiple_of(RECORD_SIZE)
    {
        return Err(invalid(
            "HED must contain complete 32-byte records within the 32 MiB inspection limit",
        ));
    }
    let mut entries = Vec::with_capacity(bytes.len() / RECORD_SIZE);
    let mut seen = HashSet::new();
    let mut duplicates = 0;
    let mut unaligned = 0;
    for record in bytes.as_chunks::<RECORD_SIZE>().0 {
        let name_hash = record[..16].try_into().unwrap();
        let offset = i64::from_le_bytes(record[16..24].try_into().unwrap());
        let stored = i32::from_le_bytes(record[24..28].try_into().unwrap());
        let original = i32::from_le_bytes(record[28..32].try_into().unwrap());
        if offset < 0 || stored < 0 || original < 0 {
            return Err(invalid("negative HED offset or length"));
        }
        let offset = offset as u64;
        if offset
            .checked_add(stored as u64)
            .is_none_or(|end| end > package_length)
        {
            return Err(invalid("HED range exceeds the paired package"));
        }
        if !seen.insert(name_hash) {
            duplicates += 1;
        }
        if !offset.is_multiple_of(16) {
            unaligned += 1;
        }
        entries.push(HedEntry {
            name_hash,
            package_offset: offset,
            stored_length: stored as u32,
            original_length: original as u32,
        });
    }
    let mut ranges: Vec<_> = entries
        .iter()
        .filter(|e| e.stored_length != 0)
        .map(|e| (e.package_offset, e.package_offset + e.stored_length as u64))
        .collect();
    ranges.sort_unstable();
    let mut max_end = 0;
    let mut overlaps = 0;
    for (start, end) in ranges {
        if start < max_end {
            overlaps += 1;
        }
        max_end = max_end.max(end);
    }
    Ok(IndexInventory {
        record_count: entries.len(),
        duplicate_name_hashes: duplicates,
        unaligned_offsets: unaligned,
        overlapping_ranges: overlaps,
        name_resolution: "all name hashes retained as unknown; no filename dictionary applied"
            .into(),
        entries,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    fn record(offset: i64, stored: i32, original: i32) -> Vec<u8> {
        let mut bytes = vec![0x42; 16];
        bytes.extend(offset.to_le_bytes());
        bytes.extend(stored.to_le_bytes());
        bytes.extend(original.to_le_bytes());
        bytes
    }
    #[test]
    fn retains_order_unknown_names_duplicates_alignment_and_overlap() {
        let mut bytes = record(32, 64, 100);
        bytes.extend(record(1, 32, 200));
        let result = read_hed(&bytes, 96).unwrap();
        assert_eq!(result.record_count, 2);
        assert_eq!(result.duplicate_name_hashes, 1);
        assert_eq!(result.unaligned_offsets, 1);
        assert_eq!(result.overlapping_ranges, 1);
        assert_eq!(result.entries[0].package_offset, 32);
        assert_eq!(result.entries[1].original_length, 200);
    }
    #[test]
    fn rejects_truncation_negative_fields_and_out_of_bounds() {
        assert!(read_hed(&[], 100).is_err());
        assert!(read_hed(&[0; 31], 100).is_err());
        for (offset, stored, original) in [
            (-1, 1, 1),
            (0, -1, 1),
            (0, 1, -1),
            (99, 2, 1),
            (i64::MAX, 1, 1),
        ] {
            assert!(read_hed(&record(offset, stored, original), 100).is_err());
        }
        assert!(read_hed(&record(100, 0, 0), 100).is_ok());
    }
    #[test]
    fn rejects_oversized_index_before_allocating_records() {
        assert!(read_hed(&vec![0; MAX_INDEX_BYTES + 32], u64::MAX).is_err());
    }

    #[test]
    fn checks_package_header_table_payload_and_tags() {
        let make = |size: i32, count: i32, tag: i32| {
            let mut bytes = [0u8; 16];
            bytes[0..4].copy_from_slice(&size.to_le_bytes());
            bytes[4..8].copy_from_slice(&count.to_le_bytes());
            bytes[8..12].copy_from_slice(&tag.to_le_bytes());
            bytes
        };
        assert_eq!(
            read_asset_header(&make(10, 1, -2), 74)
                .unwrap()
                .remastered_asset_count,
            1
        );
        assert!(read_asset_header(&make(10, 1, -2), 73).is_err());
        assert!(read_asset_header(&make(100, 0, 2), 18).is_ok());
        for (size, count, tag) in [(-1, 0, -2), (1, -1, -2), (1, 0, -3), (1, i32::MAX, -2)] {
            assert!(read_asset_header(&make(size, count, tag), u32::MAX).is_err());
        }
    }
}
