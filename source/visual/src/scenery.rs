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

/// Per-tick vertex positions of one looping motion.
pub struct BobAnim {
    pub frames: Vec<Vec<Vec3>>,
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

fn build_anim(model: &InspectionModel, m: &InterpolatedMotion) -> Result<BobAnim> {
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
    let frames = crate::par::map_indexed(ticks, |tick| {
        let (pose, _) = model
            .skinning
            .sample_evaluated(m, frame_start + tick as f32 / scale)?;
        Ok::<_, std::io::Error>(
            pose.position_bits
                .iter()
                .map(|p| Vec3::from_array(p.map(f32::from_bits)))
                .collect(),
        )
    })?;
    Ok(BobAnim { frames })
}

/// Reads every BOB model and the placement table of a map archive.
pub fn load(archive: &[u8]) -> Result<LoadedBobs> {
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
        let bank_arg = bank.and_then(|(b, off)| b.map(|b| (b, off, 0usize)));
        let Ok(model) = model_inspection::model_from_payload(payload, bank_arg) else {
            // Keep indices aligned with the placement table.
            models.push(None);
            continue;
        };
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
                    && let Ok(a) = build_anim(&model, &motion)
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
