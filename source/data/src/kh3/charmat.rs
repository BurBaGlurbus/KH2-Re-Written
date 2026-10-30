//! Sora's and the Keyblade's materials as the game shades them: for each slot the instance chain's parameters, the shader
//! map of the nearest package that has one, its base-pass pixel shader translated to WGSL, and the material constant
//! buffer's uniform expressions. All from the player's own install, at run time. The way through the assets follows
//! StrawHatPro's `tools/sora_material_pack.js` (MIT).
use super::dxbc::{self, Bindings};
use super::package::Package;
use super::props::{Props, export_props};
use super::shadermap::{self, Expr, ShaderCode, ShaderMap};
use super::{Paks, Result, bad, material};
use std::collections::{BTreeMap, HashMap};

/// The vertex factory of a skinned mesh and the base pass's pixel shader for it.
const FACTORY: &str = "TGPUSkinVertexFactoryfalse";
const BASE_PS: &str = "TBasePassPSFCachedPointIndirectLightingPolicy";
const MASK_PS: &str = "FDepthOnlyPS";
const COLLECTIONS: &str = "kingdom hearts iii/content/shader/";

/// A shader owner: what several slots may share.
#[derive(Clone, Debug, Default)]
pub struct Owner {
    pub asset: String,
    pub description: String,
    pub base: String,
    pub mask: Option<String>,
    pub vectors: Vec<Expr>,
    pub scalars: Vec<Expr>,
    /// Texture slots: the parameter's name, or the root's reference.
    pub textures: Vec<(Option<String>, Option<String>)>,
    /// By `MaterialCollection<n>`: rows.
    pub collections: Vec<Vec<[f32; 4]>>,
    pub blend: String,
    pub two_sided: bool,
    pub subsurface: bool,
}

/// A material slot of a mesh.
#[derive(Clone, Debug, Default)]
pub struct Slot {
    pub name: String,
    /// The owner; none when this slot's material could not be translated: `why`.
    pub owner: Option<usize>,
    pub why: String,
    pub scalars: HashMap<String, f32>,
    pub vectors: HashMap<String, [f32; 4]>,
    /// By the owner's texture slot: the texture's `/Game/...` package.
    pub textures: Vec<Option<String>>,
}

#[derive(Clone, Debug, Default)]
pub struct CharMaterials {
    pub owners: Vec<Owner>,
    /// Sora's slots, then the Keyblade's.
    pub body: Vec<Slot>,
    pub blade: Vec<Slot>,
}

/// `FMaterialUniformExpression::GetNumberValue` of each class.
pub fn eval(
    e: &Expr,
    scalars: &HashMap<String, f32>,
    vectors: &HashMap<String, [f32; 4]>,
    time: f32,
) -> [f32; 4] {
    let ev = |x: &Expr| eval(x, scalars, vectors, time);
    let per = |x: [f32; 4], f: &dyn Fn(f32) -> f32| x.map(f);
    let two = |a: [f32; 4], b: [f32; 4], f: &dyn Fn(f32, f32) -> f32| {
        [f(a[0], b[0]), f(a[1], b[1]), f(a[2], b[2]), f(a[3], b[3])]
    };
    match e {
        Expr::Constant(c) => *c,
        Expr::Vector(name, default) => *vectors.get(name).unwrap_or(default),
        Expr::Scalar(name, default) => [*scalars.get(name).unwrap_or(default); 4],
        Expr::Time => [time; 4],
        Expr::Texture { .. } => [0.; 4],
        Expr::Sine(x, cosine) => {
            let v = ev(x);
            if *cosine {
                per(v, &f32::cos)
            } else {
                per(v, &f32::sin)
            }
        }
        Expr::Unary(op, x) => {
            let v = ev(x);
            match *op {
                "sqrt" => per(v, &|x| x.max(0.).sqrt()),
                "floor" => per(v, &f32::floor),
                "ceil" => per(v, &f32::ceil),
                "frac" | "period" => per(v, &|x| x - x.floor()),
                "abs" => per(v, &f32::abs),
                "saturate" => per(v, &|x| x.clamp(0., 1.)),
                "round" => per(v, &f32::round),
                "sign" => per(v, &|x| {
                    if x > 0. {
                        1.
                    } else if x < 0. {
                        -1.
                    } else {
                        0.
                    }
                }),
                "trunc" => per(v, &f32::trunc),
                "log2" => per(v, &f32::log2),
                "log10" => per(v, &f32::log10),
                _ => v,
            }
        }
        Expr::Folded {
            a,
            b,
            op,
            components,
        } => {
            let (x, y) = (ev(a), ev(b));
            match op {
                0 => two(x, y, &|a, b| a + b),
                1 => two(x, y, &|a, b| a - b),
                2 => two(x, y, &|a, b| a * b),
                3 => two(x, y, &|a, b| if b.abs() < 1e-5 { 0. } else { a / b }),
                4 => {
                    let n = match components {
                        1 => 1,
                        2 => 2,
                        4 => 3,
                        _ => 4,
                    };
                    [(0..n).map(|k| x[k] * y[k]).sum(); 4]
                }
                _ => [0.; 4],
            }
        }
        Expr::Append { a, b, components_a } => {
            let (x, y) = (ev(a), ev(b));
            let n = *components_a as usize;
            let mut out = [0.; 4];
            for (k, slot) in out.iter_mut().enumerate() {
                *slot = if k < n { x[k] } else { y[(k - n).min(3)] };
            }
            out
        }
        Expr::Binary(op, a, b) => {
            let (x, y) = (ev(a), ev(b));
            match *op {
                "min" => two(x, y, &f32::min),
                "max" => two(x, y, &f32::max),
                _ => two(x, y, &|a, b| if b == 0. { 0. } else { a % b }),
            }
        }
        Expr::Trig { x, y, op } => {
            let (a, b) = (ev(x), ev(y));
            let fin = |v: f32| if v.is_finite() { v } else { 0. };
            match op {
                0 => per(a, &|x| fin(x.sin())),
                1 => per(a, &|x| fin(x.cos())),
                2 => per(a, &|x| fin(x.tan())),
                3 => per(a, &|x| fin(x.asin())),
                4 => per(a, &|x| fin(x.acos())),
                5 => per(a, &|x| fin(x.atan())),
                _ => two(a, b, &f32::atan2),
            }
        }
        Expr::Clamp(x, lo, hi) => {
            let (v, l, h) = (ev(x), ev(lo), ev(hi));
            [
                v[0].max(l[0]).min(h[0]),
                v[1].max(l[1]).min(h[1]),
                v[2].max(l[2]).min(h[2]),
                v[3].max(l[3]).min(h[3]),
            ]
        }
        Expr::Swizzle { x, index, elements } => {
            let v = ev(x);
            let mut out = [0.; 4];
            for k in 0..(*elements).clamp(0, 4) as usize {
                if index[k] >= 0 {
                    out[k] = v[(index[k] as usize).min(3)];
                }
            }
            out
        }
    }
}

/// The material constant buffer: vector rows, then the scalars packed four to a row.
pub fn rows_of(owner: &Owner, slot: &Slot, time: f32, out: &mut [[f32; 4]]) {
    for (k, e) in owner.vectors.iter().enumerate() {
        if let Some(row) = out.get_mut(k) {
            *row = eval(e, &slot.scalars, &slot.vectors, time);
        }
    }
    for (k, e) in owner.scalars.iter().enumerate() {
        if let Some(row) = out.get_mut(owner.vectors.len() + k / 4) {
            row[k % 4] = eval(e, &slot.scalars, &slot.vectors, time)[0];
        }
    }
}

struct Chain {
    /// The package whose export has a shader map.
    owner: String,
    scalars: HashMap<String, f32>,
    vectors: HashMap<String, [f32; 4]>,
    textures: BTreeMap<String, String>,
    root: String,
}

/// An instance chain: most derived value wins; the root's own parameters are its expressions' defaults.
fn chain(paks: &Paks, asset: &str) -> Result<Chain> {
    let merged = material::read(paks, asset)?;
    let mut owner = None;
    let mut root = asset.to_owned();
    let mut at = asset.to_owned();
    for _ in 0..16 {
        let Some(p) = paks.game_package(&at)? else {
            break;
        };
        let Some(i) = p.export_of_classes(&["MaterialInstanceConstant", "Material"]) else {
            break;
        };
        if owner.is_none() && has_native(&p, i)? {
            owner = Some(at.clone());
        }
        root.clone_from(&at);
        let (props, _) = export_props(&p, i)?;
        match props
            .object("Parent")
            .and_then(|o| material::import_path(&p, o))
        {
            Some(next) => at = next,
            None => break,
        }
    }
    Ok(Chain {
        owner: owner.ok_or_else(|| bad(format!("no shader map in the chain of {asset}")))?,
        scalars: merged.scalars.into_iter().collect(),
        vectors: merged.vectors.into_iter().collect(),
        textures: merged.textures,
        root,
    })
}

fn has_native(p: &Package, i: usize) -> Result<bool> {
    let none = |_: &str| false;
    Ok(shadermap::read(p, i, &none).ok().flatten().is_some())
}

/// `Expressions`' texture references in order, through the function calls: what `TextureIndex` counts.
fn root_textures(paks: &Paks, asset: &str) -> Result<Vec<String>> {
    let mut out = Vec::new();
    let Some(p) = paks.game_package(asset)? else {
        return Ok(out);
    };
    let Some(i) = p.export_of_class("Material") else {
        return Ok(out);
    };
    let (props, _) = export_props(&p, i)?;
    let mut push = |path: String| {
        if !out.contains(&path) {
            out.push(path);
        }
    };
    let export_of =
        |v: &super::props::Value| v.as_object().filter(|o| *o > 0).map(|o| o as usize - 1);
    for e in props.array("Expressions").unwrap_or(&[]) {
        let Some(e) = export_of(e) else { continue };
        let Ok((node, _)) = export_props(&p, e) else {
            continue;
        };
        let class = p.class_of(e).to_owned();
        if class.ends_with("FunctionCall") {
            if let Some(f) = node
                .object("MaterialFunction")
                .and_then(|o| material::import_path(&p, o))
            {
                function_textures(paks, &f, &mut push);
            }
            continue;
        }
        expression_texture(&p, &class, &node, &mut push);
    }
    Ok(out)
}

fn expression_texture(p: &Package, class: &str, node: &Props, push: &mut dyn FnMut(String)) {
    match node.object("Texture") {
        // An engine texture counts too, though its path is not a game's.
        Some(t) if t < 0 => push(
            material::import_path(p, t).unwrap_or_else(|| format!("/Engine/{}", p.object_name(t))),
        ),
        None | Some(0)
            if matches!(
                class,
                "MaterialExpressionTextureObject"
                    | "MaterialExpressionTextureObjectParameter"
                    | "MaterialExpressionTextureSampleParameter2D"
            ) =>
        {
            push("/Engine/EngineResources/DefaultTexture".to_owned());
        }
        _ => {}
    }
}

/// The expressions of a function asset by export.
fn function_expressions(paks: &Paks, asset: &str) -> Option<(Package, Vec<usize>)> {
    let p = paks.game_package(asset).ok()??;
    let i = p.export_of_class("MaterialFunction")?;
    let (props, _) = export_props(&p, i).ok()?;
    let list = props
        .array("FunctionExpressions")
        .unwrap_or(&[])
        .iter()
        .filter_map(|v| v.as_object().filter(|o| *o > 0).map(|o| o as usize - 1))
        .collect();
    Some((p, list))
}

/// A function's dependencies as `UMaterialFunction::GetDependentFunctions` lists them: each one's own first.
fn dependent_functions(paks: &Paks, asset: &str, list: &mut Vec<String>) {
    let Some((p, exprs)) = function_expressions(paks, asset) else {
        return;
    };
    for e in exprs {
        if !p.class_of(e).ends_with("FunctionCall") {
            continue;
        }
        let Ok((node, _)) = export_props(&p, e) else {
            continue;
        };
        if let Some(f) = node
            .object("MaterialFunction")
            .and_then(|o| material::import_path(&p, o))
        {
            dependent_functions(paks, &f, list);
            if !list.contains(&f) {
                list.push(f);
            }
        }
    }
}

/// A function call's textures: the function's, then its dependencies'.
fn function_textures(paks: &Paks, asset: &str, push: &mut dyn FnMut(String)) {
    let mut list = vec![asset.to_owned()];
    dependent_functions(paks, asset, &mut list);
    for f in &list {
        let Some((p, exprs)) = function_expressions(paks, f) else {
            continue;
        };
        for e in exprs {
            if p.class_of(e).ends_with("FunctionCall") {
                continue;
            }
            if let Ok((node, _)) = export_props(&p, e) {
                expression_texture(&p, p.class_of(e), &node, push);
            }
        }
    }
}

/// A material parameter collection by its `StateId`: scalars packed four to a row, then one row per vector.
fn collection_rows(paks: &Paks, guid: [u32; 4]) -> Option<Vec<[f32; 4]>> {
    for path in paks.mounted.names() {
        let Some(stem) = path.strip_suffix(".uasset") else {
            continue;
        };
        if !stem.starts_with(COLLECTIONS) {
            continue;
        }
        let name = stem.rsplit('/').next().unwrap_or(stem).to_ascii_lowercase();
        if !(name.contains("control")
            || name.contains("param")
            || name.contains("mpc")
            || name.contains("col"))
        {
            continue;
        }
        let Ok(Some(p)) = paks.package(stem) else {
            continue;
        };
        let Some(i) = p.export_of_class("MaterialParameterCollection") else {
            continue;
        };
        let Ok((props, _)) = export_props(&p, i) else {
            continue;
        };
        let matches = match props.get("StateId") {
            Some(super::props::Value::Ints(v)) if v.len() == 4 => {
                [v[0] as u32, v[1] as u32, v[2] as u32, v[3] as u32] == guid
            }
            _ => false,
        };
        if !matches {
            continue;
        }
        let mut rows: Vec<[f32; 4]> = Vec::new();
        let scalars = props.array("ScalarParameters").unwrap_or(&[]);
        for (k, s) in scalars.iter().enumerate() {
            if k % 4 == 0 {
                rows.push([0.; 4]);
            }
            if let Some(s) = s.as_props() {
                rows[k / 4][k % 4] = s.f32("DefaultValue").unwrap_or(0.);
            }
        }
        for v in props.array("VectorParameters").unwrap_or(&[]) {
            let d = v.as_props().and_then(|v| v.floats("DefaultValue"));
            rows.push(match d {
                Some(d) if d.len() == 4 => [d[0], d[1], d[2], d[3]],
                _ => [0.; 4],
            });
        }
        return Some(rows);
    }
    None
}

fn bindings(
    code: &ShaderCode,
    map: &ShaderMap,
    no_discard: bool,
    clamp_of: &dyn Fn(usize) -> bool,
) -> Bindings {
    let mut b = Bindings {
        buffers: code.buffers.clone(),
        no_discard,
        ..Bindings::default()
    };
    let Some(slot) = code.buffers.iter().position(|n| n == "Material") else {
        return b;
    };
    let n2d = map.textures.len() as u32;
    let cubes = map.cubes as u32;
    for (bind, res) in shadermap::table_of(&code.texture_map, slot) {
        if res < 2 * n2d && res % 2 == 0 {
            b.textures.insert(bind, res / 2);
        }
    }
    for (bind, res) in shadermap::table_of(&code.sampler_map, slot) {
        let clamp = if res < 2 * n2d {
            clamp_of((res / 2) as usize)
        } else {
            // The two shared samplers: wrap, clamp.
            res.wrapping_sub(2 * (n2d + cubes)) == 1
        };
        b.clamps.insert(bind, clamp);
    }
    b
}

fn describe(map: &ShaderMap, key: &str) -> Option<String> {
    let at = map.description.find(key)? + key.len();
    let rest = &map.description[at..];
    Some(
        rest.split(|c: char| c == ',' || c.is_whitespace())
            .next()
            .unwrap_or("")
            .to_owned(),
    )
}

/// Whether a texture clamps (`AddressX`; the class default wraps).
fn clamps(paks: &Paks, path: &str) -> bool {
    let Ok(Some(p)) = paks.game_package(path) else {
        return false;
    };
    let Some(i) = p.export_of_class("Texture2D") else {
        return false;
    };
    export_props(&p, i).is_ok_and(|(props, _)| {
        props
            .str("AddressX")
            .is_some_and(|a| a.ends_with("TA_Clamp"))
    })
}

fn owner_of(paks: &Paks, chain: &Chain) -> Result<Owner> {
    let (asset, root) = (chain.owner.as_str(), chain.root.as_str());
    let p = paks
        .game_package(asset)?
        .ok_or_else(|| bad(format!("{asset} is missing")))?;
    let i = p
        .export_of_classes(&["MaterialInstanceConstant", "Material"])
        .ok_or_else(|| bad(format!("{asset} is no material")))?;
    let wanted = |t: &str| t == BASE_PS || t == MASK_PS;
    let map = shadermap::read(&p, i, &wanted)?
        .ok_or_else(|| bad(format!("{asset} has no shader map")))?;
    let (_, list) = map
        .meshes
        .iter()
        .find(|m| m.0 == FACTORY)
        .ok_or_else(|| bad(format!("{asset} has no skinned shaders")))?;
    let base = list
        .iter()
        .find(|s| s.0 == BASE_PS)
        .ok_or_else(|| bad(format!("{asset} has no base pass")))?;
    let refs = root_textures(paks, root)?;
    // The walk can miss what the engine's list holds (its order is a cooked-away editor list): the parameters' defaults
    // say where each of them really lies, so a plain sample's index counts from its nearest parameter below.
    let defaults = material::read(paks, root)?.textures;
    let mut anchors: Vec<(usize, usize)> = map
        .textures
        .iter()
        .filter_map(|e| match e {
            Expr::Texture {
                parameter: Some(name),
                index,
            } => {
                let at = refs.iter().position(|r| Some(r) == defaults.get(name))?;
                let index = usize::try_from(*index).ok()?;
                (index >= at).then_some((index, index - at))
            }
            _ => None,
        })
        .collect();
    anchors.sort_unstable();
    let reference = |index: usize| {
        let shift = anchors
            .iter()
            .take_while(|(at, _)| *at <= index)
            .last()
            .map_or(0, |(_, shift)| *shift);
        index.checked_sub(shift).and_then(|k| refs.get(k).cloned())
    };
    let textures: Vec<(Option<String>, Option<String>)> = map
        .textures
        .iter()
        .map(|e| match e {
            Expr::Texture { parameter, index } => (
                parameter.clone(),
                usize::try_from(*index).ok().and_then(reference),
            ),
            _ => (None, None),
        })
        .collect();
    // A texture's own sampler: its address mode, from the first instance that uses this shader map.
    let clamp_of = |k: usize| {
        textures.get(k).is_some_and(|(parameter, reference)| {
            parameter
                .as_ref()
                .and_then(|n| chain.textures.get(n))
                .or(reference.as_ref())
                .is_some_and(|path| clamps(paks, path))
        })
    };
    let blend = describe(&map, "BlendMode=").unwrap_or_default();
    let masked = blend == "BLEND_Masked";
    let mask_code = list.iter().find(|s| s.0 == MASK_PS).filter(|_| masked);
    let base_wgsl = dxbc::translate(
        &base.1.dxbc,
        &bindings(&base.1, &map, mask_code.is_some(), &clamp_of),
    )?;
    let mask = match mask_code {
        Some((_, code)) => {
            Some(dxbc::translate(&code.dxbc, &bindings(code, &map, false, &clamp_of))?.body)
        }
        None => None,
    };
    let collections = map
        .collections
        .iter()
        .map(|g| collection_rows(paks, *g).unwrap_or_default())
        .collect();
    Ok(Owner {
        asset: asset.to_owned(),
        two_sided: map.description.contains("TwoSided=1"),
        subsurface: describe(&map, "LightingModel=").is_some_and(|m| m.contains("Subsurface")),
        description: map.description.clone(),
        base: base_wgsl.body,
        mask,
        vectors: map.vectors.clone(),
        scalars: map.scalars.clone(),
        textures,
        collections,
        blend,
    })
}

fn slot_of(paks: &Paks, name: &str, asset: Option<&str>, owners: &mut Vec<Owner>) -> Slot {
    let mut slot = Slot {
        name: name.to_owned(),
        why: "no material".to_owned(),
        ..Slot::default()
    };
    let Some(asset) = asset else {
        return slot;
    };
    let chain = match chain(paks, asset) {
        Ok(c) => c,
        Err(e) => {
            slot.why = e.to_string();
            return slot;
        }
    };
    let at = match owners.iter().position(|o| o.asset == chain.owner) {
        Some(at) => at,
        None => match owner_of(paks, &chain) {
            Ok(o) => {
                owners.push(o);
                owners.len() - 1
            }
            Err(e) => {
                slot.why = e.to_string();
                return slot;
            }
        },
    };
    slot.textures = owners[at]
        .textures
        .iter()
        .map(|(parameter, reference)| {
            parameter
                .as_ref()
                .and_then(|n| chain.textures.get(n).cloned())
                .or_else(|| reference.clone())
        })
        .collect();
    slot.owner = Some(at);
    slot.scalars = chain.scalars;
    slot.vectors = chain.vectors;
    slot
}

/// Reads the slots of the two meshes: (slot name, material asset).
pub fn read(
    paks: &Paks,
    body: &[(String, Option<String>)],
    blade: &[(String, Option<String>)],
) -> CharMaterials {
    let mut out = CharMaterials::default();
    for (name, asset) in body {
        let s = slot_of(paks, name, asset.as_deref(), &mut out.owners);
        out.body.push(s);
    }
    for (name, asset) in blade {
        let s = slot_of(paks, name, asset.as_deref(), &mut out.owners);
        out.blade.push(s);
    }
    out
}

/// A depth-only shader's body as a function that answers 0 where it would clip.
pub fn mask_body(body: &str) -> String {
    let body = body.replace("{ discard; }", "{ return 0.0; }");
    let mut out = String::new();
    for line in body.lines() {
        if line.contains("return KhOut(") {
            out.push_str("    return 1.0;");
        } else {
            out.push_str(line);
        }
        out.push('\n');
    }
    out
}

/// All the owners' functions, for a look at them.
pub fn wgsl(owners: &[Owner]) -> String {
    let mut out = String::new();
    for (k, o) in owners.iter().enumerate() {
        out.push_str(&format!(
            "// {}\nfn kh_material_{k}(i: KhIn) -> KhOut {{\n{}}}\n",
            o.asset, o.base
        ));
    }
    out
}

/// Refuses an empty reading, for a caller that needs the materials.
pub fn check(m: &CharMaterials) -> Result<()> {
    if m.owners.is_empty() {
        return Err(bad("none of the materials could be read"));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn expressions_follow_the_engine() {
        let mut scalars = HashMap::new();
        scalars.insert("A".to_owned(), 3.);
        let vectors = HashMap::new();
        let e = Expr::Folded {
            a: Box::new(Expr::Scalar("A".into(), 1.)),
            b: Box::new(Expr::Constant([2.; 4])),
            op: 2,
            components: 1,
        };
        assert_eq!(eval(&e, &scalars, &vectors, 0.)[0], 6.);
        let d = Expr::Folded {
            a: Box::new(Expr::Scalar("A".into(), 1.)),
            b: Box::new(Expr::Constant([0.; 4])),
            op: 3,
            components: 1,
        };
        assert_eq!(
            eval(&d, &scalars, &vectors, 0.)[0],
            0.,
            "a division by nothing reads 0"
        );
        let s = Expr::Swizzle {
            x: Box::new(Expr::Constant([1., 2., 3., 4.])),
            index: [2, 0, -1, -1],
            elements: 2,
        };
        assert_eq!(eval(&s, &scalars, &vectors, 0.), [3., 1., 0., 0.]);
    }

    #[test]
    fn the_buffer_is_vector_rows_then_scalars_by_four() {
        let owner = Owner {
            vectors: vec![Expr::Constant([1.; 4])],
            scalars: vec![Expr::Constant([2.; 4]), Expr::Constant([3.; 4])],
            ..Owner::default()
        };
        let slot = Slot::default();
        let mut rows = [[0.; 4]; 3];
        rows_of(&owner, &slot, 0., &mut rows);
        assert_eq!(rows[0], [1.; 4]);
        assert_eq!(rows[1], [2., 3., 0., 0.]);
    }
}
