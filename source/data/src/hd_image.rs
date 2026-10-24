//! Bounded HD texture samples. Pixels are stored-channel straight RGBA8, with
//! no color-profile transform or claim about installed shader interpretation.
use image::{ImageFormat, ImageReader, Limits};
use serde::{Deserialize, Serialize};
use std::io::{self, Cursor, Write};

const MAX_INPUT: usize = 32 * 1024 * 1024;
const MAX_RGBA: u64 = 64 * 1024 * 1024;

#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum HdImageFormat {
    Png,
    DdsBc3,
    DdsBgra8,
}

/// Explicit CPU arithmetic; neither profile qualifies installed GPU behavior.
/// PNG uses the pinned image codec; BGRA8 always records its byte-swap profile.
#[derive(Debug, Default, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum HdPixelDecodeProfile {
    #[default]
    #[serde(rename = "image_0_25_10_stored_rgba8")]
    Image02510StoredRgba8,
    #[serde(rename = "bc3_normalized_nearest_rgba8_v1")]
    Bc3NormalizedNearestRgba8V1,
    #[serde(rename = "legacy_dds_bgra8_straight_rgba8_v1")]
    LegacyDdsBgra8StraightRgba8V1,
}

pub struct DecodedHdImage {
    pub format: HdImageFormat,
    pub pixel_decode_profile: HdPixelDecodeProfile,
    pub width: u32,
    pub height: u32,
    pub encoded_length: usize,
    pub storage_padding: usize,
    pub pixels_rgba: Vec<u8>,
}

pub const BC3_MIP_LAYOUT_PROFILE: &str = "legacy_dds_bc3_mip_chain_v1";
#[derive(Debug, Clone, Serialize)]
pub struct Bc3MipLevelLayout {
    pub ordinal: u32,
    pub width: u32,
    pub height: u32,
    pub encoded_offset: usize,
    pub encoded_length: usize,
}
#[derive(Debug, Serialize)]
pub struct Bc3MipLayout {
    pub profile: &'static str,
    pub header_words: [u32; 31],
    pub encoded_length: usize,
    pub storage_padding: usize,
    pub rgba_byte_count: usize,
    pub levels: Vec<Bc3MipLevelLayout>,
    pub runtime_verified: bool,
}
pub struct DecodedBc3MipChain {
    pub layout: Bc3MipLayout,
    pub pixel_decode_profile: HdPixelDecodeProfile,
    pub levels_rgba: Vec<Vec<u8>>,
}

fn invalid(message: &str) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, message)
}
fn unsupported(message: &str) -> io::Error {
    io::Error::new(io::ErrorKind::Unsupported, message)
}
/// The longest side an HD image may have: the renderer's 2D texture limit
/// (wgpu's default). Some installed textures are tall strips (Mulan's
/// 192 x 7296); the total size stays bounded by the RGBA limits.
pub const MAX_SIDE: u32 = 8192;

fn dimensions(width: u32, height: u32) -> io::Result<()> {
    if width == 0 || height == 0 || width > MAX_SIDE || height > MAX_SIDE {
        return Err(invalid("HD image dimensions exceed the inspection bounds"));
    }
    Ok(())
}

fn png_length(bytes: &[u8]) -> io::Result<usize> {
    let mut cursor = 8usize;
    let mut has_data = false;
    let mut data_ended = false;
    let mut has_palette = false;
    while cursor < bytes.len() {
        let header = bytes
            .get(cursor..cursor + 8)
            .ok_or_else(|| invalid("truncated PNG chunk"))?;
        let length = u32::from_be_bytes(header[..4].try_into().unwrap()) as usize;
        let end = cursor
            .checked_add(12)
            .and_then(|v| v.checked_add(length))
            .filter(|end| *end <= bytes.len())
            .ok_or_else(|| invalid("PNG chunk exceeds source"))?;
        let kind = &header[4..8];
        if !kind.iter().all(u8::is_ascii_alphabetic) || !kind[2].is_ascii_uppercase() {
            return Err(invalid("invalid PNG chunk type"));
        }
        if crc32fast::hash(&bytes[cursor + 4..end - 4])
            != u32::from_be_bytes(bytes[end - 4..end].try_into().unwrap())
        {
            return Err(invalid("PNG chunk checksum mismatch"));
        }
        if cursor == 8 {
            if kind != b"IHDR" || length != 13 {
                return Err(invalid("PNG must start with a 13-byte IHDR"));
            }
            dimensions(
                u32::from_be_bytes(bytes[16..20].try_into().unwrap()),
                u32::from_be_bytes(bytes[20..24].try_into().unwrap()),
            )?;
            if bytes[24] == 16 {
                return Err(unsupported(
                    "16-bit PNG channels require a lossless higher-depth representation",
                ));
            }
        } else if kind == b"IHDR" {
            return Err(invalid("duplicate PNG IHDR"));
        }
        match kind {
            b"acTL" | b"fcTL" | b"fdAT" => {
                return Err(unsupported("animated PNG is not a static HD texture"));
            }
            b"IDAT" => {
                if data_ended {
                    return Err(invalid("non-contiguous PNG image data"));
                }
                has_data = true;
            }
            b"IEND" => {
                if length != 0 || !has_data || end != bytes.len() {
                    return Err(invalid("invalid PNG end or non-storage trailing data"));
                }
                return Ok(end);
            }
            b"IHDR" => {}
            b"PLTE" => {
                if has_palette
                    || has_data
                    || length == 0
                    || length > 768
                    || !length.is_multiple_of(3)
                {
                    return Err(invalid("invalid PNG palette length/order"));
                }
                has_palette = true;
            }
            _ if kind[0].is_ascii_uppercase() => {
                return Err(unsupported("unknown critical PNG chunk"));
            }
            _ => {}
        }
        if has_data && kind != b"IDAT" {
            data_ended = true;
        }
        cursor = end;
    }
    Err(invalid("PNG end chunk is missing"))
}

fn dds_layout(bytes: &[u8]) -> io::Result<(HdImageFormat, usize)> {
    if bytes.len() < 128 {
        return Err(invalid("truncated DDS header"));
    }
    let value = |offset| u32::from_le_bytes(bytes[offset..offset + 4].try_into().unwrap());
    if value(4) != 124 || value(76) != 32 || value(8) & 0x1007 != 0x1007 {
        return Err(invalid("invalid DDS header sizes/required flags"));
    }
    let (width, height) = (value(16), value(12));
    dimensions(width, height)?;
    if value(112) != 0 || value(24) > 1 || value(28) > 1 || value(108) & 0x1000 == 0 {
        return Err(unsupported(
            "HD inspection requires single-level 2D legacy DDS",
        ));
    }
    if value(80) == 0x41 && value(84) == 0 {
        if value(88) != 32
            || [value(92), value(96), value(100), value(104)]
                != [0x00ff_0000, 0x0000_ff00, 0x0000_00ff, 0xff00_0000]
            // The pitch flag (0x8) is required unless the header carries
            // only the required flags; character face sheets (obj/*.a.us)
            // omit it while still storing the tight pitch.
            || (value(8) & 0x100f != 0x100f && value(8) != 0x1007)
            || value(8) & !0x2100f != 0
            || value(20) != width * 4
            || value(108) != 0x1000
            || value(116) != 0
            || value(120) != 0
        {
            return Err(unsupported(
                "BGRA8 DDS inspection requires exact masks and tightly packed rows",
            ));
        }
        let expected = 128 + width as usize * height as usize * 4;
        if bytes.len() != expected {
            return Err(invalid("BGRA8 DDS base-level length differs"));
        }
        return Ok((HdImageFormat::DdsBgra8, expected));
    }
    if value(80) & 4 == 0 || &bytes[84..88] != b"DXT5" {
        return Err(unsupported("DDS pixel format outside BC3/BGRA8 profile"));
    }
    if !width.is_multiple_of(4) || !height.is_multiple_of(4) {
        return Err(unsupported(
            "the pinned BC3 decoder requires complete 4x4 dimensions",
        ));
    }
    let expected = 128 + width as usize / 4 * (height as usize / 4) * 16;
    if bytes.len() != expected {
        return Err(invalid(
            "DDS base-level length differs or extra surfaces are present",
        ));
    }
    Ok((HdImageFormat::DdsBc3, expected))
}

/// `declared_length` comes from validated replacement metadata. Uncompressed
/// alignment bytes remain an opaque source span; their values are not guessed.
pub fn decode_hd_image(bytes: &[u8], declared_length: usize) -> io::Result<DecodedHdImage> {
    decode_hd_image_with_profile(bytes, declared_length, HdPixelDecodeProfile::default())
}

/// Common admission bounds precede either CPU decoder. The override applies
/// only to BC3; the result records the decoder actually used for each image.
pub fn decode_hd_image_with_profile(
    bytes: &[u8],
    declared_length: usize,
    dds_profile: HdPixelDecodeProfile,
) -> io::Result<DecodedHdImage> {
    let (bytes, storage_padding) = logical_hd_input(bytes, declared_length)?;
    let (format, codec, encoded_length) = if bytes.starts_with(b"\x89PNG\r\n\x1a\n") {
        (HdImageFormat::Png, ImageFormat::Png, png_length(bytes)?)
    } else if bytes.starts_with(b"DDS ") {
        let (format, length) = dds_layout(bytes)?;
        (format, ImageFormat::Dds, length)
    } else {
        return Err(unsupported("unknown HD image signature"));
    };
    decode_single_hd_image(
        bytes,
        storage_padding,
        format,
        codec,
        encoded_length,
        dds_profile,
    )
}

fn logical_hd_input(bytes: &[u8], declared_length: usize) -> io::Result<(&[u8], usize)> {
    if bytes.len() > MAX_INPUT {
        return Err(invalid("HD image exceeds 32 MiB input limit"));
    }
    if declared_length == 0
        || declared_length > bytes.len()
        || bytes.len() - declared_length > 15
        || (bytes.len() != declared_length && bytes.len() != declared_length.next_multiple_of(16))
    {
        return Err(invalid(
            "HD image logical length/padded storage range is inconsistent",
        ));
    }
    Ok((&bytes[..declared_length], bytes.len() - declared_length))
}

fn decode_single_hd_image(
    bytes: &[u8],
    storage_padding: usize,
    format: HdImageFormat,
    codec: ImageFormat,
    encoded_length: usize,
    dds_profile: HdPixelDecodeProfile,
) -> io::Result<DecodedHdImage> {
    if format == HdImageFormat::DdsBgra8 {
        let width = u32::from_le_bytes(bytes[16..20].try_into().unwrap());
        let height = u32::from_le_bytes(bytes[12..16].try_into().unwrap());
        let pixels_rgba = bytes[128..]
            .as_chunks::<4>()
            .0
            .iter()
            .flat_map(|&[b, g, r, a]| [r, g, b, a])
            .collect();
        return Ok(DecodedHdImage {
            format,
            pixel_decode_profile: HdPixelDecodeProfile::LegacyDdsBgra8StraightRgba8V1,
            width,
            height,
            encoded_length,
            storage_padding,
            pixels_rgba,
        });
    }
    if format == HdImageFormat::DdsBc3
        && dds_profile == HdPixelDecodeProfile::LegacyDdsBgra8StraightRgba8V1
    {
        return Err(unsupported("BGRA8 profile cannot select a BC3 decoder"));
    }
    if format == HdImageFormat::DdsBc3
        && dds_profile == HdPixelDecodeProfile::Bc3NormalizedNearestRgba8V1
    {
        let width = u32::from_le_bytes(bytes[16..20].try_into().unwrap());
        let height = u32::from_le_bytes(bytes[12..16].try_into().unwrap());
        return Ok(DecodedHdImage {
            format,
            pixel_decode_profile: dds_profile,
            width,
            height,
            encoded_length,
            storage_padding,
            pixels_rgba: normalized_bc3_blocks(&bytes[128..], width as usize, height as usize),
        });
    }
    let mut limits = Limits::default();
    limits.max_image_width = Some(MAX_SIDE);
    limits.max_image_height = Some(MAX_SIDE);
    limits.max_alloc = Some(MAX_RGBA);
    let mut reader = ImageReader::with_format(Cursor::new(&bytes[..encoded_length]), codec);
    reader.limits(limits);
    let pixels = reader
        .decode()
        .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?
        .into_rgba8();
    let (width, height) = pixels.dimensions();
    Ok(DecodedHdImage {
        format,
        pixel_decode_profile: HdPixelDecodeProfile::Image02510StoredRgba8,
        width,
        height,
        encoded_length,
        storage_padding,
        pixels_rgba: pixels.into_raw(),
    })
}

/// Explicit full-chain admission; the ordinary single-image route still rejects
/// mip chains. Raw count/header words are retained; excessive counts never clamp.
pub fn read_bc3_mip_layout(bytes: &[u8], declared_length: usize) -> io::Result<Bc3MipLayout> {
    let (bytes, storage_padding) = logical_hd_input(bytes, declared_length)?;
    if bytes.len() < 128 || !bytes.starts_with(b"DDS ") {
        return Err(invalid("BC3 mip chain requires a complete DDS header"));
    }
    let header_words = std::array::from_fn(|i| {
        u32::from_le_bytes(bytes[4 + i * 4..8 + i * 4].try_into().unwrap())
    });
    let word = |offset: usize| header_words[offset / 4 - 1];
    let (mut width, mut height) = (word(16), word(12));
    dimensions(width, height)?;
    let count = word(28).max(1);
    let maximum = u32::BITS - width.max(height).leading_zeros();
    if word(4) != 124
        || word(76) != 32
        || word(80) != 4
        || &bytes[84..88] != b"DXT5"
        || word(8) & 0x81007 != 0x81007
        || word(8) & !0xa1007 != 0
        || word(24) > 1
        || word(112) != 0
        || word(116) != 0
        || word(120) != 0
        || count > maximum
        || (count > 1 && (word(8) & 0x20000 == 0 || word(108) != 0x401008))
        || (count == 1 && word(108) != 0x1000)
    {
        return Err(unsupported(
            "BC3 mip header outside bounded legacy 2D profile",
        ));
    }
    let mut layout = Bc3MipLayout {
        profile: BC3_MIP_LAYOUT_PROFILE,
        header_words,
        encoded_length: bytes.len(),
        storage_padding,
        rgba_byte_count: 0,
        levels: Vec::new(),
        runtime_verified: false,
    };
    let mut offset = 128;
    for ordinal in 0..count {
        let length = width.div_ceil(4) as usize * height.div_ceil(4) as usize * 16;
        if ordinal == 0 && word(20) as usize != length {
            return Err(invalid("BC3 mip base linear size differs"));
        }
        layout.rgba_byte_count += width as usize * height as usize * 4;
        if layout.rgba_byte_count > MAX_RGBA as usize {
            return Err(invalid("BC3 mip cumulative RGBA exceeds 64 MiB"));
        }
        layout.levels.push(Bc3MipLevelLayout {
            ordinal,
            width,
            height,
            encoded_offset: offset,
            encoded_length: length,
        });
        offset += length;
        width = (width / 2).max(1);
        height = (height / 2).max(1);
    }
    if offset != bytes.len() {
        return Err(invalid(
            "BC3 mip chain length differs or extra surfaces exist",
        ));
    }
    Ok(layout)
}

pub fn decode_bc3_mip_chain(
    bytes: &[u8],
    declared_length: usize,
) -> io::Result<DecodedBc3MipChain> {
    let layout = read_bc3_mip_layout(bytes, declared_length)?;
    let levels_rgba = layout
        .levels
        .iter()
        .map(|level| {
            normalized_bc3_blocks(
                &bytes[level.encoded_offset..level.encoded_offset + level.encoded_length],
                level.width as usize,
                level.height as usize,
            )
        })
        .collect();
    Ok(DecodedBc3MipChain {
        layout,
        pixel_decode_profile: HdPixelDecodeProfile::Bc3NormalizedNearestRgba8V1,
        levels_rgba,
    })
}

// Public BC3 layout/normalized interpolation, followed by nearest UNORM8.
// Qualification: T024 pinned DirectXTex methods + WIC, finite scalar corpus.
// Called only after bounded single-level or complete-chain admission, preserving hidden
// RGB and straight alpha. No gamma transform, premultiplication or filtering.
fn normalized_bc3_blocks(bytes: &[u8], width: usize, height: usize) -> Vec<u8> {
    let nearest = |value: u32, divisor: u32| ((value + divisor / 2) / divisor) as u8;
    let mut rgba = vec![0; width * height * 4];
    for (block_index, block) in bytes.as_chunks::<16>().0.iter().enumerate() {
        let first = u16::from_le_bytes(block[8..10].try_into().unwrap()) as u32;
        let second = u16::from_le_bytes(block[10..12].try_into().unwrap()) as u32;
        let mut colors = [[0u8; 3]; 4];
        for (channel, (mask, shift)) in [(31, 11), (63, 5), (31, 0)].into_iter().enumerate() {
            let (a, b) = ((first >> shift) & mask, (second >> shift) & mask);
            for (selector, (n, m, divisor)) in [(1, 0, 1), (0, 1, 1), (2, 1, 3), (1, 2, 3)]
                .into_iter()
                .enumerate()
            {
                colors[selector][channel] = nearest((n * a + m * b) * 255, divisor * mask);
            }
        }
        let mut alphas = [0u8; 8];
        alphas[..2].copy_from_slice(&block[..2]);
        let (divisor, end) = if block[0] > block[1] { (7, 8) } else { (5, 6) };
        for (selector, alpha) in alphas.iter_mut().enumerate().take(end).skip(2) {
            *alpha = nearest(
                (divisor + 1 - selector as u32) * block[0] as u32
                    + (selector as u32 - 1) * block[1] as u32,
                divisor,
            );
        }
        if end == 6 {
            alphas[7] = 255;
        }
        let colors_index = u32::from_le_bytes(block[12..16].try_into().unwrap());
        let mut alpha_bytes = [0u8; 8];
        alpha_bytes[..6].copy_from_slice(&block[2..8]);
        let alphas_index = u64::from_le_bytes(alpha_bytes);
        for p in 0..16 {
            let x = (block_index % width.div_ceil(4)) * 4 + p % 4;
            let y = (block_index / width.div_ceil(4)) * 4 + p / 4;
            if x >= width || y >= height {
                continue;
            }
            let position = (y * width + x) * 4;
            rgba[position..position + 3]
                .copy_from_slice(&colors[((colors_index >> (p * 2)) & 3) as usize]);
            rgba[position + 3] = alphas[((alphas_index >> (p * 3)) & 7) as usize];
        }
    }
    rgba
}

impl DecodedHdImage {
    pub fn write_preview(&self, writer: &mut impl Write) -> io::Result<()> {
        crate::font::write_rgba_preview(
            writer,
            self.width as usize,
            self.height as usize,
            &self.pixels_rgba,
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use image::{ExtendedColorType, ImageEncoder, codecs::png::PngEncoder};

    fn decode(bytes: &[u8]) -> io::Result<DecodedHdImage> {
        decode_hd_image(bytes, bytes.len())
    }

    fn png() -> Vec<u8> {
        let mut bytes = Vec::new();
        PngEncoder::new(&mut bytes)
            .write_image(
                &[1, 2, 3, 0, 200, 100, 50, 128],
                2,
                1,
                ExtendedColorType::Rgba8,
            )
            .unwrap();
        bytes
    }
    fn chunk(kind: &[u8; 4], data: &[u8]) -> Vec<u8> {
        let mut bytes = (data.len() as u32).to_be_bytes().to_vec();
        bytes.extend(kind);
        bytes.extend(data);
        bytes.extend(crc32fast::hash(&bytes[4..]).to_be_bytes());
        bytes
    }
    fn dds() -> Vec<u8> {
        let mut bytes = vec![0; 144];
        bytes[..4].copy_from_slice(b"DDS ");
        for (offset, value) in [
            (4, 124u32),
            (8, 0x81007),
            (12, 4),
            (16, 4),
            (20, 16),
            (28, 1),
            (76, 32),
            (80, 4),
            (108, 0x1000),
        ] {
            bytes[offset..offset + 4].copy_from_slice(&value.to_le_bytes());
        }
        bytes[84..88].copy_from_slice(b"DXT5");
        bytes[128] = 255;
        bytes[136..138].copy_from_slice(&0xf800u16.to_le_bytes());
        bytes
    }

    fn bgra() -> Vec<u8> {
        let mut bytes = dds();
        for (offset, value) in [
            (8, 0x2100fu32),
            (12, 2),
            (16, 3),
            (20, 12),
            (80, 0x41),
            (84, 0),
            (88, 32),
            (92, 0x00ff_0000),
            (96, 0x0000_ff00),
            (100, 0xff),
            (104, 0xff00_0000),
        ] {
            bytes[offset..offset + 4].copy_from_slice(&value.to_le_bytes());
        }
        bytes.truncate(128);
        bytes.extend([
            3, 6, 9, 0, 10, 20, 30, 1, 40, 50, 60, 127, 70, 80, 90, 128, 100, 110, 120, 254, 130,
            140, 150, 255,
        ]);
        bytes
    }

    fn mip_dds(mut width: u32, mut height: u32, count: u32) -> Vec<u8> {
        let mut bytes = dds();
        for (offset, value) in [
            (8, if count > 1 { 0xa1007 } else { 0x81007 }),
            (12, height),
            (16, width),
            (20, width.div_ceil(4) * height.div_ceil(4) * 16),
            (28, count),
            (108, if count > 1 { 0x401008 } else { 0x1000 }),
        ] {
            bytes[offset..offset + 4].copy_from_slice(&value.to_le_bytes());
        }
        bytes.truncate(128);
        for ordinal in 0..count.max(1) {
            for block in 0..width.div_ceil(4) * height.div_ceil(4) {
                let mut data = [0u8; 16];
                data[0] = 255;
                let color = if (block + ordinal).is_multiple_of(2) {
                    0xf800u16
                } else {
                    0x07e0
                };
                data[8..10].copy_from_slice(&color.to_le_bytes());
                // Alternating endpoint colors across columns exposes clipping/row stride.
                data[10..12].copy_from_slice(&0x001fu16.to_le_bytes());
                data[12..16].copy_from_slice(&0x44444444u32.to_le_bytes());
                bytes.extend(data);
            }
            width = (width / 2).max(1);
            height = (height / 2).max(1);
        }
        bytes
    }

    #[test]
    fn bc3_complete_chain_retains_layout_and_clips_partial_blocks() {
        let source = mip_dds(5, 7, 3);
        let chain = decode_bc3_mip_chain(&source, source.len()).unwrap();
        assert_eq!(chain.layout.rgba_byte_count, (35 + 6 + 1) * 4);
        assert_eq!(chain.layout.header_words[6], 3);
        assert!(!chain.layout.runtime_verified);
        assert_eq!(
            chain.pixel_decode_profile,
            HdPixelDecodeProfile::Bc3NormalizedNearestRgba8V1
        );
        let shapes: Vec<_> = chain
            .layout
            .levels
            .iter()
            .map(|m| {
                (
                    m.ordinal,
                    m.width,
                    m.height,
                    m.encoded_offset,
                    m.encoded_length,
                )
            })
            .collect();
        assert_eq!(
            shapes,
            [(0, 5, 7, 128, 64), (1, 2, 3, 192, 16), (2, 1, 1, 208, 16)]
        );
        for (mip, pixels) in chain.layout.levels.iter().zip(&chain.levels_rgba) {
            assert_eq!(pixels.len(), mip.width as usize * mip.height as usize * 4);
            for y in 0..mip.height {
                for x in 0..mip.width {
                    let block = y / 4 * mip.width.div_ceil(4) + x / 4;
                    let expected = if x % 2 == 1 {
                        [0, 0, 255, 255]
                    } else if (block + mip.ordinal).is_multiple_of(2) {
                        [255, 0, 0, 255]
                    } else {
                        [0, 255, 0, 255]
                    };
                    let position = (y * mip.width + x) as usize * 4;
                    assert_eq!(&pixels[position..position + 4], &expected);
                }
            }
        }
        assert!(decode(&source).is_err());
    }

    #[test]
    fn bc3_mip_header_counts_lengths_surfaces_and_padding_are_strict() {
        let source = mip_dds(4, 4, 3);
        assert_eq!(
            read_bc3_mip_layout(&source, source.len())
                .unwrap()
                .levels
                .len(),
            3
        );
        for (offset, word) in [
            (4, 123u32),
            (76, 31),
            (80, 5),
            (8, 0x81007),
            (8, 0x8a1007),
            (20, 32),
            (24, 2),
            (28, 4),
            (28, 2),
            (108, 0x1000),
            (112, 0x200),
            (116, 1),
            (120, 1),
            (16, 4097),
            (12, 0),
        ] {
            let mut changed = source.clone();
            changed[offset..offset + 4].copy_from_slice(&word.to_le_bytes());
            assert!(
                read_bc3_mip_layout(&changed, changed.len()).is_err(),
                "offset {offset}, word {word}"
            );
        }
        for length in [0, 127, source.len() - 1, source.len() + 1] {
            assert!(read_bc3_mip_layout(&source, length).is_err());
        }
        let mut extra = source.clone();
        extra.push(0);
        assert!(read_bc3_mip_layout(&extra, extra.len()).is_err());
        let mut single = mip_dds(1, 1, 0);
        let layout = read_bc3_mip_layout(&single, single.len()).unwrap();
        assert_eq!(layout.header_words[6], 0);
        assert_eq!(layout.levels.len(), 1);
        let logical = single.len();
        single.resize(logical + 16, 0xa5);
        assert!(read_bc3_mip_layout(&single, logical).is_err());
        let mut misaligned = source.clone();
        misaligned.truncate(source.len() - 1);
        misaligned.resize(source.len(), 0xa5);
        assert!(read_bc3_mip_layout(&misaligned, source.len() - 1).is_err());
    }

    #[test]
    fn bc3_mip_cumulative_budget_rejects_before_pixel_decode() {
        // A valid compressed chain fits the source bound but RGBA expansion does not.
        let source = mip_dds(4096, 4096, 13);
        assert!(source.len() < MAX_INPUT);
        assert_eq!(
            decode_bc3_mip_chain(&source, source.len())
                .err()
                .unwrap()
                .to_string(),
            "BC3 mip cumulative RGBA exceeds 64 MiB"
        );
    }

    #[test]
    fn bgra_rows_preserve_channels_hidden_rgb_and_storage_padding() {
        let mut source = bgra();
        let length = source.len();
        source.resize(length.next_multiple_of(16), 0xa5);
        for requested in [
            HdPixelDecodeProfile::default(),
            HdPixelDecodeProfile::Bc3NormalizedNearestRgba8V1,
            HdPixelDecodeProfile::LegacyDdsBgra8StraightRgba8V1,
        ] {
            let image = decode_hd_image_with_profile(&source, length, requested).unwrap();
            assert_eq!(image.format, HdImageFormat::DdsBgra8);
            assert_eq!(
                image.pixel_decode_profile,
                HdPixelDecodeProfile::LegacyDdsBgra8StraightRgba8V1
            );
            assert_eq!(
                (
                    image.width,
                    image.height,
                    image.encoded_length,
                    image.storage_padding
                ),
                (3, 2, length, 8)
            );
            assert_eq!(
                image.pixels_rgba,
                [
                    9, 6, 3, 0, 30, 20, 10, 1, 60, 50, 40, 127, 90, 80, 70, 128, 120, 110, 100,
                    254, 150, 140, 130, 255
                ]
            );
        }
    }

    #[test]
    fn bgra_rejects_unknown_masks_pitch_flags_levels_surfaces_and_lengths() {
        let source = bgra();
        for (offset, value) in [
            (4, 123u32),
            (76, 31),
            (80, 0x40),
            (80, 0x45),
            (84, 1),
            (88, 24),
            (92, 0xff),
            (96, 0xff),
            (100, 0xff0000),
            (104, 0),
            (8, 0x21007),
            (8, 0x82100f),
            (20, 16),
            (24, 2),
            (28, 2),
            (108, 0x401008),
            (112, 0x200),
            (116, 1),
            (120, 1),
            (16, 4097),
            (12, 0),
        ] {
            let mut data = source.clone();
            data[offset..offset + 4].copy_from_slice(&value.to_le_bytes());
            assert!(decode(&data).is_err(), "offset {offset}, word {value}");
        }
        assert!(decode(&source[..source.len() - 1]).is_err());
        // Required flags only, with the tight pitch still stored.
        let mut no_pitch_flag = source.clone();
        no_pitch_flag[8..12].copy_from_slice(&0x1007u32.to_le_bytes());
        assert!(decode(&no_pitch_flag).is_ok());
        let mut extra = source;
        extra.push(0);
        assert!(decode(&extra).is_err());
    }

    #[test]
    fn bgra_selection_cannot_silently_choose_bc3_arithmetic() {
        let requested = HdPixelDecodeProfile::LegacyDdsBgra8StraightRgba8V1;
        let source = dds();
        assert_eq!(
            decode_hd_image_with_profile(&source, source.len(), requested)
                .err()
                .unwrap()
                .kind(),
            io::ErrorKind::Unsupported
        );
        let source = png();
        assert_eq!(
            decode_hd_image_with_profile(&source, source.len(), requested)
                .unwrap()
                .pixel_decode_profile,
            HdPixelDecodeProfile::default()
        );
    }

    #[test]
    fn png_preserves_straight_samples_and_separates_storage_padding() {
        let mut bytes = png();
        let encoded = bytes.len();
        bytes.resize(encoded.next_multiple_of(16), 0xab);
        let decoded = decode_hd_image(&bytes, encoded).unwrap();
        assert_eq!(decoded.format, HdImageFormat::Png);
        assert_eq!(
            (
                decoded.width,
                decoded.height,
                decoded.encoded_length,
                decoded.storage_padding
            ),
            (2, 1, encoded, bytes.len() - encoded)
        );
        assert_eq!(decoded.pixels_rgba, [1, 2, 3, 0, 200, 100, 50, 128]);
        assert!(decode_hd_image(&bytes, bytes.len() + 1).is_err());
        bytes.extend([0; 16]);
        assert!(decode_hd_image(&bytes, encoded).is_err());
    }

    #[test]
    fn png_rejects_crc_truncation_animation_unknown_critical_and_oversized_headers() {
        let source = png();
        for prefix in [0, 7, 15, 32, source.len() - 1] {
            assert!(decode(&source[..prefix]).is_err());
        }
        let mut corrupt = source.clone();
        corrupt[29] ^= 1;
        assert!(decode(&corrupt).is_err());
        for kind in [b"acTL", b"ABCD"] {
            let mut bytes = source[..33].to_vec();
            bytes.extend(chunk(kind, &[0; 8]));
            bytes.extend(&source[33..]);
            assert_eq!(
                decode(&bytes).err().unwrap().kind(),
                io::ErrorKind::Unsupported
            );
        }
        let mut oversized = source.clone();
        oversized[16..20].copy_from_slice(&(MAX_SIDE + 1).to_be_bytes());
        let crc = crc32fast::hash(&oversized[12..29]);
        oversized[29..33].copy_from_slice(&crc.to_be_bytes());
        assert!(decode(&oversized).is_err());
        let mut high_depth = source;
        high_depth[24] = 16;
        let crc = crc32fast::hash(&high_depth[12..29]);
        high_depth[29..33].copy_from_slice(&crc.to_be_bytes());
        assert_eq!(
            decode(&high_depth).err().unwrap().kind(),
            io::ErrorKind::Unsupported
        );
    }

    #[test]
    fn bc3_block_and_container_admission_are_separate() {
        let source = dds();
        let decoded = decode(&source).unwrap();
        assert_eq!(
            (decoded.width, decoded.height, decoded.storage_padding),
            (4, 4, 0)
        );
        assert_eq!(decoded.pixels_rgba, [255, 0, 0, 255].repeat(16));
        assert!(decode(&source[..143]).is_err());
        let mut tail = source.clone();
        tail.push(0);
        assert!(decode(&tail).is_err());
        for (offset, value) in [
            (4, 123u32),
            (12, 0),
            (16, u32::MAX),
            (28, 2),
            (112, 0x200),
            (16, 3),
        ] {
            let mut bytes = source.clone();
            bytes[offset..offset + 4].copy_from_slice(&value.to_le_bytes());
            assert!(decode(&bytes).is_err());
        }
        let mut unknown = source;
        unknown[84..88].copy_from_slice(b"DX10");
        assert_eq!(
            decode(&unknown).err().unwrap().kind(),
            io::ErrorKind::Unsupported
        );
    }

    #[test]
    fn explicit_bc3_profile_distinguishes_endpoint_and_alpha_quantization() {
        let mut bytes = dds();
        bytes[128] = 1;
        bytes[129] = 0;
        let mut indices = 0u64;
        for p in 0..16 {
            indices |= (p as u64 % 8) << (p * 3);
        }
        bytes[130..136].copy_from_slice(&indices.to_le_bytes()[..6]);
        bytes[136..138].copy_from_slice(&(3u16 << 11).to_le_bytes());
        bytes[138..140].copy_from_slice(&(3u16 << 11).to_le_bytes());
        let legacy = decode(&bytes).unwrap();
        let normalized = decode_hd_image_with_profile(
            &bytes,
            bytes.len(),
            HdPixelDecodeProfile::Bc3NormalizedNearestRgba8V1,
        )
        .unwrap();
        assert_eq!(&legacy.pixels_rgba[8..12], &[24, 0, 0, 0]);
        assert_eq!(&normalized.pixels_rgba[8..12], &[25, 0, 0, 1]);
        assert_eq!(
            legacy.pixel_decode_profile,
            HdPixelDecodeProfile::Image02510StoredRgba8
        );
        assert_eq!(
            normalized.pixel_decode_profile,
            HdPixelDecodeProfile::Bc3NormalizedNearestRgba8V1
        );
    }

    #[test]
    fn normalized_bc3_preserves_four_colors_hidden_rgb_and_equal_alpha_branch() {
        let mut bytes = dds();
        bytes[128..144].copy_from_slice(&[
            0, 0, 0x88, 0xc6, 0xfa, 0x88, 0xc6, 0xfa, 0, 0, 0, 0xf8, 0xe4, 0xe4, 0xe4, 0xe4,
        ]);
        let normalized = decode_hd_image_with_profile(
            &bytes,
            bytes.len(),
            HdPixelDecodeProfile::Bc3NormalizedNearestRgba8V1,
        )
        .unwrap();
        for (p, pixel) in normalized.pixels_rgba.as_chunks::<4>().0.iter().enumerate() {
            assert_eq!(
                *pixel,
                [
                    [0, 0, 0, 0],
                    [255, 0, 0, 0],
                    [85, 0, 0, 0],
                    [170, 0, 0, 0],
                    [0, 0, 0, 0],
                    [255, 0, 0, 0],
                    [85, 0, 0, 0],
                    [170, 0, 0, 255]
                ][p % 8]
            );
        }
    }

    #[test]
    fn profiles_share_admission_and_png_always_reports_its_actual_decoder() {
        let profile = HdPixelDecodeProfile::Bc3NormalizedNearestRgba8V1;
        let mut bytes = png();
        let logical = bytes.len();
        bytes.resize(logical.next_multiple_of(16), 0xab);
        let legacy = decode_hd_image(&bytes, logical).unwrap();
        let overridden = decode_hd_image_with_profile(&bytes, logical, profile).unwrap();
        assert_eq!(legacy.pixels_rgba, overridden.pixels_rgba);
        assert_eq!(
            overridden.pixel_decode_profile,
            HdPixelDecodeProfile::default()
        );
        let mut invalid_dds = dds();
        invalid_dds[28..32].copy_from_slice(&2u32.to_le_bytes());
        for bytes in [&invalid_dds[..], &invalid_dds[..143], b"unknown".as_slice()] {
            let legacy = decode_hd_image(bytes, bytes.len()).err().unwrap();
            let normalized = decode_hd_image_with_profile(bytes, bytes.len(), profile)
                .err()
                .unwrap();
            assert_eq!(legacy.kind(), normalized.kind());
        }
        assert!(serde_json::from_str::<HdPixelDecodeProfile>("\"unqualified_gpu\"").is_err());
    }
}
