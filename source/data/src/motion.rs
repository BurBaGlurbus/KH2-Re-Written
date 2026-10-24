//! Interpolated skeletal motion records from pinned OpenKH Motion.cs (NOTICE).
//! Constraint/helper structures survive even when a preview samples only FK.
use serde::Serialize;
use sha2::{Digest, Sha256};
use std::io;

#[derive(Clone, Copy, Debug, Default, Serialize)]
pub struct LocalPose {
    pub rotation: [f32; 3],
    pub translation: [f32; 3],
}

#[derive(Debug, Serialize)]
pub struct InitialPose {
    pub bone_id: i16,
    pub channel: i16,
    pub value_bits: u32,
}
#[derive(Debug, Serialize)]
pub struct Curve {
    pub joint_id: i16,
    pub channel: u8,
    pub key_count: u8,
    pub key_start: i16,
}
#[derive(Debug, Serialize)]
pub struct Table {
    pub offset: i32,
    pub stride: usize,
    pub bytes: Vec<u8>,
}
#[derive(Debug, Serialize)]
pub struct InterpolatedMotion {
    /// Authoritative type-one records use this matrix profile; the legacy
    /// interpolated fields below stay empty rather than fabricating curves.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub raw: Option<crate::raw_motion::RawMotion>,
    pub header_words: [u32; 4],
    pub interpolated_header_words: Vec<u32>,
    pub bone_count: usize,
    pub total_bone_count: usize,
    pub initial_poses: Vec<InitialPose>,
    pub forward_curves: Vec<Curve>,
    pub inverse_curves: Vec<Curve>,
    pub keys: Vec<[i16; 4]>,
    pub key_tail_bytes: Vec<u8>,
    pub time_bits: Vec<u32>,
    pub value_bits: Vec<u32>,
    pub tangent_bits: Vec<u32>,
    pub constraints: Table,
    pub activations: Table,
    pub limiters: Table,
    pub expressions: Table,
    pub expression_nodes: Table,
    pub helpers: Table,
    pub joints: Table,
    pub root: Table,
    pub external_effectors: Table,
    pub extra_sha256: String,
    pub extra_size: usize,
}
fn invalid(message: &str) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, message)
}
fn words<const N: usize>(bytes: &[u8]) -> [u32; N] {
    std::array::from_fn(|i| u32::from_le_bytes(bytes[i * 4..i * 4 + 4].try_into().unwrap()))
}
fn half(bytes: &[u8], at: usize) -> i16 {
    i16::from_le_bytes(bytes[at..at + 2].try_into().unwrap())
}
fn count(value: u32) -> io::Result<usize> {
    if value > 262144 {
        Err(invalid("motion record count exceeds bounded profile"))
    } else {
        Ok(value as usize)
    }
}
fn span(bytes: &[u8], offset: u32, len: usize) -> io::Result<&[u8]> {
    if offset > i32::MAX as u32 {
        return Err(invalid("negative motion offset"));
    }
    bytes
        .get(
            offset as usize
                ..(offset as usize)
                    .checked_add(len)
                    .ok_or_else(|| invalid("motion range overflow"))?,
        )
        .ok_or_else(|| invalid("motion range outside payload"))
}
fn between(start: u32, end: u32, stride: usize) -> io::Result<usize> {
    let length = end
        .checked_sub(start)
        .ok_or_else(|| invalid("motion table order invalid"))? as usize;
    if !length.is_multiple_of(stride) {
        return Err(invalid("motion table stride invalid"));
    }
    count((length / stride) as u32)
}
fn table(bytes: &[u8], offset: u32, records: usize, stride: usize) -> io::Result<Table> {
    if records > 262144 {
        return Err(invalid("motion table count exceeds limit"));
    }
    Ok(Table {
        offset: offset as i32,
        stride,
        bytes: span(bytes, offset, records * stride)?.to_vec(),
    })
}
fn curves(bytes: &[u8], offset: u32, records: u32) -> io::Result<Vec<Curve>> {
    Ok(span(bytes, offset, count(records)? * 6)?
        .as_chunks::<6>()
        .0
        .iter()
        .map(|b| Curve {
            joint_id: half(b, 0),
            channel: b[2],
            key_count: b[3],
            key_start: half(b, 4),
        })
        .collect())
}
fn pool(bytes: &[u8], offset: u32, records: usize) -> io::Result<Vec<u32>> {
    Ok(span(bytes, offset, records * 4)?
        .as_chunks::<4>()
        .0
        .iter()
        .map(|b| u32::from_le_bytes(*b))
        .collect())
}
pub fn read_interpolated(bytes: &[u8]) -> io::Result<InterpolatedMotion> {
    if bytes.len() > 32 * 1024 * 1024 {
        return Err(invalid("motion exceeds 32 MiB"));
    }
    let body = bytes
        .get(0x90..)
        .ok_or_else(|| invalid("motion reserved area absent"))?;
    let header = words::<4>(span(body, 0, 16)?);
    if header[0] == 1 {
        let raw = crate::raw_motion::read(bytes)?;
        let empty = |stride| Table {
            offset: 0,
            stride,
            bytes: Vec::new(),
        };
        return Ok(InterpolatedMotion {
            header_words: header,
            interpolated_header_words: Vec::new(),
            bone_count: raw.bone_count,
            total_bone_count: raw.bone_count,
            initial_poses: Vec::new(),
            forward_curves: Vec::new(),
            inverse_curves: Vec::new(),
            keys: Vec::new(),
            key_tail_bytes: Vec::new(),
            time_bits: Vec::new(),
            value_bits: Vec::new(),
            tangent_bits: Vec::new(),
            constraints: empty(12),
            activations: empty(8),
            limiters: empty(48),
            expressions: empty(8),
            expression_nodes: empty(12),
            helpers: empty(64),
            joints: empty(4),
            root: empty(84),
            external_effectors: empty(2),
            extra_sha256: raw.extra_sha256.clone(),
            extra_size: raw.extra_size,
            raw: Some(raw),
        });
    }
    if header[0] != 0 {
        return Err(invalid("unsupported skeletal motion type"));
    }
    let h = words::<40>(span(body, 16, 160)?);
    let bone_count = count(h[0] & 65535)?;
    let total_bone_count = count(h[0] >> 16)?;
    if bone_count == 0
        || bone_count > 2048
        || total_bone_count < bone_count
        || total_bone_count > 4096
    {
        return Err(invalid("motion bone counts invalid"));
    }
    let initial_poses = span(body, h[5], count(h[6])? * 8)?
        .as_chunks::<8>()
        .0
        .iter()
        .map(|b| InitialPose {
            bone_id: half(b, 0),
            channel: half(b, 2),
            value_bits: u32::from_le_bytes(b[4..8].try_into().unwrap()),
        })
        .collect::<Vec<_>>();
    let forward_curves = curves(body, h[8], h[9])?;
    let inverse_curves = curves(body, h[10], h[11])?;
    // Motion.cs uses integer division for this extent. KeyTimeOffset can include
    // alignment bytes after the final complete key; retain them separately.
    let key_length = h[13]
        .checked_sub(h[12])
        .ok_or_else(|| invalid("motion key table order invalid"))? as usize;
    let key_count = count((key_length / 8) as u32)?;
    let key_span = span(body, h[12], key_length)?;
    let keys = key_span[..key_count * 8]
        .as_chunks::<8>()
        .0
        .iter()
        .map(|b| std::array::from_fn(|i| half(b, i * 2)))
        .collect::<Vec<_>>();
    let time_bits = pool(body, h[13], count(h[4])?)?;
    let value_bits = pool(body, h[14], between(h[14], h[15], 4)?)?;
    let tangent_bits = pool(body, h[15], between(h[15], h[16], 4)?)?;
    let constraints = table(body, h[16], count(h[17])?, 12)?;
    let mut activation_count = 0usize;
    let mut limiter_count = 0usize;
    for record in constraints.bytes.as_chunks::<12>().0 {
        let activations = usize::try_from(half(record, 8))
            .map_err(|_| invalid("negative motion activation count"))?;
        activation_count += activations;
        limiter_count = limiter_count.max((i32::from(half(record, 6)) + 1).max(0) as usize);
    }
    let activations = table(body, h[18], activation_count, 8)?;
    let limiters = table(body, h[19], limiter_count, 48)?;
    let expressions = table(body, h[20], count(h[21])?, 8)?;
    let expression_nodes = table(body, h[22], count(h[23])?, 12)?;
    let helpers = table(body, h[2], total_bone_count - bone_count, 64)?;
    let joints = table(body, h[3], total_bone_count, 4)?;
    let root = table(body, h[7], 1, 84)?;
    let mut external_count = 0;
    let mut cursor = h[36];
    while cursor < header[2] {
        let value = half(span(body, cursor, 2)?, 0);
        if value == 0 {
            break;
        }
        external_count += 1;
        if external_count > 4096 {
            return Err(invalid("external effector count exceeds limit"));
        }
        cursor = cursor
            .checked_add(2)
            .ok_or_else(|| invalid("effector offset overflow"))?;
    }
    let external_effectors = table(body, h[36], external_count, 2)?;
    let extra = span(body, header[2], count(header[3])?)?;
    for curve in forward_curves.iter().chain(&inverse_curves) {
        let start =
            usize::try_from(curve.key_start).map_err(|_| invalid("negative motion key start"))?;
        if start + curve.key_count as usize > keys.len() {
            return Err(invalid("curve key range outside table"));
        }
    }
    Ok(InterpolatedMotion {
        raw: None,
        header_words: header,
        interpolated_header_words: h.to_vec(),
        bone_count,
        total_bone_count,
        initial_poses,
        forward_curves,
        inverse_curves,
        keys,
        key_tail_bytes: key_span[key_count * 8..].to_vec(),
        time_bits,
        value_bits,
        tangent_bits,
        constraints,
        activations,
        limiters,
        expressions,
        expression_nodes,
        helpers,
        joints,
        root,
        external_effectors,
        extra_sha256: format!("{:x}", Sha256::digest(extra)),
        extra_size: extra.len(),
    })
}

impl InterpolatedMotion {
    /// Key `index` as (time, value, in-tangent, out-tangent).
    pub(crate) fn key_record(&self, index: usize) -> io::Result<[f32; 4]> {
        self.key(index)
    }
    fn key(&self, index: usize) -> io::Result<[f32; 4]> {
        let key = self
            .keys
            .get(index)
            .ok_or_else(|| invalid("sample key absent"))?;
        // Pinned legacy Motion.Read clamps time/value indices at the upper end.
        // Negative indices and empty pools are rejected rather than repaired.
        let clamped = |pool: &[u32], index: i16| -> io::Result<f32> {
            let index =
                usize::try_from(index).map_err(|_| invalid("negative sample pool index"))?;
            let last = pool
                .len()
                .checked_sub(1)
                .ok_or_else(|| invalid("empty sample pool"))?;
            Ok(f32::from_bits(pool[index.min(last)]))
        };
        let tangent = |index: i16| -> io::Result<f32> {
            usize::try_from(index)
                .ok()
                .and_then(|i| self.tangent_bits.get(i))
                .copied()
                .map(f32::from_bits)
                .ok_or_else(|| invalid("sample tangent index outside pool"))
        };
        let values = [
            clamped(&self.time_bits, key[0] >> 2)?,
            clamped(&self.value_bits, key[1])?,
            tangent(key[2])?,
            tangent(key[3])?,
        ];
        if values.iter().any(|v| !v.is_finite()) {
            return Err(invalid("nonfinite sample key"));
        }
        Ok(values)
    }
    /// Samples explicit integer frames using only the pinned basic FK consumer.
    /// There is no implicit clock/loop or installed-runtime qualification.
    pub fn sample_fk(
        &self,
        bones: &[crate::model_skeleton::BoneRecord],
        frame: u32,
    ) -> io::Result<Vec<LocalPose>> {
        if self.raw.is_some() {
            return Err(invalid(
                "raw matrices require stored-matrix inspection sampling",
            ));
        }
        if bones.len() != self.bone_count || frame > 16_777_216 {
            return Err(invalid("sample skeleton/frame mismatch"));
        }
        let mut locals = vec![LocalPose::default(); self.total_bone_count];
        for (local, bone) in locals.iter_mut().zip(bones) {
            let t = bone.transform_bits.map(f32::from_bits);
            if t.iter().any(|v| !v.is_finite()) {
                return Err(invalid("nonfinite sample bone"));
            }
            local.rotation.copy_from_slice(&t[4..7]);
            local.translation.copy_from_slice(&t[8..11]);
        }
        let apply =
            |locals: &mut [LocalPose], joint: i16, channel: u8, value: f32| -> io::Result<()> {
                if !(3..=8).contains(&channel) {
                    return Ok(());
                } // Scale is ignored by this public consumer.
                let local = usize::try_from(joint)
                    .ok()
                    .and_then(|i| locals.get_mut(i))
                    .ok_or_else(|| invalid("sample joint outside skeleton"))?;
                if !value.is_finite() {
                    return Err(invalid("nonfinite interpolated sample"));
                }
                if channel < 6 {
                    local.rotation[(channel - 3) as usize] = value;
                } else {
                    local.translation[(channel - 6) as usize] = value;
                }
                Ok(())
            };
        for pose in &self.initial_poses {
            // Initial-pose channels are signed full words, unlike curve flags.
            if (3..=8).contains(&pose.channel) {
                apply(
                    &mut locals,
                    pose.bone_id,
                    pose.channel as u8,
                    f32::from_bits(pose.value_bits),
                )?;
            }
        }
        for curve in &self.forward_curves {
            let start = usize::try_from(curve.key_start)
                .map_err(|_| invalid("negative sample key start"))?;
            let end = start
                .checked_add(curve.key_count as usize)
                .filter(|&end| end <= self.keys.len())
                .ok_or_else(|| invalid("sample curve range invalid"))?;
            for index in (start..end).rev() {
                let left = self.key(index)?;
                if (frame as f32) < left[0] {
                    continue;
                }
                let right = self.key(if index + 1 < end { index + 1 } else { start })?;
                let kind = self.keys[index][0] & 3;
                let value = if kind == 0 {
                    left[1]
                } else {
                    let delta = right[0] - left[0];
                    if delta == 0. {
                        return Err(invalid(
                            "public FK has undefined zero-duration interpolation",
                        ));
                    }
                    let n = (frame as f32 - left[0]) / delta;
                    if kind == 1 {
                        left[1] * (1. - n) + right[1] * n
                    } else {
                        let n2 = n * n;
                        let n3 = n2 * n;
                        (2. * n3 - 3. * n2 + 1.) * left[1]
                            + (n3 - 2. * n2 + n) * left[2]
                            + (-2. * n3 + 3. * n2) * right[1]
                            + (n3 - n2) * left[3]
                    }
                };
                apply(&mut locals, curve.joint_id, curve.channel & 15, value)?;
                break;
            }
        }
        locals.truncate(bones.len());
        Ok(locals)
    }
}

pub struct SelectedMotion {
    pub slot: usize,
    pub sha256: String,
    pub motion: InterpolatedMotion,
    pub triggers: Vec<crate::motion_trigger::SelectedTriggers>,
}
/// Fresh source-bound inspection admission; selections are locators, not proof
/// of installed action routing or supported gameplay.
pub fn read_selected(
    source: &crate::probe_source::ProbeSource,
    path: &std::path::Path,
) -> Result<Vec<SelectedMotion>, Box<dyn std::error::Error>> {
    use std::{fs::File, io::Read};
    #[derive(serde::Deserialize)]
    struct Selection {
        schema_version: u32,
        source_content_identity: String,
        index: String,
        ordinal: usize,
        name_hash: [u8; 16],
        slots: Vec<usize>,
    }
    let mut bytes = Vec::new();
    File::open(path)?.take(1_048_577).read_to_end(&mut bytes)?;
    if bytes.len() > 1_048_576 {
        return Err("motion selection exceeds limit".into());
    }
    let selected: Selection = serde_json::from_slice(&bytes)?;
    if selected.schema_version != 1
        || selected.source_content_identity != source.content_identity()
        || selected.slots.is_empty()
        || selected.slots.len() > 8
        || selected
            .slots
            .iter()
            .enumerate()
            .any(|(i, slot)| selected.slots[..i].contains(slot))
    {
        return Err("motion selection identity/count invalid".into());
    }
    source.verify_full_source()?;
    read_slots(
        source,
        &selected.index,
        selected.ordinal,
        selected.name_hash,
        &selected.slots,
    )
}

/// One motion archive (an ANB BAR holding one type-9 motion and its
/// triggers), tagged with `slot`.
pub fn read_anb(anb: &[u8], slot: usize) -> Result<SelectedMotion, Box<dyn std::error::Error>> {
    let nested = crate::bar::read_bar(anb)?;
    let entries: Vec<_> = nested
        .entries
        .iter()
        .filter(|e| e.entry_type == 9 && e.offset > 0 && e.size > 0)
        .collect();
    if entries.len() != 1 {
        return Err("one nested motion required".into());
    }
    let entry = entries[0];
    let payload = &anb[entry.offset as usize..entry.offset as usize + entry.size as usize];
    Ok(SelectedMotion {
        slot,
        sha256: format!("{:x}", Sha256::digest(payload)),
        motion: read_interpolated(payload)?,
        triggers: crate::motion_trigger::inspect_anb(anb)?,
    })
}

/// Motions at explicit BAR slots of a motion set. Sets whose header byte
/// +0xc bit 0 is clear (e.g. M_EX020.mset) index slots directly by motion id
/// (executable FUN_1403c7c50 / FUN_14040d7d0).
pub fn read_slots(
    source: &crate::probe_source::ProbeSource,
    index: &str,
    ordinal: usize,
    name_hash: [u8; 16],
    slots: &[usize],
) -> Result<Vec<SelectedMotion>, Box<dyn std::error::Error>> {
    if slots.is_empty() || slots.len() > 32 {
        return Err("motion slot count invalid".into());
    }
    let mut checked = source.open_index(index)?;
    let entry = checked
        .inventory
        .entries
        .get(ordinal)
        .filter(|e| e.name_hash == name_hash)
        .ok_or("motion source entry mismatch")?;
    let original = crate::original_payload::read_original(&mut checked.reader, entry)?;
    let table = crate::bar::read_bar(&original.bytes)?;
    let mut result = Vec::new();
    for &slot in slots {
        let entry = table
            .entries
            .get(slot)
            .filter(|e| e.offset > 0 && e.size > 0)
            .ok_or("motion slot absent/dummy")?;
        let anb = original
            .bytes
            .get(entry.offset as usize..entry.offset as usize + entry.size as usize)
            .ok_or("motion slot exceeds set")?;
        result.push(read_anb(anb, slot)?);
    }
    checked.verify_unchanged()?;
    Ok(result)
}

/// Like [`read_slots`], reading the set once; an absent or dummy slot gives
/// `None` (any count). A weapon plays the slot its owner plays
/// (FUN_1403c8cd0 hands the slot down to the child controllers).
pub fn read_slots_optional(
    source: &crate::probe_source::ProbeSource,
    index: &str,
    ordinal: usize,
    name_hash: [u8; 16],
    slots: &[usize],
) -> Result<Vec<Option<SelectedMotion>>, Box<dyn std::error::Error>> {
    if slots.len() > 1024 {
        return Err("motion slot count invalid".into());
    }
    source.verify_full_source()?;
    let mut checked = source.open_index(index)?;
    let entry = checked
        .inventory
        .entries
        .get(ordinal)
        .filter(|e| e.name_hash == name_hash)
        .ok_or("motion source entry mismatch")?;
    let original = crate::original_payload::read_original(&mut checked.reader, entry)?;
    let table = crate::bar::read_bar(&original.bytes)?;
    let mut result = Vec::new();
    for &slot in slots {
        let Some(entry) = table
            .entries
            .get(slot)
            .filter(|e| e.offset > 0 && e.size > 0)
        else {
            result.push(None);
            continue;
        };
        let anb = original
            .bytes
            .get(entry.offset as usize..entry.offset as usize + entry.size as usize)
            .ok_or("motion slot exceeds set")?;
        result.push(Some(read_anb(anb, slot)?));
    }
    checked.verify_unchanged()?;
    Ok(result)
}

/// Public slot chooser from pinned OpenKH MotionSet.GetMotionSetIndex: four
/// variant slots per motion ID, probed in a mode-dependent order.
pub fn motion_set_index(
    present: impl Fn(usize) -> bool,
    motion_id: u16,
    battle: bool,
    has_weapon: bool,
) -> Option<usize> {
    const MODE_TABLE: [usize; 16] = [0, 1, 3, 2, 1, 0, 2, 3, 2, 3, 1, 0, 3, 2, 0, 1];
    let mut mode = if battle { 1 } else { 2 };
    if has_weapon {
        mode ^= 1;
    }
    (0..4)
        .map(|i| MODE_TABLE[mode * 4 + i] + motion_id as usize * 4)
        .find(|&slot| present(slot))
}

/// One source-bound motion set resolved by motion ID through the public slot
/// chooser. The chooser is a public lead, not observed installed routing.
#[derive(Clone)]
pub struct MotionSetSelection {
    pub index: String,
    pub ordinal: usize,
    pub name_hash: [u8; 16],
    pub battle: bool,
    pub has_weapon: bool,
}
pub fn read_motion_ids(
    source: &crate::probe_source::ProbeSource,
    selection: &MotionSetSelection,
    motion_ids: &[u16],
) -> Result<Vec<SelectedMotion>, Box<dyn std::error::Error>> {
    if motion_ids.is_empty() || motion_ids.len() > 32 {
        return Err("motion ID selection count invalid".into());
    }
    source.verify_full_source()?;
    let mut checked = source.open_index(&selection.index)?;
    let entry = checked
        .inventory
        .entries
        .get(selection.ordinal)
        .filter(|e| e.name_hash == selection.name_hash)
        .ok_or("motion source entry mismatch")?;
    let original = crate::original_payload::read_original(&mut checked.reader, entry)?;
    let table = crate::bar::read_bar(&original.bytes)?;
    let present = |slot: usize| {
        table
            .entries
            .get(slot)
            .is_some_and(|e| e.offset > 0 && e.size > 0)
    };
    let mut result = Vec::new();
    for &id in motion_ids {
        let slot = motion_set_index(present, id, selection.battle, selection.has_weapon)
            .ok_or("motion ID has no populated slot")?;
        let entry = &table.entries[slot];
        let anb = original
            .bytes
            .get(entry.offset as usize..entry.offset as usize + entry.size as usize)
            .ok_or("motion slot exceeds set")?;
        result.push(read_anb(anb, slot)?);
    }
    checked.verify_unchanged()?;
    Ok(result)
}

/// Like [`read_motion_ids`], reading the set once; an id with no populated
/// slot gives `None` instead of failing (any count).
pub fn read_motion_ids_optional(
    source: &crate::probe_source::ProbeSource,
    selection: &MotionSetSelection,
    motion_ids: &[u16],
) -> Result<Vec<Option<SelectedMotion>>, Box<dyn std::error::Error>> {
    if motion_ids.len() > 1024 {
        return Err("motion ID selection count invalid".into());
    }
    source.verify_full_source()?;
    let mut checked = source.open_index(&selection.index)?;
    let entry = checked
        .inventory
        .entries
        .get(selection.ordinal)
        .filter(|e| e.name_hash == selection.name_hash)
        .ok_or("motion source entry mismatch")?;
    let original = crate::original_payload::read_original(&mut checked.reader, entry)?;
    let table = crate::bar::read_bar(&original.bytes)?;
    let present = |slot: usize| {
        table
            .entries
            .get(slot)
            .is_some_and(|e| e.offset > 0 && e.size > 0)
    };
    let mut result = Vec::new();
    for &id in motion_ids {
        let Some(slot) = motion_set_index(present, id, selection.battle, selection.has_weapon)
        else {
            result.push(None);
            continue;
        };
        let entry = &table.entries[slot];
        let anb = original
            .bytes
            .get(entry.offset as usize..entry.offset as usize + entry.size as usize)
            .ok_or("motion slot exceeds set")?;
        result.push(Some(read_anb(anb, slot)?));
    }
    checked.verify_unchanged()?;
    Ok(result)
}

impl InterpolatedMotion {
    /// Public FrameData words (FrameStart, FrameEnd, FramePerSecond,
    /// FrameReturn) at interpolated header words 32..35. Key times in this
    /// set use twice the declared 30-per-second frame units.
    pub fn frame_window(&self) -> io::Result<(f32, f32, f32, f32)> {
        if let Some(raw) = &self.raw {
            return raw.frame_window();
        }
        let word = |i: usize| {
            self.interpolated_header_words
                .get(i)
                .map(|&w| f32::from_bits(w))
                .filter(|v| v.is_finite())
                .ok_or_else(|| invalid("motion frame window absent"))
        };
        Ok((word(32)?, word(33)?, word(34)?, word(35)?))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn public_slot_chooser_orders_by_mode() {
        let present = |slots: &'static [usize]| move |s: usize| slots.contains(&s);
        // Field without weapon probes variant 2 first.
        assert_eq!(motion_set_index(present(&[4, 6]), 1, false, false), Some(6));
        // Battle with weapon probes variants 0, 1, 3, 2.
        assert_eq!(motion_set_index(present(&[4, 6]), 1, true, true), Some(4));
        assert_eq!(motion_set_index(present(&[14]), 3, true, true), Some(14));
        assert_eq!(motion_set_index(present(&[]), 3, true, true), None);
    }
    fn fixture() -> Vec<u8> {
        let mut bytes = vec![0; 0x90 + 320];
        let body = &mut bytes[0x90..];
        body[8..12].copy_from_slice(&320u32.to_le_bytes());
        let mut h = [0u32; 40];
        for (i, v) in [
            (0, 0x10001),
            (1, 2),
            (2, 228),
            (3, 228),
            (4, 2),
            (5, 176),
            (6, 1),
            (7, 232),
            (8, 184),
            (9, 1),
            (10, 190),
            (12, 192),
            (13, 208),
            (14, 216),
            (15, 224),
            (16, 228),
            (18, 228),
            (19, 228),
            (20, 228),
            (22, 228),
            (36, 316),
        ] {
            h[i] = v;
        }
        h[33] = 30f32.to_bits();
        h[34] = 30f32.to_bits();
        for (i, v) in h.into_iter().enumerate() {
            body[16 + i * 4..20 + i * 4].copy_from_slice(&v.to_le_bytes());
        }
        body[178..180].copy_from_slice(&3i16.to_le_bytes());
        body[180..184].copy_from_slice(&0.5f32.to_le_bytes());
        body[186] = 3;
        body[187] = 2;
        for (i, entry) in [[1i16, 0, 0, 0], [5, 1, 0, 0]].into_iter().enumerate() {
            for (j, v) in entry.into_iter().enumerate() {
                body[192 + i * 8 + j * 2..194 + i * 8 + j * 2].copy_from_slice(&v.to_le_bytes());
            }
        }
        body[212..216].copy_from_slice(&30f32.to_le_bytes());
        body[220..224].copy_from_slice(&1f32.to_le_bytes());
        bytes
    }
    #[test]
    fn preserves_motion_pools_raw_flags_and_complete_root_span() {
        let motion = read_interpolated(&fixture()).unwrap();
        assert_eq!(motion.bone_count, 1);
        assert_eq!(motion.total_bone_count, 1);
        assert_eq!(motion.initial_poses[0].value_bits, 0.5f32.to_bits());
        assert_eq!(motion.forward_curves[0].key_count, 2);
        assert_eq!(motion.keys, [[1, 0, 0, 0], [5, 1, 0, 0]]);
        assert_eq!(motion.time_bits, [0, 30f32.to_bits()]);
        assert_eq!(motion.value_bits, [0, 1f32.to_bits()]);
        assert_eq!(motion.root.bytes.len(), 84);
        assert_eq!(motion.joints.bytes.len(), 4);
        assert_eq!(motion.constraints.bytes.len(), 0);
        assert_eq!(motion.extra_size, 0);
    }
    #[test]
    fn rejects_bad_types_counts_ranges_and_curve_key_spans() {
        let good = fixture();
        for len in [0, 143, 160, 319, good.len() - 1] {
            assert!(read_interpolated(&good[..len]).is_err());
        }
        for (offset, value) in [
            (0x90, 1u32),
            (0xa0, 0x10002),
            (0xa0 + 24, 262145),
            (0xa0 + 20, u32::MAX),
            (0xa0 + 52, 191),
        ] {
            let mut bytes = good.clone();
            bytes[offset..offset + 4].copy_from_slice(&value.to_le_bytes());
            assert!(read_interpolated(&bytes).is_err(), "field {offset}");
        }
        let mut bad = good;
        bad[0x90 + 188..0x90 + 190].copy_from_slice(&2i16.to_le_bytes());
        assert!(read_interpolated(&bad).is_err());
    }
    #[test]
    fn retains_key_alignment_bytes_without_decoding_a_partial_key() {
        let mut bytes = fixture();
        // Move the start two bytes earlier and retain both complete records.
        bytes.copy_within(0x90 + 192..0x90 + 208, 0x90 + 190);
        bytes[0xa0 + 48..0xa0 + 52].copy_from_slice(&190u32.to_le_bytes());
        bytes[0x90 + 206..0x90 + 208].copy_from_slice(&[0xa5, 0x5a]);
        let motion = read_interpolated(&bytes).unwrap();
        assert_eq!(motion.keys, [[1, 0, 0, 0], [5, 1, 0, 0]]);
        assert_eq!(motion.key_tail_bytes, [0xa5, 0x5a]);
    }
    fn sample_bone() -> crate::model_skeleton::BoneRecord {
        crate::model_skeleton::BoneRecord {
            header_i32: [0, -1, 0, 0],
            header_i16: [0, 0, -1, 0, 0, 0, 0, 0],
            transform_bits: [1., 1., 1., 0., 0., 0., 0., 0., 2., 3., 4., 0.].map(f32::to_bits),
        }
    }
    #[test]
    fn explicit_samples_preserve_public_linear_wrap_clamping_and_flags() {
        let mut m = read_interpolated(&fixture()).unwrap();
        m.forward_curves[0].channel = 0xc6;
        let bones = [sample_bone()];
        assert_eq!(
            m.sample_fk(&bones, 15).unwrap()[0].translation,
            [0.5, 3., 4.]
        );
        assert_eq!(
            m.sample_fk(&bones, 60).unwrap()[0].translation,
            [2., 3., 4.]
        ); // Public last-to-first extrapolation.
        m.keys[1][1] = i16::MAX;
        assert_eq!(m.sample_fk(&bones, 15).unwrap()[0].translation[0], 0.5);
        m.forward_curves[0].channel = 0;
        assert_eq!(m.sample_fk(&bones, 15).unwrap()[0].translation[0], 2.);
        m.forward_curves[0].channel = 6;
        m.time_bits[0] = 10f32.to_bits();
        assert_eq!(m.sample_fk(&bones, 0).unwrap()[0].translation[0], 2.);
    }
    #[test]
    fn hermite_uses_both_left_tangents_and_undefined_spans_are_rejected() {
        let mut m = read_interpolated(&fixture()).unwrap();
        m.forward_curves[0].channel = 6;
        m.keys[0] = [2, 0, 0, 1];
        m.tangent_bits = vec![2f32.to_bits(), 4f32.to_bits()];
        m.keys[1][2] = 1;
        m.keys[1][3] = 0;
        assert_eq!(
            m.sample_fk(&[sample_bone()], 15).unwrap()[0].translation[0],
            0.25
        );
        m.forward_curves[0].key_count = 1;
        assert!(m.sample_fk(&[sample_bone()], 0).is_err());
        m.keys[0][0] = 0;
        assert_eq!(
            m.sample_fk(&[sample_bone()], 15).unwrap()[0].translation[0],
            0.
        );
        m.keys[0][2] = -1;
        assert!(m.sample_fk(&[sample_bone()], 0).is_err());
        assert!(m.sample_fk(&[], 0).is_err());
        assert!(m.sample_fk(&[sample_bone()], 16_777_217).is_err());
    }
}
