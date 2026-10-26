//! Effect sources for the test room: object PAX containers (BAR type 18
//! of `obj/<model>.a.us`) and the resident common atlas
//! (`effect/texcommon.dpd` texture 10, TBP 10752), with the PC remaster's
//! replacement for the atlas when present.
use data::{
    bar, hd_image, object_effect, original_payload, probe_source::ProbeSource, replacement_payload,
};
use std::io;

/// Texture base pointer of the common effect atlas (texcommon #10).
pub const ATLAS_TBP: u16 = 10752;

pub struct Atlas {
    pub rgba: Vec<u8>,
    pub size: [u32; 2],
    /// The original texture's size: sprite texel coordinates normalise by it.
    pub texel_size: [u32; 2],
    pub hd: bool,
    /// Mean absolute RGBA difference between the original texture and the
    /// replacement's first rows (alignment evidence for the log).
    pub alignment: Option<f64>,
}

fn read_entry(
    source: &ProbeSource,
    index: &str,
    ordinal: usize,
    name_hash: [u8; 16],
    replacements: bool,
) -> Result<(Vec<u8>, Vec<replacement_payload::ReplacementSample>), Box<dyn std::error::Error>> {
    let mut checked = source.open_index(index)?;
    let entry = checked
        .inventory
        .entries
        .get(ordinal)
        .filter(|e| e.name_hash == name_hash)
        .ok_or("effect source entry mismatch")?;
    let bytes = original_payload::read_original(&mut checked.reader, entry)?.bytes;
    let hd = if replacements {
        replacement_payload::read_all_replacements(&mut checked.reader, entry)?
    } else {
        Vec::new()
    };
    checked.verify_unchanged()?;
    Ok((bytes, hd))
}

/// The PAX of an object archive, with its textures' remaster pages.
pub fn load_pax(
    source: &ProbeSource,
    index: &str,
    ordinal: usize,
    name_hash: [u8; 16],
) -> Result<object_effect::Pax, Box<dyn std::error::Error>> {
    let (archive, hd) = read_entry(source, index, ordinal, name_hash, true)?;
    let table = bar::read_bar(&archive)?;
    let e = table
        .entries
        .iter()
        .find(|e| e.entry_type == 18 && e.size > 0)
        .ok_or("object archive has no PAX")?;
    let bytes = archive
        .get(e.offset as usize..e.offset as usize + e.size as usize)
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, "PAX exceeds archive"))?;
    let mut pax = object_effect::read_pax(bytes)?;
    let at = e.offset as usize;
    pax.attach_hd(at, &object_effect::hd_pages_within(&hd, at, bytes.len()));
    Ok(pax)
}

/// texcommon.dpd texture 10, from its first replacement (keyed to that
/// texture's data offset) when it decodes, else the original PSMT8.
pub fn load_atlas(
    source: &ProbeSource,
    index: &str,
    ordinal: usize,
    name_hash: [u8; 16],
) -> Result<Atlas, Box<dyn std::error::Error>> {
    let (bytes, hd) = read_entry(source, index, ordinal, name_hash, true)?;
    let dpd = object_effect::read_dpd(&bytes)?;
    let original = dpd.textures.get(10).ok_or("texcommon has no texture 10")?;
    let texel_size = [original.width, original.height];
    if let Some(r) = hd.first()
        && let Ok(decoded) =
            hd_image::decode_hd_image(&r.bytes, r.metadata.declared_decompressed_length as usize)
        && decoded.width == original.width
    {
        let n = original.rgba.len().min(decoded.pixels_rgba.len());
        let sum: u64 = original.rgba[..n]
            .iter()
            .zip(&decoded.pixels_rgba[..n])
            .map(|(a, b)| a.abs_diff(*b) as u64)
            .sum();
        return Ok(Atlas {
            alignment: Some(sum as f64 / n.max(1) as f64),
            rgba: decoded.pixels_rgba,
            size: [decoded.width, decoded.height],
            texel_size,
            hd: true,
        });
    }
    Ok(Atlas {
        rgba: original.rgba.clone(),
        size: texel_size,
        texel_size,
        hd: false,
        alignment: None,
    })
}
