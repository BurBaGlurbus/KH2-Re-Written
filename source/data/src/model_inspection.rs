//! Fresh source-bound research meshes. Positions/indices retain the explicit
//! public profiles; no installed material, normal, shadow or animation claim.
use crate::{
    model_geometry::Geometry,
    model_inspection_textures::{InspectionTextures, TextureMode, UvProfile, UvRoutes},
    model_pose::{ModelPose, PoseProfile},
    probe_source::ProbeSource,
};
use serde::{Deserialize, Serialize};
use std::{
    collections::HashSet,
    fs::File,
    io::{self, Read},
    path::Path,
    sync::Arc,
};

#[derive(Deserialize)]
struct SelectionFile {
    schema_version: u32,
    source_content_identity: String,
    selections: Vec<Selection>,
    inspect_model_geometry: bool,
    model_pose_profiles: Vec<PoseProfile>,
    #[serde(default)]
    inspect_model_textures: bool,
    #[serde(default)]
    model_texture_modes: Vec<TextureMode>,
    #[serde(default)]
    model_uv_profiles: Vec<UvProfile>,
}
#[derive(Deserialize)]
struct Selection {
    index: String,
    ordinal: usize,
    name_hash: [u8; 16],
}
#[derive(Serialize)]
pub struct InspectionPart {
    pub group_ordinal: usize,
    pub packet_ordinal: usize,
    pub texture_index: i32,
    pub source_is_opaque: bool,
    /// The mesh group's header word 0 ([`model_geometry::GeometryPart`]).
    #[serde(skip)]
    pub attributes: u32,
    pub indices: Vec<u32>,
    pub cross_part_corners: usize,
    /// The mesh's part group, bits 6..10 of its descriptor word 0 (what a
    /// script's `SeqPart` list names); filled by `read_models`.
    #[serde(skip)]
    pub mesh_group: u8,
}
pub struct InspectionModel {
    pub index: String,
    pub ordinal: usize,
    pub bar_ordinal: usize,
    pub excluded_linked_groups: usize,
    pub parts: Vec<InspectionPart>,
    pub poses: Vec<ModelPose>,
    pub skinning: crate::model_pose::Skinning,
    pub textures: Option<Arc<InspectionTextures>>,
    pub texture_routes: Vec<UvRoutes>,
}
impl InspectionModel {
    pub fn texture_modes(&self) -> &[TextureMode] {
        self.textures
            .as_ref()
            .map_or(&[TextureMode::PartColors], |t| t.modes.as_slice())
    }
    pub fn uv_profile_count(&self) -> usize {
        self.texture_routes.len().max(1)
    }
}
fn invalid(message: &str) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, message)
}
fn parts(geometry: &Geometry, poses: &[ModelPose]) -> io::Result<Vec<InspectionPart>> {
    if geometry.parts.is_empty() || poses.is_empty() || poses.len() > 2 {
        return Err(invalid("empty mesh or invalid explicit pose count"));
    }
    let count: usize = geometry
        .parts
        .iter()
        .map(|part| part.assignments.len())
        .sum();
    if count == 0
        || count > 262_144
        || poses.iter().any(|pose| {
            pose.position_bits.len() != count
                || pose
                    .position_bits
                    .iter()
                    .flatten()
                    .any(|&word| !f32::from_bits(word).is_finite())
        })
    {
        return Err(invalid("mesh/pose positions disagree or are nonfinite"));
    }
    let mut result = Vec::new();
    let mut vertex_base = 0;
    let mut corners = 0;
    for part in &geometry.parts {
        if part.vertex_base != vertex_base {
            return Err(invalid("noncontiguous global vertex bases"));
        }
        let end = vertex_base + part.assignments.len();
        let mut indices = Vec::new();
        let mut cross_part_corners = 0;
        for triangle in &part.triangles {
            for vertex in triangle {
                if vertex.vertex_index >= count {
                    return Err(invalid("global triangle position out of range"));
                }
                indices.push(vertex.vertex_index as u32);
                cross_part_corners +=
                    usize::from(!(vertex_base..end).contains(&vertex.vertex_index));
            }
        }
        corners += indices.len();
        if corners > 524_288 * 3 {
            return Err(invalid("inspection triangle budget exceeded"));
        }
        result.push(InspectionPart {
            group_ordinal: part.group_ordinal,
            packet_ordinal: part.packet_ordinal,
            texture_index: part.texture_index,
            source_is_opaque: part.is_opaque,
            attributes: part.attributes,
            indices,
            cross_part_corners,
            mesh_group: 0,
        });
        vertex_base = end;
    }
    if corners == 0 {
        return Err(invalid("inspection model has no triangles"));
    }
    Ok(result)
}
/// Decode selected original BAR models only after a complete fresh source
/// scan. HD replacements and prior derived reports are not mesh inputs.
pub fn read_selected_models(
    source: &ProbeSource,
    path: &Path,
) -> Result<Vec<InspectionModel>, Box<dyn std::error::Error>> {
    let mut bytes = Vec::new();
    File::open(path)?
        .take(1024 * 1024 + 1)
        .read_to_end(&mut bytes)?;
    if bytes.len() > 1024 * 1024 {
        return Err("model selection exceeds 1 MiB".into());
    }
    let selection: SelectionFile = serde_json::from_slice(&bytes)?;
    read_selection_models(source, selection)
}

/// One installed model by its HED identity, with the explicit profiles the
/// viewers and the test room use (legacy bind, original indexed textures,
/// integer-truncated UVs). Used for models named at run time.
pub fn read_model_at(
    source: &ProbeSource,
    index: &str,
    ordinal: usize,
    name_hash: [u8; 16],
) -> Result<Vec<InspectionModel>, Box<dyn std::error::Error>> {
    read_selection_models(
        source,
        SelectionFile {
            schema_version: 1,
            source_content_identity: source.content_identity().to_owned(),
            selections: vec![Selection {
                index: index.to_owned(),
                ordinal,
                name_hash,
            }],
            inspect_model_geometry: true,
            model_pose_profiles: vec![PoseProfile::Legacy],
            inspect_model_textures: true,
            model_texture_modes: vec![TextureMode::OriginalIndexed],
            model_uv_profiles: vec![UvProfile::Kkdf2IntegerTruncated],
        },
    )
}

/// Explicit original-only inspection. HD replacements are not requested or
/// decoded. `textures=false` is a geometry preview, never a textured fallback.
/// The caller retains the reason for choosing the untextured profile.
pub fn read_original_model_at(
    source: &ProbeSource,
    index: &str,
    ordinal: usize,
    name_hash: [u8; 16],
    textures: bool,
) -> Result<Vec<InspectionModel>, Box<dyn std::error::Error>> {
    source.verify_full_source()?;
    read_models_profile(
        source,
        &[ModelItem {
            index: index.to_owned(),
            ordinal,
            name_hash,
        }],
        &ModelOptions {
            pose_profiles: vec![PoseProfile::Legacy],
            textures,
            texture_modes: if textures {
                vec![TextureMode::OriginalIndexed]
            } else {
                Vec::new()
            },
            uv_profiles: if textures {
                vec![UvProfile::Kkdf2IntegerTruncated]
            } else {
                Vec::new()
            },
        },
        false,
    )
}

fn read_selection_models(
    source: &ProbeSource,
    selection: SelectionFile,
) -> Result<Vec<InspectionModel>, Box<dyn std::error::Error>> {
    if selection.schema_version != 1
        || selection.source_content_identity != source.content_identity()
        || !selection.inspect_model_geometry
        || selection.selections.is_empty()
        || selection.selections.len() > 64
        || selection.model_pose_profiles.is_empty()
        || selection.model_pose_profiles.len() > 2
        || (selection.model_pose_profiles.len() == 2
            && selection.model_pose_profiles[0] == selection.model_pose_profiles[1])
    {
        return Err("model selection identity/schema/count/explicit profiles invalid".into());
    }
    if selection.inspect_model_textures {
        if selection.model_texture_modes.is_empty()
            || selection.model_texture_modes.len() > 3
            || selection.model_uv_profiles.is_empty()
            || selection.model_uv_profiles.len() > 2
            || selection
                .model_texture_modes
                .iter()
                .enumerate()
                .any(|(i, m)| selection.model_texture_modes[..i].contains(m))
            || (selection.model_uv_profiles.len() == 2
                && selection.model_uv_profiles[0] == selection.model_uv_profiles[1])
        {
            return Err("texture inspection requires distinct explicit texture/UV choices".into());
        }
    } else if !selection.model_texture_modes.is_empty() || !selection.model_uv_profiles.is_empty() {
        return Err("texture/UV choices require explicit model texture inspection".into());
    }
    let mut seen = HashSet::new();
    for item in &selection.selections {
        if !validation::required_files()
            .iter()
            .any(|(name, _)| name == &item.index && name.ends_with(".hed"))
            || !seen.insert((&item.index, item.ordinal))
        {
            return Err("invalid/duplicate model source identity".into());
        }
    }
    source.verify_full_source()?;
    let items: Vec<_> = selection
        .selections
        .iter()
        .map(|s| ModelItem {
            index: s.index.clone(),
            ordinal: s.ordinal,
            name_hash: s.name_hash,
        })
        .collect();
    read_models(
        source,
        &items,
        &ModelOptions {
            pose_profiles: selection.model_pose_profiles,
            textures: selection.inspect_model_textures,
            texture_modes: selection.model_texture_modes,
            uv_profiles: selection.model_uv_profiles,
        },
    )
}

/// One installed model archive (`obj/<NAME>.mdlx`) by HED locator.
#[derive(Clone, Debug)]
pub struct ModelItem {
    pub index: String,
    pub ordinal: usize,
    pub name_hash: [u8; 16],
}

/// Decoding choices for [`read_models`].
#[derive(Clone, Debug)]
pub struct ModelOptions {
    pub pose_profiles: Vec<PoseProfile>,
    pub textures: bool,
    pub texture_modes: Vec<TextureMode>,
    pub uv_profiles: Vec<UvProfile>,
}

/// Decodes the type-4 models of already admitted archives; the caller has
/// verified the bound source.
pub fn read_models(
    source: &ProbeSource,
    items: &[ModelItem],
    options: &ModelOptions,
) -> Result<Vec<InspectionModel>, Box<dyn std::error::Error>> {
    read_models_profile(source, items, options, true)
}

fn read_models_profile(
    source: &ProbeSource,
    items: &[ModelItem],
    options: &ModelOptions,
    include_replacements: bool,
) -> Result<Vec<InspectionModel>, Box<dyn std::error::Error>> {
    let mut models = Vec::new();
    let mut storage_bytes = 0usize;
    let mut position_words = 0usize;
    let mut index_words = 0usize;
    let mut texture_rgba = 0usize;
    let mut texture_images = 0usize;
    let mut uv_corners = 0usize;
    for (relative, _) in validation::required_files()
        .into_iter()
        .filter(|(name, _)| name.ends_with(".hed"))
    {
        let selected: Vec<_> = items.iter().filter(|item| item.index == relative).collect();
        if selected.is_empty() {
            continue;
        }
        let mut checked = source.open_index(&relative)?;
        for item in selected {
            let entry = checked
                .inventory
                .entries
                .get(item.ordinal)
                .filter(|entry| entry.name_hash == item.name_hash)
                .ok_or("model entry identity differs from HED")?;
            let original = crate::original_payload::read_original(&mut checked.reader, entry)?;
            storage_bytes += original.bytes.len();
            if storage_bytes > 128 * 1024 * 1024 {
                return Err("model storage exceeds 128 MiB".into());
            }
            let bar = crate::bar::read_bar(&original.bytes)?;
            let textures = if options.textures {
                let replacements = if include_replacements {
                    crate::replacement_payload::read_all_replacements(&mut checked.reader, entry)?
                } else {
                    Vec::new()
                };
                storage_bytes += replacements.iter().map(|r| r.bytes.len()).sum::<usize>();
                if storage_bytes > 128 * 1024 * 1024 {
                    return Err("model/replacement storage exceeds 128 MiB".into());
                }
                let entries: Vec<_> = bar
                    .entries
                    .iter()
                    .enumerate()
                    .filter(|(_, e)| e.entry_type == 7)
                    .collect();
                if entries.len() != 1
                    || entries[0].1.name_bytes != *b"tim_"
                    || entries[0].1.offset < 0
                    || entries[0].1.size == 0
                {
                    return Err(
                        "textured inspection requires exactly one nonempty tim_ BAR/type-7 entry"
                            .into(),
                    );
                }
                let (ordinal, record) = entries[0];
                let start = record.offset as usize;
                let set = crate::model_inspection_textures::read_textures(
                    &original.bytes[start..start + record.size as usize],
                    start,
                    ordinal,
                    &replacements,
                    &options.texture_modes,
                    &mut texture_rgba,
                )?;
                texture_images += set.images.len();
                if texture_images > 64 {
                    return Err("cumulative inspection texture count exceeds 64".into());
                }
                Some(Arc::new(set))
            } else {
                None
            };
            let before = models.len();
            for (bar_ordinal, entry) in bar
                .entries
                .iter()
                .enumerate()
                .filter(|(_, entry)| entry.entry_type == 4)
            {
                if entry.offset < 0 || entry.size == 0 {
                    return Err("model BAR entry has no payload".into());
                }
                let start = entry.offset as usize;
                let payload = &original.bytes[start..start + entry.size as usize];
                let geometry = crate::model_geometry::read_geometry(payload)?;
                let skinning = crate::model_pose::Skinning::read(payload)?;
                let poses: Vec<_> = options
                    .pose_profiles
                    .iter()
                    .map(|&profile| skinning.bind_pose(profile))
                    .collect::<io::Result<_>>()?;
                let mut mesh_parts = parts(&geometry, &poses)?;
                if let Some(root) = crate::model_skeleton::read_model_structure(payload)?
                    .blocks
                    .first()
                {
                    for part in &mut mesh_parts {
                        if let Some(header) = root.group_headers.get(part.group_ordinal) {
                            part.mesh_group = ((header[0] >> 6) & 0x1f) as u8;
                        }
                    }
                }
                let texture_routes = if let Some(set) = &textures {
                    let environments = set
                        .images
                        .iter()
                        .filter(|i| {
                            matches!(
                                i.identity,
                                crate::model_inspection_textures::ImageIdentity::Original { .. }
                            )
                        })
                        .count();
                    let routes = crate::model_inspection_textures::read_uv_routes(
                        payload,
                        &geometry,
                        environments,
                        &options.uv_profiles,
                    )?;
                    uv_corners += routes
                        .iter()
                        .flat_map(|r| &r.parts)
                        .map(|p| p.expanded_uv_bits.len())
                        .sum::<usize>();
                    if uv_corners > 2 * 524_288 * 3 {
                        return Err("cumulative inspection UV corner budget exceeded".into());
                    }
                    routes
                } else {
                    Vec::new()
                };
                position_words += poses
                    .iter()
                    .map(|pose| pose.position_bits.len() * 3)
                    .sum::<usize>();
                index_words += mesh_parts
                    .iter()
                    .map(|part| part.indices.len())
                    .sum::<usize>();
                if position_words > 2 * 262_144 * 3
                    || index_words > 524_288 * 3
                    || models.len() >= 64
                {
                    return Err("cumulative inspection mesh budget exceeded".into());
                }
                models.push(InspectionModel {
                    index: relative.clone(),
                    ordinal: item.ordinal,
                    bar_ordinal,
                    excluded_linked_groups: geometry.excluded_linked_groups,
                    parts: mesh_parts,
                    poses,
                    skinning,
                    textures: textures.clone(),
                    texture_routes,
                });
            }
            if models.len() == before {
                return Err("selected original has no model BAR entry".into());
            }
        }
        checked.verify_unchanged()?;
    }
    if models.is_empty() {
        return Err("no admitted inspection models".into());
    }
    Ok(models)
}

/// A model from its BAR payloads (type 4 and the same-name type 7 bank),
/// as used for map objects that live inside the map archive. Original
/// indexed textures only; the legacy bind pose and integer-truncated UVs
/// are the same explicit profiles as the named loader.
pub fn model_from_payload(
    model: &[u8],
    textures: Option<(&[u8], usize, usize)>,
) -> io::Result<InspectionModel> {
    let geometry = crate::model_geometry::read_geometry(model)?;
    let skinning = crate::model_pose::Skinning::read(model)?;
    let poses = vec![skinning.bind_pose(PoseProfile::Legacy)?];
    let mesh_parts = parts(&geometry, &poses)?;
    let (textures, texture_routes) = match textures {
        Some((bank, offset, ordinal)) => {
            let mut budget = 0usize;
            let set = crate::model_inspection_textures::read_textures(
                bank,
                offset,
                ordinal,
                &[],
                &[TextureMode::OriginalIndexed],
                &mut budget,
            )?;
            let environments = set.images.len();
            let routes = crate::model_inspection_textures::read_uv_routes(
                model,
                &geometry,
                environments,
                &[UvProfile::Kkdf2IntegerTruncated],
            )?;
            (Some(Arc::new(set)), routes)
        }
        None => (None, Vec::new()),
    };
    Ok(InspectionModel {
        index: String::new(),
        ordinal: 0,
        bar_ordinal: 0,
        excluded_linked_groups: geometry.excluded_linked_groups,
        parts: mesh_parts,
        poses,
        skinning,
        textures,
        texture_routes,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model_geometry::{GeometryPart, PositionBinding, VertexRef};
    fn fixture() -> (Geometry, Vec<ModelPose>) {
        let mut result = Geometry {
            profile: crate::model_geometry::PROFILE,
            block_offset: 0x90,
            excluded_linked_groups: 0,
            parts: Vec::new(),
            runtime_verified: false,
        };
        for base in [0, 2] {
            result.parts.push(GeometryPart {
                group_ordinal: base / 2,
                packet_ordinal: 0,
                texture_index: 0,
                is_opaque: false,
                attributes: 1,
                vertex_base: base,
                uv_base: 0,
                assignments: vec![
                    vec![PositionBinding {
                        matrix_index: 0,
                        vertex_index: 0
                    }];
                    2
                ],
                uv_bits: Vec::new(),
                triangles: vec![[
                    VertexRef {
                        vertex_index: 0,
                        uv_index: 0,
                    },
                    VertexRef {
                        vertex_index: 1,
                        uv_index: 0,
                    },
                    VertexRef {
                        vertex_index: 2,
                        uv_index: 0,
                    },
                ]],
            });
        }
        (
            result,
            vec![ModelPose {
                profile: PoseProfile::Legacy,
                arithmetic_profile: "authored",
                matrix_bits: Vec::new(),
                position_bits: vec![[0; 3]; 4],
                runtime_verified: false,
            }],
        )
    }
    #[test]
    fn global_cross_part_corners_are_not_rebased_or_dropped() {
        let (geometry, poses) = fixture();
        let mesh = parts(&geometry, &poses).unwrap();
        assert_eq!(mesh[0].indices, [0, 1, 2]);
        assert_eq!(mesh[1].indices, [0, 1, 2]);
        assert_eq!(mesh[0].cross_part_corners, 1);
        assert_eq!(mesh[1].cross_part_corners, 2);
    }
    #[test]
    fn rejects_nonfinite_mismatched_positions_bad_bases_and_global_indices() {
        let (mut geometry, mut poses) = fixture();
        poses[0].position_bits[0][0] = f32::INFINITY.to_bits();
        assert!(parts(&geometry, &poses).is_err());
        poses[0].position_bits[0][0] = 0;
        poses[0].position_bits.pop();
        assert!(parts(&geometry, &poses).is_err());
        poses[0].position_bits.push([0; 3]);
        geometry.parts[1].vertex_base = 0;
        assert!(parts(&geometry, &poses).is_err());
        geometry.parts[1].vertex_base = 2;
        geometry.parts[1].triangles[0][0].vertex_index = 4;
        assert!(parts(&geometry, &poses).is_err());
    }
}
