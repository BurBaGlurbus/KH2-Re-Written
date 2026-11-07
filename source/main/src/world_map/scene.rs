//! The files of the world map events' character scenes (world_map::widgets)
//! for `kh2fm wm`: each event's archive (op 0x101's
//! `libretto/<lang>/wm<event>.bar`), whose entries are motion archives
//! (ANB, BAR type 0x11) and "ORIGIN" voice stubs (type 0x30), and the
//! models of the objects the scripts put in widgets.
use data::{bar, motion, motion::InterpolatedMotion, motion_trigger};
use std::{collections::HashMap, sync::Arc};
use world_map::{events, widgets::Clip};

type Result<T> = std::result::Result<T, Box<dyn std::error::Error>>;

/// Op 0x101's language: "fm" for languages 0 and 7, "us" otherwise (the
/// selected install is English).
pub const LANGUAGE: &str = "us";

/// Motion trigger 8 (LAB_1405359b0): sound `bank × 1000 + number`.
const SOUND_TRIGGER: u8 = 8;

/// One event's archive, by entry index.
#[derive(Default)]
pub struct EventArchive {
    /// The motion of each ANB entry (its type-9 entry) and what the widget
    /// player needs of it.
    pub motions: Vec<Option<Arc<InterpolatedMotion>>>,
    pub clips: Vec<Option<Clip>>,
    /// The installed voice file of each ORIGIN voice stub.
    pub voices: HashMap<u32, String>,
}

/// The widget player's view of a motion: its length in ticks (vtable +8,
/// header word 1 of an interpolated motion), the loop point (+0x30,
/// `(frameReturn − frameStart) × 60 / fps`) and the sounds of its frame
/// triggers (the ANB's type-16 entry).
fn clip(m: &InterpolatedMotion, triggers: &[motion_trigger::SelectedTriggers]) -> Clip {
    let (start, end, fps, ret) = m.frame_window().unwrap_or((0., 1., 30., 0.));
    let scale = 60. / fps.max(1.);
    let length = match m.interpolated_header_words.get(1) {
        Some(&w) if m.raw.is_none() => w as i32 as f32,
        _ => (end - start) * scale,
    };
    let sounds = triggers
        .iter()
        .flat_map(|t| t.records.frames.iter())
        .filter(|f| f.trigger == SOUND_TRIGGER && f.parameters.len() >= 2)
        .map(|f| {
            (
                f.frame as f32,
                f.parameters[0] as u16 as u32 * 1000 + f.parameters[1] as u16 as u32,
            )
        })
        .collect();
    Clip {
        length,
        loop_start: (ret - start) * scale,
        sounds,
    }
}

/// Reads an event's archive. Entries that are neither motions nor voice
/// stubs stay empty; a motion that does not read is reported and left out.
pub fn read_archive(bytes: &[u8], log: &mut Vec<serde_json::Value>) -> Result<EventArchive> {
    let table = bar::read_bar(bytes)?;
    let mut out = EventArchive::default();
    for (i, e) in table.entries.iter().enumerate() {
        let data = bytes.get(e.offset as usize..e.offset as usize + e.size as usize);
        let (mut m, mut c) = (None, None);
        match (e.entry_type, data) {
            (0x11, Some(anb)) => match motion::read_anb(anb, i) {
                Ok(sel) => {
                    c = Some(clip(&sel.motion, &sel.triggers));
                    m = Some(Arc::new(sel.motion));
                }
                Err(err) => log.push(serde_json::json!({"event":"widget_motion_unread",
                    "entry":i,"error":err.to_string()})),
            },
            (0x1f | 0x22 | 0x30, Some(stub)) => {
                if let Some(file) =
                    events::origin_path(stub).and_then(|p| events::voice_file(&p, LANGUAGE))
                {
                    out.voices.insert(i as u32, file);
                }
            }
            _ => {}
        }
        out.motions.push(m);
        out.clips.push(c);
    }
    Ok(out)
}

/// The objects a script puts in each slot (op 0x102).
pub fn widget_objects(script: &[events::Op]) -> Vec<(usize, u32)> {
    let mut out = Vec::new();
    for op in script {
        if let events::Op::Widget(slot, object) = *op
            && (slot as usize) < world_map::widgets::SLOTS
            && !out.contains(&(slot as usize, object))
        {
            out.push((slot as usize, object));
        }
    }
    out
}

/// A motion's key time `ticks` after its start (the playing entry's clock,
/// which the widget player loops itself).
pub fn key_time(m: &InterpolatedMotion, ticks: f32) -> f32 {
    let (start, _, fps, _) = m.frame_window().unwrap_or((0., 1., 30., 0.));
    start + ticks * fps / 60.
}

/// The exe's row-vector, Y-down widget matrix as a scene transform: the
/// scene is the engine with Y and Z negated, in model space as in the
/// world, so `B = S · Mᵀ · S` with S = diag(1, −1, −1). [I: the field
/// actors' π − yaw turn would face every character away from the widget
/// camera; with the plain flip Chip and Dale face it at the hangar's
/// console and Sora, Donald and Goofy face it from their seats, as the
/// scenes are staged.]
pub fn scene_matrix(m: &world_map::widgets::Matrix) -> [[f32; 4]; 4] {
    let s = [1., -1., -1.];
    // Column-major for Mat4::from_cols_array_2d: column j holds the scene
    // image of model axis j.
    let mut cols = [[0.; 4]; 4];
    for (j, col) in cols.iter_mut().enumerate().take(3) {
        for i in 0..3 {
            col[i] = s[i] * m[j][i] * s[j];
        }
    }
    cols[3] = [m[3][0] * s[0], m[3][1] * s[1], m[3][2] * s[2], 1.];
    cols
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn widget_matrices_flip_y_and_z() {
        // An engine turn by Y about the (downward) Y axis is a scene turn by
        // −Y about +Y.
        let yaw = 0.7f32;
        let mut local = world_map::widgets::rotate_xyz(world_map::widgets::IDENTITY, [0., yaw, 0.]);
        local[3] = [10., 20., 30., 1.];
        let b = scene_matrix(&local);
        let want = bevy::math::Mat3::from_rotation_y(-yaw);
        for j in 0..3 {
            for i in 0..3 {
                assert!((b[j][i] - want.col(j)[i]).abs() < 1e-5, "{b:?}");
            }
        }
        assert_eq!(b[3], [10., -20., -30., 1.]);
    }
}
