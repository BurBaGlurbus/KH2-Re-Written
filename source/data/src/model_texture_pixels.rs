//! Explicit bounded CPU GetData profile, adapted from pinned OpenKH
//! ModelTexture/Ps2.Reform32/8/4. No installed GS/material/HD claim.
use crate::model_texture;
use std::io;

pub const PROFILE: &str = "openkh_model_indexed_getdata_v1";
const MAX_RGBA: usize = 64 * 1024 * 1024;
const MAX_TOTAL_RGBA: usize = 128 * 1024 * 1024;
fn invalid(message: &str) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, message)
}

pub struct DecodedTexture {
    pub environment_ordinal: usize,
    pub width: u32,
    pub height: u32,
    pub indexed_bits_per_pixel: u32,
    pub upload_psm: u32,
    /// Indexed4 retains the public high-nibble-first output packing.
    pub packed_indices: Vec<u8>,
    pub clut_rgba: Vec<u8>,
    pub pixels_rgba: Vec<u8>,
}

/// Reproduces the public Texture.GetData indexed branch within bounded inputs.
/// PSM-zero requires complete pages; `decode_images` zero-fills a tail the
/// upload does not cover first. Nonzero upload values retain the public
/// direct path.
pub fn decode_stored_indices(
    data: &[u8],
    width: u32,
    height: u32,
    depth: u32,
    upload_psm: u32,
) -> io::Result<Vec<u8>> {
    if !(1..=4096).contains(&width)
        || !(1..=4096).contains(&height)
        || ![4, 8].contains(&depth)
        || upload_psm > 63
    {
        return Err(invalid(
            "indexed texture dimensions/depth/upload outside profile",
        ));
    }
    let pixels = width as usize * height as usize;
    if pixels * 4 > MAX_RGBA
        || (depth == 4 && !pixels.is_multiple_of(2))
        || data.len() != pixels / (8 / depth) as usize
    {
        return Err(invalid("indexed texture input length or RGBA bound"));
    }
    if upload_psm != 0 {
        return Ok(data.to_vec());
    }
    let page_height = if depth == 8 { 64 } else { 128 };
    if !width.is_multiple_of(128) || !height.is_multiple_of(page_height) {
        return Err(invalid("indexed texture swizzle requires complete pages"));
    }
    let bw = width as usize / 128;
    let bh = height as usize / page_height as usize;
    let memory = encode32(data, bw, bh);
    Ok(decode_memory(&memory, bw, bh, depth))
}

/// Pure byte rearrangement: no source-file writer or PS2 microprogram executes.
fn encode32(data: &[u8], bw: usize, bh: usize) -> Vec<u8> {
    let mut memory = vec![0; data.len()];
    for page_y in 0..bh {
        for page_x in 0..bw {
            let page = 8192 * (page_x + bw * page_y);
            for block_y in 0..4 {
                for block_x in 0..8 {
                    let block = 256 * BLOCK32[block_x + block_y * 8];
                    for column in 0..4 {
                        for (lane, &address) in COLUMN32.iter().enumerate() {
                            let x = page_x * 64 + block_x * 8 + lane % 8;
                            let y = page_y * 32 + block_y * 8 + 2 * column + lane / 8;
                            let source = 4 * (x + 64 * bw * y);
                            let target = page + block + 64 * column + 4 * address;
                            memory[target..target + 4].copy_from_slice(&data[source..source + 4]);
                        }
                    }
                }
            }
        }
    }
    memory
}

fn decode_memory(memory: &[u8], bw: usize, bh: usize, depth: u32) -> Vec<u8> {
    let mut output = vec![0; memory.len()];
    let (page_height, block_width, blocks_x, blocks_y) = if depth == 8 {
        (64, 16, 8, 4)
    } else {
        (128, 32, 4, 8)
    };
    for page_y in 0..bh {
        for page_x in 0..bw {
            let page = 8192 * (page_x + bw * page_y);
            for block_y in 0..blocks_y {
                for block_x in 0..blocks_x {
                    let block_table = if depth == 8 { &BLOCK32 } else { &BLOCK4 };
                    let block = 256 * block_table[block_x + block_y * blocks_x];
                    for column in 0..4 {
                        let addresses: &[usize] = match (depth, column % 2) {
                            (8, 0) => &COLUMN8_0,
                            (8, _) => &COLUMN8_1,
                            (4, 0) => &COLUMN4_0,
                            _ => &COLUMN4_1,
                        };
                        for (lane, &address) in addresses.iter().enumerate() {
                            let x = page_x * 128 + block_x * block_width + lane % block_width;
                            let y = page_y * page_height
                                + block_y * 16
                                + column * 4
                                + lane / block_width;
                            let pixel = x + 128 * bw * y;
                            let base = page + block + column * 64;
                            if depth == 8 {
                                output[pixel] = memory[base + address];
                            } else {
                                let value = (memory[base + address / 8] >> (address % 8)) & 15;
                                if pixel.is_multiple_of(2) {
                                    output[pixel / 2] = (output[pixel / 2] & 15) | (value << 4);
                                } else {
                                    output[pixel / 2] = (output[pixel / 2] & 240) | value;
                                }
                            }
                        }
                    }
                }
            }
        }
    }
    output
}

/// Revalidates the original layout; caller-supplied decoded fields cannot select
/// source spans or palettes. Opaque footer animation remains outside decoding.
pub fn decode_images(bytes: &[u8]) -> io::Result<Vec<DecodedTexture>> {
    let layout = model_texture::read_layout(bytes)?;
    layout.images.iter().try_fold(0usize, |total, image| {
        total
            .checked_add(image.width as usize * image.height as usize * 4)
            .filter(|&value| value <= MAX_TOTAL_RGBA)
            .ok_or_else(|| invalid("model texture cumulative RGBA exceeds 128 MiB"))
    })?;
    let mut images = Vec::new();
    for image in layout.images {
        // An image taller than its upload: the missing tail reads as zeros.
        let declared = image.width as usize * image.height as usize
            / (8 / image.indexed_bits_per_pixel) as usize;
        let mut stored = bytes[image.data_offset..image.data_offset + image.data_length].to_vec();
        stored.resize(declared.max(stored.len()), 0);
        let packed_indices = decode_stored_indices(
            &stored,
            image.width,
            image.height,
            image.indexed_bits_per_pixel,
            image.upload_psm,
        )?;
        let clut_rgba: Vec<_> = image.clut_rgba.into_iter().flatten().collect();
        let count = image.width as usize * image.height as usize;
        let mut pixels_rgba = Vec::with_capacity(count * 4);
        for pixel in 0..count {
            let index = if image.indexed_bits_per_pixel == 8 {
                packed_indices[pixel] as usize
            } else {
                ((packed_indices[pixel / 2] >> if pixel.is_multiple_of(2) { 4 } else { 0 }) & 15)
                    as usize
            };
            pixels_rgba.extend_from_slice(&clut_rgba[index * 4..index * 4 + 4]);
        }
        images.push(DecodedTexture {
            environment_ordinal: image.environment_ordinal,
            width: image.width,
            height: image.height,
            indexed_bits_per_pixel: image.indexed_bits_per_pixel,
            upload_psm: image.upload_psm,
            packed_indices,
            clut_rgba,
            pixels_rgba,
        });
    }
    Ok(images)
}

const BLOCK32: [usize; 32] = [
    0, 1, 4, 5, 16, 17, 20, 21, 2, 3, 6, 7, 18, 19, 22, 23, 8, 9, 12, 13, 24, 25, 28, 29, 10, 11,
    14, 15, 26, 27, 30, 31,
];
const COLUMN32: [usize; 16] = [0, 1, 4, 5, 8, 9, 12, 13, 2, 3, 6, 7, 10, 11, 14, 15];
const BLOCK4: [usize; 32] = [
    0, 2, 8, 10, 1, 3, 9, 11, 4, 6, 12, 14, 5, 7, 13, 15, 16, 18, 24, 26, 17, 19, 25, 27, 20, 22,
    28, 30, 21, 23, 29, 31,
];
const COLUMN8_0: [usize; 64] = [
    0x00, 0x04, 0x10, 0x14, 0x20, 0x24, 0x30, 0x34, 0x02, 0x06, 0x12, 0x16, 0x22, 0x26, 0x32, 0x36,
    0x08, 0x0c, 0x18, 0x1c, 0x28, 0x2c, 0x38, 0x3c, 0x0a, 0x0e, 0x1a, 0x1e, 0x2a, 0x2e, 0x3a, 0x3e,
    0x21, 0x25, 0x31, 0x35, 0x01, 0x05, 0x11, 0x15, 0x23, 0x27, 0x33, 0x37, 0x03, 0x07, 0x13, 0x17,
    0x29, 0x2d, 0x39, 0x3d, 0x09, 0x0d, 0x19, 0x1d, 0x2b, 0x2f, 0x3b, 0x3f, 0x0b, 0x0f, 0x1b, 0x1f,
];
const COLUMN8_1: [usize; 64] = [
    0x20, 0x24, 0x30, 0x34, 0x00, 0x04, 0x10, 0x14, 0x22, 0x26, 0x32, 0x36, 0x02, 0x06, 0x12, 0x16,
    0x28, 0x2c, 0x38, 0x3c, 0x08, 0x0c, 0x18, 0x1c, 0x2a, 0x2e, 0x3a, 0x3e, 0x0a, 0x0e, 0x1a, 0x1e,
    0x01, 0x05, 0x11, 0x15, 0x21, 0x25, 0x31, 0x35, 0x03, 0x07, 0x13, 0x17, 0x23, 0x27, 0x33, 0x37,
    0x09, 0x0d, 0x19, 0x1d, 0x29, 0x2d, 0x39, 0x3d, 0x0b, 0x0f, 0x1b, 0x1f, 0x2b, 0x2f, 0x3b, 0x3f,
];
const COLUMN4_0: [usize; 128] = [
    0x00, 0x20, 0x80, 0xa0, 0x100, 0x120, 0x180, 0x1a0, 0x08, 0x28, 0x88, 0xa8, 0x108, 0x128,
    0x188, 0x1a8, 0x10, 0x30, 0x90, 0xb0, 0x110, 0x130, 0x190, 0x1b0, 0x18, 0x38, 0x98, 0xb8,
    0x118, 0x138, 0x198, 0x1b8, 0x40, 0x60, 0xc0, 0xe0, 0x140, 0x160, 0x1c0, 0x1e0, 0x48, 0x68,
    0xc8, 0xe8, 0x148, 0x168, 0x1c8, 0x1e8, 0x50, 0x70, 0xd0, 0xf0, 0x150, 0x170, 0x1d0, 0x1f0,
    0x58, 0x78, 0xd8, 0xf8, 0x158, 0x178, 0x1d8, 0x1f8, 0x104, 0x124, 0x184, 0x1a4, 0x04, 0x24,
    0x84, 0xa4, 0x10c, 0x12c, 0x18c, 0x1ac, 0x0c, 0x2c, 0x8c, 0xac, 0x114, 0x134, 0x194, 0x1b4,
    0x14, 0x34, 0x94, 0xb4, 0x11c, 0x13c, 0x19c, 0x1bc, 0x1c, 0x3c, 0x9c, 0xbc, 0x144, 0x164,
    0x1c4, 0x1e4, 0x44, 0x64, 0xc4, 0xe4, 0x14c, 0x16c, 0x1cc, 0x1ec, 0x4c, 0x6c, 0xcc, 0xec,
    0x154, 0x174, 0x1d4, 0x1f4, 0x54, 0x74, 0xd4, 0xf4, 0x15c, 0x17c, 0x1dc, 0x1fc, 0x5c, 0x7c,
    0xdc, 0xfc,
];
const COLUMN4_1: [usize; 128] = [
    0x100, 0x120, 0x180, 0x1a0, 0x00, 0x20, 0x80, 0xa0, 0x108, 0x128, 0x188, 0x1a8, 0x08, 0x28,
    0x88, 0xa8, 0x110, 0x130, 0x190, 0x1b0, 0x10, 0x30, 0x90, 0xb0, 0x118, 0x138, 0x198, 0x1b8,
    0x18, 0x38, 0x98, 0xb8, 0x140, 0x160, 0x1c0, 0x1e0, 0x40, 0x60, 0xc0, 0xe0, 0x148, 0x168,
    0x1c8, 0x1e8, 0x48, 0x68, 0xc8, 0xe8, 0x150, 0x170, 0x1d0, 0x1f0, 0x50, 0x70, 0xd0, 0xf0,
    0x158, 0x178, 0x1d8, 0x1f8, 0x58, 0x78, 0xd8, 0xf8, 0x04, 0x24, 0x84, 0xa4, 0x104, 0x124,
    0x184, 0x1a4, 0x0c, 0x2c, 0x8c, 0xac, 0x10c, 0x12c, 0x18c, 0x1ac, 0x14, 0x34, 0x94, 0xb4,
    0x114, 0x134, 0x194, 0x1b4, 0x1c, 0x3c, 0x9c, 0xbc, 0x11c, 0x13c, 0x19c, 0x1bc, 0x44, 0x64,
    0xc4, 0xe4, 0x144, 0x164, 0x1c4, 0x1e4, 0x4c, 0x6c, 0xcc, 0xec, 0x14c, 0x16c, 0x1cc, 0x1ec,
    0x54, 0x74, 0xd4, 0xf4, 0x154, 0x174, 0x1d4, 0x1f4, 0x5c, 0x7c, 0xdc, 0xfc, 0x15c, 0x17c,
    0x1dc, 0x1fc,
];

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn public_page_addressing_and_indexed4_high_first_packing() {
        let bytes: Vec<_> = (0..8192).map(|i| (i % 251) as u8).collect();
        let indexed8 = decode_stored_indices(&bytes, 128, 64, 8, 0).unwrap();
        assert_eq!(
            &indexed8[..16],
            &[0, 4, 8, 12, 16, 20, 24, 28, 2, 6, 10, 14, 18, 22, 26, 30]
        );
        let indexed4 = decode_stored_indices(&bytes, 128, 128, 4, 0).unwrap();
        assert_eq!(
            &indexed4[..8],
            &[0x04, 0x8c, 0x04, 0x8c, 0x15, 0x9d, 0x15, 0x9d]
        );
        assert!(indexed8.iter().any(|&v| v > 127));
    }
    #[test]
    fn direct_upload_preserves_bytes_and_rejects_partial_or_oversized_inputs() {
        assert_eq!(
            decode_stored_indices(&[0xab, 0x10], 2, 2, 4, 20).unwrap(),
            [0xab, 0x10]
        );
        assert_eq!(
            decode_stored_indices(&[0, 8, 16, 255], 2, 2, 8, 19).unwrap(),
            [0, 8, 16, 255]
        );
        for (w, h, depth, upload) in [
            (0, 1, 8, 0),
            (8192, 1, 8, 0),
            (1, 1, 4, 20),
            (2, 2, 16, 0),
            (2, 2, 8, 64),
            (2, 2, 8, 0),
            (128, 64, 8, 0),
        ] {
            assert!(decode_stored_indices(&[0; 4], w, h, depth, upload).is_err());
        }
    }
    #[test]
    fn original_layout_palette_and_hidden_rgb_remain_separate() {
        let mut bytes = vec![0; 4840];
        for (i, word) in [0i32, 1024, 1, 1, 36, 40, 328, 488, 744]
            .into_iter()
            .enumerate()
        {
            bytes[i * 4..i * 4 + 4].copy_from_slice(&word.to_le_bytes());
        }
        bytes[76..80].copy_from_slice(&100u32.to_le_bytes());
        bytes[220..224].copy_from_slice(&(19u32 << 24).to_le_bytes());
        bytes[300..304].copy_from_slice(&488i32.to_le_bytes());
        let tex0 = (19u64 << 20) | (4u64 << 26) | (4u64 << 30) | (100u64 << 37);
        bytes[440..448].copy_from_slice(&tex0.to_le_bytes());
        bytes[488..491].copy_from_slice(&[0, 8, 16]);
        for i in 0..1024 {
            bytes[744 + i * 4..748 + i * 4].copy_from_slice(&[i as u8, 17, 23, i.min(255) as u8]);
        }
        let images = decode_images(&bytes).unwrap();
        assert_eq!(images.len(), 1);
        assert_eq!(
            &images[0].pixels_rgba[..12],
            &[0, 17, 23, 0, 64, 17, 23, 128, 8, 17, 23, 16]
        );
        bytes[220..224].copy_from_slice(&(20u32 << 24).to_le_bytes());
        let tex0 = (20u64 << 20) | (4u64 << 26) | (4u64 << 30) | (100u64 << 37);
        bytes[440..448].copy_from_slice(&tex0.to_le_bytes());
        bytes[488..490].copy_from_slice(&[0xab, 0x10]);
        let images = decode_images(&bytes).unwrap();
        assert_eq!(
            &images[0].pixels_rgba[..16],
            &[66, 17, 23, 132, 67, 17, 23, 134, 1, 17, 23, 2, 0, 17, 23, 0]
        );
        bytes[0..4].copy_from_slice(&(-1i32).to_le_bytes());
        assert!(decode_images(&bytes).unwrap().is_empty());
    }
    #[test]
    fn cumulative_rgba_rejects_before_materializing_repeated_large_images() {
        let count = 33usize;
        let transfer_base = 36 + count;
        let environment_base = transfer_base + 288;
        let picture_base = environment_base + 160 * count;
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
        bytes[transfer_base + 144 + 36..transfer_base + 144 + 40]
            .copy_from_slice(&(19u32 << 24).to_le_bytes());
        bytes[transfer_base + 144 + 116..transfer_base + 144 + 120]
            .copy_from_slice(&(picture_base as i32).to_le_bytes());
        let tex0 = (19u64 << 20) | (10u64 << 26) | (10u64 << 30);
        for ordinal in 0..count {
            let offset = environment_base + ordinal * 160 + 112;
            bytes[offset..offset + 8].copy_from_slice(&tex0.to_le_bytes());
        }
        match decode_images(&bytes) {
            Err(error) => assert_eq!(
                error.to_string(),
                "model texture cumulative RGBA exceeds 128 MiB"
            ),
            Ok(_) => panic!("oversized repeated RGBA admitted"),
        }
    }
}
