//! A cooked material's inline shader map (`FMaterialShaderMap`): its uniform expressions and the compiled shaders by
//! vertex factory, from the player's own install. The layout follows StrawHatPro's `docs/formats/material.md` (MIT).
use super::package::{Package, Rd, read_name};
use super::props::read_props;
use super::{Result, bad};

/// `FMaterialUniformExpression...`.
#[derive(Clone, Debug, PartialEq)]
pub enum Expr {
    Constant([f32; 4]),
    Vector(String, [f32; 4]),
    Scalar(String, f32),
    Texture {
        parameter: Option<String>,
        index: i32,
    },
    Time,
    Sine(Box<Expr>, bool),
    Unary(&'static str, Box<Expr>),
    Folded {
        a: Box<Expr>,
        b: Box<Expr>,
        op: u8,
        components: u32,
    },
    Append {
        a: Box<Expr>,
        b: Box<Expr>,
        components_a: u32,
    },
    Binary(&'static str, Box<Expr>, Box<Expr>),
    Trig {
        x: Box<Expr>,
        y: Box<Expr>,
        op: u8,
    },
    Clamp(Box<Expr>, Box<Expr>, Box<Expr>),
    Swizzle {
        x: Box<Expr>,
        index: [i8; 4],
        elements: i8,
    },
}

/// One shader's code as the D3D11 RHI stores it.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ShaderCode {
    pub dxbc: Vec<u8>,
    /// Uniform buffer names by constant buffer slot.
    pub buffers: Vec<String>,
    /// `FD3D11ShaderResourceTable`'s maps.
    pub texture_map: Vec<u32>,
    pub sampler_map: Vec<u32>,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct ShaderMap {
    pub description: String,
    pub vectors: Vec<Expr>,
    pub scalars: Vec<Expr>,
    pub textures: Vec<Expr>,
    pub cubes: usize,
    /// `ReferencedParameterCollections`' guids as the uniform buffers `MaterialCollection<n>` name them.
    pub collections: Vec<[u32; 4]>,
    /// (vertex factory, [(shader type, code)]).
    pub meshes: Vec<(String, Vec<(String, ShaderCode)>)>,
}

fn static_parameters(r: &mut Rd, names: &[String]) -> Result<()> {
    for _ in 0..r.count(32)? {
        read_name(r, names)?;
        r.skip(4 + 4 + 16)?;
    }
    for _ in 0..r.count(44)? {
        read_name(r, names)?;
        r.skip(16 + 4 + 16)?;
    }
    for _ in 0..r.count(32)? {
        read_name(r, names)?;
        r.skip(4 + 4 + 16)?;
    }
    Ok(())
}

fn guids(r: &mut Rd) -> Result<Vec<[u32; 4]>> {
    let n = r.count(16)?;
    let mut out = Vec::with_capacity(n);
    for _ in 0..n {
        out.push([r.u32()?, r.u32()?, r.u32()?, r.u32()?]);
    }
    Ok(out)
}

fn dependencies(r: &mut Rd, names: &[String]) -> Result<()> {
    for _ in 0..r.count(28)? {
        read_name(r, names)?;
        r.skip(20)?;
    }
    Ok(())
}

fn shader_map_id(r: &mut Rd, names: &[String]) -> Result<()> {
    r.skip(4 + 16 + 4 + 4)?;
    static_parameters(r, names)?;
    guids(r)?;
    guids(r)?;
    dependencies(r, names)?;
    dependencies(r, names)?;
    dependencies(r, names)?;
    r.skip(40)
}

fn colour(r: &mut Rd) -> Result<[f32; 4]> {
    Ok([r.f32()?, r.f32()?, r.f32()?, r.f32()?])
}

fn expression(r: &mut Rd, names: &[String]) -> Result<Expr> {
    let kind = read_name(r, names)?;
    let kind = kind
        .strip_prefix("FMaterialUniformExpression")
        .unwrap_or(&kind)
        .to_owned();
    let sub = |r: &mut Rd| -> Result<Box<Expr>> { expression(r, names).map(Box::new) };
    Ok(match kind.as_str() {
        "Constant" => {
            let v = colour(r)?;
            r.skip(1)?;
            Expr::Constant(v)
        }
        "VectorParameter" => {
            let name = read_name(r, names)?;
            Expr::Vector(name, colour(r)?)
        }
        "ScalarParameter" => {
            let name = read_name(r, names)?;
            Expr::Scalar(name, r.f32()?)
        }
        "Texture" | "FlipBookTextureParameter" => {
            let index = r.i32()?;
            r.skip(4)?;
            Expr::Texture {
                parameter: None,
                index,
            }
        }
        "TextureParameter" => {
            let name = read_name(r, names)?;
            let index = r.i32()?;
            r.skip(4)?;
            Expr::Texture {
                parameter: Some(name),
                index,
            }
        }
        "Time" | "RealTime" => Expr::Time,
        "Sine" => {
            let x = sub(r)?;
            Expr::Sine(x, r.u32()? != 0)
        }
        "SquareRoot" => Expr::Unary("sqrt", sub(r)?),
        "Period" => Expr::Unary("period", sub(r)?),
        "Floor" => Expr::Unary("floor", sub(r)?),
        "Ceil" => Expr::Unary("ceil", sub(r)?),
        "Frac" | "Periodic" => Expr::Unary("frac", sub(r)?),
        "Abs" => Expr::Unary("abs", sub(r)?),
        "Saturate" => Expr::Unary("saturate", sub(r)?),
        "Round" => Expr::Unary("round", sub(r)?),
        "Sign" => Expr::Unary("sign", sub(r)?),
        "Truncate" => Expr::Unary("trunc", sub(r)?),
        "Logarithm2" => Expr::Unary("log2", sub(r)?),
        "Logarithm10" => Expr::Unary("log10", sub(r)?),
        "TrigMath" => {
            let (x, y) = (sub(r)?, sub(r)?);
            Expr::Trig { x, y, op: r.u8()? }
        }
        "FoldedMath" => {
            let (a, b) = (sub(r)?, sub(r)?);
            let op = r.u8()?;
            Expr::Folded {
                a,
                b,
                op,
                components: r.u32()?,
            }
        }
        "AppendVector" => {
            let (a, b) = (sub(r)?, sub(r)?);
            Expr::Append {
                a,
                b,
                components_a: r.u32()?,
            }
        }
        "Min" => Expr::Binary("min", sub(r)?, sub(r)?),
        "Max" => Expr::Binary("max", sub(r)?, sub(r)?),
        "Fmod" => Expr::Binary("fmod", sub(r)?, sub(r)?),
        "Clamp" => Expr::Clamp(sub(r)?, sub(r)?, sub(r)?),
        "ComponentSwizzle" => {
            let x = sub(r)?;
            let b = r.take(5)?;
            Expr::Swizzle {
                x,
                index: [b[0] as i8, b[1] as i8, b[2] as i8, b[3] as i8],
                elements: b[4] as i8,
            }
        }
        other => return Err(bad(format!("a uniform expression {other}"))),
    })
}

fn expressions(r: &mut Rd, names: &[String]) -> Result<Vec<Expr>> {
    let n = r.count(8)?;
    let mut out = Vec::with_capacity(n);
    for _ in 0..n {
        out.push(expression(r, names)?);
    }
    Ok(out)
}

/// The code of a shader record, found by its container: the resource table lies before `DXBC`, optional data after.
fn shader_code(bytes: &[u8]) -> Option<ShaderCode> {
    let at = bytes.windows(4).position(|w| w == b"DXBC")?;
    let size = u32::from_le_bytes(bytes.get(at + 24..at + 28)?.try_into().ok()?) as usize;
    let dxbc = bytes.get(at..at + size)?.to_vec();
    // The code array is the record's end: its int32 size lies right before the resource table, so the table is the
    // one that parses from a size field ending exactly at the record's end.
    let mut table = None;
    let mut k = at.checked_sub(4)?;
    let floor = at.saturating_sub(16_384);
    while k > floor {
        k -= 1;
        let Some(field) = bytes.get(k..k + 4) else {
            continue;
        };
        let code_size = u32::from_le_bytes(field.try_into().ok()?) as usize;
        if k + 4 + code_size == bytes.len() {
            table = Some(k + 4);
            break;
        }
    }
    let start = table?;
    let mut r = Rd::at(bytes, start);
    r.u32().ok()?;
    let list = |r: &mut Rd| -> Option<Vec<u32>> {
        let n = r.count(4).ok()?;
        let mut v = Vec::with_capacity(n);
        for _ in 0..n {
            v.push(r.u32().ok()?);
        }
        Some(v)
    };
    let _views = list(&mut r)?;
    let sampler_map = list(&mut r)?;
    let _unordered = list(&mut r)?;
    let _hashes = list(&mut r)?;
    let texture_map = list(&mut r)?;
    if r.p != at {
        return None;
    }
    // Optional data: { u8 key, int32 size, data }, then its total size.
    let mut buffers = Vec::new();
    let end = bytes.len().checked_sub(4)?;
    let mut o = Rd::at(bytes, at + size);
    while o.p < end {
        let key = o.u8().ok()?;
        let n = usize::try_from(o.i32().ok()?).ok()?;
        let data = o.take(n).ok()?;
        if key == b'u' {
            let mut d = Rd::new(data);
            for _ in 0..d.count(4).ok()? {
                buffers.push(d.fstring().ok()?);
            }
        }
    }
    Some(ShaderCode {
        dxbc,
        buffers,
        texture_map,
        sampler_map,
    })
}

/// `Shaders`: count + { FName type, int32 end offset in the package's whole file, the record }.
fn shaders(
    r: &mut Rd,
    names: &[String],
    base: usize,
    wanted: &dyn Fn(&str) -> bool,
) -> Result<Vec<(String, ShaderCode)>> {
    let n = r.count(12)?;
    let mut out = Vec::new();
    for _ in 0..n {
        let kind = read_name(r, names)?;
        let start = r.p;
        let end = usize::try_from(r.i32()?)
            .ok()
            .and_then(|e| e.checked_sub(base))
            .filter(|e| *e > start && *e <= r.b.len())
            .ok_or_else(|| bad("a shader's end offset is out of range"))?;
        if wanted(&kind)
            && let Some(code) = shader_code(&r.b[r.p..end])
        {
            out.push((kind, code));
        }
        r.p = end;
    }
    Ok(out)
}

fn pipelines(r: &mut Rd, names: &[String], base: usize) -> Result<()> {
    for _ in 0..r.count(12)? {
        read_name(r, names)?;
        let none = |_: &str| false;
        shaders(r, names, base, &none)?;
    }
    Ok(())
}

/// The first valid shader map of a `Material` or `MaterialInstanceConstant` export; none when it has no native data.
/// `wanted`: the shader types whose code is kept.
pub fn read(
    p: &Package,
    export: usize,
    wanted: &dyn Fn(&str) -> bool,
) -> Result<Option<ShaderMap>> {
    let bytes = p.export_bytes(export)?;
    let names = &p.names;
    let mut r = Rd::new(bytes);
    read_props(&mut r, names)?;
    r.i32()?;
    if p.class_of(export) != "Material" {
        if r.left() == 0 {
            return Ok(None);
        }
        static_parameters(&mut r, names)?;
    }
    if r.left() < 4 {
        return Ok(None);
    }
    let resources = r.count(1)?;
    let base = usize::try_from(p.exports[export].offset).map_err(|_| bad("an export's offset"))?;
    for _ in 0..resources {
        let _cooked = r.u32()?;
        if r.u32()? == 0 {
            continue;
        }
        shader_map_id(&mut r, names)?;
        r.skip(4)?;
        r.fstring()?;
        let vectors = expressions(&mut r, names)?;
        let scalars = expressions(&mut r, names)?;
        let textures = expressions(&mut r, names)?;
        let cubes = expressions(&mut r, names)?.len();
        expressions(&mut r, names)?;
        let collections = guids(&mut r)?;
        for _ in 0..4 {
            expressions(&mut r, names)?;
        }
        r.skip(9 * 4)?;
        let description = r.fstring()?;
        let none = |_: &str| false;
        shaders(&mut r, names, base, &none)?;
        pipelines(&mut r, names, base)?;
        let mut meshes = Vec::new();
        for _ in 0..r.count(12)? {
            let factory = read_name(&mut r, names)?;
            let list = shaders(&mut r, names, base, wanted)?;
            pipelines(&mut r, names, base)?;
            meshes.push((factory, list));
        }
        return Ok(Some(ShaderMap {
            description,
            vectors,
            scalars,
            textures,
            cubes,
            collections,
            meshes,
        }));
    }
    Ok(None)
}

/// The texture and sampler registers a material's shader reads: `(bind, resource)` of the uniform buffer `slot`
/// (`e >> 24` is the buffer, `(e >> 8) & 0xffff` the resource, `e & 0xff` the bind; 0xffffffff ends a run).
pub fn table_of(map: &[u32], slot: usize) -> Vec<(u32, u32)> {
    let Some(&offset) = map.get(slot) else {
        return Vec::new();
    };
    let offset = offset as usize;
    if offset == 0 || offset >= map.len() {
        return Vec::new();
    }
    map[offset..]
        .iter()
        .take_while(|e| **e != u32::MAX && (*e >> 24) as usize == slot)
        .map(|e| (e & 0xff, (e >> 8) & 0xffff))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_table_entry_gives_its_bind_and_resource() {
        // Slot 1's run starts at 2: two entries of buffer 1.
        let map = [
            0,
            2,
            (1 << 24) | (3 << 8) | 5,
            (1 << 24) | (4 << 8) | 6,
            u32::MAX,
        ];
        assert_eq!(table_of(&map, 1), vec![(5, 3), (6, 4)]);
        assert!(table_of(&map, 0).is_empty());
    }

    #[test]
    fn code_is_found_by_its_container() {
        // size field, table (bits and five empty lists), container of 32 bytes, optional data.
        let mut dxbc = b"DXBC".to_vec();
        dxbc.resize(24, 0);
        dxbc.extend_from_slice(&32u32.to_le_bytes());
        dxbc.resize(32, 0);
        let mut code = vec![0u8; 4 + 5 * 4];
        code.extend_from_slice(&dxbc);
        let mut names = Vec::new();
        names.extend_from_slice(&1i32.to_le_bytes());
        names.extend_from_slice(&9i32.to_le_bytes());
        names.extend_from_slice(b"Material\0");
        code.push(b'u');
        code.extend_from_slice(&(names.len() as i32).to_le_bytes());
        code.extend_from_slice(&names);
        code.extend_from_slice(&0i32.to_le_bytes());
        let mut record = vec![7u8; 8];
        record.extend_from_slice(&(code.len() as i32).to_le_bytes());
        record.extend_from_slice(&code);
        let found = shader_code(&record).unwrap();
        assert_eq!(found.dxbc.len(), 32);
        assert_eq!(found.buffers, vec!["Material".to_owned()]);
    }
}
