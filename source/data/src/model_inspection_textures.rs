//! Explicit texture/UV inspection choices; no installed material or animation.
use crate::{
    hd_image::{self, HdPixelDecodeProfile},
    model_geometry::Geometry,
    model_texture_routing::{self, PartTextureRoute, TargetKind, TextureLinks},
    replacement_payload::ReplacementSample,
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::io;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum TextureMode {
    PartColors,
    OriginalIndexed,
    ExactHdBase,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
pub enum UvProfile {
    #[serde(rename = "openkh_kkdf2_integer_truncated_uv_v1")]
    Kkdf2IntegerTruncated,
    #[serde(rename = "openkh_legacy_signed16_uv_on_kkdf2_topology_v1")]
    LegacySigned16OnKkdf2Topology,
}
impl UvProfile {
    fn components(self, [u, v]: [i32; 2]) -> [u32; 2] {
        [u, v].map(|word| match self {
            Self::Kkdf2IntegerTruncated => ((word / 16) as f32 / 256.0).to_bits(),
            Self::LegacySigned16OnKkdf2Topology => (word as u16 as i16 as f32 / 4096.0).to_bits(),
        })
    }
}
#[derive(Serialize)]
pub struct UvRoutes {
    pub profile: UvProfile,
    pub parts: Vec<PartTextureRoute>,
}

/// Only UV expressions vary. The qualified Kkdf2 positions/topology stay intact.
pub fn read_uv_routes(
    payload: &[u8],
    geometry: &Geometry,
    environments: usize,
    profiles: &[UvProfile],
) -> io::Result<Vec<UvRoutes>> {
    if profiles.is_empty()
        || profiles.len() > 2
        || (profiles.len() == 2 && profiles[0] == profiles[1])
    {
        return Err(invalid("UV profiles must be explicit and distinct"));
    }
    let groups = crate::model_packets::read_packet_groups(payload)?;
    let mut pools: Vec<_> = profiles.iter().map(|_| Vec::new()).collect();
    let mut ordinal = 0;
    for group in groups
        .iter()
        .filter(|g| g.block_offset == geometry.block_offset)
    {
        for (packet_ordinal, packet) in group.packets.iter().enumerate() {
            let data = packet
                .vpu_data
                .as_ref()
                .ok_or_else(|| invalid("UV inspection requires type-1 root packets"))?;
            let part = geometry
                .parts
                .get(ordinal)
                .ok_or_else(|| invalid("extra UV packet"))?;
            if part.group_ordinal != group.group_ordinal
                || part.packet_ordinal != packet_ordinal
                || part.uv_bits.len() != data.strips.len()
                || part
                    .uv_base
                    .checked_add(data.strips.len())
                    .is_none_or(|n| n > 262_144)
            {
                return Err(invalid("UV packet/geometry identity or budget differs"));
            }
            for (profile, pool) in profiles.iter().zip(&mut pools) {
                if part.uv_base != pool.len() {
                    return Err(invalid("UV packet bases are not contiguous"));
                }
                pool.extend(data.strips.iter().map(|v| profile.components([v[0], v[1]])));
            }
            ordinal += 1;
        }
    }
    if ordinal != geometry.parts.len() {
        return Err(invalid("UV packet count differs"));
    }
    profiles
        .iter()
        .zip(pools)
        .map(|(&profile, pool)| {
            Ok(UvRoutes {
                profile,
                parts: model_texture_routing::associate_geometry_with_uv_pool(
                    geometry,
                    environments,
                    &pool,
                )?,
            })
        })
        .collect()
}

#[derive(Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum ImageIdentity {
    Original { environment_ordinal: usize },
    Replacement { replacement_ordinal: u32 },
}
#[derive(Serialize)]
pub struct TextureLevel {
    pub width: u32,
    pub height: u32,
    pub pixels_rgba_sha256: String,
    #[serde(skip_serializing)]
    pub pixels_rgba: Vec<u8>,
}
impl TextureLevel {
    fn new(width: u32, height: u32, pixels_rgba: Vec<u8>) -> Self {
        Self {
            width,
            height,
            pixels_rgba_sha256: format!("{:x}", Sha256::digest(&pixels_rgba)),
            pixels_rgba,
        }
    }
}
#[derive(Serialize)]
pub struct TextureImage {
    pub identity: ImageIdentity,
    pub pixel_decode_profile: serde_json::Value,
    pub mip_layout: Option<hd_image::Bc3MipLayout>,
    pub levels: Vec<TextureLevel>,
}
#[derive(Serialize)]
pub struct InspectionTextures {
    pub texture_bar_ordinal: usize,
    pub modes: Vec<TextureMode>,
    pub images: Vec<TextureImage>,
    pub links: TextureLinks,
    /// Playable TEXA patches with the palette of the texture they paint.
    #[serde(skip)]
    pub play: Vec<TexturePlay>,
}
/// A TEXA patch and the CLUT (256 RGBA entries) of its target environment.
pub struct TexturePlay {
    pub patch: crate::model_texture_play::Patch,
    pub clut_rgba: Vec<u8>,
}
fn invalid(message: &str) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, message)
}
fn reserve(bytes: &mut usize, additional: usize) -> io::Result<()> {
    *bytes = bytes
        .checked_add(additional)
        .filter(|&n| n <= 128 * 1024 * 1024)
        .ok_or_else(|| invalid("inspection texture cumulative RGBA exceeds 128 MiB"))?;
    Ok(())
}
/// Read original environments and EVERY replacement, retaining complete chains.
/// Global expansion is admitted before pixels allocate; raw headers alone do
/// not authorize a format. Existing decoders still perform their full admission.
pub fn read_textures(
    payload: &[u8],
    asset_offset: usize,
    texture_bar_ordinal: usize,
    replacements: &[ReplacementSample],
    modes: &[TextureMode],
    total_rgba: &mut usize,
) -> io::Result<InspectionTextures> {
    let layout = crate::model_texture::read_layout(payload)?;
    if layout.images.is_empty() || layout.images.len() + replacements.len() > 64 {
        return Err(invalid("inspection texture count outside 1..64"));
    }
    let original_bytes: usize = layout
        .images
        .iter()
        .map(|i| i.width as usize * i.height as usize * 4)
        .sum();
    reserve(total_rgba, original_bytes)?;
    let metadata: Vec<_> = replacements.iter().map(|r| r.metadata.clone()).collect();
    let links = model_texture_routing::read_links(payload, asset_offset, &metadata)?;
    let decoded = crate::model_texture_pixels::decode_images(payload)?;
    let play = crate::model_texture_play::read_patches(payload)?
        .into_iter()
        .filter_map(|patch| {
            let target = decoded
                .iter()
                .find(|d| d.environment_ordinal == patch.environment)?;
            (target.indexed_bits_per_pixel == 8
                && patch.x + patch.width <= target.width
                && patch.y + patch.height <= target.height)
                .then(|| TexturePlay {
                    clut_rgba: target.clut_rgba.clone(),
                    patch,
                })
        })
        .collect();
    let mut images: Vec<_> = decoded
        .into_iter()
        .map(|i| TextureImage {
            identity: ImageIdentity::Original {
                environment_ordinal: i.environment_ordinal,
            },
            pixel_decode_profile: crate::model_texture_pixels::PROFILE.into(),
            mip_layout: None,
            levels: vec![TextureLevel::new(i.width, i.height, i.pixels_rgba)],
        })
        .collect();
    for replacement in replacements {
        let bytes = &replacement.bytes;
        let declared = replacement.metadata.declared_decompressed_length as usize;
        let count = bytes
            .get(28..32)
            .map(|v| u32::from_le_bytes(v.try_into().unwrap()));
        let (levels, mip_layout, pixel_decode_profile) =
            if bytes.starts_with(b"DDS ") && count.is_some_and(|n| n > 1) {
                let layout = hd_image::read_bc3_mip_layout(bytes, declared)?;
                reserve(total_rgba, layout.rgba_byte_count)?;
                let chain = hd_image::decode_bc3_mip_chain(bytes, declared)?;
                let levels = chain
                    .layout
                    .levels
                    .iter()
                    .zip(chain.levels_rgba)
                    .map(|(m, p)| TextureLevel::new(m.width, m.height, p))
                    .collect();
                (
                    levels,
                    Some(chain.layout),
                    serde_json::to_value(chain.pixel_decode_profile)?,
                )
            } else {
                let (width, height) = if bytes.starts_with(b"DDS ") && bytes.len() >= 128 {
                    (
                        u32::from_le_bytes(bytes[16..20].try_into().unwrap()),
                        u32::from_le_bytes(bytes[12..16].try_into().unwrap()),
                    )
                } else if bytes.starts_with(b"\x89PNG\r\n\x1a\n") && bytes.len() >= 33 {
                    (
                        u32::from_be_bytes(bytes[16..20].try_into().unwrap()),
                        u32::from_be_bytes(bytes[20..24].try_into().unwrap()),
                    )
                } else {
                    return Err(invalid(
                        "replacement texture header is incomplete or unknown",
                    ));
                };
                if !(1..=hd_image::MAX_SIDE).contains(&width)
                    || !(1..=hd_image::MAX_SIDE).contains(&height)
                {
                    return Err(invalid(&format!(
                        "replacement inspection dimensions outside bound ({width}x{height})"
                    )));
                }
                reserve(total_rgba, width as usize * height as usize * 4)?;
                let decoded = hd_image::decode_hd_image_with_profile(
                    bytes,
                    declared,
                    HdPixelDecodeProfile::Bc3NormalizedNearestRgba8V1,
                )?;
                if decoded.width != width
                    || decoded.height != height
                    || decoded.pixels_rgba.len() != width as usize * height as usize * 4
                {
                    return Err(invalid("decoded texture differs from allocation admission"));
                }
                (
                    vec![TextureLevel::new(width, height, decoded.pixels_rgba)],
                    None,
                    serde_json::to_value(decoded.pixel_decode_profile)?,
                )
            };
        images.push(TextureImage {
            identity: ImageIdentity::Replacement {
                replacement_ordinal: replacement.metadata.ordinal,
            },
            pixel_decode_profile,
            mip_layout,
            levels,
        });
    }
    Ok(InspectionTextures {
        texture_bar_ordinal,
        modes: modes.to_vec(),
        images,
        links,
        play,
    })
}

#[derive(Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum BindingStatus {
    PartColors,
    OriginalIndexed,
    ExactHdBase,
    UnmatchedHdBase,
    AmbiguousHdBase,
}
#[derive(Serialize)]
pub struct TextureBinding {
    pub status: BindingStatus,
    pub image_index: Option<usize>,
    pub environment_ordinal: usize,
}
impl InspectionTextures {
    /// The environment's HD replacement when exactly one replacement matches
    /// it (as `ExactHdBase` binds), else its original image: the PC draws
    /// the remastered texture where the install has one.
    pub fn bind_hd_or_original(&self, environment: usize) -> io::Result<TextureBinding> {
        if let Ok(binding) = self.bind(environment, TextureMode::ExactHdBase)
            && binding.status == BindingStatus::ExactHdBase
        {
            return Ok(binding);
        }
        self.bind(environment, TextureMode::OriginalIndexed)
    }

    pub fn bind(&self, environment: usize, mode: TextureMode) -> io::Result<TextureBinding> {
        let mut binding = TextureBinding {
            status: BindingStatus::PartColors,
            image_index: None,
            environment_ordinal: environment,
        };
        match mode {
            TextureMode::PartColors => {}
            TextureMode::OriginalIndexed => {
                binding.status = BindingStatus::OriginalIndexed;
                binding.image_index=self.images.iter().position(|i|matches!(i.identity,ImageIdentity::Original {environment_ordinal} if environment_ordinal==environment));
                if binding.image_index.is_none() {
                    return Err(invalid("original environment image missing"));
                }
            }
            TextureMode::ExactHdBase => {
                let target = self
                    .links
                    .ordered_targets
                    .iter()
                    .find(|t| {
                        matches!(t.kind, TargetKind::Base) && t.environment_ordinal == environment
                    })
                    .ok_or_else(|| invalid("base texture target missing"))?;
                binding.status = if target.matching_replacement_ordinals.is_empty() {
                    BindingStatus::UnmatchedHdBase
                } else if target.matching_replacement_ordinals.len() != 1
                    || self
                        .links
                        .duplicate_candidate_offsets
                        .contains(&target.candidate_original_asset_offset)
                {
                    BindingStatus::AmbiguousHdBase
                } else {
                    BindingStatus::ExactHdBase
                };
                if binding.status == BindingStatus::ExactHdBase {
                    let ordinal = target.matching_replacement_ordinals[0];
                    binding.image_index=self.images.iter().position(|i|matches!(i.identity,ImageIdentity::Replacement {replacement_ordinal} if replacement_ordinal==ordinal));
                    if binding.image_index.is_none() {
                        return Err(invalid("exact replacement image missing"));
                    }
                }
            }
        }
        Ok(binding)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn explicit_uv_expressions_retain_truncation_sign_and_low_bits() {
        assert_eq!(
            UvProfile::Kkdf2IntegerTruncated.components([15, -15]),
            [0.0f32.to_bits(); 2]
        );
        assert_eq!(
            UvProfile::LegacySigned16OnKkdf2Topology.components([15, -15]),
            [(15.0f32 / 4096.0).to_bits(), (-15.0f32 / 4096.0).to_bits()]
        );
        assert_eq!(
            UvProfile::LegacySigned16OnKkdf2Topology.components([65535, 32768]),
            [(-1.0f32 / 4096.0).to_bits(), (-8.0f32).to_bits()]
        );
    }
    fn textures() -> InspectionTextures {
        InspectionTextures {
            texture_bar_ordinal: 0,
            modes: vec![TextureMode::ExactHdBase],
            images: vec![TextureImage {
                identity: ImageIdentity::Original {
                    environment_ordinal: 0,
                },
                pixel_decode_profile: "authored".into(),
                mip_layout: None,
                levels: vec![TextureLevel::new(1, 1, vec![1, 2, 3, 4])],
            }],
            links: TextureLinks {
                profile: model_texture_routing::PROFILE,
                texa_headers: vec![],
                ordered_targets: vec![model_texture_routing::Target {
                    kind: TargetKind::Base,
                    environment_ordinal: 0,
                    candidate_original_asset_offset: 0x20000100,
                    matching_replacement_ordinals: vec![],
                }],
                unmatched_replacement_ordinals: vec![0],
                duplicate_candidate_offsets: vec![],
                runtime_verified: false,
            },
            play: Vec::new(),
        }
    }
    #[test]
    fn hd_or_original_takes_the_exact_replacement_else_the_original() {
        let mut t = textures();
        let fallback = t.bind_hd_or_original(0).unwrap();
        assert_eq!(fallback.status, BindingStatus::OriginalIndexed);
        assert_eq!(fallback.image_index, Some(0));
        t.images.push(TextureImage {
            identity: ImageIdentity::Replacement {
                replacement_ordinal: 7,
            },
            pixel_decode_profile: "authored".into(),
            mip_layout: None,
            levels: vec![TextureLevel::new(2, 2, vec![0; 16])],
        });
        t.links.ordered_targets[0].matching_replacement_ordinals = vec![7];
        let hd = t.bind_hd_or_original(0).unwrap();
        assert_eq!(hd.status, BindingStatus::ExactHdBase);
        assert_eq!(hd.image_index, Some(1));
        // Two candidates: ambiguous, so the original.
        t.links.ordered_targets[0].matching_replacement_ordinals = vec![7, 8];
        assert_eq!(t.bind_hd_or_original(0).unwrap().image_index, Some(0));
    }

    #[test]
    fn unresolved_hd_never_falls_back_to_original_or_an_ordinal() {
        let mut t = textures();
        let missing = t.bind(0, TextureMode::ExactHdBase).unwrap();
        assert_eq!(missing.status, BindingStatus::UnmatchedHdBase);
        assert!(missing.image_index.is_none());
        assert_eq!(
            t.bind(0, TextureMode::OriginalIndexed).unwrap().image_index,
            Some(0)
        );
        t.links.ordered_targets[0].matching_replacement_ordinals = vec![0, 1];
        assert_eq!(
            t.bind(0, TextureMode::ExactHdBase).unwrap().status,
            BindingStatus::AmbiguousHdBase
        );
        t.links.ordered_targets[0].matching_replacement_ordinals = vec![0];
        assert!(t.bind(0, TextureMode::ExactHdBase).is_err());
        t.links.duplicate_candidate_offsets = vec![0x20000100];
        assert_eq!(
            t.bind(0, TextureMode::ExactHdBase).unwrap().status,
            BindingStatus::AmbiguousHdBase
        );
    }
    #[test]
    fn allocation_admission_checks_cumulative_expansion_and_serialization_excludes_pixels() {
        let mut bytes = 128 * 1024 * 1024 - 4;
        reserve(&mut bytes, 4).unwrap();
        assert!(reserve(&mut bytes, 1).is_err());
        let json = serde_json::to_string(&textures()).unwrap();
        assert!(!json.contains("pixels_rgba\":"));
        assert!(json.contains("pixels_rgba_sha256"));
    }
}
