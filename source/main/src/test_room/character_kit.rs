//! One playable character kit: Sora's (or a Drive form's) model, Keyblade,
//! motion set, PTYA set and movement record, with every clip's poses
//! precomputed. The test room builds base Sora's kit at load and a form's
//! when a Drive command first asks for it (the original loads the form's
//! object entry during the drive-start motion, FUN_1403e0630, and waits
//! for the load, FUN_14039e340).
use super::{
    ClipPoses, HURT_COUNT, LOCOMOTION, SORA_GUARD, SORA_HURT, Skin, WEAPON_BANK, clip_for,
    ptya_record, sound_triggers,
};
use bevy::prelude::*;
use data::{model_inspection, motion, object_collision, probe_source::ProbeSource, ptya};
use player::{Action, AttackStep, Clip, Clips, Params};
use std::{collections::HashMap, sync::Arc};

/// What a kit is built from.
pub(super) struct KitInputs<'a> {
    /// The form (obj+0xde0) this kit plays: 0 for base Sora.
    pub form: u8,
    pub actor: model_inspection::InspectionModel,
    pub weapon: model_inspection::InspectionModel,
    /// Slot 1's Keyblade (the form's own); the same model as slot 0 when
    /// absent.
    pub weapon2: Option<model_inspection::InspectionModel>,
    /// The two Keyblades' ATTACK spheres by collision group (their mdlx
    /// object-collision tables, bone 2): offset, radius.
    pub blades: [Blade; 2],
    /// Each Keyblade's reach ([`blade_reach`]).
    pub blade_reach: [f32; 2],
    /// The slot 0 Keyblade's own effects (`obj/<model>.a.us`), when the kit
    /// was built for a chosen Keyblade.
    pub weapon_pax: Option<data::object_effect::Pax>,
    pub weapon2_pax: Option<data::object_effect::Pax>,
    /// Actor joints carrying weapon slots 0 and 1 (SKLT primary and
    /// secondary bone); slot 1 only for forms with a second Keyblade.
    pub weapon_joint: usize,
    pub second_joint: Option<usize>,
    pub motion_set: motion::MotionSetSelection,
    /// Base Sora's set: a motion the form's set lacks is read from it
    /// [test-room choice; the original's handling of an absent slot was
    /// not traced].
    pub fallback: Option<motion::MotionSetSelection>,
    /// The Keyblade's own motion set at the same slots (attack volumes,
    /// swing sounds, key chain).
    pub weapon_motion_set: Option<motion::MotionSetSelection>,
    /// Slot 1's Keyblade motions (03system `wmst` row set × 2 + 1).
    pub second_weapon_motion_set: Option<motion::MotionSetSelection>,
    /// 00battle `ptya` and the actor's entry (objentry +0x4e).
    pub ptya: &'a [u8],
    pub ptya_pointer: usize,
    /// Cast motions to load (the magic table's), when the set holds them.
    pub spell_ids: Vec<u16>,
    /// Load the damage motions (an enemy can hit the player).
    pub hurt: bool,
    /// The actor's archive bytes (object collision, entry type 23).
    pub collision_archive: Option<Vec<u8>>,
    pub basic_fk: bool,
    pub params: Params,
    /// The object's own effects (its `.a.us` PAX), if installed.
    pub pax: Option<data::object_effect::Pax>,
    /// Motions its script starts by id (Limit Form's limits), loaded when
    /// the set holds them.
    pub scripted: Vec<u16>,
    /// Scripted motions from other files (the party limits' own sets): id,
    /// the motion and its Keyblade motion.
    pub external: Vec<(u16, motion::SelectedMotion, Option<motion::SelectedMotion>)>,
}

/// A Keyblade's attack spheres by collision group: offset, radius.
pub(super) type Blade = HashMap<u8, Vec<(Vec3, f32)>>;

/// The ATTACK spheres of a weapon object's collision table (kind 6, the
/// Keyblade bone 2) from its mdlx archive.
pub(super) fn blade_spheres(archive: &[u8]) -> Blade {
    let table = data::bar::read_bar(archive).ok();
    let collision = table
        .as_ref()
        .and_then(|t| {
            t.entries
                .iter()
                .find(|e| e.entry_type == 23 && e.offset > 0 && e.size > 0)
        })
        .and_then(|e| archive.get(e.offset as usize..e.offset as usize + e.size as usize))
        .and_then(|b| object_collision::read(b).ok())
        .unwrap_or_default();
    let mut blade = Blade::new();
    for e in collision
        .iter()
        .filter(|e| e.kind == object_collision::ATTACK && e.bone == 2)
    {
        blade.entry(e.group).or_default().push((
            Vec3::new(
                e.position[0] as f32,
                e.position[1] as f32,
                e.position[2] as f32,
            ),
            e.radius as f32,
        ));
    }
    blade
}

/// The Kingdom Key's reach, for a kit that reads no Keyblade of its own.
pub(super) const KINGDOM_KEY_REACH: f32 = -78.;

/// The Keyblade's reach along its blade bone: the x of the first shape of
/// kind 15 in its collision table. FUN_1403b5d40 (weapon slot matrix) returns
/// this for FUN_1403b50f0's codes 0x4003..0x4008, so 0x4003 / 0x4004 sit at
/// (reach, 0, 0) on the bone and 0x4007 / 0x4008 at half of it.
pub(super) fn blade_reach(archive: &[u8]) -> f32 {
    let table = data::bar::read_bar(archive).ok();
    table
        .as_ref()
        .and_then(|t| {
            t.entries
                .iter()
                .find(|e| e.entry_type == 23 && e.offset > 0 && e.size > 0)
        })
        .and_then(|e| archive.get(e.offset as usize..e.offset as usize + e.size as usize))
        .and_then(|b| object_collision::read(b).ok())
        .and_then(|c| c.into_iter().find(|e| e.kind == 15))
        .map_or(KINGDOM_KEY_REACH, |e| f32::from(e.position[0]))
}

/// A built kit.
pub(super) struct Kit {
    pub params: Params,
    pub clips: Clips,
    pub actions: Vec<((Action, bool), usize)>,
    pub clip_poses: Vec<ClipPoses>,
    pub actor: model_inspection::InspectionModel,
    pub weapon: model_inspection::InspectionModel,
    pub weapon2: Option<model_inspection::InspectionModel>,
    pub blades: [Blade; 2],
    pub weapon_pax: Option<data::object_effect::Pax>,
    pub weapon2_pax: Option<data::object_effect::Pax>,
    /// HIT spheres (bone, offset, radius, high) and the TARGET shape's
    /// radius and height.
    pub body: Vec<(usize, Vec3, f32, bool)>,
    /// ATTACK shapes (kind 1) by collision group: bone, offset, radius.
    pub attack: HashMap<u8, Vec<(usize, Vec3, f32)>>,
    pub target_radius: f32,
    pub target_shape: Option<object_collision::Entry>,
    /// Push shapes (object collision kind 4; FUN_1403b4f10(obj, 4)).
    pub push_shapes: Vec<object_collision::Entry>,
    /// Body shapes (kind 1, groups on by default): obj+0xa00, the shapes
    /// battle objects push each other with (FUN_14040ad70).
    pub obj_shapes: Vec<object_collision::Entry>,
    pub target_lift: f32,
    pub body_lift: f32,
    pub heal_lift: f32,
    pub summary: serde_json::Value,
    pub set: Vec<ptya::PtyaEntry>,
    pub pax: Option<data::object_effect::Pax>,
    /// Weapon slots with a motion set (0 leaves the Keyblade unattached).
    pub weapon_motions: u8,
    /// The trigger tables of the motions scripts start by id (Limit
    /// Form's limits run on the object-script engine, which reads them).
    pub script_events: HashMap<u16, enemy::motion::MotionEvents>,
}

/// Reads `ids` from the set, falling back to `fallback` for absent ones;
/// an id in neither is an error.
fn read_with_fallback(
    source: &ProbeSource,
    set: &motion::MotionSetSelection,
    fallback: Option<&motion::MotionSetSelection>,
    ids: &[u16],
) -> Result<Vec<motion::SelectedMotion>, Box<dyn std::error::Error>> {
    let mut found = motion::read_motion_ids_optional(source, set, ids)?;
    if found.iter().any(Option::is_none)
        && let Some(f) = fallback
    {
        let missing: Vec<u16> = ids
            .iter()
            .zip(&found)
            .filter(|(_, m)| m.is_none())
            .map(|(&id, _)| id)
            .collect();
        let mut more = motion::read_motion_ids_optional(source, f, &missing)?.into_iter();
        for m in found.iter_mut().filter(|m| m.is_none()) {
            *m = more.next().flatten();
        }
    }
    ids.iter()
        .zip(found)
        .map(|(id, m)| m.ok_or_else(|| format!("motion {id:#x} absent from the set").into()))
        .collect()
}

/// `Skinning::sample_evaluated` without skinning the vertices: the clips
/// that keep their joints are skinned when shown, so loading needs only the
/// joints' matrices (the same matrices, in the same layout). Raw-matrix
/// motions keep their vertices and go through `sample_evaluated`.
fn evaluate_matrices(
    skinning: &data::model_pose::Skinning,
    motion: &data::motion::InterpolatedMotion,
    frame: f32,
) -> std::io::Result<(data::model_pose::ModelPose, data::motion_eval::Evaluated)> {
    if motion.raw.is_some() {
        return skinning.sample_evaluated(motion, frame);
    }
    let evaluated = data::motion_eval::evaluate(&skinning.bones, motion, frame)?;
    let matrix_bits = evaluated.joints[..skinning.bones.len()]
        .iter()
        .map(|w| {
            let m = w.matrix();
            std::array::from_fn(|i| m[i / 4][i % 4].to_bits())
        })
        .collect();
    Ok((
        data::model_pose::ModelPose {
            profile: data::model_pose::PoseProfile::Legacy,
            arithmetic_profile: "motion_eval_ik_constraints_v1",
            matrix_bits,
            position_bits: Vec::new(),
            runtime_verified: false,
        },
        evaluated,
    ))
}

/// A clip's frames and its evaluation statistics (constraints, IK chains,
/// expressions, unsupported constraints).
type BuiltClip = std::io::Result<(ClipPoses, [usize; 4])>;

pub(super) fn load_kit(
    source: &ProbeSource,
    k: KitInputs,
) -> Result<Kit, Box<dyn std::error::Error>> {
    let KitInputs {
        form,
        actor,
        weapon,
        weapon2,
        blades,
        blade_reach,
        weapon_pax,
        weapon2_pax,
        weapon_joint,
        second_joint,
        motion_set,
        fallback,
        weapon_motion_set,
        second_weapon_motion_set,
        ptya: ptya_bytes,
        ptya_pointer,
        spell_ids,
        hurt,
        collision_archive,
        basic_fk,
        params,
        pax,
        scripted,
        external,
    } = k;
    // Bones the object's bound effects follow (element flag 1, bone below
    // 0x4000), kept per frame with the HIT bones.
    let effect_bones: Vec<usize> = pax
        .iter()
        .flat_map(|p| &p.elements)
        .filter(|e| e.flags & 1 != 0 && (0..0x4000).contains(&e.bone))
        .map(|e| e.bone as usize)
        .chain(
            pax.iter()
                .flat_map(|p| &p.elements)
                .filter(|e| e.flags & 0x10000 != 0)
                .flat_map(|e| [e.bone_pos.bone_a as usize, e.bone_pos.bone_b as usize]),
        )
        .filter(|&b| b < actor.skinning.bones.len())
        .collect();
    if weapon_joint >= actor.skinning.bones.len()
        || second_joint.is_some_and(|j| j >= actor.skinning.bones.len())
    {
        return Err("weapon joint outside the actor skeleton".into());
    }
    let table = ptya::read_table(ptya_bytes)?;
    let set = ptya::read_set(ptya_bytes, &table, ptya_pointer)?;
    // Every record of the player's PTYA set: the attack command chooses
    // among them per press (player::ptya). Each distinct motion loads
    // once.
    let records: Vec<player::ptya::Record> = set.iter().map(ptya_record).collect();
    let mut attack_motions: Vec<u16> = Vec::new();
    for e in &set {
        if e.motion_id != 0 && !attack_motions.contains(&e.motion_id) {
            attack_motions.push(e.motion_id);
        }
    }
    // A form's set may lack the base cast motions (Valor cannot cast).
    let present = motion::read_motion_ids_optional(source, &motion_set, &spell_ids)?;
    let spell_ids: Vec<u16> = spell_ids
        .iter()
        .zip(&present)
        .filter(|(_, m)| m.is_some() || form == 0)
        .map(|(&id, _)| id)
        .collect();
    for &m in &spell_ids {
        if !attack_motions.contains(&m) {
            attack_motions.push(m);
        }
    }
    let present = motion::read_motion_ids_optional(source, &motion_set, &scripted)?;
    let script_events: HashMap<u16, enemy::motion::MotionEvents> = scripted
        .iter()
        .zip(&present)
        .filter_map(|(&id, m)| Some((id, super::party_limit::events_of(m.as_ref()?))))
        .collect();
    let scripted: Vec<u16> = scripted
        .iter()
        .zip(&present)
        .filter(|(_, m)| m.is_some())
        .map(|(&id, _)| id)
        .collect();
    for &m in &scripted {
        if !attack_motions.contains(&m) {
            attack_motions.push(m);
        }
    }
    if attack_motions.is_empty() {
        return Err("PTYA set has no attack motions".into());
    }
    let fb = fallback.as_ref();
    let mut ids: Vec<u16> = LOCOMOTION.iter().map(|&(_, id)| id).collect();
    let mut motions = read_with_fallback(source, &motion_set, fb, &ids)?;
    motions.extend(read_with_fallback(
        source,
        &motion_set,
        fb,
        &attack_motions,
    )?);
    ids.extend(&attack_motions);
    // Field-mode (weapon put away) idle/walk/run through the same public
    // chooser with battle=false, has_weapon=false.
    let field = |s: &motion::MotionSetSelection| motion::MotionSetSelection {
        battle: false,
        has_weapon: false,
        ..s.clone()
    };
    let field_fb = fb.map(field);
    motions.extend(read_with_fallback(
        source,
        &field(&motion_set),
        field_fb.as_ref(),
        &[0, 1, 2],
    )?);
    ids.extend([0, 1, 2]);
    let battle_count = ids.len() - 3;
    // Damage reactions (motion ids 10..13) when an enemy can hit Sora.
    let hurt_count = if hurt {
        motions.extend(read_with_fallback(source, &motion_set, fb, &SORA_HURT)?);
        ids.extend(SORA_HURT);
        SORA_HURT.len()
    } else {
        0
    };
    // Guard (PTYA 0x0b → motion 173), its success recoil (motion 42) and the
    // recoil of a swing that met the map (motion 0x29).
    motions.extend(read_with_fallback(source, &motion_set, fb, &SORA_GUARD)?);
    ids.extend(SORA_GUARD);
    // The drive-start (0x34) and drive-in (0x35) motions: base Sora's set
    // holds 0x34, the forms' sets 0x35 (FUN_14040ce60, FUN_140405630).
    let drive_ids = [0x34u16, 0x35];
    let drive_found = motion::read_motion_ids_optional(source, &motion_set, &drive_ids)?;
    let drive_present: Vec<bool> = drive_found.iter().map(Option::is_some).collect();
    motions.extend(drive_found.into_iter().flatten());
    ids.extend(
        drive_ids
            .iter()
            .zip(&drive_present)
            .filter(|(_, p)| **p)
            .map(|(&id, _)| id),
    );
    // The growth abilities' motions (201..209): the set's own, else base
    // Sora's.
    let mut growth_found =
        motion::read_motion_ids_optional(source, &motion_set, &player::growth::MOTIONS)?;
    if let Some(f) = fb {
        let base = motion::read_motion_ids_optional(source, f, &player::growth::MOTIONS)?;
        for (g, b) in growth_found.iter_mut().zip(base) {
            if g.is_none() {
                *g = b;
            }
        }
    }
    let growth_ids: Vec<u16> = player::growth::MOTIONS
        .iter()
        .zip(&growth_found)
        .filter(|(_, m)| m.is_some())
        .map(|(&id, _)| id)
        .collect();
    motions.extend(growth_found.into_iter().flatten());
    ids.extend(&growth_ids);
    // Motions from other files, after everything read from the set.
    let external_at = ids.len();
    let external_ids: Vec<u16> = external.iter().map(|e| e.0).collect();
    let mut external_weapons = Vec::new();
    for (id, body, weapon) in external {
        ids.push(id);
        external_weapons.push((body.slot, weapon));
        motions.push(body);
    }
    // HIT spheres (object collision type 2), for enemy attacks.
    let mut target_radius = 0f32;
    let mut target_shape = None;
    let mut push_shapes = Vec::new();
    let mut obj_shapes = Vec::new();
    let mut target_lift = 70f32;
    // Bone codes 0x4001 and 0x4009 (FUN_1403b50f0): the first type-1 and
    // type-9 collision entries' centres, here above the object's origin.
    let (mut body_lift, mut heal_lift) = (85f32, 105f32);
    let body: Vec<(usize, Vec3, f32, bool)> = match &collision_archive {
        Some(archive) => {
            let shapes = object_collision::read(super::bar_entry_typed(archive, 23)?)?;
            push_shapes = shapes.iter().filter(|c| c.kind == 4).copied().collect();
            let groups = object_collision::default_groups(super::bar_entry_typed(archive, 23)?)?;
            obj_shapes = shapes
                .iter()
                .filter(|c| c.kind == 1 && groups >> (c.group & 31) & 1 != 0)
                .copied()
                .collect();
            // FUN_1403d7520 reads the attacker's TARGET shape (kind 3) via
            // FUN_1403b5650(obj, 3, -1) / FUN_1403c9c00.
            if let Some(c) = shapes
                .iter()
                .find(|c| c.kind == object_collision::TARGET && c.group == 0)
                .or_else(|| shapes.iter().find(|c| c.kind == object_collision::TARGET))
            {
                target_shape = Some(*c);
                target_radius = c.radius as f32;
                target_lift = c.position_height as f32;
            }
            if let Some(c) = shapes.iter().find(|c| c.kind == 1 && c.group == 0) {
                body_lift = c.position_height as f32;
            }
            if let Some(c) = shapes.iter().find(|c| c.kind == 9 && c.group == 0) {
                heal_lift = c.position_height as f32;
            }
            shapes
                .iter()
                .filter(|c| c.kind == object_collision::HIT && c.group == 0 && c.bone >= 0)
                .map(|c| {
                    (
                        c.bone as usize,
                        Vec3::new(
                            c.position[0] as f32,
                            c.position[1] as f32,
                            c.position[2] as f32,
                        ),
                        c.radius as f32,
                        matches!(c.argument, 0 | 2 | 3 | 4 | 5),
                    )
                })
                .collect()
        }
        None => Vec::new(),
    };
    // The Keyblades' motion controllers are children of the player's
    // (FUN_1403c8e90): each plays the player's motion slot.
    let mut attack: HashMap<u8, Vec<(usize, Vec3, f32)>> = HashMap::new();
    if let Some(archive) = &collision_archive {
        for c in object_collision::read(super::bar_entry_typed(archive, 23)?)?
            .iter()
            .filter(|c| c.kind == object_collision::ATTACK && c.bone >= 0)
        {
            attack.entry(c.group).or_default().push((
                c.bone as usize,
                Vec3::new(
                    c.position[0] as f32,
                    c.position[1] as f32,
                    c.position[2] as f32,
                ),
                c.radius as f32,
            ));
        }
    }
    let attack_bones: Vec<usize> = attack
        .values()
        .flatten()
        .map(|a| a.0)
        .filter(|&b| b < actor.skinning.bones.len())
        .collect();
    let mut weapon_motions: [HashMap<usize, motion::SelectedMotion>; 2] = Default::default();
    for (k, set) in [&weapon_motion_set, &second_weapon_motion_set]
        .into_iter()
        .enumerate()
    {
        let Some(set) = set else { continue };
        let found = motion::read_motion_ids_optional(source, set, &ids[..battle_count])?;
        for (selected, own) in found.into_iter().zip(&motions) {
            if let Some(selected) = selected {
                weapon_motions[k].insert(own.slot, selected);
            }
        }
        // The guard volumes (ATKP 161/162) live in the weapon's motions.
        let guard_at = battle_count + 3 + hurt_count;
        let found = motion::read_motion_ids_optional(source, set, &SORA_GUARD)?;
        for (selected, own) in found
            .into_iter()
            .zip(&motions[guard_at..guard_at + SORA_GUARD.len()])
        {
            if let Some(selected) = selected {
                weapon_motions[k].insert(own.slot, selected);
            }
        }
    }

    for (slot, weapon) in external_weapons {
        if let Some(w) = weapon {
            weapon_motions[0].insert(slot, w);
        }
    }
    // Precompute every frame of every clip; undefined basic-FK frames reject
    // the session instead of substituting a pose.
    let skin = Arc::new(Skin {
        skinning: actor.skinning.clone(),
        parents: actor
            .skinning
            .bones
            .iter()
            .map(|b| b.header_i32[1])
            .collect(),
        weapon_joint,
    });
    // One clip's frames: the evaluated joints (skinned when shown), the
    // matrices the simulation reads, the weapon's own vertices, and the
    // evaluation statistics.
    let build = |selected: &motion::SelectedMotion, end: u32, key_start: u32| -> BuiltClip {
        let mut stats = [0usize; 4];
        let mut frames = Vec::with_capacity(end as usize);
        let mut joints = Vec::new();
        let mut hand = Vec::with_capacity(end as usize);
        let mut hand2 = Vec::new();
        let mut root = Vec::with_capacity(end as usize);
        let mut weapon_frames = Vec::new();
        let mut weapon2_frames = Vec::new();
        let mut blade = Vec::new();
        let mut blade2 = Vec::new();
        // FUN_1401a7af0's return: the blade bone's length (record +0x1c),
        // which a swing trail's ratios multiply (FUN_1403b54f0).
        let bone_length = |m: &model_inspection::InspectionModel| {
            m.skinning
                .bones
                .get(2)
                .map_or(0., |b| f32::from_bits(b.transform_bits[3]))
        };
        let blade_length = [
            bone_length(&weapon),
            bone_length(weapon2.as_ref().unwrap_or(&weapon)),
        ];
        let mut body_frames = Vec::new();
        // Attack volumes: the weapon motion's range triggers 10.
        let own_hits = |m: &motion::SelectedMotion| -> Vec<(u32, u32, u16, u8)> {
            m.triggers
                .iter()
                .flat_map(|t| &t.records.ranges)
                .filter(|r| r.trigger == 10 && r.parameters.len() >= 2)
                .map(|r| {
                    (
                        r.start_frame.max(0) as u32,
                        if r.end_frame < 0 {
                            end
                        } else {
                            r.end_frame as u32
                        },
                        r.parameters[0] as u16,
                        r.parameters[1] as u8,
                    )
                })
                .collect()
        };
        let hits = weapon_motions[0]
            .get(&selected.slot)
            .map(own_hits)
            .unwrap_or_default();
        let hits2 = weapon_motions[1]
            .get(&selected.slot)
            .map(own_hits)
            .unwrap_or_default();
        // The weapon motion's range trigger 12 marks the swing trail
        // (each slot's own motion).
        let trail_windows = |k: usize| -> Vec<(u32, u32)> {
            weapon_motions[k]
                .get(&selected.slot)
                .map(|m| {
                    m.triggers
                        .iter()
                        .flat_map(|t| &t.records.ranges)
                        .filter(|r| r.trigger == 12)
                        .map(|r| {
                            (
                                r.start_frame.max(0) as u32,
                                if r.end_frame < 0 {
                                    end
                                } else {
                                    r.end_frame as u32
                                },
                            )
                        })
                        .collect()
                })
                .unwrap_or_default()
        };
        let trails = trail_windows(0);
        let trails2 = trail_windows(1);
        for offset in 0..=end {
            let frame = key_start + offset;
            for (k, (frames_k, blade_k)) in [
                (&mut weapon_frames, &mut blade),
                (&mut weapon2_frames, &mut blade2),
            ]
            .into_iter()
            .enumerate()
            {
                if let Some(own) = weapon_motions[k].get(&selected.slot) {
                    let model = if k == 1 {
                        weapon2.as_ref().unwrap_or(&weapon)
                    } else {
                        &weapon
                    };
                    let (pose, _) = model.skinning.sample_evaluated(&own.motion, frame as f32)?;
                    frames_k.push(
                        pose.position_bits
                            .iter()
                            .map(|p| Vec3::from_array(p.map(f32::from_bits)))
                            .collect(),
                    );
                    if let Some(m) = pose.matrix_bits.get(2) {
                        blade_k.push(Mat4::from_cols_array(&m.map(f32::from_bits)));
                    }
                }
            }
            let pose = if basic_fk {
                root.push(Vec3::ZERO);
                actor.skinning.sample_fk(&selected.motion, frame)?
            } else {
                let (mut pose, evaluated) =
                    evaluate_matrices(&actor.skinning, &selected.motion, frame as f32)?;
                // Root motion moves the player; the rendered pose
                // stays at the player origin.
                let r = evaluated.root_translation;
                root.push(Vec3::from_array(r));
                // Raw-matrix motions have no joints to blend; the
                // others keep their joints (root included) and are
                // skinned when shown (`ClipPoses::frame`).
                if let Some(own) = evaluated.joints.get(..skin.parents.len()) {
                    joints.push(own.to_vec());
                }
                for p in &mut pose.position_bits {
                    *p = std::array::from_fn(|i| (f32::from_bits(p[i]) - r[i]).to_bits());
                }
                for m in &mut pose.matrix_bits {
                    for i in 0..3 {
                        m[12 + i] = (f32::from_bits(m[12 + i]) - r[i]).to_bits();
                    }
                }
                let s = evaluated.stats;
                stats[0] += s.constraints_applied;
                stats[1] += s.ik_chains_solved;
                stats[2] += s.expressions_applied;
                stats[3] += s.unsupported_constraints;
                pose
            };
            body_frames.push(
                body.iter()
                    .map(|&(b, ..)| b)
                    .chain(effect_bones.iter().copied())
                    .chain(attack_bones.iter().copied())
                    .chain(
                        target_shape
                            .filter(|s| s.bone >= 0 && s.bone != 0x4000)
                            .map(|s| s.bone as usize),
                    )
                    .filter_map(|b| {
                        let m = pose.matrix_bits.get(b)?;
                        Some((b, Mat4::from_cols_array(&m.map(f32::from_bits))))
                    })
                    .collect(),
            );
            // Row-vector matrices: the row-major words are Bevy's
            // column-major layout of the same transform.
            hand.push(Mat4::from_cols_array(
                &pose.matrix_bits[weapon_joint].map(f32::from_bits),
            ));
            if let Some(j) = second_joint {
                hand2.push(Mat4::from_cols_array(
                    &pose.matrix_bits[j].map(f32::from_bits),
                ));
            }
            if joints.is_empty() {
                frames.push(
                    pose.position_bits
                        .iter()
                        .map(|p| Vec3::from_array(p.map(f32::from_bits)))
                        .collect(),
                );
            }
        }
        Ok((
            ClipPoses {
                frames,
                joints,
                skin: Some(skin.clone()),
                weapon: hand,
                weapon2: hand2,
                weapon_frames,
                weapon2_frames,
                root,
                blade,
                blade2,
                blade_length,
                blade_reach,
                hits,
                hits2,
                trails,
                trails2,
                actor_hits: own_hits(selected),
                body: body_frames,
                sounds: weapon_motions[0]
                    .get(&selected.slot)
                    .map(|own| sound_triggers(own, WEAPON_BANK))
                    .unwrap_or_default(),
            },
            stats,
        ))
    };
    let mut jobs: Vec<(&motion::SelectedMotion, u32, u32)> = Vec::new();
    let mut clip_list: Vec<(Clip, usize)> = Vec::new();
    let mut by_slot: HashMap<usize, usize> = HashMap::new();
    let mut slots = Vec::new();
    for selected in &motions {
        let (mut clip, key_start) = clip_for(selected)?;
        // Range 35 opens the combo window only for its own motion id
        // (FUN_14040d810 case 0x23).
        let own_id = ids[clip_list.len()];
        for record in selected.triggers.iter().flat_map(|t| &t.records.ranges) {
            if record.trigger == 35 && record.parameters.first().map(|&m| m as u16) == Some(own_id)
            {
                let start = record.start_frame.max(0) as u32;
                let stop = if record.end_frame < 0 {
                    clip.end
                } else {
                    record.end_frame as u32
                };
                clip.combo_windows.push((start, stop));
            }
        }
        let index = match by_slot.get(&selected.slot) {
            Some(&i) => i,
            None => {
                // Evaluated below, every new clip on its own thread.
                jobs.push((selected, clip.end, key_start));
                by_slot.insert(selected.slot, jobs.len() - 1);
                jobs.len() - 1
            }
        };
        slots.push(serde_json::json!({"motion_id":ids[clip_list.len()],"slot":selected.slot,"sha256":selected.sha256,
            "end_ticks":clip.end,"loop_start":clip.loop_start,"combo_windows":clip.combo_windows,
            "blend_to_idle":clip.blend_to_idle,"no_gravity":clip.no_gravity,"command_lockout":clip.command_lockout,"no_first_person":clip.no_first_person,"actionable":clip.actionable,
            "inertia":clip.inertia,"turn_to_target":clip.turn_to_target,"face_target":clip.face_target,"takeoff":clip.takeoff,"effects":clip.effects}));
        clip_list.push((clip, index));
    }
    // The clips are independent: evaluate them on several threads (each
    // takes every n-th), then keep them in their first-use order.
    let threads = std::thread::available_parallelism()
        .map_or(1, |n| n.get())
        .clamp(1, 8);
    let mut built: Vec<Option<BuiltClip>> = (0..jobs.len()).map(|_| None).collect();
    std::thread::scope(|scope| {
        let workers: Vec<_> = (0..threads)
            .map(|t| {
                let (jobs, build) = (&jobs, &build);
                scope.spawn(move || {
                    jobs.iter()
                        .enumerate()
                        .skip(t)
                        .step_by(threads)
                        .map(|(i, &(selected, end, key_start))| {
                            (i, build(selected, end, key_start))
                        })
                        .collect::<Vec<_>>()
                })
            })
            .collect();
        for worker in workers {
            for (i, result) in worker.join().expect("clip worker") {
                built[i] = Some(result);
            }
        }
    });
    let mut clip_poses = Vec::with_capacity(jobs.len());
    let mut eval_totals = [0usize; 4];
    for result in built {
        let (poses, stats) = result.expect("every clip built")?;
        for (total, s) in eval_totals.iter_mut().zip(stats) {
            *total += s;
        }
        clip_poses.push(poses);
    }
    for (slot, (_, index)) in slots.iter_mut().zip(&clip_list) {
        let root = &clip_poses[*index].root;
        slot["root_start"] = serde_json::json!(root.first().map(|r| r.to_array()));
        slot["root_end"] = serde_json::json!(root.last().map(|r| r.to_array()));
    }
    let step = |entry: &ptya::PtyaEntry, (clip, index): &(Clip, usize)| AttackStep {
        clip: clip.clone(),
        hop: f32::from_bits(entry.float_bits[0]),
        hop_speed: f32::from_bits(entry.float_bits[4]),
        hop_max: f32::from_bits(entry.float_bits[1]),
        hop_min: f32::from_bits(entry.float_bits[2]),
        hop_speed_min: f32::from_bits(entry.float_bits[3]),
        hop_reach: f32::from_bits(entry.float_bits[12]),
        hop_jump_reach: entry.flag & player::ptya::flag::JUMP_REACH != 0,
        blend: f32::from_bits(entry.float_bits[11]),
        root: clip_poses[*index]
            .root
            .iter()
            .map(|r| r.to_array())
            .collect(),
    };
    let n = LOCOMOTION.len();
    let field = n + attack_motions.len();
    let attack_clip = |motion: u16| {
        &clip_list[n + attack_motions
            .iter()
            .position(|&m| m == motion)
            .unwrap_or(0)]
    };
    let spells: Vec<player::SpellClip> = spell_ids
        .iter()
        .map(|&m| {
            let (clip, index) = attack_clip(m);
            player::SpellClip {
                motion: m,
                clip: clip.clone(),
                root: clip_poses[*index]
                    .root
                    .iter()
                    .map(|r| r.to_array())
                    .collect(),
            }
        })
        .collect();
    let guard_at = field + 3 + hurt_count;
    let drive_at = guard_at + SORA_GUARD.len();
    let mut drive_clips = [None, None];
    let mut k = drive_at;
    for (i, present) in drive_present.iter().enumerate() {
        if *present {
            drive_clips[i] = Some(k);
            k += 1;
        }
    }
    let growth_at = k;
    let clips = Clips {
        idle: clip_list[0].0.clone(),
        walk: clip_list[1].0.clone(),
        run: clip_list[2].0.clone(),
        field_idle: clip_list[field].0.clone(),
        field_walk: clip_list[field + 1].0.clone(),
        field_run: clip_list[field + 2].0.clone(),
        jump: clip_list[3].0.clone(),
        fall: clip_list[4].0.clone(),
        land: clip_list[5].0.clone(),
        records: records.clone(),
        spells,
        attacks: set
            .iter()
            .map(|e| step(e, attack_clip(e.motion_id)))
            .collect(),
        hurt: clip_list[field + 3..field + 3 + hurt_count.min(HURT_COUNT)]
            .iter()
            .map(|c| c.0.clone())
            .collect(),
        guard: Some(clip_list[guard_at].0.clone()),
        guard_recoil: Some(clip_list[guard_at + 1].0.clone()),
        wall_recoil: Some(clip_list[guard_at + 2].0.clone()),
        item: (hurt_count > HURT_COUNT).then(|| clip_list[field + 3 + HURT_COUNT].0.clone()),
        drive: drive_clips[0].map(|i| clip_list[i].0.clone()),
        drive_in: drive_clips[1].map(|i| clip_list[i].0.clone()),
        scripted: scripted
            .iter()
            .map(|&m| {
                let (clip, index) = attack_clip(m);
                let root = clip_poses[*index]
                    .root
                    .iter()
                    .map(|r| r.to_array())
                    .collect();
                (m, clip.clone(), root)
            })
            .chain(external_ids.iter().enumerate().map(|(k, &m)| {
                let (clip, index) = &clip_list[external_at + k];
                let root = clip_poses[*index]
                    .root
                    .iter()
                    .map(|r| r.to_array())
                    .collect();
                (m, clip.clone(), root)
            }))
            .collect(),
        growth: growth_ids
            .iter()
            .enumerate()
            .map(|(k, &m)| {
                let (clip, index) = &clip_list[k + growth_at];
                let root = clip_poses[*index]
                    .root
                    .iter()
                    .map(|r| r.to_array())
                    .collect();
                (m, clip.clone(), root)
            })
            .collect(),
    };
    // Pose clip per (action, weapon drawn).
    let mut actions = vec![
        ((Action::Idle, true), clip_list[0].1),
        ((Action::Walk, true), clip_list[1].1),
        ((Action::Run, true), clip_list[2].1),
        ((Action::Idle, false), clip_list[field].1),
        ((Action::Walk, false), clip_list[field + 1].1),
        ((Action::Run, false), clip_list[field + 2].1),
    ];
    let mut both = vec![
        (Action::Jump, clip_list[3].1),
        (Action::Fall, clip_list[4].1),
        (Action::Land, clip_list[5].1),
    ];
    for (i, e) in set.iter().enumerate() {
        let pose = attack_clip(e.motion_id).1;
        both.push((Action::Attack(i), pose));
        both.push((Action::AirAttack(i), pose));
    }
    for (i, &m) in spell_ids.iter().enumerate() {
        let pose = attack_clip(m).1;
        both.push((Action::Cast(i), pose));
        both.push((Action::AirCast(i), pose));
    }
    for i in 0..hurt_count.min(HURT_COUNT) {
        both.push((Action::Hurt(i), clip_list[field + 3 + i].1));
    }
    if hurt_count > HURT_COUNT {
        both.push((Action::Item, clip_list[field + 3 + HURT_COUNT].1));
    }
    both.push((Action::Guard, clip_list[guard_at].1));
    both.push((Action::GuardRecoil, clip_list[guard_at + 1].1));
    both.push((Action::WallRecoil, clip_list[guard_at + 2].1));
    if let Some(i) = drive_clips[0] {
        both.push((Action::Drive, clip_list[i].1));
    }
    if let Some(i) = drive_clips[1] {
        both.push((Action::DriveIn, clip_list[i].1));
    }
    for &m in &scripted {
        both.push((Action::Scripted(m), attack_clip(m).1));
    }
    for (k, &m) in external_ids.iter().enumerate() {
        both.push((Action::Scripted(m), clip_list[external_at + k].1));
    }
    for (k, &m) in growth_ids.iter().enumerate() {
        let pose = clip_list[growth_at + k].1;
        both.push((Action::Growth(m), pose));
        if m == player::growth::motion::HIGH_JUMP {
            both.push((Action::HighJump, pose));
        }
    }
    for (action, clip) in both {
        actions.push(((action, true), clip));
        actions.push(((action, false), clip));
    }
    let summary = serde_json::json!({"form":form,
        "ptya":{"pointer":ptya_pointer,"records":set.iter().map(|e| serde_json::json!({"id":e.id,"kind":e.kind,"alternate":e.sub,"combo_offset":e.combo_offset,"flag":e.flag,"motion_id":e.motion_id,"ability":e.ability,"floats":e.float_bits.map(f32::from_bits)})).collect::<Vec<_>>()},
        "motions":slots,
        "evaluation_totals":{"constraints":eval_totals[0],"ik_chains":eval_totals[1],"expressions":eval_totals[2],"unsupported_constraints":eval_totals[3]}});
    Ok(Kit {
        params,
        clips,
        actions,
        clip_poses,
        actor,
        weapon,
        weapon2,
        blades,
        weapon_pax,
        weapon2_pax,
        body,
        attack,
        target_radius,
        target_shape,
        push_shapes,
        obj_shapes,
        target_lift,
        body_lift,
        heal_lift,
        summary,
        set,
        script_events,
        pax,
        weapon_motions: weapon_motion_set.is_some() as u8
            + second_weapon_motion_set.is_some() as u8,
    })
}
