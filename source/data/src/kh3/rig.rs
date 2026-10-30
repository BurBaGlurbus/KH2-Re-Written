//! Sora's helper bones (`SQEX_KineDriverData`) and cloth (`SQEX_BonamikAsset`) from the player's install, in the
//! form `game/kh3_rig` reads. The rules that turn the asset's rows into operators and particles are from StrawHatPro's
//! `tools/sora_outfit_kdi.js` and `tools/research/bnm_extract.js` (MIT).
use super::props::{self, Props, Value};
use super::skeletal::Bone;
use super::{Paks, Result, bad};
use serde_json::{Map, Value as J, json};

const KINE: &str = "/Game/Character/pc/p_ex001/mdl/p_ex001_0_kdi";
const BONAMIK: &str = "/Game/Character/pc/p_ex001/mdl/p_ex001_Bonamik";
const MESH: &str = "/Game/Character/pc/p_ex001/mdl/p_ex001_0";
const KEYCHAIN: &str = "/Game/Character/wep/w_so010/mdl/w_so010_Bonamik";
/// The class default of `TeleportDistance` (the Keyblade's mesh leaves it, Sora's sets it).
const TELEPORT: f32 = 200.;

/// The two sheets as JSON text.
#[derive(Clone, Debug, Default)]
pub struct RigData {
    pub kine: String,
    pub bonamik: String,
    /// The Kingdom Key's keychain, on the Keyblade's own skeleton.
    pub keychain: String,
}

fn value(v: &Value) -> J {
    match v {
        Value::Bool(b) => json!(b),
        Value::Int(i) => json!(i),
        Value::Float(f) => json!(f),
        Value::Str(s) | Value::Name(s) => json!(s),
        Value::Byte(b) => json!(b),
        Value::Floats(f) => json!(f),
        Value::Ints(f) => json!(f),
        Value::Struct(_, p) => plain(p),
        Value::Array(a) => J::Array(a.iter().map(value).collect()),
        Value::Object(_) | Value::Map(_) | Value::Raw(_) => J::Null,
    }
}

/// A row's properties as an object; a static array's elements become a list by their index.
pub fn plain(p: &Props) -> J {
    let mut out = Map::new();
    for q in &p.0 {
        let v = value(&q.value);
        if q.index > 0 {
            let at = q.index as usize;
            let slot = out
                .entry(q.name.clone())
                .or_insert_with(|| J::Array(Vec::new()));
            if !slot.is_array() {
                *slot = J::Array(vec![slot.clone()]);
            }
            if let J::Array(list) = slot {
                if list.len() <= at {
                    list.resize(at + 1, J::Null);
                }
                list[at] = v;
            }
        } else {
            out.insert(q.name.clone(), v);
        }
    }
    J::Object(out)
}

fn export_of(paks: &Paks, path: &str, class: &str) -> Result<J> {
    let p = paks
        .game_package(path)?
        .ok_or_else(|| bad(format!("{path} is missing")))?;
    let i = p
        .export_of_class(class)
        .ok_or_else(|| bad(format!("{path} has no {class}")))?;
    let (props, _) = props::export_props(&p, i)?;
    Ok(plain(&props))
}

fn strip(v: &J, prefix: &str) -> J {
    match v.as_str() {
        Some(s) => json!(s.strip_prefix(prefix).unwrap_or(s)),
        None => v.clone(),
    }
}

/// The asset's bone index is its skeleton's: the mesh's bone by the row's name.
fn bone_of(bones: &[Bone], index: &J, name: &J) -> Result<i64> {
    let i = index.as_i64().unwrap_or(-1);
    if i < 0 {
        return Ok(-1);
    }
    match name.as_str().filter(|n| *n != "None") {
        Some(n) => bones
            .iter()
            .position(|b| b.name == n)
            .map(|k| k as i64)
            .ok_or_else(|| bad(format!("the KineDriver's bone {n} is not on the mesh"))),
        None => Ok(if (i as usize) < bones.len() { i } else { -1 }),
    }
}

fn kine(raw: &J, bones: &[Bone]) -> Result<J> {
    let empty = Vec::new();
    let mut operators = Vec::new();
    let mut driven = std::collections::BTreeMap::<i64, Vec<J>>::new();
    for (i, head) in raw["Operators"]
        .as_array()
        .unwrap_or(&empty)
        .iter()
        .enumerate()
    {
        let kind = head["OpType"]
            .as_str()
            .unwrap_or("")
            .trim_start_matches("ESQEX_KD_OperatorType_");
        let list = &raw[format!("{kind}Body")];
        let b = list
            .get(head["OperatorBody"].as_u64().unwrap_or(0) as usize)
            .ok_or_else(|| bad(format!("KineDriver operator {i} has no body")))?;
        let mut o = Map::new();
        o.insert("index".into(), json!(i));
        o.insert("type".into(), json!(kind));
        let target = bone_of(bones, &b["TargetObject"], &b["TargetObjectBoneName"])?;
        let base = bone_of(bones, &b["BaseSpace"], &b["BaseSpaceBoneName"])?;
        let parent = |t: i64| {
            bones
                .get(t.max(0) as usize)
                .map_or(-1, |x| i64::from(x.parent))
        };
        let mut set = |k: &str, v: J| {
            o.insert(k.to_owned(), v);
        };
        let sources = |names: &str, list: &str| -> Result<J> {
            let empty = Vec::new();
            let mut out = Vec::new();
            for (k, s) in b[list].as_array().unwrap_or(&empty).iter().enumerate() {
                out.push(json!(bone_of(bones, s, &b[names][k])?));
            }
            Ok(J::Array(out))
        };
        match kind {
            "ComputeSpaceBases" => {
                set("target", json!(target));
                set("parent", json!(parent(target)));
                set("is_scale_op_child", b["IsScaleOpChild"].clone());
            }
            "SourceRotate" => {
                set("sources", sources("SourceBoneNameArray", "SourceArray")?);
                set("weights", b["WeightArray"].clone());
                set("base", json!(base));
                set("neutral_rotate", b["NeutralRotate"].clone());
                set("aim", b["AimVector"].clone());
                set("up", b["UpVector"].clone());
                set("cross", b["CrossVector"].clone());
            }
            "TargetTranslate" => {
                set("target", json!(target));
                set("translate", b["Translate"].clone());
                set("base", json!(base));
                set("neutral_translate", b["NeutralTranslate"].clone());
                set("neutral_rotate", b["NeutralRotate"].clone());
            }
            "TargetScale" => {
                set("target", json!(target));
                set("target_parent", json!(parent(target)));
                set("scale", b["Scale"].clone());
                set("clamp_zero", b["ClampZero"].clone());
                set("base", json!(base));
                set(
                    "segment_scale_compensate",
                    b["SegmentScaleCompensate"].clone(),
                );
            }
            "TargetRotate" | "TargetBendRoll" => {
                set("target", json!(target));
                set("source_quat_op", b["SourceQuat"].clone());
                set("quat_weight", b["QuatWeight"].clone());
                set("as_quat_angle", b["AsQuatAngle"].clone());
                set("roll", b["Roll"].clone());
                set("base", json!(base));
                set("neutral_rotate", b["NeutralRotate"].clone());
                set("aim", b["AimVector"].clone());
            }
            "TargetBendSTRoll" => {
                set("target", json!(target));
                set("bend_s", b["BendS"].clone());
                set("bend_t", b["BendT"].clone());
                set("roll", b["Roll"].clone());
                set("base", json!(base));
                set("neutral_rotate", b["NeutralRotate"].clone());
                set("aim", b["AimVector"].clone());
                set("up", b["UpVector"].clone());
                set("cross", b["CrossVector"].clone());
            }
            "ConnectEZParamLink" | "ConnectEZParamLinkLinear" | "ConnectEquals" => {
                let param = |v: &J| match v.as_str() {
                    Some(s) => json!(s.rsplit_once("Type_").map_or(s, |x| x.1)),
                    None => v.clone(),
                };
                set("in_op", b["InOperator"].clone());
                set("in_param", param(&b["InParameterType"]));
                set("out_op", b["OutOperator"].clone());
                set("out_param", param(&b["OutParameterType"]));
                for k in [
                    "PX0", "VX1_0", "VX2_1", "Grad0", "Grad1", "PY0", "PY0A", "PY0B", "PY1",
                    "PY1A", "PY1B", "PY2",
                ] {
                    set(k, b[k].clone());
                }
                set("by_coef", b["ByCoef"].clone());
                set("scale", b["Scale"].clone());
                set("offset", b["Offset"].clone());
                set("clamp_min", b["ClampMin"].clone());
                set("clamp_max", b["ClampMax"].clone());
                set("enable_min", b["EnableMin"].clone());
                set("enable_max", b["EnableMax"].clone());
                set("coef", b["Coef"].clone());
                if let (Some(out), Some(param)) =
                    (b["OutOperator"].as_i64(), b["OutParameterType"].as_str())
                {
                    driven.entry(out).or_default().push(json!(param));
                }
            }
            other => return Err(bad(format!("a KineDriver operator {other}"))),
        }
        operators.push(J::Object(o));
    }
    // The bones the operators write.
    let mut written = vec![false; bones.len()];
    for o in &operators {
        let kind = o["type"].as_str().unwrap_or("");
        if kind == "ComputeSpaceBases" || kind.starts_with("Target") {
            if let Some(t) = o["target"].as_i64().filter(|t| *t >= 0) {
                written[t as usize] = true;
            }
        }
    }
    let table: Vec<J> = written
        .iter()
        .enumerate()
        .map(|(i, w)| json!({"index": i, "written": w}))
        .collect();
    Ok(json!({
        "settings": {"EnableScaleOpChildSSC": false},
        "operators": operators,
        "bones": table,
    }))
}

fn rename(row: &J, names: &[(&str, &str)]) -> Map<String, J> {
    names
        .iter()
        .map(|(to, from)| ((*to).to_owned(), row[*from].clone()))
        .collect()
}

fn bonamik(raw: &J, bones: &[Bone], teleport: f32) -> Result<J> {
    let empty = Vec::new();
    let list = |k: &str| raw[k].as_array().unwrap_or(&empty);
    let groups: Vec<J> = list("m_Solvers")
        .iter()
        .map(|s| {
            J::Object(rename(
                s,
                &[
                    ("id", "m_GroupId"),
                    ("name", "m_GroupName"),
                    ("enable", "m_IsEnable"),
                    ("useConeSEQ", "m_IsUseConeSEQ"),
                    ("colIter", "m_ColIter"),
                    ("consIter", "m_ConsIter"),
                    ("coneIter", "m_ConeIter"),
                    ("linkIter", "m_LinkIter"),
                    ("solverIter", "m_SolverIter"),
                    ("tagId", "m_TagId"),
                    ("collisionTagId", "m_CollisionTagId"),
                    ("enableGround", "m_EnableGround"),
                    ("lodDistance", "m_LODdistance"),
                    ("refUpVec", "m_RefUpVec"),
                    ("fpsCorrection", "m_FpsCorrection"),
                ],
            ))
        })
        .collect();
    let bodies: Vec<J> = list("m_Bodies")
        .iter()
        .map(|b| {
            let mut o = rename(
                b,
                &[
                    ("index", "m_Index"),
                    ("joint", "m_JointName"),
                    ("parentBody", "m_ParentId"),
                    ("childBody", "m_ChildId"),
                    ("group", "m_GroupId"),
                    ("kinematic", "m_IsKinematic"),
                    ("collision", "m_IsCollision"),
                    ("multipleCollisionObject", "m_IsMultipleCollisionObject"),
                    ("colId", "m_ColId"),
                    ("mass", "m_Mass"),
                    ("inertia", "m_Inertia"),
                    ("radius", "m_Radius"),
                    ("damping", "m_Damping"),
                    ("bounciness", "m_Bounciness"),
                    ("friction", "m_Friction"),
                    ("windDrag", "m_WindDrag"),
                    ("windLift", "m_WindLift"),
                    ("localTransCoeff", "m_LocalTransCoeff"),
                    ("maxLocalTrans", "m_MaxLocalTrans"),
                    ("addiConstStr", "m_AddiConstStr"),
                    ("addiConstRad", "m_AddiConstRad"),
                    ("localTranslation", "m_LocalTransform"),
                    ("localRotation", "m_LocalRotation"),
                    ("localScale", "m_LocalScale"),
                    ("collisionBatchNo", "m_CollisionBatchNo"),
                    ("concaveRatio", "m_ConcaveRatio"),
                    ("lfCoeff", "m_LFcoeff"),
                    ("parkRate", "m_ParkRate"),
                    ("maxLF", "m_MaxLF"),
                    ("lfDrag", "m_LFdrag"),
                    ("lfLift", "m_LFlift"),
                ],
            );
            o.insert(
                "colShape".into(),
                strip(&b["m_ColShape"], "ESQEX_Bonamik_CollisionShape_"),
            );
            o.insert(
                "addiConstType".into(),
                strip(&b["m_AddiConstType"], "ESQEX_Bonamik_ConstraintType_"),
            );
            let name = b["m_JointName"].as_str().unwrap_or("");
            let index = bones.iter().position(|x| x.name == name);
            o.insert("bone_index".into(), json!(index.map_or(-1, |i| i as i64)));
            J::Object(o)
        })
        .collect();
    let links: Vec<J> = list("m_Links")
        .iter()
        .enumerate()
        .map(|(i, l)| {
            let mut o = rename(
                l,
                &[
                    ("parent", "m_ParentId"),
                    ("child", "m_ChildId"),
                    ("selfCollision", "m_IsSelfCollision"),
                    ("jointTwist", "m_EnableJointTwist"),
                    ("cone", "m_EnableCone"),
                    ("coneAnimation", "m_EnableConeAnimation"),
                    ("linkStr", "m_LinkStr"),
                    ("linkParentStr", "m_LinkParentStr"),
                    ("coneInnerStr", "m_ConeInnerStr"),
                    ("coneOuterStr", "m_ConeOuterStr"),
                    ("coneParentStr", "m_ConeParentStr"),
                    ("limitYdeg", "m_LimitY"),
                    ("limitZdeg", "m_LimitZ"),
                    ("coneOffsetYdeg", "m_ConeOffsetY"),
                    ("coneOffsetZdeg", "m_ConeOffsetZ"),
                    ("limitOffsetYdeg", "m_LimitOffsetY"),
                    ("limitOffsetZdeg", "m_LimitOffsetZ"),
                    ("collisionReceiver", "m_IsCollisionReceiver"),
                    ("collisionRepulsor", "m_IsCollisionRepulsor"),
                    ("lra", "m_IsLRA"),
                    ("lraStr", "m_LRAstr"),
                    ("lraDamping", "m_LRAdamping"),
                    ("linkBatchNo", "m_LinkBatchNo"),
                    ("coneBatchNo", "m_ConeBatchNo"),
                    ("collisionBatchNo", "m_CollisionBatchNo"),
                    ("pinRadius", "m_PinRadius"),
                    ("pinStr", "m_PinStr"),
                    ("kdiTrans", "m_KdiTrans"),
                    ("kdiRot", "m_KdiRot"),
                    ("concaveRatio", "m_ConcaveRatio"),
                ],
            );
            o.insert("index".into(), json!(i));
            o.insert(
                "type".into(),
                strip(&l["m_LinkType"], "ESQEX_Bonamik_LinkType_"),
            );
            J::Object(o)
        })
        .collect();
    let planes: Vec<J> = list("m_Planes")
        .iter()
        .map(|p| {
            J::Object(rename(
                p,
                &[
                    ("index", "m_Index"),
                    ("group", "m_GroupId"),
                    ("particleA", "m_ParticleA"),
                    ("particleB", "m_ParticleB"),
                    ("particleC", "m_ParticleC"),
                    ("thickness", "m_Thickness"),
                    ("offset", "m_Offset"),
                    ("collision", "m_IsCollision"),
                    ("collisionBatchNo", "m_CollisionBatchNo"),
                    ("colId", "m_ColId"),
                ],
            ))
        })
        .collect();
    let s = &raw["m_SceneManager"];
    let mut scene = rename(
        s,
        &[
            ("isEnable", "m_IsEnable"),
            ("subStep", "m_SubStep"),
            ("totalGroup", "m_TotalGroup"),
            ("lssCoeffTran", "m_LSScoeff_tran"),
            ("lssCoeffRot", "m_LSScoeff_rot"),
            ("rootName", "m_RootName"),
            ("gravity", "m_Gravity"),
            ("enableGroundCollision", "m_EnableGroundCollision"),
            ("refUpVec", "m_RefUpVec"),
        ],
    );
    scene.insert(
        "fastMotionHandlingMode".into(),
        strip(&s["m_FastMotionHandlingMode"], "ESQEX_Bonamik_FMH_"),
    );
    // Not serialized: default 0 (SEQ).
    scene.insert(
        "jointLinkSimulationMode".into(),
        strip(
            &s["m_JointLinkSimulationMode"],
            "ESQEX_Bonamik_JOINTLINK_MODE_",
        ),
    );
    Ok(json!({
        "source": {"versionNo": raw["m_VersionNo"]},
        "user_data": {"TeleportDistance": teleport},
        "scene": scene,
        "groups": groups,
        "bodies": bodies,
        "links": links,
        "planes": planes,
    }))
}

/// Reads them for Sora's and the Keyblade's bones.
pub fn read(paks: &Paks, bones: &[Bone], keyblade: &[Bone]) -> Result<RigData> {
    let kine = kine(&export_of(paks, KINE, "SQEX_KineDriverData")?, bones)?;
    let teleport = paks
        .game_package(MESH)?
        .and_then(|p| {
            let i = p.export_of_class("SQEX_BonamikAssetUserData")?;
            let (props, _) = props::export_props(&p, i).ok()?;
            props.f32("TeleportDistance")
        })
        .unwrap_or(TELEPORT);
    let cloth = bonamik(
        &export_of(paks, BONAMIK, "SQEX_BonamikAsset")?,
        bones,
        teleport,
    )?;
    let keychain = bonamik(
        &export_of(paks, KEYCHAIN, "SQEX_BonamikAsset")?,
        keyblade,
        TELEPORT,
    )?;
    Ok(RigData {
        kine: kine.to_string(),
        bonamik: cloth.to_string(),
        keychain: keychain.to_string(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_static_array_becomes_a_list_by_index() {
        let prop = |index: i32, value: Value| props::Prop {
            name: "m_ColId".into(),
            index,
            struct_name: String::new(),
            value,
        };
        let p = Props(vec![
            prop(0, Value::Int(-1)),
            prop(1, Value::Int(11)),
            prop(2, Value::Int(-1)),
        ]);
        assert_eq!(plain(&p)["m_ColId"], json!([-1, 11, -1]));
    }

    #[test]
    fn a_kinedriver_operator_finds_its_bone_by_name() {
        let bones = vec![
            Bone {
                name: "a".into(),
                parent: -1,
                rotation: [0., 0., 0., 1.],
                translation: [0.; 3],
                scale: [1.; 3],
            },
            Bone {
                name: "b".into(),
                parent: 0,
                rotation: [0., 0., 0., 1.],
                translation: [0.; 3],
                scale: [1.; 3],
            },
        ];
        let raw = json!({
            "Operators": [{"OpType": "ESQEX_KD_OperatorType_ComputeSpaceBases", "OperatorBody": 0}],
            "ComputeSpaceBasesBody": [{"TargetObject": 0, "TargetObjectBoneName": "b", "IsScaleOpChild": false}],
        });
        let out = kine(&raw, &bones).unwrap();
        assert_eq!(out["operators"][0]["target"], json!(1));
        assert_eq!(out["operators"][0]["parent"], json!(0));
        assert_eq!(out["bones"][1]["written"], json!(true));
    }
}
