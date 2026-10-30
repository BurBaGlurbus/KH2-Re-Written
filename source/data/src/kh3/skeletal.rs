//! `USkeletalMesh` and `USkeleton` exports: the reference skeleton and the
//! first LOD's vertices, indices, skin weights and sections. Layout from
//! StrawHatPro's `docs/formats/skeletalmesh.md` (MIT).
use super::package::{Package, Rd, read_name, read_u16s};
use super::props::{Props, read_props};
use super::{Result, bad};

#[derive(Clone, Debug, PartialEq)]
pub struct Bone {
    pub name: String,
    pub parent: i32,
    pub rotation: [f32; 4],
    pub translation: [f32; 3],
    pub scale: [f32; 3],
}

#[derive(Clone, Debug, PartialEq)]
pub struct MaterialSlot {
    /// The package index of the material or material instance.
    pub object: i32,
    pub slot: String,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Section {
    pub material: u16,
    pub base_index: u32,
    pub triangles: u32,
    pub base_vertex: u32,
    pub vertices: i32,
    pub bone_map: Vec<u16>,
}

/// LOD 0, with joints already mapped to skeleton bones.
#[derive(Clone, Debug, Default)]
pub struct Lod {
    pub sections: Vec<Section>,
    pub indices: Vec<u32>,
    pub positions: Vec<[f32; 3]>,
    /// `byte / 127.5 - 1`; the normal's W is the bitangent's sign.
    pub normals: Vec<[f32; 4]>,
    pub tangents: Vec<[f32; 3]>,
    /// The first two UV sets.
    pub uv0: Vec<[f32; 2]>,
    pub uv1: Vec<[f32; 2]>,
    pub joints: Vec<[u16; 4]>,
    pub weights: Vec<[f32; 4]>,
}

#[derive(Clone, Debug, Default)]
pub struct SkeletalMesh {
    pub props: Props,
    pub bones: Vec<Bone>,
    pub materials: Vec<MaterialSlot>,
    pub lod: Lod,
}

/// Three arrays of the same length: bone info, pose, name map.
pub fn read_ref_skeleton(r: &mut Rd, names: &[String]) -> Result<Vec<Bone>> {
    let n = r.count(12)?;
    let mut bones = Vec::with_capacity(n);
    for _ in 0..n {
        let name = read_name(r, names)?;
        let parent = r.i32()?;
        bones.push(Bone {
            name,
            parent,
            rotation: [0., 0., 0., 1.],
            translation: [0.; 3],
            scale: [1.; 3],
        });
    }
    if r.count(40)? != n {
        return Err(bad("bone pose count differs"));
    }
    for b in &mut bones {
        b.rotation = [r.f32()?, r.f32()?, r.f32()?, r.f32()?];
        b.translation = [r.f32()?, r.f32()?, r.f32()?];
        b.scale = [r.f32()?, r.f32()?, r.f32()?];
    }
    let map = r.count(12)?;
    r.skip(map * 12)?;
    Ok(bones)
}

/// A `Skeleton` export's reference skeleton.
pub fn read_skeleton(p: &Package, i: usize) -> Result<Vec<Bone>> {
    let bytes = p.export_bytes(i)?;
    let mut r = Rd::new(bytes);
    read_props(&mut r, &p.names)?;
    r.i32()?;
    read_ref_skeleton(&mut r, &p.names)
}

fn read_section(r: &mut Rd) -> Result<Section> {
    r.skip(2)?;
    let material = r.u16()?;
    let base_index = r.u32()?;
    let triangles = r.u32()?;
    r.skip(1 + 4 + 2 + 1 + 4 + 4)?;
    let base_vertex = r.u32()?;
    let bone_map = read_u16s(r)?;
    let vertices = r.i32()?;
    r.i32()?;
    let cloth = r.count(64)?;
    r.skip(cloth * 64)?;
    let phys = r.count(12)?;
    r.skip(phys * 12)?;
    let normals = r.count(12)?;
    r.skip(normals * 12)?;
    r.skip(2 + 16 + 4)?;
    r.i32()?;
    let tail = r.count(24)?;
    r.skip(tail * 24)?;
    Ok(Section {
        material,
        base_index,
        triangles,
        base_vertex,
        vertices,
        bone_map,
    })
}

fn half(v: u16) -> f32 {
    let sign = if v & 0x8000 != 0 { -1. } else { 1. };
    let exp = i32::from((v >> 10) & 0x1f);
    let frac = f32::from(v & 0x3ff);
    sign * match exp {
        0 => frac * 2f32.powi(-24),
        31 => f32::INFINITY,
        _ => (1. + frac / 1024.) * 2f32.powi(exp - 15),
    }
}

fn read_lod(r: &mut Rd) -> Result<Lod> {
    let mut lod = Lod::default();
    r.skip(2)?;
    let sections = r.count(4)?;
    for _ in 0..sections {
        lod.sections.push(read_section(r)?);
    }
    let index_size = usize::from(r.u8()?);
    let element = r.i32()? as usize;
    if element != index_size {
        return Err(bad("index element size differs"));
    }
    let count = r.count(index_size)?;
    let raw = r.take(count * index_size)?;
    lod.indices = if index_size == 2 {
        raw.chunks_exact(2)
            .map(|c| u32::from(u16::from_le_bytes([c[0], c[1]])))
            .collect()
    } else {
        raw.chunks_exact(4)
            .map(|c| u32::from_le_bytes([c[0], c[1], c[2], c[3]]))
            .collect()
    };
    read_u16s(r)?;
    r.u32()?;
    let vertex_count = r.u32()? as usize;
    read_u16s(r)?;
    let map = r.count(4)?;
    r.skip(map * 4)?;
    r.i32()?;
    let num_uv = r.u32()? as usize;
    // Vertex buffer.
    r.skip(2)?;
    r.u32()?;
    let full_uv = r.i32()? != 0;
    r.skip(24)?;
    let stride = r.i32()? as usize;
    let verts = r.count(stride)?;
    if verts != vertex_count {
        return Err(bad("vertex count differs"));
    }
    let raw = r.take(verts * stride)?;
    let uv_size = if full_uv { 8 } else { 4 };
    if stride != 8 + 12 + num_uv * uv_size {
        return Err(bad(format!("vertex stride {stride} with {num_uv} UV sets")));
    }
    let unit = |b: u8| f32::from(b) / 127.5 - 1.;
    lod.positions.reserve(verts);
    for v in raw.chunks_exact(stride) {
        lod.tangents.push([unit(v[0]), unit(v[1]), unit(v[2])]);
        lod.normals.push([
            unit(v[4]),
            unit(v[5]),
            unit(v[6]),
            if v[7] >= 128 { 1. } else { -1. },
        ]);
        let f = |o: usize| f32::from_le_bytes([v[o], v[o + 1], v[o + 2], v[o + 3]]);
        lod.positions.push([f(8), f(12), f(16)]);
        let uv = |set: usize| -> [f32; 2] {
            let o = 20 + set * uv_size;
            if full_uv {
                [f(o), f(o + 4)]
            } else {
                [
                    half(u16::from_le_bytes([v[o], v[o + 1]])),
                    half(u16::from_le_bytes([v[o + 2], v[o + 3]])),
                ]
            }
        };
        lod.uv0.push(uv(0));
        lod.uv1.push(if num_uv > 1 { uv(1) } else { uv(0) });
    }
    // Skin weights: the four heaviest influences, as skeleton bones.
    r.skip(2)?;
    let extra = r.i32()? != 0;
    r.u32()?;
    let per = if extra { 16 } else { 8 };
    if r.i32()? as usize != per {
        return Err(bad("skin weight size differs"));
    }
    let n = r.count(per)?;
    if n != verts {
        return Err(bad("weight count differs"));
    }
    let raw = r.take(n * per)?;
    lod.joints = vec![[0; 4]; n];
    lod.weights = vec![[0.; 4]; n];
    let influences = per / 2;
    for s in &lod.sections {
        let from = s.base_vertex as usize;
        let to = from + s.vertices.max(0) as usize;
        for v in from..to.min(n) {
            let w = &raw[v * per..v * per + per];
            let mut pairs: Vec<(u8, u8)> =
                (0..influences).map(|k| (w[k + influences], w[k])).collect();
            pairs.sort_by(|a, b| b.0.cmp(&a.0));
            let total: f32 = pairs.iter().take(4).map(|p| f32::from(p.0)).sum();
            for (k, &(weight, local)) in pairs.iter().take(4).enumerate() {
                lod.joints[v][k] = s.bone_map.get(usize::from(local)).copied().unwrap_or(0);
                lod.weights[v][k] = if total > 0. {
                    f32::from(weight) / total
                } else {
                    (k == 0) as u8 as f32
                };
            }
        }
    }
    Ok(lod)
}

impl SkeletalMesh {
    pub fn read(p: &Package, i: usize) -> Result<Self> {
        let bytes = p.export_bytes(i)?;
        let mut r = Rd::new(bytes);
        let props = read_props(&mut r, &p.names)?;
        r.i32()?;
        r.skip(2)?;
        r.skip(28)?;
        let slots = r.count(36)?;
        let mut materials = Vec::with_capacity(slots);
        for _ in 0..slots {
            let object = r.i32()?;
            let slot = read_name(&mut r, &p.names)?;
            r.skip(4 + 4 + 16)?;
            materials.push(MaterialSlot { object, slot });
        }
        let bones = read_ref_skeleton(&mut r, &p.names)?;
        if r.i32()? < 1 {
            return Err(bad("mesh without LODs"));
        }
        let lod = read_lod(&mut r)?;
        Ok(Self {
            props,
            bones,
            materials,
            lod,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn half_floats() {
        assert_eq!(half(0), 0.);
        assert_eq!(half(0x3c00), 1.);
        assert_eq!(half(0xc000), -2.);
        assert_eq!(half(0x3800), 0.5);
    }

    fn mesh_export(names: &[String]) -> Vec<u8> {
        let mut e = Vec::new();
        let none = names.iter().position(|n| n == "None").unwrap() as i32;
        let root = names.iter().position(|n| n == "root").unwrap() as i32;
        let push_name = |e: &mut Vec<u8>, i: i32| {
            e.extend(i.to_le_bytes());
            e.extend(0i32.to_le_bytes());
        };
        push_name(&mut e, none);
        e.extend(0i32.to_le_bytes());
        e.extend([1, 0]);
        e.extend([0; 28]);
        e.extend(1i32.to_le_bytes());
        e.extend((-1i32).to_le_bytes());
        push_name(&mut e, root);
        e.extend([0; 24]);
        // Skeleton.
        e.extend(1i32.to_le_bytes());
        push_name(&mut e, root);
        e.extend((-1i32).to_le_bytes());
        e.extend(1i32.to_le_bytes());
        for f in [0f32, 0., 0., 1., 1., 2., 3., 1., 1., 1.] {
            e.extend(f.to_le_bytes());
        }
        e.extend(1i32.to_le_bytes());
        push_name(&mut e, root);
        e.extend(0i32.to_le_bytes());
        // One LOD.
        e.extend(1i32.to_le_bytes());
        e.extend([1, 0]);
        e.extend(1i32.to_le_bytes());
        // Section.
        e.extend([1, 0]);
        e.extend(0u16.to_le_bytes());
        e.extend(0u32.to_le_bytes());
        e.extend(1u32.to_le_bytes());
        e.extend([0; 1 + 4 + 2 + 1 + 4 + 4]);
        e.extend(0u32.to_le_bytes());
        e.extend(1i32.to_le_bytes());
        e.extend(5u16.to_le_bytes());
        e.extend(3i32.to_le_bytes());
        e.extend(4i32.to_le_bytes());
        e.extend(0i32.to_le_bytes());
        e.extend(0i32.to_le_bytes());
        e.extend(0i32.to_le_bytes());
        e.extend([0; 2 + 16 + 4]);
        e.extend(0i32.to_le_bytes());
        e.extend(0i32.to_le_bytes());
        // Indices.
        e.push(2);
        e.extend(2i32.to_le_bytes());
        e.extend(3i32.to_le_bytes());
        for i in [0u16, 1, 2] {
            e.extend(i.to_le_bytes());
        }
        e.extend(0i32.to_le_bytes());
        e.extend(0u32.to_le_bytes());
        e.extend(3u32.to_le_bytes());
        e.extend(0i32.to_le_bytes());
        e.extend(0i32.to_le_bytes());
        e.extend(0i32.to_le_bytes());
        e.extend(1u32.to_le_bytes());
        // Vertices: half UVs.
        e.extend([1, 0]);
        e.extend(1u32.to_le_bytes());
        e.extend(0i32.to_le_bytes());
        e.extend([0; 24]);
        e.extend(24i32.to_le_bytes());
        e.extend(3i32.to_le_bytes());
        for i in 0..3 {
            e.extend([255, 127, 127, 0]);
            e.extend([127, 127, 255, 255]);
            for f in [i as f32, 0., 0.] {
                e.extend(f.to_le_bytes());
            }
            e.extend(0x3c00u16.to_le_bytes());
            e.extend(0x3800u16.to_le_bytes());
        }
        // Weights.
        e.extend([1, 0]);
        e.extend(0i32.to_le_bytes());
        e.extend(3u32.to_le_bytes());
        e.extend(8i32.to_le_bytes());
        e.extend(3i32.to_le_bytes());
        for _ in 0..3 {
            e.extend([0, 0, 0, 0, 255, 0, 0, 0]);
        }
        e
    }

    #[test]
    fn the_first_lod_is_read() {
        let names: Vec<String> = ["None", "root"].iter().map(|s| (*s).to_owned()).collect();
        let p = Package {
            names,
            imports: Vec::new(),
            exports: vec![super::super::package::Export {
                class: 0,
                sup: 0,
                template: 0,
                outer: 0,
                name: "Mesh".into(),
                flags: 0,
                size: 0,
                offset: 0,
                is_asset: true,
            }],
            header_size: 0,
            bulk_start: 0,
            uexp: Vec::new(),
            ubulk: Vec::new(),
        };
        let bytes = mesh_export(&p.names);
        let mut p = p;
        p.exports[0].size = bytes.len() as i64;
        p.uexp = bytes;
        let m = SkeletalMesh::read(&p, 0).unwrap();
        assert_eq!(m.bones[0].translation, [1., 2., 3.]);
        assert_eq!(m.materials[0].slot, "root");
        assert_eq!(m.lod.indices, [0, 1, 2]);
        assert_eq!(m.lod.positions[2], [2., 0., 0.]);
        assert_eq!(m.lod.uv0[0], [1., 0.5]);
        assert_eq!(m.lod.joints[0][0], 5);
        assert_eq!(m.lod.weights[0][0], 1.);
    }
}
