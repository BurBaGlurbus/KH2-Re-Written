//! A `ParticleSystem` package to the JSON shape `effect::cascade` reads: class
//! defaults overlaid with the tags the asset serializes, each distribution's
//! baked table, in the order Cascade runs them. Walk from StrawHatPro's
//! `tools/sora_vfx_pack.js` (MIT). Only what the player's own paks hold goes
//! through here, at run time.
//!
//! Cooked objects leave out properties equal to their class default, and those
//! defaults live in the executable's constructors. `class_defaults` holds the
//! few whose absence changes behaviour.
use super::package::Package;
use super::props::{Prop, Props, Value, export_props};
use super::{Paks, Result, bad};
use serde_json::{Map, Value as J, json};

pub struct BuiltEffect {
    pub name: String,
    pub json: J,
    pub materials: Vec<String>,
    pub meshes: Vec<String>,
    pub nested: Vec<String>,
}

/// The base name a `/Game/.../name.name` path is known by.
pub fn base_name(path: &str) -> String {
    let tail = path.rsplit('/').next().unwrap_or(path);
    tail.split('.').next().unwrap_or(tail).to_owned()
}

fn native_json(struct_name: &str, f: &[f32]) -> J {
    let num = |v: f32| J::from(f64::from(v));
    match (struct_name, f.len()) {
        ("Rotator", 3) => json!({"pitch":num(f[0]),"yaw":num(f[1]),"roll":num(f[2])}),
        ("LinearColor", 4) => json!({"r":num(f[0]),"g":num(f[1]),"b":num(f[2]),"a":num(f[3])}),
        ("Quat" | "Vector4", 4) => json!({"x":num(f[0]),"y":num(f[1]),"z":num(f[2]),"w":num(f[3])}),
        (_, 2) => json!({"x":num(f[0]),"y":num(f[1])}),
        (_, 3) => json!({"x":num(f[0]),"y":num(f[1]),"z":num(f[2])}),
        _ => J::Array(f.iter().map(|v| num(*v)).collect()),
    }
}

struct Walk<'a> {
    p: &'a Package,
    materials: Vec<String>,
    meshes: Vec<String>,
    nested: Vec<String>,
}

impl Walk<'_> {
    fn note(&mut self, class: &str, path: &str) {
        let list = match class {
            c if c.contains("Material") => &mut self.materials,
            c if c.contains("Mesh") => &mut self.meshes,
            "ParticleSystem" => &mut self.nested,
            _ => return,
        };
        if !list.iter().any(|x| x == path) {
            list.push(path.to_owned());
        }
    }

    fn object(&mut self, index: i32) -> J {
        if index == 0 {
            return J::Null;
        }
        if index > 0 {
            let e = (index as usize) - 1;
            let class = self.p.class_of(e).to_owned();
            if class.contains("Distribution") && !class.starts_with("Particle") {
                return json!({"distribution": self.distribution(index, None)});
            }
            let name = self
                .p
                .exports
                .get(e)
                .map_or("", |x| x.name.as_str())
                .to_owned();
            return json!({"export":name,"class":class});
        }
        let Some(import) = self.p.imports.get(index.unsigned_abs() as usize - 1) else {
            return J::Null;
        };
        let (class, name) = (import.class.clone(), import.name.clone());
        match self.p.import_package(index) {
            Some(package) if package.starts_with('/') => {
                let path = format!("{package}.{name}");
                self.note(&class, package);
                J::String(path)
            }
            _ => J::String(name),
        }
    }

    fn table(&mut self, t: &Props) -> J {
        let values: Vec<J> = t
            .array("Values")
            .unwrap_or(&[])
            .iter()
            .map(|v| J::from(f64::from(v.as_f32().unwrap_or(0.))))
            .collect();
        json!({
            "op": t.int("Op").unwrap_or(0),
            "entry_count": t.int("EntryCount").unwrap_or(0),
            "entry_stride": t.int("EntryStride").unwrap_or(0),
            "sub_entry_stride": t.int("SubEntryStride").unwrap_or(0),
            "time_scale": f64::from(t.f32("TimeScale").unwrap_or(0.)),
            "time_bias": f64::from(t.f32("TimeBias").unwrap_or(0.)),
            "lock_flag": t.int("LockFlag").unwrap_or(0),
            "values": values,
        })
    }

    /// A distribution: the raw struct's table, and the object behind it.
    fn distribution(&mut self, object: i32, raw: Option<&Props>) -> J {
        let mut out = Map::new();
        let index = if object != 0 {
            object
        } else {
            raw.and_then(|r| r.object("Distribution")).unwrap_or(0)
        };
        if let Some(table) = raw.and_then(|r| r.props("Table")) {
            out.insert("lookup_table".into(), self.table(table));
        }
        if index > 0
            && let Ok((d, _)) = export_props(self.p, index as usize - 1)
        {
            let class = self.p.class_of(index as usize - 1).to_owned();
            let vector = class.contains("Vector");
            out.insert("class".into(), J::String(class.clone()));
            let value = |k: &str| -> J {
                match d.get(k) {
                    Some(Value::Float(f)) => J::from(f64::from(*f)),
                    Some(Value::Floats(f)) => {
                        J::Array(f.iter().map(|v| J::from(f64::from(*v))).collect())
                    }
                    _ if vector => json!([0., 0., 0.]),
                    _ => J::from(0.),
                }
            };
            out.insert("constant".into(), value("Constant"));
            out.insert("min".into(), value("Min"));
            out.insert("max".into(), value("Max"));
            if class.starts_with("SQEX") {
                let mut props = Map::new();
                self.props_into(&d, &mut props);
                out.insert("properties".into(), J::Object(props));
            }
        }
        J::Object(out)
    }

    fn value(&mut self, prop: Option<&Prop>, v: &Value) -> J {
        let struct_name = prop.map_or("", |p| p.struct_name.as_str());
        match v {
            Value::Bool(b) => J::Bool(*b),
            Value::Int(i) => J::from(*i),
            Value::Byte(b) => J::from(*b),
            Value::Float(f) => J::from(f64::from(*f)),
            Value::Str(s) | Value::Name(s) => J::String(s.clone()),
            Value::Object(o) => self.object(*o),
            Value::Floats(f) => native_json(struct_name, f),
            Value::Ints(i) => J::Array(i.iter().map(|x| J::from(*x)).collect()),
            Value::Struct(name, props) => {
                if name == "RawDistributionFloat" || name == "RawDistributionVector" {
                    return json!({"distribution": self.distribution(0, Some(props))});
                }
                let mut o = Map::new();
                self.props_into(props, &mut o);
                J::Object(o)
            }
            Value::Array(items) => J::Array(items.iter().map(|i| self.value(None, i)).collect()),
            Value::Map(_) | Value::Raw(_) => J::Null,
        }
    }

    fn props_into(&mut self, props: &Props, out: &mut Map<String, J>) {
        for p in &props.0 {
            let v = self.value(Some(p), &p.value);
            let key = if p.index != 0 {
                format!("{}[{}]", p.name, p.index)
            } else {
                p.name.clone()
            };
            out.insert(key, v);
        }
    }

    /// Class defaults overlaid with the serialized tags.
    fn params(&mut self, export: usize) -> Result<Map<String, J>> {
        let (props, _) = export_props(self.p, export)?;
        let class = self.p.class_of(export);
        let mut params = class_defaults(class);
        let mut own = Map::new();
        self.props_into(&props, &mut own);
        for (k, v) in own {
            match (params.get_mut(&k), &v) {
                (Some(J::Object(have)), J::Object(new)) if !new.contains_key("distribution") => {
                    for (kk, vv) in new {
                        have.insert(kk.clone(), vv.clone());
                    }
                }
                _ => {
                    params.insert(k, v);
                }
            }
        }
        Ok(params)
    }

    fn module(&mut self, index: i32) -> Result<J> {
        let e = index as usize - 1;
        let params = self.params(e)?;
        let enabled = params.get("bEnabled").and_then(J::as_bool).unwrap_or(true);
        Ok(json!({
            "class": self.p.class_of(e),
            "enabled": enabled,
            "params": J::Object(params),
        }))
    }

    fn emitter(&mut self, index: i32) -> Result<J> {
        if index <= 0 {
            return Ok(J::Null);
        }
        let e = index as usize - 1;
        let (props, _) = export_props(self.p, e)?;
        let name = match props.str("EmitterName") {
            Some(n) if n != "None" => n.to_owned(),
            _ => self.p.exports[e].name.clone(),
        };
        let render_mode = props.str("EmitterRenderMode").map(str::to_owned);
        let lods = props.array("LODLevels").unwrap_or(&[]);
        let lod0 = lods.iter().find_map(Value::as_object).filter(|o| *o > 0);
        let Some(lod0) = lod0 else {
            return Ok(json!({"name":name,"enabled":false,"required":{},"modules":[]}));
        };
        let (lod, _) = export_props(self.p, lod0 as usize - 1)?;
        let enabled = lod.bool("bEnabled").unwrap_or(true);
        let required = lod.object("RequiredModule").unwrap_or(0);
        let spawn = lod.object("SpawnModule").unwrap_or(0);
        let typedata = lod.object("TypeDataModule").unwrap_or(0);
        let mut out = Map::new();
        out.insert("name".into(), J::String(name));
        out.insert("enabled".into(), J::Bool(enabled));
        if let Some(mode) = render_mode {
            out.insert("render_mode".into(), J::String(mode));
        }
        if required > 0 {
            let module = self.module(required)?;
            out.insert("required".into(), required_json(&module));
        }
        if spawn > 0 {
            let module = self.module(spawn)?;
            out.insert("spawn".into(), spawn_json(&module));
        }
        if typedata > 0 {
            let module = self.module(typedata)?;
            let class = module["class"].as_str().unwrap_or("").to_owned();
            let mut t = Map::new();
            t.insert("kind".into(), J::String(typedata_kind(&class)));
            t.insert("params".into(), module["params"].clone());
            if let Some(mesh) = module["params"]["Mesh"].as_str() {
                t.insert("mesh".into(), J::String(mesh.to_owned()));
            }
            let lists = sub_mesh_files(&module["params"]);
            if !lists.is_empty() {
                t.insert("sub_mesh_files".into(), J::Array(lists));
            }
            out.insert("typedata".into(), J::Object(t));
        } else {
            out.insert("typedata".into(), json!({"kind":"sprite"}));
        }
        let mut modules = Vec::new();
        for m in lod.array("Modules").unwrap_or(&[]) {
            if let Some(o) = m.as_object().filter(|o| *o > 0) {
                modules.push(self.module(o)?);
            }
        }
        out.insert("modules".into(), J::Array(modules));
        Ok(J::Object(out))
    }
}

fn sub_mesh_files(params: &J) -> Vec<J> {
    params["SubMeshLists"]
        .as_array()
        .map(|lists| {
            lists
                .iter()
                .map(|l| {
                    J::Array(
                        l["SubMeshList"]
                            .as_array()
                            .map(|a| {
                                a.iter()
                                    .filter_map(|m| m["SubMesh"].as_str())
                                    .map(|s| J::String(s.to_owned()))
                                    .collect()
                            })
                            .unwrap_or_default(),
                    )
                })
                .collect()
        })
        .unwrap_or_default()
}

fn typedata_kind(class: &str) -> String {
    match class {
        "ParticleModuleTypeDataMesh" => "mesh".into(),
        "ParticleModuleTypeDataRibbon" => "ribbon".into(),
        "ParticleModuleTypeDataBeam2" => "beam".into(),
        "ParticleModuleTypeDataAnimTrail" => "anim_trail".into(),
        "ParticleModuleTypeDataGpu" => "gpu_sprite".into(),
        "ParticleModuleTypeDataSQEX_Control" => "sqex_control".into(),
        "ParticleModuleTypeDataSQEX_TailPolygon" => "sqex_tail_polygon".into(),
        "ParticleModuleTypeDataSQEX_TailShape" => "sqex_tail_shape".into(),
        other => other
            .strip_prefix("ParticleModuleTypeData")
            .unwrap_or(other)
            .to_ascii_lowercase(),
    }
}

fn required_json(module: &J) -> J {
    let p = &module["params"];
    let get = |k: &str| p.get(k).cloned().unwrap_or(J::Null);
    json!({
        "material": get("Material"),
        "screen_alignment": get("ScreenAlignment"),
        "use_local_space": get("bUseLocalSpace"),
        "use_emitter_local_space": get("bUseEmitterLocalSpace"),
        "kill_on_deactivate": get("bKillOnDeactivate"),
        "kill_on_completed": get("bKillOnCompleted"),
        "duration": get("EmitterDuration"),
        "loops": get("EmitterLoops"),
        "delay": get("EmitterDelay"),
        "sqex_emitter_time": {
            "start_time": get("EmitterStartTime"),
            "loop_time": get("EmitterLoopTime"),
            "loop_duration": get("EmitterLoopDuration"),
            "duration_time": get("EmitterDurationTime"),
            "use_legacy_emitter_time": get("bUseLegacyEmitterTime"),
        },
        "sub_uv": {
            "horizontal": get("SubImages_Horizontal"),
            "vertical": get("SubImages_Vertical"),
            "interpolation": get("InterpolationMethod"),
        },
        "use_max_draw_count": get("bUseMaxDrawCount"),
        "max_draw_count": get("MaxDrawCount"),
        "emitter_origin": get("EmitterOrigin"),
        "emitter_rotation": get("EmitterRotation"),
        "params": p,
    })
}

fn spawn_json(module: &J) -> J {
    let p = &module["params"];
    let get = |k: &str| p.get(k).cloned().unwrap_or(J::Null);
    let bursts: Vec<J> = p["BurstList"]
        .as_array()
        .map(|a| {
            a.iter()
                .map(|b| json!({"count":b["Count"],"count_low":b["CountLow"],"time":b["Time"]}))
                .collect()
        })
        .unwrap_or_default();
    let dist = |k: &str| {
        p.get(k)
            .and_then(|d| d.get("distribution"))
            .cloned()
            .unwrap_or(J::Null)
    };
    json!({
        "enabled": module["enabled"],
        "process_spawn_rate": get("bProcessSpawnRate"),
        "process_burst_list": get("bProcessBurstList"),
        "rate": dist("Rate"),
        "rate_scale": dist("RateScale"),
        "bursts": bursts,
        "burst_scale": dist("BurstScale"),
    })
}

/// The class defaults the evaluator's own fallbacks differ on.
pub fn class_defaults(class: &str) -> Map<String, J> {
    let mut m = Map::new();
    let mut set = |k: &str, v: J| {
        m.insert(k.to_owned(), v);
    };
    if class == "ParticleModuleRequired" {
        // An emitter without `EmitterLoops` loops until ended.
        set("EmitterLoops", J::from(0));
        set("EmitterDuration", J::from(1.));
        set("EmitterDelay", J::from(0.));
        set("EmitterStartTime", J::from(0.));
        set("EmitterLoopTime", J::from(0.));
        set("EmitterLoopDuration", J::from(0.));
        set("EmitterDurationTime", J::from(0.));
        set("bUseLegacyEmitterTime", J::Bool(false));
        set("SubImages_Horizontal", J::from(1));
        set("SubImages_Vertical", J::from(1));
        set("MaxDrawCount", J::from(500));
        set("bUseMaxDrawCount", J::Bool(true));
    }
    m
}

/// Reads an effect by `/Game/...` path.
pub fn build(paks: &Paks, game_path: &str) -> Result<BuiltEffect> {
    let package = game_path.split('.').next().unwrap_or(game_path);
    let p = paks
        .game_package(package)?
        .ok_or_else(|| bad(format!("{package} is missing")))?;
    let system = (0..p.exports.len())
        .find(|&i| p.class_of(i) == "ParticleSystem" && p.exports[i].outer == 0)
        .or_else(|| p.export_of_class("ParticleSystem"))
        .ok_or_else(|| bad(format!("{package} has no particle system")))?;
    let mut w = Walk {
        p: &p,
        materials: Vec::new(),
        meshes: Vec::new(),
        nested: Vec::new(),
    };
    let params = w.params(system)?;
    let (props, _) = export_props(&p, system)?;
    let mut emitters = Vec::new();
    for e in props.array("Emitters").unwrap_or(&[]) {
        let j = e.as_object().map_or(Ok(J::Null), |o| w.emitter(o))?;
        // A null slot keeps the indices.
        emitters.push(if j.is_null() {
            json!({"name":"","enabled":false,"required":{},"modules":[]})
        } else {
            j
        });
    }
    let name = base_name(package).to_ascii_lowercase();
    let mut params = params;
    params.remove("Emitters");
    params.remove("CurveEdSetup");
    params.remove("ThumbnailImage");
    Ok(BuiltEffect {
        json: json!({"name":name,"system":{"params":J::Object(params)},"emitters":emitters}),
        name,
        materials: w.materials,
        meshes: w.meshes,
        nested: w.nested,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn natives_name_their_members() {
        assert_eq!(native_json("Rotator", &[1., 2., 3.])["yaw"], 2.);
        assert_eq!(native_json("LinearColor", &[1., 2., 3., 4.])["a"], 4.);
        assert_eq!(native_json("Vector", &[1., 2., 3.])["z"], 3.);
    }

    #[test]
    fn kinds_follow_the_type_data_class() {
        assert_eq!(typedata_kind("ParticleModuleTypeDataMesh"), "mesh");
        assert_eq!(
            typedata_kind("ParticleModuleTypeDataSQEX_Decal"),
            "sqex_decal"
        );
        assert_eq!(typedata_kind("ParticleModuleTypeDataGpu"), "gpu_sprite");
    }

    #[test]
    fn required_defaults_loop_until_ended() {
        let d = class_defaults("ParticleModuleRequired");
        assert_eq!(d["EmitterLoops"], 0);
        assert!(class_defaults("Nothing").is_empty());
    }

    #[test]
    fn base_names_lose_the_path() {
        assert_eq!(base_name("/Game/A/B.B"), "B");
        assert_eq!(base_name("/Game/A/B"), "B");
    }
}
