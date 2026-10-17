//! Explicit bounded public texture-link candidates, not installed routing.
//! EgsTools.RAW formulas and TextureAnimation header at the NOTICE pin;
//! only bounded footer records are considered, never whole-payload tag scans.
use crate::{model_geometry::Geometry, model_texture, replacement_payload::ReplacementMetadata};
use serde::Serialize;
use sha2::{Digest, Sha256};
use std::{collections::HashSet, io};
pub const PROFILE: &str = "openkh_egs_raw_bounded_footer_links_v1";
fn invalid(message: &str) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, message)
}
#[derive(Debug, Serialize)]
pub struct TexaHeader {
    pub footer_ordinal: usize,
    pub header_u16: [u16; 12],
    pub header_i32: [i32; 4],
    pub sprite_data_offset: usize,
    pub sprite_data_length: usize,
    pub sprite_data_sha256: String,
}
#[derive(Debug, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum TargetKind {
    Base,
    Texa { footer_ordinal: usize },
}
#[derive(Debug, Serialize)]
pub struct Target {
    #[serde(flatten)]
    pub kind: TargetKind,
    pub environment_ordinal: usize,
    pub candidate_original_asset_offset: u64,
    pub matching_replacement_ordinals: Vec<u32>,
}
#[derive(Debug, Serialize)]
pub struct TextureLinks {
    pub profile: &'static str,
    pub texa_headers: Vec<TexaHeader>,
    pub ordered_targets: Vec<Target>,
    pub unmatched_replacement_ordinals: Vec<u32>,
    pub duplicate_candidate_offsets: Vec<u64>,
    pub runtime_verified: bool,
}
fn matching(offset: u64, replacements: &[ReplacementMetadata]) -> Vec<u32> {
    replacements
        .iter()
        .filter(|r| i64::from(r.original_asset_offset) == offset as i64)
        .map(|r| r.ordinal)
        .collect()
}
pub fn read_links(
    bytes: &[u8],
    asset_offset: usize,
    replacements: &[ReplacementMetadata],
) -> io::Result<TextureLinks> {
    if asset_offset
        .checked_add(bytes.len())
        .is_none_or(|end| end > 32 * 1024 * 1024)
        || replacements.len() > 64
    {
        return Err(invalid("texture link source/replacement bounds"));
    }
    let mut ordinals = HashSet::new();
    if replacements.iter().any(|r| !ordinals.insert(r.ordinal)) {
        return Err(invalid("duplicate replacement ordinal"));
    }
    let layout = model_texture::read_layout(bytes)?;
    let mut result = TextureLinks {
        profile: PROFILE,
        texa_headers: Vec::new(),
        ordered_targets: Vec::new(),
        unmatched_replacement_ordinals: Vec::new(),
        duplicate_candidate_offsets: Vec::new(),
        runtime_verified: false,
    };
    let mut animations = Vec::new();
    for (footer_ordinal, chunk) in layout
        .footer
        .iter()
        .enumerate()
        .filter(|(_, c)| &c.tag == b"TEXA")
    {
        let start = chunk.offset + 8;
        let body = &bytes[start..start + chunk.length];
        let header = body.get(..40).ok_or_else(|| invalid("short TEXA header"))?;
        let u16s: [u16; 12] = std::array::from_fn(|i| {
            u16::from_le_bytes(header[i * 2..i * 2 + 2].try_into().unwrap())
        });
        let i32s: [i32; 4] = std::array::from_fn(|i| {
            i32::from_le_bytes(header[24 + i * 4..28 + i * 4].try_into().unwrap())
        });
        let environment_ordinal = u16s[1] as usize;
        if environment_ordinal >= layout.images.len() {
            return Err(invalid("TEXA texture environment outside layout"));
        }
        let stride = match u16s[3] {
            4 => (u16s[10] as usize).div_ceil(2),
            8 => u16s[10] as usize,
            _ => return Err(invalid("TEXA sprite depth outside indexed profile")),
        };
        let length = stride
            .checked_mul(u16s[11] as usize)
            .and_then(|v| v.checked_mul(u16s[7] as usize))
            .filter(|&v| v <= 32 * 1024 * 1024)
            .ok_or_else(|| invalid("TEXA sprite size exceeds bound"))?;
        let offset =
            usize::try_from(i32s[2]).map_err(|_| invalid("negative TEXA sprite pointer"))?;
        let end = offset
            .checked_add(length)
            .ok_or_else(|| invalid("TEXA sprite range overflow"))?;
        let sprite = body
            .get(offset..end)
            .ok_or_else(|| invalid("TEXA sprite exceeds its footer chunk"))?;
        let candidate =
            (asset_offset + start + offset + environment_ordinal * 16) as u64 + 0x2000_0000;
        animations.push(Target {
            kind: TargetKind::Texa { footer_ordinal },
            environment_ordinal,
            candidate_original_asset_offset: candidate,
            matching_replacement_ordinals: matching(candidate, replacements),
        });
        result.texa_headers.push(TexaHeader {
            footer_ordinal,
            header_u16: u16s,
            header_i32: i32s,
            sprite_data_offset: start + offset,
            sprite_data_length: length,
            sprite_data_sha256: format!("{:x}", Sha256::digest(sprite)),
        });
    }
    let modifier = layout
        .images
        .iter()
        .filter(|i| i.indexed_bits_per_pixel != 4)
        .count();
    let (mut pxl4, mut pxl8) = (0usize, 0usize);
    for image in layout.images {
        let adjustment = if image.indexed_bits_per_pixel == 4 {
            let value = pxl4 + modifier * 16;
            pxl4 += 1;
            value
        } else {
            let value = pxl8 * 16;
            pxl8 += 1;
            value
        };
        let candidate = (asset_offset + image.data_offset + adjustment) as u64 + 0x2000_0000;
        result.ordered_targets.push(Target {
            kind: TargetKind::Base,
            environment_ordinal: image.environment_ordinal,
            candidate_original_asset_offset: candidate,
            matching_replacement_ordinals: matching(candidate, replacements),
        });
        let mut remaining = Vec::new();
        for animation in animations {
            if animation.environment_ordinal == image.environment_ordinal {
                result.ordered_targets.push(animation);
            } else {
                remaining.push(animation);
            }
        }
        animations = remaining;
    }
    let mut seen = HashSet::new();
    let mut duplicate_seen = HashSet::new();
    let mut matched = HashSet::<u32>::new();
    for target in &result.ordered_targets {
        if !seen.insert(target.candidate_original_asset_offset)
            && duplicate_seen.insert(target.candidate_original_asset_offset)
        {
            result
                .duplicate_candidate_offsets
                .push(target.candidate_original_asset_offset);
        }
        matched.extend(target.matching_replacement_ordinals.iter().copied());
    }
    result.unmatched_replacement_ordinals = replacements
        .iter()
        .filter(|r| !matched.contains(&r.ordinal))
        .map(|r| r.ordinal)
        .collect();
    Ok(result)
}
#[derive(Debug, Serialize)]
pub struct PartTextureRoute {
    pub part_ordinal: usize,
    pub raw_texture_index: i32,
    pub environment_ordinal: usize,
    pub expanded_uv_bits: Vec<[u32; 2]>,
    pub cross_part_uv_corners: usize,
}
/// Global UV references remain global; high texture-index bits remain raw.
/// Out-of-range indices reject, never inherit a previous public draw's texture.
pub fn associate_geometry(
    geometry: &Geometry,
    environment_count: usize,
) -> io::Result<Vec<PartTextureRoute>> {
    if environment_count == 0 || environment_count > 4096 {
        return Err(invalid("texture environment count outside profile"));
    }
    let mut uv_pool = Vec::new();
    for part in &geometry.parts {
        if part.uv_base != uv_pool.len()
            || uv_pool.len() + part.uv_bits.len() > 262_144
            || part
                .uv_bits
                .iter()
                .flatten()
                .any(|&v| !f32::from_bits(v).is_finite())
        {
            return Err(invalid("invalid global UV pool"));
        }
        uv_pool.extend_from_slice(&part.uv_bits);
    }
    associate_geometry_with_uv_pool(geometry, environment_count, &uv_pool)
}

/// Alternate explicit UV expressions use the same qualified global topology.
/// The complete bounded pool must correspond to every original UV record.
pub fn associate_geometry_with_uv_pool(
    geometry: &Geometry,
    environment_count: usize,
    uv_pool: &[[u32; 2]],
) -> io::Result<Vec<PartTextureRoute>> {
    if environment_count == 0
        || environment_count > 4096
        || uv_pool.len() > 262_144
        || uv_pool
            .iter()
            .flatten()
            .any(|&v| !f32::from_bits(v).is_finite())
    {
        return Err(invalid("alternate UV pool/environment bounds"));
    }
    let mut count = 0;
    for part in &geometry.parts {
        if part.uv_base != count || count + part.uv_bits.len() > uv_pool.len() {
            return Err(invalid("alternate UV pool shape differs from geometry"));
        }
        count += part.uv_bits.len();
    }
    if count != uv_pool.len() {
        return Err(invalid("alternate UV pool has extra records"));
    }
    let mut result = Vec::new();
    let mut corners = 0usize;
    for (part_ordinal, part) in geometry.parts.iter().enumerate() {
        let environment_ordinal = (part.texture_index as u32 & 0xffff) as usize;
        if environment_ordinal >= environment_count {
            return Err(invalid("masked texture index outside environments"));
        }
        let part_corners = part
            .triangles
            .len()
            .checked_mul(3)
            .ok_or_else(|| invalid("texture UV corner count overflow"))?;
        corners = corners
            .checked_add(part_corners)
            .filter(|&n| n <= 524_288 * 3)
            .ok_or_else(|| invalid("texture UV corner budget exceeded"))?;
        let mut expanded_uv_bits = Vec::with_capacity(part_corners);
        let mut cross_part_uv_corners = 0;
        for triangle in &part.triangles {
            for corner in triangle {
                let uv = uv_pool
                    .get(corner.uv_index)
                    .ok_or_else(|| invalid("global UV corner outside pool"))?;
                expanded_uv_bits.push(*uv);
                cross_part_uv_corners += usize::from(
                    !(part.uv_base..part.uv_base + part.uv_bits.len()).contains(&corner.uv_index),
                );
            }
        }
        result.push(PartTextureRoute {
            part_ordinal,
            raw_texture_index: part.texture_index,
            environment_ordinal,
            expanded_uv_bits,
            cross_part_uv_corners,
        });
    }
    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model_geometry::{GeometryPart, VertexRef};
    fn fixture() -> Vec<u8> {
        let mut bytes = vec![0; 5160];
        for (i, v) in [0i32, 1024, 1, 3, 36, 40, 328, 808, 1064]
            .into_iter()
            .enumerate()
        {
            bytes[i * 4..i * 4 + 4].copy_from_slice(&v.to_le_bytes());
        }
        bytes[76..80].copy_from_slice(&100u32.to_le_bytes());
        bytes[300..304].copy_from_slice(&808i32.to_le_bytes());
        for (ordinal, psm) in [20u64, 19, 20].into_iter().enumerate() {
            let tex0 = (psm << 20) | (4u64 << 26) | (4u64 << 30) | (100u64 << 37);
            let start = 328 + ordinal * 160 + 112;
            bytes[start..start + 8].copy_from_slice(&tex0.to_le_bytes());
        }
        bytes
    }
    fn replacement(ordinal: u32, offset: i32) -> ReplacementMetadata {
        ReplacementMetadata {
            ordinal,
            name_bytes: [0; 32],
            declared_offset: 0,
            original_asset_offset: offset,
            declared_decompressed_length: 0,
            compression_tag: -2,
            package_offset: 0,
            padded_stored_length: 0,
            compressed_bytes_consumed: None,
            uncompressed_padding_retained: false,
        }
    }
    fn texa(bytes: &mut Vec<u8>) {
        bytes.extend(b"TEXA");
        bytes.extend(48i32.to_le_bytes());
        for word in [0u16, 1, 0, 4, 0, 0, 0, 2, 0, 0, 3, 2] {
            bytes.extend(word.to_le_bytes());
        }
        for word in [40i32, 40, 40, -1] {
            bytes.extend(word.to_le_bytes());
        }
        bytes.extend([1u8, 2, 3, 4, 5, 6, 7, 8]);
    }
    #[test]
    fn mixed_indexed_depth_adjustments_keep_ambiguous_and_unmatched_metadata() {
        let offsets = [824u64, 808, 825].map(|v| v + 64 + 0x2000_0000);
        let links = read_links(
            &fixture(),
            64,
            &[
                replacement(5, offsets[1] as i32),
                replacement(7, offsets[1] as i32),
                replacement(9, -1),
            ],
        )
        .unwrap();
        assert_eq!(
            links
                .ordered_targets
                .iter()
                .map(|t| t.candidate_original_asset_offset)
                .collect::<Vec<_>>(),
            offsets
        );
        assert_eq!(
            links.ordered_targets[1].matching_replacement_ordinals,
            [5, 7]
        );
        assert_eq!(links.unmatched_replacement_ordinals, [9]);
        assert!(!links.runtime_verified);
    }
    #[test]
    fn texa_header_odd_width_sprite_span_and_order_are_bounded() {
        let mut bytes = fixture();
        texa(&mut bytes);
        let links = read_links(&bytes, 64, &[]).unwrap();
        assert_eq!(links.texa_headers[0].header_i32, [40, 40, 40, -1]);
        assert_eq!(links.texa_headers[0].sprite_data_length, 8);
        assert_eq!(links.texa_headers[0].sprite_data_offset, 5208);
        assert_eq!(
            links
                .ordered_targets
                .iter()
                .map(|t| t.environment_ordinal)
                .collect::<Vec<_>>(),
            [0, 1, 1, 2]
        );
        assert!(matches!(
            links.ordered_targets[2].kind,
            TargetKind::Texa { footer_ordinal: 0 }
        ));
        for (offset, value) in [(5160 + 8 + 32, -1i32), (5160 + 8 + 32, 47)] {
            let mut invalid = bytes.clone();
            invalid[offset..offset + 4].copy_from_slice(&value.to_le_bytes());
            assert!(read_links(&invalid, 64, &[]).is_err());
        }
        let mut invalid = bytes.clone();
        invalid[5170..5172].copy_from_slice(&3u16.to_le_bytes());
        assert!(read_links(&invalid, 64, &[]).is_err());
        let mut invalid = bytes.clone();
        invalid[5174..5176].copy_from_slice(&32u16.to_le_bytes());
        assert!(read_links(&invalid, 64, &[]).is_err());
    }
    #[test]
    fn tag_signatures_in_picture_bytes_never_become_footer_records() {
        let mut bytes = fixture();
        bytes[808..812].copy_from_slice(b"TEXA");
        let links = read_links(&bytes, 64, &[]).unwrap();
        assert!(links.texa_headers.is_empty());
        assert_eq!(links.ordered_targets.len(), 3);
        assert!(read_links(&bytes, 32 * 1024 * 1024, &[]).is_err());
        assert!(read_links(&bytes, 64, &[replacement(0, 0), replacement(0, 1)]).is_err());
    }
    fn geometry() -> Geometry {
        let part = |base, texture, uv, triangles| GeometryPart {
            group_ordinal: 0,
            packet_ordinal: 0,
            texture_index: texture,
            is_opaque: true,
            attributes: 0,
            vertex_base: 0,
            uv_base: base,
            assignments: Vec::new(),
            uv_bits: uv,
            triangles,
        };
        let corner = |uv| VertexRef {
            vertex_index: 0,
            uv_index: uv,
        };
        Geometry {
            profile: crate::model_geometry::PROFILE,
            block_offset: 144,
            excluded_linked_groups: 0,
            runtime_verified: false,
            parts: vec![
                part(0, 0, vec![[(-0.25f32).to_bits(), 2.0f32.to_bits()]], vec![]),
                part(
                    1,
                    0x40000001,
                    vec![[0, 1.0f32.to_bits()]],
                    vec![[corner(0), corner(1), corner(0)]],
                ),
            ],
        }
    }
    #[test]
    fn masks_texture_index_and_resolves_global_cross_part_uvs_without_clamping() {
        let routes = associate_geometry(&geometry(), 2).unwrap();
        assert_eq!(routes[1].environment_ordinal, 1);
        assert_eq!(routes[1].raw_texture_index, 0x40000001);
        assert_eq!(routes[1].cross_part_uv_corners, 2);
        assert_eq!(
            routes[1].expanded_uv_bits[0],
            [(-0.25f32).to_bits(), 2.0f32.to_bits()]
        );
    }
    #[test]
    fn alternate_uv_pool_preserves_global_corners_and_rejects_shape_or_nonfinite_data() {
        let data = geometry();
        let pool = [
            [1.0f32.to_bits(), (-2.0f32).to_bits()],
            [3.0f32.to_bits(), 4.0f32.to_bits()],
        ];
        let routes = associate_geometry_with_uv_pool(&data, 2, &pool).unwrap();
        assert_eq!(routes[1].expanded_uv_bits, [pool[0], pool[1], pool[0]]);
        assert_eq!(routes[1].cross_part_uv_corners, 2);
        assert!(associate_geometry_with_uv_pool(&data, 2, &pool[..1]).is_err());
        let extra = [pool[0], pool[1], pool[0]];
        assert!(associate_geometry_with_uv_pool(&data, 2, &extra).is_err());
        let nonfinite = [[f32::NAN.to_bits(), 0], pool[1]];
        assert!(associate_geometry_with_uv_pool(&data, 2, &nonfinite).is_err());
        let mut bad = data;
        bad.parts[1].uv_base = 0;
        assert!(associate_geometry_with_uv_pool(&bad, 2, &pool).is_err());
    }
    #[test]
    fn unknown_environment_bad_global_bases_corners_and_nonfinite_uvs_reject() {
        assert!(associate_geometry(&geometry(), 1).is_err());
        let mut data = geometry();
        data.parts[1].uv_base = 0;
        assert!(associate_geometry(&data, 2).is_err());
        let mut data = geometry();
        data.parts[1].triangles[0][0].uv_index = 2;
        assert!(associate_geometry(&data, 2).is_err());
        let mut data = geometry();
        data.parts[0].uv_bits[0][0] = f32::NAN.to_bits();
        assert!(associate_geometry(&data, 2).is_err());
        let mut data = geometry();
        data.parts[1].triangles = vec![data.parts[1].triangles[0]; 524_289];
        assert!(associate_geometry(&data, 2).is_err());
    }
}
