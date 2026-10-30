//! Material parameters without the shaders: a `MaterialInstanceConstant`'s
//! chain up to its root `Material`, with each texture, vector and scalar
//! parameter resolved root-most first, as UE overrides them. KH3's math nodes
//! do not survive cooking, so a look built from these is an approximation.
//! Walk from StrawHatPro's `tools/sora_vfx_pack.js` (MIT).
use super::package::Package;
use super::props::{Props, Value, export_props};
use super::{Paks, Result};
use std::collections::BTreeMap;

/// Blend modes as UE names them, without the prefix.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Blend {
    #[default]
    Opaque,
    Masked,
    Translucent,
    Additive,
    Modulate,
}

impl Blend {
    pub fn named(name: &str) -> Blend {
        match name.rsplit("BLEND_").next().unwrap_or(name) {
            "Masked" => Blend::Masked,
            "Translucent" => Blend::Translucent,
            "Additive" => Blend::Additive,
            "Modulate" => Blend::Modulate,
            _ => Blend::Opaque,
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct MaterialParams {
    /// Parameter name to the texture's `/Game/...` package.
    pub textures: BTreeMap<String, String>,
    pub vectors: BTreeMap<String, [f32; 4]>,
    pub scalars: BTreeMap<String, f32>,
    pub blend: Blend,
    pub two_sided: bool,
    /// The root material's package name, for choosing a recipe.
    pub root: String,
}

/// The `/Game/...` path of an import, if it is one.
pub fn import_path(p: &Package, index: i32) -> Option<String> {
    (index < 0).then(|| p.import_package(index).map(str::to_owned))?
}

fn parameter_name(row: &Props) -> Option<&str> {
    row.str("ParameterName")
        .or_else(|| row.props("ParameterInfo").and_then(|info| info.str("Name")))
}

fn apply_instance(p: &Package, props: &Props, out: &mut MaterialParams) {
    for row in props.array("TextureParameterValues").unwrap_or(&[]) {
        let Some(row) = row.as_props() else { continue };
        if let (Some(name), Some(object)) = (parameter_name(row), row.object("ParameterValue"))
            && let Some(path) = import_path(p, object)
        {
            out.textures.insert(name.to_owned(), path);
        }
    }
    for row in props.array("VectorParameterValues").unwrap_or(&[]) {
        let Some(row) = row.as_props() else { continue };
        if let (Some(name), Some(v)) = (parameter_name(row), row.floats("ParameterValue"))
            && v.len() == 4
        {
            out.vectors
                .insert(name.to_owned(), [v[0], v[1], v[2], v[3]]);
        }
    }
    for row in props.array("ScalarParameterValues").unwrap_or(&[]) {
        let Some(row) = row.as_props() else { continue };
        if let (Some(name), Some(v)) = (parameter_name(row), row.f32("ParameterValue")) {
            out.scalars.insert(name.to_owned(), v);
        }
    }
    if let Some(over) = props.props("BasePropertyOverrides") {
        if over.bool("bOverride_BlendMode") == Some(true)
            && let Some(b) = over.str("BlendMode")
        {
            out.blend = Blend::named(b);
        }
        if over.bool("bOverride_TwoSided") == Some(true)
            && let Some(t) = over.bool("TwoSided")
        {
            out.two_sided = t;
        }
    }
}

/// A root material: its blend, sides and each parameter node's default.
fn apply_root(p: &Package, i: usize, out: &mut MaterialParams) -> Result<()> {
    let (props, _) = export_props(p, i)?;
    if let Some(b) = props.str("BlendMode") {
        out.blend = Blend::named(b);
    }
    out.two_sided = props.bool("TwoSided").unwrap_or(false);
    out.root = p.exports[i].name.clone();
    for e in 0..p.exports.len() {
        let class = p.class_of(e);
        if !class.starts_with("MaterialExpression") || !class.contains("Parameter") {
            continue;
        }
        let Ok((node, _)) = export_props(p, e) else {
            continue;
        };
        let Some(name) = node.str("ParameterName") else {
            continue;
        };
        if class.contains("Texture") {
            if let Some(path) = node.object("Texture").and_then(|o| import_path(p, o)) {
                out.textures.entry(name.to_owned()).or_insert(path);
            }
        } else if class.contains("Vector") {
            if let Some(v) = node.floats("DefaultValue").filter(|v| v.len() == 4) {
                out.vectors
                    .entry(name.to_owned())
                    .or_insert([v[0], v[1], v[2], v[3]]);
            }
        } else if let Some(v) = node.f32("DefaultValue") {
            out.scalars.entry(name.to_owned()).or_insert(v);
        }
    }
    Ok(())
}

/// Reads a material or instance by `/Game/...` path, following `Parent`.
pub fn read(paks: &Paks, game_path: &str) -> Result<MaterialParams> {
    let mut chain = Vec::new();
    let mut at = game_path.to_owned();
    for _ in 0..16 {
        let Some(p) = paks.game_package(&at)? else {
            break;
        };
        let Some(i) = p.export_of_classes(&["MaterialInstanceConstant", "Material"]) else {
            break;
        };
        let (props, _) = export_props(&p, i)?;
        let parent = props.object("Parent").and_then(|o| import_path(&p, o));
        chain.push((p, i, props));
        match parent {
            Some(next) => at = next,
            None => break,
        }
    }
    let mut out = MaterialParams::default();
    for (p, i, props) in chain.iter().rev() {
        if p.class_of(*i) == "Material" {
            apply_root(p, *i, &mut out)?;
        } else {
            apply_instance(p, props, &mut out);
        }
    }
    Ok(out)
}

/// A value's short text, for the research probe.
pub fn describe(v: &Value) -> String {
    match v {
        Value::Floats(f) => format!("{f:?}"),
        Value::Float(f) => f.to_string(),
        Value::Str(s) | Value::Name(s) => s.clone(),
        other => format!("{other:?}"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn blend_names() {
        assert_eq!(Blend::named("BLEND_Additive"), Blend::Additive);
        assert_eq!(Blend::named("EBlendMode::BLEND_Masked"), Blend::Masked);
        assert_eq!(Blend::named("nonsense"), Blend::Opaque);
    }
}
