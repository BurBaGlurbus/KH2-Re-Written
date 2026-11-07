//! The Keyblade's swing trail: the weapon motion's range trigger 12 marks
//! the swing, the weapon PAX's trail element (id 10, flag 0x10000, a
//! BonePos pair on bone 2) draws it, and every tick the two points of the
//! pair on the blade bone are handed to the running instance
//! (FUN_1402c5f70 → FUN_1403b54f0 build them the same way).
//!
//! The window rule is the exe's (FUN_1403effa0, see [`tick`]): one effect
//! per open window, ended when the window closes. [I] The bone length
//! (FUN_1401a7af0's skeleton field, taken from the blade mesh's extent along
//! its bone) is inferred; the element, its points and the strip are the exe's.
use super::*;
use data::object_effect::{BonePos, Pax};

/// The weapon PAX element the trail uses.
const TRAIL_ELEMENT: u16 = 10;

/// The two points of `bone` in engine world space for this tick, or `None`
/// without a weapon pose.
pub fn points(game: &Game, bone: &BonePos) -> Option<[[f32; 3]; 2]> {
    let clip = *game.poses.get(&(game.player.action, game.player.armed))?;
    let cp = game.clip_poses.get(clip)?;
    let frame = game.player.motion_frame(&game.clips) as usize;
    let blade = cp.blade.get(frame.min(cp.blade.len().checked_sub(1)?))?;
    let hand = cp.weapon.get(frame.min(cp.weapon.len().checked_sub(1)?))?;
    let length = game.trail_length?;
    let at = Vec3::from_array(game.player.position);
    let turn = Quat::from_rotation_y(game.player.yaw);
    let to_scene = |ratio: f32| {
        let p = at + turn * (*hand * *blade).transform_point3(Vec3::X * length * ratio);
        to_world(p.to_array())
    };
    Some([to_scene(bone.ratio_a), to_scene(bone.ratio_b)])
}

/// The blade bone's length: the mesh extent along the bone's x axis divided
/// by the trail element's far ratio (3.7 for the Keyblades).
fn calibrate(game: &Game, far_ratio: f32) -> Option<f32> {
    let clip = *game.poses.get(&(game.player.action, game.player.armed))?;
    let cp = game.clip_poses.get(clip)?;
    let blade = cp.blade.first()?;
    let verts = cp.weapon_frames.first()?;
    let inverse = blade.inverse();
    let (mut lo, mut hi) = (f32::MAX, f32::MIN);
    for v in verts {
        let x = inverse.transform_point3(*v).x;
        lo = lo.min(x);
        hi = hi.max(x);
    }
    if lo > hi || far_ratio == 0. {
        return None;
    }
    // The ratios are negative: the tip lies on the bone's -x side.
    Some(lo.abs().max(hi.abs()) / far_ratio.abs())
}

/// The effect owner id of the trail's instance.
pub const OWNER: u8 = 0xfc;

/// FUN_1403effa0, the weapon object's per-frame trail routine: with no live
/// trail effect (`obj+0xc00` clear), an open range-12 window (`obj+0x18c`
/// bit 11) starts element 10 once; while the effect lives it stays as long as
/// the window holds, and when the window closes the effect is ended
/// (FUN_1402be8e0) and the handle cleared, so the next window starts a new
/// one. [I] FUN_1403ba840's blocking conditions (`obj+0x124 & 0x180`, an event
/// mode, a menu) are taken as clear.
pub fn tick(game: &mut Game, pax: Option<&Arc<Pax>>) {
    let Some(pax) = pax else { return };
    let Some(element) = pax.by_id(TRAIL_ELEMENT) else {
        return;
    };
    let window = window_open(game);
    if window && game.trail_length.is_none() {
        game.trail_length = calibrate(game, element.bone_pos.ratio_b);
        let tick = game.tick;
        let length = game.trail_length;
        game.event(serde_json::json!({"event":"weapon_trail_length","tick":tick,"length":length}));
    }
    if !game.trail_live {
        if window {
            game.fx_queue.push(drive_fx::Placed {
                req: drive_fx::FxRequest {
                    pax: pax.clone(),
                    id: TRAIL_ELEMENT,
                    owner: OWNER,
                },
                anchor: drive_fx::Anchor::Weapon,
            });
            game.trail_live = true;
        }
    } else if !window {
        game.fx_release.push(OWNER);
        game.trail_live = false;
    }
}

/// Whether the weapon motion's range 12 holds at this frame.
fn window_open(game: &Game) -> bool {
    if !game.player.armed {
        return false;
    }
    let Some(&clip) = game.poses.get(&(game.player.action, game.player.armed)) else {
        return false;
    };
    let frame = game.player.motion_frame(&game.clips);
    game.clip_poses[clip]
        .trails
        .iter()
        .any(|&(start, end)| start <= frame && frame <= end)
}
