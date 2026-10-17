//! Explicit public CPU topology profile. No bind pose, skinning or shadow mesh.
//! Adapted from pinned OpenKH VpuPacketExtensions and Kkdf2MdlxParser (NOTICE).
use crate::{model_packets::PacketGroup, model_skeleton::ModelStructure, vpu::VpuData};
use serde::Serialize;
use std::io;

pub const PROFILE: &str = "openkh_kkdf2_type1_topology_v1";
const MAX_ITEMS: usize = 262_144;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
pub struct PositionBinding {
    pub matrix_index: i32,
    pub vertex_index: usize,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
pub struct VertexRef {
    pub vertex_index: usize,
    pub uv_index: usize,
}
#[derive(Debug, Serialize)]
pub struct GeometryPart {
    pub group_ordinal: usize,
    pub packet_ordinal: usize,
    pub texture_index: i32,
    pub is_opaque: bool,
    /// The mesh group's header word 0 (draw attributes; bit 0 = the
    /// translucent pass, bits 6..10 = part group).
    pub attributes: u32,
    pub vertex_base: usize,
    pub uv_base: usize,
    pub assignments: Vec<Vec<PositionBinding>>,
    pub uv_bits: Vec<[u32; 2]>,
    pub triangles: Vec<[VertexRef; 3]>,
}
#[derive(Debug, Serialize)]
pub struct Geometry {
    pub profile: &'static str,
    pub block_offset: usize,
    pub excluded_linked_groups: usize,
    pub parts: Vec<GeometryPart>,
    pub runtime_verified: bool,
}
fn invalid(message: &str) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, message)
}
fn assignments(data: &VpuData, matrices: &[i32]) -> io::Result<Vec<Vec<PositionBinding>>> {
    if matrices.len() != data.matrix_counts.len() || matrices.iter().any(|&m| m < 0) {
        return Err(invalid("matrix-list/range disagreement"));
    }
    let mut flat = Vec::new();
    for (&matrix_index, &count) in matrices.iter().zip(&data.matrix_counts) {
        if count < 0 || flat.len() + count as usize > data.vertex_bits.len() {
            return Err(invalid("matrix range exceeds coordinate array"));
        }
        for _ in 0..count {
            flat.push(PositionBinding {
                matrix_index,
                vertex_index: flat.len(),
            });
        }
    }
    if flat.len() != data.vertex_bits.len() {
        return Err(invalid("matrix ranges do not cover coordinates"));
    }
    if data.weighted_indices.is_empty() {
        return Ok(flat.into_iter().map(|binding| vec![binding]).collect());
    }
    if data.weighted_indices.len() > 32
        || data.weighted_indices.len() != data.weight_group_counts.len()
    {
        return Err(invalid("invalid weight-group shape"));
    }
    let mut result = Vec::new();
    let mut binding_count = 0;
    for (group, (entries, &count)) in data
        .weighted_indices
        .iter()
        .zip(&data.weight_group_counts)
        .enumerate()
    {
        if count < 0 || count as usize != entries.len() {
            return Err(invalid("weight-group count disagreement"));
        }
        for entry in entries {
            binding_count += entry.len();
            if entry.len() != group + 1 || binding_count > MAX_ITEMS {
                return Err(invalid("weight associations exceed profile"));
            }
            let mut bindings = Vec::new();
            for &index in entry {
                let binding = usize::try_from(index)
                    .ok()
                    .and_then(|i| flat.get(i))
                    .ok_or_else(|| invalid("weight coordinate index out of range"))?;
                bindings.push(*binding);
            }
            result.push(bindings);
        }
    }
    Ok(result)
}
pub fn read_geometry(bytes: &[u8]) -> io::Result<Geometry> {
    let structure = crate::model_skeleton::read_model_structure(bytes)?;
    let groups = crate::model_packets::read_packet_groups(bytes)?;
    assemble(&structure, &groups)
}
fn assemble(structure: &ModelStructure, groups: &[PacketGroup]) -> io::Result<Geometry> {
    let root = structure
        .blocks
        .first()
        .ok_or_else(|| invalid("missing model root"))?;
    if root.offset != 0x90 || root.header_words[0] != 3 {
        return Err(invalid("unsupported topology root"));
    }
    let root_groups: Vec<_> = groups
        .iter()
        .filter(|g| g.block_offset == root.offset)
        .collect();
    if root_groups.len() != root.group_headers.len() {
        return Err(invalid("root group count disagreement"));
    }
    let mut result = Geometry {
        profile: PROFILE,
        block_offset: root.offset,
        excluded_linked_groups: groups.len() - root_groups.len(),
        parts: Vec::new(),
        runtime_verified: false,
    };
    let mut ring: [Option<VertexRef>; 4] = [None; 4];
    let mut ring_index = 0;
    let mut vertex_base = 0;
    let mut uv_base = 0;
    let mut total_triangles = 0;
    let mut total_bindings = 0;
    for (ordinal, group) in root_groups.into_iter().enumerate() {
        if group.group_ordinal != ordinal {
            return Err(invalid("root group order disagreement"));
        }
        let matrix_lists: Vec<_> = group.matrix_entries.split(|&m| m == -1).collect();
        if matrix_lists.len() != group.packets.len() {
            return Err(invalid("matrix-list/packet count disagreement"));
        }
        let header = root.group_headers[ordinal];
        for (packet_ordinal, (packet, matrices)) in
            group.packets.iter().zip(matrix_lists).enumerate()
        {
            if packet.vpu_header_words[0] != 1 {
                return Err(invalid("root packet is outside type-1 topology"));
            }
            let data = packet
                .vpu_data
                .as_ref()
                .ok_or_else(|| invalid("missing type-1 data"))?;
            let assignments = assignments(data, matrices)?;
            total_bindings += assignments.iter().map(Vec::len).sum::<usize>();
            if vertex_base + assignments.len() > MAX_ITEMS
                || uv_base + data.strips.len() > MAX_ITEMS
                || total_bindings > MAX_ITEMS
            {
                return Err(invalid("cumulative geometry exceeds profile"));
            }
            let mut part = GeometryPart {
                group_ordinal: ordinal,
                packet_ordinal,
                texture_index: header[1],
                is_opaque: header[0] & 1 == 0,
                attributes: header[0] as u32,
                vertex_base,
                uv_base,
                assignments,
                uv_bits: Vec::new(),
                triangles: Vec::new(),
            };
            for (strip_ordinal, &[u, v, index, flag]) in data.strips.iter().enumerate() {
                if index < 0 || index as usize >= part.assignments.len() {
                    return Err(invalid("strip vertex index out of range"));
                }
                if ![0x00, 0x10, 0x20, 0x30].contains(&flag) {
                    return Err(invalid("unqualified strip function"));
                }
                ring[ring_index] = Some(VertexRef {
                    vertex_index: vertex_base + index as usize,
                    uv_index: uv_base + strip_ordinal,
                });
                ring_index = (ring_index + 1) & 3;
                for order in [[1, 3, 2], [1, 2, 3]] {
                    if (order[1] == 3 && [0x00, 0x20].contains(&flag))
                        || (order[1] == 2 && [0x00, 0x30].contains(&flag))
                    {
                        let triangle = order.map(|back| ring[(ring_index + 4 - back) & 3]);
                        let [Some(a), Some(b), Some(c)] = triangle else {
                            return Err(invalid("triangle references uninitialized strip ring"));
                        };
                        total_triangles += 1;
                        if total_triangles > MAX_ITEMS * 2 {
                            return Err(invalid("too many cumulative triangles"));
                        }
                        part.triangles.push([a, b, c]);
                    }
                }
                // Public expression uses integer /16 BEFORE float /256.
                part.uv_bits.push([
                    ((u / 16) as f32 / 256.0).to_bits(),
                    ((v / 16) as f32 / 256.0).to_bits(),
                ]);
            }
            vertex_base += part.assignments.len();
            uv_base += part.uv_bits.len();
            result.parts.push(part);
        }
    }
    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{model_packets::Packet, model_skeleton::read_model_structure};
    fn data() -> VpuData {
        VpuData {
            normal_words: None,
            matrix_counts: vec![1, 2],
            strips: vec![[31, -31, 2, 0x10], [0, 0, 0, 0x10], [0, 0, 1, 0x20]],
            vertex_bits: vec![[0; 4]; 3],
            colors: vec![],
            normal_bits: vec![],
            weight_group_counts: vec![],
            weighted_indices: vec![],
        }
    }
    fn packet(data: VpuData) -> Packet {
        Packet {
            offset: 0,
            code_offset: 0,
            code_length: 0,
            base_address: 0,
            code_sha256: String::new(),
            tags: vec![],
            memory_sha256: String::new(),
            program_counter: 0,
            stop_state: "end",
            vpu_header_words: [1; 16],
            vpu_data: Some(data),
        }
    }
    fn structure() -> ModelStructure {
        let mut bytes = vec![0; 512];
        for (offset, value) in [
            (0x90, 3i32),
            (0x90 + 24, 64),
            (0x90 + 28, 1),
            (0xb0, 1),
            (0xb4, 7),
        ] {
            bytes[offset..offset + 4].copy_from_slice(&value.to_le_bytes());
        }
        read_model_structure(&bytes).unwrap()
    }
    fn group(packets: Vec<Packet>, matrix_entries: Vec<i32>) -> PacketGroup {
        PacketGroup {
            block_offset: 0x90,
            group_ordinal: 0,
            matrix_entries,
            packets,
        }
    }
    #[test]
    fn triangle_winding_global_indices_and_integer_uv_truncation() {
        let mut second = data();
        second.strips = vec![[0, 0, 2, 0x30], [0, 0, 1, 0]];
        let result = assemble(
            &structure(),
            &[group(
                vec![packet(data()), packet(second)],
                vec![7, 9, -1, 4, 8],
            )],
        )
        .unwrap();
        let first = &result.parts[0];
        assert_eq!(first.texture_index, 7);
        assert!(!first.is_opaque);
        assert_eq!(
            first.uv_bits[0],
            [(1.0f32 / 256.0).to_bits(), (-1.0f32 / 256.0).to_bits()]
        );
        assert_eq!(first.triangles[0].map(|v| v.vertex_index), [1, 2, 0]);
        let second = &result.parts[1];
        assert_eq!(
            second
                .triangles
                .iter()
                .map(|t| t.map(|v| v.vertex_index))
                .collect::<Vec<_>>(),
            [[5, 1, 0], [4, 1, 5], [4, 5, 1]]
        );
        assert_eq!(second.triangles[0].map(|v| v.uv_index), [3, 2, 1]);
        assert_eq!(
            second.assignments[2],
            [PositionBinding {
                matrix_index: 8,
                vertex_index: 2
            }]
        );
    }
    #[test]
    fn ring_persists_across_groups_and_linked_groups_are_excluded() {
        let mut root = structure();
        root.blocks[0].group_headers.push([0; 8]);
        let mut a = data();
        a.strips.truncate(2);
        let mut b = data();
        b.strips = vec![[0, 0, 2, 0x20]];
        let mut next = group(vec![packet(b)], vec![4, 8]);
        next.group_ordinal = 1;
        let mut shadow = group(vec![], vec![]);
        shadow.block_offset = 0x400;
        let result = assemble(&root, &[group(vec![packet(a)], vec![7, 9]), next, shadow]).unwrap();
        assert_eq!(result.excluded_linked_groups, 1);
        assert_eq!(
            result.parts[1].triangles[0].map(|v| v.vertex_index),
            [5, 2, 0]
        );
    }
    #[test]
    fn weighted_associations_follow_group_order_and_reject_invalid_ranges() {
        let mut d = data();
        d.weight_group_counts = vec![1, 1];
        d.weighted_indices = vec![vec![vec![2]], vec![vec![1, 0]]];
        let a = assignments(&d, &[7, 9]).unwrap();
        assert_eq!(
            a[1],
            [
                PositionBinding {
                    matrix_index: 9,
                    vertex_index: 1
                },
                PositionBinding {
                    matrix_index: 7,
                    vertex_index: 0
                }
            ]
        );
        d.weighted_indices[1][0][0] = 3;
        assert!(assignments(&d, &[7, 9]).is_err());
        assert!(assignments(&data(), &[7]).is_err());
        assert!(assignments(&data(), &[-2, 9]).is_err());
        d = data();
        d.matrix_counts[0] = 0;
        assert!(assignments(&d, &[7, 9]).is_err());
    }
    #[test]
    fn rejects_unknown_flags_uninitialized_ring_and_bad_strip_indices() {
        for (index, flag) in [(0, 0x20), (0, 0x40), (-1, 0x10), (3, 0x10)] {
            let mut d = data();
            d.strips = vec![[0, 0, index, flag]];
            assert!(assemble(&structure(), &[group(vec![packet(d)], vec![7, 9])]).is_err());
        }
        assert!(assemble(&structure(), &[group(vec![packet(data())], vec![7, -1, 9])]).is_err());
    }
    #[test]
    fn bounds_cumulative_associations_before_publishing_geometry() {
        let packets = (0..257)
            .map(|_| {
                let mut d = data();
                d.matrix_counts = vec![1024];
                d.vertex_bits = vec![[0; 4]; 1024];
                d.strips.clear();
                packet(d)
            })
            .collect();
        let entries = (0..257)
            .flat_map(|i| if i == 0 { vec![7] } else { vec![-1, 7] })
            .collect();
        let error = assemble(&structure(), &[group(packets, entries)]).unwrap_err();
        assert_eq!(error.to_string(), "cumulative geometry exceeds profile");
    }
}
