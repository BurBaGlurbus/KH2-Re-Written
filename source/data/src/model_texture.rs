//! Bounded public model-texture layout and CLUT profile. Pixel swizzle,
//! HD routing, installed materials and animation remain unqualified.
//! Adapted from OpenKH ModelTexture/Tm2/Ps2 at the pin recorded in NOTICE.
use serde::Serialize;
use sha2::{Digest, Sha256};
use std::io;
pub const PROFILE: &str = "openkh_model_texture_layout_clut_v1";
const MAX_BYTES: usize = 32 * 1024 * 1024;
const MAX_IMAGE_READ_BYTES: usize = 128 * 1024 * 1024;
#[derive(Debug, Serialize)]
pub struct Transfer {
    pub words: Vec<u32>,
    pub destination_base: u32,
    pub upload_psm: u32,
    pub data_offset: i32,
    pub quad_word_count: u32,
}
#[derive(Debug, Serialize)]
pub struct Environment {
    pub words: [u64; 20],
    pub address: [u32; 6],
    pub tex0: [u32; 12],
}
#[derive(Debug, Serialize)]
pub struct ImageView {
    pub environment_ordinal: usize,
    pub transfer_ordinal: usize,
    pub width: u32,
    pub height: u32,
    pub indexed_bits_per_pixel: u32,
    pub upload_psm: u32,
    pub data_offset: usize,
    /// The stored bytes: the declared size, or less when the image is
    /// taller than its upload (the rest decodes as zeros).
    pub data_length: usize,
    pub stored_data_sha256: String,
    pub relative_clut_base: i32,
    pub clut_entry_offset: u32,
    pub clut_source_indices: Vec<usize>,
    pub clut_rgba: Vec<[u8; 4]>,
}
#[derive(Debug, Serialize)]
pub struct FooterChunk {
    pub offset: usize,
    pub tag: [u8; 4],
    pub length: usize,
    pub payload_sha256: String,
    pub uv_scroll_words: Option<[u32; 3]>,
}
#[derive(Debug, Serialize)]
pub struct TextureLayout {
    pub profile: &'static str,
    pub header_words: [i32; 9],
    pub disabled: bool,
    pub offset_data: Vec<u8>,
    pub transfers: Vec<Transfer>,
    pub environments: Vec<Environment>,
    pub picture_sha256: Option<String>,
    pub palette_sha256: Option<String>,
    pub images: Vec<ImageView>,
    pub footer: Vec<FooterChunk>,
    pub pixel_swizzle_decoded: bool,
    pub runtime_verified: bool,
}
fn invalid(message: &str) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, message)
}
fn range(bytes: &[u8], offset: usize, length: usize) -> io::Result<&[u8]> {
    let end = offset
        .checked_add(length)
        .ok_or_else(|| invalid("texture range overflow"))?;
    bytes
        .get(offset..end)
        .ok_or_else(|| invalid("texture range outside payload"))
}
fn hash(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}
fn bits(word: u64, position: usize, length: usize) -> u32 {
    ((word >> position) & ((1 << length) - 1)) as u32
}
fn clut_index(index: usize, cbp: usize, csa: usize) -> usize {
    (index & 7)
        + (index & 8) * 8
        + (index & 16) / 2
        + (index & !31) * 4
        + (cbp & 7) * 4
        + (cbp & 8) * 128
        + (cbp & 16) * 2
        + (cbp & !31) * 64
        + (csa & 1) * 8
        + (csa & 14) * 64
}
pub fn read_layout(bytes: &[u8]) -> io::Result<TextureLayout> {
    if bytes.len() > MAX_BYTES {
        return Err(invalid("model texture exceeds 32 MiB"));
    }
    let header = range(bytes, 0, 36)?;
    let header_words =
        std::array::from_fn(|i| i32::from_le_bytes(header[i * 4..i * 4 + 4].try_into().unwrap()));
    let mut result = TextureLayout {
        profile: PROFILE,
        header_words,
        disabled: false,
        offset_data: Vec::new(),
        transfers: Vec::new(),
        environments: Vec::new(),
        picture_sha256: None,
        palette_sha256: None,
        images: Vec::new(),
        footer: Vec::new(),
        pixel_swizzle_decoded: false,
        runtime_verified: false,
    };
    if header_words[0] == -1 {
        result.disabled = true;
        return Ok(result);
    }
    if header_words[0] != 0
        || !(0..=1_048_576).contains(&header_words[1])
        || !(1..=4096).contains(&header_words[2])
        || !(1..=4096).contains(&header_words[3])
        || header_words[4..].iter().any(|&v| v < 36)
    {
        return Err(invalid("unsupported model texture header"));
    }
    let [
        _,
        colors,
        transfers,
        environments,
        map,
        transfer_base,
        environment_base,
        picture_base,
        palette_base,
    ] = header_words.map(|v| v as usize);
    if map + environments > transfer_base
        || transfer_base + (transfers + 1) * 144 > environment_base
        || environment_base + environments * 160 > picture_base
        || picture_base > palette_base
    {
        return Err(invalid("model texture tables overlap or are unordered"));
    }
    result.offset_data = range(bytes, map, environments)?.to_vec();
    let picture = range(bytes, picture_base, palette_base - picture_base)?;
    let palette = range(bytes, palette_base, colors * 4)?;
    result.picture_sha256 = Some(hash(picture));
    result.palette_sha256 = Some(hash(palette));
    for ordinal in 0..=transfers {
        let raw = range(bytes, transfer_base + ordinal * 144, 144)?;
        let words: [u32; 36] =
            std::array::from_fn(|i| u32::from_le_bytes(raw[i * 4..i * 4 + 4].try_into().unwrap()));
        let bitblt = u64::from_le_bytes(raw[32..40].try_into().unwrap());
        result.transfers.push(Transfer {
            words: words.to_vec(),
            destination_base: bits(bitblt, 32, 14),
            upload_psm: bits(bitblt, 56, 6),
            data_offset: words[29] as i32,
            quad_word_count: words[24] & 0x7fff,
        });
    }
    let mut image_read_bytes = 0usize;
    for ordinal in 0..environments {
        let raw = range(bytes, environment_base + ordinal * 160, 160)?;
        let words =
            std::array::from_fn(|i| u64::from_le_bytes(raw[i * 8..i * 8 + 8].try_into().unwrap()));
        let address = [(0, 2), (2, 2), (4, 10), (14, 10), (24, 10), (34, 10)]
            .map(|(p, l)| bits(words[16], p, l));
        let tex0 = [
            (0, 14),
            (14, 6),
            (20, 6),
            (26, 4),
            (30, 4),
            (34, 1),
            (35, 2),
            (37, 14),
            (51, 4),
            (55, 1),
            (56, 5),
            (61, 3),
        ]
        .map(|(p, l)| bits(words[14], p, l));
        result.environments.push(Environment {
            words,
            address,
            tex0,
        });
        let transfer_ordinal = result.offset_data[ordinal] as usize;
        let transfer = result
            .transfers
            .get(transfer_ordinal + 1)
            .ok_or_else(|| invalid("texture map outside transfer array"))?;
        let width = 1u32 << tex0[3];
        let height = 1u32 << tex0[4];
        let indexed_bits_per_pixel = match tex0[2] {
            19 => 8,
            20 => 4,
            _ => return Err(invalid("texture PSM outside indexed public profile")),
        };
        let data_length = width as usize * height as usize / (8 / indexed_bits_per_pixel) as usize;
        image_read_bytes = image_read_bytes
            .checked_add(data_length)
            .filter(|&total| total <= MAX_IMAGE_READ_BYTES)
            .ok_or_else(|| invalid("cumulative texture image reads exceed 128 MiB"))?;
        let data_offset = usize::try_from(transfer.data_offset)
            .map_err(|_| invalid("negative texture image pointer"))?;
        // A texture taller than its upload whose declared size runs past the
        // payload (WM_SYMBOL_CA: 128x256 over 1536 quadwords) keeps the
        // bytes its transfer uploads; the decoder zero-fills the rest, as
        // OpenKH's GetData does.
        let available = bytes.len().saturating_sub(data_offset);
        let data_length = if data_length <= available {
            data_length
        } else {
            data_length.min(transfer.quad_word_count as usize * 16)
        };
        if data_length == 0 {
            return Err(invalid("texture image has no uploaded bytes"));
        }
        let data = range(bytes, data_offset, data_length)?;
        let relative_clut_base = tex0[7] as i32 - result.transfers[0].destination_base as i32;
        let cbp = usize::try_from(relative_clut_base)
            .map_err(|_| invalid("negative relative CLUT pointer"))?;
        let count = if indexed_bits_per_pixel == 4 { 16 } else { 256 };
        let mut clut_source_indices = Vec::new();
        let mut clut_rgba = Vec::new();
        for index in 0..count {
            let src = clut_index(index, cbp, tex0[10] as usize);
            let color = range(palette, src * 4, 4)?;
            clut_source_indices.push(src);
            clut_rgba.push([color[0], color[1], color[2], color[3].saturating_mul(2)]);
        }
        result.images.push(ImageView {
            environment_ordinal: ordinal,
            transfer_ordinal,
            width,
            height,
            indexed_bits_per_pixel,
            upload_psm: transfer.upload_psm,
            data_offset,
            data_length,
            stored_data_sha256: hash(data),
            relative_clut_base,
            clut_entry_offset: tex0[10],
            clut_source_indices,
            clut_rgba,
        });
    }
    let mut cursor = palette_base + palette.len();
    while cursor < bytes.len() {
        if result.footer.len() >= 4096 {
            return Err(invalid("texture footer chunk count exceeds limit"));
        }
        let tag: [u8; 4] = range(bytes, cursor, 4)?.try_into().unwrap();
        let (start, length) = if &tag == b"_KN5" {
            (cursor + 4, bytes.len() - cursor - 4)
        } else {
            let length = i32::from_le_bytes(range(bytes, cursor + 4, 4)?.try_into().unwrap());
            if length < 0 {
                return Err(invalid("negative texture footer chunk length"));
            }
            (cursor + 8, length as usize)
        };
        let payload = range(bytes, start, length)?;
        let uv_scroll_words = if &tag == b"UVSC" {
            let raw = range(payload, 0, 12)?;
            Some(std::array::from_fn(|i| {
                u32::from_le_bytes(raw[i * 4..i * 4 + 4].try_into().unwrap())
            }))
        } else {
            None
        };
        result.footer.push(FooterChunk {
            offset: cursor,
            tag,
            length,
            payload_sha256: hash(payload),
            uv_scroll_words,
        });
        cursor = start + length;
    }
    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;
    fn fixture() -> Vec<u8> {
        let mut bytes = vec![0; 36 + 4 + 2 * 144 + 160 + 256 + 4096];
        for (i, v) in [0i32, 1024, 1, 1, 36, 40, 328, 488, 744]
            .into_iter()
            .enumerate()
        {
            bytes[i * 4..i * 4 + 4].copy_from_slice(&v.to_le_bytes());
        }
        bytes[40 + 36..40 + 40].copy_from_slice(&100u32.to_le_bytes());
        bytes[184 + 116..184 + 120].copy_from_slice(&488i32.to_le_bytes());
        let tex0 = (19u64 << 20) | (4u64 << 26) | (4u64 << 30) | (100u64 << 37);
        bytes[328 + 112..328 + 120].copy_from_slice(&tex0.to_le_bytes());
        for i in 0..1024 {
            bytes[744 + i * 4..744 + i * 4 + 4].copy_from_slice(&[
                i as u8,
                17,
                23,
                i.min(255) as u8,
            ]);
        }
        bytes
    }
    #[test]
    fn separates_environment_map_transfer_header_and_public_palette_addressing() {
        let bytes = fixture();
        let result = read_layout(&bytes).unwrap();
        let image = &result.images[0];
        assert_eq!(
            (
                image.width,
                image.height,
                image.data_offset,
                image.data_length
            ),
            (16, 16, 488, 256)
        );
        assert_eq!(image.clut_source_indices[8], 64);
        assert_eq!(image.clut_source_indices[16], 8);
        assert_eq!(image.clut_rgba[8], [64, 17, 23, 128]);
        assert_eq!(image.clut_rgba[128][3], 255);
        assert!(!result.pixel_swizzle_decoded);
    }
    #[test]
    fn indexed4_reads_sixteen_palette_entries_and_preserves_csa_bits() {
        let mut bytes = fixture();
        let tex0 = (20u64 << 20) | (4u64 << 26) | (4u64 << 30) | (100u64 << 37) | (1u64 << 56);
        bytes[440..448].copy_from_slice(&tex0.to_le_bytes());
        let result = read_layout(&bytes).unwrap();
        let image = &result.images[0];
        assert_eq!(image.data_length, 128);
        assert_eq!(image.clut_rgba.len(), 16);
        assert_eq!(image.clut_entry_offset, 1);
        assert_eq!(image.clut_source_indices[0], 8);
    }
    #[test]
    fn an_image_taller_than_its_upload_keeps_the_uploaded_bytes() {
        let mut bytes = fixture();
        // The 16x16 image's transfer sits 128 bytes before the end and
        // uploads 8 quadwords; the declared 256 bytes run past the payload.
        let at = bytes.len() - 128;
        bytes[184 + 116..184 + 120].copy_from_slice(&(at as i32).to_le_bytes());
        bytes[184 + 96..184 + 100].copy_from_slice(&8u32.to_le_bytes());
        let image = &read_layout(&bytes).unwrap().images[0];
        assert_eq!((image.data_offset, image.data_length), (at, 128));
        // Without the uploaded bytes in the payload it is still refused.
        bytes[184 + 96..184 + 100].copy_from_slice(&9u32.to_le_bytes());
        assert!(read_layout(&bytes).is_err());
        bytes[184 + 96..184 + 100].copy_from_slice(&0u32.to_le_bytes());
        assert!(read_layout(&bytes).is_err());
    }
    #[test]
    fn retains_footer_unknown_payload_and_uv_scroll_float_bits() {
        let mut bytes = fixture();
        bytes.extend(b"UVSC");
        bytes.extend(12i32.to_le_bytes());
        for word in [1u32, (-0.0f32).to_bits(), f32::NAN.to_bits()] {
            bytes.extend(word.to_le_bytes());
        }
        bytes.extend(b"_KN5opaque");
        let result = read_layout(&bytes).unwrap();
        assert_eq!(result.footer.len(), 2);
        assert_eq!(result.footer[0].uv_scroll_words.unwrap()[1], 0x80000000);
        assert_eq!(result.footer[1].length, 6);
        let mut invalid = fixture();
        invalid.extend(b"TEXA");
        invalid.extend((-1i32).to_le_bytes());
        assert!(read_layout(&invalid).is_err());
    }
    #[test]
    fn rejects_truncation_bad_map_range_psm_palette_and_table_order() {
        for len in [0, 35, 200, 743, 1000] {
            assert!(read_layout(&fixture()[..len]).is_err());
        }
        let mut bytes = fixture();
        bytes[36] = 1;
        assert!(read_layout(&bytes).is_err());
        let mut bytes = fixture();
        bytes[300..304].copy_from_slice(&(-1i32).to_le_bytes());
        assert!(read_layout(&bytes).is_err());
        let mut bytes = fixture();
        bytes[440..448].fill(0);
        assert!(read_layout(&bytes).is_err());
        let mut bytes = fixture();
        bytes[4..8].copy_from_slice(&1i32.to_le_bytes());
        assert!(read_layout(&bytes).is_err());
        let mut bytes = fixture();
        bytes[24..28].copy_from_slice(&40i32.to_le_bytes());
        assert!(read_layout(&bytes).is_err());
    }
    #[test]
    fn disabled_header_retains_words_without_reading_untrusted_offsets() {
        let words = [-1i32, i32::MAX, -1, -1, -1, -1, -1, -1, -1];
        let bytes: Vec<_> = words.into_iter().flat_map(i32::to_le_bytes).collect();
        let result = read_layout(&bytes).unwrap();
        assert!(result.disabled);
        assert_eq!(result.header_words, words);
        assert!(result.images.is_empty());
        assert!(result.picture_sha256.is_none());
    }
    #[test]
    fn preserves_high_register_fields_and_palette_bank_bits() {
        let mut bytes = fixture();
        let tex0 = (16383u64)
            | (63u64 << 14)
            | (20u64 << 20)
            | (4u64 << 26)
            | (4u64 << 30)
            | (1u64 << 34)
            | (3u64 << 35)
            | (100u64 << 37)
            | (15u64 << 51)
            | (1u64 << 55)
            | (31u64 << 56)
            | (7u64 << 61);
        bytes[440..448].copy_from_slice(&tex0.to_le_bytes());
        bytes[456..464].copy_from_slice(&u64::MAX.to_le_bytes());
        let result = read_layout(&bytes).unwrap();
        assert_eq!(
            result.environments[0].tex0,
            [16383, 63, 20, 4, 4, 1, 3, 100, 15, 1, 31, 7]
        );
        assert_eq!(
            result.environments[0].address,
            [3, 3, 1023, 1023, 1023, 1023]
        );
        assert_eq!(result.images[0].clut_entry_offset, 31);
        assert_eq!(result.images[0].clut_source_indices[0], 904);
    }
    #[test]
    fn bounds_repeated_overlapping_image_hash_work() {
        let original = fixture();
        let count = 129usize;
        let transfer_base = 36 + count;
        let environment_base = transfer_base + 288;
        let picture_base = environment_base + count * 160;
        let palette_base = picture_base + 1024 * 1024;
        let mut bytes = vec![0; palette_base + 4096];
        for (i, word) in [
            0,
            1024,
            1,
            count as i32,
            36,
            transfer_base as i32,
            environment_base as i32,
            picture_base as i32,
            palette_base as i32,
        ]
        .into_iter()
        .enumerate()
        {
            bytes[i * 4..i * 4 + 4].copy_from_slice(&word.to_le_bytes());
        }
        bytes[transfer_base..transfer_base + 288].copy_from_slice(&original[40..328]);
        bytes[transfer_base + 144 + 116..transfer_base + 144 + 120]
            .copy_from_slice(&(picture_base as i32).to_le_bytes());
        let tex0 = (19u64 << 20) | (10u64 << 26) | (10u64 << 30) | (100u64 << 37);
        for ordinal in 0..count {
            let offset = environment_base + ordinal * 160 + 112;
            bytes[offset..offset + 8].copy_from_slice(&tex0.to_le_bytes());
        }
        assert_eq!(
            read_layout(&bytes).unwrap_err().to_string(),
            "cumulative texture image reads exceed 128 MiB"
        );
    }
}
