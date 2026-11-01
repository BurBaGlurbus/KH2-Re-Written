//! Map objects (BOBs): the type-4/7/30 'BOB' triples of a map archive
//! placed by its type-21 'out' records (draw-01 §6, area-01 §2.3).
//!
//! World matrix [O FUN_14016dca0]: `v' = (v·S)·Rx·Ry·Rz + T` with row
//! vectors. A BOB's motion is an interpolated type-9 motion advanced one
//! key-time unit per 60 Hz tick of its own frame rate.
use bevy::prelude::*;
use data::{
    bar,
    model_inspection::{self, InspectionModel},
    motion::{self, InterpolatedMotion},
};

type Result<T> = std::result::Result<T, Box<dyn std::error::Error>>;

/// One looping motion, evaluated and skinned when shown as the original does.
pub struct BobAnim {
    motion: InterpolatedMotion,
    ticks: usize,
    frame_start: f32,
    scale: f32,
}

impl BobAnim {
    pub fn len(&self) -> usize {
        self.ticks
    }

    pub fn is_empty(&self) -> bool {
        self.ticks == 0
    }

    /// The vertex positions at `tick` (wrapped into the loop).
    pub fn positions(&self, model: &InspectionModel, tick: usize) -> Vec<Vec3> {
        if self.ticks == 0 {
            return Vec::new();
        }
        let frame = self.frame_start + (tick % self.ticks) as f32 / self.scale;
        model
            .skinning
            .sample_evaluated(&self.motion, frame)
            .map(|(pose, _)| {
                pose.position_bits
                    .iter()
                    .map(|p| Vec3::from_array(p.map(f32::from_bits)))
                    .collect()
            })
            .unwrap_or_default()
    }
}

/// The poses of a frame's motion parts, so the parts of one object at one
/// tick skin it once.
#[derive(Default)]
pub struct PoseCache(std::collections::HashMap<(usize, i32, usize), Vec<Vec3>>);

impl PoseCache {
    pub fn positions(&mut self, bob: &Bob, placement: &Placement, tick: usize) -> &[Vec3] {
        let key = (placement.model, placement.motion, tick);
        self.0
            .entry(key)
            .or_insert_with(|| {
                bob.anims
                    .get(placement.motion as usize)
                    .map(|a| a.positions(&bob.model, tick))
                    .unwrap_or_default()
            })
            .as_slice()
    }
}

pub struct Bob {
    pub model: InspectionModel,
    /// One animation per type-9 entry of the BOB's motion BAR.
    pub anims: Vec<BobAnim>,
}

/// An 'out' record (0x68 bytes).
#[derive(Clone, Debug)]
pub struct Placement {
    /// Index into the BOB triples.
    pub model: usize,
    /// Scene-space translation (the map's model space).
    pub translation: Vec3,
    pub rotation: [f32; 3],
    pub scale: Vec3,
    /// Background group bit gating creation.
    pub group: u8,
    /// Motion index in the BOB's motion BAR, −1 = none.
    pub motion: i32,
    /// Start offset in ticks (`rec +0x30 × 2.0`).
    pub start: f32,
    pub flags: u32,
}

impl Placement {
    /// `(v·S)·Rx·Ry·Rz + T` as a scene transform.
    pub fn transform(&self) -> Transform {
        let q = Quat::from_rotation_z(self.rotation[2])
            * Quat::from_rotation_y(self.rotation[1])
            * Quat::from_rotation_x(self.rotation[0]);
        Transform {
            translation: self.translation,
            rotation: q,
            scale: self.scale,
        }
    }
}

pub struct LoadedBobs {
    /// Indexed like the 'BOB' type-4 entries; `None` where the model did
    /// not decode.
    pub models: Vec<Option<Bob>>,
    pub placements: Vec<Placement>,
}

fn build_anim(model: &InspectionModel, m: InterpolatedMotion) -> Result<BobAnim> {
    let (frame_start, frame_end, fps, _) = m.frame_window()?;
    if !(1. ..=60.).contains(&fps) || frame_end <= frame_start {
        return Err("BOB motion frame window outside profile".into());
    }
    // Motion length in ticks (header word 1).
    let ticks = m
        .interpolated_header_words
        .get(1)
        .map_or(0, |&t| t as usize)
        .clamp(1, 4096);
    let scale = 60. / fps;
    // The ticks are independent: evaluated on several threads, in order.
    // A motion that cannot be sampled is left out, as before.
    for tick in [0, ticks - 1] {
        model
            .skinning
            .sample_evaluated(&m, frame_start + tick as f32 / scale)?;
    }
    Ok(BobAnim {
        motion: m,
        ticks,
        frame_start,
        scale,
    })
}

/// Places the model's region-sized remaster images in their pages (the
/// regions from each environment's CLAMP register, as the field map's).
fn fit_hd_regions(model: &mut InspectionModel, bank: &[u8]) {
    let Some(set) = model.textures.as_mut().and_then(std::sync::Arc::get_mut) else {
        return;
    };
    let Ok(layout) = data::model_texture::read_layout(bank) else {
        return;
    };
    let crops: Vec<_> = layout
        .environments
        .iter()
        .enumerate()
        .map(|(ordinal, env)| {
            let (w, h) = (1u32 << env.tex0[3], 1u32 << env.tex0[4]);
            let a = env.address;
            let region = crate::room::EnvRegion::new(a[0], a[1], [a[2], a[3], a[4], a[5]], [w, h]);
            (ordinal, region.crop)
        })
        .collect();
    set.fit_hd_regions(&crops);
}

/// Reads every BOB model and the placement table of a map archive, with the
/// map's remaster images for the models' textures.
/// A model's own data starts after 0x90 bytes the runtime keeps for its object.
const MODEL_DATA: usize = 0x90;

/// A level container (the data's first word -1, FUN_1401c3330): u32 offsets
/// from the data's +0x10, ending at 0, each to one level's whole model. The
/// room draws the first, which the object's level byte +0x139 selects at 0 [I:
/// its writer was not traced]; its texture numbers start at 0 (FUN_1401c3200
/// counts only earlier levels). Any other payload is itself.
fn level_payload(payload: &[u8]) -> &[u8] {
    let word = |at: usize| -> Option<u32> {
        Some(u32::from_le_bytes(
            payload
                .get(MODEL_DATA + at..MODEL_DATA + at + 4)?
                .try_into()
                .ok()?,
        ))
    };
    if word(0) != Some(u32::MAX) {
        return payload;
    }
    let Some(first) = word(0x10).filter(|&o| o != 0) else {
        return payload;
    };
    let end = word(0x14)
        .filter(|&o| o != 0)
        .map_or(payload.len(), |o| MODEL_DATA + o as usize);
    payload
        .get(MODEL_DATA + first as usize..end)
        .unwrap_or(payload)
}

/// A level container's texture bank (FUN_1401a5180: first i16 -1, u32 count at
/// +4, u32 offsets at +8): the first level's, and how far in it starts.
fn level_bank(bank: &[u8]) -> (&[u8], usize) {
    let word = |at: usize| -> Option<usize> {
        Some(u32::from_le_bytes(bank.get(at..at + 4)?.try_into().ok()?) as usize)
    };
    if bank.get(..2) == Some(&[0xff, 0xff])
        && word(4).is_some_and(|n| n > 0)
        && let Some(first) = word(8)
        && let Some(rest) = bank.get(first..)
    {
        return (rest, first);
    }
    (bank, 0)
}

pub fn load(
    archive: &[u8],
    replacements: &[data::replacement_payload::ReplacementSample],
) -> Result<LoadedBobs> {
    let table = bar::read_bar(archive)?;
    let slice = |e: &bar::BarEntry| -> Option<&[u8]> {
        archive.get(e.offset as usize..e.offset as usize + e.size as usize)
    };
    let mut models = Vec::new();
    // The triples follow each other: 4, then 7 and 30 when present.
    let entries = &table.entries;
    for (i, e) in entries.iter().enumerate() {
        if e.entry_type != 4 || e.name_bytes[..3] != *b"BOB" || e.size == 0 {
            continue;
        }
        let Some(payload) = slice(e) else { continue };
        let payload = level_payload(payload);
        // The next entries of the same name until the next type 4.
        let mut bank = None;
        let mut motions = None;
        for next in entries[i + 1..].iter() {
            if next.entry_type == 4 {
                break;
            }
            if next.name_bytes[..3] != *b"BOB" {
                continue;
            }
            match next.entry_type {
                7 if bank.is_none() => bank = Some((slice(next), next.offset as usize)),
                30 if motions.is_none() => motions = slice(next),
                _ => {}
            }
        }
        let bank_arg = bank.and_then(|(b, off)| {
            b.map(|b| {
                let (b, skip) = level_bank(b);
                (b, off + skip, 0usize)
            })
        });
        let mut model = match model_inspection::model_from_payload(payload, bank_arg, replacements)
        {
            Ok(m) => m,
            Err(error) => {
                eprintln!("Map object {i} rejected: {error}");
                // Keep indices aligned with the placement table.
                models.push(None);
                continue;
            }
        };
        if let Some((bank, ..)) = bank_arg {
            fit_hd_regions(&mut model, bank);
        }
        model.merge_parts_by_texture();
        let mut anims = Vec::new();
        if let Some(bytes) = motions
            && let Ok(inner) = bar::read_bar(bytes)
        {
            for m in inner
                .entries
                .iter()
                .filter(|m| m.entry_type == 9 && m.size > 0)
            {
                let Some(raw) = bytes.get(m.offset as usize..m.offset as usize + m.size as usize)
                else {
                    continue;
                };
                if let Ok(motion) = motion::read_interpolated(raw)
                    && let Ok(a) = build_anim(&model, motion)
                {
                    anims.push(a);
                }
            }
        }
        models.push(Some(Bob { model, anims }));
    }
    // Placement table: BAR type 21 'out' = {i32 dataOff, i32 dataSize}.
    let mut placements = Vec::new();
    if let Some(e) = table
        .entries
        .iter()
        .find(|e| e.entry_type == 21 && e.size > 0)
        && let Some(d) = slice(e)
        && d.len() >= 8
    {
        let off = i32::from_le_bytes(d[0..4].try_into().unwrap()).max(0) as usize;
        let size = i32::from_le_bytes(d[4..8].try_into().unwrap()).max(0) as usize;
        let f = |r: &[u8], o: usize| f32::from_le_bytes(r[o..o + 4].try_into().unwrap());
        let i = |r: &[u8], o: usize| i32::from_le_bytes(r[o..o + 4].try_into().unwrap());
        for r in d.get(off..off + size).unwrap_or(&[]).as_chunks::<0x68>().0 {
            placements.push(Placement {
                model: i(r, 0x24).max(0) as usize,
                translation: Vec3::new(f(r, 0), f(r, 4), f(r, 8)),
                rotation: [f(r, 0x0c), f(r, 0x10), f(r, 0x14)],
                scale: Vec3::new(f(r, 0x18), f(r, 0x1c), f(r, 0x20)),
                group: i(r, 0x28).clamp(0, 255) as u8,
                motion: i(r, 0x2c),
                start: i(r, 0x30) as f32 * 2.0,
                flags: i(r, 0x34) as u32,
            });
        }
    }
    Ok(LoadedBobs { models, placements })
}

#[cfg(test)]
mod level_tests {
    use super::{MODEL_DATA, level_bank, level_payload};

    #[test]
    fn a_level_container_gives_its_first_model() {
        let mut p = vec![0u8; MODEL_DATA];
        p.extend(u32::MAX.to_le_bytes());
        p.resize(MODEL_DATA + 0x10, 0);
        p.extend(0x20u32.to_le_bytes());
        p.extend(0x24u32.to_le_bytes());
        p.extend(0u32.to_le_bytes());
        p.resize(MODEL_DATA + 0x20, 0);
        p.extend([1, 2, 3, 4, 5, 6]);
        assert_eq!(level_payload(&p), &[1, 2, 3, 4]);
        let plain = vec![7u8; MODEL_DATA + 8];
        assert_eq!(level_payload(&plain), &plain[..]);
    }

    #[test]
    fn a_level_container_gives_its_first_bank() {
        let mut b = vec![0xff, 0xff, 0, 0];
        b.extend(2u32.to_le_bytes());
        b.extend(0x10u32.to_le_bytes());
        b.extend(0x12u32.to_le_bytes());
        b.extend([9, 8, 7]);
        assert_eq!(level_bank(&b), (&[9u8, 8, 7][..], 0x10));
        assert_eq!(level_bank(&[1, 2]), (&[1u8, 2][..], 0));
    }
}
