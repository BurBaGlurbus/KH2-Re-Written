//! Source-bound enemy inspection inventory. No AI or semantic motion names are
//! invented: journal categories join by their explicit object id, and every
//! populated motion BAR slot remains visible, including unsupported motions.
use data::{
    bar::{self, BarEntry},
    enemy_actions,
    message_text::{self, InternationalContent},
    model_inspection::{self, InspectionModel},
    motion::{self, SelectedMotion},
    objentry::{self, ObjectEntry},
    path_lookup::Library,
    probe_source::ProbeSource,
};
use std::{collections::HashMap, sync::Arc};
use visual::hud;

pub type Result<T> = std::result::Result<T, Box<dyn std::error::Error>>;

#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize)]
pub enum Category {
    Heartless,
    Nobody,
    Unresolved,
}
impl Category {
    pub fn label(self) -> &'static str {
        match self {
            Self::Heartless => "Heartless",
            Self::Nobody => "Nobodies",
            Self::Unresolved => "Other enemy candidates",
        }
    }
}

#[derive(Clone, Debug)]
pub struct Entry {
    pub object_id: u32,
    pub character: u16,
    pub object_type: u8,
    pub model_name: String,
    pub animation_name: String,
    pub name: String,
    /// Original Jiminy name bytes for the existing native font renderer.
    pub name_bytes: Option<Vec<u8>>,
    pub category: Category,
    /// Direct journal object id, same exact model inference, or unresolved.
    pub classification: &'static str,
    pub model_available: bool,
    pub motions_available: bool,
}

pub struct Catalog {
    pub entries: Vec<Entry>,
    /// Missing or conflicting classification evidence is retained explicitly.
    pub issues: Vec<String>,
}

struct JournalEntry {
    object: u32,
    category: Category,
    name: u16,
}

fn payload<'a>(bytes: &'a [u8], entry: &BarEntry) -> Result<&'a [u8]> {
    let start = usize::try_from(entry.offset)?;
    let end = start
        .checked_add(entry.size as usize)
        .ok_or("BAR payload range overflow")?;
    Ok(bytes.get(start..end).ok_or("BAR payload outside archive")?)
}

/// Pinned OpenKH Jiminy/Char.cs, 7a3b945c: explicit ObjectId at +0x0c.
/// The journal's +4 seen bit and objentry's +0x4c character are different IDs.
fn journal_entries(bytes: &[u8]) -> Result<Vec<JournalEntry>> {
    if !bytes.starts_with(b"JMCH") {
        return Err("journal character signature invalid".into());
    }
    let count = u32::from_le_bytes(
        bytes
            .get(8..12)
            .ok_or("journal header truncated")?
            .try_into()?,
    ) as usize;
    let end = count
        .checked_mul(36)
        .and_then(|n| n.checked_add(16))
        .ok_or("journal record count overflow")?;
    let records = bytes.get(16..end).ok_or("journal records truncated")?;
    Ok(records
        .as_chunks::<36>()
        .0
        .iter()
        .filter_map(|r| {
            let category = match r[0] {
                0x11 => Category::Heartless,
                0x12 => Category::Nobody,
                _ => return None,
            };
            Some(JournalEntry {
                object: u16::from_le_bytes([r[12], r[13]]) as u32,
                category,
                name: u16::from_le_bytes([r[6], r[7]]),
            })
        })
        .collect())
}

fn plain_name(bytes: &[u8]) -> Result<String> {
    let mut terminated = bytes.to_vec();
    terminated.push(0);
    let mut name = String::new();
    for segment in message_text::decode_international_system(&terminated)? {
        match segment.content {
            InternationalContent::Text(text) => name.push_str(&text),
            InternationalContent::Complex(text) => name.push_str(text),
            InternationalContent::Command => {}
            _ => return Err("journal name has an unqualified text mapping".into()),
        }
    }
    Ok(name)
}

fn model_path(name: &str) -> String {
    format!("obj/{name}.mdlx")
}
fn set_path(name: &str) -> String {
    format!(
        "obj/{}{}",
        name,
        if name.ends_with(".mset") { "" } else { ".mset" }
    )
}

impl Catalog {
    pub fn load(source: &ProbeSource) -> Result<Self> {
        let mut library = Library::open(source)?;
        let objects = objentry::read(&library.read("00objentry.bin")?)?;
        let mut issues = Vec::new();
        let mut labels = HashMap::new();
        let journal = (|| -> Result<Vec<JournalEntry>> {
            let bytes = library.read("menu/us/jiminy.bar")?;
            let table = bar::read_bar(&bytes)?;
            let entry = table
                .entries
                .iter()
                .find(|e| e.name_bytes == *b"char" && e.size != 0)
                .ok_or("journal character entry absent")?;
            journal_entries(payload(&bytes, entry)?)
        })();
        let journal = match journal {
            Ok(journal) => journal,
            Err(e) => {
                issues.push(format!("Journal classification unavailable: {e}"));
                Vec::new()
            }
        };
        if let Some(found) = library.find("msg/us/jm.bar") {
            let ids: Vec<_> = journal.iter().map(|j| j.name as u32).collect();
            match hud::load_messages_optional(
                source,
                found.index,
                found.ordinal,
                found.name_hash,
                &ids,
            ) {
                Ok(names) => {
                    for (j, bytes) in journal.iter().zip(names) {
                        if let Some(bytes) = bytes {
                            labels.insert(j.name, bytes);
                        }
                    }
                }
                Err(e) => issues.push(format!("Journal names unavailable: {e}")),
            }
        } else {
            issues.push("Journal message archive unavailable".into());
        }
        // Exact model reuse only: this does not classify unrelated variants by
        // an M_ or B_ prefix, nor join unrelated gameplay character IDs.
        let mut classified: HashMap<Vec<u8>, (Category, u16)> = HashMap::new();
        for j in &journal {
            if let Some(object) = objects.iter().find(|o| o.object_id == j.object) {
                if let Some((category, _)) = classified.get(&object.model_name)
                    && *category != j.category
                {
                    issues.push(format!(
                        "Conflicting journal categories for object {}",
                        j.object
                    ));
                    classified.insert(object.model_name.clone(), (Category::Unresolved, j.name));
                } else if !classified.contains_key(&object.model_name) {
                    classified.insert(object.model_name.clone(), (j.category, j.name));
                }
            } else {
                issues.push(format!(
                    "Journal object {} has no objentry record",
                    j.object
                ));
            }
        }
        let mut entries = Vec::new();
        for object in &objects {
            // Objentry.cs names 3/12 bosses and 4/21 normal enemies. Keep
            // every candidate and every direct journal enemy, without treating
            // boss type as proof of Heartless/Nobody species.
            if !matches!(object.object_type, 3 | 4 | 12 | 21)
                && !journal.iter().any(|j| j.object == object.object_id)
            {
                continue;
            }
            let mut row = entry(object, &classified, &labels, &library)?;
            if let Some(j) = journal.iter().find(|j| j.object == object.object_id) {
                row.category = j.category;
                row.classification = "direct journal object id";
                row.name_bytes = labels.get(&j.name).cloned();
                row.name = row
                    .name_bytes
                    .as_deref()
                    .and_then(|b| plain_name(b).ok())
                    .filter(|n| !n.is_empty())
                    .unwrap_or_else(|| row.model_name.clone());
            }
            entries.push(row);
        }
        entries.sort_by(|a, b| {
            (a.category as u8, &a.name, &a.model_name, a.object_id).cmp(&(
                b.category as u8,
                &b.name,
                &b.model_name,
                b.object_id,
            ))
        });
        Ok(Self { entries, issues })
    }

    pub fn summary(&self) -> serde_json::Value {
        serde_json::json!({
            "entries": self.entries.len(),
            "heartless": self.entries.iter().filter(|e| e.category == Category::Heartless).count(),
            "nobodies": self.entries.iter().filter(|e| e.category == Category::Nobody).count(),
            "unresolved": self.entries.iter().filter(|e| e.category == Category::Unresolved).count(),
            "model_unavailable": self.entries.iter().filter(|e| !e.model_available).count(),
            "motions_unavailable": self.entries.iter().filter(|e| !e.motions_available).count(),
            "issues": self.issues,
            "coverage": "installed objentry enemy/boss candidates and explicit Heartless/Nobody Jiminy references; family variants without a direct journal object/model match remain unresolved"
        })
    }

    /// Full sequential inspection denominator, for a private caller-selected
    /// report. Sampling success qualifies this evaluator, not reference poses.
    pub fn audit(&self, source: &ProbeSource, forward_kinematics_only: bool) -> serde_json::Value {
        let mut cache = HashMap::new();
        let mut rows = Vec::new();
        for (index, entry) in self.entries.iter().enumerate() {
            eprintln!(
                "Enemy corpus {}/{}: {}",
                index + 1,
                self.entries.len(),
                entry.model_name
            );
            let key = (entry.model_name.clone(), entry.animation_name.clone());
            let reused = cache.contains_key(&key);
            let detail = cache.entry(key).or_insert_with(|| {
                match self.load_kind(source, index) {
                    Err(e) => serde_json::json!({"status":"FAIL", "error":e.to_string()}),
                    Ok(kind) => {
                        let motions: Vec<_> = kind.motions.iter().enumerate().map(|(i, choice)| {
                            let result = kind.load_motion(i).and_then(|selected| {
                                let window = selected.motion.frame_window()?;
                                if [window.0, window.1, window.2, window.3].iter().any(|n| !n.is_finite())
                                    || window.0.abs() > 16_777_216. || window.1 < window.0 || window.1.abs() > 16_777_216.
                                    || !(0. ..=240.).contains(&window.2) || window.2 == 0.
                                    || window.3.abs() > 16_777_216. || window.3 > window.1
                                {
                                    return Err("motion frame window outside inspection profile".into());
                                }
                                let mut samples = Vec::new();
                                for frame in [window.0, (window.0 + window.1) * 0.5, window.1] {
                                    let pose = if forward_kinematics_only {
                                        if frame < 0. { return Err("basic FK cannot sample negative source keys; use the evaluated profile".into()); }
                                        kind.model.skinning.sample_fk(&selected.motion, frame.round() as u32)?
                                    } else {
                                        kind.model.skinning.sample_evaluated(&selected.motion, frame)?.0
                                    };
                                    if pose.position_bits.len() != kind.model.poses[0].position_bits.len()
                                        || pose.matrix_bits.len() != kind.model.skinning.bones.len()
                                        || pose.position_bits.iter().flatten().chain(pose.matrix_bits.iter().flatten()).any(|&b| !f32::from_bits(b).is_finite())
                                    {
                                        return Err("sample count mismatch or nonfinite pose".into());
                                    }
                                    samples.push(serde_json::json!({"key":frame,"vertices":pose.position_bits.len(),"matrices":pose.matrix_bits.len()}));
                                }
                                Ok(serde_json::json!({"status":"PASS","frame_window":[window.0,window.1,window.2,window.3],"samples":samples,
                                    "profile":if selected.motion.raw.is_some() {data::raw_motion::PROFILE} else {"motion_eval_ik_constraints_v1"}}))
                            });
                            serde_json::json!({"slot":choice.slot,"name":choice.name,"result": match result {
                                Ok(result) => result,
                                Err(e) => serde_json::json!({"status":"FAIL","error":e.to_string(),"declared_frame_window":kind.load_motion(i).ok().and_then(|m| m.motion.frame_window().ok())}),
                            }})
                        }).collect();
                        serde_json::json!({"status":"PASS","parts":kind.model.parts.len(),"vertices":kind.model.poses[0].position_bits.len(),"bones":kind.model.skinning.bones.len(),"extra_models":kind.extra_models.len(),"scripts":kind.scripts.len(),"motions":motions,"motion_count":kind.motions.len(),"actions":kind.actions,"action_issues":kind.action_issues,"variants":kind.variants,"action_choices":kind.action_choices(),"material_issue":kind.material_issue,"material_status":if kind.material_issue.is_some() {"UNQUALIFIED"} else {"ORIGINAL_INDEXED_DECODED"},"source_layout":kind.source_layout})
                    }
                }
            });
            rows.push(serde_json::json!({
                "index":index,"object_id":entry.object_id,"object_type":entry.object_type,"character":entry.character,
                "name":entry.name,"name_bytes":entry.name_bytes,"model":entry.model_name,"animation":entry.animation_name,
                "category":entry.category,"classification":entry.classification,"reused_same_model_and_animation":reused,"inspection":detail
            }));
        }
        serde_json::json!({"schema_version":1,"summary":self.summary(),"objects":rows,"distinct_model_and_animation_pairs":cache.len(),"evaluator":if forward_kinematics_only {"basic FK"} else {"motion evaluator IK/constraints"},"installed_comparison":"NOT RUN"})
    }

    pub fn load_kind(&self, source: &ProbeSource, index: usize) -> Result<Kind> {
        let entry = self
            .entries
            .get(index)
            .ok_or("enemy index outside catalog")?;
        let mut library = Library::open(source)?;
        let found = library
            .find(&model_path(&entry.model_name))
            .ok_or("enemy model unavailable")?;
        let archive = library.read_found(&found)?;
        let source_bar = bar::read_bar(&archive)?;
        let textures: Vec<_> = source_bar
            .entries
            .iter()
            .filter(|e| e.entry_type == 7)
            .collect();
        let material_issue = if textures.is_empty() {
            Some("Source has no type-7 tim_ material; geometry-only preview".into())
        } else if textures.len() == 1
            && textures[0].name_bytes == *b"tim_"
            && textures[0].size >= 36
            && payload(&archive, textures[0])?[..4] == (-1i32).to_le_bytes()
        {
            Some("Source tim_ uses the disabled/custom -1 profile; its texture decoding is unsupported; geometry-only preview".into())
        } else {
            None
        };
        let source_layout = serde_json::json!({
            "original_bytes":archive.len(),
            "entries":source_bar.entries.iter().map(|e| serde_json::json!({"type":e.entry_type,"name":e.name_bytes,"size":e.size})).collect::<Vec<_>>(),
            "texture_profile":"original-only; HD replacement payloads not requested"
        });
        let mut models = model_inspection::read_original_model_at(
            source,
            found.index,
            found.ordinal,
            found.name_hash,
            material_issue.is_none(),
        )?;
        if models.is_empty() {
            return Err("enemy model has no supported mesh".into());
        }
        let model = models.remove(0);
        let mut scripts = Vec::new();
        for entry in bar::read_bar(&archive)?.entries {
            if entry.entry_type == 3 && entry.offset >= 0 && entry.size != 0 {
                scripts.push(payload(&archive, &entry)?.to_vec());
            }
        }
        // A missing set still permits inspecting the installed static model.
        let set = library.find(&set_path(&entry.animation_name));
        let bytes = match set {
            Some(set) => library.read_found(&set)?,
            None => Vec::new(),
        };
        let motions = if bytes.is_empty() {
            Vec::new()
        } else {
            motion_choices(&bytes)?
        };
        let variants = !bytes.is_empty() && enemy_actions::uses_motion_variants(&bytes)?;
        let mut actions = Vec::new();
        let mut action_issues = Vec::new();
        for (index, script) in scripts.iter().enumerate() {
            match enemy_actions::inspect(script) {
                Ok(inspection) => {
                    if inspection.unresolved_registrations != 0 {
                        action_issues.push(format!(
                            "Script {index}: {} unresolved action registrations",
                            inspection.unresolved_registrations
                        ));
                    }
                    actions.extend(inspection.actions);
                }
                Err(error) => action_issues.push(format!("Script {index}: {error}")),
            }
        }
        Ok(Kind {
            model,
            extra_models: models,
            motions,
            scripts,
            bytes,
            actions,
            action_issues,
            variants,
            material_issue,
            source_layout,
        })
    }
}

fn entry(
    object: &ObjectEntry,
    classified: &HashMap<Vec<u8>, (Category, u16)>,
    labels: &HashMap<u16, Vec<u8>>,
    library: &Library,
) -> Result<Entry> {
    let model_name = std::str::from_utf8(&object.model_name)?.to_string();
    let animation_name = std::str::from_utf8(&object.animation_name)?.to_string();
    let (category, name_id) = classified
        .get(&object.model_name)
        .copied()
        .unwrap_or((Category::Unresolved, 0));
    let name_bytes = labels.get(&name_id).cloned();
    let name = name_bytes
        .as_deref()
        .and_then(|b| plain_name(b).ok())
        .filter(|n| !n.is_empty())
        .unwrap_or_else(|| model_name.clone());
    Ok(Entry {
        object_id: object.object_id,
        character: object.character,
        object_type: object.object_type,
        model_available: library.find(&model_path(&model_name)).is_some(),
        motions_available: !animation_name.is_empty()
            && library.find(&set_path(&animation_name)).is_some(),
        model_name,
        animation_name,
        name,
        name_bytes,
        category,
        classification: if category == Category::Unresolved {
            "unresolved"
        } else {
            "same exact journal model inference"
        },
    })
}

#[derive(Clone, Debug)]
pub struct MotionChoice {
    pub slot: usize,
    pub name: String,
    /// Populated bytes survive parser failures as visible disabled choices.
    pub status: Option<String>,
}

pub struct Kind {
    pub model: InspectionModel,
    pub extra_models: Vec<InspectionModel>,
    pub motions: Vec<MotionChoice>,
    pub scripts: Vec<Vec<u8>>,
    pub actions: Vec<enemy_actions::Action>,
    pub action_issues: Vec<String>,
    /// The source MSET header flag. The actor's contextual variant is unknown.
    pub variants: bool,
    pub material_issue: Option<String>,
    pub source_layout: serde_json::Value,
    bytes: Vec<u8>,
}

fn motion_choices(bytes: &[u8]) -> Result<Vec<MotionChoice>> {
    let table = bar::read_bar(bytes)?;
    let mut choices = Vec::new();
    for (slot, entry) in table.entries.iter().enumerate() {
        if entry.entry_type != 17 || entry.offset < 0 || entry.size == 0 {
            continue;
        }
        let name = String::from_utf8_lossy(&entry.name_bytes)
            .trim_end_matches('\0')
            .to_string();
        let status = motion::read_anb(payload(bytes, entry)?, slot)
            .and_then(|m| Ok(m.motion.frame_window()?))
            .err()
            .map(|e| e.to_string());
        choices.push(MotionChoice { slot, name, status });
    }
    Ok(choices)
}

impl Kind {
    #[cfg(test)]
    pub(super) fn synthetic(
        model: InspectionModel,
        motions: Vec<MotionChoice>,
        bytes: Vec<u8>,
    ) -> Self {
        Self {
            model,
            motions,
            bytes,
            extra_models: Vec::new(),
            scripts: Vec::new(),
            actions: Vec::new(),
            action_issues: Vec::new(),
            variants: false,
            material_issue: None,
            source_layout: serde_json::Value::Null,
        }
    }

    /// Every populated raw slot requested on a literal source action's direct
    /// callback paths. Conditional/dynamic requests do not select a branch.
    /// Variant sets expose all four candidates, preserving the unknown context.
    pub fn action_choices(&self) -> Vec<(String, usize)> {
        source_action_choices(&self.actions, self.variants, &self.motions)
    }

    /// The names of the script actions that request the motion in `slot`,
    /// in script order, with underscores as spaces. The motion set's own
    /// names are bare codes, so these are the readable names.
    pub fn action_names_for_slot(&self, slot: usize) -> Vec<String> {
        let mut names: Vec<String> = Vec::new();
        for action in &self.actions {
            let requests_slot = action.motions.iter().any(|request| {
                let logical = request.logical_motion as usize;
                if self.variants {
                    logical == slot / 4
                } else {
                    logical == slot
                }
            });
            let name = String::from_utf8_lossy(&action.name).replace('_', " ");
            if requests_slot && !names.contains(&name) {
                names.push(name);
            }
        }
        names
    }

    /// Only an unambiguous literal source action in a direct-index set can
    /// supply the development preview's appearance/idle shortcut.
    pub fn unique_action_motion(&self, name: &[u8]) -> Option<usize> {
        unique_source_action(&self.actions, self.variants, &self.motions, name)
    }
    /// Decode just the selected source slot. The presentation evaluates one
    /// pose at a time instead of retaining every motion's vertex frames.
    pub fn load_motion(&self, choice_index: usize) -> Result<Arc<SelectedMotion>> {
        let choice = self
            .motions
            .get(choice_index)
            .ok_or("motion choice outside catalog")?;
        if let Some(status) = &choice.status {
            return Err(status.clone().into());
        }
        let table = bar::read_bar(&self.bytes)?;
        let entry = table
            .entries
            .get(choice.slot)
            .ok_or("motion slot outside set")?;
        Ok(Arc::new(motion::read_anb(
            payload(&self.bytes, entry)?,
            choice.slot,
        )?))
    }
}

fn unique_source_action(
    actions: &[enemy_actions::Action],
    variants: bool,
    motions: &[MotionChoice],
    name: &[u8],
) -> Option<usize> {
    if variants {
        return None;
    }
    let actions: Vec<_> = actions
        .iter()
        .filter(|a| a.name.eq_ignore_ascii_case(name))
        .collect();
    if actions.is_empty()
        || actions
            .iter()
            .any(|a| a.unresolved_motion_requests != 0 || a.callback_error.is_some())
    {
        return None;
    }
    let mut requests: Vec<_> = actions
        .iter()
        .flat_map(|a| &a.motions)
        .map(|m| m.logical_motion as usize)
        .collect();
    requests.sort_unstable();
    requests.dedup();
    if requests.len() != 1 {
        return None;
    }
    motions
        .iter()
        .position(|m| m.slot == requests[0] && m.status.is_none())
}

fn source_action_choices(
    actions: &[enemy_actions::Action],
    variants: bool,
    motions: &[MotionChoice],
) -> Vec<(String, usize)> {
    let mut choices = Vec::new();
    for action in actions {
        let name = String::from_utf8_lossy(&action.name).replace('_', " ");
        for request in &action.motions {
            let Some(base) = request
                .logical_motion
                .checked_mul(if variants { 4 } else { 1 })
            else {
                continue;
            };
            for variant in 0..if variants { 4 } else { 1 } {
                let Some(slot) = base
                    .checked_add(variant)
                    .and_then(|s| usize::try_from(s).ok())
                else {
                    continue;
                };
                if let Some((index, _)) = motions.iter().enumerate().find(|(_, m)| m.slot == slot) {
                    let label = if variants {
                        format!("{name} - variant {variant}, slot {slot}")
                    } else {
                        format!("{name} - slot {slot}")
                    };
                    if !choices.contains(&(label.clone(), index)) {
                        choices.push((label, index));
                    }
                }
            }
        }
    }
    choices
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn appearance_shortcut_requires_a_supported_unambiguous_literal_action() {
        let mut action = enemy_actions::Action {
            name: b"appear".to_vec(),
            registration: 0,
            callback: 0,
            flags: 0,
            motions: vec![enemy_actions::MotionCandidate {
                instruction: 0,
                trap: 11,
                logical_motion: 44,
            }],
            unresolved_motion_requests: 0,
            callback_error: None,
        };
        let motions = [MotionChoice {
            slot: 44,
            name: "generated".into(),
            status: None,
        }];
        assert_eq!(
            unique_source_action(std::slice::from_ref(&action), false, &motions, b"appear"),
            Some(0)
        );
        assert_eq!(
            unique_source_action(std::slice::from_ref(&action), true, &motions, b"appear"),
            None
        );
        assert_eq!(
            unique_source_action(std::slice::from_ref(&action), false, &motions, b"idle"),
            None
        );
        action.unresolved_motion_requests = 1;
        assert_eq!(
            unique_source_action(std::slice::from_ref(&action), false, &motions, b"appear"),
            None
        );
        action.unresolved_motion_requests = 0;
        action.motions.push(enemy_actions::MotionCandidate {
            instruction: 0,
            trap: 11,
            logical_motion: 0,
        });
        assert_eq!(
            unique_source_action(&[action], false, &motions, b"appear"),
            None
        );
    }

    #[test]
    fn named_actions_keep_conditional_requests_and_all_present_variants() {
        let action = enemy_actions::Action {
            name: b"generated_action".to_vec(),
            registration: 0,
            callback: 0,
            flags: 0,
            motions: [2, 3, u32::MAX]
                .map(|logical_motion| enemy_actions::MotionCandidate {
                    instruction: 0,
                    trap: 11,
                    logical_motion,
                })
                .to_vec(),
            unresolved_motion_requests: 1,
            callback_error: None,
        };
        let motions: Vec<_> = [2, 3, 8, 10, 11, 12]
            .map(|slot| MotionChoice {
                slot,
                name: "synthetic".into(),
                status: None,
            })
            .into();
        let direct = source_action_choices(std::slice::from_ref(&action), false, &motions);
        assert_eq!(direct.iter().map(|c| c.1).collect::<Vec<_>>(), [0, 1]);
        let variants = source_action_choices(&[action], true, &motions);
        assert_eq!(
            variants.iter().map(|c| c.1).collect::<Vec<_>>(),
            [2, 3, 4, 5]
        );
        assert_eq!(variants[1].0, "generated action - variant 2, slot 10");
        assert_eq!(direct[1].0, "generated action - slot 3");
    }

    #[test]
    fn motions_are_named_by_the_script_actions_that_request_them() {
        let action = |name: &[u8], logical_motion| enemy_actions::Action {
            name: name.to_vec(),
            registration: 0,
            callback: 0,
            flags: 0,
            motions: vec![enemy_actions::MotionCandidate {
                instruction: 0,
                trap: 11,
                logical_motion,
            }],
            unresolved_motion_requests: 0,
            callback_error: None,
        };
        let mut kind = Kind::synthetic(
            data::model_inspection::InspectionModel {
                index: String::new(),
                ordinal: 0,
                bar_ordinal: 0,
                excluded_linked_groups: 0,
                parts: Vec::new(),
                poses: Vec::new(),
                skinning: Default::default(),
                textures: None,
                texture_routes: Vec::new(),
            },
            Vec::new(),
            Vec::new(),
        );
        kind.actions = vec![
            action(b"atk_claw", 3),
            action(b"idle", 3),
            action(b"idle", 3),
            action(b"leave", 5),
        ];
        assert_eq!(kind.action_names_for_slot(3), ["atk claw", "idle"]);
        assert_eq!(kind.action_names_for_slot(5), ["leave"]);
        assert!(kind.action_names_for_slot(4).is_empty());
        kind.variants = true;
        // Four slots per logical motion: slots 12 to 15 are motion 3.
        assert_eq!(kind.action_names_for_slot(14), ["atk claw", "idle"]);
        assert!(kind.action_names_for_slot(3).is_empty());
    }

    #[test]
    fn journal_uses_explicit_object_id_and_keeps_both_categories() {
        let mut bytes = vec![0; 16 + 3 * 36];
        bytes[..4].copy_from_slice(b"JMCH");
        bytes[8..12].copy_from_slice(&3u32.to_le_bytes());
        for (row, world, id, title) in [
            (0, 0x11, 200u16, 900u16),
            (1, 0x12, 201, 901),
            (2, 2, 202, 902),
        ] {
            let at = 16 + row * 36;
            bytes[at] = world;
            bytes[at + 4..at + 6].copy_from_slice(&500u16.to_le_bytes());
            bytes[at + 6..at + 8].copy_from_slice(&title.to_le_bytes());
            bytes[at + 12..at + 14].copy_from_slice(&id.to_le_bytes());
        }
        let rows = journal_entries(&bytes).unwrap();
        assert_eq!(rows.len(), 2);
        assert_eq!(
            (rows[0].object, rows[0].name, rows[0].category),
            (200, 900, Category::Heartless)
        );
        assert_eq!((rows[1].object, rows[1].category), (201, Category::Nobody));
        assert!(journal_entries(&bytes[..bytes.len() - 1]).is_err());
    }

    #[test]
    fn populated_unsupported_motion_remains_in_denominator() {
        let mut bytes = vec![0; 16 + 3 * 16 + 4];
        bytes[..4].copy_from_slice(b"BAR\x01");
        bytes[4..8].copy_from_slice(&3u32.to_le_bytes());
        bytes[16..18].copy_from_slice(&17u16.to_le_bytes());
        bytes[20..24].copy_from_slice(b"X01A");
        bytes[24..28].copy_from_slice(&64i32.to_le_bytes());
        bytes[28..32].copy_from_slice(&4i32.to_le_bytes());
        bytes[32..34].copy_from_slice(&17u16.to_le_bytes()); // Dummy slot.
        bytes[48..50].copy_from_slice(&2u16.to_le_bytes()); // Not an ANB.
        let rows = motion_choices(&bytes).unwrap();
        assert_eq!(rows.len(), 1);
        assert_eq!((rows[0].slot, rows[0].name.as_str()), (0, "X01A"));
        assert!(rows[0].status.is_some());
    }
}
