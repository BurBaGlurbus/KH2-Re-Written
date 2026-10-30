//! `UStaticMesh` exports for mesh particles: the first LOD's positions, UVs,
//! vertex colours, indices and sections. Layout from StrawHatPro's
//! `docs/formats/staticmesh.md` (MIT).
use super::package::{Package, Rd, read_name};
use super::props::read_props;
use super::{Paks, Result, bad};

#[derive(Clone, Debug, Default, PartialEq)]
pub struct StaticSection {
    pub material: i32,
    pub first_index: u32,
    pub triangles: u32,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct StaticMesh {
    pub positions: Vec<[f32; 3]>,
    pub uv: Vec<[f32; 2]>,
    pub colors: Vec<[u8; 4]>,
    pub indices: Vec<u32>,
    pub sections: Vec<StaticSection>,
    /// The slots' materials, `/Game/...` packages.
    pub materials: Vec<String>,
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

fn read_lod(r: &mut Rd, out: &mut StaticMesh) -> Result<()> {
    r.skip(2)?;
    let sections = r.count(28)?;
    for _ in 0..sections {
        let material = r.i32()?;
        let first_index = r.u32()?;
        let triangles = r.u32()?;
        r.skip(16)?;
        out.sections.push(StaticSection {
            material,
            first_index,
            triangles,
        });
    }
    r.f32()?;
    let stride = r.i32()? as usize;
    let vertices = r.i32()? as usize;
    let element = r.i32()? as usize;
    let count = r.count(element)?;
    if stride != 12 || element != 12 || count != vertices {
        return Err(bad("unexpected static mesh positions"));
    }
    let raw = r.take(count * 12)?;
    out.positions = raw
        .chunks_exact(12)
        .map(|c| {
            let f = |o: usize| f32::from_le_bytes([c[o], c[o + 1], c[o + 2], c[o + 3]]);
            [f(0), f(4), f(8)]
        })
        .collect();
    r.skip(2)?;
    r.i32()?;
    let stride = r.i32()? as usize;
    let _ = r.i32()?;
    let full = r.i32()? != 0;
    let high = r.i32()? != 0;
    let element = r.i32()? as usize;
    let count = r.count(element)?;
    if element != stride {
        return Err(bad("unexpected static mesh vertices"));
    }
    let raw = r.take(count * element)?;
    let skip = if high { 16 } else { 8 };
    out.uv = raw
        .chunks_exact(element)
        .map(|v| {
            if full {
                [
                    f32::from_le_bytes([v[skip], v[skip + 1], v[skip + 2], v[skip + 3]]),
                    f32::from_le_bytes([v[skip + 4], v[skip + 5], v[skip + 6], v[skip + 7]]),
                ]
            } else {
                [
                    half(u16::from_le_bytes([v[skip], v[skip + 1]])),
                    half(u16::from_le_bytes([v[skip + 2], v[skip + 3]])),
                ]
            }
        })
        .collect();
    r.skip(2)?;
    let _ = r.i32()?;
    let colors = r.i32()? as usize;
    if colors > 0 {
        let element = r.i32()? as usize;
        let n = r.count(element)?;
        let raw = r.take(n * element)?;
        out.colors = raw
            .chunks_exact(4)
            .map(|c| [c[2], c[1], c[0], c[3]])
            .collect();
    }
    let wide = r.i32()? != 0;
    let element = r.i32()? as usize;
    let n = r.count(element)?;
    let raw = r.take(n * element)?;
    out.indices = if wide {
        raw.chunks_exact(4)
            .map(|c| u32::from_le_bytes([c[0], c[1], c[2], c[3]]))
            .collect()
    } else {
        raw.chunks_exact(2)
            .map(|c| u32::from(u16::from_le_bytes([c[0], c[1]])))
            .collect()
    };
    Ok(())
}

impl StaticMesh {
    pub fn read(p: &Package, i: usize) -> Result<Self> {
        let bytes = p.export_bytes(i)?;
        let mut r = Rd::new(bytes);
        let props = read_props(&mut r, &p.names)?;
        r.i32()?;
        r.skip(2)?;
        if r.i32()? != 1 {
            return Err(bad("static mesh is not cooked"));
        }
        r.skip(8 + 16)?;
        let sockets = r.count(4)?;
        r.skip(sockets * 4)?;
        let lods = r.i32()?;
        if lods < 1 {
            return Err(bad("static mesh without LODs"));
        }
        let mut out = StaticMesh::default();
        read_lod(&mut r, &mut out)?;
        for slot in props.array("StaticMaterials").unwrap_or(&[]) {
            let path = slot
                .as_props()
                .and_then(|s| s.object("MaterialInterface"))
                .and_then(|o| p.import_package(o))
                .unwrap_or("");
            out.materials.push(path.to_owned());
        }
        let _ = read_name;
        Ok(out)
    }

    /// A mesh by `/Game/...` path.
    pub fn load(paks: &Paks, game_path: &str) -> Result<Self> {
        let package = game_path.split('.').next().unwrap_or(game_path);
        let p = paks
            .game_package(package)?
            .ok_or_else(|| bad(format!("{package} is missing")))?;
        let i = p
            .export_of_class("StaticMesh")
            .ok_or_else(|| bad(format!("{package} has no mesh")))?;
        Self::read(&p, i)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn half_floats_decode() {
        assert_eq!(half(0x3c00), 1.);
        assert_eq!(half(0x3800), 0.5);
    }

    #[test]
    fn a_first_lod_is_read() {
        let mut e = Vec::new();
        e.extend([1, 0]);
        e.extend(1i32.to_le_bytes());
        e.extend(0i32.to_le_bytes());
        e.extend(0u32.to_le_bytes());
        e.extend(1u32.to_le_bytes());
        e.extend([0; 16]);
        e.extend(0f32.to_le_bytes());
        // Positions.
        e.extend(12i32.to_le_bytes());
        e.extend(3i32.to_le_bytes());
        e.extend(12i32.to_le_bytes());
        e.extend(3i32.to_le_bytes());
        for i in 0..3 {
            for f in [i as f32, 0., 0.] {
                e.extend(f.to_le_bytes());
            }
        }
        // Vertices: tangents, half UVs.
        e.extend([1, 0]);
        e.extend(1i32.to_le_bytes());
        e.extend(12i32.to_le_bytes());
        e.extend(3i32.to_le_bytes());
        e.extend(0i32.to_le_bytes());
        e.extend(0i32.to_le_bytes());
        e.extend(12i32.to_le_bytes());
        e.extend(3i32.to_le_bytes());
        for _ in 0..3 {
            e.extend([0; 8]);
            e.extend(0x3c00u16.to_le_bytes());
            e.extend(0x3800u16.to_le_bytes());
        }
        // Colours.
        e.extend([1, 0]);
        e.extend(4i32.to_le_bytes());
        e.extend(0i32.to_le_bytes());
        // Indices.
        e.extend(0i32.to_le_bytes());
        e.extend(1i32.to_le_bytes());
        e.extend(6i32.to_le_bytes());
        for i in [0u16, 1, 2] {
            e.extend(i.to_le_bytes());
        }
        let mut r = Rd::new(&e);
        let mut m = StaticMesh::default();
        read_lod(&mut r, &mut m).unwrap();
        assert_eq!(m.positions[2], [2., 0., 0.]);
        assert_eq!(m.uv[0], [1., 0.5]);
        assert_eq!(m.indices, [0, 1, 2]);
        assert_eq!(m.sections[0].triangles, 1);
    }
}
