//! The camp's party members other than Sora (Tz::MenuObj slots 1 and 2,
//! camp-03): the party slot's MENU_FRIEND object from table `0x1405babf0`
//! ({PLAYER 0x236, MENU_FRIEND_1 0x319, MENU_FRIEND_2 0x31a, MENU_FRIEND_3
//! 0x3ee}), its own model and set, posed on motion 0x8b variant 2 (variants
//! 3, 1, 0 when that slot is empty; `FUN_1403e6230`) and walked in place one
//! tick per camp frame like Sora's (`FUN_1403001b0`).
use super::*;
use data::path_lookup::Library;

/// The MENU_FRIEND objects by party slot (slot 0 is the player).
const OBJECTS: [u32; 3] = [0x319, 0x31a, 0x3ee];
/// The camp walk's motion id (`FUN_1402feb20(…, 0x8b, 2, …)`).
const MOTION: u16 = 0x8b;

pub struct MenuFriend {
    pub model: model_inspection::InspectionModel,
    /// The vertices at each tick 0..=length, root motion kept (camp-03 §7).
    pub frames: Vec<Vec<Vec3>>,
    /// The motion's length in ticks; the walk runs 1..=length.
    pub length: f32,
}

/// Party slot `slot`'s (1-based) MENU_FRIEND object, read from the install.
pub fn load(
    source: &ProbeSource,
    library: &mut Library,
    objects: &[objentry::ObjectEntry],
    slot: usize,
) -> Result<MenuFriend, Box<dyn std::error::Error>> {
    let id = *OBJECTS
        .get(slot.wrapping_sub(1))
        .ok_or("no MENU_FRIEND object for this party slot")?;
    let entry = objects
        .iter()
        .find(|o| o.object_id == id)
        .ok_or_else(|| format!("objentry {id:#x} absent"))?;
    let name = String::from_utf8(entry.model_name.clone())?;
    let set = String::from_utf8(entry.animation_name.clone())?;
    let found = library
        .find(&format!("obj/{name}.mdlx"))
        .ok_or_else(|| format!("obj/{name}.mdlx is not installed"))?;
    let mut models =
        model_inspection::read_model_at(source, found.index, found.ordinal, found.name_hash)?;
    if models.is_empty() {
        return Err(format!("obj/{name}.mdlx has no mesh").into());
    }
    let model = models.remove(0);
    let found = library
        .find(&format!("obj/{set}"))
        .ok_or_else(|| format!("obj/{set} is not installed"))?;
    // Field, no weapon: the chooser's order 2, 3, 1, 0 is FUN_1403e6230's
    // variant 2 followed by 3, 2, 1, 0.
    let selection = motion::MotionSetSelection {
        index: found.index.to_string(),
        ordinal: found.ordinal,
        name_hash: found.name_hash,
        battle: false,
        has_weapon: false,
    };
    let selected = motion::read_motion_ids_optional(source, &selection, &[MOTION])?
        .pop()
        .flatten()
        .ok_or_else(|| format!("obj/{set} has no motion {MOTION:#x}"))?;
    let (start, end, fps, _) = selected.motion.frame_window()?;
    if !(1. ..=60.).contains(&fps) || !(0. ..end).contains(&start) {
        return Err(format!("obj/{set} motion {MOTION:#x} frame window {start}..{end}").into());
    }
    // The keys run at 60 a second (one per tick, as the party's clips).
    let length = ((end - start) * 60. / fps).round();
    let key0 = (start * 60. / fps).round();
    let frames = (0..=length as usize)
        .map(|k| {
            let (pose, _) = model
                .skinning
                .sample_evaluated(&selected.motion, key0 + k as f32)?;
            Ok(pose
                .position_bits
                .iter()
                .map(|p| Vec3::from_array(p.map(f32::from_bits)))
                .collect())
        })
        .collect::<Result<_, std::io::Error>>()?;
    Ok(MenuFriend {
        model,
        frames,
        length,
    })
}

/// The walk time after `steps` camp updates (FUN_1403001b0: 1..=length,
/// 0 before the first update).
pub fn time(steps: u32, length: f32) -> usize {
    match (steps, length as u32) {
        (0, _) | (_, 0) => 0,
        (n, len) => ((n - 1) % len + 1) as usize,
    }
}

/// Camp updates between two readings of Sora's walk time (which wraps at
/// `pause_menu::MODEL_LENGTH`); the presenter may skip camp updates.
pub fn steps_between(last: Option<f32>, now: f32) -> u32 {
    match last {
        None => now as u32,
        Some(last) => {
            let len = pause_menu::MODEL_LENGTH;
            ((now - last).rem_euclid(len)) as u32
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_walk_runs_one_to_its_length() {
        assert_eq!(time(0, 50.), 0);
        assert_eq!(time(1, 50.), 1);
        assert_eq!(time(50, 50.), 50);
        assert_eq!(time(51, 50.), 1);
        assert_eq!(time(7, 0.), 0);
    }

    #[test]
    fn steps_follow_sora_walk_across_its_wrap() {
        assert_eq!(steps_between(None, 0.), 0);
        assert_eq!(steps_between(None, 1.), 1);
        assert_eq!(steps_between(Some(1.), 1.), 0);
        assert_eq!(steps_between(Some(1.), 3.), 2);
        assert_eq!(steps_between(Some(64.), 1.), 1);
        assert_eq!(steps_between(Some(63.), 2.), 3);
    }
}
